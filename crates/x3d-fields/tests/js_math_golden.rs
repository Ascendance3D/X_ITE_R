//! The Chrome-flavor `Math` functions of jsmath.rs equal CORE-MATH bit for bit. The expected
//! hashes are CORE-MATH's (the `core-math` crate 1.4.0, the C library jsmath.rs used before pxfm)
//! on the inputs of js_math/inputs.rs: 1,000,000 generated inputs per function plus the edges
//! (1,000,841 pairs for atan2). Pure Rust, so it runs on every platform.

#[path = "js_math/inputs.rs"]
mod inputs;

use inputs::{CASES, N, hash, inputs, pairs};
use x3d_fields::jsmath::{self, MathFlavor};

const EXPECTED: &[(&str, u64)] = &[
    ("sin", 0x1573298dcd5ed8b5),
    ("cos", 0x0e5dbbdba327bbe7),
    ("tan", 0x45388236f4518a92),
    ("asin", 0xcd60380902a4ede2),
    ("acos", 0x2d2da0989a7e696b),
    ("atan", 0x3a66164f0cbbdae9),
    ("exp", 0xb56ae71f1562fb8f),
    ("log", 0xa8feee75c55ca46d),
    ("log10", 0x559b032da2a6ae50),
    ("atan2", 0x6e7b02d2efdb2229),
];

fn expected(name: &str) -> u64 {
    EXPECTED.iter().find(|(n, _)| *n == name).unwrap().1
}

#[test]
fn chrome_math_equals_core_math() {
    jsmath::set_flavor(MathFlavor::Chrome);
    let mut failures = Vec::new();

    for &(name, seed, range, positive) in CASES {
        let f: fn(f64) -> f64 = match name {
            "sin" => jsmath::sin,
            "cos" => jsmath::cos,
            "tan" => jsmath::tan,
            "asin" => jsmath::asin,
            "acos" => jsmath::acos,
            "atan" => jsmath::atan,
            "exp" => jsmath::exp,
            "log" => jsmath::log,
            "log10" => jsmath::log10,
            _ => unreachable!(),
        };
        let v = inputs(seed, N, range, positive);
        let h = hash(v.iter().map(|&x| f(x)));

        if h != expected(name) {
            failures.push(format!(
                "{name}: 0x{h:016x}, expected 0x{:016x}",
                expected(name)
            ));
        }
    }

    let h = hash(pairs().iter().map(|&(y, x)| jsmath::atan2(y, x)));

    if h != expected("atan2") {
        failures.push(format!(
            "atan2: 0x{h:016x}, expected 0x{:016x}",
            expected("atan2")
        ));
    }

    // The cases pxfm alone gets wrong: the result underflows to zero.
    assert_eq!(
        jsmath::atan2(-5.9996406918275855e-270, 6.524587767809302e59).to_bits(),
        (-0.0f64).to_bits()
    );
    assert_eq!(
        jsmath::atan2(5.9996406918275855e-270, 6.524587767809302e59).to_bits(),
        0.0f64.to_bits()
    );
    assert_eq!(jsmath::atan2(-0.0, 1.0).to_bits(), (-0.0f64).to_bits());
    assert_eq!(jsmath::atan2(-0.0, 0.0).to_bits(), (-0.0f64).to_bits());
    assert_eq!(jsmath::atan2(-0.0, -0.0), -std::f64::consts::PI);

    // One test: the flavor is process-wide, and the Node flavor is the default.
    jsmath::set_flavor(MathFlavor::Node);
    assert!(failures.is_empty(), "{failures:#?}");
}
