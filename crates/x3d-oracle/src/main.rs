//! x3d-oracle: the X_ITE reference runs of oracle/*.mjs without Node.js or npm.
//!
//! ```text
//! x3d-oracle anim        <world inside oracle/> <script.json> [--out result.json] [--png-dir dir]
//! x3d-oracle anim-msaa   <world> <script.json> [--out f] [--png-dir d] [--aa] [--ms N]
//! x3d-oracle anim-sized  <world> <script.json> [--out f] [--png-dir d]
//! x3d-oracle particles   <world> <script.json> [--out particles.json] [--variant b07|events|06r]
//!                        [--png-dir d] [--aa] [--ms N] [--png-size WxH]
//! ```
//!
//! Options of this tool (all modes): `--chrome PATH` (default $X3D_ORACLE_CHROME or /usr/bin/google-chrome,
//! the executable the Node tools launch), `--root DIR` (the served oracle/ directory; default
//! $X3D_ORACLE_ROOT or this repository's oracle/), `--timeout SECONDS` (whole run, default 1800),
//! `--trace-cdp FILE` (every DevTools message). Output, exit status and files are those of the Node tool.

mod anim;
mod chrome;
mod input;
mod json;
mod page;
mod page_js;
mod particles;
mod particles_js;
mod png;
mod server;

use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct Common {
    pub chrome: String,
    pub root: PathBuf,
    pub deadline: Instant,
    pub timeout: Duration,
    pub trace: Option<PathBuf>,
}

/// Last line of defence: if the run is still going 15 s after its deadline (a blocked pipe write, a hung
/// browser), kill the browser's process group and exit 1.
pub fn watchdog(page: &page::Page, common: &Common) {
    let pid = page.chrome.pid();
    let at = common.deadline + Duration::from_secs(15);
    let timeout = common.timeout;
    let _ = std::thread::Builder::new()
        .name("watchdog".into())
        .spawn(move || {
            while Instant::now() < at {
                std::thread::sleep(Duration::from_millis(200));
            }
            eprintln!(
                "x3d-oracle: watchdog: run exceeded {} s, killing the browser",
                timeout.as_secs()
            );
            #[cfg(unix)]
            if let Some(pid) = pid {
                // SAFETY: plain syscall on the browser's process group.
                unsafe {
                    libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
                }
            }
            #[cfg(not(unix))]
            let _ = pid;
            std::process::exit(1);
        });
}

fn default_root() -> PathBuf {
    if let Some(r) = std::env::var_os("X3D_ORACLE_ROOT") {
        return PathBuf::from(r);
    }
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle");
    if here.is_dir() {
        here
    } else {
        PathBuf::from("oracle")
    }
}

const USAGE: &str = "usage: x3d-oracle <anim|anim-msaa|anim-sized> <world inside oracle/> <script.json> [--out result.json] [--png-dir dir] [--aa] [--ms N]\n       x3d-oracle particles <world inside oracle/> <script.json> [--out particles.json] [--variant b07|events|06r] [--png-dir dir] [--aa] [--ms N] [--png-size WxH]\n       [--chrome PATH] [--root DIR] [--timeout SECONDS] [--trace-cdp FILE]";

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        println!("{USAGE}");
        std::process::exit(if args.is_empty() { 2 } else { 0 });
    }
    let mode = args.remove(0);
    let argv = anim::Argv(args);
    let timeout = Duration::from_secs(
        argv.opt("--timeout")
            .and_then(|t| t.parse().ok())
            .unwrap_or(1800),
    );
    let root = argv
        .opt("--root")
        .map(PathBuf::from)
        .unwrap_or_else(default_root);
    let root = match std::fs::canonicalize(&root) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("x3d-oracle: oracle root {}: {e}", root.display());
            std::process::exit(1);
        }
    };
    let common = Common {
        chrome: argv
            .opt("--chrome")
            .or_else(|| std::env::var("X3D_ORACLE_CHROME").ok())
            .unwrap_or_else(|| "/usr/bin/google-chrome".into()),
        root,
        deadline: Instant::now() + timeout,
        timeout,
        trace: argv.opt("--trace-cdp").map(PathBuf::from),
    };
    if mode == "particles" {
        match particles::run(&argv, &common) {
            Ok((_, ok, line)) => {
                println!("{line}");
                std::process::exit(if ok { 0 } else { 1 });
            }
            Err(e) => {
                eprintln!("x3d-oracle: {e}");
                std::process::exit(1);
            }
        }
    }
    let variant = match mode.as_str() {
        "anim" => anim::Variant::Anim,
        "anim-msaa" => anim::Variant::Msaa,
        "anim-sized" => anim::Variant::Sized,
        _ => {
            eprintln!("x3d-oracle: unknown mode {mode:?}\n{USAGE}");
            std::process::exit(2);
        }
    };
    match anim::run(variant, &argv, &common) {
        Ok((text, ok)) => {
            match argv.opt("--out").filter(|o| !o.is_empty()) {
                Some(out) => {
                    if let Err(e) = std::fs::write(&out, &text) {
                        eprintln!("x3d-oracle: {out}: {e}");
                        std::process::exit(1);
                    }
                }
                None => println!("{text}"),
            }
            std::process::exit(if ok { 0 } else { 1 });
        }
        Err(e) => {
            // The Node tools die with an uncaught exception here (bad arguments, unreadable script, no browser).
            eprintln!("x3d-oracle: {e}");
            std::process::exit(1);
        }
    }
}
