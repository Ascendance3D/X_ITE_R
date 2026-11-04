//! Issue #95 ShaderPart lane: stepped cases against pinned X_ITE 16.5.0 (oracle/rea-b24-shaderpart-baseline, made by
//! `oracle/tools/rea_b24_shaderpart.py record`, worlds oracle/synthetic-rea-b24-shaderpart): url changes of a
//! loaded ShaderPart (MFString fallback, empty url, missing file, load around a url write), vertex-stage url
//! and error text, ShaderRegistry includes used without their context, and shader-list selection when a part
//! becomes valid and invalid. Every case is recorded at MSAA 0 and MSAA 4 (three identical X_ITE runs each; the MSAA 4
//! records equal the MSAA 0 records in scene text and log, and differ in the pictures). The MSAA 4 cases run
//! `x3d-drive --msaa 4 --gl-samples` and are skipped without a GPU adapter (the skip is named in the test output).
//!
//! Each case of cases.txt (`name world script msaa`) runs `x3d-drive` and compares like
//! oracle/tools/x10_compare.py: scene text exact, poses within 1e-4, the sorted console log exact, captured
//! frames with at most 0.5% of the pixels more than 8 off (skipped without a GPU adapter).
//! `REA_ONLY=<substring>` restricts the cases.

/// Cases whose console log differs from X_ITE on purpose, with the reason; every other part of the case (scene text,
/// poses, pictures) is still compared. The log must still differ: a case that now matches fails until it is removed.
const KNOWN: &[(&str, &str)] = &[
    (
        "include-Fragment",
        "ANGLE's preprocessor rejects `#if X3D_NUM_TEXTURE_COORDINATES > 0` when the macro is undefined (\"unexpected token after conditional expression\"); this port's preprocessor takes an unknown identifier as 0 and fails later in another include",
    ),
    (
        "include-Material",
        "same as include-Fragment (Material includes Fragment)",
    ),
    (
        "include-PBR",
        "same as include-Fragment (PBR includes Material)",
    ),
    (
        "include-Unlit",
        "same as include-Fragment (Unlit includes Fragment)",
    ),
    (
        "include-Stipple",
        "same as include-Fragment: the undefined macro in an `#if`",
    ),
    (
        "include-Texture",
        "ANGLE: 'X3D_NUM_TEXTURE_COORDINATES' undeclared identifier in an array size; this port: 'array size 0 out of range' (naga checks GLSL 4.50)",
    ),
    (
        "include-PointSize",
        "ANGLE: multiple fragment outputs need explicit locations; the port's compiler does not check output locations",
    ),
    (
        "include-Vertex",
        "ANGLE: 'out' invalid qualifier combination at line 114; the port's compiler reports gl_PointSize undeclared at line 166 (it checks later code first)",
    ),
    (
        "vertex-known",
        "a missing semicolon at a line end: ANGLE reports the next token's line, this port the statement's line; and a type error inside a function: ANGLE 'function return is not matching type', naga \"Function [0] 'f' is invalid\" (glsl_errors_b08.rs GAPS)",
    ),
];

/// Pinned picture difference of every case, measured on the RTX 3060 Ti (Vulkan, NVIDIA driver): (case, MSAA, pixels
/// that differ from X_ITE in any channel, summed over the captured pictures of the case, largest channel difference).
/// Cause (measured, issue #95): every fragment source of these worlds says `precision mediump float`. The pinned page runs
/// it through ANGLE, and the NVIDIA driver executes mediump in half precision: `vec4 (0.9, 0.7, 0.2, 1.0)` gives
/// (229, 179, 51) there, because 0.7 is 0.70019531 in half precision (x 255 = 178.55). This port keeps 32-bit floats
/// (naga drops the precision qualifier) and gives (229, 178, 51). The same worlds with `highp` in both renderers differ by
/// 0 pixels at MSAA 4 (include-Fog, select-steps, url-steps, vertex-steps), so the difference is the precision and nothing
/// else (neither the MSAA 4 edges nor the shading). The test fails when a count or the maximum changes in either direction;
/// a case that becomes exact must be removed with its cause. Every other picture bound (8 per channel, 0.5% of the pixels)
/// stays as it was. Counts need this GPU: the test applies them only on an adapter named RTX 3060 Ti; on another adapter it applies the picture bounds only (run `REA_STATS=1` and read the `STATS` lines to measure it).
const PINNED: &[(&str, u32, usize, u8)] = &[
    ("include-Fog", 0, 145192, 1),
    ("include-Fog", 4, 145296, 1),
    ("include-Fragment", 0, 145192, 1),
    ("include-Fragment", 4, 145296, 1),
    ("include-Material", 0, 145192, 1),
    ("include-Material", 4, 145296, 1),
    ("include-MaterialInfo", 0, 145192, 1),
    ("include-MaterialInfo", 4, 145296, 1),
    ("include-Normal", 0, 145192, 1),
    ("include-Normal", 4, 145296, 1),
    ("include-PBR", 0, 145192, 1),
    ("include-PBR", 4, 145296, 1),
    ("include-PointSize", 0, 145192, 1),
    ("include-PointSize", 4, 145296, 1),
    ("include-Stipple", 0, 145192, 1),
    ("include-Stipple", 4, 145296, 1),
    ("include-Texture", 0, 145192, 1),
    ("include-Texture", 4, 145296, 1),
    ("include-Unlit", 0, 145192, 1),
    ("include-Unlit", 4, 145296, 1),
    ("include-Vertex", 0, 145192, 1),
    ("include-Vertex", 4, 145296, 1),
    ("include-steps", 0, 217788, 1),
    ("include-steps", 4, 217944, 1),
    ("select-steps", 0, 53333, 1),
    ("select-steps", 4, 53242, 1),
    ("url-steps", 0, 48045, 1),
    ("url-steps", 4, 47970, 1),
    ("vertex-known", 0, 13068, 1),
    ("vertex-known", 4, 13056, 1),
    ("vertex-steps", 0, 22850, 1),
    ("vertex-steps", 4, 22886, 1),
];

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
    name: &str,
    world: &str,
    script: &str,
    msaa: u32,
    text_tol: f64,
    gpu: bool,
    exact: bool,
    failures: &mut Vec<String>,
) {
    let dir = root.join(format!(
        "oracle/rea-b24-shaderpart-baseline/{name}-msaa{msaa}"
    ));
    // MSAA 0 keeps the record in NAME-msaa0/js.json; a multisampled record is the flat NAME-msaaN.json beside the
    // NAME-msaaN/js pictures (the review gate leaves out oracle/*-baseline/*.json).
    let json = if msaa == 0 {
        dir.join("js.json")
    } else {
        PathBuf::from(format!("{}.json", dir.display()))
    };
    let js: Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!(
        "rea-b24-shaderpart-{}-{name}-msaa{msaa}",
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
        .arg(&json);

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

    let known = KNOWN.iter().any(|(k, _)| *k == name);

    if known {
        if js.get("log") == rs.get("log") {
            fail("log now matches X_ITE: remove the case from KNOWN".into());
        }
    } else if js.get("log") != rs.get("log") {
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

    let (mut differing, mut largest) = (0usize, 0u8);

    if images {
        for entry in std::fs::read_dir(dir.join("js")).unwrap() {
            let p = entry.unwrap().path();
            let file = p.file_name().unwrap().to_string_lossy().to_string();
            let (w, h, want) = x3d_render::compare::load_png(&p).unwrap();

            match x3d_render::compare::load_png(&png_dir.join(&file)) {
                Ok((rw, rh, got)) if (rw, rh) == (w, h) => {
                    let s = x3d_render::compare::compare(w, h, &want, &got, 8);

                    differing += s.pixels_diff;
                    largest = largest.max(s.max_delta);

                    if std::env::var_os("REA_STATS").is_some() {
                        eprintln!(
                            "STATS {name} MSAA {msaa} {file}: differing {} over8 {} max {}",
                            s.pixels_diff, s.pixels_over, s.max_delta
                        );
                    }

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

    if images && exact {
        match PINNED.iter().find(|p| p.0 == name && p.1 == msaa) {
            Some(&(_, _, pixels, max)) if (pixels, max) != (differing, largest) => fail(format!(
                "pictures differ from X_ITE in {differing} pixels, largest {largest}; pinned {pixels}, {max} (mediump in half precision, see PINNED)"
            )),
            Some(_) => {}
            None => fail("no PINNED entry".into()),
        }
    }

    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn rea_b24_shaderpart_cases_match_x_ite() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let cases = std::fs::read_to_string(root.join("oracle/rea-b24-shaderpart-baseline/cases.txt"))
        .expect("baseline missing: oracle/rea-b24-shaderpart-baseline/cases.txt (make it with oracle/tools/rea_b24_shaderpart.py record); comparison NOT RUN");
    let only = std::env::var("REA_ONLY").ok();
    // One GPU at a time: the cases run in order in this test.
    let adapter = x3d_render::gpu::Gpu::new()
        .ok()
        .map(|g| g.adapter_info.name.clone());
    let gpu = adapter.is_some();
    if !gpu {
        x3d_evidence::no_gpu("image comparisons skipped");
    }
    // The exact PINNED counts were measured on this adapter (half-precision mediump, see PINNED). On any other adapter
    // the per-picture bound (8 per channel, 0.5% of the pixels) is the whole picture rule, as before this table.
    let exact = adapter
        .as_deref()
        .is_some_and(|n| n.contains("RTX 3060 Ti"));
    if let Some(n) = adapter.as_deref().filter(|_| !exact) {
        eprintln!("PINNED counts not applied: adapter {n} is not the measured RTX 3060 Ti");
    }
    let mut failures = vec![];
    let mut checked = 0;
    let mut skipped = 0;

    for line in cases.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (name, world, script, msaa) = (f[0], f[1], f[2], f[3].parse::<u32>().unwrap());
        let text_tol = f.get(4).map_or(0.0, |t| t.parse::<f64>().unwrap());

        if only.as_deref().is_some_and(|o| !name.contains(o)) {
            continue;
        }

        if msaa > 0 && !gpu {
            skipped += 1;
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
            exact,
            &mut failures,
        );
        checked += 1;
    }

    eprintln!(
        "{checked} cases checked (images: {gpu}), {skipped} MSAA cases skipped without a GPU adapter"
    );
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
