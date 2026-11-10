//! sub:render.transparency-sorting: the `orderIndependentTransparency` option in the reference worlds of the transparency
//! pass of issue #92 (the `i92-tr-*` worlds and `i92-fg-transparent` of oracle/synthetic-rea-i92-render-items, which
//! render_rea_i92_items replays with the option off). The references are pinned X_ITE 16.5.0 with the option on, in
//! oracle/rea-oit-i92-render-items-baseline/png (antialiased="false") and /msaa4 (Multisampling 4), recorded by
//! `oracle/tools/rea_i92_render_items.py shots --oit-i92` with three identical runs per pass. A world whose multisampled
//! X_ITE output differs between its own runs has no msaa4 reference and is listed in msaa4/unstable.json.
//!
//! Bound: at most 0.5% of the pixels more than 8 off (1.0% for multisampled passes on macOS, as in render_rea_i92_items).
//! A world without a reference, or one whose X_ITE run logged an error, fails the test. `REA_ONLY=<substring>` restricts
//! the worlds.

use std::path::PathBuf;
use x3d_render::gpu::Gpu;

/// The worlds without an msaa4 reference (msaa4/unstable.json). The list must be exactly this one.
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
            if unstable.contains(world) {
                eprintln!("{world} {pass}: no reference (X_ITE is not reproducible here)");
                continue;
            }

            if !results.contains(world) {
                failures.push(format!("{world} {pass}: no result in {pass}/index.json"));
                continue;
            }

            let png = base.join(pass).join(format!("{world}.png"));

            if !png.exists() {
                failures.push(format!("{world} {pass}: no reference"));
                continue;
            }

            let (w, h, want) = x3d_render::compare::load_png(&png).unwrap();
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
