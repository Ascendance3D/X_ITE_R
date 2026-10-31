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

use x3d_fields::{FieldType, FieldValue, NodeId, access};
use x3d_render::traverse::Frame;
use x3d_runtime::data::{self, Data};
use x3d_runtime::{Fetcher, Runtime};
use x3d_viewer::nav::Touches;
use x3d_viewer::viewer::{UiEvent, Viewer3d};

/// The node id of an event of the world's `rootNodes` field in [`Session::take_field_events`]: no node has it (the
/// field belongs to the scene, not to a node).
pub const ROOT_NODES_EVENT: NodeId = NodeId::MAX;

/// A route of the world as the page's X3DRoute object shows it: the ends and the field names X3DRoute.getSourceField /
/// getDestinationField print (`source_field` / `destination_field`), and the names as they were given (`raw_*`), which the
/// route keeps printing after it is deleted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteInfo {
    pub id: usize,
    pub source: NodeId,
    pub source_field: String,
    pub destination: NodeId,
    pub destination_field: String,
    pub raw_source_field: String,
    pub raw_destination_field: String,
}

impl RouteInfo {
    fn of(r: &x3d_scene::scene::Route) -> RouteInfo {
        RouteInfo {
            id: r.id,
            source: r.source_node,
            source_field: r.source_field.clone(),
            destination: r.destination_node,
            destination_field: r.destination_field.clone(),
            raw_source_field: r.raw_source_field.clone(),
            raw_destination_field: r.raw_destination_field.clone(),
        }
    }
}

/// A field of a node as the page sees it: X3DField.getTypeName, the access type and `toString ()`.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldInfo {
    pub type_name: &'static str,
    pub access: &'static str,
    pub value: String,
}

/// One entry of X3DNode.getFieldDefinitions: a predefined field's definition holds the class's default value, a
/// user-defined field's definition the field itself (`default` is `None`).
#[derive(Clone, Debug, PartialEq)]
pub struct FieldDefinition {
    pub name: String,
    pub access: u8,
    pub type_name: &'static str,
    pub default: Option<Data>,
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
    /// The world was unloaded ([`Session::unload`]).
    unloaded: bool,
    /// The INITIALIZED_EVENT of the world's own replaceWorld is still to come: the page fires that one itself.
    own_initialized: bool,
}

/// A world that parsed and waits for [`Session::begin`].
pub struct ParsedWorld {
    scene: x3d_scene::Scene,
    fetcher: Box<dyn Fetcher>,
    url: String,
}

/// The scene's text in the form `kind` (`vrml`, `xml` or `json`), with X_ITE's default generator options.
fn print_scene_of(scene: &x3d_scene::Scene, kind: &str) -> Result<String, String> {
    match kind {
        "vrml" => Ok(scene.to_vrml_string()),
        "xml" => Ok(scene.to_xml_string_ctx(scene.world)),
        "json" => Ok(scene.to_json_string_ctx(scene.world)),
        _ => Err(format!("unknown text form '{kind}'")),
    }
}

/// The browser's constant setup of every parse.
fn parse_setup() {
    x3d_fields::jsmath::set_flavor(x3d_fields::jsmath::MathFlavor::Chrome);
    x3d_render::traverse::install_particle_geometry();
}

/// GoldenGate.parseIntoScene's rejection when no parser handles a file: a data URL is not named. X3DScene.worldURL is
/// a URL that a URL parser serialized, so its scheme is in lower case (`DATA:` is a data URL too).
fn no_handler(url: &str) -> String {
    if url::Url::parse(url).is_ok_and(|u| u.scheme() == "data") {
        "Error: Couldn't parse X3D. No suitable file handler found for 'data:' URL.".to_string()
    } else {
        format!("Error: Couldn't parse X3D. No suitable file handler found for '{url}'.")
    }
}

impl Session {
    /// Parse the world `data` from `url` (FileLoader). `fetcher` serves the world's Inline, EXTERNPROTO and texture
    /// URLs. A world that does not parse is an error.
    pub fn parse(data: &[u8], url: &str, fetcher: Box<dyn Fetcher>) -> Result<ParsedWorld, String> {
        Session::parse_logged(data, url, fetcher, |_| {})
    }

    /// [`Session::parse`] as FileLoader.createX3DFromURLAsync parses a candidate of the page's loadURL: the error is
    /// the rejection FileLoader.printError reports (as for an Inline), and the parser's own messages go to `log`
    /// first (`warn: …` or `error: …`; VRMLParser prints the original error in DEVELOPMENT). A glTF's buffers do not
    /// wait here; see [`Session::parse_page`].
    pub fn parse_logged(
        data: &[u8],
        url: &str,
        fetcher: Box<dyn Fetcher>,
        log: impl FnMut(String),
    ) -> Result<ParsedWorld, String> {
        parse_setup();

        let scene = x3d_runtime::parse_bytes_with_fetcher(&*fetcher, data, url).scene;

        Session::parsed(scene, url, fetcher, log)
    }

    /// [`Session::parse_logged`] for the `accept` of the page's loadURL ([`Runtime::load_page_document`] in `rt`):
    /// a glTF's buffers load through `rt`'s fetcher, which shares the responses of `fetcher`. The world waits for
    /// them (`Err (None)`: the chain goes on with this candidate when they are in), and a buffer that failed rejects
    /// the candidate like GLTF2Parser: with no error, after which the buffer's own message is logged. It does so at
    /// once, also while other buffers are in flight; those are followed ([`Runtime::follow_buffers`]).
    pub fn parse_page(
        rt: &mut Runtime,
        data: &[u8],
        url: &str,
        fetcher: Box<dyn Fetcher>,
    ) -> Result<ParsedWorld, Option<String>> {
        parse_setup();

        let mut parsed = rt.parse_candidate(data, url);

        // A buffer that failed rejects the candidate at once, also while others are still in flight.
        if let Some(thrown) = rt.gltf_buffer_failed_first(&mut parsed) {
            return Err(thrown);
        }

        if parsed.buffers_pending {
            return Err(None);
        }

        let scene = match parsed.scene {
            Err(x3d_parse::LoadError::Parse(e)) if e.line == 0 => {
                if let Some(thrown) = rt.gltf_buffer_rejection(parsed.buffer_errors, &e.message) {
                    return Err(thrown);
                }

                Err(x3d_parse::LoadError::Parse(e))
            }
            scene => scene,
        };

        Session::parsed(scene, url, fetcher, |line| rt.log.push(line)).map_err(Some)
    }

    /// The world of a parse result (`Err`: the rejection; the parser's own messages go to `log` first).
    fn parsed(
        scene: Result<x3d_scene::Scene, x3d_parse::LoadError>,
        url: &str,
        fetcher: Box<dyn Fetcher>,
        mut log: impl FnMut(String),
    ) -> Result<ParsedWorld, String> {
        let scene = match scene {
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
                return Err(no_handler(url));
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
                return Err(no_handler(url));
            }
            Err(e) => return Err(format!("Error: {e}")),
        };

        Ok(ParsedWorld {
            scene,
            fetcher,
            url: url.to_string(),
        })
    }

    /// The empty world the browser starts with and X3DBrowser.replaceWorld (null) makes: a scene no source set up, so it
    /// has no profile (its VRML text has no PROFILE line, its XML and JSON text say `Full`).
    pub fn empty_world(url: &str, fetcher: Box<dyn Fetcher>) -> ParsedWorld {
        let mut world = Session::parse_string("#X3D V4.0 utf8\n", url, fetcher)
            .expect("the empty scene parses");

        world.scene.profile = None;
        world
    }

    /// X3DScene.toVRMLString / toXMLString / toJSONString of the empty world a canvas has before any load.
    pub fn print_empty_scene(kind: &str) -> Result<String, String> {
        parse_setup();

        let mut scene =
            x3d_parse::parse_string("#X3D V4.0 utf8\n", "").map_err(|e| format!("Error: {e}"))?;

        scene.profile = None;
        print_scene_of(&scene, kind)
    }

    /// Set the parsed world up at `now` (seconds since the Unix epoch, X3DBrowser.getCurrentTime) and start its
    /// loads. The session is not live yet: see [`Session::poll_load`] and [`Session::go_live`].
    pub fn begin(world: ParsedWorld, now: f64, width: u32, height: u32) -> Session {
        Session::begin_with_scripts(world, now, width, height, |_| None)
    }

    /// [`Session::begin`] with the Script host that `scripts` gives for the world's runtime, before any Script of the
    /// world loads (`None`: the Scripts do not run; each logs that there is no script engine). The runtime does not
    /// move before [`Session::go_live`], which gives the host its new place ([`x3d_runtime::ScriptHost::attach`]).
    pub fn begin_with_scripts(
        world: ParsedWorld,
        now: f64,
        width: u32,
        height: u32,
        scripts: impl FnOnce(&mut Runtime) -> Option<Box<dyn x3d_runtime::ScriptHost>>,
    ) -> Session {
        let mut rt = Box::new(Runtime::new(world.scene, world.fetcher));

        rt.page_url = world.url.clone();
        // The page calls its field callbacks (X3DField.addFieldCallback) after the frame; the Scripts' run in the cascade.
        rt.page_field_events = Some(vec![]);
        // X_ITE 16.5.0 does not call a Script's `shutdown` when the world is replaced.
        rt.replace_skips_shutdown = true;
        // Browser.print of a Script: console.log of the page.
        rt.log_prints = true;
        // The page's objects of nodes are keyed by node id: it learns which ids were released.
        rt.page_released = Some(vec![]);
        x3d_render::shader_gpu::install(&mut rt);
        rt.live = true;
        rt.now = now;
        rt.browser_options = x3d_runtime::browser_options::BrowserOptions::new(width, height);
        rt.script_host = scripts(&mut *rt);
        rt.begin_load_scene();

        let mut session = Session {
            stage: Stage::Loading(rt, false),
            size: (width, height),
            log_seen: 0,
            unloaded: false,
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

        // The runtime moved into the viewer: the Script host reaches it there.
        let rt = self.rt_mut();

        if let Some(mut host) = rt.script_host.take() {
            host.attach(rt);
            rt.script_host = Some(host);
        }

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

    /// Has a Sound node of this world made the audio context (X3DSoundContext.getAudioContext)?
    pub fn has_audio(&self) -> bool {
        self.rt().audio.is_some()
    }

    /// Did a sound source or MovieTexture of this world start its media since the last call
    /// ([`Runtime::audio_element_started`])? Clears the flag.
    pub fn take_audio_element_started(&mut self) -> bool {
        std::mem::take(&mut self.rt_mut().audio_element_started)
    }

    /// The calls the world's audio engine made on its Web Audio nodes since the last call, with the engine's number
    /// ([`x3d_runtime::sound::AudioEngine::take_ops`]); the page makes them on real nodes of its AudioContext.
    pub fn take_audio_ops(&self) -> Option<(u64, Vec<x3d_runtime::sound::ops::Op>)> {
        let audio = self.rt().audio.as_ref()?;

        Some((audio.uid(), audio.take_ops()))
    }

    /// The page's audio clock (`audioContext .currentTime`) for the world's audio context, if it has one
    /// ([`x3d_runtime::sound::AudioEngine::set_clock`]).
    pub fn set_audio_clock(&mut self, time: f64) {
        if let Some(audio) = self.rt_mut().audio.as_mut() {
            audio.set_clock(time);
        }
    }

    /// `setBrowserOption ("Mute", mute)` and `("SoundIntensity", intensity)`: the runtime's options take the page's
    /// values, and its BrowserOptions.set_SoundIntensity__ sets the gain of the sound destinations in the next event
    /// cascade.
    pub fn set_sound_options(&mut self, mute: bool, intensity: f64) {
        set_sound_options(self.rt_mut(), mute, intensity);
    }

    /// The world's audio graph ([`x3d_runtime::sound::AudioEngine::graph`]), or `None` while no Sound node has made the audio
    /// context.
    pub fn audio_graph(&self) -> Option<x3d_runtime::sound::AudioGraph> {
        Some(self.rt().audio.as_ref()?.graph())
    }

    /// The world's audio graph as JSON (a diagnostic snapshot, [`x3d_runtime::sound::AudioGraph`]), or `None` while
    /// no Sound node has made the audio context (X3DSoundContext.getAudioContext). Numbers are JavaScript numbers:
    /// an `f32` is widened to `f64`, as a page reads an AudioParam's float value. `listener` is the nine values of
    /// the context's listener: position, forward and up.
    pub fn audio_graph_json(&self) -> Option<String> {
        let audio = self.rt().audio.as_ref()?;
        let g = audio.graph();
        let num = |v: f64| {
            if v.is_finite() {
                format!("{v:?}")
            } else {
                "null".into()
            }
        };
        let opt = |v: Option<f64>| v.map_or("null".into(), num);
        let nodes: Vec<String> = g
            .nodes
            .iter()
            .map(|n| {
                let source = n.source.as_ref().map_or("null".into(), |s| {
                    format!(
                        r#"{{"buffer":{},"loop":{},"loopStart":{},"loopEnd":{},"started":{},"stopped":{}}}"#,
                        s.buffer.map_or("null".into(), |(c, l, r)| format!(
                            r#"{{"numberOfChannels":{c},"length":{l},"sampleRate":{}}}"#,
                            num(r as f64)
                        )),
                        s.loop_,
                        num(s.loop_start),
                        num(s.loop_end),
                        s.started.map_or("null".into(), |(w, o)| format!("[{},{}]", num(w), num(o))),
                        opt(s.stopped),
                    )
                });

                format!(
                    r#"{{"id":{},"serial":{},"kind":"{}","numberOfInputs":{},"numberOfOutputs":{},"channelCount":{},"channelCountMode":"{}","channelInterpretation":"{}","gain":{},"playbackRate":{},"source":{}}}"#,
                    n.id,
                    n.serial,
                    n.kind,
                    n.number_of_inputs,
                    n.number_of_outputs,
                    n.channel_count,
                    n.channel_count_mode,
                    n.channel_interpretation,
                    opt(n.gain.map(f64::from)),
                    opt(n.playback_rate.map(f64::from)),
                    source,
                )
            })
            .collect();
        // audioContext .listener .{position,forward,up}{X,Y,Z} .value, as ListenerPointSource sets them.
        let listener: Vec<String> = audio
            .listener_values()
            .iter()
            .map(|&v| num(f64::from(v)))
            .collect();
        let connections: Vec<String> = g
            .connections
            .iter()
            .map(|(a, o, b, i)| format!("[{a},{o},{b},{i}]"))
            .collect();

        Some(format!(
            r#"{{"nodes":[{}],"connections":[{}],"destination":{},"currentTime":{},"sampleRate":{},"listener":[{}]}}"#,
            nodes.join(","),
            connections.join(","),
            g.destination,
            num(g.current_time),
            num(g.sample_rate as f64),
            listener.join(","),
        ))
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

    /// X3DBrowser.createVrmlFromString (text) of a page (an external browser): the root nodes of the new scene, made in
    /// the world's arena so the page can put them into a field of the world. The scene's world URL is the page's `base`.
    /// The error is the thrown Error's `String (error)`.
    pub fn create_vrml_from_string(
        &mut self,
        text: &str,
        base: &str,
    ) -> Result<Vec<Option<NodeId>>, String> {
        let rt = self.rt_mut();
        let world = rt.scene.world;

        rt.create_vrml_from_string_at(text, world, base)
    }

    /// X3DBrowser.createX3DFromURL (url, node, event) / createVrmlFromURL of a page (an external browser): FileLoader for
    /// the X3DWorld in the world's own runtime, so the root nodes of the scene are the world's and can go into a field of
    /// it. The urls resolve against the page's `base`. The id of the request: [`Session::take_scene_requests`] gives its
    /// outcome once the scene is loaded and live.
    pub fn create_x3d_from_url(&mut self, urls: Vec<String>, base: &str) -> u64 {
        self.rt_mut().browser_create_from_page_url(urls, base)
    }

    /// The requests of [`Session::create_x3d_from_url`] that settled since the last call: the root nodes of the scene,
    /// or the rejection's message (`Couldn't load X3D file.`).
    pub fn take_scene_requests(&mut self) -> Vec<(u64, Result<Vec<Option<NodeId>>, String>)> {
        let rt = self.rt_mut();

        rt.take_page_browser_settled()
            .into_iter()
            .map(|(id, outcome)| (id, outcome.map(|ctx| rt.scene.ctx(ctx).root_nodes.clone())))
            .collect()
    }

    /// X3DScene.profile and components of the world: the name of its profile (`None`: the scene has none) and its
    /// components (name, level), in the order X_ITE's ComponentInfoArray holds them.
    pub fn profile_and_components(&self) -> (Option<String>, Vec<(String, i32)>) {
        let scene = &self.rt().scene;

        (
            scene.profile_of(scene.world).map(String::from),
            scene.components_of(scene.world).to_vec(),
        )
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

    /// `rootNodes .assign (list)` of the world (X3DScene.rootNodes of the page, addRootNode, removeRootNode,
    /// setRootNodes): the world holds `list` at once and the `rootNodes` field sends its event in the next frame's
    /// event cascade, among the other events in the order they were made (see [`ROOT_NODES_EVENT`]).
    pub fn set_root_nodes(&mut self, list: Vec<Option<NodeId>>) -> Result<(), String> {
        if let Some(n) = list.iter().flatten().find(|&&n| !self.node_exists(n)) {
            return Err(format!("rootNodes: node {n} is not in the world"));
        }

        let rt = self.rt_mut();
        let world = rt.scene.world;

        // The runtime queues the event of the page's rootNodes field (also for a Script's change).
        rt.set_root_nodes(world, list);
        Ok(())
    }

    /// Does the page call field callbacks for the world's `rootNodes` (X3DField.addFieldCallback)?
    pub fn observe_root_nodes(&mut self, on: bool) {
        let rt = self.rt_mut();
        let k = rt.page_root_nodes_field();

        if on {
            rt.page_observed.insert(k);
        } else {
            rt.page_observed.remove(&k);
        }
    }

    /// X3DExecutionContext.routes of the world: the routes in the order they were added.
    pub fn routes(&self) -> Vec<RouteInfo> {
        let scene = &self.rt().scene;

        scene
            .ctx(scene.world)
            .routes
            .iter()
            .map(|r| RouteInfo::of(r))
            .collect()
    }

    /// The route with these ends (X3DExecutionContext.getRoute without the node checks, which are the page's): the fields
    /// must exist and X3DRoute.checkFields must accept them (`Err`, the text X_ITE throws), else the route or `None`.
    pub fn get_route(
        &self,
        source: NodeId,
        source_field: &str,
        destination: NodeId,
        destination_field: &str,
    ) -> Result<Option<RouteInfo>, String> {
        self.route_nodes(source, destination)?;

        let scene = &self.rt().scene;

        scene.route_ends(source, source_field, destination, destination_field)?;

        Ok(scene
            .find_route_index(
                scene.world,
                source,
                source_field,
                destination,
                destination_field,
            )
            .map(|i| RouteInfo::of(&scene.ctx(scene.world).routes[i])))
    }

    /// X3DExecutionContext.addRoute: the route that exists with these ends, else a new one that carries events from now on.
    pub fn add_route(
        &mut self,
        source: NodeId,
        source_field: &str,
        destination: NodeId,
        destination_field: &str,
    ) -> Result<RouteInfo, String> {
        if let Some(route) = self.get_route(source, source_field, destination, destination_field)? {
            return Ok(route);
        }

        let rt = self.rt_mut();
        let world = rt.scene.world;

        rt.add_route(world, source, source_field, destination, destination_field)?;

        // The route is the last one of the world's list.
        Ok(RouteInfo::of(
            rt.scene
                .ctx(world)
                .routes
                .last()
                .expect("add_route made a route"),
        ))
    }

    /// X3DExecutionContext.deleteRoute (route): the route with this id leaves the world; whether there was one.
    pub fn delete_route(&mut self, id: usize) -> bool {
        let rt = self.rt_mut();
        let world = rt.scene.world;

        rt.delete_route_by_id(world, id)
    }

    fn route_nodes(&self, source: NodeId, destination: NodeId) -> Result<(), String> {
        if !self.node_exists(source) || !self.node_exists(destination) {
            return Err("node is not in the world".to_string());
        }

        Ok(())
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

    /// X3DObject.toVRMLString / toXMLString / toJSONString of an SFNode or MFNode value (`ty`, `data` of the page): the
    /// nodes in full, named by their own execution context, as the Script engine prints them. `kind` is `vrml`, `xml`
    /// or `json`. A node that is not in the world is an error.
    ///
    /// Checks run in this order: the field type (only SFNode and MFNode), the data shape, that every node is in the world,
    /// then `kind`. This is an adapter contract, not an X_ITE one: X_ITE has no such call (its `toVRMLString` takes no
    /// `kind`), so the order is not compared with the pinned build. Every failure is an `Err` before any text is made.
    pub fn print_nodes(
        &mut self,
        kind: &str,
        ty: FieldType,
        data: &Data,
    ) -> Result<String, String> {
        if !matches!(ty, FieldType::SFNode | FieldType::MFNode) {
            return Err(format!("{} is not a node field type", ty.name()));
        }

        data::check_data(ty, data)?;

        if let Some(n) = data::data_nodes(ty, data)
            .into_iter()
            .find(|&n| !self.node_exists(n))
        {
            return Err(format!("node {n} is not in the world"));
        }

        // X3DGenerator.NeedsName reads each node's clone count as it is now, not as it was when the world loaded.
        self.rt_mut().publish_clone_counts();

        let scene = &self.rt().scene;
        let value = data::data_value(ty, data);

        match kind {
            "vrml" => Ok(scene.node_value_to_vrml_string(&value)),
            "xml" => Ok(scene.field_value_to_xml_string(&value)),
            "json" => Ok(scene.field_value_to_json_string(&value)),
            _ => Err(format!("unknown text form '{kind}'")),
        }
    }

    /// X3DScene.toVRMLString / toXMLString / toJSONString of the world, with X_ITE's default generator options; `kind` is
    /// `vrml`, `xml` or `json`.
    pub fn print_scene(&mut self, kind: &str) -> Result<String, String> {
        self.rt_mut().publish_clone_counts();
        print_scene_of(&self.rt().scene, kind)
    }

    /// X3DField.fromVRMLString / fromXMLString (value, scene) for a field of type `ty` named `name`: the data of the
    /// value read from `text`. The nodes it makes belong to this world and are set up; `unit` converts the values. A
    /// value that cannot be read is an error with X_ITE's message.
    pub fn parse_field(
        &mut self,
        ty: FieldType,
        text: &str,
        xml: bool,
        name: &str,
        unit: Option<String>,
    ) -> Result<Data, String> {
        let ctx = Some(self.rt().scene.world);

        match self
            .rt_mut()
            .field_from_string(ty, text, ctx, xml, unit, name)
        {
            Ok(Some(value)) => Ok(data::value_data(&value)),
            Ok(None) => Err(format!("Couldn't read value for field '{name}'.")),
            Err(Some(message)) => Err(message),
            Err(None) => Err("bad field value".to_string()),
        }
    }

    // --- field objects of the page (X_ITE's own field classes, mirrored with the runtime) -------------------------

    /// The type, access type and data (x3d_runtime::data) of a node field. The page holds an object of the field from
    /// now on: [`Session::take_field_changes`] reports its changes.
    pub fn field_data(
        &mut self,
        node: NodeId,
        name: &str,
    ) -> Result<(&'static str, u8, Data), String> {
        let k = self.field_key(node, name)?;
        let rt = self.rt_mut();

        rt.page_mirror.entry(k).or_insert(false);

        let f = rt.field(k);

        Ok((f.ty().name(), f.access, data::value_data(&f.value)))
    }

    /// X3DField.addEvent of the page's object of a node field: the field takes the object's value and sends its event
    /// in the next frame's event cascade, as X3DField.setValue does. The access type is not checked here: X_ITE's
    /// SFNode proxy refuses an outputOnly field, a field object of it does not.
    pub fn set_field_data(&mut self, node: NodeId, name: &str, value: &Data) -> Result<(), String> {
        let k = self.field_key(node, name)?;
        let ty = self.rt().field(k).ty();

        data::check_data(ty, value).map_err(|e| format!("field '{name}': {e}"))?;

        if let Some(n) = data::data_nodes(ty, value)
            .into_iter()
            .find(|&n| !self.node_exists(n))
        {
            return Err(format!("field '{name}': node {n} is not in the world"));
        }

        let rt = self.rt_mut();
        let value = data::data_value(ty, value);

        rt.set_value(k, value);
        // The page's object holds this value already.
        rt.page_clean(k);
        Ok(())
    }

    /// The node fields the page holds objects of whose value changed since the last call: node, field name, data.
    pub fn take_field_changes(&mut self) -> Vec<(NodeId, String, Data)> {
        let rt = self.rt_mut();

        rt.take_page_dirty()
            .into_iter()
            .map(|k| {
                let f = rt.field(k);

                (k.0, f.name.clone(), data::value_data(&f.value))
            })
            .collect()
    }

    /// Does the page call field callbacks for this field (X3DField.addFieldCallback)?
    pub fn observe_field(&mut self, node: NodeId, name: &str, on: bool) -> Result<(), String> {
        let k = self.field_key(node, name)?;
        let rt = self.rt_mut();

        if on {
            rt.page_observed.insert(k);
        } else {
            rt.page_observed.remove(&k);
        }

        Ok(())
    }

    /// The events of the observed fields since the last call, in event order: node, field name and the value each
    /// event delivered (X3DField.processEvent calls the callbacks with it).
    pub fn take_field_events(&mut self) -> Vec<(NodeId, String, Data)> {
        self.take_field_events_logged()
            .into_iter()
            .map(|(node, name, data, _)| (node, name, data))
            .collect()
    }

    /// Like [`Session::take_field_events`], with the number of log lines (those [`Session::take_log`] has not given yet)
    /// that came before each event: the page prints them before it calls the event's callbacks, as X_ITE does. Call
    /// [`Session::take_log`] right after, so the numbers count from the same line.
    pub fn take_field_events_logged(&mut self) -> Vec<(NodeId, String, Data, usize)> {
        let seen = self.log_seen;
        let rt = self.rt_mut();

        // The release sweep drops the events of released nodes; a node that is not live is skipped all the same, so
        // no event reads a released slot.
        let root_nodes = rt.private_field(rt.browser_node, "rootNodes");

        rt.take_page_field_events()
            .into_iter()
            .filter(|(k, _, _)| rt.scene.is_live(k.0))
            .map(|(k, value, log_len)| {
                let before = log_len.saturating_sub(seen);

                if Some(k) == root_nodes {
                    // X3DField.processEvent: `callback (this .valueOf ())`, the live field, so the current root nodes.
                    let roots = rt.scene.ctx(rt.scene.world).root_nodes.clone();

                    return (
                        ROOT_NODES_EVENT,
                        "rootNodes".to_string(),
                        data::value_data(&FieldValue::MFNode(roots)),
                        before,
                    );
                }

                (
                    k.0,
                    rt.field(k).name.clone(),
                    data::value_data(&value),
                    before,
                )
            })
            .collect()
    }

    /// A page field callback threw: stop the event processing for good (see [`Runtime::halt_events`]).
    pub fn halt_events(&mut self) {
        self.rt_mut().halt_events();
    }

    /// The nodes the runtime released since the last call: their ids may name other nodes from now on, so the page
    /// drops its objects of them.
    pub fn take_released_nodes(&mut self) -> Vec<NodeId> {
        self.rt_mut().take_page_released()
    }

    /// X3DNode.getFieldDefinitions: the predefined fields with the class's default values, then the user-defined
    /// fields.
    pub fn field_definitions(&self, node: NodeId) -> Vec<FieldDefinition> {
        if !self.node_exists(node) {
            return vec![];
        }

        let scene = &self.rt().scene;
        let n = scene.node(node);
        let defaults = match &n.kind {
            x3d_scene::NodeKind::Builtin(t) => Some(
                &scene.table.types[*t]
                    .variant(scene.spec_version_of(n.ctx))
                    .fields,
            ),
            _ => None,
        };
        let definition = |f: &x3d_scene::Field, default: Option<Data>| FieldDefinition {
            name: f.name.clone(),
            access: f.access,
            type_name: f.ty().name(),
            default,
        };

        n.fields
            .iter()
            .map(|f| {
                let default = defaults
                    .and_then(|d| d.iter().find(|d| d.name == f.name))
                    .map(|d| data::value_data(&d.default));

                definition(f, default)
            })
            .chain(n.user_fields.iter().map(|f| definition(f, None)))
            .collect()
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

    /// X3DBrowser.viewAll (transitionTime) of the active layer, or of `layer` (a node id the page checked to be an
    /// X3DLayerNode of this world).
    pub fn view_all(&mut self, layer: Option<NodeId>, transition_time: f64) {
        if let Some(v) = self.viewer() {
            let layer = layer.filter(|&n| v.rt.scene.is_live(n));

            v.browser_view_all(layer, transition_time);
        }
    }

    /// X3DBrowserContext.setLive of the browser (beginUpdate, endUpdate): the live state of the world. A world that is
    /// not live pauses its TimeSensors and the other nodes that follow the live state; the events of the page's own
    /// field assignments still go through. The state outlives a world: [`WebBrowser::replace`] gives it to the next one.
    pub fn set_live(&mut self, live: bool) {
        match &mut self.stage {
            Stage::Loading(rt, _) => rt.live = live,
            Stage::Live(v) => {
                v.rt.live = live;
                // A world that started while the browser was not live still has to go live (scene .setLive (true)).
                v.rt.go_live();
                v.rt.refresh_live();
            }
            Stage::Gone => {}
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
        self.unload();
    }
}

impl Session {
    /// The world goes (X3DBrowser.replaceWorld): `shutdown` of its nodes, then its scenes are disposed, once. Its last
    /// log lines (what `shutdown` printed) stay for [`Session::take_log`].
    pub fn unload(&mut self) {
        if let Stage::Live(v) = &mut self.stage {
            if !std::mem::replace(&mut self.unloaded, true) {
                v.rt.unload_world();
            }
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

/// The page's Mute and SoundIntensity options for a runtime (see [`Session::set_sound_options`]).
pub fn set_sound_options(rt: &mut Runtime, mute: bool, intensity: f64) {
    use x3d_runtime::browser_options::OptionValue;

    rt.browser_options
        .set_option("Mute", OptionValue::Bool(mute));
    rt.browser_options
        .set_option("SoundIntensity", OptionValue::Float(intensity));
}
