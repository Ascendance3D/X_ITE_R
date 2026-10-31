//! X3D Script node engine, ported from X_ITE
//! (https://github.com/create3000/x_ite, MIT, Copyright (c) 2009 Holger Seelig and other
//! contributors). Pinned reference: oracle/x_ite-pin (fork commit 34cdf9f97).
//!
//! [`sai`] is the Script implementation, independent of the JavaScript engine. [`QuickJsHost`] (feature `quickjs`, on by
//! default) runs it on QuickJS for the native viewer; the browser build (x3d-web) runs it in the page's own engine.

#[cfg(feature = "quickjs")]
pub mod compare;
#[cfg(feature = "quickjs")]
pub mod engine;
pub mod sai;

#[cfg(feature = "quickjs")]
pub use engine::QuickJsHost;
