//! CSS Color 5 relative color syntax (`lab(from red l a b)`) through `X3DParser.convertColor`
//! against headless Chrome.
//! Baseline: `oracle/css5-relative-colors-baseline/colors.json`, written by
//! `XITE_TESTS=<x_ite-tests> node oracle/x_ite-css5-relative-colors.mjs <baseline>`
//! (`CHROME=<binary>` records another Chrome).
//!
//! Chrome computes `rgb()`, `hsl()` and `hwb()` to `color(srgb ...)`; the other functions keep
//! their name. `convertColor` misreads either string like the other CSS Color 4 functions of
//! `rea_b24_css4_color.rs`: the first channel is NaN and the rest shift one place.
//!
//! Two lists, compared differently:
//!
//! * `entries`: syntax, keywords, `calc()`, `none`, alpha, clamping, nesting, wrong syntax, and
//!   colors read in the color space they were written in. Every number must match exactly, NaN
//!   included (the tolerance of `rea_b24_css4_color.rs`), and a value Chrome rejects must be
//!   rejected.
//! * `converted`: colors that pass through another color space (and polar to rectangular
//!   coordinates). Chrome computes those in 32-bit floats with its own transfer functions. The
//!   port computes them in 64-bit floats, so a number can differ in its last printed digit or
//!   two. Each number must agree within a tolerance of the larger of the value and 1: 5e-6
//!   (`CONVERTED_TOLERANCE`, half a unit of the sixth printed digit; the measured maximum over 900
//!   channels is 4.6e-6) and, only for the 15 values read from a `color(srgb-linear ...)` origin,
//!   1e-4 (`SRGB_LINEAR_ORIGIN_TOLERANCE`, measured maximum 5.8e-5). The test prints how many are
//!   exact and the worst difference of each group.

use serde_json::Value;

/// Relative to `max(|chrome|, 1)`. Chrome serializes a computed color with six significant digits
/// and the port prints the same way, so two correct results can differ by half a unit of the sixth
/// digit (5e-6). Measured over the 900 finite channels outside the group below: maximum 4.6e-6,
/// 13 channels above 2e-6, none above 5e-6.
const CONVERTED_TOLERANCE: f64 = 5e-6;

/// Only for a value whose origin is `color(srgb-linear ...)` (15 values of the 317). Chrome's
/// encode of linear sRGB light differs from the exact sRGB formula: for linear 0.3 Chrome prints
/// 0.583889 where the exact formula (and the port) give 0.583831, a constant 5.8e-5 that no
/// rounding explains, and the same offset reaches every color computed from that sRGB value. The
/// measured maximum is 5.8e-5 (45 channels); this tolerance is never used for another value.
const SRGB_LINEAR_ORIGIN_TOLERANCE: f64 = 1e-4;
const SRGB_LINEAR_ORIGIN_VALUES: usize = 15;

fn tolerance_for(value: &str) -> f64 {
    if value.contains("(from color(srgb-linear ") {
        SRGB_LINEAR_ORIGIN_TOLERANCE
    } else {
        CONVERTED_TOLERANCE
    }
}

fn baseline() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/css5-relative-colors-baseline/colors.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn number(v: &Value) -> f64 {
    match v {
        Value::String(s) if s == "NaN" => f64::NAN,
        other => other.as_f64().unwrap(),
    }
}

fn same(a: f64, b: f64, tolerance: f64) -> bool {
    (a.is_nan() && b.is_nan()) || (a - b).abs() <= tolerance
}

/// Comparison of one converted number (Chrome's `expected`, Rust's `got`, both on the same scale).
/// A relative tolerance alone would let a wrong sign, a wrong zero or a non-finite value pass, so:
/// * NaN must be NaN on both sides, never NaN against a number;
/// * an infinity must be the same infinity (the difference of two infinities is NaN, which a bare
///   `error <= tolerance` would also reject, but an infinity against a finite number is never close);
/// * below magnitude 1 the error is absolute, so an expected zero accepts at most `tolerance` and a
///   sign flip is accepted only when both numbers are within it;
/// * from magnitude 1 up the error is relative to the expected value.
fn converted_same(expected: f64, got: f64, tolerance: f64) -> bool {
    if expected.is_nan() || got.is_nan() {
        return expected.is_nan() && got.is_nan();
    }
    if expected.is_infinite() || got.is_infinite() {
        return expected == got;
    }
    (expected - got).abs() / expected.abs().max(1.0) <= tolerance
}

#[test]
fn relative_colors_match_chrome() {
    let baseline = baseline();
    let mut failures = vec![];
    let (mut accepted, mut rejected) = (0, 0);

    for entry in baseline["entries"].as_array().unwrap() {
        let value = entry[0].as_str().unwrap();
        let ours = x3d_parse::color::try_convert_color(value);

        if entry[1].is_null() {
            rejected += 1;
            if let Some(c) = ours {
                failures.push(format!("{value}: Chrome rejects it, Rust gives {c:?}"));
            }
            continue;
        }

        accepted += 1;
        let computed = entry[1].as_str().unwrap();
        let expected: Vec<f64> = entry[2].as_array().unwrap().iter().map(number).collect();
        let Some(c) = ours else {
            failures.push(format!("{value}: Chrome gives {computed}, Rust rejects it"));
            continue;
        };

        // Relative colors never compute to `rgb()`: every channel is a float in the string.
        assert!(!computed.starts_with("rgb"), "{value}: {computed}");

        if !(0..4).all(|i| same(expected[i], c[i], 1e-12 * expected[i].abs().max(1.0))) {
            failures.push(format!(
                "{value}: Chrome gives {computed} = {expected:?}, Rust gives {c:?}"
            ));
        }
    }

    println!(
        "{} values, {accepted} accepted, {rejected} rejected, {} failures",
        accepted + rejected,
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "{}",
        failures
            .iter()
            .take(if std::env::var("CSS5_ALL").is_ok() {
                usize::MAX
            } else {
                40
            })
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn converted_relative_colors_match_chrome_to_float_precision() {
    let baseline = baseline();
    let mut failures = vec![];
    let (mut exact, mut worst, mut worst_srgb_linear) = (0, 0.0f64, 0.0f64);
    let (mut srgb_linear_values, mut chrome_nan, mut rust_nan, mut nan_mismatch) = (0, 0, 0, 0);
    let entries = baseline["converted"].as_array().unwrap();

    for entry in entries {
        let value = entry[0].as_str().unwrap();
        let computed = entry[1]
            .as_str()
            .expect("Chrome accepts every converted value");
        let expected: Vec<f64> = entry[2].as_array().unwrap().iter().map(number).collect();
        let Some(c) = x3d_parse::color::try_convert_color(value) else {
            failures.push(format!("{value}: Chrome gives {computed}, Rust rejects it"));
            continue;
        };

        // The shifted channels are components over 255; the third one is the raw alpha slot.
        let scale = |i: usize| if i < 3 { 255.0 } else { 1.0 };
        let tolerance = tolerance_for(value);
        srgb_linear_values += usize::from(tolerance == SRGB_LINEAR_ORIGIN_TOLERANCE);
        let mut all_exact = true;
        let mut ok = true;

        for i in 0..4 {
            let (e, g) = (expected[i] * scale(i), c[i] * scale(i));
            chrome_nan += usize::from(e.is_nan());
            rust_nan += usize::from(g.is_nan());
            nan_mismatch += usize::from(e.is_nan() != g.is_nan());
            if e.is_nan() || g.is_nan() || e.is_infinite() || g.is_infinite() {
                ok &= converted_same(e, g, tolerance);
                all_exact &= e.to_bits() == g.to_bits() || (e.is_nan() && g.is_nan());
                continue;
            }
            let error = (e - g).abs() / e.abs().max(1.0);
            all_exact &= same(expected[i], c[i], 1e-12 * expected[i].abs().max(1.0));
            if tolerance == SRGB_LINEAR_ORIGIN_TOLERANCE {
                worst_srgb_linear = worst_srgb_linear.max(error);
            } else {
                worst = worst.max(error);
            }
            ok &= converted_same(e, g, tolerance);
        }

        exact += usize::from(all_exact);
        if !ok {
            failures.push(format!(
                "{value}: Chrome gives {computed} = {expected:?}, Rust gives {c:?}"
            ));
        }
    }

    println!(
        "{} converted values, {exact} exact; worst relative difference {worst:.2e} \
         ({} values), {worst_srgb_linear:.2e} ({srgb_linear_values} srgb-linear-origin values); \
         NaN channels: Chrome {chrome_nan}, Rust {rust_nan}, in different places {nan_mismatch}",
        entries.len(),
        entries.len() - srgb_linear_values
    );
    // The carve-out cannot grow silently, and NaN channels must agree in number and place.
    assert_eq!(srgb_linear_values, SRGB_LINEAR_ORIGIN_VALUES);
    assert_eq!((chrome_nan, nan_mismatch), (rust_nan, 0));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The baseline must keep exercising every function, both outcomes, the keywords and `calc()`, so
/// an emptied grid cannot pass.
#[test]
fn relative_baseline_covers_every_function() {
    let baseline = baseline();
    let entries = baseline["entries"].as_array().unwrap();
    let converted = baseline["converted"].as_array().unwrap();

    for name in [
        "rgb(", "hsl(", "hwb(", "lab(", "lch(", "oklab(", "oklch(", "color(",
    ] {
        let of_name = |list: &Vec<Value>, accepted: bool| {
            list.iter()
                .filter(|e| {
                    e[0].as_str()
                        .unwrap()
                        .to_ascii_lowercase()
                        .starts_with(name)
                })
                .filter(|e| e[1].is_null() != accepted)
                .count()
        };

        assert!(of_name(entries, true) >= 50, "{name} accepted values");
        assert!(of_name(entries, false) >= 5, "{name} rejected values");
        assert!(of_name(converted, true) >= 15, "{name} converted values");
    }

    let count = |needle: &str| {
        entries
            .iter()
            .filter(|e| e[0].as_str().unwrap().contains(needle) && !e[1].is_null())
            .count()
    };

    assert!(count("calc(") >= 200, "calc() values");
    assert!(count("none") >= 40, "none values");
    assert!(count("/ alpha") >= 50, "alpha keyword values");
    assert!(
        count("(from rgb(from") + count("(from lab(from") >= 2,
        "nested values"
    );
    assert!(converted.len() >= 300, "converted values");
}

/// The converted comparison must not hide errors at zero, near zero, below zero or at non-finite
/// values (owner condition on `CONVERTED_TOLERANCE`).
#[test]
fn converted_comparison_does_not_hide_edge_cases() {
    let t = CONVERTED_TOLERANCE;
    let converted_same = |expected: f64, got: f64| self::converted_same(expected, got, t);
    // Zero: only a difference within the tolerance passes, either sign.
    assert!(converted_same(0.0, 0.0));
    assert!(converted_same(0.0, 0.5 * t) && converted_same(0.0, -0.5 * t));
    assert!(!converted_same(0.0, 2.0 * t) && !converted_same(0.0, -2.0 * t));
    // Near zero (below 1 the error is absolute, so a relative error of 100% still fails above t).
    assert!(!converted_same(1e-3, 2e-3) && !converted_same(1e-3, -1e-3));
    assert!(converted_same(1e-3, 1e-3 + 0.5 * t));
    // Negative values: the sign counts.
    assert!(converted_same(-0.5, -0.5 + 0.5 * t));
    assert!(!converted_same(-0.5, 0.5) && !converted_same(0.5, -0.5));
    assert!(!converted_same(-2.0, 2.0));
    // From magnitude 1 up the error is relative: 100 may differ by 100 * t, not by more.
    assert!(converted_same(100.0, 100.0 * (1.0 + 0.5 * t)));
    assert!(!converted_same(100.0, 100.0 * (1.0 + 2.0 * t)));
    // Non-finite: NaN only against NaN, an infinity only against the same infinity.
    assert!(converted_same(f64::NAN, f64::NAN));
    assert!(!converted_same(f64::NAN, 0.0) && !converted_same(0.0, f64::NAN));
    assert!(converted_same(f64::INFINITY, f64::INFINITY));
    assert!(!converted_same(f64::INFINITY, f64::NEG_INFINITY));
    assert!(!converted_same(f64::INFINITY, f64::MAX) && !converted_same(1.0, f64::INFINITY));
    assert!(converted_same(f64::NEG_INFINITY, f64::NEG_INFINITY));
}
