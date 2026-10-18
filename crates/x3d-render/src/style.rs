//! The style textures of Browser/Shape/X3DShapeContext.js: getHatchStyleTexture (index) and
//! getLinetypeTexture (), X_ITE's own assets (assets/hatching/1.png … 19.png and
//! assets/linetype/linetypes.png from the pinned source) with getLineFillTextureProperties:
//! NEAREST_PIXEL filters, generateMipMaps FALSE, REPEAT. The shaders sample them directly
//! (Hatch2.glsl, Stipple2.glsl), without the ImageTexture FLIP_Y.

use crate::texture::{Filter, Sampler, TextureImage, Wrap};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

const HATCHING: [&[u8]; 19] = [
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/1.png"),
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/2.png"),
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/3.png"),
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/4.png"),
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/5.png"),
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/6.png"),
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/7.png"),
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/8.png"),
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/9.png"),
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/10.png"),
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/11.png"),
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/12.png"),
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/13.png"),
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/14.png"),
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/15.png"),
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/16.png"),
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/17.png"),
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/18.png"),
    include_bytes!("../../../oracle/x_ite-pin/src/assets/hatching/19.png"),
];

const LINETYPES: &[u8] = include_bytes!("../../../oracle/x_ite-pin/src/assets/linetype/linetypes.png");

thread_local! {
    static CACHE: RefCell<HashMap<i32, Rc<TextureImage>>> = RefCell::new(HashMap::new());
}

fn decode(data: &[u8]) -> TextureImage {
    let img = image::load_from_memory(data).expect("X_ITE style texture").to_rgba8();
    let (width, height) = img.dimensions();
    let rgba = img.into_raw();
    let transparent = rgba.chunks_exact(4).any(|p| p[3] != 255);

    TextureImage {
        width,
        height,
        rgba,
        transparent,
        flip_y: false,
        repeat_s: true,
        repeat_t: true,
        nearest: true,
        no_mipmaps: true,
        url: String::new(),
        sampler: Sampler { mag: Filter::Nearest, min: Filter::Nearest, mipmap: None, wrap_s: Wrap::Repeat, wrap_t: Wrap::Repeat, wrap_r: Wrap::Repeat, anisotropy: 1.0 },
        cube: false,
        generated: None,
        volume: false,
        depth: 1,
        float: false,
    }
}

fn cached(key: i32, data: &'static [u8]) -> Rc<TextureImage> {
    CACHE.with(|c| c.borrow_mut().entry(key).or_insert_with(|| Rc::new(decode(data))).clone())
}

/// getHatchStyleTexture (hatchStyle), hatchStyle in 1 … 19.
pub fn hatch_texture(style: i32) -> Rc<TextureImage> {
    let style = if (1..=19).contains(&style) { style } else { 1 };
    cached(style, HATCHING[style as usize - 1])
}

/// getLinetypeTexture (): one row per linetype.
pub fn linetype_texture() -> Rc<TextureImage> {
    cached(0, LINETYPES)
}
