//! sub:render.transparency-sorting: the `orderIndependentTransparency` option in the reference worlds of the transparency
//! pass of issue #92 (the `i92-tr-*` worlds and `i92-fg-transparent` of oracle/synthetic-rea-i92-render-items, which
//! render_rea_i92_items replays with the option off). The references are pinned X_ITE 16.5.0 with the option on, in
//! oracle/rea-oit-i92-render-items-baseline/png (antialiased="false") and /msaa4 (Multisampling 4), recorded by
//! `oracle/tools/rea_i92_render_items.py shots --oit-i92` with three identical runs per pass. A world whose multisampled
//! X_ITE output differs between its own runs has no msaa4 reference and is listed in msaa4/unstable.json. Those worlds are
//! compared with /msaa4-envelope instead (`oracle/tools/rea_oit_msaa4_envelope.py`, eight runs): the lowest and the highest
//! value of every channel of every pixel over the runs, which are at most 2 levels apart. A pixel is measured from the
//! nearest value between the two, so the envelope does not widen the bound by more than the spread of X_ITE itself.
//!
//! Bound: at most 0.5% of the pixels more than 8 off (1.0% for multisampled passes on macOS, as in render_rea_i92_items).
//! A world without a reference, or one whose X_ITE run logged an error, fails the test. `REA_ONLY=<substring>` restricts
//! the worlds.

use std::path::PathBuf;
use x3d_render::gpu::Gpu;

/// The worlds without a single msaa4 reference (msaa4/unstable.json). The list must be exactly this one. Each has an envelope.
const EXPECTED_UNSTABLE: &[&str] = &["i92-tr-many.wrl", "i92-tr-nested.wrl"];

/// (world, why Rust differs).
const KNOWN_GAPS: &[(&str, &str)] = &[];

fn over_limit_permille(samples: u32) -> usize {
    if cfg!(target_os = "macos") && samples > 1 {
        10
    } else {
        5
    }
}

/// The pictures of a world whose multisampled X_ITE output differs between runs: the lowest and the highest value of every
/// channel over the recorded runs (RGBA, alpha 255).
struct Envelope {
    width: u32,
    height: u32,
    min: Vec<u8>,
    max: Vec<u8>,
}

/// The largest spread of a channel an envelope may have (the recorder's MAX_SPREAD), and the fewest runs.
const ENVELOPE_MAX_SPREAD: u8 = 2;
const ENVELOPE_MIN_RUNS: u64 = 5;

impl Envelope {
    /// For every channel the value of the envelope nearest to `rgba`'s.
    fn nearest(&self, rgba: &[u8]) -> Vec<u8> {
        rgba.iter()
            .zip(self.min.iter().zip(&self.max))
            .map(|(v, (lo, hi))| (*v).clamp(*lo, *hi))
            .collect()
    }
}

/// `<world>.min.png` and `<world>.max.png` of `dir`, checked: an OIT recording of enough runs without a logged error, and
/// pictures that agree with index.json and with each other (min <= max, no channel further apart than the limit).
fn load_envelope(dir: &std::path::Path, world: &str) -> Result<Envelope, String> {
    let index: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("index.json"))
            .map_err(|e| format!("no envelope index.json: {e}"))?,
    )
    .map_err(|e| format!("envelope index.json: {e}"))?;
    let entry = &index["worlds"][world];

    if index["oit"] != true || index["runs"].as_u64().unwrap_or(0) < ENVELOPE_MIN_RUNS {
        return Err("the envelope is not an OIT recording of at least 5 runs".into());
    }

    if entry["runs"].as_array().map(|r| r.len() as u64) != index["runs"].as_u64() {
        return Err("the envelope does not list every run of the world".into());
    }

    let (w, h, min) = x3d_render::compare::load_png(&dir.join(format!("{world}.min.png")))
        .map_err(|e| format!("envelope min: {e}"))?;
    let (w2, h2, max) = x3d_render::compare::load_png(&dir.join(format!("{world}.max.png")))
        .map_err(|e| format!("envelope max: {e}"))?;

    if (w, h) != (w2, h2) || min.len() != max.len() {
        return Err("the envelope pictures differ in size".into());
    }

    if min.iter().zip(&max).any(|(lo, hi)| lo > hi) {
        return Err("the envelope has a lowest value above the highest".into());
    }

    let spread = min
        .iter()
        .zip(&max)
        .map(|(lo, hi)| hi - lo)
        .max()
        .unwrap_or(0);
    let differing = min
        .chunks_exact(4)
        .zip(max.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count() as u64;

    if spread > ENVELOPE_MAX_SPREAD
        || Some(spread as u64) != entry["largestSpread"].as_u64()
        || Some(differing) != entry["differingPixels"].as_u64()
    {
        return Err(format!(
            "the envelope pictures ({differing} pixels differ, spread {spread}) do not agree with index.json or the limit"
        ));
    }

    Ok(Envelope {
        width: w,
        height: h,
        min,
        max,
    })
}

fn run(oit: bool) -> Option<(Vec<String>, Vec<String>, Vec<String>)> {
    let Ok(mut g) = Gpu::new() else {
        x3d_evidence::no_gpu("no adapter");
        return None;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap();
    let only = std::env::var("REA_ONLY").unwrap_or_default();
    let base = root.join("rea-oit-i92-render-items-baseline");
    let (mut failures, mut over, mut compared) = (vec![], vec![], vec![]);
    let mut worlds: Vec<String> = std::fs::read_dir(root.join("synthetic-rea-i92-render-items"))
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| {
            n.ends_with(".wrl")
                && (n.starts_with("i92-tr-") || n == "i92-fg-transparent.wrl")
                && n.contains(&only)
        })
        .collect();

    worlds.sort();
    assert!(!worlds.is_empty(), "no worlds");
    g.renderer.set_order_independent_transparency(oit);

    for (samples, pass) in [(0u32, "png"), (4, "msaa4")] {
        g.renderer.set_sample_count(samples);
        g.renderer.set_gl_sample_orientation(samples > 1);

        if samples > 1 && g.renderer.sample_count() != samples {
            failures.push(format!("{samples}x multisampling is not supported here"));
            continue;
        }

        let index: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(base.join(pass).join("index.json"))
                .unwrap_or_else(|e| panic!("{pass}/index.json: {e}")),
        )
        .unwrap();

        assert_eq!(
            index["oit"], true,
            "{pass}/index.json: not an OIT recording"
        );

        let mut results = vec![];

        for r in index["results"].as_array().unwrap() {
            let file = r["file"].as_str().unwrap_or("?");

            results.push(file.to_string());

            if !file.contains(&only) {
                continue;
            }

            if r["ok"] != true {
                failures.push(format!("{file} {pass}: the reference run failed"));
            }

            for l in r["log"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|l| l.as_str())
            {
                if l.to_lowercase().contains("error") {
                    failures.push(format!("{file} {pass}: the reference logged {l:?}"));
                }
            }
        }

        let unstable: Vec<String> = std::fs::read_to_string(base.join(pass).join("unstable.json"))
            .ok()
            .map(|t| serde_json::from_str(&t).unwrap())
            .unwrap_or_default();
        let mut expected: Vec<String> = if samples > 1 {
            EXPECTED_UNSTABLE.iter().map(|w| w.to_string()).collect()
        } else {
            vec![]
        };
        let mut listed = unstable.clone();

        expected.sort();
        listed.sort();

        if only.is_empty() && listed != expected {
            failures.push(format!(
                "{pass}/unstable.json is {listed:?}, expected {expected:?}"
            ));
        }

        for world in &worlds {
            let envelope = if unstable.contains(world) {
                match load_envelope(&base.join(format!("{pass}-envelope")), world) {
                    Ok(e) => Some(e),
                    Err(e) => {
                        failures.push(format!("{world} {pass}: {e}"));
                        continue;
                    }
                }
            } else {
                None
            };

            if envelope.is_none() && !results.contains(world) {
                failures.push(format!("{world} {pass}: no result in {pass}/index.json"));
                continue;
            }

            let png = base.join(pass).join(format!("{world}.png"));

            if envelope.is_none() && !png.exists() {
                failures.push(format!("{world} {pass}: no reference"));
                continue;
            }

            let (w, h, want) = match &envelope {
                Some(e) => (e.width, e.height, vec![]),
                None => x3d_render::compare::load_png(&png).unwrap(),
            };
            let shot = x3d_render::render_file(
                &mut g,
                &root,
                &root.join(format!("synthetic-rea-i92-render-items/{world}")),
                w,
                h,
            )
            .unwrap();
            let rgba: Vec<u8> = shot
                .rgba
                .chunks_exact(4)
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect();
            // With an envelope the reference of a channel is the value nearest to Rust's between X_ITE's lowest and highest.
            let want = match &envelope {
                Some(e) => e.nearest(&rgba),
                None => want,
            };
            let s = x3d_render::compare::compare(w, h, &want, &rgba, 8);

            eprintln!(
                "{world} {pass} oit={oit}: {} pixels over 8 ({:.3}%), max {}",
                s.pixels_over, s.pct_over, s.max_delta
            );
            compared.push(format!("{world} {pass}"));

            if s.pixels_over * 1000 > (w * h) as usize * over_limit_permille(samples) {
                over.push(format!(
                    "{world} {pass}: {} pixels over 8 ({:.3}%), max {}",
                    s.pixels_over, s.pct_over, s.max_delta
                ));
            }
        }
    }

    Some((failures, over, compared))
}

#[test]
fn rea_oit_i92_render_items_match_x_ite() {
    let Some((mut failures, over, compared)) = run(true) else {
        return;
    };

    for line in &over {
        let world = line.split(' ').next().unwrap();

        if let Some((_, why)) = KNOWN_GAPS.iter().find(|(gap, _)| world == *gap) {
            eprintln!("known gap: {line} ({why})");
        } else {
            failures.push(line.clone());
        }
    }

    for (gap, why) in KNOWN_GAPS {
        if !over.iter().any(|l| l.starts_with(gap)) {
            failures.push(format!(
                "{gap}: matches X_ITE now; remove it from KNOWN_GAPS ({why})"
            ));
        }
    }

    eprintln!("{} shots checked", compared.len());
    assert!(!compared.is_empty(), "no baselines found");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Negative control: the option off on the Rust side against the same OIT-on references. A shot that stays within the
/// bound cannot tell order independent transparency from sorted blending, so it proves nothing about OIT; those shots are
/// listed here by name, and the list must be exact (a new world that cannot tell the two apart, or a listed one that now
/// can, fails).
/// i92-fg-transparent: six solid transparent spheres side by side, so each pixel has at most one transparent layer, and
/// one layer blends the same with and without OIT (measured: 20 and 12 pixels over 8 with the option off). Every other
/// shot differs by 2.1% to 34% of the pixels with the option off.
const OPTION_OFF_WITHIN_BOUND: &[&str] =
    &["i92-fg-transparent.wrl png", "i92-fg-transparent.wrl msaa4"];

#[test]
fn rea_oit_i92_render_items_option_off_is_told_apart() {
    let Some((failures, over, compared)) = run(false) else {
        return;
    };

    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(!compared.is_empty(), "no baselines found");

    let within: Vec<&str> = compared
        .iter()
        .map(String::as_str)
        .filter(|c| !over.iter().any(|o| o.starts_with(c)))
        .collect();

    // Under REA_ONLY only the selected worlds run, so only their listed shots are expected.
    let expected: Vec<&str> = OPTION_OFF_WITHIN_BOUND
        .iter()
        .copied()
        .filter(|e| compared.iter().any(|c| c == e))
        .collect();

    assert_eq!(
        within, expected,
        "shots that match the OIT references with the option off"
    );
}
