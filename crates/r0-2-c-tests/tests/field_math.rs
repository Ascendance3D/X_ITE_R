//! Suite 3 — Field and math compatibility (Node math flavor, the process default).
//!
//! Expected values are exact IEEE-754 bit patterns or exact strings. Sources:
//!
//! * `fixtures/matrix-oracle.json` — pinned X_ITE 34cdf9f97 `Matrix4.js` evaluated in
//!   node v24.21.0 and headless Chromium 151 (`fixtures/tools/probe-matrix.mjs`).
//!   Matrix products, determinant and inverse are bit-identical in both engines.
//!   `factor` differs by 1 ulp in the rotation axis; this file uses the node value,
//!   `js_math_chrome.rs` the Chromium value.
//! * `fixtures/js-oracle.json` — `Math.*` and X3DGenerator.js `FloatFormat` /
//!   `DoubleFormat` (pinned 34cdf9f97, `Intl.NumberFormat` with the same options),
//!   evaluated in node v24.21.0 and headless Chromium 151; both agree on every value.
//! * `crates/x3d-fields/tests/chrome_format.rs` — Chrome 155 `Intl.NumberFormat` vectors
//!   recorded by the port (oracle/README.md, "Followers").
//!
//! No test in this file calls `jsmath::set_flavor`.

use x3d_fields::math::matrix::Matrix4 as EMatrix4;
use x3d_fields::math::rotation::Rotation4 as ERotation4;
use x3d_fields::math::vector::Vector3 as EVector3;

fn bits16(a: &[f64; 16]) -> [u64; 16] {
    a.map(f64::to_bits)
}

// Pinned X_ITE: new Matrix4 () .setTransform (new Vector3 (1, 2, 3), new Rotation4 (0, 1, 0, 0.5), new Vector3 (2, 3, 4)).
const A: [u64; 16] = [
    0x3ffc1528065b7d50, 0, 0xbfeeaee8744b05f0, 0, 0, 0x4008000000000000, 0, 0,
    0x3ffeaee8744b05f0, 0, 0x400c1528065b7d50, 0, 0x3ff0000000000000, 0x4000000000000000, 0x4008000000000000, 0x3ff0000000000000,
];
// Pinned X_ITE: new Matrix4 () .setTransform (new Vector3 (-1, 0.5, 2), new Rotation4 (1, 0, 0, 0.25)).
const B: [u64; 16] = [
    0x3ff0000000000000, 0, 0, 0, 0, 0x3fef01549f7deea1, 0x3fcfaaeed4f31577, 0,
    0, 0xbfcfaaeed4f31577, 0x3fef01549f7deea1, 0, 0xbff0000000000000, 0x3fe0000000000000, 0x4000000000000000, 0x3ff0000000000000,
];
const DET_A: u64 = 0x4038000000000001;
const INV_A: [u64; 16] = [
    0x3fdc1528065b7d4e, 0, 0x3fbeaee8744b05ee, 0, 0, 0x3fd5555555555554, 0, 0,
    0xbfceaee8744b05ee, 0, 0x3fcc1528065b7d4e, 0, 0x3fd1f134a8150b97, 0xbfe5555555555554, 0xbfe8e5bb134dfeb9, 0x3fefffffffffffff,
];
const A_MULT_RIGHT_B: [u64; 16] = [
    0x3ffc1528065b7d50, 0x3fce5d5764a3360b, 0xbfedbab7cbfc546b, 0, 0, 0x400740ff779e72f9, 0x3fe7c0331fb65019, 0,
    0x3ffeaee8744b05f0, 0xbfebca80c3059360, 0x400b35a9b2e987e0, 0, 0, 0x3ffb213b0fa2c694, 0x40159b2ea91e6ad4, 0x3ff0000000000000,
];

fn engine_a() -> EMatrix4 {
    let mut m = EMatrix4::IDENTITY;
    m.set_transform(
        Some(&EVector3::new(1.0, 2.0, 3.0)),
        Some(&ERotation4::new(0.0, 1.0, 0.0, 0.5)),
        Some(&EVector3::new(2.0, 3.0, 4.0)),
        None,
        None,
    );
    m
}

fn engine_b() -> EMatrix4 {
    let mut m = EMatrix4::IDENTITY;
    m.set_transform(Some(&EVector3::new(-1.0, 0.5, 2.0)), Some(&ERotation4::new(1.0, 0.0, 0.0, 0.25)), None, None, None);
    m
}

fn published_a() -> vrml97::fields::math::Matrix4 {
    use vrml97::fields::math::{Matrix4, Rotation4, Vector3};
    let mut m = Matrix4::IDENTITY;
    m.set_transform(
        Some(&Vector3::new(1.0, 2.0, 3.0)),
        Some(&Rotation4::new(0.0, 1.0, 0.0, 0.5)),
        Some(&Vector3::new(2.0, 3.0, 4.0)),
        None,
        None,
    );
    m
}

fn published_b() -> vrml97::fields::math::Matrix4 {
    use vrml97::fields::math::{Matrix4, Rotation4, Vector3};
    let mut m = Matrix4::IDENTITY;
    m.set_transform(Some(&Vector3::new(-1.0, 0.5, 2.0)), Some(&Rotation4::new(1.0, 0.0, 0.0, 0.25)), None, None, None);
    m
}

// -------------------------------------------------------------------------
// Matrix4, engine and published, exact bits
// -------------------------------------------------------------------------

#[test]
fn matrix_set_transform_matches_xite() {
    assert_eq!(bits16(&engine_a().0), A);
    assert_eq!(bits16(&engine_b().0), B);
    assert_eq!(bits16(&published_a().0), A);
    assert_eq!(bits16(&published_b().0), B);
}

#[test]
fn matrix_determinant_matches_xite() {
    assert_eq!(engine_a().determinant().to_bits(), DET_A);
    assert_eq!(published_a().determinant().to_bits(), DET_A);
}

#[test]
fn matrix_inverse_matches_xite() {
    let mut e = engine_a();
    e.inverse();
    assert_eq!(bits16(&e.0), INV_A);
    let mut p = published_a();
    p.inverse();
    assert_eq!(bits16(&p.0), INV_A);
}

#[test]
fn matrix_mult_right_matches_xite() {
    let mut e = engine_a();
    e.mult_right(&engine_b());
    assert_eq!(bits16(&e.0), A_MULT_RIGHT_B);
    let mut p = published_a();
    p.mult_right(&published_b());
    assert_eq!(bits16(&p.0), A_MULT_RIGHT_B);
}

#[test]
fn matrix_identity_is_neutral() {
    let mut e = engine_a();
    e.mult_right(&EMatrix4::IDENTITY);
    assert_eq!(bits16(&e.0), A);
    assert_eq!(EMatrix4::IDENTITY.determinant(), 1.0);
}

#[test]
fn matrix_factor_matches_xite_node() {
    // node v24.21.0 result; Chromium differs in the rotation y bit (js_math_chrome.rs).
    let (mut t, mut r, mut s, mut so) = (EVector3::ZERO, ERotation4::identity(), EVector3::ZERO, ERotation4::identity());
    engine_a().factor(Some(&mut t), Some(&mut r), Some(&mut s), Some(&mut so));
    assert_eq!([t.x, t.y, t.z], [1.0, 2.0, 3.0]);
    assert_eq!([s.x, s.y, s.z], [2.0, 3.0, 4.0]);
    assert_eq!([r.x().to_bits(), r.y().to_bits(), r.z().to_bits(), r.angle().to_bits()], [0, 0x3ff000000000000d, 0, 0x3fdfffffffffffe7]);
    assert_eq!([so.x(), so.y(), so.z(), so.angle()], [0.0, 0.0, 1.0, 0.0]);
}

#[test]
fn matrix_get_transform_without_center_equals_factor() {
    // Matrix4.js getTransform: no center means this.factor (...).
    let (mut t, mut r, mut s, mut so) = (EVector3::ZERO, ERotation4::identity(), EVector3::ZERO, ERotation4::identity());
    engine_a().get_transform(Some(&mut t), Some(&mut r), Some(&mut s), Some(&mut so), None);
    assert_eq!([t.x, t.y, t.z], [1.0, 2.0, 3.0]);
    assert_eq!([s.x, s.y, s.z], [2.0, 3.0, 4.0]);
    assert_eq!([r.x().to_bits(), r.y().to_bits(), r.z().to_bits(), r.angle().to_bits()], [0, 0x3ff000000000000d, 0, 0x3fdfffffffffffe7]);
}

#[test]
fn quaternion_identity_is_0_0_0_1() {
    let q = x3d_fields::math::quaternion::Quaternion::IDENTITY;
    assert_eq!([q.x, q.y, q.z, q.w], [0.0, 0.0, 0.0, 1.0]);
    let q = vrml97::fields::math::quaternion::Quaternion::IDENTITY;
    assert_eq!([q.x, q.y, q.z, q.w], [0.0, 0.0, 0.0, 1.0]);
}

// -------------------------------------------------------------------------
// Node-flavor Math (default flavor; X1 oracle runs in node)
// -------------------------------------------------------------------------

// node v24.21.0 Math.sin/cos/tan (0.5), Math.atan2 (1, 1) — fixtures/js-oracle.json.
const SIN_0_5: u64 = 0x3fdeaee8744b05f0;
const COS_0_5: u64 = 0x3fec1528065b7d50;
const TAN_0_5: u64 = 0x3fe17b4f5bf3474a;
const ATAN2_1_1: u64 = 0x3fe921fb54442d18;

#[test]
fn node_flavor_is_the_default() {
    assert_eq!(vrml97::fields::jsmath::flavor(), vrml97::fields::jsmath::MathFlavor::Node);
    assert_eq!(x3d_fields::jsmath::flavor(), x3d_fields::jsmath::MathFlavor::Node);
}

#[test]
fn node_math_published_matches_node() {
    use vrml97::fields::jsmath as m;
    assert_eq!(m::sin(0.5).to_bits(), SIN_0_5);
    assert_eq!(m::cos(0.5).to_bits(), COS_0_5);
    assert_eq!(m::tan(0.5).to_bits(), TAN_0_5);
    assert_eq!(m::atan2(1.0, 1.0).to_bits(), ATAN2_1_1);
}

#[test]
fn node_math_engine_matches_node() {
    use x3d_fields::jsmath as m;
    assert_eq!(m::sin(0.5).to_bits(), SIN_0_5);
    assert_eq!(m::cos(0.5).to_bits(), COS_0_5);
    assert_eq!(m::tan(0.5).to_bits(), TAN_0_5);
    assert_eq!(m::atan2(1.0, 1.0).to_bits(), ATAN2_1_1);
}

// -------------------------------------------------------------------------
// Float formatting: X3DGenerator.js DoubleFormat (15) and FloatFormat (7)
// -------------------------------------------------------------------------

// (input bits, DoubleFormat, FloatFormat) — fixtures/js-oracle.json, Chromium 151 = node 24.
const GENERATOR_FORMAT: &[(u64, &str, &str)] = &[
    (0x3ff0000000000000, "1", "1"),
    (0x3feffffffffffffb, "1", "1"),
    (0x3fd3333333333334, "0.3", "0.3"),
    (0x3ff8000000000000, "1.5", "1.5"),
    (0x4004000000000000, "2.5", "2.5"),
    (0x40fe240c9fbe76c9, "123456.789", "123456.8"),
    (0x3ee4f8b588e368f1, "1e-5", "1e-5"),
    (0x444b1ae4d6e2ef50, "1e21", "1e21"),
    (0x3e90c6f7a0b5ed8d, "2.5e-7", "2.5e-7"),
    (0x3fd5555555555555, "0.333333333333333", "0.3333333"),
    (0x41678c29c0000000, "12345678", "1.234568e7"),
    (0xbfe0000000000000, "-0.5", "-0.5"),
    (0x0000000000000000, "0", "0"),
];

#[test]
fn generator_format_engine_matches_xite() {
    for (b, double, float) in GENERATOR_FORMAT {
        let v = f64::from_bits(*b);
        assert_eq!(x3d_fields::format::double_format(v), *double, "DoubleFormat({v:e})");
        assert_eq!(x3d_fields::format::float_format(v), *float, "FloatFormat({v:e})");
    }
}

/// Known difference 2 (R0.2-A section 7.4): the published formatter rounds the shortest
/// digits half away from zero; ICU first builds an approximate decimal. The only input in
/// GENERATOR_FORMAT where that matters is 0.9999999999999994 at 15 digits.
const PUBLISHED_GENERATOR_DIFFERENCES: &[(u64, &str)] = &[(0x3feffffffffffffb, "0.999999999999999")];

#[test]
fn generator_format_published_matches_xite_except_known_difference_2() {
    for (b, double, float) in GENERATOR_FORMAT {
        let v = f64::from_bits(*b);
        let got = vrml97::fields::format::double_format(v);
        match PUBLISHED_GENERATOR_DIFFERENCES.iter().find(|(k, _)| k == b) {
            Some((_, published)) => {
                assert_eq!(got, *published, "DoubleFormat({v:e}) published value");
                assert_ne!(got, *double, "known difference 2 closed for {v:e}; retire this record");
            }
            None => assert_eq!(got, *double, "DoubleFormat({v:e})"),
        }
        assert_eq!(vrml97::fields::format::float_format(v), *float, "FloatFormat({v:e})");
    }
}

// crates/x3d-fields/tests/chrome_format.rs CASES (Chrome 155 Intl.NumberFormat), as
// (bits, maximumSignificantDigits, Chrome text).
const CHROME_INTL: &[(u64, usize, &str)] = &[
    (0x3fefffffffffffff, 15, "1"),
    (0x3fefffffffffffff, 16, "1"),
    (0x3feffffffffffffe, 15, "1"),
    (0x3feffffffffffffe, 16, "1"),
    (0x3feffffffffffffd, 15, "1"),
    (0x3feffffffffffffd, 16, "1"),
    (0x3feffffffffffffc, 15, "1"),
    (0x3feffffffffffffc, 16, "1"),
    (0x3feffffffffffffb, 15, "1"),
    (0x3feffffffffffffb, 16, "0.999999999999999"),
    (0x3feffffffffffffa, 16, "0.9999999999999993"),
    (0x3feffffffffffff9, 16, "0.9999999999999992"),
    (0x3feffffffffffff8, 16, "0.9999999999999991"),
    (0x3feffffffffffff7, 16, "0.999999999999999"),
    (0x3feffffffffffff6, 16, "0.9999999999999989"),
    (0x3feffffffffffff5, 16, "0.9999999999999988"),
    (0x3fc10cd84c924c23, 15, "0.133204495800414"),
    (0x3fc10cd84c924c23, 7, "0.1332045"),
    (0x3f1b0cca0f30aabb, 15, "0.000103187401507536"),
    (0x3f1b0cca0f30aabb, 7, "0.0001031874"),
    (0x3fb06cee8c6659d0, 15, "0.0641621678135842"),
    (0x3fb06cee8c6659d0, 7, "0.06416217"),
];

#[test]
fn intl_standard_engine_matches_chrome() {
    for (b, p, want) in CHROME_INTL {
        let v = f64::from_bits(*b);
        assert_eq!(x3d_fields::format::intl_standard(v, *p), *want, "{v:e} at {p}");
    }
}

/// Known difference 2, exact published values where they differ from Chrome.
const PUBLISHED_INTL_DIFFERENCES: &[(u64, usize, &str)] = &[
    (0x3fefffffffffffff, 16, "0.9999999999999999"),
    (0x3feffffffffffffe, 16, "0.9999999999999998"),
    (0x3feffffffffffffd, 16, "0.9999999999999997"),
    (0x3feffffffffffffc, 16, "0.9999999999999996"),
    (0x3feffffffffffffb, 15, "0.999999999999999"),
    (0x3feffffffffffffb, 16, "0.9999999999999994"),
];

#[test]
fn intl_standard_published_matches_chrome_except_known_difference_2() {
    for (b, p, want) in CHROME_INTL {
        let v = f64::from_bits(*b);
        let got = vrml97::fields::format::intl_standard(v, *p);
        match PUBLISHED_INTL_DIFFERENCES.iter().find(|(k, q, _)| k == b && q == p) {
            Some((_, _, published)) => {
                assert_eq!(got, *published, "{v:e} at {p}: published value");
                assert_ne!(got, *want, "known difference 2 closed for {v:e} at {p}; retire this record");
            }
            None => assert_eq!(got, *want, "{v:e} at {p}"),
        }
    }
}

// -------------------------------------------------------------------------
// FieldValue construction and printing
// -------------------------------------------------------------------------

fn vrml_text(v: &vrml97::fields::FieldValue) -> String {
    let mut g = vrml97::fields::Generator::new(vrml97::fields::Style::Tidy);
    v.to_vrml_stream(&mut g);
    g.string
}

#[test]
fn field_value_published_sfbool() {
    use vrml97::fields::{FieldType, FieldValue};
    let v = FieldValue::SFBool(true);
    assert_eq!(v.field_type(), FieldType::SFBool);
    assert!(v.equals(&FieldValue::SFBool(true)));
    assert!(!v.equals(&FieldValue::SFBool(false)));
    // ORACLE.json import.wrl prints `global TRUE`.
    assert_eq!(vrml_text(&v), "TRUE");
}

#[test]
fn field_value_published_sfstring() {
    use vrml97::fields::{FieldType, FieldValue};
    let v = FieldValue::SFString("hello".to_string());
    assert_eq!(v.field_type(), FieldType::SFString);
    // ORACLE.json bom.wrl prints `title "bom"`.
    assert_eq!(vrml_text(&v), "\"hello\"");
}

#[test]
fn field_value_published_sfvec3f() {
    use vrml97::fields::{FieldType, FieldValue, math::Vector3};
    let v = FieldValue::SFVec3f(Vector3 { x: 1.0, y: 2.0, z: 3.0 });
    assert_eq!(v.field_type(), FieldType::SFVec3f);
    assert!(!v.is_default_value());
    // ORACLE.json group-children.wrl prints `translation 1 2 3`.
    assert_eq!(vrml_text(&v), "1 2 3");
}

#[test]
fn field_value_engine_sfbool() {
    use x3d_fields::{FieldType, FieldValue};
    let v = FieldValue::SFBool(true);
    assert_eq!(v.field_type(), FieldType::SFBool);
    let mut g = x3d_fields::Generator::new(x3d_fields::Style::Tidy);
    v.to_vrml_stream(&mut g);
    assert_eq!(g.string, "TRUE");
}
