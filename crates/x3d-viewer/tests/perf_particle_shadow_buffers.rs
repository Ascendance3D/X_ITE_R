//! GPU-buffer allocation of wide LINE particle shadows (lane XITE_R_NEXT_LANE particle shadow performance).
//!
//! Not a regression test: `cargo test --release -p x3d-viewer --test perf_particle_shadow_buffers -- --ignored
//! --nocapture` prints, for 100, 1,000 and 10,000 LINE particles (gl.lineWidth 24) in one and two layers lit by a
//! shadow-casting DirectionalLight, per measured frame: the draw calls, geometry uploads and vertex buffers created, the
//! frame time (step, build_frame, render with readback) and its parts, the GPU time of the main render pass (timestamp
//! queries), the CPU cores busy (process CPU time / wall time), the wgpu allocator's allocated bytes, and the least and
//! most particles alive in a measured frame.
//!
//! [`wide_line_particle_shadows_reuse_vertex_buffers`] is the regression test: it runs without `--ignored` (and is
//! skipped without an adapter).

use std::time::Instant;
use x3d_viewer::viewer::{ORACLE_PAGE_ATTRIBUTES, Viewer3d};

/// X_ITE emits maxParticles / particleLifetime particles per second (ParticleSystem.js animateParticles): the systems
/// are full after 4 s, 120 frames, before the measured frames start.
const WARMUP: usize = 150;
const FRAMES: usize = 300;
const DT: f64 = 1.0 / 30.0;
const T0: f64 = 946_684_800.0;

fn layer(ps: &str) -> String {
    format!(
        "NavigationInfo {{ headlight FALSE }}
Viewpoint {{ position 0 1.6 3.2 orientation 1 0 0 -0.45 }}
DirectionalLight {{ direction 0.15 -1 -0.3 ambientIntensity 0.3 shadows TRUE shadowIntensity 1 }}
Transform {{ translation 0 -0.05 0 children Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0.8 0.8 0.8 }} }} geometry Box {{ size 4 0.1 4 }} }} }}
{ps}"
    )
}

fn world(particles: usize, layers: usize) -> String {
    let ps = format!(
        "DEF PS ParticleSystem {{
  geometryType \"LINE\"
  maxParticles {particles}
  particleLifetime 4
  lifetimeVariation 0
  particleSize 0.1 0.4
  emitter PointEmitter {{ position 0 0.6 0 direction 0 1 0 speed 0.4 variation 0.9 }}
  appearance Appearance {{ material Material {{ emissiveColor 1 0.5 0.2 }} lineProperties LineProperties {{ linewidthScaleFactor 24 }} }}
}}"
    );
    let head = "#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:3\nCOMPONENT Shape:5\nCOMPONENT Layering:1\n\
                Background { skyColor 0.1 0.1 0.2 }\n";

    match layers {
        1 => format!("{head}{}\n", layer(&ps)),
        _ => format!(
            "{head}LayerSet {{ activeLayer 1 order [1 2] layers [
Layer {{ viewport Viewport {{ clipBoundary [0 0.5 0 1] }} children [
{}
] }}
Layer {{ viewport Viewport {{ clipBoundary [0.5 1 0 1] }} children [
{}
] }}
] }}
",
            layer(&ps),
            layer("USE PS")
        ),
    }
}

fn load(path: &std::path::Path, gpu: &mut x3d_render::gpu::Gpu) -> Viewer3d {
    let mut v = Viewer3d::load_attributes(
        path,
        None,
        T0,
        512,
        512,
        Some(1.0 / DT),
        false,
        ORACLE_PAGE_ATTRIBUTES,
    )
    .unwrap();
    v.frame_rate = 1.0 / DT;
    v.rendering_properties().apply(&mut gpu.renderer);
    v
}

/// Frame `k`: step, build, render (with readback). Returns their milliseconds.
fn render(v: &mut Viewer3d, gpu: &mut x3d_render::gpu::Gpu, k: usize) -> [f64; 3] {
    let t = Instant::now();
    v.step(T0 + (k + 1) as f64 * DT);
    let a = t.elapsed().as_secs_f64();
    let frame = v.build_frame();
    let b = t.elapsed().as_secs_f64();
    let (w, h) = v.framebuffer_size();
    gpu.render(&frame, w, h);
    let c = t.elapsed().as_secs_f64();

    [a * 1e3, (b - a) * 1e3, (c - b) * 1e3]
}

/// The CPU seconds of this process so far (utime + stime of /proc/self/stat).
fn cpu_seconds() -> f64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let fields: Vec<&str> = stat
        .rsplit_once(')')
        .map_or(vec![], |(_, rest)| rest.split_whitespace().collect());
    let ticks = |i: usize| {
        fields
            .get(i)
            .and_then(|f| f.parse::<f64>().ok())
            .unwrap_or(0.0)
    };

    (ticks(11) + ticks(12)) / 100.0
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len().max(1) as f64
}

fn percentile(v: &[f64], p: usize) -> f64 {
    let mut v = v.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[(v.len() * p / 100).min(v.len() - 1)]
}

#[test]
#[ignore]
fn wide_line_particle_shadow_buffers() {
    if let Err(e) = x3d_render::gpu::Gpu::new() {
        return x3d_evidence::no_gpu(format_args!("skipped: no GPU ({e})"));
    }
    let dir = std::env::temp_dir().join(format!("perf-particle-shadow-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    println!(
        "particles layers | alive min max | draws | uploads created | frame ms mean p95 | step build render | GPU ms | CPU cores | MiB end peak"
    );

    for particles in [100usize, 1_000, 10_000] {
        for layers in [1usize, 2] {
            // A fresh device per case: its allocator report holds this case's resources only.
            let mut gpu = x3d_render::gpu::Gpu::new().unwrap();
            let path = dir.join(format!("p{particles}-l{layers}.x3dv"));
            std::fs::write(&path, world(particles, layers)).unwrap();

            let mut v = load(&path, &mut gpu);

            let ps = v.rt.scene.contexts[x3d_scene::ROOT]
                .named_nodes
                .get("PS")
                .expect("PS");
            let (mut uploads, mut created, mut draws, mut times, mut gpu_ms) =
                (vec![], vec![], vec![], vec![], vec![]);
            let (mut phases, mut alive, mut peak, mut end) =
                ([0.0; 3], (usize::MAX, 0), 0u64, 0u64);
            let (mut wall, mut cpu) = (0.0, 0.0);

            for k in 0..WARMUP + FRAMES {
                let (start, cpu_start) = (Instant::now(), cpu_seconds());
                let p = render(&mut v, &mut gpu, k);

                if k >= WARMUP {
                    wall += start.elapsed().as_secs_f64();
                    cpu += cpu_seconds() - cpu_start;
                    times.push(p.iter().sum::<f64>());
                    (0..3).for_each(|i| phases[i] += p[i] / FRAMES as f64);
                    gpu_ms.extend(gpu.renderer.gpu_time_ms(true));
                    uploads.push(gpu.renderer.stats.geometry_uploads as f64);
                    created.push(gpu.renderer.stats.vertex_buffers_created as f64);
                    draws.push(gpu.renderer.stats.draw_calls as f64);
                    let n = v.rt.particle_sim(ps).map_or(0, |s| s.num_particles);
                    alive = (alive.0.min(n), alive.1.max(n));

                    end = gpu
                        .device
                        .generate_allocator_report()
                        .map_or(0, |r| r.total_allocated_bytes);
                    peak = peak.max(end);
                }
            }

            println!(
                "{particles:>9} {layers:>6} | {:>5} {:>5} | {:>5.0} | {:>7.2} {:>7.2} | {:>6.2} {:>6.2} | {:>5.2} {:>5.2} {:>6.2} | {:>6.2} | {:>9.2} | {:>5.1} {:>5.1}",
                alive.0,
                alive.1,
                mean(&draws),
                mean(&uploads),
                mean(&created),
                mean(&times),
                percentile(&times, 95),
                phases[0],
                phases[1],
                phases[2],
                mean(&gpu_ms),
                cpu / wall,
                end as f64 / 1048576.0,
                peak as f64 / 1048576.0,
            );
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
}

/// The wide lines of every frame are new geometries. Once the system is full, their vertex buffers are free buffers
/// rewritten: neither the buffers created nor the geometry cache grow with the frames (before the reuse, each new
/// geometry created a buffer and the cache kept it for KEEP_FRAMES).
#[test]
fn wide_line_particle_shadows_reuse_vertex_buffers() {
    let mut gpu = match x3d_render::gpu::Gpu::new() {
        Ok(g) => g,
        Err(e) => return x3d_evidence::no_gpu(format_args!("skipped: no GPU ({e})")),
    };
    let dir = std::env::temp_dir().join(format!("reuse-particle-shadow-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    for layers in [1usize, 2] {
        let path = dir.join(format!("p1000-l{layers}.x3dv"));
        std::fs::write(&path, world(1_000, layers)).unwrap();
        let mut v = load(&path, &mut gpu);
        let (mut created, mut cached) = (0, 0);

        for k in 0..WARMUP + 90 {
            render(&mut v, &mut gpu, k);

            if k >= WARMUP {
                created += gpu.renderer.stats.vertex_buffers_created;
                cached = cached.max(gpu.renderer.resource_counts()[0]);
            }
        }

        // Measured (one, two layers): 1 and 5 buffers, at most 3 and 7 geometries; before the reuse, 25 and 115
        // buffers, 27 and 179 geometries.
        assert!(
            created <= 10,
            "{layers} layer(s): {created} vertex buffers created in 90 frames"
        );
        assert!(
            cached <= 16,
            "{layers} layer(s): {cached} geometries cached"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}
