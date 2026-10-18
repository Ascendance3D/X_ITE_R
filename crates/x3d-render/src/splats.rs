//! GaussianSplats (component GaussianSplats, X_ITE 16.5.0, experimental): the CPU side.
//!
//! X_ITE wraps the node's fields in a private `GaussianSplatsShape` (Browser/GaussianSplats/
//! GaussianSplatsShape.js): the fields go into float textures (positions and scales RGB32F,
//! orientations RGBA16F, opacities R16F, the spherical harmonics an RGB16F texture array), a
//! WebAssembly counting sort (assets/lib/mkkellogg-sort.worker.js, sorter_no_simd_non_shared.wasm)
//! orders the splat indices back to front, and one instanced quad per splat is drawn with
//! GaussianSplats.vs.js / GaussianSplats.fs.js. This module builds the draw data and ports the
//! sort; [`crate::splats_gpu`] and `splats.wgsl` draw it.
//!
//! Ported behavior (and quirks):
//! - `numSplats` is `positions.length`; shorter orientations, scales, opacities and spherical
//!   harmonics fields leave zeros (`Float32Array.set` of a shorter array).
//! - `degrees` is the highest spherical harmonics degree whose coefficient fields (and the ones of
//!   all lower degrees) are all non-empty, -1 if even degree 0 is missing. With no degree the
//!   texture keeps what an earlier rebuild put there (nothing on the first), and the shader still
//!   reads layer 0 of it: color 0.5 + C0 * layer0.
//! - orientations, opacities and the spherical harmonics are stored as half floats.
//! - the bounding box is the one of the positions, or `bboxSize` / `bboxCenter`.
//! - the sort key is the rotation-only z row of the model-view matrix applied to the position, in
//!   32 bit float arithmetic, scaled by 4096, truncated, and counted into 2^20 bins; ties keep the
//!   higher index first. Nothing is added for the translation.

use std::rc::Rc;
use x3d_fields::math::Matrix4;

/// Spherical harmonics coefficient counts per degree (GaussianSplatsShape.js SH_COEFS).
pub const SH_COEFS: [usize; 4] = [1, 3, 5, 7];

/// DIST_MAP_RANGE of the sort worker.
pub const DIST_MAP_RANGE: usize = 1 << 20;

/// The spherical harmonics texture array of a rebuild: `layers` layers of `width` x `width` texels
/// with three values each (stored as four per texel in `texels`).
#[derive(Debug, Default)]
pub struct ShTexture {
    pub layers: usize,
    pub width: usize,
    pub texels: Vec<f32>,
}

/// What GaussianSplatsShape.rebuild () uploads.
#[derive(Debug)]
pub struct SplatData {
    pub count: usize,
    /// textureWidth: ceil (sqrt (count)).
    pub width: usize,
    /// xyz per splat (RGB32F).
    pub positions: Vec<f32>,
    /// xyzw per splat (RGBA16F, rounded to half floats).
    pub orientations: Vec<f32>,
    /// xyz per splat (RGB32F).
    pub scales: Vec<f32>,
    /// One per splat (R16F, rounded to half floats).
    pub opacities: Vec<f32>,
    /// this.degrees: -1 .. 3.
    pub degrees: i32,
    pub sh: Rc<ShTexture>,
    /// The bounding box of the positions (set_bbox__ without bboxSize): size and center, `None`
    /// without positions.
    pub bbox: Option<([f64; 3], [f64; 3])>,
}

/// The value of `x` as a half float, rounded to nearest even (an overflow becomes infinity).
pub fn half_round(x: f32) -> f32 {
    half_to_f32(f32_to_half(x))
}

pub fn f32_to_half(x: f32) -> u16 {
    let bits = x.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xff) as i32;
    let mant = bits & 0x7f_ffff;

    if exp == 0xff {
        // Infinity or NaN.
        return sign | 0x7c00 | if mant != 0 { 0x200 } else { 0 };
    }

    let e = exp - 127 + 15;

    if e >= 0x1f {
        return sign | 0x7c00;
    }

    if e <= 0 {
        if e < -10 {
            return sign;
        }

        // Subnormal: the implicit one joins the mantissa.
        let m = mant | 0x80_0000;
        let shift = (14 - e) as u32;
        let half = m >> shift;
        let rest = m & ((1 << shift) - 1);
        let halfway = 1 << (shift - 1);
        let rounded = if rest > halfway || (rest == halfway && half & 1 == 1) { half + 1 } else { half };
        return sign | rounded as u16;
    }

    let half = ((e as u32) << 10) | (mant >> 13);
    let rest = mant & 0x1fff;
    let rounded = if rest > 0x1000 || (rest == 0x1000 && half & 1 == 1) { half + 1 } else { half };
    // A carry out of the mantissa raises the exponent, up to infinity.
    sign | rounded as u16
}

pub fn half_to_f32(h: u16) -> f32 {
    let sign = ((h & 0x8000) as u32) << 16;
    let exp = ((h >> 10) & 0x1f) as u32;
    let mant = (h & 0x3ff) as u32;

    let bits = if exp == 0 {
        if mant == 0 {
            sign
        } else {
            // Subnormal half: normalize.
            let mut m = mant;
            let mut e = 127 - 15 + 1;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            sign | ((e as u32) << 23) | ((m & 0x3ff) << 13)
        }
    } else if exp == 0x1f {
        sign | 0x7f80_0000 | (mant << 13)
    } else {
        sign | ((exp + 127 - 15) << 23) | (mant << 13)
    };

    f32::from_bits(bits)
}

/// GaussianSplatsShape.rebuild () and set_bbox__ () for the fields of a GaussianSplats node.
/// `previous` is the data of the earlier rebuild: the spherical harmonics texture survives a
/// rebuild without any degree.
pub struct SplatFields<'a> {
    pub positions: &'a [f32],
    pub orientations: &'a [f64],
    pub scales: &'a [f32],
    pub opacities: &'a [f32],
    /// The 16 coefficient fields in order (degree 0 coef 0, degree 1 coef 0..2, ...).
    pub sh: [&'a [f32]; 16],
}

pub fn build(f: &SplatFields, previous: Option<&SplatData>) -> SplatData {
    let count = f.positions.len() / 3;
    let width = (count as f64).sqrt().ceil() as usize;
    let mut bbox = None;

    if count > 0 {
        let (mut min, mut max) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);

        for p in f.positions.chunks_exact(3) {
            for k in 0..3 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }

        let size = [0, 1, 2].map(|k| max[k] as f64 - min[k] as f64);
        let center = [0, 1, 2].map(|k| (max[k] as f64 + min[k] as f64) / 2.0);
        bbox = Some((size, center));
    }

    let mut data = SplatData {
        count,
        width,
        positions: vec![],
        orientations: vec![],
        scales: vec![],
        opacities: vec![],
        degrees: previous.map_or(-1, |p| p.degrees),
        sh: previous.map_or_else(|| Rc::new(ShTexture::default()), |p| p.sh.clone()),
        bbox,
    };

    if width == 0 {
        return data;
    }

    let size = width * width;
    let take = |source: &[f32], per: usize| -> Vec<f32> {
        let mut v = vec![0.0f32; size * per];
        let n = source.len().min(count * per);
        v[..n].copy_from_slice(&source[..n]);
        v
    };

    data.positions = take(f.positions, 3);
    data.scales = take(f.scales, 3);
    data.opacities = take(f.opacities, 1).into_iter().map(half_round).collect();

    let mut orientations = vec![0.0f32; size * 4];

    for (o, q) in orientations.iter_mut().zip(f.orientations.iter().take(count * 4)) {
        *o = half_round(*q as f32);
    }

    data.orientations = orientations;

    // Spherical harmonic degrees MUST NOT be partially defined.
    let mut degrees = -1;
    let mut first = 0;

    for (degree, coefs) in SH_COEFS.iter().enumerate() {
        if !(0..*coefs).all(|c| !f.sh[first + c].is_empty()) {
            break;
        }

        degrees = degree as i32;
        first += coefs;
    }

    data.degrees = degrees;
    let dimensions = ((degrees + 1) * (degrees + 1)) as usize;

    if dimensions > 0 {
        let mut texels = vec![0.0f32; dimensions * size * 4];
        let mut layer = 0;
        let mut coef_index = 0;

        'degrees: for (degree, coefs) in SH_COEFS.iter().enumerate() {
            if degree as i32 > degrees {
                break;
            }

            for _ in 0..*coefs {
                let source = f.sh[coef_index];
                coef_index += 1;

                for i in 0..count.min(source.len() / 3) {
                    for k in 0..3 {
                        texels[(layer * size + i) * 4 + k] = half_round(source[i * 3 + k]);
                    }
                }

                layer += 1;

                if layer == dimensions {
                    break 'degrees;
                }
            }
        }

        data.sh = Rc::new(ShTexture { layers: dimensions, width, texels });
    }

    data
}

/// The fields of the GaussianSplats node `n`.
pub fn fields_of<'a>(sg: &'a crate::sg::Sg, n: x3d_fields::NodeId) -> SplatFields<'a> {
    const NAMES: [&str; 16] = [
        "sphericalHarmonicsDegree0Coef0",
        "sphericalHarmonicsDegree1Coef0",
        "sphericalHarmonicsDegree1Coef1",
        "sphericalHarmonicsDegree1Coef2",
        "sphericalHarmonicsDegree2Coef0",
        "sphericalHarmonicsDegree2Coef1",
        "sphericalHarmonicsDegree2Coef2",
        "sphericalHarmonicsDegree2Coef3",
        "sphericalHarmonicsDegree2Coef4",
        "sphericalHarmonicsDegree3Coef0",
        "sphericalHarmonicsDegree3Coef1",
        "sphericalHarmonicsDegree3Coef2",
        "sphericalHarmonicsDegree3Coef3",
        "sphericalHarmonicsDegree3Coef4",
        "sphericalHarmonicsDegree3Coef5",
        "sphericalHarmonicsDegree3Coef6",
    ];
    let mut sh: [&[f32]; 16] = [&[]; 16];

    for (slot, name) in sh.iter_mut().zip(NAMES) {
        *slot = sg.f32s(n, name);
    }

    let orientations: &[f64] = match sg.value(n, "orientations") {
        Some(x3d_fields::FieldValue::MFQuaternion(v)) => v,
        _ => &[],
    };

    SplatFields { positions: sg.f32s(n, "positions"), orientations, scales: sg.f32s(n, "scales"), opacities: sg.f32s(n, "opacities"), sh }
}

/// What a draw item carries: the data and where it is drawn.
#[derive(Clone, Debug)]
pub struct SplatDraw {
    pub node: x3d_fields::NodeId,
    pub data: Rc<SplatData>,
    pub model_view: Matrix4,
    /// colorSpace LIN_REC709_DISPLAY (X3D_COLORSPACE_LINEAR).
    pub linear: bool,
}

/// The working memory of the sort (the worker's WebAssembly memory).
#[derive(Default)]
pub struct SortScratch {
    mapped: Vec<i32>,
    frequencies: Vec<u32>,
}

/// mkkellogg-sort.worker.js `sort`: the splat indices ordered by `sortIndexes` of
/// sorter_no_simd_non_shared.wasm (float, no precomputed distances, not dynamic) for the view
/// matrix `model_view` (column-major float32), the z row negated as the worker does.
pub fn sort_indices(positions: &[f32], count: usize, model_view: &[f32; 16], scratch: &mut SortScratch) -> Vec<u32> {
    let m2 = -model_view[2];
    let m6 = -model_view[6];
    let m10 = -model_view[10];

    scratch.mapped.clear();
    scratch.mapped.resize(count, 0);

    let mut min_distance = 2147483640i32;
    let mut max_distance = -2147483640i32;

    for (i, mapped) in scratch.mapped.iter_mut().enumerate() {
        let (x, y, z) = (positions[i * 3], positions[i * 3 + 1], positions[i * 3 + 2]);
        let depth: f32 = (m10 * z) + ((m2 * x) + (m6 * y));
        let scaled = depth as f64 * 4096.0;
        let distance = if scaled.abs() < 2147483648.0 { scaled as i32 } else { i32::MIN };

        *mapped = distance;
        min_distance = min_distance.min(distance);
        max_distance = max_distance.max(distance);
    }

    scratch.frequencies.clear();
    scratch.frequencies.resize(DIST_MAP_RANGE, 0);

    let mut out = vec![0u32; count];

    if count == 0 {
        return out;
    }

    // `(distanceMapRange - 1) / (max - min)` in float32.
    let range_map = ((DIST_MAP_RANGE - 1) as u32 as f32) / ((max_distance as f32) - (min_distance as f32));

    for mapped in scratch.mapped.iter_mut() {
        let t = mapped.wrapping_sub(min_distance);
        let v = range_map * (t as f32);
        let index = if v.abs() < 2147483648.0 { v as i32 } else { i32::MIN };

        *mapped = index;
        scratch.frequencies[bin(index)] += 1;
    }

    let mut running = scratch.frequencies[0];

    for f in scratch.frequencies.iter_mut().skip(1) {
        *f += running;
        running = *f;
    }

    for j in (0..count).rev() {
        let b = bin(scratch.mapped[j]);
        let f = scratch.frequencies[b];

        out[count - f as usize] = j as u32;
        scratch.frequencies[b] = f - 1;
    }

    out
}

/// The slot of `frequencies` the address `frequencies + (index << 2)` of the 32 bit code names.
fn bin(index: i32) -> usize {
    // (index << 2) wraps in 32 bits; a valid index is below 2^20, INT_MIN wraps to 0.
    (((index as u32).wrapping_shl(2)) >> 2) as usize
}
