//! GPU-buffer allocation of wide LINE particle shadows (lane XITE_R_NEXT_LANE particle shadow performance).
//!
//! Not a regression test: `cargo test --release -p x3d-viewer --test perf_particle_shadow_buffers -- --ignored
//! --nocapture` prints, for 100, 1,000 and 10,000 LINE particles (gl.lineWidth 24) in one and two layers lit by a
//! shadow-casting DirectionalLight, per measured frame: the geometry uploads, the vertex buffers created for them, the wgpu
//! allocator's allocated bytes and the frame time (step, build_frame, render with readback).
//!
//! [`wide_line_particle_shadows_reuse_vertex_buffers`] is the regression test: it runs without `--ignored` (and is
//! skipped without an adapter).

use std::time::Instant;
use x3d_viewer::viewer::{ORACLE_PAGE_ATTRIBUTES, Viewer3d};

const WARMUP: usize = 60;
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
  emissionRate {}
  particleLifetime 4
  lifetimeVariation 0
  particleSize 0.1 0.4
  emitter PointEmitter {{ position 0 0.6 0 direction 0 1 0 speed 0.4 variation 0.9 }}
  appearance Appearance {{ material Material {{ emissiveColor 1 0.5 0.2 }} lineProperties LineProperties {{ linewidthScaleFactor 24 }} }}
}}",
        particles as f64 / 2.0
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

/// Frame `k`: step, build, render (with readback).
fn render(v: &mut Viewer3d, gpu: &mut x3d_render::gpu::Gpu, k: usize) {
    v.step(T0 + (k + 1) as f64 * DT);
    let frame = v.build_frame();
    let (w, h) = v.framebuffer_size();
    gpu.render(&frame, w, h);
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
        return eprintln!("skipped: no GPU ({e})");
    }
    let dir = std::env::temp_dir().join(format!("perf-particle-shadow-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    println!(
        "particles layers | drawn | uploads/frame | buffers created/frame | allocated MiB (end, peak) | frame ms (mean, p50, p95)"
    );

    for particles in [100usize, 1_000, 10_000] {
        for layers in [1usize, 2] {
            // A fresh device per case: its allocator report holds this case's resources only.
            let mut gpu = x3d_render::gpu::Gpu::new().unwrap();
            let path = dir.join(format!("p{particles}-l{layers}.x3dv"));
            std::fs::write(&path, world(particles, layers)).unwrap();

            let mut v = load(&path, &mut gpu);

            let (mut uploads, mut created, mut times, mut peak, mut end) =
                (vec![], vec![], vec![], 0u64, 0u64);

            for k in 0..WARMUP + FRAMES {
                let start = Instant::now();
                render(&mut v, &mut gpu, k);
                let ms = start.elapsed().as_secs_f64() * 1e3;

                if k >= WARMUP {
                    times.push(ms);
                    uploads.push(gpu.renderer.stats.geometry_uploads as f64);
                    created.push(gpu.renderer.stats.vertex_buffers_created as f64);

                    end = gpu
                        .device
                        .generate_allocator_report()
                        .map_or(0, |r| r.total_allocated_bytes);
                    peak = peak.max(end);
                }
            }

            let ps = v.rt.scene.contexts[x3d_scene::ROOT]
                .named_nodes
                .get("PS")
                .expect("PS");
            let drawn = v.rt.particle_sim(ps).map_or(0, |s| s.num_particles);
            let mean = times.iter().sum::<f64>() / times.len() as f64;

            println!(
                "{particles:>9} {layers:>6} | {drawn:>5} | {:>13.2} | {:>21.2} | {:>8.1} {:>8.1} | {mean:>6.2} {:>6.2} {:>6.2}",
                uploads.iter().sum::<f64>() / uploads.len() as f64,
                created.iter().sum::<f64>() / created.len() as f64,
                end as f64 / 1048576.0,
                peak as f64 / 1048576.0,
                percentile(&times, 50),
                percentile(&times, 95),
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
        Err(e) => return eprintln!("skipped: no GPU ({e})"),
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
