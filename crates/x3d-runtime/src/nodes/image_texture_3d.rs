//! ImageTexture3D (Components/Texturing3D/ImageTexture3D.js) with the file parsers it calls:
//! Browser/Texturing3D/NRRDParser.js and DICOMParser.js (the latter reads through the dicom-parser
//! library, `dicomParser.parseDicom`; see `dicom.rs` in this module's sibling code below).
//!
//! The parsers are pure: bytes in, a [`Volume`] (width, height, depth, components and the 8 bit
//! samples X_ITE hands to `texImage3D`) or the JavaScript error the parse throws, as the text
//! `String (error)` has (`Error: …`, `RangeError: …`). The renderer turns a [`Volume`] into a
//! texture (x3d-render's `texture::image_texture_3d`).

use crate::*;
use std::rc::Rc;

/// What a parser hands to `setTextureData` and `updateOutputs`. The sizes are JavaScript numbers:
/// NaN stands for the `undefined` of a header without `sizes`.
#[derive(Clone, Debug)]
pub struct Volume {
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    pub components: f64,
    /// `None`: the parser set no data (an NRRD without `encoding`): `texImage3D` gets null.
    pub data: Option<Vec<u8>>,
}

/// gl.MAX_3D_TEXTURE_SIZE of the oracle's GPU (NVIDIA under ANGLE: 16384, read with
/// `getParameter` on the oracle's Chrome).
pub const MAX_3D_TEXTURE_SIZE: f64 = 16384.0;

// ---- JavaScript string helpers -------------------------------------------------------------

/// The characters `\s` and `String.prototype.trim` take as white space and line terminators.
fn is_js_space(c: u16) -> bool {
    matches!(c, 0x09..=0x0d | 0x20 | 0xa0 | 0x1680 | 0x2000..=0x200a | 0x2028 | 0x2029 | 0x202f | 0x205f | 0x3000 | 0xfeff)
}

fn is_word(c: u16) -> bool {
    matches!(c, 0x30..=0x39 | 0x41..=0x5a | 0x61..=0x7a | 0x5f)
}

/// `.` of a RegExp without the s flag.
fn is_line_terminator(c: u16) -> bool {
    matches!(c, 0x0a | 0x0d | 0x2028 | 0x2029)
}

fn units_to_string(u: &[u16]) -> String {
    String::from_utf16_lossy(u)
}

/// `String.prototype.trim`.
fn js_trim(s: &str) -> String {
    s.trim_matches(|c: char| is_js_space(c as u32 as u16) && (c as u32) < 0x10000).to_string()
}

/// `parseInt (string)`: NaN without digits; a 0x prefix reads hexadecimal.
pub fn js_parse_int(s: &str) -> f64 {
    let t = s.trim_start_matches(|c: char| (c as u32) < 0x10000 && is_js_space(c as u32 as u16));
    let (negative, t) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let (radix, digits) = match t.as_bytes() {
        [b'0', b'x' | b'X', ..] => (16, &t[2..]),
        _ => (10, t),
    };
    let end = digits.find(|c: char| !c.is_digit(radix)).unwrap_or(digits.len());

    if end == 0 {
        return f64::NAN;
    }

    let mut v = 0.0f64;

    for c in digits[..end].chars() {
        v = v * radix as f64 + c.to_digit(radix).unwrap() as f64;
    }

    if negative {
        -v
    } else {
        v
    }
}

/// `parseFloat (string)`.
pub fn js_parse_float(s: &str) -> f64 {
    let t = s.trim_start_matches(|c: char| (c as u32) < 0x10000 && is_js_space(c as u32 as u16));
    let b = t.as_bytes();
    let mut i = 0;

    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }

    if t[i..].starts_with("Infinity") {
        return if b.first() == Some(&b'-') { f64::NEG_INFINITY } else { f64::INFINITY };
    }

    let digits_start = i;

    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }

    let mut any = i > digits_start;

    if i < b.len() && b[i] == b'.' {
        let frac = i + 1;
        let mut j = frac;

        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }

        if j > frac || any {
            any = any || j > frac;
            i = j;
        }
    }

    if !any {
        return f64::NAN;
    }

    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;

        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }

        let exp = j;

        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }

        if j > exp {
            i = j;
        }
    }

    t[..i].parse::<f64>().unwrap_or(f64::NAN)
}

/// Assignment to an element of a Uint8Array (ToUint8): truncate, modulo 256, NaN and the
/// infinities 0.
fn to_uint8(x: f64) -> u8 {
    if !x.is_finite() {
        return 0;
    }

    x.trunc().rem_euclid(256.0) as u8
}

// ---- NRRDParser.js ------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum ByteType {
    SignedChar,
    UnsignedChar,
    SignedShort,
    UnsignedShort,
    SignedInt,
    UnsignedInt,
    Float,
    Double,
}

fn nrrd_type(value: &str) -> Result<(ByteType, f64), String> {
    use ByteType::*;

    Ok(match value {
        "signed char" | "int8" | "int8_t" => (SignedChar, 1.0),
        "uchar" | "unsigned char" | "uint8" | "uint8_t" => (UnsignedChar, 1.0),
        "short" | "short int" | "signed short" | "signed short int" | "int16" | "int16_t" => (SignedShort, 2.0),
        "ushort" | "unsigned short" | "unsigned short int" | "uint16" | "uint16_t" => (UnsignedShort, 2.0),
        "int" | "signed int" | "int32" | "int32_t" => (SignedInt, 4.0),
        "uint" | "unsigned int" | "uint32" | "uint32_t" => (UnsignedInt, 4.0),
        "float" => (Float, 4.0),
        "double" => (Double, 8.0),
        _ => return Err(format!("Error: Unsupported NRRD type '{value}'.")),
    })
}

#[derive(Clone, Copy, PartialEq)]
enum Encoding {
    Ascii,
    Raw,
    Hex,
    Gzip,
}

fn nrrd_encoding(value: &str) -> Result<Encoding, String> {
    Ok(match value {
        "ascii" | "txt" | "text" => Encoding::Ascii,
        "raw" => Encoding::Raw,
        "hex" => Encoding::Hex,
        "gz" | "gzip" => Encoding::Gzip,
        _ => return Err(format!("Error: Unsupported NRRD encoding '{value}'.")),
    })
}

/// The `/(\d+)/` match of `getDimension`: the first run of ASCII digits.
fn first_digits(value: &str) -> Option<&str> {
    let start = value.find(|c: char| c.is_ascii_digit())?;
    let len = value[start..].find(|c: char| !c.is_ascii_digit()).unwrap_or(value.len() - start);

    Some(&value[start..start + len])
}

/// `/\s*(\d+)/y` repeatedly from the start of `value`.
fn sticky_numbers(value: &str) -> Vec<f64> {
    let u: Vec<u16> = value.encode_utf16().collect();
    let mut out = vec![];
    let mut i = 0;

    loop {
        let mut j = i;

        while j < u.len() && is_js_space(u[j]) {
            j += 1;
        }

        let d = j;

        while j < u.len() && (0x30..=0x39).contains(&u[j]) {
            j += 1;
        }

        if j == d {
            return out;
        }

        out.push(js_parse_int(&units_to_string(&u[d..j])));
        i = j;
    }
}

/// `Grammar .comment`: `#[^\n]*\n` at `pos`.
fn match_comment(s: &[u16], pos: usize) -> Option<usize> {
    if s.get(pos) != Some(&0x23) {
        return None;
    }

    let nl = s[pos..].iter().position(|&c| c == 0x0a)?;

    Some(pos + nl + 1)
}

/// `Grammar .field`: `([\w\s]+):\s*(.+?)\n` at `pos`; the key, the value and the end of the match.
fn match_field(s: &[u16], pos: usize) -> Option<(String, String, usize)> {
    let mut i = pos;

    while i < s.len() && (is_word(s[i]) || is_js_space(s[i])) {
        i += 1;
    }

    if i == pos || s.get(i) != Some(&0x3a) {
        return None;
    }

    let key = units_to_string(&s[pos..i]);
    let first = i + 1;
    let mut ws_end = first;

    while ws_end < s.len() && is_js_space(s[ws_end]) {
        ws_end += 1;
    }

    // `\s*` is greedy and gives back what `.+?` needs.
    for start in (first..=ws_end).rev() {
        if start >= s.len() || is_line_terminator(s[start]) {
            continue;
        }

        let mut q = start + 1;

        loop {
            match s.get(q) {
                Some(&0x0a) => return Some((key, units_to_string(&s[start..q]), q + 1)),
                Some(&c) if !is_line_terminator(c) => q += 1,
                _ => break,
            }
        }
    }

    None
}

struct Nrrd {
    components: f64,
    width: f64,
    height: f64,
    depth: f64,
    data: Option<Vec<u8>>,
}

/// `new NRRDParser () .parse (input)`: `Ok (None)` when `input` is not an NRRD file (`nrrd .nrrd`
/// false), the volume, or the error `parse` rejects with.
pub fn parse_nrrd(input: &[u8]) -> Result<Option<Volume>, String> {
    let text: Vec<u16> = x3d_parse::decode_text(input).encode_utf16().collect();

    // Grammar .NRRD: /^NRRD(\d+)\n/y.
    if !text.starts_with(&[0x4e, 0x52, 0x52, 0x44]) {
        return Ok(None);
    }

    let mut last = 4;

    while last < text.len() && (0x30..=0x39).contains(&text[last]) {
        last += 1;
    }

    if last == 4 || text.get(last) != Some(&0x0a) {
        return Ok(None);
    }

    last += 1;

    let mut little_endian = true;
    let mut byte_type: Option<(ByteType, f64)> = None;
    let mut encoding: Option<Encoding> = None;
    let mut nrrd = Nrrd { components: f64::NAN, width: f64::NAN, height: f64::NAN, depth: f64::NAN, data: None };

    // getFields.
    while let Some(end) = match_comment(&text, last) {
        last = end;
    }

    while let Some((key, value, end)) = match_field(&text, last) {
        last = end;
        let key = key.to_lowercase();
        let value = js_trim(&value).to_lowercase();

        match key.as_str() {
            "type" => byte_type = Some(nrrd_type(&value)?),
            "encoding" => encoding = Some(nrrd_encoding(&value)?),
            "dimension" => match first_digits(&value).map(js_parse_int) {
                Some(v) if (1.0..=4.0).contains(&v) => {}
                other => {
                    let shown = first_digits(&value).map_or("undefined".to_string(), str::to_string);
                    let _ = other;
                    return Err(format!("Error: Unsupported NRRD dimension '{shown}', must be 1, 2, 3, or 4."));
                }
            },
            "sizes" => {
                let sizes = sticky_numbers(&value);

                match sizes.len() {
                    1 => (nrrd.components, nrrd.width, nrrd.height, nrrd.depth) = (1.0, sizes[0], 1.0, 1.0),
                    2 => (nrrd.components, nrrd.width, nrrd.height, nrrd.depth) = (1.0, sizes[0], sizes[1], 1.0),
                    3 => (nrrd.components, nrrd.width, nrrd.height, nrrd.depth) = (1.0, sizes[0], sizes[1], sizes[2]),
                    4 => (nrrd.components, nrrd.width, nrrd.height, nrrd.depth) = (sizes[0], sizes[1], sizes[2], sizes[3]),
                    _ => return Err("Error: Unsupported NRRD sizes.".into()),
                }
            }
            "endian" => match value.as_str() {
                "little" => little_endian = true,
                "big" => little_endian = false,
                _ => return Err("Error: Unsupported NRRD endian, must be either 'little' or 'big'.".into()),
            },
            _ => {}
        }

        while let Some(end) = match_comment(&text, last) {
            last = end;
        }
    }

    // getData.
    let data_length = nrrd.components * nrrd.width * nrrd.height * nrrd.depth;

    if let Some(encoding) = encoding {
        let typed = |len: f64| -> Result<usize, String> {
            // `new Uint8Array (NaN)` is empty.
            if len.is_nan() {
                return Ok(0);
            }

            if !(0.0..=2147483648.0).contains(&len) {
                return Err("RangeError: Array buffer allocation failed".into());
            }

            Ok(len as usize)
        };

        let count = typed(data_length)?;

        nrrd.data = Some(match encoding {
            Encoding::Ascii => {
                let mut data = vec![0u8; count];
                let rest = units_to_string(&text[last.min(text.len())..]);

                if !rest.is_empty() {
                    let trimmed = js_trim(&rest);
                    // `split (/\s+/)`.
                    let numbers: Vec<&str> = if trimmed.is_empty() { vec![""] } else { trimmed.split(|c: char| (c as u32) < 0x10000 && is_js_space(c as u32 as u16)).filter(|t| !t.is_empty()).collect() };

                    for (i, token) in numbers.iter().enumerate() {
                        if i >= count {
                            break;
                        }

                        data[i] = match byte_type.map(|t| t.0) {
                            Some(ByteType::SignedChar | ByteType::UnsignedChar) => to_uint8(js_parse_int(token)),
                            Some(ByteType::SignedShort | ByteType::UnsignedShort) => to_uint8(js_parse_int(token) / 256.0),
                            Some(ByteType::SignedInt | ByteType::UnsignedInt) => to_uint8(js_parse_int(token) / 16777216.0),
                            Some(ByteType::Float) => to_uint8(js_parse_float(token) / 256.0),
                            Some(ByteType::Double) => to_uint8(js_parse_float(token) / 16777216.0),
                            None => 0,
                        };
                    }
                }

                data
            }
            Encoding::Raw => read_raw(input, count, data_length, byte_type, little_endian)?,
            Encoding::Hex => {
                let rest = units_to_string(&text[last.min(text.len())..]);

                if rest.is_empty() {
                    return Err("Error: Invalid NRRD data.".into());
                }

                // `rest .match (/([0-9a-fA-F]{2})/g)`.
                let b = rest.as_bytes();
                let mut bytes = vec![];
                let mut i = 0;

                while i + 1 < b.len() {
                    if b[i].is_ascii_hexdigit() && b[i + 1].is_ascii_hexdigit() {
                        bytes.push(u8::from_str_radix(&rest[i..i + 2], 16).unwrap());
                        i += 2;
                    } else {
                        i += 1;
                    }
                }

                if bytes.is_empty() {
                    return Err("Error: Invalid NRRD data.".into());
                }

                read_raw(&bytes, count, data_length, byte_type, little_endian)?
            }
            Encoding::Gzip => {
                // Any failure is `Invalid NRRD data.`; the offset is the index into the decoded
                // text, used as a byte offset like X_ITE does.
                if text.get(last) != Some(&0x0a) {
                    return Err("Error: Invalid NRRD data.".into());
                }

                let inflated = x3d_parse::gunzip(input.get(last + 1..).unwrap_or(&[]));

                read_raw(&inflated, count, data_length, byte_type, little_endian).map_err(|_| "Error: Invalid NRRD data.".to_string())?
            }
        });
    }

    Ok(Some(Volume { width: nrrd.width, height: nrrd.height, depth: nrrd.depth, components: nrrd.components, data: nrrd.data }))
}

/// NRRDParser.raw: the last `dataLength × bytes` bytes of the buffer, read as `byteType`.
fn read_raw(buffer: &[u8], count: usize, data_length: f64, byte_type: Option<(ByteType, f64)>, little_endian: bool) -> Result<Vec<u8>, String> {
    let mut data = vec![0u8; count];
    let Some((ty, bytes)) = byte_type else { return Ok(data) };
    let length = data_length * bytes;

    if length.is_nan() || count == 0 {
        return Ok(data);
    }

    let start = buffer.len() as f64 - length;

    if start < 0.0 {
        return Err("RangeError: Offset is outside the bounds of the DataView".into());
    }

    let start = start as usize;
    let step = bytes as usize;

    for (d, out) in data.iter_mut().enumerate() {
        let at = start + d * step;
        let raw = &buffer[at..at + step];

        *out = match ty {
            ByteType::SignedChar | ByteType::UnsignedChar => raw[0],
            ByteType::SignedShort | ByteType::UnsignedShort => {
                let v = if little_endian { u16::from_le_bytes([raw[0], raw[1]]) } else { u16::from_be_bytes([raw[0], raw[1]]) };
                to_uint8(v as f64 / 256.0)
            }
            ByteType::SignedInt | ByteType::UnsignedInt => {
                let a = [raw[0], raw[1], raw[2], raw[3]];
                let v = if little_endian { u32::from_le_bytes(a) } else { u32::from_be_bytes(a) };
                to_uint8(v as f64 / 16777216.0)
            }
            ByteType::Float => {
                let a = [raw[0], raw[1], raw[2], raw[3]];
                let v = if little_endian { f32::from_le_bytes(a) } else { f32::from_be_bytes(a) };
                to_uint8(v as f64 / 256.0)
            }
            ByteType::Double => {
                let a = [raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7]];
                let v = if little_endian { f64::from_le_bytes(a) } else { f64::from_be_bytes(a) };
                to_uint8(v / 16777216.0)
            }
        };
    }

    Ok(data)
}

// ---- the node ------------------------------------------------------------------------------------

impl Runtime {
    /// ImageTexture3D.loadData, a candidate whose data was fetched: NRRD, then DICOM; the first
    /// that accepts the data sets the texture data, the outputs and (by the caller) COMPLETE_STATE.
    /// An error fails the candidate.
    pub(crate) fn image_texture_3d_set_data(&mut self, node: NodeId, data: &[u8]) -> Result<(), String> {
        let volume = match parse_nrrd(data)? {
            Some(v) => v,
            None => match self.parse_dicom_logged(data)? {
                Some(v) => v,
                None => return Err("Error: ImageTexture3D: No suitable file handler found.".into()),
            },
        };

        // X3DTexture3DNode.setTextureData: the size limit.
        if volume.width > MAX_3D_TEXTURE_SIZE || volume.height > MAX_3D_TEXTURE_SIZE || volume.depth > MAX_3D_TEXTURE_SIZE {
            let n = |v: f64| if v.is_nan() { "undefined".to_string() } else { format!("{v}") };

            return Err(format!(
                "Error: At least one dimension ({} × {} × {}) is greater than the maximum texture size ({} px).",
                n(volume.width),
                n(volume.height),
                n(volume.depth),
                MAX_3D_TEXTURE_SIZE
            ));
        }

        let int = |v: f64| if v.is_nan() { 0 } else { v as i32 };
        let outputs = [int(volume.width), int(volume.height), int(volume.depth), int(volume.components)];

        self.nrt_mut(node).volume = Some(Rc::new(volume));
        self.add_node_event(node);
        self.image_texture_atlas_outputs(node, outputs);

        Ok(())
    }

    /// ImageTexture3D clearTexture with updateOutputs (0, 0, 0, 0): unloadData, a failed load.
    pub(crate) fn image_texture_3d_clear(&mut self, node: NodeId) {
        self.nrt_mut(node).volume = None;
        self.add_node_event(node);
        self.image_texture_atlas_outputs(node, [0; 4]);
    }

    /// The volume an ImageTexture3D shows (`None`: the cleared texture).
    pub fn image_texture_3d_volume(&self, node: NodeId) -> Option<Rc<Volume>> {
        self.nrt(node).and_then(|n| n.volume.clone())
    }

    /// DICOMParser.parse: a parse failure of dicom-parser is `console .error (error)` and "not DICOM".
    fn parse_dicom_logged(&mut self, data: &[u8]) -> Result<Option<Volume>, String> {
        match dicom::parse_dicom(data) {
            Ok(v) => Ok(Some(v)),
            Err(dicom::DicomError::NotDicom(text)) => {
                self.log_error(text);
                Ok(None)
            }
            Err(dicom::DicomError::Throw(text)) => Err(text),
        }
    }
}

pub mod dicom;
