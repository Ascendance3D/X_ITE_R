use std::f64;
use x3d_fields::math::*;

#[test]
fn test_vector2_format() {
    assert_eq!(Vector2::new(1.0, 2.0).to_string(), "1 2"); // integers without .0
    assert_eq!(Vector2::new(-0.0, 0.5).to_string(), "0 0.5"); // -0 -> "0", fraction
    assert_eq!(
        Vector2::new(-3.25, 0.1 + 0.2).to_string(),
        "-3.25 0.30000000000000004"
    ); // negative, 0.1+0.2
    assert_eq!(Vector2::new(1e21, 1e-7).to_string(), "1e+21 1e-7"); // large/small exponents
    assert_eq!(
        Vector2::new(f64::NAN, f64::INFINITY).to_string(),
        "NaN Infinity"
    );
    assert_eq!(
        Vector2::new(f64::NEG_INFINITY, 0.0).to_string(),
        "-Infinity 0"
    );
}

#[test]
fn test_vector3_format() {
    assert_eq!(Vector3::new(1.0, 2.0, -3.5).to_string(), "1 2 -3.5");
    assert_eq!(Vector3::new(-0.0, 0.5, 1e25).to_string(), "0 0.5 1e+25");
}

#[test]
fn test_vector4_format() {
    assert_eq!(Vector4::new(1.0, 2.0, 3.0, 4.0).to_string(), "1 2 3 4");
    assert_eq!(
        Vector4::new(-0.0, 0.5, 1e25, -f64::INFINITY).to_string(),
        "0 0.5 1e+25 -Infinity"
    );
}

#[test]
fn test_color3_format() {
    assert_eq!(Color3::from_raw(0.0, 0.5, 1.0).to_string(), "0 0.5 1");
}

#[test]
fn test_color4_format() {
    // Note: color4 new() clamps to [0,1]
    let c = Color4::new(0.0, 0.5, 1.0, 0.8);
    assert_eq!(c.to_string(), "0 0.5 1 0.8");
}

#[test]
fn test_quaternion_format() {
    let q = Quaternion::new(1.0, 0.0, -0.0, 0.5);
    assert_eq!(q.to_string(), "1 0 0 0.5");
}

#[test]
fn test_rotation4_format() {
    let r = Rotation4::new(0.0, 1.0, 0.0, 3.5);
    assert_eq!(r.to_string(), "0 1 0 3.5");

    // test tainted
    let mut rt = Rotation4::new(0.0, 2.0, 0.0, std::f64::consts::PI);
    rt.set_quaternion(&Quaternion::new(0.0, 1.0, 0.0, 0.0));

    // store original for comparison
    let r_clone = rt;

    // untainted should print the quaternion's axis/angle without modifying the state
    assert_eq!(rt.to_string(), "0 1 0 3.141592653589793");
    assert_eq!(rt, r_clone); // ensure to_string on a tainted rotation doesn't modify the struct
}

#[test]
fn test_matrix3_format() {
    let m = Matrix3([1.0, -0.0, 1e21, 4.0, 5.5, 6.0, f64::NAN, 8.0, 9.0]);
    assert_eq!(m.to_string(), "1 0 1e+21 4 5.5 6 NaN 8 9");
}

#[test]
fn test_matrix4_format() {
    let m = Matrix4([
        1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
    ]);
    assert_eq!(m.to_string(), "1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16");
}
