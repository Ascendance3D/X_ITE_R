//! R0.3-D-FIX-1: what a disposed scene leaves in the scene-wide tables, and the EXTERNPROTO scene
//! cache across Inline reloads. Newly authored worlds, served from memory.
//!
//! * `STALE_EXPORT_IMPORT_PINS_REUSED_SLOTS`: the EXPORT and IMPORT entries of a context that the
//!   sweep emptied must not keep the node that takes their slot later (ISO/IEC 19775-1 4.4.6).
//! * `CACHED_EXTERNPROTO_DISPOSED_ON_INLINE_UNLOAD`: a cached EXTERNPROTO scene belongs to the
//!   browser (X_ITE 16.5.0 X3DExternProtoDeclaration.setInternalScene gives it the default scene as
//!   execution context); the Inline that fetched it first may go, the next one must find its PROTO.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use x3d_fields::math::Vector3;
use x3d_fields::{FieldValue, NodeId};
use x3d_runtime::{CacheEntry, Fetcher, Runtime};
use x3d_scene::{CtxId, ROOT};
use x3d_script::QuickJsHost;

#[derive(Clone, Default)]
struct Files(Rc<RefCell<HashMap<String, Vec<u8>>>>);

impl Files {
    fn put(&self, name: &str, text: &str) {
        self.0.borrow_mut().insert(format!("http://test/{name}"), text.as_bytes().to_vec());
    }
}

impl Fetcher for Files {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        self.0.borrow().get(url.as_str()).cloned().ok_or_else(|| format!("Error: Not Found ({url})"))
    }
}

fn big_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(64 << 20).spawn(f).unwrap().join().unwrap();
}

fn start(files: &[(&str, &str)], main: &str) -> (Runtime, Files) {
    x3d_fields::jsmath::set_flavor(x3d_fields::jsmath::MathFlavor::Chrome);

    let mem = Files::default();

    for (name, text) in files {
        mem.put(name, text);
    }

    let scene = x3d_parse::parse_bytes(main.as_bytes(), "http://test/main.wrl").expect("main.wrl");
    let mut rt = Runtime::new(scene, Box::new(mem.clone()));

    rt.script_host = Some(Box::new(QuickJsHost::new()));
    rt.live = true;
    rt.now = 1000.0;
    rt.record_console = true;
    rt.load_scene();
    rt.replace_world();
    rt.settle();
    frames(&mut rt, 2);
    (rt, mem)
}

fn frames(rt: &mut Runtime, n: usize) {
    for _ in 0..n {
        rt.settle();
        rt.now += 1.0 / 60.0;
        let now = rt.now;
        rt.fire_timeouts(now);
        rt.frame();
        rt.settle();
    }
}

fn named(rt: &Runtime, ctx: CtxId, name: &str) -> NodeId {
    rt.scene.contexts[ctx].named_nodes.get(name).unwrap_or_else(|| panic!("no node {name} in context {ctx}"))
}

fn set_url(rt: &mut Runtime, node: NodeId, url: &str) {
    let k = rt.get_field(node, "url").unwrap();
    rt.set_value(k, FieldValue::MFString(vec![url.into()]));
}

fn set_load(rt: &mut Runtime, node: NodeId, load: bool) {
    let k = rt.get_field(node, "load").unwrap();
    rt.set_value(k, FieldValue::SFBool(load));
}

fn v3(rt: &Runtime, node: NodeId, name: &str) -> Vector3 {
    match rt.pre_value(node, name) {
        Some(FieldValue::SFVec3f(v)) => *v,
        other => panic!("{name}: {other:?}"),
    }
}

/// (arena length, live nodes)
fn arena(rt: &Runtime) -> (usize, usize) {
    (rt.scene.nodes.len(), rt.scene.nodes.len() - rt.scene.free_nodes.len())
}

/// Every cached scene is a live context that still has its PROTO declarations.
fn assert_cache_sound(rt: &Runtime, at: &str) {
    for (key, entry) in &rt.extern_cache {
        if let CacheEntry::Ready(c, _) = entry {
            assert!(!rt.ctx_dead(*c), "{at}: cached scene {key} (context {c}) was disposed");
            assert!(!rt.scene.contexts[*c].protos.is_empty(), "{at}: cached scene {key} (context {c}) lost its PROTOs");
        }
    }
}

/// Reload `Inl` `cycles` times between `a` and `b`; after `warm_up` cycles the arena must not grow
/// by a single slot, and `check` runs after every cycle.
fn reload_flat(rt: &mut Runtime, a: &str, b: &str, cycles: usize, warm_up: usize, mut check: impl FnMut(&mut Runtime, usize)) -> Vec<(usize, (usize, usize))> {
    let inl = named(rt, ROOT, "Inl");
    let mut base = None;
    let mut samples = vec![];

    for cycle in 1..=cycles {
        // Enough frames for a fetch, the scene's loadCount, its attach and the sweep of the old one.
        set_url(rt, inl, if cycle % 2 == 0 { a } else { b });
        frames(rt, 6);
        rt.console.clear();

        if cycle == warm_up {
            base = Some(arena(rt));
        }

        if let Some(base) = base {
            assert_eq!(arena(rt), base, "cycle {cycle}: the arena grew after the warm-up (length, live)");
        }

        check(rt, cycle);

        if cycle == 1 || cycle % 250 == 0 {
            samples.push((cycle, arena(rt)));
        }
    }

    samples
}

// --- STALE_EXPORT_IMPORT_PINS_REUSED_SLOTS -------------------------------------------------------

const EXPORT_A: &str = "#VRML V2.0 utf8\nDEF ExpT Transform { translation 1 2 3 children [ Shape { geometry Box { } } ] }\nEXPORT ExpT AS MyExport\n";
const EXPORT_B: &str = "#VRML V2.0 utf8\nDEF ExpT Transform { translation 4 5 6 children [ Shape { geometry Sphere { } } ] }\nEXPORT ExpT AS MyExport\n";

/// The independent QA reproduction: an Inline whose scene EXPORTs a node, reloaded 1,000 times.
/// Before the repair the emptied context's EXPORT entry kept whatever took its slot (6 → 1,506 live).
#[test]
fn reloading_an_inline_with_export_keeps_the_arena_flat() {
    big_stack(|| {
        let (mut rt, _) = start(&[("a.wrl", EXPORT_A), ("b.wrl", EXPORT_B)], "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\n");
        let samples = reload_flat(&mut rt, "a.wrl", "b.wrl", 1000, 4, |_, _| {});

        println!("EXPORT reloads (cycle, (arena, live)): {samples:?}, exported entries {}", rt.scene.exported.len());
    });
}

/// An IMPORT in the reloaded scene (from an Inline nested in it): its entry names that Inline and its
/// X3DImportedNodeInstance, both slots of the scene that goes.
#[test]
fn reloading_an_inline_with_import_keeps_the_arena_flat() {
    big_stack(|| {
        let leaf = "#VRML V2.0 utf8\nDEF LeafT Transform { children [ Shape { geometry Cone { } } ] }\nEXPORT LeafT\n";
        let scene = |x: u32| format!("#VRML V2.0 utf8\nDEF Src Transform {{ translation {x} 0 0 }}\nDEF Leaf Inline {{ url \"leaf.wrl\" }}\nIMPORT Leaf.LeafT AS LT\nROUTE Src.translation_changed TO LT.set_translation\n");
        let (a, b) = (scene(1), scene(2));
        let (mut rt, _) = start(&[("leaf.wrl", leaf), ("a.wrl", &a), ("b.wrl", &b)], "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\n");

        let samples = reload_flat(&mut rt, "a.wrl", "b.wrl", 1000, 4, |rt, cycle| {
            // The new scene's IMPORT works: an event through it reaches the node its Inline exports.
            if cycle % 100 == 0 {
                let inl = named(rt, ROOT, "Inl");
                let ctx = rt.inline_scenes[&inl];
                let src = named(rt, ctx, "Src");
                let leaf = named(rt, ctx, "Leaf");
                let leaf_t = rt.scene.get_exported_node(rt.inline_scenes[&leaf], "LeafT").expect("LeafT");
                let k = rt.get_field(src, "translation").unwrap();

                rt.set_value(k, FieldValue::SFVec3f(Vector3::new(cycle as f64, 7.0, 0.0)));
                frames(rt, 1);
                assert_eq!(v3(rt, leaf_t, "translation"), Vector3::new(cycle as f64, 7.0, 0.0), "cycle {cycle}: the IMPORT route does not reach LeafT");
            }
        });

        println!("IMPORT reloads (cycle, (arena, live)): {samples:?}, imported entries {}", rt.scene.imported.len());
    });
}

/// Both at once, and the world's own IMPORT of the reloaded Inline's EXPORT: that entry belongs to a
/// live context and must keep working on every new scene, while the old scenes' entries count for nothing.
#[test]
fn mixed_import_export_reloads_keep_live_references_and_the_arena_flat() {
    big_stack(|| {
        let leaf = "#VRML V2.0 utf8\nDEF LeafT Transform { }\nEXPORT LeafT\n";
        let scene = |s: &str| {
            format!(
                "#VRML V2.0 utf8\nDEF Leaf Inline {{ url \"leaf.wrl\" }}\nIMPORT Leaf.LeafT AS LT\nDEF ExpT Transform {{ children [ Shape {{ geometry {s} {{ }} }} ] }}\nEXPORT ExpT AS MyExport\nROUTE ExpT.translation_changed TO LT.set_translation\n"
            )
        };
        let (a, b) = (scene("Box"), scene("Sphere"));
        let main = "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\nIMPORT Inl.MyExport AS Imp\nDEF Src Transform { }\nROUTE Src.translation_changed TO Imp.set_translation\n";
        let (mut rt, _) = start(&[("leaf.wrl", leaf), ("a.wrl", &a), ("b.wrl", &b)], main);

        let samples = reload_flat(&mut rt, "a.wrl", "b.wrl", 1000, 4, |rt, cycle| {
            if cycle % 50 == 0 {
                let src = named(rt, ROOT, "Src");
                let inl = named(rt, ROOT, "Inl");
                let ctx = rt.inline_scenes[&inl];
                let exp_t = rt.scene.get_exported_node(ctx, "MyExport").expect("MyExport");
                let leaf = named(rt, ctx, "Leaf");
                let leaf_t = rt.scene.get_exported_node(rt.inline_scenes[&leaf], "LeafT").expect("LeafT");
                let k = rt.get_field(src, "translation").unwrap();
                let v = Vector3::new(cycle as f64, 1.0, 2.0);

                rt.set_value(k, FieldValue::SFVec3f(v));
                frames(rt, 1);
                assert_eq!(v3(rt, exp_t, "translation"), v, "cycle {cycle}: the world's IMPORT does not reach the new ExpT");
                assert_eq!(v3(rt, leaf_t, "translation"), v, "cycle {cycle}: the scene's IMPORT does not reach LeafT");
            }
        });

        println!(
            "mixed reloads (cycle, (arena, live)): {samples:?}, contexts {}, imported {}, exported {}",
            rt.scene.contexts.len(),
            rt.scene.imported.len(),
            rt.scene.exported.len()
        );
    });
}

// --- CACHED_EXTERNPROTO_DISPOSED_ON_INLINE_UNLOAD ------------------------------------------------

const EXT_LIB: &str = "#VRML V2.0 utf8\nPROTO ExtBox [ field SFVec3f sz 2 2 2 ] {\n  Shape { geometry Box { size IS sz } }\n  DEF ExtScript Script { url \"javascript: function initialize () { print ('ExtScript init'); } function shutdown () { print ('ExtScript shutdown'); }\" }\n}\nDEF LibTop Script { url \"javascript: function initialize () { print ('LibTop init'); } function shutdown () { print ('LibTop shutdown'); }\" }\n";

fn uses(lib: &str, inst: &str, size: u32) -> String {
    format!("#VRML V2.0 utf8\nEXTERNPROTO ExtBox [ field SFVec3f sz ] \"{lib}\"\nDEF {inst} ExtBox {{ sz {size} {size} {size} }}\n")
}

/// The body of `inst` in the Inline's current scene: its Box has the given size.
fn assert_body(rt: &Runtime, inst: &str, size: f64, at: &str) {
    let inl = named(rt, ROOT, "Inl");
    let ctx = *rt.inline_scenes.get(&inl).unwrap_or_else(|| panic!("{at}: no scene; log {:?}", rt.log));
    let n = named(rt, ctx, inst);
    let body = *rt.instance_bodies.get(&n).unwrap_or_else(|| panic!("{at}: {inst} has no body; log {:?}", rt.log));
    let shape = rt.scene.contexts[body].root_nodes.iter().flatten().copied().find(|&s| rt.builtin_name(s) == Some("Shape")).expect("Shape");
    let Some(FieldValue::SFNode(Some(geometry))) = rt.pre_value(shape, "geometry") else { panic!("{at}: no geometry") };

    assert_eq!(v3(rt, *geometry, "size"), Vector3::new(size, size, size), "{at}: {inst}");
}

/// The independent QA reproduction, inverted: Inline A fetches the EXTERNPROTO and goes; Inline B
/// declares the same URL and gets its PROTO from the cache, 1,000 times over.
#[test]
fn a_cached_externproto_outlives_the_inline_that_fetched_it() {
    big_stack(|| {
        let (a, b) = (uses("ext_lib.wrl", "InstA", 3), uses("ext_lib.wrl", "InstB", 4));
        let (mut rt, _) = start(&[("ext_lib.wrl", EXT_LIB), ("inl_a.wrl", &a), ("inl_b.wrl", &b)], "#VRML V2.0 utf8\nDEF Inl Inline { url \"inl_a.wrl\" }\n");
        let inl = named(&rt, ROOT, "Inl");

        frames(&mut rt, 2);
        assert_body(&rt, "InstA", 3.0, "first load");

        let mut base = None;

        for cycle in 1..=1000 {
            let (file, inst, size) = if cycle % 2 == 0 { ("inl_a.wrl", "InstA", 3.0) } else { ("inl_b.wrl", "InstB", 4.0) };

            // Unload for a frame, then load the other file (the QA sequence).
            set_load(&mut rt, inl, false);
            frames(&mut rt, 1);
            set_url(&mut rt, inl, file);
            set_load(&mut rt, inl, true);
            frames(&mut rt, 3);

            let at = format!("cycle {cycle}");

            assert!(!rt.log.iter().any(|l| l.contains("PROTO not found")), "{at}: {:?}", rt.log);
            assert_body(&rt, inst, size, &at);
            assert_cache_sound(&rt, &at);

            if cycle == 4 {
                base = Some(arena(&rt));
            }

            if let Some(base) = base {
                assert_eq!(arena(&rt), base, "{at}: the arena grew");
            }

            rt.console.clear();
        }

        assert!(rt.log.is_empty(), "{:?}", rt.log);
        println!("cached EXTERNPROTO reloads: arena {:?}, contexts {}, protos {}, externprotos {}", arena(&rt), rt.scene.contexts.len(), rt.scene.protos.len(), rt.scene.externprotos.len());
    });
}

/// The cached scene is the world's: its own Scripts are shut down with the world, once, not with the
/// Inline that fetched it; the cache is empty after `unload_world`.
#[test]
fn a_cached_externproto_scene_goes_with_the_world() {
    big_stack(|| {
        let (a, b) = (uses("ext_lib.wrl", "InstA", 3), uses("ext_lib.wrl", "InstB", 4));
        let (mut rt, _) = start(&[("ext_lib.wrl", EXT_LIB), ("inl_a.wrl", &a), ("inl_b.wrl", &b)], "#VRML V2.0 utf8\nDEF Inl Inline { url \"inl_a.wrl\" }\n");
        let inl = named(&rt, ROOT, "Inl");
        let count = |rt: &Runtime, l: &str| rt.console.iter().filter(|c| c.as_str() == l).count();

        frames(&mut rt, 2);
        set_url(&mut rt, inl, "inl_b.wrl");
        frames(&mut rt, 3);
        assert_body(&rt, "InstB", 4.0, "replacement");
        assert_eq!(count(&rt, "log: ExtScript shutdown"), 1, "the replaced instance body shuts down: {:?}", rt.console);
        assert_eq!(count(&rt, "log: LibTop shutdown"), 0, "the cached scene stays: {:?}", rt.console);
        assert_eq!(count(&rt, "log: LibTop init"), 1, "the cached scene is not fetched again: {:?}", rt.console);

        rt.unload_world();
        rt.settle();
        rt.unload_world();
        assert_eq!(count(&rt, "log: LibTop shutdown"), 1, "{:?}", rt.console);
        assert_eq!(count(&rt, "log: ExtScript shutdown"), 2, "{:?}", rt.console);
        assert!(rt.extern_cache.is_empty());
    });
}

/// An EXTERNPROTO used inside a PROTO body inside a nested Inline: replacing the outer Inline's scene
/// disposes the nested scenes, not the cached one.
#[test]
fn nested_proto_and_inline_combinations_find_the_cached_proto() {
    big_stack(|| {
        let inner = |inst: &str, size: u32| format!("#VRML V2.0 utf8\nEXTERNPROTO ExtBox [ field SFVec3f sz ] \"ext_lib.wrl\"\nPROTO Wrap [ ] {{ Group {{ children [ DEF {inst} ExtBox {{ sz {size} {size} {size} }} ] }} }}\nDEF W Wrap {{ }}\n");
        let outer = |file: &str| format!("#VRML V2.0 utf8\nPROTO Holder [ ] {{ Inline {{ url \"{file}\" }} }}\nDEF H Holder {{ }}\n");
        let (ia, ib, oa, ob) = (inner("InstA", 3), inner("InstB", 4), outer("inner_a.wrl"), outer("inner_b.wrl"));
        let (mut rt, _) = start(&[("ext_lib.wrl", EXT_LIB), ("inner_a.wrl", &ia), ("inner_b.wrl", &ib), ("a.wrl", &oa), ("b.wrl", &ob)], "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\n");

        reload_flat(&mut rt, "a.wrl", "b.wrl", 200, 6, |rt, cycle| {
            assert!(!rt.log.iter().any(|l| l.contains("PROTO not found")), "cycle {cycle}: {:?}", rt.log);
            assert_cache_sound(rt, &format!("cycle {cycle}"));

            // Holder's body → its Inline's scene → W's body has the ExtBox instance with a body.
            let inl = named(rt, ROOT, "Inl");
            let h = named(rt, rt.inline_scenes[&inl], "H");
            let hbody = rt.instance_bodies[&h];
            let nested = rt.scene.contexts[hbody].root_nodes.iter().flatten().copied().find(|&n| rt.builtin_name(n) == Some("Inline")).expect("Inline");
            let w = named(rt, rt.inline_scenes[&nested], "W");
            let wbody = rt.instance_bodies[&w];
            let inst = named(rt, wbody, if cycle % 2 == 0 { "InstA" } else { "InstB" });

            assert!(rt.instance_bodies.contains_key(&inst), "cycle {cycle}: the ExtBox instance has no body: {:?}", rt.log);
        });
    });
}

/// Two URLs, one PROTO name: each declaration gets the scene of its own URL from the cache.
#[test]
fn different_urls_with_the_same_proto_name_stay_apart() {
    big_stack(|| {
        let lib2 = EXT_LIB.replace("2 2 2", "9 9 9");
        let (a, b) = (format!("#VRML V2.0 utf8\nEXTERNPROTO ExtBox [ field SFVec3f sz ] \"lib1.wrl\"\nDEF InstA ExtBox {{ }}\n"), format!("#VRML V2.0 utf8\nEXTERNPROTO ExtBox [ field SFVec3f sz ] \"lib2.wrl\"\nDEF InstB ExtBox {{ }}\n"));
        let (mut rt, _) = start(&[("lib1.wrl", EXT_LIB), ("lib2.wrl", &lib2), ("inl_a.wrl", &a), ("inl_b.wrl", &b)], "#VRML V2.0 utf8\nDEF Inl Inline { url \"inl_a.wrl\" }\n");

        reload_flat(&mut rt, "inl_a.wrl", "inl_b.wrl", 200, 4, |rt, cycle| {
            let at = format!("cycle {cycle}");

            if cycle % 2 == 0 {
                assert_body(rt, "InstA", 2.0, &at);
            } else {
                assert_body(rt, "InstB", 9.0, &at);
            }

            assert_cache_sound(rt, &at);
        });

        assert_eq!(rt.extern_cache.len(), 2);
    });
}

/// A URL that fails again and again leaves nothing in the cache; once the file is there, the next
/// load finds the PROTO, and the scene is cached from then on.
#[test]
fn repeated_fetch_failures_recover() {
    big_stack(|| {
        let a = uses("late.wrl", "InstA", 3);
        let b = uses("late.wrl", "InstB", 4);
        let (mut rt, files) = start(&[("inl_a.wrl", &a), ("inl_b.wrl", &b)], "#VRML V2.0 utf8\nDEF Inl Inline { url \"inl_a.wrl\" }\n");
        let inl = named(&rt, ROOT, "Inl");

        for cycle in 1..=20 {
            set_url(&mut rt, inl, if cycle % 2 == 0 { "inl_a.wrl" } else { "inl_b.wrl" });
            frames(&mut rt, 6);
            assert!(rt.log.iter().any(|l| l.contains("late.wrl")), "cycle {cycle}: the failure is reported: {:?}", rt.log);
            assert!(rt.extern_cache.is_empty(), "cycle {cycle}: a failed fetch is not cached: {:?}", rt.extern_cache.len());
            rt.log.clear();
        }

        files.put("late.wrl", EXT_LIB);

        for cycle in 1..=20 {
            let (file, inst, size) = if cycle % 2 == 0 { ("inl_a.wrl", "InstA", 3.0) } else { ("inl_b.wrl", "InstB", 4.0) };

            set_url(&mut rt, inl, file);
            frames(&mut rt, 6);
            assert!(rt.log.is_empty(), "cycle {cycle}: {:?}", rt.log);
            assert_body(&rt, inst, size, &format!("recovered cycle {cycle}"));
            assert_cache_sound(&rt, &format!("recovered cycle {cycle}"));
        }

        assert_eq!(rt.console.iter().filter(|l| l.as_str() == "log: LibTop init").count(), 1, "fetched once after it appeared");
    });
}
