//! GaussianSplats sphere culling against pinned X_ITE (oracle/x_ite-pin, issue #69):
//! X3DRenderObject.addDisplayShape (X3DRenderObject.js:735) drops a shape whose bbox sphere
//! (`bboxSize .norm () / 2` at `bboxCenter`, in view space) misses the view volume; the splats' bbox is
//! GaussianSplatsShape.set_bbox__ (GaussianSplatsShape.js:152): bboxSize / bboxCenter, else the positions' box.
//! These tests need no GPU.

use std::path::PathBuf;
use x3d_render::traverse::Builder;
use x3d_runtime::{LocalFetcher, Runtime};

/// Draw items with splats for a GaussianSplats node `fields` placed by `translation`.
fn splat_items(translation: &str, fields: &str) -> usize {
    let source = format!(
        "#X3D V4.1 utf8\nPROFILE Full\nCOMPONENT GaussianSplats:1\nViewpoint {{ position 0 0 10 }}\n\
         Transform {{ translation {translation} children [ GaussianSplats {{ {fields}\n\
         positions [ 0 0 0, 0.5 0.5 0 ] orientations [ 0 0 0 1, 0 0 0 1 ] scales [ 0.1 0.1 0.1, 0.1 0.1 0.1 ]\n\
         opacities [ 1, 1 ] sphericalHarmonicsDegree0Coef0 [ 1 0 0, 0 1 0 ] }} ] }}\n"
    );
    let scene = x3d_parse::Parser::new(&source, "http://oracle/splat-cull.x3dv")
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
    let f = Builder::new(&rt).build(64, 64);

    f.items.iter().filter(|i| i.splats.is_some()).count()
}

#[test]
fn splats_in_view_are_drawn() {
    assert_eq!(splat_items("0 0 -5", ""), 1);
}

#[test]
fn splats_behind_the_viewer_are_culled() {
    assert_eq!(splat_items("0 0 30", ""), 0);
}

#[test]
fn splats_far_to_the_side_are_culled() {
    assert_eq!(splat_items("500 0 -5", ""), 0);
}

#[test]
fn declared_bbox_decides_the_culling() {
    // The positions are in view, but the declared box is far off to the side.
    assert_eq!(
        splat_items("0 0 -5", "bboxSize 1 1 1 bboxCenter 500 0 0"),
        0
    );
    // The positions are far off, but the declared box is in view.
    assert_eq!(
        splat_items("500 0 -5", "bboxSize 1 1 1 bboxCenter -500 0 0"),
        1
    );
}
