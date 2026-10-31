//! sub:fields.math: `toString` of every class of the pinned X_ITE's standard/Math (Numbers and Geometry) against
//! oracle/math-tostring-baseline.json, which oracle/tools/math_tostring_xite.mjs writes from X_ITE 16.5.0 (34cdf9f97) itself
//! (three runs identical). Each case holds the inputs as the hex of their IEEE 754 doubles and the exact string. No tolerance.

use serde_json::Value;
use x3d_fields::math::{
    Color3, Color4, Complex, Matrix2, Matrix3, Matrix4, Quaternion, Rotation4, Vector2, Vector3,
    Vector4,
};
use x3d_viewer::geom::{Cylinder3, Line2, Line3, Plane3, Sphere3};
use x3d_viewer::geom2::{Box2, Spheroid3};
use x3d_viewer::lookat::Box3;

fn v2(a: &[f64], i: usize) -> Vector2 {
    Vector2::new(a[i], a[i + 1])
}

fn v3(a: &[f64], i: usize) -> Vector3 {
    Vector3::new(a[i], a[i + 1], a[i + 2])
}

/// The string the port produces for `cls`, built from the inputs in the order the dumper passes them.
fn rust_string(cls: &str, a: &[f64]) -> String {
    match cls {
        "Complex" => Complex::new(a[0], a[1]).to_string(),
        "Color3" => Color3::new(a[0], a[1], a[2]).to_string(),
        "Color4" => Color4::new(a[0], a[1], a[2], a[3]).to_string(),
        "Matrix2" => Matrix2(a.try_into().unwrap()).to_string(),
        "Matrix3" => Matrix3(a.try_into().unwrap()).to_string(),
        "Matrix4" => Matrix4(a.try_into().unwrap()).to_string(),
        "Quaternion" => Quaternion::new(a[0], a[1], a[2], a[3]).to_string(),
        "Rotation4" => Rotation4::new(a[0], a[1], a[2], a[3]).to_string(),
        "Vector2" => v2(a, 0).to_string(),
        "Vector3" => v3(a, 0).to_string(),
        "Vector4" => Vector4::new(a[0], a[1], a[2], a[3]).to_string(),
        "Line2" => Line2::new(v2(a, 0), v2(a, 2)).to_string(),
        "Line3" => Line3::new(v3(a, 0), v3(a, 3)).to_string(),
        "Plane3" => Plane3::new(v3(a, 0), v3(a, 3)).to_string(),
        "Sphere3" => Sphere3::new(a[0], v3(a, 1)).to_string(),
        "Cylinder3" => Cylinder3::new(Line3::new(v3(a, 0), v3(a, 3)), a[6]).to_string(),
        "Box2" => Box2::new(&v2(a, 0), &v2(a, 2)).to_string(),
        "Box3" => {
            let mut b = Box3 {
                matrix: Matrix4::default(),
            };
            b.set(Some((&v3(a, 0), &v3(a, 3))));
            b.to_string()
        }
        "Spheroid3" => Spheroid3::new(a[0], a[1], false).to_string(),
        other => panic!("class {other} is not in the test"),
    }
}

#[test]
fn to_string_equals_pinned_x_ite_for_every_math_class() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../oracle/math-tostring-baseline.json"
    ))
    .unwrap();
    let doc: Value = serde_json::from_str(&text).unwrap();
    let cases = doc["cases"].as_array().unwrap();
    let mut classes = std::collections::BTreeSet::new();
    let mut bad = Vec::new();
    for c in cases {
        let cls = c["cls"].as_str().unwrap();
        let input: Vec<f64> = c["in"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| f64::from_bits(u64::from_str_radix(h.as_str().unwrap(), 16).unwrap()))
            .collect();
        let want = c["out"].as_str().unwrap();
        let got = rust_string(cls, &input);
        classes.insert(cls.to_string());
        if got != want {
            bad.push(format!("{cls} {input:?}: X_ITE {want:?}, Rust {got:?}"));
        }
    }
    assert_eq!(
        classes.len(),
        19,
        "the baseline must hold all 19 classes: {classes:?}"
    );
    assert!(cases.len() > 3000, "{} cases", cases.len());
    assert!(
        bad.is_empty(),
        "{} of {} differ, first 10:\n{}",
        bad.len(),
        cases.len(),
        bad.iter().take(10).cloned().collect::<Vec<_>>().join("\n")
    );
}
