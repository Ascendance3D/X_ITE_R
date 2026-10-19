//! Batch 06R closure cases against the saved pinned X_ITE output (`oracle/rea-batch06-rem`), with the
//! tolerances of `oracle/tools/rea_b06_particles.py` (position 5e-6 + 2e-6 |ref|, velocity 3e-6 + 1e-6 |ref|,
//! everything else exact).

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

const STRIDE: usize = 28;
const GROUPS: [(&str, &[usize], f64, f64); 6] = [
    ("life", &[0], 0.0, 0.0),
    ("lifetime", &[1], 0.0, 0.0),
    ("elapsed", &[2], 0.0, 0.0),
    ("color", &[4, 5, 6, 7], 0.0, 0.0),
    ("velocity", &[8, 9, 10], 3e-6, 1e-6),
    ("position", &[24, 25, 26, 27], 5e-6, 2e-6),
];

fn floats(path: &PathBuf) -> Vec<f32> {
    std::fs::read(path).unwrap().chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect()
}

/// (max position error, max velocity error, violations) of the Rust driver against the saved run.
fn compare(case: &str) -> (f64, f64, usize) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/rea-batch06-rem").join(case);
    let out = std::env::temp_dir().join(format!("rem-{case}-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let rs_json = out.join("rs.json");
    let status = Command::new(env!("CARGO_BIN_EXE_x3d-particles-drive"))
        .arg(dir.join("world.x3dv"))
        .arg(dir.join("script.json"))
        .arg("--seeds-from")
        .arg(dir.join("js.json"))
        .arg("--out")
        .arg(&rs_json)
        .status()
        .unwrap();
    assert!(status.success(), "{case}: the driver failed");

    let js: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("js.json")).unwrap()).unwrap();
    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&rs_json).unwrap()).unwrap();
    let (a, b) = (floats(&dir.join("js.f32")), floats(&out.join("rs.f32")));
    let (mut position, mut velocity, mut violations) = (0.0f64, 0.0f64, 0usize);

    for (frame, d) in js["dumps"].as_object().unwrap() {
        let n = d["numParticles"].as_u64().unwrap() as usize;
        let (oa, ob) = (d["offset"].as_u64().unwrap() as usize, rs["dumps"][frame]["offset"].as_u64().unwrap() as usize);
        assert_eq!(rs["dumps"][frame]["numParticles"].as_u64().unwrap() as usize, n, "{case} frame {frame}: particle count");

        for i in 0..n {
            for (name, cols, abs, rel) in GROUPS {
                for &c in cols {
                    let (x, y) = (a[oa + i * STRIDE + c] as f64, b[ob + i * STRIDE + c] as f64);

                    if x.is_nan() && y.is_nan() {
                        continue;
                    }

                    let err = (x - y).abs();

                    if name == "position" {
                        position = position.max(err);
                    } else if name == "velocity" {
                        velocity = velocity.max(err);
                    }

                    if err.is_nan() || err > abs + rel * x.abs() {
                        violations += 1;
                    }
                }
            }
        }
    }

    let _ = std::fs::remove_dir_all(&out);
    (position, velocity, violations)
}

/// Known difference. One particle (9, born in frame 20) starts 2.4e-5 away: its ray starts on the face
/// x = 3.5 at x = 3.5000002 (the float32 of the barycentric point), the ray is nearly parallel to the
/// face (direction.x = 0.0088) and the intersection moves by 2.4e-7 / 0.0088 along it, so one ulp of
/// the origin is amplified to 1.7e-5. Computing the surface point as one multiply and two fused
/// multiply-adds (`r.z * v0` rounded last, `fma (v0, r.z, fma (v1, r.x, r.y * v2))`) removes it (8e-7),
/// but the same change moves `particles-volume-twocubes-stat` row 1 from 1.5e-5 to 2.1e-4 away from
/// X_ITE, so it is not the GPU's pattern; no pattern of 192 tried (surface point, barycentric hit, dot,
/// cross) fits both. The test pins the state so that any change is looked at.
#[test]
fn volume_emitter_with_an_offset_volume_stays_at_its_known_difference() {
    let (position, velocity, violations) = compare("volume-offset");
    assert_eq!(velocity, 0.0);
    assert!(position <= 1.8e-5, "position error {position:e}");
    assert_eq!(violations, 82, "position error {position:e}");
}

/// Known difference, cause identified: the velocity of particle 1 already differs by 9.5e-7 on the
/// frame it is born (PointEmitter, spherical direction: sin, cos and sqrt of the GPU), before any
/// physics acts, and 72 frames of that error leave one position component 6.7e-6 away (limit 5.96e-6).
/// The test pins the state so that any change (better or worse) is looked at.
#[test]
fn bounded_physics_model_small_box_stays_at_its_known_difference() {
    let (position, velocity, violations) = compare("bounded-small");
    assert!(velocity <= 1.5e-6, "velocity error {velocity:e}");
    assert!(position <= 7e-6, "position error {position:e}");
    assert_eq!(violations, 1, "position error {position:e}");
}
