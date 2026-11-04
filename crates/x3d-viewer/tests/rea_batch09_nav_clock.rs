//! The fly and walk viewer step on the page's clock: `Date .now ()` is a millisecond number and
//! X3DFlyViewer takes `dt = (now - startTime) / 1000` of two such numbers.
//!
//! The pinned page's poses do not depend on the GPU (the NVIDIA recording and the Apple M1 recording of `walk-steps`
//! have the same poses). A viewer that took the difference of two seconds numbers instead (`t0 + k * dt`, about 9.5e8,
//! where one float64 step is 1.2e-7) stepped a different distance by up to 1e-7 per frame, so its probes of the collision
//! pass came from another pose than the page's and a probe on the edge of a depth step (`walk-steps` probe 46 on the
//! Apple M1, 4.14e-5 off) was far off (issue #89). Frames 35 to 80 of `walk-steps` are free walking: the poses are the page's to
//! float64 rounding without a GPU. At frame 100 the avatar is on the stairs and its height comes from the depth pass; without
//! a GPU adapter the CPU stand-in differs there by 4.8e-8 (hosted CI), so frame 100 is compared only with an adapter and
//! otherwise reported as not compared (a failure under `X3D_EVIDENCE=strict`).

mod common;

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

/// Float64 rounding of the page's arithmetic (observed 0 to 1.8e-15), far below the 1e-7 of a seconds-number clock.
const POSE_TOL: f64 = 1e-13;

fn nums(v: &Value) -> Vec<f64> {
    v.as_array()
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default()
}

#[test]
fn walk_poses_follow_the_millisecond_clock_of_the_page() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let js_path = common::reference::js_json(&root, "rea-b09-nav-baseline", "walk-steps", 0);
    let js: Value = serde_json::from_str(&std::fs::read_to_string(&js_path).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!("rea-b09-clock-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let rs_json = out.join("rs.json");
    let status = Command::new(env!("CARGO_BIN_EXE_x3d-drive"))
        .current_dir(&root)
        .args([
            "oracle/synthetic-rea-b09-nav/nav-walk.wrl",
            "oracle/synthetic-rea-b09-nav/scripts/walk-steps.json",
            "--oracle",
            "oracle",
        ])
        .arg("--out")
        .arg(&rs_json)
        .arg("--warmup-from")
        .arg(&js_path)
        .status()
        .unwrap();

    assert!(status.success(), "x3d-drive exited with {status}");

    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&rs_json).unwrap()).unwrap();
    let mut checked = 0;

    for frame in ["35", "60", "80", "100"] {
        if frame == "100" {
            if let Err(e) = x3d_render::gpu::Gpu::new() {
                // Not a pass: the poses up to frame 80 were compared, frame 100 was not.
                x3d_evidence::no_gpu(format!("frame 100 not compared: {e}"));
                break;
            }
        }

        let (a, b) = (&js["poses"][frame], &rs["poses"][frame]);
        let d = nums(&a["position"])
            .iter()
            .zip(nums(&b["position"]))
            .map(|(p, q)| (p - q).abs())
            .fold(0.0, f64::max);

        assert!(
            d <= POSE_TOL,
            "frame {frame}: position {} / Rust {} ({d:.3e} apart, allowed {POSE_TOL:e})",
            a["position"],
            b["position"]
        );
        checked += 1;
    }

    assert!(checked >= 3, "checked {checked} frames");
    let _ = std::fs::remove_dir_all(&out);
}
