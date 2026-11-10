//! REA ledger entry sub:nav.collision-gravity, items C17 and C18 (docs/reports/NAV_ITEM_LISTS.md): gravity and collision with two
//! Layers (the LayerSet's active layer, switched at run time by a Script) and the skip of collision and gravity while a viewpoint
//! transition is active, against pinned X_ITE 16.5.0. Worlds and scripts: oracle/synthetic-rea-gravity-layers (generator
//! oracle/tools/gen_rea_gravity_layers.py); baselines: oracle/rea-gravity-layers-baseline/NAME-msaa0.json (oracle/tools/
//! rea_gravity_layers.py record, three identical X_ITE runs each, no pictures). The comparison is the one of rea_b24_nav.rs, copied
//! with the same thresholds (poses 1e-4; collision probe distance and normal 1e-5, PROBE_TOL; scene text exact; sorted log exact):
//!
//! Each case of oracle/rea-gravity-layers-baseline/cases.txt (`name world script msaa`) runs `x3d-drive` on the world and its event
//! script and compares the result with the oracle's JSON. The scene text carries the isActive state and change count of the
//! Collision nodes (a Script writes them into a WorldInfo), so collide is compared directly, not only through poses.
//! Without a GPU adapter `x3d-drive` falls back to the CPU depth sampling and the collision parts are not bit-exact; the test
//! requires the adapter (X3D_EVIDENCE=strict: a missing adapter fails, otherwise it is reported by name via x3d_evidence).
//!
//! The cases are split over `SHARDS` tests so that they run side by side. `REA_ONLY=<substring>` restricts the cases.

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
    failures: &mut Vec<String>,
) {
    let dir = common::reference::flat_json(
        root,
        "rea-gravity-layers-baseline",
        &format!("{name}-msaa{msaa}"),
    );
    let js: Value = serde_json::from_str(&std::fs::read_to_string(&dir).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!(
        "rea-gravity-layers-{}-{name}-msaa{msaa}",
        std::process::id()
    ));
    std::fs::create_dir_all(&out).unwrap();
    let rs_json = out.join("rs.json");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_x3d-drive"));
    cmd.current_dir(root)
        .args([world, script, "--oracle", "oracle"])
        .arg("--out")
        .arg(&rs_json)
        .arg("--warmup-from")
        .arg(&dir);

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

    let _ = std::fs::remove_dir_all(&out);
}

const SHARDS: usize = 3;

/// (case name, why Rust differs). The test requires these to keep differing from X_ITE.
const KNOWN_GAPS: &[(&str, &str)] = &[
    ("trans-skip-probe", STALE_SHAPES),
    ("trans-layer-probe", STALE_SHAPES),
];

/// Cause read from the source, not instrumented: trans-skip-probe-start, where the eye has not moved yet, matches.
/// X3DLayerNode.collision returns before renderObject.render while the NavigationInfo's transition is active, so the layer
/// keeps the collision shapes (and their view-space matrices) of the last collision pass before the transition. A
/// getClosestObject call during the transition (the script key collisionProbes, or the SAI Browser.getClosestObject and
/// constrainTranslation) reads those stale shapes against the moved eye. x3d-drive rebuilds the shapes from the frame it
/// is at. Nothing else reads the shapes in a transition: collide and gravitate are skipped (compared directly by
/// trans-gravity-skip and trans-layer-skip), and the viewer stops the transition before it moves the eye.
///
/// Classified 2026-11-10 as an upstream defect, kept on purpose (not copied). X3DRenderObject.getCollisionShape draws each
/// stored shape with its stored `modelViewMatrix`, which holds the view matrix of the last collision pass, and
/// getClosestObject multiplies it with a view matrix built from the current user position. In a transition the two view
/// matrices differ, so the result follows the old eye, not the geometry: a static wall seems to move with the viewer. The
/// early return of X3DLayerNode.collision only skips collide and gravitate. getClosestObject is an X_ITE extension that no
/// standard defines; ISO/IEC 19775-1 V4.0 23.2.4 ties collision detection to "other objects in the scene", which a result
/// that follows the old eye does not measure. REA: analysis ev_bdb28dc35b50b73af2c0a0f9bae3faab1c34b450a2bd853bcfaf96bfa886153e
/// (summary view ev_ed4b3ac34d8319f9e34fba3a8f7f7e0c246d369387be082d7aceae25d7d17e3e), module trace of X3DLayerNode.js
/// ev_94297619aaada34f6ef3bfb1be12f333446490a64dadb104fc51d808d4934563; the graph is partial, and the line-level reading
/// (X3DLayerNode.js 466-467, X3DRenderObject.js 470-485 and 513-529) is from the pinned source text.
const STALE_SHAPES: &str = "getClosestObject in a transition reads X_ITE's stale collision shapes";

fn run_shard(shard: usize) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let cases =
        std::fs::read_to_string(root.join("oracle/rea-gravity-layers-baseline/cases.txt")).unwrap();
    let only = std::env::var("REA_ONLY").ok();

    // Strict mode (X3D_EVIDENCE=strict) fails here; otherwise the skip is recorded by name.
    if !renderer_available() {
        x3d_evidence::no_gpu("the collision depth passes need a Vulkan adapter");
        return;
    }

    let mut failures = vec![];
    let mut checked = 0;

    for (i, line) in cases.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        if i % SHARDS != shard {
            continue;
        }

        let f: Vec<&str> = line.split_whitespace().collect();
        let (name, world, script, msaa) = (f[0], f[1], f[2], f[3].parse::<u32>().unwrap());
        let text_tol = f.get(4).map_or(0.0, |t| t.parse::<f64>().unwrap());

        if only.as_deref().is_some_and(|o| !name.contains(o)) {
            continue;
        }

        if let Some((_, why)) = KNOWN_GAPS.iter().find(|(gap, _)| *gap == name) {
            let mut own = vec![];
            compare_case(&root, name, world, script, msaa, text_tol, &mut own);

            if own.is_empty() {
                failures.push(format!(
                    "{name} MSAA {msaa}: matches X_ITE now; remove it from KNOWN_GAPS ({why})"
                ));
            } else {
                eprintln!(
                    "{name} MSAA {msaa}: known gap, {} differences ({why}): {}",
                    own.len(),
                    own[0]
                );
            }
        } else {
            compare_case(&root, name, world, script, msaa, text_tol, &mut failures);
        }

        checked += 1;
    }

    eprintln!("shard {shard}: {checked} cases checked");
    assert!(only.is_some() || checked > 2, "only {checked} cases ran");
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn rea_gravity_layers_shard0_match_x_ite() {
    run_shard(0);
}

#[test]
fn rea_gravity_layers_shard1_match_x_ite() {
    run_shard(1);
}

#[test]
fn rea_gravity_layers_shard2_match_x_ite() {
    run_shard(2);
}

/// The evidence for STALE_SHAPES in the pinned recordings themselves (no renderer needed): in trans-skip-probe the eye has
/// moved from (0 1.6 12) at frame 10 to (0 3.8 6) at frame 40, yet X_ITE's probes at frame 40 equal, value for value, its
/// probes of trans-skip-probe-start at frame 10. The answer follows the eye of the last collision pass, not the geometry,
/// which is why the port keeps its fresh shapes (KNOWN_GAPS) instead of copying the defect.
#[test]
fn x_ite_mid_transition_probes_follow_the_old_eye() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/rea-gravity-layers-baseline");
    let load = |name: &str| -> Value {
        serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("{name}-msaa0.json"))).unwrap(),
        )
        .unwrap()
    };
    let (mid, start) = (load("trans-skip-probe"), load("trans-skip-probe-start"));
    let position = |v: &Value, frame: &str| v["poses"][frame]["position"].clone();

    assert_eq!(
        position(&mid, "10"),
        position(&start, "10"),
        "same start pose"
    );
    let (from, to) = (position(&mid, "10"), position(&mid, "40"));
    let height = |p: &Value| p[1].as_f64().unwrap();
    assert!(
        height(&to) - height(&from) > 2.0,
        "the eye rises by more than 2 m: {from} -> {to}"
    );

    let probes = mid["collisionProbes"].as_array().unwrap();
    assert!(!probes.is_empty());
    assert_eq!(
        mid["collisionProbes"], start["collisionProbes"],
        "X_ITE's probes did not follow the eye"
    );
}
