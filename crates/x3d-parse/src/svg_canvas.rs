//! The canvas 2D part of SVGParser.js `drawGradient`: a `CanvasGradient` filled over a square
//! canvas with a transform, then `canvas .toDataURL ("image/png")`.
//!
//! The browser paints this with Skia. This file follows Skia's CPU pipeline for gradients
//! (clamp tiling; stops as constant / interpolated intervals; unpremultiplied interpolation then
//! premultiply; the 8x8 ordered dither of the raster pipeline; round to 8 bits; PNG export
//! unpremultiplies) with 32-bit floats. It reproduces the pixels of the pinned page to the limit
//! recorded in the lane report; the PNG file bytes differ (a different deflate), so the scene text
//! cannot be equal there.

use std::io::Write;

pub enum GradientKind {
    Linear { x1: f64, y1: f64, x2: f64, y2: f64 },
    Radial { fx: f64, fy: f64, fr: f64, cx: f64, cy: f64, r: f64 },
}

/// One interval of Skia's `gradient` stage: for `t >= ts [i]` the color is `t * f [i] + b [i]`.
struct Stops {
    ts: Vec<f32>,
    f: Vec<[f32; 4]>,
    b: Vec<[f32; 4]>,
}

/// SkGradientShader with clamp tiling for the canvas stops `(offset, [r, g, b, a])` (r, g, b in
/// 0..255, a in 0..1), already in the order `addColorStop` keeps them.
fn prepare(stops: &[(f64, [f64; 4])]) -> Stops {
    // CanvasGradient keeps stops sorted by offset, equal offsets in insertion order.
    let mut sorted: Vec<(f64, [f64; 4])> = stops.to_vec();

    sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // The CSS parser gives 8-bit channels, alpha included (SkColor).
    let color = |c: &[f64; 4]| -> [f32; 4] {
        let q = |v: f64| (v.clamp(0.0, 255.0).round() / 255.0) as f32;

        [q(c[0]), q(c[1]), q(c[2]), (c[3].clamp(0.0, 1.0) * 255.0).round() as f32 / 255.0]
    };

    let mut pos: Vec<f32> = sorted.iter().map(|s| s.0 as f32).collect();
    let mut colors: Vec<[f32; 4]> = sorted.iter().map(|s| color(&s.1)).collect();

    // Skia adds a stop at 0 and at 1 when the first and last position are not there.
    if pos[0] != 0.0 {
        pos.insert(0, 0.0);
        colors.insert(0, colors[0]);
    }

    if *pos.last().unwrap() != 1.0 {
        pos.push(1.0);
        colors.push(*colors.last().unwrap());
    }

    let mut out = Stops { ts: vec![], f: vec![], b: vec![] };
    let mut t_l = pos[0];
    let mut c_l = colors[0];

    // Index 0 is the color before the first stop.
    out.ts.push(0.0);
    out.f.push([0.0; 4]);
    out.b.push(c_l);

    for i in 0..pos.len() - 1 {
        let t_r = pos[i + 1];
        let c_r = colors[i + 1];

        if t_l < t_r {
            let mut f = [0.0f32; 4];
            let mut b = [0.0f32; 4];

            for k in 0..4 {
                f[k] = (c_r[k] - c_l[k]) / (t_r - t_l);
                b[k] = c_l[k] - f[k] * t_l;
            }

            out.ts.push(t_l);
            out.f.push(f);
            out.b.push(b);
        }

        t_l = t_r;
        c_l = c_r;
    }

    // After the last stop.
    out.ts.push(t_l);
    out.f.push([0.0; 4]);
    out.b.push(c_l);
    out
}

impl Stops {
    /// The `gradient` stage: unpremultiplied color for `t`.
    fn color(&self, t: f32) -> [f32; 4] {
        let mut idx = 0;

        for i in 1..self.ts.len() {
            if t >= self.ts[i] {
                idx += 1;
            }
        }

        let (f, b) = (&self.f[idx], &self.b[idx]);

        [t * f[0] + b[0], t * f[1] + b[1], t * f[2] + b[2], t * f[3] + b[3]]
    }
}


// --- the matrix chain of Skia's gradient shaders ------------------------------------------------
//
// SkMatrix is 32-bit with double intermediates in concat and invert. The shader's device to
// gradient-unit matrix is `Concat (ptsToUnit, inverse (ctm))`; the pipeline then evaluates
// `x * sx + (y * kx + tx)` per pixel centre. The order and the rounding matter for values that
// land exactly between two 8-bit levels.

/// SkMatrix [scaleX, skewX, transX, skewY, scaleY, transY] (affine only).
#[derive(Clone, Copy, Debug)]
struct SkM([f32; 6]);

fn muladdmul(a: f32, b: f32, c: f32, d: f32) -> f32 {
    (a as f64 * b as f64 + c as f64 * d as f64) as f32
}

impl SkM {
    const IDENTITY: SkM = SkM([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);

    fn concat(a: &SkM, b: &SkM) -> SkM {
        let (a, b) = (&a.0, &b.0);

        SkM([
            muladdmul(a[0], b[0], a[1], b[3]),
            muladdmul(a[0], b[1], a[1], b[4]),
            muladdmul(a[0], b[2], a[1], b[5]) + a[2],
            muladdmul(a[3], b[0], a[4], b[3]),
            muladdmul(a[3], b[1], a[4], b[4]),
            muladdmul(a[3], b[2], a[4], b[5]) + a[5],
        ])
    }

    fn scale(sx: f32, sy: f32) -> SkM {
        SkM([sx, 0.0, 0.0, 0.0, sy, 0.0])
    }

    fn post_scale(&mut self, sx: f32, sy: f32) {
        *self = SkM::concat(&SkM::scale(sx, sy), self);
    }

    fn post_translate(&mut self, dx: f32, dy: f32) {
        self.0[2] += dx;
        self.0[5] += dy;
    }

    /// SkMatrix::invert.
    fn invert(&self) -> Option<SkM> {
        let m = &self.0;

        if m[1] == 0.0 && m[3] == 0.0 {
            // Scale and translate.
            if m[0] == 0.0 || m[4] == 0.0 {
                return None;
            }

            let (ix, iy) = (1.0 / m[0], 1.0 / m[4]);

            return Some(SkM([ix, 0.0, -m[2] * ix, 0.0, iy, -m[5] * iy]));
        }

        let det = m[0] as f64 * m[4] as f64 - m[1] as f64 * m[3] as f64;
        let inv_det = 1.0 / det;

        if det == 0.0 || !inv_det.is_finite() {
            return None;
        }

        let cross = |a: f32, b: f32, c: f32, d: f32| ((a as f64 * b as f64 - c as f64 * d as f64) * inv_det) as f32;

        Some(SkM([
            (m[4] as f64 * inv_det) as f32,
            (-(m[1] as f64) * inv_det) as f32,
            cross(m[1], m[5], m[4], m[2]),
            (-(m[3] as f64) * inv_det) as f32,
            (m[0] as f64 * inv_det) as f32,
            cross(m[3], m[2], m[0], m[5]),
        ]))
    }

    /// The pipeline's `matrix_2x3` stage.
    fn map(&self, x: f32, y: f32) -> (f32, f32) {
        let m = &self.0;

        (x.mul_add(m[0], y.mul_add(m[1], m[2])), x.mul_add(m[3], y.mul_add(m[4], m[5])))
    }
}

/// SkLinearGradient::MapPtsToUnit.
fn linear_points_to_unit(x1: f32, y1: f32, x2: f32, y2: f32) -> SkM {
    let (mut vx, mut vy) = (x2 - x1, y2 - y1);
    let mag = (vx * vx + vy * vy).sqrt();
    let inv = if mag != 0.0 { 1.0 / mag } else { 0.0 };

    vx *= inv;
    vy *= inv;

    // setSinCos (-vy, vx, x1, y1)
    let (sin, cos) = (-vy, vx);
    let one_minus_cos = 1.0 - cos;
    let mut m = SkM([cos, -sin, sin * y1 + one_minus_cos * x1, sin, cos, -sin * x1 + one_minus_cos * y1]);

    m.post_translate(-x1, -y1);
    m.post_scale(inv, inv);
    m
}

/// Paints the gradient over the `size` x `size` canvas with the transform `ctm` = (a, b, c, d, e, f)
/// of `CanvasRenderingContext2D.transform`. Returns unpremultiplied RGBA bytes, or the message of
/// the exception the canvas API throws.
pub fn paint(size: usize, kind: &GradientKind, stops: &[(f64, [f64; 4])], ctm: [f64; 6]) -> Result<Vec<u8>, String> {
    // createLinearGradient / createRadialGradient
    match kind {
        GradientKind::Linear { x1, y1, x2, y2 } => {
            if ![x1, y1, x2, y2].iter().all(|v| v.is_finite()) {
                return Err("TypeError: Failed to execute 'createLinearGradient' on 'CanvasRenderingContext2D': The provided double value is non-finite.".into());
            }
        }
        GradientKind::Radial { fx, fy, fr, cx, cy, r } => {
            if ![fx, fy, fr, cx, cy, r].iter().all(|v| v.is_finite()) {
                return Err("TypeError: Failed to execute 'createRadialGradient' on 'CanvasRenderingContext2D': The provided double value is non-finite.".into());
            }

            if *fr < 0.0 {
                return Err("IndexSizeError: Failed to execute 'createRadialGradient' on 'CanvasRenderingContext2D': The r0 provided is less than 0.".to_string());
            }

            if *r < 0.0 {
                return Err("IndexSizeError: Failed to execute 'createRadialGradient' on 'CanvasRenderingContext2D': The r1 provided is less than 0.".to_string());
            }
        }
    }

    // addColorStop
    for (offset, _) in stops {
        if !(0.0..=1.0).contains(offset) {
            return Err(format!(
                "IndexSizeError: Failed to execute 'addColorStop' on 'CanvasGradient': The provided value ({}) is outside the range (0.0, 1.0).",
                x3d_fields::jsmath::number_to_string(*offset)
            ));
        }
    }

    let mut rgba = vec![0u8; size * size * 4];

    // clearRect, then transform (non-finite arguments are ignored: the transform stays the identity).
    let ctm = if ctm.iter().all(|v| v.is_finite()) { ctm } else { [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] };
    let [a, b, c, d, e, f] = ctm;
    let det = a * d - b * c;

    // A transform that cannot be inverted paints nothing.
    if det == 0.0 || !det.is_finite() || stops.is_empty() {
        return Ok(rgba);
    }

    let stops = prepare(stops);

    // Device pixel to gradient space.
    let inv = [d / det, -b / det, -c / det, a / det, (c * f - d * e) / det, (b * e - a * f) / det];
    let map = |x: f64, y: f64| (inv[0] * x + inv[2] * y + inv[4], inv[1] * x + inv[3] * y + inv[5]);

    // Linear: t is the projection on the vector p0 -> p1, as Skia's points-to-unit matrix gives it.
    let linear = match kind {
        GradientKind::Linear { x1, y1, x2, y2 } => {
            let (dx, dy) = (x2 - x1, y2 - y1);
            let len2 = dx * dx + dy * dy;

            Some((*x1, *y1, dx, dy, len2))
        }
        _ => None,
    };

    // Canvas specification (and Chrome): a linear gradient whose points coincide paints nothing.
    if let Some((_, _, _, _, len2)) = linear {
        if len2 == 0.0 {
            return Ok(rgba);
        }
    }

    // device -> unit space of the linear gradient, 32-bit as Skia builds it
    let linear_total = match kind {
        GradientKind::Linear { x1, y1, x2, y2 } => {
            let ctm32 = SkM([a as f32, c as f32, e as f32, b as f32, d as f32, f as f32]);

            ctm32.invert().map(|inv32| SkM::concat(&linear_points_to_unit(*x1 as f32, *y1 as f32, *x2 as f32, *y2 as f32), &inv32))
        }
        _ => None,
    };

    if matches!(kind, GradientKind::Linear { .. }) && linear_total.is_none() {
        return Ok(rgba);
    }

    for y in 0..size {
        for x in 0..size {
            let (ux, uy) = map(x as f64 + 0.5, y as f64 + 0.5);

            let t = match (&linear, kind) {
                (Some(_), _) => {
                    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);

                    Some(linear_total.unwrap().map(fx, fy).0)
                }
                (None, GradientKind::Radial { fx, fy, fr, cx, cy, r }) => two_point_conical(ux, uy, *fx, *fy, *fr, *cx, *cy, *r),
                _ => None,
            };

            match t {
                Some(t) => {
                    let color = stops.color(t);

                    store(&mut rgba, size, x, y, color);
                }
                None => {
                    // Nothing is painted where no circle of the cone covers the point.
                }
            }
        }
    }

    Ok(finish(rgba))
}

/// The two-point conical gradient of the canvas specification: the largest `w` with a
/// non-negative radius whose circle passes through the point.
#[allow(clippy::too_many_arguments)]
fn two_point_conical(x: f64, y: f64, fx: f64, fy: f64, fr: f64, cx: f64, cy: f64, r: f64) -> Option<f32> {
    let (cdx, cdy, dr) = (cx - fx, cy - fy, r - fr);
    let (pdx, pdy) = (x - fx, y - fy);
    let a = cdx * cdx + cdy * cdy - dr * dr;
    let b = pdx * cdx + pdy * cdy + fr * dr;
    let c = pdx * pdx + pdy * pdy - fr * fr;
    let valid = |w: f64| fr + w * dr >= 0.0;

    if a == 0.0 {
        if b == 0.0 {
            return None;
        }

        let w = c / (2.0 * b);

        return valid(w).then_some(w as f32);
    }

    let disc = b * b - a * c;

    if disc < 0.0 {
        return None;
    }

    let root = disc.sqrt();
    let (w1, w2) = ((b + root) / a, (b - root) / a);
    let (hi, lo) = if w1 > w2 { (w1, w2) } else { (w2, w1) };

    if valid(hi) {
        Some(hi as f32)
    } else if valid(lo) {
        Some(lo as f32)
    } else {
        None
    }
}

/// premul, the raster pipeline's 8x8 ordered dither, round to 8 bits: stored premultiplied.
fn store(rgba: &mut [u8], size: usize, x: usize, y: usize, color: [f32; 4]) {
    let a = color[3].clamp(0.0, 1.0);
    let mut c = [color[0] * a, color[1] * a, color[2] * a];

    // SkRasterPipeline dither stage, rate = 1/255.
    let (xi, yi) = (x as u32, (y as u32) ^ (x as u32));
    let m = (yi & 1) << 5 | (xi & 1) << 4 | (yi & 2) << 2 | (xi & 2) << 1 | (yi & 4) >> 1 | (xi & 4) >> 2;
    let dither = m as f32 * (2.0 / 128.0) - (63.0 / 128.0);
    let rate = 1.0f32 / 255.0;

    for v in &mut c {
        // The stage's `mad (dither, rate, r)` is a fused multiply-add on the CPUs that matter.
        *v = dither.mul_add(rate, *v);
        *v = v.clamp(0.0, a);
    }

    let o = (y * size + x) * 4;
    // to_unorm: `v * 255` rounded to nearest (cvtps_epi32, ties to even).
    let unorm = |v: f32| (v * 255.0).round_ties_even() as u8;

    rgba[o] = unorm(c[0]);
    rgba[o + 1] = unorm(c[1]);
    rgba[o + 2] = unorm(c[2]);
    rgba[o + 3] = unorm(a);
}

/// The canvas holds premultiplied pixels; `toDataURL` writes them unpremultiplied.
fn finish(mut rgba: Vec<u8>) -> Vec<u8> {
    for px in rgba.chunks_exact_mut(4) {
        let a = px[3];

        if a == 0 {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
        } else if a != 255 {
            // The raster pipeline's unpremul: bytes to floats (`* 1/255`), `* 1/a`, back to bytes.
            let fa = a as f32 * (1.0 / 255.0);
            let scale = 1.0 / fa;

            for v in &mut px[..3] {
                let unpremul = *v as f32 * (1.0 / 255.0) * scale;

                *v = (unpremul * 255.0).round_ties_even().min(255.0) as u8;
            }
        }
    }

    rgba
}

// --- PNG and base64 ----------------------------------------------------------------------------

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;

    for &byte in data {
        crc ^= byte as u32;

        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }

    !crc
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());

    let start = out.len();

    out.extend_from_slice(kind);
    out.extend_from_slice(data);

    let crc = crc32(&out[start..]);

    out.extend_from_slice(&crc.to_be_bytes());
}

/// An 8-bit RGBA PNG of a `size` x `size` image.
fn png(size: usize, rgba: &[u8]) -> Vec<u8> {
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let mut header = vec![];

    header.extend_from_slice(&(size as u32).to_be_bytes());
    header.extend_from_slice(&(size as u32).to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header);

    // Sub filter on every row.
    let stride = size * 4;
    let mut raw = Vec::with_capacity((stride + 1) * size);

    for row in rgba.chunks_exact(stride) {
        raw.push(1);

        for i in 0..stride {
            let left = if i >= 4 { row[i - 4] } else { 0 };

            raw.push(row[i].wrapping_sub(left));
        }
    }

    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());

    encoder.write_all(&raw).expect("in-memory write");
    chunk(&mut out, b"IDAT", &encoder.finish().expect("in-memory finish"));
    chunk(&mut out, b"IEND", &[]);
    out
}

fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);

    for chunk in data.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;

        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }

    out
}

/// `canvas .toDataURL ("image/png")`
pub fn png_data_url(size: usize, rgba: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64(&png(size, rgba)))
}
