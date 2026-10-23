//! ScreenGroup (X3DLayoutContext.getScreenScaleMatrix): its children are in pixels, whatever the depth or the scale around it.

mod common;

use common::gpu::gpu;
use std::path::PathBuf;
use x3d_render::gpu::Gpu;

fn oracle() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap()
}

/// The pixels of a 60 x 40 pixel quad in a ScreenGroup under `outer` (a Transform without its children and closing brace), drawn in a 200 x 160 picture with the viewpoint `vp`.
fn lit_pixels(g: &mut Gpu, name: &str, vp: &str, outer: &str) -> (usize, [usize; 4]) {
    let dir = std::env::temp_dir().join(format!("x3d-render-screen-group-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("{name}.wrl"));
    let text = format!(
        "#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT Layout:2\nCOMPONENT Navigation:3\n{vp}\n\
         {outer} children ScreenGroup {{ children Shape {{ appearance Appearance {{ material Material {{ emissiveColor 1 1 1 diffuseColor 0 0 0 }} }} \
         geometry IndexedFaceSet {{ coord Coordinate {{ point [ -30 -20 0, 30 -20 0, 30 20 0, -30 20 0 ] }} coordIndex [ 0 1 2 3 -1 ] }} }} }} }}\n"
    );
    std::fs::write(&file, text).unwrap();
    let shot = x3d_render::render_file(g, &oracle(), &file, 200, 160).unwrap();
    let mut n = 0;
    let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0, usize::MAX, 0);
    for y in 0..160 {
        for x in 0..200 {
            if shot.rgba[(y * 200 + x) * 4] > 200 {
                n += 1;
                (x0, x1, y0, y1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y));
            }
        }
    }
    (n, [x0, x1, y0, y1])
}

#[test]
fn screen_group_children_are_in_pixels_at_any_depth_and_scale() {
    let Ok(mut g) = gpu() else {
        return eprintln!("no adapter");
    };
    let vp = "Viewpoint { position 0 0 10 }";

    // The quad covers 60 x 40 pixels around the middle of the picture.
    let near = lit_pixels(&mut g, "near", vp, "Transform { translation 0 0 0");
    assert_eq!(near, (60 * 40, [70, 129, 60, 99]), "at the origin");
    // Twice as far from the viewpoint, three times larger around it: still 60 x 40 pixels.
    let far = lit_pixels(
        &mut g,
        "far",
        vp,
        "Transform { translation 0 0 -10 scale 3 3 3",
    );
    assert_eq!(far, near, "farther away and scaled up");
    // A rotation around the ScreenGroup does not turn it: its axes are the model-view axes, normalized.
    let turned = lit_pixels(
        &mut g,
        "turned",
        vp,
        "Transform { translation 0 0 -5 rotation 0 0 1 1.5707963267948966",
    );
    assert_eq!(turned.0, 60 * 40, "rotated by a quarter turn");
    assert_eq!(
        (turned.1[1] - turned.1[0] + 1, turned.1[3] - turned.1[2] + 1),
        (40, 60),
        "rotated by a quarter turn: 40 x 60 pixels"
    );
}

#[test]
fn screen_group_in_an_ortho_viewpoint_is_in_pixels() {
    let Ok(mut g) = gpu() else {
        return eprintln!("no adapter");
    };
    let vp = "OrthoViewpoint { position 0 0 10 fieldOfView [ -5 -5 5 5 ] }";
    let a = lit_pixels(&mut g, "ortho-a", vp, "Transform { translation 0 0 0");
    assert_eq!(a, (60 * 40, [70, 129, 60, 99]));
    let b = lit_pixels(
        &mut g,
        "ortho-b",
        vp,
        "Transform { translation 0 0 -4 scale 2 2 2",
    );
    assert_eq!(b, a);
}
