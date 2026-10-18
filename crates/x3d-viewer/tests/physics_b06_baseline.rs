//! REA batch 06 (RigidBodyPhysics): real .x3dv scenes (oracle/synthetic-rea-b06) are run through the real
//! runtime (the x3d-drive binary: Viewer3d, virtual time, the pinned frame rate) and every recorded value is
//! compared EXACTLY (f64 equality, no tolerance) with what X_ITE 16.5.0 produced for the same scene
//! (oracle/rea-batch06-baseline/NAME/js.json): every field of every listed node in every recorded
//! frame, the `getTimeStep ()` results and the frame each was made in.
//!
//! The expected numbers are X_ITE's, not ours. All 47 cases are asserted exact.

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Runs x3d-drive on NAME and returns (baseline physics, our physics).
fn run(name: &str) -> (Value, Value) {
    let oracle = root().join("oracle");
    let world = oracle.join("synthetic-rea-b06").join(format!("{name}.x3dv"));
    let script = oracle.join("synthetic-rea-b06/scripts").join(format!("{name}.json"));
    let js_path = oracle.join("rea-batch06-baseline").join(name).join("js.json");
    let out = std::env::temp_dir().join(format!("x3d-physics-b06-{}-{name}.json", std::process::id()));
    let r = Command::new(env!("CARGO_BIN_EXE_x3d-drive"))
        .args([world.as_os_str(), script.as_os_str()])
        .arg("--oracle")
        .arg(&oracle)
        .arg("--out")
        .arg(&out)
        .arg("--warmup-from")
        .arg(&js_path)
        .output()
        .expect("x3d-drive");

    assert!(r.status.success(), "{name}: x3d-drive failed: {}", String::from_utf8_lossy(&r.stderr));

    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    let js: Value = serde_json::from_str(&std::fs::read_to_string(&js_path).unwrap()).unwrap();

    let _ = std::fs::remove_file(&out);
    assert_eq!(rs["ok"], Value::Bool(true), "{name}: run not ok");
    (js["physics"].clone(), rs["physics"].clone())
}

/// Every leaf of `a` equals the leaf of `b` (numbers as f64), with its path. Returns the first difference.
fn first_difference(path: &str, a: &Value, b: &Value) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            if x.len() != y.len() {
                return Some(format!("{path}: {} keys vs {}", x.len(), y.len()));
            }
            x.iter().find_map(|(k, v)| match y.get(k) {
                Some(w) => first_difference(&format!("{path}/{k}"), v, w),
                None => Some(format!("{path}/{k}: missing")),
            })
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                return Some(format!("{path}: {} items vs {}", x.len(), y.len()));
            }
            x.iter().zip(y).enumerate().find_map(|(i, (v, w))| first_difference(&format!("{path}[{i}]"), v, w))
        }
        (Value::Number(x), Value::Number(y)) => (x.as_f64() != y.as_f64()).then(|| format!("{path}: {x} vs {y}")),
        _ => (a != b).then(|| format!("{path}: {a} vs {b}")),
    }
}

fn exact(name: &str) {
    let (js, rs) = run(name);

    assert!(js["frames"].as_object().is_some_and(|f| f.len() > 1), "{name}: the baseline has no frames");
    assert!(js["dt"].is_array(), "{name}: the baseline has no time step log");
    assert_eq!(first_difference("", &js, &rs), None, "{name}: x3d-drive differs from X_ITE");
}

macro_rules! exact_cases {
    ($($test:ident => $name:literal),* $(,)?) => { $( #[test] fn $test() { exact($name); } )* };
}

exact_cases! {
    fall_sphere => "fall-sphere",
    fall_masses => "fall-masses",
    gravity_flags => "gravity-flags",
    center_of_mass => "center-of-mass",
    collidable_offset => "collidable-offset",
    collider_bounce => "collider-bounce",
    collider_friction => "collider-friction",
    collision_space => "collision-space",
    damping => "damping",
    fields_roundtrip => "fields-roundtrip",
    finite_rotation => "finite-rotation",
    forces_torques => "forces-torques",
    inertia_ignored => "inertia-ignored",
    joint_ball => "joint-ball",
    joint_ball_offaxis => "joint-ball-offaxis",
    joint_chain => "joint-chain",
    joint_hinge_double => "joint-hinge-double",
    joint_hinge_double_rate => "joint-hinge-double-rate",
    joint_hinge_single => "joint-hinge-single",
    joint_hinge_single_all => "joint-hinge-single-all",
    joint_motor => "joint-motor",
    joint_slider => "joint-slider",
    joint_slider_gravity => "joint-slider-gravity",
    joint_universal => "joint-universal",
    prefer_accuracy => "prefer-accuracy",
    reload_inline => "reload-inline",
    rt_body => "rt-body",
    rt_collection => "rt-collection",
    rt_collider => "rt-collider",
    rt_joint => "rt-joint",
    rt_sensor => "rt-sensor",
    sensor_partial => "sensor-partial",
    shape_box => "shape-box",
    shape_box_box => "shape-box-box",
    shape_cone => "shape-cone",
    shape_cone_open => "shape-cone-open",
    shape_cylinder => "shape-cylinder",
    shape_cylinder_open => "shape-cylinder-open",
    shape_elevation => "shape-elevation",
    shape_elevation_origin => "shape-elevation-origin",
    shape_ifs_dynamic => "shape-ifs-dynamic",
    shape_ifs_floor => "shape-ifs-floor",
    shape_sphere => "shape-sphere",
    shape_thickness => "shape-thickness",
    sleep_rest => "sleep-rest",
    sleep_wake => "sleep-wake",
    velocities => "velocities",
}

/// Quirks of X_ITE that the baselines carry: they are asserted against the baseline, one by one.
#[test]
fn quirks_of_x_ite_are_in_the_baselines_and_in_ours() {
    // UniversalJoint and MotorJoint create no constraint: the outputs never leave their initial values.
    for name in ["joint-universal", "joint-motor"] {
        let (js, rs) = run(name);

        assert_eq!(first_difference("", &js, &rs), None);

        let last = js["frames"].as_object().unwrap().keys().max_by_key(|k| k.parse::<u64>().unwrap()).unwrap().clone();

        for (path, node) in rs["frames"][&last].as_object().unwrap() {
            if node["type"] == "UniversalJoint" || node["type"] == "MotorJoint" {
                for (field, v) in node.as_object().unwrap().iter().filter(|(f, _)| *f != "type") {
                    let zero = |v: &Value| v.as_f64() == Some(0.0) || v.as_array().is_some_and(|a| a.iter().all(|x| x.as_f64() == Some(0.0)));

                    assert!(zero(v), "{name}: {path}.{field} = {v}: an unsupported joint reports nothing");
                }
            }
        }
    }

    // A SliderJoint never writes separation and separationRate.
    let (_, rs) = run("joint-slider");

    for (_, frame) in rs["frames"].as_object().unwrap() {
        for (_, node) in frame.as_object().unwrap() {
            if node["type"] == "SliderJoint" {
                assert_eq!(node["separation"], serde_json::json!(0.0));
            }
        }
    }
}
