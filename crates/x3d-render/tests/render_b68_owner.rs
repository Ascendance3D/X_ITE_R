//! Issue #68: the two VolumeRendering decisions that pinned X_ITE cannot draw or draws against the standard
//! (docs/adr/0012-owner-decisions-batch.md). Worlds: oracle/synthetic-rea-b68-owner; the X_ITE side is recorded in
//! `xite-runs.json` (5 identical runs of oracle/x_ite-shot.mjs, pinned 16.5.0 `34cdf9f97`).
//!
//! * BlendedVolumeStyle weightFunction ALPHA1 / ALPHA2 follow ISO/IEC 19775-1 V4.0 Table 41.3: ALPHA1 is the original opacity
//!   (the picture of ALPHA0, which X_ITE draws), ALPHA2 the opacity of the blend voxels. X_ITE draws ALPHA2 as CONSTANT.
//! * ShadedVolumeStyle with a material that is no Material: X_ITE never draws a frame (its `addShaderFields` copies fields
//!   the node does not have); Rust draws the volume and lists the node type as unsupported.

use std::path::PathBuf;
use x3d_render::gpu::Gpu;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap()
}

#[test]
fn x_ite_record_is_five_identical_runs_with_the_expected_outcomes() {
    let rec: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("synthetic-rea-b68-owner/xite-runs.json")).unwrap(),
    )
    .unwrap();
    let runs = rec["runs"].as_array().unwrap();

    assert_eq!(runs.len(), 5);
    assert!(runs.iter().all(|r| r == &runs[0]), "the runs differ");

    let r = &runs[0];
    assert_eq!(r["sh-material.wrl"]["ok"], true);
    assert_eq!(r["sh-physicalmaterial.wrl"]["ok"], false);
    assert_eq!(r["sh-physicalmaterial.wrl"]["error"], "TIMEOUT 45s");
    // X_ITE draws ALPHA2 as the default (CONSTANT) and never draws ALPHA1.
    assert_eq!(
        r["bl-iso2.wrl"]["rgbaSha256"],
        r["bl-const.wrl"]["rgbaSha256"]
    );
    assert_eq!(r["bl-iso1.wrl"]["ok"], false);
}

#[test]
fn blended_alpha_weight_functions_follow_the_standard() {
    let Ok(mut g) = Gpu::new() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let root = root();
    let shot = |g: &mut Gpu, file: &str| {
        x3d_render::render_file(g, &root, &root.join(file), 256, 256)
            .unwrap()
            .rgba
    };
    let alpha0 = shot(&mut g, "synthetic-rea-b07-vol/static/bl-alpha0.wrl");
    let iso1 = shot(&mut g, "synthetic-rea-b68-owner/bl-iso1.wrl");
    let iso2 = shot(&mut g, "synthetic-rea-b68-owner/bl-iso2.wrl");
    let constant = shot(&mut g, "synthetic-rea-b68-owner/bl-const.wrl");

    assert!(alpha0.iter().any(|&b| b != alpha0[0]), "blank picture");
    assert_eq!(
        iso1, alpha0,
        "ALPHA1 / ONE_MINUS_ALPHA1 are Ov (ALPHA0's picture)"
    );
    assert_ne!(iso2, iso1, "ALPHA2 is Oblend, not Ov");
    assert_ne!(iso2, constant, "ALPHA2 is not CONSTANT (X_ITE draws it so)");
}

#[test]
fn shaded_style_with_a_non_material_is_drawn_and_reported() {
    let Ok(mut g) = Gpu::new() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let root = root();
    let physical = x3d_render::render_file(
        &mut g,
        &root,
        &root.join("synthetic-rea-b68-owner/sh-physicalmaterial.wrl"),
        128,
        128,
    )
    .unwrap();
    let plain = x3d_render::render_file(
        &mut g,
        &root,
        &root.join("synthetic-rea-b68-owner/sh-material.wrl"),
        128,
        128,
    )
    .unwrap();

    assert!(plain.unsupported.is_empty(), "{:?}", plain.unsupported);
    assert_eq!(physical.unsupported, vec!["PhysicalMaterial".to_string()]);
    assert!(
        physical.rgba.iter().any(|&b| b != physical.rgba[0]),
        "blank picture"
    );
}
