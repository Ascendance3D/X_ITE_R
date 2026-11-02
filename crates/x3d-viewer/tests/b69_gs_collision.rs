//! GaussianSplats avatar collision (issue #69, pass 4) against pinned X_ITE 16.5.0.
//!
//! The worlds and scripts are oracle/synthetic-b69-gs-collision (oracle/tools/gen_b69_gs_collision.py). A case's baseline is
//! oracle/b69-gs-collision-baseline/NAME/ (`js.json`, `js.f32`), recorded by `oracle/tools/b69_gs_collision_record.py record`
//! with `x3d-oracle particles` (five byte-identical runs). Its `collisionProbes` are, after the listed frames: the names of
//! the frame's collision shapes, getClosestObject along nine directions (hit, shape, distance, normal), constrainTranslation of
//! a few moves, the user pose, the viewpoint's positionOffset and the named Collision nodes' isActive and collideTime. Each
//! case runs `x3d-particles-drive` (the same probes in the embedded viewer, whose collision depth buffer is on the wgpu GPU)
//! and compares them like rea_b15_particle_collision: hit states, shape names, poses and Collision nodes exactly, distances,
//! normals and translations within PROBE_TOL.
//!
//! A GaussianSplats shape has no CPU stand-in in the collision depth buffer, so there is no `--cpu-pointing` run here. The
//! test needs an adapter and is skipped without one (hosted CI) through `x3d_evidence::no_gpu`. `REA_ONLY=<substring>`
//! restricts the cases.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Allowed absolute difference of a probe's distance, normal and constrained translation.
const PROBE_TOL: f64 = 1e-6;

/// Allowed difference of the pose and positionOffset.
const POSE_TOL: f64 = 1e-4;

/// Cases whose pose and positionOffset X_ITE reports this many frames later than Rust does. A WALK avatar falls under
/// gravity (the splat floor of gc-walk-floor does not stop it, in X_ITE or here), and the pinned page's fall starts two
/// frames after the probe script's frame 0 when the scene holds GaussianSplats: its first probes are the values Rust has
/// at frames 3 and 4. The cause is the page's asynchronous splat load (docs/gaussian-splats-walk-start-lag.md): the
/// NavigationInfo's viewer ("WALK") takes effect when the browser's `initialized` event fires, that waits for loadCount 0,
/// and the splat worker's ready and first sorted message keep loadCount at 1 for two more traversed frames. Rust has no
/// such load, so its avatar is a WALK avatar from the first frame. gc-walk-lag-control (no splats) shows no lag, and
/// the splat count does not change it (gc-walk-lag-n441, gc-walk-lag-n3969). The test compares Rust's frame k with
/// X_ITE's frame k + LAG; `walk_start_lag_follows_the_splat_load` pins the measured difference in the X_ITE baselines.
const LAGGED: &[(&str, u64)] = &[
    ("gc-walk-floor", 2),
    ("gc-walk-lag-n441", 2),
    ("gc-walk-lag-n3969", 2),
];

/// (case, frame, why Rust differs).
const KNOWN_GAPS: &[(&str, &str, &str)] = &[];

type Gaps = &'static [(&'static str, &'static str, &'static str)];

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

fn run_case(root: &Path, name: &str, cpu: bool, gaps: Gaps, failures: &mut Vec<String>) {
    let base = root.join("oracle/b69-gs-collision-baseline").join(name);
    let world = root
        .join("oracle/synthetic-b69-gs-collision")
        .join(format!("{name}.x3dv"));
    let script_path = root
        .join("oracle/synthetic-b69-gs-collision/scripts")
        .join(format!("{name}.json"));
    let script = json(&script_path);
    let js = json(&base.join("js.json"));
    let out = std::env::temp_dir().join(format!(
        "b69-gs-collision-{}-{name}{}",
        std::process::id(),
        if cpu { "-cpu" } else { "" }
    ));
    std::fs::create_dir_all(&out).unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_x3d-particles-drive"));
    cmd.arg(&world)
        .arg(&script_path)
        .arg("--out")
        .arg(out.join("rs.json"))
        // --png-dir sizes the canvas like the oracle page (512 x 512): the splat quads are sized from the display viewport.
        .arg("--png-dir")
        .arg(out.join("png"));

    if script["seedFree"] != Value::Bool(true) {
        cmd.arg("--seeds-from").arg(base.join("js.json"));
    }

    if cpu {
        cmd.arg("--cpu-pointing");
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
        || rs["depthGpu"] != Value::Bool(!cpu)
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

        let lagged = LAGGED
            .iter()
            .find(|(case, _)| *case == name)
            .map(|(_, lag)| *lag);
        let mut w = w.clone();

        if let Some(lag) = lagged {
            // X_ITE's whole frame k + lag against Rust's frame k (every probe is taken at the avatar's pose); a frame
            // without such a partner is not compared.
            match frame
                .parse::<u64>()
                .ok()
                .and_then(|f| want.get(&(f + lag).to_string()))
            {
                Some(a) => w = a.clone(),
                None => continue,
            }
        }

        let (hits, values) = compare_frame(&w, g);
        let gap = gaps
            .iter()
            .find(|(case, f, _)| *case == name && (*f == "*" || f == frame));

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

    for (_, frame, why) in gaps.iter().filter(|(case, _, _)| *case == name) {
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

/// Every baseline case (or those `REA_ONLY` names), `threads` drives at a time.
fn check_all(gaps: Gaps, threads: usize) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let only = std::env::var("REA_ONLY").ok();
    let mut names: Vec<String> = std::fs::read_dir(root.join("oracle/b69-gs-collision-baseline"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.retain(|n| only.as_deref().is_none_or(|o| n.contains(o)));

    assert!(
        only.is_some() || names.len() >= 6,
        "baselines missing: {names:?}"
    );

    let failures = std::sync::Mutex::new(vec![]);
    let next = std::sync::atomic::AtomicUsize::new(0);

    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(name) = names.get(i) else { break };
                    let mut own = vec![];

                    run_case(&root, name, false, gaps, &mut own);
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

#[test]
fn b69_gs_collision_matches_x_ite() {
    if x3d_render::pointing_gpu::PointingGpu::new_headless().is_err() {
        return x3d_evidence::no_gpu("no adapter: the collision depth buffer needs a GPU, skipped");
    }

    check_all(KNOWN_GAPS, 3);
}

/// X_ITE's recorded fall (pose y per frame) of a gc-walk case.
fn walk_fall(root: &Path, name: &str) -> Vec<f64> {
    let v = json(
        &root
            .join("oracle/b69-gs-collision-baseline")
            .join(name)
            .join("js.json"),
    );
    let probes = v["collisionProbes"].as_object().unwrap();

    (1..=14)
        .map(|k| {
            probes[&k.to_string()]["pose"]["position"][1]
                .as_f64()
                .unwrap()
        })
        .collect()
}

/// Issue #69: in the X_ITE baselines the WALK fall of a scene with splats is exactly LAG frames behind the same fall
/// without splats, for every splat count recorded. This is data only (no GPU), so hosted CI checks it too.
#[test]
fn walk_start_lag_follows_the_splat_load() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let control = walk_fall(&root, "gc-walk-lag-control");

    assert!(control[0] < -0.3 && control[0] > -0.5, "{control:?}");

    for (name, lag) in LAGGED {
        let fall = walk_fall(&root, name);
        let lag = *lag as usize;

        assert!(
            (fall[0] - control[0]).abs() > 0.1,
            "{name} starts level with the control: {fall:?}"
        );

        for k in 0..control.len() - lag {
            assert!(
                (fall[k + lag] - control[k]).abs() < 1e-6,
                "{name} frame {} is not the control's frame {}: {} vs {}",
                k + 1 + lag,
                k + 1,
                fall[k + lag],
                control[k]
            );
        }
    }
}

/// Issue #69, MSAA 4: the collision depth buffer is offscreen, so a multisampled display must change no probe. The pinned page
/// recorded at `--aa --ms 4` (oracle/b69-gs-collision-msaa4-baseline, `b69_gs_collision_record.py record NAME 4`, five
/// byte-identical runs each) gives the same `collisionProbes` as at MSAA 0 in every case, WALK fall included. Rust has no
/// display-dependent collision path, so the MSAA 0 comparison above covers both. Data only (no GPU).
#[test]
fn collision_probes_do_not_depend_on_msaa() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let mut n = 0;

    for e in std::fs::read_dir(root.join("oracle/b69-gs-collision-baseline")).unwrap() {
        let name = e.unwrap().file_name().to_string_lossy().into_owned();
        let at = |dir: &str| json(&root.join("oracle").join(dir).join(&name).join("js.json"));
        let (a, b) = (
            at("b69-gs-collision-baseline"),
            at("b69-gs-collision-msaa4-baseline"),
        );

        assert_eq!(
            a["collisionProbes"], b["collisionProbes"],
            "{name}: MSAA 4 changes the collision probes"
        );
        n += 1;
    }

    assert!(n >= 13, "cases missing: {n}");
}
