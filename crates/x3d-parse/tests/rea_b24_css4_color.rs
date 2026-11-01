//! REA batch 24: CSS Color 4 functions (`hwb`, `lab`, `lch`, `oklab`, `oklch`, `color`) through
//! `X3DParser.convertColor` against headless Chrome.
//! Baseline: `oracle/synthetic-rea-b24-css4/css4-colors-baseline.json`, written by
//! `XITE_TESTS=<x_ite-tests> node oracle/x_ite-css4-colors.mjs <baseline>`.
//!
//! `hwb()` computes to `rgb()`, so it must give the same 8-bit channels. The other five compute to
//! `lab(...)` style strings, which `convertColor` misreads: the first channel is NaN and the rest
//! are shifted one place (see `unparsed_function`). Those must match number for number, NaN
//! included. A value Chrome rejects must be rejected too.

use serde_json::Value;

/// The 85 `hwb ()` values of the grid whose result differs from Chrome by one 8-bit level in
/// one channel. Same class as the `hsl ()` ties of `css_colors_b08` (owner decision, #55): each is
/// a value where a channel is exactly x.5 in real arithmetic, Blink's 32-bit evaluation lands on
/// one side of it, and the port's lands on the other. `known_hwb_ties_are_exact_ties` proves every
/// entry really is such a tie, so the list cannot hide a real error. Every other value must match
/// exactly. Not covered: relative colors (`lab(from red l a b)`), which Chrome accepts and the port
/// does not parse.
const KNOWN_HWB_TIES: &[&str] = &[
    "hwb(10 0% 0%)",
    "hwb(10 10% 10%)",
    "hwb(12 20% 30%)",
    "hwb(14 0% 0%)",
    "hwb(14 25% 25%)",
    "hwb(14 5% 45%)",
    "hwb(25 10% 10%)",
    "hwb(42 0% 0%)",
    "hwb(46 0% 0%)",
    "hwb(46 5% 45%)",
    "hwb(65 10% 10%)",
    "hwb(80 10% 10%)",
    "hwb(82 0% 0%)",
    "hwb(82 25% 25%)",
    "hwb(82 5% 45%)",
    "hwb(84 20% 30%)",
    "hwb(86 0% 0%)",
    "hwb(95 10% 10%)",
    "hwb(102 0% 0%)",
    "hwb(110 0% 0%)",
    "hwb(110 10% 10%)",
    "hwb(114 0% 0%)",
    "hwb(114 25% 25%)",
    "hwb(114 5% 45%)",
    "hwb(116 20% 30%)",
    "hwb(118 0% 0%)",
    "hwb(122 0% 0%)",
    "hwb(124 20% 30%)",
    "hwb(126 0% 0%)",
    "hwb(126 25% 25%)",
    "hwb(126 5% 45%)",
    "hwb(132 20% 30%)",
    "hwb(138 0% 0%)",
    "hwb(142 0% 0%)",
    "hwb(142 25% 25%)",
    "hwb(142 5% 45%)",
    "hwb(154 0% 0%)",
    "hwb(156 20% 30%)",
    "hwb(162 0% 0%)",
    "hwb(166 0% 0%)",
    "hwb(166 5% 45%)",
    "hwb(172 20% 30%)",
    "hwb(188 20% 30%)",
    "hwb(194 0% 0%)",
    "hwb(194 5% 45%)",
    "hwb(198 0% 0%)",
    "hwb(204 20% 30%)",
    "hwb(206 0% 0%)",
    "hwb(218 0% 0%)",
    "hwb(218 25% 25%)",
    "hwb(218 5% 45%)",
    "hwb(222 0% 0%)",
    "hwb(228 20% 30%)",
    "hwb(234 0% 0%)",
    "hwb(234 25% 25%)",
    "hwb(234 5% 45%)",
    "hwb(236 20% 30%)",
    "hwb(238 0% 0%)",
    "hwb(242 0% 0%)",
    "hwb(244 20% 30%)",
    "hwb(246 0% 0%)",
    "hwb(246 25% 25%)",
    "hwb(246 5% 45%)",
    "hwb(274 0% 0%)",
    "hwb(276 20% 30%)",
    "hwb(278 0% 0%)",
    "hwb(278 25% 25%)",
    "hwb(278 5% 45%)",
    "hwb(280 10% 10%)",
    "hwb(284 20% 30%)",
    "hwb(295 10% 10%)",
    "hwb(305 10% 10%)",
    "hwb(316 20% 30%)",
    "hwb(320 10% 10%)",
    "hwb(322 0% 0%)",
    "hwb(322 25% 25%)",
    "hwb(322 5% 45%)",
    "hwb(324 20% 30%)",
    "hwb(326 0% 0%)",
    "hwb(354 0% 0%)",
    "hwb(354 25% 25%)",
    "hwb(354 5% 45%)",
    "hwb(356 20% 30%)",
    "hwb(358 0% 0%)",
    "hwb(120 150% 30%)",
];

fn baseline() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/synthetic-rea-b24-css4/css4-colors-baseline.json");
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
fn css4_colors_match_chrome() {
    let baseline = baseline();
    let mut failures = vec![];
    let mut checked = 0;
    let mut nan_forms = 0;

    for entry in baseline["entries"].as_array().unwrap() {
        let value = entry[0].as_str().unwrap();

        // Relative color syntax is not implemented (see `KNOWN_HWB_TIES`).
        if value.contains("(from ") {
            continue;
        }

        let ours = x3d_parse::color::try_convert_color(value);

        if entry[1].is_null() {
            if let Some(c) = ours {
                failures.push(format!("{value}: Chrome rejects it, Rust gives {c:?}"));
            }
            continue;
        }

        let computed = entry[1].as_str().unwrap();
        let expected: Vec<f64> = entry[2].as_array().unwrap().iter().map(number).collect();
        let Some(c) = ours else {
            failures.push(format!("{value}: Chrome gives {computed}, Rust rejects it"));
            continue;
        };

        checked += 1;

        let is_rgb = computed.starts_with("rgb");
        nan_forms += usize::from(!is_rgb);

        // `rgb()` results are 8-bit channels over 255; alpha is the 8-bit step Chrome keeps.
        // Everything else is exact, NaN included.
        let level = if KNOWN_HWB_TIES.contains(&value) {
            1.0
        } else {
            0.0
        };
        let ok = if is_rgb {
            (0..3).all(|i| same((expected[i] * 255.0).round(), (c[i] * 255.0).round(), level))
                && same(expected[3], c[3], 0.005)
        } else {
            (0..4).all(|i| same(expected[i], c[i], 1e-12 * expected[i].abs().max(1.0)))
        };

        if !ok {
            failures.push(format!(
                "{value}: Chrome gives {computed} = {expected:?}, Rust gives {c:?}"
            ));
        }
    }

    for name in ["hwb(", "lab(", "lch(", "oklab(", "oklch(", "color("] {
        let n = failures
            .iter()
            .filter(|f| f.to_ascii_lowercase().starts_with(name))
            .count();
        println!("{name} failures: {n}");
    }

    println!(
        "{} values, {} compared, {} through the NaN path, {} failures",
        baseline["entries"].as_array().unwrap().len(),
        checked,
        nan_forms,
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "{}",
        failures
            .iter()
            .take(if std::env::var("CSS4_ALL").is_ok() {
                usize::MAX
            } else {
                40
            })
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The baseline must keep exercising every function and both outcomes, so an emptied grid cannot
/// pass.
#[test]
fn css4_baseline_covers_every_function() {
    let baseline = baseline();
    let entries = baseline["entries"].as_array().unwrap();

    for name in ["hwb(", "lab(", "lch(", "oklab(", "oklch(", "color("] {
        let accepted = entries
            .iter()
            .filter(|e| {
                e[0].as_str()
                    .unwrap()
                    .to_ascii_lowercase()
                    .starts_with(name)
            })
            .filter(|e| !e[1].is_null())
            .count();
        let rejected = entries
            .iter()
            .filter(|e| {
                e[0].as_str()
                    .unwrap()
                    .to_ascii_lowercase()
                    .starts_with(name)
            })
            .filter(|e| e[1].is_null())
            .count();

        assert!(accepted >= 20, "{name} accepted values: {accepted}");
        assert!(rejected >= 2, "{name} rejected values: {rejected}");
    }
}

/// Every tie in the list must be in the baseline and must be an exact x.5 in real arithmetic for
/// at least one channel, in integers (hue in sixths, whiteness and blackness in percent).
#[test]
fn known_hwb_ties_are_exact_ties() {
    let baseline = baseline();
    let entries = baseline["entries"].as_array().unwrap();

    for tie in KNOWN_HWB_TIES {
        assert!(
            entries.iter().any(|e| e[0] == *tie),
            "{tie} not in the baseline"
        );

        let n: Vec<i64> = tie
            .split(|c: char| !c.is_ascii_digit())
            .filter_map(|p| p.parse().ok())
            .collect();
        let (h, w, b) = (n[0], n[1], n[2]);

        // channel * 255 = numerator / denominator; a tie has a fractional part of exactly 1/2.
        let is_tie =
            |numerator: i64, denominator: i64| 2 * (numerator % denominator) == denominator;

        let exact = if w + b >= 100 {
            vec![is_tie(255 * w, w + b)]
        } else {
            // The pure hue in sixths of a unit times 60 (an integer for whole degrees).
            let pure = |offset: i64| {
                let hv = (h + offset * 60).rem_euclid(360);
                match hv {
                    0..=59 => hv,
                    60..=179 => 60,
                    180..=239 => 240 - hv,
                    _ => 0,
                }
            };

            [pure(2), pure(0), pure(-2)]
                .iter()
                .map(|f| is_tie(255 * (f * (100 - w - b) + 60 * w), 6000))
                .collect()
        };

        assert!(exact.iter().any(|t| *t), "{tie} is not an exact tie");
    }
}
