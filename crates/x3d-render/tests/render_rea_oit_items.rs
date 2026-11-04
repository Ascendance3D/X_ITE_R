//! sub:render.transparency-sorting: the `orderIndependentTransparency` browser option against pinned X_ITE 16.5.0.
//! Every world of oracle/synthetic-rea-oit-render-items (`oracle/tools/gen_rea_oit_render_items.py`) has a reference in
//! oracle/rea-oit-render-items-baseline/png (antialiased="false") and /msaa4 (Multisampling 4), recorded by
//! `oracle/tools/rea_i92_render_items.py shots --oit` (the option set with `setBrowserOption`, `x_ite-shot*.mjs --oit`) with
//! three identical runs per pass.
//!
//! A world whose multisampled X_ITE output differs between three runs has no msaa4 reference and is listed in
//! msaa4/unstable.json (the png pass still has one).
//!
//! The bound is the one of `x3d-shot compare` and of render_rea_i92_items: at most 0.5% of the pixels more than 8 off. A
//! world without a reference, or a reference whose X_ITE run logged an error, fails the test (an absent answer never passes).
//! `REA_ONLY=<substring>` restricts the worlds.
//!
//! Negative control: with the option off the renderer sorts the transparent shapes and blends them in order, which is not
//! the weighted blend of the pinned picture, so the same worlds must be over the bound.

use std::path::PathBuf;
use x3d_render::gpu::Gpu;

/// The worlds whose X_ITE output differs between its own multisampled runs (msaa4/unstable.json): intersecting and
/// layered transparency sums in RGBA16F, a few pixels one level apart. The list must be exactly this one; a new entry or
/// a missing one fails the test, so a world cannot be dropped from the comparison by editing the data alone.
const EXPECTED_UNSTABLE: &[&str] = &[
    "oit-intersect.wrl",
    "oit-layers.wrl",
    "oit-many.wrl",
    "oit-physical.wrl",
];

/// (world, why Rust differs).
const KNOWN_GAPS: &[(&str, &str)] = &[];

// Hosted macOS (virtualized Apple Paravirtual device, Metal): the multisampled edge pixels differ from the pinned image by
// more than on the RTX 3060 Ti (see render_rea_i92_items.rs). Limit for multisampled passes on macOS: 1.0%; single-sample
// passes and every other platform keep 0.5%.
fn over_limit_permille(samples: u32) -> usize {
    if cfg!(target_os = "macos") && samples > 1 {
        10
    } else {
        5
    }
}

/// One pass of every world: (world, pass, pixels over the bound?, a note). The failures of the reference files are
/// returned in `failures`.
struct Run {
    failures: Vec<String>,
    /// (world, pass) that are over the bound.
    over: Vec<String>,
    /// (world, pass) that were compared.
    compared: Vec<String>,
}

fn run(oit: bool) -> Option<Run> {
    let Ok(mut g) = Gpu::new() else {
        x3d_evidence::no_gpu("no adapter");
        return None;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap();
    let only = std::env::var("REA_ONLY").unwrap_or_default();
    let base = root.join("rea-oit-render-items-baseline");
    let mut run = Run {
        failures: vec![],
        over: vec![],
        compared: vec![],
    };
    let mut worlds: Vec<String> = std::fs::read_dir(root.join("synthetic-rea-oit-render-items"))
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".wrl") && n.contains(&only))
        .collect();

    worlds.sort();
    assert!(!worlds.is_empty(), "no worlds");
    g.renderer.set_order_independent_transparency(oit);

    for (samples, pass) in [(0u32, "png"), (4, "msaa4")] {
        g.renderer.set_sample_count(samples);
        g.renderer.set_gl_sample_orientation(samples > 1);

        if samples > 1 && g.renderer.sample_count() != samples {
            run.failures
                .push(format!("{samples}x multisampling is not supported here"));
            continue;
        }

        let index: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(base.join(pass).join("index.json"))
                .unwrap_or_else(|e| panic!("{pass}/index.json: {e}")),
        )
        .unwrap();

        // The references were recorded with the option on.
        assert_eq!(
            index["oit"], true,
            "{pass}/index.json: not an OIT recording"
        );

        for r in index["results"].as_array().unwrap() {
            let file = r["file"].as_str().unwrap_or("?");

            if !file.contains(&only) {
                continue;
            }

            if r["ok"] != true {
                run.failures
                    .push(format!("{file} {pass}: the reference run failed"));
            }

            for l in r["log"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|l| l.as_str())
            {
                if l.to_lowercase().contains("error") {
                    run.failures
                        .push(format!("{file} {pass}: the reference logged {l:?}"));
                }
            }
        }

        // Worlds whose X_ITE output differs between its own multisampled runs have no reference (unstable.json).
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
            run.failures.push(format!(
                "{pass}/unstable.json is {listed:?}, expected {expected:?}"
            ));
        }

        let results: Vec<String> = index["results"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|r| r["file"].as_str().map(str::to_string))
            .collect();

        for world in &worlds {
            if unstable.contains(world) {
                eprintln!("{world} {pass}: no reference (X_ITE is not reproducible here)");
                continue;
            }

            if !results.contains(world) {
                run.failures
                    .push(format!("{world} {pass}: no result in {pass}/index.json"));
                continue;
            }

            let png = base.join(pass).join(format!("{world}.png"));

            if !png.exists() {
                run.failures.push(format!(
                    "{world} {pass}: no reference in {}",
                    base.display()
                ));
                continue;
            }

            let (w, h, want) = x3d_render::compare::load_png(&png).unwrap();
            let shot = x3d_render::render_file(
                &mut g,
                &root,
                &root.join(format!("synthetic-rea-oit-render-items/{world}")),
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
            let over = s.pixels_over * 1000 > (w * h) as usize * over_limit_permille(samples);

            eprintln!(
                "{world} {pass} oit={oit}: {} pixels over 8 ({:.3}%), max {}",
                s.pixels_over, s.pct_over, s.max_delta
            );
            run.compared.push(format!("{world} {pass}"));

            if over {
                run.over.push(format!(
                    "{world} {pass}: {} pixels over 8 ({:.3}%), max {}",
                    s.pixels_over, s.pct_over, s.max_delta
                ));
            }
        }
    }

    Some(run)
}

#[test]
fn rea_oit_render_items_match_x_ite() {
    let Some(r) = run(true) else {
        return;
    };
    let mut failures = r.failures;

    for line in &r.over {
        let world = line.split(' ').next().unwrap();

        if let Some((_, why)) = KNOWN_GAPS.iter().find(|(gap, _)| world == *gap) {
            eprintln!("known gap: {line} ({why})");
        } else {
            failures.push(line.clone());
        }
    }

    for (gap, why) in KNOWN_GAPS {
        if !r.over.iter().any(|l| l.starts_with(gap)) {
            failures.push(format!(
                "{gap}: matches X_ITE now; remove it from KNOWN_GAPS ({why})"
            ));
        }
    }

    eprintln!(
        "{} shots checked, {} without a reference (msaa4/unstable.json)",
        r.compared.len(),
        EXPECTED_UNSTABLE.len()
    );
    assert!(!r.compared.is_empty(), "no baselines found");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Negative control: the option off on the Rust side, the same references. A world that stays within the bound would show
/// that the world does not tell order independent transparency from sorted blending. Every world must therefore differ:
/// each is built with overlapping, over-bright transparent shapes (the smallest margin is 1.04% of the pixels against the
/// 0.5% bound), and a new world that cannot tell the two apart is not a useful comparison and should fail here.
#[test]
fn rea_oit_render_items_differ_with_the_option_off() {
    let Some(r) = run(false) else {
        return;
    };

    assert!(r.failures.is_empty(), "{}", r.failures.join("\n"));
    assert!(!r.compared.is_empty(), "no baselines found");

    let within: Vec<&String> = r
        .compared
        .iter()
        .filter(|c| !r.over.iter().any(|o| o.starts_with(c.as_str())))
        .collect();

    assert!(
        within.is_empty(),
        "with the option off these shots still match the OIT references: {within:?}"
    );
}

/// A RenderedTexture is drawn by an auxiliary pass, and X_ITE's TextureBuffer has no OIT (TextureBuffer.js:99-101,
/// getOIT false; X3DRenderObject.js:1293 weights only the main buffer). The main scene here is one opaque quad that shows
/// the texture, and the overlapping transparent, over-bright (emissive + diffuse = 2) boxes are inside the texture only,
/// so the picture must be identical with the option on and off.
#[test]
fn rea_oit_leaves_a_rendered_texture_on_sorted_blending() {
    let Ok(mut g) = Gpu::new() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let dir = std::env::temp_dir().join(format!("x3d-oit-rendered-{}", std::process::id()));

    std::fs::create_dir_all(&dir).unwrap();

    let file = dir.join("rendered.wrl");

    std::fs::write(
        &file,
        "#X3D V4.1 utf8\nPROFILE Interchange\nCOMPONENT Texturing:4\nViewpoint { position 0 0 3 }\nBackground { skyColor [ 0.1 0.1 0.2 ] }\n\
Shape { appearance Appearance { material Material { diffuseColor 1 1 1 } texture RenderedTexture { width 64 height 64 children [ Background { skyColor [ 0.2 0.3 0.6 ] } \
Viewpoint { position 0 0 6 } \
Shape { appearance Appearance { material Material { diffuseColor 1 0 0 emissiveColor 1 0 0 transparency 0.5 } } geometry Box { size 2 2 0.1 } } \
Transform { translation 0.5 0 0.5 children Shape { appearance Appearance { material Material { diffuseColor 0 1 0 emissiveColor 0 1 0 transparency 0.5 } } geometry Box { size 2 2 0.1 } } } ] } } \
geometry IndexedFaceSet { solid FALSE coord Coordinate { point [ -1 -1 0, 1 -1 0, 1 1 0, -1 1 0 ] } coordIndex [ 0 1 2 3 -1 ] \
texCoord TextureCoordinate { point [ 0 0, 1 0, 1 1, 0 1 ] } } }\n",
    )
    .unwrap();

    let shot = |g: &mut Gpu, on: bool| {
        g.renderer.set_order_independent_transparency(on);
        x3d_render::render_file(g, &dir, &file, 64, 64)
            .unwrap()
            .rgba
    };
    let (on, off) = (shot(&mut g, true), shot(&mut g, false));
    let mid = (32 * 64 + 32) * 4;
    // The texture is drawn at all: the green box over the red one, not the white of an unrendered texture.
    assert!(
        off[mid] < 200 && off[mid + 1] > 40,
        "{:?}",
        &off[mid..mid + 4]
    );
    assert_eq!(on, off);
}

/// The accumulation pass keeps the depth state of the normal path (the pipeline key's depth test, write and function,
/// the pass loads the opaque depth): with the opaque occluders of oit-opaque.wrl in front of transparent boxes, the
/// logarithmic and the reversed depth variants draw the picture of the standard one.
#[test]
fn rea_oit_depth_variants_keep_the_opaque_occluders() {
    use x3d_render::gpu::DepthMode;

    let Ok(mut g) = Gpu::new() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap();
    let file = root.join("synthetic-rea-oit-render-items/oit-opaque.wrl");

    g.renderer.set_order_independent_transparency(true);

    let mut shot = |mode: DepthMode| {
        g.renderer.set_depth_mode(mode);
        x3d_render::render_file(&mut g, &root, &file, 320, 240)
            .unwrap()
            .rgba
    };
    let standard = shot(DepthMode::Standard);

    for mode in [DepthMode::Logarithmic, DepthMode::Reversed] {
        let other = shot(mode);
        let s = x3d_render::compare::compare(320, 240, &standard, &other, 8);

        eprintln!(
            "{mode:?}: {} pixels over 8, max {}",
            s.pixels_over, s.max_delta
        );
        assert!(
            s.pixels_over * 1000 <= 320 * 240 * 5,
            "{mode:?}: {} pixels over 8",
            s.pixels_over
        );
    }
}
