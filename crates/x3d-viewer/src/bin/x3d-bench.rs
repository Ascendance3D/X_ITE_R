//! x3d-bench <file.wrl> [--oracle dir] [--size WxH] [--frames N] [--readback]
//!
//! Frame-time benchmark of the interactive loop without a window: an EXAMINE drag (the camera
//! moves every frame, so every frame is a real redraw) on a virtual 60 Hz clock. Per frame it
//! times, in ms: the viewer step, the frame build, the CPU cost of encoding and submitting the
//! render (the direct path of the window: one device, rendering into a texture, no readback),
//! and the GPU time of the render pass from timestamp queries. `--readback` also measures the
//! offscreen path the screenshot tools use (render + copy + map). Prints mean / median / p95.

use std::path::PathBuf;
use std::time::Instant;
use x3d_viewer::viewer::{UiEvent, Viewer3d};

fn stats(v: &mut [f64]) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = v.iter().sum::<f64>() / v.len().max(1) as f64;
    (
        mean,
        v[v.len() / 2],
        v[(v.len() * 95 / 100).min(v.len() - 1)],
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let opt = |f: &str| {
        args.iter()
            .position(|a| a == f)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let Some(path) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        eprintln!(
            "usage: x3d-bench <file.wrl> [--oracle dir] [--size WxH] [--frames N] [--readback]"
        );
        std::process::exit(2);
    };
    let (mut w, mut h) = (1024u32, 768u32);

    if let Some((a, b)) = opt("--size").as_deref().and_then(|s| s.split_once('x')) {
        w = a.parse().unwrap_or(w);
        h = b.parse().unwrap_or(h);
    }

    let readback = args.iter().any(|a| a == "--readback");
    let frames: usize = opt("--frames").and_then(|f| f.parse().ok()).unwrap_or(120);
    let oracle = opt("--oracle").map(PathBuf::from);
    let r = std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || run(PathBuf::from(path), oracle, w, h, frames, readback))
        .unwrap()
        .join()
        .unwrap();

    if let Err(e) = r {
        eprintln!("x3d-bench: {e}");
        std::process::exit(1);
    }
}

fn run(
    path: PathBuf,
    oracle: Option<PathBuf>,
    w: u32,
    h: u32,
    frames: usize,
    readback: bool,
) -> Result<(), String> {
    let t0 = 946684800.0;
    let mut v = Viewer3d::load(&path, oracle.as_deref(), t0, w, h)?;
    v.frame_rate = 60.0;

    for _ in 0..8 {
        v.step(t0);
    }

    let mut gpu = x3d_render::gpu::Gpu::new()?;
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
    let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
    v.input(UiEvent::MouseDown {
        button: 0,
        x: cx,
        y: cy,
    });
    let (mut step, mut build, mut submit, mut idle, mut gpu_ms, mut total, mut rb) =
        (vec![], vec![], vec![], vec![], vec![], vec![], vec![]);
    let (mut uploads, mut draws) = (0usize, 0usize);

    for k in 1..=frames + 10 {
        let a = k as f64 * 0.11;
        v.input(UiEvent::MouseMove {
            x: cx + 0.3 * cx * a.sin(),
            y: cy + 0.3 * cy * (a * 0.7).cos(),
        });
        let t = t0 + k as f64 / 60.0;
        v.date_now = t;
        let t_start = Instant::now();
        v.step(t);
        let t1 = Instant::now();
        let frame = v.build_frame();
        let t2 = Instant::now();
        gpu.renderer
            .render(&frame, &view, wgpu::TextureFormat::Bgra8Unorm, w, h)?;
        let t3 = Instant::now();
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let t4 = Instant::now();
        let g = gpu.renderer.gpu_time_ms(true);

        if k > 10 {
            step.push((t1 - t_start).as_secs_f64() * 1e3);
            build.push((t2 - t1).as_secs_f64() * 1e3);
            submit.push((t3 - t2).as_secs_f64() * 1e3);
            idle.push((t4 - t3).as_secs_f64() * 1e3);
            total.push((t4 - t_start).as_secs_f64() * 1e3);
            gpu_ms.push(g.unwrap_or(f64::NAN));
            uploads += gpu.renderer.stats.geometry_uploads;
            draws = gpu.renderer.stats.draw_calls;
        }

        if readback && k > 10 {
            let t5 = Instant::now();
            let rgba = gpu.renderer.render_to_rgba(&frame, w, h);
            std::hint::black_box(&rgba);
            rb.push(t5.elapsed().as_secs_f64() * 1e3);
        }
    }

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let (s, b, u, i, t) = (
        stats(&mut step),
        stats(&mut build),
        stats(&mut submit),
        stats(&mut idle),
        stats(&mut total),
    );
    let gm = if gpu_ms.iter().any(|x| x.is_nan()) {
        "n/a".to_string()
    } else {
        let g = stats(&mut gpu_ms);
        format!("{:.2}/{:.2}", g.0, g.1)
    };
    let mut line = format!(
        "{name:30} {w}x{h} step {:5.2}  build {:5.2}  submit {:5.2}  gpu-idle {:5.2}  GPU {gm:>10}  | total mean {:5.2} median {:5.2} p95 {:5.2} ms ({draws} draws, {uploads} geometry uploads over {frames} frames)",
        s.0, b.0, u.0, i.0, t.0, t.1, t.2
    );

    if readback {
        let r = stats(&mut rb);
        line.push_str(&format!("  | offscreen render+readback {:5.2} ms", r.0));
    }

    println!("{line}");
    Ok(())
}
