//! Text layout: `Browser/Text/X3DTextGeometry.js` (`configure`, `horizontal`, `vertical`,
//! `stringToGlyphs`, `getHorizontalLineExtents`), `Browser/Text/PolygonText.js` (`build`,
//! `getGlyphExtents`) and `Components/Text/X3DFontStyleNode.js` (`set_justify__`,
//! `getAlignment`). Variable names follow the JavaScript; the module-level scratch vectors of the
//! JS files are locals here, assigned at the same points.

use crate::font::{Font, PrimitiveQuality};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    First,
    Begin,
    Middle,
    End,
}

/// BrowserOptions TextCompression (default CHAR_SPACING).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextCompression {
    CharSpacing,
    Scaling,
}

/// The fields of a FontStyle the layout reads.
#[derive(Clone, Debug)]
pub struct FontStyleParams {
    /// `fontStyle .getScale ()` (FontStyle.size).
    pub size: f64,
    pub spacing: f64,
    pub horizontal: bool,
    pub left_to_right: bool,
    pub top_to_bottom: bool,
    pub justify: Vec<String>,
}

impl Default for FontStyleParams {
    fn default() -> Self {
        FontStyleParams { size: 1.0, spacing: 1.0, horizontal: true, left_to_right: true, top_to_bottom: true, justify: vec!["BEGIN".into()] }
    }
}

impl FontStyleParams {
    /// X3DFontStyleNode.getAlignment
    fn get_alignment(&self, index: usize, normal: bool) -> Alignment {
        let j = self.justify[index].as_str();

        if normal {
            match j {
                "FIRST" => return Alignment::First,
                "BEGIN" => return Alignment::Begin,
                "MIDDLE" => return Alignment::Middle,
                "END" => return Alignment::End,
                _ => {}
            }
        } else {
            match j {
                "FIRST" | "BEGIN" => return Alignment::End,
                "MIDDLE" => return Alignment::Middle,
                "END" => return Alignment::Begin,
                _ => {}
            }
        }

        if index != 0 { Alignment::First } else { Alignment::Begin }
    }

    /// X3DFontStyleNode.set_justify__: (major, minor).
    pub fn alignments(&self) -> (Alignment, Alignment) {
        let major_normal = if self.horizontal { self.left_to_right } else { self.top_to_bottom };
        let major = if !self.justify.is_empty() { self.get_alignment(0, major_normal) } else if major_normal { Alignment::Begin } else { Alignment::End };
        let minor_normal = if self.horizontal { self.top_to_bottom } else { self.left_to_right };
        let minor = if self.justify.len() > 1 { self.get_alignment(1, minor_normal) } else if minor_normal { Alignment::First } else { Alignment::End };

        (major, minor)
    }
}

/// The Text node fields the layout reads.
#[derive(Clone, Debug, Default)]
pub struct TextParams {
    /// `string`, as UTF-16 code units (JS `line [c]` indexes code units).
    pub string: Vec<Vec<u16>>,
    pub length: Vec<f64>,
    pub max_extent: f64,
}

impl TextParams {
    pub fn from_strings(strings: &[String], length: &[f64], max_extent: f64) -> Self {
        TextParams { string: strings.iter().map(|s| s.encode_utf16().collect()).collect(), length: length.to_vec(), max_extent }
    }

    /// Text.getLength
    fn get_length(&self, index: usize) -> f64 {
        if index < self.length.len() { self.length[index].max(0.0) } else { 0.0 }
    }
}

/// What Text writes to its fields and geometry arrays.
#[derive(Clone, Debug, Default)]
pub struct TextLayout {
    /// xyzw per vertex (triangles).
    pub vertices: Vec<f64>,
    /// xyzw per vertex.
    pub tex_coords: Vec<f64>,
    /// xyz per vertex.
    pub normals: Vec<f64>,
    /// Text.getMin / getMax (the extents of the bbox).
    pub min: [f64; 3],
    pub max: [f64; 3],
    pub origin: [f64; 3],
    pub text_bounds: [f64; 2],
    pub line_bounds: Vec<[f64; 2]>,
}

// --- Box2 / Matrix3 (standard/Math/Geometry/Box2.js) ------------------------------------------

#[derive(Clone, Copy)]
struct Box2 {
    m: [f64; 9],
}

impl Box2 {
    fn zero() -> Box2 {
        Box2 { m: [0.0; 9] }
    }

    fn with(size: [f64; 2], center: [f64; 2]) -> Box2 {
        Box2 { m: [size[0] / 2.0, 0.0, 0.0, 0.0, size[1] / 2.0, 0.0, center[0], center[1], 1.0] }
    }

    fn set_extents(min: [f64; 2], max: [f64; 2]) -> Box2 {
        let (sx, sy, cx, cy) = ((max[0] - min[0]) / 2.0, (max[1] - min[1]) / 2.0, (max[0] + min[0]) / 2.0, (max[1] + min[1]) / 2.0);

        Box2 { m: [sx, 0.0, 0.0, 0.0, sy, 0.0, cx, cy, 1.0] }
    }

    fn is_empty(&self) -> bool {
        self.m[8] == 0.0
    }

    /// getExtents: absolute extents plus the center.
    fn extents(&self) -> ([f64; 2], [f64; 2]) {
        let x = [self.m[0], self.m[1]];
        let y = [self.m[3], self.m[4]];
        let p1 = [x[0] + y[0], x[1] + y[1]];
        let p2 = [y[0] - x[0], y[1] - x[1]];
        let mut min = [p1[0].min(p2[0]), p1[1].min(p2[1])];
        let mut max = [p1[0].max(p2[0]), p1[1].max(p2[1])];
        let (n1, n2) = ([-p1[0], -p1[1]], [-p2[0], -p2[1]]);

        min = [min[0].min(n1[0]).min(n2[0]), min[1].min(n1[1]).min(n2[1])];
        max = [max[0].max(n1[0]).max(n2[0]), max[1].max(n1[1]).max(n2[1])];

        let c = [self.m[6], self.m[7]];

        ([min[0] + c[0], min[1] + c[1]], [max[0] + c[0], max[1] + c[1]])
    }

    fn add(&mut self, other: &Box2) {
        if self.is_empty() {
            *self = *other;
            return;
        }

        if other.is_empty() {
            return;
        }

        let (lmin, lmax) = self.extents();
        let (rmin, rmax) = other.extents();

        *self = Box2::set_extents([lmin[0].min(rmin[0]), lmin[1].min(rmin[1])], [lmax[0].max(rmax[0]), lmax[1].max(rmax[1])]);
    }

    /// Matrix3.multRight
    fn mult_right(&mut self, b: [f64; 9]) {
        let a = &mut self.m;
        let (mut a0, mut a1, mut a2) = (a[0], a[1], a[2]);

        a[0] = a0 * b[0] + a1 * b[3] + a2 * b[6];
        a[1] = a0 * b[1] + a1 * b[4] + a2 * b[7];
        a[2] = a0 * b[2] + a1 * b[5] + a2 * b[8];
        (a0, a1, a2) = (a[3], a[4], a[5]);
        a[3] = a0 * b[0] + a1 * b[3] + a2 * b[6];
        a[4] = a0 * b[1] + a1 * b[4] + a2 * b[7];
        a[5] = a0 * b[2] + a1 * b[5] + a2 * b[8];
        (a0, a1, a2) = (a[6], a[7], a[8]);
        a[6] = a0 * b[0] + a1 * b[3] + a2 * b[6];
        a[7] = a0 * b[1] + a1 * b[4] + a2 * b[7];
        a[8] = a0 * b[2] + a1 * b[5] + a2 * b[8];
    }
}

/// PolygonText.getGlyphExtents: (min, max) of the glyph's triangles, (0, 0) for an empty glyph.
fn glyph_extents(font: &Font, glyph: u16, quality: PrimitiveQuality) -> ([f64; 2], [f64; 2]) {
    let v = font.glyph_geometry(glyph, quality);

    if v.is_empty() {
        return ([0.0; 2], [0.0; 2]);
    }

    let (mut min, mut max) = (v[0], v[0]);

    for p in v.iter() {
        min = [min[0].min(p[0]), min[1].min(p[1])];
        max = [max[0].max(p[0]), max[1].max(p[1])];
    }

    (min, max)
}

struct State<'a> {
    font: &'a Font,
    quality: PrimitiveQuality,
    compression: TextCompression,
    glyphs: Vec<Vec<u16>>,
    translations: Vec<[f64; 2]>,
    line_indices: Vec<usize>,
    char_spacings: Vec<f64>,
    scales: Vec<f64>,
    minor_alignment: [f64; 2],
    line_bounds: Vec<[f64; 2]>,
    text_bounds: [f64; 2],
    origin: [f64; 3],
    bbox_min: [f64; 3],
    bbox_max: [f64; 3],
}

impl State<'_> {
    /// X3DTextGeometry.stringToGlyphs
    fn string_to_glyphs(&mut self, line: &[u16], normal: bool, line_number: usize) -> Vec<u16> {
        while self.glyphs.len() <= line_number {
            self.glyphs.push(vec![]);
        }

        let mut out = Vec::with_capacity(line.len());

        if normal {
            for c in line {
                out.push(self.font.char_to_glyph(*c));
            }
        } else {
            for c in line.iter().rev() {
                out.push(self.font.char_to_glyph(*c));
            }
        }

        self.glyphs[line_number] = out.clone();
        out
    }

    /// X3DTextGeometry.getHorizontalLineExtents: (glyphs, min, max).
    fn horizontal_line_extents(&mut self, fs: &FontStyleParams, major: Alignment, line: &[u16], line_number: usize) -> (Vec<u16>, [f64; 2], [f64; 2]) {
        let normal = if fs.horizontal { fs.left_to_right } else { fs.top_to_bottom };
        let glyphs = self.string_to_glyphs(line, normal, line_number);
        let num_glyphs = glyphs.len();
        let (mut x_min, mut x_max) = (0.0f64, 0.0f64);
        let (mut y_min, mut y_max) = (f64::INFINITY, f64::NEG_INFINITY);

        for g in 0..num_glyphs {
            let glyph = glyphs[g];
            let kerning = if g + 1 < num_glyphs { self.font.kerning(glyph, glyphs[g + 1]) } else { 0.0 };
            let (gmin, gmax) = glyph_extents(self.font, glyph, self.quality);

            x_max += if g + 1 < num_glyphs { self.font.advance_width(glyph) + kerning } else { gmax[0] * self.font.units_per_em };
            y_min = y_min.min(gmin[1]);
            y_max = y_max.max(gmax[1]);
        }

        if !glyphs.is_empty() {
            let (gmin, _) = glyph_extents(self.font, glyphs[0], self.quality);

            x_min = gmin[0];
        } else {
            y_min = 0.0;
            y_max = 0.0;
        }

        let mut min = [x_min, y_min];
        let max = [x_max / self.font.units_per_em, y_max];

        if matches!(major, Alignment::Begin | Alignment::First) {
            min[0] = 0.0;
        }

        (glyphs, min, max)
    }

    fn horizontal(&mut self, text: &TextParams, fs: &FontStyleParams, major: Alignment, minor: Alignment) {
        let num_lines = text.string.len();
        let max_extent = text.max_extent.max(0.0);
        let top_to_bottom = fs.top_to_bottom;
        let scale = fs.size;
        let content_scale = 1.0;
        let spacing = fs.spacing;
        let mut bbox = Box2::zero();

        self.translations = vec![[0.0; 2]; num_lines];
        self.line_indices.clear();
        self.char_spacings = vec![0.0; num_lines];
        self.scales = vec![0.0; num_lines];
        self.line_bounds = vec![[0.0; 2]; num_lines];

        let (first, last, step): (isize, isize, isize) = if top_to_bottom { (0, num_lines as isize, 1) } else { (num_lines as isize - 1, -1, -1) };
        let mut l = first;
        let mut ll = 0usize;

        while l != last {
            let line = text.string[l as usize].clone();
            let (glyphs, min, max) = self.horizontal_line_extents(fs, major, &line, ll);
            let mut size = [max[0] - min[0], max[1] - min[1]];
            let length = text.get_length(l as usize) * content_scale;
            let w = size[0] * scale;
            let mut char_spacing = 0.0;
            let mut line_bound = [w, if ll == 0 { max[1] - self.font.descender / self.font.units_per_em * scale } else { spacing }];

            if length != 0.0 {
                if self.compression == TextCompression::CharSpacing && glyphs.len() > 1 {
                    char_spacing = (length - line_bound[0]) / (glyphs.len() - 1) as f64;
                }

                line_bound[0] = length;
                size[0] = length / scale;
            }

            self.char_spacings[ll] = char_spacing;
            self.scales[ll] = if self.compression == TextCompression::Scaling { line_bound[0] / w } else { 1.0 };
            self.line_bounds[l as usize] = line_bound;

            let t = match major {
                Alignment::Begin | Alignment::First => [0.0, -(ll as f64) * spacing],
                Alignment::Middle => [-min[0] - size[0] / 2.0, -(ll as f64) * spacing],
                Alignment::End => [-min[0] - size[0], -(ll as f64) * spacing],
            };

            self.translations[ll] = [t[0] * scale, t[1] * scale];

            let mut center = [min[0] + size[0] / 2.0, min[1] + size[1] / 2.0];

            center = [center[0] * scale, center[1] * scale];
            center = [center[0] + self.translations[ll][0], center[1] + self.translations[ll][1]];
            size = [size[0] * scale, size[1] * scale];
            bbox.add(&Box2::with(size, center));

            l += step;
            ll += 1;
        }

        if max_extent != 0.0 {
            let extent = self.line_bounds.iter().fold(0.0f64, |p, c| p.max(c[0]));

            if extent > max_extent {
                let s = (max_extent * content_scale) / extent;

                match self.compression {
                    TextCompression::CharSpacing => {
                        for i in 0..self.char_spacings.len() {
                            if text.string[i].len() < 2 {
                                continue;
                            }

                            self.char_spacings[i] -= (self.line_bounds[i][0] - self.line_bounds[i][0] * s) / (text.string[i].len() - 1) as f64;
                        }
                    }
                    TextCompression::Scaling => {
                        for sc in self.scales.iter_mut() {
                            *sc *= s;
                        }
                    }
                }

                for t in self.translations.iter_mut() {
                    t[0] *= s;
                }

                for lb in self.line_bounds.iter_mut() {
                    lb[0] *= s;
                }

                bbox.mult_right([s, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]);
            }
        }

        let (min, max) = bbox.extents();
        let size = [max[0] - min[0], max[1] - min[1]];

        self.text_bounds = size;

        let bearing = [0.0, -max[1]];

        self.minor_alignment = match minor {
            Alignment::Begin => bearing,
            Alignment::First => [0.0, 0.0],
            Alignment::Middle => [0.0, size[1] / 2.0 - max[1]],
            Alignment::End => [0.0, (num_lines as f64 - 1.0) * spacing * scale],
        };

        self.finish(min, max);
    }

    fn vertical(&mut self, text: &TextParams, fs: &FontStyleParams, major: Alignment, minor: Alignment) {
        let num_lines = text.string.len();
        let max_extent = text.max_extent.max(0.0);
        let (left_to_right, top_to_bottom) = (fs.left_to_right, fs.top_to_bottom);
        let scale = fs.size;
        let content_scale = 1.0;
        let spacing = fs.spacing;
        let num_chars: usize = text.string.iter().map(|s| s.len()).sum();
        let mut bbox = Box2::zero();

        self.translations = vec![[0.0; 2]; num_chars];
        self.line_indices = vec![0; num_chars];
        self.char_spacings = vec![0.0; num_lines];
        self.scales = vec![0.0; num_lines];
        self.line_bounds = vec![[0.0; 2]; num_lines];

        let (first_l, last_l, step_l): (isize, isize, isize) = if left_to_right { (0, num_lines as isize, 1) } else { (num_lines as isize - 1, -1, -1) };
        let mut t = 0usize;
        let mut l = first_l;

        while l != last_l {
            let lu = l as usize;
            let line = text.string[lu].clone();
            let glyphs = self.string_to_glyphs(&line, true, lu);
            let t0 = t;
            let num_chars_line = glyphs.len();
            let mut line_bbox = Box2::zero();
            let (first_g, last_g, step_g): (isize, isize, isize) = if top_to_bottom { (0, num_chars_line as isize, 1) } else { (num_chars_line as isize - 1, -1, -1) };
            let mut g = first_g;

            while g != last_g {
                let glyph = glyphs[g as usize];
                let (gmin, gmax) = glyph_extents(self.font, glyph, self.quality);
                let size = [gmax[0] - gmin[0], gmax[1] - gmin[1]];
                let glyph_number = if top_to_bottom { g } else { num_chars_line as isize - g - 1 };

                self.line_indices[t] = lu;
                self.translations[t] = [(spacing - size[0] - gmin[0]) / 2.0, -(glyph_number as f64)];

                let mut center = [gmin[0] + size[0] / 2.0, gmin[1] + size[1] / 2.0];

                center = [center[0] + self.translations[t][0], center[1] + self.translations[t][1]];
                line_bbox.add(&Box2::with(size, center));

                g += step_g;
                t += 1;
            }

            let (mut min, mut max) = line_bbox.extents();
            let mut size = [max[0] - min[0], max[1] - min[1]];
            let line_number = if left_to_right { lu } else { num_lines - lu - 1 };
            let padding = (spacing - size[0]) / 2.0;
            let length = text.get_length(lu) * content_scale;
            let mut char_spacing = 0.0;
            let mut line_bound = [if lu == 0 { spacing - padding } else { spacing }, if num_chars_line != 0 { size[1] } else { 0.0 }];

            line_bound = [line_bound[0] * scale, line_bound[1] * scale];

            let h = line_bound[1];

            if length != 0.0 {
                match self.compression {
                    TextCompression::CharSpacing => {
                        if glyphs.len() > 1 {
                            char_spacing = (length - line_bound[1]) / (glyphs.len() - 1) as f64;
                        }
                    }
                    TextCompression::Scaling => {
                        if major == Alignment::Middle {
                            max[1] += (length - line_bound[1]) / 2.0;
                        }
                    }
                }

                line_bound[1] = length;
                size[1] = length / scale;
                min[1] = max[1] - size[1];
            }

            self.char_spacings[lu] = char_spacing;
            self.scales[lu] = if self.compression == TextCompression::Scaling { line_bound[1] / h } else { 1.0 };
            self.line_bounds[lu] = line_bound;

            let ls = line_number as f64 * spacing;
            let (translation1, translation2) = match major {
                Alignment::Begin | Alignment::First => ([ls, -max[1]], [ls, -max[1]]),
                Alignment::Middle => ([ls, size[1] / 2.0 - max[1]], [ls, size[1] / 2.0 - max[1]]),
                Alignment::End => ([ls, size[1] / self.scales[lu] - max[1]], [ls, size[1] - max[1]]),
            };

            for tt in t0..t {
                self.translations[tt] = [(self.translations[tt][0] + translation1[0]) * scale, (self.translations[tt][1] + translation1[1]) * scale];
            }

            let mut center = [min[0] + size[0] / 2.0, min[1] + size[1] / 2.0];

            center = [(center[0] + translation2[0]) * scale, (center[1] + translation2[1]) * scale];
            size = [size[0] * scale, size[1] * scale];
            bbox.add(&Box2::with(size, center));

            l += step_l;
        }

        if max_extent != 0.0 {
            let extent = self.line_bounds.iter().fold(0.0f64, |p, c| p.max(c[1]));

            if extent > max_extent {
                let s = (max_extent * content_scale) / extent;

                match self.compression {
                    TextCompression::CharSpacing => {
                        for i in 0..self.char_spacings.len() {
                            if text.string[i].len() < 2 {
                                continue;
                            }

                            self.char_spacings[i] -= (self.line_bounds[i][1] - self.line_bounds[i][1] * s) / (text.string[i].len() - 1) as f64;
                        }

                        match major {
                            Alignment::Middle => {
                                for t in 0..self.translations.len() {
                                    let l = self.line_indices[t];

                                    self.translations[t][1] -= (self.line_bounds[l][1] - self.line_bounds[l][1] * s) / 2.0;
                                }
                            }
                            Alignment::End => {
                                for t in 0..self.translations.len() {
                                    let l = self.line_indices[t];

                                    self.translations[t][1] -= self.line_bounds[l][1] - self.line_bounds[l][1] * s;
                                }
                            }
                            _ => {}
                        }
                    }
                    TextCompression::Scaling => {
                        for sc in self.scales.iter_mut() {
                            *sc *= s;
                        }
                    }
                }

                for lb in self.line_bounds.iter_mut() {
                    lb[1] *= s;
                }

                bbox.mult_right([1.0, 0.0, 0.0, 0.0, s, 0.0, 0.0, 0.0, 0.0]);
            }
        }

        let (min, max) = bbox.extents();
        let size = [max[0] - min[0], max[1] - min[1]];

        self.text_bounds = size;

        self.minor_alignment = match minor {
            Alignment::Begin | Alignment::First => [-min[0], 0.0],
            Alignment::Middle => [-min[0] - size[0] / 2.0, 0.0],
            Alignment::End => [-min[0] - size[0], 0.0],
        };

        self.finish(min, max);
    }

    /// The common tail of `horizontal` / `vertical`: translate by minorAlignment, set origin and
    /// the Box3 extents (`this .bbox .setExtents`, read back with getExtents).
    fn finish(&mut self, mut min: [f64; 2], mut max: [f64; 2]) {
        min = [min[0] + self.minor_alignment[0], min[1] + self.minor_alignment[1]];
        max = [max[0] + self.minor_alignment[0], max[1] + self.minor_alignment[1]];
        self.origin = [min[0], max[1], 0.0];

        let (sx, sy, cx, cy) = ((max[0] - min[0]) / 2.0, (max[1] - min[1]) / 2.0, (max[0] + min[0]) / 2.0, (max[1] + min[1]) / 2.0);

        self.bbox_min = [cx - sx, cy - sy, 0.0];
        self.bbox_max = [cx + sx, cy + sy, 0.0];
    }
}

/// X3DTextGeometry.update (configure + PolygonText.build) for one Text/FontStyle.
pub fn layout(font: Option<&Font>, text: &TextParams, fs: &FontStyleParams, quality: PrimitiveQuality, compression: TextCompression) -> TextLayout {
    let num_lines = text.string.len();
    let mut out = TextLayout { line_bounds: vec![[0.0; 2]; num_lines], ..Default::default() };

    let Some(font) = font else { return out };

    if num_lines == 0 {
        return out;
    }

    let (major, minor) = fs.alignments();
    let mut st = State {
        font,
        quality,
        compression,
        glyphs: vec![],
        translations: vec![],
        line_indices: vec![],
        char_spacings: vec![],
        scales: vec![],
        minor_alignment: [0.0; 2],
        line_bounds: vec![],
        text_bounds: [0.0; 2],
        origin: [0.0; 3],
        bbox_min: [0.0; 3],
        bbox_max: [0.0; 3],
    };

    if fs.horizontal {
        st.horizontal(text, fs, major, minor);
    } else {
        st.vertical(text, fs, major, minor);
    }

    out.min = st.bbox_min;
    out.max = st.bbox_max;
    out.origin = st.origin;
    out.text_bounds = st.text_bounds;
    out.line_bounds = st.line_bounds.clone();

    // PolygonText.build
    let size = fs.size;
    let spacing = fs.spacing;
    let origin = st.origin;
    let size_units_per_em = size / font.units_per_em;
    let push = |out: &mut TextLayout, x: f64, y: f64| {
        out.tex_coords.extend_from_slice(&[(x - origin[0]) / spacing, (y - origin[1]) / spacing, 0.0, 1.0]);
        out.normals.extend_from_slice(&[0.0, 0.0, 1.0]);
        out.vertices.extend_from_slice(&[x, y, 0.0, 1.0]);
    };

    if fs.horizontal {
        for l in 0..st.glyphs.len() {
            let line = st.glyphs[l].clone();
            let translation = st.translations[l];
            let char_spacing = st.char_spacings[l];
            let scale = st.scales[l];
            let mut advance_width = 0.0;

            for g in 0..line.len() {
                let glyph = line[g];
                let geometry = font.glyph_geometry(glyph, quality);
                let x_offset = st.minor_alignment[0] + translation[0] + advance_width * scale + g as f64 * char_spacing;
                let y_offset = st.minor_alignment[1] + translation[1];

                for p in geometry.iter() {
                    push(&mut out, p[0] * size * scale + x_offset, p[1] * size + y_offset);
                }

                let kerning = if g + 1 < line.len() { font.kerning(glyph, line[g + 1]) } else { 0.0 };

                advance_width += (font.advance_width(glyph) + kerning) * size_units_per_em;
            }
        }
    } else {
        let (left_to_right, top_to_bottom) = (fs.left_to_right, fs.top_to_bottom);
        let n = text.string.len() as isize;
        let (first, last, step): (isize, isize, isize) = if left_to_right { (0, n, 1) } else { (n - 1, -1, -1) };
        let mut l = first;
        let mut t = 0usize;

        while l != last {
            let line = st.glyphs[l as usize].clone();
            let num_chars = line.len() as isize;
            let (first_g, last_g, step_g): (isize, isize, isize) = if top_to_bottom { (0, num_chars, 1) } else { (num_chars - 1, -1, -1) };
            let char_spacing = st.char_spacings[l as usize];
            let scale = st.scales[l as usize];
            let mut g = first_g;

            while g != last_g {
                let translation = st.translations[t];
                let geometry = font.glyph_geometry(line[g as usize], quality);
                let glyph_number = if top_to_bottom { g } else { num_chars - g - 1 };

                for p in geometry.iter() {
                    let x = p[0] * size + st.minor_alignment[0] + translation[0];
                    let y = p[1] * size * scale + st.minor_alignment[1] + translation[1] * scale - glyph_number as f64 * char_spacing;

                    push(&mut out, x, y);
                }

                g += step_g;
                t += 1;
            }

            l += step;
        }
    }

    out
}
