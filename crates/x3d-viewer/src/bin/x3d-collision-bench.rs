//! x3d-collision-bench [--frames N] [--only SUBSTRING] [--cpu] [--json FILE]
//!
//! Frame-time benchmark of the collision depth passes. It writes its own scenes: no particles, 1,000 and 10,000 QUAD,
//! POINT and LINE particles around a WALK avatar above a floor, each with the content inside an enabled Collision node
//! (X3DRenderObject.collide along six axes, gravitate, constrained moves) and inside a disabled one (gravitate and the
//! moves on an empty collision world). The avatar walks forward every frame (a WALK drag) on a virtual 60 Hz clock.
//!
//! Per frame it records, from [`x3d_render::depth_gpu::DepthStats`] and [`Viewer3d::collision_time`]: the GPU
//! submissions, polls and blocking readbacks of the depth buffer, the time blocked waiting for them, the time reading
//! the mapped bytes out, the GPU time of the passes (timestamp queries, when the device has them), the CPU time of the
//! collision work (its wall time less the waits), the upload and readback bytes, and the whole frame (step, frame
//! build, render and the GPU idle wait). It prints the median and p95 of each. `--cpu` runs the CPU stand-in instead.

use std::path::{Path, PathBuf};
use std::time::Instant;
use x3d_viewer::viewer::{UiEvent, Viewer3d};

const HEAD: &str = "#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:3\n";

/// A WALK avatar 5 m back from the particles, eye height 1.6, on a 40 x 40 floor.
const AVATAR: &str = "NavigationInfo { type \"WALK\" }\nViewpoint { position 0 1.6 5 }\n";
const FLOOR: &str = "Transform { translation 0 -0.05 0 children Shape { appearance Appearance { material Material { } } \
                     geometry Box { size 40 0.1 40 } } }\n";

struct Scene {
    name: String,
    text: String,
}

fn scenes() -> Vec<Scene> {
    let mut out = vec![];

    for enabled in [true, false] {
        let on = if enabled { "on" } else { "off" };
        let wrap = |body: &str| {
            format!(
                "{HEAD}{AVATAR}Collision {{ enabled {} children [\n{body}] }}\n",
                if enabled { "TRUE" } else { "FALSE" }
            )
        };

        out.push(Scene {
            name: format!("empty/{on}"),
            text: wrap(""),
        });
        out.push(Scene {
            name: format!("floor/{on}"),
            text: wrap(FLOOR),
        });

        for count in [1000, 10000] {
            for geometry in ["QUAD", "POINT", "LINE"] {
                // A cloud of particles about 1 m around (0 1.2 3), in front of the walking avatar.
                let system = format!(
                    "Transform {{ translation 0 1.2 3 children ParticleSystem {{ geometryType \"{geometry}\" \
                     maxParticles {count} particleLifetime 2 lifetimeVariation 0 particleSize 0.05 0.05 \
                     emitter VolumeEmitter {{ coordIndex [0 1 2 -1 0 2 3 -1 0 3 1 -1 1 3 2 -1] coord Coordinate {{ \
                     point [0 1 0, -1 -1 1, 1 -1 1, 0 -1 -1] }} speed 0.05 variation 0.5 }} \
                     appearance Appearance {{ material Material {{ }} }} }} }}\n"
                );
                out.push(Scene {
                    name: format!("{}-{count}/{on}", geometry.to_lowercase()),
                    text: wrap(&format!("{FLOOR}{system}")),
                });
            }
        }
    }

    out
}

fn percentile(v: &mut [f64], p: usize) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }

    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[(v.len() * p / 100).min(v.len() - 1)]
}

#[derive(Default)]
struct Samples {
    submissions: Vec<f64>,
    polls: Vec<f64>,
    readbacks: Vec<f64>,
    queries: Vec<f64>,
    wait: Vec<f64>,
    transfer: Vec<f64>,
    encode: Vec<f64>,
    gpu: Vec<f64>,
    cpu: Vec<f64>,
    collision: Vec<f64>,
    frame: Vec<f64>,
    upload: Vec<f64>,
    readback_bytes: Vec<f64>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let opt = |f: &str| {
        args.iter()
            .position(|a| a == f)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let frames: usize = opt("--frames").and_then(|f| f.parse().ok()).unwrap_or(240);
    let only = opt("--only");
    let json = opt("--json").map(PathBuf::from);
    let cpu = args.iter().any(|a| a == "--cpu");
    let r = std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || run(frames, only, cpu, json))
        .unwrap()
        .join()
        .unwrap();

    if let Err(e) = r {
        eprintln!("x3d-collision-bench: {e}");
        std::process::exit(1);
    }
}

fn run(
    frames: usize,
    only: Option<String>,
    cpu: bool,
    json: Option<PathBuf>,
) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("x3d-collision-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut gpu = x3d_render::gpu::Gpu::new()?;
    let mut report = serde_json::Map::new();

    println!(
        "{:16} {:>5} | {:>9} {:>9} {:>9} | {:>11} {:>11} {:>11} {:>11} {:>11} {:>11} | {:>9} {:>9}",
        "scene",
        "q/f",
        "submit/f",
        "poll/f",
        "readbk/f",
        "wait ms",
        "xfer ms",
        "gpu ms",
        "cpu ms",
        "coll ms",
        "frame ms",
        "up B/f",
        "rb B/f"
    );

    for scene in scenes() {
        if only.as_deref().is_some_and(|o| !scene.name.contains(o)) {
            continue;
        }

        let path = dir.join(format!("{}.x3dv", scene.name.replace('/', "-")));
        std::fs::write(&path, &scene.text).map_err(|e| e.to_string())?;
        let mut s = bench(&mut gpu, &path, frames, cpu)?;
        let med = |v: &mut Vec<f64>| percentile(v, 50);
        let p95 = |v: &mut Vec<f64>| percentile(v, 95);
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;

        println!(
            "{:16} {:5.1} | {:9.2} {:9.2} {:9.2} | {:5.3}/{:5.3} {:5.3}/{:5.3} {:5.3}/{:5.3} {:5.3}/{:5.3} {:5.3}/{:5.3} {:5.2}/{:5.2} | {:9.0} {:9.0}",
            scene.name,
            mean(&s.queries),
            mean(&s.submissions),
            mean(&s.polls),
            mean(&s.readbacks),
            med(&mut s.wait),
            p95(&mut s.wait),
            med(&mut s.transfer),
            p95(&mut s.transfer),
            med(&mut s.gpu),
            p95(&mut s.gpu),
            med(&mut s.cpu),
            p95(&mut s.cpu),
            med(&mut s.collision),
            p95(&mut s.collision),
            med(&mut s.frame),
            p95(&mut s.frame),
            mean(&s.upload),
            mean(&s.readback_bytes),
        );

        let mut entry = serde_json::Map::new();
        let fields: [(&str, &mut Vec<f64>); 13] = [
            ("queries", &mut s.queries),
            ("submissions", &mut s.submissions),
            ("polls", &mut s.polls),
            ("readbacks", &mut s.readbacks),
            ("waitMs", &mut s.wait),
            ("transferMs", &mut s.transfer),
            ("encodeMs", &mut s.encode),
            ("gpuMs", &mut s.gpu),
            ("cpuMs", &mut s.cpu),
            ("collisionMs", &mut s.collision),
            ("frameMs", &mut s.frame),
            ("uploadBytes", &mut s.upload),
            ("readbackBytes", &mut s.readback_bytes),
        ];

        for (k, v) in fields {
            entry.insert(
                k.into(),
                serde_json::json!({ "mean": mean(v), "median": med(v), "p95": p95(v) }),
            );
        }

        report.insert(scene.name.clone(), serde_json::Value::Object(entry));
    }

    if let Some(json) = json {
        std::fs::write(
            &json,
            serde_json::to_string_pretty(&serde_json::Value::Object(report)).unwrap(),
        )
        .map_err(|e| e.to_string())?;
    }

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

fn bench(
    gpu: &mut x3d_render::gpu::Gpu,
    path: &Path,
    frames: usize,
    cpu: bool,
) -> Result<Samples, String> {
    let (w, h) = (256u32, 256u32);
    let t0 = 946684800.0;
    let dt = 1.0 / 60.0;
    let mut v = Viewer3d::load(path, None, t0, w, h)?;
    v.frame_rate = 60.0;

    let depth = (!cpu).then(|| {
        let d = std::rc::Rc::new(std::cell::RefCell::new(
            x3d_render::depth_gpu::DepthGpu::new(gpu.device.clone(), gpu.queue.clone()),
        ));
        d.borrow_mut().set_timestamps(true);
        d
    });
    let pointing =
        x3d_render::pointing_gpu::PointingGpu::new(gpu.device.clone(), gpu.queue.clone());
    v.set_gpu(&gpu.device.clone(), Some(pointing), depth.clone())
        .map_err(|e| format!("{e:?}"))?;

    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bench"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let mut s = Samples::default();
    // Two seconds: the particle systems reach their maxParticles before the measured frames.
    let warmup = 120;
    let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);

    for k in 1..=warmup + frames {
        let t = t0 + k as f64 * dt;

        // The WALK drag starts with the measured frames: the avatar walks forward from then on.
        if k == warmup {
            v.input(UiEvent::MouseDown {
                button: 0,
                x: cx,
                y: cy,
            });
            v.input(UiEvent::MouseMove {
                x: cx,
                y: cy - 20.0,
            });
        }

        if let Some(d) = &depth {
            d.borrow_mut().take_stats();
        }

        v.collision_time = std::time::Duration::ZERO;
        v.date_now = t;
        let start = Instant::now();
        v.step(t);
        let frame = v.build_frame();
        gpu.renderer
            .render(&frame, &view, wgpu::TextureFormat::Bgra8Unorm, w, h)?;
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let total = start.elapsed();

        if k <= warmup {
            continue;
        }

        let st = depth
            .as_ref()
            .map(|d| d.borrow_mut().take_stats())
            .unwrap_or_default();
        let ms = |d: std::time::Duration| d.as_secs_f64() * 1e3;
        let collision = ms(v.collision_time);

        s.submissions.push(st.submissions as f64);
        s.polls.push(st.polls as f64);
        s.readbacks.push(st.readbacks as f64);
        s.queries.push(st.queries as f64);
        s.wait.push(ms(st.wait));
        s.transfer.push(ms(st.transfer));
        s.encode.push(ms(st.encode));
        s.gpu.push(ms(st.gpu));
        s.cpu.push(collision - ms(st.wait));
        s.collision.push(collision);
        s.frame.push(ms(total));
        s.upload.push(st.upload_bytes as f64);
        s.readback_bytes.push(st.readback_bytes as f64);
    }

    Ok(s)
}
