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

const PRELUDE: &str = include_str!("prelude.js");
const NOFIELD: &str = "\u{0}nofield";

#[derive(Clone, Debug)]
enum Slot {
    Field(FKey),
    Value(FieldValue),
    Element(u32, usize),
}

struct State {
    rt: *mut Runtime,
    slots: Vec<Slot>,
    attached: HashMap<FKey, u32>,
    description: String,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn with_state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    STATE.with(|s| f(s.borrow_mut().as_mut().expect("script engine state")))
}

/// The runtime of the world whose script is running.
fn rt<'a>() -> &'a mut Runtime {
    let p = with_state(|s| s.rt);
    // SAFETY: set by QuickJsHost for the duration of a host call; the runtime outlives it.
    unsafe { &mut *p }
}

pub struct QuickJsHost {
    _runtime: rquickjs::Runtime,
    context: Context,
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

        STATE.with(|s| *s.borrow_mut() = Some(State { rt: std::ptr::null_mut(), slots: vec![], attached: HashMap::new(), description: String::new() }));

        context.with(|ctx| {
            let f = Function::new(ctx.clone(), native).expect("native __x");
            ctx.globals().set("__x", f).unwrap();

            if let Err(e) = ctx.eval::<(), _>(PRELUDE) {
                let msg = describe(&ctx, e);
                panic!("prelude.js failed: {msg}");
            }
        });

        QuickJsHost { _runtime: runtime, context, deadline }
    }

    fn enter(&self, rt: &mut Runtime) {
        with_state(|s| s.rt = rt as *mut Runtime);
        self.deadline.set(Some(std::time::Instant::now() + std::time::Duration::from_secs(10)));
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

        let fields: Vec<(String, u8, u32)> = (0..rt.scene.node(node).user_fields.len())
            .map(|i| {
                let f = &rt.scene.node(node).user_fields[i];
                (f.name.clone(), f.access, attach((node, FieldLoc::User(i))))
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

            match create.call::<_, Vec<bool>>((node, source, cbs, list)) {
                Ok(v) => Ok(v),
                Err(e) => Err(ScriptError(describe(&ctx, e))),
            }
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

            match result {
                Ok(v) if v.is_undefined() => Ok(()),
                Ok(v) => Err(ScriptError(v.get::<Coerced<String>>().map(|c| c.0).unwrap_or_default())),
                Err(e) => Err(ScriptError(describe(&ctx, e))),
            }
        })
    }
}

// --- slots ---------------------------------------------------------------------------------

fn attach(k: FKey) -> u32 {
    with_state(|s| {
        if let Some(h) = s.attached.get(&k) {
            return *h;
        }

        s.slots.push(Slot::Field(k));
        let h = (s.slots.len() - 1) as u32;
        s.attached.insert(k, h);
        h
    })
}

fn detached(v: FieldValue) -> u32 {
    with_state(|s| {
        s.slots.push(Slot::Value(v));
        (s.slots.len() - 1) as u32
    })
}

fn element(parent: u32, i: usize) -> u32 {
    with_state(|s| {
        s.slots.push(Slot::Element(parent, i));
        (s.slots.len() - 1) as u32
    })
}

fn slot(h: u32) -> Slot {
    with_state(|s| s.slots[h as usize].clone())
}

fn read(h: u32) -> FieldValue {
    match slot(h) {
        Slot::Field(k) => rt().value(k).clone(),
        Slot::Value(v) => v,
        Slot::Element(p, i) => mf_get(&read(p), i).unwrap_or(FieldValue::SFBool(false)),
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

/// A field handle passed from JS as `{ __h }`.
fn arg_handle(v: &Value<'_>) -> Option<u32> {
    obj_num(v, "__h").filter(|x| !x.is_nan()).map(|x| x as u32)
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
    o.set("__h", h).unwrap();
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
            Ok(Value::new_number(ctx.clone(), detached(v) as f64))
        }
        "fv" => {
            let h = num_arg(ctx, a, 0) as u32;
            Ok(field_value_js(ctx, h))
        }
        "fname" => {
            let h = num_arg(ctx, a, 0) as u32;
            let n = slot_key(h).map(|k| rt().field(k).name.clone()).unwrap_or_default();
            Ok(rquickjs::String::from_str(ctx.clone(), &n)?.into_value())
        }
        "faccess" => {
            let h = num_arg(ctx, a, 0) as u32;
            let n = slot_key(h).map(|k| rt().field(k).access).unwrap_or(access::INPUT_OUTPUT);
            Ok(Value::new_int(ctx.clone(), n as i32))
        }
        "set" => {
            let h = num_arg(ctx, a, 0) as u32;
            let ty = read(h).field_type();

            if let Some(v) = a.get(1).and_then(|v| convert(ctx, ty, v)) {
                write(h, v);
            }

            Ok(undefined)
        }
        "copy" => {
            let h = num_arg(ctx, a, 0) as u32;
            Ok(new_value_js(ctx, read(h)))
        }
        "eq" => {
            let h = num_arg(ctx, a, 0) as u32;
            let other = value_arg(a, 1);
            Ok(Value::new_bool(ctx.clone(), other.is_some_and(|o| read(h).equals(&o))))
        }
        "isdef" => {
            let h = num_arg(ctx, a, 0) as u32;
            Ok(Value::new_bool(ctx.clone(), read(h).is_default_value()))
        }
        "str" => {
            let h = num_arg(ctx, a, 0) as u32;
            Ok(rquickjs::String::from_str(ctx.clone(), &to_vrml(&read(h)))?.into_value())
        }
        "g" => {
            let h = num_arg(ctx, a, 0) as u32;
            component_get(ctx, h, a.get(1))
        }
        "s" => {
            let h = num_arg(ctx, a, 0) as u32;
            component_set(ctx, h, a.get(1), a.get(2));
            Ok(undefined)
        }
        "m" => {
            let h = num_arg(ctx, a, 0) as u32;
            let m = str_arg(ctx, a, 1);
            method(ctx, h, &m, &a[2.min(a.len())..])
        }
        "len" => {
            let h = num_arg(ctx, a, 0) as u32;
            Ok(Value::new_int(ctx.clone(), read(h).len() as i32))
        }
        "setlen" => {
            let h = num_arg(ctx, a, 0) as u32;
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
            let h = num_arg(ctx, a, 0) as u32;
            let i = num_arg(ctx, a, 1) as usize;
            let v = read(h);

            match mf_get(&v, i) {
                None => Ok(undefined),
                Some(sf) if is_scalar(sf.field_type()) || sf.field_type() == FieldType::SFNode => Ok(primitive_or_handle(ctx, &sf, 0, false)),
                Some(sf) => Ok(handle_js(ctx, element(h, i), sf.field_type().name(), false)),
            }
        }
        "put" => {
            let h = num_arg(ctx, a, 0) as u32;
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
            let id = num_arg(ctx, a, 0) as NodeId;
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
            let id = num_arg(ctx, a, 0) as NodeId;
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
            let id = num_arg(ctx, a, 0) as NodeId;
            let field = str_arg(ctx, a, 1);
            Ok(Value::new_bool(ctx.clone(), rt().get_field(id, &field).is_some()))
        }
        "ntype" => {
            let id = num_arg(ctx, a, 0) as NodeId;
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
            let id = num_arg(ctx, a, 0) as NodeId;
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
        "time" => Ok(Value::new_number(ctx.clone(), rt().now)),
        // Browser.nextViewpoint () and friends: queued for the viewer (X3DBrowser viewpoint API).
        "brequest" => {
            let r = str_arg(ctx, a, 0);
            rt().browser_requests.push(r);
            Ok(undefined)
        }
        "specVersion" => {
            let script = num_arg(ctx, a, 0);
            let c = if script >= 0.0 { rt().scene.node(script as NodeId).ctx } else { ROOT };
            Ok(rquickjs::String::from_str(ctx.clone(), rt().scene.spec_version_of(c))?.into_value())
        }
        "worldURL" => {
            let script = num_arg(ctx, a, 0);
            let c = if script >= 0.0 { rt().scene.node(script as NodeId).ctx } else { ROOT };
            Ok(rquickjs::String::from_str(ctx.clone(), rt().scene.world_url_of(c))?.into_value())
        }
        "createVrml" => {
            let text = str_arg(ctx, a, 0);
            let script = num_arg(ctx, a, 1);
            let parent = if script >= 0.0 { rt().scene.node(script as NodeId).ctx } else { ROOT };

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
        "createVrmlFromURL" => Ok(undefined),
        "sfnodeFromString" => {
            let text = str_arg(ctx, a, 0);
            let script = num_arg(ctx, a, 1);
            let parent = if script >= 0.0 { rt().scene.node(script as NodeId).ctx } else { ROOT };

            match rt().sfnode_from_string(&text, parent) {
                Ok(Some(n)) => Ok(Value::new_number(ctx.clone(), n as f64)),
                Ok(None) => throw(ctx, "SFNode.new: invalid argument."),
                Err(e) => throw(ctx, &e),
            }
        }
        "addRoute" | "deleteRoute" => {
            let script = num_arg(ctx, a, 0) as NodeId;
            let ctx_id = rt().scene.node(script).ctx;
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
        "rootNodes" => {
            let script = num_arg(ctx, a, 0) as NodeId;
            let c = rt().scene.node(script).ctx;
            let roots = rt().scene.ctx(c).root_nodes.clone();
            Ok(new_value_js(ctx, FieldValue::MFNode(roots)))
        }
        "createNode" => {
            let script = num_arg(ctx, a, 0) as NodeId;
            let ty = str_arg(ctx, a, 1);
            let c = rt().scene.node(script).ctx;

            match rt().create_node_setup(c, &ty) {
                Some(n) => Ok(node_js(ctx, Some(n))),
                None => throw(ctx, &format!("Unknown node type '{ty}'.")),
            }
        }
        "createProto" => {
            let script = num_arg(ctx, a, 0) as NodeId;
            let ty = str_arg(ctx, a, 1);
            let c = rt().scene.node(script).ctx;

            match rt().create_proto_setup(c, &ty) {
                Some(n) => Ok(node_js(ctx, Some(n))),
                None => throw(ctx, &format!("Unknown proto or externproto type '{ty}'.")),
            }
        }
        "namedNode" => {
            let script = num_arg(ctx, a, 0) as NodeId;
            let nn = str_arg(ctx, a, 1);
            let c = rt().scene.node(script).ctx;

            match rt().scene.get_named_node(c, &nn) {
                Some(n) => Ok(node_js(ctx, Some(n))),
                None => throw(ctx, &format!("Named node '{nn}' not found.")),
            }
        }
        other => throw(ctx, &format!("x3d-script: unknown native op '{other}'")),
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
