//! The Script node (Components/Scripting/Script.js, X3DScriptNode.js) without the engine:
//! url loading, callback wiring (initialize__), set_field__, call__, setError. The ECMAScript
//! engine is a [`ScriptHost`] (crate x3d-script: QuickJS natively, the page's own engine in the browser build).

use crate::loader::{self, FileLoader};
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
    fn create_context(
        &mut self,
        rt: &mut Runtime,
        node: NodeId,
        source: &str,
        callbacks: &[String],
    ) -> Result<Vec<bool>, ScriptError>;

    /// Script.call__ / set_field__.
    fn call(&mut self, rt: &mut Runtime, node: NodeId, call: ScriptCall)
    -> Result<(), ScriptError>;

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

    /// The raw components a print meets, in print order (see [`x3d_scene::RawPrint`]): for each request, `String (component)`
    /// or its ToNumber, taken now in the host's realm; None when the host does not hold the value.
    fn raw_coerce(
        &mut self,
        _rt: &mut Runtime,
        requests: &[RawRequest],
    ) -> Vec<Option<Vec<Option<RawComponent>>>> {
        vec![None; requests.len()]
    }

    /// The Script is gone for good (`shutdown ()` has run): forget its global object and callbacks.
    fn destroy_context(&mut self, _node: NodeId) {}

    /// The runtime that owns the host moved to `rt`. A host whose JavaScript can run between host calls (the page's
    /// engine runs promise jobs after the call returns) reaches the runtime there from now on.
    fn attach(&mut self, _rt: &mut Runtime) {}

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

    /// Which of `candidates` (root contexts of scenes that `createX3DFromURL` / `createVrmlFromURL` gave a Script, none of
    /// whose nodes anything else keeps) JavaScript can still reach through their X3DScene object. Like
    /// [`ScriptHost::held_nodes`], the host collects garbage first; `None` means the host cannot say, and every scene is held.
    fn held_scenes(&mut self, _rt: &mut Runtime, _candidates: &[CtxId]) -> Option<Vec<CtxId>> {
        None
    }

    /// The scenes were released and their context ids may be handed out again: the host forgets its objects and
    /// callbacks of them.
    fn release_scenes(&mut self, _rt: &mut Runtime, _ctxs: &[CtxId]) {}
}

/// A Script candidate that failed: what FileLoader.printError reports when the next candidate starts.
#[derive(Clone, Debug)]
pub struct CandidateError {
    pub candidate: String,
    pub resolved: Option<url::Url>,
    pub error: String,
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
        if !self.browser_options.load_url_objects()
            || !self.get_bool(node, "load")
            || self.url_strings(node).is_empty()
        {
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

    fn script_loader(&self, node: NodeId) -> FileLoader {
        FileLoader {
            type_name: "Script".into(),
            base: self.base_url(node),
            data_as_string: true,
        }
    }

    /// FileLoader.loadDocumentAsync of Script candidate `index` (Script.loadData's FileLoader), after printing the failure of the
    /// candidate before it (`failed`, see loadDocumentError). The microtask ticks until the next candidate starts are X_ITE's:
    ///
    /// * an empty URL: loadDocumentError at once (`return this .loadDocumentError (...)`), the next candidate in the same tick;
    /// * a Script URL or a data URL that is not base64: the callback runs `initialize__` now; when it throws, the rejection
    ///   passes `await this .initialize__`, `await this .dataCallback` and `.catch (loadDocumentError)`: three ticks;
    /// * a URL `new URL` cannot parse: loadDocumentAsync rejects at once, `.catch` one tick later;
    /// * a URL `fetch` rejects at once (a scheme it does not load, a data URL without a comma): `await fetch`, `.catch`: two
    ///   ticks;
    /// * anything else is fetched (a macrotask, [`Runtime::script_fetch`]).
    pub(crate) fn script_candidate(
        &mut self,
        node: NodeId,
        index: usize,
        failed: Option<CandidateError>,
    ) {
        let loader = self.script_loader(node);
        let urls = self.url_strings(node);
        let mut index = index;
        let mut failed = failed;

        loop {
            if let Some(f) = failed.take() {
                self.print_load_error(
                    &loader,
                    &f.candidate,
                    f.resolved.as_ref(),
                    &f.error,
                    index,
                    urls.len().saturating_sub(index),
                );
            }

            let Some(candidate) = urls.get(index).cloned() else {
                // `this .dataCallback (null)`: no URL could be loaded.
                if !urls.is_empty() {
                    self.set_load_state(node, nodes::LoadState::Failed);
                }

                return;
            };

            let fail = |error: String, resolved: Option<url::Url>| {
                Some(Box::new(CandidateError {
                    candidate: candidate.clone(),
                    resolved,
                    error,
                }))
            };

            if candidate.is_empty() {
                failed = fail("Error: Empty URL.".into(), None).map(|b| *b);
                index += 1;
                continue;
            }

            // Script URLs: /^\s*(?:ecmascript|javascript|vrmlscript):/s
            if let Some((_, source)) = crate::loader::strip_script_scheme(&candidate) {
                return self.script_accept(node, index, &candidate, source, None);
            }

            let Some(resolved) = url::Url::parse(&loader.base)
                .ok()
                .and_then(|b| crate::loader::join_url(&b, &candidate).ok())
            else {
                let error = "TypeError: Failed to construct 'URL': Invalid URL".to_string();
                self.tasks
                    .push_back(Task::ScriptCandidate(node, index + 1, 0, fail(error, None)));
                return;
            };

            if let Some(text) = loader::data_url_text(&candidate) {
                return self.script_accept(node, index, &candidate, &text, Some(resolved));
            }

            let fetched = matches!(resolved.scheme(), "http" | "https" | "file" | "blob")
                || (resolved.scheme() == "data" && candidate.contains(','));

            if !fetched {
                self.tasks.push_back(Task::ScriptCandidate(
                    node,
                    index + 1,
                    1,
                    fail("TypeError: Failed to fetch".into(), Some(resolved)),
                ));
                return;
            }

            self.fetches.push_back(Task::ScriptFetch(node, index));
            return;
        }
    }

    /// The response of Script candidate `index` is in, or there is nothing to wait for (an asynchronous [`crate::Fetcher`],
    /// see [`Runtime::fetch_task_ready`]).
    pub(crate) fn script_fetch_ready(&self, node: NodeId, index: usize) -> bool {
        if node as usize >= self.scene.nodes.len() || self.scene.free_nodes.contains(&node) {
            return true;
        }

        let loader = self.script_loader(node);
        let Some(resolved) = self.url_strings(node).get(index).and_then(|candidate| {
            url::Url::parse(&loader.base)
                .ok()
                .and_then(|b| crate::loader::join_url(&b, candidate).ok())
        }) else {
            return true;
        };

        !matches!(self.fetcher.poll(&resolved), crate::FetchPoll::Pending)
    }

    /// The fetch of Script candidate `index` completed (a macrotask): its source, or `checkResponse` threw (two ticks to the
    /// next candidate).
    pub(crate) fn script_fetch(&mut self, node: NodeId, index: usize) {
        let loader = self.script_loader(node);
        let Some(candidate) = self.url_strings(node).get(index).cloned() else {
            return;
        };
        let Some(resolved) = url::Url::parse(&loader.base)
            .ok()
            .and_then(|b| crate::loader::join_url(&b, &candidate).ok())
        else {
            return;
        };

        match self.fetcher.fetch(&resolved) {
            Ok(data) => {
                let source = x3d_parse::decode_text(&x3d_parse::gunzip(&data));
                self.script_accept(node, index, &candidate, &source, Some(resolved));
            }
            Err(error) => {
                let failed = CandidateError {
                    candidate,
                    resolved: Some(resolved),
                    error,
                };
                self.tasks.push_back(Task::ScriptCandidate(
                    node,
                    index + 1,
                    1,
                    Some(Box::new(failed)),
                ));
            }
        }
    }

    /// Script.loadData's callback with a candidate's source: `await this .initialize__ (...)`, then COMPLETE; a throw rejects
    /// three ticks before the next candidate starts.
    fn script_accept(
        &mut self,
        node: NodeId,
        index: usize,
        candidate: &str,
        source: &str,
        resolved: Option<url::Url>,
    ) {
        match self.script_initialize_source(node, source) {
            Ok(()) => self.set_load_state(node, nodes::LoadState::Complete),
            Err(error) => {
                let failed = CandidateError {
                    candidate: candidate.to_string(),
                    resolved,
                    error,
                };
                self.tasks.push_back(Task::ScriptCandidate(
                    node,
                    index + 1,
                    2,
                    Some(Box::new(failed)),
                ));
            }
        }
    }

    /// Script.initialize__ (sourceText).
    fn script_initialize_source(&mut self, node: NodeId, source: &str) -> Result<(), String> {
        let mut callbacks: Vec<String> =
            ["initialize", "prepareEvents", "eventsProcessed", "shutdown"]
                .iter()
                .map(|s| s.to_string())
                .collect();
        let user: Vec<(usize, String, u8)> = self
            .scene
            .node(node)
            .user_fields
            .iter()
            .enumerate()
            .map(|(i, f)| (i, f.name.clone(), f.access))
            .collect();

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
        let is_fn = |name: &str| {
            callbacks
                .iter()
                .position(|c| c == name)
                .is_some_and(|i| defined[i])
        };

        self.scripts.push(node);
        self.script_state(node).functions = callbacks
            .iter()
            .zip(&defined)
            .filter(|(_, d)| **d)
            .map(|(c, _)| c.clone())
            .collect();

        if is_fn("prepareEvents") {
            let k = self
                .private_field(self.browser_node, "prepareEvents")
                .unwrap();
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
        let has_shutdown = self
            .script_state(node)
            .functions
            .iter()
            .any(|f| f == "shutdown");

        // `this .shutdown = null`: a reentrant unload does not call it twice.
        self.script_state(node).functions.clear();

        if initialized && has_shutdown {
            self.script_call(node, ScriptCall::Plain("shutdown".into()), "shutdown");
        }

        if let Some(k) = self.private_field(self.browser_node, "prepareEvents") {
            self.remove_interest(k, node, Cb::ScriptPrepareEvents);
        }

        self.nrt_mut(node)
            .interests
            .retain(|i| !(i.node == node && i.cb == Cb::ScriptEventsProcessed));

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
        if self.script_state(node).closed
            && !matches!(&call, ScriptCall::Plain(n) if n == "shutdown")
        {
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
        let world = self
            .scene
            .world_url_of(self.scene.node(node).ctx)
            .to_string();
        let world = if world.starts_with("data:") {
            "data:".to_string()
        } else {
            world
        };

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
                let callback = if access_type == access::INPUT_OUTPUT {
                    format!("set_{name}")
                } else {
                    name.clone()
                };

                self.script_call(node, ScriptCall::Field(callback, value), &name);
            }
            Cb::ScriptEventsProcessed => self.script_call(
                node,
                ScriptCall::Plain("eventsProcessed".into()),
                "eventsProcessed",
            ),
            Cb::ScriptPrepareEvents => self.script_call(
                node,
                ScriptCall::Plain("prepareEvents".into()),
                "prepareEvents",
            ),
            _ => {}
        }
    }
}
