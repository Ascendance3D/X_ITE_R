//! Components/Followers: X3DFollowerNode.js, X3DChaserNode.js, X3DDamperNode.js, the 14 nodes
//! (Color/Coordinate/Orientation/Position/Position2D/Scalar/TexCoord2D x Chaser/Damper) and
//! Browser/Followers/X3DArrayFollowerTemplate.js + X3DArrayChaserTemplate.js (Coordinate and
//! TexCoord2D). Pinned reference: oracle/x_ite-pin (fork commit 34cdf9f97).
//!
//! The JavaScript is written once against `getVector ()`, `equals`, `interpolate`, `step`,
//! `assign` and `duplicate`; here that is the [`Kind`] trait and the generic [`Chaser`] /
//! [`Damper`]. Each method returns the events the JavaScript generates, in order ([`Out`]); the
//! runtime applies them (`_value_changed = value`, `_isActive = value`) and keeps the node in the
//! browser's prepareEvents interests exactly while `getLive () && isActive`
//! (X3DFollowerNode.set_live__). The browser's interests are a Map, so followers tick in the
//! order they last became active.
//!
//! Number types follow X_ITE: SFFloat / SFVec3f / SFRotation ... are JavaScript doubles; only the
//! MFVec3f / MFVec2f fields (Coordinate*, TexCoord*) are Float32Arrays, so only their
//! `value_changed` is rounded to float32. Internal buffers are doubles in both.

use super::State;
use crate::*;
use x3d_fields::jsmath;
use x3d_fields::math::{Color3, Rotation4, Vector2, Vector3};

const NUM_BUFFERS: usize = 60;

/// An event X_ITE generates while a follower runs, in order.
#[derive(Debug)]
pub enum Out {
    /// `this ._value_changed = value` (always an event).
    Value(FieldValue),
    /// `this ._isActive = value` (only sent when it changes).
    Active(bool),
}

/// Browser and field state a follower reads.
#[derive(Debug, Clone)]
pub struct Env {
    /// `getBrowser () .getCurrentTime ()`.
    pub now: f64,
    /// `getBrowser () .currentFrameRate`.
    pub frame_rate: f64,
    pub duration: f64,
    pub tau: f64,
    /// The raw tolerance field (X3DDamperNode.getTolerance maps negative to 1e-4).
    pub tolerance: f64,
    pub order: i32,
    /// The current value of `value_changed`.
    pub current: FieldValue,
}

pub trait Follower: std::fmt::Debug {
    fn initialize(&mut self, e: &Env, initial_value: &FieldValue, initial_destination: &FieldValue, out: &mut Vec<Out>);
    fn set_value(&mut self, e: &Env, value: &FieldValue, out: &mut Vec<Out>);
    fn set_destination(&mut self, e: &Env, destination: &FieldValue, out: &mut Vec<Out>);
    fn set_duration(&mut self, _e: &Env) {}
    fn prepare_events(&mut self, e: &Env, out: &mut Vec<Out>);
}

// --- value kinds ---------------------------------------------------------------------------

/// What differs between the node types: getVector, getValue / getInitialValue conversions,
/// setValue, equals, interpolate, step (and the array overrides).
pub trait Kind: std::fmt::Debug + 'static {
    type S: Clone + std::fmt::Debug;

    /// getVector (): for arrays an empty array.
    fn zero() -> Self::S;
    /// getValue () / getDestination () / getInitialValue () / getInitialDestination () of a field.
    fn read(v: &FieldValue) -> Self::S;
    /// `this .setValue (value)` where value is a field (always sends an event) or a number.
    fn write_always(s: &Self::S) -> FieldValue;
    /// `this .setValue (value)` from prepareEvents: arrays send no event when empty and unchanged.
    fn write(s: &Self::S, _current: &FieldValue) -> Option<FieldValue> {
        Some(Self::write_always(s))
    }
    fn equals(lhs: &Self::S, rhs: &Self::S, tolerance: f64) -> bool;
    fn interpolate(source: &Self::S, destination: &Self::S, weight: f64) -> Self::S;
    /// X3DChaserNode step: `output += (value1 - value2) * t` (per kind).
    fn step(output: &mut Self::S, value1: &Self::S, value2: &Self::S, t: f64);
    /// X3DArrayFollowerTemplate.set_destination__: resize a buffer to the destination's length.
    fn resize(_buffer: &mut Self::S, _length: usize) {}
    fn field_length(_v: &FieldValue) -> usize {
        0
    }
    const ARRAY: bool = false;
}

/// The vector types (Vector2, Vector3) as elements of vector kinds and arrays.
pub trait Elem: Copy + std::fmt::Debug + 'static {
    fn zero() -> Self;
    fn lerp(&mut self, other: &Self, t: f64);
    fn add(&mut self, other: &Self);
    fn subtract(&mut self, other: &Self);
    fn multiply(&mut self, t: f64);
    fn norm(&self) -> f64;
    fn read_sf(v: &FieldValue) -> Self;
    fn write_sf(&self) -> FieldValue;
    fn read_mf(v: &FieldValue) -> Vec<Self>;
    fn write_mf(v: &[Self]) -> FieldValue;
}

impl Elem for Vector3 {
    fn zero() -> Self {
        Vector3::ZERO
    }

    fn lerp(&mut self, other: &Self, t: f64) {
        Vector3::lerp(self, other, t);
    }

    fn add(&mut self, other: &Self) {
        Vector3::add(self, other);
    }

    fn subtract(&mut self, other: &Self) {
        Vector3::subtract(self, other);
    }

    fn multiply(&mut self, t: f64) {
        Vector3::multiply(self, t);
    }

    fn norm(&self) -> f64 {
        Vector3::norm(self)
    }

    fn read_sf(v: &FieldValue) -> Self {
        match v {
            FieldValue::SFVec3f(v) => *v,
            _ => Vector3::ZERO,
        }
    }

    fn write_sf(&self) -> FieldValue {
        FieldValue::SFVec3f(*self)
    }

    fn read_mf(v: &FieldValue) -> Vec<Self> {
        match v {
            FieldValue::MFVec3f(a) => a.chunks_exact(3).map(|c| Vector3::new(c[0] as f64, c[1] as f64, c[2] as f64)).collect(),
            _ => vec![],
        }
    }

    fn write_mf(v: &[Self]) -> FieldValue {
        // typed array element writes: Float32Array stores Math.fround of each component.
        FieldValue::MFVec3f(v.iter().flat_map(|e| [e.x as f32, e.y as f32, e.z as f32]).collect())
    }
}

impl Elem for Vector2 {
    fn zero() -> Self {
        Vector2::ZERO
    }

    fn lerp(&mut self, other: &Self, t: f64) {
        Vector2::lerp(self, other, t);
    }

    fn add(&mut self, other: &Self) {
        Vector2::add(self, other);
    }

    fn subtract(&mut self, other: &Self) {
        Vector2::subtract(self, other);
    }

    fn multiply(&mut self, t: f64) {
        Vector2::multiply(self, t);
    }

    fn norm(&self) -> f64 {
        Vector2::norm(self)
    }

    fn read_sf(v: &FieldValue) -> Self {
        match v {
            FieldValue::SFVec2f(v) => *v,
            _ => Vector2::ZERO,
        }
    }

    fn write_sf(&self) -> FieldValue {
        FieldValue::SFVec2f(*self)
    }

    fn read_mf(v: &FieldValue) -> Vec<Self> {
        match v {
            FieldValue::MFVec2f(a) => a.chunks_exact(2).map(|c| Vector2::new(c[0] as f64, c[1] as f64)).collect(),
            _ => vec![],
        }
    }

    fn write_mf(v: &[Self]) -> FieldValue {
        FieldValue::MFVec2f(v.iter().flat_map(|e| [e.x as f32, e.y as f32]).collect())
    }
}

/// PositionChaser / PositionDamper / PositionChaser2D / PositionDamper2D: the default
/// X3DFollowerNode methods on a vector.
#[derive(Debug)]
pub struct VecK<E: Elem>(std::marker::PhantomData<E>);

impl<E: Elem> Kind for VecK<E> {
    type S = E;

    fn zero() -> E {
        E::zero()
    }

    fn read(v: &FieldValue) -> E {
        E::read_sf(v)
    }

    fn write_always(s: &E) -> FieldValue {
        s.write_sf()
    }

    fn equals(lhs: &E, rhs: &E, tolerance: f64) -> bool {
        let mut a = *lhs;
        a.subtract(rhs);
        a.norm() < tolerance
    }

    fn interpolate(source: &E, destination: &E, weight: f64) -> E {
        let mut v = *source;
        v.lerp(destination, weight);
        v
    }

    fn step(output: &mut E, value1: &E, value2: &E, t: f64) {
        let mut delta = *value1;
        delta.subtract(value2);
        delta.multiply(t);
        output.add(&delta);
    }
}

/// ScalarChaser / ScalarDamper: plain numbers.
#[derive(Debug)]
pub struct ScalarK;

impl Kind for ScalarK {
    type S = f64;

    fn zero() -> f64 {
        0.0
    }

    fn read(v: &FieldValue) -> f64 {
        match v {
            FieldValue::SFFloat(v) => *v,
            _ => 0.0,
        }
    }

    fn write_always(s: &f64) -> FieldValue {
        FieldValue::SFFloat(*s)
    }

    fn equals(lhs: &f64, rhs: &f64, tolerance: f64) -> bool {
        (lhs - rhs).abs() < tolerance
    }

    fn interpolate(source: &f64, destination: &f64, weight: f64) -> f64 {
        source + weight * (destination - source)
    }

    fn step(output: &mut f64, value1: &f64, value2: &f64, t: f64) {
        *output += (value1 - value2) * t;
    }
}

/// ColorChaser / ColorDamper: HSV vectors (getHSV / setHSV, Color3.lerp).
#[derive(Debug)]
pub struct ColorK;

fn hsv_vec(a: [f64; 3]) -> Vector3 {
    Vector3::new(a[0], a[1], a[2])
}

impl Kind for ColorK {
    type S = Vector3;

    fn zero() -> Vector3 {
        Vector3::ZERO
    }

    fn read(v: &FieldValue) -> Vector3 {
        match v {
            FieldValue::SFColor(c) => hsv_vec(c.get_hsv()),
            _ => Vector3::ZERO,
        }
    }

    fn write_always(s: &Vector3) -> FieldValue {
        let mut c = Color3::default();
        c.set_hsv(s.x, s.y, s.z);
        FieldValue::SFColor(c)
    }

    fn equals(lhs: &Vector3, rhs: &Vector3, tolerance: f64) -> bool {
        let mut a = *lhs;
        a.subtract(rhs);
        a.norm() < tolerance
    }

    fn interpolate(source: &Vector3, destination: &Vector3, weight: f64) -> Vector3 {
        hsv_vec(super::interp::hsv_lerp(source.to_array(), destination.to_array(), weight))
    }

    fn step(output: &mut Vector3, value1: &Vector3, value2: &Vector3, t: f64) {
        // deltaOut .assign (this .output) .add (value1) .subtract (value2); Color3 .lerp (output, deltaOut, t, output)
        let mut delta = *output;
        delta.add(value1);
        delta.subtract(value2);
        *output = hsv_vec(super::interp::hsv_lerp(output.to_array(), delta.to_array(), t));
    }
}

/// OrientationChaser / OrientationDamper: Rotation4 with slerp and the angle of the difference.
#[derive(Debug)]
pub struct RotK;

impl Kind for RotK {
    type S = Rotation4;

    fn zero() -> Rotation4 {
        Rotation4::default()
    }

    fn read(v: &FieldValue) -> Rotation4 {
        match v {
            FieldValue::SFRotation(r) => *r,
            _ => Rotation4::default(),
        }
    }

    fn write_always(s: &Rotation4) -> FieldValue {
        FieldValue::SFRotation(*s)
    }

    fn equals(lhs: &Rotation4, rhs: &Rotation4, tolerance: f64) -> bool {
        let mut a = *lhs;
        a.inverse().mult_right(rhs);
        a.angle().abs() < tolerance
    }

    fn interpolate(source: &Rotation4, destination: &Rotation4, weight: f64) -> Rotation4 {
        let mut r = *source;
        r.slerp(destination, weight);
        r
    }

    fn step(output: &mut Rotation4, value1: &Rotation4, value2: &Rotation4, t: f64) {
        // this .deltaOut .assign (value2) .inverse () .multRight (value1) .multLeft (this .output);
        // this .output .slerp (this .deltaOut, t);
        let mut delta = *value2;
        delta.inverse().mult_right(value1).mult_left(output);
        output.slerp(&delta, t);
    }
}

/// CoordinateChaser / CoordinateDamper / TexCoordChaser2D / TexCoordDamper2D: arrays of vectors
/// (X3DArrayFollowerTemplate). Missing elements read as `this .zero`.
#[derive(Debug)]
pub struct ArrK<E: Elem>(std::marker::PhantomData<E>);

impl<E: Elem> Kind for ArrK<E> {
    type S = Vec<E>;

    const ARRAY: bool = true;

    fn zero() -> Vec<E> {
        vec![]
    }

    fn read(v: &FieldValue) -> Vec<E> {
        E::read_mf(v)
    }

    fn write_always(s: &Vec<E>) -> FieldValue {
        E::write_mf(s)
    }

    fn write(s: &Vec<E>, current: &FieldValue) -> Option<FieldValue> {
        // value_changed [i] = value [i] (an event per element) and `value_changed .length = length`
        // (an event only when it shrinks): nothing is sent when both are empty.
        if s.is_empty() && E::read_mf(current).is_empty() {
            return None;
        }

        Some(E::write_mf(s))
    }

    fn equals(lhs: &Vec<E>, rhs: &Vec<E>, tolerance: f64) -> bool {
        if lhs.len() != rhs.len() {
            return false;
        }

        let mut distance = 0.0;

        for (l, r) in lhs.iter().zip(rhs) {
            let mut a = *l;
            a.subtract(r);
            distance = jsmath::js_max(a.norm(), distance);
        }

        distance < tolerance
    }

    fn interpolate(source: &Vec<E>, destination: &Vec<E>, weight: f64) -> Vec<E> {
        let mut array = source.clone();

        for (i, e) in array.iter_mut().enumerate() {
            e.lerp(destination.get(i).unwrap_or(&E::zero()), weight);
        }

        array
    }

    fn step(output: &mut Vec<E>, value1: &Vec<E>, value2: &Vec<E>, t: f64) {
        let zero = E::zero();

        for (i, o) in output.iter_mut().enumerate() {
            let mut delta = *value1.get(i).unwrap_or(&zero);
            delta.subtract(value2.get(i).unwrap_or(&zero));
            delta.multiply(t);
            o.add(&delta);
        }
    }

    fn resize(buffer: &mut Vec<E>, length: usize) {
        // for (let b = buffer .length; b < l; ++ b) buffer [b] = this .getVector (); buffer .length = l;
        buffer.resize(length, E::zero());
    }

    fn field_length(v: &FieldValue) -> usize {
        E::read_mf(v).len()
    }
}

// --- X3DFollowerNode / X3DChaserNode ---------------------------------------------------------

#[derive(Debug)]
pub struct Chaser<K: Kind> {
    active: bool,
    buffer: Vec<K::S>,
    destination: K::S,
    previous: K::S,
    buffer_end_time: f64,
    step_time: f64,
}

impl<K: Kind> Chaser<K> {
    pub fn new() -> Self {
        Chaser { active: false, buffer: vec![], destination: K::zero(), previous: K::zero(), buffer_end_time: 0.0, step_time: 0.0 }
    }

    fn tolerance() -> f64 {
        1e-8
    }

    /// X3DFollowerNode.set_active__ (the live handling is the runtime's).
    fn set_active(&mut self, value: bool, out: &mut Vec<Out>) {
        if value == self.active {
            return;
        }

        self.active = value;
        out.push(Out::Active(value));
    }

    fn step_response(t: f64, duration: f64) -> f64 {
        if t <= 0.0 {
            return 0.0;
        }

        if t >= duration {
            return 1.0;
        }

        0.5 - 0.5 * jsmath::cos((t / duration) * std::f64::consts::PI)
    }

    fn update_buffer(&mut self, now: f64) -> f64 {
        let n = self.buffer.len();
        let mut fraction = (now - self.buffer_end_time) / self.step_time;

        if fraction >= 1.0 {
            let seconds = fraction.floor();

            fraction -= seconds;

            if seconds < n as f64 {
                let s = seconds as usize;

                self.previous = self.buffer[n - s].clone();

                for i in (s..n).rev() {
                    self.buffer[i] = self.buffer[i - s].clone();
                }

                for i in 0..s {
                    let alpha = i as f64 / seconds;

                    self.buffer[i] = K::interpolate(&self.destination, &self.buffer[s], alpha);
                }
            } else {
                self.previous = if seconds == n as f64 { self.buffer[0].clone() } else { self.destination.clone() };

                for b in self.buffer.iter_mut() {
                    *b = self.destination.clone();
                }
            }

            self.buffer_end_time += seconds * self.step_time;
        }

        fraction
    }
}

impl<K: Kind> Follower for Chaser<K> {
    fn initialize(&mut self, e: &Env, initial_value: &FieldValue, initial_destination: &FieldValue, out: &mut Vec<Out>) {
        self.set_duration(e);

        let (iv, id) = (K::read(initial_value), K::read(initial_destination));

        self.buffer_end_time = e.now;
        self.previous = iv.clone();
        self.buffer = vec![id.clone()];

        for _ in 1..NUM_BUFFERS {
            self.buffer.push(iv.clone());
        }

        self.destination = id.clone();

        if K::equals(&id, &iv, Self::tolerance()) {
            out.push(Out::Value(K::write_always(&id)));
        } else {
            self.set_active(true, out);
        }
    }

    fn set_value(&mut self, e: &Env, value: &FieldValue, out: &mut Vec<Out>) {
        if !self.active {
            self.buffer_end_time = e.now;
        }

        let value = K::read(value);

        for b in self.buffer.iter_mut() {
            *b = value.clone();
        }

        self.previous = value.clone();
        out.push(Out::Value(K::write_always(&value)));
        self.set_active(true, out);
    }

    fn set_destination(&mut self, e: &Env, destination: &FieldValue, out: &mut Vec<Out>) {
        if K::ARRAY {
            // X3DArrayFollowerTemplate.set_destination__
            let l = K::field_length(destination);

            for b in self.buffer.iter_mut() {
                K::resize(b, l);
            }
        }

        self.destination = K::read(destination);

        if !self.active {
            self.buffer_end_time = e.now;
        }

        self.set_active(true, out);
    }

    fn set_duration(&mut self, e: &Env) {
        self.step_time = e.duration / NUM_BUFFERS as f64;
    }

    fn prepare_events(&mut self, e: &Env, out: &mut Vec<Out>) {
        let n = self.buffer.len();
        let fraction = self.update_buffer(e.now);
        let mut output = K::interpolate(&self.previous, &self.buffer[n - 1], Self::step_response((n as f64 - 1.0 + fraction) * self.step_time, e.duration));

        for i in (0..n - 1).rev() {
            let t = Self::step_response((i as f64 + fraction) * self.step_time, e.duration);

            K::step(&mut output, &self.buffer[i], &self.buffer[i + 1], t);
        }

        if let Some(v) = K::write(&output, &e.current) {
            out.push(Out::Value(v));
        }

        if K::equals(&output, &self.destination, Self::tolerance()) {
            self.set_active(false, out);
        }
    }
}

// --- X3DDamperNode -----------------------------------------------------------------------------

#[derive(Debug)]
pub struct Damper<K: Kind> {
    active: bool,
    buffer: Vec<K::S>,
}

impl<K: Kind> Damper<K> {
    pub fn new() -> Self {
        Damper { active: false, buffer: vec![] }
    }

    /// X3DDamperNode.getOrder: Algorithm.clamp (order, 0, 5).
    fn order(e: &Env) -> usize {
        jsmath::js_min(jsmath::js_max(e.order as f64, 0.0), 5.0) as usize
    }

    /// X3DDamperNode.getTolerance.
    fn tolerance(e: &Env) -> f64 {
        if e.tolerance < 0.0 {
            return 1e-4;
        }

        e.tolerance
    }

    fn set_active(&mut self, value: bool, out: &mut Vec<Out>) {
        if value == self.active {
            return;
        }

        self.active = value;
        out.push(Out::Active(value));
    }
}

impl<K: Kind> Follower for Damper<K> {
    fn initialize(&mut self, e: &Env, initial_value: &FieldValue, initial_destination: &FieldValue, out: &mut Vec<Out>) {
        let (iv, id) = (K::read(initial_value), K::read(initial_destination));

        self.buffer = vec![id.clone()];

        for _ in 1..Self::order(e) + 1 {
            self.buffer.push(iv.clone());
        }

        if K::equals(&id, &iv, Self::tolerance(e)) {
            out.push(Out::Value(K::write_always(&id)));
        } else {
            self.set_active(true, out);
        }
    }

    fn set_value(&mut self, _e: &Env, value: &FieldValue, out: &mut Vec<Out>) {
        let value = K::read(value);

        for b in self.buffer.iter_mut().skip(1) {
            *b = value.clone();
        }

        out.push(Out::Value(K::write_always(&value)));
        self.set_active(true, out);
    }

    fn set_destination(&mut self, _e: &Env, destination: &FieldValue, out: &mut Vec<Out>) {
        if K::ARRAY {
            let l = K::field_length(destination);

            for b in self.buffer.iter_mut() {
                K::resize(b, l);
            }
        }

        self.buffer[0] = K::read(destination);
        self.set_active(true, out);
    }

    fn prepare_events(&mut self, e: &Env, out: &mut Vec<Out>) {
        let mut order = self.buffer.len() - 1;

        // `if (this ._tau .getValue ())`: 0 and NaN are falsy.
        if e.tau != 0.0 && !e.tau.is_nan() {
            let delta = 1.0 / e.frame_rate;
            let alpha = jsmath::exp(-delta / e.tau);

            for i in 0..order {
                self.buffer[i + 1] = K::interpolate(&self.buffer[i], &self.buffer[i + 1], alpha);
            }

            if let Some(v) = K::write(&self.buffer[order], &e.current) {
                out.push(Out::Value(v));
            }

            if !K::equals(&self.buffer[order], &self.buffer[0], Self::tolerance(e)) {
                return;
            }
        } else {
            if let Some(v) = K::write(&self.buffer[0], &e.current) {
                out.push(Out::Value(v));
            }

            order = 0;
        }

        for i in 1..self.buffer.len() {
            self.buffer[i] = self.buffer[order].clone();
        }

        self.set_active(false, out);
    }
}

// --- runtime glue --------------------------------------------------------------------------------

type V3 = VecK<Vector3>;
type V2 = VecK<Vector2>;
type A3 = ArrK<Vector3>;
type A2 = ArrK<Vector2>;

/// The concrete follower for a node type name.
pub fn make(name: &str) -> Option<Box<dyn Follower>> {
    Some(match name {
        "ColorChaser" => Box::new(Chaser::<ColorK>::new()),
        "CoordinateChaser" => Box::new(Chaser::<A3>::new()),
        "OrientationChaser" => Box::new(Chaser::<RotK>::new()),
        "PositionChaser" => Box::new(Chaser::<V3>::new()),
        "PositionChaser2D" => Box::new(Chaser::<V2>::new()),
        "ScalarChaser" => Box::new(Chaser::<ScalarK>::new()),
        "TexCoordChaser2D" => Box::new(Chaser::<A2>::new()),
        "ColorDamper" => Box::new(Damper::<ColorK>::new()),
        "CoordinateDamper" => Box::new(Damper::<A3>::new()),
        "OrientationDamper" => Box::new(Damper::<RotK>::new()),
        "PositionDamper" => Box::new(Damper::<V3>::new()),
        "PositionDamper2D" => Box::new(Damper::<V2>::new()),
        "ScalarDamper" => Box::new(Damper::<ScalarK>::new()),
        "TexCoordDamper2D" => Box::new(Damper::<A2>::new()),
        _ => return None,
    })
}

impl Runtime {
    fn follower_env(&self, node: NodeId) -> Env {
        let num = |name: &str| self.get_f64(node, name);

        Env {
            now: self.now,
            frame_rate: self.frame_rate,
            duration: num("duration"),
            tau: num("tau"),
            tolerance: num("tolerance"),
            order: num("order") as i32,
            current: self.pre_value(node, "value_changed").cloned().unwrap_or(FieldValue::SFBool(false)),
        }
    }

    /// X3DFollowerNode.initialize + X3DChaserNode / X3DDamperNode.initialize.
    pub(crate) fn follower_initialize(&mut self, node: NodeId) {
        self.followers.push(node);
        self.interest(node, "set_value", Cb::FollowerSetValue);
        self.interest(node, "set_destination", Cb::FollowerSetDestination);

        if self.pre(node, "duration").is_some() {
            self.interest(node, "duration", Cb::FollowerSetDuration);
        }

        // X3DDamperNode: `this ._order .addInterest ("set_order__", this)`. order is initializeOnly,
        // so nothing can send it an event, and set_order__ throws a ReferenceError (`length` is
        // block scoped in its for loop); no interest is needed.

        let initial_value = self.pre_value(node, "initialValue").cloned().unwrap_or(FieldValue::SFBool(false));
        let initial_destination = self.pre_value(node, "initialDestination").cloned().unwrap_or(FieldValue::SFBool(false));

        self.follower_call(node, |f, e, out| f.initialize(e, &initial_value, &initial_destination, out));
    }

    fn follower_call(&mut self, node: NodeId, call: impl FnOnce(&mut dyn Follower, &Env, &mut Vec<Out>)) {
        let State::Follower(mut follower) = std::mem::take(&mut self.nrt_mut(node).state) else {
            return;
        };

        let env = self.follower_env(node);
        let mut out = vec![];

        call(follower.as_mut(), &env, &mut out);
        self.nrt_mut(node).state = State::Follower(follower);

        for o in out {
            match o {
                Out::Value(v) => {
                    if let Some(k) = self.pre(node, "value_changed") {
                        self.set_value(k, v);
                    }
                }
                Out::Active(a) => {
                    if let Some(k) = self.pre(node, "isActive") {
                        self.set_value(k, FieldValue::SFBool(a));
                    }

                    self.follower_set_live(node);
                }
            }
        }
    }

    /// X3DFollowerNode.set_live__: `getLive () .getValue () && isActive` puts the node into the
    /// browser's prepareEvents interests (a Map: re-adding keeps the position), else removes it.
    pub(crate) fn follower_set_live(&mut self, node: NodeId) {
        let prepare = self.private_field(self.browser_node, "prepareEvents").unwrap();

        if self.ctx_live(self.scene.node(node).ctx) && self.get_bool(node, "isActive") {
            self.add_interest(prepare, node, Cb::FollowerPrepare);
        } else {
            self.remove_interest(prepare, node, Cb::FollowerPrepare);
        }
    }

    pub(crate) fn follower_interest(&mut self, node: NodeId, cb: Cb) {
        match cb {
            Cb::FollowerSetValue => {
                let v = self.pre_value(node, "set_value").cloned().unwrap_or(FieldValue::SFBool(false));
                self.follower_call(node, |f, e, out| f.set_value(e, &v, out));
            }
            Cb::FollowerSetDestination => {
                let v = self.pre_value(node, "set_destination").cloned().unwrap_or(FieldValue::SFBool(false));
                self.follower_call(node, |f, e, out| f.set_destination(e, &v, out));
            }
            Cb::FollowerSetDuration => self.follower_call(node, |f, e, _| f.set_duration(e)),
            Cb::FollowerPrepare => self.follower_call(node, |f, e, out| f.prepare_events(e, out)),
            _ => {}
        }
    }
}
