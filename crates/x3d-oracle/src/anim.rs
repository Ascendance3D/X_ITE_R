//! `x3d-oracle anim | anim-msaa | anim-sized`: oracle/x_ite-anim.mjs, x_ite-anim-msaa.mjs and
//! x_ite-anim-sized.mjs in Rust. Same arguments, same script keys, same page calls in the same order, same
//! js.json bytes and PNG bytes. Each variant keeps exactly the features its Node original has.

use crate::json::{self, J, js_number, js_round};
use crate::page::Page;
use crate::page_js as js;
use crate::{Common, input::Input};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    /// oracle/x_ite-anim.mjs (page anim.html, fixed 512x512 canvas).
    Anim,
    /// oracle/x_ite-anim-msaa.mjs (page anim-msaa.html, --aa / --ms).
    Msaa,
    /// oracle/x_ite-anim-sized.mjs (page anim-sized.html).
    Sized,
}

impl Variant {
    fn settle(self) -> bool {
        self != Variant::Sized
    }
    fn full(self) -> bool {
        self == Variant::Anim
    }
}

/// `process.argv.slice (2)` helpers of the Node tools.
pub struct Argv(pub Vec<String>);

impl Argv {
    pub fn opt(&self, name: &str) -> Option<String> {
        let i = self.0.iter().position(|a| a == name)?;
        Some(self.0.get(i + 1).cloned().unwrap_or_default())
    }

    /// The positional arguments: not starting with "--" and not following an argument that does (the MSAA
    /// tool exempts "--aa", which takes no value).
    pub fn positional(&self, aa_is_flag: bool) -> Vec<String> {
        let a = &self.0;
        (0..a.len())
            .filter(|&i| {
                !a[i].starts_with("--") && !(i > 0 && a[i - 1].starts_with("--") && !(aa_is_flag && a[i - 1] == "--aa"))
            })
            .map(|i| a[i].clone())
            .collect()
    }
}

/// Lexical `path.resolve`.
pub fn resolve(p: &str) -> PathBuf {
    if Path::new(p).is_absolute() {
        return crate::server::join_normalize(Path::new("/"), p);
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    crate::server::join_normalize(&cwd, p)
}

/// `path.relative (from, to)` for absolute, normalized paths.
pub fn relative(from: &Path, to: &Path) -> String {
    let f: Vec<_> = from.components().collect();
    let t: Vec<_> = to.components().collect();
    let common = f.iter().zip(&t).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".into(); f.len() - common];
    parts.extend(t[common..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()));
    parts.join("/")
}

fn num(v: Option<&Value>, def: f64) -> f64 {
    match v {
        None | Some(Value::Null) => def,
        Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(f64::NAN),
        Some(Value::Bool(b)) => *b as u8 as f64,
        Some(_) => f64::NAN,
    }
}

/// `new Set (script .capture ?? [ ])`: only numbers can equal the frame number k.
fn num_set(v: Option<&Value>) -> Vec<f64> {
    v.and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default()
}

/// The JavaScript text of a script value passed to the page.
fn lit(v: Option<&Value>) -> String {
    match v {
        None => "undefined".into(),
        Some(v) => serde_json::to_string(v).unwrap(),
    }
}

struct Run<'a> {
    page: &'a Page,
    variant: Variant,
}

impl Run<'_> {
    fn eval(&self, f: &str, arg: Option<&str>) -> Result<Option<String>, String> {
        self.page.evaluate(f, arg)
    }

    /// The page result as a parsed tree (JSON text kept exact).
    fn eval_j(&self, f: &str, arg: Option<&str>) -> Result<J, String> {
        match self.eval(f, arg)? {
            None => Ok(J::Undef),
            Some(t) => json::parse(&t).map_err(|e| format!("x3d-oracle: bad page JSON: {e}")),
        }
    }

    fn settle(&self, enabled: bool) -> Result<(), String> {
        if !(self.variant.settle() && enabled) {
            return Ok(());
        }
        let mut quiet = 0;
        while quiet < 3 {
            while self.page.in_flight() > 0 {
                if std::time::Instant::now() >= self.page.deadline {
                    return Err("x3d-oracle: timeout waiting for requests to settle".into());
                }
                if let Some(c) = self.page.closed_reason() {
                    return Err(format!("page.evaluate: {c}"));
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            self.eval(js::YIELD_TASK, None)?;
            self.eval(js::YIELD_TASK, None)?;
            quiet = if self.page.in_flight() > 0 { 0 } else { quiet + 1 };
        }
        Ok(())
    }
}

fn mouse_button(b: Option<&Value>) -> &'static str {
    let key = match b {
        Some(Value::Number(n)) => js_number(n.as_f64().unwrap_or(f64::NAN)),
        Some(Value::String(s)) => s.clone(),
        _ => return "left",
    };
    match key.as_str() {
        "0" => "left",
        "1" => "middle",
        "2" => "right",
        _ => "left",
    }
}

/// Runs one script; returns (js.json text, ok).
pub fn run(variant: Variant, argv: &Argv, common: &Common) -> Result<(String, bool), String> {
    let pos = argv.positional(variant == Variant::Msaa);
    let (file_arg, script_arg) = match (pos.first(), pos.get(1)) {
        (Some(f), Some(s)) => (f.clone(), s.clone()),
        _ => return Err("usage: x3d-oracle anim|anim-msaa|anim-sized <world inside oracle/> <script.json> [--out result.json] [--png-dir dir]".into()),
    };
    let script_text = std::fs::read_to_string(&script_arg).map_err(|e| format!("ENOENT: {script_arg}: {e}"))?;
    let script: Value = serde_json::from_str(&script_text).map_err(|e| format!("SyntaxError: {script_arg}: {e}"))?;
    let script_raw = json::parse(&script_text).map_err(|e| format!("SyntaxError: {script_arg}: {e}"))?;
    let g = |k: &str| script.get(k).filter(|v| !v.is_null());

    let size = g("size").and_then(Value::as_array);
    let w = size.and_then(|a| a.first()).map(|v| num(Some(v), 512.0)).unwrap_or(512.0);
    let h = size.and_then(|a| a.get(1)).map(|v| num(Some(v), 512.0)).unwrap_or(512.0);
    let t0 = num(g("t0"), 946684800.0);
    let dt = num(g("dt"), 1.0 / 60.0);
    let frames = num(g("frames"), 1.0);
    let warmup = num(g("warmup"), 8.0);
    let capture: BTreeSet<u64> = num_set(g("capture")).into_iter().map(f64::to_bits).collect();
    let dump: BTreeSet<u64> = num_set(g("dump")).into_iter().map(f64::to_bits).collect();
    let events: Vec<Value> = g("events").and_then(Value::as_array).cloned().unwrap_or_default();
    let png_dir = argv.opt("--png-dir").unwrap_or_default();
    let truthy = |k: &str| json::truthy(script.get(k));

    let ms = if variant == Variant::Msaa { argv.opt("--ms").unwrap_or_default() } else { String::new() };
    let aa = if argv.0.iter().any(|a| a == "--aa") || !ms.is_empty() { "1" } else { "0" };

    // Script key "audio" (anim only).
    let audio = (variant.full() && truthy("audio")).then(|| {
        let a = &script["audio"];
        (num(a.get("sampleRate").filter(|v| !v.is_null()), 48000.0), num(a.get("channels").filter(|v| !v.is_null()), 2.0), num(a.get("tail").filter(|v| !v.is_null()), 4.0))
    });
    let audio_quantum = |k: f64, sr: f64| (js_round(k * dt * sr) / 128.0).ceil();

    let mut result = J::obj();
    result.set("ok", J::bool(false));
    result.set("poses", J::obj());
    result.set("vrml", J::obj());
    result.set("prints", J::Arr(vec![]));
    result.set("log", J::Arr(vec![]));
    result.set("anchors", J::Arr(vec![]));

    let server = crate::server::Server::start(common.root.clone()).map_err(|e| format!("x3d-oracle: http server: {e}"))?;
    let base = format!("http://127.0.0.1:{}", server.port);
    let track = variant.settle() && truthy("settle");
    let console = variant.full() && truthy("console");

    let mut page = Page::open(&common.chrome, crate::chrome::ORACLE_ARGS, w as u32, h as u32, &base, common.trace.clone(), common.deadline)?;
    crate::watchdog(&page, common);
    page.set_console(console);
    let console_from = 0;

    let outcome = (|| -> Result<(), String> {
        let r = Run { page: &page, variant };
        if let Some((sr, ch, tail)) = audio {
            let length = (audio_quantum(frames, sr) + tail) * 128.0;
            let arg = format!(
                "{{\"sampleRate\":{},\"channels\":{},\"tail\":{},\"length\":{}}}",
                js_number(sr),
                js_number(ch),
                js_number(tail),
                js_number(length)
            );
            page.add_init_script(&format!("({})({})", js::AUDIO_INIT, arg))?;
        }
        let url = match variant {
            Variant::Anim => format!("{base}/x_ite-pin/src/anim.html"),
            Variant::Msaa => format!(
                "{base}/x_ite-pin/src/anim-msaa.html?w={}&h={}&aa={aa}{}",
                js_number(w),
                js_number(h),
                if ms.is_empty() { String::new() } else { format!("&ms={ms}") }
            ),
            Variant::Sized => format!("{base}/x_ite-pin/src/anim-sized.html?w={}&h={}", js_number(w), js_number(h)),
        };
        page.goto(&url)?;
        page.wait_for_function(js::READY, 30000)?;

        if variant != Variant::Msaa && truthy("pinFrameRate") {
            r.eval(js::PIN_FRAME_RATE, Some(&js_number(1.0 / dt.max(1e-9))))?;
        }
        if variant.full() && truthy("physics") {
            r.eval(js::PHYSICS_SETUP, None)?;
        }

        let abs = resolve(&file_arg);
        let rel = relative(&common.root, &abs);
        let world = format!("{base}/{}", rel.split('/').map(json::encode_uri_component).collect::<Vec<_>>().join("/"));
        let load_arg = format!("[{},{},{},{}]", json::escape(&world), js_number(t0), js_number(dt), js_number(warmup));
        let loaded_text = r.eval(js::LOAD, Some(&load_arg))?.unwrap_or_else(|| "{}".into());
        let loaded = json::parse(&loaded_text).map_err(|e| format!("x3d-oracle: bad page JSON: {e}"))?;
        let loaded_v: Value = serde_json::from_str(&loaded_text).unwrap_or(Value::Null);

        if variant == Variant::Msaa {
            result.set("info", r.eval_j(js::INFO, None)?);
        }
        result.set("loadFrames", loaded.member("loadFrames"));
        result.set("warmupFrames", loaded.member("frames"));

        if !json::truthy(loaded_v.get("ok")) {
            // throw new Error (loaded .error)
            return Err(match loaded_v.get("error") {
                None | Some(Value::Null) => String::new(),
                Some(Value::String(s)) => s.clone(),
                Some(v) => v.to_string(),
            });
        }

        r.eval(js::SET_DT, Some(&js_number(dt)))?;
        result.set("geometry", r.eval_j(js::GEOMETRY, None)?);
        r.eval(js::FOCUS, None)?;

        r.settle(track)?;

        // Object key order matters here (Object .entries in __physSnap): taken from the script text as written.
        let physics_nodes = script_raw.member("physics").member("nodes").compact();
        let physics = variant.full() && truthy("physics");
        if physics {
            let mut ph = J::obj();
            ph.set("dt", J::Arr(vec![]));
            let mut fr = J::obj();
            fr.set("0", r.eval_j(js::PHYS_SNAP, Some(&physics_nodes))?);
            ph.set("frames", fr);
            result.set("physics", ph);
        }

        if let Some((sr, _, _)) = audio {
            let n = frames.max(0.0) as u64;
            let times: Vec<String> = (1..=n).map(|k| js_number((audio_quantum(k as f64, sr) * 128.0 + 0.5) / sr)).collect();
            r.eval(js::AUDIO_ARM, Some(&format!("[{}]", times.join(","))))?;
            result.set("audioSuspend", J::Arr(vec![]));
        }

        let mut input = Input::default();
        let mut k = 1.0f64;
        while k <= frames {
            let prev = t0 + (k - 1.0) * dt;

            if let Some((sr, _, _)) = audio {
                let t = r.eval(js::AUDIO_WAIT, Some(&js_number(k)))?.unwrap_or_default();
                let v: f64 = t.parse().unwrap_or(f64::NAN);
                if let Some(J::Arr(a)) = result.get_mut("audioSuspend") {
                    a.push(J::num(js_round(v * sr)));
                }
                r.eval(js::AUDIO_FLUSH, None)?;
            }

            for e in events.iter().filter(|e| e.get("frame").and_then(Value::as_f64) == Some(k)) {
                let et = num(e.get("t").filter(|v| !v.is_null()), 0.0);
                r.eval(js::SET_DATE, Some(&js_number((prev + et) * 1000.0)))?;
                let x = num(e.get("x"), f64::NAN);
                let y = h - num(e.get("y"), f64::NAN);
                match e.get("type").and_then(Value::as_str).unwrap_or("") {
                    "mousedown" => {
                        page.input(input.mouse_move(x, y), "mouse.move")?;
                        page.input(input.mouse_down(mouse_button(e.get("button")), 1), "mouse.down")?;
                    }
                    "mouseup" => {
                        page.input(input.mouse_move(x, y), "mouse.move")?;
                        page.input(input.mouse_up(mouse_button(e.get("button")), 1), "mouse.up")?;
                    }
                    "mousemove" => page.input(input.mouse_move(x, y), "mouse.move")?,
                    "wheel" => {
                        page.input(input.mouse_move(x, y), "mouse.move")?;
                        page.input(input.mouse_wheel(0.0, num(e.get("deltaY"), f64::NAN)), "mouse.wheel")?;
                    }
                    "dblclick" => page.input(input.mouse_dblclick(x, y), "mouse.dblclick")?,
                    "viewer" => {
                        r.eval(js::SET_VIEWER, Some(&lit(e.get("name"))))?;
                    }
                    "resize" if variant.settle() => {
                        r.eval(js::RESIZE, Some(&format!("[{},{}]", lit(e.get("width")), lit(e.get("height")))))?;
                        r.eval(js::TWO_FRAMES, None)?;
                    }
                    "key" => {
                        let key = e.get("key").and_then(Value::as_str).unwrap_or("");
                        page.input(input.key_press(key).map_err(|m| format!("keyboard.press: {m}"))?, "keyboard.press")?;
                    }
                    "keydown" if variant.full() => {
                        let key = e.get("key").and_then(Value::as_str).unwrap_or("");
                        page.input(input.key_down(key).map_err(|m| format!("keyboard.down: {m}"))?, "keyboard.down")?;
                    }
                    "keyup" if variant.full() => {
                        let key = e.get("key").and_then(Value::as_str).unwrap_or("");
                        page.input(input.key_up(key).map_err(|m| format!("keyboard.up: {m}"))?, "keyboard.up")?;
                    }
                    "mods" => {
                        for (prop, key) in [("shift", "Shift"), ("ctrl", "Control"), ("alt", "Alt")] {
                            let cmds = if json::truthy(e.get(prop)) { input.key_down(key) } else { input.key_up(key) };
                            page.input(cmds?, "keyboard.down")?;
                        }
                    }
                    _ => {}
                }
            }

            r.settle(track)?;
            r.eval(js::SET_DATE, Some(&js_number((t0 + k * dt) * 1000.0)))?;
            if physics {
                r.eval(js::PH_FRAME, Some(&js_number(k)))?;
            }
            r.eval(js::STEP, Some(&js_number(t0 + k * dt)))?;
            r.settle(track)?;

            let key = js_number(k);
            if physics {
                let snap = r.eval_j(js::PHYS_SNAP, Some(&physics_nodes))?;
                if let Some(J::Obj(_)) = result.get_mut("physics")
                    && let Some(fr) = result.get_mut("physics").and_then(|p| p.get_mut("frames")) {
                        fr.set(&key, snap);
                    }
            }
            if audio.is_some() {
                r.eval(js::AUDIO_RESUME, None)?;
            }

            let kb = k.to_bits();
            let last = k == frames;
            if capture.contains(&kb) || dump.contains(&kb) || last {
                let pose = r.eval_j(js::POSE, None)?;
                result.get_mut("poses").unwrap().set(&key, pose);
            }
            if dump.contains(&kb) || last {
                let v = r.eval_j(js::VRML, None)?;
                result.get_mut("vrml").unwrap().set(&key, v);
            }
            if variant.full() && truthy("dumpInlines") && (dump.contains(&kb) || last) {
                if result.get_mut("inlines").is_none() {
                    result.set("inlines", J::obj());
                }
                let v = r.eval_j(js::INLINES, None)?;
                result.get_mut("inlines").unwrap().set(&key, v);
            }
            if capture.contains(&kb) && !png_dir.is_empty() {
                let t = r.eval(js::PIXELS, None)?.unwrap_or_default();
                let p: Value = serde_json::from_str(&t).map_err(|e| format!("x3d-oracle: pixels: {e}"))?;
                let (pw, ph) = (p["width"].as_u64().unwrap_or(0) as u32, p["height"].as_u64().unwrap_or(0) as u32);
                let rgba = crate::png::flip_rows(&json::base64_decode(p["rgba"].as_str().unwrap_or("")), pw, ph);
                std::fs::create_dir_all(&png_dir).map_err(|e| format!("EACCES: {png_dir}: {e}"))?;
                let path = Path::new(&png_dir).join(format!("{key}.png"));
                std::fs::write(&path, crate::png::encode(pw, ph, &rgba)).map_err(|e| format!("{}: {e}", path.display()))?;
            }
            k += 1.0;
        }

        if physics {
            let v = r.eval_j(js::PH_DT, None)?;
            result.get_mut("physics").unwrap().set("dt", v);
        }
        if variant.settle() && truthy("pickProbes") {
            result.set("pickProbes", r.eval_j(js::PICK_PROBES, Some(&lit(script.get("pickProbes"))))?);
        }
        if variant.settle() && truthy("collisionProbes") {
            result.set("collisionProbes", r.eval_j(js::COLLISION_PROBES, Some(&lit(script.get("collisionProbes"))))?);
        }

        let log = r.eval(js::GET_LOG, None)?.unwrap_or_else(|| "[]".into()).replace(&base, "http://oracle");
        result.set("log", json::parse(&log).map_err(|e| format!("x3d-oracle: bad page JSON: {e}"))?);

        if console {
            let lines: Vec<J> = page
                .console_since(console_from)
                .into_iter()
                .filter(|(t, _)| t == "log" || t == "warning" || t == "error")
                .map(|(t, text)| {
                    let text = text.replace(&base, "http://oracle");
                    J::str(&if t == "log" {
                        format!("log: {text}")
                    } else {
                        format!("{}: {}", if t == "warning" { "warn" } else { "error" }, text.split('\n').next().unwrap_or(""))
                    })
                })
                .collect();
            result.set("console", J::Arr(lines));
        }

        if audio.is_some() {
            let t = r.eval(js::AUDIO_DONE, None)?.unwrap_or_default();
            let a = json::parse(&t).map_err(|e| format!("x3d-oracle: bad page JSON: {e}"))?;
            let v: Value = serde_json::from_str(&t).unwrap_or(Value::Null);
            let chans = v["channels"].as_array().cloned().unwrap_or_default();
            let mut data = Vec::new();
            for c in &chans {
                data.extend(json::base64_decode(c.as_str().unwrap_or("")));
            }
            let file = format!("{}.audio.f32", argv.opt("--out").unwrap_or_else(|| "audio".into()));
            let mut o = J::obj();
            o.set("sampleRate", a.member("sampleRate"));
            o.set("length", a.member("length"));
            o.set("channels", J::int(chans.len() as i64));
            o.set("file", J::str(&file));
            result.set("audio", o);
            std::fs::write(&file, data).map_err(|e| format!("{file}: {e}"))?;
        }

        result.set("ok", J::bool(true));
        Ok(())
    })();

    if let Err(e) = outcome {
        result.set("error", J::str(e.split('\n').next().unwrap_or("")));
    }
    for err in page.page_errors() {
        eprintln!("x3d-oracle: page error: {err}");
    }
    if let Some(c) = page.closed_reason() {
        eprintln!("x3d-oracle: browser: {c}");
        for l in page.chrome.stderr_tail() {
            eprintln!("x3d-oracle: chrome: {l}");
        }
    }
    page.close();
    let ok = matches!(result.member("ok"), J::Tok(ref t) if t == "true");
    Ok((result.pretty().unwrap(), ok))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_like_node() {
        let a = Argv(["w.wrl", "--out", "o.json", "s.json", "--png-dir", "d"].map(String::from).to_vec());
        assert_eq!(a.positional(false), vec!["w.wrl", "s.json"]);
        assert_eq!(a.opt("--out").as_deref(), Some("o.json"));
        let a = Argv(["--aa", "w.wrl", "s.json", "--ms", "4"].map(String::from).to_vec());
        assert_eq!(a.positional(true), vec!["w.wrl", "s.json"]);
        assert_eq!(a.positional(false), vec!["s.json"]);
    }

    #[test]
    fn page_js_is_verbatim() {
        let anim = include_str!("../../../oracle/x_ite-anim.mjs");
        let msaa = include_str!("../../../oracle/x_ite-anim-msaa.mjs");
        let sized = include_str!("../../../oracle/x_ite-anim-sized.mjs");
        for c in [
            js::AUDIO_INIT, js::PIN_FRAME_RATE, js::PHYSICS_SETUP, js::READY, js::LOAD, js::SET_DT, js::GEOMETRY, js::FOCUS, js::PHYS_SNAP,
            js::AUDIO_ARM, js::AUDIO_WAIT, js::AUDIO_FLUSH, js::SET_DATE, js::SET_VIEWER, js::RESIZE, js::TWO_FRAMES, js::PH_FRAME, js::STEP,
            js::AUDIO_RESUME, js::VRML, js::PIXELS, js::PH_DT, js::GET_LOG, js::AUDIO_DONE, js::YIELD_TASK, js::POSE, js::INLINES,
            js::PICK_PROBES, js::COLLISION_PROBES,
        ] {
            assert!(anim.contains(c), "not in x_ite-anim.mjs: {c}");
        }
        for c in [js::READY, js::LOAD, js::SET_DT, js::GEOMETRY, js::FOCUS, js::SET_DATE, js::STEP, js::POSE, js::VRML, js::PIXELS, js::GET_LOG] {
            assert!(msaa.contains(c) && sized.contains(c), "variant drift: {c}");
        }
        for c in [js::INFO, js::RESIZE, js::TWO_FRAMES, js::YIELD_TASK, js::PICK_PROBES, js::COLLISION_PROBES] {
            assert!(msaa.contains(c), "not in x_ite-anim-msaa.mjs: {c}");
        }
        assert!(sized.contains(js::PIN_FRAME_RATE));
    }

    #[test]
    fn relative_paths() {
        assert_eq!(relative(Path::new("/a/oracle"), Path::new("/a/oracle/x/y z.wrl")), "x/y z.wrl");
        assert_eq!(relative(Path::new("/a/oracle"), Path::new("/a/b.wrl")), "../b.wrl");
    }
}
