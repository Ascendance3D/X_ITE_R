//! Fonts: what X_ITE gets from opentype.js 2.0 (`OpenType.parse`) for TrueType-flavored fonts
//! (WOFF2 is decompressed first, like `X3DTextContext.decompressFont`), and the glyph geometry
//! of `Browser/Text/PolygonText.js` (`createGlyphGeometry`).
//!
//! OpenType-CFF fonts (`OTTO`, a `CFF ` or `CFF2` table) load too; their charstring outlines come from [`crate::cff`], a port of
//! opentype.js's CFF parser and Type 2 interpreter (plain, CID-keyed and CFF2), and are the `getPath` commands (`M`, `L`, `C`).
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

/// One entry of an opentype.js path (`Glyph.getPath` output: M, L, Q for TrueType and M, L, C for CFF;
/// the Z commands are only emitted for stroked paths, so there are none).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cmd {
    M(f64, f64),
    L(f64, f64),
    Q(f64, f64, f64, f64),
    /// A cubic Bezier (CFF charstrings): two control points, then the end point.
    C(f64, f64, f64, f64, f64, f64),
}

pub struct Font {
    data: Vec<u8>,
    pub units_per_em: f64,
    pub ascender: f64,
    pub descender: f64,
    pub num_glyphs: u16,
    loca: Vec<u32>,
    glyf: (usize, usize),
    /// An OpenType-CFF font: the outlines are Type 2 charstrings, `loca` and `glyf` stay empty.
    cff: bool,
    /// The parsed `CFF ` / `CFF2` table (`None` for a TrueType font).
    cff_tables: Option<crate::cff::Cff>,
    /// The path of each CFF glyph built so far (opentype.js builds it once): the commands in font units, or the exception text.
    cff_paths: RefCell<HashMap<u16, Result<std::rc::Rc<Vec<Cmd>>, String>>>,
    /// `console .error` / `.warn` lines of the parse and of the path builds, as `error: text` / `warn: text`, until [`Font::take_log`].
    log: RefCell<Vec<String>>,
    /// opentype.js `parseCFFCharstring` ends with `if (haveWidth) glyph .advanceWidth = width`: building a glyph's path
    /// replaces the advance it took from `hmtx`. Filled in by the first [`Font::glyph_commands`] of a glyph.
    cff_widths: RefCell<HashMap<u16, f64>>,
    advances: Vec<u16>,
    cmap: HashMap<u32, u16>,
    has_gpos: bool,
    kern_cache: RefCell<HashMap<(u16, u16), i32>>,
    geometry_cache: RefCell<HashMap<(u16, PrimitiveQuality), std::rc::Rc<Vec<[f64; 2]>>>>,
}

#[derive(Debug)]
pub enum FontError {
    Parse(String),
    /// The text of the exception opentype.js throws (`Error: ...`): X_ITE logs it with the url.
    OpenType(String),
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
            woff2_patched::convert_woff2_to_ttf(&mut std::io::Cursor::new(bytes))
                .map_err(|e| FontError::Parse(format!("woff2: {e:?}")))?
        } else {
            bytes.to_vec()
        };

        let face = Face::parse(&data, 0).map_err(|e| FontError::Parse(format!("{e:?}")))?;
        let raw = RawFace::parse(&data, 0).map_err(|e| FontError::Parse(format!("{e:?}")))?;
        let table = |tag: &[u8; 4]| raw.table(Tag::from_bytes(tag));
        // opentype.js `parse`: `glyf` + `loca` for TrueType outlines, else a `CFF ` table, else a `CFF2` table.
        let cff =
            table(b"glyf").is_none() && (table(b"CFF ").is_some() || table(b"CFF2").is_some());
        let glyf_tag = match table(b"glyf") {
            Some(t) => t,
            None if cff => &[][..],
            None => {
                return Err(FontError::Parse(
                    "no glyf or CFF table (only TrueType and CFF outlines are supported)".into(),
                ));
            }
        };
        let loca_tag = match table(b"loca") {
            Some(t) => t,
            None if cff => &[][..],
            None => return Err(FontError::Parse("no loca table".into())),
        };
        let head = table(b"head").ok_or_else(|| FontError::Parse("no head table".into()))?;
        let long_loca = be16s(head, 50).unwrap_or(0) != 0;
        let num_glyphs = face.number_of_glyphs();

        let mut loca = Vec::with_capacity(if cff { 0 } else { num_glyphs as usize + 1 });

        for i in 0..=num_glyphs as usize {
            if cff {
                break;
            }

            let v = if long_loca {
                let o = i * 4;
                u32::from_be_bytes([
                    *loca_tag.get(o).unwrap_or(&0),
                    *loca_tag.get(o + 1).unwrap_or(&0),
                    *loca_tag.get(o + 2).unwrap_or(&0),
                    *loca_tag.get(o + 3).unwrap_or(&0),
                ])
            } else {
                be16(loca_tag, i * 2).unwrap_or(0) as u32 * 2
            };

            loca.push(v);
        }

        let mut log = vec![];
        let cff_tables = if cff {
            let t = table(b"CFF ").or_else(|| table(b"CFF2")).unwrap_or(&[]);
            let start = t.as_ptr() as usize - data.as_ptr() as usize;
            let (c, l) = crate::cff::Cff::parse(&data, start, num_glyphs as usize)
                .map_err(FontError::OpenType)?;

            log = l;
            Some(c)
        } else {
            None
        };
        let glyf_start = if cff {
            0
        } else {
            glyf_tag.as_ptr() as usize - data.as_ptr() as usize
        };
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
                let ok = (st.platform_id == ttf_parser::PlatformId::Windows
                    && matches!(st.encoding_id, 0 | 1 | 10))
                    || (st.platform_id == ttf_parser::PlatformId::Unicode
                        && matches!(st.encoding_id, 0..=4 | 6))
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
            cff,
            cff_tables,
            cff_paths: RefCell::new(HashMap::new()),
            log: RefCell::new(log),
            cff_widths: RefCell::new(HashMap::new()),
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
        if let Some(w) = self.cff_widths.borrow().get(&glyph) {
            return *w;
        }

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
        let Ok(face) = Face::parse(&self.data, 0) else {
            return 0;
        };
        let (l, r) = (ttf_parser::GlyphId(left), ttf_parser::GlyphId(right));

        if self.has_gpos {
            let Some(gpos) = face.tables().gpos else {
                return 0;
            };
            let default_script = if gpos.scripts.find(Tag::from_bytes(b"DFLT")).is_some() {
                Tag::from_bytes(b"DFLT")
            } else if gpos.scripts.find(Tag::from_bytes(b"latn")).is_some() {
                Tag::from_bytes(b"latn")
            } else {
                // getDefaultScriptName () is undefined: getScriptTable (undefined) → "DFLT".
                Tag::from_bytes(b"DFLT")
            };
            let Some(script) = gpos.scripts.find(default_script) else {
                return 0;
            };
            let Some(lang) = script.default_language else {
                return 0;
            };

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
                let Some(lookup) = gpos.lookups.get(li) else {
                    continue;
                };

                for subtable in lookup
                    .subtables
                    .into_iter::<ttf_parser::gpos::PositioningSubtable>()
                {
                    let ttf_parser::gpos::PositioningSubtable::Pair(pair) = subtable else {
                        continue;
                    };

                    match pair {
                        ttf_parser::gpos::PairAdjustment::Format1 { coverage, sets } => {
                            let Some(cov) = coverage.get(l) else { continue };
                            let Some(set) = sets.get(cov) else { continue };

                            if let Some((v1, _)) = set.get(r) {
                                return v1.x_advance as i32;
                            }
                        }
                        ttf_parser::gpos::PairAdjustment::Format2 {
                            coverage,
                            classes,
                            matrix,
                        } => {
                            if coverage.get(l).is_none() {
                                continue;
                            }

                            let c1 = classes.0.get(l);
                            let c2 = classes.1.get(r);

                            return matrix
                                .get((c1, c2))
                                .map(|(v1, _)| v1.x_advance as i32)
                                .unwrap_or(0);
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
        let (o, n) = (
            self.loca[glyph as usize] as usize,
            self.loca[glyph as usize + 1] as usize,
        );

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

            let mut points: Vec<Point> = flags
                .iter()
                .enumerate()
                .map(|(i, f)| Point {
                    x: 0.0,
                    y: 0.0,
                    on_curve: f & 1 != 0,
                    last: ends.contains(&i),
                })
                .collect();
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

            let mut c = Component {
                index,
                x_scale: 1.0,
                scale01: 0.0,
                scale10: 0.0,
                y_scale: 1.0,
                dx: 0.0,
                dy: 0.0,
                matched: None,
            };

            if flags & 1 != 0 {
                if flags & 2 != 0 {
                    c.dx = be16s(d, pos).unwrap_or(0) as f64;
                    c.dy = be16s(d, pos + 2).unwrap_or(0) as f64;
                } else {
                    c.matched = Some((
                        be16(d, pos).unwrap_or(0) as usize,
                        be16(d, pos + 2).unwrap_or(0) as usize,
                    ));
                }

                pos += 4;
            } else {
                if flags & 2 != 0 {
                    c.dx = *d.get(pos).unwrap_or(&0) as i8 as f64;
                    c.dy = *d.get(pos + 1).unwrap_or(&0) as i8 as f64;
                } else {
                    c.matched = Some((
                        *d.get(pos).unwrap_or(&0) as usize,
                        *d.get(pos + 1).unwrap_or(&0) as usize,
                    ));
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
                    let (Some(first), Some(second)) = (points.get(a), sub.get(b)) else {
                        continue;
                    };
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
        // A CFF glyph has no `xMin` and friends in opentype.js (ScreenText reads `glyph .xMin || 0`).
        if glyph >= self.num_glyphs || self.cff {
            return [0.0; 4];
        }

        let (start, len) = self.glyf;
        let glyf = &self.data[start..start + len];
        let (o, n) = (
            self.loca[glyph as usize] as usize,
            self.loca[glyph as usize + 1] as usize,
        );

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
        if self.cff {
            return self.cff_commands(glyph);
        }

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
                let (x, y) = s(&Point {
                    x: (curr.x + next.x) * 0.5,
                    y: (curr.y + next.y) * 0.5,
                    on_curve: true,
                    last: false,
                });
                out.push(Cmd::M(x, y));
            }

            for i in 0..contour.len() {
                curr = next;
                next = contour[(i + 1) % contour.len()];

                if curr.on_curve {
                    let (x, y) = s(&curr);
                    out.push(Cmd::L(x, y));
                } else {
                    let next2 = if next.on_curve {
                        next
                    } else {
                        Point {
                            x: (curr.x + next.x) * 0.5,
                            y: (curr.y + next.y) * 0.5,
                            on_curve: true,
                            last: false,
                        }
                    };
                    let (x1, y1) = s(&curr);
                    let (x, y) = s(&next2);
                    out.push(Cmd::Q(x1, y1, x, y));
                }
            }
        }

        out
    }

    /// `parseCFFCharstring` + `Glyph.getPath (0, 0, 1)` of a CFF glyph: `M`, `L` and `C` commands scaled to font size 1.
    /// The path is built on first use in opentype.js, and that replaces the glyph's advance by the charstring width (when the charstring
    /// has one) and writes the build's `console` lines. A build that throws leaves no commands (see [`Font::glyph_error`]).
    fn cff_commands(&self, glyph: u16) -> Vec<Cmd> {
        if glyph >= self.num_glyphs {
            return vec![];
        }

        let scale = 1.0 / self.units_per_em * 1.0;
        let scaled = |cmds: &[Cmd]| -> Vec<Cmd> {
            cmds.iter()
                .map(|c| match *c {
                    Cmd::M(x, y) => Cmd::M(x * scale, y * scale),
                    Cmd::L(x, y) => Cmd::L(x * scale, y * scale),
                    Cmd::Q(a, b, x, y) => Cmd::Q(a * scale, b * scale, x * scale, y * scale),
                    Cmd::C(a, b, c, d, x, y) => Cmd::C(
                        a * scale,
                        b * scale,
                        c * scale,
                        d * scale,
                        x * scale,
                        y * scale,
                    ),
                })
                .collect()
        };

        if let Some(done) = self.cff_paths.borrow().get(&glyph) {
            return done.as_ref().map(|c| scaled(c)).unwrap_or_default();
        }

        let Some(tables) = &self.cff_tables else {
            return vec![];
        };
        let result = tables
            .glyph(&self.data, glyph as usize)
            .map(|mut o| {
                self.log.borrow_mut().append(&mut o.log);

                if let Some(w) = o.width {
                    self.cff_widths.borrow_mut().entry(glyph).or_insert(w);
                }

                if o.seac {
                    self.cff_seac_commands(glyph)
                } else {
                    o.cmds
                }
            })
            .map(std::rc::Rc::new);
        let out = result.as_ref().map(|c| scaled(c)).unwrap_or_default();

        self.cff_paths.borrow_mut().insert(glyph, result);
        out
    }

    /// An accented composite (`endchar` with four arguments) of a plain CFF font: the base glyph's and the shifted accent's outlines.
    /// opentype.js builds it from the two glyphs' paths; ttf-parser's interpreter does the same lookup by standard-encoding name.
    fn cff_seac_commands(&self, glyph: u16) -> Vec<Cmd> {
        struct Out {
            cmds: Vec<Cmd>,
        }

        impl ttf_parser::OutlineBuilder for Out {
            fn move_to(&mut self, x: f32, y: f32) {
                self.cmds.push(Cmd::M(x as f64, y as f64));
            }

            fn line_to(&mut self, x: f32, y: f32) {
                self.cmds.push(Cmd::L(x as f64, y as f64));
            }

            fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
                self.cmds
                    .push(Cmd::Q(x1 as f64, y1 as f64, x as f64, y as f64));
            }

            fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
                self.cmds.push(Cmd::C(
                    x1 as f64, y1 as f64, x2 as f64, y2 as f64, x as f64, y as f64,
                ));
            }

            // `Path.closePath` Z commands are dropped by `getPath` (no stroke).
            fn close(&mut self) {}
        }

        let Ok(face) = Face::parse(&self.data, 0) else {
            return vec![];
        };
        let mut out = Out { cmds: vec![] };

        let _ = face.outline_glyph(ttf_parser::GlyphId(glyph), &mut out);
        out.cmds
    }

    /// `font .isCIDFont`: a CID-keyed CFF font (a `ROS` entry in the top DICT).
    pub fn is_cid_keyed(&self) -> bool {
        self.cff_tables.as_ref().is_some_and(|c| c.is_cid())
    }

    /// The exception text of a CFF glyph whose path build threw (`TypeError: ...`), once the path has been asked for.
    pub fn glyph_error(&self, glyph: u16) -> Option<String> {
        self.glyph_commands(glyph);
        self.cff_paths
            .borrow()
            .get(&glyph)
            .and_then(|r| r.as_ref().err().cloned())
    }

    /// `console .error` / `.warn` lines written since the last call, as `error: text` / `warn: text`: the CFF parse (at load) and the path builds.
    pub fn take_log(&self) -> Vec<String> {
        std::mem::take(&mut *self.log.borrow_mut())
    }

    /// PolygonText.getGlyphGeometry: the triangles (three vertices each, x/y in em units) libtess
    /// produces for the glyph's contours, cached per glyph and quality.
    pub fn glyph_geometry(
        &self,
        glyph: u16,
        quality: PrimitiveQuality,
    ) -> std::rc::Rc<Vec<[f64; 2]>> {
        if let Some(g) = self.geometry_cache.borrow().get(&(glyph, quality)) {
            return g.clone();
        }

        let g = std::rc::Rc::new(create_glyph_geometry(&self.glyph_commands(glyph), quality));

        self.geometry_cache
            .borrow_mut()
            .insert((glyph, quality), g.clone());
        g
    }
}

/// Bezier.quadric (standard/Math/Algorithms/Bezier.js), keeping the operation order of
/// `Matrix3.multLeft` and `Matrix3.multVecMatrix`.
fn bezier_quadric(
    p0: [f64; 2],
    p1: [f64; 2],
    p2: [f64; 2],
    steps: usize,
    points: &mut Vec<[f64; 2]>,
) {
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

/// Bezier.cubic (standard/Math/Algorithms/Bezier.js): `p = Matrix4 (x0 y0 z0 0, ..., x3 y3 z3 0) .multLeft (c)`, then
/// `p .multVecMatrix (Vector4 (1, t, t², t³))`, with the operation order of `Matrix4.multLeft` and `multVecMatrix`.
fn bezier_cubic(
    p0: [f64; 2],
    p1: [f64; 2],
    p2: [f64; 2],
    p3: [f64; 2],
    steps: usize,
    points: &mut Vec<[f64; 2]>,
) {
    const C: [f64; 16] = [
        1.0, 0.0, 0.0, 0.0, -3.0, 3.0, 0.0, 0.0, 3.0, -6.0, 3.0, 0.0, -1.0, 3.0, -3.0, 1.0,
    ];
    // a [4 k + j]: coordinate j of point k (z and w are 0).
    let a = [
        p0[0], p0[1], 0.0, 0.0, p1[0], p1[1], 0.0, 0.0, p2[0], p2[1], 0.0, 0.0, p3[0], p3[1], 0.0,
        0.0,
    ];
    // Matrix4.multLeft: this [4 r + j] = a00 * b0 + a04 * b1 + a08 * b2 + a12 * b3.
    let mut m = [0.0f64; 16];

    for r in 0..4 {
        for j in 0..4 {
            m[4 * r + j] = a[j] * C[4 * r]
                + a[4 + j] * C[4 * r + 1]
                + a[8 + j] * C[4 * r + 2]
                + a[12 + j] * C[4 * r + 3];
        }
    }

    let d = (steps - 1) as f64;

    for i in 0..steps {
        let t = i as f64 / d;
        let tt = t * t;
        let ttt = t * t * t;
        let x = 1.0 * m[0] + t * m[4] + tt * m[8] + ttt * m[12];
        let y = 1.0 * m[1] + t * m[5] + tt * m[9] + ttt * m[13];

        points.push([x, y]);
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
            Cmd::C(x1, y1, x2, y2, cx, cy) => {
                bezier_cubic([x, y], [x1, y1], [x2, y2], [cx, cy], steps, &mut points);
                x = cx;
                y = cy;
            }
        }
    }

    flush(&mut points, &mut contours);
    libtess::triangulate(&contours)
}
