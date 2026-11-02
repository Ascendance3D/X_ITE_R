//! Issue #69, criterion 1 ("a CPU fallback where the viewer has one"): GaussianSplats has none, by decision (ADR 0013).
//!
//! `--cpu-pointing` makes picks and avatar collision use the CPU ray cast and depth stand-in. Neither draws a splat: the
//! splat quads exist only in the GPU programs, as in the pinned X_ITE 16.5.0, whose picking and collision read GPU
//! buffers only. This test pins that: with the GPU path the worlds of #275 and #277 hit the splats (the counts of the
//! pinned baselines), with `--cpu-pointing` every probe misses. If a CPU path is added later, this test must change
//! with ADR 0013, not silently.
//! Skipped without an adapter (hosted CI): the viewer needs one to start.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Runs x3d-particles-drive on `world` with `script`, optionally with `--cpu-pointing`; returns its result JSON.
fn drive(world: &Path, script: &Path, cpu: bool, tag: &str) -> Value {
    let out = std::env::temp_dir().join(format!("b69-gs-cpu-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_x3d-particles-drive"));
    cmd.arg(world)
        .arg(script)
        .arg("--out")
        .arg(out.join("rs.json"))
        // The splat quads are sized from the display viewport, 512 x 512 as in the oracle page.
        .arg("--png-dir")
        .arg(out.join("png"));

    if cpu {
        cmd.arg("--cpu-pointing");
    }

    assert!(cmd.status().unwrap().success(), "{tag}: drive failed");

    let v = json(&out.join("rs.json"));
    let _ = std::fs::remove_dir_all(&out);
    v
}

/// Probes of a pick frame that hit a shape (a miss is `{"id": 0}`).
fn pick_hits(v: &Value) -> usize {
    v["pickProbes"]["3"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["id"].as_i64().unwrap() > 0)
        .count()
}

/// getClosestObject results of frame 3 that hit a shape (a miss is id -1 at the far distance).
fn collision_hits(v: &Value) -> usize {
    v["collisionProbes"]["3"]["closest"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["id"].as_i64().unwrap() >= 0)
        .count()
}

#[test]
fn splats_have_no_cpu_stand_in_for_pointing_and_collision() {
    if x3d_render::pointing_gpu::PointingGpu::new_headless().is_err() {
        return x3d_evidence::no_gpu("no adapter: the viewer needs one to start, skipped");
    }

    let root = root();

    // Pointing: the #275 world gs-pick-basic.
    let pick_world = root.join("oracle/synthetic-b69-gs-pick/gs-pick-basic.x3dv");
    let pick_script = root.join("oracle/synthetic-b69-gs-pick/scripts/gs-pick-basic.json");
    let want = pick_hits(&json(
        &root.join("oracle/b69-gs-pick-baseline/msaa0/gs-pick-basic/js.json"),
    ));
    let gpu = pick_hits(&drive(&pick_world, &pick_script, false, "pick-gpu"));
    let cpu = pick_hits(&drive(&pick_world, &pick_script, true, "pick-cpu"));

    assert_eq!(want, 217, "the pinned baseline has 217 hits");
    assert_eq!(gpu, want, "the GPU pointing pass picks the splats");
    assert_eq!(
        cpu, 0,
        "the CPU ray cast draws no splat: every probe misses"
    );

    // Collision: the #277 world gc-basic.
    let world = root.join("oracle/synthetic-b69-gs-collision/gc-basic.x3dv");
    let script = root.join("oracle/synthetic-b69-gs-collision/scripts/gc-basic.json");
    let want = collision_hits(&json(
        &root.join("oracle/b69-gs-collision-baseline/gc-basic/js.json"),
    ));
    let gpu = collision_hits(&drive(&world, &script, false, "col-gpu"));
    let cpu = collision_hits(&drive(&world, &script, true, "col-cpu"));

    assert_eq!(want, 7, "the pinned baseline has 7 closest-object hits");
    assert_eq!(gpu, want, "the GPU depth buffer sees the splats");
    assert_eq!(cpu, 0, "the CPU depth stand-in draws no splat");
}
