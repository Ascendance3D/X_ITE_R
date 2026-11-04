//! The `OrderIndependentTransparency` browser option reaches the renderer: the `<x3d-canvas>` attribute, the option set
//! with `setBrowserOption`, and the renderer's switch (sub:render.transparency-sorting). The picture is compared in
//! crates/x3d-render/tests/render_rea_oit_items.rs.

use x3d_render::gpu::Gpu;
use x3d_runtime::browser_options::OptionValue;
use x3d_viewer::viewer::{ORACLE_PAGE_ATTRIBUTES, Viewer3d};

const WORLD: &str = "#X3D V4.0 utf8\nPROFILE Interchange\nViewpoint { position 0 0 6 }\n\
Shape { appearance Appearance { material Material { diffuseColor 1 0 0 emissiveColor 1 0 0 transparency 0.5 } } geometry Box { size 2 2 0.1 } }\n";

fn load(attributes: &[(&str, &str)]) -> (Viewer3d, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("x3d-oit-option-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("world.wrl");
    std::fs::write(&path, WORLD).unwrap();
    let mut all: Vec<(&str, &str)> = ORACLE_PAGE_ATTRIBUTES.to_vec();
    all.extend_from_slice(attributes);
    let v = Viewer3d::load_attributes(&path, None, 946_684_800.0, 64, 64, Some(30.0), false, &all)
        .unwrap();

    (v, dir)
}

#[test]
fn the_option_follows_the_attribute_and_setbrowseroption() {
    let (mut v, dir) = load(&[]);

    assert_eq!(
        v.rendering_properties().order_independent_transparency,
        Some(false)
    );
    assert_eq!(
        v.take_rendering_changes().order_independent_transparency,
        None
    );

    v.set_browser_option("OrderIndependentTransparency", OptionValue::Bool(true));
    assert_eq!(
        v.take_rendering_changes().order_independent_transparency,
        Some(true)
    );
    assert_eq!(
        v.take_rendering_changes().order_independent_transparency,
        None
    );
    assert_eq!(
        v.rendering_properties().order_independent_transparency,
        Some(true)
    );

    v.set_browser_option("OrderIndependentTransparency", OptionValue::Bool(false));
    assert_eq!(
        v.take_rendering_changes().order_independent_transparency,
        Some(false)
    );

    let (v, dir2) = load(&[("orderIndependentTransparency", "true")]);

    assert_eq!(
        v.rendering_properties().order_independent_transparency,
        Some(true)
    );
    std::fs::remove_dir_all(dir).ok();
    std::fs::remove_dir_all(dir2).ok();
}

#[test]
fn the_renderer_takes_the_option_and_draws_with_it() {
    let Ok(mut gpu) = Gpu::new() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let (mut v, dir) = load(&[("orderIndependentTransparency", "true")]);

    assert!(!gpu.renderer.order_independent_transparency());
    v.rendering_properties().apply(&mut gpu.renderer);
    assert!(gpu.renderer.order_independent_transparency());

    v.step(946_684_800.1);

    let frame = v.build_frame();
    let (w, h) = v.framebuffer_size();
    let on = gpu.render(&frame, w, h);

    v.set_browser_option("OrderIndependentTransparency", OptionValue::Bool(false));
    v.take_rendering_changes().apply(&mut gpu.renderer);
    assert!(!gpu.renderer.order_independent_transparency());

    let off = gpu.render(&frame, w, h);

    // One layer of alpha 0.5: the weighted blend of the pinned page is brighter than the sorted blend (the shader
    // output is not clamped before it is weighted), so the two pictures differ at the middle of the box.
    let mid = ((h / 2 * w + w / 2) * 4) as usize;

    assert_ne!(on[mid..mid + 4], off[mid..mid + 4]);
    // revealage 0.5 times the unclamped color 2 (emissive + diffuse): 1.0; the sorted blend gives 0.5.
    assert!(
        on[mid] >= 250 && (120..=135).contains(&off[mid]),
        "{:?} {:?}",
        &on[mid..mid + 4],
        &off[mid..mid + 4]
    );
    std::fs::remove_dir_all(dir).ok();
}
