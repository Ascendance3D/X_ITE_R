//! REA batch 09 still images of the node-survey groups against pinned X_ITE: every world of
//! oracle/synthetic-rea-b09-visual (`oracle/tools/gen_rea_b09_visual.py`, and the vis-items-* worlds of
//! `oracle/tools/gen_rea_b09_visual_items.py`, issue #90: HAnim, Espdu, CoordinateDouble/Normal/PointSet, Shape/Appearance, textures, MovieTexture;
//! recorded three times with identical pixels, see index-items.json beside the baselines) with a baseline in
//! oracle/render-rea-b09-visual-baseline and oracle/render-msaa4-rea-b09-visual-baseline
//! (`oracle/tools/rea_b09_visual.py shots`, x_ite-shot.mjs / x_ite-shot-msaa.mjs, Multisampling 4).
//!
//! The bound is the one of `x3d-shot compare`: at most 0.5% of the pixels more than 8 off.
//! `REA_ONLY=<substring>` restricts the worlds. A world of the corpus without a reference PNG fails the test.
//!
//! KNOWN_GAPS lists worlds the renderer cannot draw yet. Their references are kept, and the test requires
//! them to stay over the bound: when the feature lands the entry fails the test and must be removed.

use std::path::PathBuf;
use x3d_render::gpu::Gpu;

/// (world, why the renderer differs): worlds the renderer is known not to match. A world listed here must stay over the
/// bound; the test fails once it matches, so its entry is removed then. vis-items-alpha and vis-items-movie are the gaps the
/// item-level worlds of issue #90 left (gen_rea_b09_visual_items.py); vis-items-espdu and vis-items-shape-bbox matched once
/// EspduTransform drew its children and Shape culled by its declared bbox.
const KNOWN_GAPS: &[(&str, &str)] = &[
    (
        "vis-items-alpha.wrl",
        "Appearance alphaMode / alphaCutoff are not applied (Appearance.set_alphaMode__, set_transparent__): OPAQUE still blends the texture and material alpha, MASK neither cuts at alphaCutoff nor draws the kept pixels opaque",
    ),
    (
        "vis-items-movie.wrl",
        "MovieTexture frames are not decoded (GIF, APNG, WebM): the texture stays untextured white (x3d-shot: 'not rendered: MovieTexture')",
    ),
];

#[test]
fn rea_b09_visual_renders_match_x_ite() {
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

    for (samples, baseline) in [
        (0u32, "render-rea-b09-visual-baseline"),
        (4, "render-msaa4-rea-b09-visual-baseline"),
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

        // A world without its reference fails: an absent answer key never passes (issue #90).
        for e in std::fs::read_dir(root.join("synthetic-rea-b09-visual")).unwrap() {
            let world = e.unwrap().file_name().into_string().unwrap();

            if world.ends_with(".wrl")
                && world.contains(&only)
                && !names
                    .iter()
                    .any(|n| n.split('@').next() == Some(world.as_str()))
            {
                failures.push(format!(
                    "{world} MSAA {samples}: no reference in {baseline}"
                ));
            }
        }

        for name in names {
            let (w, h, want) =
                x3d_render::compare::load_png(&root.join(format!("{baseline}/{name}.png")))
                    .unwrap();
            let world = name.split('@').next().unwrap();
            let shot = x3d_render::render_file(
                &mut g,
                &root,
                &root.join(format!("synthetic-rea-b09-visual/{world}")),
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

            let over = s.pixels_over * 1000 > (w * h) as usize * 5;

            // X_ITE logged no warning for these worlds (the "log" of their index.json entries is empty), not even for
            // the invalid indexes of vis-linecolor-invalid: Rust must stay silent too and draw everything.
            if world.starts_with("vis-linecolor")
                || world.starts_with("vis-material-ambient")
                || world.starts_with("vis-colorrgba")
            {
                if !shot.log.is_empty() || !shot.unsupported.is_empty() {
                    failures.push(format!("{name} MSAA {samples}: X_ITE logged nothing, Rust logged {:?} / unsupported {:?}", shot.log, shot.unsupported));
                }
            }

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
                failures.push(format!(
                    "{name} MSAA {samples}: {} pixels over 8 ({:.3}%), max {}",
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
