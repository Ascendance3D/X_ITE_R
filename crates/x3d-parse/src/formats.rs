//! Ports of the geometry file parsers of src/x_ite/Parser/: STLAParser.js, STLBParser.js,
//! PLYAParser.js, PLYBParser.js and OBJParser.js (with its MaterialParser). Each builds its scene
//! through the scene API (X3DExecutionContext.createNode, field assignments, addNamedNode,
//! addExportedNode, rootNodes), as the JS parsers do; the lexical grammars are the parsers' own
//! sticky regular expressions, written out as functions below.

use crate::lexer::{self, js_parse_int};
use crate::{LoadError, ParseError};
use std::collections::HashMap;
use x3d_fields::{FieldValue, NodeId, jsmath};
use x3d_scene::*;

type R<T> = Result<T, LoadError>;

fn error<T>(message: impl Into<String>) -> R<T> {
    Err(LoadError::Parse(ParseError { message: message.into(), line: 0 }))
}

// --- scene API ------------------------------------------------------------------------------

/// X3DExecutionContext.createNode (typeName): unknown types and types outside the
/// specification range throw; a type outside the profile and components only warns.
pub(crate) fn create(scene: &mut Scene, type_name: &str) -> R<NodeId> {
    match scene.create_node(ROOT, type_name) {
        Some(n) => Ok(n),
        None => error(format!("Unknown node type '{type_name}'.")),
    }
}

/// `node [name] = value` for a predefined field.
pub(crate) fn set(scene: &mut Scene, node: NodeId, name: &str, value: FieldValue) {
    let i = scene.get_predefined_field(node, name).unwrap_or_else(|| panic!("{} has no field {name}", scene.type_name(node)));
    let f = &mut scene.node_mut(node).fields[i];

    f.value = value;
    f.modified = true;
}

/// `node [name] = array` for a typed MF field, from JS numbers.
pub(crate) fn set_numbers(scene: &mut Scene, node: NodeId, name: &str, values: &[f64]) {
    let i = scene.get_predefined_field(node, name).unwrap();
    let f = &mut scene.node_mut(node).fields[i];

    f.value.set_flat_numbers(values);
    f.modified = true;
}

pub(crate) fn set_node(scene: &mut Scene, node: NodeId, name: &str, value: Option<NodeId>) {
    set(scene, node, name, FieldValue::SFNode(value));
}

/// `node [name] .push (child)` on an MFNode field.
pub(crate) fn push_child(scene: &mut Scene, node: NodeId, name: &str, child: NodeId) {
    let i = scene.get_predefined_field(node, name).unwrap();
    let f = &mut scene.node_mut(node).fields[i];

    if let FieldValue::MFNode(v) = &mut f.value {
        v.push(Some(child));
    }

    f.modified = true;
}

pub(crate) fn node_field(scene: &Scene, node: NodeId, name: &str) -> FieldValue {
    scene.node(node).fields[scene.get_predefined_field(node, name).unwrap()].value.clone()
}

/// `scene .addNamedNode (scene .getUniqueName (name), node)` and the same for the exported name.
fn name_and_export(scene: &mut Scene, name: &str, node: NodeId) {
    let unique = scene.get_unique_name(ROOT, name);
    scene.update_named_node(ROOT, &unique, node);

    let exported = scene.get_unique_export_name(name);
    let _ = scene.update_exported_node(ROOT, &exported, node, "");
}

fn add_named(scene: &mut Scene, name: &str, node: NodeId) {
    let unique = scene.get_unique_name(ROOT, name);
    scene.update_named_node(ROOT, &unique, node);
}

fn root_push(scene: &mut Scene, node: NodeId) {
    scene.ctx_mut(ROOT).root_nodes.push(Some(node));
}

fn begin(world_url: &str, encoding: &str) -> Scene {
    let mut scene = Scene::new(world_url);

    scene.encoding = encoding.into();
    scene.profile = Some("Interchange".into());
    scene
}

/// X3DParser.rotateAxes90: (x, y, z) → (x, z, -y), for STL and PLY models.
fn rotate_axes_90(array: &mut [f64]) {
    for v in array.chunks_exact_mut(3) {
        let z = -v[1];
        v[1] = v[2];
        v[2] = z;
    }
}

/// JS `\s` (WhiteSpace and LineTerminator).
fn is_js_space(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

/// X3DParser.sanitizeName.
pub(crate) fn sanitize_name(name: &str) -> String {
    // NonIdFirstChar
    let non_id_first = |c: char| matches!(c as u32, 0x30..=0x39 | 0x00..=0x20 | 0x22 | 0x23 | 0x27 | 0x2b | 0x2c | 0x2d | 0x2e | 0x5b | 0x5c | 0x5d | 0x7b | 0x7d | 0x7f);
    let name = name.trim_start_matches(non_id_first);

    // NonIdChars → "-"
    let non_id = |c: char| matches!(c as u32, 0x00..=0x20 | 0x22 | 0x23 | 0x27 | 0x2c | 0x2e | 0x5b | 0x5c | 0x5d | 0x7b | 0x7d | 0x7f);
    let mut out = String::new();
    let mut in_run = false;

    for c in name.chars() {
        if non_id(c) {
            if !in_run {
                out.push('-');
            }
            in_run = true;
        } else {
            out.push(c);
            in_run = false;
        }
    }

    // Spaces: trim, then runs of [\s_-] → "-"
    let trimmed = out.trim_matches(is_js_space);
    let mut out = String::new();
    let mut in_run = false;

    for c in trimmed.chars() {
        if is_js_space(c) || c == '_' || c == '-' {
            if !in_run {
                out.push('-');
            }
            in_run = true;
        } else {
            out.push(c);
            in_run = false;
        }
    }

    out.trim_matches('-').to_string()
}

/// The last path segment of the world URL, percent-decoded (`decodeURIComponent (url .pathname
/// .split ('/') .at (-1))`), or `default` for a `data:` URL.
fn model_title(world_url: &str, default: &str) -> String {
    if world_url.starts_with("data:") {
        return default.into();
    }

    let path = world_url.split(['?', '#']).next().unwrap_or("");
    let last = path.rsplit('/').next().unwrap_or("");

    percent_decode(last)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = vec![];
    let mut i = 0;

    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }

        out.push(b[i]);
        i += 1;
    }

    String::from_utf8_lossy(&out).into_owned()
}

// --- shared lexical grammar -----------------------------------------------------------------

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// A sticky-regex cursor: `input`, `lastIndex`, the last match.
struct Cursor<'a> {
    s: &'a [u8],
    i: usize,
    result: &'a str,
    value: f64,
}

impl<'a> Cursor<'a> {
    fn new(s: &'a str) -> Self {
        Cursor { s: s.as_bytes(), i: 0, result: "", value: 0.0 }
    }

    fn text(&self, a: usize, b: usize) -> &'a str {
        std::str::from_utf8(&self.s[a..b]).unwrap_or("")
    }

    fn take(&mut self, end: usize) -> bool {
        self.result = self.text(self.i, end);
        self.i = end;
        true
    }

    fn run(&mut self, f: impl Fn(u8) -> bool) -> bool {
        let mut e = self.i;

        while e < self.s.len() && f(self.s[e]) {
            e += 1;
        }

        e > self.i && self.take(e)
    }

    /// `/[\x20\n\t\r,]+/y`
    fn whitespaces(&mut self) -> bool {
        self.run(|b| matches!(b, b' ' | b'\n' | b'\t' | b'\r' | b','))
    }

    /// `/[\x20\t]+/y`
    fn whitespaces_no_line_terminator(&mut self) -> bool {
        self.run(|b| matches!(b, b' ' | b'\t'))
    }

    /// `/[^\r\n]+/y`
    fn until_end_of_line(&mut self) -> bool {
        self.run(|b| !matches!(b, b'\r' | b'\n'))
    }

    /// `/<c>[^\r\n]*(?=[\r\n]|$)/y` — a comment to the end of the line.
    fn line_comment(&mut self, c: u8) -> bool {
        if self.s.get(self.i) != Some(&c) {
            return false;
        }

        let mut e = self.i + 1;

        while e < self.s.len() && !matches!(self.s[e], b'\r' | b'\n') {
            e += 1;
        }

        self.take(e)
    }

    fn boundary(&self, at: usize) -> bool {
        let before = at > 0 && is_word(self.s[at - 1]);
        let after = at < self.s.len() && is_word(self.s[at]);

        before != after
    }

    /// `/\bword\b/y`
    fn keyword(&mut self, word: &str) -> bool {
        let e = self.i + word.len();

        self.s[self.i..].starts_with(word.as_bytes()) && self.boundary(self.i) && self.boundary(e) && self.take(e)
    }

    /// `/word/y` (no boundaries)
    fn literal(&mut self, word: &str) -> bool {
        self.s[self.i..].starts_with(word.as_bytes()) && self.take(self.i + word.len())
    }

    /// `/\b\S+\b/y`
    fn bounded_non_space(&mut self) -> bool {
        if !self.boundary(self.i) {
            return false;
        }

        let mut e = self.i;

        while e < self.s.len() && !self.s[e].is_ascii_whitespace() {
            e += 1;
        }

        while e > self.i && !self.boundary(e) {
            e -= 1;
        }

        e > self.i && self.take(e)
    }

    /// `/\w+/y`
    fn name(&mut self) -> bool {
        self.run(is_word)
    }

    /// `/[+-]?(?:(?:(?:\d*\.\d+)|(?:\d+(?:\.)?))(?:[eE][+-]?\d+)?)/y` (no `0x` exclusion).
    fn double_token(&mut self) -> bool {
        let s = &self.s[self.i..];

        let end = if s.len() >= 2 && s[0] == b'0' && (s[1] == b'x' || s[1] == b'X') {
            Some(self.i + 1)
        } else {
            lexer::double(self.s, self.i)
        };

        match end {
            Some(e) => self.take(e),
            None => false,
        }
    }

    /// `/([+-])((?:NAN|INF|INFINITY))/iy`: the sign is required, and `INF` wins over `INFINITY`.
    fn sign_constant(&mut self) -> Option<f64> {
        let s = &self.s[self.i..];
        let negative = match s.first() {
            Some(b'+') => false,
            Some(b'-') => true,
            _ => return None,
        };

        let word = s.get(1..4)?;
        let v = if word.eq_ignore_ascii_case(b"NAN") {
            f64::NAN
        } else if word.eq_ignore_ascii_case(b"INF") {
            f64::INFINITY
        } else {
            return None;
        };

        self.take(self.i + 4);
        Some(if negative { -v } else { v })
    }

    /// `/(?:0[xX][\da-fA-F]+)|(?:[+-]?\d+)/y` and `parseInt`.
    fn int32_token(&mut self) -> bool {
        match lexer::int32(self.s, self.i) {
            Some(e) => {
                self.take(e);
                self.value = js_parse_int(self.result);
                true
            }
            None => false,
        }
    }

    /// The parsers' `double ()`: a double token (parseFloat) or a signed constant.
    fn number(&mut self) -> bool {
        if self.double_token() {
            self.value = lexer::js_parse_float(self.result);
            return true;
        }

        if let Some(v) = self.sign_constant() {
            self.value = v;
            return true;
        }

        false
    }
}

// --- STL ------------------------------------------------------------------------------------

/// STLAParser.isValid: `/^(?:[\x20\n\t\r]|;[^\r\n]*[\r\n])*\bsolid\b.*?\bendsolid\b/s`.
pub(crate) fn is_stl_ascii(input: &str) -> bool {
    let mut c = Cursor::new(input);

    loop {
        if matches!(c.s.get(c.i), Some(b' ' | b'\n' | b'\t' | b'\r')) {
            c.i += 1;
            continue;
        }

        if c.s.get(c.i) == Some(&b';') {
            let mut e = c.i + 1;
            while e < c.s.len() && !matches!(c.s[e], b'\r' | b'\n') {
                e += 1;
            }
            if e < c.s.len() {
                c.i = e + 1;
                continue;
            }
        }

        break;
    }

    if !c.keyword("solid") {
        return false;
    }

    // .*?\bendsolid\b
    let mut k = c.i;

    while k + 8 <= c.s.len() {
        if c.s[k..].starts_with(b"endsolid") && c.boundary(k) && c.boundary(k + 8) {
            return true;
        }
        k += 1;
    }

    false
}

fn stl_material(scene: &mut Scene) -> R<NodeId> {
    let material = create(scene, "Material")?;
    let appearance = create(scene, "Appearance")?;

    set(scene, material, "diffuseColor", FieldValue::SFColor(x3d_fields::math::Color3::new(1.0, 1.0, 1.0)));
    set_node(scene, appearance, "material", Some(material));
    Ok(appearance)
}

struct Stla<'a> {
    c: Cursor<'a>,
    normals: Option<Vec<f64>>,
}

impl Stla<'_> {
    fn comments(&mut self) {
        loop {
            self.c.whitespaces();

            if !self.c.line_comment(b';') {
                break;
            }
        }
    }

    fn double(&mut self) -> bool {
        self.c.whitespaces_no_line_terminator();
        self.c.number()
    }

    fn vec3(&mut self, into: &mut Vec<f64>) -> R<()> {
        for _ in 0..3 {
            if !self.double() {
                return error("Expected a double.");
            }
            into.push(self.c.value);
        }

        Ok(())
    }

    fn solid(&mut self, scene: &mut Scene, appearance: NodeId) -> R<bool> {
        self.comments();

        if !self.c.keyword("solid") {
            return Ok(false);
        }

        self.c.whitespaces_no_line_terminator();

        let shape = create(scene, "Shape")?;
        let geometry = create(scene, "TriangleSet")?;
        let coordinate = create(scene, "Coordinate")?;
        // `this .normals` still holds the previous solid's normals here.
        let has_normals = self.normals.as_ref().is_some_and(|n| n.iter().any(|v| *v != 0.0));
        let name = sanitize_name(if self.c.name() { self.c.result } else { "" });

        self.c.until_end_of_line();

        let mut normals = vec![];
        let mut vertices = vec![];

        while self.facet(&mut normals, &mut vertices)? {}

        rotate_axes_90(&mut vertices);

        set_node(scene, shape, "appearance", Some(appearance));
        set_node(scene, shape, "geometry", Some(geometry));
        set_numbers(scene, coordinate, "point", &vertices);
        set(scene, geometry, "normalPerVertex", FieldValue::SFBool(false));
        set_node(scene, geometry, "coord", Some(coordinate));

        if has_normals {
            let normal = create(scene, "Normal")?;

            rotate_axes_90(&mut normals);
            set_numbers(scene, normal, "vector", &normals);
            set_node(scene, geometry, "normal", Some(normal));
        }

        self.normals = Some(normals);

        if !name.is_empty() {
            name_and_export(scene, &name, shape);
        }

        root_push(scene, shape);
        self.comments();

        if self.c.keyword("endsolid") {
            return Ok(true);
        }

        error("Expected 'endsolid' statement.")
    }

    fn facet(&mut self, normals: &mut Vec<f64>, vertices: &mut Vec<f64>) -> R<bool> {
        self.comments();

        if !self.c.keyword("facet") {
            return Ok(false);
        }

        // normal
        self.c.whitespaces_no_line_terminator();

        if !self.c.keyword("normal") {
            return error("Expected 'normal' statement.");
        }

        self.vec3(normals)?;

        // outer loop
        self.comments();

        if !self.c.keyword("outer") {
            return error("Expected 'outer' statement.");
        }

        self.c.whitespaces_no_line_terminator();

        if !self.c.keyword("loop") {
            return error("Expected 'loop' statement.");
        }

        for _ in 0..3 {
            self.comments();

            if !self.c.keyword("vertex") {
                return error("Expected 'vertex' statement.");
            }

            self.vec3(vertices)?;
        }

        self.comments();

        if !self.c.keyword("endloop") {
            return error("Expected 'endloop' statement.");
        }

        self.comments();

        if self.c.keyword("endfacet") {
            return Ok(true);
        }

        error("Expected 'endfacet' statement.")
    }
}

pub(crate) fn parse_stl_ascii(input: &str, world_url: &str) -> R<Scene> {
    let mut scene = begin(world_url, "STL");
    let appearance = stl_material(&mut scene)?;
    let mut p = Stla { c: Cursor::new(input), normals: None };

    while p.solid(&mut scene, appearance)? {}

    Ok(scene)
}

/// STLBParser.isValid: 84-byte header and count, 50 bytes per facet, nothing more or less.
pub(crate) fn is_stl_binary(data: &[u8]) -> bool {
    data.len() >= 84 && (u32::from_le_bytes(data[80..84].try_into().unwrap()) as u64) * 50 + 84 == data.len() as u64
}

pub(crate) fn parse_stl_binary(data: &[u8], world_url: &str) -> R<Scene> {
    let mut scene = begin(world_url, "STL");
    let appearance = stl_material(&mut scene)?;

    // header: the text before the first NUL, trimmed.
    let header = crate::decode_text(&data[..80]);
    let header = header.split('\0').next().unwrap_or("").trim_matches(is_js_space).to_string();

    if !header.is_empty() {
        let world_info = create(&mut scene, "WorldInfo")?;

        set(&mut scene, world_info, "title", FieldValue::SFString(model_title(world_url, "STL Model")));
        set(&mut scene, world_info, "info", FieldValue::MFString(vec![header]));
        root_push(&mut scene, world_info);
    }

    let shape = create(&mut scene, "Shape")?;
    let geometry = create(&mut scene, "TriangleSet")?;
    let coordinate = create(&mut scene, "Coordinate")?;
    let f32_at = |i: usize| f32::from_le_bytes(data[i..i + 4].try_into().unwrap()) as f64;
    let mut normals = vec![];
    let mut points = vec![];

    for i in (84..data.len()).step_by(50) {
        for f in 0..3 {
            normals.push(f32_at(i + f * 4));
        }

        for f in 3..12 {
            points.push(f32_at(i + f * 4));
        }
    }

    rotate_axes_90(&mut points);

    set_node(&mut scene, shape, "appearance", Some(appearance));
    set_node(&mut scene, shape, "geometry", Some(geometry));
    set_numbers(&mut scene, coordinate, "point", &points);
    set(&mut scene, geometry, "normalPerVertex", FieldValue::SFBool(false));
    set_node(&mut scene, geometry, "coord", Some(coordinate));

    if normals.iter().any(|v| *v != 0.0) {
        let normal = create(&mut scene, "Normal")?;

        rotate_axes_90(&mut normals);
        set_numbers(&mut scene, normal, "vector", &normals);
        set_node(&mut scene, geometry, "normal", Some(normal));
    }

    root_push(&mut scene, shape);
    Ok(scene)
}

// --- PLY ------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum PlyType {
    Char,
    UChar,
    Short,
    UShort,
    Int,
    UInt,
    Float,
    Double,
}

impl PlyType {
    /// The `propertyType` alternatives and their typeMapping entries.
    fn from_name(name: &str) -> Option<(PlyType, &'static str)> {
        use PlyType::*;

        Some(match name {
            "char" => (Char, "char"),
            "uchar" => (UChar, "uchar"),
            "short" => (Short, "short"),
            "ushort" => (UShort, "ushort"),
            "int" => (Int, "int"),
            "uint" => (UInt, "uint"),
            "float" => (Float, "float"),
            "double" => (Double, "double"),
            "int8" => (Char, "int8"),
            "uint8" => (UChar, "uint8"),
            "int16" => (Short, "int16"),
            "uint16" => (UShort, "uint16"),
            "int32" => (Int, "int32"),
            "uint32" => (UInt, "uint32"),
            "float32" => (Float, "float32"),
            "float64" => (Double, "float64"),
            _ => return None,
        })
    }

    fn bytes(self) -> usize {
        use PlyType::*;

        match self {
            Char | UChar => 1,
            Short | UShort => 2,
            Int | UInt | Float => 4,
            Double => 8,
        }
    }

    fn is_float(self) -> bool {
        matches!(self, PlyType::Float | PlyType::Double)
    }
}

#[derive(Clone, Debug)]
struct PlyProperty {
    count: Option<PlyType>,
    value: PlyType,
    /// The type name as written (convertColor switches on it).
    type_name: &'static str,
    name: String,
}

#[derive(Clone, Debug)]
struct PlyElement {
    name: String,
    count: f64,
    properties: Vec<PlyProperty>,
}

/// PLYAParser.convertColor.
fn convert_color(value: f64, type_name: &str) -> f64 {
    match type_name {
        "uchar" | "uint8" => value / 255.0,
        // X_ITE divides 16-bit colors by 0xfffff.
        "ushort" | "uint16" => value / 1048575.0,
        "uint" | "uint32" => value / 4294967295.0,
        "float" | "float32" | "double" | "float64" => value,
        // No case: the function returns undefined.
        _ => f64::NAN,
    }
}

struct Ply<'a> {
    c: Cursor<'a>,
    /// PLYBParser: the binary data, the byte offset, the byte order.
    binary: Option<(&'a [u8], usize, bool)>,
    comments: Vec<String>,
    must_rotate_axes: bool,
    triangles: bool,
    alpha: bool,
    colors: Vec<f64>,
    tex_coords: Vec<f64>,
    normals: Vec<f64>,
    points: Vec<f64>,
    coord_index: Option<Vec<f64>>,
    tex_coord_index: Option<Vec<f64>>,
    splats: Option<Splats>,
}

struct Splats {
    quaternions: Vec<f64>,
    scales: Vec<f64>,
    opacities: Vec<f64>,
    sh: Vec<Vec<f64>>,
}

impl<'a> Ply<'a> {
    fn whitespaces_or_comments(&mut self) {
        loop {
            if self.c.whitespaces() {
                continue;
            }

            if self.comment() {
                continue;
            }

            break;
        }
    }

    fn comment(&mut self) -> bool {
        if self.c.keyword("comment") && self.c.until_end_of_line() {
            let value = self.c.result.trim_matches(is_js_space).to_string();

            if !value.is_empty() {
                self.comments.push(value.clone());
            }

            let lower = value.to_ascii_lowercase();
            let bounded = |w: &str| {
                let b = lower.as_bytes();
                let mut k = 0;
                while let Some(p) = lower[k..].find(w) {
                    let s = k + p;
                    let e = s + w.len();
                    if (s == 0 || !is_word(b[s - 1])) && (e == b.len() || !is_word(b[e])) {
                        return true;
                    }
                    k = s + 1;
                }
                false
            };

            self.must_rotate_axes |= ["blender", "artec", "polycam"].iter().any(|w| bounded(w));
            return true;
        }

        false
    }

    fn header(&mut self, scene: &mut Scene, world_url: &str) -> R<Vec<PlyElement>> {
        let mut elements = vec![];

        self.c.literal("ply");
        self.c.whitespaces();
        self.c.literal("format ascii 1.0");

        // headings
        loop {
            if self.element(&mut elements) {
                continue;
            }

            if self.c.keyword("end_header") {
                break;
            }

            if self.c.until_end_of_line() {
                continue;
            }

            break;
        }

        let world_info = create(scene, "WorldInfo")?;

        set(scene, world_info, "title", FieldValue::SFString(model_title(world_url, "PLY Model")));
        set(scene, world_info, "info", FieldValue::MFString(self.comments.clone()));
        root_push(scene, world_info);

        Ok(elements)
    }

    fn element(&mut self, elements: &mut Vec<PlyElement>) -> bool {
        self.whitespaces_or_comments();

        if !self.c.keyword("element") {
            return false;
        }

        self.c.whitespaces_no_line_terminator();

        if !self.c.bounded_non_space() {
            return false;
        }

        let name = self.c.result.to_string();

        if !self.ascii_int32() {
            return false;
        }

        let mut element = PlyElement { name, count: self.c.value, properties: vec![] };

        while let Some(p) = self.property() {
            element.properties.push(p);
        }

        elements.push(element);
        true
    }

    fn property_type(&mut self) -> Option<(PlyType, &'static str)> {
        let start = self.c.i;

        if !self.c.boundary(start) {
            return None;
        }

        let mut e = start;
        while e < self.c.s.len() && is_word(self.c.s[e]) {
            e += 1;
        }

        let t = PlyType::from_name(self.c.text(start, e))?;
        self.c.take(e);
        Some(t)
    }

    fn property(&mut self) -> Option<PlyProperty> {
        self.whitespaces_or_comments();

        if !self.c.keyword("property") {
            return None;
        }

        self.c.whitespaces_no_line_terminator();

        if let Some((value, type_name)) = self.property_type() {
            self.c.whitespaces_no_line_terminator();

            if self.c.bounded_non_space() {
                return Some(PlyProperty { count: None, value, type_name, name: self.c.result.to_string() });
            }
        }

        if self.c.keyword("list") {
            self.c.whitespaces_no_line_terminator();

            if let Some((count, _)) = self.property_type() {
                self.c.whitespaces_no_line_terminator();

                if let Some((value, type_name)) = self.property_type() {
                    self.c.whitespaces_no_line_terminator();

                    if self.c.bounded_non_space() {
                        return Some(PlyProperty { count: Some(count), value, type_name, name: self.c.result.to_string() });
                    }
                }
            }
        }

        None
    }

    fn ascii_int32(&mut self) -> bool {
        self.c.whitespaces_no_line_terminator();
        self.c.int32_token()
    }

    fn ascii_double(&mut self) -> bool {
        self.c.whitespaces_no_line_terminator();

        if self.c.double_token() {
            self.c.value = lexer::js_parse_float(self.c.result);
            return true;
        }

        false
    }

    /// `value .call (this)` for a property type: ASCII int32 / double, or a DataView read.
    fn read(&mut self, t: PlyType) -> R<bool> {
        let Some((data, offset, little)) = self.binary else {
            return Ok(if t.is_float() { self.ascii_double() } else { self.ascii_int32() });
        };

        let n = t.bytes();

        if offset + n > data.len() {
            return error("Offset is outside the bounds of the DataView");
        }

        let b = &data[offset..offset + n];
        let get = |b: &[u8]| -> [u8; 8] {
            let mut a = [0u8; 8];
            if little {
                a[..b.len()].copy_from_slice(b);
            } else {
                for (k, x) in b.iter().rev().enumerate() {
                    a[k] = *x;
                }
            }
            a
        };
        let a = get(b);

        self.c.value = match t {
            PlyType::Char => a[0] as i8 as f64,
            PlyType::UChar => a[0] as f64,
            PlyType::Short => i16::from_le_bytes([a[0], a[1]]) as f64,
            PlyType::UShort => u16::from_le_bytes([a[0], a[1]]) as f64,
            PlyType::Int => i32::from_le_bytes([a[0], a[1], a[2], a[3]]) as f64,
            PlyType::UInt => u32::from_le_bytes([a[0], a[1], a[2], a[3]]) as f64,
            PlyType::Float => f32::from_le_bytes([a[0], a[1], a[2], a[3]]) as f64,
            PlyType::Double => f64::from_le_bytes(a),
        };

        self.binary = Some((data, offset + n, little));
        Ok(true)
    }

    fn whitespaces(&mut self) {
        if self.binary.is_none() {
            self.c.whitespaces();
        }
    }

    fn parse_vertices(&mut self, element: &PlyElement) -> R<()> {
        let (mut colors, mut tex_coords, mut normals, mut points) = (vec![], vec![], vec![], vec![]);
        let (mut scales, mut quaternions, mut opacities, mut sh0) = (vec![], vec![], vec![], vec![]);
        let mut rest: Vec<Vec<f64>> = vec![vec![]; 45];
        let mut i = 0.0;

        while i < element.count {
            self.whitespaces();

            for p in &element.properties {
                if !self.read(p.value)? {
                    return error(format!("Couldn't parse value for property {}.", p.name));
                }

                let v = self.c.value;

                match p.name.as_str() {
                    "red" | "green" | "blue" | "alpha" | "r" | "g" | "b" | "a" => colors.push(convert_color(v, p.type_name)),
                    "s" | "t" | "u" | "v" => tex_coords.push(v),
                    "nx" | "ny" | "nz" => normals.push(v),
                    "x" | "y" | "z" => points.push(v),
                    "rot_0" | "rot_1" | "rot_2" | "rot_3" => quaternions.push(v),
                    "scale_0" | "scale_1" | "scale_2" => scales.push(jsmath::exp(v)),
                    "opacity" => opacities.push(1.0 / (1.0 + jsmath::exp(-v))),
                    "f_dc_0" | "f_dc_1" | "f_dc_2" => sh0.push(v),
                    name => {
                        if let Some(k) = name.strip_prefix("f_rest_").and_then(|k| k.parse::<usize>().ok()).filter(|k| *k < 45 && name == format!("f_rest_{k}")) {
                            rest[k].push(v);
                        }
                    }
                }
            }

            i += 1.0;
        }

        self.alpha = element.properties.iter().any(|p| p.name == "alpha" || p.name == "a");
        self.colors = colors;
        self.tex_coords = tex_coords;
        self.normals = normals;
        self.points = points;

        if !sh0.is_empty() {
            let num_splats = self.points.len() / 3;
            let degree = if !rest[44].is_empty() {
                3
            } else if !rest[23].is_empty() {
                2
            } else if !rest[8].is_empty() {
                1
            } else {
                0
            };
            let d1 = (degree + 1) * (degree + 1) - 1;
            let mut shs: Vec<Vec<f64>> = vec![vec![]; 15];

            for d in 0..d1 {
                for s in 0..num_splats {
                    let at = |k: usize| rest[k].get(s).copied().unwrap_or(f64::NAN);
                    shs[d].push(at(d));
                    shs[d].push(at(d1 + d));
                    shs[d].push(at(d1 * 2 + d));
                }
            }

            shs.insert(0, sh0);
            self.splats = Some(Splats { quaternions, scales, opacities, sh: shs });
        }

        Ok(())
    }

    fn parse_faces(&mut self, element: &PlyElement) -> R<()> {
        let mut coord_index = vec![];
        let mut i = 0.0;

        while i < element.count {
            self.whitespaces();

            for p in &element.properties {
                let Some(count) = p.count else {
                    if !self.read(p.value)? {
                        return error(format!("Couldn't parse a property value for {}.", p.name));
                    }
                    continue;
                };

                if !self.read(count)? {
                    return error(format!("Couldn't parse property count for {}.", p.name));
                }

                let length = self.c.value;
                self.triangles &= length == 3.0;

                let mut k = 0.0;
                while k < length {
                    if !self.read(p.value)? {
                        return error(format!("Couldn't parse a property value for {}.", p.name));
                    }
                    coord_index.push(self.c.value);
                    k += 1.0;
                }

                coord_index.push(-1.0);
            }

            i += 1.0;
        }

        self.coord_index = Some(coord_index);
        Ok(())
    }

    fn parse_multi_texture_vertices(&mut self, element: &PlyElement) -> R<()> {
        let mut tex_coords = vec![];
        let mut i = 0.0;

        while i < element.count {
            self.whitespaces();

            for p in &element.properties {
                if !self.read(p.value)? {
                    return error(format!("Couldn't parse value for property {}.", p.name));
                }

                if matches!(p.name.as_str(), "s" | "t" | "u" | "v") {
                    tex_coords.push(self.c.value);
                }
            }

            i += 1.0;
        }

        self.tex_coords = tex_coords;
        Ok(())
    }

    fn parse_multi_texture_faces(&mut self, element: &PlyElement) -> R<()> {
        let mut tex_coord_index = vec![];
        let mut i = 0.0;

        // No whitespaces () before each face, as in X_ITE.
        while i < element.count {
            for p in &element.properties {
                match p.count {
                    Some(count) => {
                        if !self.read(count)? {
                            return error(format!("Couldn't parse property count for {}.", p.name));
                        }

                        let length = self.c.value;
                        let mut k = 0.0;

                        while k < length {
                            if !self.read(p.value)? {
                                return error(format!("Couldn't parse value for property {}.", p.name));
                            }
                            tex_coord_index.push(self.c.value);
                            k += 1.0;
                        }

                        tex_coord_index.push(-1.0);
                    }
                    None => {
                        if !self.read(p.value)? {
                            return error(format!("Couldn't parse value for property {}.", p.name));
                        }
                    }
                }
            }

            i += 1.0;
        }

        self.tex_coord_index = Some(tex_coord_index);
        Ok(())
    }

    fn parse_unknown(&mut self, element: &PlyElement) -> R<()> {
        match self.binary {
            None => {
                self.c.whitespaces();

                let mut i = 0.0;
                while i < element.count {
                    // /[^\r\n]*\r?\n/y
                    let mut e = self.c.i;
                    while e < self.c.s.len() && !matches!(self.c.s[e], b'\r' | b'\n') {
                        e += 1;
                    }
                    if self.c.s.get(e) == Some(&b'\r') && self.c.s.get(e + 1) == Some(&b'\n') {
                        self.c.take(e + 2);
                    } else if self.c.s.get(e) == Some(&b'\n') {
                        self.c.take(e + 1);
                    }
                    i += 1.0;
                }
            }
            Some(_) => {
                let mut i = 0.0;
                while i < element.count {
                    for p in &element.properties {
                        match p.count {
                            Some(count) => {
                                self.read(count)?;
                                let length = self.c.value;
                                let (data, offset, little) = self.binary.unwrap();
                                self.binary = Some((data, (offset as f64 + length * p.value.bytes() as f64) as usize, little));
                            }
                            None => {
                                let (data, offset, little) = self.binary.unwrap();
                                self.binary = Some((data, offset + p.value.bytes(), little));
                            }
                        }
                    }
                    i += 1.0;
                }
            }
        }

        Ok(())
    }

    fn create_color(&self, scene: &mut Scene) -> R<Option<NodeId>> {
        if self.colors.is_empty() {
            return Ok(None);
        }

        let alpha = self.alpha && self.colors.iter().enumerate().any(|(i, v)| i % 4 == 3 && *v < 1.0);
        let color = create(scene, if alpha { "ColorRGBA" } else { "Color" })?;
        let values: Vec<f64> = if alpha || !self.alpha {
            self.colors.clone()
        } else {
            self.colors.iter().enumerate().filter(|(i, _)| i % 4 != 3).map(|(_, v)| *v).collect()
        };

        set_numbers(scene, color, "color", &values);
        Ok(Some(color))
    }

    fn process_elements(&mut self, scene: &mut Scene, elements: &[PlyElement]) -> R<()> {
        for element in elements {
            match element.name.as_str() {
                "vertex" => self.parse_vertices(element)?,
                "face" => self.parse_faces(element)?,
                "multi_texture_vertex" => self.parse_multi_texture_vertices(element)?,
                "multi_texture_face" => self.parse_multi_texture_faces(element)?,
                _ => self.parse_unknown(element)?,
            }
        }

        if self.points.is_empty() {
            return Ok(());
        }

        if let Some(splats) = self.splats.take() {
            if let Some(level) = scene.supported_component_level("GaussianSplats") {
                scene.update_component("GaussianSplats", level);
            }

            let transform = create(scene, "Transform")?;
            let gs = create(scene, "GaussianSplats")?;
            let mut orientations = vec![];

            // wxyz → xyzw
            for q in splats.quaternions.chunks(4) {
                let at = |k: usize| q.get(k).copied().unwrap_or(f64::NAN);
                orientations.extend([at(1), at(2), at(3), at(0)]);
            }

            set_numbers(scene, gs, "positions", &self.points);
            set_numbers(scene, gs, "orientations", &orientations);
            set_numbers(scene, gs, "scales", &splats.scales);
            set_numbers(scene, gs, "opacities", &splats.opacities);

            let mut i = 0;

            for (degree, coefs) in [1, 3, 5, 7].into_iter().enumerate() {
                for coef in 0..coefs {
                    set_numbers(scene, gs, &format!("sphericalHarmonicsDegree{degree}Coef{coef}"), &splats.sh[i]);
                    i += 1;
                }
            }

            set(scene, transform, "rotation", FieldValue::SFRotation(x3d_fields::math::Rotation4::new(1.0, 0.0, 0.0, std::f64::consts::PI)));
            push_child(scene, transform, "children", gs);
            root_push(scene, transform);
            name_and_export(scene, "GaussianSplats", gs);
            return Ok(());
        }

        let has_normals = self.normals.iter().any(|v| *v != 0.0);
        let shape = create(scene, "Shape")?;
        let appearance = create(scene, "Appearance")?;

        let geometry;

        if let Some(coord_index) = self.coord_index.clone() {
            let tex_index_len = self.tex_coord_index.as_ref().map(|t| t.len()).unwrap_or(0);
            let triangles = self.triangles && tex_index_len == 0;
            let material = create(scene, "Material")?;

            geometry = create(scene, if triangles { "IndexedTriangleSet" } else { "IndexedFaceSet" })?;

            if triangles {
                let index: Vec<f64> = coord_index.iter().copied().filter(|v| *v != -1.0).collect();
                set_numbers(scene, geometry, "index", &index);
            } else {
                set_numbers(scene, geometry, "coordIndex", &coord_index);
            }

            let color = self.create_color(scene)?;
            set_node(scene, geometry, "color", color);

            if !self.tex_coords.is_empty() {
                let texture_transform = create(scene, "TextureTransform")?;

                set(scene, texture_transform, "translation", FieldValue::SFVec2f(x3d_fields::math::Vector2::new(0.0, -1.0)));
                set(scene, texture_transform, "scale", FieldValue::SFVec2f(x3d_fields::math::Vector2::new(1.0, -1.0)));
                set_node(scene, appearance, "textureTransform", Some(texture_transform));

                let tex_coord = create(scene, "TextureCoordinate")?;

                set_numbers(scene, tex_coord, "point", &self.tex_coords);
                set_node(scene, geometry, "texCoord", Some(tex_coord));

                if tex_index_len != 0 {
                    let t = self.tex_coord_index.clone().unwrap();
                    set_numbers(scene, geometry, "texCoordIndex", &t);
                }
            }

            self.finish_shape(scene, has_normals, geometry)?;
            set_node(scene, appearance, "material", Some(material));
        } else {
            let material = create(scene, if has_normals { "Material" } else { "UnlitMaterial" })?;

            geometry = create(scene, "PointSet")?;

            let color = self.create_color(scene)?;
            set_node(scene, geometry, "color", color);

            self.finish_shape(scene, has_normals, geometry)?;
            set_node(scene, appearance, "material", Some(material));
        }

        set_node(scene, shape, "appearance", Some(appearance));
        set_node(scene, shape, "geometry", Some(geometry));
        root_push(scene, shape);
        name_and_export(scene, "Shape", shape);
        Ok(())
    }

    fn finish_shape(&mut self, scene: &mut Scene, has_normals: bool, geometry: NodeId) -> R<()> {
        if has_normals {
            let normal = create(scene, "Normal")?;

            if self.must_rotate_axes {
                rotate_axes_90(&mut self.normals);
            }

            set_numbers(scene, normal, "vector", &self.normals);
            set_node(scene, geometry, "normal", Some(normal));
        }

        if self.must_rotate_axes {
            rotate_axes_90(&mut self.points);
        }

        let coordinate = create(scene, "Coordinate")?;

        set_numbers(scene, coordinate, "point", &self.points);
        set_node(scene, geometry, "coord", Some(coordinate));
        Ok(())
    }
}

/// PLYAParser.isValid: `/^ply\r?\nformat ascii 1.0/`.
pub(crate) fn is_ply_ascii(input: &str) -> bool {
    let rest = input.strip_prefix("ply").and_then(|r| r.strip_prefix("\r\n").or_else(|| r.strip_prefix('\n')));

    // `1.0`: the `.` is any character but a line terminator.
    rest.and_then(|r| r.strip_prefix("format ascii 1")).is_some_and(|r| {
        let mut c = r.chars();
        c.next().is_some_and(|d| !matches!(d, '\n' | '\r' | '\u{2028}' | '\u{2029}')) && c.next() == Some('0')
    })
}

fn new_ply<'a>(c: Cursor<'a>, binary: Option<(&'a [u8], usize, bool)>) -> Ply<'a> {
    Ply {
        c,
        binary,
        comments: vec![],
        must_rotate_axes: false,
        triangles: true,
        alpha: false,
        colors: vec![],
        tex_coords: vec![],
        normals: vec![],
        points: vec![],
        coord_index: None,
        tex_coord_index: None,
        splats: None,
    }
}

pub(crate) fn parse_ply_ascii(input: &str, world_url: &str) -> R<Scene> {
    let mut scene = begin(world_url, "PLY");
    let mut p = new_ply(Cursor::new(input), None);
    let elements = p.header(&mut scene, world_url)?;

    p.process_elements(&mut scene, &elements)?;
    Ok(scene)
}

/// PLYBParser.setInput: the header regex on the first 4096 bytes decoded as text,
/// `/^ply\r?\nformat (binary_(?:little|big)_endian) 1.0.*?end_header\r?\n/s`. Returns the header
/// text, the byte offset (the match length in UTF-16 code units, as X_ITE counts it) and the
/// byte order.
fn ply_binary_magic(data: &[u8]) -> Option<(String, usize, bool)> {
    let text = crate::decode_text(&data[..data.len().min(4096)]);
    let rest = text.strip_prefix("ply")?;
    let rest = rest.strip_prefix("\r\n").or_else(|| rest.strip_prefix('\n'))?;
    let rest = rest.strip_prefix("format binary_")?;

    let little = if rest.starts_with("little_endian 1") {
        true
    } else if rest.starts_with("big_endian 1") {
        false
    } else {
        return None;
    };

    // `1.0`: the `.` matches any character.
    let after = &rest[if little { "little_endian 1".len() } else { "big_endian 1".len() }..];
    let mut chars = after.chars();
    chars.next()?;

    if chars.next()? != '0' {
        return None;
    }

    let after = chars.as_str();
    let k = after.find("end_header")?;
    let tail = &after[k + "end_header".len()..];
    let nl = if tail.starts_with("\r\n") {
        2
    } else if tail.starts_with('\n') {
        1
    } else {
        // `.*?` could find a later end_header followed by a line break.
        let mut from = k + 1;
        loop {
            let k2 = after[from..].find("end_header")? + from;
            let t = &after[k2 + 10..];
            if t.starts_with("\r\n") || t.starts_with('\n') {
                let n = if t.starts_with("\r\n") { 2 } else { 1 };
                let end = text.len() - after.len() + k2 + 10 + n;
                let header = text[..end].to_string();
                let units = header.encode_utf16().count();
                return Some((header, units, little));
            }
            from = k2 + 1;
        }
    };

    let end = text.len() - after.len() + k + "end_header".len() + nl;
    let header = text[..end].to_string();
    let units = header.encode_utf16().count();

    Some((header, units, little))
}

pub(crate) fn is_ply_binary(data: &[u8]) -> bool {
    ply_binary_magic(data).is_some()
}

pub(crate) fn parse_ply_binary(data: &[u8], world_url: &str) -> R<Scene> {
    let (_, offset, little) = ply_binary_magic(data).unwrap();
    let text = crate::decode_text(&data[..data.len().min(4096)]);
    let mut scene = begin(world_url, "PLY");
    let mut p = new_ply(Cursor::new(&text), Some((data, offset, little)));
    let elements = p.header(&mut scene, world_url)?;

    p.process_elements(&mut scene, &elements)?;
    Ok(scene)
}

// --- OBJ ------------------------------------------------------------------------------------

/// OBJParser.isValid: `/^(?:[\x20\n\t\r]|#[^\r\n]*[\r\n])*\b(?:mtllib|usemtl|o|g|s|vt|vn|v|f)\b/`.
pub(crate) fn is_obj(input: &str) -> bool {
    let mut c = Cursor::new(input);

    loop {
        if matches!(c.s.get(c.i), Some(b' ' | b'\n' | b'\t' | b'\r')) {
            c.i += 1;
            continue;
        }

        if c.s.get(c.i) == Some(&b'#') {
            let mut e = c.i + 1;
            while e < c.s.len() && !matches!(c.s[e], b'\r' | b'\n') {
                e += 1;
            }
            if e < c.s.len() {
                c.i = e + 1;
                continue;
            }
        }

        break;
    }

    ["mtllib", "usemtl", "o", "g", "s", "vt", "vn", "v", "f"].iter().any(|k| {
        let mut t = Cursor { s: c.s, i: c.i, result: "", value: 0.0 };
        t.keyword(k)
    })
}

struct GeometryIndices {
    tex_coord_index: Vec<f64>,
    normal_index: Vec<f64>,
    coord_index: Vec<f64>,
}

struct Obj<'a> {
    c: Cursor<'a>,
    world_url: String,
    object: NodeId,
    group: NodeId,
    material: NodeId,
    color: NodeId,
    tex_coord: NodeId,
    normal: NodeId,
    coord: NodeId,
    colors: Vec<f64>,
    tex_coords: Vec<f64>,
    normals: Vec<f64>,
    vertices: Vec<f64>,
    /// In creation order, like the JS Map.
    geometry_indices: Vec<(NodeId, GeometryIndices)>,
    num_geometries: usize,
    smoothing_group: f64,
    smoothing_groups: HashMap<(u64, NodeId, NodeId), NodeId>,
    groups: HashMap<String, NodeId>,
    /// `None` is the `undefined` key of the default material.
    materials: HashMap<Option<String>, NodeId>,
}

impl Obj<'_> {
    fn comments(&mut self) {
        loop {
            self.c.whitespaces();

            if !self.c.line_comment(b'#') {
                break;
            }
        }
    }

    fn int32(&mut self) -> bool {
        self.c.whitespaces();
        self.c.int32_token()
    }

    fn double(&mut self) -> bool {
        self.c.whitespaces();
        self.c.number()
    }

    fn vec(&mut self, n: usize, into: &mut Vec<f64>) -> bool {
        let mut v = vec![];

        for _ in 0..n {
            if !self.double() {
                return false;
            }
            v.push(self.c.value);
        }

        into.extend(v);
        true
    }

    fn statement(&mut self, scene: &mut Scene) -> R<bool> {
        if self.mtllibs(scene) {
            return Ok(true);
        }
        if self.usemtl(scene)? {
            return Ok(true);
        }
        if self.o(scene)? {
            return Ok(true);
        }
        if self.g(scene)? {
            return Ok(true);
        }
        if self.s() {
            return Ok(true);
        }
        if self.vts()? {
            return Ok(true);
        }
        if self.vns()? {
            return Ok(true);
        }
        if self.vs()? {
            return Ok(true);
        }
        if self.fs(scene)? {
            return Ok(true);
        }

        // Skip empty and unknown lines.
        Ok(self.c.until_end_of_line())
    }

    fn mtllibs(&mut self, scene: &mut Scene) -> bool {
        self.comments();

        if !self.c.keyword("mtllib") {
            return false;
        }

        self.c.whitespaces_no_line_terminator();

        if self.c.until_end_of_line() {
            let paths: Vec<String> = self.c.result.trim_matches(is_js_space).split(is_js_space).filter(|p| !p.is_empty()).map(String::from).collect();

            for path in paths {
                self.mtllib(scene, &path);
            }
        }

        true
    }

    fn mtllib(&mut self, scene: &mut Scene, path: &str) {
        match resolve_local(&self.world_url, path).and_then(|p| std::fs::read(&p).map_err(|e| e.to_string())) {
            Ok(data) => {
                let input = crate::decode_text(&crate::gunzip(&data));

                match parse_mtl(scene, &input) {
                    Ok(materials) => {
                        for (id, m) in materials {
                            self.materials.insert(Some(id), m);
                        }
                    }
                    Err(e) => scene.warn(e),
                }
            }
            Err(e) => scene.warn(format!("TypeError: Failed to fetch {path}: {e}")),
        }
    }

    fn usemtl(&mut self, scene: &mut Scene) -> R<bool> {
        self.comments();

        if !self.c.keyword("usemtl") {
            return Ok(false);
        }

        self.c.whitespaces_no_line_terminator();

        if self.c.until_end_of_line() {
            let id = self.c.result.to_string();
            let material = match self.materials.get(&Some(id.clone())) {
                Some(m) => *m,
                None => {
                    let m = create(scene, "Material")?;
                    self.materials.insert(Some(id.clone()), m);
                    m
                }
            };
            let name = sanitize_name(&id);

            if !name.is_empty() && scene.node(material).name.is_empty() {
                name_and_export(scene, &name, material);
            }

            self.material = material;
        }

        Ok(true)
    }

    fn o(&mut self, scene: &mut Scene) -> R<bool> {
        self.comments();

        if !self.c.keyword("o") {
            return Ok(false);
        }

        self.c.whitespaces_no_line_terminator();

        if self.c.until_end_of_line() {
            let name = sanitize_name(self.c.result);

            if !node_field(scene, self.group, "children").is_empty() {
                self.object = create(scene, "Group")?;
                self.group = create(scene, "Group")?;
                push_child(scene, self.object, "children", self.group);
                root_push(scene, self.object);
            }

            if !name.is_empty() && scene.node(self.object).name.is_empty() {
                name_and_export(scene, &name, self.object);
            }
        }

        Ok(true)
    }

    fn g(&mut self, scene: &mut Scene) -> R<bool> {
        self.comments();

        if !self.c.keyword("g") {
            return Ok(false);
        }

        self.c.whitespaces_no_line_terminator();

        if self.c.until_end_of_line() {
            let id = self.c.result.to_string();
            let name = sanitize_name(&id);

            match self.groups.get(&id) {
                Some(g) => self.group = *g,
                None => {
                    if !node_field(scene, self.group, "children").is_empty() {
                        self.group = create(scene, "Group")?;
                        push_child(scene, self.object, "children", self.group);
                    }
                }
            }

            self.groups.insert(id, self.group);

            if !name.is_empty() && scene.node(self.group).name.is_empty() {
                name_and_export(scene, &name, self.group);
            }

            self.smoothing_group = -1.0;
        }

        Ok(true)
    }

    fn s(&mut self) -> bool {
        self.comments();

        if !self.c.keyword("s") {
            return false;
        }

        self.c.whitespaces_no_line_terminator();

        if self.c.keyword("off") {
            self.smoothing_group = 0.0;
            return true;
        }

        if self.int32() {
            self.smoothing_group = self.c.value;
        }

        true
    }

    fn vts(&mut self) -> R<bool> {
        let mut result = false;

        loop {
            self.comments();

            if !self.c.keyword("vt") {
                return Ok(result);
            }

            let mut t = std::mem::take(&mut self.tex_coords);
            let ok = self.vec(2, &mut t);
            self.tex_coords = t;

            if !ok {
                return error("Expected a texture coordinate.");
            }

            result = true;
        }
    }

    fn vns(&mut self) -> R<bool> {
        let mut result = false;

        loop {
            self.comments();

            if !self.c.keyword("vn") {
                return Ok(result);
            }

            let mut n = std::mem::take(&mut self.normals);
            let ok = self.vec(3, &mut n);
            self.normals = n;

            if !ok {
                return error("Expected a normal vector.");
            }

            result = true;
        }
    }

    fn vs(&mut self) -> R<bool> {
        let mut result = false;

        loop {
            self.comments();

            if !self.c.keyword("v") {
                return Ok(result);
            }

            let mut v = std::mem::take(&mut self.vertices);
            let ok = self.vec(3, &mut v);
            self.vertices = v;

            if !ok {
                return error("Expected a vertex coordinate.");
            }

            self.c.whitespaces_no_line_terminator();

            let mut colors = std::mem::take(&mut self.colors);

            if self.vec(3, &mut colors) && self.double() {
                // RGBA
                colors.push(self.c.value);
            }

            self.colors = colors;
            result = true;
        }
    }

    fn smoothing_group_id(&self) -> (u64, NodeId, NodeId) {
        (self.smoothing_group.to_bits(), self.group, self.material)
    }

    fn fs(&mut self, scene: &mut Scene) -> R<bool> {
        self.comments();

        // Grammar .f .lookahead
        let mut look = Cursor { s: self.c.s, i: self.c.i, result: "", value: 0.0 };

        if !look.keyword("f") {
            return Ok(false);
        }

        let key = self.smoothing_group_id();
        let geometry = match self.smoothing_groups.get(&key) {
            Some(&shape) => match node_field(scene, shape, "geometry") {
                FieldValue::SFNode(Some(g)) => g,
                _ => unreachable!(),
            },
            None => {
                let appearance = create(scene, "Appearance")?;
                let shape = create(scene, "Shape")?;
                let geometry = create(scene, "IndexedFaceSet")?;

                add_named(scene, &format!("Geometry-{}", self.num_geometries), geometry);
                self.num_geometries += 1;

                self.geometry_indices.push((geometry, GeometryIndices { tex_coord_index: vec![], normal_index: vec![], coord_index: vec![] }));

                set_node(scene, appearance, "material", Some(self.material));
                set(scene, geometry, "creaseAngle", FieldValue::SFFloat(if self.smoothing_group != 0.0 && !self.smoothing_group.is_nan() { std::f64::consts::PI } else { 0.0 }));
                set_node(scene, shape, "appearance", Some(appearance));
                set_node(scene, shape, "geometry", Some(geometry));
                push_child(scene, self.group, "children", shape);
                self.smoothing_groups.insert(self.smoothing_group_id(), shape);
                geometry
            }
        };

        let gi = self.geometry_indices.iter().position(|(g, _)| *g == geometry).unwrap();

        while self.f(gi)? {}

        if !self.colors.is_empty() {
            set_node(scene, geometry, "color", Some(self.color));
        }

        let indices = &self.geometry_indices[gi].1;

        if !indices.tex_coord_index.is_empty() {
            set_node(scene, geometry, "texCoord", Some(self.tex_coord));
        }

        if !indices.normal_index.is_empty() {
            set_node(scene, geometry, "normal", Some(self.normal));
        }

        set_node(scene, geometry, "coord", Some(self.coord));
        Ok(true)
    }

    fn f(&mut self, gi: usize) -> R<bool> {
        self.comments();

        if !self.c.keyword("f") {
            return Ok(false);
        }

        let (num_tex_coords, num_normals, num_coords) = (self.tex_coords.len() as f64, self.normals.len() as f64, self.vertices.len() as f64);
        let (nt, nn) = (self.geometry_indices[gi].1.tex_coord_index.len(), self.geometry_indices[gi].1.normal_index.len());

        let index = |index: f64, length: f64| -> R<f64> {
            if index == 0.0 {
                return error("Invalid index.");
            }

            Ok(if index < 0.0 { length + index } else { index - 1.0 })
        };

        while self.int32() {
            let v = index(self.c.value, num_coords)?;
            self.geometry_indices[gi].1.coord_index.push(v);

            if self.c.literal("/") {
                if self.int32() {
                    let v = index(self.c.value, num_tex_coords)?;
                    self.geometry_indices[gi].1.tex_coord_index.push(v);
                }

                if self.c.literal("/") && self.int32() {
                    let v = index(self.c.value, num_normals)?;
                    self.geometry_indices[gi].1.normal_index.push(v);
                }
            }
        }

        let indices = &mut self.geometry_indices[gi].1;

        if indices.tex_coord_index.len() != nt {
            indices.tex_coord_index.push(-1.0);
        }

        if indices.normal_index.len() != nn {
            indices.normal_index.push(-1.0);
        }

        indices.coord_index.push(-1.0);
        Ok(true)
    }
}

/// `new URL (path, scene .getBaseURL ())` for a local world: the file path to read.
fn resolve_local(world_url: &str, path: &str) -> Result<std::path::PathBuf, String> {
    let base = world_url.strip_prefix("file://").ok_or_else(|| format!("cannot fetch relative to '{world_url}'"))?;
    let base = percent_decode(base);
    let dir = std::path::Path::new(&base).parent().unwrap_or(std::path::Path::new("/"));
    let path = percent_decode(path);

    Ok(if path.starts_with('/') { std::path::PathBuf::from(path) } else { dir.join(path) })
}

/// MaterialParser: the materials of an MTL file by id.
fn parse_mtl(scene: &mut Scene, input: &str) -> Result<Vec<(String, NodeId)>, String> {
    let mut c = Cursor::new(input);
    let mut material = create(scene, "Material").map_err(|e| e.to_string())?;
    let mut materials: Vec<(String, NodeId)> = vec![];
    let mut textures: HashMap<String, NodeId> = HashMap::new();
    let mut id = String::new();
    let mut color = [0.0f64; 3];

    let comments = |c: &mut Cursor| loop {
        c.whitespaces();

        if !c.line_comment(b'#') {
            break;
        }
    };

    let double = |c: &mut Cursor| {
        c.whitespaces();
        c.number()
    };

    // Color3 r, g, b setters clamp to [0, 1].
    let col3 = |c: &mut Cursor, color: &mut [f64; 3]| {
        for k in 0..3 {
            if !double(c) {
                return false;
            }
            color[k] = jsmath::clamp(c.value, 0.0, 1.0);
        }
        true
    };

    let set_material = |materials: &mut Vec<(String, NodeId)>, id: &str, m: NodeId| match materials.iter_mut().find(|(k, _)| k == id) {
        Some(e) => e.1 = m,
        None => materials.push((id.to_string(), m)),
    };

    loop {
        // newmtl
        comments(&mut c);

        if c.keyword("newmtl") {
            c.whitespaces_no_line_terminator();
            id.clear();

            if c.until_end_of_line() {
                id = c.result.to_string();
                material = create(scene, "Material").map_err(|e| e.to_string())?;
                set_material(&mut materials, &id, material);
                continue;
            }

            c.until_end_of_line();
            continue;
        }

        let mut handled = false;

        for (k, field) in [("Ka", "ambientIntensity"), ("Kd", "diffuseColor"), ("Ks", "specularColor")] {
            comments(&mut c);

            if c.keyword(k) {
                if col3(&mut c, &mut color) {
                    let value = if field == "ambientIntensity" {
                        // Color3.getHSV: v = max (r, g, b)
                        FieldValue::SFFloat(color[0].max(color[1]).max(color[2]))
                    } else {
                        FieldValue::SFColor(x3d_fields::math::Color3::new(color[0], color[1], color[2]))
                    };
                    set(scene, material, field, value);
                } else {
                    c.until_end_of_line();
                }

                handled = true;
                break;
            }
        }

        if handled {
            continue;
        }

        for k in ["Ns", "d", "Tr"] {
            comments(&mut c);

            if c.keyword(k) {
                if double(&mut c) {
                    let v = c.value;
                    let (field, value) = match k {
                        "Ns" => ("shininess", v / 1000.0),
                        "d" => ("transparency", 1.0 - v),
                        _ => ("transparency", v),
                    };
                    set(scene, material, field, FieldValue::SFFloat(value));
                } else {
                    c.until_end_of_line();
                }

                handled = true;
                break;
            }
        }

        if handled {
            continue;
        }

        comments(&mut c);

        if c.keyword("illum") {
            c.whitespaces();

            if !c.int32_token() {
                c.until_end_of_line();
            }

            continue;
        }

        for (k, field) in [("map_Ka", "ambientTexture"), ("map_Kd", "diffuseTexture"), ("map_Ks", "specularTexture"), ("map_Ns", "shininessTexture")] {
            comments(&mut c);

            if c.keyword(k) {
                c.whitespaces_no_line_terminator();

                if c.until_end_of_line() {
                    let string = c.result.trim_matches(is_js_space).to_string();

                    if !string.is_empty() && !id.is_empty() {
                        let paths: Vec<&str> = string.split(is_js_space).collect();
                        let path = paths.last().unwrap().replace('\\', "/");

                        let texture = match textures.get(&string) {
                            Some(t) => *t,
                            None => {
                                // repeat = !string .match (/-clamp\s+on/)
                                let clamp = string.match_indices("-clamp").any(|(i, _)| {
                                    let rest = &string[i + 6..];
                                    let t = rest.trim_start_matches(is_js_space);
                                    t.len() < rest.len() && t.starts_with("on")
                                });
                                let t = create(scene, "ImageTexture").map_err(|e| e.to_string())?;

                                set(scene, t, "url", FieldValue::MFString(vec![path]));
                                set(scene, t, "repeatS", FieldValue::SFBool(!clamp));
                                set(scene, t, "repeatT", FieldValue::SFBool(!clamp));
                                textures.insert(string, t);
                                t
                            }
                        };

                        set_node(scene, material, field, Some(texture));
                    }
                } else {
                    c.until_end_of_line();
                }

                handled = true;
                break;
            }
        }

        if handled {
            continue;
        }

        // Skip empty and unknown lines.
        if !c.until_end_of_line() {
            break;
        }
    }

    Ok(materials)
}

pub(crate) fn parse_obj(input: &str, world_url: &str) -> R<Scene> {
    let mut scene = begin(world_url, "OBJ");

    let object = create(&mut scene, "Group")?;
    let group = create(&mut scene, "Group")?;
    let material = create(&mut scene, "Material")?;
    let color = create(&mut scene, "Color")?;
    let tex_coord = create(&mut scene, "TextureCoordinate")?;
    let normal = create(&mut scene, "Normal")?;
    let coord = create(&mut scene, "Coordinate")?;

    let mut p = Obj {
        c: Cursor::new(input),
        world_url: world_url.to_string(),
        object,
        group,
        material,
        color,
        tex_coord,
        normal,
        coord,
        colors: vec![],
        tex_coords: vec![],
        normals: vec![],
        vertices: vec![],
        geometry_indices: vec![],
        num_geometries: 0,
        smoothing_group: -1.0,
        smoothing_groups: HashMap::new(),
        groups: HashMap::new(),
        materials: HashMap::from([(None, material)]),
    };

    push_child(&mut scene, object, "children", group);
    root_push(&mut scene, object);

    while p.statement(&mut scene)? {}

    for (geometry, indices) in std::mem::take(&mut p.geometry_indices) {
        set_numbers(&mut scene, geometry, "texCoordIndex", &indices.tex_coord_index);
        set_numbers(&mut scene, geometry, "normalIndex", &indices.normal_index);
        set_numbers(&mut scene, geometry, "coordIndex", &indices.coord_index);
    }

    set_numbers(&mut scene, color, "color", &p.colors);
    set_numbers(&mut scene, tex_coord, "point", &p.tex_coords);
    set_numbers(&mut scene, normal, "vector", &p.normals);
    set_numbers(&mut scene, coord, "point", &p.vertices);

    crate::optimizer::Optimizer { remove_empty_groups: true, ..Default::default() }.optimize_scene_graph(&mut scene, ROOT);
    Ok(scene)
}
