//! GLSL ES 1.00 / 3.00 (what X_ITE's ShaderPart hands to WebGL 2) to GLSL 4.50 that naga's GLSL
//! front end accepts, then naga to WGSL for wgpu (Batch 08, shaders lane).
//!
//! naga 30 accepts only `#version 440/450/460`, requires `layout (location)` on every stage input
//! and output, requires uniforms in a block with a binding, and has no combined samplers. This
//! module rewrites a ShaderPart's source for that, and never changes the line count of the
//! user's text (declarations are replaced in place, the added text is on lines of its own before
//! the user's text), so compile errors name the user's line.
//!
//! Design decisions (see the lane report):
//!
//! * Uniforms. Every loose `uniform T name;` becomes a plain (private) global of the same name and
//!   the value is copied in at the start of a generated `main` from one `vec4 _u [N]` uniform
//!   block. The host writes `N` vec4 slots; it never needs std140 rules. Booleans, scalars,
//!   vectors, arrays and structs of those all work (bool and scalar arrays are not host-shareable
//!   in a block, which is why the copy exists).
//! * Samplers. `uniform sampler2D s;` becomes a `texture2D s_t` and a `sampler s_s` (set 1) and
//!   `texture2D (s, uv)` becomes `texture (sampler2D (s_t, s_s), uv)`.
//! * Stage interface. attribute / varying / in / out get locations: x3d_* attributes by a fixed
//!   table, other attributes after them, varyings by their order in the vertex shader (matched by
//!   name in the fragment shader), `gl_FragColor` becomes output 0.
//! * Clip space. WebGL clip z is -1..1, WebGPU 0..1: the generated `main` of the vertex stage maps
//!   z unless the renderer uses reversed depth, and flips y when drawing into a texture.
//!   `gl_FragCoord.y` counts from the bottom as in WebGL.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    Vertex,
    Fragment,
}

impl Stage {
    pub fn from_type(t: &str) -> Stage {
        // ShaderPart.getShaderType: anything but FRAGMENT that is known is not a fragment shader.
        if t == "FRAGMENT" { Stage::Fragment } else { Stage::Vertex }
    }
}

// ---- types ------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scalar {
    Float,
    Int,
    Uint,
    Bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
    Scalar(Scalar),
    Vec(Scalar, u8),
    /// matN (square only).
    Mat(u8),
    Struct(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Member {
    pub name: String,
    pub ty: Ty,
    pub array: Option<u32>,
}

/// One uniform of the program (its slots are `slot .. slot + slots`).
#[derive(Clone, Debug)]
pub struct UniformDecl {
    pub name: String,
    pub ty: Ty,
    pub array: Option<u32>,
    pub slot: u32,
    pub slots: u32,
}

/// A leaf of a uniform: one scalar, vector or matrix at a path such as `x3d_LightSource[1].color`.
#[derive(Clone, Debug)]
pub struct Leaf {
    pub slot: u32,
    pub ty: Ty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SamplerDim {
    D2,
    D3,
    Cube,
}

/// A texture + sampler pair of the program (bind group 1, bindings `binding` and `binding + 1`).
/// `name` is the uniform name (`x3d_Texture2D` for an array), `index` the array element.
#[derive(Clone, Debug)]
pub struct SamplerDecl {
    pub name: String,
    pub index: Option<u32>,
    pub dim: SamplerDim,
    pub binding: u32,
}

#[derive(Clone, Debug)]
pub struct AttrDecl {
    pub name: String,
    pub location: u32,
    /// float components of the declared type (1..4), or 9 / 16 for a matrix.
    pub components: u32,
}

/// The result of linking a vertex and a fragment part.
#[derive(Clone, Debug)]
pub struct Program {
    pub key: u64,
    pub vs_wgsl: String,
    pub fs_wgsl: String,
    /// vec4 slots of the uniform block (slot 0 is the renderer's: see [`MISC_SLOT`]).
    pub slots: u32,
    pub uniforms: Vec<UniformDecl>,
    pub leaves: HashMap<String, Leaf>,
    pub samplers: Vec<SamplerDecl>,
    pub attributes: Vec<AttrDecl>,
    /// Deprecation warnings X3DProgrammableShaderObject.getDefaultUniformsAndAttributes prints.
    pub warnings: Vec<String>,
}

/// Slot 0 of every program: x = height of the render target, y = 1 with reversed depth,
/// z = y scale of clip space (-1 when drawing into a texture), w = 1 when drawing into a texture.
pub const MISC_SLOT: u32 = 0;

/// Vertex attribute locations of the x3d_ attributes (the layout [`crate::shader_gpu`] builds).
pub const ATTRIBUTE_LOCATIONS: &[(&str, u32)] = &[
    ("x3d_Vertex", 0),
    ("x3d_Normal", 1),
    ("x3d_TexCoord0", 2),
    ("x3d_TexCoord", 2),
    ("x3d_Color", 3),
    ("x3d_Tangent", 4),
    ("x3d_TexCoord1", 5),
    ("x3d_TexCoord2", 6),
    ("x3d_TexCoord3", 7),
    ("x3d_FogDepth", 8),
];

/// First location of an attribute that is not an x3d_ one.
pub const FIRST_CUSTOM_LOCATION: u32 = 9;
pub const MAX_ATTRIBUTE_LOCATIONS: u32 = 16;
pub const MAX_SLOTS: u32 = 4096;

// ---- lexer ------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Ident,
    Number,
    Punct,
}

#[derive(Clone, Copy, Debug)]
struct Tok {
    kind: Kind,
    start: usize,
    end: usize,
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// `s` with every preprocessor line blanked (same byte offsets), for the declaration scan.
fn mask_directives(s: &str) -> String {
    let mut out = String::with_capacity(s.len());

    for (n, line) in s.split('\n').enumerate() {
        if n > 0 {
            out.push('\n');
        }

        if line.trim_start().starts_with('#') {
            out.extend(std::iter::repeat_n(' ', line.len()));
        } else {
            out.push_str(line);
        }
    }

    out
}

fn lex(s: &str) -> Vec<Tok> {
    let b = s.as_bytes();
    let mut out = vec![];
    let mut i = 0;

    while i < b.len() {
        let c = b[i];

        if c.is_ascii_whitespace() {
            i += 1;
        } else if is_ident_start(c) {
            let start = i;

            while i < b.len() && is_ident(b[i]) {
                i += 1;
            }

            out.push(Tok { kind: Kind::Ident, start, end: i });
        } else if c.is_ascii_digit() || (c == b'.' && i + 1 < b.len() && b[i + 1].is_ascii_digit()) {
            let start = i;

            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'.' || ((b[i] == b'+' || b[i] == b'-') && matches!(b[i - 1], b'e' | b'E') && !s[start..i].starts_with("0x"))) {
                i += 1;
            }

            out.push(Tok { kind: Kind::Number, start, end: i });
        } else {
            out.push(Tok { kind: Kind::Punct, start: i, end: i + 1 });
            i += 1;
        }
    }

    out
}

fn text<'a>(s: &'a str, t: &Tok) -> &'a str {
    &s[t.start..t.end]
}

fn newlines(s: &str) -> usize {
    s.bytes().filter(|&c| c == b'\n').count()
}

// ---- comments and conditional compilation -----------------------------------------------------

/// Comments become white space; newlines stay.
fn strip_comments(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;

    while i < b.len() {
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            i += 2;
            out.push(b' ');

            while i < b.len() && !(b[i] == b'*' && i + 1 < b.len() && b[i + 1] == b'/') {
                if b[i] == b'\n' {
                    out.push(b'\n');
                }

                i += 1;
            }

            i += 2;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }

    String::from_utf8_lossy(&out).into_owned()
}

type Defines = HashMap<String, String>;

/// A preprocessor expression (`#if`): integers, defined, macros, the C operators.
struct Expr<'a> {
    toks: Vec<Tok>,
    src: &'a str,
    pos: usize,
    defines: &'a Defines,
    depth: u32,
}

impl Expr<'_> {
    fn peek(&self) -> Option<&str> {
        self.toks.get(self.pos).map(|t| text(self.src, t))
    }

    fn punct2(&self) -> Option<String> {
        let a = self.toks.get(self.pos)?;
        let b = self.toks.get(self.pos + 1)?;

        if b.start == a.end && a.kind == Kind::Punct && b.kind == Kind::Punct {
            Some(format!("{}{}", text(self.src, a), text(self.src, b)))
        } else {
            None
        }
    }

    fn eat_op(&mut self, ops: &[&str]) -> Option<String> {
        if let Some(p) = self.punct2() {
            if ops.contains(&p.as_str()) {
                self.pos += 2;
                return Some(p);
            }
        }

        let p = self.peek()?.to_string();

        // A one character operator must not be the first half of a two character one.
        let two_follows = self.punct2().is_some_and(|q| matches!(q.as_str(), "&&" | "||" | "==" | "!=" | "<=" | ">=" | "<<" | ">>"));

        if ops.contains(&p.as_str()) && !two_follows {
            self.pos += 1;
            return Some(p);
        }

        None
    }

    fn binary(&mut self, level: usize) -> Result<i64, String> {
        const LEVELS: &[&[&str]] = &[&["||"], &["&&"], &["==", "!="], &["<", ">", "<=", ">="], &["+", "-"], &["*", "/", "%"]];

        if level == LEVELS.len() {
            return self.unary();
        }

        let mut v = self.binary(level + 1)?;

        while let Some(op) = self.eat_op(LEVELS[level]) {
            let r = self.binary(level + 1)?;

            v = match op.as_str() {
                "||" => (v != 0 || r != 0) as i64,
                "&&" => (v != 0 && r != 0) as i64,
                "==" => (v == r) as i64,
                "!=" => (v != r) as i64,
                "<" => (v < r) as i64,
                ">" => (v > r) as i64,
                "<=" => (v <= r) as i64,
                ">=" => (v >= r) as i64,
                "+" => v.wrapping_add(r),
                "-" => v.wrapping_sub(r),
                "*" => v.wrapping_mul(r),
                "/" => {
                    if r == 0 {
                        return Err("division by zero".into());
                    }
                    v / r
                }
                _ => {
                    if r == 0 {
                        return Err("modulus by zero".into());
                    }
                    v % r
                }
            };
        }

        Ok(v)
    }

    fn unary(&mut self) -> Result<i64, String> {
        match self.peek() {
            Some("!") => {
                self.pos += 1;
                Ok((self.unary()? == 0) as i64)
            }
            Some("-") => {
                self.pos += 1;
                Ok(self.unary()?.wrapping_neg())
            }
            Some("+") => {
                self.pos += 1;
                self.unary()
            }
            Some("(") => {
                self.pos += 1;
                let v = self.binary(0)?;

                if self.peek() != Some(")") {
                    return Err("missing ')'".into());
                }

                self.pos += 1;
                Ok(v)
            }
            Some("defined") => {
                self.pos += 1;
                let paren = self.peek() == Some("(");

                if paren {
                    self.pos += 1;
                }

                let name = self.peek().ok_or("defined without a name")?.to_string();
                self.pos += 1;

                if paren {
                    if self.peek() != Some(")") {
                        return Err("missing ')'".into());
                    }

                    self.pos += 1;
                }

                Ok(self.defines.contains_key(&name) as i64)
            }
            Some(t) => {
                let t = t.to_string();
                let tok = self.toks[self.pos];
                self.pos += 1;

                if tok.kind == Kind::Number {
                    let digits = t.trim_end_matches(['u', 'U']);
                    return if let Some(h) = digits.strip_prefix("0x").or_else(|| digits.strip_prefix("0X")) { i64::from_str_radix(h, 16).map_err(|e| e.to_string()) } else { digits.parse::<i64>().map_err(|e| e.to_string()) };
                }

                // A macro: evaluate its body; an unknown identifier is 0.
                match self.defines.get(&t) {
                    Some(body) if self.depth < 16 => {
                        let toks = lex(body);
                        let mut e = Expr { toks, src: body, pos: 0, defines: self.defines, depth: self.depth + 1 };
                        if e.toks.is_empty() { Ok(0) } else { e.binary(0) }
                    }
                    _ => Ok(0),
                }
            }
            None => Err("missing operand".into()),
        }
    }
}

fn eval_expr(s: &str, defines: &Defines) -> Result<i64, String> {
    let mut e = Expr { toks: lex(s), src: s, pos: 0, defines, depth: 0 };
    let v = e.binary(0)?;

    if e.pos != e.toks.len() {
        return Err("unexpected token".into());
    }

    Ok(v)
}

/// The conditional directives resolved: inactive lines and the directive lines themselves become
/// empty lines. `#define` / `#undef` are tracked for the conditions and kept. `#version` and
/// `#extension` lines are removed (the version is returned). Returns the text and `#version`.
fn resolve_conditionals(s: &str, defines: &mut Defines) -> Result<(String, Option<u32>), String> {
    // (this branch is active, a branch was taken, parent active)
    let mut stack: Vec<(bool, bool, bool)> = vec![];
    let mut out = String::with_capacity(s.len());
    let mut version = None;

    for (n, line) in s.split('\n').enumerate() {
        if n > 0 {
            out.push('\n');
        }

        let active = stack.last().is_none_or(|e| e.0);
        let trimmed = line.trim_start();

        let Some(rest) = trimmed.strip_prefix('#') else {
            if active {
                out.push_str(line);
            }

            continue;
        };

        let rest = rest.trim_start();
        let word_end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(rest.len());
        let (word, arg) = (&rest[..word_end], rest[word_end..].trim());
        let err = |m: String| format!("ERROR: 0:{}: '#{}' : {}", n + 1, word, m);

        match word {
            "ifdef" | "ifndef" => {
                let name = arg.split_whitespace().next().unwrap_or("");
                let v = defines.contains_key(name) == (word == "ifdef");
                stack.push((active && v, v, active));
            }
            "if" => {
                let v = if active { eval_expr(arg, defines).map_err(|m| err(m))? != 0 } else { false };
                stack.push((active && v, v, active));
            }
            "elif" => {
                let Some(top) = stack.last().copied() else { return Err(err("unexpected #elif".into())) };
                let v = if top.2 && !top.1 { eval_expr(arg, defines).map_err(|m| err(m))? != 0 } else { false };
                let l = stack.last_mut().unwrap();
                *l = (top.2 && !top.1 && v, top.1 || v, top.2);
            }
            "else" => {
                let Some(top) = stack.last_mut() else { return Err(err("unexpected #else".into())) };
                let v = top.2 && !top.1;
                *top = (v, true, top.2);
            }
            "endif" => {
                if stack.pop().is_none() {
                    return Err(err("unexpected #endif".into()));
                }
            }
            "define" if active => {
                let name_end = arg.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(arg.len());
                let name = &arg[..name_end];
                let body = &arg[name_end..];
                // A function-like macro has `(` right after the name: remember the name only.
                defines.insert(name.to_string(), if body.starts_with('(') { String::new() } else { body.trim().to_string() });
                out.push_str(line);
            }
            "undef" if active => {
                defines.remove(arg.split_whitespace().next().unwrap_or(""));
                out.push_str(line);
            }
            "version" if active => version = arg.split_whitespace().next().and_then(|v| v.parse().ok()),
            "extension" if active => {}
            "error" if active => return Err(format!("ERROR: 0:{}: '#error' : {}", n + 1, arg)),
            "line" | "pragma" => {}
            "define" | "undef" | "version" | "extension" | "error" => {}
            _ => {
                if active {
                    out.push_str(line);
                }
            }
        }
    }

    if !stack.is_empty() {
        return Err(format!("ERROR: 0:{}: '#if' : unterminated conditional directive", newlines(s) + 1));
    }

    Ok((out, version))
}

// ---- declarations -----------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
struct StructDef {
    members: Vec<Member>,
}

#[derive(Clone, Debug)]
struct Declarator {
    name: String,
    array: Option<u32>,
}

#[derive(Clone, Debug)]
struct Decl {
    /// Byte range of the whole item in the text, `;` included.
    range: (usize, usize),
    /// `uniform`, `attribute`, `varying`, `in`, `out`.
    storage: String,
    /// `flat`, `smooth`, `centroid`, precision, `invariant`.
    qualifiers: Vec<String>,
    layout: Option<String>,
    type_name: String,
    names: Vec<Declarator>,
}

/// What the scan of a stage found.
struct Scan {
    text: String,
    /// `text` with the preprocessor lines blanked (same offsets).
    masked: String,
    /// Byte ranges of the struct definitions.
    struct_ranges: Vec<(usize, usize)>,
    decls: Vec<Decl>,
    structs: HashMap<String, StructDef>,
    has_main: bool,
    /// Ranges of function bodies are not needed; `main`'s `void` and name tokens are:
    main_rename: Option<(usize, usize)>,
    /// How the declared floats are qualified (for the wrapper's helper declarations).
    es3: bool,
}

fn parse_scalar_ty(name: &str, structs: &HashMap<String, StructDef>) -> Option<Ty> {
    let ty = match name {
        "float" => Ty::Scalar(Scalar::Float),
        "int" => Ty::Scalar(Scalar::Int),
        "uint" => Ty::Scalar(Scalar::Uint),
        "bool" => Ty::Scalar(Scalar::Bool),
        "vec2" => Ty::Vec(Scalar::Float, 2),
        "vec3" => Ty::Vec(Scalar::Float, 3),
        "vec4" => Ty::Vec(Scalar::Float, 4),
        "ivec2" => Ty::Vec(Scalar::Int, 2),
        "ivec3" => Ty::Vec(Scalar::Int, 3),
        "ivec4" => Ty::Vec(Scalar::Int, 4),
        "uvec2" => Ty::Vec(Scalar::Uint, 2),
        "uvec3" => Ty::Vec(Scalar::Uint, 3),
        "uvec4" => Ty::Vec(Scalar::Uint, 4),
        "bvec2" => Ty::Vec(Scalar::Bool, 2),
        "bvec3" => Ty::Vec(Scalar::Bool, 3),
        "bvec4" => Ty::Vec(Scalar::Bool, 4),
        "mat2" | "mat2x2" => Ty::Mat(2),
        "mat3" | "mat3x3" => Ty::Mat(3),
        "mat4" | "mat4x4" => Ty::Mat(4),
        n if structs.contains_key(n) => Ty::Struct(n.to_string()),
        _ => return None,
    };

    Some(ty)
}

fn sampler_dim(name: &str) -> Option<SamplerDim> {
    match name {
        "sampler2D" => Some(SamplerDim::D2),
        "sampler3D" => Some(SamplerDim::D3),
        "samplerCube" => Some(SamplerDim::Cube),
        _ => None,
    }
}

fn is_precision(w: &str) -> bool {
    matches!(w, "lowp" | "mediump" | "highp")
}

/// A constant array size: an integer, or a macro / expression the preprocessor can evaluate.
fn eval_size(expr: &str, defines: &Defines) -> Result<u32, String> {
    let v = eval_expr(expr, defines)?;

    if !(1..=65536).contains(&v) {
        return Err(format!("array size {v} out of range"));
    }

    Ok(v as u32)
}

/// Split the global scope of `text` into items and parse declarations and structs.
fn scan(text: &str, defines: &Defines, es3: bool) -> Result<Scan, String> {
    let masked = mask_directives(text);
    let toks = lex(&masked);
    let text_full = text;
    let text = masked.as_str();
    let line_of = |pos: usize| newlines(&text[..pos]) + 1;
    let mut decls = vec![];
    let mut structs: HashMap<String, StructDef> = HashMap::new();
    let mut has_main = false;
    let mut struct_ranges = vec![];
    let mut main_rename = None;
    let mut i = 0;

    while i < toks.len() {
        // One item: up to `;` at depth 0, or the `}` that closes a function body.
        let first = i;
        let (mut paren, mut brace) = (0i32, 0i32);
        let mut saw_brace = false;
        let mut end = None;

        while i < toks.len() {
            let t = text_of(text, &toks[i]);

            match t {
                "(" => paren += 1,
                ")" => paren -= 1,
                "{" => {
                    brace += 1;
                    saw_brace = true;
                }
                "}" => {
                    brace -= 1;

                    if brace == 0 && paren == 0 {
                        // A function body ends the item; a struct or initializer continues to `;`.
                        let head: Vec<&str> = toks[first..i].iter().map(|t| text_of(text, t)).collect();
                        let is_struct = head.first() == Some(&"struct") || head.contains(&"struct");

                        if !is_struct && head.contains(&"(") && saw_brace {
                            end = Some(i);
                            i += 1;
                            break;
                        }
                    }
                }
                ";" if brace == 0 && paren == 0 => {
                    end = Some(i);
                    i += 1;
                    break;
                }
                _ => {}
            }

            i += 1;
        }

        let Some(last) = end else {
            let t = &toks[first];
            return Err(format!("ERROR: 0:{}: '{}' : syntax error", line_of(t.start), text_of(text, t)));
        };

        let item: Vec<&Tok> = toks[first..=last].iter().collect();
        let words: Vec<&str> = item.iter().map(|t| text_of(text, t)).collect();
        let range = (item[0].start, item[item.len() - 1].end);

        // A function definition.
        if words.last() == Some(&"}") && words.contains(&"(") && words[0] != "struct" {
            if let Some(p) = words.iter().position(|w| *w == "(") {
                if p >= 1 && words[p - 1] == "main" && p >= 2 && words[p - 2] == "void" {
                    has_main = true;
                    main_rename = Some((item[p - 2].start, item[p - 1].end));
                }
            }

            continue;
        }

        if words[0] == "precision" {
            continue;
        }

        // struct S { members };  (also `struct S { } name;`, which this port treats as a type only)
        if words[0] == "struct" && words.len() > 2 && words[2] == "{" {
            let name = words[1].to_string();
            let mut members = vec![];
            let mut j = 3;

            while j < words.len() && words[j] != "}" {
                // [precision] type name [ [N] ] {, name [ [N] ]} ;
                let mut k = j;

                while k < words.len() && is_precision(words[k]) {
                    k += 1;
                }

                let type_name = words.get(k).copied().unwrap_or("");
                let ty = parse_scalar_ty(type_name, &structs).or_else(|| sampler_dim(type_name).map(|_| Ty::Scalar(Scalar::Int)));

                let Some(ty) = ty else {
                    return Err(format!("ERROR: 0:{}: '{}' : unsupported type in struct '{}'", line_of(item[k.min(item.len() - 1)].start), type_name, name));
                };

                if sampler_dim(type_name).is_some() {
                    return Err(format!("ERROR: 0:{}: '{}' : samplers in structs are not supported (struct '{}')", line_of(item[k].start), type_name, name));
                }

                k += 1;

                loop {
                    let member = words.get(k).copied().unwrap_or("").to_string();
                    k += 1;
                    let mut array = None;

                    if words.get(k) == Some(&"[") {
                        let mut e = String::new();
                        k += 1;

                        while k < words.len() && words[k] != "]" {
                            e.push_str(words[k]);
                            e.push(' ');
                            k += 1;
                        }

                        k += 1;
                        array = Some(eval_size(&e, defines).map_err(|m| format!("ERROR: 0:{}: '[' : {m}", line_of(item[0].start)))?);
                    }

                    members.push(Member { name: member, ty: ty.clone(), array });

                    match words.get(k) {
                        Some(&",") => k += 1,
                        _ => break,
                    }
                }

                // the `;`
                j = k + 1;
            }

            structs.insert(name, StructDef { members });
            struct_ranges.push(range);
            continue;
        }

        // A declaration with a storage qualifier.
        let mut storage = None;
        let mut qualifiers = vec![];
        let mut layout = None;
        let mut k = 0;

        while k < words.len() {
            let w = words[k];

            if matches!(w, "attribute" | "varying" | "uniform" | "in" | "out") && storage.is_none() {
                storage = Some(w.to_string());
                k += 1;
            } else if w == "layout" && words.get(k + 1) == Some(&"(") {
                let mut e = String::new();
                k += 2;

                while k < words.len() && words[k] != ")" {
                    e.push_str(words[k]);
                    k += 1;
                }

                k += 1;
                layout = Some(e);
            } else if matches!(w, "flat" | "smooth" | "centroid" | "invariant" | "const" | "inout") || is_precision(w) {
                qualifiers.push(w.to_string());
                k += 1;
            } else {
                break;
            }
        }

        let Some(storage) = storage else { continue };

        let type_name = words.get(k).copied().unwrap_or("").to_string();
        k += 1;
        let mut names = vec![];

        loop {
            let name = words.get(k).copied().unwrap_or("").to_string();

            if name.is_empty() || name == ";" || !name.as_bytes().first().is_some_and(|c| is_ident_start(*c)) {
                return Err(format!("ERROR: 0:{}: '{}' : syntax error", line_of(item[0].start), name));
            }

            k += 1;
            let mut array = None;

            if words.get(k) == Some(&"[") {
                let mut e = String::new();
                k += 1;

                while k < words.len() && words[k] != "]" {
                    e.push_str(words[k]);
                    e.push(' ');
                    k += 1;
                }

                k += 1;
                array = Some(eval_size(&e, defines).map_err(|m| format!("ERROR: 0:{}: '[' : {m}", line_of(item[0].start)))?);
            }

            names.push(Declarator { name, array });

            match words.get(k) {
                Some(&",") => k += 1,
                Some(&";") => break,
                _ => return Err(format!("ERROR: 0:{}: '{}' : syntax error", line_of(item[0].start), words.get(k).copied().unwrap_or(""))),
            }
        }

        decls.push(Decl { range, storage, qualifiers, layout, type_name, names });
    }

    Ok(Scan { text: text_full.to_string(), masked: masked.clone(), decls, structs, struct_ranges, has_main, main_rename, es3 })
}

fn text_of<'a>(s: &'a str, t: &Tok) -> &'a str {
    text(s, t)
}

// ---- types: layout in slots -------------------------------------------------------------------

fn ty_slots(ty: &Ty, structs: &HashMap<String, StructDef>) -> u32 {
    match ty {
        Ty::Scalar(_) | Ty::Vec(..) => 1,
        Ty::Mat(n) => *n as u32,
        Ty::Struct(s) => structs.get(s).map_or(0, |d| d.members.iter().map(|m| ty_slots(&m.ty, structs) * m.array.unwrap_or(1)).sum()),
    }
}

fn glsl_ty_name(ty: &Ty) -> String {
    match ty {
        Ty::Scalar(Scalar::Float) => "float".into(),
        Ty::Scalar(Scalar::Int) => "int".into(),
        Ty::Scalar(Scalar::Uint) => "uint".into(),
        Ty::Scalar(Scalar::Bool) => "bool".into(),
        Ty::Vec(s, n) => format!("{}vec{n}", match s { Scalar::Float => "", Scalar::Int => "i", Scalar::Uint => "u", Scalar::Bool => "b" }),
        Ty::Mat(n) => format!("mat{n}"),
        Ty::Struct(s) => s.clone(),
    }
}

/// The copy statements of one uniform value: `target = <expr from slot>;` for every leaf.
fn emit_copy(out: &mut String, target: &str, ty: &Ty, slot: &mut u32, structs: &HashMap<String, StructDef>, leaves: &mut HashMap<String, Leaf>, copy: bool) {
    match ty {
        Ty::Scalar(s) => {
            let e = format!("_u[{slot}].x");
            let e = match s {
                Scalar::Float => e,
                Scalar::Int => format!("int({e})"),
                Scalar::Uint => format!("uint({e})"),
                Scalar::Bool => format!("({e} != 0.0)"),
            };

            if copy {
                out.push_str(&format!("{target} = {e};"));
            }

            leaves.insert(target.to_string(), Leaf { slot: *slot, ty: ty.clone() });
            *slot += 1;
        }
        Ty::Vec(s, n) => {
            let sw = &"xyzw"[..*n as usize];
            let e = format!("_u[{slot}].{sw}");
            let e = match s {
                Scalar::Float => e,
                Scalar::Int => format!("ivec{n}({e})"),
                Scalar::Uint => format!("uvec{n}({e})"),
                Scalar::Bool => format!("notEqual({e}, vec{n}(0.0))"),
            };

            if copy {
                out.push_str(&format!("{target} = {e};"));
            }

            leaves.insert(target.to_string(), Leaf { slot: *slot, ty: ty.clone() });
            *slot += 1;
        }
        Ty::Mat(n) => {
            let sw = &"xyzw"[..*n as usize];
            let cols: Vec<String> = (0..*n as u32).map(|c| format!("_u[{}].{sw}", *slot + c)).collect();

            if copy {
                out.push_str(&format!("{target} = mat{n}({});", cols.join(", ")));
            }

            leaves.insert(target.to_string(), Leaf { slot: *slot, ty: ty.clone() });
            *slot += *n as u32;
        }
        Ty::Struct(name) => {
            if let Some(def) = structs.get(name) {
                for m in &def.members {
                    match m.array {
                        Some(len) => {
                            for k in 0..len {
                                emit_copy(out, &format!("{target}.{}[{k}]", m.name), &m.ty, slot, structs, leaves, copy);
                            }
                        }
                        None => emit_copy(out, &format!("{target}.{}", m.name), &m.ty, slot, structs, leaves, copy),
                    }
                }
            }
        }
    }
}

// ---- analysis of one stage --------------------------------------------------------------------

struct Analysis {
    stage: Stage,
    scan: Scan,
    /// Number of lines before the user's first line in the text handed to naga (set by emit).
    uniforms: Vec<(String, Ty, Option<u32>)>,
    samplers: Vec<(String, Option<u32>, SamplerDim)>,
    attributes: Vec<(String, String, Option<String>)>,
    /// (name, type name, qualifiers, array)
    varyings: Vec<(String, String, Vec<String>, Option<u32>)>,
    outputs: Vec<(String, String, Vec<String>)>,
    /// Lines of the preamble (types) before the user's text.
    prefix_lines: usize,
    es3: bool,
    /// Names the code uses (a uniform the code never reads is inactive: WebGL has no location for it).
    used: Used,
}

/// Identifier uses in the user's code outside declarations.
#[derive(Default, Clone)]
struct Used {
    idents: std::collections::HashSet<String>,
    /// Names after a `.`.
    members: std::collections::HashSet<String>,
    /// Identifiers used other than as `name.member` / `name[..].member`.
    whole: std::collections::HashSet<String>,
}

fn used_names(code: &str, skip: &[(usize, usize)]) -> Used {
    // Declarations are blanked: their names are not uses.
    let mut masked = code.to_string();

    for (a, b) in skip {
        if *a < masked.len() && *b <= masked.len() {
            let blank: String = masked[*a..*b].chars().map(|c| if c == '\n' { '\n' } else { ' ' }).collect();
            masked.replace_range(*a..*b, &blank);
        }
    }

    let toks = lex(&masked);
    let mut u = Used::default();

    for (i, t) in toks.iter().enumerate() {
        if t.kind != Kind::Ident {
            continue;
        }

        let name = text(&masked, t).to_string();

        if i > 0 && toks[i - 1].kind == Kind::Punct && text(&masked, &toks[i - 1]) == "." {
            u.members.insert(name);
            continue;
        }

        // name [ ... ] . member, or name . member
        let mut j = i + 1;

        if j < toks.len() && text(&masked, &toks[j]) == "[" {
            let mut depth = 0;

            while j < toks.len() {
                match text(&masked, &toks[j]) {
                    "[" => depth += 1,
                    "]" => {
                        depth -= 1;

                        if depth == 0 {
                            j += 1;
                            break;
                        }
                    }
                    _ => {}
                }

                j += 1;
            }
        }

        if !(j < toks.len() && text(&masked, &toks[j]) == ".") {
            u.whole.insert(name.clone());
        }

        u.idents.insert(name);
    }

    u
}

/// X_ITE's `ShaderSource.getSource`: the defines every ShaderPart sees.
fn base_defines(stage: Stage, es3: bool, options: &[String]) -> Defines {
    let mut d = Defines::new();
    d.insert("GL_ES".into(), "1".into());
    d.insert("__VERSION__".into(), if es3 { "300" } else { "100" }.into());
    d.insert("X_ITE".into(), String::new());
    d.insert(if stage == Stage::Vertex { "X3D_VERTEX_SHADER" } else { "X3D_FRAGMENT_SHADER" }.into(), String::new());

    for o in options {
        d.insert(o.clone(), String::new());
    }

    d.insert("x3d_MaxLights".into(), "8".into());
    d
}

/// ShaderPart's `customOptions`.
pub const CUSTOM_OPTIONS: &[&str] = &[
    "X3D_GEOMETRY_0D",
    "X3D_GEOMETRY_1D",
    "X3D_GEOMETRY_2D",
    "X3D_GEOMETRY_3D",
    "X3D_FOG",
    "X3D_STYLE_PROPERTIES",
    "X3D_UNLIT_MATERIAL",
    "X3D_PHONG_MATERIAL",
    "X3D_LIGHTING",
    "X3D_TEXTURE",
    "X3D_MULTI_TEXTURING",
];

/// The part of assets/shaders/Types.glsl.js a custom shader sees (the options above are defined,
/// X3D_SHADOWS, X3D_USE_IBL and the *_TEXTURE ones are not).
const TYPES: &str = include_str!("glsl_types.glsl");

fn analyze(stage: Stage, source: &str, options: &[String]) -> Result<Analysis, String> {
    let es3 = {
        // `#version 300 es` after comments.
        let stripped = strip_comments(source);
        stripped.lines().any(|l| {
            let t = l.trim_start();
            t.starts_with("#version") && t.split_whitespace().nth(1).is_some_and(|v| v == "300")
        })
    };
    let mut defines = base_defines(stage, es3, options);
    // The injected types come first: their constants are macros the user's conditions can see.
    let (types, _) = resolve_conditionals(&strip_comments(TYPES), &mut defines)?;
    let (user, _version) = resolve_conditionals(&strip_comments(source), &mut defines)?;
    let user = mask_sampler_precision(&user);
    let prefix_lines = newlines(&types) + 1;
    let combined = format!("{types}\n{user}");

    let s = scan(&combined, &defines, es3).map_err(|e| shift_error(&e, prefix_lines))?;
    let boundary_at = types.len() + 1;
    let skip: Vec<(usize, usize)> = s.decls.iter().filter(|d| d.range.0 >= boundary_at).map(|d| (d.range.0 - boundary_at, d.range.1 - boundary_at)).collect();
    let used = used_names(&s.text[boundary_at.min(s.text.len())..], &skip);
    let mut a = Analysis { stage, scan: s, uniforms: vec![], samplers: vec![], attributes: vec![], varyings: vec![], outputs: vec![], prefix_lines, es3, used };

    // Only the user's declarations count: those after the injected types.
    let boundary = types.len() + 1;
    let decls = a.scan.decls.clone();

    for d in decls.iter().filter(|d| d.range.0 >= boundary) {
        let line = newlines(&a.scan.text[..d.range.0]) + 1 - prefix_lines;
        let err = |m: String| format!("ERROR: 0:{line}: '{}' : {m}", d.type_name);

        match d.storage.as_str() {
            "uniform" => {
                if let Some(dim) = sampler_dim(&d.type_name) {
                    for n in &d.names {
                        a.samplers.push((n.name.clone(), n.array, dim));
                    }
                } else {
                    let Some(ty) = parse_scalar_ty(&d.type_name, &a.scan.structs) else {
                        return Err(err(format!("unsupported uniform type '{}'", d.type_name)));
                    };

                    for n in &d.names {
                        a.uniforms.push((n.name.clone(), ty.clone(), n.array));
                    }
                }
            }
            "attribute" | "in" if stage == Stage::Vertex => {
                for n in &d.names {
                    a.attributes.push((n.name.clone(), d.type_name.clone(), d.layout.clone()));
                }
            }
            "varying" | "in" if stage == Stage::Fragment => {
                for n in &d.names {
                    a.varyings.push((n.name.clone(), d.type_name.clone(), d.qualifiers.clone(), n.array));
                }
            }
            "varying" | "out" if stage == Stage::Vertex => {
                for n in &d.names {
                    a.varyings.push((n.name.clone(), d.type_name.clone(), d.qualifiers.clone(), n.array));
                }
            }
            "out" if stage == Stage::Fragment => {
                for n in &d.names {
                    a.outputs.push((n.name.clone(), d.type_name.clone(), d.qualifiers.clone()));
                }
            }
            "attribute" | "varying" => {
                // ANGLE: `'attribute' :  supported in vertex shaders only` (two spaces).
                return Err(format!("ERROR: 0:{line}: '{}' :  supported in vertex shaders only", d.storage));
            }
            _ => {}
        }
    }

    if !a.scan.has_main {
        return Err("ERROR: -1:-1: '' : Missing main()".into());
    }

    Ok(a)
}

fn shift_error(e: &str, prefix_lines: usize) -> String {
    // "ERROR: 0:N: …" with N counted in the combined text.
    if let Some(rest) = e.strip_prefix("ERROR: 0:") {
        if let Some((n, tail)) = rest.split_once(':') {
            if let Ok(n) = n.parse::<usize>() {
                return format!("ERROR: 0:{}:{tail}", n.saturating_sub(prefix_lines));
            }
        }
    }

    e.to_string()
}

// ---- link and emit ----------------------------------------------------------------------------

fn find_word(s: &str, word: &str) -> bool {
    lex(s).iter().any(|t| t.kind == Kind::Ident && text(s, t) == word)
}

/// The rewritten text of one stage.
struct Emitted {
    text: String,
    /// Lines in front of the user's first line (naga's line - this = the user's line).
    line_base: usize,
}

struct LinkInfo<'a> {
    slots: u32,
    samplers: &'a [SamplerDecl],
    sampler_elem: &'a HashMap<(String, Option<u32>), usize>,
    attr_locations: &'a HashMap<String, u32>,
    varying_locations: &'a HashMap<String, u32>,
    structs: &'a HashMap<String, StructDef>,
    leaves: &'a HashMap<String, Leaf>,
    uniforms: &'a [UniformDecl],
}

const BUILTIN_SAMPLING: &[(&str, &str)] = &[
    ("texture2D", "texture"),
    ("texture3D", "texture"),
    ("textureCube", "texture"),
    ("texture", "texture"),
    ("texture2DProj", "textureProj"),
    ("texture3DProj", "textureProj"),
    ("textureProj", "textureProj"),
    ("texture2DLod", "textureLod"),
    ("textureCubeLod", "textureLod"),
    ("textureLod", "textureLod"),
    ("textureGrad", "textureGrad"),
    ("textureOffset", "textureOffset"),
    ("textureProjLod", "textureProjLod"),
];

const BUILTIN_FETCH: &[&str] = &["texelFetch", "textureSize", "textureQueryLevels"];

fn emit(a: &Analysis, info: &LinkInfo) -> Result<Emitted, String> {
    let text = &a.scan.text;
    let boundary_line = a.prefix_lines;
    let line_of = |pos: usize| newlines(&text[..pos]) + 1 - boundary_line.min(newlines(&text[..pos]) + 1);
    let mut edits: Vec<(usize, usize, String)> = vec![];

    // Declarations.
    for d in &a.scan.decls {
        let nl = text[d.range.0..d.range.1].matches('\n').count();
        let pad = "\n".repeat(nl);
        let is_user = d.range.0 > 0;
        let _ = is_user;

        match d.storage.as_str() {
            "uniform" => {
                if sampler_dim(&d.type_name).is_some() {
                    // Declared in the preamble.
                    edits.push((d.range.0, d.range.1, pad));
                } else {
                    let quals: String = d.qualifiers.iter().filter(|q| is_precision(q)).map(|q| format!("{q} ")).collect();
                    let decls: Vec<String> = d
                        .names
                        .iter()
                        .map(|n| match n.array {
                            Some(len) => format!("{quals}{} {}[{len}];", d.type_name, n.name),
                            None => format!("{quals}{} {};", d.type_name, n.name),
                        })
                        .collect();
                    edits.push((d.range.0, d.range.1, format!("{}{pad}", decls.join(" "))));
                }
            }
            "attribute" | "in" if a.stage == Stage::Vertex => {
                let mut parts = vec![];

                for n in &d.names {
                    let loc = info.attr_locations.get(&n.name).copied().unwrap_or(0);

                    // A matrix attribute is one vector input per column (a stage input cannot be a matrix);
                    // the wrapper main assembles it.
                    if let Some((cols, col_ty)) = matrix_columns(&d.type_name) {
                        for k in 0..cols {
                            parts.push(format!("layout(location={}) in {col_ty} _x3d_a_{}_{k};", loc + k, n.name));
                        }

                        parts.push(format!("{} {};", d.type_name, n.name));
                        continue;
                    }

                    parts.push(format!("layout(location={loc}) in {} {};", d.type_name, n.name));
                }

                edits.push((d.range.0, d.range.1, format!("{}{pad}", parts.join(" "))));
            }
            "varying" | "out" | "in" if a.stage == Stage::Vertex && d.storage != "in" => {
                let mut parts = vec![];

                for n in &d.names {
                    let loc = info.varying_locations.get(&n.name).copied().unwrap_or(0);
                    let q: String = d.qualifiers.iter().filter(|q| matches!(q.as_str(), "flat" | "smooth" | "centroid")).map(|q| format!("{q} ")).collect();
                    let arr = n.array.map(|l| format!("[{l}]")).unwrap_or_default();
                    parts.push(format!("layout(location={loc}) {q}out {} {}{arr};", d.type_name, n.name));
                }

                edits.push((d.range.0, d.range.1, format!("{}{pad}", parts.join(" "))));
            }
            "varying" | "in" if a.stage == Stage::Fragment => {
                let mut parts = vec![];

                for n in &d.names {
                    let Some(loc) = info.varying_locations.get(&n.name).copied() else {
                        return Err(format!("ERROR: 0:{}: '{}' : fragment input has no matching output in the vertex shader", line_of(d.range.0), n.name));
                    };
                    let q: String = d.qualifiers.iter().filter(|q| matches!(q.as_str(), "flat" | "smooth" | "centroid")).map(|q| format!("{q} ")).collect();
                    let arr = n.array.map(|l| format!("[{l}]")).unwrap_or_default();
                    parts.push(format!("layout(location={loc}) {q}in {} {}{arr};", d.type_name, n.name));
                }

                edits.push((d.range.0, d.range.1, format!("{}{pad}", parts.join(" "))));
            }
            "out" if a.stage == Stage::Fragment => {
                let mut parts = vec![];

                for (k, n) in d.names.iter().enumerate() {
                    let loc = d.layout.as_deref().and_then(|l| l.split(',').find_map(|p| p.trim().strip_prefix("location=").and_then(|v| v.trim().parse::<u32>().ok()))).unwrap_or(k as u32);
                    parts.push(format!("layout(location={loc}) out {} {};", d.type_name, n.name));
                }

                edits.push((d.range.0, d.range.1, format!("{}{pad}", parts.join(" "))));
            }
            _ => {}
        }
    }

    // main -> _x3d_main
    if let Some((s, e)) = a.scan.main_rename {
        edits.push((s, e, "void _x3d_main".to_string()));
    }

    // naga does not take precision qualifiers on struct members.
    let masked = &a.scan.masked;
    let toks = lex(masked);

    for t in &toks {
        if t.kind == Kind::Ident && is_precision(text_of(masked, t)) && a.scan.struct_ranges.iter().any(|r| t.start >= r.0 && t.start < r.1) {
            edits.push((t.start, t.end, " ".repeat(t.end - t.start)));
        }
    }

    // Identifier level rewrites outside declarations.
    let in_decl = |pos: usize| a.scan.decls.iter().any(|d| pos >= d.range.0 && pos < d.range.1);
    let sampler_names: HashSet<&str> = info.samplers.iter().map(|s| s.name.as_str()).collect();
    let mut used_frag_output = false;
    let mut i = 0;

    while i < toks.len() {
        let t = toks[i];

        if t.kind != Kind::Ident || in_decl(t.start) {
            i += 1;
            continue;
        }

        let word = text_of(masked, &t);
        let user_line = || line_of(t.start);

        match word {
            // GLSL ES 1.00 / 3.00 `const in` parameter (read-only); naga takes `in` and `const` alone.
            "const" if toks.get(i + 1).is_some_and(|x| x.kind == Kind::Ident && text_of(masked, x) == "in") => {
                edits.push((t.start, toks[i + 1].end, "in".into()));
                i += 2;
                continue;
            }
            "gl_FragColor" if a.stage == Stage::Fragment => {
                edits.push((t.start, t.end, "_x3d_FragColor".into()));
                used_frag_output = true;
            }
            "gl_FragData" if a.stage == Stage::Fragment => {
                // gl_FragData [ 0 ]
                let ok = toks.get(i + 1).map(|x| text_of(text, x)) == Some("[") && toks.get(i + 2).map(|x| text_of(text, x)) == Some("0") && toks.get(i + 3).map(|x| text_of(text, x)) == Some("]");

                if !ok {
                    return Err(format!("ERROR: 0:{}: 'gl_FragData' : only gl_FragData [0] is supported", user_line()));
                }

                edits.push((t.start, toks[i + 3].end, "_x3d_FragColor".into()));
                used_frag_output = true;
                i += 4;
                continue;
            }
            "gl_FragCoord" if a.stage == Stage::Fragment => edits.push((t.start, t.end, "_x3d_FragCoord()".into())),
            "gl_PointSize" if a.stage == Stage::Vertex => edits.push((t.start, t.end, "_x3d_PointSize".into())),
            w if BUILTIN_SAMPLING.iter().any(|(n, _)| *n == w) || BUILTIN_FETCH.contains(&w) => {
                // name ( sampler , …
                if toks.get(i + 1).map(|x| text_of(text, x)) == Some("(") {
                    if let Some(arg) = toks.get(i + 2).filter(|x| x.kind == Kind::Ident) {
                        let sname = text_of(text, arg);

                        if sampler_names.contains(sname) {
                            // optional [ literal ]
                            let mut index = None;
                            let mut last = i + 2;

                            if toks.get(i + 3).map(|x| text_of(text, x)) == Some("[") {
                                let lit = toks.get(i + 4).filter(|x| x.kind == Kind::Number).and_then(|x| text_of(text, x).parse::<u32>().ok());

                                if lit.is_none() || toks.get(i + 5).map(|x| text_of(text, x)) != Some("]") {
                                    return Err(format!("ERROR: 0:{}: '{sname}' : an array of samplers must be indexed with a constant", user_line()));
                                }

                                index = lit;
                                last = i + 5;
                            }

                            let Some(&e) = info.sampler_elem.get(&(sname.to_string(), index)) else {
                                return Err(format!("ERROR: 0:{}: '{sname}' : sampler index out of range", user_line()));
                            };
                            let decl = &info.samplers[e];
                            let base = format!("_x3d_t{}", decl.binding);
                            let call = if BUILTIN_FETCH.contains(&w) {
                                format!("{w} ({base}")
                            } else {
                                let target = BUILTIN_SAMPLING.iter().find(|(n, _)| *n == w).unwrap().1;
                                let ctor = match decl.dim {
                                    SamplerDim::D2 => "sampler2D",
                                    SamplerDim::D3 => "sampler3D",
                                    SamplerDim::Cube => "samplerCube",
                                };
                                format!("{target} ({ctor} ({base}, _x3d_s{}) ", decl.binding)
                            };
                            edits.push((t.start, toks[last].end, call));
                            i = last + 1;
                            continue;
                        }
                    }
                }
            }
            w if sampler_names.contains(w) => {
                return Err(format!("ERROR: 0:{}: '{w}' : a sampler can only be used as the first argument of a texture function", user_line()));
            }
            _ => {}
        }

        i += 1;
    }

    let _ = used_frag_output;

    // Assemble: a head, then the text with the edits.
    let mut edits = edits;
    edits.sort_by_key(|e| e.0);
    let mut body = String::with_capacity(text.len() + 256);
    let mut pos = 0;

    for (s, e, r) in &edits {
        if *s < pos {
            continue;
        }

        body.push_str(&text[pos..*s]);
        body.push_str(r);
        pos = *e;
    }

    body.push_str(&text[pos..]);

    // The preamble: one line each.
    let mut head = String::from("#version 450\n#define x3d_MaxLights 8\n");
    head.push_str(&format!("layout(std140, set=0, binding=0) uniform _X3DU {{ vec4 _u[{}]; }};\n", info.slots.max(1)));

    for s in info.samplers {
        let (tex, ctor) = match s.dim {
            SamplerDim::D2 => ("texture2D", ""),
            SamplerDim::D3 => ("texture3D", ""),
            SamplerDim::Cube => ("textureCube", ""),
        };
        let _ = ctor;
        head.push_str(&format!("layout(set=1, binding={}) uniform {tex} _x3d_t{};\n", s.binding, s.binding));
        head.push_str(&format!("layout(set=1, binding={}) uniform sampler _x3d_s{};\n", s.binding + 1, s.binding));
    }

    match a.stage {
        Stage::Fragment => {
            if !a.outputs.iter().any(|_| true) {
                head.push_str("layout(location=0) out vec4 _x3d_FragColor;\n");
            }

            head.push_str("vec4 _x3d_FragCoord() { return vec4(gl_FragCoord.x, _u[0].w > 0.5 ? gl_FragCoord.y : _u[0].x - gl_FragCoord.y, gl_FragCoord.z, gl_FragCoord.w); }\n");
        }
        Stage::Vertex => head.push_str("float _x3d_PointSize;\n"),
    }

    let line_base = newlines(&head);

    // The wrapper main.
    let mut copy = String::new();
    let mut leaves_unused = HashMap::new();

    for u in info.uniforms {
        // Only uniforms this stage declares are copied (the block holds all of them).
        if !a.uniforms.iter().any(|(n, ..)| *n == u.name) {
            continue;
        }

        let mut slot = u.slot;

        match u.array {
            Some(len) => {
                for k in 0..len {
                    emit_copy(&mut copy, &format!("{}[{k}]", u.name), &u.ty, &mut slot, info.structs, &mut leaves_unused, true);
                }
            }
            None => emit_copy(&mut copy, &u.name, &u.ty, &mut slot, info.structs, &mut leaves_unused, true),
        }

        copy.push('\n');
    }

    for (name, ty, _) in &a.attributes {
        if let Some((cols, _)) = matrix_columns(ty) {
            let columns: Vec<String> = (0..cols).map(|k| format!("_x3d_a_{name}_{k}")).collect();
            copy.push_str(&format!("{name} = {ty}({});\n", columns.join(", ")));
        }
    }

    let _ = info.leaves;
    let mut main = String::from("\nvoid main() {\n");
    main.push_str(&copy);
    main.push_str("_x3d_main();\n");

    if a.stage == Stage::Vertex {
        main.push_str("gl_Position.y = gl_Position.y * _u[0].z;\n");
        main.push_str("if (_u[0].y == 0.0) { gl_Position.z = 0.5 * (gl_Position.z + gl_Position.w); }\n");
    }

    main.push_str("}\n");

    let _ = a.es3;
    Ok(Emitted { text: format!("{head}{body}{main}"), line_base: line_base + a.prefix_lines })
}

/// The column count and column type of a matrix attribute type.
fn matrix_columns(ty: &str) -> Option<(u32, &'static str)> {
    match ty {
        "mat3" => Some((3, "vec3")),
        "mat4" => Some((4, "vec4")),
        _ => None,
    }
}

/// `precision mediump sampler3D;` is legal GLSL ES, naga only knows the statement for float and int:
/// the statement becomes blanks (the sampler types have no precision in this port).
fn mask_sampler_precision(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;

    while let Some(at) = rest.find("precision") {
        let (before, after) = rest.split_at(at);
        out.push_str(before);

        // Only a whole word at a statement start.
        let word_start = before.chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        let end = after.find(';');
        let masked = end.filter(|_| word_start).and_then(|e| {
            let words: Vec<&str> = after[..e].split_whitespace().collect();
            (words.len() == 3 && is_precision(words[1]) && (words[2].contains("sampler") || words[2].contains("image"))).then_some(e + 1)
        });

        match masked {
            Some(len) => {
                out.extend(after[..len].chars().map(|c| if c == '\n' { '\n' } else { ' ' }));
                rest = &after[len..];
            }
            None => {
                out.push_str("precision");
                rest = &after["precision".len()..];
            }
        }
    }

    out.push_str(rest);
    out
}

fn ty_components(ty: &str) -> u32 {
    match ty {
        "float" | "int" | "uint" | "bool" => 1,
        "vec2" | "ivec2" | "uvec2" => 2,
        "vec3" | "ivec3" | "uvec3" => 3,
        "vec4" | "ivec4" | "uvec4" => 4,
        "mat3" => 9,
        "mat4" => 16,
        "mat2" => 4,
        _ => 4,
    }
}

// ---- naga -------------------------------------------------------------------------------------

/// naga GLSL -> validated module -> WGSL, then WGSL -> module again with the capabilities wgpu
/// gives a default device, so the pipeline creation cannot fail on it later.
fn to_wgsl(stage: Stage, e: &Emitted) -> Result<String, String> {
    use naga::front::glsl::{Frontend, Options};
    let mut front = Frontend::default();
    let ns = match stage {
        Stage::Vertex => naga::ShaderStage::Vertex,
        Stage::Fragment => naga::ShaderStage::Fragment,
    };
    let module = front.parse(&Options::from(ns), &e.text).map_err(|errs| {
        if std::env::var_os("X3D_GLSL_DEBUG").is_some() {
            eprintln!("---- naga input ({stage:?}) ----\n{}", e.text);
        }

        let mut out = vec![];

        for err in errs.errors {
            let line = err.meta.location(&e.text).line_number as usize;
            let line = line.saturating_sub(e.line_base).max(1);
            let message = err.kind.to_string();

            // The compiler of the pinned X_ITE is ANGLE: its texts for the two commonest mistakes.
            if let Some(name) = message.strip_prefix("Unknown variable: ") {
                out.push(format!("ERROR: 0:{line}: '{name}' : undeclared identifier"));
            } else if message.starts_with("Expected ") && message.contains(", found ") {
                let token = err.meta.to_range().and_then(|r| e.text.get(r)).map(str::trim).filter(|t| !t.is_empty() && t.len() < 40).map(str::to_string).or_else(|| {
                    let found = message.rsplit(", found ").next()?;
                    found.strip_prefix("Identifier(\"").and_then(|x| x.strip_suffix("\")")).map(str::to_string).or_else(|| (found == "Semicolon").then(|| ";".to_string()))
                });
                out.push(format!("ERROR: 0:{line}: '{}' : syntax error", token.unwrap_or_default()));
            } else {
                out.push(format!("ERROR: 0:{line}: {message}"));
            }
        }

        out.join("\n")
    })?;
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
        .validate(&module)
        .map_err(|err| format!("ERROR: 0:1: {}", err.as_inner()))?;
    let wgsl = naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty()).map_err(|err| format!("ERROR: 0:1: {err}"))?;
    let again = naga::front::wgsl::parse_str(&wgsl).map_err(|err| format!("ERROR: 0:1: {}", err.message()))?;
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty()).validate(&again).map_err(|err| format!("ERROR: 0:1: {}", err.as_inner()))?;
    Ok(wgsl)
}

// ---- public API -------------------------------------------------------------------------------

fn parse_options(options: &[String]) -> Vec<String> {
    if options.is_empty() { CUSTOM_OPTIONS.iter().map(|s| s.to_string()).collect() } else { options.to_vec() }
}

/// ShaderPart.loadData's compile: does `source` compile as a `stage` shader? The error is the
/// log (`ERROR: 0:line: message`), the line counted in the user's text.
pub fn compile_part(stage: Stage, source: &str, options: &[String]) -> Result<(), String> {
    let options = parse_options(options);
    let a = analyze(stage, source, &options)?;
    let p = link_analyses(&[&a], false)?;
    drop(p);
    Ok(())
}

/// ComposedShader.set_loaded__: link the vertex and fragment parts.
pub fn link(vertex: &[(String, Vec<String>)], fragment: &[(String, Vec<String>)]) -> Result<Program, String> {
    // X_ITE attaches every part; the program must contain both stages. ANGLE links several parts of one
    // stage (each has its own `main`; measured: the program draws, oracle/synthetic-rea-b08-shaders/bad-two-vertex.wrl);
    // which one wins is not known, this port links the first of each.
    let (Some(v), Some(f)) = (vertex.first(), fragment.first()) else {
        return Err("The program must contain objects to form both a vertex and fragment shader.".into());
    };

    let va = analyze(Stage::Vertex, &v.0, &parse_options(&v.1))?;
    let fa = analyze(Stage::Fragment, &f.0, &parse_options(&f.1))?;
    link_analyses(&[&va, &fa], true)
}

fn link_analyses(parts: &[&Analysis], both: bool) -> Result<Program, String> {
    // Uniform union, in order of appearance. slot 0 is the renderer's.
    let mut structs: HashMap<String, StructDef> = HashMap::new();

    for p in parts {
        for (k, v) in &p.scan.structs {
            structs.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }

    let mut uniforms: Vec<UniformDecl> = vec![];
    let mut next = 1u32;

    for p in parts {
        for (name, ty, array) in &p.uniforms {
            if let Some(u) = uniforms.iter().find(|u| u.name == *name) {
                if u.ty != *ty || u.array != *array {
                    return Err(format!("ERROR: Uniform '{name}' is declared with different types in the shaders."));
                }

                continue;
            }

            let slots = ty_slots(ty, &structs) * array.unwrap_or(1);
            uniforms.push(UniformDecl { name: name.clone(), ty: ty.clone(), array: *array, slot: next, slots });
            next += slots;
        }
    }

    if next > MAX_SLOTS {
        return Err(format!("ERROR: The uniforms need {next} vec4 slots, the limit is {MAX_SLOTS}."));
    }

    let mut leaves = HashMap::new();

    for u in &uniforms {
        let mut slot = u.slot;
        let mut unused = String::new();

        match u.array {
            Some(len) => {
                for k in 0..len {
                    emit_copy(&mut unused, &format!("{}[{k}]", u.name), &u.ty, &mut slot, &structs, &mut leaves, false);
                }
            }
            None => emit_copy(&mut unused, &u.name, &u.ty, &mut slot, &structs, &mut leaves, false),
        }
    }

    // Samplers: texture and sampler binding pairs, by name and element.
    let mut samplers: Vec<SamplerDecl> = vec![];
    let mut sampler_elem: HashMap<(String, Option<u32>), usize> = HashMap::new();
    let mut seen: BTreeMap<String, (SamplerDim, Option<u32>)> = BTreeMap::new();

    for p in parts {
        for (name, array, dim) in &p.samplers {
            if let Some((d, a)) = seen.get(name) {
                if d != dim || a != array {
                    return Err(format!("ERROR: Uniform '{name}' is declared with different types in the shaders."));
                }

                continue;
            }

            seen.insert(name.clone(), (*dim, *array));

            match array {
                Some(len) => {
                    for k in 0..*len {
                        sampler_elem.insert((name.clone(), Some(k)), samplers.len());
                        samplers.push(SamplerDecl { name: name.clone(), index: Some(k), dim: *dim, binding: 2 * samplers.len() as u32 });
                    }
                }
                None => {
                    sampler_elem.insert((name.clone(), None), samplers.len());
                    samplers.push(SamplerDecl { name: name.clone(), index: None, dim: *dim, binding: 2 * samplers.len() as u32 });
                }
            }
        }
    }

    if samplers.len() > 8 {
        return Err(format!("ERROR: {} samplers; at most 8 are supported.", samplers.len()));
    }

    // Attributes.
    let mut attr_locations: HashMap<String, u32> = HashMap::new();
    let mut attributes: Vec<AttrDecl> = vec![];
    let mut next_custom = FIRST_CUSTOM_LOCATION;

    if let Some(v) = parts.iter().find(|p| p.stage == Stage::Vertex) {
        for (name, ty, _layout) in &v.attributes {
            if attr_locations.contains_key(name) {
                continue;
            }

            let components = ty_components(ty);
            let cols = match components {
                9 => 3,
                16 => 4,
                _ => 1,
            };

            let loc = match ATTRIBUTE_LOCATIONS.iter().find(|(n, _)| n == name) {
                Some((_, l)) => *l,
                None => {
                    let l = next_custom;
                    next_custom += cols;
                    l
                }
            };

            if loc + cols > MAX_ATTRIBUTE_LOCATIONS {
                return Err(format!("ERROR: Too many vertex attributes ('{name}')."));
            }

            attr_locations.insert(name.clone(), loc);
            attributes.push(AttrDecl { name: name.clone(), location: loc, components });
        }
    }

    // Varyings: the vertex shader's outputs in order; the fragment shader's inputs match by name.
    let mut varying_locations: HashMap<String, u32> = HashMap::new();

    if let Some(v) = parts.iter().find(|p| p.stage == Stage::Vertex) {
        let mut loc = 0;

        for (name, ty, _q, array) in &v.varyings {
            if varying_locations.contains_key(name) {
                continue;
            }

            varying_locations.insert(name.clone(), loc);
            loc += array.unwrap_or(1) * if ty == "mat4" { 4 } else if ty == "mat3" { 3 } else { 1 };
        }
    }

    if both {
        let v = parts.iter().find(|p| p.stage == Stage::Vertex).unwrap();
        let f = parts.iter().find(|p| p.stage == Stage::Fragment).unwrap();

        for (name, ty, ..) in &f.varyings {
            match v.varyings.iter().find(|(n, ..)| n == name) {
                Some((_, vty, ..)) if vty == ty => {}
                Some(_) => return Err(format!("ERROR: Varying '{name}' has different types in the vertex and fragment shaders.")),
                None => return Err(format!("ERROR: Fragment shader input varying '{name}' has no matching output in the vertex shader.")),
            }
        }
    } else {
        // A single stage: its varyings get locations in order, fragment inputs included.
        for p in parts {
            for (i, (name, ..)) in p.varyings.iter().enumerate() {
                varying_locations.entry(name.clone()).or_insert(i as u32);
            }
        }
    }

    let info = LinkInfo { slots: next, samplers: &samplers, sampler_elem: &sampler_elem, attr_locations: &attr_locations, varying_locations: &varying_locations, structs: &structs, leaves: &leaves, uniforms: &uniforms };
    let mut vs_wgsl = String::new();
    let mut fs_wgsl = String::new();
    let mut h = DefaultHasher::new();

    for p in parts {
        let e = emit(p, &info)?;
        e.text.hash(&mut h);
        let w = to_wgsl(p.stage, &e)?;

        match p.stage {
            Stage::Vertex => vs_wgsl = w,
            Stage::Fragment => fs_wgsl = w,
        }
    }

    let warnings = deprecated_warnings(parts, &leaves, &samplers, &attributes);

    Ok(Program { key: h.finish(), vs_wgsl, fs_wgsl, slots: next, uniforms, leaves, samplers, attributes, warnings })
}

// ---- helpers for the host ---------------------------------------------------------------------

impl Program {
    /// The vec4 slot array of a draw, zeroed, with the renderer's slot 0 set.
    pub fn new_buffer(&self) -> Vec<[f32; 4]> {
        vec![[0.0; 4]; self.slots.max(1) as usize]
    }

    /// Write floats at a leaf path (`x3d_ModelViewMatrix`, `x3d_LightSource[0].color`). Columns of
    /// a matrix come in column-major order; ints and bools are written as numbers.
    pub fn set(&self, buf: &mut [[f32; 4]], path: &str, values: &[f32]) -> bool {
        let Some(leaf) = self.leaves.get(path) else { return false };
        let (rows, cols) = match &leaf.ty {
            Ty::Scalar(_) => (1, 1),
            Ty::Vec(_, n) => (*n as usize, 1),
            Ty::Mat(n) => (*n as usize, *n as usize),
            Ty::Struct(_) => return false,
        };

        for c in 0..cols {
            let slot = &mut buf[leaf.slot as usize + c];

            for r in 0..rows {
                slot[r] = values.get(c * rows + r).copied().unwrap_or(0.0);
            }
        }

        true
    }

    pub fn has(&self, path: &str) -> bool {
        self.leaves.contains_key(path)
    }

    pub fn uniform(&self, name: &str) -> Option<&UniformDecl> {
        self.uniforms.iter().find(|u| u.name == name)
    }

    /// Leaf paths in slot order (for tests).
    pub fn leaf_paths(&self) -> Vec<String> {
        let mut v: Vec<(&String, &Leaf)> = self.leaves.iter().collect();
        v.sort_by_key(|(_, l)| l.slot);
        v.into_iter().map(|(p, _)| p.clone()).collect()
    }
}

/// X3DProgrammableShaderObject.getDefaultUniformsAndAttributes: a legacy name found where the new one is
/// absent logs a warning, in the order the code asks (fog, lights, material, cube maps, texture coordinates).
fn deprecated_warnings(parts: &[&Analysis], leaves: &HashMap<String, Leaf>, samplers: &[SamplerDecl], attributes: &[AttrDecl]) -> Vec<String> {
    let active_leaf = |path: &str| -> bool {
        if !leaves.contains_key(path) {
            return false;
        }

        let base = path.split(['.', '[']).next().unwrap_or(path);
        let member = path.rsplit('.').next().filter(|_| path.contains('.'));

        parts.iter().any(|p| {
            p.used.idents.contains(base)
                && match member {
                    Some(m) => p.used.whole.contains(base) || p.used.members.contains(m),
                    None => true,
                }
        })
    };
    let active_sampler = |name: &str, index: u32| samplers.iter().any(|s| s.name == name && s.index == Some(index) && parts.iter().any(|p| p.used.idents.contains(name)));
    let uniform = |old: &str, new: &str| format!("Using uniform location name '{old}' is deprecated, use '{new}'. See https://create3000.github.io/x_ite/custom-shaders.");
    let mut warnings = vec![];
    let mut check = |old: String, new: String| {
        if !active_leaf(&new) && active_leaf(&old) {
            warnings.push(uniform(&old, &new));
        }
    };

    for (new, old) in [("color", "Color"), ("visibilityStart", "VisibilityStart"), ("visibilityRange", "VisibilityRange"), ("matrix", "Matrix")] {
        check(format!("x3d_Fog{old}"), format!("x3d_Fog.{new}"));
    }

    for i in 0..8 {
        for (new, old) in [
            ("type", "Type"),
            ("color", "Color"),
            ("ambientIntensity", "AmbientIntensity"),
            ("intensity", "Intensity"),
            ("attenuation", "Attenuation"),
            ("location", "Location"),
            ("direction", "Direction"),
            ("beamWidth", "BeamWidth"),
            ("cutOffAngle", "CutOffAngle"),
            ("radius", "Radius"),
            ("matrix", "Matrix"),
        ] {
            check(format!("x3d_Light{old}[{i}]"), format!("x3d_LightSource[{i}].{new}"));
        }
    }

    for m in ["ambientIntensity", "diffuseColor", "specularColor", "emissiveColor", "shininess", "transparency"] {
        check(format!("x3d_FrontMaterial.{m}"), format!("x3d_Material.{m}"));
    }

    for i in 0..4 {
        if !active_sampler("x3d_TextureCube", i) && active_sampler("x3d_CubeMapTexture", i) {
            warnings.push(uniform(&format!("x3d_CubeMapTexture[{i}]"), &format!("x3d_TextureCube[{i}]")));
        }
    }

    // The attribute x3d_TexCoord (texture coordinate 0) is deprecated for x3d_TexCoord0.
    let attribute = |n: &str| attributes.iter().any(|a| a.name == n) && parts.iter().any(|p| p.stage == Stage::Vertex && p.used.idents.contains(n));

    if !attribute("x3d_TexCoord0") && attribute("x3d_TexCoord") {
        warnings.push("Using attribute location name 'x3d_TexCoord' is deprecated, use 'x3d_TexCoord0'. See https://create3000.github.io/x_ite/custom-shaders.".to_string());
    }

    warnings
}

/// Memoized link (the node link and the renderer ask for the same program).
pub fn link_cached(vertex: &[(String, Vec<String>)], fragment: &[(String, Vec<String>)]) -> Result<std::rc::Rc<Program>, String> {
    thread_local! {
        static CACHE: std::cell::RefCell<HashMap<u64, Result<std::rc::Rc<Program>, String>>> = Default::default();
    }

    let mut h = DefaultHasher::new();
    vertex.hash(&mut h);
    fragment.hash(&mut h);
    let key = h.finish();

    if let Some(r) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return r;
    }

    let r = link(vertex, fragment).map(std::rc::Rc::new);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();

        if c.len() > 256 {
            c.clear();
        }

        c.insert(key, r.clone());
    });
    r
}
