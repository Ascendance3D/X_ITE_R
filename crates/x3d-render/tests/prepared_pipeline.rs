//! `Renderer::new_preparing`: the main pipeline made ahead on another thread draws the same
//! picture as the one a frame makes, and is not used once it no longer fits (another sample
//! count, another shader variant).

mod common;

use common::gpu::gpu;
use std::path::{Path, PathBuf};
use x3d_render::gpu::{DepthMode, Gpu, Renderer};

/// The target format of `render_to_rgba`, which `render_file` draws through.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// A world whose only draw is opaque solid triangles: the pipeline the renderer prepares. In a
/// directory of the test's own (`test`), as the tests run in parallel.
fn world(test: &str) -> (PathBuf, PathBuf) {
    let dir =
        std::env::temp_dir().join(format!("x3d-render-prepared-{}-{test}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("box.wrl");
    std::fs::write(&file, "#VRML V2.0 utf8\nShape { appearance Appearance { material Material { diffuseColor 1 0.4 0.2 } } geometry Box {} }\n").unwrap();
    (dir, file)
}

/// The picture and how many main pipelines its frame created.
fn shot(g: &mut Gpu, dir: &Path, file: &Path) -> (Vec<u8>, usize) {
    let rgba = x3d_render::render_file(g, dir, file, 64, 64).unwrap().rgba;
    (rgba, g.renderer.stats.pipelines_created)
}

/// A renderer like `g`'s, at `samples`, preparing for `prepared` samples.
fn renderer(g: &Gpu, prepared: Option<u32>, samples: u32) -> Renderer {
    let mut r = match prepared {
        Some(n) => Renderer::new_preparing(g.device.clone(), g.queue.clone(), FORMAT, n),
        None => Renderer::new(g.device.clone(), g.queue.clone()),
    };
    r.set_supported_samples(g.renderer.supported_samples().to_vec());
    r.set_sample_count(samples);
    r
}

#[test]
fn a_prepared_pipeline_draws_the_same_picture() {
    let Ok(mut g) = gpu() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let (dir, file) = world("same");

    for samples in [1, 4] {
        g.renderer = renderer(&g, None, samples);
        let (reference, made) = shot(&mut g, &dir, &file);
        assert_eq!(made, 1, "{samples}x: the frame makes the box's pipeline");

        // Waited for before the frame: the frame makes none.
        g.renderer = renderer(&g, Some(samples), samples);
        g.renderer.finish_prepared();
        let (picture, made) = shot(&mut g, &dir, &file);
        assert_eq!(made, 0, "{samples}x: the frame uses the prepared pipeline");
        assert!(
            picture == reference,
            "{samples}x: the prepared pipeline draws another picture"
        );

        // Not waited for: the frame takes it when it gets there.
        g.renderer = renderer(&g, Some(samples), samples);
        let (picture, _) = shot(&mut g, &dir, &file);
        assert!(
            picture == reference,
            "{samples}x: the prepared pipeline, taken by the frame, draws another picture"
        );
        let (_, made) = shot(&mut g, &dir, &file);
        assert_eq!(made, 0, "{samples}x: the second frame reuses the pipeline");
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_prepared_pipeline_that_no_longer_fits_is_not_used() {
    let Ok(mut g) = gpu() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let (dir, file) = world("unfit");

    // Prepared for 4 samples, drawn with 1.
    g.renderer = renderer(&g, None, 1);
    let (reference, _) = shot(&mut g, &dir, &file);
    g.renderer = renderer(&g, Some(4), 1);
    g.renderer.finish_prepared();
    let (picture, made) = shot(&mut g, &dir, &file);
    assert_eq!(
        made, 1,
        "another sample count: the frame makes its own pipeline"
    );
    assert!(picture == reference, "another sample count: wrong picture");

    // Prepared with the standard shader, drawn with the reversed depth variant.
    g.renderer = renderer(&g, None, 1);
    g.renderer.set_depth_mode(DepthMode::Reversed);
    let (reference, _) = shot(&mut g, &dir, &file);
    g.renderer = renderer(&g, Some(1), 1);
    g.renderer.set_depth_mode(DepthMode::Reversed);
    g.renderer.finish_prepared();
    let (picture, made) = shot(&mut g, &dir, &file);
    assert_eq!(
        made, 1,
        "another shader variant: the frame makes its own pipeline"
    );
    assert!(
        picture == reference,
        "another shader variant: wrong picture"
    );
    g.renderer.set_depth_mode(DepthMode::Standard);

    let _ = std::fs::remove_dir_all(&dir);
}
