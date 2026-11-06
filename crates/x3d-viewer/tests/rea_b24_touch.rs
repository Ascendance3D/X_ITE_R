//! E14 of docs/reports/NAV_ITEM_LISTS.md (sub:nav.examine): one-finger touch rotate, two-finger pinch zoom, rotate then pinch and the
//! spin at the end of a touch rotate of the EXAMINE viewer against pinned X_ITE 16.5.0 (34cdf9f97). Worlds and scripts:
//! oracle/synthetic-rea-b24-nav (`touch-*`, generator oracle/tools/gen_rea_b24_touch.py); baselines: oracle/rea-b24-nav-touch-baseline
//! (oracle/tools/rea_e14_e18_record.py, three identical X_ITE runs each, Chrome DevTools touch events, no pictures: the result is the
//! camera pose). The comparison is the one of rea_b24_nav.rs, copied (poses 1e-4, probe distance 1e-5, pictures threshold 8 and
//! 0.5% of the pixels): `x3d-drive` feeds the viewer the touchstart, touchmove and touchend events of the script.
//!
//! `REA_ONLY=<substring>` restricts the cases.

mod common;

use common::gpu::renderer_available;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn quat(r: &[f64]) -> [f64; 4] {
    let n = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
    let n = if n == 0.0 { 1.0 } else { n };
    let s = (r[3] / 2.0).sin() / n;
    [r[0] * s, r[1] * s, r[2] * s, (r[3] / 2.0).cos()]
}

/// x10_compare.py text_equal: identical, or identical after replacing every number and the
/// numbers within `tol`.
fn text_equal(a: &str, b: &str, tol: f64) -> bool {
    if a == b {
        return true;
    }

    if tol == 0.0 {
        return false;
    }

    fn split(s: &str) -> (String, Vec<f64>) {
        let (mut text, mut numbers) = (String::new(), vec![]);
        let bytes = s.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            let start = i;
            let digit = |j: usize| j < bytes.len() && bytes[j].is_ascii_digit();

            if digit(i) || (bytes[i] == b'-' && digit(i + 1)) {
                i += 1;
                while digit(i) {
                    i += 1;
                }
                if i < bytes.len() && bytes[i] == b'.' && digit(i + 1) {
                    i += 1;
                    while digit(i) {
                        i += 1;
                    }
                }
                if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
                    let mut j = i + 1;
                    if j < bytes.len() && (bytes[j] == b'-' || bytes[j] == b'+') {
                        j += 1;
                    }
                    if digit(j) {
                        i = j;
                        while digit(i) {
                            i += 1;
                        }
                    }
                }
                numbers.push(s[start..i].parse().unwrap_or(f64::NAN));
                text.push('#');
            } else {
                let c = s[i..].chars().next().unwrap();
                text.push(c);
                i += c.len_utf8();
            }
        }

        (text, numbers)
    }

    let ((ta, na), (tb, nb)) = (split(a), split(b));

    ta == tb
        && na
            .iter()
            .zip(&nb)
            .all(|(x, y)| (x - y).abs() / x.abs().max(1.0) <= tol)
}

/// Equal JSON values where numbers are equal as doubles (JS prints 0 where Rust prints 0.0).
fn same_bits(a: Option<&Value>, b: Option<&Value>) -> bool {
    match (a, b) {
        (Some(Value::Array(x)), Some(Value::Array(y))) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same_bits(Some(p), Some(q)))
        }
        (Some(Value::Object(x)), Some(Value::Object(y))) => {
            x.len() == y.len() && x.iter().all(|(k, v)| same_bits(Some(v), y.get(k)))
        }
        (Some(Value::Number(x)), Some(Value::Number(y))) => {
            x.as_f64().map(f64::to_bits) == y.as_f64().map(f64::to_bits)
        }
        (x, y) => x == y,
    }
}

/// Allowed difference of a collision probe's distance and normal (absolute); widened from batch 09's 1e-6 for the slanted
/// hits of the tilted-Transform and ceiling cases (the largest of this suite is in the file header of the lane report). The distance comes from
/// a float32 window z, one ulp of which is `(far - near) * 6e-8` (2e-7 for avatarHeight 1.6); the
/// largest difference seen over 1000+ probes is 5.5e-7.
const PROBE_TOL: f64 = 1e-5;

fn nums(v: &Value) -> Vec<f64> {
    v.as_array()
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn compare_case(
    root: &Path,
    name: &str,
    world: &str,
    script: &str,
    msaa: u32,
    text_tol: f64,
    gpu: bool,
    failures: &mut Vec<String>,
) {
    let dir = root.join(format!(
        "oracle/rea-b24-nav-touch-baseline/{name}-msaa{msaa}"
    ));
    let js_path = common::reference::js_json(root, "rea-b24-nav-touch-baseline", name, msaa);
    let js: Value = serde_json::from_str(&std::fs::read_to_string(&js_path).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!(
        "rea_b24_touch-{}-{name}-msaa{msaa}",
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
        .arg(js_path);

    let images = gpu && dir.join("js").is_dir();

    if images {
        cmd.arg("--png-dir").arg(&png_dir);
    }

    if msaa > 0 {
        cmd.args(["--msaa", &msaa.to_string(), "--gl-samples"]);
    }

    let status = cmd.status().unwrap();
    let mut fail = |what: String| failures.push(format!("{name} MSAA {msaa}: {what}"));

    if !status.success() {
        return fail(format!("x3d-drive exited with {status}"));
    }

    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&rs_json).unwrap()).unwrap();

    if js["ok"] != Value::Bool(true) || rs["ok"] != Value::Bool(true) {
        return fail(format!("ok js {} rs {}", js["ok"], rs["ok"]));
    }

    for (k, a) in js["vrml"].as_object().unwrap() {
        let equal = match (a.as_str(), rs["vrml"].get(k).and_then(Value::as_str)) {
            (Some(a), Some(b)) => text_equal(a, b, text_tol),
            _ => rs["vrml"].get(k) == Some(a),
        };

        if !equal {
            fail(format!("scene text differs at frame {k}"));
        }
    }

    let (a, b) = (
        js["pickProbes"].as_array().cloned().unwrap_or_default(),
        rs["pickProbes"].as_array().cloned().unwrap_or_default(),
    );
    let bad: Vec<usize> = (0..a.len().max(b.len()))
        .filter(|&i| !same_bits(a.get(i), b.get(i)))
        .collect();

    if !bad.is_empty() {
        fail(format!(
            "{} of {} pick probes differ, first {}: X_ITE {} / Rust {}",
            bad.len(),
            a.len(),
            bad[0],
            a.get(bad[0]).unwrap_or(&Value::Null),
            b.get(bad[0]).unwrap_or(&Value::Null)
        ));
    }

    // collisionProbes: getClosestObject (direction) after the last frame. Both depth buffers are drawn
    // on the same GPU, the Rust one through Vulkan and X_ITE's through ANGLE on OpenGL: the hit flag
    // and normal agree, the window z of a pixel differs by a few float32 ulp (see PROBE_TOL).
    if js.get("collisionProbes").is_some() || rs.get("collisionProbes").is_some() {
        let (a, b) = (
            js["collisionProbes"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
            rs["collisionProbes"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
        );

        if a.len() != b.len() {
            fail(format!("{} collision probes, X_ITE {}", b.len(), a.len()));
        }

        for (i, (p, q)) in a.iter().zip(&b).enumerate() {
            let hit = |v: &Value| v["id"].as_i64().unwrap_or(-1) >= 0;
            let dd = (p["distance"].as_f64().unwrap_or(f64::NAN)
                - q["distance"].as_f64().unwrap_or(f64::NAN))
            .abs();
            let dn = nums(&p["normal"])
                .iter()
                .zip(nums(&q["normal"]))
                .map(|(x, y)| (x - y).abs())
                .fold(0.0, f64::max);

            // avatarSize[0] 0: X_ITE's depth read is NaN (JSON null) and so is Rust's.
            let no_distance = p["distance"].is_null() && q["distance"].is_null();

            if hit(p) != hit(q)
                || !(no_distance || dd <= PROBE_TOL)
                || (hit(p) && !(dn <= PROBE_TOL))
            {
                fail(format!(
                    "collision probe {i}: X_ITE {p} / Rust {q} (distance {dd:.2e}, normal {dn:.2e})"
                ));
                break;
            }
        }
    }

    if js.get("log") != rs.get("log") {
        fail(format!("log differs: {} / {}", js["log"], rs["log"]));
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
        let viewpoint = a["viewpoint"] == b["viewpoint"] || b["viewpoint"] == "<default>";

        if d("position") > 1e-4
            || d("centerOfRotation") > 1e-4
            || angle > 1e-4
            || fov > 1e-4
            || a["viewer"] != b["viewer"]
            || !viewpoint
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

    if images {
        for entry in std::fs::read_dir(dir.join("js")).unwrap() {
            let p = entry.unwrap().path();
            let file = p.file_name().unwrap().to_string_lossy().to_string();
            let (w, h, want) = x3d_render::compare::load_png(&p).unwrap();

            match x3d_render::compare::load_png(&png_dir.join(&file)) {
                Ok((rw, rh, got)) if (rw, rh) == (w, h) => {
                    let s = x3d_render::compare::compare(w, h, &want, &got, 8);

                    if s.pixels_over * 1000 > (w * h) as usize * 5 {
                        fail(format!(
                            "{file}: {} pixels over 8 ({:.3}%), max {}",
                            s.pixels_over, s.pct_over, s.max_delta
                        ));
                    }
                }
                Ok((rw, rh, _)) => fail(format!("{file}: size {rw}x{rh}, X_ITE {w}x{h}")),
                Err(e) => fail(format!("{file}: {e}")),
            }
        }
    }

    let _ = std::fs::remove_dir_all(&out);
}

fn run_cases() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let cases =
        std::fs::read_to_string(root.join("oracle/rea-b24-nav-touch-baseline/cases.txt")).unwrap();
    let only = std::env::var("REA_ONLY").ok();
    let gpu = renderer_available();
    if !gpu {
        x3d_evidence::no_gpu("image comparisons skipped");
    }
    assert!(
        gpu,
        "the pointing and collision passes need a Vulkan adapter"
    );
    let mut failures = vec![];
    let mut checked = 0;

    for line in cases.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (name, world, script, msaa) = (f[0], f[1], f[2], f[3].parse::<u32>().unwrap());
        let text_tol = f.get(4).map_or(0.0, |t| t.parse::<f64>().unwrap());

        if only.as_deref().is_some_and(|o| !name.contains(o)) {
            continue;
        }

        compare_case(
            &root,
            name,
            world,
            script,
            msaa,
            text_tol,
            gpu,
            &mut failures,
        );
        checked += 1;
    }

    eprintln!("{checked} cases checked");
    assert!(only.is_some() || checked >= 5, "only {checked} cases ran");
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn rea_b24_touch_match_x_ite() {
    run_cases();
}
