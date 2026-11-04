//! Shared by the viewer test binaries. Each binary that says `mod common;` compiles its own copy.
#![allow(dead_code)]

pub mod gpu;
pub mod reference;
pub mod ulps;
