//! Appearance alphaMode / alphaCutoff against pinned X_ITE (oracle/x_ite-pin, issue #90): AlphaMode.js names,
//! Appearance.set_alphaMode__ (an unknown name is AUTO), Appearance.set_alphaCutoff__ (the cutoff only for MASK) and
//! X3DShapeNode.set_transparent__ / ParticleSystem.set_transparent__ (AUTO keeps the appearance's and the geometry's
//! transparency; any other mode is transparent only for BLEND). The pixels are checked by render_rea_b09_visual.rs
//! (vis-items-alpha.wrl); these tests need no GPU.

use std::path::PathBuf;
use x3d_render::traverse::{AlphaMode, Builder};
use x3d_runtime::{LocalFetcher, Runtime};

#[test]
fn alpha_mode_names_are_exact_and_unknown_names_are_auto() {
    assert_eq!(AlphaMode::from_name("AUTO"), AlphaMode::Auto);
    assert_eq!(AlphaMode::from_name("OPAQUE"), AlphaMode::Opaque);
    assert_eq!(AlphaMode::from_name("MASK"), AlphaMode::Mask);
    assert_eq!(AlphaMode::from_name("BLEND"), AlphaMode::Blend);

    // AlphaMode .get is a Map lookup: no case folding, no trimming.
    for name in ["", "blend", "Mask", " OPAQUE", "CUTOFF", "auto"] {
        assert_eq!(AlphaMode::from_name(name), AlphaMode::Auto, "{name:?}");
    }
}

#[test]
fn alpha_mode_resolves_transparency_like_set_transparent() {
    use AlphaMode::*;

    // (mode, appearance or geometry transparent) -> (transparent, effective mode)
    let cases = [
        (Auto, false, (false, Opaque)),
        (Auto, true, (true, Blend)),
        (Opaque, false, (false, Opaque)),
        (Opaque, true, (false, Opaque)),
        (Mask, false, (false, Mask)),
        (Mask, true, (false, Mask)),
        (Blend, false, (true, Blend)),
        (Blend, true, (true, Blend)),
    ];

    for (mode, auto, want) in cases {
        assert_eq!(mode.resolve(auto), want, "{mode:?} {auto}");
    }
}

#[test]
fn alpha_cutoff_is_used_only_for_mask() {
    assert_eq!(AlphaMode::Mask.cutoff(0.2), 0.2);
    assert_eq!(AlphaMode::Mask.cutoff(0.0), 0.0);

    for mode in [AlphaMode::Auto, AlphaMode::Opaque, AlphaMode::Blend] {
        assert_eq!(mode.cutoff(0.9), 0.0, "{mode:?}");
    }
}

fn frame(source: &str) -> x3d_render::traverse::Frame {
    let scene = x3d_parse::Parser::new(source, "http://oracle/alpha.x3dv")
        .parse()
        .expect("parse");
    let mut rt = Runtime::new(
        scene,
        Box::new(LocalFetcher {
            root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle"),
            prefix: "http://oracle/".into(),
        }),
    );
    rt.load_world();
    Builder::new(&rt).build(64, 64)
}

/// The draw items of the scene's shapes, in scene order: (transparent, alpha mode, alpha cutoff).
fn items(shapes: &[&str]) -> Vec<(bool, AlphaMode, f32)> {
    let body: String = shapes
        .iter()
        .enumerate()
        .map(|(i, s)| format!("Transform {{ translation {i} 0 -5 children [ {s} ] }}\n"))
        .collect();
    let f = frame(&format!(
        "#X3D V4.0 utf8\nPROFILE Full\nViewpoint {{ position 0 0 10 }}\n{body}"
    ));

    f.items
        .iter()
        .map(|i| (i.transparent, i.alpha_mode, i.alpha_cutoff))
        .collect()
}

fn shape(appearance: &str) -> String {
    format!("Shape {{ {appearance} geometry Box {{ }} }}")
}

#[test]
fn shapes_carry_the_effective_alpha_mode_and_cutoff() {
    use AlphaMode::*;

    let opaque = "material Material { }";
    let transparent = "material Material { transparency 0.5 }";
    let cases: Vec<(String, (bool, AlphaMode, f32))> = vec![
        // AUTO keeps today's decision: transparent with a transparent material, else opaque.
        (shape(""), (false, Opaque, 0.0)),
        (
            shape(&format!("appearance Appearance {{ {opaque} }}")),
            (false, Opaque, 0.0),
        ),
        (
            shape(&format!("appearance Appearance {{ {transparent} }}")),
            (true, Blend, 0.0),
        ),
        (
            shape(&format!(
                "appearance Appearance {{ alphaMode \"AUTO\" alphaCutoff 0.7 {transparent} }}"
            )),
            (true, Blend, 0.0),
        ),
        // An unknown name is AUTO.
        (
            shape(&format!(
                "appearance Appearance {{ alphaMode \"blend\" {opaque} }}"
            )),
            (false, Opaque, 0.0),
        ),
        (
            shape(&format!(
                "appearance Appearance {{ alphaMode \"CUTOFF\" {transparent} }}"
            )),
            (true, Blend, 0.0),
        ),
        // OPAQUE and MASK draw with the opaque shapes, BLEND with the transparent ones.
        (
            shape(&format!(
                "appearance Appearance {{ alphaMode \"OPAQUE\" alphaCutoff 0.7 {transparent} }}"
            )),
            (false, Opaque, 0.0),
        ),
        (
            shape(&format!(
                "appearance Appearance {{ alphaMode \"MASK\" {transparent} }}"
            )),
            (false, Mask, 0.5),
        ),
        (
            shape(&format!(
                "appearance Appearance {{ alphaMode \"MASK\" alphaCutoff 0.25 {opaque} }}"
            )),
            (false, Mask, 0.25),
        ),
        (
            shape(&format!(
                "appearance Appearance {{ alphaMode \"BLEND\" alphaCutoff 0.7 {opaque} }}"
            )),
            (true, Blend, 0.0),
        ),
        // The geometry's transparency (ColorRGBA) counts only for AUTO.
        (
            "Shape { appearance Appearance { } geometry IndexedFaceSet { colorPerVertex FALSE color ColorRGBA { color [ 1 0 0 0.5 ] } coord Coordinate { point [ 0 0 0, 1 0 0, 0 1 0 ] } coordIndex [ 0 1 2 -1 ] } }".into(),
            (true, Blend, 0.0),
        ),
        (
            "Shape { appearance Appearance { alphaMode \"OPAQUE\" } geometry IndexedFaceSet { colorPerVertex FALSE color ColorRGBA { color [ 1 0 0 0.5 ] } coord Coordinate { point [ 0 0 0, 1 0 0, 0 1 0 ] } coordIndex [ 0 1 2 -1 ] } }".into(),
            (false, Opaque, 0.0),
        ),
    ];

    let shapes: Vec<&str> = cases.iter().map(|(s, _)| s.as_str()).collect();
    let got = items(&shapes);
    let want: Vec<_> = cases.iter().map(|(_, w)| *w).collect();

    assert_eq!(got.len(), want.len(), "{got:?}");

    for (i, (g, w)) in got.iter().zip(&want).enumerate() {
        assert_eq!(g, w, "case {i}: {}", cases[i].0);
    }
}
