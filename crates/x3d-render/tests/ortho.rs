//! OrthoViewpoint rendering support.

mod common;

use common::gpu::gpu;
use std::path::PathBuf;
use x3d_render::gpu::Gpu;
use x3d_render::traverse::{ortho, ortho_projection};

fn oracle() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle").canonicalize().unwrap()
}

#[test]
fn ortho_projection_matches_camera_bounds() {
    let m = ortho(-5.0, 5.0, -5.0, 5.0, 0.125, 10000.0);
    // a = 2.0 / 10.0 = 0.2
    assert!((m.0[0] - 0.2).abs() < 1e-9);
    // b = 2.0 / 10.0 = 0.2
    assert!((m.0[5] - 0.2).abs() < 1e-9);

    let proj = ortho_projection([-5.0, -5.0, 5.0, 5.0], 0.125, 10000.0, 512.0, 512.0);
    assert!((proj.0[0] - 0.2).abs() < 1e-9);
    assert!((proj.0[5] - 0.2).abs() < 1e-9);
}

#[test]
fn ortho_viewpoint_renders_without_unsupported_warning() {
    let Ok(mut g) = gpu() else { return eprintln!("no adapter") };
    let root = oracle();
    let file = root.join("x10-scripts/plane-ortho.wrl");
    let shot = x3d_render::render_file(&mut g, &root, &file, 512, 512).unwrap();
    assert!(!shot.unsupported.contains(&"OrthoViewpoint".to_string()));
    assert_eq!(shot.rgba.len(), 512 * 512 * 4);
}

/// X3D 4.1 stores an OrthoViewpoint's fieldOfView as an SFVec4f (the SVG parser's scenes too), up to 4.0 as an MFFloat.
/// Both give the same extents: the picture is the same, and not the one of the default extents.
#[test]
fn ortho_field_of_view_reads_sfvec4f_and_mffloat() {
    let Ok(mut g) = gpu() else { return eprintln!("no adapter") };
    let root = oracle();
    let dir = std::env::temp_dir().join(format!("x3d-render-ortho-fov-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let render = |g: &mut Gpu, version: &str, fov: &str| {
        let file = dir.join(format!("v{version}-{}.wrl", fov.replace([' ', '-', '.'], "_")));
        let text = format!(
            "#X3D V{version} utf8\nPROFILE Interchange\nCOMPONENT Navigation:3\n\
             OrthoViewpoint {{ position 0 0 10 {fov} }}\n\
             Shape {{ appearance Appearance {{ material Material {{ diffuseColor 1 0.4 0.2 }} }} geometry Box {{ size 3 1 1 }} }}\n"
        );
        std::fs::write(&file, text).unwrap();
        x3d_render::render_file(g, &root, &file, 128, 128).unwrap().rgba
    };

    let v41 = render(&mut g, "4.1", "fieldOfView -4 -4 4 4");
    let v40 = render(&mut g, "4.0", "fieldOfView [ -4 -4 4 4 ]");
    let dflt = render(&mut g, "4.1", "");
    assert_eq!(v41, v40, "SFVec4f (4.1) and MFFloat (4.0) extents must draw the same picture");
    assert_ne!(v41, dflt, "extents -4 -4 4 4 must not draw the default -1 -1 1 1 picture");
}
