//! VRML 1.0 (`#VRML V1.0 ascii`) loader.
//!
//! A Rust port of the `x_ite-vrml1-parser` 1.1.0 extension for X_ITE
//! (https://github.com/create3000/x_ite-vrml1-parser, MIT, Copyright (c) Holger Seelig and
//! other contributors), the extension X_ITE 16.7.0 documents for VRML 1.0. It is **not** part
//! of the X_ITE 16.5.0 pin that the rest of this workspace is checked against: the extension is
//! a separate port, done by reading the shipped package and diffing against its output.
//!
//! Like X_ITE, where the application registers the extension with `GoldenGate.addParsers`,
//! `x3d-parse` loads VRML 1.0 only when asked: build it with its Cargo feature `vrml1` (off by
//! default) and pass `x3d_parse::ParseOptions { vrml1: true }` to `parse_bytes_with` or
//! `parse_string_with`. Otherwise `#VRML V1.0` is rejected, as in the pinned X_ITE.
//!
//! Like the extension, the loader works in two passes. The text is first read into a tree of
//! VRML 1.0 nodes (the extension's `VRML1Parser`), then the tree is converted into X3D nodes of
//! the engine's [`Scene`] (its `Converter`). VRML 1.0 is a state machine: `Material`,
//! `Transform` and similar nodes affect the shapes that come *after* them inside a `Separator`,
//! and the state is restored when the `Separator` ends. The converter keeps that state in
//! stacks; the module docs of `convert.rs` list the state model and the extension's quirks, which are kept.
//!
//! ```
//! let scene = x3d_vrml1::parse_str(
//!     "#VRML V1.0 ascii\nSeparator { Material { diffuseColor 1 0 0 } Cube { } }\n",
//!     "file:///a.wrl",
//! )
//! .unwrap();
//! assert_eq!(scene.ctx(x3d_scene::ROOT).root_nodes.len(), 1);
//! ```
//!
//! The crate has no dependencies beyond the engine's field and scene crates, so it builds for
//! `wasm32-unknown-unknown`. It reads text only: decompress `.wrl.gz` input first.

mod convert;
mod nodes;
mod parser;

use x3d_scene::Scene;

/// The `providerURL` of X_ITE, which the extension stores as `reference` of the
/// `MetadataString` it makes for an `Info` node inside a `Separator`.
pub const DEFAULT_PROVIDER_URL: &str = "https://create3000.github.io/x_ite/";

/// A parse or conversion error, with the position in the input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vrml1Error {
    pub message: String,
    /// 1-based line.
    pub line: usize,
    /// 1-based column, in characters.
    pub column: usize,
}

impl std::fmt::Display for Vrml1Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "VRML 1.0 parser error at line {}, column {}: {}",
            self.line, self.column, self.message
        )
    }
}

impl std::error::Error for Vrml1Error {}

/// Options of the conversion.
#[derive(Clone, Debug)]
pub struct Options {
    /// `reference` of the `MetadataString` made for an `Info` node inside a `Separator`.
    pub provider_url: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            provider_url: DEFAULT_PROVIDER_URL.to_string(),
        }
    }
}

/// `VRML1Parser.isValid`: the input starts with `#VRML V1.0`.
pub fn is_vrml1(input: &str) -> bool {
    input.starts_with("#VRML V1.0")
}

/// Parse VRML 1.0 text into a scene (X3D encoding "VRML", profile Interchange).
pub fn parse_str(input: &str, world_url: &str) -> Result<Scene, Vrml1Error> {
    parse_str_with(input, world_url, &Options::default())
}

/// [`parse_str`] with options.
pub fn parse_str_with(
    input: &str,
    world_url: &str,
    options: &Options,
) -> Result<Scene, Vrml1Error> {
    let doc = parser::parse(input)?;

    convert::convert(&doc, world_url, options, input)
}

/// [`parse_str`] for bytes: UTF-8 with replacement of invalid sequences, a byte order mark removed.
pub fn parse_bytes(data: &[u8], world_url: &str) -> Result<Scene, Vrml1Error> {
    let text = String::from_utf8_lossy(data);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);

    parse_str(text, world_url)
}
