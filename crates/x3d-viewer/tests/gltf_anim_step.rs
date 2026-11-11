//! glTF animation worlds against pinned X_ITE 16.5.0 (34cdf9f97), sub:parse.gltf2: the STEP, LINEAR and CUBICSPLINE
//! animations (translation, rotation, scale, weights) and the STEP `KHR_animation_pointer` animation, each loaded by an
//! Inline, started by a click and dumped inside the animation (the Inline's scene text carries the interpolated
//! values). Cases: `oracle/rea-gltf-anim-baseline` (`oracle/tools/rea_e14_e18_record.py`, three identical pinned runs,
//! MSAA 0 and 4, pictures) and the three driver cases of `oracle/rea-b08-gltf-baseline`. Worlds and scripts:
//! `oracle/synthetic-rea-b08-gltf/anim` (`oracle/tools/gen_rea_b08_gltf.py`).
//!
//! Compared like oracle/tools/x10_compare.py: scene text and the scene text of every Inline exact, the console and
//! the sorted log exact, poses within 1e-4, captured frames with at most 0.5% of the pixels more than 8 off (skipped
//! without a GPU adapter). `KNOWN` names the pinned differences that are not passed, with the reason.
//!
//! `REA_ONLY=<substring>` restricts the cases.

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

fn nums(v: &Value) -> Vec<f64> {
    v.as_array()
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn compare_case(
    root: &Path,
    base: &str,
    name: &str,
    world: &str,
    script: &str,
    msaa: u32,
    text_tol: f64,
    gpu: bool,
    failures: &mut Vec<String>,
) {
    let dir = root.join(format!("oracle/{base}/{name}-msaa{msaa}"));
    let js: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("js.json")).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!(
        "gltf-anim-{}-{name}-msaa{msaa}",
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
        .arg(dir.join("js.json"));

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

    // The pinned page (anim.html) replaces console.warn and console.error, so its recorded console holds only the `log:`
    // lines; warnings and errors are compared through `log`. The MSAA 4 page records neither console nor inlines.
    let logs = |v: &Value| -> Vec<Value> {
        v.as_array()
            .into_iter()
            .flatten()
            .filter(|l| l.as_str().is_some_and(|l| l.starts_with("log: ")))
            .cloned()
            .collect()
    };

    if !js["console"].is_null() && logs(&js["console"]) != logs(&rs["console"]) {
        fail(format!(
            "console differs: {} / {}",
            js["console"], rs["console"]
        ));
    }

    let known = KNOWN.iter().find(|(n, _, _)| *n == name);

    if known.is_some() {
        // The corrected STEP: weights (0, 0) until 0.5 s, (1, 0) until 1 s, then (0.5, 1).
        let points = |frame: &str| {
            rs["inlines"][frame]["Gltf"]
                .as_str()
                .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
                .unwrap_or_default()
        };

        for (frame, want) in [
            ("3", "point [ -1 -1 0, 1 -1 0, 1 1 0, -1 1 0 ]"),
            ("14", "point [ -1 -1 0, 1 -1 0, 1 1 0, -1 1 0 ]"),
            ("26", "point [ -1 -1 1, 1 -1 1, 1 1 -1, -1 1 -1 ]"),
            ("63", "point [ 0 -1 0.5, 2 -1 0.5, 2 1 -0.5, 0 1 -0.5 ]"),
        ] {
            if !points(frame).contains(want) {
                fail(format!("STEP weights do not hold at frame {frame}"));
            }
        }
    }

    for (k, a) in js["inlines"].as_object().into_iter().flatten() {
        let b = &rs["inlines"][k];
        let empty = serde_json::Map::new();
        let (a, b) = (a.as_object().unwrap_or(&empty), b.as_object());
        let mut paths: Vec<&String> = a.keys().chain(b.iter().flat_map(|b| b.keys())).collect();
        paths.sort();
        paths.dedup();

        for p in paths {
            let (x, y) = (a.get(p), b.and_then(|b| b.get(p)));
            let equal = match (x.and_then(Value::as_str), y.and_then(Value::as_str)) {
                (Some(x), Some(y)) => text_equal(x, y, text_tol),
                _ => x == y,
            };

            if !equal && known.is_none() {
                fail(format!("inline {p} scene text differs at frame {k}"));
            }

            // A known STEP-weights case differs from the pinned text only in the interpolator numbers: the path must exist on
            // both sides and the text without its numbers must be equal, so another change in the Inline still fails.
            if !equal && known.is_some() {
                let skeleton = |t: &str| -> Vec<String> {
                    t.split_whitespace()
                        .map(|w| {
                            let n = w.trim_matches(|c| c == ',' || c == '[' || c == ']');
                            if !n.is_empty() && n.parse::<f64>().is_ok() {
                                w.replace(n, "#")
                            } else {
                                w.to_string()
                            }
                        })
                        .collect()
                };
                match (x.and_then(Value::as_str), y.and_then(Value::as_str)) {
                    (Some(x), Some(y)) if skeleton(x) == skeleton(y) => {}
                    _ => fail(format!(
                        "inline {p} at frame {k}: the known STEP-weights case differs in more than its numbers"
                    )),
                }
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

            if known.is_some_and(|(_, skip, _)| skip.contains(&file.as_str())) {
                continue;
            }

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

/// Pinned differences that are not passed: (case, pictures not compared, reason). For these cases the Inline scene text
/// is not compared with the pinned page; the corrected STEP values are checked instead.
///
/// glTF 2.0 STEP holds a keyframe value until the next keyframe. Pinned X_ITE 16.5.0 does that for translation,
/// rotation, scale and pointer channels (`createNamedInterpolator`), but its `createArrayInterpolator` (weights)
/// writes the keys `[t0, t1, t1, t2, t2]` with the values `[v0, v1, v1, v2, v2]`, so the morph interpolates linearly
/// from v0 to v1 over the first segment instead of holding v0 (frame 14, 0.33 s: the pinned page has weights (0.67, 0)).
/// An upstream defect against the specification, not copied. Frames 3, 44 and 63 are compared.
const KNOWN: &[(&str, &[&str], &str)] = &[
    (
        "gltf-anim-weights-step",
        &["14.png", "26.png"],
        "STEP weights interpolated",
    ),
    (
        "gltf-anim-weights-step-m4",
        &["14.png", "26.png"],
        "STEP weights interpolated",
    ),
];

#[test]
fn gltf_animation_cases_match_x_ite() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let (mut cases, mut sources) = (vec![], vec![]);

    for base in ["rea-gltf-anim-baseline", "rea-b08-gltf-baseline"] {
        let text = std::fs::read_to_string(root.join(format!("oracle/{base}/cases.txt"))).unwrap();

        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            // The b08 baseline lists other cases too: only the driver cases are glTF animations.
            if base == "rea-b08-gltf-baseline" && !line.contains("/anim/") {
                continue;
            }

            cases.push(line.to_string());
            sources.push(base);
        }
    }

    let only = std::env::var("REA_ONLY").ok();
    // One GPU at a time: the cases run in order in this test.
    let gpu = x3d_render::gpu::Gpu::new().is_ok();
    if !gpu {
        x3d_evidence::no_gpu("image comparisons skipped");
    }
    let mut failures = vec![];
    let mut checked = 0;

    for (line, base) in cases.iter().zip(sources) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (name, world, script, msaa) = (f[0], f[1], f[2], f[3].parse::<u32>().unwrap());
        let text_tol = f.get(4).map_or(0.0, |t| t.parse::<f64>().unwrap());

        if only.as_deref().is_some_and(|o| !name.contains(o)) {
            continue;
        }

        compare_case(
            &root,
            base,
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

    eprintln!("{checked} cases checked (images: {gpu})");
    assert!(checked > 0, "no case ran");
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
