//! REA batch 03: the contact test of PointPickSensor, PrimitivePickSensor and VolumePickSensor
//! against the pinned X_ITE's Ammo. oracle/pick-vectors/ammo-contacts.json holds seeded random
//! shape pairs (every shape kind X_ITE builds, rotated and non-uniformly scaled) and the result
//! of VolumePicker.contactTest in Ammo (`oracle/tools/ammo_pick_vectors.mjs`). The Rust test must
//! give the same yes/no answer for every case except the ones in oracle/pick-vectors/ammo-known.tsv.
//! The generator runs all cases in one picker (as a sensor does over frames), so the Rust test does
//! too: the contact breaking threshold grows with every shape the picker has held. The known set
//! must match exactly, so a new difference fails.

use serde_json::Value;
use std::path::PathBuf;
use x3d_fields::math::{Matrix4, Vector3};
use x3d_viewer::pick_shapes::{BaseShape, PickShapes};

fn nums(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()
}

fn shape(s: &Value) -> Option<BaseShape> {
    let n = |k: &str| s[k].as_f64().unwrap();
    let v3 = |k: &str| {
        let v = nums(&s[k]);
        [v[0], v[1], v[2]]
    };

    Some(match s["kind"].as_str().unwrap() {
        "box" => BaseShape::new_box(v3("size")),
        "sphere" => BaseShape::Sphere { radius: n("r") },
        "cone" => BaseShape::Cone { radius: n("r"), height: n("h") },
        "cylinder" => BaseShape::new_cylinder(n("r"), n("h")),
        "hull" => BaseShape::Hull { points: nums(&s["points"]).chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect() },
        "mesh" => BaseShape::Mesh { triangles: nums(&s["tris"]).chunks_exact(9).map(|t| [[t[0], t[1], t[2]], [t[3], t[4], t[5]], [t[6], t[7], t[8]]]).collect() },
        "height" => return BaseShape::new_height_field(s["xd"].as_i64().unwrap(), s["zd"].as_i64().unwrap(), n("xs"), n("zs"), &nums(&s["heights"])),
        _ => unreachable!(),
    })
}

fn matrix(v: &Value) -> Matrix4 {
    let m = nums(v);
    Matrix4(std::array::from_fn(|i| m[i]))
}

#[test]
fn contacts_match_ammo() {
    // PICK_VECTORS=<file> runs other cases (investigation); the known list then does not apply.
    let custom = std::env::var("PICK_VECTORS").ok();
    let path = custom.clone().map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/pick-vectors/ammo-contacts.json"));
    contacts_match(path, custom.is_some());
}

/// Three seed-20261018 cases (4697 cylinder/mesh, 5480 cylinder/cylinder, 11878 cylinder/height
/// field) where Ammo reports no contact: the inner compounds' dbvt walk drops the pair because
/// a non-uniformly scaled cylinder's AABB is narrower than its support function.
#[test]
fn nested_compound_tree_culls_like_ammo() {
    contacts_match(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/pick-vectors/ammo-nested-tree.json"), true);
}

fn contacts_match(path: PathBuf, custom: bool) {
    let data: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let cases = data["cases"].as_array().unwrap();
    let mut wrong = vec![];
    let mut kinds = std::collections::BTreeMap::<String, (usize, usize)>::new();

    let only: Option<usize> = std::env::var("PICK_CASE").ok().and_then(|v| v.parse().ok());

    // One picker (sensor 1) for all cases; new shapes for each case.
    let mut shapes = PickShapes::default();

    for (i, c) in cases.iter().enumerate() {
        let (ma, mb) = (matrix(&c["ma"]), matrix(&c["mb"]));
        let a = &c["a"];
        let b = shape(&c["b"]).unwrap();

        let placed_a = match a["kind"].as_str().unwrap() {
            "point" => {
                let p = nums(&a["p"]);
                PickShapes::place_point(&Vector3::new(p[0], p[1], p[2]), &ma)
            }
            _ => shapes.place(10 + 2 * i as u32, &shape(a).unwrap(), &ma),
        };
        let placed_b = shapes.place(11 + 2 * i as u32, &b, &mb);
        shapes.set_child(1, 0, Some(&placed_a));
        shapes.set_child(1, 1, Some(&placed_b));

        if only.is_some_and(|o| o != i) {
            continue;
        }

        let got = shapes.contact(1, &placed_a, &placed_b);
        if only.is_some() {
            eprintln!("case {i}: manifold {:?}", shapes.manifold(1, &placed_a, &placed_b).map(|m| m.distances()));
        }
        let want = c["hit"].as_bool().unwrap();
        let key = format!("{}-{}", a["kind"].as_str().unwrap(), c["b"]["kind"].as_str().unwrap());
        let e = kinds.entry(key.clone()).or_default();
        e.0 += 1;

        if got != want {
            e.1 += 1;
            wrong.push(format!("case {i} {key}: ammo {want}, rust {got}"));
        }
    }

    for (k, (n, w)) in &kinds {
        eprintln!("{k:20} {n:5} cases {w:3} differ");
    }

    let known_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/pick-vectors/ammo-known.tsv");
    let known: Vec<usize> = if custom { String::new() } else { std::fs::read_to_string(known_path).unwrap() }.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()).map(|l| l.split('\t').next().unwrap().parse().unwrap()).collect();
    let unexpected: Vec<&String> = wrong.iter().filter(|w| !known.iter().any(|k| w.starts_with(&format!("case {k} ")))).collect();
    let fixed: Vec<&usize> = known.iter().filter(|k| only.is_none_or(|o| o == **k) && !wrong.iter().any(|w| w.starts_with(&format!("case {k} ")))).collect();

    eprintln!("{} cases, {} differ ({} known)", cases.len(), wrong.len(), wrong.len() - unexpected.len());
    assert!(unexpected.is_empty(), "{}", unexpected.iter().take(40).map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    assert!(fixed.is_empty(), "known differences no longer differ: {fixed:?}");
}
