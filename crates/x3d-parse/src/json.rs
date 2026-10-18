//! Port of src/x_ite/Parser/JSONParser.js: `JSON.parse`, then the conversion of the JSON object
//! into X3D DOM elements, which XMLParser.js then parses (xml.rs).
//!
//! JSONParser walks objects with `for (const key in object)`, so the element and attribute
//! order is JavaScript property order: array-index keys ascending, then the other keys in
//! insertion order (a repeated key keeps its first position and its last value).

use crate::dom::{Element, XNode};
use crate::lexer::js_parse_int;
use x3d_fields::jsmath::number_to_string;

#[derive(Clone, Debug)]
pub(crate) enum JsValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsValue>),
    Object(Vec<(String, JsValue)>),
}

impl JsValue {
    /// `typeof value === "object"` (arrays and `null` included).
    fn is_object(&self) -> bool {
        matches!(self, JsValue::Null | JsValue::Array(_) | JsValue::Object(_))
    }

    /// `for (const key in value)`: the enumerable keys with their values.
    fn entries(&self) -> Vec<(String, &JsValue)> {
        match self {
            JsValue::Array(a) => a.iter().enumerate().map(|(i, v)| (i.to_string(), v)).collect(),
            JsValue::Object(o) => o.iter().map(|(k, v)| (k.clone(), v)).collect(),
            _ => vec![],
        }
    }
}

/// A canonical array index ("0", "1", … below 2^32 - 1), which JS enumerates first.
fn is_array_index(key: &str) -> bool {
    !key.is_empty()
        && key.bytes().all(|b| b.is_ascii_digit())
        && (key == "0" || !key.starts_with('0'))
        && key.parse::<u64>().is_ok_and(|n| n < u32::MAX as u64)
}

// --- JSON.parse -----------------------------------------------------------------------------

struct Json<'a> {
    s: &'a [u8],
    i: usize,
}

impl Json<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn eat(&mut self, b: u8) -> bool {
        self.ws();

        if self.s.get(self.i) == Some(&b) {
            self.i += 1;
            return true;
        }

        false
    }

    fn value(&mut self) -> Result<JsValue, String> {
        self.ws();

        match self.s.get(self.i) {
            Some(b'{') => {
                self.i += 1;

                let mut index: Vec<(u64, JsValue)> = vec![];
                let mut named: Vec<(String, JsValue)> = vec![];

                if !self.eat(b'}') {
                    loop {
                        self.ws();
                        let key = self.string()?;

                        if !self.eat(b':') {
                            return Err("Expected ':'.".into());
                        }

                        let v = self.value()?;

                        if is_array_index(&key) {
                            let n = key.parse::<u64>().unwrap();
                            match index.iter_mut().find(|e| e.0 == n) {
                                Some(e) => e.1 = v,
                                None => index.push((n, v)),
                            }
                        } else {
                            match named.iter_mut().find(|e| e.0 == key) {
                                Some(e) => e.1 = v,
                                None => named.push((key, v)),
                            }
                        }

                        if self.eat(b',') {
                            continue;
                        }

                        if self.eat(b'}') {
                            break;
                        }

                        return Err("Expected ',' or '}'.".into());
                    }
                }

                index.sort_by_key(|e| e.0);

                let mut entries: Vec<(String, JsValue)> = index.into_iter().map(|(n, v)| (n.to_string(), v)).collect();
                entries.extend(named);
                Ok(JsValue::Object(entries))
            }
            Some(b'[') => {
                self.i += 1;

                let mut items = vec![];

                if !self.eat(b']') {
                    loop {
                        items.push(self.value()?);

                        if self.eat(b',') {
                            continue;
                        }

                        if self.eat(b']') {
                            break;
                        }

                        return Err("Expected ',' or ']'.".into());
                    }
                }

                Ok(JsValue::Array(items))
            }
            Some(b'"') => Ok(JsValue::String(self.string()?)),
            Some(b't') if self.s[self.i..].starts_with(b"true") => {
                self.i += 4;
                Ok(JsValue::Bool(true))
            }
            Some(b'f') if self.s[self.i..].starts_with(b"false") => {
                self.i += 5;
                Ok(JsValue::Bool(false))
            }
            Some(b'n') if self.s[self.i..].starts_with(b"null") => {
                self.i += 4;
                Ok(JsValue::Null)
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err("Unexpected token.".into()),
        }
    }

    fn number(&mut self) -> Result<JsValue, String> {
        let start = self.i;
        let s = self.s;
        let digits = |i: &mut usize| {
            let k = *i;
            while *i < s.len() && s[*i].is_ascii_digit() {
                *i += 1;
            }
            *i - k
        };

        if s.get(self.i) == Some(&b'-') {
            self.i += 1;
        }

        if s.get(self.i) == Some(&b'0') {
            self.i += 1;
        } else if digits(&mut self.i) == 0 {
            return Err("No number after minus sign.".into());
        }

        if s.get(self.i) == Some(&b'.') {
            self.i += 1;
            if digits(&mut self.i) == 0 {
                return Err("Unterminated fractional number.".into());
            }
        }

        if matches!(s.get(self.i), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(s.get(self.i), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if digits(&mut self.i) == 0 {
                return Err("Exponent part is missing a number.".into());
            }
        }

        let text = std::str::from_utf8(&s[start..self.i]).unwrap();
        Ok(JsValue::Number(text.parse::<f64>().map_err(|e| e.to_string())?))
    }

    fn string(&mut self) -> Result<String, String> {
        if self.s.get(self.i) != Some(&b'"') {
            return Err("Expected string.".into());
        }

        self.i += 1;

        // UTF-16 code units, so that escaped surrogate pairs combine (and lone ones are kept as
        // replacement characters, the closest a Rust String gets).
        let mut units: Vec<u16> = vec![];

        loop {
            let Some(&b) = self.s.get(self.i) else {
                return Err("Unterminated string in JSON.".into());
            };

            match b {
                b'"' => {
                    self.i += 1;
                    break;
                }
                b'\\' => {
                    self.i += 1;

                    let e = *self.s.get(self.i).ok_or("Bad escaped character in JSON.")?;
                    self.i += 1;

                    let c = match e {
                        b'"' => '"' as u16,
                        b'\\' => '\\' as u16,
                        b'/' => '/' as u16,
                        b'b' => 8,
                        b'f' => 12,
                        b'n' => 10,
                        b'r' => 13,
                        b't' => 9,
                        b'u' => {
                            let hex = self.s.get(self.i..self.i + 4).ok_or("Bad Unicode escape in JSON.")?;
                            let hex = std::str::from_utf8(hex).map_err(|e| e.to_string())?;
                            self.i += 4;
                            u16::from_str_radix(hex, 16).map_err(|_| "Bad Unicode escape in JSON.")?
                        }
                        _ => return Err("Bad escaped character in JSON.".into()),
                    };

                    units.push(c);
                }
                0..=0x1f => return Err("Bad control character in string literal in JSON.".into()),
                _ => {
                    // One UTF-8 character.
                    let rest = std::str::from_utf8(&self.s[self.i..]).map_err(|e| e.to_string())?;
                    let c = rest.chars().next().unwrap();

                    self.i += c.len_utf8();

                    let mut buf = [0u16; 2];
                    units.extend_from_slice(c.encode_utf16(&mut buf));
                }
            }
        }

        Ok(String::from_utf16_lossy(&units))
    }
}

/// `JSON.parse (string)`.
pub(crate) fn json_parse(text: &str) -> Result<JsValue, String> {
    let mut p = Json { s: text.as_bytes(), i: 0 };
    let v = p.value()?;

    p.ws();

    if p.i != p.s.len() {
        return Err("Unexpected non-whitespace character after JSON.".into());
    }

    Ok(v)
}

/// JSONParser.isValid: an object whose keys are all "X3D".
pub(crate) fn is_valid(v: &JsValue) -> bool {
    match v {
        JsValue::Object(o) => o.iter().all(|(k, _)| k == "X3D"),
        JsValue::Array(a) => a.is_empty(),
        _ => false,
    }
}

// --- JSON to DOM ----------------------------------------------------------------------------

/// The DOM's Name check for createElementNS and setAttribute (InvalidCharacterError).
fn valid_name(name: &str) -> bool {
    let start = |c: char| c.is_alphabetic() || c == '_' || c == ':' || !c.is_ascii();
    let rest = |c: char| start(c) || c.is_ascii_digit() || c == '-' || c == '.';
    let mut chars = name.chars();

    matches!(chars.next(), Some(c) if start(c)) && chars.all(rest)
}

/// `isNaN (parseInt (key))`.
fn not_int(key: &str) -> bool {
    js_parse_int(key).is_nan()
}

/// `key .substring (1)`.
fn substring1(key: &str) -> &str {
    key.char_indices().nth(1).map(|(i, _)| &key[i..]).unwrap_or("")
}

fn js_string(v: &JsValue) -> String {
    match v {
        JsValue::Null => "null".into(),
        JsValue::Bool(b) => b.to_string(),
        JsValue::Number(n) => number_to_string(*n),
        JsValue::String(s) => s.clone(),
        _ => String::new(),
    }
}

struct Converter;

type R = Result<(), String>;

impl Converter {
    fn set_attribute(element: &mut Element, key: &str, value: String) -> R {
        match key {
            "SON schema" | "ncoding" => Ok(()),
            _ => {
                if !valid_name(key) {
                    return Err(format!("Failed to execute 'setAttribute' on 'Element': '{key}' is not a valid attribute name."));
                }

                element.set_attr(key, value);
                Ok(())
            }
        }
    }

    fn create_element(key: &str, container_field: Option<&str>) -> Result<Element, String> {
        if !valid_name(key) {
            return Err(format!("Failed to execute 'createElementNS' on 'Document': The qualified name provided ('{key}') contains the invalid name-start character."));
        }

        let mut child = Element::new(key);

        if let Some(cf) = container_field {
            Self::set_attribute(&mut child, "containerField", cf.to_string())?;
        }

        Ok(child)
    }

    fn convert_children(parent_key: &str, object: &JsValue, element: &mut Element) -> R {
        let cf = substring1(parent_key);

        for (key, value) in object.entries() {
            if !value.is_object() {
                continue;
            }

            if not_int(&key) {
                Self::convert_object(&key, value, element, Some(cf))?;
            } else {
                Self::convert_to_dom(value, &key, element, Some(cf))?;
            }
        }

        Ok(())
    }

    /// convertObject (key, object, element, containerField) with `object [key]` = `value`.
    fn convert_object(key: &str, value: &JsValue, element: &mut Element, container_field: Option<&str>) -> R {
        if !value.is_object() {
            return Ok(());
        }

        if key.starts_with('@') {
            Self::convert_to_dom(value, key, element, None)
        } else if key.starts_with('-') {
            Self::convert_children(key, value, element)
        } else if key == "#comment" {
            for (_, c) in value.entries() {
                element.children.push(XNode::Comment(js_string(c).replace("\\\\", "\\")));
            }
            Ok(())
        } else if key == "#sourceCode" || key == "#sourceText" {
            // object [key] .join ("\n")
            let JsValue::Array(lines) = value else {
                return Err("object[key].join is not a function".into());
            };

            let text = lines.iter().map(|l| if matches!(l, JsValue::Null) { String::new() } else { js_string(l) }).collect::<Vec<_>>().join("\n");
            element.children.push(XNode::CData(text));
            Ok(())
        } else if matches!(key, "connect" | "fieldValue" | "field" | "meta" | "component" | "unit") {
            for (child_key, child) in value.entries() {
                if child.is_object() {
                    let mut c = Self::create_element(key, container_field)?;
                    Self::convert_to_dom(child, &child_key, &mut c, None)?;
                    element.children.push(XNode::Element(c));
                }
            }
            Ok(())
        } else {
            let mut child = Self::create_element(key, container_field)?;
            Self::convert_to_dom(value, key, &mut child, None)?;
            element.children.push(XNode::Element(child));
            Ok(())
        }
    }

    fn convert_to_dom(object: &JsValue, parent_key: &str, element: &mut Element, container_field: Option<&str>) -> R {
        if matches!(object, JsValue::Null) {
            if !not_int(parent_key) {
                let child = Self::create_element("NULL", container_field)?;
                element.children.push(XNode::Element(child));
            } else {
                Self::set_attribute(element, "value", "null".into())?;
            }

            return Ok(());
        }

        let mut is_array = false;
        let mut local: Vec<String> = vec![];
        let mut strings = false;

        for (key, value) in object.entries() {
            is_array = !not_int(&key);

            if is_array {
                match value {
                    JsValue::Number(_) | JsValue::Bool(_) => local.push(js_string(value)),
                    JsValue::String(s) => {
                        local.push(s.clone());
                        strings = true;
                    }
                    _ => Self::convert_to_dom(value, &key, element, None)?,
                }
            } else {
                match value {
                    _ if value.is_object() => {
                        if key == "X3D" {
                            Self::convert_to_dom(value, &key, element, None)?;
                        } else {
                            Self::convert_object(&key, value, element, container_field)?;
                        }
                    }
                    JsValue::Number(_) | JsValue::Bool(_) => Self::set_attribute(element, substring1(&key), js_string(value))?,
                    JsValue::String(s) => {
                        if key != "#comment" {
                            // JSONStringToXML
                            Self::set_attribute(element, substring1(&key), s.replace('\\', "\\\\").replace('\n', "\\n"))?;
                        } else {
                            element.children.push(XNode::Comment(s.replace("\\\\", "\\")));
                        }
                    }
                    _ => {}
                }
            }
        }

        if is_array && parent_key.starts_with('@') {
            let name = substring1(parent_key);

            if strings {
                // SFStringToXML: backslashes and double quotes escaped.
                let quoted: Vec<String> = local.iter().map(|s| s.replace('\\', "\\\\").replace('"', "\\\"")).collect();
                Self::set_attribute(element, name, format!("\"{}\"", quoted.join("\" \"")))?;
            } else {
                Self::set_attribute(element, name, local.join(" "))?;
            }
        }

        Ok(())
    }
}

/// JSONParser.parseIntoScene up to the DOM: the `<X3D>` element XMLParser parses.
pub(crate) fn to_x3d_element(input: &JsValue) -> Result<Element, String> {
    let mut child = Element::new("X3D");

    Converter::convert_to_dom(input, "", &mut child, None)?;
    Ok(child)
}
