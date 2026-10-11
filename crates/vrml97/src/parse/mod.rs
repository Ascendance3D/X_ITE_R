//! VRML97 / X3D classic encoding parser, ported from X_ITE
//! (https://github.com/create3000/x_ite, MIT, Copyright (c) 2009 Holger Seelig and other
//! contributors). Reference: X_ITE 16.5.0.
//!
//! `load_file` → bytes (gunzip by magic, like `$.gunzip` in src/lib/helper.js) → text
//! (`TextDecoder`, UTF-8 with replacement, BOM stripped) → `Parser` → `crate::scene::Scene`.

pub mod color;

pub mod lexer;
pub mod parser;

pub use parser::{ParseError, Parser};

use std::io::Read;
use crate::scene::Scene;

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

    for k in ["PROFILE", "COMPONENT", "META", "UNIT", "EXTERNPROTO", "PROTO", "DEF", "NULL", "IMPORT", "EXPORT", "ROUTE"] {
        if rest.starts_with(k) {
            return true;
        }
    }

    // \w+ (whitespace | comments)* \{
    let word = rest.bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'_').count();

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
            LoadError::NoHandler => write!(f, "Couldn't parse X3D. No suitable file handler found."),
            LoadError::Parse(e) => write!(f, "{e}"),
        }
    }
}

pub fn parse_bytes(data: &[u8], world_url: &str) -> Result<Scene, LoadError> {
    let text = decode_text(&gunzip(data));

    if !is_valid_vrml(&text) {
        return Err(LoadError::NoHandler);
    }

    Parser::new(&text, world_url).parse().map_err(LoadError::Parse)
}

pub fn load_file(path: &std::path::Path) -> Result<Scene, LoadError> {
    let data = std::fs::read(path).map_err(LoadError::Io)?;
    let url = format!("file://{}", path.display());

    parse_bytes(&data, &url)
}
