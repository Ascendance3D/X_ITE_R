//! Text's output fields: X3DGeometryNode's rebuild cascade (Components/Rendering/X3DGeometryNode.js `setup`, `initialize`,
//! `requestRebuild`, `rebuild`), Text.js (`set_live__`, `set_fontStyle__`, `build`) and the fields X3DTextGeometry.configure /
//! horizontal / vertical (Browser/Text/X3DTextGeometry.js) give `origin`, `textBounds` and `lineBounds`. The layout itself is
//! the x3d-text port the renderer uses; the renderer builds the geometry on its own.
//!
//! A font is the one the FontStyle's last loadData chose (X3DFontStyleNode.font, kept until the next load completes); the
//! browser's default FontStyle loads its font once, with the steps of any other loadData (`await $.sleep (0)`, then `loadFont`).
//! It is a node of the browser's private scene, so the world's load does not wait for it: a Text that uses it builds once
//! without a font in the first frame, as in every X_ITE run (oracle/synthetic-rea-b11-text/warm-stamp.wrl).

use crate::*;
use std::rc::Rc;
use x3d_text::{Font, FontStyleParams, PrimitiveQuality, TextCompression, TextParams};

/// The font file of the default FontStyle (getDefaultFont ("SERIF", "PLAIN")), the key of browser.loadFont's cache.
const DEFAULT_FONT_FILE: &str = "Droid/DroidSerif-Regular.woff2";

/// The browser's default FontStyle (X3DTextContext.getDefaultFontStyle): its font is loading or loaded.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum DefaultFont {
    #[default]
    NotStarted,
    Loading,
    Loaded,
}

/// An X3DFontStyleNode.loadData run that has not chosen its font: the family (with "SERIF" appended) and style it copied
/// when it began, and how far it got.
#[derive(Debug, Clone)]
pub struct FontStyleLoad {
    pub node: NodeId,
    pub family: Vec<String>,
    pub style: String,
    /// The `_loadData` event of a field change (several changes in one cascade load once) rather than the load of setup.
    pub reload: bool,
    pub stage: LoadStage,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum LoadStage {
    #[default]
    /// `await $.sleep (0)`: the timer fires at the start of the next frame.
    Timer,
    /// The timer fired; the continuation runs after that frame (a microtask of the timer).
    Fired,
    /// browser.loadFont fetches a font file that is not cached: it arrives between two frames.
    Fetch,
}

impl Runtime {
    /// X3DFontStyleNode: requestImmediateLoad → loadData (setup) or `_loadData` (a change of `family` or `style`). The load
    /// of setup takes the same steps (its `sleep (0)` timer fires with the first frame): the world's load does not wait for
    /// it, so a Text builds once without a font in the first frame, as in every X_ITE run
    /// (oracle/synthetic-rea-b11-text/warm-stamp-fs.wrl).
    pub(crate) fn font_style_request(&mut self, node: NodeId, reload: bool) {
        let (family, style) = self.font_style_families(node);

        if reload {
            if let Some(l) = self.font_style_loads.iter_mut().find(|l| l.node == node && l.reload && l.stage == LoadStage::Timer) {
                l.family = family;
                l.style = style;
                return;
            }
        }

        self.font_style_loads.push(FontStyleLoad { node, family, style, reload, stage: LoadStage::Timer });
    }

    /// `this ._family .copy ()` with "SERIF" pushed, and `this ._style .getValue ()`.
    pub(crate) fn font_style_families(&self, node: NodeId) -> (Vec<String>, String) {
        let style = match self.pre_value(node, "style") {
            Some(FieldValue::SFString(s)) => s.clone(),
            _ => String::new(),
        };
        let mut family = match self.pre_value(node, "family") {
            Some(FieldValue::MFString(v)) => v.clone(),
            _ => vec![],
        };

        family.push("SERIF".into());
        (family, style)
    }

    /// Is a FontStyle (or the default FontStyle) still loading its font? x_ite-shot.mjs waits for these during its settle time.
    pub fn fonts_loading(&self) -> bool {
        !self.font_style_loads.is_empty() || !self.pending_font_styles.is_empty() || self.default_font == DefaultFont::Loading
    }

    /// A harness without frames (the corpus dump) waits for the font loads as the oracle page does: the `sleep (0)` timers fire,
    /// the fetches complete, the warnings of failed families are logged.
    pub fn finish_font_loads(&mut self) {
        for _ in 0..4 {
            if !self.fonts_loading() {
                break;
            }

            for l in &mut self.font_style_loads {
                if l.stage == LoadStage::Timer {
                    l.stage = LoadStage::Fired;
                }
            }

            if self.default_font == DefaultFont::Loading && self.default_font_stage == LoadStage::Timer {
                self.default_font_stage = LoadStage::Fired;
            }

            self.font_style_continuations();
            self.run_tasks();
        }
    }

    /// The start of a frame (the harness runs the due timers first): the `sleep (0)` timers fire.
    pub(crate) fn font_style_timers(&mut self) {
        self.frames_started = true;
        self.content_scale_fire();

        for l in &mut self.font_style_loads {
            if l.stage == LoadStage::Timer {
                l.stage = LoadStage::Fired;
            }
        }

        if self.default_font == DefaultFont::Loading && self.default_font_stage == LoadStage::Timer {
            self.default_font_stage = LoadStage::Fired;
        }
    }

    /// After a frame: the loads whose timer fired go on (a cached font file completes now, another one is fetched), and the
    /// fetches started after the frame before complete. The node event they end with is processed in the next frame.
    pub(crate) fn font_style_continuations(&mut self) {
        let loads = std::mem::take(&mut self.font_style_loads);
        let mut keep = vec![];

        for mut l in loads {
            match l.stage {
                LoadStage::Timer => keep.push(l),
                LoadStage::Fired => {
                    let file = l.family.iter().find_map(|f| x3d_text::fonts::default_font_file(f, &l.style).map(|(name, _)| name.to_string()));

                    match file {
                        Some(key) if !self.loaded_font_files.contains(&key) => {
                            self.loaded_font_files.insert(key);
                            l.stage = LoadStage::Fetch;
                            keep.push(l);
                        }
                        _ => self.font_style_run(l),
                    }
                }
                LoadStage::Fetch => self.font_style_run(l),
            }
        }

        keep.extend(std::mem::take(&mut self.font_style_loads));
        self.font_style_loads = keep;

        // The default FontStyle (family SERIF, style PLAIN) takes the same steps.
        if self.default_font == DefaultFont::Loading {
            match self.default_font_stage {
                LoadStage::Timer => {}
                LoadStage::Fired if !self.loaded_font_files.contains(DEFAULT_FONT_FILE) => {
                    self.loaded_font_files.insert(DEFAULT_FONT_FILE.into());
                    self.default_font_stage = LoadStage::Fetch;
                }
                LoadStage::Fired | LoadStage::Fetch => self.default_font_loaded(),
            }
        }
    }

    /// The rest of loadData with the families it copied: the font is chosen (X3DFontStyleNode.font, addNodeEvent).
    fn font_style_run(&mut self, l: FontStyleLoad) {
        if !self.scene.is_live(l.node) {
            return;
        }

        self.font_style_snapshot.insert(l.node, (l.family.clone(), l.style.clone()));
        self.font_style_continue(l.node, l.family, 0, l.style);
    }

    /// Text constructor: X3DGeometryNode's `_rebuild` (outputOnly SFTime).
    pub(crate) fn text_construct(&mut self, node: NodeId) {
        self.add_private_field(node, "rebuild", access::OUTPUT_ONLY, FieldValue::SFTime(0.0));
    }

    /// X3DGeometryNode.initialize, then Text.initialize.
    pub(crate) fn text_initialize(&mut self, node: NodeId) {
        // `this .getLive () .addInterest ("set_live__", this)`
        self.watch_live(node);
        // `this .addInterest ("requestRebuild", this)`, `this ._rebuild .addInterest ("rebuild", this)`
        self.add_node_interest(node, node, Cb::TextRequestRebuild);

        if let Some(k) = self.private_field(node, "rebuild") {
            self.add_interest(k, node, Cb::TextRebuild);
        }

        self.text_set_live(node);
        // Text.initialize: `this ._fontStyle .addInterest ("set_fontStyle__", this)`, `this .set_fontStyle__ ()`.
        self.interest(node, "fontStyle", Cb::TextSetFontStyle);
        self.text_set_font_style(node);
    }

    /// Text.set_live__: a live Text asks for a rebuild.
    pub(crate) fn text_set_live(&mut self, node: NodeId) {
        if self.time_node_live(node) {
            self.text_request_rebuild(node);
        }
    }

    /// ScreenFontStyle.initialize: `getRenderingProperties () ._ContentScale .addInterest ("addNodeEvent", this)`, so a changed
    /// ContentScale makes every ScreenFontStyle's Texts rebuild (their size is in pixels × ContentScale). X_ITE's Texts report the
    /// new origin, textBounds and lineBounds in the frame after the one that applied the option (text-screen-scale).
    pub(crate) fn content_scale_events(&mut self) {
        if std::mem::take(&mut self.browser_options.content_scale_changed) {
            self.content_scale_armed = true;
        }
    }

    /// The start of a frame: the ContentScale change of the frame before reaches the ScreenFontStyles.
    fn content_scale_fire(&mut self) {
        if !std::mem::take(&mut self.content_scale_armed) {
            return;
        }

        let styles: Vec<NodeId> = (0..self.scene.nodes.len() as NodeId).filter(|&n| self.builtin_name(n) == Some("ScreenFontStyle")).collect();

        for n in styles {
            self.add_node_event(n);
        }
    }

    /// X3DGeometryNode.requestRebuild: `this ._rebuild = Date .now () / 1000`.
    pub(crate) fn text_request_rebuild(&mut self, node: NodeId) {
        if let Some(k) = self.private_field(node, "rebuild") {
            self.set_value(k, FieldValue::SFTime(self.now));
        }
    }

    /// The X3DFontStyleNode of `fontStyle` (X3DCast), or None for the browser's default FontStyle.
    fn text_font_style_node(&self, node: NodeId) -> Option<NodeId> {
        let fs = match self.pre_value(node, "fontStyle") {
            Some(FieldValue::SFNode(Some(n))) => self.inner_node(*n),
            _ => None,
        }?;

        matches!(self.builtin_name(fs), Some("FontStyle" | "ScreenFontStyle")).then_some(fs)
    }

    /// Text.set_fontStyle__: the interest moves to the new font style node.
    pub(crate) fn text_set_font_style(&mut self, node: NodeId) {
        match self.text_font_styles.get(&node).copied() {
            Some(Some(old)) => self.remove_node_interest(old, node, Cb::TextRequestRebuild),
            Some(None) => self.default_font_texts.retain(|&t| t != node),
            None => {}
        }

        let fs = self.text_font_style_node(node);

        self.text_font_styles.insert(node, fs);

        match fs {
            Some(f) => self.add_node_interest(f, node, Cb::TextRequestRebuild),
            None => {
                self.default_font_texts.push(node);

                // getDefaultFontStyle: the FontStyle is set up (requestImmediateLoad) the first time a Text asks for it.
                if self.default_font == DefaultFont::NotStarted {
                    self.default_font = DefaultFont::Loading;
                    self.default_font_stage = LoadStage::Timer;
                }
            }
        }
    }

    /// The default FontStyle's loadData completed: `this .addNodeEvent ()`, so its Texts ask for a rebuild.
    fn default_font_loaded(&mut self) {
        self.default_font = DefaultFont::Loaded;

        for node in self.default_font_texts.clone() {
            self.text_request_rebuild(node);
        }
    }

    /// X3DFontStyleNode.loadData completed for `node` without a FontLibrary font: the default family or font file its
    /// families name (`this .font`).
    pub(crate) fn font_style_loaded_font(&self, node: NodeId) -> Option<Rc<Font>> {
        let (family, style) = match self.font_style_snapshot.get(&node) {
            Some(s) => s.clone(),
            None => self.font_style_families(node),
        };

        let base = url::Url::parse(&self.base_url(node)).ok();

        for f in &family {
            if x3d_text::fonts::default_font_file(f, &style).is_some() {
                return x3d_text::fonts::load_default_font(f, &style);
            }

            let file_url = base.as_ref().and_then(|b| b.join(f).ok())?;
            let path = file_url.path().to_ascii_lowercase();

            if file_url.scheme() == "data" || [".woff2", ".woff", ".otf", ".ttf"].iter().any(|e| path.ends_with(e)) {
                match x3d_text::fonts::url_font(file_url.as_str()) {
                    Some(Some(font)) => return Some(font),
                    Some(None) => return None,
                    None => continue,
                }
            }
        }

        None
    }

    /// `fontStyle .getFont ()`: None while the font style has not loaded a font.
    fn text_font(&self, node: NodeId) -> Option<Rc<Font>> {
        match self.text_font_styles.get(&node).copied().flatten() {
            Some(fs) => self.font_style_font.get(&fs).cloned().flatten(),
            None => (self.default_font == DefaultFont::Loaded).then(|| x3d_text::fonts::load_default_font("SERIF", "PLAIN")).flatten(),
        }
    }

    pub(crate) fn text_font_style_params_for_alignments(&self, f: NodeId) -> FontStyleParams {
        let mut p = self.text_font_style_params(Some(f));

        p.cached_alignments = None;
        p
    }

    fn text_font_style_params(&self, fs: Option<NodeId>) -> FontStyleParams {
        let Some(f) = fs else { return FontStyleParams::default() };
        let num = |name: &str, default: f64| match self.pre_value(f, name) {
            Some(FieldValue::SFFloat(v)) => *v as f64,
            _ => default,
        };
        let flag = |name: &str| !matches!(self.pre_value(f, name), Some(FieldValue::SFBool(false)));
        let justify = match self.pre_value(f, "justify") {
            Some(FieldValue::MFString(v)) => v.clone(),
            _ => vec![],
        };

        // ScreenFontStyle.getScale: pointSize × PixelsPerPoint × ContentScale (pixels); getContentScale: ContentScale.
        let screen = self.builtin_name(f) == Some("ScreenFontStyle");
        let rendering = |name: &str| self.browser_options.rendering.get(name).map_or(1.0, |v| v.as_f64());
        let content_scale = if screen { rendering("ContentScale") } else { 1.0 };

        FontStyleParams {
            size: if screen { num("pointSize", 12.0) * rendering("PixelsPerPoint") * content_scale } else { num("size", 1.0) },
            spacing: num("spacing", 1.0),
            horizontal: flag("horizontal"),
            left_to_right: flag("leftToRight"),
            top_to_bottom: flag("topToBottom"),
            justify,
            content_scale,
            screen,
            cached_alignments: self.font_style_alignments.get(&f).copied(),
        }
    }

    /// X3DGeometryNode.rebuild → Text.build → X3DTextGeometry.update (configure, build): the output fields.
    pub(crate) fn text_rebuild(&mut self, node: NodeId) {
        let fs = self.text_font_styles.get(&node).copied().flatten();

        let screen = fs.is_some_and(|f| self.builtin_name(f) == Some("ScreenFontStyle"));

        let strings = match self.pre_value(node, "string") {
            Some(FieldValue::MFString(v)) => v.clone(),
            _ => vec![],
        };
        let num_lines = strings.len();
        let (Some(origin), Some(text_bounds), Some(line_bounds)) = (self.pre(node, "origin"), self.pre(node, "textBounds"), self.pre(node, "lineBounds")) else {
            return;
        };

        // `text ._lineBounds .length = numLines`: an event when the length changes.
        let old: Vec<f32> = match self.value(line_bounds) {
            FieldValue::MFVec2f(v) => v.clone(),
            _ => vec![],
        };

        if old.len() != num_lines * 2 {
            let mut resized = old.clone();

            resized.resize(num_lines * 2, 0.0);
            self.set_value(line_bounds, FieldValue::MFVec2f(resized));
        }

        let font = self.text_font(node);

        if num_lines == 0 || font.is_none() {
            // ScreenText.configure goes on after X3DTextGeometry.configure: the box rounded to whole pixels around the empty one, and
            // every line bound assigned (an event when there are lines).
            let (o, b) = if screen {
                let layout = x3d_text::layout(None, &TextParams::default(), &self.text_font_style_params(fs), PrimitiveQuality::Medium, TextCompression::CharSpacing);

                (layout.origin, layout.text_bounds)
            } else {
                ([0.0; 3], [0.0; 2])
            };

            self.set_value(origin, FieldValue::SFVec3f(x3d_fields::math::Vector3::new(o[0], o[1], o[2])));
            self.set_value(text_bounds, FieldValue::SFVec2f(x3d_fields::math::Vector2::new(b[0], b[1])));

            if screen && num_lines != 0 {
                let content_scale = self.text_font_style_params(fs).content_scale;
                let bounds = match self.value(line_bounds) {
                    FieldValue::MFVec2f(v) => v.iter().map(|x| (*x as f64 / content_scale) as f32).collect(),
                    _ => vec![],
                };

                self.set_value(line_bounds, FieldValue::MFVec2f(bounds));
            }

            return;
        }

        let length: Vec<f64> = match self.pre_value(node, "length") {
            Some(FieldValue::MFFloat(v)) => v.iter().map(|x| *x as f64).collect(),
            _ => vec![],
        };
        let max_extent = match self.pre_value(node, "maxExtent") {
            Some(FieldValue::SFFloat(v)) => *v as f64,
            _ => 0.0,
        };
        let option = |name: &str| self.browser_options.get_option(name).map(|v| v.as_str().to_string());
        let quality = match option("PrimitiveQuality").as_deref() {
            Some("LOW") => PrimitiveQuality::Low,
            Some("HIGH") => PrimitiveQuality::High,
            _ => PrimitiveQuality::Medium,
        };
        let compression = if option("TextCompression").as_deref() == Some("SCALING") { TextCompression::Scaling } else { TextCompression::CharSpacing };
        let text = TextParams::from_strings(&strings, &length, max_extent);
        let layout = x3d_text::layout(font.as_deref(), &text, &self.text_font_style_params(fs), quality, compression);

        // horizontal / vertical: `text ._lineBounds [l] = lineBound`, then `text ._textBounds = size`, then `text ._origin .setValue`.
        let bounds: Vec<f32> = layout.line_bounds.iter().flat_map(|b| [b[0] as f32, b[1] as f32]).collect();

        self.set_value(line_bounds, FieldValue::MFVec2f(bounds));
        self.set_value(text_bounds, FieldValue::SFVec2f(x3d_fields::math::Vector2::new(layout.text_bounds[0], layout.text_bounds[1])));
        self.set_value(origin, FieldValue::SFVec3f(x3d_fields::math::Vector3::new(layout.origin[0], layout.origin[1], layout.origin[2])));
    }
}
