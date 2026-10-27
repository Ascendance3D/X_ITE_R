//! One world of an `<x3d-canvas>`: the Rust runtime and the live frame loop of x3d-viewer, without a window,
//! a file system or a GPU. The wasm32 module ([`crate::wasm`]) draws its frames; the native tests drive it directly.
//!
//! The steps are those of `Viewer3d::load_attributes` (X3DBrowser.loadURL → FileLoader → replaceWorld), with the
//! world's bytes and URL from the page instead of a file, and no Script host (QuickJS does not build for the web).
//!
//! A session loads in two stages, like X_ITE's FileLoader: [`Session::begin`] sets the scene up and requests its
//! Inline, EXTERNPROTO and texture URLs; [`Session::poll_load`] continues while their responses arrive (an
//! asynchronous [`Fetcher`] parks a load until then). Only a loaded session goes live ([`Session::go_live`],
//! X3DBrowser.replaceWorld), so the current world stays while the next one loads.

use x3d_fields::{FieldValue, NodeId, access};
use x3d_render::traverse::Frame;
use x3d_runtime::{Fetcher, Runtime};
use x3d_viewer::nav::Touches;
use x3d_viewer::viewer::{UiEvent, Viewer3d};

/// A field of a node as the page sees it: X3DField.getTypeName, the access type and `toString ()`.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldInfo {
    pub type_name: &'static str,
    pub access: &'static str,
    pub value: String,
}

/// A pointer event of the page, at pointer coordinates (X3DPointingDeviceSensorContext.getPointerFromEvent: pixels
/// of the framebuffer, the origin at the bottom left).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pointer {
    Down { button: u8, x: f64, y: f64 },
    Up { button: u8, x: f64, y: f64 },
    Move { x: f64, y: f64 },
    DblClick { x: f64, y: f64 },
}

/// A touch event of the page: touchstart, touchmove or touchend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Touch {
    Start,
    Move,
    End,
}

enum Stage {
    /// The scene is set up and its loads run (FileLoader: `scene ._loadCount` is not 0 yet).
    Loading(Box<Runtime>, bool),
    /// The world of the browser (after X3DBrowser.replaceWorld).
    Live(Box<Viewer3d>),
    /// Only while [`Session::go_live`] moves the runtime into its viewer.
    Gone,
}

pub struct Session {
    stage: Stage,
    size: (u32, u32),
    /// How much of the runtime's log [`Session::take_log`] returned.
    log_seen: usize,
    /// The INITIALIZED_EVENT of the world's own replaceWorld is still to come: the page fires that one itself.
    own_initialized: bool,
}

/// A world that parsed and waits for [`Session::begin`].
pub struct ParsedWorld {
    scene: x3d_scene::Scene,
    fetcher: Box<dyn Fetcher>,
    url: String,
}

/// The browser's constant setup of every parse.
fn parse_setup() {
    x3d_fields::jsmath::set_flavor(x3d_fields::jsmath::MathFlavor::Chrome);
    x3d_render::traverse::install_particle_geometry();
}

impl Session {
    /// Parse the world `data` from `url` (FileLoader). `fetcher` serves the world's Inline, EXTERNPROTO and texture
    /// URLs. A world that does not parse is an error.
    pub fn parse(data: &[u8], url: &str, fetcher: Box<dyn Fetcher>) -> Result<ParsedWorld, String> {
        Session::parse_logged(data, url, fetcher, |_| {})
    }

    /// [`Session::parse`] as FileLoader.createX3DFromURLAsync parses a candidate of the page's loadURL: the error is
    /// the rejection FileLoader.printError reports (as for an Inline), and the parser's own messages go to `log`
    /// first (`warn: …` or `error: …`; VRMLParser prints the original error in DEVELOPMENT).
    pub fn parse_logged(
        data: &[u8],
        url: &str,
        fetcher: Box<dyn Fetcher>,
        mut log: impl FnMut(String),
    ) -> Result<ParsedWorld, String> {
        parse_setup();

        let scene = match x3d_runtime::parse_bytes_with_fetcher(&*fetcher, data, url).scene {
            Ok(scene) => scene,
            Err(x3d_parse::LoadError::Parse(e)) => {
                for w in x3d_parse::take_error_warnings() {
                    log(format!("warn: {w}"));
                }

                log(format!("error: Error: {}", e.message));
                return Err(format!("Error: \n{e}"));
            }
            // GoldenGate.parseIntoScene names the url of a file that no parser handles.
            Err(x3d_parse::LoadError::NoHandler) => {
                return Err(format!(
                    "Error: Couldn't parse X3D. No suitable file handler found for '{url}'."
                ));
            }
            Err(e) => return Err(format!("Error: {e}")),
        };

        Ok(ParsedWorld {
            scene,
            fetcher,
            url: url.to_string(),
        })
    }

    /// X3DBrowser.createX3DFromString of a page (an external browser): the text with the base URL `url` as its world
    /// URL. The error is the rejection's `String (error)`, like X3DBrowser's promise gives it.
    pub fn parse_string(
        text: &str,
        url: &str,
        fetcher: Box<dyn Fetcher>,
    ) -> Result<ParsedWorld, String> {
        parse_setup();

        let scene = match x3d_parse::parse_string(text, url) {
            Ok(scene) => scene,
            Err(x3d_parse::LoadError::Parse(e))
                if e.line == 0
                    && ["TypeError: ", "RangeError: ", "Error: "]
                        .iter()
                        .any(|p| e.message.starts_with(p)) =>
            {
                return Err(e.message);
            }
            Err(x3d_parse::LoadError::Parse(e)) => {
                x3d_parse::take_error_warnings();

                return Err(
                    x3d_parse::take_error_detail().unwrap_or_else(|| format!("Error: \n{e}"))
                );
            }
            Err(x3d_parse::LoadError::NoHandler) => {
                return Err(format!(
                    "Error: Couldn't parse X3D. No suitable file handler found for '{url}'."
                ));
            }
            Err(e) => return Err(format!("Error: {e}")),
        };

        Ok(ParsedWorld {
            scene,
            fetcher,
            url: url.to_string(),
        })
    }

    /// Set the parsed world up at `now` (seconds since the Unix epoch, X3DBrowser.getCurrentTime) and start its
    /// loads. The session is not live yet: see [`Session::poll_load`] and [`Session::go_live`].
    pub fn begin(world: ParsedWorld, now: f64, width: u32, height: u32) -> Session {
        let mut rt = Runtime::new(world.scene, world.fetcher);

        rt.page_url = world.url.clone();
        x3d_render::shader_gpu::install(&mut rt);
        rt.live = true;
        rt.now = now;
        rt.browser_options = x3d_runtime::browser_options::BrowserOptions::new(width, height);
        rt.begin_load_scene();

        let mut session = Session {
            stage: Stage::Loading(Box::new(rt), false),
            size: (width, height),
            log_seen: 0,
            own_initialized: true,
        };

        session.poll_load(now);
        session
    }

    /// Continue the loads with the responses that arrived. Returns whether the scene is loaded (its loadCount is 0
    /// and it had its frames): then it can go live. A live session is loaded.
    pub fn poll_load(&mut self, now: f64) -> bool {
        match &mut self.stage {
            Stage::Loading(rt, loaded) => {
                if !*loaded {
                    rt.now = rt.now.max(now);
                    rt.resume_fetches();
                    *loaded = rt.continue_load_scene();
                }

                *loaded
            }
            Stage::Live(_) | Stage::Gone => true,
        }
    }

    /// Is a load of the scene waiting for a response?
    pub fn is_loaded(&self) -> bool {
        match &self.stage {
            Stage::Loading(_, loaded) => *loaded,
            Stage::Live(_) | Stage::Gone => true,
        }
    }

    pub fn is_live(&self) -> bool {
        matches!(self.stage, Stage::Live(_))
    }

    /// X3DBrowser.replaceWorld with this loaded scene, up to its first live frame at `now`. Drop the session of the
    /// old world first: its shutdown comes before. Returns false (and changes nothing) for a scene still loading.
    pub fn go_live(&mut self, now: f64) -> bool {
        let Stage::Loading(_, true) = &self.stage else {
            return self.is_live();
        };
        let Stage::Loading(mut rt, _) = std::mem::replace(&mut self.stage, Stage::Gone) else {
            unreachable!()
        };

        rt.now = now;
        rt.replace_world();
        // From now on the browser events of the world go to the page: an Anchor can replace it.
        rt.browser_events = Some(vec![]);

        self.stage = Stage::Live(Box::new(Viewer3d::from_runtime(
            *rt,
            now,
            self.size.0,
            self.size.1,
        )));
        true
    }

    /// [`Session::parse`], [`Session::begin`] and [`Session::go_live`], for a synchronous `fetcher`.
    pub fn load(
        data: &[u8],
        url: &str,
        fetcher: Box<dyn Fetcher>,
        now: f64,
        width: u32,
        height: u32,
    ) -> Result<Session, String> {
        let mut s = Self::begin(Self::parse(data, url, fetcher)?, now, width, height);

        if !s.go_live(now) {
            return Err("the world waits for a response".to_string());
        }

        Ok(s)
    }

    /// X3DScene.worldURL of the world (an Anchor can have replaced it).
    pub fn world_url(&self) -> String {
        let scene = &self.rt().scene;

        scene.world_url_of(scene.world).to_string()
    }

    /// Counts the worlds this session has had (an Anchor replaces the world inside the runtime): the page's node
    /// handles belong to one of them.
    pub fn world_generation(&self) -> u64 {
        self.rt().world_state.generation
    }

    fn rt(&self) -> &Runtime {
        match &self.stage {
            Stage::Loading(rt, _) => rt,
            Stage::Live(v) => &v.rt,
            Stage::Gone => unreachable!("a session is never seen while it goes live"),
        }
    }

    fn rt_mut(&mut self) -> &mut Runtime {
        match &mut self.stage {
            Stage::Loading(rt, _) => rt,
            Stage::Live(v) => &mut v.rt,
            Stage::Gone => unreachable!("a session is never seen while it goes live"),
        }
    }

    /// Is `node` a node of the world now (not a released or out-of-range id from the page)?
    fn node_exists(&self, node: NodeId) -> bool {
        let scene = &self.rt().scene;

        (node as usize) < scene.nodes.len() && !scene.free_nodes.contains(&node)
    }

    /// X3DExecutionContext.getNamedNode (name) of the world; `None` where X_ITE throws "Named node not found".
    pub fn named_node(&self, name: &str) -> Option<NodeId> {
        let scene = &self.rt().scene;

        scene.get_named_node(scene.world, name)
    }

    /// X3DExecutionContext.rootNodes of the world (NULL entries kept).
    pub fn root_nodes(&self) -> Vec<Option<NodeId>> {
        let scene = &self.rt().scene;

        scene.ctx(scene.world).root_nodes.clone()
    }

    /// X3DNode.getNodeTypeName.
    pub fn type_name(&self, node: NodeId) -> Option<String> {
        self.node_exists(node).then(|| self.rt().type_name(node))
    }

    /// X3DNode.getNodeName.
    pub fn node_name(&self, node: NodeId) -> Option<String> {
        self.node_exists(node)
            .then(|| self.rt().scene.node(node).name.clone())
    }

    /// The nodes of an SFNode or MFNode field (`None` for NULL); `Err` for a field of another type.
    pub fn node_value(&self, node: NodeId, name: &str) -> Result<Vec<Option<NodeId>>, String> {
        let k = self.field_key(node, name)?;

        match &self.rt().field(k).value {
            FieldValue::SFNode(n) => Ok(vec![*n]),
            FieldValue::MFNode(list) => Ok(list.clone()),
            v => Err(format!("{} is not a node field", v.field_type().name())),
        }
    }

    /// The names of X3DNode.getFieldDefinitions, then the user-defined fields.
    pub fn field_names(&self, node: NodeId) -> Vec<String> {
        if !self.node_exists(node) {
            return vec![];
        }

        let n = self.rt().scene.node(node);

        n.fields
            .iter()
            .chain(&n.user_fields)
            .map(|f| f.name.clone())
            .collect()
    }

    /// X3DNode.getField (name): its type, access type and `toString ()` value.
    pub fn field(&self, node: NodeId, name: &str) -> Result<FieldInfo, String> {
        let k = self.field_key(node, name)?;
        let rt = self.rt();
        let f = rt.field(k);

        Ok(FieldInfo {
            type_name: f.ty().name(),
            access: access::name(f.access),
            value: value_string(rt, &f.value),
        })
    }

    /// `node [name] = value` (SFNode's proxy setter, X3DField.setValue) with the value in the VRML encoding: the field
    /// takes the value now and sends its event in the next frame's event cascade. An outputOnly field refuses it, like
    /// the proxy setter's `return false`.
    pub fn set_field(&mut self, node: NodeId, name: &str, vrml: &str) -> Result<(), String> {
        let k = self.field_key(node, name)?;
        let rt = self.rt_mut();
        let (ty, access) = {
            let f = rt.field(k);
            (f.ty(), f.access)
        };

        if access == access::OUTPUT_ONLY {
            return Err(format!("field '{name}' is outputOnly"));
        }

        let ctx = rt.scene.node(node).ctx;
        let value = match rt.field_from_string(ty, vrml, Some(ctx), false, None, name) {
            Ok(Some(v)) => v,
            Ok(None) => return Err(format!("no {} value in '{vrml}'", ty.name())),
            Err(e) => return Err(e.unwrap_or_else(|| format!("bad {} value", ty.name()))),
        };

        rt.set_value(k, value);
        Ok(())
    }

    fn field_key(&self, node: NodeId, name: &str) -> Result<x3d_runtime::FKey, String> {
        if !self.node_exists(node) {
            return Err("node is not in the world".to_string());
        }

        self.rt().get_field(node, name).ok_or_else(|| {
            format!(
                "Unknown field '{name}' in node class {}.",
                self.rt().type_name(node)
            )
        })
    }

    /// One frame of the browser's event loop at `now` (seconds since the Unix epoch). The loads whose responses
    /// arrived continue first. A session that is not live has no frames.
    pub fn step(&mut self, now: f64) {
        if let Stage::Live(v) = &mut self.stage {
            v.rt.resume_fetches();
            v.date_now = now;
            v.step(now);
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.size = (width.max(1), height.max(1));

        if let Stage::Live(v) = &mut self.stage {
            v.resize(self.size.0, self.size.1);
        }
    }

    /// The frame to draw: the DISPLAY traversal of the current state (an empty frame before the session is live).
    pub fn build_frame(&mut self) -> Frame {
        match &mut self.stage {
            Stage::Live(v) => v.build_frame(),
            _ => crate::empty_frame(),
        }
    }

    /// The warnings and errors the runtime logged (parse warnings, failed loads, ...) since the last call.
    pub fn take_log(&mut self) -> Vec<String> {
        let seen = self.log_seen;
        let log = &self.rt().log;
        let new = log[seen.min(log.len())..].to_vec();

        self.log_seen = log.len();
        new
    }

    /// The live frame loop (navigation, bindables, sensors) of the world.
    pub fn viewer(&mut self) -> Option<&mut Viewer3d> {
        match &mut self.stage {
            Stage::Live(v) => Some(v),
            _ => None,
        }
    }

    // --- input ---------------------------------------------------------------------------------------------------

    /// A pointer event: PointingDevice (picking, the pointing device sensors, Anchor), then the viewer.
    pub fn pointer(&mut self, event: Pointer) {
        let Some(v) = self.viewer() else {
            return;
        };

        v.input(match event {
            Pointer::Down { button, x, y } => UiEvent::MouseDown { button, x, y },
            Pointer::Up { button, x, y } => UiEvent::MouseUp { button, x, y },
            Pointer::Move { x, y } => UiEvent::MouseMove { x, y },
            Pointer::DblClick { x, y } => UiEvent::DblClick { x, y },
        });
    }

    /// A wheel event (WheelEvent.deltaY). Returns whether the viewer takes it (X3DViewer's `preventDefault ()`):
    /// a navigating viewer does, NONE does not.
    pub fn wheel(&mut self, delta_y: f64, x: f64, y: f64) -> bool {
        let Some(v) = self.viewer() else {
            return false;
        };

        v.input(UiEvent::Wheel { delta_y, x, y });
        v.viewer_kind() != "NONE"
    }

    /// A touch event with the touches still down (`event .touches`, the first two count), at pointer coordinates.
    /// `zoom_width` is `window .innerWidth` at pointer scale (ExamineViewer's pinch zoom factor).
    pub fn touch(&mut self, event: Touch, touches: &[(f64, f64)], zoom_width: f64) {
        let Some(v) = self.viewer() else {
            return;
        };
        let mut t = Touches {
            n: touches.len(),
            p: [(0.0, 0.0); 2],
            zoom_width,
        };

        for (i, p) in touches.iter().take(2).enumerate() {
            t.p[i] = *p;
        }

        v.input(match event {
            Touch::Start => UiEvent::TouchStart(t),
            Touch::Move => UiEvent::TouchMove(t),
            Touch::End => UiEvent::TouchEnd(t),
        });
    }

    /// A keydown or keyup of the element: `event .key` and `event .keyCode`.
    pub fn key(&mut self, down: bool, key: &str, code: u32) {
        if let Some(v) = self.viewer() {
            v.input(UiEvent::NativeKey {
                down,
                key: key.to_string(),
                code,
            });
        }
    }

    /// The CSS cursor of the canvas (X3DPointingDeviceSensorContext.updateCursor: the cursor type in lower case).
    pub fn cursor(&self) -> &'static str {
        match &self.stage {
            Stage::Live(v) => match v.cursor {
                "POINTER" => "pointer",
                "GRABBING" => "grabbing",
                "MOVE" => "move",
                _ => "default",
            },
            _ => "default",
        }
    }

    /// The viewer: "EXAMINE", "WALK", "FLY", "PLANE", "LOOKAT" or "NONE".
    pub fn navigation_type(&self) -> &'static str {
        match &self.stage {
            Stage::Live(v) => v.viewer_kind(),
            _ => "NONE",
        }
    }

    /// The browser events since the last call: SHUTDOWN_EVENT and INITIALIZED_EVENT when an Anchor replaced the world.
    pub fn take_browser_events(&mut self) -> Vec<u32> {
        let mut events = self
            .rt_mut()
            .browser_events
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default();

        if self.own_initialized
            && let Some(i) = events
                .iter()
                .position(|&e| e == x3d_runtime::browser_load::INITIALIZED_EVENT)
        {
            events.remove(i);
            self.own_initialized = false;
        }

        events
    }

    /// The loadURL and Anchor requests of another window or a page since the last call: (target, url).
    pub fn take_foreign(&mut self) -> Vec<(String, String)> {
        self.viewer()
            .map(|v| std::mem::take(&mut v.foreign))
            .unwrap_or_default()
    }

    // --- viewpoints ----------------------------------------------------------------------------------------------

    /// X3DBrowser.firstViewpoint, previousViewpoint, nextViewpoint, lastViewpoint and changeViewpoint (name) of the
    /// active layer: `set_bind` of the viewpoint, which the next frame's events process.
    pub fn viewpoint_request(&mut self, request: &str) {
        if self.is_live() {
            self.rt_mut().browser_requests.push(request.to_string());
        }
    }

    /// X3DBrowser.viewAll (transitionTime) of the active layer.
    pub fn view_all(&mut self, transition_time: f64) {
        if let Some(v) = self.viewer() {
            v.browser_view_all(None, transition_time);
        }
    }

    /// X3DBrowserContext.getActiveViewpoint: the bound viewpoint of the active layer, `None` for its default one.
    pub fn active_viewpoint(&self) -> Option<NodeId> {
        match &self.stage {
            Stage::Live(v) => v.active_viewpoint(),
            _ => None,
        }
    }

    /// X3DLayerNode.getUserViewpoints of the active layer: the viewpoints with a description.
    pub fn user_viewpoints(&self) -> Vec<NodeId> {
        match &self.stage {
            Stage::Live(v) => v.user_viewpoints(),
            _ => vec![],
        }
    }
}

/// The world goes like on X3DBrowser.replaceWorld: `shutdown` of its nodes, then its scenes are disposed. A scene
/// that never went live is only dropped.
impl Drop for Session {
    fn drop(&mut self) {
        if let Stage::Live(v) = &mut self.stage {
            v.rt.unload_world();
        }
    }
}

/// X3DField.toString (): the VRML encoding, `TypeName { }` for node values.
fn value_string(rt: &Runtime, value: &FieldValue) -> String {
    match value {
        FieldValue::SFNode(_) | FieldValue::MFNode(_) => rt.scene.node_value_to_string(value),
        _ => rt.scene.node_value_to_vrml_string(value),
    }
}
