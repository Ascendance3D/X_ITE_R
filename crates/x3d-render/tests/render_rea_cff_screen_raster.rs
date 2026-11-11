//! node:FontLibrary, sub:text.fonts: pictures of ScreenText with the authored CFF font (FontLibrary, justify, sizes, vertical
//! text, glyphs of lines only) and with the default TrueType font, against pinned X_ITE 16.5.0 in
//! oracle/rea-cff-screen-raster-baseline (antialiased="false"; oracle/tools/rea_cff_screen_raster.py, three identical runs).
//! The worlds are oracle/synthetic-rea-cff-screen-raster.
//!
//! Bound: at most 0.5% of the pixels more than 8 off. The picture shows the text box where the page put it, so this test
//! checks the place and the size of the canvas. The glyph edges themselves are compared on the canvas, far more strictly, in
//! crates/x3d-text/tests/rea_screen_canvas.rs.
//!
//! A world without a reference, or one whose X_ITE run failed or logged an error, fails the test.

use std::path::PathBuf;
use x3d_render::gpu::Gpu;

#[test]
fn rea_cff_screen_raster_pictures_match_x_ite() {
    let Ok(mut g) = Gpu::new() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap();
    let base = root.join("rea-cff-screen-raster-baseline");
    let index: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(base.join("index.json")).unwrap()).unwrap();
    let mut worlds: Vec<String> = std::fs::read_dir(root.join("synthetic-rea-cff-screen-raster"))
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".wrl"))
        .collect();
    let mut failures = vec![];

    worlds.sort();
    assert_eq!(worlds.len(), 6, "{worlds:?}");
    g.renderer.set_sample_count(0);

    for world in &worlds {
        let Some(result) = index["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["file"] == world.as_str())
        else {
            failures.push(format!("{world}: no result in index.json"));
            continue;
        };
        let errors = result["log"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|l| l.as_str())
            .any(|l| l.to_lowercase().contains("error"));

        if result["ok"] != true || errors {
            failures.push(format!(
                "{world}: the reference run failed or logged an error"
            ));
            continue;
        }

        let (w, h, want) =
            x3d_render::compare::load_png(&base.join(format!("{world}.png"))).unwrap();
        let shot = x3d_render::render_file(
            &mut g,
            &root,
            &root.join(format!("synthetic-rea-cff-screen-raster/{world}")),
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
        // The reference shows text: an empty picture would compare nothing.
        let lit = want.chunks_exact(4).filter(|p| p[0] > 128).count();

        eprintln!(
            "{world}: {} pixels over 8 ({:.3}%), max {}, {lit} text pixels",
            s.pixels_over, s.pct_over, s.max_delta
        );

        if lit < 200 {
            failures.push(format!("{world}: only {lit} text pixels in the reference"));
        }

        if s.pixels_over * 1000 > (w * h) as usize * 5 {
            failures.push(format!(
                "{world}: {} pixels over 8 ({:.3}%), max {}",
                s.pixels_over, s.pct_over, s.max_delta
            ));
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
