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
//!   two. Each number must agree within `CONVERTED_TOLERANCE` of the larger of the value and 1.
//!   The test prints how many are exact.

use serde_json::Value;

/// Relative to `max(|chrome|, 1)`. The measured maximum is 5.8e-5 (sRGB from linear sRGB, where
/// Chrome's transfer function is off from the exact one); every other pair stays below 5e-6.
const CONVERTED_TOLERANCE: f64 = 1e-4;

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
    let (mut exact, mut worst) = (0, 0.0f64);
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
        let mut all_exact = true;
        let mut ok = true;

        for i in 0..4 {
            let (e, g) = (expected[i] * scale(i), c[i] * scale(i));
            if e.is_nan() || g.is_nan() {
                ok &= e.is_nan() && g.is_nan();
                continue;
            }
            let error = (e - g).abs() / e.abs().max(1.0);
            all_exact &= same(expected[i], c[i], 1e-12 * expected[i].abs().max(1.0));
            worst = worst.max(error);
            ok &= error <= CONVERTED_TOLERANCE;
        }

        exact += usize::from(all_exact);
        if !ok {
            failures.push(format!(
                "{value}: Chrome gives {computed} = {expected:?}, Rust gives {c:?}"
            ));
        }
    }

    println!(
        "{} converted values, {exact} exact, worst relative difference {worst:.2e}",
        entries.len()
    );
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
