//! JavaScript value semantics used by GLTF2Parser.js: a read-only view of the parsed JSON
//! (`V`, an optional reference where `None` is `undefined`), typed-array element kinds, base64,
//! half floats and `ArrayBuffer.slice`.

use crate::json::JsValue as J;
use x3d_fields::jsmath::number_to_string;

/// A JSON value or `undefined`.
#[derive(Clone, Copy)]
pub(crate) struct V<'a>(pub Option<&'a J>);

pub(crate) const UNDEF: V<'static> = V(None);

fn array_index(key: &str) -> Option<usize> {
    if key.is_empty()
        || !key.bytes().all(|b| b.is_ascii_digit())
        || (key.len() > 1 && key.starts_with('0'))
    {
        return None;
    }

    key.parse::<usize>()
        .ok()
        .filter(|n| (*n as u64) < u32::MAX as u64)
}

impl<'a> V<'a> {
    pub fn new(v: &'a J) -> Self {
        V(Some(v))
    }

    /// `value[key]` for an object (own property) or an array (index).
    pub fn get(self, key: &str) -> V<'a> {
        match self.0 {
            Some(J::Object(o)) => V(o.iter().find(|(k, _)| k == key).map(|(_, v)| v)),
            Some(J::Array(a)) => V(array_index(key).and_then(|i| a.get(i))),
            _ => V(None),
        }
    }

    /// `value?.[key]` as used on possibly-null values: identical to `get` here.
    pub fn at(self, i: usize) -> V<'a> {
        match self.0 {
            Some(J::Array(a)) => V(a.get(i)),
            _ => V(None),
        }
    }

    /// `instanceof Object` (arrays are objects).
    pub fn is_obj(self) -> bool {
        matches!(self.0, Some(J::Object(_) | J::Array(_)))
    }

    pub fn is_array(self) -> bool {
        matches!(self.0, Some(J::Array(_)))
    }

    pub fn arr(self) -> Option<&'a [J]> {
        match self.0 {
            Some(J::Array(a)) => Some(a),
            _ => None,
        }
    }

    pub fn entries(self) -> Vec<(String, V<'a>)> {
        match self.0 {
            Some(J::Object(o)) => o.iter().map(|(k, v)| (k.clone(), V(Some(v)))).collect(),
            Some(J::Array(a)) => a
                .iter()
                .enumerate()
                .map(|(i, v)| (i.to_string(), V(Some(v))))
                .collect(),
            _ => vec![],
        }
    }

    pub fn is_undef(self) -> bool {
        self.0.is_none()
    }

    /// Neither `undefined` nor `null` (`?.` / `??` test).
    pub fn nn(self) -> bool {
        !matches!(self.0, None | Some(J::Null))
    }

    pub fn num(self) -> Option<f64> {
        match self.0 {
            Some(J::Number(n)) => Some(*n),
            _ => None,
        }
    }

    pub fn str(self) -> Option<&'a str> {
        match self.0 {
            Some(J::String(s)) => Some(s),
            _ => None,
        }
    }

    pub fn bool_(self) -> Option<bool> {
        match self.0 {
            Some(J::Bool(b)) => Some(*b),
            _ => None,
        }
    }

    /// JS truthiness.
    pub fn truthy(self) -> bool {
        match self.0 {
            None | Some(J::Null) => false,
            Some(J::Bool(b)) => *b,
            Some(J::Number(n)) => *n != 0.0 && !n.is_nan(),
            Some(J::String(s)) => !s.is_empty(),
            Some(_) => true,
        }
    }

    /// `numberValue`: the value when `typeof value === "number"`, else the default.
    pub fn number_or(self, default: f64) -> f64 {
        self.num().unwrap_or(default)
    }

    /// `value || 0` converted with ToNumber (numbers, numeric strings, booleans).
    pub fn to_number(self) -> f64 {
        match self.0 {
            Some(J::Number(n)) => *n,
            Some(J::Bool(b)) => *b as u8 as f64,
            Some(J::Null) => 0.0,
            Some(J::String(s)) => {
                let t = s.trim();
                if t.is_empty() {
                    0.0
                } else {
                    t.parse::<f64>().unwrap_or(f64::NAN)
                }
            }
            None => f64::NAN,
            Some(_) => f64::NAN,
        }
    }

    /// `array[key]` index: a non-negative integer number or its canonical string, below `len`.
    pub fn index(self, len: usize) -> Option<usize> {
        let i = match self.0 {
            Some(J::Number(n)) if *n >= 0.0 && n.fract() == 0.0 && *n < 4294967295.0 => *n as usize,
            Some(J::String(s)) => array_index(s)?,
            _ => return None,
        };

        (i < len).then_some(i)
    }

    /// `String (value)` for the values the parser interpolates into names.
    pub fn to_js_string(self) -> String {
        match self.0 {
            None => "undefined".into(),
            Some(J::Null) => "null".into(),
            Some(J::Bool(b)) => b.to_string(),
            Some(J::Number(n)) => number_to_string(*n),
            Some(J::String(s)) => s.clone(),
            Some(J::Array(a)) => a
                .iter()
                .map(|v| V(Some(v)).to_js_string_arr())
                .collect::<Vec<_>>()
                .join(","),
            Some(J::Object(_)) => "[object Object]".into(),
        }
    }

    fn to_js_string_arr(self) -> String {
        match self.0 {
            None | Some(J::Null) => String::new(),
            _ => self.to_js_string(),
        }
    }

    /// Numbers of a JSON array when every element is a number (`vectorValue` copies with
    /// `vector .set (... array)`, which would pass other values through; they become NaN/0).
    pub fn numbers(self) -> Option<Vec<f64>> {
        let a = self.arr()?;

        Some(a.iter().map(|v| V(Some(v)).to_number()).collect())
    }
}

/// `vectorValue (array, vector)`: the array's numbers when it is an `Array` of exactly `len`.
pub(crate) fn vector_value(array: V, len: usize) -> Option<Vec<f64>> {
    let a = array.arr()?;

    if a.len() != len {
        return None;
    }

    Some(a.iter().map(|v| V(Some(v)).to_number()).collect())
}

/// `$.toUpperCaseFirst`.
pub(crate) fn upper_first(s: &str) -> String {
    let mut c = s.chars();

    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// `description (string)`: `string ?.replace (/_+/g, " ") .trim () ?? ""`.
pub(crate) fn description(s: Option<&str>) -> String {
    let Some(s) = s else { return String::new() };
    let mut out = String::new();
    let mut run = false;

    for c in s.chars() {
        if c == '_' {
            if !run {
                out.push(' ');
            }
            run = true;
        } else {
            out.push(c);
            run = false;
        }
    }

    out.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
        .to_string()
}

// --- typed arrays -----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ct {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F16,
    F32,
    F64,
}

impl Ct {
    pub fn from_component_type(v: V) -> Option<Ct> {
        Some(match v.num()? as i64 {
            5120 => Ct::I8,
            5121 => Ct::U8,
            5122 => Ct::I16,
            5123 => Ct::U16,
            5124 => Ct::I32,
            5125 => Ct::U32,
            5131 => Ct::F16,
            5126 => Ct::F32,
            5130 => Ct::F64,
            _ => return None,
        })
    }

    pub fn size(self) -> usize {
        match self {
            Ct::I8 | Ct::U8 => 1,
            Ct::I16 | Ct::U16 | Ct::F16 => 2,
            Ct::I32 | Ct::U32 | Ct::F32 => 4,
            Ct::F64 => 8,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Ct::I8 => "Int8Array",
            Ct::U8 => "Uint8Array",
            Ct::I16 => "Int16Array",
            Ct::U16 => "Uint16Array",
            Ct::I32 => "Int32Array",
            Ct::U32 => "Uint32Array",
            Ct::F16 => "Float16Array",
            Ct::F32 => "Float32Array",
            Ct::F64 => "Float64Array",
        }
    }

    pub fn is_float(self) -> bool {
        matches!(self, Ct::F16 | Ct::F32 | Ct::F64)
    }

    /// Assigning a JS number to an element: `undefined` / NaN becomes 0 in integer arrays.
    pub fn store(self, v: f64) -> f64 {
        match self {
            Ct::F32 => v as f32 as f64,
            Ct::F16 => half_to_f32(f32_to_half(v as f32)) as f64,
            Ct::F64 => v,
            Ct::I8 => (x3d_fields::jsmath::to_int32(v) as i8) as f64,
            Ct::U8 => (x3d_fields::jsmath::to_int32(v) as u8) as f64,
            Ct::I16 => (x3d_fields::jsmath::to_int32(v) as i16) as f64,
            Ct::U16 => (x3d_fields::jsmath::to_int32(v) as u16) as f64,
            Ct::I32 => x3d_fields::jsmath::to_int32(v) as f64,
            Ct::U32 => (x3d_fields::jsmath::to_int32(v) as u32) as f64,
        }
    }

    pub fn read(self, b: &[u8]) -> f64 {
        match self {
            Ct::I8 => b[0] as i8 as f64,
            Ct::U8 => b[0] as f64,
            Ct::I16 => i16::from_le_bytes([b[0], b[1]]) as f64,
            Ct::U16 => u16::from_le_bytes([b[0], b[1]]) as f64,
            Ct::I32 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
            Ct::U32 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
            Ct::F16 => half_to_f32(u16::from_le_bytes([b[0], b[1]])) as f64,
            Ct::F32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
            Ct::F64 => f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]),
        }
    }
}

/// A typed array as the parser sees it: element kind and the elements as JS numbers.
#[derive(Clone, Debug)]
pub(crate) struct TArr {
    pub ct: Ct,
    pub v: Vec<f64>,
}

impl TArr {
    pub fn len(&self) -> usize {
        self.v.len()
    }
}

/// `ToIndex (length)` as `new TypedArray (buffer, offset, length)` applies it.
pub(crate) fn to_index(v: f64) -> Result<usize, String> {
    if v.is_nan() {
        return Ok(0);
    }

    let t = v.trunc();

    if t < 0.0 || t > 9007199254740991.0 {
        return Err(format!(
            "RangeError: Invalid typed array length: {}",
            number_to_string(v)
        ));
    }

    Ok(t as usize)
}

/// `new TypedArray (buffer, byteOffset, length)` over an ArrayBuffer (`None` is `undefined`,
/// which makes an empty array).
pub(crate) fn typed_array(
    ct: Ct,
    buffer: Option<&[u8]>,
    byte_offset: f64,
    length: Option<f64>,
) -> Result<TArr, String> {
    let Some(buffer) = buffer else {
        return Ok(TArr { ct, v: vec![] });
    };

    let size = ct.size();
    let offset = to_index(byte_offset).map_err(|_| {
        format!(
            "RangeError: Start offset {} is outside the bounds of the buffer",
            number_to_string(byte_offset)
        )
    })?;

    if offset % size != 0 {
        return Err(format!(
            "RangeError: start offset of {} should be a multiple of {}",
            ct.name(),
            size
        ));
    }

    let n = match length {
        Some(l) => {
            let n = to_index(l)?;

            if offset > buffer.len() {
                return Err(format!(
                    "RangeError: Start offset {} is outside the bounds of the buffer",
                    offset
                ));
            }

            if offset as u128 + n as u128 * size as u128 > buffer.len() as u128 {
                return Err(format!("RangeError: Invalid typed array length: {n}"));
            }

            n
        }
        None => {
            if offset > buffer.len() {
                return Err(format!(
                    "RangeError: Start offset {} is outside the bounds of the buffer",
                    offset
                ));
            }

            if (buffer.len() - offset) % size != 0 {
                return Err(format!(
                    "RangeError: byte length of {} should be a multiple of {}",
                    ct.name(),
                    size
                ));
            }

            (buffer.len() - offset) / size
        }
    };

    let mut v = Vec::with_capacity(n);

    for i in 0..n {
        v.push(ct.read(&buffer[offset + i * size..]));
    }

    Ok(TArr { ct, v })
}

/// `ArrayBuffer.prototype.slice (start, end)` with JS argument conversion (`NaN` is 0).
pub(crate) fn js_slice(len: usize, start: f64, end: f64) -> (usize, usize) {
    let rel = |v: f64| -> usize {
        let v = if v.is_nan() { 0.0 } else { v.trunc() };
        let len = len as f64;

        if v < 0.0 {
            (len + v).max(0.0) as usize
        } else {
            v.min(len) as usize
        }
    };

    let (s, e) = (rel(start), rel(end));

    (s, e.max(s))
}

// --- half floats ----------------------------------------------------------------------------------

pub(crate) fn half_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1f) as u32;
    let frac = (h & 0x3ff) as u32;

    let bits = if exp == 0 {
        if frac == 0 {
            sign << 31
        } else {
            // Subnormal: normalize.
            let mut e = 127 - 15 + 1;
            let mut f = frac;

            while f & 0x400 == 0 {
                f <<= 1;
                e -= 1;
            }

            (sign << 31) | ((e as u32) << 23) | ((f & 0x3ff) << 13)
        }
    } else if exp == 0x1f {
        (sign << 31) | (0xff << 23) | (frac << 13)
    } else {
        (sign << 31) | ((exp + 127 - 15) << 23) | (frac << 13)
    };

    f32::from_bits(bits)
}

pub(crate) fn f32_to_half(f: f32) -> u16 {
    let bits = f.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xff) as i32;
    let frac = bits & 0x7fffff;

    if exp == 0xff {
        return sign | 0x7c00 | if frac != 0 { 0x200 } else { 0 };
    }

    let e = exp - 127 + 15;

    if e >= 0x1f {
        return sign | 0x7c00;
    }

    if e <= 0 {
        if e < -10 {
            return sign;
        }

        let m = frac | 0x800000;
        let shift = (14 - e) as u32;
        let mut half = (m >> shift) as u16;
        let rem = m & ((1 << shift) - 1);
        let halfway = 1 << (shift - 1);

        if rem > halfway || (rem == halfway && half & 1 == 1) {
            half += 1;
        }

        return sign | half;
    }

    let mut half = sign | ((e as u16) << 10) | ((frac >> 13) as u16);
    let rem = frac & 0x1fff;

    if rem > 0x1000 || (rem == 0x1000 && half & 1 == 1) {
        half += 1;
    }

    half
}

// --- base64 -----------------------------------------------------------------------------------------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub(crate) fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);

    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;

        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }

    out
}

/// `fetch ("data:...;base64,...")` body: forgiving base64 (whitespace skipped, padding optional).
pub(crate) fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;

    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };

        acc = (acc << 6) | v as u32;
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }

    Some(out)
}

// --- percent decoding --------------------------------------------------------------------------------

pub(crate) fn percent_decode_bytes(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;

    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() + 0 && i + 2 <= b.len() - 1 + 0 {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }

        out.push(b[i]);
        i += 1;
    }

    out
}

pub(crate) fn percent_decode_string(s: &str) -> String {
    String::from_utf8_lossy(&percent_decode_bytes(s)).into_owned()
}

impl V<'_> {
    /// An omitted constructor argument is 0; any other value goes through ToNumber.
    pub fn to_number_or_zero(self) -> f64 {
        if self.is_undef() {
            0.0
        } else {
            self.to_number()
        }
    }
}
