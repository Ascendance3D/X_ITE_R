//! x3d-drive <file.wrl> <script.json> [--oracle <dir>] [--out result.json] [--png-dir <dir>]
//!
//! Headless, deterministic run of the viewer core: load the world live at t0, then for frame
//! k = 1..=frames apply the scripted input events of frame k and step the clock to t0 + k * dt.
//! The same script drives the X_ITE oracle (oracle/x_ite-anim.mjs), so the results compare.
//!
//! Script (JSON): { "size": [512, 512], "t0": 946684800, "dt": 0.016666666666666666, "frames": 60,
//!   "events": [ { "frame": 1, "type": "mousedown", "button": 0, "x": 256, "y": 256 }, ... ],
//!   "capture": [10, 60], "dump": [60] }
//! "physics": { "nodes": { PATH: [ field, ... ] } } (REA batch 06, RigidBodyPhysics): the result gets
//! `physics.dt` (every `RigidBodyCollection .getTimeStep ()` result with the frame it was made in; frame 0 is
//! the load and the warm-up) and `physics.frames` (after the warm-up and after each frame, the listed
//! fields of the listed nodes). PATH is a DEF name, or `Inline/Name` for a DEF inside a DEF-named Inline.
//! Pseudo fields: `@bodyActive` (the btRigidBody's isActive ()), and on a CollisionSensor `contacts`
//! (position, contactNormal, depth, geometry1, geometry2, body1, body2 of every Contact). A script with
//! "physics" pins `currentFrameRate` to 1 / dt, like the oracle page does.
//! Event types: mousedown, mouseup, mousemove, wheel (deltaY), dblclick, key (Home, End, PageUp,
//! PageDown), mods (shift, ctrl, alt), viewer (name). Pointer coordinates are pixels with the
//! origin at the bottom left of the canvas. "t" (seconds) offsets the event's Date.now ().

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use x3d_viewer::script::apply;
use x3d_viewer::viewer::Viewer3d;

/// The `physics` object of the result as text (see `main`).
static PHYSICS_TEXT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

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

    let out_path = out.clone();
    let r = std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || run(&file, &script, oracle.as_deref().map(Path::new), png_dir.as_deref().map(Path::new), warmup_override, out_path.as_deref()))
        .unwrap()
        .join()
        .unwrap();

    let mut text = serde_json::to_string_pretty(&r).unwrap();

    // The physics object keeps the key order of the script (serde_json sorts keys): it is written as text.
    if let Some(p) = PHYSICS_TEXT.lock().unwrap().take() {
        text = text.replace("\"@@physics@@\"", &p);
    }

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
    let name = if x3d_viewer::layer::is_default(bound) { "<default>".to_string() } else { v.rt.scene.node(bound).name.clone() };
    let (fov_scale, fov_offset) = v.field_of_view_state();
    let mut j = json!({ "position": vec3(&p), "orientation": rot(&o), "centerOfRotation": vec3(&c), "viewpoint": name, "viewer": v.viewer_field(), "fieldOfViewScale": fov_scale });
    // OrthoViewpoint ._fieldOfViewOffset (the oracle reports it for an OrthoViewpoint only).
    if let Some(o) = fov_offset {
        j["fieldOfViewOffset"] = json!(o);
    }
    j
}

fn run(file: &Path, script: &Value, oracle: Option<&Path>, png_dir: Option<&Path>, warmup_override: Option<u64>, out: Option<&str>) -> Value {
    let size = script["size"].as_array().map(|a| (a[0].as_u64().unwrap_or(512) as u32, a[1].as_u64().unwrap_or(512) as u32)).unwrap_or((512, 512));
    let t0 = script["t0"].as_f64().unwrap_or(946684800.0);
    let dt = script["dt"].as_f64().unwrap_or(1.0 / 60.0);
    let frames = script["frames"].as_u64().unwrap_or(1);
    let list = |k: &str| script[k].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect::<Vec<_>>()).unwrap_or_default();
    let (capture, dump) = (list("capture"), list("dump"));
    let events = script["events"].as_array().cloned().unwrap_or_default();

    // `"pinFrameRate": true`: browser .currentFrameRate is 1 / dt (the oracle pins it the same way).
    let physics = script["physics"].is_object();
    let pinned = (script["pinFrameRate"].as_bool().unwrap_or(false) || physics).then(|| 1.0 / dt);

    if physics {
        x3d_runtime::nodes::rigid_body_physics::record_time_steps(true);
    }

    // `"console": true`: record every console message in order (`console` in the result), like the
    // oracle's page listener. `"dumpInlines": true`: the scene text of every DEF-named Inline.
    let console = script["console"].as_bool().unwrap_or(false);
    let dump_inlines = script["dumpInlines"].as_bool().unwrap_or(false);
    // `--msaa N`: the MSAA oracle page (anim-msaa.html) has `antialiased="true" multisampling="N"`
    // instead of anim.html's `antialiased="false"`.
    let msaa = std::env::args().skip_while(|a| a != "--msaa").nth(1);
    let mut attributes = x3d_viewer::viewer::ORACLE_PAGE_ATTRIBUTES.to_vec();

    if let Some(n) = msaa.as_deref() {
        attributes.retain(|(a, _)| *a != "antialiased");
        attributes.extend([("antialiased", "true"), ("multisampling", n)]);
    }

    // `"audio": { "sampleRate": 48000, "channels": 2, "tail": 4 }` (REA batch 05): the browser's
    // audio renders offline in step with the frames, like oracle/x_ite-anim.mjs does: frame k waits
    // until rendering is suspended at render quantum q (k), applies its events, steps, resumes.
    // The rendered buffer is written next to --out as <out>.audio.f32 (planar float32).
    let audio = script["audio"].is_object().then(|| {
        let a = &script["audio"];
        (a["sampleRate"].as_f64().unwrap_or(48000.0), a["channels"].as_u64().unwrap_or(2) as usize, a["tail"].as_u64().unwrap_or(4) as usize)
    });
    let quantum = |k: u64, rate: f64| ((k as f64 * dt * rate).round() / 128.0).ceil() as usize;

    if let Some((rate, channels, tail)) = audio {
        let length = (quantum(frames, rate) + tail) * 128;
        x3d_runtime::sound::set_default_mode(x3d_runtime::sound::AudioMode::Offline { channels, sample_rate: rate as f32, length });
    }

    let mut v = match Viewer3d::load_attributes(file, oracle, t0, size.0, size.1, pinned, console, &attributes) {
        Ok(v) => v,
        Err(e) => return json!({ "ok": false, "error": e }),
    };

    v.frame_rate = 1.0 / dt;

    // Warm-up frames at t0, like the oracle's frames while the world loads (anim.html).
    // `--warmup-from <oracle.json>` copies the oracle's warmupFrames (frames traversed while the
    // loaded world was current), so frame-count dependent behavior (WALK gravity) lines up.
    let warmup = warmup_override.or(script["warmup"].as_u64()).unwrap_or(8);

    // The pointing buffer on the GPU, like X_ITE's (`--cpu-pointing`: the ray cast's double values).
    if !std::env::args().any(|a| a == "--cpu-pointing") {
        v.pointing_gpu = match x3d_render::pointing_gpu::PointingGpu::new_headless() {
            Ok(g) => {
                // The collision depth buffer shares the device.
                v.depth_gpu = Some(std::rc::Rc::new(std::cell::RefCell::new(x3d_render::depth_gpu::DepthGpu::new(g.device().clone(), g.queue().clone()))));
                Some(g)
            }
            Err(e) => {
                eprintln!("x3d-drive: no pointing GPU ({e}); picks use the ray cast");
                None
            }
        };
    }

    let mut gpu = if png_dir.is_some() { x3d_render::gpu::Gpu::new().ok() } else { None };
    // The RenderingProperties of the page (Multisampling 0 for anim.html, N for `--msaa N`).
    if let Some(g) = gpu.as_mut() {
        v.rendering_properties().apply(&mut g.renderer);
        // `--gl-samples`: mirror the draw so the MSAA sample pattern matches the GL oracle's.
        g.renderer.set_gl_sample_orientation(std::env::args().any(|a| a == "--gl-samples"));
    }

    // The browser draws every frame; GeneratedCubeMapTextures and RenderedTextures (rendered
    // textures) change with each one. Worlds that have them draw their texture passes on frames not captured, too.
    // (A texture that starts with update NONE counts once the root scene names the node type, or
    // once a captured frame draws one.)
    // (Checked without drawing: a browser draws the first texture pass after the first traverse.)
    let mut live_textures = gpu.is_some() && {
        let f = v.build_frame();
        let text = v.rt.scene.to_vrml_string();
        !f.cube_passes.is_empty() || !f.rendered_passes.is_empty() || text.contains("GeneratedCubeMapTexture") || text.contains("RenderedTexture")
    };

    for _ in 0..warmup {
        v.step(t0);

        if let (true, Some(g)) = (live_textures, gpu.as_mut()) {
            let _ = g.renderer.render_cube_passes(&v.build_frame());
        }
    }
    let physics_spec = if physics { physics_nodes(&std::fs::read_to_string(std::env::args().nth(2).unwrap()).unwrap_or_default()) } else { vec![] };
    let mut physics_frames: Vec<(u64, String)> = vec![];
    let mut physics_dt: Vec<Value> = vec![];

    if physics {
        take_dt(&mut v, 0, &mut physics_dt);
        physics_frames.push((0, physics_snapshot(&mut v, &physics_spec)));
    }

    let mut poses = serde_json::Map::new();
    let mut vrml = serde_json::Map::new();
    let mut events_log: Vec<Value> = vec![];
    let mut inlines = serde_json::Map::new();
    let mut audio_suspend: Vec<Value> = vec![];

    if let Some((rate, ..)) = audio {
        let quanta: Vec<usize> = (1..=frames).map(|k| quantum(k, rate)).collect();
        v.rt.audio().arm(&quanta);
        // From here a decodeAudioData result waits for the start of the next frame (the oracle
        // page gates the browser's decoder thread the same way); during the load it does not.
        v.rt.defer_audio_decodes = true;
    }

    for k in 1..=frames {
        let prev = t0 + (k - 1) as f64 * dt;

        if audio.is_some() {
            audio_suspend.push(json!(v.rt.audio().wait()));
            v.rt.flush_audio_decodes();
        }

        for e in events.iter().filter(|e| e["frame"].as_u64() == Some(k)) {
            v.date_now = prev + e["t"].as_f64().unwrap_or(0.0);
            apply(&mut v, e);
        }

        v.date_now = t0 + k as f64 * dt;
        v.step(t0 + k as f64 * dt);

        if physics {
            take_dt(&mut v, k, &mut physics_dt);
            physics_frames.push((k, physics_snapshot(&mut v, &physics_spec)));
        }

        if let (true, Some(g)) = (live_textures && !capture.contains(&k), gpu.as_mut()) {
            let _ = g.renderer.render_cube_passes(&v.build_frame());
        }

        if capture.contains(&k) || dump.contains(&k) || k == frames {
            poses.insert(k.to_string(), pose(&mut v));
        }

        if dump.contains(&k) || k == frames {
            v.rt.publish_clone_counts();
            vrml.insert(k.to_string(), json!(v.rt.scene.to_vrml_string()));
        }

        if dump_inlines && (dump.contains(&k) || k == frames) {
            v.rt.publish_clone_counts();
            let mut out = serde_json::Map::new();
            inline_scenes(&v.rt, x3d_scene::ROOT, "", &mut out);
            inlines.insert(k.to_string(), Value::Object(out));
        }

        if capture.contains(&k) {
            if let (Some(g), Some(dir)) = (gpu.as_mut(), png_dir) {
                // A Script's setBrowserOption changed Multisampling, LogarithmicDepthBuffer or Shading.
                v.take_rendering_changes().apply(&mut g.renderer);

                let frame = v.build_frame();
                live_textures |= !frame.cube_passes.is_empty() || !frame.rendered_passes.is_empty();
                let rgba = g.render(&frame, size.0, size.1);
                let rgba: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
                std::fs::create_dir_all(dir).ok();
                let _ = x3d_render::write_png(&dir.join(format!("{k}.png")), size.0, size.1, &rgba);
            }
        }

        events_log.push(json!(v.anchors.len()));

        if audio.is_some() {
            v.rt.audio().resume();
        }
    }

    let audio_result = audio.and_then(|_| {
        let buffer = v.rt.audio().finish()?;
        let file = format!("{}.audio.f32", out.unwrap_or("audio"));
        let mut bytes = Vec::with_capacity(buffer.length() * buffer.number_of_channels() * 4);

        for c in 0..buffer.number_of_channels() {
            for x in buffer.get_channel_data(c) {
                bytes.extend_from_slice(&x.to_le_bytes());
            }
        }

        std::fs::write(&file, bytes).ok()?;
        Some(json!({ "sampleRate": buffer.sample_rate(), "length": buffer.length(), "channels": buffer.number_of_channels(), "file": file }))
    });

    let mut log = v.rt.log.clone();
    log.sort();

    let mut result = json!({
        "ok": true,
        "poses": poses,
        "vrml": vrml,
        "prints": v.rt.prints,
        "log": log,
        "anchors": v.anchors.iter().map(|a| json!({ "description": a.description, "url": a.urls, "parameter": a.parameters })).collect::<Vec<_>>(),
        "status": v.status,
        "title": v.title(),
    });

    if console {
        result["console"] = json!(v.rt.console);
    }

    // Script key "collisionProbes" (see oracle/x_ite-anim.mjs): getClosestObject of the active layer.
    if let Some(directions) = script["collisionProbes"].as_array() {
        let probes: Vec<Value> = directions
            .iter()
            .map(|d| {
                let c = v.probe_closest(x3d_fields::math::Vector3::new(d[0].as_f64().unwrap_or(0.0), d[1].as_f64().unwrap_or(0.0), d[2].as_f64().unwrap_or(0.0)));

                json!({ "distance": c.distance, "id": if c.hit { 0 } else { -1 }, "normal": vec3(&c.normal) })
            })
            .collect();

        result["collisionProbes"] = Value::Array(probes);
    }

    // Script key "pickProbes" (see oracle/x_ite-anim.mjs): the hit at each point, as X_ITE's touch records it.
    if let Some(points) = script["pickProbes"].as_array() {
        let probes: Vec<Value> = points
            .iter()
            .map(|p| {
                let h = v.probe_pick(p[0].as_f64().unwrap_or(0.0), p[1].as_f64().unwrap_or(0.0));

                if !h.id {
                    return json!({ "id": 0 });
                }

                json!({ "id": 1, "point": vec3(&h.point), "normal": vec3(&h.normal), "texCoord": [h.tex_coord.x, h.tex_coord.y, h.tex_coord.z, h.tex_coord.w] })
            })
            .collect();

        result["pickProbes"] = Value::Array(probes);
    }

    if physics {
        let frames: Vec<String> = physics_frames.iter().map(|(k, f)| format!("\"{k}\":{f}")).collect();

        *PHYSICS_TEXT.lock().unwrap() = Some(format!("{{\"dt\":{},\"frames\":{{{}}}}}", Value::Array(physics_dt), frames.join(",")));
        result["physics"] = json!("@@physics@@");
    }

    if dump_inlines {
        result["inlines"] = Value::Object(inlines);
    }

    if audio.is_some() {
        result["audioSuspend"] = json!(audio_suspend);
        result["audio"] = audio_result.unwrap_or(Value::Null);
    }

    result
}

/// The scene text of every DEF-named Inline of `ctx` (nested Inlines as `A/B`): `null` while the
/// Inline has no scene (Inline.getInternalScene () is the empty default scene).
fn inline_scenes(rt: &x3d_runtime::Runtime, ctx: usize, prefix: &str, out: &mut serde_json::Map<String, Value>) {
    for &n in rt.scene.ctx(ctx).named_nodes.values() {
        if rt.builtin_name(n) != Some("Inline") {
            continue;
        }

        let name = format!("{prefix}{}", rt.scene.node(n).name);

        match rt.inline_scenes.get(&n) {
            Some(&c) => {
                out.insert(name.clone(), json!(rt.scene.to_vrml_string_ctx(c)));
                inline_scenes(rt, c, &format!("{name}/"), out);
            }
            None => {
                out.insert(name, Value::Null);
            }
        }
    }
}

/// The `getTimeStep ()` results made since the last call, tagged with the frame.
fn take_dt(v: &mut Viewer3d, frame: u64, out: &mut Vec<Value>) {
    for dt in v.rt.take_physics_dt_log() {
        out.push(json!({ "frame": frame, "dt": dt }));
    }
}

/// The node a path names: a DEF name, or `Inline/Name`.
fn physics_resolve(rt: &x3d_runtime::Runtime, path: &str) -> Option<x3d_fields::NodeId> {
    let mut ctx = x3d_scene::ROOT;
    let mut node = None;
    let parts: Vec<&str> = path.split('/').collect();

    for (i, part) in parts.iter().enumerate() {
        let n = rt.scene.ctx(ctx).named_nodes.get(*part)?;

        node = Some(n);

        if i + 1 < parts.len() {
            if rt.builtin_name(n) != Some("Inline") {
                return None;
            }

            ctx = *rt.inline_scenes.get(&n)?;
        }
    }

    node
}

/// A node as the oracle's `plain ()` names it: its DEF name, or `<Type>`.
fn node_name(rt: &x3d_runtime::Runtime, n: x3d_fields::NodeId) -> Value {
    let name = &rt.scene.node(n).name;

    if name.is_empty() { json!(format!("<{}>", rt.type_name(n))) } else { json!(name) }
}

fn plain(rt: &x3d_runtime::Runtime, value: &x3d_fields::FieldValue) -> Value {
    use x3d_fields::FieldValue as F;

    match value {
        F::SFBool(b) => json!(b),
        F::SFFloat(x) | F::SFDouble(x) | F::SFTime(x) => json!(x),
        F::SFInt32(x) => json!(x),
        F::SFString(s) => json!(s),
        F::SFVec3f(x) | F::SFVec3d(x) => json!([x.x, x.y, x.z]),
        F::SFVec2f(x) | F::SFVec2d(x) => json!([x.x, x.y]),
        F::SFRotation(r) => {
            let mut r = *r;

            json!(r.components())
        }
        F::SFNode(None) => Value::Null,
        F::SFNode(Some(n)) => node_name(rt, *n),
        F::MFNode(list) => Value::Array(list.iter().map(|n| n.map_or(Value::Null, |n| node_name(rt, n))).collect()),
        F::MFString(list) => json!(list),
        F::MFVec3f(v) => Value::Array(v.chunks_exact(3).map(|c| json!([c[0], c[1], c[2]])).collect()),
        F::MFFloat(v) => json!(v),
        other => json!(format!("{other:?}")),
    }
}

/// The nodes and fields of the script's `physics.nodes`, in the order the script lists them (a JSON
/// object read by hand: serde_json sorts the keys).
fn physics_nodes(script: &str) -> Vec<(String, Vec<String>)> {
    fn skip_ws(b: &[u8], i: &mut usize) {
        while *i < b.len() && b[*i].is_ascii_whitespace() {
            *i += 1;
        }
    }

    fn string(b: &[u8], i: &mut usize) -> String {
        let start = *i;

        *i += 1;

        while b[*i] != b'"' {
            if b[*i] == b'\\' {
                *i += 1;
            }

            *i += 1;
        }

        *i += 1;
        serde_json::from_slice(&b[start..*i]).unwrap_or_default()
    }

    let b = script.as_bytes();
    let Some(p) = script.find("\"physics\"") else {
        return vec![];
    };
    let Some(n) = script[p..].find("\"nodes\"") else {
        return vec![];
    };
    let mut i = p + n + "\"nodes\"".len();
    let mut out = vec![];

    skip_ws(b, &mut i);
    i += 1; // :
    skip_ws(b, &mut i);
    i += 1; // {

    loop {
        skip_ws(b, &mut i);

        match b[i] {
            b'}' => break,
            b',' => i += 1,
            b'"' => {
                let path = string(b, &mut i);
                let mut fields = vec![];

                skip_ws(b, &mut i);
                i += 1; // :
                skip_ws(b, &mut i);
                i += 1; // [

                loop {
                    skip_ws(b, &mut i);

                    match b[i] {
                        b']' => {
                            i += 1;
                            break;
                        }
                        b',' => i += 1,
                        _ => fields.push(string(b, &mut i)),
                    }
                }

                out.push((path, fields));
            }
            _ => break,
        }
    }

    out
}

/// `__physSnap`: the listed fields of the listed nodes, as JSON text in the listed order.
fn physics_snapshot(v: &mut Viewer3d, nodes: &[(String, Vec<String>)]) -> String {
    let mut out: Vec<String> = vec![];

    for (path, fields) in nodes {
        let key = serde_json::to_string(path).unwrap();
        let Some(node) = physics_resolve(&v.rt, path) else {
            out.push(format!("{key}:null"));
            continue;
        };
        let ty = v.rt.type_name(node);
        let mut o = vec![format!("\"type\":{}", serde_json::to_string(&ty).unwrap())];

        for f in fields {
            let value = if f == "@bodyActive" {
                v.rt.physics_body_active(node).map_or("null".to_string(), |b| b.to_string())
            } else if f == "contacts" && ty == "CollisionSensor" {
                let list = match v.rt.pre_value(node, "contacts") {
                    Some(x3d_fields::FieldValue::MFNode(l)) => l.clone(),
                    _ => vec![],
                };
                let items: Vec<String> = list
                    .into_iter()
                    .flatten()
                    .map(|c| {
                        let members: Vec<String> = ["position", "contactNormal", "depth", "geometry1", "geometry2", "body1", "body2"]
                            .iter()
                            .map(|n| format!("\"{n}\":{}", v.rt.pre_value(c, n).map_or(Value::Null, |x| plain(&v.rt, x))))
                            .collect();

                        format!("{{{}}}", members.join(","))
                    })
                    .collect();

                format!("[{}]", items.join(","))
            } else {
                v.rt.pre_value(node, f).map_or(Value::Null, |x| plain(&v.rt, x)).to_string()
            };

            o.push(format!("{}:{value}", serde_json::to_string(f).unwrap()));
        }

        out.push(format!("{key}:{{{}}}", o.join(",")));
    }

    format!("{{{}}}", out.join(","))
}
