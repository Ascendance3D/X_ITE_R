//! REA batch 09 still images of the node-survey groups against pinned X_ITE: every world of
//! oracle/synthetic-rea-b09-visual (`oracle/tools/gen_rea_b09_visual.py`) with a baseline in
//! oracle/render-rea-b09-visual-baseline and oracle/render-msaa4-rea-b09-visual-baseline
//! (`oracle/tools/rea_b09_visual.py shots`, x_ite-shot.mjs / x_ite-shot-msaa.mjs, Multisampling 4).
//!
//! The bound is the one of `x3d-shot compare`: at most 0.5% of the pixels more than 8 off.
//! `REA_ONLY=<substring>` restricts the worlds.
//!
//! KNOWN_GAPS lists worlds the renderer cannot draw yet. Their references are kept, and the test requires
//! them to stay over the bound: when the feature lands the entry fails the test and must be removed.

use std::path::PathBuf;
use x3d_render::gpu::Gpu;

/// (world, why the renderer differs). The renderer reports ScreenGroup and ScreenFontStyle as unsupported
/// (`x3d-shot`: "not rendered: ScreenGroup"; text.rs: ScreenFontStyle), so their children and text are not drawn.
const KNOWN_GAPS: &[(&str, &str)] = &[("vis-screen.wrl", "ScreenGroup and ScreenFontStyle are not rendered")];

#[test]
fn rea_b09_visual_renders_match_x_ite() {
    let Ok(mut g) = Gpu::new() else { return eprintln!("no adapter") };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle").canonicalize().unwrap();
    let only = std::env::var("REA_ONLY").unwrap_or_default();
    let mut failures = vec![];
    let mut checked = 0;

    for (samples, baseline) in [(0u32, "render-rea-b09-visual-baseline"), (4, "render-msaa4-rea-b09-visual-baseline")] {
        g.renderer.set_sample_count(samples);
        g.renderer.set_gl_sample_orientation(samples > 1);

        if samples > 1 && g.renderer.sample_count() != samples {
            eprintln!("{samples}x not supported here");
            continue;
        }

        let mut names: Vec<String> = std::fs::read_dir(root.join(baseline))
            .unwrap()
            .filter_map(|e| e.ok()?.file_name().into_string().ok()?.strip_suffix(".png").map(str::to_string))
            .filter(|n| n.contains(&only))
            .collect();
        names.sort();

        for name in names {
            let (w, h, want) = x3d_render::compare::load_png(&root.join(format!("{baseline}/{name}.png"))).unwrap();
            let world = name.split('@').next().unwrap();
            let shot = x3d_render::render_file(&mut g, &root, &root.join(format!("synthetic-rea-b09-visual/{world}")), w, h).unwrap();
            let rgba: Vec<u8> = shot.rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
            let s = x3d_render::compare::compare(w, h, &want, &rgba, 8);

            eprintln!("{name} MSAA {samples}: {} pixels over 8 ({:.3}%), max {}", s.pixels_over, s.pct_over, s.max_delta);

            let over = s.pixels_over * 1000 > (w * h) as usize * 5;

            if let Some((_, why)) = KNOWN_GAPS.iter().find(|(gap, _)| name.starts_with(gap)) {
                if over {
                    eprintln!("  known gap: {why}");
                } else {
                    failures.push(format!("{name} MSAA {samples}: matches X_ITE now; remove it from KNOWN_GAPS ({why})"));
                }

                checked += 1;
                continue;
            }

            if over {
                failures.push(format!("{name} MSAA {samples}: {} pixels over 8 ({:.3}%), max {}", s.pixels_over, s.pct_over, s.max_delta));
            }

            checked += 1;
        }
    }

    eprintln!("{checked} shots checked");
    assert!(checked > 0, "no baselines found");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
