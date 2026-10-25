//! REA batch 12: stepped cases against pinned X_ITE 16.5.0 (oracle/rea-batch12-baseline, made by `oracle/tools/rea_b12.py record`,
//! three identical runs each, MSAA 0 and 4). Worlds and scripts: oracle/synthetic-rea-b12-* (generators oracle/tools/gen_rea_b12_*.py):
//! vw-* Browser.viewAll, StraightenHorizon, pose and bind events, LookAt on Ortho / perspective / Geo viewpoints and layers.
//! bbb-* Billboard bboxDisplay, camera moves, run-time changes and the SHADOW pass.
//! pk-* the pointing buffer: points, lines, stacked and coincident targets (script key pickProbes, compared bit for bit: id,
//! point, normal, texCoord of every probe) and TouchSensor event order (pk-events, through the scene text).
//!
//! Same comparison as rea_batch10_screen.rs: scene text exact, poses within 1e-4, the sorted log exact, captured frames with at most
//! 0.5% of the pixels more than 8 off (the limit is not widened). Without a GPU adapter the image part is skipped.
//!
//! KNOWN_GAPS lists cases the renderer cannot match yet; the test requires them to keep failing.
//!
//! The cases are split by prefix into two tests (before 2026-10-21 both were one test, rea_batch12_cases_match_x_ite):
//! rea_batch12_pick_cases_match_x_ite (pk-*) needs a GPU: without one x3d-drive picks with the ray cast, whose float64 hits
//! differ from the GPU pointing buffer. rea_batch12_view_billboard_cases_match_x_ite (vw-*, bbb-*) also runs without a GPU
//! (images skipped, scene text, log and poses still compared). A case with another prefix fails both tests.
//!
//! `REA_ONLY=<substring>` restricts the cases.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// One GPU at a time: the two tests run their cases in order, never together.
static GPU: Mutex<()> = Mutex::new(());

/// Case name prefixes of each test.
const PICK: &[&str] = &["pk-"];
const VIEW_BILLBOARD: &[&str] = &["vw-", "bbb-"];

/// (case name, why Rust differs).
const KNOWN_GAPS: &[(&str, &str)] = &[
    (
        "pk-points-style",
        "PointProperties points: hit ids, points and normals are bit-exact, but gl_PointCoord (the hit texCoord) differs by 1 to 3 \
         float32 ulps at some pixels (584 of 2479 probes): the oracle GPU's GL point sprite coordinate is not reproduced exactly",
    ),
    (
        "pk-lines",
        "lines: every probe hits the same line, but the interpolated hit point differs by up to 63 float32 ulps on aliased lines \
         (GL line rasterization vs. the quad emulation) and up to 398 ulps on a transformed thick line (X_ITE makes its \
         triangles by transform feedback in float32, Rust on the CPU in double)",
    ),
];

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

/// JSON equality with numbers compared by value, exactly (X_ITE writes `0` and `-10` where serde writes `0.0` and `-10.0`;
/// Python's x10_compare.py treats them as equal too).
fn same_value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same_value(p, q))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, p)| y.get(k).is_some_and(|q| same_value(p, q)))
        }
        _ => a == b,
    }
}

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
    let dir = root.join(format!("oracle/rea-batch12-baseline/{name}-msaa{msaa}"));
    let js: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("js.json")).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!(
        "rea-batch12-{}-{name}-msaa{msaa}",
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

    if js.get("log") != rs.get("log") {
        fail(format!("log differs: {} / {}", js["log"], rs["log"]));
    }

    // Script key "pickProbes": the pointing buffer's hit at each probe, bit for bit (x10_compare.py).
    if let Some(want) = js.get("pickProbes").and_then(Value::as_array) {
        let got = rs
            .get("pickProbes")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let differ: Vec<usize> = (0..want.len().max(got.len()))
            .filter(
                |&i| !matches!((want.get(i), got.get(i)), (Some(a), Some(b)) if same_value(a, b)),
            )
            .collect();

        if !differ.is_empty() {
            let i = differ[0];
            fail(format!(
                "pickProbes: {} of {} differ; first #{i}: X_ITE {} / Rust {}",
                differ.len(),
                want.len(),
                want.get(i).unwrap_or(&Value::Null),
                got.get(i).unwrap_or(&Value::Null)
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
                    eprintln!(
                        "{name} MSAA {msaa} {file}: {} pixels over 8 ({:.4}%), max delta {}",
                        s.pixels_over, s.pct_over, s.max_delta
                    );

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

/// Runs the cases.txt cases whose name starts with one of `prefixes`.
fn check_cases(prefixes: &[&str]) {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let cases =
        std::fs::read_to_string(root.join("oracle/rea-batch12-baseline/cases.txt")).unwrap();
    let only = std::env::var("REA_ONLY").ok();
    let gpu = x3d_render::gpu::Gpu::new().is_ok();
    if !gpu {
        x3d_evidence::no_gpu("image comparisons skipped");
    }
    let mut failures = vec![];
    let mut checked = 0;

    for line in cases.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (name, world, script, msaa) = (f[0], f[1], f[2], f[3].parse::<u32>().unwrap());
        let text_tol = f.get(4).map_or(0.0, |t| t.parse::<f64>().unwrap());

        if !PICK
            .iter()
            .chain(VIEW_BILLBOARD)
            .any(|p| name.starts_with(p))
        {
            failures.push(format!("{name} MSAA {msaa}: no test owns this case prefix"));
            continue;
        }

        if !prefixes.iter().any(|p| name.starts_with(p))
            || only.as_deref().is_some_and(|o| !name.contains(o))
        {
            continue;
        }

        if let Some((_, why)) = KNOWN_GAPS.iter().find(|(gap, _)| *gap == name) {
            let mut own = vec![];
            compare_case(&root, name, world, script, msaa, text_tol, gpu, &mut own);

            if own.is_empty() {
                failures.push(format!(
                    "{name} MSAA {msaa}: matches X_ITE now; remove it from KNOWN_GAPS ({why})"
                ));
            } else {
                eprintln!(
                    "{name} MSAA {msaa}: known gap, {} differences ({why})",
                    own.len()
                );
            }
        } else {
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
        }

        checked += 1;
    }

    eprintln!("{checked} cases checked (images: {gpu})");
    assert!(
        only.is_some() || checked > 0,
        "no case starts with {prefixes:?}"
    );
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// pk-*: needs a GPU (the CI runner skips it by name).
#[test]
fn rea_batch12_pick_cases_match_x_ite() {
    check_cases(PICK);
}

/// vw-*, bbb-*: runs with or without a GPU.
#[test]
fn rea_batch12_view_billboard_cases_match_x_ite() {
    check_cases(VIEW_BILLBOARD);
}
