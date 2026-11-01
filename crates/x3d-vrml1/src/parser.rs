//! The reading pass: the extension's `VRML1Parser` (a recursive-descent parser over the sticky
//! regular expressions of its `Grammar`, reusing X_ITE's `VRMLParser` for field values).
//!
//! Lexical rules kept from the extension: white space is `[ \n,\t\r]+` (so commas separate), a
//! comment is `#` to the end of the line or `#/* ... */#`, keywords `DEF`, `USE`, `NULL` and
//! `fields` match as bare prefixes, and an `Id` allows nearly every character (so `(SIDES|TOP)`
//! is one Id). Any name followed by `{` is a node; a name the extension has no class for is a
//! custom node.

use crate::Vrml1Error;
use crate::nodes::{self, Fields};
use std::collections::HashMap;
use x3d_fields::math::{Color3, Matrix4, Rotation4, Vector2, Vector3};
use x3d_fields::value::unescape;
use x3d_fields::{FieldType, FieldValue as V, Image};

#[derive(Debug)]
pub(crate) struct V1Node {
    pub type_name: String,
    /// The `DEF` name, empty for none.
    pub name: String,
    pub fields: Fields,
    /// Child nodes, as indices into [`Doc::nodes`].
    pub children: Vec<usize>,
}

impl V1Node {
    pub fn get(&self, name: &str) -> &V {
        &self
            .fields
            .iter()
            .find(|(k, _)| k == name)
            .unwrap_or_else(|| panic!("{} has no field {name}", self.type_name))
            .1
    }

    pub fn float(&self, name: &str) -> f64 {
        match self.get(name) {
            V::SFFloat(v) | V::SFDouble(v) | V::SFTime(v) => *v,
            V::SFInt32(v) => *v as f64,
            _ => panic!("{name} is not a float"),
        }
    }

    pub fn string(&self, name: &str) -> &str {
        match self.get(name) {
            V::SFString(s) => s,
            _ => panic!("{name} is not a string"),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Doc {
    pub nodes: Vec<V1Node>,
    pub roots: Vec<usize>,
}

#[derive(Clone, Copy, PartialEq)]
enum Stmt {
    Node(usize),
    Null,
    No,
}

struct Parser<'a> {
    src: &'a str,
    b: &'a [u8],
    pos: usize,
    names: HashMap<String, usize>,
    doc: Doc,
}

pub(crate) fn line_column(src: &str, pos: usize) -> (usize, usize) {
    let pos = pos.min(src.len());
    let b = src.as_bytes();
    let mut line = 1;
    let mut start = 0;
    let mut i = 0;

    while i < pos {
        match b[i] {
            b'\n' => {
                line += 1;
                start = i + 1;
            }
            // A lone CR is a line break too (old Mac files), CR LF is one.
            b'\r' if b.get(i + 1) != Some(&b'\n') => {
                line += 1;
                start = i + 1;
            }
            b'\r' => start = i + 2,
            _ => {}
        }
        i += 1;
    }

    // `pos` can fall inside a multi-byte character only by a bug; step back to a boundary.
    let mut end = pos;
    while !src.is_char_boundary(end) {
        end -= 1;
    }
    let start = start.min(end);

    (line, src[start..end].chars().count() + 1)
}

pub(crate) fn error_at(src: &str, pos: usize, message: impl Into<String>) -> Vrml1Error {
    let (line, column) = line_column(src, pos);

    Vrml1Error {
        message: message.into(),
        line,
        column,
    }
}

pub(crate) fn parse(src: &str) -> Result<Doc, Vrml1Error> {
    let mut p = Parser {
        src,
        b: src.as_bytes(),
        pos: 0,
        names: HashMap::new(),
        doc: Doc::default(),
    };

    p.vrml_scene()?;
    Ok(p.doc)
}

type R<T> = Result<T, Vrml1Error>;

fn is_id_first(b: u8) -> bool {
    !(b <= 0x20
        || matches!(
            b,
            0x30..=0x39
                | 0x22
                | 0x23
                | 0x27
                | 0x2b
                | 0x2c
                | 0x2d
                | 0x2e
                | 0x5b
                | 0x5c
                | 0x5d
                | 0x7b
                | 0x7d
                | 0x7f
        ))
}

fn is_id_rest(b: u8) -> bool {
    !(b <= 0x20
        || matches!(
            b,
            0x22 | 0x23 | 0x27 | 0x2c | 0x2e | 0x5b | 0x5c | 0x5d | 0x7b | 0x7d | 0x7f
        ))
}

impl<'a> Parser<'a> {
    fn err<T>(&self, pos: usize, message: impl Into<String>) -> R<T> {
        Err(error_at(self.src, pos, message))
    }

    // --- lexical -------------------------------------------------------------------------

    /// `comments`: white space and comments, any number.
    fn comments(&mut self) {
        loop {
            let start = self.pos;

            while self.pos < self.b.len()
                && matches!(self.b[self.pos], b' ' | b'\n' | b',' | b'\t' | b'\r')
            {
                self.pos += 1;
            }

            if self.b.get(self.pos) == Some(&b'#') {
                let mut done = false;

                if self.b[self.pos..].starts_with(b"#/*") {
                    // `#/*.*?*/#`
                    let mut i = self.pos + 3;

                    while i + 3 <= self.b.len() {
                        if &self.b[i..i + 3] == b"*/#" {
                            self.pos = i + 3;
                            done = true;
                            break;
                        }
                        i += 1;
                    }
                }

                if !done {
                    let mut i = self.pos + 1;

                    while i < self.b.len() && self.b[i] != b'\r' && self.b[i] != b'\n' {
                        i += 1;
                    }
                    self.pos = i;
                }
            }

            if self.pos == start {
                return;
            }
        }
    }

    /// A bare literal such as `/DEF/y`.
    fn literal(&mut self, word: &str) -> bool {
        if self.b[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            true
        } else {
            false
        }
    }

    fn punct(&mut self, c: u8) -> bool {
        self.comments();

        if self.b.get(self.pos) == Some(&c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// `Id`
    fn id(&mut self) -> Option<String> {
        self.comments();

        let start = self.pos;

        if start >= self.b.len() || !is_id_first(self.b[start]) {
            return None;
        }

        let mut i = start + 1;

        while i < self.b.len() && is_id_rest(self.b[i]) {
            i += 1;
        }

        self.pos = i;
        Some(self.src[start..i].to_string())
    }

    // --- scene ---------------------------------------------------------------------------

    /// `vrmlScene`: statements, then the input must be used up.
    fn vrml_scene(&mut self) -> R<()> {
        while let Stmt::Node(n) = self.node_statement()? {
            self.doc.roots.push(n);
        }

        if self.pos < self.b.len() {
            return self.err(self.pos, "Unknown statement.");
        }

        Ok(())
    }

    fn node_statement(&mut self) -> R<Stmt> {
        self.comments();

        if self.literal("DEF") {
            let at = self.pos;

            return match self.id() {
                Some(name) => self.node(&name),
                None => self.err(at, "No name given after DEF."),
            };
        }

        if self.literal("USE") {
            let at = self.pos;

            return match self.id() {
                Some(name) => Ok(match self.names.get(&name) {
                    Some(n) => Stmt::Node(*n),
                    None => Stmt::No,
                }),
                None => self.err(at, "No name given after USE."),
            };
        }

        if self.literal("NULL") {
            return Ok(Stmt::Null);
        }

        self.node("")
    }

    fn node(&mut self, def: &str) -> R<Stmt> {
        let save = self.pos;

        let Some(type_name) = self.id() else {
            return Ok(Stmt::No);
        };

        if !self.punct(b'{') {
            self.pos = save;
            return Ok(Stmt::No);
        }

        let known = nodes::defaults(&type_name);
        let custom = known.is_none();
        let index = self.doc.nodes.len();

        self.doc.nodes.push(V1Node {
            type_name,
            name: def.to_string(),
            fields: known.unwrap_or_default(),
            children: vec![],
        });

        if !def.is_empty() {
            self.names.insert(def.to_string(), index);
        }

        if custom {
            self.field_elements(index)?;
        }

        while self.node_body_element(index)? {}

        self.comments();

        if self.punct(b'}') {
            return Ok(Stmt::Node(index));
        }

        self.err(self.pos, "Expected '}' at the end of node body.")
    }

    fn node_body_element(&mut self, node: usize) -> R<bool> {
        if let Stmt::Node(child) = self.node_statement()? {
            self.doc.nodes[node].children.push(child);
            return Ok(true);
        }

        let Some(field_id) = self.id() else {
            return Ok(false);
        };
        let at = self.pos;
        let type_name = self.doc.nodes[node].type_name.clone();
        let Some(slot) = self.doc.nodes[node]
            .fields
            .iter()
            .position(|(k, _)| *k == field_id)
        else {
            return self.err(
                at - field_id.len(),
                format!("Unknown field '{field_id}' in class '{type_name}'."),
            );
        };
        let ty = self.doc.nodes[node].fields[slot].1.field_type();

        match self.field_value(ty)? {
            Some(value) => {
                self.doc.nodes[node].fields[slot].1 = value;
                Ok(true)
            }
            None => self.err(
                self.pos,
                format!("Couldn't read value for field '{field_id}' of node {type_name}."),
            ),
        }
    }

    /// `fieldElements`: the `fields [ type name ... ]` declaration of a custom node.
    fn field_elements(&mut self, node: usize) -> R<()> {
        self.comments();

        if !self.literal("fields") {
            return Ok(());
        }

        if self.punct(b'[') {
            while let Some(ty) = self.id() {
                let at = self.pos;
                let Some(name) = self.id() else { break };
                let ty_name = nodes::substitute_field_type(&ty);
                let Some(field_type) = FieldType::from_name(ty_name) else {
                    return self.err(at - ty.len(), format!("Unknown field type '{ty}'."));
                };

                let fields = &mut self.doc.nodes[node].fields;
                let value = V::default_for(field_type);

                match fields.iter_mut().find(|(k, _)| *k == name) {
                    Some(slot) => slot.1 = value,
                    None => fields.push((name, value)),
                }
            }

            if self.punct(b']') {
                return Ok(());
            }
        }

        self.err(self.pos, "Couldn't parse custom fields.")
    }

    // --- field values (VRMLParser, VRML 1.0 subset) ----------------------------------------

    fn double(&mut self) -> Option<f64> {
        self.comments();

        let s = &self.b[self.pos..];

        if s.len() >= 2 && s[0] == b'0' && (s[1] == b'x' || s[1] == b'X') {
            return None;
        }

        let digits = |from: usize| {
            let mut j = from;
            while j < s.len() && s[j].is_ascii_digit() {
                j += 1;
            }
            j
        };
        let i = usize::from(!s.is_empty() && (s[0] == b'+' || s[0] == b'-'));
        let int_end = digits(i);
        let mut end = None;

        // \d*\.\d+
        if int_end < s.len() && s[int_end] == b'.' {
            let frac_end = digits(int_end + 1);

            if frac_end > int_end + 1 {
                end = Some(frac_end);
            }
        }

        // \d+\.?
        if end.is_none() && int_end > i {
            end = Some(if int_end < s.len() && s[int_end] == b'.' {
                int_end + 1
            } else {
                int_end
            });
        }

        let mut end = end?;

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

        let value = self.src[self.pos..self.pos + end].parse::<f64>().ok()?;

        self.pos += end;
        Some(value)
    }

    fn int32(&mut self) -> Option<f64> {
        self.comments();

        let s = &self.b[self.pos..];

        if s.len() >= 3
            && s[0] == b'0'
            && (s[1] == b'x' || s[1] == b'X')
            && s[2].is_ascii_hexdigit()
        {
            let mut i = 3;

            while i < s.len() && s[i].is_ascii_hexdigit() {
                i += 1;
            }

            let value = u64::from_str_radix(&self.src[self.pos + 2..self.pos + i], 16)
                .map(|v| v as f64)
                .unwrap_or(f64::INFINITY);

            self.pos += i;
            return Some(value);
        }

        let mut i = usize::from(!s.is_empty() && (s[0] == b'+' || s[0] == b'-'));
        let start = i;

        while i < s.len() && s[i].is_ascii_digit() {
            i += 1;
        }

        if i == start {
            return None;
        }

        let value = self.src[self.pos..self.pos + i].parse::<f64>().ok()?;

        self.pos += i;
        Some(value)
    }

    /// `VRMLParser.string`: a double-quoted string, `\"` and `\\` escaped.
    fn string(&mut self) -> Option<String> {
        self.comments();

        if self.b.get(self.pos) != Some(&b'"') {
            return None;
        }

        let mut i = self.pos + 1;
        let mut raw = String::new();

        loop {
            let start = i;

            while i < self.b.len() && self.b[i] != b'"' {
                i += 1;
            }

            raw.push_str(&self.src[start..i]);

            let trailing = raw.bytes().rev().take_while(|b| *b == b'\\').count();

            if i >= self.b.len() || trailing % 2 == 0 {
                break;
            }

            // An escaped quote: keep it and read on.
            raw.push('"');
            i += 1;
        }

        if self.b.get(i) == Some(&b'"') {
            i += 1;
        }

        self.pos = i;
        Some(unescape(&raw))
    }

    fn bool_(&mut self) -> Option<bool> {
        self.comments();

        for (word, value) in [
            ("TRUE", true),
            ("true", true),
            ("FALSE", false),
            ("false", false),
        ] {
            if self.literal(word) {
                return Some(value);
            }
        }

        None
    }

    fn doubles(&mut self, n: usize) -> Option<Vec<f64>> {
        (0..n).map(|_| self.double()).collect()
    }

    /// SFString of the extension: a quoted string, else an SFEnum / SFBitMask written as
    /// `Id ( '|' Id )*`, kept as `A|B` without the spaces.
    fn sfstring(&mut self) -> Option<String> {
        if let Some(s) = self.string() {
            return Some(s);
        }

        let mut value = String::new();

        while let Some(id) = self.id() {
            value.push_str(&id);

            if self.punct(b'|') {
                value.push('|');
                continue;
            }

            return Some(value);
        }

        // The JS sets "" and returns false; the caller reports the error.
        None
    }

    fn sfimage(&mut self) -> Option<Image> {
        let width = self.int32()?;
        let height = self.int32()?;
        let comp = self.int32()?;
        let mut array = vec![];
        let mut i = 0.0;

        while i < width * height {
            array.push(x3d_fields::jsmath::to_int32(self.int32()?));
            i += 1.0;
        }

        Some(Image::new(width, height, comp, array))
    }

    /// A multi-valued field: one element without brackets, or a bracketed list.
    fn mf_numbers(&mut self, components: usize, int: bool, clamp01: bool) -> R<Option<Vec<f64>>> {
        let read = |p: &mut Self| -> Option<f64> { if int { p.int32() } else { p.double() } };

        if self.punct(b'[') {
            let mut out = vec![];

            while let Some(v) = read(self) {
                out.push(v);
            }

            if self.punct(b']') {
                return Ok(Some(out));
            }

            return self.err(self.pos, "Expected ']'.");
        }

        let mut out = vec![];

        for _ in 0..components {
            match read(self) {
                Some(v) => out.push(if clamp01 { v.clamp(0.0, 1.0) } else { v }),
                None => return Ok(None),
            }
        }

        Ok(Some(out))
    }

    /// `VRMLParser.fieldValue` for the field types a VRML 1.0 scene can use. `None` is the JS
    /// `false`: no value there.
    fn field_value(&mut self, ty: FieldType) -> R<Option<V>> {
        use FieldType as T;

        Ok(match ty {
            T::SFBool => self.bool_().map(V::SFBool),
            T::SFFloat => self.double().map(V::SFFloat),
            T::SFDouble => self.double().map(V::SFDouble),
            T::SFTime => self.double().map(V::SFTime),
            T::SFInt32 => self
                .int32()
                .map(|v| V::SFInt32(x3d_fields::jsmath::to_int32(v))),
            T::SFString => self.sfstring().map(V::SFString),
            T::SFVec2f => self
                .doubles(2)
                .map(|v| V::SFVec2f(Vector2::new(v[0], v[1]))),
            T::SFVec3f => self
                .doubles(3)
                .map(|v| V::SFVec3f(Vector3::new(v[0], v[1], v[2]))),
            T::SFColor => self
                .doubles(3)
                .map(|v| V::SFColor(Color3::new(v[0], v[1], v[2]))),
            T::SFRotation => self
                .doubles(4)
                .map(|v| V::SFRotation(Rotation4::new(v[0], v[1], v[2], v[3]))),
            T::SFMatrix4f => self.doubles(16).map(|v| {
                let mut m = [0.0; 16];
                m.copy_from_slice(&v);
                V::SFMatrix4f(Matrix4(m))
            }),
            T::SFImage => self.sfimage().map(V::SFImage),
            T::MFString => {
                if let Some(s) = self.string() {
                    Some(V::MFString(vec![s]))
                } else if self.punct(b'[') {
                    let mut list = vec![];

                    while let Some(s) = self.string() {
                        list.push(s);
                    }

                    if !self.punct(b']') {
                        return self.err(self.pos, "Expected ']'.");
                    }

                    Some(V::MFString(list))
                } else {
                    None
                }
            }
            T::MFBool
            | T::MFColor
            | T::MFDouble
            | T::MFFloat
            | T::MFInt32
            | T::MFRotation
            | T::MFTime
            | T::MFVec2f
            | T::MFVec3f
            | T::MFVec4f
            | T::MFMatrix4f => {
                let int = ty == T::MFInt32;
                let colors = ty == T::MFColor;

                if ty == T::MFBool {
                    return Ok(self.mf_bools());
                }

                match self.mf_numbers(ty.components(), int, colors)? {
                    Some(list) => {
                        let mut value = V::default_for(ty);

                        value.set_flat_numbers(&list);
                        Some(value)
                    }
                    None => None,
                }
            }
            // Node-valued fields: the extension's SFNode reader returns false.
            _ => None,
        })
    }

    fn mf_bools(&mut self) -> Option<V> {
        if self.punct(b'[') {
            let mut list = vec![];

            while let Some(b) = self.bool_() {
                list.push(b);
            }

            return self.punct(b']').then_some(V::MFBool(list));
        }

        self.bool_().map(|b| V::MFBool(vec![b]))
    }
}
