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

        if self.script_state(node).load_state != nodes::LoadState::NotStarted {
            return;
        }

        if !self.get_bool(node, "load") || self.url_strings(node).is_empty() {
            return;
        }

        self.script_state(node).load_state = nodes::LoadState::InProgress;

        // loadData: `await this .unloadData ()` — continues as a microtask.
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

        self.script_state(node).load_state = if ok { nodes::LoadState::Complete } else { nodes::LoadState::Failed };
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

    /// The Script of a scene that was disposed (an Inline whose scene was replaced): it stops
    /// getting `prepareEvents`, `eventsProcessed` and field events. X_ITE keeps them all.
    pub(crate) fn script_close(&mut self, node: NodeId) {
        if self.script_state(node).closed {
            return;
        }

        self.script_state(node).closed = true;

        if let Some(k) = self.private_field(self.browser_node, "prepareEvents") {
            self.remove_interest(k, node, Cb::ScriptPrepareEvents);
        }

        self.nrt_mut(node).interests.retain(|i| !(i.node == node && i.cb == Cb::ScriptEventsProcessed));

        for i in 0..self.scene.node(node).user_fields.len() {
            self.remove_interest((node, FieldLoc::User(i)), node, Cb::ScriptField(i));
        }
    }

    /// call__ / set_field__ with setError on a thrown value.
    fn script_call(&mut self, node: NodeId, call: ScriptCall, reason_name: &str) {
        let Some(mut host) = self.script_host.take() else {
            return;
        };

        let result = host.call(self, node, call);
        self.script_host = Some(host);

        if let Err(e) = result {
            let name = self.scene.node(node).name.clone();
            let world = self.scene.world_url_of(self.scene.node(node).ctx).to_string();
            let world = if world.starts_with("data:") { "data:".to_string() } else { world };

            self.log_error(format!("JavaScript Error in Script '{name}', in function '{reason_name}'\nworld url is '{world}':"));
            self.log_error(&e.0);
        }
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
