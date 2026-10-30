//! The ECMAScript side of the Script node, without the engine: X_ITE's Script realm (the prelude), the field handle
//! table, the native operations `__x (op, …)` that the prelude calls, and the host calls of [`ScriptHost`]. Each
//! engine supplies a [`Cx`] (how to make, read and call its values) and gets this one Script implementation: QuickJS
//! in the native viewer ([`crate::QuickJsHost`]), the page's own JavaScript engine in the browser build (x3d-web).
//!
//! X_ITE runs every Script of a world in one JavaScript realm (the page), evaluating each
//! source with `with (globalObject) eval (source)` (Browser/Scripting/evaluate.js). A host
//! does the same: one realm per world, the same evaluate function (prelude.js).
//!
//! Field objects are handles into Rust: a node field (live: writes add events), a detached
//! value (`new SFVec3f (…)`, callback argument copies), or an element view of an MF field
//! (X3DTypedArrayField returns references to elements). All value semantics — conversions,
//! vector/rotation/matrix math, VRML printing — run in Rust on `FieldValue` with the
//! x3d-fields ports of X_ITE's math, so they match X_ITE bit for bit where those do.

use std::cell::RefCell;
use std::collections::HashMap;
use x3d_fields::math::*;
use x3d_fields::{FieldType, FieldValue, Generator, Image, NodeId, access, jsmath};
use x3d_runtime::ctx_events::CtxField;
use x3d_runtime::*;
use x3d_scene::*;

mod ctx_api;

/// The Script realm. prelude/head.js lists the parts; math.js, fields.js, config.js and node_types.js are generated
/// (tools/gen_prelude_xite.py). tools/build-web.sh makes the browser's copy from the same files.
pub const PRELUDE: &str = concat!(
    include_str!("prelude/head.js"),
    include_str!("prelude/math.js"),
    include_str!("prelude/fields.js"),
    include_str!("prelude/config.js"),
    include_str!("prelude/node_types.js"),
    include_str!("prelude/bridge.js"),
    include_str!("prelude/rest.js"),
);
const NOFIELD: &str = "\u{0}nofield";

// --- values of an engine ---------------------------------------------------------------------

/// A value that a native call gives JavaScript. The engine makes it ([`Cx::realize`]) when the call returns. Plain
/// arrays and objects have no effects when they are made, so this is the same as making them during the call.
pub enum J<V> {
    Undefined,
    Null,
    Bool(bool),
    Int(i32),
    /// A number as QuickJS `Value::new_number` makes it: an integral value is an int, so `-0` becomes `0`.
    Number(f64),
    /// A number with its sign kept (`-0` stays `-0`).
    Float(f64),
    Str(String),
    Array(Vec<J<V>>),
    Object(Vec<(&'static str, J<V>)>),
    /// A value of the engine.
    Value(V),
}

impl<V> From<bool> for J<V> {
    fn from(b: bool) -> Self {
        J::Bool(b)
    }
}

impl<V> From<i32> for J<V> {
    fn from(x: i32) -> Self {
        J::Int(x)
    }
}

impl<V> From<f64> for J<V> {
    fn from(x: f64) -> Self {
        J::Float(x)
    }
}

/// An int when it fits, else a float (rquickjs `IntoJs` of an unsigned integer).
impl<V> From<u32> for J<V> {
    fn from(x: u32) -> Self {
        match i32::try_from(x) {
            Ok(i) => J::Int(i),
            Err(_) => J::Float(x as f64),
        }
    }
}

impl<V> From<usize> for J<V> {
    fn from(x: usize) -> Self {
        match i32::try_from(x) {
            Ok(i) => J::Int(i),
            Err(_) => J::Float(x as f64),
        }
    }
}

impl<V> From<&str> for J<V> {
    fn from(s: &str) -> Self {
        J::Str(s.to_string())
    }
}

impl<V> From<&String> for J<V> {
    fn from(s: &String) -> Self {
        J::Str(s.clone())
    }
}

impl<V> From<String> for J<V> {
    fn from(s: String) -> Self {
        J::Str(s)
    }
}

/// A JavaScript array that a native call fills element by element.
pub struct Arr<V>(RefCell<Vec<J<V>>>);

impl<V> Arr<V> {
    pub fn new() -> Self {
        Arr(RefCell::new(vec![]))
    }

    /// `array [i] = v`.
    pub fn set(&self, i: usize, v: impl Into<J<V>>) {
        let mut a = self.0.borrow_mut();

        while a.len() < i {
            a.push(J::Undefined);
        }

        if i < a.len() {
            a[i] = v.into();
        } else {
            a.push(v.into());
        }
    }
}

impl<V> Default for Arr<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> From<Arr<V>> for J<V> {
    fn from(a: Arr<V>) -> Self {
        J::Array(a.0.into_inner())
    }
}

/// A plain JavaScript object that a native call fills property by property.
pub struct Obj<V>(RefCell<Vec<(&'static str, J<V>)>>);

impl<V> Obj<V> {
    pub fn new() -> Self {
        Obj(RefCell::new(vec![]))
    }

    /// `object [key] = v`.
    pub fn set(&self, key: &'static str, v: impl Into<J<V>>) {
        let mut o = self.0.borrow_mut();

        match o.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = v.into(),
            None => o.push((key, v.into())),
        }
    }
}

impl<V> Default for Obj<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> From<Obj<V>> for J<V> {
    fn from(o: Obj<V>) -> Self {
        J::Object(o.0.into_inner())
    }
}

/// A JavaScript value of an engine, as the native calls read it.
pub trait Val: Clone {
    fn is_null(&self) -> bool;
    fn is_undefined(&self) -> bool;
    /// An object of any kind: also an array or a function.
    fn is_object(&self) -> bool;
    fn is_string(&self) -> bool;
    /// The number of a number value, with no conversion.
    fn as_number(&self) -> Option<f64>;
    fn as_bool(&self) -> Option<bool>;
    /// The text of a string value, with no conversion.
    fn as_text(&self) -> Option<String>;
    /// The elements of an array (`value [i]` for each index below its length). An element whose read threw is `None`.
    fn elements(&self) -> Option<Vec<Option<Self>>>;
    /// `object [key]` of an object. `Err` when the read threw: the exception stays pending (see [`Val::catch`]).
    fn get(&self, key: &str) -> Result<Self, ()>;
    /// `key in object`. `Err` when the test threw.
    fn has(&self, key: &str) -> Result<bool, ()>;
    /// ToNumber. `Err` when it threw.
    fn to_number(&self) -> Result<f64, ()>;
    /// ToString. `Err` when it threw.
    fn to_text(&self) -> Result<String, ()>;
    /// ToBoolean.
    fn to_bool(&self) -> Result<bool, ()>;
    /// Clear the exception that a read left pending (QuickJS keeps it until it is caught).
    fn catch(&self);
}

/// A JavaScript engine that holds the prelude: what the native calls and the host calls need of it.
pub trait Cx {
    type V: Val;
    /// A thrown value as the engine returns it.
    type E;

    /// Make the value in the engine.
    fn realize(&self, j: J<Self::V>) -> Result<Self::V, Self::E>;
    /// `throw new Error (msg)`.
    fn throw(&self, msg: &str) -> Self::E;
    /// `throw new TypeError (msg)`.
    fn throw_type(&self, msg: &str) -> Self::E;
    /// Does the prelude have the entry point `name`?
    fn has_global(&self, name: &str) -> bool;
    /// Call the prelude's entry point `name`; `None` when it has none.
    fn call_global(&self, name: &str, args: Vec<J<Self::V>>) -> Option<Result<Self::V, Self::E>>;
    /// Run the pending promise jobs. An engine that cannot run them now runs them when the host call returns.
    fn run_jobs(&self);
    /// `String (error)` of a thrown value.
    fn describe(&self, e: Self::E) -> String;
    /// The V8 (fdlibm) math function `name` that the prelude puts on `Math`; null when the engine's own are V8's.
    fn math_fn(&self, name: &str) -> Result<J<Self::V>, Self::E>;
}

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
///   counts them out (`disown`) when the engine frees the object.
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
    SlotStats {
        len: s.slots.len(),
        capacity: s.slots.capacity(),
        free: s.free.len(),
        attached: s.attached.len(),
    }
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
        let live = (h as usize) < s.slots.len()
            && !matches!(s.slots[h as usize], Slot::Free)
            && s.generations[h as usize] as f64 == generation;
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
// host entry point sets ([`Table::select`]) before it runs JavaScript.
thread_local! {
    static STATES: RefCell<HashMap<u64, State>> = RefCell::new(HashMap::new());
    static CURRENT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static NEXT_HOST: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
    /// The table size of the last host that was dropped while it was the current one: what `slot_stats`
    /// reports when no host is current.
    static LAST_DROPPED: std::cell::Cell<SlotStats> = const { std::cell::Cell::new(SlotStats { len: 0, capacity: 0, free: 0, attached: 0 }) };
}

fn with_state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    STATES.with(|s| {
        f(s.borrow_mut()
            .get_mut(&CURRENT.get())
            .expect("script engine state"))
    })
}

/// The runtime of the world whose script is running.
fn rt<'a>() -> &'a mut Runtime {
    let p = with_state(|s| s.rt);
    // SAFETY: set by the host (Table::enter) to its runtime, which owns the host and outlives it; a host whose
    // runtime moves enters again before JavaScript runs.
    unsafe { &mut *p }
}

/// The field handle table of one Script host, and the host's key in the thread's tables.
pub struct Table {
    id: u64,
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}

impl Table {
    /// A new, empty table. It is the current one until another host runs.
    pub fn new() -> Self {
        let id = NEXT_HOST.get();
        NEXT_HOST.set(id + 1);

        STATES.with(|s| {
            s.borrow_mut().insert(
                id,
                State {
                    rt: std::ptr::null_mut(),
                    slots: vec![],
                    generations: vec![],
                    owners: vec![],
                    children: vec![],
                    free: vec![],
                    fresh: vec![],
                    attached: HashMap::new(),
                    description: String::new(),
                },
            );
        });
        CURRENT.set(id);

        Table { id }
    }

    /// This host's key: the native calls of its realm name it ([`select_host`]).
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Make this host's table the one native calls from JavaScript use.
    pub fn select(&self) {
        CURRENT.set(self.id);
    }

    /// The start of a host call: this table is the current one, and `rt` the runtime its native calls reach.
    pub fn enter(&self, rt: &mut Runtime) {
        self.select();
        with_state(|s| s.rt = rt as *mut Runtime);
    }

    /// The end of a host call: the slots JavaScript did not take are released.
    pub fn release_fresh(&self) {
        self.select();
        with_state(State::release_fresh);
    }

    /// The host goes: `free_objects` frees the engine's objects while this table is still the current one (their
    /// finalizers release handles), then the table goes. If it was the current one, `slot_stats` keeps its last size.
    pub fn close(&self, free_objects: impl FnOnce()) {
        let before = CURRENT.replace(self.id);

        free_objects();

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

/// A native call of the realm of host `id` (an engine whose `__x` names its host): its table is the current one. False
/// when the host is gone or never entered a runtime; the call must then not run.
pub fn select_host(id: u64) -> bool {
    STATES.with(|s| match s.borrow().get(&id) {
        Some(state) if !state.rt.is_null() => {
            CURRENT.set(id);
            true
        }
        _ => false,
    })
}

/// The native entry point `__x (op, …)` of the prelude. The host's table must be the current one.
pub fn native<C: Cx>(cx: &C, args: Vec<C::V>) -> Result<C::V, C::E> {
    let value = op(cx, args)?;

    cx.realize(value)
}

// --- the host calls (ScriptHost), the same for every engine ------------------------------------
//
// The host enters its table first ([`Table::enter`]) and arms its own limits; these functions run the JavaScript.

/// prelude/bridge.js: the node fields a Script holds as objects are refreshed before its code runs (`__enter`), and what it
/// changed in place goes to the runtime after it and its promise jobs ran (`__leave`).
fn js_enter<C: Cx>(cx: &C) {
    let _ = cx.call_global("__enter", vec![]);
}

fn js_leave<C: Cx>(cx: &C) {
    let _ = cx.call_global("__leave", vec![]);
}

/// ScriptHost::create_context.
pub fn create_context<C: Cx>(
    cx: &C,
    rt: &Runtime,
    node: NodeId,
    source: &str,
    callbacks: &[String],
) -> Result<Vec<bool>, ScriptError> {
    let fields: Vec<(String, u8, f64, &'static str)> = (0..rt.scene.node(node).user_fields.len())
        .map(|i| {
            let f = &rt.scene.node(node).user_fields[i];
            (
                f.name.clone(),
                f.access,
                encode(attach((node, FieldLoc::User(i)))),
                f.ty().name(),
            )
        })
        .collect();

    js_enter(cx);

    let list = fields
        .into_iter()
        .map(|(name, a, h, ty)| {
            J::Array(vec![
                J::from(name),
                J::Int(a as i32),
                J::Float(h),
                J::from(ty),
            ])
        })
        .collect();
    let cbs = callbacks.iter().map(|c| J::from(c)).collect();

    let result = match cx
        .call_global(
            "__create",
            vec![
                J::from(node),
                J::from(source),
                J::Array(cbs),
                J::Array(list),
            ],
        )
        .expect("prelude __create")
    {
        Ok(v) => v
            .elements()
            .and_then(|items| {
                items
                    .into_iter()
                    .map(|x| x.and_then(|x| x.as_bool()))
                    .collect::<Option<Vec<bool>>>()
            })
            .ok_or_else(|| ScriptError("Error: __create gave no callback list".into())),
        Err(e) => Err(ScriptError(cx.describe(e))),
    };

    cx.run_jobs();
    js_leave(cx);
    with_state(State::release_fresh);
    result
}

/// ScriptHost::call.
pub fn call<C: Cx>(
    cx: &C,
    rt: &Runtime,
    node: NodeId,
    call: ScriptCall,
) -> Result<(), ScriptError> {
    let time = rt.now;

    js_enter(cx);

    let args = match call {
        ScriptCall::Plain(name) => {
            let reason = name.clone();
            vec![
                J::from(node),
                J::from(name),
                J::Undefined,
                J::Float(time),
                J::Bool(false),
                J::from(reason),
            ]
        }
        ScriptCall::Field(name, value) => {
            // The header of an error names the field, not `set_<field>`.
            let reason = match name.strip_prefix("set_") {
                Some(field)
                    if rt
                        .scene
                        .node(node)
                        .user_fields
                        .iter()
                        .any(|f| f.name == field && f.access == access::INPUT_OUTPUT) =>
                {
                    field.to_string()
                }
                _ => name.clone(),
            };
            let h = detached(value);
            let v = field_value_js(h);
            vec![
                J::from(node),
                J::from(name),
                v,
                J::Float(time),
                J::Bool(true),
                J::from(reason),
            ]
        }
    };

    let result = cx.call_global("__call", args).expect("prelude __call");

    // Run pending promise jobs (async callbacks continue as microtasks).
    cx.run_jobs();
    js_leave(cx);

    with_state(State::release_fresh);

    match result {
        Ok(v) if v.is_undefined() => Ok(()),
        Ok(v) => Err(ScriptError(v.to_text().unwrap_or_default())),
        Err(e) => Err(ScriptError(cx.describe(e))),
    }
}

/// ScriptHost::settle and settle_plain: `value` is the root context of the scene, or 0 for a promise without a value.
pub fn settle<C: Cx>(cx: &C, id: u64, outcome: Result<f64, String>) {
    js_enter(cx);

    let args = match outcome {
        Ok(c) => vec![J::Float(id as f64), J::Bool(true), J::Float(c)],
        Err(m) => vec![J::Float(id as f64), J::Bool(false), J::from(m)],
    };
    let _ = cx.call_global("__settle", args);

    cx.run_jobs();
    js_leave(cx);

    let _ = cx.call_global("__endSettle", vec![]);

    with_state(State::release_fresh);
}

/// ScriptHost::next_timer.
pub fn next_timer<C: Cx>(cx: &C, until_ms: f64) -> Option<f64> {
    match cx.call_global("__nextTimer", vec![J::Float(until_ms)])? {
        Ok(v) if v.is_undefined() || v.is_null() => None,
        Ok(v) => v.as_number(),
        Err(_) => None,
    }
}

/// A host call that runs JavaScript with no value: fire_timer (`__fireTimer`), browser_event (`__browserEvent`) and
/// ctx_event (`__ctxEvent`).
pub fn event<C: Cx>(cx: &C, name: &str, args: Vec<J<C::V>>) {
    js_enter(cx);

    let _ = cx.call_global(name, args);

    cx.run_jobs();
    js_leave(cx);
    with_state(State::release_fresh);
}

/// ScriptHost::field_event.
///
/// X3DField.processEvent calls the callbacks synchronously, inside the event cascade: the promises they make run
/// with the jobs of the next host call, as in X_ITE they run after processEvents.
pub fn field_event<C: Cx>(cx: &C, k: FKey) {
    let _ = cx.call_global_with("__fieldEvent", || {
        vec![J::from(fkey_id(k)), field_value_js(attach(k))]
    });

    with_state(State::release_fresh);
}

/// ScriptHost::raw_coerce.
pub fn raw_coerce<C: Cx>(
    cx: &C,
    requests: &[RawRequest],
) -> Vec<Option<Vec<Option<RawComponent>>>> {
    let supply = raw_coerce_js(cx, requests);

    cx.run_jobs();
    supply
}

/// The first half of ScriptHost::held_nodes: the candidates' objects are unpinned (prelude/rest.js). The host then
/// collects garbage and calls [`held_nodes`]. `None` when the realm cannot say.
pub fn unpin_nodes<C: Cx>(cx: &C, candidates: &[NodeId]) -> Option<()> {
    let ids = candidates.iter().map(|n| J::Float(*n as f64)).collect();

    match cx.call_global("__unpin", vec![J::Array(ids)])? {
        Ok(_) => Some(()),
        Err(_) => None,
    }
}

/// The second half of ScriptHost::held_nodes: the candidates whose objects are left are held.
pub fn held_nodes<C: Cx>(cx: &C, candidates: &[NodeId]) -> Option<Vec<NodeId>> {
    let ids = candidates.iter().map(|n| J::Float(*n as f64)).collect();

    let held = match cx.call_global("__heldNodes", vec![J::Array(ids)])? {
        Ok(v) => v.elements().and_then(|items| {
            items
                .into_iter()
                .map(|x| x.and_then(|x| x.as_number()).map(|i| i as NodeId))
                .collect::<Option<Vec<NodeId>>>()
        }),
        Err(_) => None,
    }?;

    cx.run_jobs();
    Some(held)
}

/// The first half of ScriptHost::held_scenes: the X3DScene objects of the candidate scenes are unpinned. The host
/// collects garbage before [`held_scenes`].
pub fn unpin_scenes<C: Cx>(cx: &C, candidates: &[CtxId]) -> Option<()> {
    let ids = candidates.iter().map(|c| J::Float(*c as f64)).collect();

    match cx.call_global("__unpinScenes", vec![J::Array(ids)])? {
        Ok(_) => Some(()),
        Err(_) => None,
    }
}

/// The second half of ScriptHost::held_scenes: the candidate scenes whose X3DScene objects are left are held.
pub fn held_scenes<C: Cx>(cx: &C, candidates: &[CtxId]) -> Option<Vec<CtxId>> {
    let ids = candidates.iter().map(|c| J::Float(*c as f64)).collect();

    let held = match cx.call_global("__heldScenes", vec![J::Array(ids)])? {
        Ok(v) => v.elements().and_then(|items| {
            items
                .into_iter()
                .map(|x| x.and_then(|x| x.as_number()).map(|i| i as CtxId))
                .collect::<Option<Vec<CtxId>>>()
        }),
        Err(_) => None,
    }?;

    cx.run_jobs();
    Some(held)
}

/// ScriptHost::release_scenes: the realm forgets its objects and callbacks of the released scenes.
pub fn release_scenes<C: Cx>(cx: &C, ctxs: &[CtxId]) {
    let ids = ctxs.iter().map(|c| J::Float(*c as f64)).collect();

    let _ = cx.call_global("__releaseScenes", vec![J::Array(ids)]);
}

/// The table side of ScriptHost::release_nodes: a field handle of a released node keeps the value it had and no longer
/// reaches the field. Returns the handles that were detached, for [`release_nodes`].
pub fn release_slots(nodes: &[NodeId]) -> Vec<f64> {
    let dead: std::collections::HashSet<NodeId> = nodes.iter().copied().collect();

    with_state(|s| {
        let stale: Vec<(FKey, u32)> = s
            .attached
            .iter()
            .filter(|(k, _)| dead.contains(&k.0))
            .map(|(k, h)| (*k, *h))
            .collect();
        let mut detached = vec![];

        for (k, h) in stale {
            s.attached.remove(&k);

            // SAFETY: the host entered its runtime (Table::enter) for this call.
            let value = unsafe { (*s.rt).value(k).clone() };

            s.slots[h as usize] = Slot::Value(value);
            detached.push(s.generations[h as usize] as f64 * 4294967296.0 + h as f64);
            s.try_free(h);
        }

        detached
    })
}

/// The realm side of ScriptHost::release_nodes: the Script objects of the nodes stand for `tombstone` from now on.
pub fn release_nodes<C: Cx>(cx: &C, nodes: &[NodeId], tombstone: NodeId, detached: Vec<f64>) {
    let ids = nodes.iter().map(|n| J::from(*n)).collect();
    let detached = detached.into_iter().map(J::Float).collect();

    let _ = cx.call_global(
        "__release",
        vec![J::Array(ids), J::from(tombstone), J::Array(detached)],
    );
}

/// ScriptHost::destroy_context: the realm forgets the Script's global object and callbacks.
pub fn destroy<C: Cx>(cx: &C, node: NodeId) {
    let _ = cx.call_global("__destroy", vec![J::from(node)]);
}

/// [`Cx::call_global`] with arguments made only when the realm has the entry point.
trait CallWith: Cx {
    fn call_global_with(
        &self,
        name: &str,
        args: impl FnOnce() -> Vec<J<Self::V>>,
    ) -> Option<Result<Self::V, Self::E>>;
}

impl<C: Cx> CallWith for C {
    fn call_global_with(
        &self,
        name: &str,
        args: impl FnOnce() -> Vec<J<Self::V>>,
    ) -> Option<Result<Self::V, Self::E>> {
        if !self.has_global(name) {
            return None;
        }

        self.call_global(name, args())
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

fn data_number<V>(x: f64) -> J<V> {
    if jsmath::is_js_undefined(x) {
        return J::Undefined;
    }

    if jsmath::is_js_null(x) {
        return J::Null;
    }

    // An integral number is an int (Value::new_number), and -0 with it: -0 keeps its sign.
    if x == 0.0 && x.is_sign_negative() {
        return J::Float(x);
    }

    J::Number(x)
}

fn data_array<V>(items: impl IntoIterator<Item = J<V>>) -> J<V> {
    J::Array(items.into_iter().collect())
}

fn image_data<V>(img: &Image) -> J<V> {
    let pixels = data_array(img.array.iter().map(|p| J::Int(*p)));

    data_array([
        J::Int(img.width),
        J::Int(img.height),
        J::Int(img.comp),
        pixels,
    ])
}

fn node_data<V>(n: &Option<NodeId>) -> J<V> {
    match n {
        Some(n) => J::Number(*n as f64),
        None => J::Null,
    }
}

/// FieldValue -> the plain data of a field object.
fn value_data<V>(v: &FieldValue) -> J<V> {
    let nums = |xs: &mut dyn Iterator<Item = f64>| data_array(xs.map(data_number));

    match v {
        FieldValue::SFBool(b) => J::Bool(*b),
        FieldValue::SFInt32(x) => J::Int(*x),
        FieldValue::SFFloat(x) | FieldValue::SFDouble(x) | FieldValue::SFTime(x) => data_number(*x),
        FieldValue::SFString(s) => J::from(s),
        FieldValue::SFNode(n) => node_data(n),
        FieldValue::SFImage(img) => image_data(img),
        FieldValue::SFRotation(r) => {
            let (aa, tainted, q) = r.state();
            nums(
                &mut aa
                    .into_iter()
                    .chain([if tainted { 1.0 } else { 0.0 }, q.x, q.y, q.z, q.w]),
            )
        }
        FieldValue::SFColor(_)
        | FieldValue::SFColorRGBA(_)
        | FieldValue::SFVec2d(_)
        | FieldValue::SFVec2f(_)
        | FieldValue::SFVec3d(_)
        | FieldValue::SFVec3f(_)
        | FieldValue::SFVec4d(_)
        | FieldValue::SFVec4f(_)
        | FieldValue::SFQuaternion(_)
        | FieldValue::SFMatrix3d(_)
        | FieldValue::SFMatrix3f(_)
        | FieldValue::SFMatrix4d(_)
        | FieldValue::SFMatrix4f(_) => nums(&mut sf_numbers(v).into_iter()),
        FieldValue::MFBool(a) => data_array(a.iter().map(|b| J::Bool(*b))),
        FieldValue::MFInt32(a) => nums(&mut a.iter().map(|x| *x as f64)),
        FieldValue::MFColor(a)
        | FieldValue::MFColorRGBA(a)
        | FieldValue::MFFloat(a)
        | FieldValue::MFMatrix3f(a)
        | FieldValue::MFMatrix4f(a)
        | FieldValue::MFVec2f(a)
        | FieldValue::MFVec3f(a)
        | FieldValue::MFVec4f(a) => nums(&mut a.iter().map(|x| *x as f64)),
        FieldValue::MFDouble(a)
        | FieldValue::MFMatrix3d(a)
        | FieldValue::MFMatrix4d(a)
        | FieldValue::MFQuaternion(a)
        | FieldValue::MFRotation(a)
        | FieldValue::MFTime(a)
        | FieldValue::MFVec2d(a)
        | FieldValue::MFVec3d(a)
        | FieldValue::MFVec4d(a) => nums(&mut a.iter().copied()),
        FieldValue::MFString(a) => data_array(a.iter().map(J::from)),
        FieldValue::MFNode(a) => data_array(a.iter().map(node_data)),
        FieldValue::MFImage(a) => data_array(a.iter().map(image_data)),
    }
}

fn data_numbers<V: Val>(v: &V) -> Vec<f64> {
    let Some(a) = v.elements() else {
        return vec![];
    };

    a.into_iter()
        // The number the engine uses is the component's ToNumber (X_ITE's generator and renderer read `+ value`); a component that
        // is not a number stays as it is in the Script's objects (bridge.js raw values). An object is not converted here: its
        // valueOf / toString may have effects, and X_ITE calls them only where it uses the value (a print takes it then).
        .map(|x| match x {
            Some(x) if x.is_object() => f64::NAN,
            Some(x) => x.as_number().unwrap_or_else(|| {
                x.to_number().unwrap_or_else(|_| {
                    x.catch();
                    f64::NAN
                })
            }),
            None => f64::NAN,
        })
        .collect()
}

fn data_image<V: Val>(v: &V) -> Image {
    let Some(a) = v.elements() else {
        return Image::default();
    };
    let at = |i: usize| a.get(i).cloned().flatten();
    let n = |i: usize| at(i).and_then(|x| x.as_number()).unwrap_or(0.0);
    let pixels = at(3).map(|p| data_numbers(&p)).unwrap_or_default();

    Image {
        width: n(0) as i32,
        height: n(1) as i32,
        comp: n(2) as i32,
        array: pixels.into_iter().map(|p| p as i32).collect(),
    }
}

fn data_node<V: Val>(v: &V) -> Option<NodeId> {
    v.as_number().map(|n| n as NodeId)
}

/// The plain data of a field object -> FieldValue of type `ty`.
fn data_value<V: Val>(ty: FieldType, v: &V) -> FieldValue {
    use FieldType as T;

    let items = || v.elements().unwrap_or_default();
    let list = |f: &dyn Fn(V) -> FieldValue| -> Vec<FieldValue> {
        items().into_iter().flatten().map(f).collect()
    };
    let f32s = || {
        data_numbers(v)
            .into_iter()
            .map(|x| x as f32)
            .collect::<Vec<f32>>()
    };

    match ty {
        T::SFBool => FieldValue::SFBool(v.as_bool().unwrap_or(false)),
        T::SFInt32 => FieldValue::SFInt32(v.as_number().map(jsmath::to_int32).unwrap_or(0)),
        T::SFFloat => FieldValue::SFFloat(v.as_number().unwrap_or(0.0)),
        T::SFDouble => FieldValue::SFDouble(v.as_number().unwrap_or(0.0)),
        T::SFTime => FieldValue::SFTime(v.as_number().unwrap_or(0.0)),
        T::SFString => FieldValue::SFString(v.as_text().unwrap_or_default()),
        T::SFNode => FieldValue::SFNode(data_node(v)),
        T::SFImage => FieldValue::SFImage(data_image(v)),
        T::SFRotation => {
            let n = data_numbers(v);
            let g = |i: usize| n.get(i).copied().unwrap_or(0.0);
            FieldValue::SFRotation(Rotation4::from_state(
                [g(0), g(1), g(2), g(3)],
                g(4) != 0.0,
                Quaternion::new(g(5), g(6), g(7), g(8)),
            ))
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
        T::MFBool => FieldValue::MFBool(
            list(&|x| FieldValue::SFBool(x.as_bool().unwrap_or(false)))
                .into_iter()
                .map(|x| matches!(x, FieldValue::SFBool(true)))
                .collect(),
        ),
        T::MFInt32 => {
            FieldValue::MFInt32(data_numbers(v).into_iter().map(jsmath::to_int32).collect())
        }
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
        T::MFString => FieldValue::MFString(
            items()
                .into_iter()
                .map(|x| x.and_then(|x| x.to_text().ok()).unwrap_or_default())
                .collect(),
        ),
        T::MFNode => FieldValue::MFNode(
            items()
                .into_iter()
                .map(|x| x.and_then(|x| data_node(&x)))
                .collect(),
        ),
        T::MFImage => FieldValue::MFImage(
            items()
                .into_iter()
                .flatten()
                .map(|x| data_image(&x))
                .collect(),
        ),
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
        FieldType::SFColorRGBA => {
            FieldValue::SFColorRGBA(Color4::new(g(0, 0.0), g(1, 0.0), g(2, 0.0), g(3, 0.0)))
        }
        FieldType::SFVec2d => FieldValue::SFVec2d(Vector2::new(g(0, 0.0), g(1, 0.0))),
        FieldType::SFVec2f => FieldValue::SFVec2f(Vector2::new(g(0, 0.0), g(1, 0.0))),
        FieldType::SFVec3d => FieldValue::SFVec3d(Vector3::new(g(0, 0.0), g(1, 0.0), g(2, 0.0))),
        FieldType::SFVec3f => FieldValue::SFVec3f(Vector3::new(g(0, 0.0), g(1, 0.0), g(2, 0.0))),
        FieldType::SFVec4d => {
            FieldValue::SFVec4d(Vector4::new(g(0, 0.0), g(1, 0.0), g(2, 0.0), g(3, 1.0)))
        }
        FieldType::SFVec4f => {
            FieldValue::SFVec4f(Vector4::new(g(0, 0.0), g(1, 0.0), g(2, 0.0), g(3, 1.0)))
        }
        FieldType::SFRotation => {
            FieldValue::SFRotation(Rotation4::new(g(0, 0.0), g(1, 0.0), g(2, 1.0), g(3, 0.0)))
        }
        FieldType::SFQuaternion => {
            FieldValue::SFQuaternion(Quaternion::new(g(0, 0.0), g(1, 0.0), g(2, 0.0), g(3, 0.0)))
        }
        FieldType::SFMatrix3d | FieldType::SFMatrix3f => {
            let mut m = Matrix3::default();
            if n.len() >= 9 {
                m.0.copy_from_slice(&n[..9]);
            }
            if ty == FieldType::SFMatrix3d {
                FieldValue::SFMatrix3d(m)
            } else {
                FieldValue::SFMatrix3f(m)
            }
        }
        FieldType::SFMatrix4d | FieldType::SFMatrix4f => {
            let mut m = Matrix4::default();
            if n.len() >= 16 {
                m.0.copy_from_slice(&n[..16]);
            }
            if ty == FieldType::SFMatrix4d {
                FieldValue::SFMatrix4d(m)
            } else {
                FieldValue::SFMatrix4f(m)
            }
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
        FieldValue::MFRotation(v) => FieldValue::SFRotation(Rotation4::new(
            v[i * 4],
            v[i * 4 + 1],
            v[i * 4 + 2],
            v[i * 4 + 3],
        )),
        _ => {
            let c = mf.field_type().components();
            let nums: Vec<f64> = match mf {
                FieldValue::MFColor(v)
                | FieldValue::MFColorRGBA(v)
                | FieldValue::MFVec2f(v)
                | FieldValue::MFVec3f(v)
                | FieldValue::MFVec4f(v)
                | FieldValue::MFMatrix3f(v)
                | FieldValue::MFMatrix4f(v) => {
                    v[i * c..(i + 1) * c].iter().map(|x| *x as f64).collect()
                }
                FieldValue::MFVec2d(v)
                | FieldValue::MFVec3d(v)
                | FieldValue::MFVec4d(v)
                | FieldValue::MFMatrix3d(v)
                | FieldValue::MFMatrix4d(v)
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
        FieldValue::MFInt32(v) => {
            v[i] = sf_numbers(sf)
                .first()
                .map(|x| jsmath::to_int32(*x))
                .unwrap_or(0)
        }
        FieldValue::MFFloat(v) => v[i] = sf_numbers(sf).first().copied().unwrap_or(0.0) as f32,
        FieldValue::MFDouble(v) | FieldValue::MFTime(v) => {
            v[i] = sf_numbers(sf).first().copied().unwrap_or(0.0)
        }
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
        FieldValue::MFColor(v)
        | FieldValue::MFColorRGBA(v)
        | FieldValue::MFVec2f(v)
        | FieldValue::MFVec3f(v)
        | FieldValue::MFVec4f(v)
        | FieldValue::MFMatrix3f(v)
        | FieldValue::MFMatrix4f(v) => {
            for (j, x) in mf_numbers_of(sf, c).into_iter().enumerate() {
                v[i * c + j] = x as f32;
            }
        }
        FieldValue::MFVec2d(v)
        | FieldValue::MFVec3d(v)
        | FieldValue::MFVec4d(v)
        | FieldValue::MFMatrix3d(v)
        | FieldValue::MFMatrix4d(v)
        | FieldValue::MFQuaternion(v)
        | FieldValue::MFRotation(v) => {
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
    matches!(
        t,
        FieldType::SFBool
            | FieldType::SFInt32
            | FieldType::SFFloat
            | FieldType::SFDouble
            | FieldType::SFTime
            | FieldType::SFString
    )
}

/// A Script prints with each node's clone count as it is now (X3DGenerator.NeedsName: X3DNode.getCloneCount); what the host
/// published for its own dumps is kept.
fn with_live_clone_counts<T>(f: impl FnOnce() -> T) -> T {
    let saved = rt().scene.clone_counts.take();
    rt().publish_clone_counts();
    let r = f();
    rt().scene.clone_counts = saved;
    r
}

/// Runtime::print_raw for a print a Script asks for: the raw components are coerced in this realm directly.
fn print_raw_js<C: Cx, R>(ctx: &C, print: impl Fn(&Scene) -> R) -> R {
    if rt().scene.raw_tokens.is_empty() {
        return print(&rt().scene);
    }

    *rt().scene.raw_print.borrow_mut() = RawPrint::Log(vec![]);
    print(&rt().scene);

    let requests = match std::mem::take(&mut *rt().scene.raw_print.borrow_mut()) {
        RawPrint::Log(log) => log,
        _ => vec![],
    };
    let supply = raw_coerce_js(ctx, &requests);

    *rt().scene.raw_print.borrow_mut() = RawPrint::Supply(supply.into());

    let result = print(&rt().scene);

    *rt().scene.raw_print.borrow_mut() = RawPrint::Off;
    result
}

/// prelude/bridge.js `__rawCoerce`: for each request of this host, `String (component)` or `+ component` of its raw components.
fn raw_coerce_js<C: Cx>(
    ctx: &C,
    requests: &[RawRequest],
) -> Vec<Option<Vec<Option<RawComponent>>>> {
    let cur = CURRENT.get();
    let none = || vec![None; requests.len()];
    let mut list = vec![];

    for r in requests {
        let token = if r.host == cur { r.token as f64 } else { -1.0 };

        list.push(J::Float(token));
        list.push(J::Bool(r.number));
    }

    let result = match ctx.call_global("__rawCoerce", vec![J::Array(list)]) {
        None => return none(),
        Some(Ok(a)) => a,
        Some(Err(e)) => {
            // The exception is caught (its text is not needed).
            let _ = ctx.describe(e);
            return none();
        }
    };
    let Some(items) = result.elements() else {
        return none();
    };

    (0..requests.len())
        .map(|i| {
            let entry = items.get(i).cloned().flatten()?;
            let components = entry.elements()?;

            Some(
                components
                    .into_iter()
                    .map(|c| {
                        let c = c?;

                        if c.is_string() {
                            return c.as_text().map(RawComponent::Text);
                        }

                        c.as_number().map(RawComponent::Number)
                    })
                    .collect(),
            )
        })
        .collect()
}

/// `field .toString ()` (X3DObject.toString with the default generator).
fn to_vrml(v: &FieldValue) -> String {
    let mut g = Generator::default();

    match v {
        FieldValue::SFNode(_) | FieldValue::MFNode(_) => {
            with_live_clone_counts(|| rt().scene.node_value_to_vrml_string(v))
        }
        _ => {
            v.to_vrml_stream(&mut g);
            g.string
        }
    }
}

// --- JS conversions --------------------------------------------------------------------------

fn obj_num<V: Val>(v: &V, key: &str) -> Option<f64> {
    if !v.is_object() {
        return None;
    }

    // A getter that throws (the Script's global object, a Proxy) must not leave its exception pending.
    let x = match v.get(key) {
        Ok(x) => x,
        Err(_) => {
            v.catch();
            return None;
        }
    };

    if x.is_null() {
        return Some(f64::NAN);
    }

    x.as_number()
}

/// A field handle passed from JS as `{ __h }` (none if its slot was released since).
fn arg_handle<V: Val>(v: &V) -> Option<u32> {
    obj_num(v, "__h").and_then(decode)
}

/// The field handle argument `i` of a native call; a released one throws.
fn handle_at<C: Cx>(ctx: &C, args: &[C::V], i: usize) -> Result<u32, C::E> {
    match args.get(i).and_then(|v| v.as_number()).and_then(decode) {
        Some(h) => Ok(h),
        None => throw(ctx, "Error: X3DField handle is no longer valid."),
    }
}

/// A node passed as `{ __n }` (or `null`): Some(None) for null.
fn arg_node<V: Val>(v: &V) -> Option<Option<NodeId>> {
    if v.is_null() || v.is_undefined() {
        return Some(None);
    }

    if !v.is_object() {
        return None;
    }

    if !v.has("__n").unwrap_or(false) {
        v.catch();
        return None;
    }

    let x = match v.get("__n") {
        Ok(x) => x,
        Err(_) => {
            v.catch();
            return None;
        }
    };
    Some(x.as_number().map(|n| n as NodeId))
}

fn coerce_f64<C: Cx>(_ctx: &C, v: &C::V) -> f64 {
    if let Some(h) = arg_handle(v) {
        // `+field`: a scalar field converts through valueOf; every other field object becomes NaN.
        return match read(h) {
            FieldValue::SFBool(b) => f64::from(u8::from(b)),
            FieldValue::SFInt32(n) => f64::from(n),
            FieldValue::SFFloat(n) | FieldValue::SFDouble(n) | FieldValue::SFTime(n) => n,
            _ => f64::NAN,
        };
    }

    v.to_number().unwrap_or_else(|_| {
        v.catch();
        f64::NAN
    })
}

fn coerce_string<C: Cx>(_ctx: &C, v: &C::V) -> String {
    if let Some(h) = arg_handle(v) {
        return match read(h) {
            FieldValue::SFString(s) => s,
            other => to_vrml(&other),
        };
    }

    v.to_text().unwrap_or_else(|_| {
        v.catch();
        String::new()
    })
}

fn coerce_bool<V: Val>(v: &V) -> bool {
    if let Some(h) = arg_handle(v) {
        return match read(h) {
            FieldValue::SFBool(b) => b,
            _ => true,
        };
    }

    v.to_bool().unwrap_or(false)
}

/// X3DField.setValue (value) conversion for a target of type `ty`.
fn convert<C: Cx>(ctx: &C, ty: FieldType, v: &C::V) -> Option<FieldValue> {
    match ty {
        FieldType::SFBool => return Some(FieldValue::SFBool(coerce_bool(v))),
        FieldType::SFInt32 => {
            return Some(FieldValue::SFInt32(jsmath::to_int32(coerce_f64(ctx, v))));
        }
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

        if let Some(items) = v.elements() {
            for e in items {
                let e = e?;
                let sf = convert(ctx, st, &e).unwrap_or_else(|| FieldValue::default_for(st));
                mf_push(&mut mf, &sf);
            }
        }

        return Some(mf);
    }

    None
}

/// A value for JS: primitives for scalars, `{ __n }` for nodes, `{ __h, __t, __a }` otherwise.
fn field_value_js<V>(h: u32) -> J<V> {
    let v = read(h);
    let attached = matches!(slot(h), Slot::Field(_));

    primitive_or_handle(&v, h, attached)
}

fn primitive_or_handle<V>(v: &FieldValue, h: u32, attached: bool) -> J<V> {
    match v {
        FieldValue::SFBool(b) => J::Bool(*b),
        FieldValue::SFInt32(x) => J::Int(*x),
        FieldValue::SFFloat(x) | FieldValue::SFDouble(x) | FieldValue::SFTime(x) => data_number(*x),
        FieldValue::SFString(s) => J::from(s),
        FieldValue::SFNode(n) => node_js(*n),
        _ => handle_js(h, v.field_type().name(), attached),
    }
}

fn handle_js<V>(h: u32, ty: &str, attached: bool) -> J<V> {
    J::Object(vec![
        ("__h", J::Float(encode(h))),
        ("__t", J::from(ty)),
        ("__a", J::Bool(attached)),
    ])
}

fn node_js<V>(n: Option<NodeId>) -> J<V> {
    match n {
        Some(n) => J::Object(vec![("__n", J::from(n))]),
        None => J::Object(vec![("__n", J::Null)]),
    }
}

/// A fresh detached field for a returned value.
fn new_value_js<V>(v: FieldValue) -> J<V> {
    let ty = v.field_type();

    if is_scalar(ty) || ty == FieldType::SFNode {
        return primitive_or_handle(&v, 0, false);
    }

    let h = detached(v);
    handle_js(h, ty.name(), false)
}

fn throw<C: Cx, T>(ctx: &C, msg: &str) -> Result<T, C::E> {
    Err(ctx.throw(msg))
}

fn throw_type<C: Cx, T>(ctx: &C, msg: &str) -> Result<T, C::E> {
    Err(ctx.throw_type(msg))
}

/// The elements of an array-like JS value, as `X3DTypedArrayField.set` and `X3DObjectArrayField.set` read it
/// (`value .length`, `value [i]`): arrays and strings (one element per UTF-16 unit). Other values have none.
fn array_like<C: Cx>(ctx: &C, v: &C::V) -> Option<Vec<C::V>> {
    let undefined = || ctx.realize(J::Undefined).ok();

    if let Some(items) = v.elements() {
        return Some(
            items
                .into_iter()
                .filter_map(|x| x.or_else(undefined))
                .collect(),
        );
    }

    if v.is_string() {
        let text = v.as_text()?;
        let units: Vec<u16> = text.encode_utf16().collect();

        return Some(
            units
                .iter()
                .filter_map(|u| {
                    ctx.realize(J::Str(String::from_utf16_lossy(&[*u])))
                        .ok()
                        .or_else(undefined)
                })
                .collect(),
        );
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
fn set_convert<C: Cx>(ctx: &C, ty: FieldType, v: &C::V) -> Result<Option<FieldValue>, C::E> {
    use FieldType::*;

    if matches!(
        ty,
        SFBool | SFInt32 | SFFloat | SFDouble | SFTime | SFString
    ) {
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
        if matches!(
            ty,
            SFVec2f | SFVec2d | SFVec3f | SFVec3d | SFVec4f | SFVec4d
        ) {
            if v.is_object() {
                let n = ty.components();
                let mut nums = vec![];

                for key in ["x", "y", "z", "w"].iter().take(n) {
                    // A read that throws gives undefined, whose ToNumber is NaN.
                    nums.push(match v.get(key) {
                        Ok(e) => coerce_f64(ctx, &e),
                        Err(_) => f64::NAN,
                    });
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
        if value.field_type().is_multi()
            && ty.components() == 1
            && value.field_type().components() == 1
            && !matches!(ty, MFString | MFNode | MFImage)
            && !matches!(value.field_type(), MFString | MFNode | MFImage)
        {
            let mut mf = FieldValue::default_for(ty);
            let nums: Vec<f64> = (0..value.len())
                .map(|i| {
                    mf_get(&value, i)
                        .map(|e| sf_numbers(&e).first().copied().unwrap_or(f64::NAN))
                        .unwrap_or(f64::NAN)
                })
                .collect();

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
            let mut nums: Vec<f64> = elements
                .iter()
                .map(|e| {
                    if arg_handle(e).is_some() {
                        f64::NAN
                    } else {
                        coerce_f64(ctx, e)
                    }
                })
                .collect();
            let rest = nums.len() % components;

            if rest != 0 {
                let keep = nums.len() - rest;

                nums.truncate(keep);
                rt().log_warn(format!(
                    "Array length must be multiple of components size, which is {components}."
                ));
            }

            let mut mf = FieldValue::default_for(ty);

            mf.set_flat_numbers(&nums);
            Ok(Some(mf))
        }
    }
}

fn str_arg<C: Cx>(ctx: &C, args: &[C::V], i: usize) -> String {
    args.get(i)
        .map(|v| coerce_string(ctx, v))
        .unwrap_or_default()
}

fn num_arg<C: Cx>(ctx: &C, args: &[C::V], i: usize) -> f64 {
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

fn handle_arg<V: Val>(args: &[V], i: usize) -> Option<u32> {
    args.get(i).and_then(arg_handle)
}

fn value_arg<V: Val>(args: &[V], i: usize) -> Option<FieldValue> {
    handle_arg(args, i).map(read)
}

// --- the native entry point --------------------------------------------------------------------

fn op<C: Cx>(ctx: &C, args: Vec<C::V>) -> Result<J<C::V>, C::E> {
    let name = args.first().and_then(|v| v.as_text()).unwrap_or_default();
    let a = &args[1.min(args.len())..];

    match name.as_str() {
        "nofield" => Ok(J::from(NOFIELD)),
        // Browser.print goes to console.log, which the oracle does not record. X3D_PRINT=1
        // shows it on stderr for debugging.
        "print" => {
            if std::env::var_os("X3D_PRINT").is_some() {
                eprintln!("print: {}", str_arg(ctx, a, 0));
            }

            let text = str_arg(ctx, a, 0);
            rt().print(text);

            Ok(J::Undefined)
        }
        "console" => {
            let text = str_arg(ctx, a, 1);

            match str_arg(ctx, a, 0).as_str() {
                "warn" => rt().log_warn(text),
                "error" => rt().log_error(text),
                _ => rt().print(text),
            }

            Ok(J::Undefined)
        }
        "mathfn" => ctx.math_fn(&str_arg(ctx, a, 0)),
        "fname" => {
            let h = handle_at(ctx, a, 0)?;
            let n = slot_key(h)
                .map(|k| rt().field(k).name.clone())
                .unwrap_or_default();
            Ok(J::from(&n))
        }
        "faccess" => {
            let h = handle_at(ctx, a, 0)?;
            let n = slot_key(h)
                .map(|k| rt().field(k).access)
                .unwrap_or(access::INITIALIZE_ONLY);
            Ok(J::Int(n as i32))
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

            Ok(J::Undefined)
        }
        "str" => {
            let h = handle_at(ctx, a, 0)?;
            Ok(J::from(&to_vrml(&read(h))))
        }
        "at" => {
            let h = handle_at(ctx, a, 0)?;
            let i = num_arg(ctx, a, 1) as usize;
            let v = read(h);

            match mf_get(&v, i) {
                None => Ok(J::Undefined),
                Some(sf) if is_scalar(sf.field_type()) || sf.field_type() == FieldType::SFNode => {
                    Ok(primitive_or_handle(&sf, 0, false))
                }
                Some(sf) => Ok(handle_js(element(h, i), sf.field_type().name(), false)),
            }
        }
        // --- nodes
        "nfield" => {
            let id = inner_node(num_arg(ctx, a, 0) as NodeId);
            let field = str_arg(ctx, a, 1);

            match rt().get_field(id, &field) {
                Some(k) => {
                    let h = attach(k);
                    Ok(handle_js(h, rt().field(k).ty().name(), true))
                }
                None => Ok(J::from(NOFIELD)),
            }
        }
        "ntype" => {
            let id = inner_node(num_arg(ctx, a, 0) as NodeId);
            Ok(J::from(&rt().type_name(id)))
        }
        "nname" => {
            let id = num_arg(ctx, a, 0) as NodeId;
            Ok(J::from(&rt().scene.node(id).name))
        }
        "fprint" => {
            let h = handle_at(ctx, a, 0)?;
            let v = read(h);
            let json = str_arg(ctx, a, 1) == "json";
            let text = print_raw_js(ctx, |s| {
                if json {
                    s.field_value_to_json_string(&v)
                } else {
                    s.field_value_to_xml_string(&v)
                }
            });
            Ok(J::from(&text))
        }
        "strn" => {
            let h = handle_at(ctx, a, 0)?;
            let v = read(h);
            let text = if matches!(v, FieldValue::SFNode(_) | FieldValue::MFNode(_)) {
                rt().scene.node_value_to_string(&v)
            } else {
                to_vrml(&v)
            };
            Ok(J::from(&text))
        }
        "nprint" => {
            let id = num_arg(ctx, a, 0) as NodeId;
            let v = FieldValue::SFNode(Some(id));
            let json = str_arg(ctx, a, 1) == "json";
            let text = print_raw_js(ctx, |s| {
                if json {
                    s.field_value_to_json_string(&v)
                } else {
                    s.field_value_to_xml_string(&v)
                }
            });
            Ok(J::from(&text))
        }
        "nstream" => {
            let id = num_arg(ctx, a, 0) as NodeId;
            Ok(J::from(&with_live_clone_counts(|| {
                rt().scene
                    .node_value_to_string(&FieldValue::SFNode(Some(id)))
            })))
        }
        "nstr" => {
            let id = num_arg(ctx, a, 0) as NodeId;
            Ok(J::from(&with_live_clone_counts(|| {
                print_raw_js(ctx, |s| s.node_to_vrml_string(id))
            })))
        }
        "nfields" => {
            let id = inner_node(num_arg(ctx, a, 0) as NodeId);
            let arr = Arr::new();
            let n = rt().scene.node(id);

            // X3DBaseNode.getFieldDefinitions: a predefined field's definition holds the class's default value (`value`), a
            // user-defined field's the field itself (`handle`).
            let defs = match &n.kind {
                NodeKind::Builtin(t) => Some(
                    &rt().scene.table.types[*t]
                        .variant(rt().scene.spec_version_of(n.ctx))
                        .fields,
                ),
                _ => None,
            };

            for (i, f) in n.fields.iter().enumerate() {
                let o = Obj::new();
                o.set("name", f.name.as_str());
                o.set("accessType", f.access as i32);
                o.set("dataType", f.ty().name());

                match defs.and_then(|d| d.iter().find(|d| d.name == f.name)) {
                    Some(d) => {
                        o.set("value", value_data(&d.default));

                        if let Some(u) = &d.unit {
                            o.set("unit", u.as_str());
                        }
                    }
                    None => o.set("handle", encode(attach((id, FieldLoc::Pre(i))))),
                }

                arr.set(i, o);
            }

            for (i, f) in n.user_fields.iter().enumerate() {
                let o = Obj::new();
                o.set("name", f.name.as_str());
                o.set("accessType", f.access as i32);
                o.set("dataType", f.ty().name());
                o.set("handle", encode(attach((id, FieldLoc::User(i)))));
                arr.set(n.fields.len() + i, o);
            }

            Ok(arr.into())
        }
        // --- browser
        "bname" => Ok(J::from(&rt().scene.table.browser_name)),
        "bversion" => Ok(J::from(&rt().scene.table.browser_version)),
        "bdescription" => Ok(J::from(&rt().description)),
        "bsetdescription" => {
            let d = str_arg(ctx, a, 0);
            rt().description = d.clone();
            with_state(|s| s.description = d);
            Ok(J::Undefined)
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
                None => J::Null,
                Some(OptionValue::Bool(v)) => J::Bool(*v),
                Some(OptionValue::Int32(v)) => J::Int(*v),
                Some(OptionValue::Float(v) | OptionValue::Double(v)) => J::Number(*v),
                Some(OptionValue::String(v)) => J::from(v),
            })
        }
        // X3DBrowserContext.getBrowserOptions () … .getFieldDefinitions (): name, type, initial value.
        "browserDefs" => {
            use x3d_runtime::browser_options::OptionValue;

            let list = Arr::new();

            for (i, (name, v)) in x3d_runtime::browser_options::default_fields(&str_arg(ctx, a, 0))
                .entries()
                .enumerate()
            {
                let value = match v {
                    OptionValue::Bool(b) => J::Bool(*b),
                    OptionValue::Int32(x) => J::Int(*x),
                    OptionValue::Float(x) | OptionValue::Double(x) => data_number(*x),
                    OptionValue::String(s) => J::from(s),
                };
                let entry = data_array([J::from(name), J::from(v.type_name()), value]);

                list.set(i, entry);
            }

            Ok(list.into())
        }
        // X3DBrowserContext.getActiveLayer … (`None`: the default node), getCurrentViewer, and the default nodes.
        "bactive" => {
            let active = &rt().browser_active;
            let n = match str_arg(ctx, a, 0).as_str() {
                "layer" => active.layer,
                "navigationInfo" => active.navigation_info,
                _ => active.viewpoint,
            };

            Ok(node_data(&n))
        }
        "bdefault" => {
            let n = rt().default_node(&str_arg(ctx, a, 0));
            Ok(node_data(&n))
        }
        "bviewer" => Ok(J::from(&rt().browser_active.viewer)),
        "optionType" => {
            let name = str_arg(ctx, a, 0);

            Ok(match rt().browser_options.get_option(&name) {
                None => J::Null,
                Some(v) => J::from(v.type_name()),
            })
        }
        // Browser.setBrowserOption: the value is already converted to the field type (prelude).
        "setOption" => {
            use x3d_runtime::browser_options::OptionValue;

            let name = str_arg(ctx, a, 0);
            let Some(current) = rt().browser_options.get_option(&name).cloned() else {
                return Ok(J::Undefined);
            };
            let value = match current {
                OptionValue::Bool(_) => {
                    OptionValue::Bool(a.get(1).and_then(|v| v.as_bool()).unwrap_or(false))
                }
                OptionValue::Int32(_) => OptionValue::Int32(num_arg(ctx, a, 1) as i32),
                OptionValue::Float(_) => OptionValue::Float(num_arg(ctx, a, 1)),
                OptionValue::Double(_) => OptionValue::Double(num_arg(ctx, a, 1)),
                OptionValue::String(_) => OptionValue::String(str_arg(ctx, a, 1)),
            };

            rt().browser_options.set_option(&name, value);
            Ok(J::Undefined)
        }
        "time" => Ok(J::Number(rt().now)),
        "asyncError" => {
            // A callback returned a promise that rejected: `await callback .call (…)` throws into call__'s catch.
            let node = num_arg(ctx, a, 0) as NodeId;
            let name = str_arg(ctx, a, 1);
            let text = str_arg(ctx, a, 2);

            rt().report_script_error(node, &name, &text);
            Ok(J::Undefined)
        }
        "datenow" => Ok(J::Number(rt().date_override.unwrap_or(rt().now) * 1000.0)),
        "framerate" => Ok(J::Number(rt().frame_rate)),
        "browserTime" => Ok(J::Number(rt().world_state.initialized_time)),
        // Browser.nextViewpoint () and friends: queued for the viewer (X3DBrowser viewpoint API).
        "brequest" => {
            let r = str_arg(ctx, a, 0);
            rt().browser_requests.push(r);
            Ok(J::Undefined)
        }
        "specVersion" => {
            let script = num_arg(ctx, a, 0);
            let c = ctx_of_script(script);
            Ok(J::from(rt().scene.spec_version_of(c)))
        }
        // X3DBrowser.getBaseURL: the page's URL (an embedder without a page: the URL of the caller's scene).
        "baseURL" => {
            let script = num_arg(ctx, a, 0);
            let c = ctx_of_script(script);
            let page = rt().page_url.clone();
            let url = if page.is_empty() {
                rt().scene.world_url_of(c).to_string()
            } else {
                page
            };
            Ok(J::from(&url))
        }
        "worldURL" => {
            let script = num_arg(ctx, a, 0);
            let c = ctx_of_script(script);
            Ok(J::from(rt().scene.world_url_of(c)))
        }
        "createVrml" => {
            let text = str_arg(ctx, a, 0);
            let script = num_arg(ctx, a, 1);
            let parent = ctx_of_script(script);

            match rt().create_vrml_from_string(&text, parent) {
                Ok(roots) => {
                    let arr = Arr::new();

                    for (i, r) in roots.iter().enumerate() {
                        match r {
                            Some(n) => arr.set(i, *n),
                            None => arr.set(i, J::Null),
                        }
                    }

                    Ok(arr.into())
                }
                Err(e) => throw(ctx, e.strip_prefix("Error: ").unwrap_or(&e)),
            }
        }
        "cx" => ctx_api::op(ctx, a),
        "ctxOf" => Ok(J::Number(ctx_of_script(num_arg(ctx, a, 0)) as f64)),
        // X3DBrowser.createX3DFromString / createX3DFromURL: the request, or the message of the rejection.
        "browserScene" => {
            let script = num_arg(ctx, a, 0);
            let parent = ctx_of_script(script);
            let node = (script >= 0.0).then_some(script as NodeId);
            let o = Obj::new();

            let kind = str_arg(ctx, a, 1);

            if kind == "create" {
                // createScene (profile, ... components): the scene is there at once.
                let profile = a
                    .get(2)
                    .filter(|v| !v.is_undefined() && !v.is_null())
                    .map(|v| coerce_string(ctx, v));
                let components: Vec<(String, i32)> = a
                    .get(3)
                    .and_then(|v| v.elements())
                    .map(|items| {
                        items
                            .into_iter()
                            .flatten()
                            .filter_map(|e| {
                                // `pair [i]`: an index past the end reads undefined ("undefined", NaN); a read that throws
                                // drops the pair.
                                let p = e.elements()?;
                                let name = match p.first() {
                                    Some(x) => x.as_ref()?.to_text().ok()?,
                                    None => "undefined".to_string(),
                                };
                                let level = match p.get(1) {
                                    Some(x) => x.as_ref()?.to_number().ok()?,
                                    None => f64::NAN,
                                };

                                Some((name, level as i32))
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                o.set(
                    "ctx",
                    rt().browser_create_scene(profile, components, parent) as f64,
                );
            } else if kind == "load" {
                // Browser.loadURL (url, parameter): `id` is absent when the url only names a viewpoint of the world.
                let strings = |i: usize| -> Vec<String> {
                    a.get(i)
                        .and_then(|v| v.elements())
                        .map(|items| {
                            items
                                .into_iter()
                                .flatten()
                                .filter_map(|x| x.to_text().ok())
                                .collect()
                        })
                        .unwrap_or_default()
                };

                match rt().browser_load_url(strings(2), strings(3), parent, node) {
                    Ok(id) => o.set("id", id as f64),
                    Err(Some(message)) => o.set("error", message),
                    Err(None) => {}
                }
            } else if kind == "string" || kind == "json" {
                let text = str_arg(ctx, a, 2);
                let made = if kind == "string" {
                    rt().browser_create_from_string(&text, parent, node)
                } else {
                    rt().browser_import_json(&text, parent, node)
                };

                match made {
                    Ok(id) => o.set("id", id as f64),
                    Err(e) => o.set("error", e),
                }
            } else {
                let urls: Vec<String> = a
                    .get(2)
                    .and_then(|v| v.elements())
                    .map(|items| {
                        items
                            .into_iter()
                            .flatten()
                            .filter_map(|x| x.to_text().ok())
                            .collect()
                    })
                    .unwrap_or_default();

                o.set(
                    "id",
                    rt().browser_create_from_url(urls, parent, node) as f64,
                );
            }

            Ok(o.into())
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
            let o = Obj::new();

            o.set("id", rt().browser_replace_world(scene) as f64);
            Ok(o.into())
        }
        "sfnodeFromString" => {
            let text = str_arg(ctx, a, 0);
            let script = num_arg(ctx, a, 1);
            let parent = ctx_of_script(script);

            match rt().sfnode_from_string(&text, parent) {
                Ok(Some(n)) => Ok(J::Number(n as f64)),
                Ok(None) => throw(ctx, "SFNode.new: invalid argument."),
                Err(e) => throw(ctx, &e),
            }
        }
        "addRoute" | "deleteRoute" => {
            let script = num_arg(ctx, a, 0);
            let ctx_id = ctx_of_script(script);
            let (src, dst) = (
                a.get(1).and_then(arg_node).flatten(),
                a.get(3).and_then(arg_node).flatten(),
            );
            let (sf, df) = (str_arg(ctx, a, 2), str_arg(ctx, a, 4));

            let (Some(src), Some(dst)) = (src, dst) else {
                return throw(
                    ctx,
                    "Bad ROUTE specification: source node must be of type X3DNode.",
                );
            };

            if name == "addRoute" {
                if let Err(e) = rt().add_route(ctx_id, src, &sf, dst, &df) {
                    return throw(ctx, &e);
                }
            } else {
                rt().delete_route(ctx_id, src, &sf, dst, &df);
            }

            Ok(J::Undefined)
        }
        // --- X3DScene / X3DExecutionContext (the script's own execution context)
        "sceneInfo" => {
            let script = num_arg(ctx, a, 0);
            let c = ctx_of_script(script);
            let s = &rt().scene;
            let o = Obj::new();
            let root = s.scene_root(c);
            let encoding = s
                .scene_infos
                .get(&root)
                .map(|i| i.encoding.clone())
                .unwrap_or_else(|| s.encoding.clone());

            o.set("isScene", s.is_scene_root(c));
            o.set("encoding", encoding);
            match s.profile_of(c) {
                Some(p) => o.set("profile", p),
                None => o.set("profile", J::Null),
            }

            let comps = Arr::new();
            for (i, (n, l)) in s.components_of(c).iter().enumerate() {
                let e = Arr::new();
                e.set(0, n.as_str());
                e.set(1, *l);
                e.set(2, s.component_level_text(c, n, *l));
                comps.set(i, e);
            }
            o.set("components", comps);

            let units = Arr::new();
            for (i, u) in s.units_of(c).iter().enumerate() {
                let e = Arr::new();
                e.set(0, u.category.as_str());
                e.set(1, u.name.as_str());
                e.set(2, u.conversion_factor);
                units.set(i, e);
            }
            o.set("units", units);

            let meta = Arr::new();
            for (i, (k, v)) in s.meta_of(c).iter().enumerate() {
                let e = Arr::new();
                e.set(0, k.as_str());
                e.set(1, v.as_str());
                meta.set(i, e);
            }
            o.set("meta", meta);
            Ok(o.into())
        }
        "configTable" => {
            // The supported components and profiles (Configuration/SupportedComponents.js, SupportedProfiles.js) and the URL of the
            // script X_ITE runs from (Browser/Networking/URLs.js getScriptURL: the page).
            let t = rt().scene.table;
            let o = Obj::new();
            let comps = Arr::new();

            for (i, c) in t.components.iter().enumerate() {
                let e = Arr::new();
                let deps = Arr::new();

                for (j, d) in c.dependencies.iter().enumerate() {
                    deps.set(j, d.as_str());
                }

                e.set(0, c.name.as_str());
                e.set(1, c.title.as_str());
                e.set(2, c.level);
                e.set(3, c.external);
                e.set(4, deps);
                comps.set(i, e);
            }

            let profiles = Arr::new();

            for (i, p) in t.profiles.iter().enumerate() {
                let e = Arr::new();
                let list = Arr::new();

                for (j, (n, l)) in p.components.iter().enumerate() {
                    let pair = Arr::new();
                    pair.set(0, n.as_str());
                    pair.set(1, *l);
                    list.set(j, pair);
                }

                e.set(0, p.name.as_str());
                e.set(1, p.title.as_str());
                e.set(2, list);
                profiles.set(i, e);
            }

            o.set("components", comps);
            o.set("profiles", profiles);
            o.set("scriptURL", rt().page_url.as_str());
            Ok(o.into())
        }
        // X3DScene.addComponent / updateComponent: `level` is the number the scene keeps, `text` is `String (level)` of the
        // ComponentInfo the Script gave (any value; the printers write it).
        "addComponent" => {
            let script = num_arg(ctx, a, 0);
            let (name, level, text) = (
                str_arg(ctx, a, 1),
                num_arg(ctx, a, 2) as i32,
                str_arg(ctx, a, 3),
            );
            let root = rt().scene.scene_root(ctx_of_script(script));
            let comps = scene_components_mut(&mut rt().scene, root);

            if comps.iter().any(|(n, _)| *n == name) {
                return throw(
                    ctx,
                    &format!("Couldn't add component. Component '{name}' already exists."),
                );
            }

            comps.push((name.clone(), level));
            set_component_level_text(&mut rt().scene, root, name, level, text);
            Ok(J::Undefined)
        }
        "updateComponent" => {
            // X3DScene.updateComponent: a component of that name is replaced where it stands, else added.
            let script = num_arg(ctx, a, 0);
            let (name, level, text) = (
                str_arg(ctx, a, 1),
                num_arg(ctx, a, 2) as i32,
                str_arg(ctx, a, 3),
            );
            let root = rt().scene.scene_root(ctx_of_script(script));
            let comps = scene_components_mut(&mut rt().scene, root);

            match comps.iter_mut().find(|(n, _)| *n == name) {
                Some(c) => c.1 = level,
                None => comps.push((name.clone(), level)),
            }

            set_component_level_text(&mut rt().scene, root, name, level, text);
            Ok(J::Undefined)
        }
        "removeComponent" => {
            let script = num_arg(ctx, a, 0);
            let name = str_arg(ctx, a, 1);
            let root = rt().scene.scene_root(ctx_of_script(script));

            scene_components_mut(&mut rt().scene, root).retain(|(n, _)| *n != name);
            set_component_level_text(&mut rt().scene, root, name, 0, "0".into());
            Ok(J::Undefined)
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

            Ok(J::Undefined)
        }
        // prelude/bridge.js: field data in and out, node fields mirrored by the Script's objects.
        // The raw-value token of a node field's current value (see "fpush"), or undefined.
        "frawtok" => {
            let h = handle_at(ctx, a, 0)?;

            match slot_key(h).and_then(|k| {
                rt().scene
                    .raw_tokens
                    .get(&k)
                    .filter(|r| r.host == CURRENT.get())
                    .map(|r| r.token)
            }) {
                Some(t) => Ok(J::Number(t as f64)),
                None => Ok(J::Undefined),
            }
        }
        // The raw-value tokens of this host's realm that a field still holds (the bridge forgets the others).
        "frawlive" => {
            let arr = Arr::new();
            let cur = CURRENT.get();
            let mut live: Vec<u32> = rt()
                .scene
                .raw_tokens
                .values()
                .filter(|r| r.host == cur)
                .map(|r| r.token)
                .collect();

            live.sort_unstable();
            live.dedup();

            for (i, t) in live.into_iter().enumerate() {
                arr.set(i, t as f64);
            }

            Ok(arr.into())
        }
        "fexport" => {
            let h = handle_at(ctx, a, 0)?;
            Ok(value_data(&read(h)))
        }
        "fimport" => {
            let ty_name = str_arg(ctx, a, 0);
            let ty = if ty_name == "VrmlMatrix" {
                FieldType::SFMatrix4f
            } else {
                FieldType::from_name(&ty_name).unwrap_or(FieldType::SFBool)
            };
            let v = match a.get(1) {
                Some(x) => data_value(ty, x),
                None => data_value(ty, &ctx.realize(J::Undefined)?),
            };
            Ok(J::Number(encode(detached(v))))
        }
        // The Script's object of a node field changed it: with an event (X3DField.addEvent) or without (a value set in place).
        "fpush" => {
            let h = handle_at(ctx, a, 0)?;
            let event = a.get(2).and_then(|v| v.as_bool()).unwrap_or(true);
            let raw = a.get(3).and_then(|v| v.as_number()).map(|t| {
                let texts: Vec<Option<String>> = a
                    .get(4)
                    .and_then(|v| v.elements())
                    .map(|items| {
                        items
                            .into_iter()
                            .map(|x| x.and_then(|x| x.as_text()))
                            .collect()
                    })
                    .unwrap_or_default();
                x3d_scene::scene::RawValue {
                    host: CURRENT.get(),
                    token: t as u32,
                    texts: texts.into(),
                }
            });

            if let Some(k) = slot_key(h) {
                let ty = rt().field(k).ty();
                let v = match a.get(1) {
                    Some(x) => data_value(ty, x),
                    None => data_value(ty, &ctx.realize(J::Undefined)?),
                };

                if event {
                    rt().set_value(k, v);
                } else {
                    rt().set_raw(k, v);
                }

                // The Script's value held components that are not numbers: their token goes with the value (Scene::raw_tokens).
                if let Some(r) = raw {
                    rt().scene.raw_tokens.insert(k, r);
                }

                rt().js_clean(k);
            }

            Ok(J::Undefined)
        }
        "fmirror" => {
            let h = handle_at(ctx, a, 0)?;

            match slot_key(h) {
                Some(k) => {
                    rt().js_mirror.entry(k).or_insert(false);
                    Ok(J::from(&fkey_id(k)))
                }
                None => Ok(J::Null),
            }
        }
        // The ids and data of the mirrored fields whose value changed since the last call.
        "fdirty" => {
            let dirty = rt().take_js_dirty();
            let mut items = vec![];

            for k in dirty {
                items.push(J::from(&fkey_id(k)));
                items.push(value_data(rt().value(k)));
            }

            Ok(data_array(items))
        }
        // X3DField.fromVRMLString / fromXMLString (value, scene): the data of the parsed value. `script` names the scene (the
        // Script's execution context) or is null; a value that cannot be read throws X_ITE's error.
        "parsefield" => {
            let ty_name = str_arg(ctx, a, 0);
            let ty = if ty_name == "VrmlMatrix" {
                FieldType::SFMatrix4f
            } else {
                FieldType::from_name(&ty_name).unwrap_or(FieldType::SFBool)
            };
            let text = str_arg(ctx, a, 1);
            let xml = str_arg(ctx, a, 2) == "xml";
            let name = str_arg(ctx, a, 3);
            let scene = a.get(4).and_then(|v| v.as_number()).map(ctx_of_script);
            let unit = a.get(5).and_then(|v| v.as_text());

            match rt().field_from_string(ty, &text, scene, xml, unit, &name) {
                Ok(Some(v)) => Ok(value_data(&v)),
                Ok(None) => throw(ctx, &format!("Couldn't read value for field '{name}'.")),
                Err(Some(message)) => throw(ctx, &message),
                // VRMLParser without an execution context: `executionContext .createNode (…)` of null.
                Err(None) => {
                    throw_type(ctx, "Cannot read properties of null (reading 'createNode')")
                }
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
                        scene.table.types[t]
                            .variant(spec)
                            .fields
                            .iter()
                            .find(|d| &d.name == name)
                            .and_then(|d| d.unit.clone())
                    }
                    _ => None,
                }
            });

            match unit {
                Some(u) => Ok(J::from(&u)),
                None => Ok(J::Undefined),
            }
        }
        // X3DGenerator createFloatFormat / createFloatExponentialFormat: Intl.NumberFormat ("en", { notation, maximumSignificantDigits }).
        "intlfmt" => {
            let scientific = a.first().and_then(|v| v.as_bool()).unwrap_or(false);
            let precision = num_arg(ctx, a, 1) as usize;
            let value = num_arg(ctx, a, 2);
            let text = if scientific {
                x3d_fields::format::intl_scientific_lower(value, precision)
            } else {
                x3d_fields::format::intl_standard(value, precision)
            };
            Ok(J::from(&text))
        }
        "ctxObserve" => {
            rt().ctx_events.observed = true;
            rt().ctx_events.watched.insert(num_arg(ctx, a, 0) as usize);

            Ok(J::Undefined)
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

            Ok(J::Undefined)
        }
        "setProfile" => {
            let script = num_arg(ctx, a, 0);
            let profile = a
                .get(1)
                .filter(|v| v.is_string())
                .map(|_| str_arg(ctx, a, 1));
            let root = rt().scene.scene_root(ctx_of_script(script));

            match rt().scene.scene_infos.get_mut(&root) {
                Some(i) => i.profile = profile,
                None => rt().scene.profile = profile,
            }

            rt().ctx_event(root, CtxField::Profile, 1);
            Ok(J::Undefined)
        }
        "metaData" => {
            // X3DScene.setMetaData / addMetaData / removeMetaData.
            let script = num_arg(ctx, a, 0);
            let (mode, name) = (str_arg(ctx, a, 1), str_arg(ctx, a, 2));
            let values: Vec<String> = match a.get(3).and_then(|v| v.elements()) {
                Some(items) => items
                    .into_iter()
                    .flatten()
                    .map(|v| coerce_string(ctx, &v))
                    .collect(),
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
            Ok(J::Undefined)
        }
        "routeList" => {
            let script = num_arg(ctx, a, 0);
            let c = ctx_of_script(script);
            let routes: Vec<_> = rt()
                .scene
                .ctx(c)
                .routes
                .iter()
                .map(|r| {
                    (
                        r.source_node,
                        r.source_field.clone(),
                        r.destination_node,
                        r.destination_field.clone(),
                    )
                })
                .collect();
            let arr = Arr::new();

            for (i, (sn, sf, dn, df)) in routes.into_iter().enumerate() {
                let o = Obj::new();
                o.set("sourceNode", node_js(Some(sn)));
                o.set("sourceField", sf);
                o.set("destinationNode", node_js(Some(dn)));
                o.set("destinationField", df);
                arr.set(i, o);
            }

            Ok(arr.into())
        }
        "rootNode" => {
            // X3DScene.addRootNode / removeRootNode.
            let script = num_arg(ctx, a, 0);
            let add = str_arg(ctx, a, 1) == "add";
            let node = a.get(2).and_then(arg_node).flatten();
            let c = ctx_of_script(script);
            let mut roots = rt().scene.ctx(c).root_nodes.clone();

            if add {
                if roots.contains(&node) {
                    return Ok(J::Undefined);
                }

                roots.push(node);
            } else {
                roots.retain(|n| *n != node);
            }

            // rootNodes .push (node) / .assign (...): the rootNodes field fires.
            rt().set_root_nodes(c, roots);
            Ok(J::Undefined)
        }
        "exportedNode" => {
            // X3DScene.addExportedNode / getExportedNode.
            let script = num_arg(ctx, a, 0);
            let (mode, name) = (str_arg(ctx, a, 1), str_arg(ctx, a, 2));
            let root = rt().scene.scene_root(ctx_of_script(script));

            if mode == "get" {
                return match rt().scene.get_exported_node(root, &name) {
                    Some(n) => Ok(node_js(Some(n))),
                    None => throw(ctx, &format!("Exported node '{name}' not found.")),
                };
            }

            if rt().scene.get_exported_node(root, &name).is_some() {
                return throw(
                    ctx,
                    &format!("Couldn't add exported node: exported name '{name}' already in use."),
                );
            }

            let Some(node) = a.get(3).and_then(arg_node).flatten() else {
                return throw(
                    ctx,
                    "Couldn't update exported node: node must be of type X3DNode.",
                );
            };

            match rt().scene.update_exported_node(root, &name, node, "") {
                Ok(()) => Ok(J::Undefined),
                Err(e) => throw(ctx, &e),
            }
        }
        "nodeArrays" => {
            // namedNodes (SFNode), importedNodes (X3DImportedNode), exportedNodes (X3DExportedNode).
            let script = num_arg(ctx, a, 0);
            let which = str_arg(ctx, a, 1);
            let c = ctx_of_script(script);
            let s = &rt().scene;
            let arr = Arr::new();

            match which.as_str() {
                "named" => {
                    for (i, &n) in s.ctx(c).named_nodes.values().iter().enumerate() {
                        arr.set(i, node_js(Some(n)));
                    }
                }
                "imported" => {
                    for (i, &id) in s.ctx(c).imported.values().iter().enumerate() {
                        let m = &s.imported[id];
                        let o = Obj::new();
                        o.set("id", id as f64);
                        o.set("inlineNode", node_js(Some(m.inline)));
                        o.set("exportedName", m.exported_name.as_str());
                        o.set("importedName", m.imported_name.as_str());
                        o.set("description", m.description.as_str());
                        arr.set(i, o);
                    }
                }
                _ => {
                    let root = s.scene_root(c);
                    for (i, &id) in s.ctx(root).exported.values().iter().enumerate() {
                        let m = &s.exported[id];
                        let o = Obj::new();
                        o.set("id", id as f64);
                        o.set("exportedName", m.exported_name.as_str());
                        o.set("description", m.description.as_str());
                        o.set("localNode", node_js(Some(m.local)));
                        arr.set(i, o);
                    }
                }
            }

            Ok(arr.into())
        }
        "sceneString" => {
            let script = num_arg(ctx, a, 0);
            let root = rt().scene.scene_root(ctx_of_script(script));
            let format = str_arg(ctx, a, 1);
            if format != "xml" {
                rt().ctx_streamed(root);
            }
            let text = with_live_clone_counts(|| {
                print_raw_js(ctx, |s| match format.as_str() {
                    "xml" => s.to_xml_string_ctx(root),
                    "json" => s.to_json_string_ctx(root),
                    _ => s.to_vrml_string_ctx(root),
                })
            });
            Ok(J::from(&text))
        }
        "worldInfos" => {
            let script = num_arg(ctx, a, 0);
            let c = ctx_of_script(script);
            let list: Vec<Option<NodeId>> = rt()
                .world_infos
                .get(&c)
                .map(|l| l.iter().map(|n| Some(*n)).collect())
                .unwrap_or_default();
            Ok(new_value_js(FieldValue::MFNode(list)))
        }
        "rootNodes" => {
            let script = num_arg(ctx, a, 0);
            let c = ctx_of_script(script);
            let roots = rt().scene.ctx(c).root_nodes.clone();
            Ok(new_value_js(FieldValue::MFNode(roots)))
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
                    Ok(node_js(Some(n)))
                }
                None => throw(ctx, &format!("Unknown node type '{ty}'.")),
            }
        }
        "createProto" => {
            let script = num_arg(ctx, a, 0);
            let ty = str_arg(ctx, a, 1);
            let c = ctx_of_script(script);

            match rt().create_proto_setup(c, &ty) {
                Some(n) => Ok(node_js(Some(n))),
                None => throw(ctx, &format!("Unknown proto or externproto type '{ty}'.")),
            }
        }
        "namedNode" => {
            let script = num_arg(ctx, a, 0);
            let nn = str_arg(ctx, a, 1);
            let c = ctx_of_script(script);

            match rt().scene.get_named_node(c, &nn) {
                Some(n) => Ok(node_js(Some(n))),
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
fn scene_components_mut(
    s: &mut x3d_scene::Scene,
    root: x3d_scene::CtxId,
) -> &mut Vec<(String, i32)> {
    if s.scene_infos.contains_key(&root) {
        &mut s.scene_infos.get_mut(&root).unwrap().components
    } else {
        &mut s.components
    }
}

/// The level text of a component (see `Scene::component_level_texts`); none when it is the number's own.
fn set_component_level_text(
    s: &mut x3d_scene::Scene,
    root: x3d_scene::CtxId,
    name: String,
    level: i32,
    text: String,
) {
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

#[cfg(test)]
mod slot_tests {
    use super::*;

    /// A handle kept from before its slot was released does not reach the value that takes the slot.
    #[test]
    fn a_stale_handle_does_not_reach_a_reused_slot() {
        let _table = Table::new();
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
        assert_eq!(
            decode(old),
            None,
            "the old handle does not reach the new value"
        );
        assert!(
            matches!(decode(encode(n)).map(read), Some(FieldValue::SFInt32(2))),
            "the new handle works"
        );
    }

    /// An element view keeps its MF slot; the MF slot goes after its last element.
    #[test]
    fn an_element_keeps_its_parent() {
        let _table = Table::new();
        let mf = detached(FieldValue::MFInt32(vec![5, 6]));
        let e = element(mf, 1);

        with_state(|s| s.owners[e as usize] += 1);
        with_state(State::release_fresh);
        assert!(
            decode(encode(mf)).is_some(),
            "the parent of a live element stays"
        );
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
