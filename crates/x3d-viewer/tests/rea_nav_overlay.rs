//! FLY / WALK rubber band (F15) and ContentScale (F17) of `sub:nav.walk-fly` (issue #51) against pinned X_ITE 16.5.0.
//!
//! Each case of oracle/synthetic-rea-nav-overlay/cases.txt (`name world script`, made by oracle/tools/rea_nav_overlay.py gen) runs
//! `x3d-drive` and is compared with oracle/rea-nav-overlay-baseline/NAME.json (recorded by `rea_nav_overlay.py record`, three
//! identical X_ITE runs):
//!
//! * poses at the captured frames: position and centerOfRotation within 1e-4, orientation angle within 1e-4, fieldOfViewScale
//!   within 1e-4 (the tolerances of rea_b24_nav);
//! * for a case with `"pixels": true`, each captured frame: the pixels that differ from the sky color (the first pixel) in either
//!   picture, compared channel by channel, at most `PIXEL_TOL` apart; the pinned picture must show the rubber band exactly when
//!   Rust's does (a case without a band is checked to have none).
//!
//! The pictures need the Vulkan adapter (the viewer's pointing and depth passes too); without one the tests record a
//! `no-gpu-adapter` skip, which `X3D_EVIDENCE=strict` turns into a failure.
//!
//! At Multisampling 4, the rubber band cases of oracle/rea-nav-overlay-msaa4-baseline/cases.txt (`rea_nav_overlay.py record --msaa 4`:
//! oracle/x_ite-anim-msaa.mjs, three identical runs) run `x3d-drive --msaa 4 --gl-samples` with the same checks: X_ITE draws the band
//! into its multisampled framebuffer before the blit, so its edges are antialiased.
//!
//! `REA_ONLY=<substring>` restricts the cases.

mod common;

use common::gpu::renderer_available;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Channel difference allowed between the pictures: one level of rounding in a blend (fixed before the first comparison).
const PIXEL_TOL: i32 = 1;
const POSE_TOL: f64 = 1e-4;

fn nums(v: &Value) -> Vec<f64> {
    v.as_array()
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default()
}

fn quat(r: &[f64]) -> [f64; 4] {
    let n = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
    let n = if n == 0.0 { 1.0 } else { n };
    let s = (r[3] / 2.0).sin() / n;
    [r[0] * s, r[1] * s, r[2] * s, (r[3] / 2.0).cos()]
}

/// The pixels of an RGBA picture that differ from its first pixel, by (x, y).
type Pixels = BTreeMap<(u32, u32), [u8; 3]>;

fn non_background(w: u32, rgba: &[u8]) -> (Vec<u8>, Pixels) {
    let bg = rgba[..3].to_vec();
    let found = rgba
        .chunks_exact(4)
        .enumerate()
        .filter(|(_, p)| p[..3] != bg[..])
        .map(|(i, p)| (((i as u32) % w, (i as u32) / w), [p[0], p[1], p[2]]))
        .collect();

    (bg, found)
}

fn compare_pictures(
    name: &str,
    frame: &str,
    want: &Value,
    png: &Path,
    fail: &mut dyn FnMut(String),
) {
    let (w, h, rgba) = match x3d_render::compare::load_png(png) {
        Ok(p) => p,
        Err(e) => return fail(format!("{name} frame {frame}: {e}")),
    };

    if nums(&want["size"]) != [w as f64, h as f64] {
        return fail(format!(
            "{name} frame {frame}: size {w}x{h}, X_ITE {}",
            want["size"]
        ));
    }

    let (bg, got) = non_background(w, &rgba);
    let want_bg: Vec<u8> = nums(&want["background"]).iter().map(|v| *v as u8).collect();
    let expected: Pixels = want["pixels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let p = nums(p);
            (
                (p[0] as u32, p[1] as u32),
                [p[2] as u8, p[3] as u8, p[4] as u8],
            )
        })
        .collect();

    if bg != want_bg {
        return fail(format!(
            "{name} frame {frame}: sky color {bg:?}, X_ITE {want_bg:?}"
        ));
    }

    let mut worst = 0;
    let mut wrong = vec![];
    let keys: std::collections::BTreeSet<_> = expected.keys().chain(got.keys()).collect();

    for k in &keys {
        let a = expected.get(k).map_or([bg[0], bg[1], bg[2]], |p| *p);
        let b = got.get(k).map_or([bg[0], bg[1], bg[2]], |p| *p);
        let d = (0..3)
            .map(|i| (a[i] as i32 - b[i] as i32).abs())
            .max()
            .unwrap();
        worst = worst.max(d);

        if d > PIXEL_TOL {
            wrong.push(format!("({}, {}) X_ITE {a:?} Rust {b:?}", k.0, k.1));
        }
    }

    if !wrong.is_empty() {
        fail(format!(
            "{name} frame {frame}: {} of {} pixels differ by more than {PIXEL_TOL} (X_ITE has {}, Rust {} off the sky color), first {}",
            wrong.len(),
            keys.len(),
            expected.len(),
            got.len(),
            wrong[0]
        ));
    }

    eprintln!(
        "{name} frame {frame}: {} pixels off the sky color in X_ITE, {} in Rust, largest channel difference {worst}",
        expected.len(),
        got.len()
    );
}

fn compare_case(
    root: &Path,
    name: &str,
    world: &str,
    script: &str,
    msaa: u32,
    failures: &mut Vec<String>,
) {
    // The NVIDIA recording, or the one of X3D_REFERENCE_PLATFORM where it has this case (common/reference.rs); MSAA 4 has its own
    // directories (oracle/rea-nav-overlay-msaa4-baseline and oracle/rea-nav-overlay-msaa4-<platform>-baseline).
    let dir = if msaa > 0 {
        format!("rea-nav-overlay-msaa{msaa}-baseline")
    } else {
        "rea-nav-overlay-baseline".to_string()
    };
    let base = common::reference::flat_json(root, &dir, name);
    let js: Value = serde_json::from_str(&std::fs::read_to_string(&base).unwrap()).unwrap();
    let pictures = js.get("frames").is_some();
    let out = std::env::temp_dir().join(format!(
        "rea-nav-overlay-{}-{name}-msaa{msaa}",
        std::process::id()
    ));
    std::fs::create_dir_all(&out).unwrap();
    let rs_json = out.join("rs.json");
    let png_dir = out.join("rs");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_x3d-drive"));
    cmd.current_dir(root)
        .args([world, script, "--oracle", "oracle"])
        .arg("--out")
        .arg(&rs_json)
        .arg("--warmup-from")
        .arg(&base);

    if msaa > 0 {
        // The MSAA oracle page draws with GL's sample pattern (x3d-drive --gl-samples, as the other MSAA comparisons).
        cmd.args(["--msaa", &msaa.to_string(), "--gl-samples"]);
    }

    if pictures {
        cmd.arg("--png-dir").arg(&png_dir);
    }

    let status = cmd.status().unwrap();
    let mut fail = |what: String| failures.push(format!("{name}: {what}"));

    if !status.success() {
        return fail(format!("x3d-drive exited with {status}"));
    }

    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&rs_json).unwrap()).unwrap();

    if js["ok"] != Value::Bool(true) || rs["ok"] != Value::Bool(true) {
        return fail(format!("ok js {} rs {}", js["ok"], rs["ok"]));
    }

    // The ContentScale case must have changed the drawing buffer in X_ITE (the page's viewport is CSS size x ContentScale).
    let script_json: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join(script)).unwrap()).unwrap();

    if let Some(scale) = script_json["contentScale"].as_f64() {
        let viewport = nums(&js["geometry"]["viewport"]);
        let expected = (512.0 * scale).floor();

        if viewport != [0.0, 0.0, expected, expected] {
            fail(format!(
                "X_ITE viewport {viewport:?} for contentScale {scale} (expected {expected})"
            ));
        }
    }

    for (k, a) in js["poses"].as_object().unwrap() {
        let Some(b) = rs["poses"].get(k) else {
            fail(format!("pose at frame {k} missing"));
            continue;
        };
        let d = |key: &str| {
            nums(&a[key])
                .iter()
                .zip(nums(&b[key]))
                .map(|(p, q)| (p - q).abs())
                .fold(0.0, f64::max)
        };
        let (qa, qb) = (
            quat(&nums(&a["orientation"])),
            quat(&nums(&b["orientation"])),
        );
        let dot = qa.iter().zip(qb).map(|(p, q)| p * q).sum::<f64>().abs();
        let angle = 2.0 * dot.min(1.0).acos();
        let fov = (a["fieldOfViewScale"].as_f64().unwrap_or(1.0)
            - b["fieldOfViewScale"].as_f64().unwrap_or(1.0))
        .abs();

        if d("position") > POSE_TOL
            || d("centerOfRotation") > POSE_TOL
            || angle > POSE_TOL
            || fov > POSE_TOL
            || a["viewer"] != b["viewer"]
        {
            fail(format!(
                "pose frame {k}: dpos {:.2e} dcenter {:.2e} dangle {angle:.2e} dfov {fov:.2e} viewer {}/{}",
                d("position"),
                d("centerOfRotation"),
                a["viewer"],
                b["viewer"]
            ));
        }
    }

    if pictures {
        for (frame, want) in js["frames"].as_object().unwrap() {
            compare_pictures(
                name,
                frame,
                want,
                &png_dir.join(format!("{frame}.png")),
                &mut fail,
            );
        }
    }

    let _ = std::fs::remove_dir_all(&out);
}

/// The cases of a group: `rubber-` (F15) or `scale-` (F17); at `msaa` above 0 only the cases of that baseline's cases.txt.
fn run_group(prefix: &str, msaa: u32) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();

    if !renderer_available() {
        return x3d_evidence::no_gpu(
            "the pictures and the depth passes need a Vulkan adapter, skipped",
        );
    }

    let cases =
        std::fs::read_to_string(root.join("oracle/synthetic-rea-nav-overlay/cases.txt")).unwrap();
    let only = std::env::var("REA_ONLY").ok();
    let listed = (msaa > 0).then(|| {
        std::fs::read_to_string(root.join(format!(
            "oracle/rea-nav-overlay-msaa{msaa}-baseline/cases.txt"
        )))
        .unwrap()
    });
    let mut failures = vec![];
    let mut checked = 0;

    for line in cases.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (name, world, script) = (f[0], f[1], f[2]);

        if !name.starts_with(prefix)
            || only.as_deref().is_some_and(|o| !name.contains(o))
            || listed
                .as_deref()
                .is_some_and(|l| !l.lines().any(|n| n.trim() == name))
        {
            continue;
        }

        compare_case(&root, name, world, script, msaa, &mut failures);
        checked += 1;
    }

    eprintln!("{prefix} MSAA {msaa}: {checked} cases checked");
    assert!(only.is_some() || checked > 5, "only {checked} cases ran");
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn rea_nav_overlay_rubber_band_matches_x_ite() {
    run_group("rubber-", 0);
}

#[test]
fn rea_nav_overlay_rubber_band_msaa4_matches_x_ite() {
    run_group("rubber-", 4);
}

#[test]
fn rea_nav_overlay_content_scale_matches_x_ite() {
    run_group("scale-", 0);
}
