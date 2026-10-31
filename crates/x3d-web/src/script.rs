//! Script nodes in the page's own JavaScript engine (issue #133): [`PageJsHost`], the browser build's
//! [`ScriptHost`].
//!
//! The Script implementation is x3d-script's ([`x3d_script::sai`]), the same as the native viewer's on QuickJS. Here
//! its realm is an instance of the prelude in the page (`web/pkg/script.mjs`, made by tools/build-web.sh from the same
//! files), and its values are the page's ([`PageCx`]). As in X_ITE, a Script runs with the privileges of the page: the
//! prelude evaluates its source with `new Function` and `eval`, so the page needs `unsafe-eval` (see web/API.md). The
//! page decides whether a world's Scripts run: it gives the realm factory ([`crate::wasm::WebBrowser`]
//! `setScriptRealm`), and the factory may refuse a world.
//!
//! The prelude's native calls `__x (op, …)` come back through [`script_op`]. They reach the runtime through the host's
//! table ([`x3d_script::sai::Table`]), not through the `WebBrowser`, which is borrowed while a host call runs.
//!
//! Kept differences from the native host:
//!
//! * The page cannot run the promise jobs of a host call before the call returns: they run when the page's JavaScript
//!   stack is empty, after the call, and what they change in place reaches the runtime at the next host call.
//! * There is no deadline: a Script that does not return hangs the page, as in X_ITE (the native host stops it after
//!   10 s).
//! * Garbage collection cannot be forced: a node a Script dropped is released once the page's engine collected it.

use std::cell::RefCell;
use std::collections::HashMap;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use x3d_fields::NodeId;
use x3d_runtime::*;
use x3d_scene::*;
use x3d_script::sai::{self, Cx, J, Table, Val};

#[wasm_bindgen(inline_js = r#"
export function scriptToNumber (value) { return + value; }
export function scriptToText (value) { return `${value}`; }
export function scriptDescribe (error) { try { return String (error); } catch { return "Error"; } }
"#)]
extern "C" {
    /// ToNumber, which can throw (a Symbol, an object whose valueOf throws).
    #[wasm_bindgen(catch, js_name = scriptToNumber)]
    fn to_number(value: &JsValue) -> Result<f64, JsValue>;
    /// ToString, which can throw (a Symbol, an object whose toString throws).
    #[wasm_bindgen(catch, js_name = scriptToText)]
    fn to_text(value: &JsValue) -> Result<String, JsValue>;
    /// `String (error)`, or `Error` when that throws.
    #[wasm_bindgen(js_name = scriptDescribe)]
    fn describe(error: &JsValue) -> String;
}

thread_local! {
    /// The realm (the prelude's global object) of each page host, by its table id.
    static REALMS: RefCell<HashMap<u64, JsValue>> = RefCell::new(HashMap::new());
}

/// A value of the page.
#[derive(Clone)]
pub struct PageValue(JsValue);

impl Val for PageValue {
    fn is_null(&self) -> bool {
        self.0.is_null()
    }

    fn is_undefined(&self) -> bool {
        self.0.is_undefined()
    }

    fn is_object(&self) -> bool {
        self.0.is_object() || self.0.is_function()
    }

    fn is_string(&self) -> bool {
        self.0.is_string()
    }

    fn as_number(&self) -> Option<f64> {
        self.0.as_f64()
    }

    fn as_bool(&self) -> Option<bool> {
        self.0.as_bool()
    }

    fn as_text(&self) -> Option<String> {
        self.0.as_string()
    }

    fn elements(&self) -> Option<Vec<Option<Self>>> {
        if !js_sys::Array::is_array(&self.0) {
            return None;
        }

        let length = js_sys::Reflect::get(&self.0, &"length".into())
            .ok()
            .and_then(|l| l.as_f64())
            .unwrap_or(0.0) as u32;

        Some(
            (0..length)
                .map(|i| js_sys::Reflect::get_u32(&self.0, i).ok().map(PageValue))
                .collect(),
        )
    }

    fn get(&self, key: &str) -> Result<Self, ()> {
        if !self.is_object() {
            return Err(());
        }

        js_sys::Reflect::get(&self.0, &key.into())
            .map(PageValue)
            .map_err(|_| ())
    }

    fn has(&self, key: &str) -> Result<bool, ()> {
        if !self.is_object() {
            return Err(());
        }

        js_sys::Reflect::has(&self.0, &key.into()).map_err(|_| ())
    }

    fn to_number(&self) -> Result<f64, ()> {
        to_number(&self.0).map_err(|_| ())
    }

    fn to_text(&self) -> Result<String, ()> {
        to_text(&self.0).map_err(|_| ())
    }

    fn to_bool(&self) -> Result<bool, ()> {
        Ok(self.0.is_truthy())
    }

    /// The page's engine leaves no exception pending: a throw is the `Err` of the call that made it.
    fn catch(&self) {}
}

/// The page's engine, with the realm of one host.
pub struct PageCx<'a> {
    realm: &'a JsValue,
}

impl PageCx<'_> {
    fn global(&self, name: &str) -> Option<js_sys::Function> {
        if !self.realm.is_object() {
            return None;
        }

        js_sys::Reflect::get(self.realm, &name.into())
            .ok()?
            .dyn_into::<js_sys::Function>()
            .ok()
    }
}

impl Cx for PageCx<'_> {
    type V = PageValue;
    type E = JsValue;

    fn realize(&self, j: J<PageValue>) -> Result<PageValue, JsValue> {
        Ok(PageValue(match j {
            J::Undefined => JsValue::UNDEFINED,
            J::Null => JsValue::NULL,
            J::Bool(b) => JsValue::from_bool(b),
            J::Int(x) => JsValue::from_f64(x as f64),
            // QuickJS makes an integral number an int, so -0 is 0 there: the same here.
            J::Number(x) => JsValue::from_f64(if x == 0.0 { 0.0 } else { x }),
            J::Float(x) => JsValue::from_f64(x),
            J::Str(s) => JsValue::from_str(&s),
            J::Array(items) => {
                let a = js_sys::Array::new();

                for x in items {
                    a.push(&self.realize(x)?.0);
                }

                a.into()
            }
            J::Object(entries) => {
                let o = js_sys::Object::new();

                for (k, x) in entries {
                    js_sys::Reflect::set(&o, &k.into(), &self.realize(x)?.0)?;
                }

                o.into()
            }
            J::Value(v) => v.0,
        }))
    }

    fn throw(&self, msg: &str) -> JsValue {
        js_sys::Error::new(msg).into()
    }

    fn throw_type(&self, msg: &str) -> JsValue {
        js_sys::TypeError::new(msg).into()
    }

    fn has_global(&self, name: &str) -> bool {
        self.global(name).is_some()
    }

    fn call_global(
        &self,
        name: &str,
        args: Vec<J<PageValue>>,
    ) -> Option<Result<PageValue, JsValue>> {
        let f = self.global(name)?;
        let list = js_sys::Array::new();

        for a in args {
            match self.realize(a) {
                Ok(v) => {
                    list.push(&v.0);
                }
                Err(e) => return Some(Err(e)),
            }
        }

        Some(f.apply(&JsValue::UNDEFINED, &list).map(PageValue))
    }

    /// The page runs them after the host call (see the module documentation).
    fn run_jobs(&self) {}

    fn describe(&self, e: JsValue) -> String {
        describe(&e)
    }

    /// The page's Math functions are V8's already.
    fn math_fn(&self, _name: &str) -> Result<J<PageValue>, JsValue> {
        Ok(J::Null)
    }
}

/// The prelude's `__x (op, …)` of the realm of host `host` (`web/pkg/script.mjs` passes its arguments as an array).
#[wasm_bindgen(js_name = scriptOp)]
pub fn script_op(host: f64, args: js_sys::Array) -> Result<JsValue, JsValue> {
    let host = host as u64;

    if !sai::select_host(host) {
        return Err(js_sys::Error::new("X_ITE_R: the Script's world is gone.").into());
    }

    // While the prelude loads, its realm is not registered yet: the calls it makes then need none of its entry points.
    let realm = REALMS.with(|r| r.borrow().get(&host).cloned());
    let realm = realm.unwrap_or(JsValue::UNDEFINED);
    let cx = PageCx { realm: &realm };

    sai::native(&cx, args.iter().map(PageValue).collect()).map(|v| v.0)
}

/// The Script host of a world's runtime in the page.
pub struct PageJsHost {
    table: Table,
    realm: JsValue,
}

impl PageJsHost {
    /// A host for `rt` with a realm of `factory` (`factory (hostId)` returns the prelude's global object, or null when
    /// the page does not let this world's Scripts run). `rt` must not move until the host is told
    /// ([`ScriptHost::attach`]). The error is the reason the realm could not be made (a Content Security Policy without
    /// `unsafe-eval`, for example).
    pub fn new(rt: &mut Runtime, factory: &js_sys::Function) -> Result<Option<PageJsHost>, String> {
        let table = Table::new();

        // The prelude makes its native calls while it loads: they reach this runtime.
        table.enter(rt);

        let realm = match factory.call1(&JsValue::NULL, &JsValue::from_f64(table.id() as f64)) {
            Ok(realm) if realm.is_object() => realm,
            Ok(_) => {
                table.close(|| {});
                return Ok(None);
            }
            Err(e) => {
                table.close(|| {});
                return Err(describe(&e));
            }
        };

        REALMS.with(|r| r.borrow_mut().insert(table.id(), realm.clone()));

        Ok(Some(PageJsHost { table, realm }))
    }

    fn cx(&self) -> PageCx<'_> {
        PageCx { realm: &self.realm }
    }
}

impl Drop for PageJsHost {
    /// The table goes; the realm stays with the page objects that still refer to it, and its native calls throw.
    fn drop(&mut self) {
        let id = self.table.id();

        REALMS.with(|r| r.borrow_mut().remove(&id));
        self.table.close(|| {});
    }
}

impl ScriptHost for PageJsHost {
    fn create_context(
        &mut self,
        rt: &mut Runtime,
        node: NodeId,
        source: &str,
        callbacks: &[String],
    ) -> Result<Vec<bool>, ScriptError> {
        self.table.enter(rt);
        sai::create_context(&self.cx(), rt, node, source, callbacks)
    }

    fn call(
        &mut self,
        rt: &mut Runtime,
        node: NodeId,
        call: ScriptCall,
    ) -> Result<(), ScriptError> {
        self.table.enter(rt);
        sai::call(&self.cx(), rt, node, call)
    }

    fn settle(&mut self, rt: &mut Runtime, id: u64, outcome: Result<CtxId, String>) {
        self.table.enter(rt);
        sai::settle(&self.cx(), id, outcome.map(|c| c as f64));
    }

    fn settle_plain(&mut self, rt: &mut Runtime, id: u64, outcome: Result<(), String>) {
        self.table.enter(rt);
        sai::settle(&self.cx(), id, outcome.map(|()| 0.0));
    }

    fn next_timer(&mut self, rt: &mut Runtime, until_ms: f64) -> Option<f64> {
        self.table.enter(rt);
        sai::next_timer(&self.cx(), until_ms)
    }

    fn fire_timer(&mut self, rt: &mut Runtime) {
        self.table.enter(rt);
        sai::event(&self.cx(), "__fireTimer", vec![]);
    }

    fn browser_event(&mut self, rt: &mut Runtime, event: u32) {
        self.table.enter(rt);
        sai::event(&self.cx(), "__browserEvent", vec![J::Float(event as f64)]);
    }

    fn ctx_event(&mut self, rt: &mut Runtime, ctx: CtxId, field: u32) {
        self.table.enter(rt);
        sai::event(
            &self.cx(),
            "__ctxEvent",
            vec![J::Float(ctx as f64), J::Float(field as f64)],
        );
    }

    fn field_event(&mut self, rt: &mut Runtime, k: FKey) {
        self.table.enter(rt);
        sai::field_event(&self.cx(), k);
    }

    fn raw_coerce(
        &mut self,
        rt: &mut Runtime,
        requests: &[RawRequest],
    ) -> Vec<Option<Vec<Option<RawComponent>>>> {
        self.table.enter(rt);
        sai::raw_coerce(&self.cx(), requests)
    }

    fn destroy_context(&mut self, node: NodeId) {
        self.table.select();
        sai::destroy(&self.cx(), node);
        self.table.release_fresh();
    }

    /// The page cannot collect garbage now: the nodes whose objects the engine has not collected yet are held.
    fn held_nodes(&mut self, rt: &mut Runtime, candidates: &[NodeId]) -> Option<Vec<NodeId>> {
        self.table.enter(rt);
        sai::unpin_nodes(&self.cx(), candidates)?;
        self.table.release_fresh();
        sai::held_nodes(&self.cx(), candidates)
    }

    /// As [`PageJsHost::held_nodes`]: the scenes whose X3DScene objects the engine has not collected yet are held.
    fn held_scenes(&mut self, rt: &mut Runtime, candidates: &[CtxId]) -> Option<Vec<CtxId>> {
        self.table.enter(rt);
        sai::unpin_scenes(&self.cx(), candidates)?;
        sai::held_scenes(&self.cx(), candidates)
    }

    fn release_scenes(&mut self, rt: &mut Runtime, ctxs: &[CtxId]) {
        self.table.enter(rt);
        sai::release_scenes(&self.cx(), ctxs);
    }

    fn release_nodes(&mut self, rt: &mut Runtime, nodes: &[NodeId], tombstone: NodeId) {
        self.table.enter(rt);

        let detached = sai::release_slots(nodes);

        sai::release_nodes(&self.cx(), nodes, tombstone, detached);
        self.table.release_fresh();
    }

    fn attach(&mut self, rt: &mut Runtime) {
        self.table.enter(rt);
    }
}
