//! The Inline node and the life of the scenes it loads (Components/Networking/Inline.js,
//! X3DUrlObject.js, InputOutput/FileLoader.js createX3DFromString / setScene / set_loadCount__).
//!
//! X_ITE has one browser: one event queue, one set of browser events (prepareEvents …), one
//! script realm. The scene an Inline loads is an X3DScene of that browser, so here it is merged
//! into the runtime's node arena ([`x3d_scene::Scene::merge_inline`]) as a context of its own,
//! and every runtime facility (routing, scripts, TimeSensors, followers) treats its nodes like
//! any other.
//!
//! Timeline of one load, as in X_ITE (each step is observable through scripts):
//!
//! 1. `requestImmediateLoad`: IN_PROGRESS, `loadData` fetches (a macrotask, see `Runtime::run_tasks`).
//! 2. The fetch completes: the scene is parsed and its nodes are set up. A Script's `initialize ()`
//!    runs in the microtasks that follow, before the scene is attached and before any frame.
//! 3. `scene ._loadCount` gets an event; `set_loadCount__` waits until the count is 0 (nested
//!    Inlines hold it) and then for `browser .nextFrame ()`: the finishEvents of the next frame.
//! 4. `setInternalScene`: the old scene is disposed, the new one is attached and made live
//!    (TimeSensors resume), the load state becomes COMPLETE.
//!
//! What belongs to which authority (R0.3-C):
//!
//! * VRML97 6.25 / X3D 4.0 Inline: the `url` list is tried in order of preference; an empty list
//!   does nothing; the results are undefined if a file changes after it was loaded. X3D adds `load`
//!   (FALSE: nothing is loaded), `autoRefresh` and `autoRefreshTimeLimit` (X3DUrlObject). VRML97 has
//!   none of these three fields; this port does not add them to VRML97 syntax, it takes whatever the
//!   parser accepts, as X_ITE does.
//! * X_ITE (not in either standard): `loadNow`, `unloadNow`, the `loadData` buffer (several requests
//!   before the next frame make one load), the frame on which each step happens.
//!
//! Where the pinned X_ITE (34cdf9f97) is unsafe, this port differs on purpose (tests in
//! `crates/x3d-script/tests/inline_reload*.rs` name each one):
//!
//! * Load identity. Every request has a generation (`UrlState::generation`), as the EXTERNPROTO
//!   declarations of R0.3-B. A scene that finishes for an older generation (a slower earlier request,
//!   or a request cancelled by `load` FALSE) is discarded; X_ITE lets it replace the current result
//!   or show a disabled Inline.
//! * A scene that is disposed takes its nested Inlines with it: their loads are cancelled, their
//!   scenes are not live, the Scripts in them stop receiving `prepareEvents` and `eventsProcessed`.
//!   X_ITE leaves all of that running.
//! * A scene that contains itself (directly, or through other scenes) is not loaded again: the
//!   candidate fails with an error. X_ITE loads it without end.
//! * A parse error prints the parser's message first, as VRMLParser does.
//! * No automatic refresh after a failed load (X3D 4.0, 9.3.2 X3DUrlObject); X_ITE keeps refreshing.

use crate::loader::{FileLoader, Loaded};
use crate::nodes::LoadState;
use crate::*;

/// A scene that was parsed and set up and waits for its loadCount to reach 0.
#[derive(Clone, Debug)]
pub struct PendingScene {
    /// The Inline node.
    pub node: NodeId,
    /// The request that fetched the scene.
    pub generation: u64,
    /// Root context of the scene.
    pub ctx: CtxId,
    /// The internal node that owns the scene's `loadCount` field.
    pub scene_node: NodeId,
}

/// `await browser .nextFrame ()` of FileLoader.set_loadCount__: setInternalScene runs when the
/// frame's finishEvents are done.
#[derive(Clone, Debug)]
pub struct FrameWaiter {
    pub node: NodeId,
    pub generation: u64,
    pub ctx: CtxId,
}

const NOT_STARTED: i32 = 0;
const IN_PROGRESS: i32 = 1;
const COMPLETE: i32 = 2;
const FAILED: i32 = 3;

impl Runtime {
    /// Inline and InlineGeometry: both ask a FileLoader for an X3DScene (`createX3DFromURL`) and keep it as their
    /// internal scene.
    pub(crate) fn is_scene_loader(&self, node: NodeId) -> bool {
        matches!(self.builtin_name(node), Some("Inline" | "InlineGeometry"))
    }

    /// The X3DUrlObject load state of a node.
    /// The url an ImageTexture shows: `Some (url of the last load request)`, `Some ([])` before
    /// any request, `None` for a node without url object state (then its field is used).
    pub fn texture_url(&self, node: NodeId) -> Option<Vec<String>> {
        match self.nrt(node).map(|n| &n.state) {
            Some(nodes::State::UrlObject(s)) => Some(s.requested_url.clone().unwrap_or_default()),
            _ => None,
        }
    }

    pub fn url_load_state(&self, node: NodeId) -> LoadState {
        match self.nrt(node).map(|n| &n.state) {
            Some(nodes::State::UrlObject(s)) => s.load_state,
            Some(nodes::State::Script(s)) => s.load_state,
            _ => LoadState::NotStarted,
        }
    }

    /// X3DUrlObject.setLoadState for the nodes that fetch: updates the state, sends the
    /// `loadState` event of an Inline and keeps the loadCount of its scene.
    pub(crate) fn set_load_state(&mut self, node: NodeId, state: LoadState) {
        self.set_load_state_node(node, state);

        // X3DUrlObject.setLoadState: every url object (a Script, a texture) is a loading object of its scene while it loads.
        if !self.is_scene_loader(node)
            && self.nrt(node).is_some_and(|n| {
                matches!(
                    n.state,
                    nodes::State::UrlObject(_) | nodes::State::Script(_)
                )
            })
        {
            self.update_load_count(self.scene.scene_root(self.scene.node(node).ctx));
        }
    }

    fn set_load_state_node(&mut self, node: NodeId, state: LoadState) {
        match &mut self.nrt_mut(node).state {
            nodes::State::UrlObject(s) => s.load_state = state,
            nodes::State::Script(s) => s.load_state = state,
            _ => return,
        }

        // `this ._loadState = value` of every X3DUrlObject.
        if let Some(k) = self.private_field(node, "loadState") {
            let value = match state {
                LoadState::NotStarted => NOT_STARTED,
                LoadState::InProgress => IN_PROGRESS,
                LoadState::Complete => COMPLETE,
                LoadState::Failed => FAILED,
            };

            self.set_value(k, FieldValue::SFInt32(value));
        }

        // BufferAudioSource, AudioClip: X3DUrlObject.setLoadState restarts the auto refresh timer after
        // COMPLETE and after FAILED.
        if matches!(
            self.builtin_name(node),
            Some("BufferAudioSource" | "AudioClip")
        ) {
            if matches!(state, LoadState::Complete | LoadState::Failed) {
                self.inline_auto_refresh_complete(node);
            }

            return;
        }

        if !self.is_scene_loader(node) {
            return;
        }

        // X3DUrlObject.setLoadState: COMPLETE restarts the auto refresh timer. X_ITE does so after
        // FAILED too; X3D 4.0 (9.3.2 X3DUrlObject) says "if the preceding file loading fails ... no
        // automatic refresh is performed", and this port follows the standard.
        match state {
            LoadState::Complete => self.inline_auto_refresh_complete(node),
            LoadState::Failed => {
                if let Some(s) = self.url_state_mut(node) {
                    s.timer = None;
                }
            }
            _ => {}
        }

        // X3DScene.addLoadingObject / removeLoadingObject: `_loadCount = size`.
        self.update_load_count(self.scene.scene_root(self.scene.node(node).ctx));
    }

    /// `scene ._loadCount = loadingObjects .size` for a scene that has not been attached yet: the
    /// Inlines and EXTERNPROTO declarations of the scene that are loading.
    pub(crate) fn update_load_count(&mut self, root: CtxId) {
        let scene_node = self
            .pending_scenes
            .iter()
            .find(|p| p.ctx == root)
            .map(|p| p.scene_node)
            .or_else(|| {
                self.pending_browser
                    .iter()
                    .find(|p| p.ctx == root)
                    .map(|p| p.scene_node)
            })
            .or_else(|| {
                self.pending_externs
                    .iter()
                    .find(|p| p.ctx == root)
                    .map(|p| p.scene_node)
            });

        let Some(scene_node) = scene_node else {
            return;
        };

        let inlines = (0..self.scene.nodes.len() as NodeId)
            .filter(|&n| {
                self.nrt(n).is_some_and(|r| {
                    matches!(
                        r.state,
                        nodes::State::UrlObject(_) | nodes::State::Script(_)
                    )
                }) && self.url_load_state(n) == LoadState::InProgress
                    && self.scene.scene_root(self.scene.node(n).ctx) == root
            })
            .count();
        let externs = self
            .extern_states
            .iter()
            .filter(|(e, s)| {
                s.load_state == LoadState::InProgress
                    && self.scene.scene_root(self.scene.externprotos[**e].ctx) == root
            })
            .count();

        let k = self.private_field(scene_node, "loadCount").unwrap();
        self.set_value(k, FieldValue::SFInt32((inlines + externs) as i32));
    }

    /// Inline.initialize: X3DUrlObject.initialize, then `requestImmediateLoad ()`.
    pub(crate) fn inline_initialize(&mut self, node: NodeId) {
        self.interest(node, "load", Cb::UrlSetLoad);
        self.interest(node, "url", Cb::UrlSetUrl);

        if self.builtin_name(node) == Some("InlineGeometry") {
            self.interest(node, "solid", Cb::InlineGeometrySolid);
            self.interest(node, "smooth", Cb::InlineGeometrySmooth);
        }

        // FontLibrary buffers its requests like an Inline (`_loadData`), but has no auto refresh timer here.
        if self.builtin_name(node) == Some("FontLibrary") {
            if let Some(k) = self.private_field(node, "loadData") {
                self.add_interest(k, node, Cb::UrlLoadData);
            }
        }

        if self.is_scene_loader(node) {
            if let Some(k) = self.private_field(node, "loadData") {
                self.add_interest(k, node, Cb::UrlLoadData);
            }

            self.interest(node, "autoRefresh", Cb::UrlSetAutoRefresh);
            self.interest(node, "autoRefreshTimeLimit", Cb::UrlSetAutoRefresh);
        }

        self.request_immediate_load(node);
    }

    /// X3DUrlObject.requestImmediateLoad (a rejected promise is caught by the callers).
    pub(crate) fn request_immediate_load(&mut self, node: NodeId) {
        if self.url_load_state(node) != LoadState::NotStarted {
            return;
        }

        let script = self.builtin_name(node) == Some("Script");

        if self.url_state_missing(node) && !script {
            return;
        }

        // A Script of a disposed scene takes no more loads.
        if script
            && matches!(self.nrt(node).map(|n| &n.state), Some(nodes::State::Script(s)) if s.closed)
        {
            return;
        }

        // `if (!browser .getBrowserOption ("LoadUrlObjects") && …) return;` (no private scene here).
        if !self.browser_options.load_url_objects() {
            return;
        }

        if !self.get_bool(node, "load") {
            return;
        }

        if self.url_strings(node).is_empty() {
            self.unload_data(node);
            return;
        }

        self.set_load_state(node, LoadState::InProgress);

        let urls = self.url_strings(node);

        if let nodes::State::UrlObject(st) = &mut self.nrt_mut(node).state {
            st.requested_url = Some(urls);
        }

        // `if (this .isInitialized ()) this ._loadData = time; else this .loadData ()`: the field
        // event is the buffer that makes several requests before it is processed one load.
        if matches!(
            self.builtin_name(node),
            Some(
                "Inline"
                    | "InlineGeometry"
                    | "FontLibrary"
                    | "Script"
                    | "BufferAudioSource"
                    | "AudioClip"
            )
        ) && self.nrt(node).is_some_and(|n| n.initialized)
        {
            if let Some(k) = self.private_field(node, "loadData") {
                let t = self.now;
                self.set_value(k, FieldValue::SFTime(t));
                return;
            }
        }

        self.url_start_load(node);
    }

    fn url_state_missing(&self, node: NodeId) -> bool {
        !matches!(
            self.nrt(node).map(|n| &n.state),
            Some(nodes::State::UrlObject(_))
        )
    }

    fn url_state_mut(&mut self, node: NodeId) -> Option<&mut nodes::UrlState> {
        match &mut self.nrt_mut(node).state {
            nodes::State::UrlObject(s) => Some(s),
            _ => None,
        }
    }

    /// Inline.loadData: `this .fileLoader ?.abort ()`, then a new FileLoader whose fetch is a macrotask.
    /// Only an Inline has a request identity; the other nodes that fetch keep generation 0.
    fn url_start_load(&mut self, node: NodeId) {
        let generation = if self.is_scene_loader(node) {
            self.inline_abort(node)
        } else {
            0
        };

        // BufferAudioSource.loadData, AudioClip.loadData: `this .setMediaElement (null)`, then the FileLoader.
        if matches!(
            self.builtin_name(node),
            Some("BufferAudioSource" | "AudioClip")
        ) {
            self.buffer_load_start(node);
        }

        self.tasks.push_back(Task::UrlLoad(node, generation));
    }

    /// FileLoader.abort () of the node's current request: whatever that request still delivers is
    /// recognized as old and dropped. A scene it already fetched and parsed is discarded now.
    /// Returns the identity of the request that follows.
    fn inline_abort(&mut self, node: NodeId) -> u64 {
        let generation = self.url_state_mut(node).map_or(0, |s| {
            s.generation += 1;
            s.generation
        });

        for p in self
            .pending_scenes
            .iter()
            .filter(|p| p.node == node)
            .cloned()
            .collect::<Vec<_>>()
        {
            self.pending_scenes.retain(|q| q.ctx != p.ctx);
            self.dispose_scene(p.ctx);
        }

        for w in self
            .frame_waiters
            .iter()
            .filter(|w| w.node == node)
            .cloned()
            .collect::<Vec<_>>()
        {
            self.frame_waiters.retain(|q| q.ctx != w.ctx);
            self.dispose_scene(w.ctx);
        }

        generation
    }

    /// Is `generation` the request the node is waiting for? A disabled, replaced, cancelled or
    /// disposed Inline is not waiting for anything.
    pub(crate) fn inline_request_is_current(&self, node: NodeId, generation: u64) -> bool {
        let current = matches!(self.nrt(node).map(|n| &n.state), Some(nodes::State::UrlObject(s)) if s.generation == generation && s.load_state == LoadState::InProgress);

        current && !self.ctx_dead(self.scene.node(node).ctx)
    }

    /// X3DUrlObject.requestUnload.
    fn request_unload(&mut self, node: NodeId) {
        let state = self.url_load_state(node);

        if state == LoadState::NotStarted || state == LoadState::Failed {
            return;
        }

        self.unload_data(node);
        self.set_load_state(node, LoadState::NotStarted);
    }

    /// `unloadData ()`: an Inline aborts its request and replaces its scene by the browser's empty
    /// default scene.
    fn unload_data(&mut self, node: NodeId) {
        match self.builtin_name(node) {
            Some("Inline") => {
                self.inline_abort(node);
                self.set_internal_scene(node, SceneArg::Default);
            }
            // InlineGeometry.unloadData: `fileLoader ?.abort ()`, `unlockGeometry ()`, `setInternalScene (null)`.
            Some("InlineGeometry") => {
                self.inline_abort(node);
                self.set_internal_scene(node, SceneArg::None);
            }
            Some("Script") => self.script_unload_data(node),
            Some("BufferAudioSource") => self.buffer_unload_data(node),
            Some("AudioClip") => self.audio_clip_unload_data(node),
            // ImageTexture.unloadData: clearTexture (an image still decoding is dropped).
            Some("ImageTexture") => {
                self.decoded_images.retain(|(n, ..)| *n != node);
                self.set_texture_2d_size(node, 1, 1)
            }
            // ImageTextureAtlas.unloadData: clearTexture, updateOutputs (0, 0, 0, 0).
            Some("ImageTextureAtlas") => self.image_texture_atlas_clear(node),
            // ImageTexture3D.unloadData: clearTexture, updateOutputs (0, 0, 0, 0).
            Some("ImageTexture3D") => self.image_texture_3d_clear(node),
            _ => {}
        }
    }

    /// X3DUrlObject.loadNow: start over from NOT_STARTED.
    pub fn inline_load_now(&mut self, node: NodeId) {
        self.set_load_state(node, LoadState::NotStarted);
        self.request_immediate_load(node);
    }

    /// X3DUrlObject.unloadNow.
    pub fn inline_unload_now(&mut self, node: NodeId) {
        self.request_unload(node);
    }

    /// X3DUrlObject.set_load__ / set_url__ / loadData / set_autoRefresh__ (the interests).
    pub(crate) fn url_object_changed(&mut self, node: NodeId, cb: Cb) {
        match cb {
            Cb::UrlSetLoad => {
                if self.get_bool(node, "load") {
                    self.request_immediate_load(node);
                } else {
                    self.request_unload(node);
                }
            }
            Cb::UrlSetUrl => {
                if !self.get_bool(node, "load") {
                    return;
                }

                self.set_load_state(node, LoadState::NotStarted);
                self.request_immediate_load(node);
            }
            Cb::UrlLoadData => {
                // A request that was cancelled before its event was processed (`load` FALSE) loads nothing.
                if self.url_load_state(node) == LoadState::InProgress {
                    if self.builtin_name(node) == Some("Script") {
                        self.script_load_data(node);
                    } else {
                        self.url_start_load(node);
                    }
                }
            }
            Cb::UrlSetAutoRefresh => self.inline_auto_refresh_changed(node),
            _ => {}
        }
    }

    /// Inline.loadData → FileLoader.createX3DFromURL (url, null, setInternalScene).
    pub(crate) fn inline_load(&mut self, node: NodeId, generation: u64) {
        let loader = FileLoader {
            type_name: self.type_name(node),
            base: self.base_url(node),
            data_as_string: true,
        };
        let urls = self.url_strings(node);

        let ok = self.load_document_opt(&loader, &urls, |rt, loaded| match loaded {
            Loaded::Script(_) => Err(Some(
                "Error: Couldn't parse X3D. No suitable file handler found.".into(),
            )),
            Loaded::Data(data, url) => rt.inline_scene(node, generation, &data, &url),
        });

        // loadDocumentError: `callback (null)`. A request that was replaced meanwhile has nothing to say.
        if !ok && self.inline_request_is_current(node, generation) {
            self.set_internal_scene(node, SceneArg::None);
        }
    }

    /// SVGParser reads the PrimitiveQuality and TextureQuality browser options when it is constructed.
    pub(crate) fn sync_svg_quality(&self) {
        let quality = |name: &str| match self.browser_options.get_option(name).map(|v| v.as_str()) {
            Some("LOW") => x3d_parse::SvgQuality::Low,
            Some("HIGH") => x3d_parse::SvgQuality::High,
            _ => x3d_parse::SvgQuality::Medium,
        };

        x3d_parse::set_svg_quality(quality("PrimitiveQuality"), quality("TextureQuality"));
    }

    /// FileLoader.createX3DFromString for an Inline: parse the scene into a new X3DScene, set up
    /// its nodes (Scripts initialize in the microtasks that follow) and wait for its loadCount.
    fn inline_scene(
        &mut self,
        node: NodeId,
        generation: u64,
        data: &[u8],
        url: &url::Url,
    ) -> Result<(), Option<String>> {
        if self.inline_is_cyclic(node, url) {
            return Err(Some("Error: Cyclic Inline: the scene is already being loaded by a scene that contains this Inline.".into()));
        }

        self.sync_svg_quality();

        // GLTF2Parser loads its buffers through the same fetcher as the Inline itself.
        let parsed = crate::loader::parse_bytes_with_fetcher(&*self.fetcher, data, url.as_str());

        let mut scene = match parsed.scene {
            Ok(scene) => scene,
            // GLTF2Parser / GLB2Parser: the thrown JS error text, nothing logged before it.
            Err(x3d_parse::LoadError::Parse(e))
                if e.line == 0
                    && ["TypeError: ", "RangeError: ", "Error: "]
                        .iter()
                        .any(|p| e.message.starts_with(p)) =>
            {
                if let Some(buf_err) = parsed.buffer_error {
                    if e.message == buf_err.error {
                        let loader = crate::loader::FileLoader {
                            type_name: String::new(),
                            base: self.base_url(node),
                            data_as_string: false,
                        };

                        self.print_load_error(
                            &loader,
                            &buf_err.candidate,
                            buf_err.resolved.as_ref(),
                            &buf_err.error,
                            1,
                            0,
                        );

                        // FileLoader.js:382 - if (!error) return;
                        return Err(None);
                    }
                }
                return Err(Some(e.message));
            }
            Err(x3d_parse::LoadError::Parse(e)) => {
                for w in x3d_parse::take_error_warnings() {
                    self.log_warn(w);
                }

                // VRMLParser (DEVELOPMENT) prints the original error before it is rethrown.
                self.log_error(format!("Error: {}", e.message));
                return Err(Some(format!("Error: \n{e}")));
            }
            // GoldenGate.parseIntoScene names the url of a file that no parser handles; Inline keeps the shorter text.
            Err(x3d_parse::LoadError::NoHandler)
                if self.builtin_name(node) == Some("InlineGeometry")
                    || url.path().ends_with(".gltf")
                    || url.path().ends_with(".glb") =>
            {
                return Err(Some(format!(
                    "Error: Couldn't parse X3D. No suitable file handler found for '{url}'."
                )));
            }
            Err(e) => return Err(Some(format!("Error: {e}"))),
        };

        for w in std::mem::take(&mut scene.warnings) {
            self.log_warn(w);
        }

        let setup_order = std::mem::take(&mut scene.setup_order);
        let ctx_count = scene.contexts.len();
        let parent = self.scene.node(node).ctx;
        let m = self.scene.merge_inline(scene, parent, node);

        let (scene_node, count) = self.new_load_count_scene();
        self.pending_scenes.push(PendingScene {
            node,
            generation,
            ctx: m.root_ctx,
            scene_node,
        });

        // `new X3DScene (browser)`: setLive (false).
        self.inline_live.insert(m.root_ctx, false);

        let root = m.root_ctx;

        self.attach_merged_scene(m, ctx_count, setup_order);

        // A Script that initialized just now may have replaced or disabled this very Inline.
        if !self.inline_request_is_current(node, generation) {
            self.pending_scenes.retain(|p| p.ctx != root);
            self.dispose_scene(root);
            return Ok(());
        }

        // InlineGeometry.setInternalScene throws when the scene has no geometry (or no node of the url's name):
        // FileLoader.set_loadCount__ passes it to `reject`, the error of this candidate url.
        if self.builtin_name(node) == Some("InlineGeometry") {
            if let Err(e) = self.inline_geometry_find(root, url) {
                self.pending_scenes.retain(|p| p.ctx != root);
                self.dispose_scene(root);
                return Err(Some(e));
            }
        }

        // FileLoader.setScene: `scene ._loadCount .addEvent ()`.
        self.add_event(count);
        Ok(())
    }

    /// Does the scene at `url` already contain this Inline, directly or through the scenes that loaded
    /// it? Such a load would never end. A file that is used in two places is not a cycle: only the
    /// scenes that contain the Inline count (no fragment).
    fn inline_is_cyclic(&self, node: NodeId, url: &url::Url) -> bool {
        let strip = |s: &str| s.split('#').next().unwrap_or("").to_string();
        let target = strip(url.as_str());
        let mut c = self.scene.node(node).ctx;

        for _ in 0..512 {
            // The body of a PROTO instance belongs to the scene of the instance.
            if let Outer::Instance(n) = self.scene.ctx(c).outer {
                c = self.scene.node(n).ctx;
                continue;
            }

            let root = self.scene.scene_root(c);

            if strip(self.scene.world_url_of(root)) == target {
                return true;
            }

            if root == ROOT {
                return false;
            }

            c = match self.scene.ctx(root).outer {
                Outer::Inline(Some(n)) => self.scene.node(n).ctx,
                _ => match self.scene.ctx(root).parent {
                    Some(p) => p,
                    None => return false,
                },
            };
        }

        true
    }

    /// The scene object that owns `loadCount` of a loaded scene (FileLoader.setScene); its
    /// interest is FileLoader.set_loadCount__.
    pub(crate) fn new_load_count_scene(&mut self) -> (NodeId, FKey) {
        let scene_node = self.create_internal_node("X3DScene");
        let count = self.add_private_field(
            scene_node,
            "loadCount",
            access::OUTPUT_ONLY,
            FieldValue::SFInt32(0),
        );

        self.nrt_mut(scene_node).setup_started = true;
        self.nrt_mut(scene_node).initialized = true;
        self.add_interest(count, scene_node, Cb::SceneLoadCount);

        (scene_node, count)
    }

    /// Construct, route and set up the nodes of a merged scene (the order of X3DParser.setupNodes).
    pub(crate) fn attach_merged_scene(
        &mut self,
        m: MergeMap,
        ctx_count: usize,
        setup_order: Vec<NodeId>,
    ) {
        for n in m.node_offset..m.node_end {
            self.nrt_mut(n);

            if !matches!(self.scene.node(n).kind, NodeKind::Internal(_)) {
                self.construct_node(n);
            }
        }

        for c in m.ctx_offset..m.ctx_offset + ctx_count {
            if matches!(self.scene.contexts[c].outer, Outer::Proto(_)) {
                continue;
            }

            for r in self.scene.contexts[c].routes.clone() {
                self.activate_route(&r);
            }
        }

        self.rebuild_parents();

        for n in setup_order {
            self.setup_node(m.node(n));
        }
    }

    /// FileLoader.set_loadCount__ (interest on the scene's loadCount).
    pub(crate) fn scene_load_count(&mut self, scene_node: NodeId) {
        if self.browser_scene_loaded(scene_node) {
            return;
        }

        let inline = self
            .pending_scenes
            .iter()
            .position(|p| p.scene_node == scene_node);
        let ext = self
            .pending_externs
            .iter()
            .position(|p| p.scene_node == scene_node);

        if inline.is_none() && ext.is_none() {
            return;
        }

        let k = self.private_field(scene_node, "loadCount").unwrap();

        if !matches!(self.value(k), FieldValue::SFInt32(0)) {
            return;
        }

        self.remove_interest(k, scene_node, Cb::SceneLoadCount);

        // await this .browser .nextFrame ()
        match (inline, ext) {
            (Some(i), _) => {
                let p = self.pending_scenes.remove(i);

                if self.inline_request_is_current(p.node, p.generation) {
                    self.frame_waiters.push(FrameWaiter {
                        node: p.node,
                        generation: p.generation,
                        ctx: p.ctx,
                    });
                } else {
                    self.dispose_scene(p.ctx);
                }
            }
            (None, Some(i)) => {
                let p = self.pending_externs.remove(i);
                self.extern_frame_waiters.push(p);
            }
            (None, None) => {}
        }
    }

    /// The continuation of the `nextFrame ()` promises: after the finishEvents of a frame.
    pub(crate) fn finish_frame(&mut self) {
        for w in std::mem::take(&mut self.frame_waiters) {
            if self.inline_request_is_current(w.node, w.generation) {
                self.set_internal_scene(w.node, SceneArg::Scene(w.ctx));
            } else {
                // The request was replaced, disabled or its Inline disposed while the scene waited.
                self.dispose_scene(w.ctx);
            }
        }

        for p in std::mem::take(&mut self.extern_frame_waiters) {
            self.extern_attach(p);
        }

        self.browser_finish_frame();
        self.font_style_continuations();

        self.run_tasks();

        // The end of a frame: nothing is being processed, and the scenes disposed during it are over.
        self.release_disposed();
    }

    /// Inline.setInternalScene.
    pub(crate) fn set_internal_scene(&mut self, node: NodeId, scene: SceneArg) {
        if let Some(old) = self.inline_scenes.remove(&node) {
            self.dispose_scene(old);
        }

        // InlineGeometry.setInternalScene: the geometry of the scene, `set_solid__`, `set_smooth__`, then the state.
        if self.builtin_name(node) == Some("InlineGeometry") {
            self.inline_geometry_clear(node);

            if let SceneArg::Scene(ctx) = scene {
                self.inline_scenes.insert(node, ctx);
                self.inline_live.insert(ctx, true);
            }

            let state = self
                .inline_geometry_state(node, scene)
                .unwrap_or(LoadState::Failed);

            // A scene without a geometry is not kept (X_ITE keeps it until the next one replaces it).
            if let (SceneArg::Scene(ctx), LoadState::Failed) = (scene, state) {
                self.inline_scenes.remove(&node);
                self.dispose_scene(ctx);
            }

            self.refresh_live();
            self.set_load_state(node, state);

            // `X3DChildObject .prototype .addEvent .call (this)`: an event on every field that holds the node (the
            // Shape's `geometry` reads its geometry again; a LoadSensor's `children` is reset).
            self.add_node_parents_event(node);
            return;
        }

        match scene {
            SceneArg::Scene(ctx) => {
                // scene .setExecutionContext (…); scene .setLive (true); this .set_live__ ()
                self.inline_scenes.insert(node, ctx);
                self.inline_live.insert(ctx, true);
                self.refresh_live();
                self.set_load_state(node, LoadState::Complete);

                // The scene joins the graph: parentsChanged, `sceneGraph_changed` of the Inline's context.
                let home = self.scene.node(node).ctx;

                self.ctx_event(home, ctx_events::CtxField::SceneGraph, 1);
                self.ctx_event(home, ctx_events::CtxField::BBox, 1);
            }
            // unloadData passes the browser's default scene, which is truthy: the same branch
            // with an empty scene (the default scene is shared, nothing of it is observable).
            SceneArg::Default => self.set_load_state(node, LoadState::Complete),
            SceneArg::None => self.set_load_state(node, LoadState::Failed),
        }
    }

    // --- live state -----------------------------------------------------------------------

    /// X3DExecutionContext.getLive () .getValue () of a context: the world's scene is live once the
    /// browser updates and `scene .setLive (true)` ran (X3DBrowser.replaceWorld, [`Runtime::go_live`]);
    /// an Inline's scene is live when it was attached and its parent context is live.
    pub fn ctx_live(&self, ctx: CtxId) -> bool {
        // A replaced instance body is disposed: not live.
        if self.ctx_dead(ctx) {
            return false;
        }

        let root = self.scene.scene_root(ctx);

        if root == self.scene.world {
            return self.live && self.scene_live;
        }

        let own = self.inline_live.get(&root).copied().unwrap_or(false);
        let parent = self.scene.contexts[root].parent.unwrap_or(ROOT);

        own && self.ctx_live(parent)
    }

    /// `node .getLive () .getValue ()` of an X3DTimeDependentNode: its own flag, its context and
    /// `evenLive` (X3DTimeDependentNode.checkLiveState).
    pub fn time_node_live(&self, node: NodeId) -> bool {
        let even = self
            .private_field(node, "evenLive")
            .is_some_and(|k| matches!(self.value(k), FieldValue::SFBool(true)));
        self.ctx_live(self.scene.node(node).ctx) || even
    }

    /// WorldInfo.setLive: add to or remove from its execution context's worldInfos.
    pub(crate) fn world_info_set_live(&mut self, node: NodeId, live: bool) {
        let ctx = self.scene.node(node).ctx;
        let list = self.world_infos.entry(ctx).or_default();

        if live {
            list.push(node);
        } else if let Some(i) = list.iter().position(|&n| n == node) {
            list.remove(i);
        }
    }

    /// Register a node for live changes (`this .getLive () .addInterest ("set_live__", this)`).
    pub(crate) fn watch_live(&mut self, node: NodeId) {
        let state = self.time_node_live(node);
        self.live_watchers.push((node, state));
    }

    /// Deliver the live changes of the nodes that watch it, in the order they registered.
    pub fn refresh_live(&mut self) {
        for i in 0..self.live_watchers.len() {
            let (node, last) = self.live_watchers[i];
            let now = self.time_node_live(node);

            if now != last {
                self.live_watchers[i].1 = now;

                if matches!(self.builtin_name(node), Some("KeySensor" | "StringSensor")) {
                    self.key_set_live(node, now);
                } else if self.builtin_name(node) == Some("TransformSensor") {
                    self.transform_sensor_set_enabled(node);
                } else if matches!(
                    self.builtin_name(node),
                    Some(
                        "LinePickSensor"
                            | "PointPickSensor"
                            | "PrimitivePickSensor"
                            | "VolumePickSensor"
                    )
                ) {
                    self.pick_sensor_set_live(node);
                } else if matches!(
                    self.builtin_name(node),
                    Some("AudioDestination" | "StreamAudioDestination" | "SpatialSound" | "Sound")
                ) {
                    self.sound_set_live(node);
                } else if self.builtin_name(node) == Some("ParticleSystem") {
                    self.particle_system_set_live(node);
                } else if self.physics_watches_live(node) {
                    self.physics_set_live(node);
                } else if self.builtin_name(node) == Some("Text") {
                    self.text_set_live(node);
                } else {
                    self.time_set_live(node, now);

                    // BufferAudioSource.set_live__, AudioClip.set_live__: X3DUrlObject.set_live__ too.
                    if matches!(
                        self.builtin_name(node),
                        Some("BufferAudioSource" | "AudioClip")
                    ) {
                        self.buffer_url_set_live(node, now);
                    }
                }
            }
        }

        for node in self.followers.clone() {
            self.follower_set_live(node);
        }
    }

    // --- dispose --------------------------------------------------------------------------

    /// X3DScene.dispose of the scene whose root context is `root`: its routes are deleted
    /// (X3DExecutionContext.dispose → deleteRoute → X3DRoute.dispose) and the scene goes not live
    /// (X3DBaseNode.dispose → setLive (false)), which pauses its TimeSensors and followers.
    ///
    /// The nodes are not disposed one by one, as in X_ITE: the root nodes' SFNode elements are not the
    /// cached wrappers (`SFNodeCache .get (node) === this` in SFNode.dispose is false), so
    /// `node .dispose ()` is never called. X_ITE leaves them to the garbage collector; here they are
    /// queued for the sweep of [`Runtime::release_disposed`] (R0.3-D), which frees the ones that nothing
    /// outside the scene refers to. This port goes further than X_ITE in what it stops in the scene and in
    /// every scene inside it (a nested Inline's scene, the body of a PROTO instance):
    ///
    /// * routes are deleted, scenes go not live;
    /// * a nested Inline cancels its request and drops the scene it has parsed (X_ITE lets it finish);
    ///   so does an EXTERNPROTO declaration of the scene (R0.3-E);
    /// * a Script gets `shutdown ()` (VRML97 4.12.3; X_ITE has none), then stops getting
    ///   `prepareEvents`, `eventsProcessed` and field events (X_ITE keeps calling them for ever).
    pub(crate) fn dispose_scene(&mut self, root: CtxId) {
        if !self.disposed_scenes.insert(root) {
            return;
        }

        let inside: Vec<CtxId> = (0..self.scene.contexts.len())
            .filter(|&c| self.ctx_within(c, root))
            .collect();
        let inside_set: std::collections::HashSet<CtxId> = inside.iter().copied().collect();
        let nodes: Vec<NodeId> = (0..self.scene.nodes.len() as NodeId)
            .filter(|&n| inside_set.contains(&self.scene.node(n).ctx))
            .collect();

        // ISO/IEC 14772-1 4.12.3: `shutdown ()` runs while the scene is still connected. What it
        // sends is queued; the routes below are deleted before the queue is processed.
        for &n in &nodes {
            if self.builtin_name(n) == Some("Script") {
                self.script_close(n);
            }
        }

        for &c in &inside {
            for r in std::mem::take(&mut self.scene.contexts[c].routes) {
                self.dispose_route(r);
            }

            if let Some(live) = self.inline_live.get_mut(&c) {
                *live = false;
            }
        }

        for &n in &nodes {
            if self.is_scene_loader(n) {
                self.inline_close(n);
            }
        }

        // The EXTERNPROTO declarations of the disposed contexts stop loading too (R0.3-E): a scene that
        // a late fetch would merge under a disposed context is never disposed or released.
        let externs: Vec<ExternId> = (0..self.scene.externprotos.len())
            .filter(|&e| {
                !self.scene.extern_slots.is_free(e)
                    && inside_set.contains(&self.scene.externprotos[e].ctx)
            })
            .collect();

        for e in externs {
            self.extern_close(e);
        }

        // Nothing refers to the scene any more, unless a live field, route or context does: the nodes
        // go at the next sweep (see `release.rs`). The whole world is never released.
        if root != ROOT {
            self.queue_release(&inside, &nodes);
        }

        // The nodes get their last live change (a TimeSensor stops, a follower pauses) and are
        // forgotten: a disposed scene never goes live again, and the lists must not grow with every reload.
        self.refresh_live();

        let dead: std::collections::HashSet<NodeId> = nodes.into_iter().collect();

        self.live_watchers.retain(|(n, _)| !dead.contains(n));
        self.followers.retain(|n| !dead.contains(n));
        self.key_sensors.retain(|n| !dead.contains(n));
        self.transform_sensors.retain(|n| !dead.contains(n));
        self.pick_sensors.retain(|n| !dead.contains(n));
        self.layer_object_types.retain(|n, _| !dead.contains(n));
        self.nurbs_scales.retain(|n, _| !dead.contains(n));
        self.viewpoint_groups.retain(|n, _| !dead.contains(n));
        self.hanim_humanoids.retain(|n, _| !dead.contains(n));
        self.hanim_poses.retain(|n, _| !dead.contains(n));
        self.hanim_motions.retain(|n, _| !dead.contains(n));
        self.nurbs_set_members.retain(|n, _| !dead.contains(n));
        for members in self.nurbs_set_members.values_mut() {
            members.retain(|n| !dead.contains(n));
        }
        for list in self.world_infos.values_mut() {
            list.retain(|n| !dead.contains(n));
        }
        self.world_infos.retain(|_, list| !list.is_empty());
    }

    /// The world is unloaded (or replaced by another one): every Script in it gets `shutdown ()`
    /// (ISO/IEC 14772-1 4.12.3) and the world stops. Events the Scripts queued are left for the
    /// caller's next event processing.
    pub fn unload_world(&mut self) {
        self.dispose_scene(self.scene.world);

        // The cached EXTERNPROTO scenes belong to the world (see `extern_scene`) and went with it.
        self.extern_cache.clear();
    }

    /// Is `ctx` the context `root`, or inside it (through scenes and PROTO instance bodies)?
    pub(crate) fn ctx_within(&self, ctx: CtxId, root: CtxId) -> bool {
        let mut c = ctx;

        for _ in 0..256 {
            if c == root {
                return true;
            }

            match self.scene.ctx(c).outer {
                Outer::Instance(n) => c = self.scene.node(n).ctx,
                _ => match self.scene.ctx(c).parent {
                    Some(p) => c = p,
                    None => return false,
                },
            }
        }

        false
    }

    /// An Inline in a scene that was disposed: no request, no timer, no scene, nothing in progress.
    fn inline_close(&mut self, node: NodeId) {
        let Some(s) = self.url_state_mut(node) else {
            return;
        };

        if s.scene_closed {
            return;
        }

        s.scene_closed = true;
        s.timer = None;

        self.inline_abort(node);

        if let Some(c) = self.inline_scenes.remove(&node) {
            self.dispose_scene(c);
        }

        self.inline_geometry_clear(node);

        if self.url_load_state(node) == LoadState::InProgress {
            self.set_load_state(node, LoadState::NotStarted);
        }
    }

    // --- X3DUrlObject auto refresh ----------------------------------------------------------

    /// `Date .now ()` in ms.
    fn url_now_ms(&self) -> f64 {
        self.now * 1000.0
    }

    /// X3DUrlObject.setLoadState (COMPLETE, FAILED): remember when, set the timer for `autoRefresh`.
    fn inline_auto_refresh_complete(&mut self, node: NodeId) {
        let now = self.url_now_ms();
        let interval = self.get_f64(node, "autoRefresh").max(0.0);

        if let Some(s) = self.url_state_mut(node) {
            s.complete_ms = now;
        }

        self.inline_set_auto_refresh_timer(node, interval);
    }

    /// X3DUrlObject.setAutoRefreshTimer.
    fn inline_set_auto_refresh_timer(&mut self, node: NodeId, interval: f64) {
        let now = self.url_now_ms();
        let (auto_refresh, limit) = (
            self.get_f64(node, "autoRefresh"),
            self.get_f64(node, "autoRefreshTimeLimit"),
        );

        if let Some(s) = self.url_state_mut(node) {
            s.timer = url_object::refresh_timer_due(now, s.start_ms, auto_refresh, limit, interval);
        }
    }

    /// X3DUrlObject.set_autoRefresh__ of a BufferAudioSource (its set_live__).
    pub(crate) fn url_set_auto_refresh(&mut self, node: NodeId) {
        self.inline_auto_refresh_changed(node);
    }

    /// X3DUrlObject.set_autoRefresh__ (`autoRefresh` or `autoRefreshTimeLimit` was set).
    fn inline_auto_refresh_changed(&mut self, node: NodeId) {
        if self.url_load_state(node) != LoadState::Complete {
            return;
        }

        let now = self.url_now_ms();
        let auto_refresh = self.get_f64(node, "autoRefresh");
        let complete = match self.nrt(node).map(|n| &n.state) {
            Some(nodes::State::UrlObject(s)) => s.complete_ms,
            _ => now,
        };

        self.inline_set_auto_refresh_timer(
            node,
            url_object::refresh_interval(auto_refresh, (now - complete) / 1000.0),
        );
    }

    /// The due time (ms) and Inline of the earliest auto refresh timer due by `now` (seconds).
    pub(crate) fn inline_next_timer(&self, now: f64) -> Option<(f64, NodeId)> {
        (0..self.scene.nodes.len() as NodeId)
            .filter_map(|n| match self.nrt(n).map(|r| &r.state) {
                Some(nodes::State::UrlObject(s))
                    if matches!(
                        self.builtin_name(n),
                        Some("Inline" | "InlineGeometry" | "BufferAudioSource" | "AudioClip")
                    ) =>
                {
                    s.timer.map(|due| (due, n))
                }
                _ => None,
            })
            .filter(|(due, _)| *due <= now * 1000.0)
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1)))
    }

    /// X3DUrlObject.performAutoRefresh: start over from NOT_STARTED (without the cache).
    pub(crate) fn inline_perform_auto_refresh(&mut self, node: NodeId) {
        if let Some(s) = self.url_state_mut(node) {
            s.timer = None;
        }

        self.set_load_state(node, LoadState::NotStarted);
        self.request_immediate_load(node);
    }
}

/// What is left of Inline loads and scenes (for tests: nothing may stay behind).
#[derive(Clone, Debug, Default)]
pub struct InlineDiagnostics {
    /// Scenes parsed and set up that wait for their loadCount.
    pub pending_scenes: usize,
    /// Scenes whose loadCount reached 0 and that wait for the next frame.
    pub frame_waiters: usize,
    /// Inline nodes whose load state is IN_PROGRESS.
    pub in_progress: usize,
    /// Scenes of Inlines that are live (their TimeSensors run).
    pub live_scenes: usize,
    /// Live scenes that no Inline shows any more.
    pub orphan_live_scenes: usize,
    /// Armed auto refresh timers.
    pub timers: usize,
}

impl Runtime {
    pub fn inline_diagnostics(&self) -> InlineDiagnostics {
        let shown: std::collections::HashSet<CtxId> =
            self.inline_scenes.values().copied().collect();
        // Only the scenes that an Inline loaded (an EXTERNPROTO's scene is live too, but no Inline shows it).
        let live: Vec<CtxId> = self
            .inline_live
            .iter()
            .filter(|(c, l)| **l && matches!(self.scene.ctx(**c).outer, Outer::Inline(Some(_))))
            .map(|(c, _)| *c)
            .collect();

        InlineDiagnostics {
            pending_scenes: self.pending_scenes.len(),
            frame_waiters: self.frame_waiters.len(),
            in_progress: (0..self.scene.nodes.len() as NodeId).filter(|&n| self.is_scene_loader(n) && self.url_load_state(n) == LoadState::InProgress).count(),
            live_scenes: live.len(),
            orphan_live_scenes: live.iter().filter(|c| !shown.contains(c)).count(),
            timers: (0..self.scene.nodes.len() as NodeId).filter(|&n| matches!(self.nrt(n).map(|r| &r.state), Some(nodes::State::UrlObject(s)) if s.timer.is_some())).count(),
        }
    }
}

/// The argument of Inline.setInternalScene.
#[derive(Clone, Copy, Debug)]
pub enum SceneArg {
    /// A loaded scene (its root context).
    Scene(CtxId),
    /// The browser's default scene (unloadData).
    Default,
    /// `null`: the load failed.
    None,
}

// --- routes that end at imported nodes (Routing/X3DRoute.js) ----------------------------------

impl Runtime {
    /// The Inline nodes an imported end of the route belongs to, source first.
    fn route_inlines(&self, r: &Route) -> Vec<NodeId> {
        [r.source_node, r.destination_node]
            .into_iter()
            .filter_map(|n| match self.scene.node(n).kind {
                NodeKind::Imported(i) => Some(self.scene.imported[i].inline),
                _ => None,
            })
            .collect()
    }

    /// X3DRoute constructor: an interest on the `loadState` of the Inline of each imported end, then
    /// `reconnect ()`. Routes between ordinary nodes just become a field interest.
    pub(crate) fn activate_route(&mut self, r: &Route) {
        let inlines = self.route_inlines(r);

        if inlines.is_empty() {
            self.add_field_interest(
                (r.source_node, r.source_loc),
                (r.destination_node, r.destination_loc),
            );
            return;
        }

        for inline in inlines {
            if let Some(k) = self.private_field(inline, "loadState") {
                self.add_interest(k, inline, Cb::RouteReconnect(r.id));
            }
        }

        self.reconnect_route(r.id);
    }

    /// X3DRoute.dispose for a route that was removed from its context.
    pub(crate) fn dispose_route(&mut self, r: Route) {
        if let Some(((s, sl), (d, dl))) = r.conn {
            self.remove_field_interest((s, sl), (d, dl));
            self.scene.field_mut(s, sl).output_routes -= 1;
            self.scene.field_mut(d, dl).input_routes -= 1;
        }

        for inline in self.route_inlines(&r) {
            if let Some(k) = self.private_field(inline, "loadState") {
                self.remove_interest(k, inline, Cb::RouteReconnect(r.id));
            }
        }
    }

    /// The node and field a route end refers to now: for an imported node the field of the node
    /// that the Inline's scene exports.
    fn route_end(&self, node: NodeId, name: &str) -> Result<(NodeId, FieldLoc), String> {
        let target = match self.scene.node(node).kind {
            NodeKind::Imported(i) => {
                let imp = &self.scene.imported[i];
                let exported = self
                    .inline_scenes
                    .get(&imp.inline)
                    .and_then(|&c| self.scene.get_exported_node(c, &imp.exported_name));

                match exported {
                    Some(n) => n,
                    None => {
                        return Err(format!(
                            "Unknown field '{name}' in node class X3DImportedNodeInstance."
                        ));
                    }
                }
            }
            _ => node,
        };

        match self.scene.get_field(target, name) {
            Some(loc) => Ok((target, loc)),
            None => Err(format!(
                "Unknown field '{name}' in node class {}.",
                self.scene.type_name(target)
            )),
        }
    }

    /// X3DExecutionContext.getRoute / addRoute with an imported end: `$.try (() => node .getField (name))` resolves the field of
    /// the exported node when its Inline is loaded, then `X3DRoute.checkFields` runs on the ends that resolved.
    pub fn check_imported_route_ends(
        &self,
        source: NodeId,
        source_field: &str,
        destination: NodeId,
        destination_field: &str,
    ) -> Result<(), String> {
        if !self.scene.is_imported(source) && !self.scene.is_imported(destination) {
            return Ok(());
        }

        let end = |n: NodeId, name: &str| self.route_end(n, name).ok();
        let (src, dst) = (
            end(source, source_field),
            end(destination, destination_field),
        );

        if let (Some(s), Some(d)) = (src, dst) {
            let (sf, df) = (self.scene.field(s.0, s.1), self.scene.field(d.0, d.1));

            if sf.ty() != df.ty() {
                return Err(format!(
                    "Bad ROUTE specification: source field type must match destination field type of fields named \"{}\" and \"{}\".",
                    sf.name, df.name
                ));
            }
        }

        if let Some(s) = src {
            let sf = self.scene.field(s.0, s.1);

            if !x3d_fields::access::is_output(sf.access) {
                return Err(format!(
                    "Bad ROUTE specification: source field \"{}\" must be an output.",
                    sf.name
                ));
            }
        }

        if let Some(d) = dst {
            let df = self.scene.field(d.0, d.1);

            if !x3d_fields::access::is_input(df.access) {
                return Err(format!(
                    "Bad ROUTE specification: destination field \"{}\" must be an input.",
                    df.name
                ));
            }
        }

        Ok(())
    }

    /// X3DRoute.reconnect: disconnect, then connect to the fields of the exported nodes.
    pub(crate) fn reconnect_route(&mut self, id: usize) {
        let Some((ctx, i)) = self.scene.find_route(id) else {
            return;
        };

        // disconnect
        let r = self.scene.contexts[ctx].routes[i].clone();

        if let Some(((s, sl), (d, dl))) = r.conn {
            self.remove_field_interest((s, sl), (d, dl));
            self.scene.field_mut(s, sl).output_routes -= 1;
            self.scene.field_mut(d, dl).input_routes -= 1;
        }

        {
            let route = &mut self.scene.contexts[ctx].routes[i];
            route.conn = None;
            route.source_field = r.raw_source_field.clone();
            route.destination_field = r.raw_destination_field.clone();
        }

        // connect
        let src = self.route_end(r.source_node, &r.raw_source_field);
        let dst = self.route_end(r.destination_node, &r.raw_destination_field);

        let result = match (&src, &dst) {
            (Ok(s), Ok(d)) => {
                let (sf, df) = (self.scene.field(s.0, s.1), self.scene.field(d.0, d.1));

                if sf.ty() != df.ty() {
                    Err(format!(
                        "Error: Bad ROUTE specification: source field type must match destination field type of fields named \"{}\" and \"{}\".",
                        sf.name, df.name
                    ))
                } else if !x3d_fields::access::is_output(sf.access) {
                    Err(format!(
                        "Error: Bad ROUTE specification: source field \"{}\" must be an output.",
                        sf.name
                    ))
                } else if !x3d_fields::access::is_input(df.access) {
                    Err(format!(
                        "Error: Bad ROUTE specification: destination field \"{}\" must be an input.",
                        df.name
                    ))
                } else {
                    Ok(())
                }
            }
            _ => Err("AggregateError: Couldn't connect route.".to_string()),
        };

        match result {
            Ok(()) => {
                let (s, d) = (src.unwrap(), dst.unwrap());
                let (sf, df) = (self.scene.field(s.0, s.1), self.scene.field(d.0, d.1));
                let source_name = if sf.access == x3d_fields::access::INPUT_OUTPUT {
                    format!("{}_changed", sf.name)
                } else {
                    sf.name.clone()
                };
                let destination_name = if df.access == x3d_fields::access::INPUT_OUTPUT {
                    format!("set_{}", df.name)
                } else {
                    df.name.clone()
                };

                self.scene.field_mut(s.0, s.1).output_routes += 1;
                self.scene.field_mut(d.0, d.1).input_routes += 1;
                self.add_field_interest(s, d);

                let route = &mut self.scene.contexts[ctx].routes[i];
                route.conn = Some((s, d));
                route.source_field = source_name;
                route.destination_field = destination_name;
            }
            Err(e) => {
                let all_complete = self
                    .route_inlines(&r)
                    .into_iter()
                    .all(|n| self.url_load_state(n) == LoadState::Complete);

                if all_complete {
                    self.log_warn(e);
                }
            }
        }
    }
}
