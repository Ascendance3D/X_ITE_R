//! REA batch 08: the CSS color conversion `X3DParser.convertColor` leaves to the browser, against
//! the colors headless Chrome computes (`getComputedStyle (div) .color`) for a grid of values.
//! Baseline: `oracle/synthetic-rea-svg/css-colors-baseline.json`, written by
//! `node oracle/x_ite-css-colors.mjs oracle/synthetic-rea-svg/css-colors-baseline.json`.
//!
//! A value Chrome rejects (`null`) must be rejected by `try_convert_color` too. Functions the
//! port does not know (hwb, lab, oklch, color()) are listed, not compared.

fn channels(computed: &str) -> Option<[f64; 4]> {
    let inner = computed
        .strip_prefix("rgba(")
        .or_else(|| computed.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let parts: Vec<f64> = inner
        .split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();

    Some([
        parts[0] / 255.0,
        parts[1] / 255.0,
        parts[2] / 255.0,
        *parts.get(3).unwrap_or(&1.0),
    ])
}

/// The 43 `hsl ()` values of the 16,200 in the grid that differ from Chrome by one level in one
/// channel. Owner decision (#55): a tolerance of 1/255 applies to these values only.
///
/// Why: each is a value where the interpolated channel is exactly x.5 in real arithmetic. Blink
/// evaluates `hsl ()` in 32-bit float and lands just below x.5, so it rounds down; the port
/// evaluates in 64-bit and lands on x.5, so it rounds up. An exact match would need Blink's
/// 32-bit evaluation order reproduced bit for bit. Every other tie (pure t1 / t2 channels) matches.
const KNOWN_HSL_TIES: &[&str] = &[
    "hsl(10,80%,50%)",
    "hsl(10,100%,50%)",
    "hsl(14,50%,50%)",
    "hsl(14,100%,50%)",
    "hsl(25,80%,50%)",
    "hsl(42,100%,50%)",
    "hsl(46,100%,50%)",
    "hsl(125,50%,40%)",
    "hsl(125,50%,60%)",
    "hsl(125,80%,50%)",
    "hsl(140,80%,50%)",
    "hsl(140,100%,25%)",
    "hsl(155,50%,40%)",
    "hsl(155,80%,50%)",
    "hsl(162,100%,50%)",
    "hsl(166,100%,50%)",
    "hsl(170,80%,50%)",
    "hsl(170,100%,50%)",
    "hsl(190,80%,50%)",
    "hsl(190,100%,50%)",
    "hsl(194,100%,50%)",
    "hsl(198,100%,50%)",
    "hsl(205,50%,40%)",
    "hsl(205,80%,50%)",
    "hsl(220,80%,50%)",
    "hsl(220,100%,25%)",
    "hsl(235,50%,40%)",
    "hsl(235,50%,60%)",
    "hsl(235,80%,50%)",
    "hsl(250,80%,50%)",
    "hsl(250,100%,50%)",
    "hsl(254,50%,50%)",
    "hsl(254,100%,50%)",
    "hsl(265,80%,50%)",
    "hsl(282,100%,50%)",
    "hsl(284,100%,25%)",
    "hsl(316,100%,25%)",
    "hsl(318,100%,50%)",
    "hsl(335,80%,50%)",
    "hsl(346,50%,50%)",
    "hsl(346,100%,50%)",
    "hsl(350,80%,50%)",
    "hsl(350,100%,50%)",
];

#[test]
fn css_colors_match_chrome() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/synthetic-rea-svg/css-colors-baseline.json");
    let baseline: Vec<(String, Option<String>)> =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut failures = vec![];
    let mut unknown = vec![];

    for (value, computed) in &baseline {
        let ours = x3d_parse::color::try_convert_color(value);

        match (computed, ours) {
            (None, None) => {}
            (None, Some(c)) => {
                failures.push(format!("{value}: Chrome rejects it, Rust gives {c:?}"))
            }
            (Some(expected), None) => {
                if value.starts_with("hwb")
                    || value.starts_with("lab")
                    || value.starts_with("oklch")
                    || value.starts_with("color(")
                    || value == "currentcolor"
                    || value == "inherit"
                {
                    unknown.push(value.clone());
                } else {
                    failures.push(format!("{value}: Chrome gives {expected}, Rust rejects it"));
                }
            }
            (Some(expected), Some(c)) => {
                let Some(e) = channels(expected) else {
                    unknown.push(value.clone());
                    continue;
                };

                // Compared as 8-bit channels; alpha to the 8-bit step Chrome keeps.
                let bytes = |c: [f64; 4]| {
                    [
                        (c[0] * 255.0).round(),
                        (c[1] * 255.0).round(),
                        (c[2] * 255.0).round(),
                    ]
                };

                // Owner decision (#55): the 43 values in `KNOWN_HSL_TIES` may differ by one 8-bit
                // level per channel; every other value must match exactly.
                let tolerance = if KNOWN_HSL_TIES.contains(&value.as_str()) {
                    1.0
                } else {
                    0.0
                };
                let (eb, cb) = (bytes(e), bytes(c));

                if (0..3).any(|i| (eb[i] - cb[i]).abs() > tolerance) || (e[3] - c[3]).abs() > 0.005
                {
                    failures.push(format!(
                        "{value}: Chrome gives {expected}, Rust gives {:?}",
                        bytes(c)
                    ));
                }
            }
        }
    }

    let other: Vec<_> = failures.iter().collect();

    println!(
        "{} values, {} other failures, {} not compared: {:?}",
        baseline.len(),
        other.len(),
        unknown.len(),
        unknown
    );
    assert!(
        other.is_empty(),
        "{}",
        other
            .iter()
            .take(30)
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Every value in the list must really be in the baseline, so a rename cannot hide a value.
#[test]
fn known_hsl_ties_are_in_the_baseline() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/synthetic-rea-svg/css-colors-baseline.json");
    let baseline: Vec<(String, Option<String>)> =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();

    assert_eq!(KNOWN_HSL_TIES.len(), 43);
    for tie in KNOWN_HSL_TIES {
        assert!(
            baseline.iter().any(|(v, _)| v == tie),
            "{tie} not in the baseline"
        );
    }
}
