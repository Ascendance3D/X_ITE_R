//! DepthMode depthRange in the main color pass (issue #51, ledger node:DepthMode): a reversed range (near > far) against the
//! pinned X_ITE 16.5.0. WebGL raises INVALID_OPERATION from gl.depthRange (near, far) with near > far and keeps the range in
//! force, so the page draws a shape with a reversed range exactly as it draws one with the default range; only a valid range
//! (here 0 0.1) moves a shape in the depth buffer. The pointing pass has the same rule (pointing_depthmode.rs).
//!
//! oracle/synthetic-depthmode-reversed holds two newly authored worlds: `reversed` (pairs of a far red and a near green
//! rectangle, the far one with depthRange 0.9 0.1, 1 0 and the valid 0 0.1, each in both draw orders) and `control` (the same
//! pairs with the default range). Baselines: oracle/render-depthmode-reversed-baseline (`x_ite-shot.mjs`) and
//! oracle/render-msaa4-depthmode-reversed-baseline (`x_ite-shot-msaa.mjs`, Multisampling 4), each taken three times with
//! identical PNGs.
//!
//! The bound is the one of `x3d-shot compare`: at most 0.5% of the pixels more than 8 off. Needs a GPU; without an adapter the
//! test is skipped by name through x3d_evidence.

use std::path::PathBuf;
use x3d_render::gpu::Gpu;

const WORLDS: [&str; 2] = ["control", "reversed"];

#[test]
fn reversed_depth_range_matches_x_ite() {
    let Ok(mut g) = Gpu::new() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap();
    let mut failures = vec![];
    let mut checked = 0;

    for (samples, baseline) in [
        (0u32, "render-depthmode-reversed-baseline"),
        (4, "render-msaa4-depthmode-reversed-baseline"),
    ] {
        g.renderer.set_sample_count(samples);
        g.renderer.set_gl_sample_orientation(samples > 1);

        if samples > 1 && g.renderer.sample_count() != samples {
            eprintln!("{samples}x not supported here");
            continue;
        }

        let mut pinned = vec![];

        for name in WORLDS {
            let (w, h, want) =
                x3d_render::compare::load_png(&root.join(format!("{baseline}/{name}.wrl.png")))
                    .unwrap();
            let shot = x3d_render::render_file(
                &mut g,
                &root,
                &root.join(format!("synthetic-depthmode-reversed/{name}.wrl")),
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
                "{name} MSAA {samples}: {} pixels over 8 ({:.3}%), max {}",
                s.pixels_over, s.pct_over, s.max_delta
            );

            if s.pixels_over * 1000 > (w * h) as usize * 5 {
                failures.push(format!(
                    "{name} MSAA {samples}: {} pixels over 8 ({:.3}%), max {}",
                    s.pixels_over, s.pct_over, s.max_delta
                ));
            }

            pinned.push((w, h, want));
            checked += 1;
        }

        // The pinned pictures themselves: the two reversed ranges leave their pairs as the control draws them and the valid
        // range changes its pair, so the world cannot pass by drawing the same picture everywhere.
        let (w, h, control) = &pinned[0];
        let (_, _, reversed) = &pinned[1];
        // The pairs occupy x 40..174, 204..338 and 369..503 of the 512 wide picture.
        let (c1, c2) = (189, 354);
        let differs = |x0: usize, x1: usize| {
            (0..*h as usize)
                .flat_map(|y| (x0..x1).map(move |x| (y * *w as usize + x) * 4))
                .filter(|&i| control[i..i + 3] != reversed[i..i + 3])
                .count()
        };

        assert_eq!(differs(0, c1), 0, "pinned pair 0 (0.9 0.1) is the control");
        assert_eq!(differs(c1, c2), 0, "pinned pair 1 (1 0) is the control");
        assert!(
            differs(c2, *w as usize) > 1000,
            "pinned pair 2 (0 0.1) differs from the control"
        );
    }

    eprintln!("{checked} shots checked");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
