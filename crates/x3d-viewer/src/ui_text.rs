//! A software canvas for the native browser UI (ContextMenu, Notification): premultiplied RGBA8,
//! rectangles with rounded corners, and text set in X_ITE's default sans font (Ubuntu) with the
//! glyph outlines of `x3d_text` and its exact-area scan converter.

use std::collections::HashMap;
use std::rc::Rc;
use x3d_text::font::Cmd;
use x3d_text::{Font, fonts, raster};

/// The canvas: rows from the top, premultiplied alpha.
#[derive(Clone, Debug, PartialEq)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// A color with straight alpha (0 to 255).
pub type Color = [u8; 4];

impl Canvas {
    pub fn new(width: u32, height: u32) -> Canvas {
        Canvas {
            width,
            height,
            rgba: vec![0; (width * height * 4) as usize],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rgba.iter().all(|b| *b == 0)
    }

    /// The pixel at (x, y), premultiplied.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;

        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }

    /// Source-over of `color` with `coverage` (0 to 1) at one pixel.
    fn blend(&mut self, x: i64, y: i64, color: Color, coverage: f64) {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return;
        }

        let a = (color[3] as f64 / 255.0) * coverage.clamp(0.0, 1.0);

        if a <= 0.0 {
            return;
        }

        let i = ((y as u32 * self.width + x as u32) * 4) as usize;

        for c in 0..3 {
            let src = color[c] as f64 * a;
            let dst = self.rgba[i + c] as f64;

            self.rgba[i + c] = (src + dst * (1.0 - a)).round().min(255.0) as u8;
        }

        let dst = self.rgba[i + 3] as f64;

        self.rgba[i + 3] = (a * 255.0 + dst * (1.0 - a)).round().min(255.0) as u8;
    }

    /// A filled rectangle with the corner radii (top left, top right, bottom right, bottom left).
    pub fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, radii: [f64; 4], color: Color) {
        let (x0, y0) = (x.floor() as i64, y.floor() as i64);
        let (x1, y1) = ((x + w).ceil() as i64, (y + h).ceil() as i64);

        for py in y0..y1 {
            for px in x0..x1 {
                // Coverage of the pixel by the rectangle (exact on the straight edges).
                let cx = ((px as f64 + 1.0).min(x + w) - (px as f64).max(x)).clamp(0.0, 1.0);
                let cy = ((py as f64 + 1.0).min(y + h) - (py as f64).max(y)).clamp(0.0, 1.0);
                let mut cover = cx * cy;

                // Rounded corners: the distance of the pixel center to the corner circle.
                let (mx, my) = (px as f64 + 0.5, py as f64 + 0.5);

                for (i, r) in radii.iter().enumerate() {
                    if *r <= 0.0 {
                        continue;
                    }

                    let (ccx, ccy) = match i {
                        0 => (x + r, y + r),
                        1 => (x + w - r, y + r),
                        2 => (x + w - r, y + h - r),
                        _ => (x + r, y + h - r),
                    };
                    let inside_corner = match i {
                        0 => mx < ccx && my < ccy,
                        1 => mx > ccx && my < ccy,
                        2 => mx > ccx && my > ccy,
                        _ => mx < ccx && my > ccy,
                    };

                    if inside_corner {
                        let d = ((mx - ccx).powi(2) + (my - ccy).powi(2)).sqrt();

                        cover *= (r - d + 0.5).clamp(0.0, 1.0);
                    }
                }

                self.blend(px, py, color, cover);
            }
        }
    }

    /// A one pixel frame inside the rectangle.
    pub fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64, color: Color) {
        self.fill_rect(x, y, w, 1.0, [0.0; 4], color);
        self.fill_rect(x, y + h - 1.0, w, 1.0, [0.0; 4], color);
        self.fill_rect(x, y, 1.0, h, [0.0; 4], color);
        self.fill_rect(x + w - 1.0, y, 1.0, h, [0.0; 4], color);
    }
}

/// One rasterized glyph: its coverage and the offset of its top left corner from the pen position on the baseline.
struct Glyph {
    w: usize,
    h: usize,
    left: f64,
    top: f64,
    coverage: Vec<f64>,
}

/// The text renderer: the default sans font and a glyph cache per pixel size.
pub struct Text {
    font: Option<Rc<Font>>,
    cache: HashMap<(u16, u32), Rc<Glyph>>,
}

impl Default for Text {
    fn default() -> Text {
        Text {
            font: fonts::load_default_font("SANS", "PLAIN"),
            cache: HashMap::new(),
        }
    }
}

impl Text {
    /// The advance width of `text` in pixels at the font size `px`.
    pub fn width(&self, text: &str, px: f64) -> f64 {
        let Some(font) = &self.font else {
            return text.chars().count() as f64 * px * 0.5;
        };

        text.encode_utf16()
            .map(|u| font.advance_width(font.char_to_glyph(u)) / font.units_per_em * px)
            .sum()
    }

    /// The pixel height of a text line.
    pub fn line_height(&self, px: f64) -> f64 {
        match &self.font {
            Some(f) => (f.ascender - f.descender) / f.units_per_em * px,
            None => px * 1.2,
        }
    }

    /// The distance from the top of a line to its baseline.
    pub fn ascent(&self, px: f64) -> f64 {
        match &self.font {
            Some(f) => f.ascender / f.units_per_em * px,
            None => px,
        }
    }

    /// Draw `text` with the pen at `x` on the baseline `y`.
    pub fn draw(&mut self, canvas: &mut Canvas, text: &str, x: f64, y: f64, px: f64, color: Color) {
        let Some(font) = self.font.clone() else {
            return;
        };
        let mut pen = x;

        for unit in text.encode_utf16() {
            let glyph = font.char_to_glyph(unit);
            let g = self.glyph(&font, glyph, px);
            let (gx, gy) = ((pen + g.left).floor(), (y + g.top).floor());

            for row in 0..g.h {
                for col in 0..g.w {
                    let c = g.coverage[row * g.w + col];

                    if c > 0.0 {
                        canvas.blend(gx as i64 + col as i64, gy as i64 + row as i64, color, c);
                    }
                }
            }

            pen += font.advance_width(glyph) / font.units_per_em * px;
        }
    }

    fn glyph(&mut self, font: &Font, glyph: u16, px: f64) -> Rc<Glyph> {
        self.cache
            .entry((glyph, px.to_bits() as u32 ^ (px.to_bits() >> 32) as u32))
            .or_insert_with(|| Rc::new(rasterize(font, glyph, px)))
            .clone()
    }
}

/// The outline of a glyph at `px`, y down, flattened to closed polygons.
fn contours(font: &Font, glyph: u16, px: f64) -> Vec<Vec<[f64; 2]>> {
    let mut out: Vec<Vec<[f64; 2]>> = vec![];
    let mut cur: Vec<[f64; 2]> = vec![];
    let mut last = [0.0, 0.0];
    let p = |x: f64, y: f64| [x * px, -y * px];

    for c in font.glyph_commands(glyph) {
        match c {
            Cmd::M(x, y) => {
                if cur.len() > 2 {
                    out.push(std::mem::take(&mut cur));
                }

                cur.clear();
                last = p(x, y);
                cur.push(last);
            }
            Cmd::L(x, y) => {
                last = p(x, y);
                cur.push(last);
            }
            Cmd::Q(cx, cy, x, y) => {
                let (c1, e) = (p(cx, cy), p(x, y));

                for i in 1..=8 {
                    let t = i as f64 / 8.0;
                    let u = 1.0 - t;

                    cur.push([
                        u * u * last[0] + 2.0 * u * t * c1[0] + t * t * e[0],
                        u * u * last[1] + 2.0 * u * t * c1[1] + t * t * e[1],
                    ]);
                }

                last = e;
            }
            Cmd::C(c1x, c1y, c2x, c2y, x, y) => {
                let (c1, c2, e) = (p(c1x, c1y), p(c2x, c2y), p(x, y));

                for i in 1..=8 {
                    let t = i as f64 / 8.0;
                    let u = 1.0 - t;

                    cur.push([
                        u * u * u * last[0]
                            + 3.0 * u * u * t * c1[0]
                            + 3.0 * u * t * t * c2[0]
                            + t * t * t * e[0],
                        u * u * u * last[1]
                            + 3.0 * u * u * t * c1[1]
                            + 3.0 * u * t * t * c2[1]
                            + t * t * t * e[1],
                    ]);
                }

                last = e;
            }
        }
    }

    if cur.len() > 2 {
        out.push(cur);
    }

    out
}

fn rasterize(font: &Font, glyph: u16, px: f64) -> Glyph {
    let polys = contours(font, glyph, px);
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);

    for p in polys.iter().flatten() {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }

    if x0 > x1 {
        return Glyph {
            w: 0,
            h: 0,
            left: 0.0,
            top: 0.0,
            coverage: vec![],
        };
    }

    let (left, top) = (x0.floor(), y0.floor());
    let (w, h) = (
        (x1.ceil() - left) as usize + 1,
        (y1.ceil() - top) as usize + 1,
    );
    let shifted: Vec<Vec<[f64; 2]>> = polys
        .iter()
        .map(|c| c.iter().map(|p| [p[0] - left, p[1] - top]).collect())
        .collect();

    Glyph {
        w,
        h,
        left,
        top,
        coverage: raster::fill(w, h, &shifted),
    }
}
