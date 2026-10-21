//! Text and FontStyle for the X3D renderer, ported from X_ITE (MIT, Copyright (c) 2009 Holger
//! Seelig and other contributors; pinned reference oracle/x_ite-pin): font loading
//! (`Browser/Text/X3DTextContext.js`, opentype.js), glyph outlines and libtess triangulation
//! (`Browser/Text/PolygonText.js`, libtess.js) and layout (`Browser/Text/X3DTextGeometry.js`).

pub mod font;
pub mod fonts;
pub mod layout;
pub mod libtess;
pub mod raster;
pub mod timsort;

pub use font::{Font, FontError, PrimitiveQuality};
pub use layout::{Alignment, FontStyleParams, TextCompression, TextLayout, TextParams, layout};
