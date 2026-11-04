//! opentype.js 2.0.0 `parseCFFTable` and `parseCFFCharstring` (opentype.mjs, "CFF" section): the `CFF ` and `CFF2` tables as X_ITE reads
//! them, quirks included. Plain CFF, CID-keyed CFF (FDSelect / FDArray, per-FD widths and local subrs) and CFF2 all go through here.
//!
//! * Every error text is the text of the exception opentype.js throws (`Error: ...`, `RangeError: ...`, `TypeError: ...`), because X_ITE logs
//!   it for a font that does not parse (`FileLoader`: `Couldn't load URL '...'. Error: ...`).
//! * The reads run over the whole font buffer with the table offset as `start`, like the JavaScript `DataView`; a read past the end of the buffer
//!   is the `RangeError` of `DataView`.
//! * A glyph's path is built on first use (`cffGlyphLoader`): `console .error` / `console .warn` calls of that build are returned as log lines.
//! * A glyph's width is taken from the charstring only when the charstring has a width argument or a stem hint (`haveWidth`); otherwise the
//!   `hmtx` advance stays. CFF2 charstrings carry no width: a stem hint sets `haveWidth` with the CFF2 `defaultWidthX` of 0.

use crate::font::Cmd;
use std::collections::HashMap;

const RANGE: &str = "RangeError: Offset is outside the bounds of the DataView";
const MAX_CALL_DEPTH: usize = 10;

type R<T> = Result<T, String>;

fn err<T>(msg: impl Into<String>) -> R<T> {
    Err(format!("Error: {}", msg.into()))
}

fn byte(d: &[u8], o: usize) -> R<u8> {
    d.get(o).copied().ok_or_else(|| RANGE.to_string())
}

fn card16(d: &[u8], o: usize) -> R<u32> {
    Ok(((byte(d, o)? as u32) << 8) | byte(d, o.saturating_add(1))? as u32)
}

fn ulong(d: &[u8], o: usize) -> R<u32> {
    Ok((card16(d, o)? << 16) | card16(d, o.saturating_add(2))?)
}

/// `Parser.getOffset`: `offsetSize` big-endian bytes.
fn offset(d: &[u8], o: usize, size: usize) -> R<usize> {
    let mut v: u64 = 0;

    for i in 0..size {
        v = (v << 8) | byte(d, o.saturating_add(i))? as u64;
    }

    Ok(v as usize)
}

/// `Parser.getBytes (data, start, end)`: the bytes `[start, end)`, empty for `end <= start`.
fn bytes(d: &[u8], start: usize, end: usize) -> R<(usize, usize)> {
    if end > start && end > d.len() {
        return Err(RANGE.to_string());
    }

    Ok((start, end.max(start)))
}

struct Index {
    /// Absolute byte ranges of the objects.
    objects: Vec<(usize, usize)>,
    end: usize,
}

/// `parseCFFIndex`.
fn parse_index(d: &[u8], start: usize, version: u8) -> R<Index> {
    let (count, count_len) = if version > 1 {
        (ulong(d, start)? as usize, 4)
    } else {
        (card16(d, start)? as usize, 2)
    };
    let mut offsets = vec![];
    let (object_offset, end);

    if count != 0 {
        let size = byte(d, start.saturating_add(count_len))? as usize;

        // Every offset takes a byte or more, so JavaScript's loop reads past the buffer (RangeError) before it finishes; a zero offset size
        // would loop for ever there. Either way a count beyond the buffer is the RangeError here.
        if count > d.len() {
            return Err(RANGE.to_string());
        }

        object_offset = start
            .saturating_add((count + 1).saturating_mul(size))
            .saturating_add(count_len);

        let mut pos = start.saturating_add(count_len + 1);

        for _ in 0..count + 1 {
            offsets.push(offset(d, pos, size)?);
            pos = pos.saturating_add(size);
        }

        end = object_offset.saturating_add(offsets[count]);
    } else {
        object_offset = 0;
        end = start.saturating_add(count_len);
    }

    let mut objects = vec![];

    for i in 0..offsets.len().saturating_sub(1) {
        objects.push(bytes(
            d,
            object_offset.saturating_add(offsets[i]),
            object_offset.saturating_add(offsets[i + 1]),
        )?);
    }

    Ok(Index { objects, end })
}

/// `parseCFFDict`: operator (`1200 + b` for an escaped one) to operands. A repeated operator that already holds a number throws, like
/// `entriesToObject`.
fn parse_dict(d: &[u8], start: usize, size: usize, version: u8) -> R<HashMap<u32, Vec<f64>>> {
    let mut o = HashMap::new();
    let mut operands: Vec<f64> = vec![];
    let operands_start = if version < 2 { 22 } else { 28 };
    let mut pos = start;

    while pos - start < size {
        let mut op = byte(d, pos)? as u32;

        pos += 1;

        if op < operands_start {
            if op == 12 {
                op = 1200 + byte(d, pos)? as u32;
                pos += 1;
            }

            if version > 1 && op == 23 {
                // parseBlend
                let n = operands.pop().unwrap_or(f64::NAN);

                while operands.len() as f64 > n {
                    operands.pop();
                }

                continue;
            }

            if o.get(&op).is_some_and(|v: &Vec<f64>| v.len() == 1) {
                return err(format!("Object [object Object] already has key {op}"));
            }

            o.insert(op, std::mem::take(&mut operands));
        } else {
            operands.push(parse_operand(d, &mut pos, op as u8)?);
        }
    }

    Ok(o)
}

fn parse_operand(d: &[u8], pos: &mut usize, b0: u8) -> R<f64> {
    let next = |pos: &mut usize| -> R<i32> {
        let b = byte(d, *pos)?;

        *pos += 1;
        Ok(b as i32)
    };

    Ok(match b0 {
        28 => {
            let (b1, b2) = (next(pos)?, next(pos)?);

            (b1 << 8 | b2) as f64
        }
        29 => {
            let (b1, b2, b3, b4) = (next(pos)?, next(pos)?, next(pos)?, next(pos)?);

            (b1 << 24 | b2 << 16 | b3 << 8 | b4) as f64
        }
        30 => {
            // parseFloatOperand
            let mut s = String::new();

            loop {
                let b = next(pos)?;

                for n in [b >> 4, b & 15] {
                    match n {
                        0..=9 => s.push(char::from(b'0' + n as u8)),
                        10 => s.push('.'),
                        11 => s.push('E'),
                        12 => s.push_str("E-"),
                        14 => s.push('-'),
                        15 => return Ok(s.parse().unwrap_or(f64::NAN)),
                        _ => {}
                    }
                }
            }
        }
        32..=246 => b0 as f64 - 139.0,
        247..=250 => ((b0 as i32 - 247) * 256 + next(pos)? + 108) as f64,
        251..=254 => (-(b0 as i32 - 251) * 256 - next(pos)? - 108) as f64,
        _ => return err(format!("Invalid b0 {b0}")),
    })
}

/// `entry[j]` of a field with array type: `dict [op] [j]`, which is `undefined` for a single operand (a number).
fn arr(dict: &HashMap<u32, Vec<f64>>, op: u32, j: usize) -> Option<f64> {
    dict.get(&op)
        .filter(|v| v.len() != 1)
        .and_then(|v| v.get(j).copied())
}

/// A field with number or offset type: the single operand, `None` when the operator is absent.
fn num(dict: &HashMap<u32, Vec<f64>>, op: u32) -> Option<f64> {
    dict.get(&op)
        .map(|v| if v.len() == 1 { v[0] } else { f64::NAN })
}

fn calc_bias(n: usize) -> f64 {
    if n < 1240 {
        107.0
    } else if n < 33900 {
        1131.0
    } else {
        32768.0
    }
}

/// A Font DICT (CFF2) or a top DICT of the FDArray (CID-keyed CFF1): what a charstring needs of its Font DICT.
struct Fd {
    /// `_subrs` (`None` where opentype.js leaves it undefined: a CFF2 Font DICT without a Subrs entry).
    subrs: Option<Vec<(usize, usize)>>,
    bias: f64,
    default_width: f64,
    nominal_width: f64,
    /// CFF2: `fdDict ._privateDict` exists (`_privateDict .vsindex` is read per glyph).
    vsindex: Option<f64>,
}

struct VarSubtable {
    region_indexes: usize,
}

pub struct Cff {
    version: u8,
    cid: bool,
    gsubrs: Vec<(usize, usize)>,
    gbias: f64,
    /// `topDict ._subrs` of a plain CFF font.
    subrs: Vec<(usize, usize)>,
    subrs_bias: f64,
    default_width: f64,
    nominal_width: f64,
    fd_array: Vec<Fd>,
    fd_select: Option<Vec<usize>>,
    /// `topDict ._vstore .itemVariationStore .itemVariationSubtables` (`None`: no VariationStore, `_vstore` is undefined).
    vstore: Option<Vec<VarSubtable>>,
    charstrings: Vec<(usize, usize)>,
}

/// What a glyph's path build gives: the commands, the charstring width (when it set one) and the lines of `console .error` / `.warn`.
pub struct Outline {
    pub cmds: Vec<Cmd>,
    pub width: Option<f64>,
    /// A plain CFF `endchar` with four arguments (an accented composite): opentype.js builds the path from two other glyphs; the caller keeps
    /// its own outline for those.
    pub seac: bool,
    pub log: Vec<String>,
}

impl Cff {
    pub fn is_cid(&self) -> bool {
        self.cid
    }

    pub fn num_glyphs(&self) -> usize {
        self.charstrings.len()
    }

    /// `parseCFFTable (data, start, font, opt)` for the table at `start`; `num_glyphs` is `font .numGlyphs` (the `maxp` count). The second value
    /// is the `console` output of the parse.
    pub fn parse(d: &[u8], start: usize, num_glyphs: usize) -> R<(Cff, Vec<String>)> {
        let mut log = vec![];
        let major = byte(d, start)?;
        let minor = byte(d, start + 1)?;

        if major > 2 {
            return err(format!("Unsupported CFF table version {major}.{minor}"));
        }

        let size = byte(d, start + 2)? as usize;
        let version = major;
        let top_dict_length = if version < 2 {
            0
        } else {
            card16(d, start + 3)? as usize
        };
        let mut n_strings = 0;
        let (top_objects, gsub_start);

        if version < 2 {
            let names = parse_index(d, start + 4, 1)?;
            let tops = parse_index(d, names.end, 1)?;
            let strs = parse_index(d, tops.end, 1)?;

            n_strings = strs.objects.len();
            gsub_start = strs.end;
            top_objects = tops.objects;
        } else {
            gsub_start = start.saturating_add(size + top_dict_length);
            top_objects = vec![];
        }

        let gsubrs = parse_index(d, gsub_start, version)?;
        let gbias = calc_bias(gsubrs.objects.len());
        // gatherCFFTopDicts: one top DICT (CFF1), or the DICT right after the header (CFF2).
        let top_ranges: Vec<(usize, usize)> = if version > 1 {
            vec![bytes(
                d,
                start.saturating_add(size),
                start.saturating_add(size + top_dict_length),
            )?]
        } else {
            top_objects
        };

        if version < 2 && top_ranges.len() != 1 {
            return err(format!(
                "CFF table has too many fonts in 'FontSet' - count of fonts NameIndex.length = {}",
                top_ranges.len()
            ));
        }

        let mut tops = vec![];

        for (s, e) in &top_ranges {
            tops.push(Self::gather_top(d, start, *s, *e, version)?);
        }

        let top = &tops[0];
        let sid_ok = |sid: Option<f64>| -> bool {
            match sid {
                // `getCFFString (strings, null)` is `cffStandardStrings [null]`, which is undefined
                None => false,
                Some(s) if s <= 390.0 => true,
                Some(s) => (s - 391.0) >= 0.0 && ((s - 391.0) as usize) < n_strings,
            }
        };
        let cid = version < 2 && sid_ok(arr(&top.dict, 1230, 0)) && sid_ok(arr(&top.dict, 1230, 1));
        let mut fd_array = vec![];
        let mut fd_select = None;
        let mut vstore = None;
        let off = |v: Option<f64>| v.unwrap_or(0.0);

        if version > 1 {
            let fd_array_offset = num(&top.dict, 1236);
            let fd_select_offset = num(&top.dict, 1237);

            let Some(fa) = fd_array_offset.filter(|v| *v != 0.0 && !v.is_nan()) else {
                return err("This is a CFF2 font, but FDArray information is missing");
            };
            let idx = parse_index(d, start.saturating_add(fa as usize), 2)?;

            for (s, e) in &idx.objects {
                fd_array.push(Self::gather_cff2_font_dict(d, start, *s, *e)?);
            }

            if let Some(fs) = fd_select_offset.filter(|v| *v != 0.0) {
                fd_select = Some(parse_fd_select(
                    d,
                    start.saturating_add(fs as usize),
                    num_glyphs,
                    fd_array.len(),
                    2,
                )?);
            }
        } else if cid {
            let fa = num(&top.dict, 1236);
            let fs = num(&top.dict, 1237);

            if fa == Some(0.0) || fs == Some(0.0) {
                return err(
                    "Font is marked as a CID font, but FDArray and/or FDSelect information is missing",
                );
            }

            // an absent offset is `null`: `null + start` is `start`
            let idx = parse_index(d, start.saturating_add(off(fa) as usize), 1)?;

            for (s, e) in &idx.objects {
                let t = Self::gather_top(d, start, *s, *e, 1)?;

                fd_array.push(Fd {
                    subrs: Some(t.subrs),
                    bias: t.bias,
                    default_width: t.default_width,
                    nominal_width: t.nominal_width,
                    vsindex: None,
                });
            }

            fd_select = Some(parse_fd_select(
                d,
                start.saturating_add(off(fs) as usize),
                num_glyphs,
                fd_array.len(),
                1,
            )?);
        }

        let (mut subrs, mut subrs_bias, mut default_width, mut nominal_width) =
            (vec![], 0.0, 0.0, 0.0);

        if version < 2 {
            // the Private DICT of the top DICT (an empty one for a CID-keyed font): `font .subrs`, `font .defaultWidthX`
            let p = arr(&top.dict, 18, 1).unwrap_or(0.0);
            let ps = arr(&top.dict, 18, 0).unwrap_or(0.0);

            parse_dict(d, start.saturating_add(p as usize), ps as usize, 1)?;
            subrs = top.subrs.clone();
            subrs_bias = top.bias;
            default_width = top.default_width;
            nominal_width = top.nominal_width;
        }

        let cs_offset = off(num(&top.dict, 17));
        let charstrings = parse_index(d, start.saturating_add(cs_offset as usize), version)?;
        let n_glyphs = charstrings.objects.len();

        if version > 1 && n_glyphs != num_glyphs {
            log.push(format!(
                "error: Glyph count in the CFF2 table ({n_glyphs}) must correspond to the glyph count in the maxp table ({num_glyphs})"
            ));
        }

        if version < 2 {
            // The charset and the encoding are only read for their errors (X_ITE reads neither).
            let charset = off(num(&top.dict, 15)) as usize;

            if charset > 2 {
                parse_charset(d, start.saturating_add(charset), n_glyphs)?;
            }

            let encoding = off(num(&top.dict, 16)) as usize;

            if encoding > 1 {
                parse_encoding(d, start.saturating_add(encoding))?;
            }
        }

        if let Some(vs) = num(&top.dict, 24).filter(|v| *v != 0.0) {
            vstore = Some(parse_variation_store(d, start.saturating_add(vs as usize))?);
        }

        Ok((
            Cff {
                version,
                cid,
                gsubrs: gsubrs.objects,
                gbias,
                subrs,
                subrs_bias,
                default_width,
                nominal_width,
                fd_array,
                fd_select,
                vstore,
                charstrings: charstrings.objects,
            },
            log,
        ))
    }

    /// `gatherCFFTopDicts` for one DICT (also each Font DICT of a CID-keyed CFF1 font's FDArray).
    fn gather_top(d: &[u8], start: usize, s: usize, e: usize, version: u8) -> R<Top> {
        let dict = parse_dict(d, s, e - s, version)?;
        let mut t = Top {
            dict,
            subrs: vec![],
            bias: 0.0,
            default_width: 0.0,
            nominal_width: 0.0,
        };
        let (psize, poff) = if version < 2 {
            (
                arr(&t.dict, 18, 0).unwrap_or(0.0),
                arr(&t.dict, 18, 1).unwrap_or(0.0),
            )
        } else {
            (0.0, 0.0)
        };

        if psize != 0.0 && poff != 0.0 {
            let pdict = parse_dict(
                d,
                (poff as usize).saturating_add(start),
                psize as usize,
                version,
            )?;

            t.default_width = num(&pdict, 20).unwrap_or(0.0);
            t.nominal_width = num(&pdict, 21).unwrap_or(0.0);

            let subrs = num(&pdict, 19).unwrap_or(0.0);

            if subrs != 0.0 {
                let idx = parse_index(d, ((poff + subrs) as usize).saturating_add(start), version)?;

                t.bias = calc_bias(idx.objects.len());
                t.subrs = idx.objects;
            }
        }

        Ok(t)
    }

    /// `gatherCFF2FontDicts`, one Font DICT.
    fn gather_cff2_font_dict(d: &[u8], start: usize, s: usize, e: usize) -> R<Fd> {
        let fdict = parse_dict(d, s, e - s, 2)?;
        let psize = arr(&fdict, 18, 0).unwrap_or(0.0);
        let poff = arr(&fdict, 18, 1).unwrap_or(0.0);
        let mut fd = Fd {
            subrs: None,
            bias: 0.0,
            default_width: 0.0,
            nominal_width: 0.0,
            vsindex: None,
        };

        if psize != 0.0 && poff != 0.0 {
            let pdict = parse_dict(d, (poff as usize).saturating_add(start), psize as usize, 2)?;

            if let Some(subrs) = num(&pdict, 19).filter(|v| *v != 0.0) {
                let idx = parse_index(d, ((poff + subrs) as usize).saturating_add(start), 2)?;

                fd.bias = calc_bias(idx.objects.len());
                fd.subrs = Some(idx.objects);
            }

            fd.vsindex = Some(num(&pdict, 22).unwrap_or(0.0));
        }

        Ok(fd)
    }

    /// `glyph .path ()` of glyph `gid`: `parseCFFCharstring (font, glyph, code, version)`.
    pub fn glyph(&self, d: &[u8], gid: usize) -> R<Outline> {
        let Some(&(cs, ce)) = self.charstrings.get(gid) else {
            return Err("TypeError: Cannot read properties of undefined".to_string());
        };
        let mut it = Interp {
            cff: self,
            d,
            p: vec![],
            stack: vec![],
            n_stems: 0,
            have_width: false,
            open: false,
            x: 0.0,
            y: 0.0,
            subrs: None,
            bias: 0.0,
            nominal: self.nominal_width,
            width: self.default_width,
            vsindex: 0.0,
            vstore: None,
            depth: 0,
            log: vec![],
            seac: false,
            gid,
        };

        if self.cid || self.version > 1 {
            let fd_index = self.fd_select.as_ref().map(|s| s.get(gid).copied());
            // `_fdSelect [glyph .index]` is undefined past the end of the FDSelect; so is `_fdArray [undefined]`
            let fd = match fd_index {
                Some(None) => None,
                Some(Some(i)) => self.fd_array.get(i),
                None => self.fd_array.first(),
            };
            let Some(fd) = fd else {
                return Err(
                    "TypeError: Cannot read properties of undefined (reading '_subrs')".to_string(),
                );
            };

            it.subrs = fd.subrs.as_deref();
            it.bias = fd.bias;

            if self.version > 1 {
                let Some(vs) = self.vstore.as_ref() else {
                    return Err("TypeError: Cannot read properties of undefined (reading 'itemVariationStore')".to_string());
                };
                let Some(vsindex) = fd.vsindex else {
                    return Err(
                        "TypeError: Cannot read properties of undefined (reading 'vsindex')"
                            .to_string(),
                    );
                };

                it.vstore = Some(vs);
                it.vsindex = vsindex;
                it.nominal = 0.0;
                it.width = 0.0;
            } else {
                it.nominal = fd.nominal_width;
                it.width = fd.default_width;
            }
        } else {
            it.subrs = Some(&self.subrs);
            it.bias = self.subrs_bias;
        }

        it.parse(d.get(cs..ce).unwrap_or(&[]))?;

        Ok(Outline {
            cmds: it.p,
            width: it.have_width.then_some(it.width),
            seac: it.seac,
            log: it.log,
        })
    }
}

struct Top {
    dict: HashMap<u32, Vec<f64>>,
    subrs: Vec<(usize, usize)>,
    bias: f64,
    default_width: f64,
    nominal_width: f64,
}

/// `parseCFFFDSelect`.
fn parse_fd_select(
    d: &[u8],
    start: usize,
    n_glyphs: usize,
    fd_count: usize,
    version: u8,
) -> R<Vec<usize>> {
    let mut sel = vec![];
    let mut pos = start;
    let format = byte(d, pos)?;

    pos += 1;

    if format == 0 {
        for _ in 0..n_glyphs {
            let fd = byte(d, pos)? as usize;

            pos += 1;

            if fd >= fd_count {
                return err(format!(
                    "CFF table CID Font FDSelect has bad FD index value {fd} (FD count {fd_count})"
                ));
            }

            sel.push(fd);
        }
    } else if format == 3 || (version > 1 && format == 4) {
        let f4 = format == 4;
        let rd = |pos: &mut usize, size: usize| -> R<usize> {
            let v = offset(d, *pos, size)?;

            *pos += size;
            Ok(v)
        };
        let n_ranges = if f4 {
            rd(&mut pos, 4)?
        } else {
            rd(&mut pos, 2)?
        };
        let mut first = if f4 {
            rd(&mut pos, 4)?
        } else {
            rd(&mut pos, 2)?
        };

        if first != 0 {
            return err(format!(
                "CFF Table CID Font FDSelect format {format} range has bad initial GID {first}"
            ));
        }

        let mut next = 0;

        for _ in 0..n_ranges {
            let fd = if f4 {
                rd(&mut pos, 2)?
            } else {
                rd(&mut pos, 1)?
            };

            next = if f4 {
                rd(&mut pos, 4)?
            } else {
                rd(&mut pos, 2)?
            };

            if fd >= fd_count {
                return err(format!(
                    "CFF table CID Font FDSelect has bad FD index value {fd} (FD count {fd_count})"
                ));
            }

            if next > n_glyphs {
                return err(format!(
                    "CFF Table CID Font FDSelect format {version} range has bad GID {next}"
                ));
            }

            while first < next {
                sel.push(fd);
                first += 1;
            }

            first = next;
        }

        if next != n_glyphs {
            return err(format!(
                "CFF Table CID Font FDSelect format 3 range has bad final (Sentinal) GID {next}"
            ));
        }
    } else {
        return err(format!(
            "CFF Table CID Font FDSelect table has unsupported format {format}"
        ));
    }

    Ok(sel)
}

/// `parseCFFCharset`, for its errors only.
fn parse_charset(d: &[u8], start: usize, n_glyphs: usize) -> R<()> {
    let mut pos = start;
    let format = byte(d, pos)?;
    let n = n_glyphs.saturating_sub(1);

    pos += 1;

    match format {
        0 => {
            for _ in 0..n {
                card16(d, pos)?;
                pos += 2;
            }
        }
        1 | 2 => {
            let mut have = 1; // [".notdef"]

            while have <= n {
                card16(d, pos)?;

                let count = if format == 1 {
                    let c = byte(d, pos + 2)? as usize;

                    pos += 3;
                    c
                } else {
                    let c = card16(d, pos + 2)? as usize;

                    pos += 4;
                    c
                };

                have += count + 1;
            }
        }
        _ => return err(format!("Unknown charset format {format}")),
    }

    Ok(())
}

/// `parseCFFEncoding`, for its errors only.
fn parse_encoding(d: &[u8], start: usize) -> R<()> {
    let format = byte(d, start)?;

    match format {
        0 => {
            let n = byte(d, start + 1)? as usize;

            for i in 0..n {
                byte(d, start + 2 + i)?;
            }
        }
        1 => {
            let n = byte(d, start + 1)? as usize;

            for i in 0..n {
                byte(d, start + 2 + i * 2)?;
                byte(d, start + 3 + i * 2)?;
            }
        }
        _ => return err(format!("Unknown encoding format {format}")),
    }

    Ok(())
}

/// `Parser.parseVariationStore`: the number of region indexes of each ItemVariationData (what `blend` reads) after the reads the parser does.
fn parse_variation_store(d: &[u8], start: usize) -> R<Vec<VarSubtable>> {
    let length = card16(d, start)? as usize;
    let _ = length;
    let store = start + 2;
    let _format = card16(d, store)?;
    let region_list = ulong(d, store + 2)? as usize;
    let count = card16(d, store + 6)? as usize;
    let mut offsets = vec![];

    for i in 0..count {
        offsets.push(ulong(d, store + 8 + i * 4)? as usize);
    }

    // parseVariationRegionList
    let rl = store + region_list;
    let axis_count = card16(d, rl)? as usize;
    let region_count = card16(d, rl + 2)? as usize;

    for i in 0..region_count * axis_count * 3 {
        card16(d, rl + 4 + i * 2)?;
    }

    let mut out = vec![];

    for o in offsets {
        let s = store + o;
        let item_count = card16(d, s)? as usize;
        let word_delta_count = card16(d, s + 2)? as usize;
        let n = card16(d, s + 4)? as usize;

        for i in 0..n {
            card16(d, s + 6 + i * 2)?;
        }

        if item_count != 0 && n != 0 {
            let long = word_delta_count & 0x8000 != 0;
            let words = word_delta_count & 0x7fff;

            if words > n {
                return err("wordCount must be less than or equal to regionIndexCount");
            }

            let (w, r) = if long { (4, 2) } else { (2, 1) };
            let item = words * w + (n - words) * r;

            for i in 0..item_count * item {
                byte(d, s + 6 + n * 2 + i)?;
            }
        }

        out.push(VarSubtable { region_indexes: n });
    }

    Ok(out)
}

/// The JavaScript spelling of a number used as a property key in a `TypeError` message.
fn js_key(v: f64) -> String {
    if v.is_nan() {
        "NaN".into()
    } else if v == v.trunc() && v.abs() < 1e21 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

struct Interp<'a> {
    cff: &'a Cff,
    d: &'a [u8],
    p: Vec<Cmd>,
    stack: Vec<f64>,
    n_stems: usize,
    have_width: bool,
    open: bool,
    x: f64,
    y: f64,
    subrs: Option<&'a [(usize, usize)]>,
    bias: f64,
    nominal: f64,
    width: f64,
    vsindex: f64,
    vstore: Option<&'a Vec<VarSubtable>>,
    depth: usize,
    log: Vec<String>,
    seac: bool,
    gid: usize,
}

impl Interp<'_> {
    /// `stack .shift ()`: `undefined` (NaN in arithmetic) when the stack is empty.
    fn shift(&mut self) -> f64 {
        if self.stack.is_empty() {
            f64::NAN
        } else {
            self.stack.remove(0)
        }
    }

    fn pop(&mut self) -> f64 {
        self.stack.pop().unwrap_or(f64::NAN)
    }

    fn new_contour(&mut self, x: f64, y: f64) {
        // `p .closePath ()` of the previous contour adds a Z; the commands of a path that is not stroked drop it in `getPath`
        self.p.push(Cmd::M(x, y));
        self.open = true;
    }

    fn parse_stems(&mut self) {
        let has_width_arg = self.stack.len() & 1 != 0;

        if has_width_arg && !self.have_width {
            self.width = self.shift() + self.nominal;
        }

        self.n_stems += self.stack.len() >> 1;
        self.stack.clear();
        self.have_width = true;
    }

    fn line_to(&mut self) {
        self.p.push(Cmd::L(self.x, self.y));
    }

    fn curve_to(&mut self, c1x: f64, c1y: f64, c2x: f64, c2y: f64, x: f64, y: f64) {
        self.p.push(Cmd::C(c1x, c1y, c2x, c2y, x, y));
    }

    fn call(&mut self, subr: Option<(usize, usize)>, what: &str) -> R<()> {
        if let Some((s, e)) = subr {
            if self.depth >= MAX_CALL_DEPTH {
                self.log.push(format!(
                    "warn: CFF charstring subroutine call depth exceeded, skipping {what}"
                ));
                return Ok(());
            }

            self.depth += 1;
            let d = self.d;

            self.parse(d.get(s..e).unwrap_or(&[]))?;
            self.depth -= 1;
        }

        Ok(())
    }

    fn parse(&mut self, code: &[u8]) -> R<()> {
        let version = self.cff.version;
        let mut i = 0;
        let g = |i: usize| code.get(i).copied();

        while i < code.len() {
            let v = code[i] as u32;

            i += 1;

            match v {
                1 | 3 | 18 | 23 => self.parse_stems(),
                4 => {
                    if self.stack.len() > 1 && !self.have_width {
                        self.width = self.shift() + self.nominal;
                        self.have_width = true;
                    }

                    self.y += self.pop();
                    self.new_contour(self.x, self.y);
                }
                5 => {
                    while !self.stack.is_empty() {
                        self.x += self.shift();
                        self.y += self.shift();
                        self.line_to();
                    }
                }
                6 | 7 => {
                    let horizontal = v == 6;

                    while !self.stack.is_empty() {
                        if horizontal {
                            self.x += self.shift();
                        } else {
                            self.y += self.shift();
                        }

                        self.line_to();

                        if self.stack.is_empty() {
                            break;
                        }

                        if horizontal {
                            self.y += self.shift();
                        } else {
                            self.x += self.shift();
                        }

                        self.line_to();
                    }
                }
                8 => {
                    while !self.stack.is_empty() {
                        let c1x = self.x + self.shift();
                        let c1y = self.y + self.shift();
                        let c2x = c1x + self.shift();
                        let c2y = c1y + self.shift();

                        self.x = c2x + self.shift();
                        self.y = c2y + self.shift();
                        self.curve_to(c1x, c1y, c2x, c2y, self.x, self.y);
                    }
                }
                10 => {
                    let idx = self.pop() + self.bias;
                    let Some(subrs) = self.subrs else {
                        return Err(format!(
                            "TypeError: Cannot read properties of undefined (reading '{}')",
                            js_key(idx)
                        ));
                    };
                    let subr = (idx >= 0.0 && idx == idx.trunc())
                        .then(|| subrs.get(idx as usize).copied())
                        .flatten();

                    self.call(subr, "callsubr")?;
                }
                11 => {
                    if version > 1 {
                        self.log.push(
                            "error: CFF CharString operator return (11) is not supported in CFF2"
                                .into(),
                        );
                    } else {
                        return Ok(());
                    }
                }
                12 => {
                    let w = g(i);

                    i += 1;

                    match w {
                        Some(35) => {
                            let c1x = self.x + self.shift();
                            let c1y = self.y + self.shift();
                            let c2x = c1x + self.shift();
                            let c2y = c1y + self.shift();
                            let jpx = c2x + self.shift();
                            let jpy = c2y + self.shift();
                            let c3x = jpx + self.shift();
                            let c3y = jpy + self.shift();
                            let c4x = c3x + self.shift();
                            let c4y = c3y + self.shift();

                            self.x = c4x + self.shift();
                            self.y = c4y + self.shift();
                            self.shift();
                            self.curve_to(c1x, c1y, c2x, c2y, jpx, jpy);
                            self.curve_to(c3x, c3y, c4x, c4y, self.x, self.y);
                        }
                        Some(34) => {
                            let c1x = self.x + self.shift();
                            let c1y = self.y;
                            let c2x = c1x + self.shift();
                            let c2y = c1y + self.shift();
                            let jpx = c2x + self.shift();
                            let jpy = c2y;
                            let c3x = jpx + self.shift();
                            let c3y = c2y;
                            let c4x = c3x + self.shift();
                            let c4y = self.y;

                            self.x = c4x + self.shift();
                            self.curve_to(c1x, c1y, c2x, c2y, jpx, jpy);
                            self.curve_to(c3x, c3y, c4x, c4y, self.x, self.y);
                        }
                        Some(36) => {
                            let c1x = self.x + self.shift();
                            let c1y = self.y + self.shift();
                            let c2x = c1x + self.shift();
                            let c2y = c1y + self.shift();
                            let jpx = c2x + self.shift();
                            let jpy = c2y;
                            let c3x = jpx + self.shift();
                            let c3y = c2y;
                            let c4x = c3x + self.shift();
                            let c4y = c3y + self.shift();

                            self.x = c4x + self.shift();
                            self.curve_to(c1x, c1y, c2x, c2y, jpx, jpy);
                            self.curve_to(c3x, c3y, c4x, c4y, self.x, self.y);
                        }
                        Some(37) => {
                            let c1x = self.x + self.shift();
                            let c1y = self.y + self.shift();
                            let c2x = c1x + self.shift();
                            let c2y = c1y + self.shift();
                            let jpx = c2x + self.shift();
                            let jpy = c2y + self.shift();
                            let c3x = jpx + self.shift();
                            let c3y = jpy + self.shift();
                            let c4x = c3x + self.shift();
                            let c4y = c3y + self.shift();

                            if (c4x - self.x).abs() > (c4y - self.y).abs() {
                                self.x = c4x + self.shift();
                            } else {
                                self.y = c4y + self.shift();
                            }

                            self.curve_to(c1x, c1y, c2x, c2y, jpx, jpy);
                            self.curve_to(c3x, c3y, c4x, c4y, self.x, self.y);
                        }
                        _ => self.stack.clear(), // console .log ("Glyph N: unknown operator 1200v")
                    }
                }
                14 => {
                    if version > 1 {
                        self.log.push(
                            "error: CFF CharString operator endchar (14) is not supported in CFF2"
                                .into(),
                        );
                    } else {
                        if self.stack.len() >= 4 {
                            // The composite glyph of two others (`seac`). A plain CFF font keeps the outline of its caller; a CID-keyed
                            // font has no standard-encoding glyph names to look for.
                            if self.cff.cid {
                                return Err(
                                    "TypeError: Cannot read properties of undefined (reading 'path')".to_string()
                                );
                            }

                            self.seac = true;
                            self.stack.truncate(self.stack.len() - 4);
                        } else if !self.stack.is_empty() && !self.have_width {
                            self.width = self.shift() + self.nominal;
                            self.have_width = true;
                        }

                        if self.open {
                            self.open = false;
                        }
                    }
                }
                15 => {
                    if version < 2 {
                        self.log.push(
                            "error: CFF2 CharString operator vsindex (15) is not supported in CFF"
                                .into(),
                        );
                    } else {
                        self.vsindex = self.pop();
                    }
                }
                16 => {
                    if version < 2 {
                        self.log.push(
                            "error: CFF2 CharString operator blend (16) is not supported in CFF"
                                .into(),
                        );
                    } else {
                        // No variation (`font .variation` is undefined): the default values stay and the deltas go.
                        let n = self.pop();
                        let sub = (self.vsindex >= 0.0 && self.vsindex == self.vsindex.trunc())
                            .then(|| self.vstore.and_then(|s| s.get(self.vsindex as usize)))
                            .flatten();
                        let Some(sub) = sub else {
                            return Err("TypeError: Cannot read properties of undefined (reading 'regionIndexes')".to_string());
                        };
                        let delta_set_count = n * sub.region_indexes as f64;

                        if delta_set_count > 0.0 {
                            let k = (delta_set_count as usize).min(self.stack.len());

                            self.stack.truncate(self.stack.len() - k);
                        }
                    }
                }
                19 | 20 => {
                    self.parse_stems();
                    i += (self.n_stems + 7) >> 3;
                }
                21 => {
                    if self.stack.len() > 2 && !self.have_width {
                        self.width = self.shift() + self.nominal;
                        self.have_width = true;
                    }

                    self.y += self.pop();
                    self.x += self.pop();
                    self.new_contour(self.x, self.y);
                }
                22 => {
                    if self.stack.len() > 1 && !self.have_width {
                        self.width = self.shift() + self.nominal;
                        self.have_width = true;
                    }

                    self.x += self.pop();
                    self.new_contour(self.x, self.y);
                }
                24 => {
                    while self.stack.len() > 2 {
                        let c1x = self.x + self.shift();
                        let c1y = self.y + self.shift();
                        let c2x = c1x + self.shift();
                        let c2y = c1y + self.shift();

                        self.x = c2x + self.shift();
                        self.y = c2y + self.shift();
                        self.curve_to(c1x, c1y, c2x, c2y, self.x, self.y);
                    }

                    self.x += self.shift();
                    self.y += self.shift();
                    self.line_to();
                }
                25 => {
                    while self.stack.len() > 6 {
                        self.x += self.shift();
                        self.y += self.shift();
                        self.line_to();
                    }

                    let c1x = self.x + self.shift();
                    let c1y = self.y + self.shift();
                    let c2x = c1x + self.shift();
                    let c2y = c1y + self.shift();

                    self.x = c2x + self.shift();
                    self.y = c2y + self.shift();
                    self.curve_to(c1x, c1y, c2x, c2y, self.x, self.y);
                }
                26 => {
                    if self.stack.len() & 1 != 0 {
                        self.x += self.shift();
                    }

                    while !self.stack.is_empty() {
                        let c1x = self.x;
                        let c1y = self.y + self.shift();
                        let c2x = c1x + self.shift();
                        let c2y = c1y + self.shift();

                        self.x = c2x;
                        self.y = c2y + self.shift();
                        self.curve_to(c1x, c1y, c2x, c2y, self.x, self.y);
                    }
                }
                27 => {
                    if self.stack.len() & 1 != 0 {
                        self.y += self.shift();
                    }

                    while !self.stack.is_empty() {
                        let c1x = self.x + self.shift();
                        let c1y = self.y;
                        let c2x = c1x + self.shift();
                        let c2y = c1y + self.shift();

                        self.x = c2x + self.shift();
                        self.y = c2y;
                        self.curve_to(c1x, c1y, c2x, c2y, self.x, self.y);
                    }
                }
                28 => {
                    let (b1, b2) = (g(i).unwrap_or(0) as i32, g(i + 1).unwrap_or(0) as i32);

                    self.stack.push((((b1 << 24) | (b2 << 16)) >> 16) as f64);
                    i += 2;
                }
                29 => {
                    let idx = self.pop() + self.cff.gbias;
                    let subr = (idx >= 0.0 && idx == idx.trunc())
                        .then(|| self.cff.gsubrs.get(idx as usize).copied())
                        .flatten();

                    self.call(subr, "callgsubr")?;
                }
                30 | 31 => {
                    let mut first_vertical = v == 30;

                    while !self.stack.is_empty() {
                        for round in 0..2 {
                            let vertical_start = first_vertical == (round == 0);

                            if vertical_start {
                                let c1x = self.x;
                                let c1y = self.y + self.shift();
                                let c2x = c1x + self.shift();
                                let c2y = c1y + self.shift();

                                self.x = c2x + self.shift();
                                self.y = c2y
                                    + if self.stack.len() == 1 {
                                        self.shift()
                                    } else {
                                        0.0
                                    };
                                self.curve_to(c1x, c1y, c2x, c2y, self.x, self.y);
                            } else {
                                let c1x = self.x + self.shift();
                                let c1y = self.y;
                                let c2x = c1x + self.shift();
                                let c2y = c1y + self.shift();

                                self.y = c2y + self.shift();
                                self.x = c2x
                                    + if self.stack.len() == 1 {
                                        self.shift()
                                    } else {
                                        0.0
                                    };
                                self.curve_to(c1x, c1y, c2x, c2y, self.x, self.y);
                            }

                            if self.stack.is_empty() {
                                break;
                            }
                        }

                        first_vertical = v == 30;
                    }
                }
                _ => {
                    if v < 32 {
                        // console .log ("Glyph N: unknown operator V"): not an error or warning, so not in the log
                        let _ = self.gid;
                    } else if v < 247 {
                        self.stack.push(v as f64 - 139.0);
                    } else if v < 251 {
                        let b1 = g(i).unwrap_or(0) as f64;

                        i += 1;
                        self.stack.push((v as f64 - 247.0) * 256.0 + b1 + 108.0);
                    } else if v < 255 {
                        let b1 = g(i).unwrap_or(0) as f64;

                        i += 1;
                        self.stack.push(-(v as f64 - 251.0) * 256.0 - b1 - 108.0);
                    } else {
                        let b: Vec<i32> = (0..4).map(|k| g(i + k).unwrap_or(0) as i32).collect();

                        i += 4;
                        self.stack.push(
                            ((b[0] << 24) | (b[1] << 16) | (b[2] << 8) | b[3]) as f64 / 65536.0,
                        );
                    }
                }
            }
        }

        Ok(())
    }
}
