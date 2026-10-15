//! Suite 7b — Future gate, Chrome math flavor (separate binary: the flavor is process-wide).
//!
//! 12. (Added in C1, known difference 3.) The published Chrome flavor equals Chrome on the
//!     recorded vectors where glibc differs (crates/x3d-fields/tests/chrome_math.rs).
#![cfg(feature = "future_gate")]

#[test]
fn gate_chrome_math_matches_chrome() {
    use vrml97::fields::jsmath as m;
    m::set_flavor(m::MathFlavor::Chrome);
    let sin: &[(u64, u64)] = &[
        (0xbfc4d644da65bda0, 0xbfc4bebd80c34c7b),
        (0xc00b18b162781378, 0x3fcf1ad94ab666a3),
        (0x400b03976beb5720, 0xbfcdd2edfb5f0b65),
        (0xc00e05b6fbe3c732, 0x3fe25cf82689c635),
    ];
    let cos: &[(u64, u64)] = &[
        (0x3feb22eaf76f67b0, 0x3fe52acdc024418d),
        (0x3ff1ce01251878f4, 0x3fdc4c4d7e78b1e5),
        (0x3ff32921a973f558, 0x3fd756637149ff05),
        (0x3fefcbd013106548, 0x3fe175fafb8f0d95),
    ];
    let tan: &[(u64, u64)] = &[
        (0x3fc691e7d2715320, 0x3fc6ce8a79d39c49),
        (0x3ff505a153b86bac, 0x400e73211f7a4d3f),
        (0xbfe016adbc016408, 0xbfe198cd3b51e153),
        (0xc0084b532498b29c, 0x3fbaee49c2144175),
    ];
    for (table, f) in [(sin, m::sin as fn(f64) -> f64), (cos, m::cos), (tan, m::tan)] {
        for (x, want) in table {
            assert_eq!(f(f64::from_bits(*x)).to_bits(), *want, "{:e}", f64::from_bits(*x));
        }
    }
}
