//! The terminal symbols of src/x_ite/Parser/VRMLParser.js (`Grammar`), as hand-written
//! matchers with the same semantics as the sticky regular expressions.
//!
//! Inputs are UTF-8 bytes. Every character class in the grammar is ASCII, and every byte of
//! a multi-byte UTF-8 sequence is >= 0x80, which all of the "Id" classes accept, so working
//! on bytes gives the same matches as X_ITE's UTF-16 regexes.

/// `/[\x20\n,\t\r]+/y`
pub fn whitespaces(input: &[u8], pos: usize) -> Option<usize> {
    let mut i = pos;

    while i < input.len() && matches!(input[i], b' ' | b'\n' | b',' | b'\t' | b'\r') {
        i += 1;
    }

    (i > pos).then_some(i)
}

/// `/#\/\*.*?\*\/#|#[^\r\n]*(?=[\r\n]|$)/sy`
pub fn comment(input: &[u8], pos: usize) -> Option<usize> {
    if input.get(pos) != Some(&b'#') {
        return None;
    }

    if input[pos..].starts_with(b"#/*") {
        let mut i = pos + 3;

        while i + 3 <= input.len() {
            if &input[i..i + 3] == b"*/#" {
                return Some(i + 3);
            }
            i += 1;
        }
    }

    let mut i = pos + 1;

    while i < input.len() && input[i] != b'\r' && input[i] != b'\n' {
        i += 1;
    }

    Some(i)
}

/// A literal keyword regex such as `/PROTO/y` (no word boundary, like X_ITE).
pub fn literal(input: &[u8], pos: usize, word: &str) -> Option<usize> {
    input[pos..].starts_with(word.as_bytes()).then_some(pos + word.len())
}

fn is_id_rest(b: u8) -> bool {
    !(b <= 0x20 || matches!(b, 0x22 | 0x23 | 0x27 | 0x2c | 0x2e | 0x5b | 0x5c | 0x5d | 0x7b | 0x7d | 0x7f))
}

fn is_id_first(b: u8) -> bool {
    is_id_rest(b) && !matches!(b, b'0'..=b'9' | 0x2b | 0x2d)
}

/// `Id`: `/[^\x30-\x39\x00-\x20\x22\x23\x27\x2b\x2c\x2d\x2e\x5b\x5c\x5d\x7b\x7d\x7f]{1}[^\x00-\x20\x22\x23\x27\x2c\x2e\x5b\x5c\x5d\x7b\x7d\x7f]*/y`
pub fn id(input: &[u8], pos: usize) -> Option<usize> {
    if pos >= input.len() || !is_id_first(input[pos]) {
        return None;
    }

    let mut i = pos + 1;

    while i < input.len() && is_id_rest(input[i]) {
        i += 1;
    }

    Some(i)
}

/// `ComponentNameId`: like `Id` but without `:`.
pub fn component_name_id(input: &[u8], pos: usize) -> Option<usize> {
    if pos >= input.len() || !is_id_first(input[pos]) || input[pos] == b':' {
        return None;
    }

    let mut i = pos + 1;

    while i < input.len() && is_id_rest(input[i]) && input[i] != b':' {
        i += 1;
    }

    Some(i)
}

/// `FieldType`: `/[SM]F(?:Bool|Color(?:RGBA)?|Double|Float|Image|Int32|Matrix[34][df]|Node|Quaternion|Rotation|String|Time|Vec[234][df])/y`
pub fn field_type(input: &[u8], pos: usize) -> Option<usize> {
    let s = &input[pos..];

    if s.len() < 2 || !(s[0] == b'S' || s[0] == b'M') || s[1] != b'F' {
        return None;
    }

    let rest = &s[2..];

    let simple: &[&str] = &["Bool", "Double", "Float", "Image", "Int32", "Node", "Quaternion", "Rotation", "String", "Time"];

    if rest.starts_with(b"ColorRGBA") {
        return Some(pos + 2 + 9);
    }

    if rest.starts_with(b"Color") {
        return Some(pos + 2 + 5);
    }

    for w in simple {
        if rest.starts_with(w.as_bytes()) {
            return Some(pos + 2 + w.len());
        }
    }

    if rest.starts_with(b"Matrix") && rest.len() >= 8 && matches!(rest[6], b'3' | b'4') && matches!(rest[7], b'd' | b'f') {
        return Some(pos + 2 + 8);
    }

    if rest.starts_with(b"Vec") && rest.len() >= 5 && matches!(rest[3], b'2' | b'3' | b'4') && matches!(rest[4], b'd' | b'f') {
        return Some(pos + 2 + 5);
    }

    None
}

/// `int32`: `/(?:0[xX][\da-fA-F]+)|(?:[+-]?\d+)/y`
pub fn int32(input: &[u8], pos: usize) -> Option<usize> {
    let s = &input[pos..];

    if s.len() >= 3 && s[0] == b'0' && (s[1] == b'x' || s[1] == b'X') && s[2].is_ascii_hexdigit() {
        let mut i = 3;
        while i < s.len() && s[i].is_ascii_hexdigit() {
            i += 1;
        }
        return Some(pos + i);
    }

    let mut i = 0;

    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        i += 1;
    }

    let start = i;

    while i < s.len() && s[i].is_ascii_digit() {
        i += 1;
    }

    (i > start).then_some(pos + i)
}

/// `double`: `/(?!0[xX])[+-]?(?:(?:(?:\d*\.\d+)|(?:\d+(?:\.)?))(?:[eE][+-]?\d+)?)/y`
pub fn double(input: &[u8], pos: usize) -> Option<usize> {
    let s = &input[pos..];

    if s.len() >= 2 && s[0] == b'0' && (s[1] == b'x' || s[1] == b'X') {
        return None;
    }

    let mut i = 0;

    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        i += 1;
    }

    let digits = |from: usize| {
        let mut j = from;
        while j < s.len() && s[j].is_ascii_digit() {
            j += 1;
        }
        j
    };

    // First alternative: \d*\.\d+
    let int_end = digits(i);
    let mut end = None;

    if int_end < s.len() && s[int_end] == b'.' {
        let frac_end = digits(int_end + 1);

        if frac_end > int_end + 1 {
            end = Some(frac_end);
        }
    }

    // Second alternative: \d+\.?
    if end.is_none() && int_end > i {
        end = Some(if int_end < s.len() && s[int_end] == b'.' { int_end + 1 } else { int_end });
    }

    let mut end = end?;

    // Optional exponent.
    if end < s.len() && (s[end] == b'e' || s[end] == b'E') {
        let mut j = end + 1;

        if j < s.len() && (s[j] == b'+' || s[j] == b'-') {
            j += 1;
        }

        let k = digits(j);

        if k > j {
            end = k;
        }
    }

    Some(pos + end)
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// `CONSTANTS`: `/([+-]?)\b(NAN|INFINITY|INF|PI(?:…)?|SQRT(?:1_2|2))\b/iy`.
/// Returns (end, negative, upper-cased constant name).
pub fn constant(input: &[u8], pos: usize) -> Option<(usize, bool, String)> {
    let mut i = pos;
    let mut negative = false;

    if i < input.len() && (input[i] == b'+' || input[i] == b'-') {
        negative = input[i] == b'-';
        i += 1;
    }

    // \b before the name: the name starts with a word character, so the preceding character
    // must not be one.
    if i < input.len() && !is_word(input[i]) {
        return None;
    }

    if i > 0 && is_word(input[i - 1]) {
        return None;
    }

    // Any matching alternative must end at a word boundary, i.e. at the end of the word.
    let mut j = i;

    while j < input.len() && is_word(input[j]) {
        j += 1;
    }

    let word = String::from_utf8_lossy(&input[i..j]).to_ascii_uppercase();

    const NAMES: &[&str] = &[
        "NAN", "INFINITY", "INF", "PI", "PI2", "PI1_4", "PI2_4", "PI3_4", "PI4_4", "PI5_4", "PI6_4", "PI7_4", "PI8_4",
        "PI1_2", "PI2_2", "PI3_2", "PI4_2", "PI1_3", "PI2_3", "PI3_3", "PI4_3", "PI5_3", "PI6_3", "SQRT1_2", "SQRT2",
    ];

    NAMES.contains(&word.as_str()).then_some((j, negative, word))
}

pub fn constant_value(name: &str) -> f64 {
    use std::f64::consts::PI;

    match name {
        "NAN" => f64::NAN,
        "INFINITY" | "INF" => f64::INFINITY,
        "PI" => PI,
        "PI2" => PI * 2.0,
        "PI1_4" => PI * 1.0 / 4.0,
        "PI2_4" => PI * 2.0 / 4.0,
        "PI3_4" => PI * 3.0 / 4.0,
        "PI4_4" => PI * 4.0 / 4.0,
        "PI5_4" => PI * 5.0 / 4.0,
        "PI6_4" => PI * 6.0 / 4.0,
        "PI7_4" => PI * 7.0 / 4.0,
        "PI8_4" => PI * 8.0 / 4.0,
        "PI1_2" => PI * 1.0 / 2.0,
        "PI2_2" => PI * 2.0 / 2.0,
        "PI3_2" => PI * 3.0 / 2.0,
        "PI4_2" => PI * 4.0 / 2.0,
        "PI1_3" => PI * 1.0 / 3.0,
        "PI2_3" => PI * 2.0 / 3.0,
        "PI3_3" => PI * 3.0 / 3.0,
        "PI4_3" => PI * 4.0 / 3.0,
        "PI5_3" => PI * 5.0 / 3.0,
        "PI6_3" => PI * 6.0 / 3.0,
        "SQRT1_2" => std::f64::consts::FRAC_1_SQRT_2,
        "SQRT2" => std::f64::consts::SQRT_2,
        _ => f64::NAN,
    }
}

/// `HTMLColor`: `/0[xX][\da-fA-F]+|[a-zA-Z]+\([^)]*\)|[a-zA-Z]+/y`
pub fn html_color(input: &[u8], pos: usize) -> Option<usize> {
    let s = &input[pos..];

    if s.len() >= 3 && s[0] == b'0' && (s[1] == b'x' || s[1] == b'X') && s[2].is_ascii_hexdigit() {
        let mut i = 3;
        while i < s.len() && s[i].is_ascii_hexdigit() {
            i += 1;
        }
        return Some(pos + i);
    }

    let mut i = 0;

    while i < s.len() && s[i].is_ascii_alphabetic() {
        i += 1;
    }

    if i == 0 {
        return None;
    }

    if i < s.len() && s[i] == b'(' {
        if let Some(close) = s[i + 1..].iter().position(|b| *b == b')') {
            return Some(pos + i + 1 + close + 1);
        }
    }

    Some(pos + i)
}

/// JS `parseInt` on a string matched by `int32`.
pub fn parse_int(text: &str) -> f64 {
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        let mut v = 0f64;
        // Exact for up to 2^53; longer literals are rounded per digit (not bit-exact with V8).
        for c in hex.chars() {
            v = v * 16.0 + c.to_digit(16).unwrap() as f64;
        }
        return v;
    }

    text.parse::<f64>().unwrap_or(f64::NAN)
}

/// JS `parseFloat` on a string matched by `double` (Rust's float parser is correctly rounded,
/// like V8's).
pub fn parse_float(text: &str) -> f64 {
    text.parse::<f64>().unwrap_or(f64::NAN)
}

/// JS StrWhiteSpaceChar (WhiteSpace and LineTerminator), as `parseFloat` / `parseInt` skip it.
fn is_js_space(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{feff}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

/// JS global `parseFloat (string)` on any string: the longest decimal (or `Infinity`) prefix
/// after leading white space, else NaN.
pub fn js_parse_float(text: &str) -> f64 {
    let s = text.trim_start_matches(is_js_space);
    let b = s.as_bytes();
    let mut i = 0;

    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }

    if s[i..].starts_with("Infinity") {
        return if b.first() == Some(&b'-') { f64::NEG_INFINITY } else { f64::INFINITY };
    }

    let digits = |i: &mut usize| {
        let start = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        *i - start
    };

    let mut n = digits(&mut i);

    if i < b.len() && b[i] == b'.' {
        let mut k = i + 1;
        let m = digits(&mut k);

        if n + m > 0 {
            i = k;
            n += m;
        }
    }

    if n == 0 {
        return f64::NAN;
    }

    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut k = i + 1;

        if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
            k += 1;
        }

        if digits(&mut k) > 0 {
            i = k;
        }
    }

    let t = &s[..i];
    let t = t.strip_suffix('.').unwrap_or(t);

    t.parse::<f64>().unwrap_or(f64::NAN)
}

/// JS global `parseInt (string)` without a radix: leading white space, a sign, `0x` for hex,
/// then the longest digit prefix, else NaN.
pub fn js_parse_int(text: &str) -> f64 {
    let s = text.trim_start_matches(is_js_space);
    let (negative, s) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };

    let (radix, s) = match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(rest) => (16, rest),
        None => (10, s),
    };

    let n = s.chars().take_while(|c| c.is_digit(radix)).count();

    if n == 0 {
        return f64::NAN;
    }

    let digits = &s[..n];
    let v = if radix == 10 { digits.parse::<f64>().unwrap_or(f64::NAN) } else { parse_int(&format!("0x{digits}")) };

    if negative { -v } else { v }
}
