//! Offscreen wgpu renderer for X3D/VRML scenes, ported from X_ITE's renderer and WebGL2 shaders
//! (https://github.com/create3000/x_ite, MIT, Copyright (c) 2009 Holger Seelig and other
//! contributors). Pinned reference: oracle/x_ite-pin (fork commit 34cdf9f97).
//!
//! Pipeline: [`load::load_world`] (x3d-runtime: parse, setup, loads, frozen-time frames) →
//! [`traverse::Builder`] (bindables, shapes, lights) → [`geometry`] (X_ITE's geometry generation)
//! → [`gpu::Renderer`] (wgpu, WGSL port of the shaders; draws into a texture view of the host's
//! device, keeps its GPU resources between frames) or [`gpu::Gpu`] (headless wrapper: own Vulkan
//! device, RGBA8 target, CPU readback). [`traverse::RenderCache`] keeps geometry and textures
//! between frames for an interactive viewer.
//! [`compare`] measures images against the render oracle (oracle/x_ite-shot.mjs).

pub mod bbox_display;
pub mod compare;
pub mod depth_gpu;
pub mod extras;
pub mod composed;
pub mod fog;
pub mod geometry;
pub mod geometry2d;
pub mod glsl_compat;
pub mod gpu;
pub mod shader_gpu;
pub mod hanim;
pub mod layer;
pub mod load;
pub mod nurbs;
pub mod pointing_gpu;
pub mod sg;
pub mod splats;
pub mod scatter_gpu;
pub mod splats_gpu;
pub mod style;
pub mod tess;
pub mod text;
pub mod texture;
pub mod traverse;
pub mod volume;
pub mod volume_gpu;

use std::path::Path;

pub struct Shot {
    pub width: u32,
    pub height: u32,
    /// RGBA8, top row first. Alpha is the framebuffer alpha (the oracle's readback is opaque).
    pub rgba: Vec<u8>,
    pub unsupported: Vec<String>,
    pub log: Vec<String>,
}

/// Load a world from the oracle directory and render it like oracle/x_ite-pin/src/shot.html.
pub fn render_file(gpu: &mut gpu::Gpu, oracle_root: &Path, file: &Path, width: u32, height: u32) -> Result<Shot, String> {
    let mut rt = load::load_world(oracle_root, file, true)?;
    // The still is the last of several frames (x_ite-shot.mjs draws 10).
    rt.rendered_textures_settled();
    let frame = traverse::Builder::new(&rt).build(width, height);
    let rgba = gpu.render(&frame, width, height);
    Ok(Shot { width, height, rgba, unsupported: frame.unsupported.into_iter().collect(), log: rt.log.clone() })
}

pub fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    image::save_buffer(path, rgba, width, height, image::ExtendedColorType::Rgba8).map_err(|e| e.to_string())
}
