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
//! The WALK cases with gravity depend on the GPU: the avatar rests on a depth read, and the pinned page itself gives other poses
//! and other ProximitySensor event counts on the Apple M1 (ANGLE Metal) than on the NVIDIA host, in three identical runs each. So
//! oracle/rea-nav-touch-apple-m1-baseline holds the M1 recording of the 6 cases that differ there
//! (oracle/tools/record_platform_baseline.py), and `X3D_REFERENCE_PLATFORM=apple-m1` selects it (`common::reference`). Measured
//! 2026-11-12 on the physical M1: against the NVIDIA recording 4 cases differ by one `position_changed` event; against the M1
//! recording all 24 cases pass, the event counts included.
//!
//! `REA_ONLY=<substring>` restricts the cases.

mod common;

use common::gpu::renderer_available;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

/// The largest distance of a pose from X_ITE's. Measured on the RTX 3060 Ti with the page's frame rate
/// (`Viewer3d::scripted_frame_rate`): 0 in every frame of every case. On the Apple M1 the comparison is with the M1 recording of
/// the pinned page (see the module documentation), within this bound. Other GPUs are not measured.
const POSE_TOLERANCE: f64 = 1e-4;

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

/// The scene text of a frame. It holds the count `PX_n` of ProximitySensor events, which is compared in every world: a WALK
/// avatar that falls with a frame rate other than the page's (`getCurrentFrameRate` is `1 / dt` there) rests 2e-7 off X_ITE's
/// height and moves by that much from frame to frame, and each such move sends a `position_changed` (measured before
/// `Viewer3d::scripted_frame_rate`: X_ITE 2 events, X_ITE_R 55 in `walk-look-around`).
fn scene_text(v: &Value, frame: &str) -> Option<String> {
    Some(v["vrml"].get(frame)?.as_str()?.to_string())
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

/// The captured pose frames and the dumped scene text frames, sorted.
fn frame_set(v: &Value) -> (Vec<u32>, Vec<u32>) {
    let keys = |key: &str| {
        let mut f: Vec<u32> = v[key]
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.parse().unwrap())
            .collect();

        f.sort_unstable();
        f
    };

    (keys("poses"), keys("vrml"))
}

/// The first frame whose pose or whose scene text differs.
fn first_difference(r: &Run) -> Option<String> {
    if frame_set(&r.js) != frame_set(&r.rs) {
        return Some(format!(
            "the frames differ: X_ITE {:?} / Rust {:?}",
            frame_set(&r.js),
            frame_set(&r.rs)
        ));
    }

    let frames = frame_set(&r.js).0;

    for f in frames {
        let k = f.to_string();
        let d = pose_distance(&pose(&r.js, &k), &pose(&r.rs, &k));

        if d > POSE_TOLERANCE {
            return Some(format!(
                "pose frame {k} differs by {d}: X_ITE {} {} / Rust {} {}",
                r.js["poses"][&k]["position"],
                r.js["poses"][&k]["orientation"],
                r.rs["poses"][&k]["position"],
                r.rs["poses"][&k]["orientation"]
            ));
        }

        if scene_text(&r.js, &k) != scene_text(&r.rs, &k) {
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

    if frame_set(&r.js) != frame_set(&r.rs) {
        fail("the captured frames differ".to_string());
    }

    if r.js.get("log") != r.rs.get("log") {
        fail(format!("log differs: {} / {}", r.js["log"], r.rs["log"]));
    }

    // Before the cancel the poses and the scene texts (the Script `hist`) are equal.
    for k in 1..20 {
        let ks = k.to_string();

        if pose_distance(&pose(&r.js, &ks), &pose(&r.rs, &ks)) > 1e-4 {
            fail(format!("frames before the cancel must be equal, frame {k}"));
        }

        if scene_text(&r.js, &ks) != scene_text(&r.rs, &ks) {
            fail(format!("scene text before the cancel differs, frame {k}"));
        }
    }

    // The deliberate difference in the history: the ProximitySensor `position_changed` events that X_ITE keeps producing while it
    // goes on driving (`PX_n`, one per frame) are missing from X_ITE_R. Everything else in `hist` is equal in the last frame.
    let px = |v: &Value| -> u32 {
        let h = hist(v, "70");
        let (_, n) = h.rsplit_once("PX_n=").unwrap_or(("", "0"));

        n.trim().parse().unwrap_or(0)
    };
    let words = |v: &Value| -> String {
        let h = hist(v, "70");
        h.rsplit_once(" PX_n=")
            .map_or(h.clone(), |(a, _)| a.to_string())
    };

    if words(&r.js) != words(&r.rs) || px(&r.js) <= px(&r.rs) {
        fail(format!(
            "hist: X_ITE {:?} must have more position events than X_ITE_R {:?}",
            hist(&r.js, "90"),
            hist(&r.rs, "90")
        ));
    }

    // X_ITE has no touchcancel handler: the drive goes on every frame until the two-finger touchstart ends it.
    for k in 19..29 {
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
        // Hosted CI has no adapter: the evidence log records the skip (X3D_EVIDENCE decides whether it is allowed).
        return x3d_evidence::no_gpu("the collision and pointing passes need a Vulkan adapter");
    }

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

/// The platform recordings of this base (no GPU needed): each is a case of the base, is chosen only when the platform is named,
/// and holds the same frames as the NVIDIA recording of its case. The M1 run of 2026-11-12 kept exactly these 6 cases.
#[test]
fn the_apple_m1_recordings_are_cases_of_the_base_with_the_same_frames() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let base = "rea-nav-touch-baseline";
    let names = |file: &str| -> Vec<String> {
        std::fs::read_to_string(root.join(file))
            .unwrap()
            .lines()
            .filter_map(|l| l.split_whitespace().next().map(str::to_string))
            .collect()
    };
    let all = names("oracle/rea-nav-touch-baseline/cases.txt");
    let m1 = names("oracle/rea-nav-touch-apple-m1-baseline/cases.txt");

    assert_eq!(
        m1,
        [
            "walk-cancel-during-move",
            "walk-end-during-move",
            "walk-look-around",
            "walk-on-sensor",
            "walk-one-two-one",
            "walk-pitch-then-drive"
        ]
    );

    for name in &all {
        let nvidia = common::reference::js_json_for(&root, base, name, 0, None);
        let chosen = common::reference::js_json_for(&root, base, name, 0, Some("apple-m1"));

        assert!(nvidia.ends_with(format!("{base}/{name}-msaa0/js.json")));
        assert_eq!(chosen != nvidia, m1.contains(name), "{name}");

        if chosen != nvidia {
            let read = |p: &std::path::Path| -> Value {
                serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
            };
            let (a, b) = (read(&nvidia), read(&chosen));
            let keys = |v: &Value, k: &str| -> Vec<String> {
                let mut keys: Vec<String> = v[k].as_object().unwrap().keys().cloned().collect();
                keys.sort();
                keys
            };

            assert_eq!(b["ok"], Value::Bool(true), "{name}");
            assert_eq!(keys(&a, "poses"), keys(&b, "poses"), "{name}: pose frames");
            assert_eq!(
                keys(&a, "vrml"),
                keys(&b, "vrml"),
                "{name}: scene text frames"
            );
        }
    }
}
