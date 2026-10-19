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
use x3d_runtime::*;
use x3d_scene::*;

mod ctx_api;

const PRELUDE: &str = include_str!("prelude.js");
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

        let fields: Vec<(String, u8, f64)> = (0..rt.scene.node(node).user_fields.len())
            .map(|i| {
                let f = &rt.scene.node(node).user_fields[i];
                (f.name.clone(), f.access, encode(attach((node, FieldLoc::User(i)))))
            })
            .collect();

        self.context.with(|ctx| {
            let create: Function = ctx.globals().get("__create").unwrap();
            let list = Array::new(ctx.clone()).unwrap();

            for (i, (name, a, h)) in fields.iter().enumerate() {
                let entry = Array::new(ctx.clone()).unwrap();
                entry.set(0, name.as_str()).unwrap();
                entry.set(1, *a as i32).unwrap();
                entry.set(2, *h).unwrap();
                list.set(i, entry).unwrap();
            }

            let cbs: Vec<&str> = callbacks.iter().map(String::as_str).collect();

            let result = match create.call::<_, Vec<bool>>((node, source, cbs, list)) {
                Ok(v) => Ok(v),
                Err(e) => Err(ScriptError(describe(&ctx, e))),
            };

            while ctx.execute_pending_job() {}
            with_state(State::release_fresh);
            result
        })
    }

    fn call(&mut self, rt: &mut Runtime, node: NodeId, call: ScriptCall) -> Result<(), ScriptError> {
        self.enter(rt);

        let time = rt.now;

        self.context.with(|ctx| {
            let f: Function = ctx.globals().get("__call").unwrap();

            let result: rquickjs::Result<Value> = match call {
                ScriptCall::Plain(name) => f.call((node, name, Value::new_undefined(ctx.clone()), time, false)),
                ScriptCall::Field(name, value) => {
                    let h = detached(value);
                    let v = field_value_js(&ctx, h);
                    f.call((node, name, v, time, true))
                }
            };

            // Run pending promise jobs (async callbacks continue as microtasks).
            while ctx.execute_pending_job() {}

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
            if let Ok(f) = ctx.globals().get::<_, Function>("__settle") {
                let _ = match outcome {
                    Ok(c) => f.call::<_, ()>((id as f64, true, c as f64)),
                    Err(m) => f.call::<_, ()>((id as f64, false, m)),
                };
            }

            while ctx.execute_pending_job() {}
            with_state(State::release_fresh);
        });
    }

    fn destroy_context(&mut self, node: NodeId) {
        self.select();
        self.destroy(node);
        self.collect();
    }

    fn held_nodes(&mut self, rt: &mut Runtime) -> Option<Vec<NodeId>> {
        self.enter(rt);
        self.collect();

        let mut held: Vec<NodeId> = self.context.with(|ctx| match ctx.globals().get::<_, Function>("__heldNodes") {
            Ok(f) => f.call::<_, Vec<f64>>(()).ok().map(|ids| ids.into_iter().map(|i| i as NodeId).collect()),
            Err(_) => None,
        })?;

        // Values that live JavaScript objects hold (`new MFNode (a, b)`, an SFNode field object), and the
        // node whose field a live field object is.
        with_state(|s| {
            for (i, slot) in s.slots.iter().enumerate() {
                if s.owners[i] == 0 && s.children[i] == 0 {
                    continue;
                }

                match slot {
                    Slot::Value(FieldValue::SFNode(Some(n))) => held.push(*n),
                    Slot::Value(FieldValue::MFNode(list)) => held.extend(list.iter().flatten().copied()),
                    Slot::Field(k) => held.push(k.0),
                    _ => {}
                }
            }
        });

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
        FieldType::SFVec4d => FieldValue::SFVec4d(Vector4::new(g(0, 0.0), g(1, 0.0), g(2, 0.0), g(3, 0.0))),
        FieldType::SFVec4f => FieldValue::SFVec4f(Vector4::new(g(0, 0.0), g(1, 0.0), g(2, 0.0), g(3, 0.0))),
        FieldType::SFRotation => FieldValue::SFRotation(Rotation4::new(g(0, 0.0), g(1, 0.0), g(2, 1.0), g(3, 0.0))),
        FieldType::SFQuaternion => FieldValue::SFQuaternion(Quaternion::new(g(0, 0.0), g(1, 0.0), g(2, 0.0), g(3, 1.0))),
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
pub fn mf_set(mf: &mut FieldValue, i: usize, sf: &FieldValue) {
    let st = sf_type_of(mf.field_type());

    while mf.len() <= i {
        mf_push(mf, &FieldValue::default_for(st));
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

fn mf_truncate(mf: &mut FieldValue, n: usize) {
    let c = mf.field_type().components();

    match mf {
        FieldValue::MFBool(v) => v.truncate(n),
        FieldValue::MFInt32(v) => v.truncate(n),
        FieldValue::MFString(v) => v.truncate(n),
        FieldValue::MFNode(v) => v.truncate(n),
        FieldValue::MFImage(v) => v.truncate(n),
        FieldValue::MFFloat(v) | FieldValue::MFColor(v) | FieldValue::MFColorRGBA(v) | FieldValue::MFVec2f(v) | FieldValue::MFVec3f(v)
        | FieldValue::MFVec4f(v) | FieldValue::MFMatrix3f(v) | FieldValue::MFMatrix4f(v) => v.truncate(n * c),
        FieldValue::MFDouble(v) | FieldValue::MFTime(v) | FieldValue::MFVec2d(v) | FieldValue::MFVec3d(v) | FieldValue::MFVec4d(v)
        | FieldValue::MFMatrix3d(v) | FieldValue::MFMatrix4d(v) | FieldValue::MFQuaternion(v) | FieldValue::MFRotation(v) => v.truncate(n * c),
        _ => {}
    }
}

fn is_scalar(t: FieldType) -> bool {
    matches!(t, FieldType::SFBool | FieldType::SFInt32 | FieldType::SFFloat | FieldType::SFDouble | FieldType::SFTime | FieldType::SFString)
}

/// `field .toString ()` (X3DObject.toString with the default generator).
fn to_vrml(v: &FieldValue) -> String {
    let mut g = Generator::default();

    match v {
        FieldValue::SFNode(Some(n)) => rt().scene.node_to_vrml_string(*n),
        FieldValue::SFNode(None) => "NULL".into(),
        _ => {
            v.to_vrml_stream(&mut g);
            g.string
        }
    }
}

// --- JS conversions --------------------------------------------------------------------------

fn obj_num<'js>(v: &Value<'js>, key: &str) -> Option<f64> {
    let o = v.as_object()?;
    let x: Value = o.get(key).ok()?;

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
        return None;
    }

    let x: Value = o.get("__n").ok()?;
    Some(x.as_number().map(|n| n as NodeId))
}

fn coerce_f64(ctx: &Ctx<'_>, v: &Value<'_>) -> f64 {
    if let Some(h) = arg_handle(v) {
        return sf_numbers(&read(h)).first().copied().unwrap_or(f64::NAN);
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
        FieldValue::SFFloat(x) | FieldValue::SFDouble(x) | FieldValue::SFTime(x) => Value::new_number(ctx.clone(), *x),
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

fn str_arg(ctx: &Ctx<'_>, args: &[Value<'_>], i: usize) -> String {
    args.get(i).map(|v| coerce_string(ctx, v)).unwrap_or_default()
}

fn num_arg(ctx: &Ctx<'_>, args: &[Value<'_>], i: usize) -> f64 {
    args.get(i).map(|v| coerce_f64(ctx, v)).unwrap_or(f64::NAN)
}

/// The execution context a script-side `script` argument names: a Script node's id (its context), or
/// `-(context + 1)` (-1 is the world, below that a scene made by createX3DFromString / createX3DFromURL).
fn ctx_of_script(x: f64) -> x3d_scene::CtxId {
    if x >= 0.0 { rt().scene.node(x as NodeId).ctx } else { (-(x + 1.0)) as x3d_scene::CtxId }
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
        "mathfn" => math_fn(ctx, &str_arg(ctx, a, 0)),
        "new" => {
            let ty_name = str_arg(ctx, a, 0);
            let v = construct(ctx, &ty_name, &a[1.min(a.len())..])?;
            Ok(Value::new_number(ctx.clone(), encode(detached(v))))
        }
        // prelude.js: a JavaScript object took the handle, or one that held it was freed.
        "own" => {
            if let Some(h) = a.first().and_then(|v| v.as_number()).and_then(decode) {
                with_state(|s| s.owners[h as usize] += 1);
            }

            Ok(undefined)
        }
        "disown" => {
            if let Some(h) = a.first().and_then(|v| v.as_number()).and_then(decode) {
                with_state(|s| {
                    s.owners[h as usize] = s.owners[h as usize].saturating_sub(1);
                    s.try_free(h);
                });
            }

            Ok(undefined)
        }
        "fv" => {
            let h = handle_at(ctx, a, 0)?;
            Ok(field_value_js(ctx, h))
        }
        "fname" => {
            let h = handle_at(ctx, a, 0)?;
            let n = slot_key(h).map(|k| rt().field(k).name.clone()).unwrap_or_default();
            Ok(rquickjs::String::from_str(ctx.clone(), &n)?.into_value())
        }
        "faccess" => {
            let h = handle_at(ctx, a, 0)?;
            let n = slot_key(h).map(|k| rt().field(k).access).unwrap_or(access::INPUT_OUTPUT);
            Ok(Value::new_int(ctx.clone(), n as i32))
        }
        "set" => {
            let h = handle_at(ctx, a, 0)?;
            let ty = read(h).field_type();

            if let Some(v) = a.get(1).and_then(|v| convert(ctx, ty, v)) {
                write(h, v);
            }

            Ok(undefined)
        }
        "copy" => {
            let h = handle_at(ctx, a, 0)?;
            Ok(new_value_js(ctx, read(h)))
        }
        "eq" => {
            let h = handle_at(ctx, a, 0)?;
            let other = value_arg(a, 1);
            Ok(Value::new_bool(ctx.clone(), other.is_some_and(|o| read(h).equals(&o))))
        }
        "isdef" => {
            let h = handle_at(ctx, a, 0)?;
            Ok(Value::new_bool(ctx.clone(), read(h).is_default_value()))
        }
        "str" => {
            let h = handle_at(ctx, a, 0)?;
            Ok(rquickjs::String::from_str(ctx.clone(), &to_vrml(&read(h)))?.into_value())
        }
        "g" => {
            let h = handle_at(ctx, a, 0)?;
            component_get(ctx, h, a.get(1))
        }
        "s" => {
            let h = handle_at(ctx, a, 0)?;
            component_set(ctx, h, a.get(1), a.get(2));
            Ok(undefined)
        }
        "m" => {
            let h = handle_at(ctx, a, 0)?;
            let m = str_arg(ctx, a, 1);
            method(ctx, h, &m, &a[2.min(a.len())..])
        }
        "len" => {
            let h = handle_at(ctx, a, 0)?;
            Ok(Value::new_int(ctx.clone(), read(h).len() as i32))
        }
        "setlen" => {
            let h = handle_at(ctx, a, 0)?;
            let n = num_arg(ctx, a, 1) as usize;
            let mut v = read(h);
            let st = sf_type_of(v.field_type());

            if n < v.len() {
                mf_truncate(&mut v, n);
            } else {
                while v.len() < n {
                    mf_push(&mut v, &FieldValue::default_for(st));
                }
            }

            write(h, v);
            Ok(undefined)
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
        "put" => {
            let h = handle_at(ctx, a, 0)?;
            let i = num_arg(ctx, a, 1) as usize;
            let mut v = read(h);
            let st = sf_type_of(v.field_type());

            if let Some(sf) = a.get(2).and_then(|x| convert(ctx, st, x)) {
                mf_set(&mut v, i, &sf);
                write(h, v);
            }

            Ok(undefined)
        }
        // --- nodes
        "nget" | "nfield" => {
            let id = inner_node(num_arg(ctx, a, 0) as NodeId);
            let field = str_arg(ctx, a, 1);

            match rt().get_field(id, &field) {
                Some(k) => {
                    if name == "nget" && rt().field(k).access == access::INPUT_ONLY {
                        return Ok(undefined);
                    }

                    let h = attach(k);

                    if name == "nfield" {
                        return Ok(handle_js(ctx, h, rt().field(k).ty().name(), true));
                    }

                    Ok(field_value_js(ctx, h))
                }
                None => Ok(rquickjs::String::from_str(ctx.clone(), NOFIELD)?.into_value()),
            }
        }
        "nset" => {
            let id = inner_node(num_arg(ctx, a, 0) as NodeId);
            let field = str_arg(ctx, a, 1);

            match rt().get_field(id, &field) {
                Some(k) => {
                    if rt().field(k).access == access::OUTPUT_ONLY {
                        return Ok(Value::new_int(ctx.clone(), 2));
                    }

                    let h = attach(k);
                    let ty = rt().field(k).ty();

                    if let Some(v) = a.get(2).and_then(|x| convert(ctx, ty, x)) {
                        write(h, v);
                    }

                    Ok(Value::new_int(ctx.clone(), 1))
                }
                None => Ok(Value::new_int(ctx.clone(), 0)),
            }
        }
        "nhas" => {
            let id = inner_node(num_arg(ctx, a, 0) as NodeId);
            let field = str_arg(ctx, a, 1);
            Ok(Value::new_bool(ctx.clone(), rt().get_field(id, &field).is_some()))
        }
        "ntype" => {
            let id = inner_node(num_arg(ctx, a, 0) as NodeId);
            Ok(rquickjs::String::from_str(ctx.clone(), &rt().type_name(id))?.into_value())
        }
        "nname" => {
            let id = num_arg(ctx, a, 0) as NodeId;
            Ok(rquickjs::String::from_str(ctx.clone(), &rt().scene.node(id).name)?.into_value())
        }
        "nstr" => {
            let id = num_arg(ctx, a, 0) as NodeId;
            Ok(rquickjs::String::from_str(ctx.clone(), &rt().scene.node_to_vrml_string(id))?.into_value())
        }
        "nfields" => {
            let id = inner_node(num_arg(ctx, a, 0) as NodeId);
            let arr = Array::new(ctx.clone())?;
            let n = rt().scene.node(id);

            for (i, f) in n.fields.iter().chain(n.user_fields.iter()).enumerate() {
                let o = Object::new(ctx.clone())?;
                o.set("name", f.name.as_str())?;
                o.set("accessType", f.access as i32)?;
                o.set("dataType", f.ty().name())?;
                arr.set(i, o)?;
            }

            Ok(arr.into_value())
        }
        // --- browser
        "bname" => Ok(rquickjs::String::from_str(ctx.clone(), &rt().scene.table.browser_name)?.into_value()),
        "bversion" => Ok(rquickjs::String::from_str(ctx.clone(), &rt().scene.table.browser_version)?.into_value()),
        "bdescription" => Ok(rquickjs::String::from_str(ctx.clone(), &with_state(|s| s.description.clone()))?.into_value()),
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

            if str_arg(ctx, a, 1) == "string" {
                match rt().browser_create_from_string(&str_arg(ctx, a, 2), parent, node) {
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
            o.set("profile", s.profile_of(c).map(|p| p.to_string()))?;

            let comps = Array::new(ctx.clone())?;
            for (i, (n, l)) in s.components_of(c).iter().enumerate() {
                let e = Array::new(ctx.clone())?;
                e.set(0, n.as_str())?;
                e.set(1, *l)?;
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
        "profileComponents" => {
            // ProfileInfo.components of a named profile (`[name, level]` pairs), null if unknown.
            let name = str_arg(ctx, a, 0);
            let Some(p) = rt().scene.table.profile(&name) else {
                return Ok(Value::new_null(ctx.clone()));
            };
            let arr = Array::new(ctx.clone())?;
            for (i, (n, l)) in p.components.iter().enumerate() {
                let e = Array::new(ctx.clone())?;
                e.set(0, n.as_str())?;
                e.set(1, *l)?;
                arr.set(i, e)?;
            }
            Ok(arr.into_value())
        }
        "hasComponent" => {
            // X3DScene.hasComponent: the profile's (Full without one) or the scene's own components.
            let script = num_arg(ctx, a, 0);
            let (name, level) = (str_arg(ctx, a, 1), num_arg(ctx, a, 2) as i32);
            let c = ctx_of_script(script);
            let s = &rt().scene;
            let profile = s.profile_of(c).unwrap_or("Full");
            let in_profile = s.table.profile(profile).is_some_and(|p| p.components.iter().any(|(n, l)| *n == name && level <= *l));
            let in_scene = s.components_of(c).iter().any(|(n, l)| *n == name && level <= *l);
            Ok(Value::new_bool(ctx.clone(), in_profile || in_scene))
        }
        "browserComponent" => {
            // X3DBrowser.getComponent: the supported component, its level clamped to 1 … supported.
            let (name, level) = (str_arg(ctx, a, 0), num_arg(ctx, a, 1) as i32);
            match rt().scene.supported_component_level(&name) {
                Some(max) => {
                    let e = Array::new(ctx.clone())?;
                    e.set(0, name.as_str())?;
                    e.set(1, if level == 0 { max } else { level.clamp(1, max) })?;
                    Ok(e.into_value())
                }
                None => throw(ctx, &format!("Component '{name}' at level '{level}' is not supported.")),
            }
        }
        "addComponent" => {
            let script = num_arg(ctx, a, 0);
            let (name, level) = (str_arg(ctx, a, 1), num_arg(ctx, a, 2) as i32);
            let root = rt().scene.scene_root(ctx_of_script(script));
            let comps = scene_components_mut(&mut rt().scene, root);

            if comps.iter().any(|(n, _)| *n == name) {
                return throw(ctx, &format!("Couldn't add component. Component '{name}' already exists."));
            }

            comps.push((name, level));
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

            Ok(undefined)
        }
        "protoDecls" => {
            // protos / externprotos: name, isExternProto, fields (FieldDefinitionArray), urls.
            let script = num_arg(ctx, a, 0);
            let extern_ = a.get(1).is_some_and(coerce_bool);
            let c = ctx_of_script(script);
            let arr = Array::new(ctx.clone())?;
            let s = &rt().scene;
            let decls: Vec<(String, Vec<x3d_scene::Field>, bool, Option<Vec<String>>)> = if extern_ {
                s.ctx(c).externprotos.values().iter().map(|&e| {
                    let d = &s.externprotos[e];
                    (d.name.clone(), d.fields.clone(), d.has_metadata, Some(d.url.clone()))
                }).collect()
            } else {
                s.ctx(c).protos.values().iter().map(|&p| {
                    let d = &s.protos[p];
                    (d.name.clone(), d.fields.clone(), d.has_metadata, None)
                }).collect()
            };

            for (i, (name, fields, has_metadata, urls)) in decls.into_iter().enumerate() {
                let o = Object::new(ctx.clone())?;
                o.set("name", name.as_str())?;
                o.set("isExternProto", urls.is_some())?;

                let fa = Array::new(ctx.clone())?;
                let mut defs: Vec<(String, i32, &'static str, FieldValue)> = vec![];

                if has_metadata {
                    defs.push(("metadata".into(), access::INPUT_OUTPUT as i32, "SFNode", FieldValue::SFNode(None)));
                }

                defs.extend(fields.iter().map(|f| (f.name.clone(), f.access as i32, f.ty().name(), f.value.clone())));

                for (j, (fname, access, ty, value)) in defs.into_iter().enumerate() {
                    let fo = Object::new(ctx.clone())?;
                    fo.set("name", fname.as_str())?;
                    fo.set("accessType", access)?;
                    fo.set("dataType", ty)?;
                    fo.set("value", new_value_js(ctx, value))?;
                    fa.set(j, fo)?;
                }

                o.set("fields", fa)?;

                if let Some(urls) = urls {
                    o.set("urls", new_value_js(ctx, FieldValue::MFString(urls)))?;
                }

                arr.set(i, o)?;
            }

            Ok(arr.into_value())
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
            Ok(undefined)
        }
        "namedNodeOp" => {
            // X3DExecutionContext.addNamedNode / removeNamedNode.
            let script = num_arg(ctx, a, 0);
            let (mode, name) = (str_arg(ctx, a, 1), str_arg(ctx, a, 2));
            let c = ctx_of_script(script);

            if mode == "remove" {
                rt().scene.remove_named_node(c, &name);
                return Ok(undefined);
            }

            let Some(node) = a.get(3).and_then(arg_node).flatten() else {
                return throw(ctx, "Couldn't add named node: node must be of type X3DNode.");
            };

            if rt().scene.get_named_node(c, &name).is_some() {
                return throw(ctx, &format!("Couldn't add named node: node name '{name}' is already in use."));
            }

            // X3DExecutionContext.updateNamedNode checks.
            if rt().scene.is_imported(node) {
                return throw(ctx, "Couldn't add named node: node cannot be of type X3DImportedNodeInstance.");
            }

            if rt().scene.node(node).ctx != c {
                return throw(ctx, "Couldn't update named node: node does not belong to this execution context.");
            }

            if name.is_empty() {
                return throw(ctx, "Couldn't update named node: node name is empty.");
            }

            rt().scene.update_named_node(c, &name, node);
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
            let text = match str_arg(ctx, a, 1).as_str() {
                "xml" if root == x3d_scene::ROOT => rt().scene.to_xml_string(),
                "json" if root == x3d_scene::ROOT => rt().scene.to_json_string(),
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
                Some(n) => Ok(node_js(ctx, Some(n))),
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
        "atan2" => Function::new(ctx.clone(), |y: Coerced<f64>, x: Coerced<f64>| jsmath::atan2(y.0, x.0))?.into_value(),
        _ => Value::new_null(ctx.clone()),
    })
}

/// Field constructors (`new SFVec3f (1, 2, 3)` …).
fn construct<'js>(ctx: &Ctx<'js>, ty_name: &str, a: &[Value<'js>]) -> rquickjs::Result<FieldValue> {
    let ty = if ty_name == "VrmlMatrix" { FieldType::SFMatrix4f } else { FieldType::from_name(ty_name).unwrap_or(FieldType::SFBool) };
    let nums = |n: usize| -> Vec<f64> { (0..n.min(a.len())).map(|i| coerce_f64(ctx, &a[i])).collect() };

    Ok(match ty {
        FieldType::SFBool => FieldValue::SFBool(a.first().map(coerce_bool).unwrap_or(false)),
        FieldType::SFString => FieldValue::SFString(a.first().map(|v| coerce_string(ctx, v)).unwrap_or_default()),
        FieldType::SFNode => FieldValue::SFNode(a.first().and_then(arg_node).flatten()),
        FieldType::SFRotation => {
            let first = value_arg(a, 0);
            let second = value_arg(a, 1);

            match (first, second) {
                (Some(FieldValue::SFVec3f(from) | FieldValue::SFVec3d(from)), Some(FieldValue::SFVec3f(to) | FieldValue::SFVec3d(to))) => {
                    let mut r = Rotation4::identity();
                    r.set_vectors(&from, &to);
                    FieldValue::SFRotation(r)
                }
                (Some(FieldValue::SFVec3f(axis) | FieldValue::SFVec3d(axis)), _) => {
                    FieldValue::SFRotation(Rotation4::new(axis.x, axis.y, axis.z, num_arg(ctx, a, 1)))
                }
                (Some(FieldValue::SFMatrix3f(m) | FieldValue::SFMatrix3d(m)), _) => {
                    let mut r = Rotation4::identity();
                    r.set_matrix(&m);
                    FieldValue::SFRotation(r)
                }
                _ => {
                    // function SFRotation (x = 0, y = 0, z = 1, angle = 0): +undefined is a default.
                    let g = |i: usize, d: f64| if i < a.len() && !a[i].is_undefined() { coerce_f64(ctx, &a[i]) } else { d };
                    FieldValue::SFRotation(Rotation4::new(g(0, 0.0), g(1, 0.0), g(2, 1.0), g(3, 0.0)))
                }
            }
        }
        FieldType::SFImage => {
            let (w, h, c) = (num_arg(ctx, a, 0), num_arg(ctx, a, 1), num_arg(ctx, a, 2));
            let arr = match value_arg(a, 3) {
                Some(FieldValue::MFInt32(v)) => v,
                _ => vec![],
            };
            let (w, h, c) = (if w.is_nan() { 0.0 } else { w }, if h.is_nan() { 0.0 } else { h }, if c.is_nan() { 0.0 } else { c });
            FieldValue::SFImage(Image::new(w, h, c, arr))
        }
        t if t.is_multi() => {
            let st = sf_type_of(t);
            let mut mf = FieldValue::default_for(t);

            for v in a {
                let sf = convert(ctx, st, v).unwrap_or_else(|| FieldValue::default_for(st));
                mf_push(&mut mf, &sf);
            }

            mf
        }
        t => {
            // Missing arguments take the constructor defaults (x = 0 …); present ones are +value.
            let n = nums(16);

            if matches!(t, FieldType::SFMatrix3d | FieldType::SFMatrix3f | FieldType::SFMatrix4d | FieldType::SFMatrix4f) && n.len() < 9 {
                FieldValue::default_for(t)
            } else {
                sf_from_numbers(t, &n)
            }
        }
    })
}

const COMPONENTS: &[&str] = &["x", "y", "z", "w"];

fn component_index(v: &FieldValue, key: &str) -> Option<usize> {
    if let Ok(i) = key.parse::<usize>() {
        return Some(i);
    }

    match v {
        FieldValue::SFColor(_) | FieldValue::SFColorRGBA(_) => ["r", "g", "b", "a"].iter().position(|k| *k == key),
        FieldValue::SFRotation(_) => ["x", "y", "z", "angle"].iter().position(|k| *k == key),
        _ => COMPONENTS.iter().position(|k| *k == key),
    }
}

fn component_get<'js>(ctx: &Ctx<'js>, h: u32, key: Option<&Value<'js>>) -> rquickjs::Result<Value<'js>> {
    let key = key.map(|k| coerce_string(ctx, k)).unwrap_or_default();
    let v = read(h);

    if let FieldValue::SFImage(img) = &v {
        return Ok(match key.as_str() {
            "width" => Value::new_int(ctx.clone(), img.width),
            "height" => Value::new_int(ctx.clone(), img.height),
            "comp" => Value::new_int(ctx.clone(), img.comp),
            "array" => new_value_js(ctx, FieldValue::MFInt32(img.array.clone())),
            _ => Value::new_undefined(ctx.clone()),
        });
    }

    let nums = sf_numbers(&v);

    Ok(match component_index(&v, &key).and_then(|i| nums.get(i)) {
        Some(x) => Value::new_number(ctx.clone(), *x),
        None => Value::new_undefined(ctx.clone()),
    })
}

fn component_set<'js>(ctx: &Ctx<'js>, h: u32, key: Option<&Value<'js>>, value: Option<&Value<'js>>) {
    let key = key.map(|k| coerce_string(ctx, k)).unwrap_or_default();
    let mut v = read(h);

    if let FieldValue::SFImage(img) = &mut v {
        let x = value.map(|x| coerce_f64(ctx, x)).unwrap_or(f64::NAN);

        let (mut w, mut hh, mut c, mut arr) = (img.width as f64, img.height as f64, img.comp as f64, img.array.clone());

        match key.as_str() {
            "width" => w = x,
            "height" => hh = x,
            "comp" => c = x,
            "array" => {
                if let Some(FieldValue::MFInt32(a)) = value.and_then(|x| convert(ctx, FieldType::MFInt32, x)) {
                    arr = a;
                }
            }
            _ => return,
        }

        write(h, FieldValue::SFImage(Image::new(w, hh, c, arr)));
        return;
    }

    let Some(i) = component_index(&v, &key) else {
        return;
    };
    let x = value.map(|x| coerce_f64(ctx, x)).unwrap_or(f64::NAN);

    match &mut v {
        FieldValue::SFColor(c) => c.set_index(i, x),
        FieldValue::SFColorRGBA(c) => c.set_index(i, x),
        FieldValue::SFVec2d(a) | FieldValue::SFVec2f(a) => a.set_index(i, x),
        FieldValue::SFVec3d(a) | FieldValue::SFVec3f(a) => a.set_index(i, x),
        FieldValue::SFVec4d(a) | FieldValue::SFVec4f(a) => a.set_index(i, x),
        FieldValue::SFRotation(r) => r.set_index(i, x),
        FieldValue::SFQuaternion(q) => q.set_index(i, x),
        FieldValue::SFMatrix3d(m) | FieldValue::SFMatrix3f(m) => {
            if i < 9 {
                m.0[i] = x
            }
        }
        FieldValue::SFMatrix4d(m) | FieldValue::SFMatrix4f(m) => {
            if i < 16 {
                m.0[i] = x
            }
        }
        _ => return,
    }

    write(h, v);
}

fn vec3_of(v: &FieldValue) -> Option<Vector3> {
    match v {
        FieldValue::SFVec3f(a) | FieldValue::SFVec3d(a) => Some(*a),
        _ => None,
    }
}

fn rot_of(v: &FieldValue) -> Option<Rotation4> {
    match v {
        FieldValue::SFRotation(r) => Some(*r),
        _ => None,
    }
}

/// Methods of the SF field classes.
fn method<'js>(ctx: &Ctx<'js>, h: u32, m: &str, a: &[Value<'js>]) -> rquickjs::Result<Value<'js>> {
    let v = read(h);
    let num = |x: f64| Value::new_number(ctx.clone(), x);
    let other = value_arg(a, 0);
    let undefined = Value::new_undefined(ctx.clone());

    macro_rules! vec_ops {
        ($ctor:path, $val:expr) => {{
            let s = $val;
            let o = match &other {
                Some($ctor(x)) => Some(*x),
                _ => None,
            };
            let mut r = s;
            let result: Option<Value> = match m {
                "abs" => Some(new_value_js(ctx, $ctor(*r.abs()))),
                "add" => o.map(|o| new_value_js(ctx, $ctor(*r.add(&o)))),
                "subtract" => o.map(|o| new_value_js(ctx, $ctor(*r.subtract(&o)))),
                "multiply" => Some(new_value_js(ctx, $ctor(*r.multiply(num_arg(ctx, a, 0))))),
                "divide" => Some(new_value_js(ctx, $ctor(*r.divide(num_arg(ctx, a, 0))))),
                "multVec" => o.map(|o| new_value_js(ctx, $ctor(*r.mult_vec(&o)))),
                "divVec" => o.map(|o| new_value_js(ctx, $ctor(*r.div_vec(&o)))),
                "dot" => o.map(|o| num(s.dot(&o))),
                "distance" => o.map(|o| num(s.distance(&o))),
                "length" => Some(num(s.norm())),
                "normalize" => Some(new_value_js(ctx, $ctor(*r.normalize()))),
                "negate" => Some(new_value_js(ctx, $ctor(*r.negate()))),
                "inverse" => Some(new_value_js(ctx, $ctor(*r.inverse()))),
                "max" => o.map(|o| new_value_js(ctx, $ctor(*r.max(&o)))),
                "min" => o.map(|o| new_value_js(ctx, $ctor(*r.min(&o)))),
                "lerp" => o.map(|o| new_value_js(ctx, $ctor(*r.lerp(&o, num_arg(ctx, a, 1))))),
                "reflect" => o.map(|o| new_value_js(ctx, $ctor(*r.reflect(&o)))),
                "clamp" => match (&other, value_arg(a, 1)) {
                    (Some($ctor(lo)), Some($ctor(hi))) => Some(new_value_js(ctx, $ctor(*r.clamp(lo, &hi)))),
                    _ => None,
                },
                _ => None,
            };
            result
        }};
    }

    let result: Option<Value> = match &v {
        FieldValue::SFVec2f(x) => vec_ops!(FieldValue::SFVec2f, *x),
        FieldValue::SFVec2d(x) => vec_ops!(FieldValue::SFVec2d, *x),
        FieldValue::SFVec3f(x) if m == "cross" => other.as_ref().and_then(vec3_of).map(|o| new_value_js(ctx, FieldValue::SFVec3f(*x.clone().cross(&o)))),
        FieldValue::SFVec3d(x) if m == "cross" => other.as_ref().and_then(vec3_of).map(|o| new_value_js(ctx, FieldValue::SFVec3d(*x.clone().cross(&o)))),
        FieldValue::SFVec3f(x) => vec_ops!(FieldValue::SFVec3f, *x),
        FieldValue::SFVec3d(x) => vec_ops!(FieldValue::SFVec3d, *x),
        FieldValue::SFVec4f(x) => vec_ops!(FieldValue::SFVec4f, *x),
        FieldValue::SFVec4d(x) => vec_ops!(FieldValue::SFVec4d, *x),
        FieldValue::SFColor(c) => match m {
            "getHSV" => {
                let arr = Array::new(ctx.clone())?;
                for (i, x) in c.get_hsv().iter().enumerate() {
                    arr.set(i, *x)?;
                }
                Some(arr.into_value())
            }
            "setHSV" => {
                let mut c = *c;
                c.set_hsv(num_arg(ctx, a, 0), num_arg(ctx, a, 1), num_arg(ctx, a, 2));
                write(h, FieldValue::SFColor(c));
                Some(undefined.clone())
            }
            "lerp" => match other {
                Some(FieldValue::SFColor(d)) => {
                    let [h0, s0, v0] = x3d_runtime::nodes::interp::hsv_lerp(c.get_hsv(), d.get_hsv(), num_arg(ctx, a, 1));
                    let mut r = Color3::default();
                    r.set_hsv(h0, s0, v0);
                    Some(new_value_js(ctx, FieldValue::SFColor(r)))
                }
                _ => None,
            },
            _ => None,
        },
        FieldValue::SFRotation(r) => match m {
            "getAxis" => Some(new_value_js(ctx, FieldValue::SFVec3f(r.clone().get_axis()))),
            "getMatrix" => Some(new_value_js(ctx, FieldValue::SFMatrix3f(r.get_matrix()))),
            "inverse" => Some(new_value_js(ctx, FieldValue::SFRotation(*r.clone().inverse()))),
            "multiply" => other.as_ref().and_then(rot_of).map(|o| new_value_js(ctx, FieldValue::SFRotation(*r.clone().mult_right(&o)))),
            "multVec" => other.clone().and_then(|o| {
                let mut x = vec3_of(&o)?;
                r.mult_vec_rot(&mut x);
                Some(new_value_js(ctx, if matches!(o, FieldValue::SFVec3d(_)) { FieldValue::SFVec3d(x) } else { FieldValue::SFVec3f(x) }))
            }),
            "slerp" => other.as_ref().and_then(rot_of).map(|o| new_value_js(ctx, FieldValue::SFRotation(*r.clone().slerp(&o, num_arg(ctx, a, 1))))),
            "setAxis" => other.as_ref().and_then(vec3_of).map(|axis| {
                let mut x = *r;
                let angle = x.angle();
                x.set(axis.x, axis.y, axis.z, angle);
                write(h, FieldValue::SFRotation(x));
                undefined.clone()
            }),
            "setMatrix" => match other {
                Some(FieldValue::SFMatrix3f(mm) | FieldValue::SFMatrix3d(mm)) => {
                    let mut x = *r;
                    x.set_matrix(&mm);
                    write(h, FieldValue::SFRotation(x));
                    Some(undefined.clone())
                }
                _ => None,
            },
            _ => None,
        },
        FieldValue::SFMatrix4f(mm) | FieldValue::SFMatrix4d(mm) => {
            let wrapm = |x: Matrix4| if matches!(v, FieldValue::SFMatrix4d(_)) { FieldValue::SFMatrix4d(x) } else { FieldValue::SFMatrix4f(x) };
            let other_m = match &other {
                Some(FieldValue::SFMatrix4f(o) | FieldValue::SFMatrix4d(o)) => Some(*o),
                _ => None,
            };

            match m {
                "setTransform" => {
                    let t = value_arg(a, 0).as_ref().and_then(vec3_of);
                    let r = value_arg(a, 1).as_ref().and_then(rot_of);
                    let s = value_arg(a, 2).as_ref().and_then(vec3_of);
                    let so = value_arg(a, 3).as_ref().and_then(rot_of);
                    let c = value_arg(a, 4).as_ref().and_then(vec3_of);
                    let mut x = *mm;
                    x.set_transform(t.as_ref(), r.as_ref(), s.as_ref(), so.as_ref(), c.as_ref());
                    // SFMatrix.setTransform modifies the value without an event.
                    match slot(h) {
                        Slot::Field(k) => rt().set_raw(k, wrapm(x)),
                        _ => write(h, wrapm(x)),
                    }
                    Some(undefined.clone())
                }
                "multLeft" => other_m.map(|o| new_value_js(ctx, wrapm(*mm.clone().mult_left(&o)))),
                "multRight" => other_m.map(|o| new_value_js(ctx, wrapm(*mm.clone().mult_right(&o)))),
                "inverse" => Some(new_value_js(ctx, wrapm(*mm.clone().inverse()))),
                "transpose" => Some(new_value_js(ctx, wrapm(*mm.clone().transpose()))),
                "determinant" => Some(num(mm.determinant())),
                "multVecMatrix" | "multMatrixVec" | "multDirMatrix" | "multMatrixDir" => other.clone().and_then(|o| {
                    let mut x = vec3_of(&o)?;
                    match m {
                        "multVecMatrix" => mm.mult_vec_matrix3(&mut x),
                        "multMatrixVec" => mm.mult_matrix_vec3(&mut x),
                        "multDirMatrix" => mm.mult_dir_matrix(&mut x),
                        _ => mm.mult_matrix_dir(&mut x),
                    }
                    Some(new_value_js(ctx, if matches!(o, FieldValue::SFVec3d(_)) { FieldValue::SFVec3d(x) } else { FieldValue::SFVec3f(x) }))
                }),
                "translate" => other.as_ref().and_then(vec3_of).map(|t| new_value_js(ctx, wrapm(*mm.clone().translate(&t)))),
                "rotate" => other.as_ref().and_then(rot_of).map(|r| new_value_js(ctx, wrapm(*mm.clone().rotate(&r)))),
                "scale" => other.as_ref().and_then(vec3_of).map(|s| new_value_js(ctx, wrapm(*mm.clone().scale(&s)))),
                _ => None,
            }
        }
        _ => None,
    };

    match result {
        Some(v) => Ok(v),
        None => {
            if m == "length" || m == "dot" {
                return Ok(num(f64::NAN));
            }

            throw(ctx, &format!("{}.{m} is not supported by the Rust port with these arguments", v.field_type().name()))
        }
    }
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
