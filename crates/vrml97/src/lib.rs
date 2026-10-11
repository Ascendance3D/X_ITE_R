//! Read and write VRML97 and X3D classic-encoding (`.wrl`, `.x3dv`) files.
//!
//! This crate is a faithful Rust port of the parser, field types and VRML serializer of
//! [X_ITE](https://github.com/create3000/x_ite) (MIT, Copyright (c) 2009 Holger Seelig and
//! other contributors). It aims for byte-identical output with X_ITE's `toVRMLString()`,
//! quirks included, and reads gzip-compressed files transparently.
//!
//! ```no_run
//! let scene = vrml97::load_file(std::path::Path::new("world.wrl")).unwrap();
//! println!("{}", scene.to_vrml_string());
//! ```
//!
//! What it does not do: run events, routes or Scripts, load Inline/EXTERNPROTO content,
//! or render. Those parts of the port are not published yet.

pub mod fields;
pub mod parse;
pub mod scene;

pub use parse::{LoadError, ParseError, Parser, decode_text, gunzip, is_valid_vrml, load_file, parse_bytes};
pub use scene::Scene;
