//! FontLibrary (Components/Text/FontLibrary.js, X3D 4.1, experimental) and the library table of
//! Browser/Text/X3DTextContext.js (`registerFontLibrary`, `getFont`). REA batch 08, lane net.
//!
//! * `url`: every entry is resolved against the node's execution context and loaded with `browser .loadFont`
//!   (one FileLoader per entry, cached per url); the first font that loads wins. No font: FAILED. A file that
//!   does not parse leaves the promise pending for good (the node stays IN_PROGRESS, nothing is registered).
//! * `family`: the registered name is the `family` field (the font file's own name is never read), upper cased.
//!   `set_family__` runs when a font arrives and whenever `family` changes; an empty family or no font registers
//!   nothing. Nothing is ever unregistered: a changed `family` leaves the old name, a failed reload leaves the old
//!   font, a removed node leaves its entry.
//! * The table belongs to the execution context of the FontLibrary (the world, the scene an Inline loaded, the body
//!   of a PROTO instance): a FontStyle finds only the fonts registered in its own context.
//! * A FontStyle looks the family up in X3DFontStyleNode.loadData, after the default families and before the
//!   deprecated font-file urls ([`Runtime::font_library_font`], read by the renderer). X_ITE waits there for
//!   FontLibrary nodes that are still loading; all loads are over by the time a frame is drawn, so no waiting is
//!   modeled.

use crate::loader::FileLoader;
use crate::nodes::LoadState;
use crate::*;
use std::rc::Rc;

/// A FontStyle load loop that waits (X3DTextContext.getFont) for a FontLibrary that is still loading.
#[derive(Clone, Debug)]
pub struct PendingFontStyle {
    pub node: NodeId,
    pub family: Vec<String>,
    /// The entry of `family` that is looked up when the loop goes on.
    pub index: usize,
    pub style: String,
}

/// `OpenType.parse (await decompressFont (data))`: WOFF 1.0 (opentype.js unpacks it), WOFF2 and SFNT.
pub(crate) fn parse_font_file(data: &[u8]) -> Result<x3d_text::Font, x3d_text::FontError> {
    match x3d_parse::woff1::woff1_to_sfnt(data) {
        Some(sfnt) => x3d_text::Font::parse(&sfnt),
        None => Err(x3d_text::FontError::Parse("woff: cannot unpack".into())),
    }
}

/// The text of opentype.js's error for a file it cannot parse (only the signature error is reproduced).
pub(crate) fn font_error_text(data: &[u8]) -> String {
    let tag: String = data.iter().take(4).map(|b| *b as char).collect();

    format!("Error: Unsupported OpenType signature {tag}")
}

impl Runtime {
    /// FontLibrary.initialize: X3DUrlObject.initialize, `family` interest, `requestImmediateLoad ()`.
    pub(crate) fn font_library_initialize(&mut self, node: NodeId) {
        self.interest(node, "family", Cb::FontLibrarySetFamily);
        self.inline_initialize(node);
    }

    /// `browser .loadFont (fileURL, node)`: the font, or `None` for a failed fetch; `Err (())` when the file does not
    /// parse (the promise never resolves).
    fn font_library_load_font(
        &mut self,
        node: NodeId,
        file_url: &url::Url,
    ) -> Result<Option<Rc<x3d_text::Font>>, ()> {
        let href = file_url.as_str().to_string();
        let cached =
            !file_url.query().is_some_and(|q| !q.is_empty()) && self.font_cache.contains_key(&href);

        if !cached {
            let loader = FileLoader {
                type_name: self.type_name(node),
                base: href.clone(),
                data_as_string: false,
            };
            let mut parsed: Option<Result<Rc<x3d_text::Font>, x3d_text::FontError>> = None;
            let ok = self.load_document(&loader, std::slice::from_ref(&href), |_, loaded| {
                if let crate::loader::Loaded::Data(data, _) = loaded {
                    // `OpenType .parse` throws inside the loader's callback: loadDocumentError, next url.
                    match parse_font_file(&data) {
                        Ok(font) => parsed = Some(Ok(Rc::new(font))),
                        Err(_) => return Err(font_error_text(&data)),
                    }
                }

                Ok(())
            });

            if let Some(result) = parsed {
                x3d_text::fonts::register_url_font(&href, result.ok());
            }

            if !ok && file_url.scheme() != "data" {
                let decoded = crate::loader::decode_uri(&href).unwrap_or_else(|| href.clone());
                self.log_warn(format!("Error loading font '{decoded}':"));
            }

            if file_url.query().is_none_or(|q| q.is_empty()) {
                self.font_cache.insert(href.clone(), ok);
            }

            if !ok {
                return Ok(None);
            }
        } else if !self.font_cache.get(&href).copied().unwrap_or(false) {
            return Ok(None);
        }

        match x3d_text::fonts::url_font(&href) {
            Some(Some(font)) => Ok(Some(font)),
            Some(None) => Err(()),
            None => Ok(None),
        }
    }

    /// FontLibrary.loadData. With an asynchronous [`crate::Fetcher`] each `await browser .loadFont (fileURL)` waits
    /// for its response: the loop stops there and goes on from that file when it is in.
    pub(crate) fn font_library_load(&mut self, node: NodeId) {
        let Ok(base) = url::Url::parse(&self.base_url(node)) else {
            return;
        };
        let candidates = self.url_strings(node);
        // `Array .from (this ._url) .map (fileURL => new URL (fileURL, base))`: one bad url rejects the whole load.
        let mut urls = vec![];

        for u in &candidates {
            match crate::loader::join_url(&base, u) {
                Ok(r) => urls.push(r),
                Err(_) => return,
            }
        }

        self.allow_fetch_wait();

        let wait = self.fetch_wait.take();
        let start = wait
            .as_ref()
            .and_then(|k| self.fetch_resume_of(k, &candidates))
            .map_or(0, |r| r.index);
        let mut font = None;

        for (index, file_url) in urls.into_iter().enumerate().skip(start) {
            if self.fetch_waits(wait.as_ref(), &candidates, index, &file_url) {
                return;
            }

            match self.font_library_load_font(node, &file_url) {
                Ok(Some(f)) => {
                    font = Some(f);
                    break;
                }
                Ok(None) => {}
                // A font file that does not parse: this load never finishes.
                Err(()) => return,
            }
        }

        let failed = font.is_none();

        self.font_library_fonts.insert(node, font);
        self.font_library_set_family(node);
        self.set_load_state(
            node,
            if failed {
                LoadState::Failed
            } else {
                LoadState::Complete
            },
        );
        self.font_styles_resume();
    }

    /// `this .font = font` of X3DFontStyleNode.loadData for a font a FontLibrary registered (`None`: the load ended with a
    /// default font, a font file, or no font, which the renderer finds itself).
    pub(crate) fn font_style_chose(&mut self, node: NodeId, font: Option<Rc<x3d_text::Font>>) {
        self.font_style_done(node, font.clone());

        let same = match (&font, self.font_style_fonts.get(&node)) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };

        if same {
            return;
        }

        match font {
            Some(f) => self.font_style_fonts.insert(node, f),
            None => self.font_style_fonts.remove(&node),
        };
        self.font_epoch += 1;
    }

    /// The end of X3DFontStyleNode.loadData: `this .font = font`, `this .addNodeEvent ()` (its Texts rebuild).
    fn font_style_done(&mut self, node: NodeId, library: Option<Rc<x3d_text::Font>>) {
        let font = library.or_else(|| self.font_style_loaded_font(node));

        self.font_style_font.insert(node, font);
        self.add_node_event(node);
    }

    /// The font a FontStyle's last load took from a FontLibrary.
    pub fn font_style_library_font(&self, node: NodeId) -> Option<Rc<x3d_text::Font>> {
        self.font_style_fonts.get(&node).cloned()
    }

    /// Is a FontLibrary loading (`getLoadingObjects ()` filtered by FontLibrary)?
    pub(crate) fn font_library_loading(&self) -> bool {
        (0..self.scene.nodes.len() as NodeId).any(|n| {
            self.builtin_name(n) == Some("FontLibrary")
                && self.url_load_state(n) == LoadState::InProgress
        })
    }

    /// A FontLibrary load ended: the FontStyle loops that waited for it look again.
    fn font_styles_resume(&mut self) {
        for p in std::mem::take(&mut self.pending_font_styles) {
            if self.scene.is_live(p.node) {
                self.font_style_continue(p.node, p.family, p.index, p.style);
            }
        }
    }

    /// FontLibrary.set_family__: `registerFontLibrary (executionContext, familyName, font)`.
    pub(crate) fn font_library_set_family(&mut self, node: NodeId) {
        let Some(Some(font)) = self.font_library_fonts.get(&node).cloned() else {
            return;
        };
        let family = match self.pre_value(node, "family") {
            Some(FieldValue::SFString(s)) => s.clone(),
            _ => String::new(),
        };

        if family.is_empty() {
            return;
        }

        let ctx = self.scene.node(node).ctx;

        self.font_libraries
            .entry(ctx)
            .or_default()
            .insert(family.to_uppercase(), font);
        self.font_epoch += 1;
    }

    /// X3DTextContext.getFont (without the wait): the font a FontLibrary of `ctx` registered under `family`.
    pub fn font_library_font(&self, ctx: CtxId, family: &str) -> Option<Rc<x3d_text::Font>> {
        self.font_libraries
            .get(&ctx)?
            .get(&family.to_uppercase())
            .cloned()
    }
}
