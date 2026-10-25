//! REA batch 01 render nodes (UnlitMaterial, TwoSidedMaterial and backMaterial, BlendMode,
//! DepthMode, ClipPlane, TextureProperties, FillProperties, PointProperties, LineProperties)
//! against pinned X_ITE: oracle/synthetic-rea-render2, baselines oracle/render-rea2-baseline
//! (`node oracle/x_ite-shot.mjs synthetic-rea-render2 …`) and oracle/render-msaa4-rea2-baseline
//! (`x_ite-shot-msaa.mjs`, Multisampling 4).
//!
//! The bound is the one of `x3d-shot compare`: at most 0.5% of the pixels more than 8 off. On the
//! reference machine the worst world is pointprops (13 pixels at MSAA 0).

use std::path::PathBuf;
use x3d_render::gpu::Gpu;

const WORLDS: [&str; 10] = [
    "blendmode",
    "clipplane",
    "depthmode",
    "fillprops",
    "lineprops",
    "pointprops",
    "textureprops",
    "textureprops-v33",
    "twosided",
    "unlit",
];

#[test]
fn render_nodes_match_x_ite() {
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
        (0u32, "render-rea2-baseline"),
        (4, "render-msaa4-rea2-baseline"),
    ] {
        g.renderer.set_sample_count(samples);
        g.renderer.set_gl_sample_orientation(samples > 1);

        if samples > 1 && g.renderer.sample_count() != samples {
            eprintln!("{samples}x not supported here");
            continue;
        }

        for name in WORLDS {
            let (w, h, want) =
                x3d_render::compare::load_png(&root.join(format!("{baseline}/{name}.wrl.png")))
                    .unwrap();
            let shot = x3d_render::render_file(
                &mut g,
                &root,
                &root.join(format!("synthetic-rea-render2/{name}.wrl")),
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

            if s.pixels_over * 1000 > (w * h) as usize * 5 {
                failures.push(format!(
                    "{name} MSAA {samples}: {} pixels over 8 ({:.3}%), max {}",
                    s.pixels_over, s.pct_over, s.max_delta
                ));
            }

            checked += 1;
        }
    }

    eprintln!("{checked} shots checked");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
