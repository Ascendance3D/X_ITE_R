//! X3D Script node engine on QuickJS (rquickjs), ported from X_ITE
//! (https://github.com/create3000/x_ite, MIT, Copyright (c) 2009 Holger Seelig and other
//! contributors). Pinned reference: oracle/x_ite-pin (fork commit 34cdf9f97).

pub mod compare;
pub mod engine;

pub use engine::QuickJsHost;
