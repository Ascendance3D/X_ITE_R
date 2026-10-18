//! REA batch 08 lane svg: SVG scenes (SVGParser) rendered and compared with pinned X_ITE. Every file of
//! oracle/synthetic-rea-svg-render with a baseline in oracle/render-rea-b08-svg-baseline
//! (`node oracle/x_ite-svg-shot.mjs`) and oracle/render-msaa4-rea-b08-svg-baseline
//! (`node oracle/x_ite-svg-shot-msaa.mjs`, Multisampling 4).
//!
//! The bound is the one of `x3d-shot compare`: at most 0.5% of the pixels more than 8 off.
//! `REA_ONLY=<substring>` restricts the worlds.

use std::path::PathBuf;
use x3d_render::gpu::Gpu;

#[test]
fn rea_b08_svg_renders_match_x_ite() {
    let Ok(mut g) = Gpu::new() else { return eprintln!("no adapter") };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle").canonicalize().unwrap();
    let only = std::env::var("REA_ONLY").unwrap_or_default();
    let mut failures = vec![];
    let mut checked = 0;

    for (samples, baseline) in [(0u32, "render-rea-b08-svg-baseline"), (4, "render-msaa4-rea-b08-svg-baseline")] {
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
            let shot = x3d_render::render_file(&mut g, &root, &root.join(format!("synthetic-rea-svg-render/{world}")), w, h).unwrap();
            let rgba: Vec<u8> = shot.rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
            let s = x3d_render::compare::compare(w, h, &want, &rgba, 8);

            eprintln!("MSAA {samples} {name}: over {} ({:.3}%), max {}", s.pixels_over, s.pct_over, s.max_delta);

            if s.pixels_over * 1000 > (w * h) as usize * 5 {
                failures.push(format!("{name} MSAA {samples}: {} pixels over 8 ({:.3}%), max {}", s.pixels_over, s.pct_over, s.max_delta));
            }

            checked += 1;
        }
    }

    eprintln!("{checked} shots checked");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
