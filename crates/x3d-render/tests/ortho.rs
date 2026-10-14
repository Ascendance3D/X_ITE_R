//! OrthoViewpoint rendering support.

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
    let Ok(mut g) = Gpu::new() else { return eprintln!("no adapter") };
    let root = oracle();
    let file = root.join("x10-scripts/plane-ortho.wrl");
    let shot = x3d_render::render_file(&mut g, &root, &file, 512, 512).unwrap();
    assert!(!shot.unsupported.contains(&"OrthoViewpoint".to_string()));
    assert_eq!(shot.rgba.len(), 512 * 512 * 4);
}
