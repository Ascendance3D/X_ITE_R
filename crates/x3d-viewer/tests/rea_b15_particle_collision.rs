//! ParticleSystem avatar collision (lane XITE_R_PARTICLE_AVATAR_COLLISION_02) against pinned X_ITE 16.5.0.
//!
//! The worlds and scripts are oracle/synthetic-rea-b15-particle-collision (oracle/tools/gen_rea_b15_particle_collision.py).
//! A case's X_ITE baseline is oracle/rea-batch15-particle-collision-baseline/NAME/ (`js.json`, `js.f32`), recorded by
//! `oracle/tools/rea_b15_collision.py record` with `x3d-oracle particles` (three identical runs). Its `collisionProbes`
//! are, after the listed frames: the DEF names of the frame's collision shapes, getClosestObject along nine directions
//! (hit, shape, distance, normal), constrainTranslation of a few moves, the user pose, the viewpoint's positionOffset
//! (WALK gravity and stepping up) and the named Collision nodes' isActive and collideTime.
//!
//! Each case runs `x3d-particles-drive` with the baseline's seeds (none for `"seedFree"` scripts) and compares:
//! * the particle count of every step, exactly;
//! * the collision shapes, as sets of names (the run lists its ParticleSystems after the Shapes, X_ITE in traversal
//!   order);
//! * every probe direction: the hit state and shape name exactly, the distance and normal within [`PROBE_TOL`] (that of
//!   rea_batch09_nav's collision probes);
//! * every constrained translation within [`PROBE_TOL`];
//! * the pose and positionOffset within 1e-4 (rea_batch09_nav's pose limit);
//! * isActive exactly and collideTime within 1e-9 of every named Collision node.
//!
//! KNOWN_GAPS lists (case, frame) pairs whose distances, normals and translations may differ; the hit states must still
//! match there, and the test requires the gap to keep differing.
//!
//! The collision depth buffer needs a GPU: without an adapter the test is skipped. `REA_ONLY=<substring>` restricts the
//! cases.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Allowed absolute difference of a probe's distance, normal and constrained translation.
const PROBE_TOL: f64 = 1e-6;

/// Allowed difference of the pose and positionOffset.
const POSE_TOL: f64 = 1e-4;

/// (case, frame, why Rust differs).
const KNOWN_GAPS: &[(&str, &str, &str)] = &[(
    "pc-line",
    "45",
    "LINE particles: one line leaves the depth buffer through its right edge, and X_ITE's depth of the last pixel it \
     covers there is 1.7e-5 farther than the diamond-exit rasterization with 1/256 px vertex snapping gives (every other \
     line fragment of the case matches); the closest object along (-0.6 0 -0.8) and the two translations it decides move \
     with it",
)];

fn json(path: &Path) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    )
    .unwrap()
}

fn nums(v: &Value) -> Vec<f64> {
    v.as_array()
        .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect())
        .unwrap_or_default()
}

/// The largest absolute difference of two number arrays (infinite when their lengths differ).
fn diff(a: &Value, b: &Value) -> f64 {
    let (a, b) = (nums(a), nums(b));

    if a.len() != b.len() {
        return f64::INFINITY;
    }

    a.iter()
        .zip(&b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
}

/// Compare the probes of one frame: (hit or shape differences, value differences, first of each).
fn compare_frame(want: &Value, got: &Value) -> (Vec<String>, Vec<String>) {
    let mut hits = vec![];
    let mut values = vec![];
    let names = |v: &Value| {
        let mut n: Vec<String> = v
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap_or("").to_string())
            .collect();
        n.sort();
        n
    };

    if names(&want["shapes"]) != names(&got["shapes"]) {
        hits.push(format!(
            "shapes X_ITE {} / Rust {}",
            want["shapes"], got["shapes"]
        ));
    }

    let (wc, gc) = (
        want["closest"].as_array().unwrap(),
        got["closest"].as_array().unwrap(),
    );

    if wc.len() != gc.len() {
        hits.push(format!("{} / {} directions", wc.len(), gc.len()));
    }

    for (i, (p, q)) in wc.iter().zip(gc).enumerate() {
        let hit = |v: &Value| v["id"].as_i64().unwrap_or(-1) >= 0;

        if hit(p) != hit(q) || (hit(p) && p["shape"] != q["shape"]) {
            hits.push(format!("direction {i}: X_ITE {p} / Rust {q}"));
            continue;
        }

        if !hit(p) {
            continue;
        }

        let dd = (p["distance"].as_f64().unwrap_or(f64::NAN)
            - q["distance"].as_f64().unwrap_or(f64::NAN))
        .abs();
        let dn = diff(&p["normal"], &q["normal"]);

        if !(dd <= PROBE_TOL && dn <= PROBE_TOL) {
            values.push(format!(
                "direction {i}: X_ITE {p} / Rust {q} (distance {dd:.2e}, normal {dn:.2e})"
            ));
        }
    }

    for (i, (p, q)) in want["constrained"]
        .as_array()
        .unwrap()
        .iter()
        .zip(got["constrained"].as_array().unwrap())
        .enumerate()
    {
        let d = diff(p, q);

        if !(d <= PROBE_TOL) {
            values.push(format!("translation {i}: X_ITE {p} / Rust {q} ({d:.2e})"));
        }
    }

    for key in ["position", "orientation"] {
        let d = diff(&want["pose"][key], &got["pose"][key]);

        if !(d <= POSE_TOL) {
            hits.push(format!(
                "pose {key}: X_ITE {} / Rust {}",
                want["pose"][key], got["pose"][key]
            ));
        }
    }

    if !(diff(&want["positionOffset"], &got["positionOffset"]) <= POSE_TOL) {
        hits.push(format!(
            "positionOffset: X_ITE {} / Rust {}",
            want["positionOffset"], got["positionOffset"]
        ));
    }

    for (node, p) in want["collisions"].as_object().unwrap() {
        let q = &got["collisions"][node];
        let dt = (p["collideTime"].as_f64().unwrap_or(f64::NAN)
            - q["collideTime"].as_f64().unwrap_or(f64::NAN))
        .abs();

        if p["isActive"] != q["isActive"] || !(dt <= 1e-9) {
            hits.push(format!("Collision {node}: X_ITE {p} / Rust {q}"));
        }
    }

    (hits, values)
}

fn run_case(root: &Path, name: &str, failures: &mut Vec<String>) {
    let base = root
        .join("oracle/rea-batch15-particle-collision-baseline")
        .join(name);
    let world = root
        .join("oracle/synthetic-rea-b15-particle-collision")
        .join(format!("{name}.x3dv"));
    let script_path = root
        .join("oracle/synthetic-rea-b15-particle-collision/scripts")
        .join(format!("{name}.json"));
    let script = json(&script_path);
    let js = json(&base.join("js.json"));
    let out = std::env::temp_dir().join(format!("rea-b15-collision-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_x3d-particles-drive"));
    cmd.arg(&world)
        .arg(&script_path)
        .arg("--out")
        .arg(out.join("rs.json"));

    if script["seedFree"] != Value::Bool(true) {
        cmd.arg("--seeds-from").arg(base.join("js.json"));
    }

    let status = cmd.status().unwrap();
    let mut fail = |what: String| failures.push(format!("{name}: {what}"));

    if !status.success() {
        return fail(format!(
            "x3d-particles-drive exited with {status}: {}",
            std::fs::read_to_string(out.join("rs.json")).unwrap_or_default()
        ));
    }

    let rs = json(&out.join("rs.json"));

    if js["ok"] != Value::Bool(true)
        || rs["ok"] != Value::Bool(true)
        || rs["depthGpu"] != Value::Bool(true)
    {
        return fail(format!(
            "ok js {} rs {}, depth GPU {}",
            js["ok"], rs["ok"], rs["depthGpu"]
        ));
    }

    let counts = |r: &Value| {
        r["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["numParticles"].as_u64())
            .collect::<Vec<_>>()
    };

    if counts(&js) != counts(&rs) {
        fail(format!(
            "numParticles per step: X_ITE {:?} / Rust {:?}",
            counts(&js),
            counts(&rs)
        ));
    }

    let want = js["collisionProbes"].as_object().unwrap();
    let mut gap_values = 0;
    let mut hit_count = 0;

    for (frame, w) in want {
        let g = &rs["collisionProbes"][frame];

        if g.is_null() {
            fail(format!("frame {frame}: no Rust probe"));
            continue;
        }

        hit_count += w["closest"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["id"].as_i64().unwrap_or(-1) >= 0)
            .count();

        let (hits, values) = compare_frame(w, g);
        let gap = KNOWN_GAPS
            .iter()
            .find(|(case, f, _)| *case == name && f == frame);

        for h in hits {
            fail(format!("frame {frame}: {h}"));
        }

        match gap {
            Some(_) => gap_values += values.len(),
            None => {
                for v in values {
                    fail(format!("frame {frame}: {v}"));
                }
            }
        }
    }

    eprintln!(
        "{name}: {} probe frames, {hit_count} X_ITE hits",
        want.len()
    );

    for (_, frame, why) in KNOWN_GAPS.iter().filter(|(case, _, _)| *case == name) {
        if gap_values == 0 {
            fail(format!(
                "frame {frame} matches X_ITE now; remove it from KNOWN_GAPS ({why})"
            ));
        } else {
            eprintln!("{name}: known gap at frame {frame}, {gap_values} value differences ({why})");
        }
    }

    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn rea_b15_particle_collision_matches_x_ite() {
    if x3d_render::pointing_gpu::PointingGpu::new_headless().is_err() {
        return eprintln!("no adapter: the collision depth buffer needs a GPU, skipped");
    }

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let only = std::env::var("REA_ONLY").ok();
    let mut names: Vec<String> =
        std::fs::read_dir(root.join("oracle/rea-batch15-particle-collision-baseline"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
    names.sort();
    names.retain(|n| only.as_deref().is_none_or(|o| n.contains(o)));

    assert!(
        only.is_some() || names.len() >= 14,
        "baselines missing: {names:?}"
    );

    for (gap, _, _) in KNOWN_GAPS {
        assert!(
            only.is_some() || names.iter().any(|n| n == gap),
            "KNOWN_GAPS names {gap}, which has no baseline"
        );
    }

    // Four drives at a time, like rea_b14_particle_pick (each holds a GPU device).
    let failures = std::sync::Mutex::new(vec![]);
    let next = std::sync::atomic::AtomicUsize::new(0);

    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(name) = names.get(i) else { break };
                    let mut own = vec![];

                    run_case(&root, name, &mut own);
                    failures.lock().unwrap().extend(own);
                }
            });
        }
    });

    let failures = failures.into_inner().unwrap();
    eprintln!("{} cases checked", names.len());
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
