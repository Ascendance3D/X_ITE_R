//! Image comparison against the render oracle.
//!
//! Only RGB is compared: the oracle reads the WebGL drawing buffer of an `alpha: true`,
//! premultiplied context through ANGLE, and every pixel comes back with alpha 255 (measured on
//! all baseline images), so the oracle carries no alpha information.

use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub width: u32,
    pub height: u32,
    /// Largest absolute channel difference (0–255) over R, G, B.
    pub max_delta: u8,
    /// Pixels where any RGB channel differs by more than `threshold`.
    pub pixels_over: usize,
    pub pct_over: f64,
    pub threshold: u8,
    /// Pixels with any difference at all.
    pub pixels_diff: usize,
    pub mean_abs: f64,
    /// PSNR over RGB in dB (infinite for identical images).
    pub psnr: f64,
    /// Mean SSIM of the luma channel over 8×8 windows (stride 4).
    pub ssim: f64,
}

pub fn load_png(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    let img = image::open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .to_rgba8();
    let (w, h) = img.dimensions();
    Ok((w, h, img.into_raw()))
}

fn luma(p: &[u8]) -> f64 {
    0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64
}

pub fn compare(w: u32, h: u32, a: &[u8], b: &[u8], threshold: u8) -> Stats {
    let n = (w * h) as usize;
    let mut s = Stats {
        width: w,
        height: h,
        threshold,
        ..Default::default()
    };
    let mut sq = 0.0f64;
    let mut abs = 0.0f64;

    for i in 0..n {
        let mut any = false;
        let mut over = false;
        for c in 0..3 {
            let d = (a[i * 4 + c] as i32 - b[i * 4 + c] as i32).unsigned_abs() as u8;
            s.max_delta = s.max_delta.max(d);
            any |= d > 0;
            over |= d > threshold;
            sq += (d as f64) * (d as f64);
            abs += d as f64;
        }
        s.pixels_diff += any as usize;
        s.pixels_over += over as usize;
    }

    let mse = sq / (3 * n) as f64;
    s.mean_abs = abs / (3 * n) as f64;
    s.pct_over = 100.0 * s.pixels_over as f64 / n as f64;
    s.psnr = if mse == 0.0 {
        f64::INFINITY
    } else {
        10.0 * (255.0f64 * 255.0 / mse).log10()
    };
    s.ssim = ssim(w, h, a, b);
    s
}

fn ssim(w: u32, h: u32, a: &[u8], b: &[u8]) -> f64 {
    let (c1, c2) = ((0.01f64 * 255.0).powi(2), (0.03f64 * 255.0).powi(2));
    let (w, h) = (w as usize, h as usize);
    let mut total = 0.0;
    let mut count = 0usize;
    let win = 8;

    let mut y = 0;
    while y + win <= h {
        let mut x = 0;
        while x + win <= w {
            let (mut ma, mut mb, mut va, mut vb, mut cov) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for yy in y..y + win {
                for xx in x..x + win {
                    let i = (yy * w + xx) * 4;
                    ma += luma(&a[i..]);
                    mb += luma(&b[i..]);
                }
            }
            let k = (win * win) as f64;
            ma /= k;
            mb /= k;
            for yy in y..y + win {
                for xx in x..x + win {
                    let i = (yy * w + xx) * 4;
                    let (da, db) = (luma(&a[i..]) - ma, luma(&b[i..]) - mb);
                    va += da * da;
                    vb += db * db;
                    cov += da * db;
                }
            }
            va /= k - 1.0;
            vb /= k - 1.0;
            cov /= k - 1.0;
            total += ((2.0 * ma * mb + c1) * (2.0 * cov + c2))
                / ((ma * ma + mb * mb + c1) * (va + vb + c2));
            count += 1;
            x += 4;
        }
        y += 4;
    }

    if count == 0 {
        1.0
    } else {
        total / count as f64
    }
}

/// Side by side: oracle | rust | difference (×8, red where over threshold).
pub fn diff_image(w: u32, h: u32, a: &[u8], b: &[u8], threshold: u8) -> Vec<u8> {
    let ow = w * 3;
    let mut out = vec![0u8; (ow * h * 4) as usize];

    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let row = (y * ow) as usize;
            for c in 0..4 {
                out[(row + x as usize) * 4 + c] = if c == 3 { 255 } else { a[i + c] };
                out[(row + (w + x) as usize) * 4 + c] = if c == 3 { 255 } else { b[i + c] };
            }
            let d: Vec<i32> = (0..3)
                .map(|c| (a[i + c] as i32 - b[i + c] as i32).abs())
                .collect();
            let m = *d.iter().max().unwrap();
            let g = (m * 8).min(255) as u8;
            let px: [u8; 4] = if m > threshold as i32 {
                [255, (m * 4).min(255) as u8, 0, 255]
            } else {
                [g, g, g, 255]
            };
            let o = (row + (2 * w + x) as usize) * 4;
            out[o..o + 4].copy_from_slice(&px);
        }
    }

    out
}
