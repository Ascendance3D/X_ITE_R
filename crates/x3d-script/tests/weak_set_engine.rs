//! IterableWeakSet in the bundled QuickJS (sub:fields.types). X_ITE's class (x_ite/Base/IterableWeakSet.js) keeps a WeakRef per
//! member and registers the member in a FinalizationRegistry with the member itself as unregister token. In this QuickJS:
//!
//! * a FinalizationRegistry entry whose unregister token is its target keeps the target alive (the first test), so X_ITE's own
//!   class, run here, holds its members strongly while they are registered;
//! * freeing a cycle in which an object's own set holds a WeakRef to it, registered with itself as token (X3DChildObject's
//!   parents set of a field that is its own parent's member), corrupts the QuickJS heap (free_zero_refcount assertion,
//!   malloc_consolidate, SIGSEGV). Reproduced with plain QuickJS and no port code:
//!   `{ const a = new Child (); a .parents () .add (a); }` with the class below and WeakRef, then the runtime freed.
//!
//! The port's prelude therefore replaces only the WeakRef by a strong reference (gen_prelude_xite.py); the registry, the
//! tokens and every method stay X_ITE's. The second test shows that this holds nothing longer than the cycle collector allows:
//! memory returns to the same size after each round of dropped members.
use rquickjs::{Context, Runtime};

#[test]
fn registry_with_its_target_as_token_keeps_the_target_alive() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| {
        ctx.eval::<(), _>(
            "var refs = [], reg = new FinalizationRegistry (() => { });\n\
             (function () { for (let i = 0; i < 1000; ++ i) { const p = { i }; refs .push (new WeakRef (p)); reg .register (p, i, p); } }) ();",
        )
        .unwrap()
    });
    rt.run_gc();

    let alive: i32 = ctx.with(|ctx| ctx.eval("refs .filter (r => r .deref ()) .length").unwrap());

    assert_eq!(alive, 1000);
}

const PORT_SET: &str = r#"
class IterableWeakSet {
   #map = new Map ();
   #registry = new FinalizationRegistry (id => this .#map .delete (id));
   add (object) { this .#map .set (object .getId (), { deref: () => object }); this .#registry .register (object, object .getId (), object); }
   delete (object) { this .#map .delete (object .getId ()); this .#registry .unregister (object); }
}
let ids = 0;
class Child { #id = ++ ids; #parents; data = new Float64Array (64); getId () { return this .#id; } parents () { return this .#parents ??= new IterableWeakSet (); } }
"#;

#[test]
fn the_ports_strong_set_frees_unreachable_members() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| ctx.eval::<(), _>(PORT_SET).unwrap());

    let mut sizes = vec![];

    for _ in 0..4 {
        ctx.with(|ctx| {
            ctx.eval::<(), _>(
                "for (let i = 0; i < 20000; ++ i) { const a = new Child (), b = new Child (); a .parents () .add (b); b .parents () .add (a); a .parents () .add (a); }",
            )
            .unwrap()
        });
        rt.run_gc();
        sizes.push(rt.memory_usage().malloc_size);
    }

    assert!(
        sizes.windows(2).all(|w| w[0] == w[1]),
        "memory grows: {sizes:?}"
    );
}
