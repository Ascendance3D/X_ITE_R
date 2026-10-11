//! X3D runtime ported from X_ITE (https://github.com/create3000/x_ite, MIT, Copyright (c) 2009
//! Holger Seelig and other contributors). Pinned reference: oracle/x_ite-pin (fork commit 34cdf9f97).
//!
//! What lives here (PLAN.md phases X3–X5, and the X6 Script node plumbing):
//!
//! * the event engine — X3DField.processEvent / addEvent / addEventObject, X3DBaseNode.addEvent /
//!   addNodeEvent, X3DRoutingContext processEvents, routes as field interests
//!   ([`events`]);
//! * PROTO instancing — X3DPrototypeInstance.realize and X3DNode.copy with IS references
//!   ([`protos`]);
//! * node behavior — initialize () side effects and interest callbacks of the node types the
//!   corpora use ([`nodes`]);
//! * loading — FileLoader URL fallback, relative URL resolution, gzip, Inline, EXTERNPROTO and
//!   external scripts against a local, read-only file root ([`loader`]);
//! * the Script node, with the JavaScript engine behind the [`ScriptHost`] trait ([`script`]).
//!
//! The oracle (oracle/x_ite-pin/src/dump.html) runs with `update="false"` and a frozen
//! `getCurrentTime` (946684800). [`Runtime::load_world`] reproduces what happens between
//! `createX3DFromURL` and its promise resolving: parse, setup, script loading, url object
//! loading and exactly one frame (FileLoader.set_loadCount__ awaits `browser .nextFrame ()`).

pub mod events;
pub mod inline;
pub mod loader;
pub mod nodes;
pub mod protos;
pub mod script;

use std::collections::HashMap;
use x3d_fields::{FieldValue, NodeId, access};
use x3d_scene::*;

pub use loader::{Fetcher, LocalFetcher};
pub use script::{ScriptCall, ScriptError, ScriptHost};

/// A field identity: node + location (X_ITE field object identity).
pub type FKey = (NodeId, FieldLoc);

/// The frozen browser time of the oracle (dump.html: FROZEN_TIME).
pub const FROZEN_TIME: f64 = 946684800.0;

/// Interest callbacks (X3DObject.addInterest (callbackName, object)). The node is the object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cb {
    // X3DTimeDependentNode
    TimeSetLoop,
    TimeSetEnabled,
    TimeSetStartTime,
    TimeSetPauseTime,
    TimeSetResumeTime,
    TimeSetStopTime,
    // TimeSensor
    TimeSensorSetCycleInterval,
    TimeSensorSetRange,
    // X3DInterpolatorNode
    InterpSetFraction,
    InterpSetKey,
    InterpSetKeyValue,
    // EventUtilities
    BooleanFilterSetBoolean,
    BooleanToggleSetBoolean,
    BooleanTriggerSetTriggerTime,
    IntegerTriggerSetBoolean,
    TimeTriggerSetBoolean,
    SequencerSetFraction,
    SequencerSetPrevious,
    SequencerSetNext,
    SequencerSetKey,
    SequencerSetKeyValue,
    // X3DUrlObject
    UrlSetLoad,
    UrlSetUrl,
    // Script (X3DScriptNode): field callback index into user-defined fields.
    ScriptField(usize),
    ScriptEventsProcessed,
    ScriptPrepareEvents,
    // X3DGroupingNode
    GroupAddChildren,
    GroupRemoveChildren,
    // X3DFollowerNode / X3DChaserNode / X3DDamperNode
    FollowerSetValue,
    FollowerSetDestination,
    FollowerSetDuration,
    /// The browser's prepareEvents interest of an active, live follower.
    FollowerPrepare,
    // Live frames (viewer): TimeSensor.set_prepare on timePrepareEvents, set_time on timeEvents.
    TimeSensorSetPrepare,
    TimeSensorSetTime,
    // X3DBindableNode.set_bind__
    BindableSetBind,
    // FileLoader.set_loadCount__ of a scene that an Inline loaded (interest on the scene's loadCount).
    SceneLoadCount,
    // X3DRoute.reconnect: interest on the loadState of an Inline that an imported node of the route belongs to.
    RouteReconnect(usize),
}

/// A pending `setTimeout` of X3DTimeDependentNode.addTimeout (virtual time).
#[derive(Clone, Debug)]
pub struct Timeout {
    pub time: f64,
    pub node: NodeId,
    /// "start", "pause", "resume" or "stop".
    pub name: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interest {
    pub node: NodeId,
    pub cb: Cb,
}

/// Per-field runtime state (X3DField / X3DChildObject private members).
#[derive(Clone, Debug, Default)]
pub struct FieldRt {
    /// `None`: tainted until the node's setup () starts (field .setTainted (!initialized)).
    pub tainted: Option<bool>,
    pub interests: Vec<Interest>,
    /// Routes and IS relations (X3DField [_fieldInterests]), insertion ordered set.
    pub field_interests: Vec<FKey>,
    /// IS references of runtime copies (X3DField [_references]).
    pub references: Vec<FKey>,
}

/// Per-node runtime state.
#[derive(Debug, Default)]
pub struct NodeRt {
    /// setup () has started: fields are untainted.
    pub setup_started: bool,
    /// X3DBaseNode [_initialized].
    pub initialized: bool,
    /// X3DChildObject [_tainted] of the node (pending eventsProcessed).
    pub tainted: bool,
    pub interests: Vec<Interest>,
    pub state: nodes::State,
}

/// An event (Events.js): the field whose value is delivered, and the fields already visited.
#[derive(Clone, Debug)]
pub struct Event {
    pub origin: FKey,
    pub visited: Vec<FKey>,
}

/// Something holding a node: X3DChildObject parents of an SFNode value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holder {
    Field(FKey),
    /// A PROTO interface field (parent is the X3DProtoDeclaration, clone count 1).
    ProtoField(ProtoId, usize),
    ExternField(ExternId, usize),
    /// X3DExecutionContext rootNodes (clone count 1).
    Root(CtxId),
}

/// Deferred work that X_ITE runs as promise continuations (microtasks) or after a load.
#[derive(Clone, Debug)]
pub enum Task {
    /// Script.loadData after `await this .unloadData ()`.
    ScriptLoad(NodeId),
    /// Script: setLoadState (COMPLETE/FAILED) after initialize__ resolved.
    ScriptLoaded(NodeId, bool),
    /// Inline.loadData / X3DUrlObject loadData: the fetch (a macrotask: it completes after the
    /// pending microtasks).
    UrlLoad(NodeId),
    ExternLoad(ExternId),
    /// A Script whose source is fetched (a macrotask, requested by the Script's microtask).
    ScriptFetch(NodeId),
}

pub struct Runtime {
    pub scene: Scene,
    pub frt: HashMap<FKey, FieldRt>,
    pub nrt: Vec<NodeRt>,
    tainted_fields: Vec<(FKey, Event)>,
    tainted_nodes: Vec<NodeId>,
    processing: bool,
    pub parents: HashMap<NodeId, Vec<Holder>>,
    /// console.error / console.warn output, formatted like dump.html (`error: …`, first line).
    pub log: Vec<String>,
    pub now: f64,
    pub tasks: std::collections::VecDeque<Task>,
    /// The browser's private event fields (prepareEvents, timeEvents, …) live on this node.
    pub browser_node: NodeId,
    pub script_host: Option<Box<dyn ScriptHost>>,
    pub fetcher: Box<dyn Fetcher>,
    /// Extern protos that finished loading: extern → (proto in loaded context).
    pub extern_loaded: HashMap<ExternId, Option<ProtoId>>,
    /// browser.loadFont cache: font URL → loaded.
    pub font_cache: HashMap<String, bool>,
    /// Script nodes in the order their contexts became active, for diagnostics.
    pub scripts: Vec<NodeId>,
    /// Inline.getInternalScene: Inline node → root context of the scene it loaded and attached.
    /// The scenes live in this runtime's node arena (`scene.merge_inline`), like X_ITE's one
    /// browser: one event queue, one set of browser events, one script realm.
    pub inline_scenes: HashMap<NodeId, CtxId>,
    /// Scenes that were parsed and set up but are not attached yet (FileLoader.setScene waits for
    /// their loadCount to reach 0 and then for the next frame).
    pub pending_scenes: Vec<inline::PendingScene>,
    /// `await browser .nextFrame ()` continuations, resolved after the finishEvents of a frame.
    pub frame_waiters: Vec<inline::FrameWaiter>,
    /// Fetches in flight in request order (macrotasks); `tasks` holds the microtasks.
    pub fetches: std::collections::VecDeque<Task>,
    /// Own live flag (X3DBaseNode [_live]) of the scenes that Inlines loaded, by root context.
    pub inline_live: HashMap<CtxId, bool>,
    /// Nodes that react to their live state (X3DTimeDependentNode.set_live__, X3DFollowerNode
    /// .set_live__), in the order their initialize () asked for the live event, with the state
    /// they last saw.
    pub live_watchers: Vec<(NodeId, bool)>,
    /// Record every console message (log / warn / error) in order in [`Runtime::console`].
    pub record_console: bool,
    /// All console output in the order it was logged: `log: text`, `warn: first line`, `error: first line`.
    pub console: Vec<String>,
    /// The scene is live (browser updating): TimeSensors tick on the browser's frames. The
    /// oracle runs (dump.html, `update="false"`) leave this false.
    pub live: bool,
    /// X3DTimeDependentNode timeouts (setTimeout), fired by [`Runtime::fire_timeouts`].
    pub timeouts: Vec<Timeout>,
    /// Browser.print / Browser.println output (console.log in X_ITE).
    pub prints: Vec<String>,
    /// Browser.description (X3DBrowser.setDescription).
    pub description: String,
    /// Requests scripts made of the browser (Browser.nextViewpoint …), drained by the viewer.
    pub browser_requests: Vec<String>,
    /// Bump counter for X3DBindableNode.modificationCount.
    pub bind_modifications: HashMap<NodeId, u64>,
    pub bind_counter: u64,
    /// Per node: the value of `stamp_counter` at the last field write (`set_raw`), 0 if never
    /// written. Lets a renderer notice which nodes changed without comparing field values.
    pub stamps: Vec<u64>,
    pub stamp_counter: u64,
    /// `getBrowser () .currentFrameRate` (X3DDamperNode.prepareEvents). 60 until a frame loop sets it.
    pub frame_rate: f64,
    /// The scene has gone live after loading (X3DBrowser.replaceWorld: `scene .setLive (true)`
    /// runs after the nodes were set up and their first events were processed). Followers
    /// register their prepareEvents interest only while this is set.
    pub scene_live: bool,
    /// Follower nodes in the order they were initialized (the order of their `set_live__`
    /// interests on the scene's live field).
    pub followers: Vec<NodeId>,
    /// Process-unique id of this runtime (stable when an Inline's runtime moves in its map).
    pub uid: u64,
}

impl Runtime {
    pub fn new(scene: Scene, fetcher: Box<dyn Fetcher>) -> Runtime {
        let mut rt = Runtime {
            scene,
            frt: HashMap::new(),
            nrt: vec![],
            tainted_fields: vec![],
            tainted_nodes: vec![],
            processing: false,
            parents: HashMap::new(),
            log: vec![],
            now: FROZEN_TIME,
            tasks: Default::default(),
            browser_node: 0,
            script_host: None,
            fetcher,
            extern_loaded: HashMap::new(),
            font_cache: HashMap::new(),
            scripts: vec![],
            inline_scenes: HashMap::new(),
            pending_scenes: vec![],
            frame_waiters: vec![],
            fetches: Default::default(),
            inline_live: HashMap::new(),
            live_watchers: vec![],
            record_console: false,
            console: vec![],
            live: false,
            timeouts: vec![],
            prints: vec![],
            description: String::new(),
            browser_requests: vec![],
            bind_modifications: HashMap::new(),
            bind_counter: 0,
            stamps: vec![],
            stamp_counter: 0,
            frame_rate: 60.0,
            scene_live: false,
            followers: vec![],
            uid: {
                static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            },
        };

        rt.browser_node = rt.create_internal_node("X3DBrowser");

        for name in ["prepareEvents", "timePrepareEvents", "timeEvents", "cameraEvents", "sensorEvents", "displayEvents", "finishEvents"] {
            rt.add_private_field(rt.browser_node, name, access::OUTPUT_ONLY, FieldValue::SFTime(0.0));
        }

        rt.nrt_mut(rt.browser_node).setup_started = true;
        rt.nrt_mut(rt.browser_node).initialized = true;
        rt.rebuild_parents();
        rt
    }

    pub fn log_error(&mut self, message: impl AsRef<str>) {
        let line = format!("error: {}", message.as_ref().split('\n').next().unwrap_or(""));

        if self.record_console {
            self.console.push(line.clone());
        }

        self.log.push(line);
    }

    pub fn log_warn(&mut self, message: impl AsRef<str>) {
        let line = format!("warn: {}", message.as_ref().split('\n').next().unwrap_or(""));

        if self.record_console {
            self.console.push(line.clone());
        }

        self.log.push(line);
    }

    /// Browser.print / Browser.println: `console .log (string)`.
    pub fn print(&mut self, text: String) {
        if self.record_console {
            self.console.push(format!("log: {text}"));
        }

        self.prints.push(text);
    }

    fn create_internal_node(&mut self, name: &'static str) -> NodeId {
        self.scene.nodes.push(Node {
            kind: NodeKind::Internal(name),
            ctx: ROOT,
            name: String::new(),
            fields: vec![],
            user_fields: vec![],
            private_fields: vec![],
            can_user_defined_fields: false,
        });

        (self.scene.nodes.len() - 1) as NodeId
    }

    // --- node and field state ------------------------------------------------------------

    pub fn nrt(&self, id: NodeId) -> Option<&NodeRt> {
        self.nrt.get(id as usize)
    }

    pub fn nrt_mut(&mut self, id: NodeId) -> &mut NodeRt {
        let i = id as usize;

        if self.nrt.len() <= i {
            self.nrt.resize_with(self.scene.nodes.len().max(i + 1), NodeRt::default);
        }

        &mut self.nrt[i]
    }

    pub fn is_initialized(&self, id: NodeId) -> bool {
        self.nrt(id).is_some_and(|n| n.initialized)
    }

    pub fn frt_mut(&mut self, k: FKey) -> &mut FieldRt {
        self.frt.entry(k).or_default()
    }

    pub fn is_field_tainted(&self, k: FKey) -> bool {
        match self.frt.get(&k).and_then(|f| f.tainted) {
            Some(t) => t,
            None => !self.nrt(k.0).is_some_and(|n| n.setup_started),
        }
    }

    pub fn set_field_tainted(&mut self, k: FKey, value: bool) {
        self.frt_mut(k).tainted = Some(value);
    }

    pub fn value(&self, k: FKey) -> &FieldValue {
        &self.scene.field(k.0, k.1).value
    }

    pub fn field(&self, k: FKey) -> &Field {
        self.scene.field(k.0, k.1)
    }

    pub fn type_name(&self, id: NodeId) -> String {
        self.scene.type_name(id)
    }

    /// Builtin type name, or `None` for proto instances and internal nodes.
    pub fn builtin_name(&self, id: NodeId) -> Option<&'static str> {
        match self.scene.node(id).kind {
            NodeKind::Builtin(t) => Some(self.scene.table.types[t].name.as_str()),
            _ => None,
        }
    }

    /// Add a private child object (X3DBaseNode.addChildObject); returns its key.
    pub fn add_private_field(&mut self, node: NodeId, name: &str, access_type: u8, value: FieldValue) -> FKey {
        let n = self.scene.node_mut(node);

        if let Some(i) = n.private_fields.iter().position(|f| f.name == name) {
            return (node, FieldLoc::Priv(i));
        }

        n.private_fields.push(Field::new(name, access_type, value));
        (node, FieldLoc::Priv(n.private_fields.len() - 1))
    }

    pub fn private_field(&self, node: NodeId, name: &str) -> Option<FKey> {
        self.scene.node(node).private_fields.iter().position(|f| f.name == name).map(|i| (node, FieldLoc::Priv(i)))
    }

    /// X3DBaseNode.getField (name) → key.
    pub fn get_field(&self, node: NodeId, name: &str) -> Option<FKey> {
        self.scene.get_field(node, name).map(|l| (node, l))
    }

    /// Predefined field by exact name (this ._name).
    pub fn pre(&self, node: NodeId, name: &str) -> Option<FKey> {
        self.scene.node(node).fields.iter().position(|f| f.name == name).map(|i| (node, FieldLoc::Pre(i)))
    }

    pub fn pre_value(&self, node: NodeId, name: &str) -> Option<&FieldValue> {
        self.pre(node, name).map(|k| self.value(k))
    }

    pub fn get_bool(&self, node: NodeId, name: &str) -> bool {
        matches!(self.pre_value(node, name), Some(FieldValue::SFBool(true)))
    }

    pub fn get_f64(&self, node: NodeId, name: &str) -> f64 {
        match self.pre_value(node, name) {
            Some(FieldValue::SFTime(v) | FieldValue::SFFloat(v) | FieldValue::SFDouble(v)) => *v,
            Some(FieldValue::SFInt32(v)) => *v as f64,
            _ => 0.0,
        }
    }

    // --- interests ------------------------------------------------------------------------

    /// X3DObject.addInterest: replaces an existing entry in place (Map.set keeps the position).
    pub fn add_interest(&mut self, k: FKey, node: NodeId, cb: Cb) {
        let interests = &mut self.frt_mut(k).interests;
        let i = Interest { node, cb };

        if !interests.contains(&i) {
            interests.push(i);
        }
    }

    pub fn remove_interest(&mut self, k: FKey, node: NodeId, cb: Cb) {
        if let Some(f) = self.frt.get_mut(&k) {
            f.interests.retain(|i| !(i.node == node && i.cb == cb));
        }
    }

    pub fn add_node_interest(&mut self, target: NodeId, node: NodeId, cb: Cb) {
        let interests = &mut self.nrt_mut(target).interests;
        let i = Interest { node, cb };

        if !interests.contains(&i) {
            interests.push(i);
        }
    }

    pub fn add_field_interest(&mut self, from: FKey, to: FKey) {
        let fi = &mut self.frt_mut(from).field_interests;

        if !fi.contains(&to) {
            fi.push(to);
        }
    }

    pub fn remove_field_interest(&mut self, from: FKey, to: FKey) {
        if let Some(f) = self.frt.get_mut(&from) {
            f.field_interests.retain(|k| *k != to);
        }
    }

    // --- parents / clone counts -------------------------------------------------------------

    fn node_values(value: &FieldValue) -> Vec<NodeId> {
        match value {
            FieldValue::SFNode(Some(n)) => vec![*n],
            FieldValue::MFNode(list) => list.iter().flatten().copied().collect(),
            _ => vec![],
        }
    }

    fn add_parent(&mut self, node: NodeId, holder: Holder) {
        let list = self.parents.entry(node).or_default();

        if !list.contains(&holder) {
            list.push(holder);
        }
    }

    /// Rebuild the parent index from the scene (after parsing or merging scenes).
    pub fn rebuild_parents(&mut self) {
        self.parents.clear();

        let mut adds = vec![];

        for (id, node) in self.scene.nodes.iter().enumerate() {
            for (i, f) in node.fields.iter().enumerate() {
                for n in Self::node_values(&f.value) {
                    adds.push((n, Holder::Field((id as NodeId, FieldLoc::Pre(i)))));
                }
            }

            for (i, f) in node.user_fields.iter().enumerate() {
                for n in Self::node_values(&f.value) {
                    adds.push((n, Holder::Field((id as NodeId, FieldLoc::User(i)))));
                }
            }
        }

        for (p, proto) in self.scene.protos.iter().enumerate() {
            for (i, f) in proto.fields.iter().enumerate() {
                for n in Self::node_values(&f.value) {
                    adds.push((n, Holder::ProtoField(p, i)));
                }
            }
        }

        for (e, proto) in self.scene.externprotos.iter().enumerate() {
            for (i, f) in proto.fields.iter().enumerate() {
                for n in Self::node_values(&f.value) {
                    adds.push((n, Holder::ExternField(e, i)));
                }
            }
        }

        for (c, ctx) in self.scene.contexts.iter().enumerate() {
            for n in ctx.root_nodes.iter().flatten() {
                adds.push((*n, Holder::Root(c)));
            }
        }

        for (n, h) in adds {
            self.add_parent(n, h);
        }
    }

    /// X3DNode.getCloneCount: one per non-private holder.
    pub fn clone_count(&self, node: NodeId) -> usize {
        self.parents.get(&node).map(|l| l.iter().filter(|h| !matches!(h, Holder::Field((_, FieldLoc::Priv(_))))).count()).unwrap_or(0)
    }

    /// Record a field write on `node` (see [`Runtime::stamps`]).
    pub fn touch(&mut self, node: NodeId) {
        let i = node as usize;

        if self.stamps.len() <= i {
            self.stamps.resize(i + 1, 0);
        }

        self.stamp_counter += 1;
        self.stamps[i] = self.stamp_counter;
    }

    /// The stamp of the last field write on `node`; changes whenever a field of the node is set.
    pub fn stamp(&self, node: NodeId) -> u64 {
        self.stamps.get(node as usize).copied().unwrap_or(0)
    }

    /// Set a field's value without an event (X3DField.set), keeping the parent index.
    pub fn set_raw(&mut self, k: FKey, value: FieldValue) {
        let old = Self::node_values(self.value(k));
        let new = Self::node_values(&value);

        self.scene.field_mut(k.0, k.1).value = value;
        self.touch(k.0);

        if old.is_empty() && new.is_empty() {
            return;
        }

        for n in old {
            if !new.contains(&n) {
                if let Some(l) = self.parents.get_mut(&n) {
                    l.retain(|h| *h != Holder::Field(k));
                }
            }
        }

        for n in new {
            self.add_parent(n, Holder::Field(k));
        }
    }

    /// X3DField.setValue: set and generate an event.
    pub fn set_value(&mut self, k: FKey, value: FieldValue) {
        self.set_raw(k, value);
        self.add_event(k);
    }

    /// Store the scene-wide clone counts for the generator (X3DGenerator.NeedsName).
    pub fn publish_clone_counts(&mut self) {
        let counts = (0..self.scene.nodes.len() as NodeId).map(|n| (n, self.clone_count(n))).filter(|(_, c)| *c > 1).collect();
        self.scene.clone_counts = Some(counts);
    }
}

impl Runtime {
    /// After parsing: construct runtime state, connect the parsed routes, and set up the nodes
    /// in X3DParser.setupNodes order (which also realizes PROTO instances).
    pub fn start(&mut self) {
        for n in 0..self.scene.nodes.len() as NodeId {
            self.nrt_mut(n);

            if !matches!(self.scene.node(n).kind, NodeKind::Internal(_)) {
                self.construct_node(n);
            }
        }

        self.connect_parsed_routes();
        self.rebuild_parents();

        for n in self.scene.setup_order.clone() {
            self.setup_node(n);
        }
    }

    /// Run deferred work like the JavaScript event loop: microtasks first (`tasks`), then the
    /// oldest fetch (a macrotask) whose completion queues new microtasks, and so on until quiet.
    /// Fetches complete in request order; a Script requests its source when its own microtask
    /// runs, after the microtasks that were queued before it (Inlines and textures asked for
    /// theirs at setup).
    pub fn run_tasks(&mut self) {
        loop {
            if let Some(task) = self.tasks.pop_front() {
                match task {
                    Task::ScriptLoad(node) => {
                        let first = self.url_strings(node).into_iter().find(|u| !u.is_empty()).unwrap_or_default();
                        let inline = ["ecmascript:", "javascript:", "vrmlscript:"].iter().any(|s| first.trim_start().starts_with(s));

                        if inline || first.is_empty() {
                            self.script_load(node);
                        } else {
                            self.fetches.push_back(Task::ScriptFetch(node));
                        }
                    }
                    other => self.fetches.push_back(other),
                }

                continue;
            }

            let Some(task) = self.fetches.pop_front() else {
                break;
            };

            match task {
                Task::UrlLoad(node) => self.url_load(node),
                Task::ExternLoad(e) => self.extern_load(e),
                Task::ScriptFetch(node) => self.script_load(node),
                Task::ScriptLoad(..) | Task::ScriptLoaded(..) => {}
            }
        }
    }

    /// Everything that settles between two frames: microtasks and the fetches they wait for.
    pub fn settle(&mut self) {
        self.run_tasks();
    }

    /// Is a load of an Inline still in progress (X3DBrowser loadCount > 0)?
    pub fn has_unfinished_loads(&self) -> bool {
        !self.pending_scenes.is_empty()
            || !self.frame_waiters.is_empty()
            || (0..self.scene.nodes.len() as NodeId).any(|n| {
                self.builtin_name(n) == Some("Inline") && self.url_load_state(n) == nodes::LoadState::InProgress
            })
    }

    /// Everything `browser .createX3DFromURL` does before its promise resolves: setup, loads,
    /// the frames the loads wait for. Returns the scene ready for `toVRMLString`.
    pub fn load_world(&mut self) {
        self.load_scene();
        self.go_live();
    }

    /// X3DBrowser.replaceWorld after the scene is loaded: it awaits one more frame (the events of
    /// the new scene are processed before the old world goes), then `scene .setLive (true)`.
    pub fn replace_world(&mut self) {
        self.frame();
        self.run_tasks();
        self.go_live();
    }

    /// The loading part of [`Runtime::load_world`], without `go_live`.
    pub fn load_scene(&mut self) {
        for w in std::mem::take(&mut self.scene.warnings) {
            self.log_warn(w);
        }

        self.start();
        self.run_tasks();

        // FileLoader.set_loadCount__ awaits `browser .nextFrame ()` once the scene's loadCount is
        // 0, and every Inline in it holds the count until its own scene is attached: one frame,
        // plus one for every level of nested Inlines.
        loop {
            let waiting = self.has_unfinished_loads();

            self.frame();
            self.run_tasks();

            if !waiting {
                break;
            }
        }

        self.publish_clone_counts();
    }

    /// `scene .setLive (true)`: every node's `set_live__` runs, in the order the nodes asked for
    /// their live event. Followers that are active start ticking with the browser's
    /// prepareEvents now, not when they were initialized.
    pub fn go_live(&mut self) {
        if !self.live || self.scene_live {
            return;
        }

        self.scene_live = true;

        for node in self.followers.clone() {
            self.follower_set_live(node);
        }

        self.refresh_live();
    }
}

impl Runtime {
    /// Merge a scene created at runtime (an X3DScene with its own root context), connect its
    /// routes and set up its nodes like X3DParser.setupNodes. Returns the merge map.
    pub fn import_scene(&mut self, mut scene: Scene, parent: CtxId, outer: Option<NodeId>) -> MergeMap {
        for w in std::mem::take(&mut scene.warnings) {
            self.log_warn(w);
        }

        let setup_order = std::mem::take(&mut scene.setup_order);
        let ctx_count = scene.contexts.len();
        let m = self.scene.merge(scene, parent, outer);

        for n in m.node_offset..self.scene.nodes.len() as NodeId {
            self.nrt_mut(n);
            self.construct_node(n);
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

        m
    }

    /// X3DBrowser.createVrmlFromString: parse `#VRML V2.0 utf8` + text with the current
    /// scene's world URL; the new scene's root nodes.
    pub fn create_vrml_from_string(&mut self, text: &str, parent: CtxId) -> Result<Vec<Option<NodeId>>, String> {
        let source = format!("#VRML V2.0 utf8\n\n{text}");
        let world_url = self.scene.world_url.clone();
        let scene = match x3d_parse::Parser::new(&source, &world_url).parse() {
            Ok(scene) => scene,
            Err(e) => {
                // VRMLParser (DEVELOPMENT) prints the original error before it is rethrown.
                self.log_error(format!("Error: {}", e.message));
                return Err(format!("Error: \n{e}"));
            }
        };
        let m = self.import_scene(scene, parent, None);

        Ok(self.scene.ctx(m.root_ctx).root_nodes.clone())
    }
}

impl Runtime {
    /// Script.js `new SFNode (vrmlSyntax)`: SFNode.fromString in the script's context, then set
    /// up the new nodes. `Err` is the message of the thrown Error.
    pub fn sfnode_from_string(&mut self, text: &str, ctx: CtxId) -> Result<Option<NodeId>, String> {
        let first_new = self.scene.nodes.len() as NodeId;
        let first_setup = self.scene.setup_order.len();
        let scene = std::mem::replace(&mut self.scene, Scene::new(""));
        let (scene, result) = x3d_parse::Parser::parse_sfnode_into(scene, ctx, text);

        self.scene = scene;

        for w in std::mem::take(&mut self.scene.warnings) {
            self.log_warn(w);
        }

        for n in first_new..self.scene.nodes.len() as NodeId {
            self.nrt_mut(n);
            self.construct_node(n);
        }

        self.rebuild_parents();

        let setup: Vec<NodeId> = self.scene.setup_order.drain(first_setup..).collect();

        for n in setup {
            self.setup_node(n);
        }

        match result {
            Ok(Some(FieldValue::SFNode(Some(n)))) => Ok(Some(n)),
            Ok(Some(_)) => Ok(None),
            Ok(None) => Err("Couldn't read value for field ''.".into()),
            Err(e) => Err(e.message),
        }
    }

    /// X3DExecutionContext.deleteRoute: also removes the field interest.
    pub fn delete_route(&mut self, ctx: CtxId, source: NodeId, source_field: &str, destination: NodeId, destination_field: &str) {
        if let Some(r) = self.scene.delete_route(ctx, source, source_field, destination, destination_field) {
            self.remove_field_interest((r.source_node, r.source_loc), (r.destination_node, r.destination_loc));
        }
    }

    /// X3DExecutionContext.createNode (typeName): created and set up.
    pub fn create_node_setup(&mut self, ctx: CtxId, type_name: &str) -> Option<NodeId> {
        let n = self.scene.create_node(ctx, type_name)?;

        for w in std::mem::take(&mut self.scene.warnings) {
            self.log_warn(w);
        }

        self.nrt_mut(n);
        self.construct_node(n);
        self.setup_node(n);
        Some(n)
    }

    /// X3DExecutionContext.createProto (name): instance created and set up.
    pub fn create_proto_setup(&mut self, ctx: CtxId, name: &str) -> Option<NodeId> {
        let n = self.scene.create_proto_instance(ctx, name)?;

        self.nrt_mut(n);
        self.construct_node(n);

        let values: Vec<(usize, FieldValue)> = self.scene.node(n).fields.iter().map(|f| f.value.clone()).enumerate().collect();
        for (i, v) in values {
            for m in Self::node_values_pub(&v) {
                self.add_parent_pub(m, Holder::Field((n, FieldLoc::Pre(i))));
            }
        }

        self.setup_node(n);
        Some(n)
    }
}
