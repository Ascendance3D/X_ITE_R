//! The Script engine on QuickJS (rquickjs): the [`ScriptHost`] of the native viewer.
//!
//! The Script implementation itself is [`crate::sai`], which the browser build shares. This module gives it the values
//! of QuickJS ([`Cx`] for `Ctx`, [`Val`] for `Value`), the V8 math functions QuickJS lacks, and the native limits: a
//! 32 MB stack and a 10 s deadline for each host call.

use crate::sai::{self, Cx, J, Table, Val};
use rquickjs::convert::Coerced;
use rquickjs::function::Rest;
use rquickjs::{Array, Context, Ctx, Exception, Function, Object, Value};
use x3d_fields::{NodeId, jsmath};
use x3d_runtime::*;
use x3d_scene::*;

pub use crate::sai::{SlotStats, live_hosts, mf_get, sf_type_of, slot_stats};

pub struct QuickJsHost {
    /// This host's field handle table.
    table: Table,
    _runtime: std::mem::ManuallyDrop<rquickjs::Runtime>,
    context: std::mem::ManuallyDrop<Context>,
    deadline: std::rc::Rc<std::cell::Cell<Option<std::time::Instant>>>,
}

impl Default for QuickJsHost {
    fn default() -> Self {
        Self::new()
    }
}

impl QuickJsHost {
    pub fn new() -> Self {
        let runtime = rquickjs::Runtime::new().expect("QuickJS runtime");
        runtime.set_max_stack_size(32 << 20);

        let deadline = std::rc::Rc::new(std::cell::Cell::new(None::<std::time::Instant>));
        let d = deadline.clone();

        // A script that never returns makes the oracle time out; stop it instead of hanging.
        runtime.set_interrupt_handler(Some(Box::new(move || {
            d.get().is_some_and(|t| std::time::Instant::now() > t)
        })));

        let context = Context::full(&runtime).expect("QuickJS context");
        let table = Table::new();

        context.with(|ctx| {
            let f = Function::new(ctx.clone(), native).expect("native __x");
            ctx.globals().set("__x", f).unwrap();

            if let Err(e) = ctx.eval::<(), _>(sai::PRELUDE) {
                let msg = describe(&ctx, e);
                panic!("prelude.js failed: {msg}");
            }
        });

        QuickJsHost {
            table,
            _runtime: std::mem::ManuallyDrop::new(runtime),
            context: std::mem::ManuallyDrop::new(context),
            deadline,
        }
    }

    fn enter(&self, rt: &mut Runtime) {
        self.table.enter(rt);
        self.deadline.set(Some(
            std::time::Instant::now() + std::time::Duration::from_secs(10),
        ));
    }

    /// Collect the objects that a shut down or released Script left (they are in cycles, which
    /// reference counting alone does not free), and run the finalizers that release their slots.
    fn collect(&mut self) {
        self._runtime.run_gc();

        self.context.with(|ctx| ctx.run_jobs());
        self.table.release_fresh();
    }
}

impl Drop for QuickJsHost {
    /// The JavaScript objects are freed while this host's table is still the current one (their finalizers
    /// release handles); then the table goes.
    fn drop(&mut self) {
        self.table.close(|| {
            // SAFETY: dropped once, context before runtime, never used again.
            unsafe {
                std::mem::ManuallyDrop::drop(&mut self.context);
                std::mem::ManuallyDrop::drop(&mut self._runtime);
            }
        });
    }
}

/// The prelude's `__x (op, …)`.
fn native<'js>(ctx: Ctx<'js>, args: Rest<Value<'js>>) -> rquickjs::Result<Value<'js>> {
    sai::native(&ctx, args.0)
}

/// `String (error)` of the pending exception.
fn describe(ctx: &Ctx<'_>, e: rquickjs::Error) -> String {
    if let rquickjs::Error::Exception = e {
        let v = ctx.catch();
        let s: Result<Coerced<String>, _> = v.get();
        return s.map(|c| c.0).unwrap_or_else(|_| "Error".into());
    }

    format!("Error: {e}")
}

impl<'js> Val for Value<'js> {
    fn is_null(&self) -> bool {
        Value::is_null(self)
    }

    fn is_undefined(&self) -> bool {
        Value::is_undefined(self)
    }

    fn is_object(&self) -> bool {
        Value::is_object(self)
    }

    fn is_string(&self) -> bool {
        Value::is_string(self)
    }

    fn as_number(&self) -> Option<f64> {
        Value::as_number(self)
    }

    fn as_bool(&self) -> Option<bool> {
        Value::as_bool(self)
    }

    fn as_text(&self) -> Option<String> {
        self.as_string().and_then(|s| s.to_string().ok())
    }

    fn elements(&self) -> Option<Vec<Option<Self>>> {
        let a = self.as_array()?;

        Some((0..a.len()).map(|i| a.get::<Value>(i).ok()).collect())
    }

    fn get(&self, key: &str) -> Result<Self, ()> {
        let o = self.as_object().ok_or(())?;

        o.get::<_, Value>(key).map_err(|_| ())
    }

    fn has(&self, key: &str) -> Result<bool, ()> {
        let o = self.as_object().ok_or(())?;

        o.contains_key(key).map_err(|_| ())
    }

    fn to_number(&self) -> Result<f64, ()> {
        Value::get::<Coerced<f64>>(self)
            .map(|c| c.0)
            .map_err(|_| ())
    }

    fn to_text(&self) -> Result<String, ()> {
        Value::get::<Coerced<String>>(self)
            .map(|c| c.0)
            .map_err(|_| ())
    }

    fn to_bool(&self) -> Result<bool, ()> {
        Value::get::<Coerced<bool>>(self)
            .map(|c| c.0)
            .map_err(|_| ())
    }

    fn catch(&self) {
        let _ = self.ctx().catch();
    }
}

impl<'js> Cx for Ctx<'js> {
    type V = Value<'js>;
    type E = rquickjs::Error;

    fn realize(&self, j: J<Value<'js>>) -> rquickjs::Result<Value<'js>> {
        Ok(match j {
            J::Undefined => Value::new_undefined(self.clone()),
            J::Null => Value::new_null(self.clone()),
            J::Bool(b) => Value::new_bool(self.clone(), b),
            J::Int(x) => Value::new_int(self.clone(), x),
            J::Number(x) => Value::new_number(self.clone(), x),
            J::Float(x) => Value::new_float(self.clone(), x),
            J::Str(s) => rquickjs::String::from_str(self.clone(), &s)?.into_value(),
            J::Array(items) => {
                let a = Array::new(self.clone())?;

                for (i, x) in items.into_iter().enumerate() {
                    a.set(i, self.realize(x)?)?;
                }

                a.into_value()
            }
            J::Object(entries) => {
                let o = Object::new(self.clone())?;

                for (k, x) in entries {
                    o.set(k, self.realize(x)?)?;
                }

                o.into_value()
            }
            J::Value(v) => v,
        })
    }

    fn throw(&self, msg: &str) -> rquickjs::Error {
        Exception::throw_message(self, msg)
    }

    fn throw_type(&self, msg: &str) -> rquickjs::Error {
        Exception::throw_type(self, msg)
    }

    fn has_global(&self, name: &str) -> bool {
        self.globals().get::<_, Function>(name).is_ok()
    }

    fn call_global(
        &self,
        name: &str,
        args: Vec<J<Value<'js>>>,
    ) -> Option<rquickjs::Result<Value<'js>>> {
        let f: Function = self.globals().get(name).ok()?;
        let args = match args
            .into_iter()
            .map(|a| self.realize(a))
            .collect::<rquickjs::Result<Vec<Value>>>()
        {
            Ok(args) => args,
            Err(e) => return Some(Err(e)),
        };

        Some(f.call((Rest(args),)))
    }

    fn run_jobs(&self) {
        while self.execute_pending_job() {}
    }

    fn describe(&self, e: rquickjs::Error) -> String {
        describe(self, e)
    }

    fn math_fn(&self, name: &str) -> rquickjs::Result<J<Value<'js>>> {
        math_fn(self, name).map(J::Value)
    }
}

/// V8's (fdlibm) math functions, which QuickJS (the C library's) does not give bit for bit.
fn math_fn<'js>(ctx: &Ctx<'js>, name: &str) -> rquickjs::Result<Value<'js>> {
    macro_rules! f1 {
        ($f:path) => {
            Function::new(ctx.clone(), |x: Coerced<f64>| $f(x.0))?.into_value()
        };
    }

    Ok(match name {
        "sin" => f1!(jsmath::sin),
        "cos" => f1!(jsmath::cos),
        "tan" => f1!(jsmath::tan),
        "exp" => f1!(jsmath::exp),
        "log" => f1!(jsmath::log),
        "acos" => f1!(jsmath::acos),
        "asin" => f1!(jsmath::asin),
        "log10" => f1!(jsmath::log10),
        "atan" => f1!(jsmath::atan),
        "sqrt" => f1!(jsmath::sqrt),
        "pow" => Function::new(ctx.clone(), |x: Coerced<f64>, y: Coerced<f64>| {
            jsmath::pow(x.0, y.0)
        })?
        .into_value(),
        // V8 MathHypot: every argument is converted with ToNumber first.
        "hypot" => Function::new(ctx.clone(), |a: Rest<Coerced<f64>>| {
            jsmath::hypot(&a.0.iter().map(|x| x.0).collect::<Vec<_>>())
        })?
        .into_value(),
        "atan2" => Function::new(ctx.clone(), |y: Coerced<f64>, x: Coerced<f64>| {
            jsmath::atan2(y.0, x.0)
        })?
        .into_value(),
        _ => Value::new_null(ctx.clone()),
    })
}

impl ScriptHost for QuickJsHost {
    fn create_context(
        &mut self,
        rt: &mut Runtime,
        node: NodeId,
        source: &str,
        callbacks: &[String],
    ) -> Result<Vec<bool>, ScriptError> {
        self.enter(rt);
        self.context
            .with(|ctx| sai::create_context(&ctx, rt, node, source, callbacks))
    }

    fn call(
        &mut self,
        rt: &mut Runtime,
        node: NodeId,
        call: ScriptCall,
    ) -> Result<(), ScriptError> {
        self.enter(rt);
        self.context.with(|ctx| sai::call(&ctx, rt, node, call))
    }

    fn settle(&mut self, rt: &mut Runtime, id: u64, outcome: Result<CtxId, String>) {
        self.enter(rt);
        self.context
            .with(|ctx| sai::settle(&ctx, id, outcome.map(|c| c as f64)));
    }

    fn settle_plain(&mut self, rt: &mut Runtime, id: u64, outcome: Result<(), String>) {
        self.enter(rt);
        self.context
            .with(|ctx| sai::settle(&ctx, id, outcome.map(|()| 0.0)));
    }

    fn next_timer(&mut self, rt: &mut Runtime, until_ms: f64) -> Option<f64> {
        self.enter(rt);
        self.context.with(|ctx| sai::next_timer(&ctx, until_ms))
    }

    fn fire_timer(&mut self, rt: &mut Runtime) {
        self.enter(rt);
        self.context
            .with(|ctx| sai::event(&ctx, "__fireTimer", vec![]));
    }

    fn browser_event(&mut self, rt: &mut Runtime, event: u32) {
        self.enter(rt);
        self.context
            .with(|ctx| sai::event(&ctx, "__browserEvent", vec![J::Float(event as f64)]));
    }

    fn ctx_event(&mut self, rt: &mut Runtime, ctx: CtxId, field: u32) {
        self.enter(rt);
        self.context.with(|js| {
            sai::event(
                &js,
                "__ctxEvent",
                vec![J::Float(ctx as f64), J::Float(field as f64)],
            )
        });
    }

    fn field_event(&mut self, rt: &mut Runtime, k: FKey) {
        self.enter(rt);
        self.context.with(|js| sai::field_event(&js, k));
    }

    fn raw_coerce(
        &mut self,
        rt: &mut Runtime,
        requests: &[RawRequest],
    ) -> Vec<Option<Vec<Option<RawComponent>>>> {
        self.enter(rt);
        self.context.with(|ctx| sai::raw_coerce(&ctx, requests))
    }

    fn destroy_context(&mut self, node: NodeId) {
        self.table.select();
        self.context.with(|ctx| sai::destroy(&ctx, node));
        self.collect();
    }

    fn held_nodes(&mut self, rt: &mut Runtime, candidates: &[NodeId]) -> Option<Vec<NodeId>> {
        self.enter(rt);

        // prelude/rest.js: the candidates' objects are unpinned, garbage is collected, and the objects left are held.
        self.context
            .with(|ctx| sai::unpin_nodes(&ctx, candidates))?;
        self.collect();
        self.context.with(|ctx| sai::held_nodes(&ctx, candidates))
    }

    fn held_scenes(
        &mut self,
        rt: &mut Runtime,
        candidates: &[x3d_scene::CtxId],
    ) -> Option<Vec<x3d_scene::CtxId>> {
        self.enter(rt);

        // prelude/rest.js: the scenes' X3DScene objects are unpinned, garbage is collected, and the objects left are held.
        self.context
            .with(|ctx| sai::unpin_scenes(&ctx, candidates))?;
        self.collect();
        self.context.with(|ctx| sai::held_scenes(&ctx, candidates))
    }

    fn release_scenes(&mut self, rt: &mut Runtime, ctxs: &[x3d_scene::CtxId]) {
        self.enter(rt);
        self.context.with(|ctx| sai::release_scenes(&ctx, ctxs));
    }

    fn release_nodes(&mut self, rt: &mut Runtime, nodes: &[NodeId], tombstone: NodeId) {
        self.enter(rt);

        let detached = sai::release_slots(nodes);

        self.context
            .with(|ctx| sai::release_nodes(&ctx, nodes, tombstone, detached));
        self.collect();
    }
}
