//! Text under InstancedShape against pinned X_ITE 16.5.0 (oracle/instanced-text-baseline, made by `oracle/tools/rea_instanced_text.py
//! record`: every case at MSAA 0 and 4, three identical X_ITE runs each, pictures byte-identical). Worlds and scripts:
//! oracle/synthetic-instanced-text (generator oracle/tools/gen_instanced_text.py; authored, no historical assets):
//! it-static    a plain Shape with Text and InstancedShapes with Text: translations, scales, rotations, scaleOrientations, centers,
//!              FontStyle, length / maxExtent, no instance field, transparency
//! it-extra     PixelTexture, solid TRUE under turned instances, bboxDisplay and explicit bbox, `length` per line, empty string, visible FALSE
//! it-pick      TouchSensor over instanced Text: a 2601-probe pointing grid (script key pickProbes), bit for bit
//! it-fields    Text output fields (textBounds, lineBounds, origin, length, maxExtent) read by a Script: plain Shape control and InstancedShapes,
//!              with and without FontStyle, before and after a run-time change of `string` / `size` (scene text exact, two pictures)
//! it-runtime   `string`, FontStyle, instance fields and `geometry` changed at run time by key events (11 pictures)
//!
//! Compared like rea_b24_anchor_collision_instanced.rs: scene text exact, the sorted log exact, poses within 1e-4, captured frames with
//! at most 0.5% of the pixels more than 8 off (the limit is not widened). The pointing probes read the GPU pointing buffer, so the
//! test is skipped without an adapter (`x3d_evidence::no_gpu`; hosted CI).
//!
//! KNOWN_GAPS lists cases the renderer cannot match yet; the test requires them to keep failing.
//!
//! `REA_ONLY=<substring>` restricts the cases.

mod common;

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// (case name, MSAA, why Rust differs).
const KNOWN_GAPS: &[(&str, u32, &str)] = &[];

const CASES: &[&str] = &[
    "it-static",
    "it-extra",
    "it-pick",
    "it-runtime",
    "it-fields",
];

fn quat(r: &[f64]) -> [f64; 4] {
    let n = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
    let n = if n == 0.0 { 1.0 } else { n };
    let s = (r[3] / 2.0).sin() / n;
    [r[0] * s, r[1] * s, r[2] * s, (r[3] / 2.0).cos()]
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

fn compare_case(
    root: &Path,
    name: &str,
    world: &str,
    script: &str,
    msaa: u32,
    failures: &mut Vec<String>,
) {
    let base = root.join("oracle/instanced-text-baseline");
    let key = format!("{name}-msaa{msaa}");
    let platform = common::reference::platform();
    let baseline = common::reference::flat_json_for(
        root,
        "instanced-text-baseline",
        &key,
        platform.as_deref(),
    );
    let pictures = base.join(format!("png/{name}-msaa{msaa}"));
    let js: Value = serde_json::from_str(&std::fs::read_to_string(&baseline).unwrap()).unwrap();
    // One directory for each call: the two tests run in threads of one process under `cargo test`, and both run the same case.
    static RUN: AtomicUsize = AtomicUsize::new(0);
    let out = std::env::temp_dir().join(format!(
        "instanced-text-{}-{}-{name}-msaa{msaa}",
        std::process::id(),
        RUN.fetch_add(1, Ordering::Relaxed)
    ));
    let _cleanup = TempDir(out.clone());
    std::fs::create_dir_all(&out).unwrap();
    let rs_json = out.join("rs.json");
    let png_dir = out.join("rs");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_x3d-drive"));
    cmd.current_dir(root)
        .args([world, script, "--oracle", "oracle"])
        .arg("--out")
        .arg(&rs_json)
        .arg("--warmup-from")
        .arg(&baseline);

    cmd.arg("--png-dir").arg(&png_dir);

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

    // Both ways: a frame that only one side has is a difference too.
    let (js_vrml, rs_vrml) = (
        js["vrml"]
            .as_object()
            .expect("the baseline has no vrml object"),
        rs["vrml"]
            .as_object()
            .expect("the Rust output has no vrml object"),
    );

    for k in js_vrml
        .keys()
        .chain(rs_vrml.keys().filter(|k| !js_vrml.contains_key(*k)))
    {
        if js_vrml.get(k) != rs_vrml.get(k) {
            fail(format!("scene text differs at frame {k}"));
        }
    }

    if js.get("log") != rs.get("log") {
        fail(format!("log differs: {} / {}", js["log"], rs["log"]));
    }

    // Script key "pickProbes": the pointing buffer's hit at each probe, bit for bit (x10_compare.py).
    if let Some(want) = js.get("pickProbes").and_then(Value::as_array) {
        let hits = want.iter().filter(|p| p["id"] == 1).count();

        // A grid that hits nothing on both sides would pass; the baseline must have hits.
        if hits < 50 {
            fail(format!("the baseline has only {hits} pointing hits"));
        }

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

    let (js_poses, rs_poses) = (
        js["poses"]
            .as_object()
            .expect("the baseline has no poses object"),
        rs["poses"]
            .as_object()
            .expect("the Rust output has no poses object"),
    );

    for k in rs_poses.keys().filter(|k| !js_poses.contains_key(*k)) {
        fail(format!("pose at frame {k} is only in the Rust output"));
    }

    for (k, a) in js_poses {
        let Some(b) = rs_poses.get(k) else {
            fail(format!("pose at frame {k} missing"));
            continue;
        };

        // A pose without its vectors is a failure, not a zero distance.
        let complete = |p: &Value| {
            nums(&p["position"]).len() == 3
                && nums(&p["centerOfRotation"]).len() == 3
                && nums(&p["orientation"]).len() == 4
        };

        if !complete(a) || !complete(b) {
            fail(format!(
                "pose frame {k}: position, centerOfRotation or orientation is missing or has the wrong length (X_ITE {}, Rust {})",
                complete(a),
                complete(b)
            ));
            continue;
        }

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

    {
        // The pictures are the captures of the script, on both sides and no others.
        let names = |dir: &Path| -> Vec<String> {
            let mut v: Vec<String> = std::fs::read_dir(dir)
                .map(|d| {
                    d.filter_map(|e| e.ok())
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .collect()
                })
                .unwrap_or_default();
            v.sort();
            v
        };
        let script_json: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join(script)).unwrap()).unwrap();
        let wanted = script_json["capture"].as_array().map_or(0, |c| c.len());
        let (have, made) = (names(&pictures), names(&png_dir));

        if have.len() != wanted {
            fail(format!(
                "the baseline has {} pictures, the script captures {wanted}",
                have.len()
            ));
        }

        if made != have {
            fail(format!("pictures: baseline {have:?}, Rust {made:?}"));
        }

        for entry in std::fs::read_dir(&pictures).unwrap() {
            let p = entry.unwrap().path();
            let file = p.file_name().unwrap().to_string_lossy().to_string();
            let want_path = common::reference::picture_for(
                root,
                "instanced-text-baseline",
                &key,
                &file,
                &p,
                platform.as_deref(),
            );
            let (w, h, want) = x3d_render::compare::load_png(&want_path).unwrap();

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
}

/// Removes its directory when dropped, also after an early return.
struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs every case at MSAA 0 and 4.
#[test]
fn instanced_text_matches_x_ite() {
    if x3d_render::gpu::Gpu::new().is_err() {
        return x3d_evidence::no_gpu("the pointing and picture cases need the GPU, skipped");
    }

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let only = std::env::var("REA_ONLY").ok();
    let mut failures = vec![];
    let mut checked = 0;

    for name in CASES {
        for msaa in [0, 4] {
            if only.as_deref().is_some_and(|o| !name.contains(o)) {
                continue;
            }

            let world = format!("oracle/synthetic-instanced-text/{name}.wrl");
            let script = format!("oracle/synthetic-instanced-text/{name}.json");

            if let Some((_, _, why)) = KNOWN_GAPS
                .iter()
                .find(|(gap, m, _)| gap == name && *m == msaa)
            {
                let mut own = vec![];
                compare_case(&root, name, &world, &script, msaa, &mut own);

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
                compare_case(&root, name, &world, &script, msaa, &mut failures);
            }

            checked += 1;
        }
    }

    eprintln!("{checked} cases checked");
    assert!(
        checked > 0,
        "REA_ONLY={only:?} selected no case (cases: {CASES:?})"
    );
    assert!(only.is_some() || checked == CASES.len() * 2);
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Negative controls: a world that differs from the recorded one in one thing must fail the comparison with the named kind of
/// difference, so a pass above is not vacuous. (case, text to find in the world, text to put there, expected failure text).
const CONTROLS: &[(&str, &str, &str, &str)] = &[
    // a glyph: the scene text differs
    ("it-static", "\"Xy\"", "\"Xz\"", "scene text differs"),
    // an instance drawn three times as large: the picture differs by more than the bound
    (
        "it-static",
        "scales [ 1 1 1, 1.4 0.6 1 ]",
        "scales [ 1 1 1, 3 3 1 ]",
        "pixels over 8",
    ),
    // an instance moved: the pointing buffer differs
    (
        "it-pick",
        "translations [ 0 0 0, 2.8 0.5 0, 5.6 -0.4 0 ]",
        "translations [ 0 0 0, 2.8 1.5 0, 5.6 -0.4 0 ]",
        "pickProbes",
    ),
    // a run-time FontStyle change of another size: the pictures after the key differ
    (
        "it-runtime",
        "fs .size = 1.6",
        "fs .size = 2.4",
        "pixels over 8",
    ),
];

#[test]
fn instanced_text_negative_controls_fail() {
    if x3d_render::gpu::Gpu::new().is_err() {
        return x3d_evidence::no_gpu("the pointing and picture cases need the GPU, skipped");
    }

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let tmp = std::env::temp_dir().join(format!("instanced-text-control-{}", std::process::id()));
    let _cleanup = TempDir(tmp.clone());
    std::fs::create_dir_all(&tmp).unwrap();

    for (i, (name, from, to, expect)) in CONTROLS.iter().enumerate() {
        let src = std::fs::read_to_string(
            root.join(format!("oracle/synthetic-instanced-text/{name}.wrl")),
        )
        .unwrap();
        assert!(src.contains(from), "{name}: the world has no `{from}`");

        let world = tmp.join(format!("{i}-{name}.wrl"));
        std::fs::write(&world, src.replacen(from, to, 1)).unwrap();

        let mut failures = vec![];
        compare_case(
            &root,
            name,
            world.to_str().unwrap(),
            &format!("oracle/synthetic-instanced-text/{name}.json"),
            0,
            &mut failures,
        );
        assert!(
            failures.iter().any(|f| f.contains(expect)),
            "control {i} ({name}: `{from}` -> `{to}`) must fail with `{expect}`, got {failures:?}"
        );
    }
}
