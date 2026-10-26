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
//!
//! This crate is a facade. Every item is a re-export of the engine crates `x3d-fields`,
//! `x3d-scene` and `x3d-parse`, so a `vrml97` type is the engine type itself.

pub use x3d_fields as fields;
pub use x3d_scene as scene;

/// The classic-encoding parser. `x3d_parse::compare` (an oracle test helper) stays private.
pub mod parse {
    pub use x3d_parse::{
        LoadError, ParseError, Parser, color, decode_text, gunzip, is_valid_vrml, lexer, load_file,
        parse_bytes, parser,
    };
}

pub use parse::{
    LoadError, ParseError, Parser, decode_text, gunzip, is_valid_vrml, load_file, parse_bytes,
};
pub use scene::Scene;

const  _CI_ACCEPTANCE:u8=0;
