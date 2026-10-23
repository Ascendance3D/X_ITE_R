//! X_ITE's own fonts (`assets/fonts`), selected like `X3DFontStyleNode.getDefaultFont`.
//! Embedded so that every binary of the workspace finds them.

use crate::font::Font;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

macro_rules! font {
    ($path:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../oracle/x_ite-pin/src/assets/fonts/",
            $path
        ))
    };
}

/// The `Fonts` map of X3DFontStyleNode.js: family → style → font file. Unknown families give
/// None; an unknown style falls back to PLAIN (`family ?.get (fontStyle) ?? family ?.get ("PLAIN")`).
pub fn default_font_file(family: &str, style: &str) -> Option<(&'static str, &'static [u8])> {
    let table: [(&str, &[u8]); 4] = match family {
        "SERIF" => [
            (
                "Droid/DroidSerif-Regular.woff2",
                font!("Droid/DroidSerif-Regular.woff2"),
            ),
            (
                "Droid/DroidSerif-Italic.woff2",
                font!("Droid/DroidSerif-Italic.woff2"),
            ),
            (
                "Droid/DroidSerif-Bold.woff2",
                font!("Droid/DroidSerif-Bold.woff2"),
            ),
            (
                "Droid/DroidSerif-BoldItalic.woff2",
                font!("Droid/DroidSerif-BoldItalic.woff2"),
            ),
        ],
        "SANS" => [
            ("Ubuntu/Ubuntu-R.woff2", font!("Ubuntu/Ubuntu-R.woff2")),
            ("Ubuntu/Ubuntu-RI.woff2", font!("Ubuntu/Ubuntu-RI.woff2")),
            ("Ubuntu/Ubuntu-B.woff2", font!("Ubuntu/Ubuntu-B.woff2")),
            ("Ubuntu/Ubuntu-BI.woff2", font!("Ubuntu/Ubuntu-BI.woff2")),
        ],
        "TYPEWRITER" => [
            (
                "Ubuntu/UbuntuMono-R.woff2",
                font!("Ubuntu/UbuntuMono-R.woff2"),
            ),
            (
                "Ubuntu/UbuntuMono-RI.woff2",
                font!("Ubuntu/UbuntuMono-RI.woff2"),
            ),
            (
                "Ubuntu/UbuntuMono-B.woff2",
                font!("Ubuntu/UbuntuMono-B.woff2"),
            ),
            (
                "Ubuntu/UbuntuMono-BI.woff2",
                font!("Ubuntu/UbuntuMono-BI.woff2"),
            ),
        ],
        _ => return None,
    };

    let index = match style {
        "PLAIN" => 0,
        "ITALIC" => 1,
        "BOLD" => 2,
        "BOLDITALIC" => 3,
        _ => 0,
    };

    Some(table[index])
}

thread_local! {
    static CACHE: RefCell<HashMap<&'static str, Option<Rc<Font>>>> = RefCell::new(HashMap::new());
}

/// browser.loadFont for one of X_ITE's default font files (cached per file, like `_fontCache`).
pub fn load_default_font(family: &str, style: &str) -> Option<Rc<Font>> {
    let (name, data) = default_font_file(family, style)?;

    CACHE.with(|c| {
        c.borrow_mut()
            .entry(name)
            .or_insert_with(|| Font::parse(data).ok().map(Rc::new))
            .clone()
    })
}

/// The default-font part of X3DFontStyleNode.loadData: every family name (plus the implicit
/// "SERIF") is tried in order; the first X_ITE default family wins. Returns None when no
/// family is a default one (FontLibrary lookups and font URLs are the caller's business).
pub fn select_default_font(family: &[String], style: &str) -> Option<Rc<Font>> {
    for f in family
        .iter()
        .map(String::as_str)
        .chain(std::iter::once("SERIF"))
    {
        if default_font_file(f, style).is_some() {
            return load_default_font(f, style);
        }
    }

    None
}

thread_local! {
    static URL_FONTS: RefCell<HashMap<String, Option<Rc<Font>>>> = RefCell::new(HashMap::new());
}

/// browser.loadFont for a font file named by a FontStyle `family` entry: x3d-runtime registers the
/// outcome per absolute URL (`Some (font)`, or `None` for data that does not parse, which leaves
/// the X_ITE promise pending forever); the renderer reads it back with [`url_font`].
pub fn register_url_font(href: &str, font: Option<Rc<Font>>) {
    URL_FONTS.with(|c| {
        c.borrow_mut().insert(href.to_string(), font);
    });
}

/// `None`: never loaded (failed fetch); `Some (None)`: loaded but unparsable; `Some (Some (font))`.
pub fn url_font(href: &str) -> Option<Option<Rc<Font>>> {
    URL_FONTS.with(|c| c.borrow().get(href).cloned())
}
