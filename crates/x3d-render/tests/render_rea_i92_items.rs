//! Issue #92: still images of the render core (transparency sorting, lighting model, headlight, fog) against pinned
//! X_ITE 16.5.0. Every world of oracle/synthetic-rea-i92-render-items (`oracle/tools/gen_rea_i92_render_items.py`) has a
//! reference in oracle/rea-i92-render-items-baseline/png (antialiased="false") and /msaa4 (Multisampling 4), recorded by
//! `oracle/tools/rea_i92_render_items.py shots` with three identical runs per pass.
//!
//! A world whose multisampled X_ITE output differs between three runs has no msaa4 reference and is listed in
//! msaa4/unstable.json (the png pass still has one).
//!
//! The bound is the one of `x3d-shot compare`: at most 0.5% of the pixels more than 8 off. A world without a reference,
//! or a reference whose X_ITE run logged an error, fails the test (an absent answer never passes).
//! `REA_ONLY=<substring>` restricts the worlds.
//!
//! KNOWN_GAPS lists worlds the renderer cannot match yet; each must stay over the bound and fails once it matches.

use std::path::PathBuf;
use x3d_render::gpu::Gpu;

/// (world, why Rust differs).
const KNOWN_GAPS: &[(&str, &str)] = &[];

#[test]
fn rea_i92_render_items_match_x_ite() {
    let Ok(mut g) = Gpu::new() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap();
    let only = std::env::var("REA_ONLY").unwrap_or_default();
    let base = root.join("rea-i92-render-items-baseline");
    let mut failures = vec![];
    let mut checked = 0;
    let mut worlds: Vec<String> = std::fs::read_dir(root.join("synthetic-rea-i92-render-items"))
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".wrl") && n.contains(&only))
        .collect();

    worlds.sort();
    assert!(!worlds.is_empty(), "no worlds");

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

        for r in index["results"].as_array().unwrap() {
            let file = r["file"].as_str().unwrap_or("?");

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

        // Worlds whose X_ITE output differs between its own multisampled runs have no reference (unstable.json).
        let unstable: Vec<String> = std::fs::read_to_string(base.join(pass).join("unstable.json"))
            .ok()
            .map(|t| serde_json::from_str(&t).unwrap())
            .unwrap_or_default();

        for world in &worlds {
            if unstable.contains(world) {
                eprintln!("{world} {pass}: no reference (X_ITE is not reproducible here)");
                continue;
            }

            let png = base.join(pass).join(format!("{world}.png"));

            if !png.exists() {
                failures.push(format!(
                    "{world} {pass}: no reference in {}",
                    base.display()
                ));
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
            let over = s.pixels_over * 1000 > (w * h) as usize * 5;

            eprintln!(
                "{world} {pass}: {} pixels over 8 ({:.3}%), max {}",
                s.pixels_over, s.pct_over, s.max_delta
            );

            if let Some((_, why)) = KNOWN_GAPS.iter().find(|(gap, _)| world == gap) {
                if over {
                    eprintln!("  known gap: {why}");
                } else {
                    failures.push(format!(
                        "{world} {pass}: matches X_ITE now; remove it from KNOWN_GAPS ({why})"
                    ));
                }
            } else if over {
                failures.push(format!(
                    "{world} {pass}: {} pixels over 8 ({:.3}%), max {}",
                    s.pixels_over, s.pct_over, s.max_delta
                ));
            }

            checked += 1;
        }
    }

    eprintln!("{checked} shots checked");
    assert!(checked > 0, "no baselines found");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
