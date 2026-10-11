//! x3d-drive <file.wrl> <script.json> [--oracle <dir>] [--out result.json] [--png-dir <dir>]
//!
//! Headless, deterministic run of the viewer core: load the world live at t0, then for frame
//! k = 1..=frames apply the scripted input events of frame k and step the clock to t0 + k * dt.
//! The same script drives the X_ITE oracle (oracle/x_ite-anim.mjs), so the results compare.
//!
//! Script (JSON): { "size": [512, 512], "t0": 946684800, "dt": 0.016666666666666666, "frames": 60,
//!   "events": [ { "frame": 1, "type": "mousedown", "button": 0, "x": 256, "y": 256 }, ... ],
//!   "capture": [10, 60], "dump": [60] }
//! Event types: mousedown, mouseup, mousemove, wheel (deltaY), key (Home, End, PageUp,
//! PageDown), mods (shift, ctrl, alt), viewer (name). Pointer coordinates are pixels with the
//! origin at the bottom left of the canvas. "t" (seconds) offsets the event's Date.now ().

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use x3d_viewer::script::apply;
use x3d_viewer::viewer::Viewer3d;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let opt = |f: &str| args.iter().position(|a| a == f).and_then(|i| args.get(i + 1)).cloned();

    if args.len() < 3 {
        eprintln!("usage: x3d-drive <file.wrl> <script.json> [--oracle dir] [--out result.json] [--png-dir dir]");
        std::process::exit(2);
    }

    let file = PathBuf::from(&args[1]);
    let script: Value = serde_json::from_str(&std::fs::read_to_string(&args[2]).expect("script")).expect("script json");
    let oracle = opt("--oracle");
    let out = opt("--out");
    let png_dir = opt("--png-dir");
    let warmup_override = opt("--warmup-from")
        .and_then(|p| serde_json::from_str::<Value>(&std::fs::read_to_string(p).ok()?).ok())
        .and_then(|v| v["warmupFrames"].as_u64())
        .or(opt("--warmup").and_then(|w| w.parse().ok()));

    let r = std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || run(&file, &script, oracle.as_deref().map(Path::new), png_dir.as_deref().map(Path::new), warmup_override))
        .unwrap()
        .join()
        .unwrap();

    let text = serde_json::to_string_pretty(&r).unwrap();

    match out {
        Some(o) => std::fs::write(o, text).unwrap(),
        None => println!("{text}"),
    }
}

fn vec3(v: &x3d_fields::math::Vector3) -> Value {
    json!([v.x, v.y, v.z])
}

fn rot(r: &x3d_fields::math::Rotation4) -> Value {
    let mut r = *r;
    json!(r.components())
}

fn pose(v: &mut Viewer3d) -> Value {
    let (p, o, c) = v.user_pose();
    let bound = v.layer.viewpoint();
    let name = if bound == x3d_viewer::layer::DEFAULT { "<default>".to_string() } else { v.rt.scene.node(bound).name.clone() };
    json!({ "position": vec3(&p), "orientation": rot(&o), "centerOfRotation": vec3(&c), "viewpoint": name, "viewer": v.viewer_kind() })
}

fn run(file: &Path, script: &Value, oracle: Option<&Path>, png_dir: Option<&Path>, warmup_override: Option<u64>) -> Value {
    let size = script["size"].as_array().map(|a| (a[0].as_u64().unwrap_or(512) as u32, a[1].as_u64().unwrap_or(512) as u32)).unwrap_or((512, 512));
    let t0 = script["t0"].as_f64().unwrap_or(946684800.0);
    let dt = script["dt"].as_f64().unwrap_or(1.0 / 60.0);
    let frames = script["frames"].as_u64().unwrap_or(1);
    let list = |k: &str| script[k].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect::<Vec<_>>()).unwrap_or_default();
    let (capture, dump) = (list("capture"), list("dump"));
    let events = script["events"].as_array().cloned().unwrap_or_default();

    let mut v = match Viewer3d::load(file, oracle, t0, size.0, size.1) {
        Ok(v) => v,
        Err(e) => return json!({ "ok": false, "error": e }),
    };

    v.frame_rate = 1.0 / dt;

    // Warm-up frames at t0, like the oracle's frames while the world loads (anim.html).
    // `--warmup-from <oracle.json>` copies the oracle's warmupFrames (frames traversed while the
    // loaded world was current), so frame-count dependent behavior (WALK gravity) lines up.
    let warmup = warmup_override.or(script["warmup"].as_u64()).unwrap_or(8);
    for _ in 0..warmup {
        v.step(t0);
    }

    let mut gpu = if png_dir.is_some() { x3d_render::gpu::Gpu::new().ok() } else { None };
    // `--msaa N`: X_ITE's Multisampling for the captured PNGs (default 0, so the oracle baselines stay valid).
    if let Some(g) = gpu.as_mut() {
        g.renderer.set_sample_count(std::env::args().skip_while(|a| a != "--msaa").nth(1).and_then(|v| v.parse().ok()).unwrap_or(0));
        // `--gl-samples`: mirror the draw so the MSAA sample pattern matches the GL oracle's.
        g.renderer.set_gl_sample_orientation(std::env::args().any(|a| a == "--gl-samples"));
    }
    let mut poses = serde_json::Map::new();
    let mut vrml = serde_json::Map::new();
    let mut events_log: Vec<Value> = vec![];

    for k in 1..=frames {
        let prev = t0 + (k - 1) as f64 * dt;

        for e in events.iter().filter(|e| e["frame"].as_u64() == Some(k)) {
            v.date_now = prev + e["t"].as_f64().unwrap_or(0.0);
            apply(&mut v, e);
        }

        v.date_now = t0 + k as f64 * dt;
        v.step(t0 + k as f64 * dt);

        if capture.contains(&k) || dump.contains(&k) || k == frames {
            poses.insert(k.to_string(), pose(&mut v));
        }

        if dump.contains(&k) || k == frames {
            v.rt.publish_clone_counts();
            vrml.insert(k.to_string(), json!(v.rt.scene.to_vrml_string()));
        }

        if capture.contains(&k) {
            if let (Some(g), Some(dir)) = (gpu.as_mut(), png_dir) {
                let frame = v.build_frame();
                let rgba = g.render(&frame, size.0, size.1);
                let rgba: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
                std::fs::create_dir_all(dir).ok();
                let _ = x3d_render::write_png(&dir.join(format!("{k}.png")), size.0, size.1, &rgba);
            }
        }

        events_log.push(json!(v.anchors.len()));
    }

    let mut log = v.rt.log.clone();
    log.sort();

    json!({
        "ok": true,
        "poses": poses,
        "vrml": vrml,
        "prints": v.rt.prints,
        "log": log,
        "anchors": v.anchors.iter().map(|a| json!({ "description": a.description, "url": a.urls, "parameter": a.parameters })).collect::<Vec<_>>(),
        "status": v.status,
        "title": v.title(),
    })
}
