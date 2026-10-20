//! The Script node (Components/Scripting/Script.js, X3DScriptNode.js) without the engine:
//! url loading, callback wiring (initialize__), set_field__, call__, setError. The ECMAScript
//! engine is a [`ScriptHost`] (crate x3d-script implements it with QuickJS).

use crate::loader::{FileLoader, Loaded};
use crate::*;

/// A thrown value, as `String (error)` (what dump.html's console capture prints).
#[derive(Clone, Debug)]
pub struct ScriptError(pub String);

/// A callback invocation.
#[derive(Clone, Debug)]
pub enum ScriptCall {
    /// initialize, prepareEvents, eventsProcessed, shutdown: `callback (time)`.
    Plain(String),
    /// A field callback: `callback (value, time)` with a copy of the field value.
    Field(String, FieldValue),
}

pub trait ScriptHost {
    /// Script.createContext: evaluate the source in a fresh global object for `node` and
    /// return which of `callbacks` are functions.
    fn create_context(&mut self, rt: &mut Runtime, node: NodeId, source: &str, callbacks: &[String]) -> Result<Vec<bool>, ScriptError>;

    /// Script.call__ / set_field__.
    fn call(&mut self, rt: &mut Runtime, node: NodeId, call: ScriptCall) -> Result<(), ScriptError>;

    /// A promise of X3DBrowser.createX3DFromString / createX3DFromURL settles: request `id` made the scene with this
    /// root context, or was rejected with the message.
    fn settle(&mut self, _rt: &mut Runtime, _id: u64, _outcome: Result<CtxId, String>) {}

    /// A promise of X3DBrowser.loadURL / replaceWorld settles (it resolves without a value).
    fn settle_plain(&mut self, _rt: &mut Runtime, _id: u64, _outcome: Result<(), String>) {}

    /// X3DBrowser.callBrowserCallbacks (event): the functions registered with `addBrowserCallback` run.
    fn browser_event(&mut self, _rt: &mut Runtime, _event: u32) {}

    /// A change field of an execution context fires (`sceneGraph_changed` = 0, `bbox_changed` = 1, `profile_changed` = 2,
    /// `metadata_changed` = 3): the callbacks Scripts gave `addFieldCallback` run with the field's value.
    fn ctx_event(&mut self, _rt: &mut Runtime, _ctx: CtxId, _field: u32) {}

    /// X3DField.processEvent of a field in [`Runtime::field_observed`]: the callbacks Scripts gave `addFieldCallback`
    /// run with the field's value, after the event went on to the routes.
    fn field_event(&mut self, _rt: &mut Runtime, _k: FKey) {}

    /// The due time (ms) of the earliest `setTimeout` / `setInterval` timer of the Scripts, if it is due at `until_ms`.
    fn next_timer(&mut self, _rt: &mut Runtime, _until_ms: f64) -> Option<f64> {
        None
    }

    /// Run the timer `next_timer` named (it is removed, or rearmed for an interval).
    fn fire_timer(&mut self, _rt: &mut Runtime) {}

    /// The Script is gone for good (`shutdown ()` has run): forget its global object and callbacks.
    fn destroy_context(&mut self, _node: NodeId) {}

    /// The nodes were released and their ids may be handed out again: a wrapper of one of them
    /// that a Script still holds points to `tombstone` from now on.
    fn release_nodes(&mut self, _rt: &mut Runtime, _nodes: &[NodeId], _tombstone: NodeId) {}

    /// Which of `candidates` (nodes only JavaScript could still hold) JavaScript can still reach: through
    /// an `SFNode` object, an `MFNode` element, a field object of the node. The host collects garbage
    /// first, so an object nothing refers to does not count (X_ITE: the garbage collector).
    ///
    /// `None` means the host cannot say. The runtime then treats every runtime-created node as held
    /// as long as a Script exists (conservative: nothing is released that a Script might use).
    fn held_nodes(&mut self, _rt: &mut Runtime, _candidates: &[NodeId]) -> Option<Vec<NodeId>> {
        None
    }
}

#[derive(Debug, Default, Clone)]
pub struct ScriptState {
    pub load_state: nodes::LoadState,
    /// Callback names connected for this script (name, is function).
    pub functions: Vec<String>,
    /// The Script belongs to a scene that was disposed: it takes no more events.
    pub closed: bool,
}

impl Runtime {
    fn script_state(&mut self, node: NodeId) -> &mut ScriptState {
        match &mut self.nrt_mut(node).state {
            nodes::State::Script(s) => s,
            _ => panic!("not a Script"),
        }
    }

    /// Script.initialize: X3DUrlObject.initialize, then requestImmediateLoad.
    pub(crate) fn script_initialize(&mut self, node: NodeId) {
        self.interest(node, "load", Cb::UrlSetLoad);
        self.interest(node, "url", Cb::UrlSetUrl);

        if let Some(k) = self.private_field(node, "loadData") {
            self.add_interest(k, node, Cb::UrlLoadData);
        }

        if self.script_state(node).load_state != nodes::LoadState::NotStarted {
            return;
        }

        // requestImmediateLoad: no load without the LoadUrlObjects option.
        if !self.browser_options.load_url_objects() || !self.get_bool(node, "load") || self.url_strings(node).is_empty() {
            return;
        }

        self.set_load_state(node, nodes::LoadState::InProgress);

        // loadData: `await this .unloadData ()` — continues as a microtask.
        self.tasks.push_back(Task::ScriptLoad(node));
    }

    /// Script.loadData (the `_loadData` event of a reload): `await this .unloadData ()`, then the fetch.
    pub(crate) fn script_load_data(&mut self, node: NodeId) {
        self.script_unload_data(node);
        self.tasks.push_back(Task::ScriptLoad(node));
    }

    /// Script.loadData after its first await.
    pub(crate) fn script_load(&mut self, node: NodeId) {
        let loader = FileLoader { type_name: "Script".into(), base: self.base_url(node), data_as_string: true };
        let urls = self.url_strings(node);

        let ok = self.load_document(&loader, &urls, |rt, loaded| {
            let source = match loaded {
                Loaded::Script(s) => s,
                Loaded::Data(d, _) => x3d_parse::decode_text(&d),
            };

            rt.script_initialize_source(node, &source)
        });

        self.set_load_state(node, if ok { nodes::LoadState::Complete } else { nodes::LoadState::Failed });
    }

    /// Script.initialize__ (sourceText).
    fn script_initialize_source(&mut self, node: NodeId, source: &str) -> Result<(), String> {
        let mut callbacks: Vec<String> = ["initialize", "prepareEvents", "eventsProcessed", "shutdown"].iter().map(|s| s.to_string()).collect();
        let user: Vec<(usize, String, u8)> =
            self.scene.node(node).user_fields.iter().enumerate().map(|(i, f)| (i, f.name.clone(), f.access)).collect();

        for (_, name, a) in &user {
            match *a {
                access::INPUT_ONLY => callbacks.push(name.clone()),
                access::INPUT_OUTPUT => callbacks.push(format!("set_{name}")),
                _ => {}
            }
        }

        let Some(mut host) = self.script_host.take() else {
            return Err("Error: no script engine".into());
        };

        let result = host.create_context(self, node, source, &callbacks);
        self.script_host = Some(host);

        let defined = result.map_err(|e| e.0)?;
        let is_fn = |name: &str| callbacks.iter().position(|c| c == name).is_some_and(|i| defined[i]);

        self.scripts.push(node);
        self.script_state(node).functions = callbacks.iter().zip(&defined).filter(|(_, d)| **d).map(|(c, _)| c.clone()).collect();

        if is_fn("prepareEvents") {
            let k = self.private_field(self.browser_node, "prepareEvents").unwrap();
            self.add_interest(k, node, Cb::ScriptPrepareEvents);
        }

        if is_fn("eventsProcessed") {
            self.add_node_interest(node, node, Cb::ScriptEventsProcessed);
        }

        for (i, name, a) in &user {
            let cb = match *a {
                access::INPUT_ONLY => name.clone(),
                access::INPUT_OUTPUT => format!("set_{name}"),
                _ => continue,
            };

            if is_fn(&cb) {
                self.add_interest((node, FieldLoc::User(*i)), node, Cb::ScriptField(*i));
            }
        }

        if is_fn("initialize") {
            self.script_call(node, ScriptCall::Plain("initialize".into()), "initialize");
        }

        Ok(())
    }

    /// The Script of a scene that was disposed (an Inline whose scene was replaced, a world that was
    /// unloaded): ISO/IEC 14772-1 4.12.3 asks for `shutdown ()` when the Script node is deleted or
    /// the world containing it is unloaded or replaced, and for no other method after it.
    ///
    /// X_ITE calls `shutdown` from Script.unloadData (reload, `load` FALSE, dispose) but not when an
    /// Inline's scene is replaced; this port also does then. The call comes first, while the Script is still
    /// connected, so what it sends is queued. The routes of the disposed scene are deleted right after
    /// (see [`Runtime::dispose_scene`]), so events for nodes that the same action deletes are not
    /// delivered, and the Script takes no event once the call has returned.
    pub(crate) fn script_close(&mut self, node: NodeId) {
        if self.script_state(node).closed {
            return;
        }

        // Set first: a `shutdown ()` that reaches its own Script, or disposes its scene again, finds it closed.
        self.script_state(node).closed = true;

        self.script_unload_data(node);
    }

    /// Script.unloadData: `shutdown ()` of the loaded source, then prepareEvents, eventsProcessed and
    /// the field callbacks are disconnected. Also runs before every reload (Script.loadData) and when
    /// `load` becomes FALSE (requestUnload).
    pub(crate) fn script_unload_data(&mut self, node: NodeId) {
        // Only a Script that was initialized has anything to shut down.
        let initialized = self.scripts.contains(&node);
        let has_shutdown = self.script_state(node).functions.iter().any(|f| f == "shutdown");

        // `this .shutdown = null`: a reentrant unload does not call it twice.
        self.script_state(node).functions.clear();

        if initialized && has_shutdown {
            self.script_call(node, ScriptCall::Plain("shutdown".into()), "shutdown");
        }

        if let Some(k) = self.private_field(self.browser_node, "prepareEvents") {
            self.remove_interest(k, node, Cb::ScriptPrepareEvents);
        }

        self.nrt_mut(node).interests.retain(|i| !(i.node == node && i.cb == Cb::ScriptEventsProcessed));

        for i in 0..self.scene.node(node).user_fields.len() {
            self.remove_interest((node, FieldLoc::User(i)), node, Cb::ScriptField(i));
        }

        self.scripts.retain(|&s| s != node);

        if initialized {
            if let Some(mut host) = self.script_host.take() {
                host.destroy_context(node);
                self.script_host = Some(host);
            }
        }
    }

    /// call__ / set_field__ with setError on a thrown value.
    fn script_call(&mut self, node: NodeId, call: ScriptCall, reason_name: &str) {
        // After `shutdown ()` no other method of the Script may run (4.12.3).
        if self.script_state(node).closed && !matches!(&call, ScriptCall::Plain(n) if n == "shutdown") {
            return;
        }

        let Some(mut host) = self.script_host.take() else {
            return;
        };

        let result = host.call(self, node, call);
        self.script_host = Some(host);

        // A callback may have dropped its last reference to a runtime-created node.
        self.dynamic_dirty = true;

        if let Err(e) = result {
            self.report_script_error(node, reason_name, &e.0);
        }
    }

    /// Script.setError: the header (script name, reason, world url) and the error itself.
    pub fn report_script_error(&mut self, node: NodeId, reason_name: &str, error: &str) {
        let name = self.scene.node(node).name.clone();
        let world = self.scene.world_url_of(self.scene.node(node).ctx).to_string();
        let world = if world.starts_with("data:") { "data:".to_string() } else { world };

        self.log_error(format!("JavaScript Error in Script '{name}', in function '{reason_name}'\nworld url is '{world}':"));
        self.log_error(error);
    }

    pub(crate) fn script_interest(&mut self, node: NodeId, cb: Cb, _k: FKey) {
        match cb {
            Cb::ScriptField(i) => {
                let Some(f) = self.scene.node(node).user_fields.get(i) else {
                    return;
                };
                let (name, access_type, value) = (f.name.clone(), f.access, f.value.clone());
                let callback = if access_type == access::INPUT_OUTPUT { format!("set_{name}") } else { name.clone() };

                self.script_call(node, ScriptCall::Field(callback, value), &name);
            }
            Cb::ScriptEventsProcessed => self.script_call(node, ScriptCall::Plain("eventsProcessed".into()), "eventsProcessed"),
            Cb::ScriptPrepareEvents => self.script_call(node, ScriptCall::Plain("prepareEvents".into()), "prepareEvents"),
            _ => {}
        }
    }
}
