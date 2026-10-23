//! ImageTexture / PixelTexture loading (Components/Texturing/ImageTexture.js, PixelTexture.js,
//! X3DTexture2DNode.js, X3DSingleTextureNode.js).
//!
//! X_ITE decodes with the browser's image decoder; here the `image` crate decodes PNG, JPEG, GIF
//! (first frame) and BMP. Colour management that Chrome applies to tagged images
//! (`colorSpaceConversion`, gAMA/iCCP chunks) is not reproduced.

use crate::sg::Sg;
use x3d_fields::{FieldValue, NodeId};

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
    /// X3DSingleTextureNode.updateTextureParameters: filters, wrap modes and anisotropy.
    pub sampler: Sampler,
    /// A cube map (X3DEnvironmentTextureNode): `rgba` holds six square faces in the GL target
    /// order +X, -X, +Y, -Y, +Z, -Z.
    pub cube: bool,
    /// A GeneratedCubeMapTexture (runtime uid, node): the renderer draws its faces
    /// ([`crate::traverse::CubePass`]) and keeps them between frames; `rgba` is empty.
    pub generated: Option<(u64, NodeId)>,
    /// An X3DTexture3DNode (TEXTURE_3D): `rgba` holds `depth` slices of `width` × `height`, slice
    /// r 0 first, each with its rows in upload order (row 0 = t 0). Not flipped.
    pub volume: bool,
    /// The number of slices (1 for a 2D or cube texture).
    pub depth: u32,
    /// X3DTexture2DNode.setFloat: a RenderedTexture depthMap (RGBA32F in X_ITE, 16-bit float here,
    /// as 32-bit float textures are not filterable in WebGPU).
    pub float: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Filter {
    Nearest,
    Linear,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wrap {
    Repeat,
    ClampToEdge,
    MirroredRepeat,
}

/// The GL texture parameters: TEXTURE_MAG_FILTER, TEXTURE_MIN_FILTER (`mipmap` None for a filter
/// without mipmaps), TEXTURE_WRAP_S / T and TEXTURE_MAX_ANISOTROPY_EXT.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sampler {
    pub mag: Filter,
    pub min: Filter,
    pub mipmap: Option<Filter>,
    pub wrap_s: Wrap,
    pub wrap_t: Wrap,
    pub wrap_r: Wrap,
    pub anisotropy: f64,
}

impl Sampler {
    pub fn new(
        mag: Filter,
        min: Filter,
        mipmap: Option<Filter>,
        repeat_s: bool,
        repeat_t: bool,
    ) -> Self {
        let wrap = |r: bool| if r { Wrap::Repeat } else { Wrap::ClampToEdge };
        Sampler {
            mag,
            min,
            mipmap,
            wrap_s: wrap(repeat_s),
            wrap_t: wrap(repeat_t),
            wrap_r: Wrap::ClampToEdge,
            anisotropy: 1.0,
        }
    }
}

/// TextureProperties.getMinificationFilter: [without mipmaps, with mipmaps] per name.
fn minification_filter(name: &str) -> Option<[(Filter, Option<Filter>); 2]> {
    use Filter::*;

    Some(match name {
        "AVG_PIXEL_AVG_MIPMAP" | "NICEST" => [(Linear, None), (Linear, Some(Linear))],
        "AVG_PIXEL_NEAREST_MIPMAP" => [(Linear, None), (Linear, Some(Nearest))],
        "NEAREST_PIXEL_AVG_MIPMAP" => [(Nearest, None), (Nearest, Some(Linear))],
        "NEAREST_PIXEL_NEAREST_MIPMAP" | "FASTEST" => [(Nearest, None), (Nearest, Some(Nearest))],
        "AVG_PIXEL" => [(Linear, None), (Linear, None)],
        "NEAREST_PIXEL" => [(Nearest, None), (Nearest, None)],
        _ => return None,
    })
}

/// TextureProperties.getMagnificationFilter.
fn magnification_filter(name: &str) -> Option<Filter> {
    match name {
        "AVG_PIXEL" | "NICEST" => Some(Filter::Linear),
        "NEAREST_PIXEL" | "FASTEST" => Some(Filter::Nearest),
        _ => None,
    }
}

/// TextureProperties.getBoundaryMode: CLAMP and CLAMP_TO_BOUNDARY are CLAMP_TO_EDGE.
fn boundary_mode(name: &str) -> Wrap {
    match name {
        "CLAMP" | "CLAMP_TO_EDGE" | "CLAMP_TO_BOUNDARY" => Wrap::ClampToEdge,
        "MIRRORED_REPEAT" => Wrap::MirroredRepeat,
        _ => Wrap::Repeat,
    }
}

/// MAX_TEXTURE_MAX_ANISOTROPY_EXT of the oracle GPU (and of the 16x WebGPU limit).
pub const MAX_ANISOTROPY: f64 = 16.0;

/// X3DSingleTextureNode.updateTextureParameters with the texture's own TextureProperties node
/// (`textureProperties` field): its filters (falling back to the browser default TextureProperties,
/// TextureQuality MEDIUM, for DEFAULT and unknown names), its boundary modes instead of
/// repeatS / repeatT, no small-image NEAREST rule, mipmaps only when generateMipMaps and the
/// texture can mipmap (`can_mipmap`: ImageTexture; PixelTexture never sets mipMaps).
pub fn apply_texture_properties(t: &mut TextureImage, sg: &Sg, texture: NodeId, can_mipmap: bool) {
    let Some(tp) = sg
        .node(texture, "textureProperties")
        .filter(|p| sg.type_name(*p) == Some("TextureProperties"))
    else {
        return;
    };

    let mipmaps = can_mipmap && sg.bool(tp, "generateMipMaps", false);
    let i = mipmaps as usize;
    // DEFAULT and unknown names: the browser default TextureProperties (TextureQuality).
    let (default_min, default_mag) = default_filters();
    let (min, mipmap) = minification_filter(&sg.string(tp, "minificationFilter", "DEFAULT"))
        .map(|f| f[i])
        .unwrap_or(minification_filter(default_min).unwrap()[i]);
    let mag = magnification_filter(&sg.string(tp, "magnificationFilter", "DEFAULT"))
        .unwrap_or(magnification_filter(default_mag).unwrap());

    t.nearest = false;
    t.no_mipmaps = !mipmaps;
    t.sampler = Sampler {
        mag,
        min,
        mipmap,
        wrap_s: boundary_mode(&sg.string(tp, "boundaryModeS", "REPEAT")),
        wrap_t: boundary_mode(&sg.string(tp, "boundaryModeT", "REPEAT")),
        wrap_r: boundary_mode(&sg.string(tp, "boundaryModeR", "REPEAT")),
        anisotropy: sg
            .f64(tp, "anisotropicDegree", 1.0)
            .clamp(0.0, MAX_ANISOTROPY),
    };
}

impl TextureImage {
    /// ScreenText's canvas as a PixelTexture: transparent whatever its alpha, TextureProperties NEAREST_PIXEL both ways,
    /// CLAMP_TO_EDGE, no mipmaps (X3DLayoutContext.getScreenTextureProperties).
    pub fn screen_text(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        let mut t = finish(
            width,
            height,
            rgba,
            false,
            false,
            false,
            String::new(),
            false,
        );

        t.transparent = true;
        t.nearest = true;
        t.no_mipmaps = true;
        t.sampler = Sampler::new(Filter::Nearest, Filter::Nearest, None, false, false);
        t
    }

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
            sampler: Sampler::new(Filter::Nearest, Filter::Nearest, None, true, true),
            cube: false,
            generated: None,
            volume: false,
            depth: 1,
            float: false,
        }
    }
}

/// The minification and magnification filters of the browser default TextureProperties
/// (generateMipMaps TRUE), X3DTexturingContext.setTextureQuality: LOW FASTEST / FASTEST, MEDIUM
/// AVG_PIXEL_NEAREST_MIPMAP / NICEST, HIGH NICEST / NICEST.
fn default_filters() -> (&'static str, &'static str) {
    match crate::traverse::texture_quality() {
        crate::traverse::Quality::Low => ("FASTEST", "FASTEST"),
        crate::traverse::Quality::Medium => ("AVG_PIXEL_NEAREST_MIPMAP", "NICEST"),
        crate::traverse::Quality::High => ("NICEST", "NICEST"),
    }
}

/// The texture parameters without a TextureProperties node: below getMinTextureSize (16) NEAREST
/// without mipmaps; else the browser default TextureProperties (TextureQuality MEDIUM: mag LINEAR,
/// min LINEAR_MIPMAP_NEAREST with mipmaps, LINEAR without); repeatS / repeatT.
fn finish(
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    flip_y: bool,
    repeat_s: bool,
    repeat_t: bool,
    url: String,
    can_mipmap: bool,
) -> TextureImage {
    let transparent = rgba.chunks_exact(4).any(|p| p[3] != 255);
    let nearest = width.max(height) < 16;
    let sampler = if nearest {
        Sampler::new(Filter::Nearest, Filter::Nearest, None, repeat_s, repeat_t)
    } else {
        let (min_name, mag_name) = default_filters();
        let (min, mipmap) = minification_filter(min_name).unwrap()[can_mipmap as usize];
        Sampler::new(
            magnification_filter(mag_name).unwrap(),
            min,
            mipmap,
            repeat_s,
            repeat_t,
        )
    };

    TextureImage {
        width,
        height,
        rgba,
        transparent,
        flip_y,
        repeat_s,
        repeat_t,
        nearest,
        no_mipmaps: !nearest && !can_mipmap,
        url,
        sampler,
        cube: false,
        generated: None,
        volume: false,
        depth: 1,
        float: false,
    }
}

/// ImageTexture.loadData: FileLoader URL fallback; a candidate that does not decode fails and
/// the next one is tried.
pub fn load_image_texture(
    sg: &Sg,
    n: NodeId,
    base: &str,
    fetcher: &dyn x3d_runtime::Fetcher,
) -> Option<TextureImage> {
    if !sg.bool(n, "load", true) {
        return None;
    }

    let base = url::Url::parse(base).ok()?;
    let repeat_s = sg.bool(n, "repeatS", true);
    let repeat_t = sg.bool(n, "repeatT", true);

    // The url of the last load request (X3DUrlObject.requestImmediateLoad): a url change that
    // did not load (LoadUrlObjects FALSE) keeps the image.
    let urls = sg.rt.texture_url(n).unwrap_or_else(|| sg.strings(n, "url"));

    for candidate in urls {
        if candidate.is_empty() {
            continue;
        }
        let Ok(resolved) = base.join(&candidate) else {
            continue;
        };
        let Ok(data) = fetcher.fetch(&resolved) else {
            continue;
        };
        let Ok(img) = image::load_from_memory(&data) else {
            continue;
        };
        let img = img.to_rgba8();
        let (w, h) = img.dimensions();
        let mut t = finish(
            w,
            h,
            img.into_raw(),
            true,
            repeat_s,
            repeat_t,
            resolved.to_string(),
            true,
        );
        apply_texture_properties(&mut t, sg, n, true);
        return Some(t);
    }

    None
}

/// The six private ImageTextures of Background (Background.initialize): the `xxxUrl` list is the
/// texture's `url`, the browser's backgroundTextureProperties (X3DEnvironmentalEffectsContext.js:
/// CLAMP_TO_EDGE, NICEST filters, generateMipMaps FALSE) replace the default TextureProperties,
/// so there is no mipmap chain, no small-image NEAREST rule and no repeat.
pub fn load_background_texture(
    urls: &[String],
    base: &str,
    fetcher: &dyn x3d_runtime::Fetcher,
) -> Option<TextureImage> {
    let base = url::Url::parse(base).ok()?;

    for candidate in urls {
        if candidate.is_empty() {
            continue;
        }
        let Ok(resolved) = base.join(candidate) else {
            continue;
        };
        let Ok(data) = fetcher.fetch(&resolved) else {
            continue;
        };
        let Ok(img) = image::load_from_memory(&data) else {
            continue;
        };
        let img = img.to_rgba8();
        let (w, h) = img.dimensions();
        let mut t = finish(
            w,
            h,
            img.into_raw(),
            true,
            false,
            false,
            resolved.to_string(),
            false,
        );

        t.nearest = false;
        t.no_mipmaps = true;
        t.sampler = Sampler::new(Filter::Linear, Filter::Linear, None, false, false);
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

    let mut t = finish(
        w,
        h,
        rgba,
        false,
        sg.bool(n, "repeatS", true),
        sg.bool(n, "repeatT", true),
        String::new(),
        false,
    );
    apply_texture_properties(&mut t, sg, n, false);
    Some(t)
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
    vec![
        (2 * x, (k - xf) / d),
        (2 * x + 1, k / d),
        (2 * x + 2, (xf + 1.0) / d),
    ]
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

/// The GL cube map layer of each X3DEnvironmentTextureNode slot (targets: front +Z, back -Z,
/// left -X, right +X, top +Y, bottom -Y); layers are +X, -X, +Y, -Y, +Z, -Z.
const SLOT_LAYER: [usize; 6] = [4, 5, 1, 0, 2, 3];

/// A cube map from six square faces in slot order, each with its rows in GL face order (row 0
/// first), with the X3DEnvironmentTextureNode texture parameters (no repeat; below 16 texels
/// NEAREST without mipmaps unless the node has TextureProperties).
fn cube_texture(
    sg: &Sg,
    n: NodeId,
    size: u32,
    faces: &[Vec<u8>; 6],
    transparent: bool,
    can_mipmap: bool,
    url: String,
) -> TextureImage {
    let mut rgba = vec![0u8; (size * size * 4 * 6) as usize];
    let face_bytes = (size * size * 4) as usize;

    for (slot, face) in faces.iter().enumerate() {
        let layer = SLOT_LAYER[slot];
        rgba[layer * face_bytes..(layer + 1) * face_bytes].copy_from_slice(face);
    }

    let mut t = finish(size, size, rgba, false, false, false, url, can_mipmap);
    t.transparent = transparent;
    t.cube = true;
    apply_texture_properties(&mut t, sg, n, can_mipmap);
    t
}

/// X3DEnvironmentTextureNode.clearTexture: six 1×1 white faces, size 1.
pub fn white_cube() -> TextureImage {
    let mut t = TextureImage::white();
    t.cube = true;
    t.rgba = vec![255; 24];
    t.flip_y = false;
    t.repeat_s = false;
    t.repeat_t = false;
    t.sampler = Sampler::new(Filter::Nearest, Filter::Nearest, None, false, false);
    t
}

/// ComposedCubeMapTexture.update: when all six slots hold X3DTexture2DNodes in COMPLETE_STATE with
/// square images of one size, the faces copied from them (an ImageTexture, texture type 1, as
/// stored; any other node flipped vertically); otherwise the cleared texture. `face` loads the
/// texture of a slot node (`None` for one that is not complete).
pub fn composed_cube(
    sg: &Sg,
    n: NodeId,
    slots: [Option<NodeId>; 6],
    mut face: impl FnMut(NodeId) -> Option<std::rc::Rc<TextureImage>>,
) -> TextureImage {
    let mut images = vec![];

    for slot in slots {
        let Some(node) = slot.filter(|s| {
            matches!(
                sg.type_name(*s),
                Some("ImageTexture" | "PixelTexture" | "MovieTexture")
            )
        }) else {
            return white_cube();
        };
        let Some(image) = face(node) else {
            return white_cube();
        };
        images.push((node, image));
    }

    let size = images[0].1.width;

    if images
        .iter()
        .any(|(_, t)| t.width != size || t.height != size)
    {
        return white_cube();
    }

    let faces: [Vec<u8>; 6] = std::array::from_fn(|i| {
        let (node, t) = &images[i];
        if sg.type_name(*node) == Some("ImageTexture") {
            t.rgba.clone()
        } else {
            let row = (size * 4) as usize;
            t.rgba.chunks_exact(row).rev().flatten().copied().collect()
        }
    });
    // setTransparent (some face transparent), setMipMaps (every face can mipmap: ImageTexture).
    let transparent = images.iter().any(|(_, t)| t.transparent);
    let can_mipmap = images
        .iter()
        .all(|(node, _)| sg.type_name(*node) == Some("ImageTexture"));

    cube_texture(sg, n, size, &faces, transparent, can_mipmap, String::new())
}

/// ImageCubeMapTexture.loadData / setImage: the first url that loads and decodes; a 4:3 image is
/// a sky box cross (skyBoxToCubeMap), a 2:1 image a panorama (panoramaToCubeMap); any other image
/// leaves the cleared texture.
pub fn load_image_cube(
    sg: &Sg,
    n: NodeId,
    base: &str,
    fetcher: &dyn x3d_runtime::Fetcher,
) -> Option<TextureImage> {
    if !sg.bool(n, "load", true) {
        return None;
    }

    let base = url::Url::parse(base).ok()?;
    let urls = sg.rt.texture_url(n).unwrap_or_else(|| sg.strings(n, "url"));

    for candidate in urls {
        if candidate.is_empty() {
            continue;
        }
        let Ok(resolved) = base.join(&candidate) else {
            continue;
        };
        let Ok(data) = fetcher.fetch(&resolved) else {
            continue;
        };
        let Ok(img) = image::load_from_memory(&data) else {
            continue;
        };
        let img = img.to_rgba8();
        let (w, h) = img.dimensions();
        let rgba = img.into_raw();
        let aspect = w as f64 / h as f64;

        if (aspect - 4.0 / 3.0).abs() < 0.01 {
            return Some(sky_box_cube(sg, n, w, h, &rgba, resolved.to_string()));
        }
        if (aspect - 2.0).abs() < 0.01 {
            return Some(panorama_cube(sg, n, w, h, &rgba, resolved.to_string()));
        }

        return Some(white_cube());
    }

    None
}

/// skyBoxToCubeMap: the faces of the cross (offsets front (1, 1), back (3, 1), left (0, 1),
/// right (2, 1), top (1, 0), bottom (1, 2) in quarter widths and third heights, rows from the top
/// of the image).
fn sky_box_cube(sg: &Sg, n: NodeId, w: u32, h: u32, rgba: &[u8], url: String) -> TextureImage {
    const OFFSETS: [(u32, u32); 6] = [(1, 1), (3, 1), (0, 1), (2, 1), (1, 0), (1, 2)];
    let (w4, h3) = (w / 4, h / 3);
    let faces: [Vec<u8>; 6] = std::array::from_fn(|i| {
        let (ox, oy) = OFFSETS[i];
        let mut face = Vec::with_capacity((w4 * h3 * 4) as usize);
        for y in 0..h3 {
            let start = (((oy * h3 + y) * w + ox * w4) * 4) as usize;
            face.extend_from_slice(&rgba[start..start + (w4 * 4) as usize]);
        }
        face
    });
    let transparent = faces.iter().any(|f| f.chunks_exact(4).any(|p| p[3] != 255));

    // A cross whose faces are not square keeps the texture size of its quarter width.
    if w4 != h3 {
        return white_cube();
    }

    cube_texture(sg, n, w4, &faces, transparent, true, url)
}

/// panoramaToCubeMap (Panorama2.fs): each face pixel samples the panorama (LINEAR,
/// MIRRORED_REPEAT, row 0 the top of the image) in the direction of its face coordinate.
fn panorama_cube(sg: &Sg, n: NodeId, w: u32, h: u32, rgba: &[u8], url: String) -> TextureImage {
    use std::f32::consts::PI;
    let size = h / 2;

    let uv_to_xyz = |face: usize, u: f32, v: f32| -> [f32; 3] {
        match face {
            0 => [1.0, v, u],
            1 => [-1.0, v, -u],
            2 => [u, v, -1.0],
            3 => [-u, v, 1.0],
            4 => [v, -1.0, u],
            _ => [-v, 1.0, u],
        }
    };
    let mirror = |i: i64, n: i64| -> usize {
        let period = 2 * n;
        let m = i.rem_euclid(period);
        (if m < n { m } else { period - 1 - m }) as usize
    };
    let texel = |x: usize, y: usize, c: usize| rgba[(y * w as usize + x) * 4 + c] as f32;
    let sample = |s: f32, t: f32| -> [u8; 3] {
        let (u, v) = (s * w as f32 - 0.5, t * h as f32 - 0.5);
        let (x0, y0) = (u.floor(), v.floor());
        let (fx, fy) = (u - x0, v - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let (xa, xb) = (mirror(x0, w as i64), mirror(x0 + 1, w as i64));
        let (ya, yb) = (mirror(y0, h as i64), mirror(y0 + 1, h as i64));
        std::array::from_fn(|c| {
            let top = texel(xa, ya, c) * (1.0 - fx) + texel(xb, ya, c) * fx;
            let bottom = texel(xa, yb, c) * (1.0 - fx) + texel(xb, yb, c) * fx;
            (top * (1.0 - fy) + bottom * fy).round().clamp(0.0, 255.0) as u8
        })
    };

    let faces: [Vec<u8>; 6] = std::array::from_fn(|face| {
        let mut out = Vec::with_capacity((size * size * 4) as usize);
        for row in 0..size {
            // FullScreen2.vs: texCoord is the clip-space position; row 0 is the bottom (y -1).
            let v = (row as f32 + 0.5) / size as f32 * 2.0 - 1.0;
            for col in 0..size {
                let u = (col as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let [x, y, z] = uv_to_xyz(face, u, v);
                let l = (x * x + y * y + z * z).sqrt();
                let (x, y, z) = (x / l, y / l, z / l);
                let s = 0.5 + 0.5 * z.atan2(x) / PI;
                let t = 1.0 - y.clamp(-1.0, 1.0).acos() / PI;
                let [r, g, b] = sample(s, t);
                out.extend_from_slice(&[r, g, b, 255]);
            }
        }
        out
    });

    cube_texture(sg, n, size, &faces, false, true, url)
}

/// GeneratedCubeMapTexture.set_size__ / updateTextureParameters: the texture the shapes sample,
/// `size` × `size` faces drawn by the renderer (no mipmaps; below 16 NEAREST unless the node has
/// TextureProperties). `transparent` is setTransparent (background .isTransparent ()).
pub fn generated_cube(sg: &Sg, n: NodeId, size: u32, transparent: bool) -> TextureImage {
    let mut t = finish(
        size,
        size,
        vec![],
        false,
        false,
        false,
        String::new(),
        false,
    );
    t.transparent = transparent;
    t.cube = true;
    t.generated = Some((sg.rt.uid, n));
    apply_texture_properties(&mut t, sg, n, false);
    t
}

/// X3DTexture3DNode.clearTexture: 1 × 1 × 1 opaque white (repeat off, as the fields' defaults).
pub fn white_3d() -> TextureImage {
    let mut t = TextureImage::white();
    t.volume = true;
    t.flip_y = false;
    t.repeat_s = false;
    t.repeat_t = false;
    t.sampler = Sampler::new(Filter::Nearest, Filter::Nearest, None, false, false);
    t
}

/// An incomplete GL texture (a 3D texture with a zero size): sampled as (0, 0, 0, 1).
fn incomplete_3d() -> TextureImage {
    let mut t = white_3d();
    t.rgba = vec![0, 0, 0, 255];
    t
}

/// X3DTexture3DNode.setTextureData / updateTextureParameters: the texture parameters of a
/// `width` × `height` × `depth` texture. Without TextureProperties: below getMinTextureSize (16) in
/// width and height (the depth is not tested) NEAREST without mipmaps, else the browser default
/// filters (with mipmaps when the node can mipmap); repeatS / repeatT / repeatR.
fn finish_3d(
    sg: &Sg,
    n: NodeId,
    width: u32,
    height: u32,
    depth: u32,
    rgba: Vec<u8>,
    transparent: bool,
    can_mipmap: bool,
) -> TextureImage {
    let (repeat_s, repeat_t, repeat_r) = (
        sg.bool(n, "repeatS", false),
        sg.bool(n, "repeatT", false),
        sg.bool(n, "repeatR", false),
    );
    let mut t = finish(
        width,
        height,
        rgba,
        false,
        repeat_s,
        repeat_t,
        String::new(),
        can_mipmap,
    );
    t.volume = true;
    t.depth = depth;
    t.transparent = transparent;
    t.sampler.wrap_r = if repeat_r {
        Wrap::Repeat
    } else {
        Wrap::ClampToEdge
    };
    apply_texture_properties(&mut t, sg, n, can_mipmap);
    t
}

/// PixelTexture3D.set_image__: image = [components, width, height, depth, pixels…]; fewer than 4
/// values or components outside 1…4 clear the texture. LUMINANCE (1) and LUMINANCE_ALPHA (2) are
/// sampled as (L, L, L, 1) and (L, L, L, A); missing pixels are 0. setTransparent (components
/// even). An invalid size (negative, beyond MAX_3D_TEXTURE_SIZE) clears; a zero size leaves an
/// incomplete texture. Never mipmapped (PixelTexture3D does not set mipMaps).
pub fn pixel_texture_3d(sg: &Sg, n: NodeId) -> TextureImage {
    let image = match sg.value(n, "image") {
        Some(FieldValue::MFInt32(v)) => v.clone(),
        _ => vec![],
    };

    if image.len() < 4 {
        return white_3d();
    }

    let (components, width, height, depth) = (image[0], image[1], image[2], image[3]);

    if !(1..=4).contains(&components) {
        return white_3d();
    }

    // `new Uint8Array (size3D)` throws for a negative size; setTextureData throws above the limit.
    let size = width as i64 * height as i64 * depth as i64;

    if width < 0
        || height < 0
        || depth < 0
        || size < 0
        || [width, height, depth]
            .iter()
            .any(|&d| d > MAX_3D_TEXTURE_SIZE)
    {
        return white_3d();
    }

    if size == 0 {
        return incomplete_3d();
    }

    let mut rgba = Vec::with_capacity(size as usize * 4);

    for i in 0..size as usize {
        let p = *image.get(4 + i).unwrap_or(&0) as u32;
        let px = match components {
            1 => [p as u8, p as u8, p as u8, 255],
            2 => [(p >> 8) as u8, (p >> 8) as u8, (p >> 8) as u8, p as u8],
            3 => [(p >> 16) as u8, (p >> 8) as u8, p as u8, 255],
            _ => [(p >> 24) as u8, (p >> 16) as u8, (p >> 8) as u8, p as u8],
        };
        rgba.extend_from_slice(&px);
    }

    finish_3d(
        sg,
        n,
        width as u32,
        height as u32,
        depth as u32,
        rgba,
        components % 2 == 0,
        false,
    )
}

/// gl.MAX_3D_TEXTURE_SIZE of the oracle's GPU (NVIDIA under ANGLE).
pub const MAX_3D_TEXTURE_SIZE: i32 = 2048;

/// ComposedTexture3D.update: the X3DTexture2DNodes of `texture` as slices when all are COMPLETE
/// (`slices`: node and its image, `None` while not complete), the size of the first; a slice of
/// another size warns and stays transparent black. An ImageTexture or MovieTexture slice (texture
/// type 1) is copied with its rows flipped. Transparent if any slice is; mipmaps if every slice
/// can (ImageTexture). Otherwise the cleared texture.
pub fn composed_texture_3d(
    sg: &Sg,
    n: NodeId,
    slices: &[(NodeId, Option<std::rc::Rc<TextureImage>>)],
    warn: &mut dyn FnMut(&str),
) -> TextureImage {
    if slices.is_empty() || slices.iter().any(|(_, t)| t.is_none()) {
        return white_3d();
    }

    let first = slices[0].1.as_ref().unwrap();
    let (width, height, depth) = (first.width, first.height, slices.len() as u32);
    let slice_bytes = (width * height * 4) as usize;
    let mut rgba = vec![0u8; slice_bytes * depth as usize];

    for (i, (node, t)) in slices.iter().enumerate() {
        let t = t.as_ref().unwrap();

        if t.width != width || t.height != height {
            warn("ComposedTexture3D: all textures must have same size.");
            continue;
        }

        // A RenderedTexture slice lives on the GPU only (not copied: transparent black).
        if t.rgba.len() < slice_bytes {
            continue;
        }

        let dst = &mut rgba[i * slice_bytes..(i + 1) * slice_bytes];

        if matches!(sg.type_name(*node), Some("ImageTexture" | "MovieTexture")) {
            let row = (width * 4) as usize;
            for (y, src) in t.rgba.chunks_exact(row).enumerate() {
                let to = height as usize - y - 1;
                dst[to * row..(to + 1) * row].copy_from_slice(src);
            }
        } else {
            dst.copy_from_slice(&t.rgba[..slice_bytes]);
        }
    }

    let transparent = slices.iter().any(|(_, t)| t.as_ref().unwrap().transparent);
    let can_mipmap = slices
        .iter()
        .all(|(node, _)| sg.type_name(*node) == Some("ImageTexture"));

    finish_3d(sg, n, width, height, depth, rgba, transparent, can_mipmap)
}

/// ImageTextureAtlas.setImage: the first url that loads and decodes, cut into slicesOverX ×
/// slicesOverY tiles of floor (w / slicesOverX) × floor (h / slicesOverY) texels, read from the
/// top left in rows, min (numberOfSlices, slicesOverX · slicesOverY) of them; each tile keeps the
/// image's row order (row 0 = top). Transparent as X_ITE tests it (see `atlas_tiles`); mipmaps. A zero size
/// leaves an incomplete texture. Returns the texture and the outputs (width, height, depth,
/// colorDepth).
pub fn image_texture_atlas(
    sg: &Sg,
    n: NodeId,
    data: &[u8],
    [over_x, over_y, slices]: [i32; 3],
) -> Option<(TextureImage, [i32; 4])> {
    let img = image::load_from_memory(data).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let (width, height, depth, rgba, transparent) =
        x3d_runtime::nodes::texture_projector::atlas_tiles(
            img.as_raw(),
            w,
            h,
            over_x,
            over_y,
            slices,
        );

    if width == 0 || height == 0 || depth == 0 {
        return Some((
            incomplete_3d(),
            [width as i32, height as i32, depth as i32, 3],
        ));
    }

    let t = finish_3d(sg, n, width, height, depth, rgba, transparent, true);

    Some((
        t,
        [
            width as i32,
            height as i32,
            depth as i32,
            if transparent { 4 } else { 3 },
        ],
    ))
}

/// gl.generateMipmap of a TEXTURE_3D: level sizes floor (n / 2) in each dimension, the same taps
/// as [`mip_chain`] in r too. Returns (width, height, depth, rgba) per level.
pub fn mip_chain_3d(t: &TextureImage) -> Vec<(u32, u32, u32, Vec<u8>)> {
    let mut levels = vec![(t.width, t.height, t.depth, t.rgba.clone())];

    if t.nearest || t.no_mipmaps {
        return levels;
    }

    loop {
        let (w, h, d, ref src) = *levels.last().unwrap();
        if w == 1 && h == 1 && d == 1 {
            break;
        }
        let (nw, nh, nd) = ((w / 2).max(1), (h / 2).max(1), (d / 2).max(1));
        let mut dst = vec![0u8; (nw * nh * nd * 4) as usize];

        for z in 0..nd {
            let tz = taps(d, z);
            for y in 0..nh {
                let ty = taps(h, y);
                for x in 0..nw {
                    let tx = taps(w, x);
                    for c in 0..4 {
                        let mut sum = 0.0;
                        for &(sz, wz) in &tz {
                            for &(sy, wy) in &ty {
                                for &(sx, wx) in &tx {
                                    sum += wx
                                        * wy
                                        * wz
                                        * src[(((sz * h + sy) * w + sx) * 4 + c) as usize] as f64;
                                }
                            }
                        }
                        dst[(((z * nh + y) * nw + x) * 4 + c) as usize] =
                            sum.round().clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }

        levels.push((nw, nh, nd, dst));
    }

    levels
}

/// ImageTextureAtlas.loadData: FileLoader URL fallback like ImageTexture, sliced as when the
/// runtime's load ended (a later slicesOverX … change waits for the next load); `None` while the
/// runtime shows the cleared texture.
pub fn load_image_texture_atlas(
    sg: &Sg,
    n: NodeId,
    base: &str,
    fetcher: &dyn x3d_runtime::Fetcher,
) -> Option<TextureImage> {
    let slicing = sg.rt.atlas_slicing(n)?;

    let base = url::Url::parse(base).ok()?;
    let urls = sg.rt.texture_url(n).unwrap_or_else(|| sg.strings(n, "url"));

    for candidate in urls {
        if candidate.is_empty() {
            continue;
        }
        let Ok(resolved) = base.join(&candidate) else {
            continue;
        };
        let Ok(data) = fetcher.fetch(&resolved) else {
            continue;
        };
        if let Some((mut t, _)) = image_texture_atlas(sg, n, &data, slicing) {
            t.url = resolved.to_string();
            return Some(t);
        }
    }

    None
}

/// RenderedTexture: set_dimensions__ makes a width × height texture the renderer draws into
/// ([`crate::traverse::RenderedPass`]; transparent black until the first renderTexture), or
/// clears it (1 × 1 white) without a framebuffer. Texture type 2 (not flipped); X3DTexture2DNode
/// texture parameters without mipmaps (the node never sets mipMaps); `transparent`: setTransparent
/// of the last renderTexture.
pub fn rendered_texture(sg: &Sg, n: NodeId, transparent: bool) -> TextureImage {
    let (width, height) = (sg.i32(n, "width", 128), sg.i32(n, "height", 128));

    if width <= 0 || height <= 0 {
        let mut t = TextureImage::white();
        t.flip_y = false;
        return t;
    }

    let mut t = finish(
        width as u32,
        height as u32,
        vec![],
        false,
        sg.bool(n, "repeatS", true),
        sg.bool(n, "repeatT", true),
        String::new(),
        false,
    );
    apply_texture_properties(&mut t, sg, n, false);
    t.generated = Some((sg.rt.uid, n));
    t.float = sg.bool(n, "depthMap", false);
    t.transparent = transparent;
    t
}

/// ImageTexture3D.loadData → X3DTexture3DNode.setTextureData: the parsed volume (NRRDParser /
/// DICOMParser: 8 bit samples, `components` per texel) as a `width` × `height` × `depth` texture.
/// LUMINANCE (1) and LUMINANCE_ALPHA (2) are sampled as (L, L, L, 1) and (L, L, L, A); a component
/// count outside 1…4 gives `texImage3D` an undefined format (INVALID_ENUM: no level 0) and a zero
/// size an incomplete texture, sampled (0, 0, 0, 1). No data (an NRRD without `encoding`) is null:
/// zeros. setTransparent (components even; NaN counts as even), mipmaps (ImageTexture3D
/// `setMipMaps (true)`).
pub fn image_texture_3d(
    sg: &Sg,
    n: NodeId,
    v: &x3d_runtime::nodes::image_texture_3d::Volume,
) -> TextureImage {
    let int = |x: f64| if x.is_nan() { 0.0 } else { x.trunc() };
    let (width, height, depth, components) =
        (int(v.width), int(v.height), int(v.depth), v.components);
    let transparent = components.is_nan() || (components as i64) & 1 == 0;
    let incomplete = || {
        let mut t = incomplete_3d();
        t.transparent = transparent;
        t
    };

    if width < 0.0 || height < 0.0 || depth < 0.0 || ![1.0, 2.0, 3.0, 4.0].contains(&components) {
        return incomplete();
    }

    let size = width * height * depth;

    if size == 0.0 {
        return incomplete();
    }

    let c = components as usize;
    let texels = size as usize;
    let zeros;
    let data: &[u8] = match &v.data {
        Some(d) => d,
        None => {
            zeros = vec![0u8; texels * c];
            &zeros
        }
    };
    let mut rgba = Vec::with_capacity(texels * 4);

    for i in 0..texels {
        let px = |k: usize| data.get(i * c + k).copied().unwrap_or(0);

        match c {
            1 => rgba.extend_from_slice(&[px(0), px(0), px(0), 255]),
            2 => rgba.extend_from_slice(&[px(0), px(0), px(0), px(1)]),
            3 => rgba.extend_from_slice(&[px(0), px(1), px(2), 255]),
            _ => rgba.extend_from_slice(&[px(0), px(1), px(2), px(3)]),
        }
    }

    finish_3d(
        sg,
        n,
        width as u32,
        height as u32,
        depth as u32,
        rgba,
        transparent,
        true,
    )
}
