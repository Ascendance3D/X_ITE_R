//! The Shaders component (Components/Shaders/{ShaderPart,ComposedShader,X3DShaderNode}.js) and the
//! shader selection of Appearance (Appearance.js set_shaders__ / set_shader__).
//!
//! * `ShaderPart` loads its `url` with the FileLoader (fallback, `data:`, load errors), then compiles
//!   the source. X_ITE compiles with the GPU driver; this port asks a [`ShaderCompiler`] the renderer
//!   installs (`Runtime::shaders.compiler`), exactly like `Runtime::script_host`.
//! * `ComposedShader` watches its parts with a private LoadSensor, links when they are loaded
//!   (`set_loaded__`), sets `isValid`, and re-links on `activate` and `language`.
//! * `Appearance` selects the first valid shader of `shaders` and counts selections (`isSelected`).
//! * `ShaderProgram`, `ProgramShader` and `PackagedShader` have no behavior in the pinned X_ITE
//!   (`requestImmediateLoad` of the url nodes does nothing, ProgramShader is a field list): the nodes
//!   exist, `isValid` stays FALSE, and an Appearance never selects them.

use super::State;
use crate::loader::{FileLoader, Loaded};
use crate::*;
use std::collections::HashMap;

/// A compiled part's source and how it was compiled.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShaderSource {
    pub source: String,
    pub options: Vec<String>,
}

/// What the runtime needs from the shader compiler (the GPU driver in X_ITE).
pub trait ShaderCompiler {
    /// `gl.compileShader`: `Err` is the info log (`ERROR: 0:line: message`).
    fn compile_part(&self, fragment: bool, source: &str, options: &[String]) -> Result<(), String>;
    /// `gl.linkProgram` of the attached parts: `Err` is the program info log.
    /// `Ok` carries the deprecation warnings of getDefaultUniformsAndAttributes (text after the node's type and name).
    fn link(&self, vertex: &[ShaderSource], fragment: &[ShaderSource]) -> Result<Vec<String>, String>;
}

/// What a ShaderPart keeps after a compile (X_ITE: `this .shader`).
#[derive(Clone, Debug)]
pub struct PartState {
    pub source: ShaderSource,
    pub fragment: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ComposedState {
    pub load_sensor: Option<NodeId>,
    /// X3DShaderNode.valid.
    pub valid: bool,
    /// The sources the last successful link used: vertex parts, then fragment parts.
    pub linked: Option<(Vec<ShaderSource>, Vec<ShaderSource>)>,
}

#[derive(Clone, Debug, Default)]
pub struct AppearanceState {
    /// Appearance.shaderNodes.
    pub nodes: Vec<NodeId>,
    /// Appearance.shaderNode.
    pub selected: Option<NodeId>,
}

#[derive(Default)]
pub struct ShaderHost {
    pub compiler: Option<Box<dyn ShaderCompiler>>,
    pub parts: HashMap<NodeId, PartState>,
    pub composed: HashMap<NodeId, ComposedState>,
    /// X3DShaderNode.selected.
    pub selected_count: HashMap<NodeId, i32>,
    pub appearances: HashMap<NodeId, AppearanceState>,
    /// ShaderPart compiles and ComposedShader links done (for tests and the lifecycle report).
    pub compiles: usize,
    pub links: usize,
}

/// Interest callbacks of the Shaders nodes (`Cb::Shader`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShaderCb {
    /// ShaderPart.set_type__.
    PartSetType,
    /// ComposedShader.set_activate__.
    ComposedActivate,
    /// `_language .addInterest ("set_loaded__")` and the load sensor's `isLoaded` after connectLoaded.
    ComposedSetLoaded,
    /// The first interest on the load sensor's `isLoaded`: ComposedShader.connectLoaded.
    ComposedConnectLoaded,
    /// Appearance.set_shaders__.
    AppearanceSetShaders,
    /// Appearance.set_shader__ (`isValid` of a shader node).
    AppearanceSetShader,
}

/// X3DConstants.X3DShaderNode.
pub fn is_shader_node(name: &str) -> bool {
    matches!(name, "ComposedShader" | "PackagedShader" | "ProgramShader")
}

/// `new TextDecoder () .decode (data)`: UTF-8, a BOM is dropped, bad sequences are U+FFFD.
fn decode_text(data: &[u8]) -> String {
    let s = String::from_utf8_lossy(data);
    s.strip_prefix('\u{feff}').unwrap_or(&s).to_string()
}

/// The `(\d+):(\d+)` the ShaderPart error message looks for in the info log: (file, line).
fn log_position(log: &str) -> Option<(String, String)> {
    let b = log.as_bytes();
    let mut i = 0;

    while i < b.len() {
        if b[i].is_ascii_digit() {
            let s = i;

            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }

            if i < b.len() && b[i] == b':' && i + 1 < b.len() && b[i + 1].is_ascii_digit() {
                let a = &log[s..i];
                let mut j = i + 1;

                while j < b.len() && b[j].is_ascii_digit() {
                    j += 1;
                }

                return Some((a.to_string(), log[i + 1..j].to_string()));
            }
        } else {
            i += 1;
        }
    }

    None
}

impl Runtime {
    pub fn shader_compiler_installed(&self) -> bool {
        self.shaders.compiler.is_some()
    }

    // ---- ShaderPart ----------------------------------------------------------------------------

    /// ShaderPart.initialize (after X3DUrlObject's interests, which `inline_initialize` adds and
    /// whose `requestImmediateLoad` follows).
    pub(crate) fn shader_part_initialize(&mut self, node: NodeId) {
        self.interest(node, "type", Cb::Shader(ShaderCb::PartSetType));
        self.inline_initialize(node);
    }

    /// ShaderPart.loadData: FileLoader, compile, then the load state.
    pub(crate) fn shader_part_load(&mut self, node: NodeId) {
        let loader = FileLoader { type_name: "ShaderPart".into(), base: self.base_url(node), data_as_string: false };
        let urls = self.url_strings(node);
        let shader_type = match self.pre_value(node, "type") {
            Some(FieldValue::SFString(s)) => s.clone(),
            _ => "VERTEX".to_string(),
        };
        let type_name = self.type_name(node);
        let name = self.scene.node(node).name.clone();

        let ok = self.load_document(&loader, &urls, |rt, loaded| {
            let (data, url) = match loaded {
                Loaded::Script(_) => return Err("Error: Couldn't parse X3D. No suitable file handler found.".into()),
                Loaded::Data(data, url) => (data, url),
            };
            // `fileURL` is not set for a data URL (FileLoader.loadDocument).
            let file_url = if url.scheme() == "data" { rt.base_url(node) } else { url.to_string() };
            let source = decode_text(&data);
            let fragment = shader_type == "FRAGMENT";
            let options: Vec<String> = vec![];

            // ShaderPart.getShaderType: an unknown type is VERTEX_SHADER; the types WebGL 2 does
            // not have (`gl [type]` is undefined) make createShader fail, and the info log is null.
            if matches!(shader_type.as_str(), "TESS_CONTROL" | "TESS_EVALUATION" | "GEOMETRY" | "COMPUTE") {
                return Err(format!("Error: Error in {type_name} '{name}' in URL '{file_url}', null"));
            }

            rt.shaders.compiles += 1;
            // `this .shader = shader` happens before the status check.
            rt.shaders.parts.insert(node, PartState { source: ShaderSource { source: source.clone(), options: options.clone() }, fragment });

            let result = match &rt.shaders.compiler {
                Some(c) => c.compile_part(fragment, &source, &options),
                None => Ok(()),
            };

            match result {
                Ok(()) => Ok(()),
                Err(log) => Err(match log_position(&log) {
                    Some((_, line)) => format!("Error: Error in {type_name} '{name}' in URL '{file_url}', line {line}, {log}"),
                    None => format!("Error: Error in {type_name} '{name}' in URL '{file_url}', {log}"),
                }),
            }
        });

        self.set_load_state(node, if ok { nodes::LoadState::Complete } else { nodes::LoadState::Failed });
    }

    // ---- ComposedShader ------------------------------------------------------------------------

    /// X3DShaderNode.setValid: `this ._isValid = this .valid = value`.
    fn shader_set_valid(&mut self, node: NodeId, value: bool) {
        self.shaders.composed.entry(node).or_default().valid = value;

        if let Some(k) = self.pre(node, "isValid") {
            self.set_value(k, FieldValue::SFBool(value));
        }
    }

    pub fn shader_is_valid_flag(&self, node: NodeId) -> bool {
        self.shaders.composed.get(&node).is_some_and(|c| c.valid)
    }

    /// ComposedShader construct: the private LoadSensor (`new LoadSensor (executionContext)`).
    pub(crate) fn composed_shader_construct(&mut self, node: NodeId) {
        let Some(&index) = self.scene.table.by_name.get("LoadSensor") else {
            return;
        };
        let ctx = self.scene.node(node).ctx;
        let sensor = self.scene.instantiate_builtin(ctx, index);
        self.nrt_mut(sensor);
        self.construct_node(sensor);
        self.shaders.composed.insert(node, ComposedState { load_sensor: Some(sensor), valid: false, linked: None });
    }

    /// ComposedShader.initialize.
    pub(crate) fn composed_shader_initialize(&mut self, node: NodeId) {
        self.interest(node, "activate", Cb::Shader(ShaderCb::ComposedActivate));
        self.interest(node, "language", Cb::Shader(ShaderCb::ComposedSetLoaded));

        let Some(sensor) = self.shaders.composed.get(&node).and_then(|c| c.load_sensor) else {
            return;
        };

        // `this ._parts .addFieldInterest (this .loadSensor ._children)`
        if let (Some(parts), Some(children)) = (self.pre(node, "parts"), self.pre(sensor, "children")) {
            self.add_field_interest(parts, children);
        }

        // `this .loadSensor ._isLoaded .addInterest ("connectLoaded", this)`
        let Some(is_loaded) = self.pre(sensor, "isLoaded") else {
            return;
        };
        self.add_interest(is_loaded, node, Cb::Shader(ShaderCb::ComposedConnectLoaded));

        // `this .loadSensor ._children = this ._parts`, setPrivate, setup.
        if let (Some(parts), Some(children)) = (self.pre(node, "parts"), self.pre(sensor, "children")) {
            let value = self.value(parts).clone();
            self.set_raw(children, value);
        }

        self.setup_node(sensor);

        if self.get_bool(sensor, "isLoaded") {
            self.composed_set_loaded(node);
        } else {
            self.composed_connect_loaded(node);
        }
    }

    /// ComposedShader.connectLoaded.
    fn composed_connect_loaded(&mut self, node: NodeId) {
        let Some(sensor) = self.shaders.composed.get(&node).and_then(|c| c.load_sensor) else {
            return;
        };
        let Some(is_loaded) = self.pre(sensor, "isLoaded") else {
            return;
        };

        self.remove_interest(is_loaded, node, Cb::Shader(ShaderCb::ComposedConnectLoaded));
        self.add_interest(is_loaded, node, Cb::Shader(ShaderCb::ComposedSetLoaded));
    }

    /// ComposedShader.set_loaded__.
    fn composed_set_loaded(&mut self, node: NodeId) {
        let Some(sensor) = self.shaders.composed.get(&node).and_then(|c| c.load_sensor) else {
            return;
        };
        let language = match self.pre_value(node, "language") {
            Some(FieldValue::SFString(s)) => s.clone(),
            _ => String::new(),
        };

        if !(self.get_bool(sensor, "isLoaded") && language == "GLSL") {
            if let Some(c) = self.shaders.composed.get_mut(&node) {
                c.linked = None;
            }

            self.shader_set_valid(node, false);
            return;
        }

        // The parts with a compiled shader, in `parts` order, attached to a new program.
        let parts: Vec<Option<NodeId>> = match self.pre_value(node, "parts") {
            Some(FieldValue::MFNode(v)) => v.clone(),
            _ => vec![],
        };
        let (mut vertex, mut fragment) = (vec![], vec![]);

        for p in parts.iter().flatten() {
            if self.builtin_name(*p) != Some("ShaderPart") {
                continue;
            }

            if let Some(state) = self.shaders.parts.get(p) {
                if state.fragment {
                    fragment.push(state.source.clone());
                } else {
                    vertex.push(state.source.clone());
                }
            }
        }

        self.shaders.links += 1;

        let result = match &self.shaders.compiler {
            Some(c) => c.link(&vertex, &fragment),
            None => Err("no shader compiler is installed".to_string()),
        };

        match result {
            Ok(warnings) => {
                if let Some(c) = self.shaders.composed.get_mut(&node) {
                    c.linked = Some((vertex, fragment));
                }

                self.shader_set_valid(node, true);

                // console .warn (this .getTypeName (), this .getName (), message)
                let type_name = self.type_name(node);
                let name = self.scene.node(node).name.clone();

                for w in warnings {
                    self.log_warn(format!("{type_name} {name} {w}"));
                }
            }
            Err(log) => {
                if let Some(c) = self.shaders.composed.get_mut(&node) {
                    c.linked = None;
                }

                self.shader_set_valid(node, false);

                if !parts.is_empty() {
                    let type_name = self.type_name(node);
                    let name = self.scene.node(node).name.clone();
                    self.log_warn(format!("Couldn't initialize {type_name} '{name}': {log}"));
                }
            }
        }
    }

    // ---- Appearance ----------------------------------------------------------------------------

    /// X3DCast (X3DConstants .X3DShaderNode, node), looking through PROTO instances like the
    /// url object cast.
    fn cast_shader_node(&self, node: NodeId) -> Option<NodeId> {
        let mut n = node;

        for _ in 0..256 {
            if let NodeKind::Instance(_) = self.scene.node(n).kind {
                n = self.scene.ctx(self.instance_body(n)?).root_nodes.first().copied().flatten()?;
                continue;
            }

            return self.builtin_name(n).is_some_and(is_shader_node).then_some(n);
        }

        None
    }

    /// Appearance.initialize: the `shaders` interest and the first `set_shaders__ ()`.
    pub(crate) fn appearance_shaders_initialize(&mut self, node: NodeId) {
        self.interest(node, "shaders", Cb::Shader(ShaderCb::AppearanceSetShaders));
        self.appearance_set_shaders(node);
    }

    /// Appearance.set_shaders__.
    fn appearance_set_shaders(&mut self, node: NodeId) {
        let old = self.shaders.appearances.get(&node).map(|s| s.nodes.clone()).unwrap_or_default();

        for s in old {
            if let Some(k) = self.pre(s, "isValid") {
                self.remove_interest(k, node, Cb::Shader(ShaderCb::AppearanceSetShader));
            }
        }

        let listed: Vec<Option<NodeId>> = match self.pre_value(node, "shaders") {
            Some(FieldValue::MFNode(v)) => v.clone(),
            _ => vec![],
        };
        let nodes: Vec<NodeId> = listed.iter().flatten().filter_map(|n| self.cast_shader_node(*n)).collect();

        for &s in &nodes {
            if let Some(k) = self.pre(s, "isValid") {
                self.add_interest(k, node, Cb::Shader(ShaderCb::AppearanceSetShader));
            }
        }

        self.shaders.appearances.entry(node).or_default().nodes = nodes;
        self.appearance_set_shader(node);
    }

    /// Appearance.set_shader__.
    fn appearance_set_shader(&mut self, node: NodeId) {
        let (nodes, previous) = {
            let s = self.shaders.appearances.entry(node).or_default();
            (s.nodes.clone(), s.selected.take())
        };

        if let Some(p) = previous {
            self.shader_deselect(p);
        }

        let mut chosen = None;

        for s in nodes {
            if self.get_bool(s, "isValid") {
                chosen = Some(s);
                break;
            }
        }

        self.shaders.appearances.entry(node).or_default().selected = chosen;

        if let Some(s) = chosen {
            self.shader_select(s);
        }
    }

    /// X3DShaderNode.select.
    fn shader_select(&mut self, node: NodeId) {
        *self.shaders.selected_count.entry(node).or_insert(0) += 1;

        if !self.get_bool(node, "isSelected") {
            if let Some(k) = self.pre(node, "isSelected") {
                self.set_value(k, FieldValue::SFBool(true));
            }
        }
    }

    /// X3DShaderNode.deselect.
    fn shader_deselect(&mut self, node: NodeId) {
        let count = {
            let c = self.shaders.selected_count.entry(node).or_insert(0);
            *c -= 1;
            *c
        };

        if count == 0 && self.get_bool(node, "isSelected") {
            if let Some(k) = self.pre(node, "isSelected") {
                self.set_value(k, FieldValue::SFBool(false));
            }
        }
    }

    /// Appearance.shaderNode: the shader the Appearance draws with, if any.
    pub fn appearance_shader(&self, appearance: NodeId) -> Option<NodeId> {
        self.shaders.appearances.get(&appearance).and_then(|s| s.selected)
    }

    /// The vertex and fragment sources the last link of a ComposedShader used.
    pub fn composed_shader_sources(&self, node: NodeId) -> Option<&(Vec<ShaderSource>, Vec<ShaderSource>)> {
        self.shaders.composed.get(&node).and_then(|c| c.linked.as_ref())
    }

    // ---- dispatch ------------------------------------------------------------------------------

    pub(crate) fn shader_interest(&mut self, node: NodeId, cb: ShaderCb) {
        match cb {
            ShaderCb::PartSetType => {
                // set_type__: setLoadState (NOT_STARTED_STATE), requestImmediateLoad.
                self.set_load_state(node, nodes::LoadState::NotStarted);
                self.request_immediate_load(node);
            }
            ShaderCb::ComposedActivate => {
                // set_activate__: only `activate TRUE` links again.
                if self.get_bool(node, "activate") {
                    self.composed_set_loaded(node);
                }
            }
            ShaderCb::ComposedSetLoaded => self.composed_set_loaded(node),
            ShaderCb::ComposedConnectLoaded => self.composed_connect_loaded(node),
            ShaderCb::AppearanceSetShaders => self.appearance_set_shaders(node),
            ShaderCb::AppearanceSetShader => self.appearance_set_shader(node),
        }
    }
}

#[allow(dead_code)]
fn _state_marker(_: &State) {}

/// `fetch (dataURL)` as Chrome does it for the URL a ShaderPart carries: the text after the first
/// comma, cut at a `#` (the fragment is not part of the data), percent-decoded (a malformed escape
/// stays), base64 decoded when the media type ends in `;base64`.
pub fn fetch_data_url(url: &str) -> Result<Vec<u8>, String> {
    let url = url.trim_start();
    let Some(rest) = url.strip_prefix("data:").or_else(|| url.strip_prefix("DATA:")) else {
        return Err("TypeError: Failed to fetch".into());
    };
    let Some((head, payload)) = rest.split_once(',') else {
        return Err("TypeError: Failed to fetch".into());
    };
    let payload = payload.split_once('#').map_or(payload, |(p, _)| p);
    let mut bytes = Vec::with_capacity(payload.len());
    let b = payload.as_bytes();
    let mut i = 0;

    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() + 0 && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() {
            bytes.push(u8::from_str_radix(&payload[i + 1..i + 3], 16).unwrap());
            i += 3;
        } else {
            bytes.push(b[i]);
            i += 1;
        }
    }

    if head.trim_end().to_ascii_lowercase().ends_with(";base64") {
        let text: Vec<u8> = bytes.into_iter().filter(|c| !c.is_ascii_whitespace()).collect();
        return decode_base64(&text).ok_or_else(|| "TypeError: Failed to fetch".to_string());
    }

    Ok(bytes)
}

fn decode_base64(s: &[u8]) -> Option<Vec<u8>> {
    let mut out = vec![];
    let (mut acc, mut bits) = (0u32, 0);

    for &c in s.iter().take_while(|c| **c != b'=') {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }

    Some(out)
}
