//! x3d-view <file.wrl> [--oracle <dir>] [--size WxH] [--msaa 0|1|2|4|8|16] [--depth standard|log|reversed]
//!       [--script script.json --png out.png]
//!
//! Opens a VRML/X3D world in a native window and runs it live: TimeSensors, interpolators,
//! Scripts, routes, navigation (EXAMINE / WALK / FLY / NONE) and the pointing device sensors.
//! Anchor activations are reported in the title bar, never opened.
//!
//! Keys go to the world (KeySensor, StringSensor; Home/End first/last viewpoint, PageUp/PageDown
//! previous/next viewpoint, as in X_ITE). The viewer's own shortcuts take Ctrl (Linux) or Command
//! (macOS): `1` EXAMINE, `2` WALK, `3` FLY, `4` NONE, `l` list viewpoints, `m` cycle multisampling
//! (default 4, like X_ITE), `z` cycle the depth buffer (standard = X_ITE, reversed, log), `s` save
//! /tmp/x3d-view-N.png, `q` quit. Escape is a world key; close the window to quit.

use std::path::PathBuf;
use x3d_viewer::window::{Options, run};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let opt = |f: &str| args.iter().position(|a| a == f).and_then(|i| args.get(i + 1)).cloned();

    let Some(path) = args.get(1).filter(|a| !a.starts_with("--")) else {
        eprintln!("usage: x3d-view <file.wrl> [--oracle dir] [--size WxH] [--msaa N] [--depth standard|log|reversed] [--script script.json --png out.png]");
        std::process::exit(2);
    };

    let script = opt("--script").map(|p| serde_json::from_str(&std::fs::read_to_string(p).expect("script")).expect("script json"));
    let mut size = (1024, 768);

    if let Some(s) = opt("--size") {
        if let Some((w, h)) = s.split_once('x') {
            size = (w.parse().unwrap_or(1024), h.parse().unwrap_or(768));
        }
    } else if let Some(s) = script.as_ref().and_then(|s: &serde_json::Value| s["size"].as_array()) {
        size = (s[0].as_u64().unwrap_or(512) as u32, s[1].as_u64().unwrap_or(512) as u32);
    }

    let opts = Options { path: PathBuf::from(path), oracle: opt("--oracle").map(PathBuf::from), script, png: opt("--png").map(PathBuf::from), size, msaa: opt("--msaa").and_then(|v| v.parse().ok()), depth: opt("--depth").map(|v| x3d_viewer::embed::DepthMode::parse(&v).unwrap_or_else(|| { eprintln!("--depth: standard, log or reversed"); std::process::exit(2) })) };

    // The Sound component plays on the default output device.
    x3d_runtime::sound::set_default_mode(x3d_runtime::sound::AudioMode::Realtime);

    // QuickJS wants a big stack (x3d-shot uses the same 256 MB).
    #[cfg(not(target_os = "macos"))]
    let r = std::thread::Builder::new().stack_size(256 << 20).spawn(move || run(opts)).unwrap().join().unwrap();

    // macOS: the event loop must run on the main thread, and its stack is at most 64 MB
    // (RLIMIT_STACK hard limit). The main thread switches to a 256 MB stack instead.
    #[cfg(target_os = "macos")]
    let r = stacker::grow(256 << 20, move || run(opts));

    match r {
        Ok(n) => eprintln!("presented {n} frames"),
        Err(e) => {
            eprintln!("x3d-view: {e}");
            std::process::exit(1);
        }
    }
}
