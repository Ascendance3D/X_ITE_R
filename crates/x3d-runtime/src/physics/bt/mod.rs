//! Port of the Bullet 2.82 dynamics pipeline that X_ITE drives through Ammo.js (zlib license,
//! Erwin Coumans). Double arithmetic with float rounding at memory stores, as asm.js without
//! Math.fround executes it.

pub mod body;
pub mod boxbox;
pub mod broadphase;
pub mod constraint;
pub mod contact;
pub mod dbvt;
pub mod dispatch;
pub mod fdlibm;
pub mod gjk;
pub mod math;
pub mod island;
pub mod shape;
pub mod solver;
pub mod world;

/// BT_TRACE=1: every call that matters is written to stderr as one JSON line, in the format of
/// oracle/tools/ammo_wrap.mjs (which logs the same calls of the real Ammo): diff the two to find the
/// first call that differs.
pub fn trace(line: impl FnOnce() -> String) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

    if *ON.get_or_init(|| std::env::var("BT_TRACE").is_ok()) && !IN_STEP.with(|c| c.get()) {
        eprintln!("BTT {}", line());
    }
}

pub fn tj(t: &math::Tf) -> String {
    let t = math::rt(*t);

    format!("{{\"T\":[[[{},{},{}],[{},{},{}],[{},{},{}]],[{},{},{}]]}}", t.b[0][0], t.b[0][1], t.b[0][2], t.b[1][0], t.b[1][1], t.b[1][2], t.b[2][0], t.b[2][1], t.b[2][2], t.o[0], t.o[1], t.o[2])
}

pub fn vj(v: &math::V3) -> String {
    format!("[{},{},{}]", math::f(v[0]), math::f(v[1]), math::f(v[2]))
}

thread_local! {
    static IN_STEP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Calls the engine makes to itself while it steps are not calls of the caller: not traced.
pub struct StepGuard(bool);

impl StepGuard {
    pub fn new() -> StepGuard {
        StepGuard(IN_STEP.with(|c| c.replace(true)))
    }
}

impl Drop for StepGuard {
    fn drop(&mut self) {
        IN_STEP.with(|c| c.set(self.0));
    }
}
