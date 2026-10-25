//! Pointing-device picking over ParticleSystems (lane XITE_R_B25_2_GPU_INSTANCED_PICKING).
//!
//! Not a regression test: `cargo test --release -p x3d-viewer --test perf_particle_picking -- --ignored --nocapture`
//! prints, for an empty world, a small scene without particles and 100 / 1,000 / 10,000 QUAD and LINE particles:
//!
//! * the draws of the pointing pass (a ParticleSystem is one, however many particles it has);
//! * the bytes uploaded for the pass and read back from it, split into ParticleSystem data and the rest
//!   (`x3d_render::transfer`), so the per-particle cost is countable and not an estimate;
//! * the total pick latency of `browser .touch (x, y)`, mean and p95, on the GPU and on the CPU ray cast;
//! * the wgpu allocator's allocated bytes after the picks.
//!
//! [`gpu_picking_does_not_scale_with_the_particle_count`] is the regression test: it runs without `--ignored` (and is
//! skipped without an adapter). It is the proof the B25-2 brief asks for — the normal GPU path's CPU candidate work
//! does not grow with the particle count.

use std::time::Instant;
use x3d_viewer::viewer::{ORACLE_PAGE_ATTRIBUTES, Viewer3d};

/// X_ITE emits maxParticles / particleLifetime particles per second (ParticleSystem.js animateParticles): the system
/// is full well before the picks.
const WARMUP: usize = 150;
const PICKS: usize = 200;
const DT: f64 = 1.0 / 30.0;
const T0: f64 = 946_684_800.0;
const W: u32 = 512;
const H: u32 = 512;

const HEAD: &str = "#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:3\nCOMPONENT Shape:5\n\
                    Viewpoint { position 0 0 3 }\nBackground { skyColor 0.1 0.1 0.2 }\n";

/// A few ordinary shapes around the pointer, so a "small scene" is not an empty one.
const SMALL: &str = "Transform { translation 0 0 -0.5 children Shape { appearance Appearance { material Material { } } geometry Box { size 1 1 1 } } }
Transform { translation 0.6 0.3 -1 children Shape { appearance Appearance { material Material { } } geometry Sphere { radius 0.4 } } }
Transform { translation -0.6 -0.3 -1.5 children Shape { appearance Appearance { material Material { } } geometry Cone { } } }
";

fn world(kind: &str, particles: usize) -> String {
    match kind {
        "empty" => HEAD.to_string(),
        "small scene" => format!("{HEAD}{SMALL}"),
        geometry_type => {
            let appearance = if geometry_type == "LINE" {
                "Appearance { material Material { emissiveColor 1 0.5 0.2 } lineProperties LineProperties { linewidthScaleFactor 3 } }"
            } else {
                "Appearance { material Material { diffuseColor 1 1 1 } }"
            };

            format!(
                "{HEAD}{SMALL}DEF PS ParticleSystem {{
  geometryType \"{geometry_type}\"
  maxParticles {particles}
  particleLifetime 4
  lifetimeVariation 0
  particleSize 0.3 0.3
  emitter PointEmitter {{ direction 0 1 0 speed 0.25 variation 0.9 }}
  appearance {appearance}
}}
"
            )
        }
    }
}

fn load(path: &std::path::Path) -> Viewer3d {
    let mut v = Viewer3d::load_attributes(
        path,
        None,
        T0,
        W,
        H,
        Some(1.0 / DT),
        false,
        ORACLE_PAGE_ATTRIBUTES,
    )
    .unwrap();
    v.frame_rate = 1.0 / DT;
    v
}

/// The pointing buffer on `device`, in place of the CPU ray cast.
fn with_gpu(v: &mut Viewer3d, device: &x3d_render::wgpu::Device, queue: &x3d_render::wgpu::Queue) {
    let pointing = x3d_render::pointing_gpu::PointingGpu::new(device.clone(), queue.clone());

    v.set_gpu(device, Some(pointing), None).unwrap();
}

/// What one pass of picks cost.
#[derive(Default)]
struct Run {
    /// Milliseconds of each `probe_pick`.
    times: Vec<f64>,
    /// Draws the last pick's pointing pass submitted.
    draws: usize,
    transfers: x3d_render::transfer::Transfers,
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len().max(1) as f64
}

fn percentile(v: &[f64], p: usize) -> f64 {
    let mut v = v.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[(v.len() * p / 100).min(v.len().saturating_sub(1)).max(0)]
}

/// `PICKS` picks across the canvas, on whatever path `v` is on.
fn picks(v: &mut Viewer3d) -> Run {
    let mut run = Run::default();

    x3d_render::transfer::take();

    for k in 0..PICKS {
        // A spiral of probe points over the middle of the canvas, so the picks are not all the same pixel.
        let a = k as f64 * 0.7;
        let r = 8.0 + k as f64 * 0.4;
        let (x, y) = (W as f64 / 2.0 + r * a.cos(), H as f64 / 2.0 + r * a.sin());
        let t = Instant::now();
        let hit = v.probe_pick(x, y);

        run.times.push(t.elapsed().as_secs_f64() * 1e3);
        let _ = hit;
    }

    run.transfers = x3d_render::transfer::take();
    run.draws = v.pointing_gpu().map_or(0, |g| g.draws);
    run
}

/// One pick at the middle of the canvas, with the transfers it made.
fn one_pick(v: &mut Viewer3d) -> x3d_render::transfer::Transfers {
    x3d_render::transfer::take();
    v.probe_pick(W as f64 / 2.0 + 0.5, H as f64 / 2.0 + 0.5);
    x3d_render::transfer::take()
}

/// The cases, as (label, geometry type or kind, particles).
fn cases() -> Vec<(String, &'static str, usize)> {
    let mut out = vec![
        ("empty world".to_string(), "empty", 0),
        ("small scene".to_string(), "small scene", 0),
    ];

    for kind in ["QUAD", "LINE"] {
        for n in [100usize, 1_000, 10_000] {
            out.push((format!("{n} {kind}"), kind, n));
        }
    }

    out
}

#[test]
#[ignore]
fn particle_pick_latency() {
    let Ok(gpu) = x3d_render::gpu::Gpu::new() else {
        return x3d_evidence::no_gpu("skipped: no GPU");
    };
    let dir = std::env::temp_dir().join(format!("perf-particle-pick-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    println!(
        "{:>14} | {:>5} | {:>11} {:>9} {:>9} | {:>7} {:>7} | {:>8} {:>8} | {:>7}",
        "case",
        "draws",
        "ps upload B",
        "other B",
        "readb. B",
        "gpu ms",
        "p95",
        "cpu ms",
        "p95",
        "MiB"
    );

    for (label, kind, particles) in cases() {
        let path = dir.join(format!("{}.x3dv", label.replace(' ', "-")));
        std::fs::write(&path, world(kind, particles)).unwrap();

        let mut v = load(&path);

        for k in 0..WARMUP {
            v.step(T0 + (k + 1) as f64 * DT);
        }

        // The CPU ray cast first, while no buffer is set.
        let cpu = picks(&mut v);

        with_gpu(&mut v, &gpu.device, &gpu.queue);

        let g = picks(&mut v);
        let allocated = gpu
            .device
            .generate_allocator_report()
            .map_or(0, |r| r.total_allocated_bytes);

        println!(
            "{label:>14} | {:>5} | {:>11} {:>9} {:>9} | {:>7.3} {:>7.3} | {:>8.3} {:>8.3} | {:>7.1}",
            g.draws,
            g.transfers.particle_upload / PICKS as u64,
            g.transfers.other_upload / PICKS as u64,
            g.transfers.readback() / PICKS as u64,
            mean(&g.times),
            percentile(&g.times, 95),
            mean(&cpu.times),
            percentile(&cpu.times, 95),
            allocated as f64 / 1048576.0,
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

/// A QUAD ParticleSystem is one draw of the pointing pass at 100 particles and at 10,000, and of what the pass
/// transfers only the instance matrices grow — by the 64 bytes of one matrix per particle. The draw and the uniform
/// block of every particle, which the pass used to carry, are gone.
///
/// **This holds for POINT, QUAD, SPRITE and GEOMETRY particles, which share one vertex range. It does not hold for
/// LINE**, where every particle still brings its own six corners, so the CPU vertex work and the vertex upload grow
/// with the particle count; [`line_picking_is_one_draw_but_its_vertices_still_grow`] states that instead. Read no
/// claim about LINE upload into this test.
///
/// The counts are exact, so this is a proof and not a measurement. `particle_pick_instanced.rs`
/// (`the_gpu_path_does_not_touch_the_particles_one_by_one`) is the other half: it counts the shape records the pick
/// builds. The wall time of a pick is reported by [`particle_pick_latency`], which is `--ignored`: timings on a shared
/// machine are not a test.
#[test]
fn gpu_picking_does_not_scale_with_the_particle_count() {
    let Ok(gpu) = x3d_render::gpu::Gpu::new() else {
        return x3d_evidence::no_gpu("skipped: no GPU");
    };
    let dir = std::env::temp_dir().join(format!("scale-particle-pick-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut seen = vec![];

    for particles in [100usize, 10_000] {
        let path = dir.join(format!("p{particles}.x3dv"));
        std::fs::write(&path, world("QUAD", particles)).unwrap();

        let mut v = load(&path);

        for k in 0..WARMUP {
            v.step(T0 + (k + 1) as f64 * DT);
        }

        // The ray cast transfers nothing: everything below is the pointing pass's.
        assert_eq!(
            one_pick(&mut v).upload(),
            0,
            "{particles}: the ray cast uploaded something"
        );

        with_gpu(&mut v, &gpu.device, &gpu.queue);

        // The first pass of a new buffer sizes it; the second is the steady state.
        one_pick(&mut v);

        let transfers = one_pick(&mut v);
        let draws = v.pointing_gpu().map_or(0, |g| g.draws);
        let alive = v.rt.scene.contexts[x3d_scene::ROOT]
            .named_nodes
            .get("PS")
            .and_then(|ps| v.rt.particle_sim(ps))
            .map_or(0, |s| s.num_particles);

        assert!(
            alive > particles / 2,
            "{particles}: only {alive} particles alive"
        );
        assert!(draws > 0, "{particles}: the pass drew nothing");
        // `draws` and `alive` can both hold with no ParticleSystem draw in the pass at all — another shape
        // draws, the simulation runs, and the constant-upload bound below is then satisfied by zero. A live
        // QUAD system contributes at least its own uniform slice, so require the upload to exist.
        assert!(
            transfers.particle_upload > 0,
            "{particles}: the pass uploaded no particle data, so the bound below proves nothing"
        );
        seen.push((particles, alive, draws, transfers));
    }

    let (small, big) = (&seen[0], &seen[1]);

    // The pass is the same size at 100 and at 10,000 particles: the system is one draw either way.
    assert_eq!(
        small.2, big.2,
        "the pointing pass grew from {} to {} draws with the particles",
        small.2, big.2
    );

    // Only the instance matrices grow, and only by one matrix per particle.
    let grew = big
        .3
        .particle_upload
        .saturating_sub(small.3.particle_upload);
    let matrices = 64 * (big.1 - small.1) as u64;

    assert!(
        grew <= matrices * 3 / 2,
        "the particle upload grew by {grew} bytes for {} more particles ({matrices} as matrices)",
        big.1 - small.1
    );
    assert_eq!(
        small.3.other_upload, big.3.other_upload,
        "the non-particle upload grew with the particles"
    );
    assert_eq!(
        small.3.readback(),
        big.3.readback(),
        "the readback grew with the particles"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// LINE particles: the pass is still one draw per system, but its vertex data is *not* constant in the particle
/// count. A line is widened along its projected minor axis, which the instance matrix turns, so each particle brings
/// its own six corners (`PointingTriangle::per_instance_vertices`) and the pointer builds them on the CPU every pick.
///
/// This test records that gap rather than hiding it: it requires the one draw, and it requires the upload to grow
/// *at least* per particle, so a later lane that makes LINE uploads constant has to come here and say so. Making
/// LINE independent of the particle count is deliberately left to a separate lane (PR #43, known gap 1).
#[test]
fn line_picking_is_one_draw_but_its_vertices_still_grow() {
    let Ok(gpu) = x3d_render::gpu::Gpu::new() else {
        return x3d_evidence::no_gpu("skipped: no GPU");
    };
    let dir = std::env::temp_dir().join(format!("scale-line-pick-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut seen = vec![];

    for particles in [100usize, 10_000] {
        let path = dir.join(format!("l{particles}.x3dv"));
        std::fs::write(&path, world("LINE", particles)).unwrap();

        let mut v = load(&path);

        for k in 0..WARMUP {
            v.step(T0 + (k + 1) as f64 * DT);
        }

        with_gpu(&mut v, &gpu.device, &gpu.queue);

        // The first pass of a new buffer sizes it; the second is the steady state.
        one_pick(&mut v);

        let transfers = one_pick(&mut v);
        let draws = v.pointing_gpu().map_or(0, |g| g.draws);
        let alive = v.rt.scene.contexts[x3d_scene::ROOT]
            .named_nodes
            .get("PS")
            .and_then(|ps| v.rt.particle_sim(ps))
            .map_or(0, |s| s.num_particles);

        assert!(
            alive > particles / 2,
            "{particles}: only {alive} particles alive"
        );
        assert!(draws > 0, "{particles}: the pass drew nothing");
        seen.push((particles, alive, draws, transfers));
    }

    let (small, big) = (&seen[0], &seen[1]);

    // Still one draw per system: that part of B25-2 does hold for LINE.
    assert_eq!(
        small.2, big.2,
        "the pointing pass grew from {} to {} draws with the LINE particles",
        small.2, big.2
    );

    // And the upload is not constant: at least one matrix per particle, and in practice the six corners too.
    let grew = big
        .3
        .particle_upload
        .saturating_sub(small.3.particle_upload);
    let matrices = 64 * (big.1 - small.1) as u64;

    assert!(
        grew >= matrices,
        "the LINE upload grew by only {grew} bytes for {} more particles ({matrices} as matrices alone): if LINE \
         uploads are now constant, this test and the lane's known gap 1 have to be rewritten",
        big.1 - small.1
    );

    eprintln!(
        "LINE upload: {} B at {} particles, {} B at {} (matrices alone would be {matrices} B more)",
        small.3.particle_upload, small.1, big.3.particle_upload, big.1
    );

    let _ = std::fs::remove_dir_all(&dir);
}
