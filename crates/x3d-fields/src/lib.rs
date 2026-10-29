//! X3D/VRML field types, math and number formatting, ported from X_ITE
//! (https://github.com/create3000/x_ite, MIT, Copyright (c) 2009 Holger Seelig and other
//! contributors). Pinned reference: oracle/x_ite-pin (fork commit 34cdf9f97).
//!
//! Semantics follow X_ITE exactly, including quirks; see the module docs.

pub mod fdlibm;
pub mod format;
pub mod generator;
pub mod jsmath;
pub mod math;
pub mod types;
pub mod value;

pub use generator::{Generator, Style};
pub use types::{FieldType, access};
pub use value::{FieldValue, Image, NodeId};
pub mod url;
