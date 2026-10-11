//! Minimal X3D scene model and VRML generator, ported from X_ITE
//! (https://github.com/create3000/x_ite, MIT, Copyright (c) 2009 Holger Seelig and other
//! contributors). Pinned reference: oracle/x_ite-pin (fork commit 34cdf9f97).

pub mod info_array;
pub mod nodes;
pub mod scene;
pub mod setup;
pub mod vrml;

pub use nodes::{NodeTable, NodeType, table};
pub use scene::*;
