//! sub:nav.walk-fly (F16): touch navigation of the WALK and FLY viewers against pinned X_ITE 16.5.0 (34cdf9f97): one finger held away
//! from where it went down drives (forward, back, turn, diagonal), two fingers look around, the transitions between one, two and
//! three fingers, a touch that ends or is cancelled while moving, a finger that presses a TouchSensor, and a pitch followed by a
//! drive. Worlds, scripts: oracle/synthetic-rea-nav-touch (generator oracle/tools/gen_rea_nav_touch.py), three worlds: WALK with
//! gravity and collision (a floor, a wall ahead, `avatarSize`), WALK with `Gravity` 0 and FLY. Baselines: oracle/rea-nav-touch-baseline
//! (oracle/tools/rea_e14_e18_record.py, three identical X_ITE runs each, Chrome DevTools touch events, the virtual clock of
//! oracle/x_ite-anim.mjs). Every frame is captured: the viewpoint pose within 1e-4, and the scene text exact (the Script string `hist`
//! lists the TouchSensor events and the ProximitySensor activity and event count, so the history is compared too).
//!
//! The `cancel-during-move` cases are the kept difference of ADR 0001 row 1, pinned by name: X_ITE's viewers have no `touchcancel`
//! handler, so a drive or look around that a touch began goes on after a cancel (until a later touchend reaches the viewer);
//! X_ITE_R ends it when the touch is cancelled.
//!
//! `REA_ONLY=<substring>` restricts the cases.

mod common;

use common::gpu::renderer_available;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn nums(v: &Value) -> Vec<f64> {
    v.as_array()
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default()
}

/// The pose of the viewpoint at frame `k`: position then orientation.
fn pose(v: &Value, k: &str) -> Vec<f64> {
    let mut p = nums(&v["poses"][k]["position"]);
    p.extend(nums(&v["poses"][k]["orientation"]));
    p
}

/// The largest difference of two poses (`f64::INFINITY` when a frame is missing).
fn pose_distance(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return f64::INFINITY;
    }

    a.iter()
        .zip(b)
        .map(|(p, q)| (p - q).abs())
        .fold(0.0, f64::max)
}

/// The words of the Script string `hist` of the scene text of a frame.
fn hist(v: &Value, frame: &str) -> String {
    let text = v["vrml"][frame].as_str().unwrap_or("");
    let Some(start) = text.find("hist \"") else {
        return String::new();
    };
    let rest = &text[start + 6..];

    rest[..rest.find('"').unwrap_or(rest.len())].to_string()
}

struct Run {
    js: Value,
    rs: Value,
}

fn run(name: &str) -> Result<Run, String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let kind = ["walk-nograv", "walk", "fly"]
        .into_iter()
        .find(|k| name.starts_with(&format!("{k}-")))
        .unwrap();
    let world = format!("oracle/synthetic-rea-nav-touch/{kind}.wrl");
    let script = format!("oracle/synthetic-rea-nav-touch/scripts/{name}.json");
    let js_path = common::reference::js_json(&root, "rea-nav-touch-baseline", name, 0);
    let js: Value = serde_json::from_str(&std::fs::read_to_string(&js_path).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!("rea_nav_touch-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let rs_json = out.join("rs.json");
    let status = Command::new(env!("CARGO_BIN_EXE_x3d-drive"))
        .current_dir(&root)
        .args([world.as_str(), script.as_str(), "--oracle", "oracle"])
        .arg("--out")
        .arg(&rs_json)
        .arg("--warmup-from")
        .arg(js_path)
        .status()
        .unwrap();

    if !status.success() {
        return Err(format!("x3d-drive exited with {status}"));
    }

    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&rs_json).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(&out);

    if js["ok"] != Value::Bool(true) || rs["ok"] != Value::Bool(true) {
        return Err(format!("ok js {} rs {}", js["ok"], rs["ok"]));
    }

    Ok(Run { js, rs })
}

/// The first frame whose pose or whose scene text differs.
fn first_difference(r: &Run) -> Option<String> {
    let mut frames: Vec<u32> = r.js["poses"]
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.parse().unwrap())
        .collect();

    frames.sort_unstable();

    for f in frames {
        let k = f.to_string();
        let d = pose_distance(&pose(&r.js, &k), &pose(&r.rs, &k));

        if d > 1e-4 {
            return Some(format!(
                "pose frame {k} differs by {d}: X_ITE {} {} / Rust {} {}",
                r.js["poses"][&k]["position"],
                r.js["poses"][&k]["orientation"],
                r.rs["poses"][&k]["position"],
                r.rs["poses"][&k]["orientation"]
            ));
        }

        if r.js["vrml"].get(&k) != r.rs["vrml"].get(&k) {
            return Some(format!(
                "scene text differs at frame {k}: X_ITE {:?} / Rust {:?}",
                hist(&r.js, &k),
                hist(&r.rs, &k)
            ));
        }
    }

    if r.js.get("log") != r.rs.get("log") {
        return Some(format!("log differs: {} / {}", r.js["log"], r.rs["log"]));
    }

    None
}

/// The kept difference of ADR 0001 row 1 (`cancel-during-move`). Events: touchstart (frame 2), touchmove (3), touchcancel (20),
/// two-finger touchstart (30, which ends a drive in X_ITE), touchcancel after one finger lifted (37), a new drive (44 to 52).
fn check_cancel(name: &str, r: &Run, failures: &mut Vec<String>) {
    let z = |v: &Value, k: u32| pose(v, &k.to_string())[2];
    let mut fail = |what: String| failures.push(format!("{name}: {what}"));

    for k in 1..20 {
        let ks = k.to_string();

        if pose_distance(&pose(&r.js, &ks), &pose(&r.rs, &ks)) > 1e-4 {
            fail(format!("frames before the cancel must be equal, frame {k}"));
        }
    }

    // X_ITE has no touchcancel handler: the drive goes on every frame until the two-finger touchstart ends it.
    for k in 20..29 {
        if z(&r.js, k + 1) >= z(&r.js, k) - 0.01 {
            fail(format!(
                "X_ITE must go on driving after the cancel, frame {k}"
            ));
        }
    }

    // X_ITE_R ends the drive with the cancel (the viewer sees a touchend).
    for k in 19..30 {
        if (z(&r.rs, k + 1) - z(&r.rs, 19)).abs() > 1e-4 {
            fail(format!("X_ITE_R must stop at the cancel, frame {}", k + 1));
        }
    }

    // The last drive (frames 44 to 52) is a plain touch: the same speed in both.
    let travel = |v: &Value| {
        pose(v, "52")[..3]
            .iter()
            .zip(&pose(v, "46")[..3])
            .map(|(p, q)| (p - q) * (p - q))
            .sum::<f64>()
            .sqrt()
    };

    if (travel(&r.js) - travel(&r.rs)).abs() > 1e-4 || travel(&r.js) < 0.1 {
        fail(format!(
            "the last drive differs: X_ITE {} / Rust {}",
            travel(&r.js),
            travel(&r.rs)
        ));
    }
}

fn check(name: &str, failures: &mut Vec<String>) {
    let r = match run(name) {
        Ok(r) => r,
        Err(e) => return failures.push(format!("{name}: {e}")),
    };

    if name.ends_with("-cancel-during-move") {
        return check_cancel(name, &r, failures);
    }

    if let Some(d) = first_difference(&r) {
        failures.push(format!("{name}: {d}"));
    }
}

#[test]
fn rea_nav_touch_match_x_ite() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let cases =
        std::fs::read_to_string(root.join("oracle/rea-nav-touch-baseline/cases.txt")).unwrap();
    let only = std::env::var("REA_ONLY").ok();
    let gpu = renderer_available();

    if !gpu {
        x3d_evidence::no_gpu("the collision and pointing passes need a Vulkan adapter");
    }

    assert!(
        gpu,
        "the collision and pointing passes need a Vulkan adapter"
    );

    let mut failures = vec![];
    let mut checked = 0;

    for line in cases.lines().filter(|l| !l.trim().is_empty()) {
        let name = line.split_whitespace().next().unwrap();

        if only.as_deref().is_some_and(|o| !name.contains(o)) {
            continue;
        }

        check(name, &mut failures);
        checked += 1;
    }

    assert!(only.is_some() || checked == 24, "only {checked} cases ran");
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
