//! ImageTexture / PixelTexture loading (Components/Texturing/ImageTexture.js, PixelTexture.js,
//! X3DTexture2DNode.js, X3DSingleTextureNode.js).
//!
//! X_ITE decodes with the browser's image decoder; here the `image` crate decodes PNG, JPEG, GIF
//! (first frame) and BMP. Colour management that Chrome applies to tagged images
//! (`colorSpaceConversion`, gAMA/iCCP chunks) is not reproduced.

use crate::sg::Sg;
use x3d_fields::NodeId;

#[derive(Clone, Debug)]
pub struct TextureImage {
    pub width: u32,
    pub height: u32,
    /// RGBA8 rows in upload order (first row = t 0 before X_ITE's FLIP_Y).
    pub rgba: Vec<u8>,
    /// isImageTransparent: any alpha ≠ 255.
    pub transparent: bool,
    /// X3D_TEXTURE_0_FLIP_Y (ImageTexture, MovieTexture: texture type 1).
    pub flip_y: bool,
    pub repeat_s: bool,
    pub repeat_t: bool,
    /// Textures smaller than getMinTextureSize (16) without TextureProperties: NEAREST, no mipmaps.
    pub nearest: bool,
    /// Upload level 0 only (TextureProperties generateMipMaps FALSE: the Background face textures).
    pub no_mipmaps: bool,
    pub url: String,
}

impl TextureImage {
    /// X3DTexture2DNode.clearTexture: 1×1 opaque white.
    pub fn white() -> Self {
        TextureImage {
            width: 1,
            height: 1,
            rgba: vec![255; 4],
            transparent: false,
            flip_y: true,
            repeat_s: true,
            repeat_t: true,
            nearest: true,
            no_mipmaps: false,
            url: String::new(),
        }
    }
}

fn finish(width: u32, height: u32, rgba: Vec<u8>, flip_y: bool, repeat_s: bool, repeat_t: bool, url: String) -> TextureImage {
    let transparent = rgba.chunks_exact(4).any(|p| p[3] != 255);
    TextureImage { width, height, rgba, transparent, flip_y, repeat_s, repeat_t, nearest: width.max(height) < 16, no_mipmaps: false, url }
}

/// ImageTexture.loadData: FileLoader URL fallback; a candidate that does not decode fails and
/// the next one is tried.
pub fn load_image_texture(sg: &Sg, n: NodeId, base: &str, fetcher: &dyn x3d_runtime::Fetcher) -> Option<TextureImage> {
    if !sg.bool(n, "load", true) {
        return None;
    }

    let base = url::Url::parse(base).ok()?;
    let repeat_s = sg.bool(n, "repeatS", true);
    let repeat_t = sg.bool(n, "repeatT", true);

    for candidate in sg.strings(n, "url") {
        if candidate.is_empty() {
            continue;
        }
        let Ok(resolved) = base.join(&candidate) else { continue };
        let Ok(data) = fetcher.fetch(&resolved) else { continue };
        let Ok(img) = image::load_from_memory(&data) else { continue };
        let img = img.to_rgba8();
        let (w, h) = img.dimensions();
        return Some(finish(w, h, img.into_raw(), true, repeat_s, repeat_t, resolved.to_string()));
    }

    None
}

/// The six private ImageTextures of Background (Background.initialize): the `xxxUrl` list is the
/// texture's `url`, the browser's backgroundTextureProperties (X3DEnvironmentalEffectsContext.js:
/// CLAMP_TO_EDGE, NICEST filters, generateMipMaps FALSE) replace the default TextureProperties,
/// so there is no mipmap chain, no small-image NEAREST rule and no repeat.
pub fn load_background_texture(urls: &[String], base: &str, fetcher: &dyn x3d_runtime::Fetcher) -> Option<TextureImage> {
    let base = url::Url::parse(base).ok()?;

    for candidate in urls {
        if candidate.is_empty() {
            continue;
        }
        let Ok(resolved) = base.join(candidate) else { continue };
        let Ok(data) = fetcher.fetch(&resolved) else { continue };
        let Ok(img) = image::load_from_memory(&data) else { continue };
        let img = img.to_rgba8();
        let (w, h) = img.dimensions();
        let mut t = finish(w, h, img.into_raw(), true, false, false, resolved.to_string());

        t.nearest = false;
        t.no_mipmaps = true;
        return Some(t);
    }

    None
}

/// PixelTexture: SFImage, first pixel lower left, uploaded as is (texture type 2, no flip).
pub fn pixel_texture(sg: &Sg, n: NodeId) -> Option<TextureImage> {
    let img = sg.image(n, "image")?;
    let (w, h, comp) = (img.width.max(0) as u32, img.height.max(0) as u32, img.comp);

    if w == 0 || h == 0 || !(1..=4).contains(&comp) {
        return None;
    }

    let mut rgba = Vec::with_capacity((w * h * 4) as usize);

    for i in 0..(w * h) as usize {
        let p = *img.array.get(i).unwrap_or(&0) as u32;
        let px = match comp {
            1 => [p as u8, p as u8, p as u8, 255],
            2 => [(p >> 8) as u8, (p >> 8) as u8, (p >> 8) as u8, p as u8],
            3 => [(p >> 16) as u8, (p >> 8) as u8, p as u8, 255],
            _ => [(p >> 24) as u8, (p >> 16) as u8, (p >> 8) as u8, p as u8],
        };
        rgba.extend_from_slice(&px);
    }

    Some(finish(w, h, rgba, false, sg.bool(n, "repeatS", true), sg.bool(n, "repeatT", true), String::new()))
}

/// Weights of one output texel when halving a dimension of `n` texels: a 2-tap box for even
/// `n`, the 3-tap polyphase filter of "Non-Power-of-Two Mipmapping" (NVIDIA, 2005) for odd `n`
/// (the filter NVIDIA's gl.generateMipmap appears to use; measured against the oracle).
fn taps(n: u32, x: u32) -> Vec<(u32, f64)> {
    if n == 1 {
        return vec![(0, 1.0)];
    }
    if n % 2 == 0 {
        return vec![(2 * x, 0.5), (2 * x + 1, 0.5)];
    }
    let k = (n / 2) as f64;
    let d = n as f64;
    let xf = x as f64;
    vec![(2 * x, (k - xf) / d), (2 * x + 1, k / d), (2 * x + 2, (xf + 1.0) / d)]
}

/// Mip chain like gl.generateMipmap (level sizes floor (n / 2)). Level 0 is the image itself.
pub fn mip_chain(t: &TextureImage) -> Vec<(u32, u32, Vec<u8>)> {
    let mut levels = vec![(t.width, t.height, t.rgba.clone())];

    if t.nearest {
        return levels;
    }

    // Filter in floating point from the previous level, round once per level.
    loop {
        let (w, h, ref src) = *levels.last().unwrap();
        if w == 1 && h == 1 {
            break;
        }
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut dst = vec![0u8; (nw * nh * 4) as usize];

        for y in 0..nh {
            let ty = taps(h, y);
            for x in 0..nw {
                let tx = taps(w, x);
                for c in 0..4 {
                    let mut sum = 0.0;
                    for &(sy, wy) in &ty {
                        for &(sx, wx) in &tx {
                            sum += wx * wy * src[((sy * w + sx) * 4 + c) as usize] as f64;
                        }
                    }
                    dst[((y * nw + x) * 4 + c) as usize] = sum.round().clamp(0.0, 255.0) as u8;
                }
            }
        }

        levels.push((nw, nh, dst));
    }

    levels
}
