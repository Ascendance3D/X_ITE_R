//! Suite 3b — Chrome math flavor.
//!
//! `jsmath::set_flavor` is process-wide, so the Chrome flavor lives in its own test
//! binary. Every test here selects the Chrome flavor first and no test selects Node.
//!
//! Sources:
//! * `crates/x3d-fields/tests/chrome_math.rs` — Chrome 155 `Math.*` vectors recorded by
//!   the port where glibc differs (oracle/README.md, "Followers").
//! * `fixtures/js-oracle.json`, `fixtures/matrix-oracle.json` — headless Chromium 151.

use vrml97::fields::jsmath as pj;
use x3d_fields::jsmath as ej;

fn chrome() {
    pj::set_flavor(pj::MathFlavor::Chrome);
    ej::set_flavor(ej::MathFlavor::Chrome);
}

// (input bits, Chrome 155 result bits), from crates/x3d-fields/tests/chrome_math.rs.
const SIN: &[(u64, u64)] = &[
    (0xbfc4d644da65bda0, 0xbfc4bebd80c34c7b),
    (0xc00b18b162781378, 0x3fcf1ad94ab666a3),
    (0x400b03976beb5720, 0xbfcdd2edfb5f0b65),
    (0xc00e05b6fbe3c732, 0x3fe25cf82689c635),
    (0x400a0f052caa5114, 0xbfbd904e7e0da65f),
    (0xc00a38529536ac0c, 0x3fc157c1859ab0c5),
    (0xbfd5d5a5f1f14bf0, 0xbfd569d825235469),
    (0xc004e6e382b5e6b4, 0xbfe02539fd704e81),
];
const COS: &[(u64, u64)] = &[
    (0x3feb22eaf76f67b0, 0x3fe52acdc024418d),
    (0x3ff1ce01251878f4, 0x3fdc4c4d7e78b1e5),
    (0x3ff32921a973f558, 0x3fd756637149ff05),
    (0x3fefcbd013106548, 0x3fe175fafb8f0d95),
    (0xbffeb29b61d06a98, 0xbfd5d04d3011209b),
    (0xbfff312e5d66aa34, 0xbfd7a98a46e8f627),
    (0x4001cc7917628b0e, 0xbfe37804ff5a92dd),
    (0xbfe99fe047ff1b30, 0x3fe646eae0aed533),
];
const TAN: &[(u64, u64)] = &[
    (0x3fc691e7d2715320, 0x3fc6ce8a79d39c49),
    (0x3ff505a153b86bac, 0x400e73211f7a4d3f),
    (0xbfe016adbc016408, 0xbfe198cd3b51e153),
    (0xc0084b532498b29c, 0x3fbaee49c2144175),
    (0x3fbf4657f99eca80, 0x3fbf6e69f691d8dd),
    (0x40087dbf85a5cc16, 0xbfb492c54374c738),
    (0xbfe613ad768d9308, 0xbfea67cdf2f38a55),
    (0xbffb19e9b3961d90, 0x40202c90eb667c05),
];
const ATAN2: &[(u64, u64, u64)] = &[
    (0x3fb10471cbe5a458, 0x3fdc72159a31c72a, 0x3fc300bc52766b73),
    (0xbfc3a1527ea70264, 0x3fbc75ec022b32c0, 0xbfee317acfb4e653),
    (0xbfdd9797877b9076, 0x3fab844e658e80b0, 0xbff748040a362fc1),
    (0x3fd6f6b800d3b0e8, 0x3fdfd10920d00678, 0x3fe40174ddbb9b33),
    (0xbfca8f7160a7a50c, 0x3fd93a3f4f48c4ee, 0xbfdf02f72d642f7f),
    (0xbfb4b0a62f9b4d88, 0x3fdfab3c3e39c608, 0xbfc4b92ef8868213),
    (0xbfd051a106f1c530, 0x3fdff9ccb9b9da48, 0xbfde338261c48637),
    (0xbfdfd3a77b504bcc, 0x3fa7d97341f293c0, 0xbff7a36e2fd4aab3),
];

fn unary(table: &[(u64, u64)], f: fn(f64) -> f64) -> Vec<(u64, u64)> {
    table.iter().filter_map(|(x, want)| {
        let got = f(f64::from_bits(*x)).to_bits();
        (got != *want).then_some((*x, got))
    }).collect()
}

#[test]
fn chrome_math_engine_matches_chrome() {
    chrome();
    assert_eq!(unary(SIN, ej::sin), vec![]);
    assert_eq!(unary(COS, ej::cos), vec![]);
    assert_eq!(unary(TAN, ej::tan), vec![]);
    for (y, x, want) in ATAN2 {
        assert_eq!(ej::atan2(f64::from_bits(*y), f64::from_bits(*x)).to_bits(), *want);
    }
    // Chromium 151, fixtures/js-oracle.json.
    assert_eq!(ej::sin(0.5).to_bits(), 0x3fdeaee8744b05f0);
    assert_eq!(ej::cos(0.5).to_bits(), 0x3fec1528065b7d50);
    assert_eq!(ej::tan(0.5).to_bits(), 0x3fe17b4f5bf3474a);
    assert_eq!(ej::atan2(1.0, 1.0).to_bits(), 0x3fe921fb54442d18);
}

/// Known difference 3 (R0.2-A section 7.4): the published Chrome flavor calls the
/// platform libm (`f64::sin` and friends), the engine calls CORE-MATH. The vectors above
/// were chosen where glibc differs from Chrome, so the published flavor must miss some.
/// The exact miss set depends on the platform libm; this records that it is not empty and
/// that each published result is the platform result.
#[test]
fn chrome_math_published_differs_from_chrome_known_difference_3() {
    chrome();
    let misses = unary(SIN, pj::sin).len() + unary(COS, pj::cos).len() + unary(TAN, pj::tan).len();
    assert!(misses > 0, "known difference 3 closed: published Chrome-flavor math matches Chrome on every recorded vector; retire this record");
    for (x, _) in SIN {
        let x = f64::from_bits(*x);
        assert_eq!(pj::sin(x).to_bits(), x.sin().to_bits());
    }
    for (x, _) in COS {
        let x = f64::from_bits(*x);
        assert_eq!(pj::cos(x).to_bits(), x.cos().to_bits());
    }
    for (x, _) in TAN {
        let x = f64::from_bits(*x);
        assert_eq!(pj::tan(x).to_bits(), x.tan().to_bits());
    }
}

#[test]
fn chrome_matrix_factor_matches_xite_chromium() {
    use x3d_fields::math::{matrix::Matrix4, rotation::Rotation4, vector::Vector3};
    chrome();
    let mut a = Matrix4::IDENTITY;
    a.set_transform(
        Some(&Vector3::new(1.0, 2.0, 3.0)),
        Some(&Rotation4::new(0.0, 1.0, 0.0, 0.5)),
        Some(&Vector3::new(2.0, 3.0, 4.0)),
        None,
        None,
    );
    let (mut t, mut r, mut s, mut so) = (Vector3::ZERO, Rotation4::identity(), Vector3::ZERO, Rotation4::identity());
    a.factor(Some(&mut t), Some(&mut r), Some(&mut s), Some(&mut so));
    assert_eq!([t.x, t.y, t.z], [1.0, 2.0, 3.0]);
    assert_eq!([s.x, s.y, s.z], [2.0, 3.0, 4.0]);
    // fixtures/matrix-oracle.json, chrome.factor.r.
    assert_eq!([r.x().to_bits(), r.y().to_bits(), r.z().to_bits(), r.angle().to_bits()], [0, 0x3ff000000000000c, 0, 0x3fdfffffffffffe7]);
    assert_eq!([so.x(), so.y(), so.z(), so.angle()], [0.0, 0.0, 1.0, 0.0]);
}
