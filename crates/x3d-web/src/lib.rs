//! X_ITE_R in a web page. The Rust engine (parser, runtime, x3d-viewer's frame loop, x3d-render's WebGPU renderer)
//! is built for wasm32-unknown-unknown; `web/x_ite_r.mjs` is the thin page adapter that defines `<x3d-canvas>` and the
//! X_ITE-named JavaScript API on top of it. No X_ITE JavaScript runs.
//!
//! [`session`] is the platform-neutral part (load a world from bytes, read and set fields, step frames) and is tested
//! natively. `wasm` (wasm32 only) adds the WebGPU device and canvas surface and exports [`wasm::WebBrowser`] through
//! wasm-bindgen. Build the page module with `tools/build-web.sh`.

pub mod session;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

/// X_ITE's default `Multisampling` rendering property (RenderingProperties.js), as x3d-viewer's embedding API uses.
pub const DEFAULT_MULTISAMPLING: u32 = 4;

/// The frame drawn while no world is loaded: the renderer's clear color only.
pub fn empty_frame() -> x3d_render::traverse::Frame {
    x3d_render::traverse::Frame {
        projection: x3d_fields::math::Matrix4::IDENTITY,
        view: x3d_fields::math::Matrix4::IDENTITY,
        items: vec![],
        lights: vec![],
        background: None,
        fog: None,
        unsupported: Default::default(),
        viewpoint: None,
        cube_passes: vec![],
        rendered_passes: vec![],
        environment: None,
        tone_mapping: 0,
        color_space: 2,
        exposure: 1.0,
        viewport: None,
        layers: vec![],
        shadow_maps: vec![],
        projectors: vec![],
    }
}
