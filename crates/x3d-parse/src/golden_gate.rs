//! GoldenGate: the parser registry of the pinned X_ITE 16.5.0, `src/x_ite/Parser/GoldenGate.js`.
//!
//! | GoldenGate.js | Rust |
//! | --- | --- |
//! | `static #parsers = [XMLParser, VRMLParser, JSONParser, GLTF2Parser, GLB2Parser, OBJParser, STLAParser, STLBParser, PLYAParser, PLYBParser, SVGParser]` (lines 19-31) | [`builtin::defaults`] |
//! | `static addParsers (... args)`: `push (... args)`, duplicates allowed (33-36) | [`GoldenGate::add_parsers`] |
//! | `static removeParsers (... args)`: `filter (parser => !args .includes (parser))`, so every copy goes (38-41) | [`GoldenGate::remove_parsers`] |
//! | `static getParsers ()`: `slice ()`, a copy (43-46) | [`GoldenGate::get_parsers`] |
//! | `parseIntoScene`: registration order is precedence, the first parser with a defined input that `isValid` wins (50-67) | [`ParserRegistry::parse_bytes_with`], [`ParserRegistry::parse_string_with`] |
//! | a parser's exception: `reject (error)` (70-77) | `Err` from [`GoldenGateParser::parse`] |
//! | no parser: `Couldn't parse X3D. No suitable file handler found for '<url>'.` (81-84) | [`LoadError::NoHandler`], text from [`no_handler_message`] |
//!
//! # Parser identity
//!
//! JavaScript compares the parser classes with `Array .includes`, which is `===`. A parser here is a
//! [`ParserRef`] (`Arc<dyn GoldenGateParser>`) and two are the same parser when they are the same
//! allocation ([`same_parser`], `Arc::ptr_eq` on the data pointer). A clone of a `ParserRef` is the same parser; a second
//! `Arc::new` of an equal value is a different one, as a second class with the same body is in JavaScript. The
//! built-in parsers are process-wide singletons ([`builtin::xml`] ...), so `removeParsers (XMLParser)` is
//! `GoldenGate::remove_parsers([&builtin::xml()])` (`remove_parsers` borrows, `add_parsers` takes the `Arc`):
//!
//! ```
//! use x3d_parse::{builtin, ParserRegistry};
//!
//! let mut registry = ParserRegistry::default();
//! let before = registry.get_parsers().len();
//!
//! registry.remove_parsers([&builtin::xml()]);
//! assert_eq!(registry.get_parsers().len(), before - 1);
//! registry.add_parsers([builtin::xml()]);
//! assert_eq!(registry.get_parsers().len(), before);
//! ```
//!
//! # Exceptions
//!
//! A JavaScript parser signals an error by throwing. Here a parser returns `Err`, and the dispatch stops with that error
//! (the later parsers are not tried). A panic is not an exception and is not caught.

use crate::{LoadError, ParseError, ParseOptions, dom, formats, gltf2, json, svg, xml};
use std::cell::OnceCell;
use std::sync::{Arc, LazyLock, PoisonError, RwLock};
use x3d_scene::Scene;

/// What GoldenGate hands a parser: the converted inputs of `GoldenGate.getInput` (lines 87-156), computed on first use
/// and shared by the built-in parsers of one call like `#inputs`. A parser outside this crate reads the public inputs
/// ([`ParserInput::string`] and [`ParserInput::bytes`]); the parsed DOM and JSON values are crate types and stay private.
pub struct ParserInput<'a> {
    data: &'a [u8],
    text: &'a str,
    world_url: &'a str,
    options: &'a ParseOptions,
    dom: OnceCell<Option<dom::Element>>,
    json: OnceCell<Option<json::JsValue>>,
}

impl<'a> ParserInput<'a> {
    pub(crate) fn new(
        data: &'a [u8],
        text: &'a str,
        world_url: &'a str,
        options: &'a ParseOptions,
    ) -> Self {
        Self {
            data,
            text,
            world_url,
            options,
            dom: OnceCell::new(),
            json: OnceCell::new(),
        }
    }

    /// `createInput ("STRING")` (105-116): `None` (JavaScript `undefined`) when non-empty bytes decode to nothing.
    pub fn string(&self) -> Option<&str> {
        (self.data.is_empty() || !self.text.is_empty()).then_some(self.text)
    }

    /// `createInput ("ARRAY_BUFFER")` (143-149): the bytes. A string input has none, which is an empty slice.
    pub fn bytes(&self) -> &[u8] {
        self.data
    }

    /// The `worldURL` of the scene being parsed.
    pub fn world_url(&self) -> &str {
        self.world_url
    }

    /// The per-call options.
    pub fn options(&self) -> &ParseOptions {
        self.options
    }

    /// `createInput ("XML")` (117-133): markup only; `None` when it does not parse. Whitespace-only text is handled by
    /// the XML parser itself.
    fn dom(&self) -> Option<&dom::Element> {
        self.dom
            .get_or_init(|| {
                let s = self.string()?;
                let trimmed = s.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');

                if !trimmed.starts_with('<') {
                    return None;
                }

                dom::parse_document(s).ok()
            })
            .as_ref()
    }

    /// `createInput ("JSON")` (134-142): `None` when `JSON.parse` throws.
    fn json(&self) -> Option<&json::JsValue> {
        self.json
            .get_or_init(|| json::json_parse(self.string()?).ok())
            .as_ref()
    }
}

/// One entry of the GoldenGate list (a parser class). `Ok(None)` is `getInput` returning `undefined` or `isValid ()`
/// returning false: the next parser is tried. `Ok(Some(scene))` wins. `Err` is a thrown exception: the dispatch
/// rejects with it.
pub trait GoldenGateParser: Send + Sync {
    /// A name for diagnostics. It takes no part in identity.
    fn name(&self) -> &str;

    /// Parse `input` into a new scene when this parser accepts it.
    fn parse(&self, input: &ParserInput<'_>) -> Result<Option<Scene>, LoadError>;
}

/// A registered parser; see the module documentation for identity.
pub type ParserRef = Arc<dyn GoldenGateParser>;

/// Whether two references are the same parser (JavaScript `===` on the class).
// The data pointers are compared, not `Arc::ptr_eq`: for a `dyn` pointer that also compares the vtable, which the
// compiler may duplicate or merge between codegen units (clippy `vtable_address_comparisons`), so one parser could
// compare unequal to itself.
pub fn same_parser(a: &ParserRef, b: &ParserRef) -> bool {
    std::ptr::eq(Arc::as_ptr(a) as *const (), Arc::as_ptr(b) as *const ())
}

/// GoldenGate.js lines 81-84: the text of the error when no parser handles a file. X3DScene.worldURL of a `data:` URL is
/// not named.
pub fn no_handler_message(world_url: &str) -> String {
    if world_url.starts_with("data:") {
        "Couldn't parse X3D. No suitable file handler found for 'data:' URL.".to_string()
    } else {
        format!("Couldn't parse X3D. No suitable file handler found for '{world_url}'.")
    }
}

/// An ordered parser list: GoldenGate's `#parsers`, usable on its own (a test, an embedding that wants its own list) and
/// the value behind the process-wide [`GoldenGate`].
#[derive(Clone)]
pub struct ParserRegistry {
    parsers: Vec<ParserRef>,
}

impl Default for ParserRegistry {
    /// The pinned default list, in order.
    fn default() -> Self {
        Self {
            parsers: builtin::defaults(),
        }
    }
}

impl ParserRegistry {
    /// A list with the pinned default parsers.
    pub fn new() -> Self {
        Self::default()
    }

    /// A list with no parser, which finds no handler for any input.
    pub fn empty() -> Self {
        Self { parsers: vec![] }
    }

    /// `addParsers`: append, in the given order. A parser already present is added again.
    pub fn add_parsers(&mut self, parsers: impl IntoIterator<Item = ParserRef>) {
        self.parsers.extend(parsers);
    }

    /// `removeParsers`: remove every entry that is the same parser as one of `parsers`. Parsers not present are ignored.
    pub fn remove_parsers<'p>(&mut self, parsers: impl IntoIterator<Item = &'p ParserRef>) {
        let args: Vec<&ParserRef> = parsers.into_iter().collect();

        self.parsers
            .retain(|p| !args.iter().any(|a| same_parser(p, a)));
    }

    /// `getParsers`: a copy of the list. Changing the copy does not change the registry.
    pub fn get_parsers(&self) -> Vec<ParserRef> {
        self.parsers.clone()
    }

    /// [`crate::parse_bytes_with`] against this list.
    pub fn parse_bytes_with(
        &self,
        data: &[u8],
        world_url: &str,
        options: &ParseOptions,
    ) -> Result<Scene, LoadError> {
        let data = crate::gunzip(data);
        let text = crate::decode_text(&data);

        self.parse_input(&data, &text, world_url, options)
    }

    /// [`crate::parse_string_with`] against this list.
    pub fn parse_string_with(
        &self,
        text: &str,
        world_url: &str,
        options: &ParseOptions,
    ) -> Result<Scene, LoadError> {
        self.parse_input(&[], text, world_url, options)
    }

    /// GoldenGate.parseIntoScene (48-85) over already converted input.
    pub(crate) fn parse_input(
        &self,
        data: &[u8],
        text: &str,
        world_url: &str,
        options: &ParseOptions,
    ) -> Result<Scene, LoadError> {
        let input = ParserInput::new(data, text, world_url, options);

        for parser in &self.parsers {
            if let Some(scene) = parser.parse(&input)? {
                return Ok(scene);
            }
        }

        // The VRML 1.0 extension parser (x_ite-vrml1-parser) is added to GoldenGate's list after the
        // others (GoldenGate.addParsers), only when the application registers it. It is not a
        // parser of the pinned X_ITE 16.5.0, which rejects `#VRML V1.0` without it.
        #[cfg(feature = "vrml1")]
        if options.vrml1
            && let Some(scene) = builtin::vrml1().parse(&input)?
        {
            return Ok(scene);
        }

        Err(LoadError::NoHandler)
    }
}

static REGISTRY: LazyLock<RwLock<ParserRegistry>> =
    LazyLock::new(|| RwLock::new(ParserRegistry::new()));

/// The process-wide registry, GoldenGate's static `#parsers`. [`crate::parse_bytes`], [`crate::parse_string`] and the
/// other entry points read it, so a parser added here is tried by every load in the process.
pub struct GoldenGate;

impl GoldenGate {
    /// `GoldenGate.addParsers`; see [`ParserRegistry::add_parsers`].
    pub fn add_parsers(parsers: impl IntoIterator<Item = ParserRef>) {
        REGISTRY
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .add_parsers(parsers);
    }

    /// `GoldenGate.removeParsers`; see [`ParserRegistry::remove_parsers`].
    pub fn remove_parsers<'p>(parsers: impl IntoIterator<Item = &'p ParserRef>) {
        REGISTRY
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .remove_parsers(parsers);
    }

    /// `GoldenGate.getParsers`; see [`ParserRegistry::get_parsers`].
    pub fn get_parsers() -> Vec<ParserRef> {
        REGISTRY
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get_parsers()
    }

    /// A copy of the process-wide registry, which a load runs against so no lock is held while a parser runs.
    pub(crate) fn snapshot() -> ParserRegistry {
        REGISTRY
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// The parsers of the pinned X_ITE 16.5.0, as singletons so they can be removed and re-added by identity.
pub mod builtin {
    use super::*;

    struct Builtin {
        name: &'static str,
        run: fn(&ParserInput<'_>) -> Result<Option<Scene>, LoadError>,
    }

    impl GoldenGateParser for Builtin {
        fn name(&self) -> &str {
            self.name
        }

        fn parse(&self, input: &ParserInput<'_>) -> Result<Option<Scene>, LoadError> {
            (self.run)(input)
        }
    }

    fn parse_err(message: String) -> LoadError {
        LoadError::Parse(ParseError { message, line: 0 })
    }

    fn run_xml(input: &ParserInput<'_>) -> Result<Option<Scene>, LoadError> {
        let Some(s) = input.string() else {
            return Ok(None);
        };

        // Blank input is `""`, which XMLParser accepts and turns into an empty scene.
        if s.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
            .is_empty()
        {
            return Ok(Some(xml::empty_scene(input.world_url)));
        }

        let Some(root) = input.dom() else {
            return Ok(None);
        };

        if root.name != "X3D" {
            return Ok(None);
        }

        let mut scene = xml::parse_x3d_element(root, input.world_url).map_err(LoadError::Parse)?;

        // DOMIntegration shows the element of an Inline's scene (`scene [_x3d]`): the page parses this text.
        scene.xml_source = Some(s.to_string());
        Ok(Some(scene))
    }

    fn run_vrml(input: &ParserInput<'_>) -> Result<Option<Scene>, LoadError> {
        match input.string() {
            Some(s) if crate::is_valid_vrml(s) => crate::Parser::new(s, input.world_url)
                .parse()
                .map(Some)
                .map_err(LoadError::Parse),
            _ => Ok(None),
        }
    }

    fn run_json(input: &ParserInput<'_>) -> Result<Option<Scene>, LoadError> {
        let Some(value) = input.json() else {
            return Ok(None);
        };

        if !json::is_valid(value) {
            return Ok(None);
        }

        let root = json::to_x3d_element(value).map_err(parse_err)?;
        let mut scene = xml::parse_x3d_element(&root, input.world_url).map_err(LoadError::Parse)?;

        scene.encoding = "JSON".into();
        Ok(Some(scene))
    }

    fn run_gltf2(input: &ParserInput<'_>) -> Result<Option<Scene>, LoadError> {
        match input.json() {
            Some(value) if gltf2::is_valid_gltf(value) => gltf2::parse_gltf(value, input.world_url)
                .map(Some)
                .map_err(parse_err),
            _ => Ok(None),
        }
    }

    // GLBParser (container versions 2 and 3)
    fn run_glb2(input: &ParserInput<'_>) -> Result<Option<Scene>, LoadError> {
        if gltf2::is_valid_glb(input.data) {
            return gltf2::parse_glb(input.data, input.world_url)
                .map(Some)
                .map_err(parse_err);
        }

        Ok(None)
    }

    fn run_obj(input: &ParserInput<'_>) -> Result<Option<Scene>, LoadError> {
        match input.string() {
            Some(s) if formats::is_obj(s) => formats::parse_obj(s, input.world_url).map(Some),
            _ => Ok(None),
        }
    }

    fn run_stla(input: &ParserInput<'_>) -> Result<Option<Scene>, LoadError> {
        match input.string() {
            Some(s) if formats::is_stl_ascii(s) => {
                formats::parse_stl_ascii(s, input.world_url).map(Some)
            }
            _ => Ok(None),
        }
    }

    fn run_stlb(input: &ParserInput<'_>) -> Result<Option<Scene>, LoadError> {
        if formats::is_stl_binary(input.data) {
            return formats::parse_stl_binary(input.data, input.world_url).map(Some);
        }

        Ok(None)
    }

    fn run_plya(input: &ParserInput<'_>) -> Result<Option<Scene>, LoadError> {
        match input.string() {
            Some(s) if formats::is_ply_ascii(s) => {
                formats::parse_ply_ascii(s, input.world_url).map(Some)
            }
            _ => Ok(None),
        }
    }

    fn run_plyb(input: &ParserInput<'_>) -> Result<Option<Scene>, LoadError> {
        if formats::is_ply_binary(input.data) {
            return formats::parse_ply_binary(input.data, input.world_url).map(Some);
        }

        Ok(None)
    }

    // SVGParser: the document element must be <svg>.
    fn run_svg(input: &ParserInput<'_>) -> Result<Option<Scene>, LoadError> {
        match input.dom() {
            Some(root) if svg::is_valid(root) => svg::parse_svg(root, input.world_url).map(Some),
            _ => Ok(None),
        }
    }

    #[cfg(feature = "vrml1")]
    fn run_vrml1(input: &ParserInput<'_>) -> Result<Option<Scene>, LoadError> {
        match input.string() {
            Some(s) if x3d_vrml1::is_vrml1(s) => x3d_vrml1::parse_str(s, input.world_url)
                .map(Some)
                .map_err(|e| {
                    LoadError::Parse(ParseError {
                        message: format!("{} (column {})", e.message, e.column),
                        line: e.line,
                    })
                }),
            _ => Ok(None),
        }
    }

    struct All {
        list: Vec<ParserRef>,
        #[cfg(feature = "vrml1")]
        vrml1: ParserRef,
    }

    static ALL: LazyLock<All> = LazyLock::new(|| {
        let p = |name, run| -> ParserRef { Arc::new(Builtin { name, run }) };

        All {
            list: vec![
                p("XMLParser", run_xml),
                p("VRMLParser", run_vrml),
                p("JSONParser", run_json),
                p("GLTF2Parser", run_gltf2),
                p("GLB2Parser", run_glb2),
                p("OBJParser", run_obj),
                p("STLAParser", run_stla),
                p("STLBParser", run_stlb),
                p("PLYAParser", run_plya),
                p("PLYBParser", run_plyb),
                p("SVGParser", run_svg),
            ],
            #[cfg(feature = "vrml1")]
            vrml1: p("VRML1Parser", run_vrml1),
        }
    });

    /// GoldenGate.js lines 19-31, in order.
    pub fn defaults() -> Vec<ParserRef> {
        ALL.list.clone()
    }

    /// `XMLParser`.
    pub fn xml() -> ParserRef {
        ALL.list[0].clone()
    }

    /// `VRMLParser`.
    pub fn vrml() -> ParserRef {
        ALL.list[1].clone()
    }

    /// `JSONParser`.
    pub fn json() -> ParserRef {
        ALL.list[2].clone()
    }

    /// `GLTF2Parser`.
    pub fn gltf2() -> ParserRef {
        ALL.list[3].clone()
    }

    /// `GLB2Parser`.
    pub fn glb2() -> ParserRef {
        ALL.list[4].clone()
    }

    /// `OBJParser`.
    pub fn obj() -> ParserRef {
        ALL.list[5].clone()
    }

    /// `STLAParser`.
    pub fn stla() -> ParserRef {
        ALL.list[6].clone()
    }

    /// `STLBParser`.
    pub fn stlb() -> ParserRef {
        ALL.list[7].clone()
    }

    /// `PLYAParser`.
    pub fn plya() -> ParserRef {
        ALL.list[8].clone()
    }

    /// `PLYBParser`.
    pub fn plyb() -> ParserRef {
        ALL.list[9].clone()
    }

    /// `SVGParser`.
    pub fn svg() -> ParserRef {
        ALL.list[10].clone()
    }

    /// The VRML 1.0 extension parser (not part of the pin). It is tried last, after the registry, for a call with
    /// [`ParseOptions::vrml1`] set; it is not in the default list.
    #[cfg(feature = "vrml1")]
    pub fn vrml1() -> ParserRef {
        ALL.vrml1.clone()
    }
}
