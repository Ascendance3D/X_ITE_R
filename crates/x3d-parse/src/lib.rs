//! VRML97 / X3D classic encoding parser, ported from X_ITE
//! (https://github.com/create3000/x_ite, MIT, Copyright (c) 2009 Holger Seelig and other
//! contributors). Pinned reference: oracle/x_ite-pin (fork commit 34cdf9f97).
//!
//! `load_file` → bytes (gunzip by magic, like `$.gunzip` in src/lib/helper.js) → text
//! (`TextDecoder`, UTF-8 with replacement, BOM stripped) → `Parser` → `x3d_scene::Scene`.

pub mod color;
pub mod compare;
mod data_url;
mod dom;
mod formats;
mod gltf2;
mod html;
mod html_support;
mod json;
pub mod lexer;
pub mod optimizer;
pub mod parser;
mod svg;
mod svg_canvas;
pub mod woff1;
mod xml;

pub use data_url::{chromium_url_input, decode_data_url, is_data_url};
pub use gltf2::{default_fetch as gltf_default_fetch, with_fetcher as with_gltf_fetcher};
pub use html::parse_html_x3d;
pub use parser::{ContextCall, ParseError, Parser, take_error_detail, take_error_warnings};
pub use svg::{Quality as SvgQuality, set_quality as set_svg_quality};

use std::io::Read;
use x3d_scene::Scene;

/// `$.gunzip`: decompress when the data starts with the gzip magic; on any error, the raw bytes.
pub fn gunzip(data: &[u8]) -> Vec<u8> {
    if data.len() < 2 || data[0] != 0x1f || data[1] != 0x8b {
        return data.to_vec();
    }

    let mut out = vec![];

    match flate2::read::GzDecoder::new(data).read_to_end(&mut out) {
        Ok(_) => out,
        Err(_) => data.to_vec(),
    }
}

/// `new TextDecoder () .decode (bytes)`: UTF-8, invalid sequences replaced, BOM removed.
pub fn decode_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes).into_owned();

    match text.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_string(),
        None => text,
    }
}

/// VRMLParser.isValid. A `#VRML V1.0` header is not VRML97. It is rejected, as in the pinned X_ITE,
/// unless the caller opts in with [`ParseOptions::vrml1`] (see [`parse_bytes_with`]).
pub fn is_valid_vrml(input: &str) -> bool {
    if input.starts_with("#VRML V1.0") {
        return false;
    }

    if input.starts_with("#X3D") || input.starts_with("#VRML") {
        return true;
    }

    let bytes = input.as_bytes();
    let mut pos = 0;

    loop {
        if let Some(e) = lexer::whitespaces(bytes, pos) {
            pos = e;
            continue;
        }

        if bytes[pos..].starts_with(b"#/*") {
            if let Some(i) = input[pos + 3..].find("*/#") {
                pos += 3 + i + 3;
                continue;
            }
        }

        if bytes.get(pos) == Some(&b'#') {
            match input[pos..].find(['\r', '\n']) {
                Some(i) => {
                    pos += i + 1;
                    continue;
                }
                None => return false,
            }
        }

        break;
    }

    let rest = &input[pos..];

    if rest.is_empty() {
        return true;
    }

    for k in [
        "PROFILE",
        "COMPONENT",
        "META",
        "UNIT",
        "EXTERNPROTO",
        "PROTO",
        "DEF",
        "NULL",
        "IMPORT",
        "EXPORT",
        "ROUTE",
    ] {
        if rest.starts_with(k) {
            return true;
        }
    }

    // \w+ (whitespace | comments)* \{
    let word = rest
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
        .count();

    if word == 0 {
        return false;
    }

    let after = rest[word..].trim_start_matches([' ', '\n', ',', '\t', '\r']);

    after.starts_with('{')
}

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    NoHandler,
    Parse(ParseError),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "{e}"),
            LoadError::NoHandler => {
                write!(f, "Couldn't parse X3D. No suitable file handler found.")
            }
            LoadError::Parse(e) => write!(f, "{e}"),
        }
    }
}

/// Per-call parser registration, the counterpart of X_ITE's `GoldenGate.addParsers`. The default
/// registers nothing beyond the built-in parsers, which is the behavior of the pinned X_ITE 16.5.0.
///
/// There are two switches for VRML 1.0 (`#VRML V1.0 ascii`, a port of the x_ite-vrml1-parser
/// extension, not a pin parity item), both off by default: the Cargo feature `vrml1` compiles the
/// `x3d-vrml1` loader in, and [`ParseOptions::vrml1`] lets one call use it. Without the feature
/// the option is ignored and the input finds no handler.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParseOptions {
    /// Try the VRML 1.0 loader, after the built-in parsers, for `#VRML V1.0` input.
    pub vrml1: bool,
}

/// GoldenGate.parseIntoScene: the first parser whose input converts and that accepts it, in
/// X_ITE's order (XML, VRML, JSON, glTF, GLB, OBJ, STL ASCII, STL binary, PLY ASCII, PLY binary,
/// SVG). glTF and GLB are not ported; input only they would accept finds no handler.
///
/// This is [`parse_bytes_with`] with the default [`ParseOptions`], so `#VRML V1.0` input finds no handler.
pub fn parse_bytes(data: &[u8], world_url: &str) -> Result<Scene, LoadError> {
    parse_bytes_with(data, world_url, &ParseOptions::default())
}

/// [`parse_bytes`] with explicit [`ParseOptions`].
pub fn parse_bytes_with(
    data: &[u8],
    world_url: &str,
    options: &ParseOptions,
) -> Result<Scene, LoadError> {
    let mut scene = golden_gate(data, world_url, options)?;

    scene.clone_counts = Some(scene.parse_clone_counts());
    Ok(scene)
}

/// [`parse_bytes`] for a JavaScript string (`createX3DFromString`): the text is the input as it is (no gunzip, no
/// byte order mark removal, as `$.decodeText` leaves a string alone). The binary parsers' input is an ArrayBuffer, which a
/// string is not, so they never match.
pub fn parse_string(text: &str, world_url: &str) -> Result<Scene, LoadError> {
    parse_string_with(text, world_url, &ParseOptions::default())
}

/// [`parse_string`] with explicit [`ParseOptions`].
pub fn parse_string_with(
    text: &str,
    world_url: &str,
    options: &ParseOptions,
) -> Result<Scene, LoadError> {
    let mut scene = golden_gate_input(Vec::new(), text.to_string(), world_url, options)?;

    scene.clone_counts = Some(scene.parse_clone_counts());
    Ok(scene)
}

fn golden_gate(data: &[u8], world_url: &str, options: &ParseOptions) -> Result<Scene, LoadError> {
    let data = gunzip(data);
    let text = decode_text(&data);

    golden_gate_input(data, text, world_url, options)
}

fn golden_gate_input(
    data: Vec<u8>,
    text: String,
    world_url: &str,
    options: &ParseOptions,
) -> Result<Scene, LoadError> {
    let mut svg_root = None;

    // createInput ("STRING"): undefined when bytes decode to nothing.
    let string = (data.is_empty() || !text.is_empty()).then_some(text.as_str());

    // XML: "" for blank input, markup only, $.parseXML must succeed, root must be <X3D>.
    if let Some(s) = string {
        let trimmed = s.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');

        if trimmed.is_empty() {
            return Ok(xml::empty_scene(world_url));
        }

        if trimmed.starts_with('<') {
            if let Ok(root) = dom::parse_document(s) {
                if root.name == "X3D" {
                    return xml::parse_x3d_element(&root, world_url).map_err(LoadError::Parse);
                }

                // The same parsed document is SVGParser's input.
                svg_root = Some(root);
            }
        }

        if is_valid_vrml(s) {
            return Parser::new(s, world_url).parse().map_err(LoadError::Parse);
        }

        if let Ok(value) = json::json_parse(s) {
            if json::is_valid(&value) {
                let root = json::to_x3d_element(&value)
                    .map_err(|message| LoadError::Parse(ParseError { message, line: 0 }))?;
                let mut scene =
                    xml::parse_x3d_element(&root, world_url).map_err(LoadError::Parse)?;

                scene.encoding = "JSON".into();
                return Ok(scene);
            }

            // GLTF2Parser (same JSON.parse result).
            if gltf2::is_valid_gltf(&value) {
                return gltf2::parse_gltf(&value, world_url)
                    .map_err(|message| LoadError::Parse(ParseError { message, line: 0 }));
            }
        }
    }

    // GLBParser (container versions 2 and 3)
    if gltf2::is_valid_glb(&data) {
        return gltf2::parse_glb(&data, world_url)
            .map_err(|message| LoadError::Parse(ParseError { message, line: 0 }));
    }

    if let Some(s) = string {
        if formats::is_obj(s) {
            return formats::parse_obj(s, world_url);
        }

        if formats::is_stl_ascii(s) {
            return formats::parse_stl_ascii(s, world_url);
        }
    }

    if formats::is_stl_binary(&data) {
        return formats::parse_stl_binary(&data, world_url);
    }

    if let Some(s) = string {
        if formats::is_ply_ascii(s) {
            return formats::parse_ply_ascii(s, world_url);
        }
    }

    if formats::is_ply_binary(&data) {
        return formats::parse_ply_binary(&data, world_url);
    }

    // SVGParser: the document element must be <svg>.
    if let Some(root) = &svg_root {
        if svg::is_valid(root) {
            return svg::parse_svg(root, world_url);
        }
    }

    // The VRML 1.0 extension parser (x_ite-vrml1-parser) is added to GoldenGate's list after the
    // built-in ones (GoldenGate.addParsers), only when the application registers it. It is not a
    // parser of the pinned X_ITE 16.5.0, which rejects `#VRML V1.0` without it.
    #[cfg(feature = "vrml1")]
    if let (true, Some(s)) = (options.vrml1, string) {
        if x3d_vrml1::is_vrml1(s) {
            return x3d_vrml1::parse_str(s, world_url).map_err(|e| {
                LoadError::Parse(ParseError {
                    message: format!("{} (column {})", e.message, e.column),
                    line: e.line,
                })
            });
        }
    }

    let _ = options;
    Err(LoadError::NoHandler)
}

/// X3DBrowser.importJS: `new JSONParser (scene)` with the JSON text as its input, parsed into a new scene (no GoldenGate checks).
pub fn parse_json_x3d(text: &str, world_url: &str) -> Result<Scene, LoadError> {
    let value = json::json_parse(text)
        .map_err(|message| LoadError::Parse(ParseError { message, line: 0 }))?;
    let root = json::to_x3d_element(&value)
        .map_err(|message| LoadError::Parse(ParseError { message, line: 0 }))?;
    let mut scene = xml::parse_x3d_element(&root, world_url).map_err(LoadError::Parse)?;

    scene.encoding = "JSON".into();
    scene.clone_counts = Some(scene.parse_clone_counts());
    Ok(scene)
}

pub fn load_file(path: &std::path::Path) -> Result<Scene, LoadError> {
    let data = std::fs::read(path).map_err(LoadError::Io)?;
    let url = format!("file://{}", path.display());

    parse_bytes(&data, &url)
}
