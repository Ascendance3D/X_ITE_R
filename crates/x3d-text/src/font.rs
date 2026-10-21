//! Fonts: what X_ITE gets from opentype.js 2.0 (`OpenType.parse`) for TrueType-flavored fonts
//! (WOFF2 is decompressed first, like `X3DTextContext.decompressFont`), and the glyph geometry
//! of `Browser/Text/PolygonText.js` (`createGlyphGeometry`).
//!
//! The glyph table is parsed here instead of with ttf-parser's outline builder because
//! libtess' output depends on where each contour starts and on the duplicated closing points
//! opentype.js `getPath` produces (`buildPath` / `getPath` in opentype.mjs).

use crate::libtess;
use std::cell::RefCell;
use std::collections::HashMap;
use ttf_parser::{Face, RawFace, Tag};

/// BrowserOptions PrimitiveQuality (default MEDIUM).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrimitiveQuality {
    Low,
    Medium,
    High,
}

impl PrimitiveQuality {
    /// PolygonText.getBezierSteps
    pub fn bezier_steps(self) -> usize {
        match self {
            PrimitiveQuality::Low => 3,
            PrimitiveQuality::High => 7,
            PrimitiveQuality::Medium => 5,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Point {
    x: f64,
    y: f64,
    on_curve: bool,
    last: bool,
}

/// One entry of an opentype.js path (`Glyph.getPath` output: M, L, Q; the Z commands are only
/// emitted for stroked paths, so there are none).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cmd {
    M(f64, f64),
    L(f64, f64),
    Q(f64, f64, f64, f64),
}

pub struct Font {
    data: Vec<u8>,
    pub units_per_em: f64,
    pub ascender: f64,
    pub descender: f64,
    pub num_glyphs: u16,
    loca: Vec<u32>,
    glyf: (usize, usize),
    advances: Vec<u16>,
    cmap: HashMap<u32, u16>,
    has_gpos: bool,
    kern_cache: RefCell<HashMap<(u16, u16), i32>>,
    geometry_cache: RefCell<HashMap<(u16, PrimitiveQuality), std::rc::Rc<Vec<[f64; 2]>>>>,
}

#[derive(Debug)]
pub enum FontError {
    Parse(String),
}

fn be16(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

fn be16s(d: &[u8], o: usize) -> Option<i16> {
    be16(d, o).map(|v| v as i16)
}

impl Font {
    /// `OpenType.parse (await decompressFont (data))`.
    pub fn parse(bytes: &[u8]) -> Result<Font, FontError> {
        let data = if bytes.len() >= 4 && &bytes[0..4] == b"wOF2" {
            woff2_patched::convert_woff2_to_ttf(&mut std::io::Cursor::new(bytes)).map_err(|e| FontError::Parse(format!("woff2: {e:?}")))?
        } else {
            bytes.to_vec()
        };

        let face = Face::parse(&data, 0).map_err(|e| FontError::Parse(format!("{e:?}")))?;
        let raw = RawFace::parse(&data, 0).map_err(|e| FontError::Parse(format!("{e:?}")))?;
        let table = |tag: &[u8; 4]| raw.table(Tag::from_bytes(tag));
        let glyf_tag = table(b"glyf").ok_or_else(|| FontError::Parse("no glyf table (only TrueType outlines are supported)".into()))?;
        let loca_tag = table(b"loca").ok_or_else(|| FontError::Parse("no loca table".into()))?;
        let head = table(b"head").ok_or_else(|| FontError::Parse("no head table".into()))?;
        let long_loca = be16s(head, 50).unwrap_or(0) != 0;
        let num_glyphs = face.number_of_glyphs();

        let mut loca = Vec::with_capacity(num_glyphs as usize + 1);

        for i in 0..=num_glyphs as usize {
            let v = if long_loca {
                let o = i * 4;
                u32::from_be_bytes([*loca_tag.get(o).unwrap_or(&0), *loca_tag.get(o + 1).unwrap_or(&0), *loca_tag.get(o + 2).unwrap_or(&0), *loca_tag.get(o + 3).unwrap_or(&0)])
            } else {
                be16(loca_tag, i * 2).unwrap_or(0) as u32 * 2
            };

            loca.push(v);
        }

        let glyf_start = glyf_tag.as_ptr() as usize - data.as_ptr() as usize;
        let mut advances = Vec::with_capacity(num_glyphs as usize);

        for g in 0..num_glyphs {
            advances.push(face.glyph_hor_advance(ttf_parser::GlyphId(g)).unwrap_or(0));
        }

        // opentype.js parseCmapTable: the last sub-table that matches (3, 0|1|10), (0, 0..4|6)
        // or (1, 0); ttf-parser parses it.
        let mut cmap = HashMap::new();

        if let Some(c) = face.tables().cmap {
            let mut chosen = None;

            for st in c.subtables {
                let ok = (st.platform_id == ttf_parser::PlatformId::Windows && matches!(st.encoding_id, 0 | 1 | 10))
                    || (st.platform_id == ttf_parser::PlatformId::Unicode && matches!(st.encoding_id, 0..=4 | 6))
                    || (st.platform_id == ttf_parser::PlatformId::Macintosh && st.encoding_id == 0);

                if ok {
                    chosen = Some(st);
                }
            }

            if let Some(st) = chosen {
                st.codepoints(|cp| {
                    if let Some(g) = st.glyph_index(cp) {
                        cmap.insert(cp, g.0);
                    }
                });
            }
        }

        Ok(Font {
            units_per_em: face.units_per_em() as f64,
            ascender: face.ascender() as f64,
            descender: face.descender() as f64,
            num_glyphs,
            loca,
            glyf: (glyf_start, glyf_tag.len()),
            advances,
            cmap,
            has_gpos: face.tables().gpos.is_some(),
            kern_cache: RefCell::new(HashMap::new()),
            geometry_cache: RefCell::new(HashMap::new()),
            data,
        })
    }

    /// `font.charToGlyph (c)`: glyph index of one UTF-16 code unit; missing characters map to
    /// glyph 0 (.notdef).
    pub fn char_to_glyph(&self, unit: u16) -> u16 {
        self.cmap.get(&(unit as u32)).copied().unwrap_or(0)
    }

    pub fn advance_width(&self, glyph: u16) -> f64 {
        self.advances.get(glyph as usize).copied().unwrap_or(0) as f64
    }

    /// `font.getKerningValue (left, right)` (opentype.js `Font.getKerningValue`): a font with a
    /// GPOS table uses only its default script's `kern` feature (lookup type 2, pair
    /// adjustment); otherwise the `kern` table. A glyph with index 0 never kerns, because
    /// `leftGlyph .index || leftGlyph` then passes the glyph object.
    pub fn kerning(&self, left: u16, right: u16) -> f64 {
        if left == 0 || right == 0 {
            return 0.0;
        }

        if let Some(v) = self.kern_cache.borrow().get(&(left, right)) {
            return *v as f64;
        }

        let v = self.kerning_uncached(left, right);

        self.kern_cache.borrow_mut().insert((left, right), v);
        v as f64
    }

    fn kerning_uncached(&self, left: u16, right: u16) -> i32 {
        let Ok(face) = Face::parse(&self.data, 0) else { return 0 };
        let (l, r) = (ttf_parser::GlyphId(left), ttf_parser::GlyphId(right));

        if self.has_gpos {
            let Some(gpos) = face.tables().gpos else { return 0 };
            let default_script = if gpos.scripts.find(Tag::from_bytes(b"DFLT")).is_some() {
                Tag::from_bytes(b"DFLT")
            } else if gpos.scripts.find(Tag::from_bytes(b"latn")).is_some() {
                Tag::from_bytes(b"latn")
            } else {
                // getDefaultScriptName () is undefined: getScriptTable (undefined) → "DFLT".
                Tag::from_bytes(b"DFLT")
            };
            let Some(script) = gpos.scripts.find(default_script) else { return 0 };
            let Some(lang) = script.default_language else { return 0 };

            // getFeatureTable: the first feature of the default language system tagged `kern`.
            let mut feature = None;

            for fi in lang.feature_indices {
                if let Some(f) = gpos.features.get(fi) {
                    if f.tag == Tag::from_bytes(b"kern") {
                        feature = Some(f);
                        break;
                    }
                }
            }

            let Some(feature) = feature else { return 0 };

            for li in feature.lookup_indices {
                let Some(lookup) = gpos.lookups.get(li) else { continue };

                for subtable in lookup.subtables.into_iter::<ttf_parser::gpos::PositioningSubtable>() {
                    let ttf_parser::gpos::PositioningSubtable::Pair(pair) = subtable else { continue };

                    match pair {
                        ttf_parser::gpos::PairAdjustment::Format1 { coverage, sets } => {
                            let Some(cov) = coverage.get(l) else { continue };
                            let Some(set) = sets.get(cov) else { continue };

                            if let Some((v1, _)) = set.get(r) {
                                return v1.x_advance as i32;
                            }
                        }
                        ttf_parser::gpos::PairAdjustment::Format2 { coverage, classes, matrix } => {
                            if coverage.get(l).is_none() {
                                continue;
                            }

                            let c1 = classes.0.get(l);
                            let c2 = classes.1.get(r);

                            return matrix.get((c1, c2)).map(|(v1, _)| v1.x_advance as i32).unwrap_or(0);
                        }
                    }
                }
            }

            return 0;
        }

        if let Some(kern) = face.tables().kern {
            for st in kern.subtables {
                if st.horizontal && !st.variable {
                    if let Some(v) = st.glyphs_kerning(l, r) {
                        return v as i32;
                    }
                }
            }
        }

        0
    }

    // --- glyph outlines (opentype.js glyf.mjs) ----------------------------------------------------

    fn glyph_points(&self, glyph: u16, depth: usize, resolving: &mut Vec<u16>) -> Vec<Point> {
        if glyph >= self.num_glyphs || depth > 8 {
            return vec![];
        }

        let (start, len) = self.glyf;
        let glyf = &self.data[start..start + len];
        let (o, n) = (self.loca[glyph as usize] as usize, self.loca[glyph as usize + 1] as usize);

        if o == n || o + 10 > glyf.len() {
            return vec![];
        }

        let d = &glyf[o..];
        let contours = be16s(d, 0).unwrap_or(0);
        let mut pos = 10usize;

        if contours > 0 {
            let contours = contours as usize;
            let mut ends = Vec::with_capacity(contours);

            for i in 0..contours {
                ends.push(be16(d, pos + i * 2).unwrap_or(0) as usize);
            }

            pos += contours * 2;

            let instr = be16(d, pos).unwrap_or(0) as usize;

            pos += 2 + instr;

            let count = ends[contours - 1] + 1;
            let mut flags: Vec<u8> = Vec::with_capacity(count);

            while flags.len() < count {
                let flag = *d.get(pos).unwrap_or(&0);

                pos += 1;
                flags.push(flag);

                if flag & 8 != 0 {
                    let repeat = *d.get(pos).unwrap_or(&0);

                    pos += 1;

                    for _ in 0..repeat {
                        flags.push(flag);
                    }
                }
            }

            flags.truncate(count);

            let mut points: Vec<Point> = flags.iter().enumerate().map(|(i, f)| Point { x: 0.0, y: 0.0, on_curve: f & 1 != 0, last: ends.contains(&i) }).collect();
            let mut prev = 0i32;

            for i in 0..count {
                let flag = flags[i];
                let v;

                if flag & 2 != 0 {
                    let b = *d.get(pos).unwrap_or(&0) as i32;

                    pos += 1;
                    v = prev + if flag & 16 != 0 { b } else { -b };
                } else if flag & 16 != 0 {
                    v = prev;
                } else {
                    v = prev + be16s(d, pos).unwrap_or(0) as i32;
                    pos += 2;
                }

                points[i].x = v as f64;
                prev = v;
            }

            prev = 0;

            for i in 0..count {
                let flag = flags[i];
                let v;

                if flag & 4 != 0 {
                    let b = *d.get(pos).unwrap_or(&0) as i32;

                    pos += 1;
                    v = prev + if flag & 32 != 0 { b } else { -b };
                } else if flag & 32 != 0 {
                    v = prev;
                } else {
                    v = prev + be16s(d, pos).unwrap_or(0) as i32;
                    pos += 2;
                }

                points[i].y = v as f64;
                prev = v;
            }

            return points;
        }

        if contours == 0 {
            return vec![];
        }

        // Composite glyph.
        struct Component {
            index: u16,
            x_scale: f64,
            scale01: f64,
            scale10: f64,
            y_scale: f64,
            dx: f64,
            dy: f64,
            matched: Option<(usize, usize)>,
        }

        let f2dot14 = |o: usize| be16s(d, o).unwrap_or(0) as f64 / 16384.0;
        let mut comps = vec![];

        loop {
            let flags = be16(d, pos).unwrap_or(0);
            let index = be16(d, pos + 2).unwrap_or(0);

            pos += 4;

            let mut c = Component { index, x_scale: 1.0, scale01: 0.0, scale10: 0.0, y_scale: 1.0, dx: 0.0, dy: 0.0, matched: None };

            if flags & 1 != 0 {
                if flags & 2 != 0 {
                    c.dx = be16s(d, pos).unwrap_or(0) as f64;
                    c.dy = be16s(d, pos + 2).unwrap_or(0) as f64;
                } else {
                    c.matched = Some((be16(d, pos).unwrap_or(0) as usize, be16(d, pos + 2).unwrap_or(0) as usize));
                }

                pos += 4;
            } else {
                if flags & 2 != 0 {
                    c.dx = *d.get(pos).unwrap_or(&0) as i8 as f64;
                    c.dy = *d.get(pos + 1).unwrap_or(&0) as i8 as f64;
                } else {
                    c.matched = Some((*d.get(pos).unwrap_or(&0) as usize, *d.get(pos + 1).unwrap_or(&0) as usize));
                }

                pos += 2;
            }

            if flags & 8 != 0 {
                c.x_scale = f2dot14(pos);
                c.y_scale = c.x_scale;
                pos += 2;
            } else if flags & 64 != 0 {
                c.x_scale = f2dot14(pos);
                c.y_scale = f2dot14(pos + 2);
                pos += 4;
            } else if flags & 128 != 0 {
                c.x_scale = f2dot14(pos);
                c.scale01 = f2dot14(pos + 2);
                c.scale10 = f2dot14(pos + 4);
                c.y_scale = f2dot14(pos + 6);
                pos += 8;
            }

            comps.push(c);

            if flags & 32 == 0 {
                break;
            }
        }

        let mut points: Vec<Point> = vec![];

        resolving.push(glyph);

        for c in comps {
            if resolving.contains(&c.index) {
                continue;
            }

            let sub = self.glyph_points(c.index, depth + 1, resolving);

            if sub.is_empty() {
                continue;
            }

            let transform = |pts: &[Point], dx: f64, dy: f64| -> Vec<Point> {
                pts.iter()
                    .map(|p| Point {
                        x: c.x_scale * p.x + c.scale10 * p.y + dx,
                        y: c.scale01 * p.x + c.y_scale * p.y + dy,
                        on_curve: p.on_curve,
                        last: p.last,
                    })
                    .collect()
            };

            let transformed = match c.matched {
                None => transform(&sub, c.dx, c.dy),
                Some((a, b)) => {
                    let (Some(first), Some(second)) = (points.get(a), sub.get(b)) else { continue };
                    let sp = transform(&[*second], 0.0, 0.0)[0];

                    transform(&sub, first.x - sp.x, first.y - sp.y)
                }
            };

            points.extend(transformed);
        }

        resolving.pop();
        points
    }

    /// opentype.js `glyph .xMin`, `yMin`, `xMax`, `yMax`: the bounding box of the glyph's `glyf` header in font units (0 where
    /// opentype.js leaves it undefined: an empty glyph).
    pub fn glyph_bounds(&self, glyph: u16) -> [f64; 4] {
        if glyph >= self.num_glyphs {
            return [0.0; 4];
        }

        let (start, len) = self.glyf;
        let glyf = &self.data[start..start + len];
        let (o, n) = (self.loca[glyph as usize] as usize, self.loca[glyph as usize + 1] as usize);

        if o == n || o + 10 > glyf.len() {
            return [0.0; 4];
        }

        let d = &glyf[o..];
        let v = |i: usize| be16s(d, 2 + i * 2).unwrap_or(0) as f64;

        [v(0), v(1), v(2), v(3)]
    }

    /// opentype.js `getPath (points)` followed by `Glyph.getPath (0, 0, 1)`: the commands with the
    /// coordinates scaled to font size 1 and y up (the double negation of opentype.js and
    /// PolygonText cancels).
    pub fn glyph_commands(&self, glyph: u16) -> Vec<Cmd> {
        let points = self.glyph_points(glyph, 0, &mut vec![]);
        let scale = 1.0 / self.units_per_em * 1.0;
        let s = |p: &Point| (p.x * scale, p.y * scale);
        let mut out = vec![];
        let mut contours: Vec<Vec<Point>> = vec![];
        let mut cur = vec![];

        for p in &points {
            cur.push(*p);

            if p.last {
                contours.push(std::mem::take(&mut cur));
            }
        }

        for contour in contours {
            let mut curr = *contour.last().unwrap();
            let mut next = contour[0];

            if curr.on_curve {
                let (x, y) = s(&curr);
                out.push(Cmd::M(x, y));
            } else if next.on_curve {
                let (x, y) = s(&next);
                out.push(Cmd::M(x, y));
            } else {
                let (x, y) = s(&Point { x: (curr.x + next.x) * 0.5, y: (curr.y + next.y) * 0.5, on_curve: true, last: false });
                out.push(Cmd::M(x, y));
            }

            for i in 0..contour.len() {
                curr = next;
                next = contour[(i + 1) % contour.len()];

                if curr.on_curve {
                    let (x, y) = s(&curr);
                    out.push(Cmd::L(x, y));
                } else {
                    let next2 = if next.on_curve { next } else { Point { x: (curr.x + next.x) * 0.5, y: (curr.y + next.y) * 0.5, on_curve: true, last: false } };
                    let (x1, y1) = s(&curr);
                    let (x, y) = s(&next2);
                    out.push(Cmd::Q(x1, y1, x, y));
                }
            }
        }

        out
    }

    /// PolygonText.getGlyphGeometry: the triangles (three vertices each, x/y in em units) libtess
    /// produces for the glyph's contours, cached per glyph and quality.
    pub fn glyph_geometry(&self, glyph: u16, quality: PrimitiveQuality) -> std::rc::Rc<Vec<[f64; 2]>> {
        if let Some(g) = self.geometry_cache.borrow().get(&(glyph, quality)) {
            return g.clone();
        }

        let g = std::rc::Rc::new(create_glyph_geometry(&self.glyph_commands(glyph), quality));

        self.geometry_cache.borrow_mut().insert((glyph, quality), g.clone());
        g
    }
}

/// Bezier.quadric (standard/Math/Algorithms/Bezier.js), keeping the operation order of
/// `Matrix3.multLeft` and `Matrix3.multVecMatrix`.
fn bezier_quadric(p0: [f64; 2], p1: [f64; 2], p2: [f64; 2], steps: usize, points: &mut Vec<[f64; 2]>) {
    // p = c · [p0; p1; p2] with c = (1 0 0 / -2 2 0 / 1 -2 1), per column (x, then y).
    let col = |a0: f64, a1: f64, a2: f64| -> (f64, f64, f64) {
        let r0 = a0 * 1.0 + a1 * 0.0 + a2 * 0.0;
        let r1 = a0 * -2.0 + a1 * 2.0 + a2 * 0.0;
        let r2 = a0 * 1.0 + a1 * -2.0 + a2 * 1.0;
        (r0, r1, r2)
    };
    let (x0, x1, x2) = col(p0[0], p1[0], p2[0]);
    let (y0, y1, y2) = col(p0[1], p1[1], p2[1]);
    let d = (steps - 1) as f64;

    for i in 0..steps {
        let t = i as f64 / d;
        let tt = t * t;

        points.push([1.0 * x0 + t * x1 + tt * x2, 1.0 * y0 + t * y1 + tt * y2]);
    }
}

/// PolygonText.createGlyphGeometry.
pub fn create_glyph_geometry(commands: &[Cmd], quality: PrimitiveQuality) -> Vec<[f64; 2]> {
    let steps = quality.bezier_steps();
    let mut contours: Vec<Vec<[f64; 2]>> = vec![];
    let mut points: Vec<[f64; 2]> = vec![];
    let (mut x, mut y) = (0.0f64, 0.0f64);

    // `path .commands .push ({ type: "Z" })` makes sure the last contour is added.
    let flush = |points: &mut Vec<[f64; 2]>, contours: &mut Vec<Vec<[f64; 2]>>| {
        if points.len() > 2 {
            contours.push(std::mem::take(points));
        }

        points.clear();
    };

    for c in commands {
        match *c {
            Cmd::M(cx, cy) => {
                flush(&mut points, &mut contours);
                points.push([cx, cy]);
                x = cx;
                y = cy;
            }
            Cmd::L(cx, cy) => {
                points.push([cx, cy]);
                x = cx;
                y = cy;
            }
            Cmd::Q(x1, y1, cx, cy) => {
                bezier_quadric([x, y], [x1, y1], [cx, cy], steps, &mut points);
                x = cx;
                y = cy;
            }
        }
    }

    flush(&mut points, &mut contours);
    libtess::triangulate(&contours)
}
