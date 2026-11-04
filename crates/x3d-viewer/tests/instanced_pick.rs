//! InstancedShape and the picking engine against pinned X_ITE 16.5.0 (oracle/instanced-pick-baseline/ip-sweep.json, made by
//! `oracle/tools/rea_instanced_pick.py record`, three identical runs). World and script: oracle/synthetic-instanced-pick
//! (generator oracle/tools/gen_instanced_pick.py).
//!
//! X_ITE collects only the base geometry of an InstancedShape under the shape's own matrix (never an instance matrix), does not take
//! an InstancedShape as a `pickTarget`, and takes it as a TransformSensor `targetObject` (X3DShapeNode). Five pick sensors per row
//! (LinePickSensor GEOMETRY and BOUNDS, Primitive-, Volume- and PointPickSensor) sweep four rows of targets and two TransformSensors
//! watch a moving InstancedShape. The scene text (the Script's frame:event history) is compared exactly. No GPU is used, so
//! hosted CI runs this test.

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn instanced_shape_picking_matches_x_ite() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let baseline = root.join("oracle/instanced-pick-baseline/ip-sweep.json");
    let js: Value = serde_json::from_str(&std::fs::read_to_string(&baseline).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!("instanced-pick-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let rs_json = out.join("rs.json");
    let status = Command::new(env!("CARGO_BIN_EXE_x3d-drive"))
        .current_dir(&root)
        .args([
            "oracle/synthetic-instanced-pick/ip-sweep.wrl",
            "oracle/synthetic-instanced-pick/ip-sweep.json",
            "--oracle",
            "oracle",
        ])
        .arg("--out")
        .arg(&rs_json)
        .arg("--warmup-from")
        .arg(&baseline)
        .status()
        .unwrap();
    assert!(status.success(), "x3d-drive exited with {status}");
    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&rs_json).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(&out);

    assert_eq!(js["ok"], Value::Bool(true));
    assert_eq!(rs["ok"], Value::Bool(true));

    let frames = js["vrml"].as_object().unwrap();
    assert!(frames.len() >= 2, "baseline has no scene text");

    for (frame, want) in frames {
        let (want, got) = (want.as_str().unwrap(), rs["vrml"][frame].as_str().unwrap());

        if want != got {
            let (a, b): (Vec<&str>, Vec<&str>) = (
                history(want).split(' ').collect(),
                history(got).split(' ').collect(),
            );
            let at = (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i));
            panic!(
                "scene text differs at frame {frame}; first differing history entry #{at:?}: X_ITE {:?} / Rust {:?}",
                at.and_then(|i| a.get(i)),
                at.and_then(|i| b.get(i))
            );
        }
    }

    assert_eq!(js.get("log"), rs.get("log"));
    assert_eq!(js.get("prints"), rs.get("prints"));

    // The world must have produced events at all (a silent sensor on both sides would pass the text comparison).
    let last = frames
        .keys()
        .max_by_key(|k| k.parse::<u32>().unwrap())
        .unwrap();
    let h = history(js["vrml"][last].as_str().unwrap());

    for tag in [
        "S0LG.pickedPoint",
        "S2PR.pickedGeometry",
        "TSI.position_changed",
        "TSM.isActive=false",
    ] {
        assert!(h.contains(tag), "the baseline history has no {tag}");
    }

    // Instances other than the base, an empty InstancedShape and an InstancedShape as `pickTarget` pick nothing in X_ITE.
    for tag in ["S3", "S4"] {
        assert!(
            !h.contains(&format!(":{tag}")),
            "X_ITE picks nothing at {tag}"
        );
    }
}

/// The `hist` field of the logging Script in a scene text dump.
fn history(text: &str) -> &str {
    let from = text.find("hist \"").expect("hist field") + 6;
    &text[from..from + text[from..].find('"').unwrap()]
}
