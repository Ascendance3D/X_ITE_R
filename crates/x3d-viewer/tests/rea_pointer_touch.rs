//! sub:pointer.picking: touch input into TouchSensor, PlaneSensor, CylinderSensor, SphereSensor and Anchor, against pinned X_ITE 16.5.0
//! (34cdf9f97). World and scripts: oracle/synthetic-rea-pointer-touch (generator oracle/tools/gen_rea_pointer_touch.py); baselines:
//! oracle/rea-pointer-touch-baseline (oracle/tools/rea_e14_e18_record.py, three identical X_ITE runs each, Chrome DevTools touch events;
//! the result is the scene text of every frame, whose Script string `hist` lists every output event of the sensors, and the viewpoint
//! pose). `x3d-drive` feeds the viewer the touchstart, touchmove, touchend and touchcancel events of the script.
//!
//! Five cases must be equal (scene text exact, poses within 1e-4): single taps on every target and on empty space, drags, a touch that
//! leaves and enters a sensor, a second finger, and the EXAMINE viewer against a sensor press. The sixth, `cancel`, is the kept
//! difference of ADR 0001 row 1 and is pinned by name: X_ITE has no `touchcancel` handler, so a sensor that a touch pressed stays active
//! (until a later touchend or mouseup reaches it); X_ITE_R ends it when the touch is cancelled (ISO/IEC 19775-1 TouchSensor: `isActive`
//! ends when the pointing device is released or lost).
//!
//! The hit point, normal and texture coordinate are the float32 pixels of the pointing buffer (PointingBuffer.getHit reads them with
//! `gl.readPixels` and copies them), so they are the GPU's interpolated varyings: on ANGLE Metal the pinned page itself gives numbers
//! 1 to 2 ulps away from the NVIDIA recording. With `X3D_REFERENCE_PLATFORM=apple-m1` the cases are read from
//! oracle/rea-pointer-touch-apple-m1-baseline (the same pinned page on the physical M1, three identical runs; common/reference.rs).
//! On both platforms the scene text is exact in five cases and the poses are within 1e-4; the `cancel` case compares the history
//! before the first touchcancel and the last tap exactly (the kept difference is between them).
//!
//! `REA_ONLY=<substring>` restricts the cases.

mod common;

use common::gpu::renderer_available;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

/// The numbers of a pose vector; a missing or non-numeric component gives NaN, so the comparison fails.
fn nums(v: &Value) -> Vec<f64> {
    v.as_array()
        .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect())
        .unwrap_or_else(|| vec![f64::NAN])
}

/// The largest component difference; vectors of another length differ by infinity.
fn max_diff(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() {
        return f64::INFINITY;
    }
    a.iter()
        .zip(b)
        .map(|(p, q)| (p - q).abs())
        .fold(
            0.0,
            |m, d| if d.is_nan() { f64::INFINITY } else { m.max(d) },
        )
}

/// The words of the Script string `hist` of the scene text of a frame.
fn hist(vrml: &Value, frame: u32) -> Vec<String> {
    let text = vrml[frame.to_string()].as_str().unwrap_or("");
    let Some(start) = text.find("hist \"") else {
        return vec![];
    };
    let rest = &text[start + 6..];

    rest[..rest.find('"').unwrap_or(rest.len())]
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

fn count(words: &[String], word: &str) -> usize {
    words.iter().filter(|w| *w == word).count()
}

fn run_case(name: &str, failures: &mut Vec<String>) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let world = "oracle/synthetic-rea-pointer-touch/pointer-touch.wrl";
    let script = format!("oracle/synthetic-rea-pointer-touch/scripts/{name}.json");
    let js_path = common::reference::js_json(&root, "rea-pointer-touch-baseline", name, 0);
    let js: Value = serde_json::from_str(&std::fs::read_to_string(&js_path).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!("rea_pointer_touch-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let rs_json = out.join("rs.json");
    let status = Command::new(env!("CARGO_BIN_EXE_x3d-drive"))
        .current_dir(&root)
        .args([world, script.as_str(), "--oracle", "oracle"])
        .arg("--out")
        .arg(&rs_json)
        .arg("--warmup-from")
        .arg(js_path)
        .status()
        .unwrap();
    let mut fail = |what: String| failures.push(format!("{name}: {what}"));

    if !status.success() {
        return fail(format!("x3d-drive exited with {status}"));
    }

    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&rs_json).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(&out);

    if js["ok"] != Value::Bool(true) || rs["ok"] != Value::Bool(true) {
        return fail(format!("ok js {} rs {}", js["ok"], rs["ok"]));
    }

    let frames = js["vrml"].as_object().unwrap().len() as u32;

    // Both sides dump the same frames, and the log is equal in every case (the cancel difference is in the events only).
    for key in ["vrml", "poses"] {
        let keys = |v: &Value| -> Vec<String> {
            let mut k: Vec<String> = v[key]
                .as_object()
                .map(|o| o.keys().cloned().collect())
                .unwrap_or_default();
            k.sort();
            k
        };
        if keys(&js) != keys(&rs) {
            fail(format!(
                "{key} frame sets differ: X_ITE {:?} / Rust {:?}",
                keys(&js),
                keys(&rs)
            ));
        }
    }

    if js.get("log") != rs.get("log") {
        fail(format!("log differs: {} / {}", js["log"], rs["log"]));
    }

    if name == "cancel" {
        // Before the first touchcancel (frame 6) the histories are equal.
        for frame in 0..6 {
            if js["vrml"].get(frame.to_string()).is_some()
                && hist(&js["vrml"], frame) != hist(&rs["vrml"], frame)
            {
                fail(format!(
                    "cancel: the history before the first touchcancel differs at frame {frame}: {:?} / {:?}",
                    hist(&js["vrml"], frame),
                    hist(&rs["vrml"], frame)
                ));
            }
        }

        // Kept difference, ADR 0001 row 1. Events: touchstart on the TouchSensor (frame 2), touchcancel (6), touchstart on the
        // PlaneSensor (10), touchcancel (14), touchstart on empty space, move, touchend (18 to 20).
        let (xa, ra) = (hist(&js["vrml"], 8), hist(&rs["vrml"], 8));

        if count(&xa, "TS_isActive=true") != 1 || count(&xa, "TS_isActive=false") != 0 {
            fail(format!(
                "X_ITE: the TouchSensor must stay active after touchcancel: {xa:?}"
            ));
        }

        if count(&ra, "TS_isActive=false") != 1 || count(&ra, "TS_touchTime=0.1667") != 1 {
            fail(format!(
                "X_ITE_R: the cancel must end the TouchSensor: {ra:?}"
            ));
        }

        let (xb, rb) = (hist(&js["vrml"], 16), hist(&rs["vrml"], 16));

        if count(&xb, "PS_isActive=false") != 0 || count(&rb, "PS_isActive=false") != 1 {
            fail(format!(
                "PlaneSensor after the second cancel: X_ITE {} ends, X_ITE_R {}",
                count(&xb, "PS_isActive=false"),
                count(&rb, "PS_isActive=false")
            ));
        }

        // The last touch (SphereSensor, frames 22 to 24) is a plain press and release: equal again.
        let (xe, re) = (hist(&js["vrml"], frames), hist(&rs["vrml"], frames));
        let tail =
            |h: &[String]| h[h.iter().position(|w| w == "SS_isActive=true").unwrap()..].to_vec();

        if tail(&xe) != tail(&re) {
            fail(format!(
                "the last tap differs: {:?} / {:?}",
                tail(&xe),
                tail(&re)
            ));
        }

        return;
    }

    for (k, a) in js["vrml"].as_object().unwrap() {
        if rs["vrml"].get(k) != Some(a) {
            fail(format!(
                "scene text differs at frame {k}: X_ITE {:?} / Rust {:?}",
                hist(&js["vrml"], k.parse().unwrap()),
                hist(&rs["vrml"], k.parse().unwrap())
            ));
            break;
        }
    }

    for (k, a) in js["poses"].as_object().unwrap() {
        let Some(b) = rs["poses"].get(k) else {
            fail(format!("pose at frame {k} missing"));
            continue;
        };
        let d = |key: &str| max_diff(&nums(&a[key]), &nums(&b[key]));
        let viewpoint = a["viewpoint"] == b["viewpoint"] || b["viewpoint"] == "<default>";

        if d("position") > 1e-4
            || d("orientation") > 1e-4
            || a["viewer"] != b["viewer"]
            || !viewpoint
        {
            fail(format!("pose frame {k}: X_ITE {a} / Rust {b}"));
        }
    }
}

#[test]
fn rea_pointer_touch_match_x_ite() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let cases =
        std::fs::read_to_string(root.join("oracle/rea-pointer-touch-baseline/cases.txt")).unwrap();
    let only = std::env::var("REA_ONLY").ok();
    let gpu = renderer_available();

    if !gpu {
        // Hosted CI has no adapter: the evidence log records the skip (X3D_EVIDENCE decides whether it is allowed).
        return x3d_evidence::no_gpu("the pointing pass needs a Vulkan adapter");
    }

    let mut failures = vec![];
    let mut checked = 0;

    for line in cases.lines().filter(|l| !l.trim().is_empty()) {
        let name = line.split_whitespace().next().unwrap();

        if only.as_deref().is_some_and(|o| !name.contains(o)) {
            continue;
        }

        run_case(name, &mut failures);
        checked += 1;
    }

    assert!(only.is_some() || checked == 6, "only {checked} cases ran");
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
