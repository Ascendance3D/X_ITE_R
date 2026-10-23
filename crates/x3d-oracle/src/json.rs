//! JSON output that is byte-identical to the Node tools' `JSON.stringify (result, null, 2)`.
//!
//! Values that come from the page are never re-encoded: the page returns `JSON.stringify (value)`, which is
//! parsed here into a tree whose scalars keep their exact source text (numbers as V8 printed them, strings
//! with V8's escapes). Only values the driver itself creates are encoded here, with the same rules as
//! `JSON.stringify` (see [`escape`], [`js_number`]).

/// A JSON value being assembled for output.
#[derive(Clone, Debug)]
pub enum J {
    /// `undefined`: omitted from objects, `null` in arrays (as `JSON.stringify`).
    Undef,
    /// A scalar token, already in JSON text form (`1.5`, `true`, `null`, `"a\nb"`).
    Tok(String),
    Arr(Vec<J>),
    /// Members in insertion order; keys are JSON string tokens (with quotes and escapes).
    Obj(Vec<(String, J)>),
}

impl J {
    pub fn str(s: &str) -> J {
        J::Tok(escape(s))
    }

    pub fn int(n: i64) -> J {
        J::Tok(n.to_string())
    }

    pub fn num(x: f64) -> J {
        if x.is_finite() {
            J::Tok(js_number(x))
        } else {
            J::Tok("null".into())
        }
    }

    pub fn bool(b: bool) -> J {
        J::Tok(if b { "true" } else { "false" }.into())
    }

    pub fn obj() -> J {
        J::Obj(Vec::new())
    }

    /// `o [key] = value`: an existing key keeps its position (as for a JavaScript object).
    pub fn set(&mut self, key: &str, value: J) {
        let J::Obj(members) = self else {
            panic!("set on a non-object")
        };
        let k = escape(key);
        match members.iter_mut().find(|(m, _)| *m == k) {
            Some(m) => m.1 = value,
            None => members.push((k, value)),
        }
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut J> {
        let J::Obj(members) = self else { return None };
        let k = escape(key);
        members.iter_mut().find(|(m, _)| *m == k).map(|m| &mut m.1)
    }

    /// The member `key` of a parsed object (key given unescaped).
    pub fn member(&self, key: &str) -> J {
        let J::Obj(members) = self else {
            return J::Undef;
        };
        let k = escape(key);
        members
            .iter()
            .find(|(m, _)| *m == k)
            .map(|m| m.1.clone())
            .unwrap_or(J::Undef)
    }

    /// `JSON.stringify (value)` (no whitespace); "undefined" for a top-level `undefined` (a page argument).
    pub fn compact(&self) -> String {
        fn w(v: &J, out: &mut String) {
            match v {
                J::Undef => out.push_str("null"),
                J::Tok(t) => out.push_str(t),
                J::Arr(items) => {
                    out.push('[');
                    for (i, item) in items.iter().enumerate() {
                        if i > 0 {
                            out.push(',');
                        }
                        w(item, out);
                    }
                    out.push(']');
                }
                J::Obj(members) => {
                    out.push('{');
                    let mut first = true;
                    for (k, m) in members.iter().filter(|(_, m)| !matches!(m, J::Undef)) {
                        if !first {
                            out.push(',');
                        }
                        first = false;
                        out.push_str(k);
                        out.push(':');
                        w(m, out);
                    }
                    out.push('}');
                }
            }
        }
        if matches!(self, J::Undef) {
            return "undefined".into();
        }
        let mut out = String::new();
        w(self, &mut out);
        out
    }

    /// `JSON.stringify (value, null, 2)`; `None` for a top-level `undefined`.
    pub fn pretty(&self) -> Option<String> {
        self.pretty_indent(2)
    }

    /// `JSON.stringify (value, null, indent)`.
    pub fn pretty_indent(&self, indent: usize) -> Option<String> {
        if matches!(self, J::Undef) {
            return None;
        }
        let mut out = String::new();
        write(self, 0, indent, &mut out);
        Some(out)
    }
}

fn write(v: &J, indent: usize, step: usize, out: &mut String) {
    match v {
        J::Undef => out.push_str("null"),
        J::Tok(t) => out.push_str(t),
        J::Arr(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                pad((indent + 1) * step, out);
                write(item, indent + 1, step, out);
            }
            out.push('\n');
            pad(indent * step, out);
            out.push(']');
        }
        J::Obj(members) => {
            let mut first = true;
            for (k, m) in members {
                if matches!(m, J::Undef) {
                    continue;
                }
                out.push_str(if first { "{\n" } else { ",\n" });
                first = false;
                pad((indent + 1) * step, out);
                out.push_str(k);
                out.push_str(": ");
                write(m, indent + 1, step, out);
            }
            if first {
                out.push_str("{}");
            } else {
                out.push('\n');
                pad(indent * step, out);
                out.push('}');
            }
        }
    }
}

fn pad(n: usize, out: &mut String) {
    for _ in 0..n {
        out.push(' ');
    }
}

/// A string as `JSON.stringify` encodes it.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// ECMAScript Number::toString (radix 10).
pub fn js_number(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x == 0.0 {
        return "0".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let sign = if x < 0.0 { "-" } else { "" };
    // Shortest round-trip digits, as "d.ddde±n".
    let e = format!("{:e}", x.abs());
    let (mant, exp) = e.split_once('e').unwrap();
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exp.parse::<i32>().unwrap() + 1;
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let es = if e < 0 {
            format!("-{}", -e)
        } else {
            format!("+{e}")
        };
        if k == 1 {
            format!("{digits}e{es}")
        } else {
            format!("{}.{}e{es}", &digits[..1], &digits[1..])
        }
    };
    format!("{sign}{body}")
}

/// `Math.round`.
pub fn js_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

/// Parses compact JSON text (as `JSON.stringify` writes it) keeping every scalar's source text.
pub fn parse(text: &str) -> Result<J, String> {
    let b = text.as_bytes();
    let mut i = 0;
    let v = value(b, &mut i)?;
    ws(b, &mut i);
    if i != b.len() {
        return Err(format!("trailing JSON at {i}"));
    }
    Ok(v)
}

fn ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && matches!(b[*i], b' ' | b'\n' | b'\r' | b'\t') {
        *i += 1;
    }
}

fn string_end(b: &[u8], start: usize) -> Result<usize, String> {
    let mut i = start + 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => return Ok(i + 1),
            _ => i += 1,
        }
    }
    Err("unterminated JSON string".into())
}

fn value(b: &[u8], i: &mut usize) -> Result<J, String> {
    ws(b, i);
    let s = |a: usize, z: usize| String::from_utf8_lossy(&b[a..z]).into_owned();
    match b.get(*i) {
        None => Err("unexpected end of JSON".into()),
        Some(b'"') => {
            let end = string_end(b, *i)?;
            let t = s(*i, end);
            *i = end;
            Ok(J::Tok(t))
        }
        Some(b'[') => {
            *i += 1;
            let mut items = Vec::new();
            ws(b, i);
            if b.get(*i) == Some(&b']') {
                *i += 1;
                return Ok(J::Arr(items));
            }
            loop {
                items.push(value(b, i)?);
                ws(b, i);
                match b.get(*i) {
                    Some(b',') => *i += 1,
                    Some(b']') => {
                        *i += 1;
                        return Ok(J::Arr(items));
                    }
                    _ => return Err(format!("bad JSON array at {i}")),
                }
            }
        }
        Some(b'{') => {
            *i += 1;
            let mut members = Vec::new();
            ws(b, i);
            if b.get(*i) == Some(&b'}') {
                *i += 1;
                return Ok(J::Obj(members));
            }
            loop {
                ws(b, i);
                if b.get(*i) != Some(&b'"') {
                    return Err(format!("bad JSON key at {i}"));
                }
                let end = string_end(b, *i)?;
                let k = s(*i, end);
                *i = end;
                ws(b, i);
                if b.get(*i) != Some(&b':') {
                    return Err(format!("bad JSON object at {i}"));
                }
                *i += 1;
                members.push((k, value(b, i)?));
                ws(b, i);
                match b.get(*i) {
                    Some(b',') => *i += 1,
                    Some(b'}') => {
                        *i += 1;
                        return Ok(J::Obj(members));
                    }
                    _ => return Err(format!("bad JSON object at {i}")),
                }
            }
        }
        Some(_) => {
            let a = *i;
            while *i < b.len()
                && !matches!(b[*i], b',' | b']' | b'}' | b' ' | b'\n' | b'\r' | b'\t')
            {
                *i += 1;
            }
            if a == *i {
                return Err(format!("bad JSON at {a}"));
            }
            Ok(J::Tok(s(a, *i)))
        }
    }
}

/// JavaScript truthiness of a parsed script value (`Null` for a missing key).
pub fn truthy(v: Option<&serde_json::Value>) -> bool {
    use serde_json::Value;
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0 && !x.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// `encodeURIComponent`.
pub fn encode_uri_component(s: &str) -> String {
    let mut out = String::new();
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Strict `decodeURIComponent` (None where JavaScript throws URIError).
pub fn decode_uri_component(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let h = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(h, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Standard base64 decoding (as Node's `Buffer.from (s, "base64")` for valid input).
pub fn base64_decode(s: &str) -> Vec<u8> {
    let mut table = [255u8; 256];
    for (i, c) in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
        .iter()
        .enumerate()
    {
        table[*c as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for &c in s.as_bytes() {
        let v = table[c as usize];
        if v == 255 {
            continue;
        }
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_print_like_javascript() {
        for (x, s) in [
            (0.0, "0"),
            (-0.0, "0"),
            (1.0, "1"),
            (-1.5, "-1.5"),
            (0.1, "0.1"),
            (1e21, "1e+21"),
            (1e20, "100000000000000000000"),
            (1e-6, "0.000001"),
            (1e-7, "1e-7"),
            (123456789.125, "123456789.125"),
            (1.2345e-9, "1.2345e-9"),
            (946684800.0, "946684800"),
            (1.0 / 3.0, "0.3333333333333333"),
            (5e-324, "5e-324"),
            (1.7976931348623157e308, "1.7976931348623157e+308"),
        ] {
            assert_eq!(js_number(x), s, "{x:e}");
        }
    }

    #[test]
    fn pretty_matches_json_stringify() {
        let v = parse(r#"{"a":[1,{"b":[]},{}],"c":"x\"y\n","d":-0.5e-7,"e":{"f":null}}"#).unwrap();
        assert_eq!(
            v.pretty().unwrap(),
            "{\n  \"a\": [\n    1,\n    {\n      \"b\": []\n    },\n    {}\n  ],\n  \"c\": \"x\\\"y\\n\",\n  \"d\": -0.5e-7,\n  \"e\": {\n    \"f\": null\n  }\n}"
        );
        let mut o = J::obj();
        o.set("ok", J::bool(false));
        o.set("u", J::Undef);
        o.set("n", J::Arr(vec![J::Undef, J::int(2)]));
        o.set("ok", J::bool(true));
        assert_eq!(
            o.pretty().unwrap(),
            "{\n  \"ok\": true,\n  \"n\": [\n    null,\n    2\n  ]\n}"
        );
    }

    #[test]
    fn strings_escape_like_json_stringify() {
        assert_eq!(
            escape("a\u{1}\u{1f}\u{7f}é\t\\"),
            "\"a\\u0001\\u001f\u{7f}é\\t\\\\\""
        );
    }

    #[test]
    fn uri_components() {
        assert_eq!(
            encode_uri_component("a b/ü(1).wrl"),
            "a%20b%2F%C3%BC(1).wrl"
        );
        assert_eq!(decode_uri_component("a%20b%C3%BC").as_deref(), Some("a bü"));
        assert_eq!(decode_uri_component("%E0%A4%A"), None);
    }

    #[test]
    fn rounding_and_base64() {
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(0.49999999999999994), 0.0);
        assert_eq!(base64_decode("aGVsbG8="), b"hello");
    }
}
