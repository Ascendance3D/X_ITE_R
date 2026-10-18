//! REA batch 08: the CSS color conversion `X3DParser.convertColor` leaves to the browser, against
//! the colors headless Chrome computes (`getComputedStyle (div) .color`) for a grid of values.
//! Baseline: `oracle/synthetic-rea-svg/css-colors-baseline.json`, written by
//! `node oracle/x_ite-css-colors.mjs oracle/synthetic-rea-svg/css-colors-baseline.json`.
//!
//! A value Chrome rejects (`null`) must be rejected by `try_convert_color` too. Functions the
//! port does not know (hwb, lab, oklch, color()) are listed, not compared.

fn channels(computed: &str) -> Option<[f64; 4]> {
    let inner = computed.strip_prefix("rgba(").or_else(|| computed.strip_prefix("rgb("))?.strip_suffix(')')?;
    let parts: Vec<f64> = inner.split(',').map(|s| s.trim().parse().unwrap()).collect();

    Some([parts[0] / 255.0, parts[1] / 255.0, parts[2] / 255.0, *parts.get(3).unwrap_or(&1.0)])
}

#[test]
fn css_colors_match_chrome() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-svg/css-colors-baseline.json");
    let baseline: Vec<(String, Option<String>)> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut failures = vec![];
    let mut unknown = vec![];

    for (value, computed) in &baseline {
        let ours = x3d_parse::color::try_convert_color(value);

        match (computed, ours) {
            (None, None) => {}
            (None, Some(c)) => failures.push(format!("{value}: Chrome rejects it, Rust gives {c:?}")),
            (Some(expected), None) => {
                if value.starts_with("hwb") || value.starts_with("lab") || value.starts_with("oklch") || value.starts_with("color(") || value == "currentcolor" || value == "inherit" {
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
                let bytes = |c: [f64; 4]| [(c[0] * 255.0).round(), (c[1] * 255.0).round(), (c[2] * 255.0).round()];

                if bytes(e) != bytes(c) || (e[3] - c[3]).abs() > 0.005 {
                    failures.push(format!("{value}: Chrome gives {expected}, Rust gives {:?}", bytes(c)));
                }
            }
        }
    }

    // Known limit, measured: 43 `hsl ()` values of the 16,200 in the grid differ by one level in
    // one channel. They are the values where the interpolated channel is exactly x.5 in real
    // arithmetic and Blink's 32-bit evaluation lands just below it; the port, evaluating in
    // 64-bit and scaling in 32-bit, lands on it. Every other tie (pure t1 / t2 channels) matches.
    const KNOWN_HSL_TIES: usize = 43;

    let (hsl, other): (Vec<_>, Vec<_>) = failures.iter().partition(|f| f.starts_with("hsl("));

    for f in hsl.iter().filter(|f| !f.contains(",") || f.contains("turn") || f.contains("deg")) {
        println!("{f}");
    }

    println!("{} values, {} hsl ties, {} other failures, {} not compared: {:?}", baseline.len(), hsl.len(), other.len(), unknown.len(), unknown);
    assert!(other.is_empty(), "{}", other.iter().take(30).map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    assert_eq!(hsl.len(), KNOWN_HSL_TIES, "the hsl () tie set changed:\n{}", hsl.iter().take(60).map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
}
