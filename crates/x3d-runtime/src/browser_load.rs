//! Scenes a Script asks the browser for: X3DBrowser.createX3DFromString, createX3DFromURL and
//! createVrmlFromURL (Browser/X3DBrowser.js, InputOutput/FileLoader.js).
//!
//! Like an Inline's scene (see `inline.rs`), the X3DScene they return is merged into the runtime's node arena as a
//! context of its own ([`x3d_scene::Scene::merge_extern`]), a child of the context of the calling Script
//! (`scene .setExecutionContext (currentScene)`). The timeline is FileLoader's:
//!
//! 1. `createX3DFromString` parses in a microtask (`Task::BrowserParse`): the parsers run their statements once
//!    `browser .loadComponents (scene)` resolved, so a request made during a frame is parsed after the frame.
//!    `createX3DFromURL` fetches in a macrotask (`Task::BrowserLoad`).
//! 2. The scene's nodes are set up (a Script's `initialize ()` runs), then `scene ._loadCount` gets an event;
//!    `set_loadCount__` waits for the count to reach 0 (nested Inlines) and then for `browser .nextFrame ()`.
//! 3. X3DBrowser `scene .setLive (true)`, then the promise (or the `createVrmlFromURL` event) settles.
//!
//! The promise itself lives in JavaScript; [`ScriptHost::settle`] hands the outcome back.

use crate::loader::{FileLoader, LoadHooks, LoadResult, Loaded};
use crate::*;

/// X3DConstants.INITIALIZED_EVENT, SHUTDOWN_EVENT and INITIALIZED_ERROR (the browser callbacks of `addBrowserCallback`).
pub const INITIALIZED_EVENT: u32 = 1003;
pub const SHUTDOWN_EVENT: u32 = 1004;
pub const INITIALIZED_ERROR: u32 = 1005;

/// A request that has not been parsed yet (`createX3DFromURL`).
#[derive(Clone, Debug)]
pub struct BrowserLoad {
    pub script: Option<NodeId>,
    pub parent: CtxId,
    pub urls: Vec<String>,
    /// `Browser.loadURL (url, parameter)`: the parameters. `None` for createX3DFromURL.
    pub world: Option<Vec<String>>,
    /// The Anchor whose activation made the request (Anchor.requestImmediateLoad): its FileLoader belongs to the Anchor.
    pub anchor: Option<NodeId>,
}

/// A `createX3DFromString` request waiting for its microtask.
#[derive(Clone, Debug)]
pub struct BrowserString {
    pub script: Option<NodeId>,
    pub parent: CtxId,
    pub text: String,
}

/// A scene that was parsed and set up and waits for its loadCount to reach 0.
#[derive(Clone, Debug)]
pub struct PendingBrowserScene {
    pub id: u64,
    pub script: Option<NodeId>,
    pub ctx: CtxId,
    pub scene_node: NodeId,
    /// The scene of a `Browser.loadURL`: it replaces the world once it is complete.
    pub world: bool,
}

/// What `Browser.replaceWorld` was given.
#[derive(Clone, Debug)]
pub enum WorldScene {
    /// An X3DScene (its root context).
    Ctx(CtxId),
    /// An MFNode (VRML97 replaceWorld): the nodes become the root nodes of a new scene.
    Roots(Vec<Option<NodeId>>),
    /// `null`, `undefined` or anything else: an empty scene.
    Empty,
}

/// A `replaceWorld` that awaits `browser .nextFrame ()` before it shuts the old world down.
#[derive(Clone, Debug)]
pub struct WorldWaiter {
    pub id: u64,
    pub scene: WorldScene,
}

/// State of X3DBrowser.replaceWorld / loadURL.
#[derive(Default, Debug)]
pub struct WorldState {
    /// `this [_fileLoader]`: the loadURL request in flight.
    pub load_current: Option<u64>,
    /// `this [_reject]`: the replaceWorld in progress (its identity).
    pub replace_current: Option<u64>,
    /// Requests that are `loadURL`s (their promise resolves without a value).
    pub loads: std::collections::HashSet<u64>,
    /// loadURL requests whose fetch was aborted (`fileLoader .abort ()`) before it ran.
    pub aborted: std::collections::HashSet<u64>,
    /// loadURL-like requests that belong to an Anchor.
    pub anchor_loads: std::collections::HashSet<u64>,
    pub waiters: Vec<WorldWaiter>,
    /// The `_loadCount` event of a new world: `checkLoadCount`. The inner value is the promise to resolve (none for the first world).
    pub init: Option<Option<u64>>,
    /// Promises to settle when the frame is over (`resolve ()` microtasks run after the frame).
    pub settles: Vec<(u64, Result<(), String>)>,
    /// Counts the worlds installed after the first; a viewer rebinds when it changes.
    pub generation: u64,
    /// The browser's `loadCount` (an internal node and its field) whose event runs `checkLoadCount`.
    pub load_count: Option<(NodeId, FKey)>,
    /// `browser .initialized ()`: the browser time of the last checkLoadCount (0 before the first).
    pub initialized_time: f64,
}

/// `String (error)` of a rejection with the message `message`: an Error whose message has no class prefix of its own.
fn error_string(message: String) -> String {
    if ["TypeError: ", "RangeError: ", "Error: ", "URIError: "]
        .iter()
        .any(|p| message.starts_with(p))
    {
        message
    } else {
        format!("Error: {message}")
    }
}

/// What a scene is parsed from: fetched bytes, or a JavaScript string.
enum Input<'a> {
    Bytes(&'a [u8]),
    Text(&'a str),
}

impl Runtime {
    /// X3DBrowser.createX3DFromString: parse `text` in the Script's scene. The parse is a microtask
    /// (VRMLParser, XMLParser and JSONParser parse their statements after `browser .loadComponents (scene)`);
    /// a rejection settles from there too.
    pub fn browser_create_from_string(
        &mut self,
        text: &str,
        parent: CtxId,
        script: Option<NodeId>,
    ) -> Result<u64, String> {
        let id = self.next_browser_request();

        self.browser_strings.insert(
            id,
            BrowserString {
                script,
                parent,
                text: text.to_string(),
            },
        );
        self.tasks.push_back(Task::BrowserParse(id));
        Ok(id)
    }

    /// `Task::BrowserParse`: the statements of a createX3DFromString request.
    pub(crate) fn browser_parse(&mut self, id: u64) {
        let Some(req) = self.browser_strings.remove(&id) else {
            return;
        };

        let world = self.scene.world_url_of(req.parent).to_string();

        if let Err(e) =
            self.browser_scene(id, req.script, req.parent, Input::Text(&req.text), &world)
        {
            self.browser_settle(id, Err(e));
        }
    }

    /// X3DBrowser.importJS: the JSON text is parsed by JSONParser straight into a new scene.
    pub fn browser_import_json(
        &mut self,
        text: &str,
        parent: CtxId,
        script: Option<NodeId>,
    ) -> Result<u64, String> {
        let id = self.next_browser_request();
        let world = self.scene.world_url_of(parent).to_string();

        self.sync_svg_quality();

        let scene = match x3d_parse::parse_json_x3d(text, &world) {
            Ok(scene) => scene,
            Err(x3d_parse::LoadError::Parse(e)) => return Err(e.message),
            Err(e) => return Err(format!("{e}")),
        };

        self.browser_attach(id, script, parent, scene);
        Ok(id)
    }

    /// X3DBrowser.createScene: an empty scene (encoding SCRIPTED) with the profile and components, live at once.
    pub fn browser_create_scene(
        &mut self,
        profile: Option<String>,
        components: Vec<(String, i32)>,
        parent: CtxId,
    ) -> CtxId {
        // `this [_worldURL] = location .toString ()`: the page (an embedder without a page: the URL of the caller's world).
        let url = if self.page_url.is_empty() {
            self.scene.world_url_of(parent).to_string()
        } else {
            self.page_url.clone()
        };
        let mut scene = Scene::new(&url);

        scene.profile = profile;

        for (name, level) in components {
            scene.update_component(&name, level);
        }

        let ctx_count = scene.contexts.len();
        let m = self.scene.merge_extern(scene, parent);

        self.attach_merged_scene(m, ctx_count, vec![]);
        self.inline_live.insert(m.root_ctx, true);
        self.refresh_live();

        // `new X3DScene (this)` has no execution context: its changes do not reach the world's.
        self.ctx_events.no_forward.insert(m.root_ctx);
        m.root_ctx
    }

    /// X3DBrowser.createX3DFromURL: the fetch is a macrotask.
    pub fn browser_create_from_url(
        &mut self,
        urls: Vec<String>,
        parent: CtxId,
        script: Option<NodeId>,
    ) -> u64 {
        let id = self.next_browser_request();

        self.browser_loads.insert(
            id,
            BrowserLoad {
                script,
                parent,
                urls,
                world: None,
                anchor: None,
            },
        );
        self.tasks.push_back(Task::BrowserLoad(id));
        id
    }

    fn next_browser_request(&mut self) -> u64 {
        self.browser_request_id += 1;
        self.browser_request_id
    }

    /// `Task::BrowserLoad`: FileLoader.createX3DFromURL (url, null, callback).
    pub(crate) fn browser_load(&mut self, id: u64) {
        let Some(req) = self.browser_loads.remove(&id) else {
            return;
        };

        if let Some(parameters) = req.world.clone() {
            return self.world_load(id, req, parameters);
        }

        let type_name = req.script.map(|s| self.type_name(s)).unwrap_or_default();
        let loader = FileLoader {
            type_name,
            base: self.scene.world_url_of(req.parent).to_string(),
            data_as_string: true,
        };
        let base = loader.base.clone();

        let ok = self.load_document(&loader, &req.urls, |rt, loaded| {
            let (data, url) = match loaded {
                Loaded::Script(source) => (
                    source.into_bytes(),
                    format!(
                        "{}null",
                        base.rsplit_once('/')
                            .map_or(String::new(), |(d, _)| format!("{d}/"))
                    ),
                ),
                Loaded::Data(data, url) => (data, url.to_string()),
            };

            rt.browser_scene(id, req.script, req.parent, Input::Bytes(&data), &url)
                .map_err(error_string)
        });

        // loadDocumentError: `callback (null)`.
        if !ok {
            self.browser_settle(id, Err("Couldn't load X3D file.".into()));
        }
    }

    /// FileLoader.createX3DFromString for the browser: parse into a new X3DScene, set up its nodes and wait for its loadCount.
    fn browser_scene(
        &mut self,
        id: u64,
        script: Option<NodeId>,
        parent: CtxId,
        input: Input<'_>,
        url: &str,
    ) -> Result<(), String> {
        self.sync_svg_quality();

        let parsed = match input {
            Input::Bytes(data) => {
                crate::loader::parse_bytes_with_fetcher(&*self.fetcher, data, url)
            }
            Input::Text(text) => x3d_parse::parse_string(text, url),
        };

        let mut scene = match parsed {
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
                for w in x3d_parse::take_error_warnings() {
                    self.log_warn(w);
                }

                // VRMLParser (DEVELOPMENT) prints the original error before it is rethrown with the line of the input.
                self.log_error(format!("Error: {}", e.message));
                return Err(
                    x3d_parse::take_error_detail().unwrap_or_else(|| format!("Error: \n{e}"))
                );
            }
            // GoldenGate.parseIntoScene.
            Err(x3d_parse::LoadError::NoHandler) => {
                return Err(format!(
                    "Couldn't parse X3D. No suitable file handler found for '{url}'."
                ));
            }
            Err(e) => return Err(format!("Error: {e}")),
        };

        self.browser_attach(id, script, parent, scene);
        Ok(())
    }

    /// Merge a parsed scene, set up its nodes and wait for its loadCount.
    fn browser_attach(&mut self, id: u64, script: Option<NodeId>, parent: CtxId, mut scene: Scene) {
        for w in std::mem::take(&mut scene.warnings) {
            self.log_warn(w);
        }

        let setup_order = std::mem::take(&mut scene.setup_order);
        let ctx_count = scene.contexts.len();
        let m = self.scene.merge_extern(scene, parent);
        let (scene_node, count) = self.new_load_count_scene();

        let world = self.world_state.loads.contains(&id);

        self.pending_browser.push(PendingBrowserScene {
            id,
            script,
            ctx: m.root_ctx,
            scene_node,
            world,
        });

        // `new X3DScene (browser)`: setLive (false); the Browser makes it live once it is delivered.
        self.inline_live.insert(m.root_ctx, false);
        self.attach_merged_scene(m, ctx_count, setup_order);
        self.ctx_scene_parsed(m.root_ctx);

        // FileLoader.setScene: `scene ._loadCount .addEvent ()`.
        self.add_event(count);
    }

    /// FileLoader.set_loadCount__ for a browser scene whose count reached 0: wait for the next frame.
    pub(crate) fn browser_scene_loaded(&mut self, scene_node: NodeId) -> bool {
        let Some(i) = self
            .pending_browser
            .iter()
            .position(|p| p.scene_node == scene_node)
        else {
            return false;
        };

        let k = self.private_field(scene_node, "loadCount").unwrap();

        if !matches!(self.value(k), FieldValue::SFInt32(0)) {
            return true;
        }

        self.remove_interest(k, scene_node, Cb::SceneLoadCount);

        let p = self.pending_browser.remove(i);
        self.browser_frame_waiters.push(p);
        true
    }

    /// The continuation of `await browser .nextFrame ()`: `scene .setLive (true)`, then the promise settles.
    pub(crate) fn browser_finish_frame(&mut self) {
        // A new world that still waited for loads when its loadCount event ran.
        if self.world_state.init.is_some()
            && self
                .world_state
                .load_count
                .is_some_and(|(_, k)| !self.is_field_tainted(k))
        {
            self.world_check_load_count();
        }

        // The promises that settled while the frame was processed (checkLoadCount) settle first: their microtasks were queued first.
        for (id, outcome) in std::mem::take(&mut self.world_state.settles) {
            self.browser_settle_plain(id, outcome);
        }

        // A replaceWorld that awaited the previous frame's nextFrame () goes on now; the ones this loop starts wait for the next frame.
        let replacing = std::mem::take(&mut self.world_state.waiters);

        for w in std::mem::take(&mut self.browser_frame_waiters) {
            if w.world {
                self.world_load_done(w);
                continue;
            }

            self.inline_live.insert(w.ctx, true);
            self.refresh_live();
            self.browser_settle(w.id, Ok(w.ctx));
        }

        for r in replacing {
            self.world_switch(r);
        }
    }

    /// Settle a promise that resolves without a value (`loadURL`, `replaceWorld`).
    fn browser_settle_plain(&mut self, id: u64, outcome: Result<(), String>) {
        self.world_state.loads.remove(&id);

        let Some(mut host) = self.script_host.take() else {
            return;
        };

        host.settle_plain(self, id, outcome);
        self.script_host = Some(host);
        self.dynamic_dirty = true;
    }

    /// X3DBrowser.callBrowserCallbacks (event): the functions Scripts gave `addBrowserCallback`.
    pub(crate) fn browser_callbacks(&mut self, event: u32) {
        let Some(mut host) = self.script_host.take() else {
            return;
        };

        host.browser_event(self, event);
        self.script_host = Some(host);
        self.dynamic_dirty = true;
    }

    fn browser_settle(&mut self, id: u64, outcome: Result<CtxId, String>) {
        let Some(mut host) = self.script_host.take() else {
            return;
        };

        host.settle(self, id, outcome);
        self.script_host = Some(host);
        self.dynamic_dirty = true;
    }

    /// Is a browser scene still loading?
    pub(crate) fn browser_loads_pending(&self) -> bool {
        !self.browser_loads.is_empty()
            || !self.browser_strings.is_empty()
            || !self.pending_browser.is_empty()
            || !self.browser_frame_waiters.is_empty()
            || !self.world_state.waiters.is_empty()
    }
}

// --- X3DBrowser.loadURL and replaceWorld ------------------------------------------------------------------------

impl Runtime {
    /// X3DBrowser.loadURL (url, parameter): the request. `Err (None)`: a first URL that only names a viewpoint of the world,
    /// the promise resolves at once (after `changeViewpoint`). `Err (Some (message))`: it rejects at once.
    pub fn browser_load_url(
        &mut self,
        urls: Vec<String>,
        parameters: Vec<String>,
        parent: CtxId,
        script: Option<NodeId>,
    ) -> Result<u64, Option<String>> {
        let base = self.scene.world_url_of(parent).to_string();

        // No URL: loadDocumentError () calls back inside createX3DFromURL, before `this [_fileLoader]` is this loader.
        if urls.is_empty() {
            return Err(Some("Loading of X3D file aborted.".into()));
        }

        // `bindViewpoint` runs inside loadDocumentAsync, before the first await: the call itself changes the viewpoint.
        if let Some(first) = urls.first() {
            let hooks = LoadHooks {
                bind_viewpoint: true,
                ..LoadHooks::default()
            };

            if let Some(name) = crate::loader::viewpoint_fragment(&base, first, &hooks) {
                self.browser_requests
                    .push(format!("changeViewpoint:{name}"));
                return Err(None);
            }
        }

        let id = self.next_browser_request();

        // `this [_fileLoader] ?.abort ()`: the fetch of the request in flight fails.
        self.abort_world_load();
        self.world_state.load_current = Some(id);
        self.world_state.loads.insert(id);
        self.browser_loads.insert(
            id,
            BrowserLoad {
                script,
                parent,
                urls,
                world: Some(parameters),
                anchor: None,
            },
        );
        self.tasks.push_back(Task::BrowserLoad(id));
        Ok(id)
    }

    /// `this [_fileLoader] ?.abort ()`: a loadURL whose fetch has not run fails with an AbortError when it does.
    fn abort_world_load(&mut self) {
        if let Some(previous) = self.world_state.load_current {
            if self.browser_loads.contains_key(&previous) {
                self.world_state.aborted.insert(previous);
            }
        }
    }

    /// Anchor.requestImmediateLoad: a click on an Anchor loads its `url` (with `parameter`) in a FileLoader of its own and
    /// replaces the world (a fragment changes the viewpoint, a foreign target goes to the application).
    pub fn anchor_load(&mut self, anchor: NodeId, urls: Vec<String>, parameters: Vec<String>) {
        let id = self.next_browser_request();
        let parent = self.scene.node(anchor).ctx;

        self.world_state.loads.insert(id);
        self.world_state.anchor_loads.insert(id);
        self.browser_loads.insert(
            id,
            BrowserLoad {
                script: None,
                parent,
                urls,
                world: Some(parameters),
                anchor: Some(anchor),
            },
        );
        self.tasks.push_back(Task::BrowserLoad(id));
    }

    /// `Task::BrowserLoad` of a loadURL: FileLoader.createX3DFromURL (url, parameter, callback, bindViewpoint, foreign).
    fn world_load(&mut self, id: u64, req: BrowserLoad, parameters: Vec<String>) {
        // A FileLoader of the world has no node type in its messages; a Script's or an Anchor's has.
        let type_name = req
            .script
            .or(req.anchor)
            .map(|s| self.type_name(s))
            .unwrap_or_default();
        let loader = FileLoader {
            type_name,
            base: self.scene.world_url_of(req.parent).to_string(),
            data_as_string: true,
        };
        let base = loader.base.clone();
        let hooks = LoadHooks {
            bind_viewpoint: true,
            foreign: true,
            parameters,
        };

        // The fetch of the first URL was aborted before it was made: `fetch` rejects with the AbortError, and as `abort ()`
        // emptied the list of URLs, that is the end.
        let first = req.urls.first().map(String::as_str).unwrap_or_default();
        let fetched = !first.is_empty()
            && !["ecmascript:", "javascript:", "vrmlscript:"]
                .iter()
                .any(|s| first.trim_start().starts_with(s));

        if self.world_state.aborted.remove(&id) && fetched {
            let resolved = url::Url::parse(&base).and_then(|b| b.join(first)).ok();

            self.print_load_error(
                &loader,
                first,
                resolved.as_ref(),
                "AbortError: signal is aborted without reason",
                1,
                0,
            );
            return self.world_load_failed(id);
        }

        let result = self.load_document_hooks(&loader, &req.urls, &hooks, |rt, loaded| {
            let (data, url) = match loaded {
                Loaded::Script(source) => (
                    source.into_bytes(),
                    format!(
                        "{}null",
                        base.rsplit_once('/')
                            .map_or(String::new(), |(d, _)| format!("{d}/"))
                    ),
                ),
                Loaded::Data(data, url) => (data, url.to_string()),
            };

            rt.browser_scene(id, req.script, req.parent, Input::Bytes(&data), &url)
                .map_err(error_string)
        });

        match result {
            LoadResult::Loaded => {}
            // The callbacks of loadURL: `changeViewpoint (fragment)`, `window .open (url, target)` / `location = url`.
            LoadResult::Fragment(name) => {
                self.browser_requests
                    .push(format!("changeViewpoint:{name}"));
                self.world_state.anchor_loads.remove(&id);
                self.world_state.load_done(id);
                self.browser_settle_plain(id, Ok(()));
            }
            LoadResult::Foreign(href, target) => {
                self.browser_requests
                    .push(format!("foreign:{target}:{href}"));
                self.world_state.anchor_loads.remove(&id);
                self.world_state.load_done(id);
                self.browser_settle_plain(id, Ok(()));
            }
            LoadResult::Failed => self.world_load_failed(id),
        }
    }

    /// The loadURL callback with `scene === null`.
    fn world_load_failed(&mut self, id: u64) {
        // Anchor.requestImmediateLoad: `setLoadState (FAILED_STATE)`, nothing else.
        if self.world_state.anchor_loads.remove(&id) {
            return;
        }

        if self.world_state.load_current != Some(id) {
            self.browser_settle_plain(id, Err("Loading of X3D file aborted.".into()));
            return;
        }

        self.browser_callbacks(INITIALIZED_ERROR);
        self.browser_settle_plain(id, Err("Couldn't load X3D file.".into()));
    }

    /// The FileLoader callback of loadURL, after `await browser .nextFrame ()`: `replaceWorld (scene)`.
    fn world_load_done(&mut self, w: PendingBrowserScene) {
        // Anchor.requestImmediateLoad: `this .getBrowser () .replaceWorld (scene)`, without a look at the browser's loader.
        if self.world_state.anchor_loads.remove(&w.id) {
            self.replace_world_request(w.id, WorldScene::Ctx(w.ctx));
            return;
        }

        if self.world_state.load_current != Some(w.id) {
            self.dispose_scene(w.ctx);
            self.browser_settle_plain(w.id, Err("Loading of X3D file aborted.".into()));
            return;
        }

        self.replace_world_request(w.id, WorldScene::Ctx(w.ctx));
    }

    /// X3DBrowser.replaceWorld (scene), up to `await this .nextFrame ()`.
    pub fn replace_world_request(&mut self, id: u64, scene: WorldScene) {
        // `this [_fileLoader] ?.abort ()`
        self.abort_world_load();

        if let Some(previous) = self.world_state.replace_current.replace(id) {
            if previous != id {
                self.world_state.waiters.retain(|w| w.id != previous);

                // The rejection runs as a microtask of the Script's turn: it is delivered with the end of the frame.
                self.world_state
                    .settles
                    .push((previous, Err("Replacing world aborted.".into())));
            }
        }

        self.world_state.waiters.push(WorldWaiter { id, scene });
    }

    /// X3DBrowser.replaceWorld (scene) called by a Script.
    pub fn browser_replace_world(&mut self, scene: WorldScene) -> u64 {
        let id = self.next_browser_request();

        self.world_state.loads.insert(id);
        self.replace_world_request(id, scene);
        id
    }

    /// The rest of replaceWorld, after the frame: the old world shuts down and the scene becomes the world.
    fn world_switch(&mut self, w: WorldWaiter) {
        let superseded = |rt: &Runtime| rt.world_state.replace_current != Some(w.id);

        if superseded(self) {
            if let WorldScene::Ctx(c) = w.scene {
                self.dispose_scene(c);
            }

            return;
        }

        // The scene leaves its parent (`scene .setExecutionContext (null)`) before the old world goes.
        let scene_ctx = match w.scene {
            WorldScene::Ctx(c) => {
                self.scene.contexts[c].parent = None;
                Some(c)
            }
            WorldScene::Roots(_) | WorldScene::Empty => None,
        };

        // getExecutionContext () .setLive (false); shutdown () .processInterests (); the SHUTDOWN_EVENT callbacks.
        self.scene_live = false;
        self.refresh_live();
        self.browser_callbacks(SHUTDOWN_EVENT);

        // A shutdown handler may have started another replaceWorld.
        if superseded(self) {
            if let Some(c) = scene_ctx {
                self.dispose_scene(c);
            }

            return;
        }

        let old = self.scene.world;

        // The old world's scene lets go of its nodes while it is still the current scene: one `sceneGraph_changed`.
        self.ctx_events
            .pending
            .push((old, ctx_events::CtxField::SceneGraph, 1));
        self.deliver_ctx_events();

        // Kept difference (owner-approved): the Scripts of the old world get shutdown (), X_ITE has none.
        self.dispose_scene(old);

        if old == ROOT {
            self.extern_cache.clear();
        }

        let new = match w.scene {
            WorldScene::Ctx(c) => c,
            WorldScene::Empty => self.empty_world_scene(),
            WorldScene::Roots(roots) => self.roots_world_scene(roots),
        };

        // `scene .setLive (true)`, then `this .setExecutionContext (scene)`: it is the world, not an Inline's scene.
        self.scene.contexts[new].outer = Outer::Scene;
        self.scene.world = new;
        self.scene_live = false;
        self.ctx_world_started(true);
        self.go_live();
        self.description.clear();
        self.world_state.generation += 1;
        self.world_state.replace_current = Some(w.id);

        // `this ._loadCount .addEvent ()`: checkLoadCount runs with the next events.
        self.world_state.init = Some(Some(w.id));
        self.world_load_count_event();
    }

    /// `new X3DScene (this)` for replaceWorld (null).
    pub(crate) fn empty_world_scene(&mut self) -> CtxId {
        let scene = Scene::new(&self.page_url.clone());
        let ctx_count = scene.contexts.len();
        let m = self.scene.merge_extern(scene, ROOT);

        self.attach_merged_scene(m, ctx_count, vec![]);
        self.scene.contexts[m.root_ctx].parent = None;
        m.root_ctx
    }

    /// The VRML97 replaceWorld (MFNode): a new scene whose root nodes are the given nodes.
    fn roots_world_scene(&mut self, roots: Vec<Option<NodeId>>) -> CtxId {
        let ctx = self.empty_world_scene();
        let mut homes: Vec<CtxId> = vec![];

        for n in roots.iter().flatten() {
            let home = self.scene.node(*n).ctx;

            if !homes.contains(&home) {
                homes.push(home);
            }
        }

        // The scenes the nodes were made in now belong to the new world.
        for home in homes {
            let root = self.scene.scene_root(home);

            if root != ctx && self.scene.contexts[root].parent.is_some() {
                self.scene.contexts[root].parent = Some(ctx);
            }
        }

        self.scene.contexts[ctx].root_nodes = roots;
        self.rebuild_parents();
        ctx
    }

    /// `this ._loadCount .addEvent ()` with `checkLoadCount` as the interest.
    pub(crate) fn world_load_count_event(&mut self) {
        let k = match self.world_state.load_count {
            Some((_, k)) => k,
            None => {
                let node = self.create_internal_node("X3DBrowser");
                let k = self.add_private_field(
                    node,
                    "loadCount",
                    access::OUTPUT_ONLY,
                    FieldValue::SFInt32(0),
                );

                self.nrt_mut(node).setup_started = true;
                self.nrt_mut(node).initialized = true;
                self.add_interest(k, node, Cb::WorldLoadCount);
                self.world_state.load_count = Some((node, k));
                k
            }
        };

        self.add_event(k);
    }

    /// `checkLoadCount`: the browser's loadCount event of a new world, at the first events after the switch.
    pub(crate) fn world_check_load_count(&mut self) {
        let Some(promise) = self.world_state.init else {
            return;
        };

        if self.root_load_count() != 0 {
            return;
        }

        self.world_state.init = None;
        self.world_state.initialized_time = self.now;
        self.browser_callbacks(INITIALIZED_EVENT);

        // `resolve ()`: the promise settles when the frame is over.
        if let Some(id) = promise {
            self.world_state.settles.push((id, Ok(())));
        }
    }
}

impl WorldState {
    /// The loadURL request `id` is over (it did not replace the world).
    pub fn load_done(&mut self, id: u64) {
        if self.load_current == Some(id) {
            self.load_current = None;
        }
    }
}
