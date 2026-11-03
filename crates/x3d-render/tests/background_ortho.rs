//! A single-colour Background under an OrthoViewpoint must fill the view, in standard and in logarithmic depth. The cube
//! of that Background is scaled by the far plane (1e10 in the logarithmic mode); its clip coordinates then reach 1e9 and
//! more, which the hosted macOS device (virtualized) and llvmpipe clip wrongly: the Background was missing (issue #89,
//! gs-ortho). The renderer scales a flat cube to the view instead, which is the same picture.

use x3d_render::gpu::{DepthMode, Gpu};

const SKY: [u8; 3] = [51, 51, 71];

fn world(viewpoint: &str, extra: &str) -> String {
    format!(
        "#X3D V4.1 utf8\nPROFILE Interchange\n{viewpoint}\n{extra}Background {{ skyColor 0.2 0.2 0.28 }}\n\
         Shape {{ geometry Box {{ size 1 1 1 }} }}\n"
    )
}

#[test]
fn flat_background_fills_an_orthographic_view_in_both_depth_modes() {
    let Ok(mut g) = Gpu::new() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let dir = std::env::temp_dir().join(format!("x3d-background-ortho-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut failures = vec![];

    for (name, viewpoint, extra) in [
        ("ortho", "OrthoViewpoint { position 0 0 7 }", ""),
        (
            "ortho-wide",
            "OrthoViewpoint { position 0 0 7 fieldOfView -500 -500 500 500 }",
            "",
        ),
        (
            "ortho-off-center",
            "OrthoViewpoint { position 40 -30 7 fieldOfView -2 -2 2 2 }",
            "",
        ),
        ("perspective", "Viewpoint { position 0 0 7 }", ""),
    ] {
        let path = dir.join(format!("{name}.x3dv"));
        std::fs::write(&path, world(viewpoint, extra)).unwrap();

        for mode in [DepthMode::Standard, DepthMode::Logarithmic] {
            g.renderer.set_depth_mode(mode);
            let shot = x3d_render::render_file(&mut g, &dir, &path, 64, 64);
            g.renderer.set_depth_mode(DepthMode::Standard);
            let shot = shot.unwrap();

            // The four corners and the middle of each edge: the Background, never the (unit) box in the middle.
            for (x, y) in [
                (1, 1),
                (62, 1),
                (1, 62),
                (62, 62),
                (32, 1),
                (1, 32),
                (62, 32),
                (32, 62),
            ] {
                let i = (y * 64 + x) * 4;
                let px = [shot.rgba[i], shot.rgba[i + 1], shot.rgba[i + 2]];

                if px.iter().zip(SKY).any(|(a, b)| a.abs_diff(b) > 2) {
                    failures.push(format!("{name} {mode:?} ({x}, {y}): {px:?}, want {SKY:?}"));
                }
            }
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "Background missing: {failures:#?}");
}
