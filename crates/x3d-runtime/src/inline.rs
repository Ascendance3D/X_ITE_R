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

use crate::loader::{FileLoader, Loaded};
use crate::nodes::LoadState;
use crate::*;

/// A scene that was parsed and set up and waits for its loadCount to reach 0.
#[derive(Clone, Debug)]
pub struct PendingScene {
    /// The Inline node.
    pub node: NodeId,
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
    pub ctx: CtxId,
}

const NOT_STARTED: i32 = 0;
const IN_PROGRESS: i32 = 1;
const COMPLETE: i32 = 2;
const FAILED: i32 = 3;

impl Runtime {
    /// The X3DUrlObject load state of a node.
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
        match &mut self.nrt_mut(node).state {
            nodes::State::UrlObject(s) => s.load_state = state,
            nodes::State::Script(s) => s.load_state = state,
            _ => return,
        }

        if self.builtin_name(node) != Some("Inline") {
            return;
        }

        if let Some(k) = self.private_field(node, "loadState") {
            let value = match state {
                LoadState::NotStarted => NOT_STARTED,
                LoadState::InProgress => IN_PROGRESS,
                LoadState::Complete => COMPLETE,
                LoadState::Failed => FAILED,
            };

            self.set_value(k, FieldValue::SFInt32(value));
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
            .or_else(|| self.pending_externs.iter().find(|p| p.ctx == root).map(|p| p.scene_node));

        let Some(scene_node) = scene_node else {
            return;
        };

        let inlines = (0..self.scene.nodes.len() as NodeId)
            .filter(|&n| {
                self.builtin_name(n) == Some("Inline")
                    && self.url_load_state(n) == LoadState::InProgress
                    && self.scene.scene_root(self.scene.node(n).ctx) == root
            })
            .count();
        let externs = self.extern_states.iter().filter(|(e, s)| s.load_state == LoadState::InProgress && self.scene.scene_root(self.scene.externprotos[**e].ctx) == root).count();

        let k = self.private_field(scene_node, "loadCount").unwrap();
        self.set_value(k, FieldValue::SFInt32((inlines + externs) as i32));
    }

    /// Inline.initialize: X3DUrlObject.initialize, then `requestImmediateLoad ()`.
    pub(crate) fn inline_initialize(&mut self, node: NodeId) {
        self.interest(node, "load", Cb::UrlSetLoad);
        self.interest(node, "url", Cb::UrlSetUrl);
        self.request_immediate_load(node);
    }

    /// X3DUrlObject.requestImmediateLoad (a rejected promise is caught by the callers).
    pub(crate) fn request_immediate_load(&mut self, node: NodeId) {
        if self.url_load_state(node) != LoadState::NotStarted {
            return;
        }

        if self.url_state_missing(node) {
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
        self.tasks.push_back(Task::UrlLoad(node));
    }

    fn url_state_missing(&self, node: NodeId) -> bool {
        !matches!(self.nrt(node).map(|n| &n.state), Some(nodes::State::UrlObject(_)))
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

    /// `unloadData ()`: an Inline replaces its scene by the browser's empty default scene.
    fn unload_data(&mut self, node: NodeId) {
        if self.builtin_name(node) == Some("Inline") {
            self.set_internal_scene(node, SceneArg::Default);
        }
    }

    /// X3DUrlObject.set_load__ / set_url__ (the interests on `load` and `url`).
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
            _ => {}
        }
    }

    /// Inline.loadData → FileLoader.createX3DFromURL (url, null, setInternalScene).
    pub(crate) fn inline_load(&mut self, node: NodeId) {
        let loader = FileLoader { type_name: "Inline".into(), base: self.base_url(node), data_as_string: true };
        let urls = self.url_strings(node);

        let ok = self.load_document(&loader, &urls, |rt, loaded| match loaded {
            Loaded::Script(_) => Err("Error: Couldn't parse X3D. No suitable file handler found.".into()),
            Loaded::Data(data, url) => rt.inline_scene(node, &data, url.as_str()),
        });

        if !ok {
            // loadDocumentError: `callback (null)`.
            self.set_internal_scene(node, SceneArg::None);
        }
    }

    /// FileLoader.createX3DFromString for an Inline: parse the scene into a new X3DScene, set up
    /// its nodes (Scripts initialize in the microtasks that follow) and wait for its loadCount.
    fn inline_scene(&mut self, node: NodeId, data: &[u8], url: &str) -> Result<(), String> {
        let mut scene = x3d_parse::parse_bytes(data, url).map_err(|e| format!("Error: {e}"))?;

        for w in std::mem::take(&mut scene.warnings) {
            self.log_warn(w);
        }

        let setup_order = std::mem::take(&mut scene.setup_order);
        let ctx_count = scene.contexts.len();
        let parent = self.scene.node(node).ctx;
        let m = self.scene.merge_inline(scene, parent, node);

        let (scene_node, count) = self.new_load_count_scene();
        self.pending_scenes.push(PendingScene { node, ctx: m.root_ctx, scene_node });

        // `new X3DScene (browser)`: setLive (false).
        self.inline_live.insert(m.root_ctx, false);

        self.attach_merged_scene(m, ctx_count, setup_order);

        // FileLoader.setScene: `scene ._loadCount .addEvent ()`.
        self.add_event(count);
        Ok(())
    }

    /// The scene object that owns `loadCount` of a loaded scene (FileLoader.setScene); its
    /// interest is FileLoader.set_loadCount__.
    pub(crate) fn new_load_count_scene(&mut self) -> (NodeId, FKey) {
        let scene_node = self.create_internal_node("X3DScene");
        let count = self.add_private_field(scene_node, "loadCount", access::OUTPUT_ONLY, FieldValue::SFInt32(0));

        self.nrt_mut(scene_node).setup_started = true;
        self.nrt_mut(scene_node).initialized = true;
        self.add_interest(count, scene_node, Cb::SceneLoadCount);

        (scene_node, count)
    }

    /// Construct, route and set up the nodes of a merged scene (the order of X3DParser.setupNodes).
    pub(crate) fn attach_merged_scene(&mut self, m: MergeMap, ctx_count: usize, setup_order: Vec<NodeId>) {
        for n in m.node_offset..self.scene.nodes.len() as NodeId {
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
        let inline = self.pending_scenes.iter().position(|p| p.scene_node == scene_node);
        let ext = self.pending_externs.iter().position(|p| p.scene_node == scene_node);

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
                self.frame_waiters.push(FrameWaiter { node: p.node, ctx: p.ctx });
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
            self.set_internal_scene(w.node, SceneArg::Scene(w.ctx));
        }

        for p in std::mem::take(&mut self.extern_frame_waiters) {
            self.extern_attach(p);
        }

        self.run_tasks();
    }

    /// Inline.setInternalScene.
    pub(crate) fn set_internal_scene(&mut self, node: NodeId, scene: SceneArg) {
        if let Some(old) = self.inline_scenes.remove(&node) {
            self.dispose_scene(old);
        }

        match scene {
            SceneArg::Scene(ctx) => {
                // scene .setExecutionContext (…); scene .setLive (true); this .set_live__ ()
                self.inline_scenes.insert(node, ctx);
                self.inline_live.insert(ctx, true);
                self.refresh_live();
                self.set_load_state(node, LoadState::Complete);
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

        if root == ROOT {
            return self.live && self.scene_live;
        }

        let own = self.inline_live.get(&root).copied().unwrap_or(false);
        let parent = self.scene.contexts[root].parent.unwrap_or(ROOT);

        own && self.ctx_live(parent)
    }

    /// `node .getLive () .getValue ()` of an X3DTimeDependentNode: its own flag, its context and
    /// `evenLive` (X3DTimeDependentNode.checkLiveState).
    pub fn time_node_live(&self, node: NodeId) -> bool {
        let even = self.private_field(node, "evenLive").is_some_and(|k| matches!(self.value(k), FieldValue::SFBool(true)));
        self.ctx_live(self.scene.node(node).ctx) || even
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
                self.time_set_live(node, now);
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
    /// The nodes are not disposed: the root nodes' SFNode elements are not the cached wrappers
    /// (`SFNodeCache .get (node) === this` in SFNode.dispose is false), so `node .dispose ()` is
    /// never called. A Script of the scene keeps its callbacks and gets no `shutdown ()`; its
    /// prepareEvents interest on the browser even keeps firing.
    pub(crate) fn dispose_scene(&mut self, root: CtxId) {
        for r in std::mem::take(&mut self.scene.contexts[root].routes) {
            self.dispose_route(r);
        }

        self.inline_live.insert(root, false);
        self.refresh_live();
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
            self.add_field_interest((r.source_node, r.source_loc), (r.destination_node, r.destination_loc));
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
                let exported = self.inline_scenes.get(&imp.inline).and_then(|&c| self.scene.get_exported_node(c, &imp.exported_name));

                match exported {
                    Some(n) => n,
                    None => return Err(format!("Unknown field '{name}' in node class X3DImportedNodeInstance.")),
                }
            }
            _ => node,
        };

        match self.scene.get_field(target, name) {
            Some(loc) => Ok((target, loc)),
            None => Err(format!("Unknown field '{name}' in node class {}.", self.scene.type_name(target))),
        }
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
                        "Bad ROUTE specification: source field type must match destination field type of fields named \"{}\" and \"{}\".",
                        sf.name, df.name
                    ))
                } else if !x3d_fields::access::is_output(sf.access) {
                    Err(format!("Bad ROUTE specification: source field \"{}\" must be an output.", sf.name))
                } else if !x3d_fields::access::is_input(df.access) {
                    Err(format!("Bad ROUTE specification: destination field \"{}\" must be an input.", df.name))
                } else {
                    Ok(())
                }
            }
            _ => Err("Couldn't connect route.".to_string()),
        };

        match result {
            Ok(()) => {
                let (s, d) = (src.unwrap(), dst.unwrap());
                let (sf, df) = (self.scene.field(s.0, s.1), self.scene.field(d.0, d.1));
                let source_name =
                    if sf.access == x3d_fields::access::INPUT_OUTPUT { format!("{}_changed", sf.name) } else { sf.name.clone() };
                let destination_name = if df.access == x3d_fields::access::INPUT_OUTPUT { format!("set_{}", df.name) } else { df.name.clone() };

                self.scene.field_mut(s.0, s.1).output_routes += 1;
                self.scene.field_mut(d.0, d.1).input_routes += 1;
                self.add_field_interest(s, d);

                let route = &mut self.scene.contexts[ctx].routes[i];
                route.conn = Some((s, d));
                route.source_field = source_name;
                route.destination_field = destination_name;
            }
            Err(e) => {
                let all_complete = self.route_inlines(&r).into_iter().all(|n| self.url_load_state(n) == LoadState::Complete);

                if all_complete {
                    self.log_warn(format!("Error: {e}"));
                }
            }
        }
    }
}
