//! The ECMAScript side of the Script node on QuickJS (rquickjs).
//!
//! X_ITE runs every Script of a world in one JavaScript realm (the page), evaluating each
//! source with `with (globalObject) eval (source)` (Browser/Scripting/evaluate.js). This host
//! does the same: one QuickJS context per world, the same evaluate function (prelude.js).
//!
//! Field objects are handles into Rust: a node field (live: writes add events), a detached
//! value (`new SFVec3f (…)`, callback argument copies), or an element view of an MF field
//! (X3DTypedArrayField returns references to elements). All value semantics — conversions,
//! vector/rotation/matrix math, VRML printing — run in Rust on `FieldValue` with the
//! x3d-fields ports of X_ITE's math, so they match X_ITE bit for bit where those do.

use rquickjs::convert::Coerced;
use rquickjs::function::Rest;
use rquickjs::{Array, Context, Ctx, Exception, Function, Object, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use x3d_fields::math::*;
use x3d_fields::{FieldType, FieldValue, Generator, Image, NodeId, access, jsmath};
use x3d_runtime::ctx_events::CtxField;
use x3d_runtime::*;
use x3d_scene::*;

mod ctx_api;

/// prelude/head.js lists the parts; math.js, fields.js, config.js and node_types.js are generated (tools/gen_prelude_xite.py).
const PRELUDE: &str = concat!(
    include_str!("prelude/head.js"),
    include_str!("prelude/math.js"),
    include_str!("prelude/fields.js"),
    include_str!("prelude/config.js"),
    include_str!("prelude/node_types.js"),
    include_str!("prelude/bridge.js"),
    include_str!("prelude/rest.js"),
);
const NOFIELD: &str = "\u{0}nofield";

#[derive(Clone, Debug)]
enum Slot {
    Field(FKey),
    Value(FieldValue),
    Element(u32, usize),
    /// Released: on the free list, reached by no JavaScript object (R0.3-E).
    Free,
}

/// Field handles (R0.3-E, `JS_SLOT_TABLE_GROWTH`).
///
/// A handle is a slot index. JavaScript sees it with the slot's generation in the high bits
/// (`generation * 2^32 + index`, exact in a double), so a number kept from before a slot was released
/// never reads or writes the slot's next value: [`decode`] rejects it.
///
/// A slot lives while something can reach it:
///
/// * `owners`: JavaScript objects that hold the handle (every field object, and the holder each
///   Script field accessor closes over). prelude.js counts them in (`own`) and a `FinalizationRegistry`
///   counts them out (`disown`) when QuickJS frees the object.
/// * `children`: element views (`Slot::Element`) of an MF slot.
/// * `attached`: the field slot of a node field, while the node lives.
///
/// A slot that none of these holds is released: when its last owner goes, when its node is released,
/// or at the end of the host call that made it (a value JavaScript never wrapped).
struct State {
    rt: *mut Runtime,
    slots: Vec<Slot>,
    generations: Vec<u32>,
    owners: Vec<u32>,
    children: Vec<u32>,
    free: Vec<u32>,
    /// Slots made during the current host call that no owner took yet.
    fresh: Vec<u32>,
    attached: HashMap<FKey, u32>,
    description: String,
}

/// What the field handle table holds (for tests and diagnostics).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlotStats {
    /// Length and capacity of the slot table.
    pub len: usize,
    pub capacity: usize,
    /// Released slots waiting to be handed out again.
    pub free: usize,
    /// Node fields that have a field slot.
    pub attached: usize,
}

/// The field handle table of the script host that ran last on this thread.
pub fn slot_stats() -> SlotStats {
    STATES.with(|s| match s.borrow().get(&CURRENT.get()) {
        Some(s) => stats_of(s),
        None => LAST_DROPPED.get(),
    })
}

/// Script hosts alive on this thread (each with its own field handle table).
pub fn live_hosts() -> usize {
    STATES.with(|s| s.borrow().len())
}

fn stats_of(s: &State) -> SlotStats {
    SlotStats { len: s.slots.len(), capacity: s.slots.capacity(), free: s.free.len(), attached: s.attached.len() }
}

/// A handle as JavaScript sees it.
fn encode(h: u32) -> f64 {
    let generation = with_state(|s| s.generations[h as usize]);
    generation as f64 * 4294967296.0 + h as f64
}

/// The slot of a handle from JavaScript, if the slot was not released since.
fn decode(x: f64) -> Option<u32> {
    if !(x >= 0.0) || x.fract() != 0.0 || x >= 9007199254740992.0 {
        return None;
    }

    let (generation, h) = ((x / 4294967296.0).floor(), (x % 4294967296.0) as u32);

    with_state(|s| {
        let live = (h as usize) < s.slots.len() && !matches!(s.slots[h as usize], Slot::Free) && s.generations[h as usize] as f64 == generation;
        live.then_some(h)
    })
}

impl State {
    fn alloc(&mut self, slot: Slot) -> u32 {
        if let Slot::Element(parent, _) = slot {
            self.children[parent as usize] += 1;
        }

        let h = match self.free.pop() {
            Some(h) => {
                self.slots[h as usize] = slot;
                h
            }
            None => {
                self.slots.push(slot);
                self.generations.push(0);
                self.owners.push(0);
                self.children.push(0);
                (self.slots.len() - 1) as u32
            }
        };

        self.fresh.push(h);
        h
    }

    /// Release the slot if nothing holds it any more (see [`State`]).
    fn try_free(&mut self, h: u32) {
        let i = h as usize;

        if matches!(self.slots[i], Slot::Free) || self.owners[i] > 0 || self.children[i] > 0 {
            return;
        }

        if let Slot::Field(k) = self.slots[i] {
            if self.attached.get(&k) == Some(&h) {
                return;
            }
        }

        let parent = match self.slots[i] {
            Slot::Element(p, _) => Some(p),
            _ => None,
        };

        self.slots[i] = Slot::Free;
        self.generations[i] = self.generations[i].wrapping_add(1);
        self.free.push(h);

        if let Some(p) = parent {
            self.children[p as usize] -= 1;
            self.try_free(p);
        }
    }

    /// The end of a host call: what JavaScript did not take is released.
    fn release_fresh(&mut self) {
        for h in std::mem::take(&mut self.fresh) {
            self.try_free(h);
        }
    }
}

// Several hosts share a thread (several EmbeddedViewers on one page, REA batch 08 `multiview`). Each host
// owns a [`State`] under its id; native calls from JavaScript find theirs through `CURRENT`, which every
// host entry point sets (`QuickJsHost::select`) before it runs JavaScript.
thread_local! {
    static STATES: RefCell<HashMap<u64, State>> = RefCell::new(HashMap::new());
    static CURRENT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static NEXT_HOST: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
    /// The table size of the last host that was dropped while it was the current one: what `slot_stats`
    /// reports when no host is current.
    static LAST_DROPPED: std::cell::Cell<SlotStats> = const { std::cell::Cell::new(SlotStats { len: 0, capacity: 0, free: 0, attached: 0 }) };
}

fn with_state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    STATES.with(|s| f(s.borrow_mut().get_mut(&CURRENT.get()).expect("script engine state")))
}

/// The runtime of the world whose script is running.
fn rt<'a>() -> &'a mut Runtime {
    let p = with_state(|s| s.rt);
    // SAFETY: set by QuickJsHost for the duration of a host call; the runtime outlives it.
    unsafe { &mut *p }
}

pub struct QuickJsHost {
    /// This host's key in `STATES`.
    id: u64,
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
        runtime.set_interrupt_handler(Some(Box::new(move || d.get().is_some_and(|t| std::time::Instant::now() > t))));

        let context = Context::full(&runtime).expect("QuickJS context");

        let id = NEXT_HOST.get();
        NEXT_HOST.set(id + 1);

        STATES.with(|s| {
            s.borrow_mut().insert(id, State {
                rt: std::ptr::null_mut(),
                slots: vec![],
                generations: vec![],
                owners: vec![],
                children: vec![],
                free: vec![],
                fresh: vec![],
                attached: HashMap::new(),
                description: String::new(),
            });
        });
        CURRENT.set(id);

        context.with(|ctx| {
            let f = Function::new(ctx.clone(), native).expect("native __x");
            ctx.globals().set("__x", f).unwrap();

            if let Err(e) = ctx.eval::<(), _>(PRELUDE) {
                let msg = describe(&ctx, e);
                panic!("prelude.js failed: {msg}");
            }
        });

        QuickJsHost { id, _runtime: std::mem::ManuallyDrop::new(runtime), context: std::mem::ManuallyDrop::new(context), deadline }
    }

    /// Make this host's table the one native calls from JavaScript use.
    fn select(&self) {
        CURRENT.set(self.id);
    }

    fn enter(&self, rt: &mut Runtime) {
        self.select();
        with_state(|s| s.rt = rt as *mut Runtime);
        self.deadline.set(Some(std::time::Instant::now() + std::time::Duration::from_secs(10)));
    }
}

impl Drop for QuickJsHost {
    /// The JavaScript objects are freed while this host's table is still the current one (their finalizers
    /// release handles); then the table goes. If it was the current one, `slot_stats` keeps its last size.
    fn drop(&mut self) {
        let before = CURRENT.replace(self.id);

        // SAFETY: dropped once, context before runtime, never used again.
        unsafe {
            std::mem::ManuallyDrop::drop(&mut self.context);
            std::mem::ManuallyDrop::drop(&mut self._runtime);
        }

        STATES.with(|s| {
            let table = s.borrow_mut().remove(&self.id);

            if before == self.id {
                LAST_DROPPED.set(table.as_ref().map(stats_of).unwrap_or_default());
                CURRENT.set(0);
            } else {
                CURRENT.set(before);
            }
        });
    }
}

/// prelude/bridge.js: the node fields a Script holds as objects are refreshed before its code runs (`__enter`), and what it
/// changed in place goes to the runtime after it and its promise jobs ran (`__leave`).
fn js_enter(ctx: &Ctx<'_>) {
    if let Ok(f) = ctx.globals().get::<_, Function>("__enter") {
        let _ = f.call::<_, ()>(());
    }
}

fn js_leave(ctx: &Ctx<'_>) {
    if let Ok(f) = ctx.globals().get::<_, Function>("__leave") {
        let _ = f.call::<_, ()>(());
    }
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

impl ScriptHost for QuickJsHost {
    fn create_context(&mut self, rt: &mut Runtime, node: NodeId, source: &str, callbacks: &[String]) -> Result<Vec<bool>, ScriptError> {
        self.enter(rt);

        let fields: Vec<(String, u8, f64, &'static str)> = (0..rt.scene.node(node).user_fields.len())
            .map(|i| {
                let f = &rt.scene.node(node).user_fields[i];
                (f.name.clone(), f.access, encode(attach((node, FieldLoc::User(i)))), f.ty().name())
            })
            .collect();

        self.context.with(|ctx| {
            js_enter(&ctx);

            let create: Function = ctx.globals().get("__create").unwrap();
            let list = Array::new(ctx.clone()).unwrap();

            for (i, (name, a, h, ty)) in fields.iter().enumerate() {
                let entry = Array::new(ctx.clone()).unwrap();
                entry.set(0, name.as_str()).unwrap();
                entry.set(1, *a as i32).unwrap();
                entry.set(2, *h).unwrap();
                entry.set(3, *ty).unwrap();
                list.set(i, entry).unwrap();
            }

            let cbs: Vec<&str> = callbacks.iter().map(String::as_str).collect();

            let result = match create.call::<_, Vec<bool>>((node, source, cbs, list)) {
                Ok(v) => Ok(v),
                Err(e) => Err(ScriptError(describe(&ctx, e))),
            };

            while ctx.execute_pending_job() {}
            js_leave(&ctx);
            with_state(State::release_fresh);
            result
        })
    }

    fn call(&mut self, rt: &mut Runtime, node: NodeId, call: ScriptCall) -> Result<(), ScriptError> {
        self.enter(rt);

        let time = rt.now;

        self.context.with(|ctx| {
            js_enter(&ctx);

            let f: Function = ctx.globals().get("__call").unwrap();

            let result: rquickjs::Result<Value> = match call {
                ScriptCall::Plain(name) => {
                    let reason = name.clone();
                    f.call((node, name, Value::new_undefined(ctx.clone()), time, false, reason))
                }
                ScriptCall::Field(name, value) => {
                    // The header of an error names the field, not `set_<field>`.
                    let reason = match name.strip_prefix("set_") {
                        Some(field) if rt.scene.node(node).user_fields.iter().any(|f| f.name == field && f.access == access::INPUT_OUTPUT) => field.to_string(),
                        _ => name.clone(),
                    };
                    let h = detached(value);
                    let v = field_value_js(&ctx, h);
                    f.call((node, name, v, time, true, reason))
                }
            };

            // Run pending promise jobs (async callbacks continue as microtasks).
            while ctx.execute_pending_job() {}
            js_leave(&ctx);

            with_state(State::release_fresh);

            match result {
                Ok(v) if v.is_undefined() => Ok(()),
                Ok(v) => Err(ScriptError(v.get::<Coerced<String>>().map(|c| c.0).unwrap_or_default())),
                Err(e) => Err(ScriptError(describe(&ctx, e))),
            }
        })
    }

    fn settle(&mut self, rt: &mut Runtime, id: u64, outcome: Result<x3d_scene::CtxId, String>) {
        self.enter(rt);

        self.context.with(|ctx| {
            js_enter(&ctx);

            if let Ok(f) = ctx.globals().get::<_, Function>("__settle") {
                let _ = match outcome {
                    Ok(c) => f.call::<_, ()>((id as f64, true, c as f64)),
                    Err(m) => f.call::<_, ()>((id as f64, false, m)),
                };
            }

            while ctx.execute_pending_job() {}
            js_leave(&ctx);

            if let Ok(f) = ctx.globals().get::<_, Function>("__endSettle") {
                let _ = f.call::<_, ()>(());
            }

            with_state(State::release_fresh);
        });
    }

    fn settle_plain(&mut self, rt: &mut Runtime, id: u64, outcome: Result<(), String>) {
        self.enter(rt);

        self.context.with(|ctx| {
            js_enter(&ctx);

            if let Ok(f) = ctx.globals().get::<_, Function>("__settle") {
                let _ = match outcome {
                    Ok(()) => f.call::<_, ()>((id as f64, true, 0.0)),
                    Err(m) => f.call::<_, ()>((id as f64, false, m)),
                };
            }

            while ctx.execute_pending_job() {}
            js_leave(&ctx);

            if let Ok(f) = ctx.globals().get::<_, Function>("__endSettle") {
                let _ = f.call::<_, ()>(());
            }

            with_state(State::release_fresh);
        });
    }

    fn next_timer(&mut self, rt: &mut Runtime, until_ms: f64) -> Option<f64> {
        self.enter(rt);

        self.context.with(|ctx| {
            let f: Function = ctx.globals().get("__nextTimer").ok()?;

            f.call::<_, Option<f64>>((until_ms,)).ok().flatten()
        })
    }

    fn fire_timer(&mut self, rt: &mut Runtime) {
        self.enter(rt);

        self.context.with(|ctx| {
            js_enter(&ctx);

            if let Ok(f) = ctx.globals().get::<_, Function>("__fireTimer") {
                let _ = f.call::<_, ()>(());
            }

            while ctx.execute_pending_job() {}
            js_leave(&ctx);
            with_state(State::release_fresh);
        });
    }

    fn browser_event(&mut self, rt: &mut Runtime, event: u32) {
        self.enter(rt);

        self.context.with(|ctx| {
            js_enter(&ctx);

            if let Ok(f) = ctx.globals().get::<_, Function>("__browserEvent") {
                let _ = f.call::<_, ()>((event as f64,));
            }

            while ctx.execute_pending_job() {}
            js_leave(&ctx);
            with_state(State::release_fresh);
        });
    }

    fn ctx_event(&mut self, rt: &mut Runtime, ctx: CtxId, field: u32) {
        self.enter(rt);

        self.context.with(|js| {
            js_enter(&js);

            if let Ok(f) = js.globals().get::<_, Function>("__ctxEvent") {
                let _ = f.call::<_, ()>((ctx as f64, field as f64));
            }

            while js.execute_pending_job() {}
            js_leave(&js);
            with_state(State::release_fresh);
        });
    }

    fn field_event(&mut self, rt: &mut Runtime, k: FKey) {
        self.enter(rt);

        // X3DField.processEvent calls the callbacks synchronously, inside the event cascade: the promises they make run
        // with the jobs of the next host call, as in X_ITE they run after processEvents.
        self.context.with(|js| {
            if let Ok(f) = js.globals().get::<_, Function>("__fieldEvent") {
                let value = field_value_js(&js, attach(k));
                let _ = f.call::<_, ()>((fkey_id(k), value));
            }

            with_state(State::release_fresh);
        });
    }

    fn destroy_context(&mut self, node: NodeId) {
        self.select();
        self.destroy(node);
        self.collect();
    }

    fn held_nodes(&mut self, rt: &mut Runtime, candidates: &[NodeId]) -> Option<Vec<NodeId>> {
        self.enter(rt);

        // prelude/rest.js: the candidates' objects are unpinned, garbage is collected, and the objects left are held.
        let ids: Vec<f64> = candidates.iter().map(|n| *n as f64).collect();

        self.context.with(|ctx| ctx.globals().get::<_, Function>("__unpin").and_then(|f| f.call::<_, ()>((ids.clone(),)))).ok()?;
        self.collect();

        let held = self.context.with(|ctx| match ctx.globals().get::<_, Function>("__heldNodes") {
            Ok(f) => f.call::<_, Vec<f64>>((ids,)).ok().map(|ids| ids.into_iter().map(|i| i as NodeId).collect()),
            Err(_) => None,
        })?;

        self.context.with(|ctx| while ctx.execute_pending_job() {});
        Some(held)
    }

    fn release_nodes(&mut self, rt: &mut Runtime, nodes: &[NodeId], tombstone: NodeId) {
        self.enter(rt);

        // A field handle of a released node keeps the value it had; it no longer reaches the field.
        let dead: std::collections::HashSet<NodeId> = nodes.iter().copied().collect();

        let detached: Vec<f64> = with_state(|s| {
            let stale: Vec<(FKey, u32)> = s.attached.iter().filter(|(k, _)| dead.contains(&k.0)).map(|(k, h)| (*k, *h)).collect();
            let mut detached = vec![];

            for (k, h) in stale {
                s.attached.remove(&k);

                // SAFETY: `enter` made `rt` the runtime of this host for the duration of the call.
                let value = unsafe { (*s.rt).value(k).clone() };

                s.slots[h as usize] = Slot::Value(value);
                detached.push(s.generations[h as usize] as f64 * 4294967296.0 + h as f64);
                s.try_free(h);
            }

            detached
        });

        self.context.with(|ctx| {
            if let Ok(f) = ctx.globals().get::<_, Function>("__release") {
                let _ = f.call::<_, ()>((nodes.to_vec(), tombstone, detached));
            }
        });

        self.collect();
    }
}

impl QuickJsHost {
    /// Collect the objects that a shut down or released Script left (they are in cycles, which
    /// reference counting alone does not free), and run the finalizers that release their slots.
    fn collect(&mut self) {
        self._runtime.run_gc();

        self.context.with(|ctx| while ctx.execute_pending_job() {});
        with_state(State::release_fresh);
    }

    fn destroy(&mut self, node: NodeId) {
        self.context.with(|ctx| {
            if let Ok(f) = ctx.globals().get::<_, Function>("__destroy") {
                let _ = f.call::<_, ()>((node,));
            }
        });
    }
}

// --- slots ---------------------------------------------------------------------------------

fn attach(k: FKey) -> u32 {
    with_state(|s| {
        if let Some(h) = s.attached.get(&k) {
            return *h;
        }

        let h = s.alloc(Slot::Field(k));
        s.attached.insert(k, h);
        h
    })
}

fn detached(v: FieldValue) -> u32 {
    with_state(|s| s.alloc(Slot::Value(v)))
}

fn element(parent: u32, i: usize) -> u32 {
    with_state(|s| s.alloc(Slot::Element(parent, i)))
}

fn slot(h: u32) -> Slot {
    with_state(|s| s.slots[h as usize].clone())
}

fn read(h: u32) -> FieldValue {
    match slot(h) {
        Slot::Field(k) => rt().value(k).clone(),
        Slot::Value(v) => v,
        Slot::Element(p, i) => mf_get(&read(p), i).unwrap_or(FieldValue::SFBool(false)),
        // Unreachable through `decode`; a released slot has no value.
        Slot::Free => FieldValue::SFBool(false),
    }
}

/// Write a value: a node field gets X3DField.setValue (an event), others are stored.
fn write(h: u32, v: FieldValue) {
    match slot(h) {
        Slot::Field(k) => rt().set_value(k, v),
        Slot::Value(_) => with_state(|s| s.slots[h as usize] = Slot::Value(v)),
        Slot::Element(p, i) => {
            let mut mf = read(p);
            mf_set(&mut mf, i, &v);
            write(p, mf);
        }
        Slot::Free => {}
    }
}

// --- X_ITE field objects (prelude/bridge.js) ------------------------------------------------
//
// A Script's field objects are X_ITE's own (prelude/fields.js) and hold their value in JavaScript. Values cross the boundary as
// plain data: a primitive for SFBool/SFInt32/SFFloat/SFDouble/SFTime/SFString, a node id (or null) for SFNode, an array of numbers
// for the other SF types (SFRotation: x y z angle, tainted, then its quaternion; SFImage: width height comp, then the pixels),
// and a flat array for MF types (MFImage: an array of SFImage data, MFNode: ids).

fn data_number<'js>(ctx: &Ctx<'js>, x: f64) -> Value<'js> {
    if jsmath::is_js_undefined(x) {
        return Value::new_undefined(ctx.clone());
    }

    if jsmath::is_js_null(x) {
        return Value::new_null(ctx.clone());
    }

    // Value::new_number makes every integral value an int, and -0 with it.
    if x == 0.0 && x.is_sign_negative() {
        return Value::new_float(ctx.clone(), x);
    }

    Value::new_number(ctx.clone(), x)
}

fn data_array<'js>(ctx: &Ctx<'js>, items: impl IntoIterator<Item = Value<'js>>) -> rquickjs::Result<Value<'js>> {
    let a = Array::new(ctx.clone())?;

    for (i, v) in items.into_iter().enumerate() {
        a.set(i, v)?;
    }

    Ok(a.into_value())
}

fn image_data<'js>(ctx: &Ctx<'js>, img: &Image) -> rquickjs::Result<Value<'js>> {
    let pixels = data_array(ctx, img.array.iter().map(|p| Value::new_int(ctx.clone(), *p)))?;
    data_array(ctx, [Value::new_int(ctx.clone(), img.width), Value::new_int(ctx.clone(), img.height), Value::new_int(ctx.clone(), img.comp), pixels])
}

fn node_data<'js>(ctx: &Ctx<'js>, n: &Option<NodeId>) -> Value<'js> {
    match n {
        Some(n) => Value::new_number(ctx.clone(), *n as f64),
        None => Value::new_null(ctx.clone()),
    }
}

/// FieldValue -> the plain data of a field object.
fn value_data<'js>(ctx: &Ctx<'js>, v: &FieldValue) -> rquickjs::Result<Value<'js>> {
    let nums = |xs: &mut dyn Iterator<Item = f64>| data_array(ctx, xs.map(|x| data_number(ctx, x)).collect::<Vec<_>>());

    Ok(match v {
        FieldValue::SFBool(b) => Value::new_bool(ctx.clone(), *b),
        FieldValue::SFInt32(x) => Value::new_int(ctx.clone(), *x),
        FieldValue::SFFloat(x) | FieldValue::SFDouble(x) | FieldValue::SFTime(x) => data_number(ctx, *x),
        FieldValue::SFString(s) => rquickjs::String::from_str(ctx.clone(), s)?.into_value(),
        FieldValue::SFNode(n) => node_data(ctx, n),
        FieldValue::SFImage(img) => image_data(ctx, img)?,
        FieldValue::SFRotation(r) => {
            let (aa, tainted, q) = r.state();
            nums(&mut aa.into_iter().chain([if tainted { 1.0 } else { 0.0 }, q.x, q.y, q.z, q.w]))?
        }
        FieldValue::SFColor(_) | FieldValue::SFColorRGBA(_) | FieldValue::SFVec2d(_) | FieldValue::SFVec2f(_) | FieldValue::SFVec3d(_)
        | FieldValue::SFVec3f(_) | FieldValue::SFVec4d(_) | FieldValue::SFVec4f(_) | FieldValue::SFQuaternion(_) | FieldValue::SFMatrix3d(_)
        | FieldValue::SFMatrix3f(_) | FieldValue::SFMatrix4d(_) | FieldValue::SFMatrix4f(_) => nums(&mut sf_numbers(v).into_iter())?,
        FieldValue::MFBool(a) => data_array(ctx, a.iter().map(|b| Value::new_bool(ctx.clone(), *b)))?,
        FieldValue::MFInt32(a) => nums(&mut a.iter().map(|x| *x as f64))?,
        FieldValue::MFColor(a) | FieldValue::MFColorRGBA(a) | FieldValue::MFFloat(a) | FieldValue::MFMatrix3f(a) | FieldValue::MFMatrix4f(a)
        | FieldValue::MFVec2f(a) | FieldValue::MFVec3f(a) | FieldValue::MFVec4f(a) => nums(&mut a.iter().map(|x| *x as f64))?,
        FieldValue::MFDouble(a) | FieldValue::MFMatrix3d(a) | FieldValue::MFMatrix4d(a) | FieldValue::MFQuaternion(a) | FieldValue::MFRotation(a)
        | FieldValue::MFTime(a) | FieldValue::MFVec2d(a) | FieldValue::MFVec3d(a) | FieldValue::MFVec4d(a) => nums(&mut a.iter().copied())?,
        FieldValue::MFString(a) => data_array(ctx, a.iter().map(|s| rquickjs::String::from_str(ctx.clone(), s).unwrap().into_value()))?,
        FieldValue::MFNode(a) => data_array(ctx, a.iter().map(|n| node_data(ctx, n)))?,
        FieldValue::MFImage(a) => {
            let items = a.iter().map(|img| image_data(ctx, img)).collect::<rquickjs::Result<Vec<_>>>()?;
            data_array(ctx, items)?
        }
    })
}

fn data_numbers(v: &Value<'_>) -> Vec<f64> {
    let Some(a) = v.as_array() else { return vec![] };

    (0..a.len())
        .map(|i| match a.get::<Value>(i) {
            Ok(x) if x.is_undefined() => jsmath::JS_UNDEFINED,
            Ok(x) if x.is_null() => jsmath::JS_NULL,
            Ok(x) => x.as_number().unwrap_or(f64::NAN),
            Err(_) => f64::NAN,
        })
        .collect()
}

fn data_image(v: &Value<'_>) -> Image {
    let Some(a) = v.as_array() else { return Image::default() };
    let n = |i: usize| a.get::<Value>(i).ok().and_then(|x| x.as_number()).unwrap_or(0.0);
    let pixels = a.get::<Value>(3).map(|p| data_numbers(&p)).unwrap_or_default();

    Image { width: n(0) as i32, height: n(1) as i32, comp: n(2) as i32, array: pixels.into_iter().map(|p| p as i32).collect() }
}

fn data_node(v: &Value<'_>) -> Option<NodeId> {
    v.as_number().map(|n| n as NodeId)
}

/// The plain data of a field object -> FieldValue of type `ty`.
fn data_value(ty: FieldType, v: &Value<'_>) -> FieldValue {
    use FieldType as T;

    let list = |f: &dyn Fn(Value) -> FieldValue| -> Vec<FieldValue> {
        v.as_array().map(|a| (0..a.len()).filter_map(|i| a.get::<Value>(i).ok()).map(f).collect()).unwrap_or_default()
    };
    let f32s = || data_numbers(v).into_iter().map(|x| x as f32).collect::<Vec<f32>>();

    match ty {
        T::SFBool => FieldValue::SFBool(v.as_bool().unwrap_or(false)),
        T::SFInt32 => FieldValue::SFInt32(v.as_number().map(jsmath::to_int32).unwrap_or(0)),
        T::SFFloat => FieldValue::SFFloat(v.as_number().unwrap_or(0.0)),
        T::SFDouble => FieldValue::SFDouble(v.as_number().unwrap_or(0.0)),
        T::SFTime => FieldValue::SFTime(v.as_number().unwrap_or(0.0)),
        T::SFString => FieldValue::SFString(v.as_string().and_then(|s| s.to_string().ok()).unwrap_or_default()),
        T::SFNode => FieldValue::SFNode(data_node(v)),
        T::SFImage => FieldValue::SFImage(data_image(v)),
        T::SFRotation => {
            let n = data_numbers(v);
            let g = |i: usize| n.get(i).copied().unwrap_or(0.0);
            FieldValue::SFRotation(Rotation4::from_state([g(0), g(1), g(2), g(3)], g(4) != 0.0, Quaternion::new(g(5), g(6), g(7), g(8))))
        }
        T::SFColor => {
            let n = data_numbers(v);
            let g = |i: usize| n.get(i).copied().unwrap_or(0.0);
            FieldValue::SFColor(Color3::from_raw(g(0), g(1), g(2)))
        }
        T::SFColorRGBA => {
            let n = data_numbers(v);
            let g = |i: usize| n.get(i).copied().unwrap_or(0.0);
            FieldValue::SFColorRGBA(Color4::from_raw(g(0), g(1), g(2), g(3)))
        }
        T::MFBool => FieldValue::MFBool(list(&|x| FieldValue::SFBool(x.as_bool().unwrap_or(false))).into_iter().map(|x| matches!(x, FieldValue::SFBool(true))).collect()),
        T::MFInt32 => FieldValue::MFInt32(data_numbers(v).into_iter().map(jsmath::to_int32).collect()),
        T::MFColor => FieldValue::MFColor(f32s()),
        T::MFColorRGBA => FieldValue::MFColorRGBA(f32s()),
        T::MFFloat => FieldValue::MFFloat(f32s()),
        T::MFMatrix3f => FieldValue::MFMatrix3f(f32s()),
        T::MFMatrix4f => FieldValue::MFMatrix4f(f32s()),
        T::MFVec2f => FieldValue::MFVec2f(f32s()),
        T::MFVec3f => FieldValue::MFVec3f(f32s()),
        T::MFVec4f => FieldValue::MFVec4f(f32s()),
        T::MFDouble => FieldValue::MFDouble(data_numbers(v)),
        T::MFMatrix3d => FieldValue::MFMatrix3d(data_numbers(v)),
        T::MFMatrix4d => FieldValue::MFMatrix4d(data_numbers(v)),
        T::MFQuaternion => FieldValue::MFQuaternion(data_numbers(v)),
        T::MFRotation => FieldValue::MFRotation(data_numbers(v)),
        T::MFTime => FieldValue::MFTime(data_numbers(v)),
        T::MFVec2d => FieldValue::MFVec2d(data_numbers(v)),
        T::MFVec3d => FieldValue::MFVec3d(data_numbers(v)),
        T::MFVec4d => FieldValue::MFVec4d(data_numbers(v)),
        T::MFString => FieldValue::MFString(v.as_array().map(|a| (0..a.len()).map(|i| a.get::<Coerced<String>>(i).map(|c| c.0).unwrap_or_default()).collect()).unwrap_or_default()),
        T::MFNode => FieldValue::MFNode(v.as_array().map(|a| (0..a.len()).map(|i| a.get::<Value>(i).ok().and_then(|x| data_node(&x))).collect()).unwrap_or_default()),
        T::MFImage => FieldValue::MFImage(v.as_array().map(|a| (0..a.len()).filter_map(|i| a.get::<Value>(i).ok()).map(|x| data_image(&x)).collect()).unwrap_or_default()),
        t => sf_from_numbers(t, &data_numbers(v)),
    }
}

/// The name prelude.js keeps the callbacks of a node field under (a handle can be released while the field lives on).
fn fkey_id(k: FKey) -> String {
    match k.1 {
        FieldLoc::Pre(i) => format!("{}:p{i}", k.0),
        FieldLoc::User(i) => format!("{}:u{i}", k.0),
        FieldLoc::Priv(i) => format!("{}:x{i}", k.0),
    }
}

fn slot_key(h: u32) -> Option<FKey> {
    match slot(h) {
        Slot::Field(k) => Some(k),
        _ => None,
    }
}

// --- FieldValue helpers ----------------------------------------------------------------------

pub fn sf_type_of(mf: FieldType) -> FieldType {
    use FieldType::*;
    match mf {
        MFBool => SFBool,
        MFColor => SFColor,
        MFColorRGBA => SFColorRGBA,
        MFDouble => SFDouble,
        MFFloat => SFFloat,
        MFImage => SFImage,
        MFInt32 => SFInt32,
        MFMatrix3d => SFMatrix3d,
        MFMatrix3f => SFMatrix3f,
        MFMatrix4d => SFMatrix4d,
        MFMatrix4f => SFMatrix4f,
        MFNode => SFNode,
        MFQuaternion => SFQuaternion,
        MFRotation => SFRotation,
        MFString => SFString,
        MFTime => SFTime,
        MFVec2d => SFVec2d,
        MFVec2f => SFVec2f,
        MFVec3d => SFVec3d,
        MFVec3f => SFVec3f,
        MFVec4d => SFVec4d,
        MFVec4f => SFVec4f,
        t => t,
    }
}

/// Numbers of an SF value (vector, color, rotation as stored x y z angle, matrix).
fn sf_numbers(v: &FieldValue) -> Vec<f64> {
    match v {
        FieldValue::SFColor(c) => c.components().to_vec(),
        FieldValue::SFColorRGBA(c) => c.components().to_vec(),
        FieldValue::SFVec2d(a) | FieldValue::SFVec2f(a) => a.to_array().to_vec(),
        FieldValue::SFVec3d(a) | FieldValue::SFVec3f(a) => a.to_array().to_vec(),
        FieldValue::SFVec4d(a) | FieldValue::SFVec4f(a) => a.to_array().to_vec(),
        FieldValue::SFRotation(r) => {
            let mut r = *r;
            r.components().to_vec()
        }
        FieldValue::SFQuaternion(q) => vec![q.x, q.y, q.z, q.w],
        FieldValue::SFMatrix3d(m) | FieldValue::SFMatrix3f(m) => m.0.to_vec(),
        FieldValue::SFMatrix4d(m) | FieldValue::SFMatrix4f(m) => m.0.to_vec(),
        FieldValue::SFFloat(x) | FieldValue::SFDouble(x) | FieldValue::SFTime(x) => vec![*x],
        FieldValue::SFInt32(x) => vec![*x as f64],
        FieldValue::SFBool(b) => vec![if *b { 1.0 } else { 0.0 }],
        _ => vec![],
    }
}

/// Build an SF value of type `ty` from numbers (missing ones are 0, like `+undefined` → NaN is
/// not reproduced for absent arguments: X_ITE constructors default them).
fn sf_from_numbers(ty: FieldType, n: &[f64]) -> FieldValue {
    let g = |i: usize, d: f64| n.get(i).copied().unwrap_or(d);

    match ty {
        FieldType::SFColor => FieldValue::SFColor(Color3::new(g(0, 0.0), g(1, 0.0), g(2, 0.0))),
        FieldType::SFColorRGBA => FieldValue::SFColorRGBA(Color4::new(g(0, 0.0), g(1, 0.0), g(2, 0.0), g(3, 0.0))),
        FieldType::SFVec2d => FieldValue::SFVec2d(Vector2::new(g(0, 0.0), g(1, 0.0))),
        FieldType::SFVec2f => FieldValue::SFVec2f(Vector2::new(g(0, 0.0), g(1, 0.0))),
        FieldType::SFVec3d => FieldValue::SFVec3d(Vector3::new(g(0, 0.0), g(1, 0.0), g(2, 0.0))),
        FieldType::SFVec3f => FieldValue::SFVec3f(Vector3::new(g(0, 0.0), g(1, 0.0), g(2, 0.0))),
        FieldType::SFVec4d => FieldValue::SFVec4d(Vector4::new(g(0, 0.0), g(1, 0.0), g(2, 0.0), g(3, 1.0))),
        FieldType::SFVec4f => FieldValue::SFVec4f(Vector4::new(g(0, 0.0), g(1, 0.0), g(2, 0.0), g(3, 1.0))),
        FieldType::SFRotation => FieldValue::SFRotation(Rotation4::new(g(0, 0.0), g(1, 0.0), g(2, 1.0), g(3, 0.0))),
        FieldType::SFQuaternion => FieldValue::SFQuaternion(Quaternion::new(g(0, 0.0), g(1, 0.0), g(2, 0.0), g(3, 0.0))),
        FieldType::SFMatrix3d | FieldType::SFMatrix3f => {
            let mut m = Matrix3::default();
            if n.len() >= 9 {
                m.0.copy_from_slice(&n[..9]);
            }
            if ty == FieldType::SFMatrix3d { FieldValue::SFMatrix3d(m) } else { FieldValue::SFMatrix3f(m) }
        }
        FieldType::SFMatrix4d | FieldType::SFMatrix4f => {
            let mut m = Matrix4::default();
            if n.len() >= 16 {
                m.0.copy_from_slice(&n[..16]);
            }
            if ty == FieldType::SFMatrix4d { FieldValue::SFMatrix4d(m) } else { FieldValue::SFMatrix4f(m) }
        }
        FieldType::SFFloat => FieldValue::SFFloat(g(0, 0.0)),
        FieldType::SFDouble => FieldValue::SFDouble(g(0, 0.0)),
        FieldType::SFTime => FieldValue::SFTime(g(0, 0.0)),
        FieldType::SFInt32 => FieldValue::SFInt32(jsmath::to_int32(g(0, 0.0))),
        FieldType::SFBool => FieldValue::SFBool(g(0, 0.0) != 0.0),
        t => FieldValue::default_for(t),
    }
}

pub fn mf_get(mf: &FieldValue, i: usize) -> Option<FieldValue> {
    if i >= mf.len() {
        return None;
    }

    let st = sf_type_of(mf.field_type());

    Some(match mf {
        FieldValue::MFBool(v) => FieldValue::SFBool(v[i]),
        FieldValue::MFInt32(v) => FieldValue::SFInt32(v[i]),
        FieldValue::MFFloat(v) => FieldValue::SFFloat(v[i] as f64),
        FieldValue::MFDouble(v) => FieldValue::SFDouble(v[i]),
        FieldValue::MFTime(v) => FieldValue::SFTime(v[i]),
        FieldValue::MFString(v) => FieldValue::SFString(v[i].clone()),
        FieldValue::MFNode(v) => FieldValue::SFNode(v[i]),
        FieldValue::MFImage(v) => FieldValue::SFImage(v[i].clone()),
        FieldValue::MFRotation(v) => FieldValue::SFRotation(Rotation4::new(v[i * 4], v[i * 4 + 1], v[i * 4 + 2], v[i * 4 + 3])),
        _ => {
            let c = mf.field_type().components();
            let nums: Vec<f64> = match mf {
                FieldValue::MFColor(v) | FieldValue::MFColorRGBA(v) | FieldValue::MFVec2f(v) | FieldValue::MFVec3f(v) | FieldValue::MFVec4f(v)
                | FieldValue::MFMatrix3f(v) | FieldValue::MFMatrix4f(v) => v[i * c..(i + 1) * c].iter().map(|x| *x as f64).collect(),
                FieldValue::MFVec2d(v) | FieldValue::MFVec3d(v) | FieldValue::MFVec4d(v) | FieldValue::MFMatrix3d(v) | FieldValue::MFMatrix4d(v)
                | FieldValue::MFQuaternion(v) => v[i * c..(i + 1) * c].to_vec(),
                _ => vec![],
            };
            sf_from_numbers(st, &nums)
        }
    })
}

fn mf_numbers_of(sf: &FieldValue, c: usize) -> Vec<f64> {
    let mut n = match sf {
        // Typed arrays store the rotation's x y z angle as given.
        FieldValue::SFRotation(r) => r.raw().to_vec(),
        v => sf_numbers(v),
    };
    n.resize(c, 0.0);
    n
}

/// Set element `i` (growing with default elements like `resize`).
/// The element a script-grown MF array is padded with: the single value of the type (`getSingleValue`); an SFVec4 starts at w = 1.
fn padding_for(st: FieldType) -> FieldValue {
    match st {
        FieldType::SFVec4f | FieldType::SFVec4d => sf_from_numbers(st, &[]),
        _ => FieldValue::default_for(st),
    }
}

pub fn mf_set(mf: &mut FieldValue, i: usize, sf: &FieldValue) {
    let st = sf_type_of(mf.field_type());

    while mf.len() <= i {
        mf_push(mf, &padding_for(st));
    }

    let c = mf.field_type().components();

    match mf {
        FieldValue::MFBool(v) => v[i] = matches!(sf, FieldValue::SFBool(true)),
        FieldValue::MFInt32(v) => v[i] = sf_numbers(sf).first().map(|x| jsmath::to_int32(*x)).unwrap_or(0),
        FieldValue::MFFloat(v) => v[i] = sf_numbers(sf).first().copied().unwrap_or(0.0) as f32,
        FieldValue::MFDouble(v) | FieldValue::MFTime(v) => v[i] = sf_numbers(sf).first().copied().unwrap_or(0.0),
        FieldValue::MFString(v) => {
            v[i] = match sf {
                FieldValue::SFString(s) => s.clone(),
                _ => String::new(),
            }
        }
        FieldValue::MFNode(v) => {
            v[i] = match sf {
                FieldValue::SFNode(n) => *n,
                _ => None,
            }
        }
        FieldValue::MFImage(v) => {
            if let FieldValue::SFImage(img) = sf {
                v[i] = img.clone();
            }
        }
        FieldValue::MFColor(v) | FieldValue::MFColorRGBA(v) | FieldValue::MFVec2f(v) | FieldValue::MFVec3f(v) | FieldValue::MFVec4f(v)
        | FieldValue::MFMatrix3f(v) | FieldValue::MFMatrix4f(v) => {
            for (j, x) in mf_numbers_of(sf, c).into_iter().enumerate() {
                v[i * c + j] = x as f32;
            }
        }
        FieldValue::MFVec2d(v) | FieldValue::MFVec3d(v) | FieldValue::MFVec4d(v) | FieldValue::MFMatrix3d(v) | FieldValue::MFMatrix4d(v)
        | FieldValue::MFQuaternion(v) | FieldValue::MFRotation(v) => {
            for (j, x) in mf_numbers_of(sf, c).into_iter().enumerate() {
                v[i * c + j] = x;
            }
        }
        _ => {}
    }
}

fn mf_push(mf: &mut FieldValue, sf: &FieldValue) {
    match mf {
        FieldValue::MFBool(v) => v.push(false),
        FieldValue::MFInt32(v) => v.push(0),
        FieldValue::MFFloat(v) => v.push(0.0),
        FieldValue::MFDouble(v) | FieldValue::MFTime(v) => v.push(0.0),
        FieldValue::MFString(v) => v.push(String::new()),
        FieldValue::MFNode(v) => v.push(None),
        FieldValue::MFImage(v) => v.push(Image::new(0.0, 0.0, 0.0, vec![])),
        _ => {
            let c = mf.field_type().components();
            mf.push_numbers(&vec![0.0; c]);
        }
    }

    let i = mf.len() - 1;
    mf_set(mf, i, sf);
}

fn is_scalar(t: FieldType) -> bool {
    matches!(t, FieldType::SFBool | FieldType::SFInt32 | FieldType::SFFloat | FieldType::SFDouble | FieldType::SFTime | FieldType::SFString)
}

/// `field .toString ()` (X3DObject.toString with the default generator).
fn to_vrml(v: &FieldValue) -> String {
    let mut g = Generator::default();

    match v {
        FieldValue::SFNode(_) | FieldValue::MFNode(_) => rt().scene.node_value_to_vrml_string(v),
        _ => {
            v.to_vrml_stream(&mut g);
            g.string
        }
    }
}

// --- JS conversions --------------------------------------------------------------------------

fn obj_num<'js>(v: &Value<'js>, key: &str) -> Option<f64> {
    let o = v.as_object()?;
    // A getter that throws (the Script's global object, a Proxy) must not leave its exception pending.
    let x: Value = match o.get(key) {
        Ok(x) => x,
        Err(_) => {
            let _ = v.ctx().catch();
            return None;
        }
    };

    if x.is_null() {
        return Some(f64::NAN);
    }

    x.as_number()
}

/// A field handle passed from JS as `{ __h }` (none if its slot was released since).
fn arg_handle(v: &Value<'_>) -> Option<u32> {
    obj_num(v, "__h").and_then(decode)
}

/// The field handle argument `i` of a native call; a released one throws.
fn handle_at(ctx: &Ctx<'_>, args: &[Value<'_>], i: usize) -> rquickjs::Result<u32> {
    match args.get(i).and_then(|v| v.as_number()).and_then(decode) {
        Some(h) => Ok(h),
        None => throw(ctx, "Error: X3DField handle is no longer valid."),
    }
}

/// A node passed as `{ __n }` (or `null`): Some(None) for null.
fn arg_node(v: &Value<'_>) -> Option<Option<NodeId>> {
    if v.is_null() || v.is_undefined() {
        return Some(None);
    }

    let o = v.as_object()?;

    if !o.contains_key("__n").unwrap_or(false) {
        let _ = v.ctx().catch();
        return None;
    }

    let x: Value = match o.get("__n") {
        Ok(x) => x,
        Err(_) => {
            let _ = v.ctx().catch();
            return None;
        }
    };
    Some(x.as_number().map(|n| n as NodeId))
}

fn coerce_f64(ctx: &Ctx<'_>, v: &Value<'_>) -> f64 {
    if let Some(h) = arg_handle(v) {
        // `+field`: a scalar field converts through valueOf; every other field object becomes NaN.
        return match read(h) {
            FieldValue::SFBool(b) => f64::from(u8::from(b)),
            FieldValue::SFInt32(n) => f64::from(n),
            FieldValue::SFFloat(n) | FieldValue::SFDouble(n) | FieldValue::SFTime(n) => n,
            _ => f64::NAN,
        };
    }

    v.get::<Coerced<f64>>().map(|c| c.0).unwrap_or_else(|_| {
        let _ = ctx.catch();
        f64::NAN
    })
}

fn coerce_string(ctx: &Ctx<'_>, v: &Value<'_>) -> String {
    if let Some(h) = arg_handle(v) {
        return match read(h) {
            FieldValue::SFString(s) => s,
            other => to_vrml(&other),
        };
    }

    v.get::<Coerced<String>>().map(|c| c.0).unwrap_or_else(|_| {
        let _ = ctx.catch();
        String::new()
    })
}

fn coerce_bool(v: &Value<'_>) -> bool {
    if let Some(h) = arg_handle(v) {
        return match read(h) {
            FieldValue::SFBool(b) => b,
            _ => true,
        };
    }

    v.get::<Coerced<bool>>().map(|c| c.0).unwrap_or(false)
}

/// X3DField.setValue (value) conversion for a target of type `ty`.
fn convert<'js>(ctx: &Ctx<'js>, ty: FieldType, v: &Value<'js>) -> Option<FieldValue> {
    match ty {
        FieldType::SFBool => return Some(FieldValue::SFBool(coerce_bool(v))),
        FieldType::SFInt32 => return Some(FieldValue::SFInt32(jsmath::to_int32(coerce_f64(ctx, v)))),
        FieldType::SFFloat => return Some(FieldValue::SFFloat(coerce_f64(ctx, v))),
        FieldType::SFDouble => return Some(FieldValue::SFDouble(coerce_f64(ctx, v))),
        FieldType::SFTime => return Some(FieldValue::SFTime(coerce_f64(ctx, v))),
        FieldType::SFString => return Some(FieldValue::SFString(coerce_string(ctx, v))),
        FieldType::SFNode => {
            if let Some(n) = arg_node(v) {
                return Some(FieldValue::SFNode(n));
            }

            if let Some(h) = arg_handle(v) {
                if let FieldValue::SFNode(n) = read(h) {
                    return Some(FieldValue::SFNode(n));
                }
            }

            return None;
        }
        _ => {}
    }

    if let Some(h) = arg_handle(v) {
        let value = read(h);

        if value.field_type() == ty {
            return Some(value);
        }

        // MF ← MF of another type, or SF of the element type: not converted by X_ITE either.
        return None;
    }

    if ty.is_multi() {
        let st = sf_type_of(ty);
        let mut mf = FieldValue::default_for(ty);

        if let Some(arr) = v.as_array() {
            for i in 0..arr.len() {
                let e: Value = arr.get(i).ok()?;
                let sf = convert(ctx, st, &e).unwrap_or_else(|| FieldValue::default_for(st));
                mf_push(&mut mf, &sf);
            }
        }

        return Some(mf);
    }

    None
}

/// A value for JS: primitives for scalars, `{ __n }` for nodes, `{ __h, __t, __a }` otherwise.
fn field_value_js<'js>(ctx: &Ctx<'js>, h: u32) -> Value<'js> {
    let v = read(h);
    let attached = matches!(slot(h), Slot::Field(_));

    primitive_or_handle(ctx, &v, h, attached)
}

fn primitive_or_handle<'js>(ctx: &Ctx<'js>, v: &FieldValue, h: u32, attached: bool) -> Value<'js> {
    match v {
        FieldValue::SFBool(b) => Value::new_bool(ctx.clone(), *b),
        FieldValue::SFInt32(x) => Value::new_int(ctx.clone(), *x),
        FieldValue::SFFloat(x) | FieldValue::SFDouble(x) | FieldValue::SFTime(x) => data_number(ctx, *x),
        FieldValue::SFString(s) => rquickjs::String::from_str(ctx.clone(), s).unwrap().into_value(),
        FieldValue::SFNode(n) => node_js(ctx, *n),
        _ => handle_js(ctx, h, v.field_type().name(), attached),
    }
}

fn handle_js<'js>(ctx: &Ctx<'js>, h: u32, ty: &str, attached: bool) -> Value<'js> {
    let o = Object::new(ctx.clone()).unwrap();
    o.set("__h", encode(h)).unwrap();
    o.set("__t", ty).unwrap();
    o.set("__a", attached).unwrap();
    o.into_value()
}

fn node_js<'js>(ctx: &Ctx<'js>, n: Option<NodeId>) -> Value<'js> {
    let o = Object::new(ctx.clone()).unwrap();

    match n {
        Some(n) => o.set("__n", n).unwrap(),
        None => o.set("__n", Value::new_null(ctx.clone())).unwrap(),
    }

    o.into_value()
}

/// A fresh detached field for a returned value.
fn new_value_js<'js>(ctx: &Ctx<'js>, v: FieldValue) -> Value<'js> {
    let ty = v.field_type();

    if is_scalar(ty) || ty == FieldType::SFNode {
        return primitive_or_handle(ctx, &v, 0, false);
    }

    let h = detached(v);
    handle_js(ctx, h, ty.name(), false)
}

fn throw<T>(ctx: &Ctx<'_>, msg: &str) -> rquickjs::Result<T> {
    Err(Exception::throw_message(ctx, msg))
}

fn throw_type<T>(ctx: &Ctx<'_>, msg: &str) -> rquickjs::Result<T> {
    Err(Exception::throw_type(ctx, msg))
}

/// The elements of an array-like JS value, as `X3DTypedArrayField.set` and `X3DObjectArrayField.set` read it
/// (`value .length`, `value [i]`): arrays and strings (one element per UTF-16 unit). Other values have none.
fn array_like<'js>(ctx: &Ctx<'js>, v: &Value<'js>) -> Option<Vec<Value<'js>>> {
    if let Some(arr) = v.as_array() {
        return Some((0..arr.len()).map(|i| arr.get::<Value>(i).unwrap_or_else(|_| Value::new_undefined(ctx.clone()))).collect());
    }

    if let Some(s) = v.as_string() {
        let text = s.to_string().ok()?;
        let units: Vec<u16> = text.encode_utf16().collect();

        return Some(units.iter().map(|u| rquickjs::String::from_str(ctx.clone(), &String::from_utf16_lossy(&[*u])).map(|s| s.into_value()).unwrap_or_else(|_| Value::new_undefined(ctx.clone()))).collect());
    }

    None
}

/// The vector families `SFVecNf` / `SFVecNd` and `SFMatrixNf` / `SFMatrixNd`: each assigns the other's components.
fn same_family(a: FieldType, b: FieldType) -> bool {
    use FieldType::*;

    matches!(
        (a, b),
        (SFVec2f | SFVec2d, SFVec2f | SFVec2d)
            | (SFVec3f | SFVec3d, SFVec3f | SFVec3d)
            | (SFVec4f | SFVec4d, SFVec4f | SFVec4d)
            | (SFMatrix3f | SFMatrix3d, SFMatrix3f | SFMatrix3d)
            | (SFMatrix4f | SFMatrix4d, SFMatrix4f | SFMatrix4d)
    )
}

/// `X3DField.setValue (value)`: `set (value instanceof this .constructor ? value .getValue () : value)`, then the type's own `set`.
/// `Ok (None)` leaves the field alone. X_ITE stores JavaScript `undefined` for a value of the wrong shape; a number cannot hold
/// that, so those assignments (an array to an SFVec3f, a number to an MFFloat) are not modeled.
fn set_convert<'js>(ctx: &Ctx<'js>, ty: FieldType, v: &Value<'js>) -> rquickjs::Result<Option<FieldValue>> {
    use FieldType::*;

    if matches!(ty, SFBool | SFInt32 | SFFloat | SFDouble | SFTime | SFString) {
        return Ok(convert(ctx, ty, v));
    }

    if ty == SFNode {
        if let Some(n) = arg_node(v) {
            return Ok(Some(FieldValue::SFNode(n)));
        }

        if let Some(h) = arg_handle(v) {
            if let FieldValue::SFNode(n) = read(h) {
                return Ok(Some(FieldValue::SFNode(n)));
            }
        }

        // `node ?.getType ()`: not a function.
        return throw_type(ctx, "node?.getType is not a function");
    }

    if v.is_null() || v.is_undefined() {
        return throw_type(ctx, "Cannot read properties of null or undefined");
    }

    if !ty.is_multi() {
        if let Some(h) = arg_handle(v) {
            let value = read(h);

            if value.field_type() == ty {
                return Ok(Some(value));
            }

            if same_family(ty, value.field_type()) {
                return Ok(Some(sf_from_numbers(ty, &sf_numbers(&value))));
            }

            if ty == SFRotation || ty == SFQuaternion {
                return throw_type(ctx, "Cannot destructure property of undefined");
            }

            return Ok(None);
        }

        // Rotation4 .assign (rotation) reads the quaternion of whatever it is given.
        if ty == SFRotation || ty == SFQuaternion {
            return throw_type(ctx, "Cannot destructure property of undefined");
        }

        // A plain object with x, y, z, w (Vector3 .assign ({ x, y, z })).
        if matches!(ty, SFVec2f | SFVec2d | SFVec3f | SFVec3d | SFVec4f | SFVec4d) {
            if let Some(o) = v.as_object() {
                let n = ty.components();
                let mut nums = vec![];

                for key in ["x", "y", "z", "w"].iter().take(n) {
                    let e: Value = o.get(*key).unwrap_or_else(|_| Value::new_undefined(ctx.clone()));
                    nums.push(coerce_f64(ctx, &e));
                }

                return Ok(Some(sf_from_numbers(ty, &nums)));
            }
        }

        return Ok(convert(ctx, ty, v));
    }

    // Multi-valued fields.
    if let Some(h) = arg_handle(v) {
        let value = read(h);

        if value.field_type() == ty {
            return Ok(Some(value));
        }

        // Another MF type: element by element through `value [i]` (one number per element for scalar types).
        if value.field_type().is_multi() && ty.components() == 1 && value.field_type().components() == 1 && !matches!(ty, MFString | MFNode | MFImage) && !matches!(value.field_type(), MFString | MFNode | MFImage) {
            let mut mf = FieldValue::default_for(ty);
            let nums: Vec<f64> = (0..value.len()).map(|i| mf_get(&value, i).map(|e| sf_numbers(&e).first().copied().unwrap_or(f64::NAN)).unwrap_or(f64::NAN)).collect();

            mf.set_flat_numbers(&nums);
            return Ok(Some(mf));
        }

        return Ok(None);
    }

    let Some(elements) = array_like(ctx, v) else {
        return Ok(None);
    };

    match ty {
        MFString => {
            let mut mf = FieldValue::default_for(ty);

            for e in &elements {
                mf_push(&mut mf, &FieldValue::SFString(coerce_string(ctx, e)));
            }

            Ok(Some(mf))
        }
        MFNode => {
            let mut nodes = vec![];

            for e in &elements {
                match arg_node(e) {
                    Some(n) => nodes.push(n),
                    None => match arg_handle(e).map(read) {
                        Some(FieldValue::SFNode(n)) => nodes.push(n),
                        _ => return throw_type(ctx, "node?.getType is not a function"),
                    },
                }
            }

            Ok(Some(FieldValue::MFNode(nodes)))
        }
        MFImage => Ok(convert(ctx, ty, v)),
        _ => {
            let components = ty.components();
            let mut nums: Vec<f64> = elements.iter().map(|e| if arg_handle(e).is_some() { f64::NAN } else { coerce_f64(ctx, e) }).collect();
            let rest = nums.len() % components;

            if rest != 0 {
                let keep = nums.len() - rest;

                nums.truncate(keep);
                rt().log_warn(format!("Array length must be multiple of components size, which is {components}."));
            }

            let mut mf = FieldValue::default_for(ty);

            mf.set_flat_numbers(&nums);
            Ok(Some(mf))
        }
    }
}

fn str_arg(ctx: &Ctx<'_>, args: &[Value<'_>], i: usize) -> String {
    args.get(i).map(|v| coerce_string(ctx, v)).unwrap_or_default()
}

fn num_arg(ctx: &Ctx<'_>, args: &[Value<'_>], i: usize) -> f64 {
    args.get(i).map(|v| coerce_f64(ctx, v)).unwrap_or(f64::NAN)
}

/// The execution context a script-side `script` argument names: a Script node's id (its context), -1 (the world, whichever
/// scene it is now), or `-(context + 2)` (a scene made by createX3DFromString / createX3DFromURL, or a world that was replaced).
fn ctx_of_script(x: f64) -> x3d_scene::CtxId {
    if x >= 0.0 {
        rt().scene.node(x as NodeId).ctx
    } else if x == -1.0 {
        rt().browser_execution_context()
    } else {
        (-(x + 2.0)) as x3d_scene::CtxId
    }
}

/// X3DImportedNodeInstance delegates getTypeName / getField / getFieldDefinitions … to the node its Inline exports,
/// as long as the Inline provides it (`$.try (() => importedNode .getExportedNode ())`).
fn inner_node(id: NodeId) -> NodeId {
    if let NodeKind::Imported(i) = rt().scene.node(id).kind {
        let rec = &rt().scene.imported[i];

        if let Some(&scene) = rt().inline_scenes.get(&rec.inline) {
            if let Some(n) = rt().scene.get_exported_node(scene, &rec.exported_name) {
                return n;
            }
        }
    }

    id
}

fn handle_arg(args: &[Value<'_>], i: usize) -> Option<u32> {
    args.get(i).and_then(arg_handle)
}

fn value_arg(args: &[Value<'_>], i: usize) -> Option<FieldValue> {
    handle_arg(args, i).map(read)
}

// --- the native entry point --------------------------------------------------------------------

fn native<'js>(ctx: Ctx<'js>, args: Rest<Value<'js>>) -> rquickjs::Result<Value<'js>> {
    op(&ctx, args.0)
}

fn op<'js>(ctx: &Ctx<'js>, args: Vec<Value<'js>>) -> rquickjs::Result<Value<'js>> {
    let name = args.first().and_then(|v| v.as_string()).and_then(|s| s.to_string().ok()).unwrap_or_default();
    let a = &args[1.min(args.len())..];
    let undefined = Value::new_undefined(ctx.clone());

    match name.as_str() {
        "nofield" => Ok(rquickjs::String::from_str(ctx.clone(), NOFIELD)?.into_value()),
        // Browser.print goes to console.log, which the oracle does not record. X3D_PRINT=1
        // shows it on stderr for debugging.
        "print" => {
            if std::env::var_os("X3D_PRINT").is_some() {
                eprintln!("print: {}", str_arg(ctx, a, 0));
            }

            let text = str_arg(ctx, a, 0);
            rt().print(text);

            Ok(undefined)
        }
        "console" => {
            let text = str_arg(ctx, a, 1);

            match str_arg(ctx, a, 0).as_str() {
                "warn" => rt().log_warn(text),
                "error" => rt().log_error(text),
                _ => rt().print(text),
            }

            Ok(undefined)
        }
        "mathfn" => math_fn(ctx, &str_arg(ctx, a, 0)),
        "fname" => {
            let h = handle_at(ctx, a, 0)?;
            let n = slot_key(h).map(|k| rt().field(k).name.clone()).unwrap_or_default();
            Ok(rquickjs::String::from_str(ctx.clone(), &n)?.into_value())
        }
        "faccess" => {
            let h = handle_at(ctx, a, 0)?;
            let n = slot_key(h).map(|k| rt().field(k).access).unwrap_or(access::INITIALIZE_ONLY);
            Ok(Value::new_int(ctx.clone(), n as i32))
        }
        "set" => {
            let h = handle_at(ctx, a, 0)?;
            let ty = read(h).field_type();

            if let Some(v) = match a.get(1) {
                Some(v) => set_convert(ctx, ty, v)?,
                None => None,
            } {
                write(h, v);
            }

            Ok(undefined)
        }
        "str" => {
            let h = handle_at(ctx, a, 0)?;
            Ok(rquickjs::String::from_str(ctx.clone(), &to_vrml(&read(h)))?.into_value())
        }
        "at" => {
            let h = handle_at(ctx, a, 0)?;
            let i = num_arg(ctx, a, 1) as usize;
            let v = read(h);

            match mf_get(&v, i) {
                None => Ok(undefined),
                Some(sf) if is_scalar(sf.field_type()) || sf.field_type() == FieldType::SFNode => Ok(primitive_or_handle(ctx, &sf, 0, false)),
                Some(sf) => Ok(handle_js(ctx, element(h, i), sf.field_type().name(), false)),
            }
        }
        // --- nodes
        "nfield" => {
            let id = inner_node(num_arg(ctx, a, 0) as NodeId);
            let field = str_arg(ctx, a, 1);

            match rt().get_field(id, &field) {
                Some(k) => {
                    let h = attach(k);
                    Ok(handle_js(ctx, h, rt().field(k).ty().name(), true))
                }
                None => Ok(rquickjs::String::from_str(ctx.clone(), NOFIELD)?.into_value()),
            }
        }
        "ntype" => {
            let id = inner_node(num_arg(ctx, a, 0) as NodeId);
            Ok(rquickjs::String::from_str(ctx.clone(), &rt().type_name(id))?.into_value())
        }
        "nname" => {
            let id = num_arg(ctx, a, 0) as NodeId;
            Ok(rquickjs::String::from_str(ctx.clone(), &rt().scene.node(id).name)?.into_value())
        }
        "fprint" => {
            let h = handle_at(ctx, a, 0)?;
            let v = read(h);
            let text = if str_arg(ctx, a, 1) == "json" { rt().scene.field_value_to_json_string(&v) } else { rt().scene.field_value_to_xml_string(&v) };
            Ok(rquickjs::String::from_str(ctx.clone(), &text)?.into_value())
        }
        "strn" => {
            let h = handle_at(ctx, a, 0)?;
            let v = read(h);
            let text = if matches!(v, FieldValue::SFNode(_) | FieldValue::MFNode(_)) { rt().scene.node_value_to_string(&v) } else { to_vrml(&v) };
            Ok(rquickjs::String::from_str(ctx.clone(), &text)?.into_value())
        }
        "nprint" => {
            let id = num_arg(ctx, a, 0) as NodeId;
            let v = FieldValue::SFNode(Some(id));
            let text = if str_arg(ctx, a, 1) == "json" { rt().scene.field_value_to_json_string(&v) } else { rt().scene.field_value_to_xml_string(&v) };
            Ok(rquickjs::String::from_str(ctx.clone(), &text)?.into_value())
        }
        "nstream" => {
            let id = num_arg(ctx, a, 0) as NodeId;
            Ok(rquickjs::String::from_str(ctx.clone(), &rt().scene.node_value_to_string(&FieldValue::SFNode(Some(id))))?.into_value())
        }
        "nstr" => {
            let id = num_arg(ctx, a, 0) as NodeId;
            Ok(rquickjs::String::from_str(ctx.clone(), &rt().scene.node_to_vrml_string(id))?.into_value())
        }
        "nfields" => {
            let id = inner_node(num_arg(ctx, a, 0) as NodeId);
            let arr = Array::new(ctx.clone())?;
            let n = rt().scene.node(id);

            // X3DBaseNode.getFieldDefinitions: a predefined field's definition holds the class's default value (`value`), a
            // user-defined field's the field itself (`handle`).
            let defs = match &n.kind {
                NodeKind::Builtin(t) => Some(&rt().scene.table.types[*t].variant(rt().scene.spec_version_of(n.ctx)).fields),
                _ => None,
            };

            for (i, f) in n.fields.iter().enumerate() {
                let o = Object::new(ctx.clone())?;
                o.set("name", f.name.as_str())?;
                o.set("accessType", f.access as i32)?;
                o.set("dataType", f.ty().name())?;

                match defs.and_then(|d| d.iter().find(|d| d.name == f.name)) {
                    Some(d) => {
                        o.set("value", value_data(ctx, &d.default)?)?;

                        if let Some(u) = &d.unit {
                            o.set("unit", u.as_str())?;
                        }
                    }
                    None => o.set("handle", encode(attach((id, FieldLoc::Pre(i)))))?,
                }

                arr.set(i, o)?;
            }

            for (i, f) in n.user_fields.iter().enumerate() {
                let o = Object::new(ctx.clone())?;
                o.set("name", f.name.as_str())?;
                o.set("accessType", f.access as i32)?;
                o.set("dataType", f.ty().name())?;
                o.set("handle", encode(attach((id, FieldLoc::User(i)))))?;
                arr.set(n.fields.len() + i, o)?;
            }

            Ok(arr.into_value())
        }
        // --- browser
        "bname" => Ok(rquickjs::String::from_str(ctx.clone(), &rt().scene.table.browser_name)?.into_value()),
        "bversion" => Ok(rquickjs::String::from_str(ctx.clone(), &rt().scene.table.browser_version)?.into_value()),
        "bdescription" => Ok(rquickjs::String::from_str(ctx.clone(), &rt().description)?.into_value()),
        "bsetdescription" => {
            let d = str_arg(ctx, a, 0);
            rt().description = d.clone();
            with_state(|s| s.description = d);
            Ok(undefined)
        }
        // Browser.getBrowserOption / getBrowserProperty / getRenderingProperty: the field value,
        // null for an unknown name.
        "browserField" => {
            let set = str_arg(ctx, a, 0);
            let name = str_arg(ctx, a, 1);
            let b = &rt().browser_options;
            let fields = match set.as_str() {
                "options" => &b.options,
                "properties" => &b.properties,
                _ => &b.rendering,
            };

            use x3d_runtime::browser_options::OptionValue;

            Ok(match fields.get(&name) {
                None => Value::new_null(ctx.clone()),
                Some(OptionValue::Bool(v)) => Value::new_bool(ctx.clone(), *v),
                Some(OptionValue::Int32(v)) => Value::new_int(ctx.clone(), *v),
                Some(OptionValue::Float(v) | OptionValue::Double(v)) => Value::new_number(ctx.clone(), *v),
                Some(OptionValue::String(v)) => rquickjs::String::from_str(ctx.clone(), v)?.into_value(),
            })
        }
        // X3DBrowserContext.getBrowserOptions () … .getFieldDefinitions (): name, type, initial value.
        "browserDefs" => {
            use x3d_runtime::browser_options::OptionValue;

            let list = Array::new(ctx.clone())?;

            for (i, (name, v)) in x3d_runtime::browser_options::default_fields(&str_arg(ctx, a, 0)).entries().enumerate() {
                let value = match v {
                    OptionValue::Bool(b) => Value::new_bool(ctx.clone(), *b),
                    OptionValue::Int32(x) => Value::new_int(ctx.clone(), *x),
                    OptionValue::Float(x) | OptionValue::Double(x) => data_number(ctx, *x),
                    OptionValue::String(s) => rquickjs::String::from_str(ctx.clone(), s)?.into_value(),
                };
                let entry = data_array(ctx, [rquickjs::String::from_str(ctx.clone(), name)?.into_value(), rquickjs::String::from_str(ctx.clone(), v.type_name())?.into_value(), value])?;

                list.set(i, entry)?;
            }

            Ok(list.into_value())
        }
        // X3DBrowserContext.getActiveLayer … (`None`: the default node), getCurrentViewer, and the default nodes.
        "bactive" => {
            let active = &rt().browser_active;
            let n = match str_arg(ctx, a, 0).as_str() {
                "layer" => active.layer,
                "navigationInfo" => active.navigation_info,
                _ => active.viewpoint,
            };

            Ok(node_data(ctx, &n))
        }
        "bdefault" => {
            let n = rt().default_node(&str_arg(ctx, a, 0));
            Ok(node_data(ctx, &n))
        }
        "bviewer" => Ok(rquickjs::String::from_str(ctx.clone(), &rt().browser_active.viewer)?.into_value()),
        "optionType" => {
            let name = str_arg(ctx, a, 0);

            Ok(match rt().browser_options.get_option(&name) {
                None => Value::new_null(ctx.clone()),
                Some(v) => rquickjs::String::from_str(ctx.clone(), v.type_name())?.into_value(),
            })
        }
        // Browser.setBrowserOption: the value is already converted to the field type (prelude).
        "setOption" => {
            use x3d_runtime::browser_options::OptionValue;

            let name = str_arg(ctx, a, 0);
            let Some(current) = rt().browser_options.get_option(&name).cloned() else { return Ok(undefined) };
            let value = match current {
                OptionValue::Bool(_) => OptionValue::Bool(a.get(1).and_then(|v| v.as_bool()).unwrap_or(false)),
                OptionValue::Int32(_) => OptionValue::Int32(num_arg(ctx, a, 1) as i32),
                OptionValue::Float(_) => OptionValue::Float(num_arg(ctx, a, 1)),
                OptionValue::Double(_) => OptionValue::Double(num_arg(ctx, a, 1)),
                OptionValue::String(_) => OptionValue::String(str_arg(ctx, a, 1)),
            };

            rt().browser_options.set_option(&name, value);
            Ok(undefined)
        }
        "time" => Ok(Value::new_number(ctx.clone(), rt().now)),
        "asyncError" => {
            // A callback returned a promise that rejected: `await callback .call (…)` throws into call__'s catch.
            let node = num_arg(ctx, a, 0) as NodeId;
            let name = str_arg(ctx, a, 1);
            let text = str_arg(ctx, a, 2);

            rt().report_script_error(node, &name, &text);
            Ok(undefined)
        }
        "datenow" => Ok(Value::new_number(ctx.clone(), rt().date_override.unwrap_or(rt().now) * 1000.0)),
        "framerate" => Ok(Value::new_number(ctx.clone(), rt().frame_rate)),
        "browserTime" => Ok(Value::new_number(ctx.clone(), rt().world_state.initialized_time)),
        // Browser.nextViewpoint () and friends: queued for the viewer (X3DBrowser viewpoint API).
        "brequest" => {
            let r = str_arg(ctx, a, 0);
            rt().browser_requests.push(r);
            Ok(undefined)
        }
        "specVersion" => {
            let script = num_arg(ctx, a, 0);
            let c = ctx_of_script(script);
            Ok(rquickjs::String::from_str(ctx.clone(), rt().scene.spec_version_of(c))?.into_value())
        }
        // X3DBrowser.getBaseURL: the page's URL (an embedder without a page: the URL of the caller's scene).
        "baseURL" => {
            let script = num_arg(ctx, a, 0);
            let c = ctx_of_script(script);
            let page = rt().page_url.clone();
            let url = if page.is_empty() { rt().scene.world_url_of(c).to_string() } else { page };
            Ok(rquickjs::String::from_str(ctx.clone(), &url)?.into_value())
        }
        "worldURL" => {
            let script = num_arg(ctx, a, 0);
            let c = ctx_of_script(script);
            Ok(rquickjs::String::from_str(ctx.clone(), rt().scene.world_url_of(c))?.into_value())
        }
        "createVrml" => {
            let text = str_arg(ctx, a, 0);
            let script = num_arg(ctx, a, 1);
            let parent = ctx_of_script(script);

            match rt().create_vrml_from_string(&text, parent) {
                Ok(roots) => {
                    let arr = Array::new(ctx.clone())?;

                    for (i, r) in roots.iter().enumerate() {
                        match r {
                            Some(n) => arr.set(i, *n)?,
                            None => arr.set(i, Value::new_null(ctx.clone()))?,
                        }
                    }

                    Ok(arr.into_value())
                }
                Err(e) => throw(ctx, e.strip_prefix("Error: ").unwrap_or(&e)),
            }
        }
        "cx" => ctx_api::op(ctx, a),
        "ctxOf" => Ok(Value::new_number(ctx.clone(), ctx_of_script(num_arg(ctx, a, 0)) as f64)),
        // X3DBrowser.createX3DFromString / createX3DFromURL: the request, or the message of the rejection.
        "browserScene" => {
            let script = num_arg(ctx, a, 0);
            let parent = ctx_of_script(script);
            let node = (script >= 0.0).then_some(script as NodeId);
            let o = Object::new(ctx.clone())?;

            let kind = str_arg(ctx, a, 1);

            if kind == "create" {
                // createScene (profile, ... components): the scene is there at once.
                let profile = a.get(2).filter(|v| !v.is_undefined() && !v.is_null()).map(|v| coerce_string(ctx, v));
                let components: Vec<(String, i32)> = a
                    .get(3)
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter::<Value>().filter_map(Result::ok).filter_map(|e| e.as_array().and_then(|p| Some((p.get::<Coerced<String>>(0).ok()?.0, p.get::<Coerced<f64>>(1).ok()?.0 as i32)))).collect())
                    .unwrap_or_default();

                o.set("ctx", rt().browser_create_scene(profile, components, parent) as f64)?;
            } else if kind == "load" {
                // Browser.loadURL (url, parameter): `id` is absent when the url only names a viewpoint of the world.
                let strings = |i: usize| -> Vec<String> { a.get(i).and_then(|v| v.as_array()).map(|arr| arr.iter::<Coerced<String>>().filter_map(Result::ok).map(|c| c.0).collect()).unwrap_or_default() };

                match rt().browser_load_url(strings(2), strings(3), parent, node) {
                    Ok(id) => o.set("id", id as f64)?,
                    Err(Some(message)) => o.set("error", message)?,
                    Err(None) => {}
                }
            } else if kind == "string" || kind == "json" {
                let text = str_arg(ctx, a, 2);
                let made = if kind == "string" { rt().browser_create_from_string(&text, parent, node) } else { rt().browser_import_json(&text, parent, node) };

                match made {
                    Ok(id) => o.set("id", id as f64)?,
                    Err(e) => o.set("error", e)?,
                }
            } else {
                let urls: Vec<String> = a
                    .get(2)
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter::<Coerced<String>>().filter_map(Result::ok).map(|c| c.0).collect())
                    .unwrap_or_default();

                o.set("id", rt().browser_create_from_url(urls, parent, node) as f64)?;
            }

            Ok(o.into_value())
        }
        // Browser.replaceWorld (scene)
        "replaceWorld" => {
            use x3d_runtime::browser_load::WorldScene;

            let scene = match str_arg(ctx, a, 1).as_str() {
                "scene" => WorldScene::Ctx(num_arg(ctx, a, 2) as x3d_scene::CtxId),
                "nodes" => match value_arg(a, 2) {
                    Some(FieldValue::MFNode(list)) => WorldScene::Roots(list),
                    _ => WorldScene::Empty,
                },
                _ => WorldScene::Empty,
            };
            let o = Object::new(ctx.clone())?;

            o.set("id", rt().browser_replace_world(scene) as f64)?;
            Ok(o.into_value())
        }
        "sfnodeFromString" => {
            let text = str_arg(ctx, a, 0);
            let script = num_arg(ctx, a, 1);
            let parent = ctx_of_script(script);

            match rt().sfnode_from_string(&text, parent) {
                Ok(Some(n)) => Ok(Value::new_number(ctx.clone(), n as f64)),
                Ok(None) => throw(ctx, "SFNode.new: invalid argument."),
                Err(e) => throw(ctx, &e),
            }
        }
        "addRoute" | "deleteRoute" => {
            let script = num_arg(ctx, a, 0);
            let ctx_id = ctx_of_script(script);
            let (src, dst) = (a.get(1).and_then(arg_node).flatten(), a.get(3).and_then(arg_node).flatten());
            let (sf, df) = (str_arg(ctx, a, 2), str_arg(ctx, a, 4));

            let (Some(src), Some(dst)) = (src, dst) else {
                return throw(ctx, "Bad ROUTE specification: source node must be of type X3DNode.");
            };

            if name == "addRoute" {
                if let Err(e) = rt().add_route(ctx_id, src, &sf, dst, &df) {
                    return throw(ctx, &e);
                }
            } else {
                rt().delete_route(ctx_id, src, &sf, dst, &df);
            }

            Ok(undefined)
        }
        // --- X3DScene / X3DExecutionContext (the script's own execution context)
        "sceneInfo" => {
            let script = num_arg(ctx, a, 0);
            let c = ctx_of_script(script);
            let s = &rt().scene;
            let o = Object::new(ctx.clone())?;
            let root = s.scene_root(c);
            let encoding = s.scene_infos.get(&root).map(|i| i.encoding.clone()).unwrap_or_else(|| s.encoding.clone());

            o.set("isScene", s.is_scene_root(c))?;
            o.set("encoding", encoding)?;
            match s.profile_of(c) {
                Some(p) => o.set("profile", p)?,
                None => o.set("profile", Value::new_null(ctx.clone()))?,
            }

            let comps = Array::new(ctx.clone())?;
            for (i, (n, l)) in s.components_of(c).iter().enumerate() {
                let e = Array::new(ctx.clone())?;
                e.set(0, n.as_str())?;
                e.set(1, *l)?;
                e.set(2, s.component_level_text(c, n, *l))?;
                comps.set(i, e)?;
            }
            o.set("components", comps)?;

            let units = Array::new(ctx.clone())?;
            for (i, u) in s.units_of(c).iter().enumerate() {
                let e = Array::new(ctx.clone())?;
                e.set(0, u.category.as_str())?;
                e.set(1, u.name.as_str())?;
                e.set(2, u.conversion_factor)?;
                units.set(i, e)?;
            }
            o.set("units", units)?;

            let meta = Array::new(ctx.clone())?;
            for (i, (k, v)) in s.meta_of(c).iter().enumerate() {
                let e = Array::new(ctx.clone())?;
                e.set(0, k.as_str())?;
                e.set(1, v.as_str())?;
                meta.set(i, e)?;
            }
            o.set("meta", meta)?;
            Ok(o.into_value())
        }
        "configTable" => {
            // The supported components and profiles (Configuration/SupportedComponents.js, SupportedProfiles.js) and the URL of the
            // script X_ITE runs from (Browser/Networking/URLs.js getScriptURL: the page).
            let t = rt().scene.table;
            let o = Object::new(ctx.clone())?;
            let comps = Array::new(ctx.clone())?;

            for (i, c) in t.components.iter().enumerate() {
                let e = Array::new(ctx.clone())?;
                let deps = Array::new(ctx.clone())?;

                for (j, d) in c.dependencies.iter().enumerate() {
                    deps.set(j, d.as_str())?;
                }

                e.set(0, c.name.as_str())?;
                e.set(1, c.title.as_str())?;
                e.set(2, c.level)?;
                e.set(3, c.external)?;
                e.set(4, deps)?;
                comps.set(i, e)?;
            }

            let profiles = Array::new(ctx.clone())?;

            for (i, p) in t.profiles.iter().enumerate() {
                let e = Array::new(ctx.clone())?;
                let list = Array::new(ctx.clone())?;

                for (j, (n, l)) in p.components.iter().enumerate() {
                    let pair = Array::new(ctx.clone())?;
                    pair.set(0, n.as_str())?;
                    pair.set(1, *l)?;
                    list.set(j, pair)?;
                }

                e.set(0, p.name.as_str())?;
                e.set(1, p.title.as_str())?;
                e.set(2, list)?;
                profiles.set(i, e)?;
            }

            o.set("components", comps)?;
            o.set("profiles", profiles)?;
            o.set("scriptURL", rt().page_url.as_str())?;
            Ok(o.into_value())
        }
        // X3DScene.addComponent / updateComponent: `level` is the number the scene keeps, `text` is `String (level)` of the
        // ComponentInfo the Script gave (any value; the printers write it).
        "addComponent" => {
            let script = num_arg(ctx, a, 0);
            let (name, level, text) = (str_arg(ctx, a, 1), num_arg(ctx, a, 2) as i32, str_arg(ctx, a, 3));
            let root = rt().scene.scene_root(ctx_of_script(script));
            let comps = scene_components_mut(&mut rt().scene, root);

            if comps.iter().any(|(n, _)| *n == name) {
                return throw(ctx, &format!("Couldn't add component. Component '{name}' already exists."));
            }

            comps.push((name.clone(), level));
            set_component_level_text(&mut rt().scene, root, name, level, text);
            Ok(undefined)
        }
        "updateComponent" => {
            // X3DScene.updateComponent: a component of that name is replaced where it stands, else added.
            let script = num_arg(ctx, a, 0);
            let (name, level, text) = (str_arg(ctx, a, 1), num_arg(ctx, a, 2) as i32, str_arg(ctx, a, 3));
            let root = rt().scene.scene_root(ctx_of_script(script));
            let comps = scene_components_mut(&mut rt().scene, root);

            match comps.iter_mut().find(|(n, _)| *n == name) {
                Some(c) => c.1 = level,
                None => comps.push((name.clone(), level)),
            }

            set_component_level_text(&mut rt().scene, root, name, level, text);
            Ok(undefined)
        }
        "removeComponent" => {
            let script = num_arg(ctx, a, 0);
            let name = str_arg(ctx, a, 1);
            let root = rt().scene.scene_root(ctx_of_script(script));

            scene_components_mut(&mut rt().scene, root).retain(|(n, _)| *n != name);
            set_component_level_text(&mut rt().scene, root, name, 0, "0".into());
            Ok(undefined)
        }
        "fobserve" => {
            let h = handle_at(ctx, a, 0)?;
            let on = a.get(1).and_then(|v| v.as_bool()).unwrap_or(false);

            if let Some(k) = slot_key(h) {
                if on {
                    rt().field_observed.insert(k);
                } else {
                    rt().field_observed.remove(&k);
                }
            }

            Ok(undefined)
        }
        // prelude/bridge.js: field data in and out, node fields mirrored by the Script's objects.
        "fexport" => {
            let h = handle_at(ctx, a, 0)?;
            value_data(ctx, &read(h))
        }
        "fimport" => {
            let ty_name = str_arg(ctx, a, 0);
            let ty = if ty_name == "VrmlMatrix" { FieldType::SFMatrix4f } else { FieldType::from_name(&ty_name).unwrap_or(FieldType::SFBool) };
            let v = data_value(ty, a.get(1).unwrap_or(&undefined));
            Ok(Value::new_number(ctx.clone(), encode(detached(v))))
        }
        // The Script's object of a node field changed it: with an event (X3DField.addEvent) or without (a value set in place).
        "fpush" => {
            let h = handle_at(ctx, a, 0)?;
            let event = a.get(2).and_then(|v| v.as_bool()).unwrap_or(true);

            if let Some(k) = slot_key(h) {
                let v = data_value(rt().field(k).ty(), a.get(1).unwrap_or(&undefined));

                if event {
                    rt().set_value(k, v);
                } else {
                    rt().set_raw(k, v);
                }

                rt().js_clean(k);
            }

            Ok(undefined)
        }
        "fmirror" => {
            let h = handle_at(ctx, a, 0)?;

            match slot_key(h) {
                Some(k) => {
                    rt().js_mirror.entry(k).or_insert(false);
                    Ok(rquickjs::String::from_str(ctx.clone(), &fkey_id(k))?.into_value())
                }
                None => Ok(Value::new_null(ctx.clone())),
            }
        }
        // The ids and data of the mirrored fields whose value changed since the last call.
        "fdirty" => {
            let dirty = rt().take_js_dirty();
            let mut items = vec![];

            for k in dirty {
                items.push(rquickjs::String::from_str(ctx.clone(), &fkey_id(k))?.into_value());
                items.push(value_data(ctx, rt().value(k))?);
            }

            data_array(ctx, items)
        }
        // X3DField.fromVRMLString / fromXMLString (value, scene): the data of the parsed value. `script` names the scene (the
        // Script's execution context) or is null; a value that cannot be read throws X_ITE's error.
        "parsefield" => {
            let ty_name = str_arg(ctx, a, 0);
            let ty = if ty_name == "VrmlMatrix" { FieldType::SFMatrix4f } else { FieldType::from_name(&ty_name).unwrap_or(FieldType::SFBool) };
            let text = str_arg(ctx, a, 1);
            let xml = str_arg(ctx, a, 2) == "xml";
            let name = str_arg(ctx, a, 3);
            let scene = a.get(4).and_then(|v| v.as_number()).map(ctx_of_script);
            let unit = a.get(5).and_then(|v| v.as_string()).and_then(|s| s.to_string().ok());

            match rt().field_from_string(ty, &text, scene, xml, unit, &name) {
                Ok(Some(v)) => value_data(ctx, &v),
                Ok(None) => throw(ctx, &format!("Couldn't read value for field '{name}'.")),
                Err(Some(message)) => throw(ctx, &message),
                // VRMLParser without an execution context: `executionContext .createNode (…)` of null.
                Err(None) => throw_type(ctx, "Cannot read properties of null (reading 'createNode')"),
            }
        }
        "funit" => {
            let h = handle_at(ctx, a, 0)?;

            // X3DField.getUnit: the unit category a node class gives a predefined field (setUnit in its constructor).
            let unit = slot_key(h).and_then(|k| {
                let scene = &rt().scene;
                let node = scene.node(k.0);

                match (node.kind.clone(), k.1) {
                    (NodeKind::Builtin(t), FieldLoc::Pre(_)) => {
                        let name = &rt().field(k).name;
                        let spec = scene.spec_version_of(node.ctx);
                        scene.table.types[t].variant(spec).fields.iter().find(|d| &d.name == name).and_then(|d| d.unit.clone())
                    }
                    _ => None,
                }
            });

            match unit {
                Some(u) => Ok(rquickjs::String::from_str(ctx.clone(), &u)?.into_value()),
                None => Ok(undefined),
            }
        }
        // X3DGenerator createFloatFormat / createFloatExponentialFormat: Intl.NumberFormat ("en", { notation, maximumSignificantDigits }).
        "intlfmt" => {
            let scientific = a.first().and_then(|v| v.as_bool()).unwrap_or(false);
            let precision = num_arg(ctx, a, 1) as usize;
            let value = num_arg(ctx, a, 2);
            let text = if scientific { x3d_fields::format::intl_scientific_lower(value, precision) } else { x3d_fields::format::intl_standard(value, precision) };
            Ok(rquickjs::String::from_str(ctx.clone(), &text)?.into_value())
        }
        "ctxObserve" => {
            rt().ctx_events.observed = true;
            rt().ctx_events.watched.insert(num_arg(ctx, a, 0) as usize);

            Ok(undefined)
        }
        "updateUnit" => {
            // X3DScene.updateUnit (category, name, conversionFactor): only a category the scene has; the name is a string, the factor a number.
            let script = num_arg(ctx, a, 0);
            let (category, name) = (str_arg(ctx, a, 1), str_arg(ctx, a, 2));
            let factor = num_arg(ctx, a, 3);
            let root = rt().scene.scene_root(ctx_of_script(script));
            let units = match rt().scene.scene_infos.get_mut(&root) {
                Some(i) => &mut i.units,
                None => &mut rt().scene.units,
            };

            if let Some(u) = units.iter_mut().find(|u| u.category == category) {
                u.name = name;
                u.conversion_factor = factor;
            }

            Ok(undefined)
        }
        "setProfile" => {
            let script = num_arg(ctx, a, 0);
            let profile = a.get(1).filter(|v| v.is_string()).map(|_| str_arg(ctx, a, 1));
            let root = rt().scene.scene_root(ctx_of_script(script));

            match rt().scene.scene_infos.get_mut(&root) {
                Some(i) => i.profile = profile,
                None => rt().scene.profile = profile,
            }

            rt().ctx_event(root, CtxField::Profile, 1);
            Ok(undefined)
        }
        "metaData" => {
            // X3DScene.setMetaData / addMetaData / removeMetaData.
            let script = num_arg(ctx, a, 0);
            let (mode, name) = (str_arg(ctx, a, 1), str_arg(ctx, a, 2));
            let values: Vec<String> = match a.get(3).and_then(|v| v.as_array()) {
                Some(arr) => arr.iter::<Value>().filter_map(|v| v.ok()).map(|v| coerce_string(ctx, &v)).collect(),
                None => vec![],
            };

            if mode != "remove" && name.is_empty() {
                return throw(ctx, "Couldn't add metadata: name is empty.");
            }

            let root = rt().scene.scene_root(ctx_of_script(script));
            let meta = scene_meta_mut(&mut rt().scene, root);

            match mode.as_str() {
                "set" => {
                    let index = meta.iter().position(|(k, _)| *k == name);
                    meta.retain(|(k, _)| *k != name);
                    let index = index.unwrap_or(meta.len());

                    for (i, v) in values.into_iter().enumerate() {
                        meta.insert(index + i, (name.clone(), v));
                    }
                }
                "add" => meta.extend(values.into_iter().map(|v| (name.clone(), v))),
                _ => meta.retain(|(k, _)| *k != name),
            }

            rt().ctx_event(root, CtxField::Metadata, 1);
            Ok(undefined)
        }
        "routeList" => {
            let script = num_arg(ctx, a, 0);
            let c = ctx_of_script(script);
            let routes: Vec<_> = rt().scene.ctx(c).routes.iter().map(|r| (r.source_node, r.source_field.clone(), r.destination_node, r.destination_field.clone())).collect();
            let arr = Array::new(ctx.clone())?;

            for (i, (sn, sf, dn, df)) in routes.into_iter().enumerate() {
                let o = Object::new(ctx.clone())?;
                o.set("sourceNode", node_js(ctx, Some(sn)))?;
                o.set("sourceField", sf)?;
                o.set("destinationNode", node_js(ctx, Some(dn)))?;
                o.set("destinationField", df)?;
                arr.set(i, o)?;
            }

            Ok(arr.into_value())
        }
        "rootNode" => {
            // X3DScene.addRootNode / removeRootNode.
            let script = num_arg(ctx, a, 0);
            let add = str_arg(ctx, a, 1) == "add";
            let node = a.get(2).and_then(arg_node).flatten();
            let c = ctx_of_script(script);
            let roots = &mut rt().scene.ctx_mut(c).root_nodes;

            if add {
                if roots.contains(&node) {
                    return Ok(undefined);
                }

                roots.push(node);
            } else {
                roots.retain(|n| *n != node);
            }

            rt().rebuild_parents();

            // rootNodes .push (node) / .assign (...): the rootNodes field fires.
            let rounds = rt().root_nodes_rounds(c);
            rt().ctx_event(c, CtxField::SceneGraph, rounds);
            rt().ctx_event(c, CtxField::BBox, rounds);
            Ok(undefined)
        }
        "exportedNode" => {
            // X3DScene.addExportedNode / getExportedNode.
            let script = num_arg(ctx, a, 0);
            let (mode, name) = (str_arg(ctx, a, 1), str_arg(ctx, a, 2));
            let root = rt().scene.scene_root(ctx_of_script(script));

            if mode == "get" {
                return match rt().scene.get_exported_node(root, &name) {
                    Some(n) => Ok(node_js(ctx, Some(n))),
                    None => throw(ctx, &format!("Exported node '{name}' not found.")),
                };
            }

            if rt().scene.get_exported_node(root, &name).is_some() {
                return throw(ctx, &format!("Couldn't add exported node: exported name '{name}' already in use."));
            }

            let Some(node) = a.get(3).and_then(arg_node).flatten() else {
                return throw(ctx, "Couldn't update exported node: node must be of type X3DNode.");
            };

            match rt().scene.update_exported_node(root, &name, node, "") {
                Ok(()) => Ok(undefined),
                Err(e) => throw(ctx, &e),
            }
        }
        "nodeArrays" => {
            // namedNodes (SFNode), importedNodes (X3DImportedNode), exportedNodes (X3DExportedNode).
            let script = num_arg(ctx, a, 0);
            let which = str_arg(ctx, a, 1);
            let c = ctx_of_script(script);
            let s = &rt().scene;
            let arr = Array::new(ctx.clone())?;

            match which.as_str() {
                "named" => {
                    for (i, &n) in s.ctx(c).named_nodes.values().iter().enumerate() {
                        arr.set(i, node_js(ctx, Some(n)))?;
                    }
                }
                "imported" => {
                    for (i, &id) in s.ctx(c).imported.values().iter().enumerate() {
                        let m = &s.imported[id];
                        let o = Object::new(ctx.clone())?;
                        o.set("id", id as f64)?;
                        o.set("inlineNode", node_js(ctx, Some(m.inline)))?;
                        o.set("exportedName", m.exported_name.as_str())?;
                        o.set("importedName", m.imported_name.as_str())?;
                        o.set("description", m.description.as_str())?;
                        arr.set(i, o)?;
                    }
                }
                _ => {
                    let root = s.scene_root(c);
                    for (i, &id) in s.ctx(root).exported.values().iter().enumerate() {
                        let m = &s.exported[id];
                        let o = Object::new(ctx.clone())?;
                        o.set("id", id as f64)?;
                        o.set("exportedName", m.exported_name.as_str())?;
                        o.set("description", m.description.as_str())?;
                        o.set("localNode", node_js(ctx, Some(m.local)))?;
                        arr.set(i, o)?;
                    }
                }
            }

            Ok(arr.into_value())
        }
        "sceneString" => {
            let script = num_arg(ctx, a, 0);
            let root = rt().scene.scene_root(ctx_of_script(script));
            let format = str_arg(ctx, a, 1);
            if format != "xml" {
                rt().ctx_streamed(root);
            }
            let text = match format.as_str() {
                "xml" => rt().scene.to_xml_string_ctx(root),
                "json" => rt().scene.to_json_string_ctx(root),
                _ => rt().scene.to_vrml_string_ctx(root),
            };
            Ok(rquickjs::String::from_str(ctx.clone(), &text)?.into_value())
        }
        "worldInfos" => {
            let script = num_arg(ctx, a, 0);
            let c = ctx_of_script(script);
            let list: Vec<Option<NodeId>> = rt().world_infos.get(&c).map(|l| l.iter().map(|n| Some(*n)).collect()).unwrap_or_default();
            Ok(new_value_js(ctx, FieldValue::MFNode(list)))
        }
        "rootNodes" => {
            let script = num_arg(ctx, a, 0);
            let c = ctx_of_script(script);
            let roots = rt().scene.ctx(c).root_nodes.clone();
            Ok(new_value_js(ctx, FieldValue::MFNode(roots)))
        }
        "createNode" => {
            let script = num_arg(ctx, a, 0);
            let ty = str_arg(ctx, a, 1);
            let c = ctx_of_script(script);

            match rt().create_node_script(c, &ty) {
                Some(n) => {
                    // The new node's parents change (X3DBaseNode.parentsChanged): its context's sceneGraph_changed.
                    rt().ctx_event(c, CtxField::SceneGraph, 1);
                    rt().ctx_event(c, CtxField::BBox, 1);
                    Ok(node_js(ctx, Some(n)))
                }
                None => throw(ctx, &format!("Unknown node type '{ty}'.")),
            }
        }
        "createProto" => {
            let script = num_arg(ctx, a, 0);
            let ty = str_arg(ctx, a, 1);
            let c = ctx_of_script(script);

            match rt().create_proto_setup(c, &ty) {
                Some(n) => Ok(node_js(ctx, Some(n))),
                None => throw(ctx, &format!("Unknown proto or externproto type '{ty}'.")),
            }
        }
        "namedNode" => {
            let script = num_arg(ctx, a, 0);
            let nn = str_arg(ctx, a, 1);
            let c = ctx_of_script(script);

            match rt().scene.get_named_node(c, &nn) {
                Some(n) => Ok(node_js(ctx, Some(n))),
                None => throw(ctx, &format!("Named node '{nn}' not found.")),
            }
        }
        other => throw(ctx, &format!("x3d-script: unknown native op '{other}'")),
    }
}

/// The metadata list of the scene whose root context is `root` (the world's lives on Scene itself).
fn scene_meta_mut(s: &mut x3d_scene::Scene, root: x3d_scene::CtxId) -> &mut Vec<(String, String)> {
    if s.scene_infos.contains_key(&root) {
        &mut s.scene_infos.get_mut(&root).unwrap().meta
    } else {
        &mut s.meta
    }
}

/// The COMPONENT list of the scene whose root context is `root`.
fn scene_components_mut(s: &mut x3d_scene::Scene, root: x3d_scene::CtxId) -> &mut Vec<(String, i32)> {
    if s.scene_infos.contains_key(&root) {
        &mut s.scene_infos.get_mut(&root).unwrap().components
    } else {
        &mut s.components
    }
}

/// The level text of a component (see `Scene::component_level_texts`); none when it is the number's own.
fn set_component_level_text(s: &mut x3d_scene::Scene, root: x3d_scene::CtxId, name: String, level: i32, text: String) {
    let texts = match s.scene_infos.get_mut(&root) {
        Some(i) => &mut i.component_level_texts,
        None => &mut s.component_level_texts,
    };

    if text == level.to_string() {
        texts.remove(&name);
    } else {
        texts.insert(name, text);
    }
}

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
        "pow" => Function::new(ctx.clone(), |x: Coerced<f64>, y: Coerced<f64>| jsmath::pow(x.0, y.0))?.into_value(),
        // V8 MathHypot: every argument is converted with ToNumber first.
        "hypot" => Function::new(ctx.clone(), |a: Rest<Coerced<f64>>| jsmath::hypot(&a.0.iter().map(|x| x.0).collect::<Vec<_>>()))?.into_value(),
        "atan2" => Function::new(ctx.clone(), |y: Coerced<f64>, x: Coerced<f64>| jsmath::atan2(y.0, x.0))?.into_value(),
        _ => Value::new_null(ctx.clone()),
    })
}

#[cfg(test)]
mod slot_tests {
    use super::*;

    /// A handle kept from before its slot was released does not reach the value that takes the slot.
    #[test]
    fn a_stale_handle_does_not_reach_a_reused_slot() {
        let _host = QuickJsHost::new();
        let h = detached(FieldValue::SFInt32(1));
        let old = encode(h);

        with_state(|s| s.owners[h as usize] += 1);
        with_state(State::release_fresh);
        assert_eq!(decode(old), Some(h), "an owned slot stays");

        with_state(|s| {
            s.owners[h as usize] -= 1;
            s.try_free(h);
        });
        assert_eq!(decode(old), None, "a released slot rejects its handle");

        let n = detached(FieldValue::SFInt32(2));

        assert_eq!(n, h, "the slot is taken again");
        assert_eq!(decode(old), None, "the old handle does not reach the new value");
        assert!(matches!(decode(encode(n)).map(read), Some(FieldValue::SFInt32(2))), "the new handle works");
    }

    /// An element view keeps its MF slot; the MF slot goes after its last element.
    #[test]
    fn an_element_keeps_its_parent() {
        let _host = QuickJsHost::new();
        let mf = detached(FieldValue::MFInt32(vec![5, 6]));
        let e = element(mf, 1);

        with_state(|s| s.owners[e as usize] += 1);
        with_state(State::release_fresh);
        assert!(decode(encode(mf)).is_some(), "the parent of a live element stays");
        assert!(matches!(read(e), FieldValue::SFInt32(6)));

        let old = encode(mf);

        with_state(|s| {
            s.owners[e as usize] -= 1;
            s.try_free(e);
        });
        assert_eq!(decode(old), None, "the parent goes with its last element");
        assert_eq!(slot_stats().free, 2);
    }
}
