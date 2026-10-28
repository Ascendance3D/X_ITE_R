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

/// VRMLParser.isValid.
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

/// GoldenGate.parseIntoScene: the first parser whose input converts and that accepts it, in
/// X_ITE's order (XML, VRML, JSON, glTF, GLB, OBJ, STL ASCII, STL binary, PLY ASCII, PLY binary,
/// SVG). glTF and GLB are not ported; input only they would accept finds no handler.
pub fn parse_bytes(data: &[u8], world_url: &str) -> Result<Scene, LoadError> {
    let mut scene = golden_gate(data, world_url)?;

    scene.clone_counts = Some(scene.parse_clone_counts());
    Ok(scene)
}

/// [`parse_bytes`] for a JavaScript string (`createX3DFromString`): the text is the input as it is (no gunzip, no
/// byte order mark removal, as `$.decodeText` leaves a string alone). The binary parsers' input is an ArrayBuffer, which a
/// string is not, so they never match.
pub fn parse_string(text: &str, world_url: &str) -> Result<Scene, LoadError> {
    let mut scene = golden_gate_input(Vec::new(), text.to_string(), world_url)?;

    scene.clone_counts = Some(scene.parse_clone_counts());
    Ok(scene)
}

fn golden_gate(data: &[u8], world_url: &str) -> Result<Scene, LoadError> {
    let data = gunzip(data);
    let text = decode_text(&data);

    golden_gate_input(data, text, world_url)
}

fn golden_gate_input(data: Vec<u8>, text: String, world_url: &str) -> Result<Scene, LoadError> {
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

    // GLB2Parser
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
