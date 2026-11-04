//! Issue #69, depth modes: GaussianSplats under the reversed depth buffer (`DepthMode::Reversed`). Pinned X_ITE 16.5.0
//! (34cdf9f97) has no reversed depth, so there is no recording of it. `Reversed` keeps the near and far planes (see
//! gpu.rs), so the visible volume is the one of the standard buffer: the splats and the opaque shapes they are tested
//! against must give the pinned standard picture. The oracle is therefore the pinned standard recording of every world of
//! oracle/synthetic-rea-b08-gs (oracle/render-rea-b08-gs-baseline, MSAA 0, and oracle/render-msaa4-rea-b08-gs-baseline,
//! Multisampling 4) and of oracle/synthetic-b69-gs-shadow (oracle/render-b69-gs-shadow-baseline and
//! oracle/render-msaa4-b69-gs-shadow-baseline), so the shadow pass is covered too. `REA_ONLY=<substring>` restricts the worlds.
//!
//! The bound is the one of `x3d-shot compare`: at most 0.5% of the pixels more than 8 off.

use std::path::PathBuf;
use x3d_render::gpu::{DepthMode, Gpu};

#[test]
fn splats_in_reversed_depth_match_the_pinned_standard_picture() {
    let Ok(mut g) = Gpu::new() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap();
    let only = std::env::var("REA_ONLY").unwrap_or_default();
    let mut failures = vec![];
    let mut checked = 0;

    g.renderer.set_depth_mode(DepthMode::Reversed);

    for (samples, baseline, worlds) in [
        (0u32, "render-rea-b08-gs-baseline", "synthetic-rea-b08-gs"),
        (
            4,
            "render-msaa4-rea-b08-gs-baseline",
            "synthetic-rea-b08-gs",
        ),
        (
            0,
            "render-b69-gs-shadow-baseline",
            "synthetic-b69-gs-shadow",
        ),
        (
            4,
            "render-msaa4-b69-gs-shadow-baseline",
            "synthetic-b69-gs-shadow",
        ),
    ] {
        g.renderer.set_sample_count(samples);
        g.renderer.set_gl_sample_orientation(samples > 1);

        if samples > 1 && g.renderer.sample_count() != samples {
            eprintln!("{samples}x not supported here");
            continue;
        }

        let mut names: Vec<String> = std::fs::read_dir(root.join(baseline))
            .unwrap()
            .filter_map(|e| {
                e.ok()?
                    .file_name()
                    .into_string()
                    .ok()?
                    .strip_suffix(".png")
                    .map(str::to_string)
            })
            .filter(|n| n.contains(&only))
            .collect();

        names.sort();

        for name in &names {
            let (w, h, want) =
                x3d_render::compare::load_png(&root.join(format!("{baseline}/{name}.png")))
                    .unwrap();
            let world = name.split('@').next().unwrap();
            let shot = x3d_render::render_file(
                &mut g,
                &root,
                &root.join(format!("{worlds}/{world}")),
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
                "{worlds}/{name} MSAA {samples}: over {} ({:.3}%), max {}",
                s.pixels_over, s.pct_over, s.max_delta
            );
            checked += 1;

            if s.pixels_over * 1000 > (w * h) as usize * 5 {
                failures.push(format!(
                    "{worlds}/{name} MSAA {samples}: {} pixels over 8 ({:.3}%), max {}",
                    s.pixels_over, s.pct_over, s.max_delta
                ));
            }
        }
    }

    g.renderer.set_sample_count(0);
    g.renderer.set_gl_sample_orientation(false);
    g.renderer.set_depth_mode(DepthMode::Standard);
    eprintln!("{checked} shots checked");
    assert!(checked > 0);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
