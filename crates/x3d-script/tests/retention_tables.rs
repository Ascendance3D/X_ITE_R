//! R0.3-E: the scene-wide tables and the JavaScript field handles stay bounded while scenes are
//! loaded and disposed, and an id that names a released slot never reaches what takes the slot.
//! Newly authored worlds, served from memory.
//!
//! * `CONTEXT_TABLE_RETENTION`: execution contexts, PROTO and EXTERNPROTO declarations, IMPORT and
//!   EXPORT records, and the `emptied_ctxs` / `dead_ctxs` / `disposed_scenes` sets.
//! * `JS_SLOT_TABLE_GROWTH`: the QuickJS host's field handle table.
//!
//! Every workload runs 1,000 load / unload cycles and records the tables at cycles 10, 100, 500 and
//! 1,000 (`MEM` lines). After the warm-up (cycle 10) no length or capacity may grow.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use x3d_fields::math::Vector3;
use x3d_fields::{FieldValue, NodeId};
use x3d_runtime::{CacheEntry, Fetcher, Runtime, Task};
use x3d_scene::{CtxId, NodeKind, ProtoRef, ROOT};
use x3d_script::QuickJsHost;
use x3d_script::engine::slot_stats;

#[derive(Clone, Default)]
struct Files(Rc<RefCell<HashMap<String, Vec<u8>>>>, Rc<RefCell<HashMap<String, usize>>>);

impl Files {
    fn put(&self, name: &str, text: &str) {
        self.0.borrow_mut().insert(format!("http://test/{name}"), text.as_bytes().to_vec());
    }

    /// How often `name` was fetched.
    fn fetched(&self, name: &str) -> usize {
        self.1.borrow().get(&format!("http://test/{name}")).copied().unwrap_or(0)
    }
}

impl Fetcher for Files {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        // A query makes an EXTERNPROTO URL uncached; the file is the same.
        let mut u = url.clone();

        u.set_fragment(None);
        u.set_query(None);
        *self.1.borrow_mut().entry(u.to_string()).or_default() += 1;
        self.0.borrow().get(u.as_str()).cloned().ok_or_else(|| format!("Error: Not Found ({url})"))
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

fn set(rt: &mut Runtime, node: NodeId, field: &str, v: FieldValue) {
    let k = rt.get_field(node, field).unwrap_or_else(|| panic!("no field {field}"));
    rt.set_value(k, v);
}

fn v3(rt: &Runtime, node: NodeId, name: &str) -> Vector3 {
    match rt.pre_value(node, name) {
        Some(FieldValue::SFVec3f(v)) => *v,
        other => panic!("{name}: {other:?}"),
    }
}

fn scene_of(rt: &Runtime, inline: NodeId) -> CtxId {
    *rt.inline_scenes.get(&inline).unwrap_or_else(|| panic!("the Inline has no scene; log {:?}", rt.log))
}

fn rss_kib() -> usize {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("VmRSS:").and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok())))
        .unwrap_or(0)
}

/// Every retained table, length and capacity.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Tables {
    nodes: (usize, usize),
    live: usize,
    free: usize,
    contexts: (usize, usize),
    protos: (usize, usize),
    externprotos: (usize, usize),
    imported: (usize, usize),
    exported: (usize, usize),
    emptied: usize,
    dead: usize,
    disposed: usize,
    js_slots: (usize, usize),
    js_attached: usize,
}

fn tables(rt: &Runtime) -> Tables {
    let s = &rt.scene;
    let js = slot_stats();

    Tables {
        nodes: (s.nodes.len(), s.nodes.capacity()),
        live: s.nodes.len() - s.free_nodes.len(),
        free: s.free_nodes.len(),
        contexts: (s.contexts.len(), s.contexts.capacity()),
        protos: (s.protos.len(), s.protos.capacity()),
        externprotos: (s.externprotos.len(), s.externprotos.capacity()),
        imported: (s.imported.len(), s.imported.capacity()),
        exported: (s.exported.len(), s.exported.capacity()),
        emptied: rt.emptied_contexts(),
        dead: rt.dead_ctxs.len(),
        disposed: rt.disposed_scenes.len(),
        js_slots: (js.len, js.capacity),
        js_attached: js.attached,
    }
}

fn mem_line(workload: &str, cycle: usize, t: &Tables) -> String {
    format!(
        "MEM\t{workload}\t{cycle}\tarena={}/{}\tlive={}\tfree={}\tcontexts={}/{}\tprotos={}/{}\texternprotos={}/{}\timported={}/{}\texported={}/{}\temptied_ctxs={}\tdead_ctxs={}\tdisposed_scenes={}\tjs_slots={}/{}\tjs_attached={}\trss_kib={}",
        t.nodes.0,
        t.nodes.1,
        t.live,
        t.free,
        t.contexts.0,
        t.contexts.1,
        t.protos.0,
        t.protos.1,
        t.externprotos.0,
        t.externprotos.1,
        t.imported.0,
        t.imported.1,
        t.exported.0,
        t.exported.1,
        t.emptied,
        t.dead,
        t.disposed,
        t.js_slots.0,
        t.js_slots.1,
        t.js_attached,
        rss_kib()
    )
}

const WARM_UP: usize = 10;

/// Run `cycle` 1,000 times. From the warm-up on, every table must be exactly as it was at the warm-up:
/// not one more slot, not one more byte of capacity.
fn bounded(workload: &str, rt: &mut Runtime, mut cycle: impl FnMut(&mut Runtime, usize)) {
    let mut base: Option<Tables> = None;

    for c in 1..=1000 {
        cycle(rt, c);
        rt.console.clear();
        rt.log.retain(|l| !l.starts_with("warn"));

        let t = tables(rt);

        if [WARM_UP, 100, 500, 1000].contains(&c) {
            println!("{}", mem_line(workload, c, &t));
        }

        if c == WARM_UP {
            base = Some(t.clone());
        }

        if let Some(b) = &base {
            assert_eq!(&t, b, "{workload}: cycle {c}: a table grew after the warm-up");
        }
    }

    assert!(rt.log.is_empty(), "{workload}: {:?}", rt.log);
}

/// Unload for a frame, then load the other file (the QA sequence of R0.3-D).
fn swap(rt: &mut Runtime, inl: NodeId, file: &str) {
    set_load(rt, inl, false);
    frames(rt, 1);
    set_url(rt, inl, file);
    set_load(rt, inl, true);
    frames(rt, 4);
}

fn alternate(c: usize) -> &'static str {
    if c % 2 == 0 { "a.wrl" } else { "b.wrl" }
}

// --- the QA-2 workloads ----------------------------------------------------------------------

const EXPORT: &str = "#VRML V2.0 utf8\nDEF ExpT Transform { translation 1 2 3 children [ Shape { geometry Box { } } ] }\nEXPORT ExpT AS MyExport\n";

/// 1. An Inline whose scene EXPORTs a node that the world IMPORTs and routes to.
#[test]
fn w1_inline_export_with_world_import() {
    big_stack(|| {
        let main = "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\nIMPORT Inl.MyExport AS Imp\nDEF Src Transform { }\nROUTE Src.translation_changed TO Imp.set_translation\n";
        let (mut rt, _) = start(&[("a.wrl", EXPORT), ("b.wrl", &EXPORT.replace("Box", "Sphere"))], main);
        let (inl, src) = (named(&rt, ROOT, "Inl"), named(&rt, ROOT, "Src"));

        bounded("inline-export", &mut rt, |rt, c| {
            set_url(rt, inl, alternate(c));
            frames(rt, 6);

            // The world's IMPORT reaches the node of the scene that is loaded now.
            let exp = rt.scene.get_exported_node(scene_of(rt, inl), "MyExport").expect("MyExport");
            let v = Vector3::new(c as f64, 1.0, 2.0);

            set(rt, src, "translation", FieldValue::SFVec3f(v));
            frames(rt, 1);
            assert_eq!(v3(rt, exp, "translation"), v, "cycle {c}");
        });
    });
}

/// 2. An Inline whose scene IMPORTs from an Inline nested in it.
#[test]
fn w2_inline_import() {
    big_stack(|| {
        let leaf = "#VRML V2.0 utf8\nDEF LeafT Transform { children [ Shape { geometry Cone { } } ] }\nEXPORT LeafT\n";
        let scene = |x: u32| format!("#VRML V2.0 utf8\nDEF Src Transform {{ translation {x} 0 0 }}\nDEF Leaf Inline {{ url \"leaf.wrl\" }}\nIMPORT Leaf.LeafT AS LT\nROUTE Src.translation_changed TO LT.set_translation\n");
        let (a, b) = (scene(1), scene(2));
        let (mut rt, _) = start(&[("leaf.wrl", leaf), ("a.wrl", &a), ("b.wrl", &b)], "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\n");
        let inl = named(&rt, ROOT, "Inl");

        bounded("inline-import", &mut rt, |rt, c| {
            set_url(rt, inl, alternate(c));
            frames(rt, 6);

            let ctx = scene_of(rt, inl);
            let (src, leaf) = (named(rt, ctx, "Src"), named(rt, ctx, "Leaf"));
            let leaf_t = rt.scene.get_exported_node(scene_of(rt, leaf), "LeafT").expect("LeafT");
            let v = Vector3::new(c as f64, 7.0, 0.0);

            set(rt, src, "translation", FieldValue::SFVec3f(v));
            frames(rt, 1);
            assert_eq!(v3(rt, leaf_t, "translation"), v, "cycle {c}: the IMPORT route does not reach LeafT");
        });
    });
}

/// 3. An Inline with a PROTO, a nested PROTO and instances.
#[test]
fn w3_inline_proto() {
    big_stack(|| {
        let scene = |x: u32| {
            format!(
                "#VRML V2.0 utf8\nPROTO Inner [ field SFVec3f s 1 1 1 ] {{ Shape {{ geometry Box {{ size IS s }} }} }}\nPROTO Outer [ field SFVec3f pos 0 0 0 ] {{ Transform {{ translation IS pos children [ Inner {{ s 2 2 2 }} ] }} }}\nDEF P1 Outer {{ pos {x} 0 0 }}\nDEF P2 Outer {{ pos 0 {x} 0 }}\n"
            )
        };
        let (a, b) = (scene(1), scene(2));
        let (mut rt, _) = start(&[("a.wrl", &a), ("b.wrl", &b)], "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\n");
        let inl = named(&rt, ROOT, "Inl");

        bounded("inline-proto", &mut rt, |rt, c| {
            set_url(rt, inl, alternate(c));
            frames(rt, 6);

            let p1 = named(rt, scene_of(rt, inl), "P1");

            assert_eq!(rt.type_name(p1), "Outer", "cycle {c}");
            assert!(rt.instance_body(p1).is_some(), "cycle {c}: P1 has no body");
        });
    });
}

const SCRIPTED: &str = "#VRML V2.0 utf8\nDEF T Transform { }\nDEF S Script {\n  eventIn SFVec3f set_p\n  eventOut SFVec3f p_changed\n  field SFVec3f tmp 0 0 0\n  field MFVec3f list [ 1 2 3, 4 5 6 ]\n  field SFNode t USE T\n  url \"javascript:\n    function initialize () { tmp = new SFVec3f (TAG, 1, 2); p_changed = tmp .add (new SFVec3f (1, 1, 1)); var e = list [1]; e .x = TAG; t .translation = e; }\n    function prepareEvents () { var v = new SFVec3f (1, 2, 3); tmp = v .multiply (2); }\n    function set_p (v) { p_changed = v .add (list [0]); }\n    function eventsProcessed () { var r = new SFRotation (0, 1, 0, 1); t .rotation = r; }\n    function shutdown () { print ('shutdown TAG'); }\n  \"\n}\nROUTE S.p_changed TO T.set_translation\n";

/// 4. An Inline with a Script that makes field objects in every callback.
#[test]
fn w4_inline_script() {
    big_stack(|| {
        let (a, b) = (SCRIPTED.replace("TAG", "1"), SCRIPTED.replace("TAG", "2"));
        let (mut rt, _) = start(&[("a.wrl", &a), ("b.wrl", &b)], "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\n");
        let inl = named(&rt, ROOT, "Inl");
        let mut shutdowns = 0;

        bounded("inline-script", &mut rt, |rt, c| {
            set_url(rt, inl, alternate(c));
            frames(rt, 6);

            shutdowns += rt.prints.iter().filter(|p| p.starts_with("shutdown")).count();
            rt.prints.clear();

            // The field access of the new Script works.
            let ctx = scene_of(rt, inl);
            let (s, t) = (named(rt, ctx, "S"), named(rt, ctx, "T"));
            let tag = if c % 2 == 0 { 1.0 } else { 2.0 };

            set(rt, s, "set_p", FieldValue::SFVec3f(Vector3::new(c as f64, 0.0, 0.0)));
            frames(rt, 1);
            assert_eq!(v3(rt, t, "translation"), Vector3::new(c as f64 + 1.0, 2.0, 3.0), "cycle {c} (tag {tag})");
        });

        // One shutdown per scene replaced (the first scene's goes with cycle 1).
        assert_eq!(shutdowns, 1000, "Script shutdown runs once per disposed scene");
    });
}

const EXT_LIB: &str = "#VRML V2.0 utf8\nPROTO ExtBox [ field SFVec3f sz 2 2 2 ] {\n  Shape { geometry Box { size IS sz } }\n  DEF ExtScript Script { url \"javascript: function initialize () { } function shutdown () { }\" }\n}\n";

fn uses(lib: &str, inst: &str, size: u32) -> String {
    format!("#VRML V2.0 utf8\nEXTERNPROTO ExtBox [ field SFVec3f sz ] \"{lib}\"\nDEF {inst} ExtBox {{ sz {size} {size} {size} }}\n")
}

/// The Box of an EXTERNPROTO instance's body.
fn body_box(rt: &Runtime, inst: NodeId) -> Vector3 {
    let body = rt.instance_body(inst).unwrap_or_else(|| panic!("no body; log {:?}", rt.log));
    let shape = rt.scene.contexts[body].root_nodes.iter().flatten().copied().find(|&s| rt.builtin_name(s) == Some("Shape")).expect("Shape");
    let Some(FieldValue::SFNode(Some(geometry))) = rt.pre_value(shape, "geometry") else { panic!("no geometry") };

    v3(rt, *geometry, "size")
}

/// 5. Inlines that declare the same cached EXTERNPROTO, one after the other.
#[test]
fn w5_externproto_cache_reuse() {
    big_stack(|| {
        let (a, b) = (uses("ext_lib.wrl", "Inst", 3), uses("ext_lib.wrl", "Inst", 4));
        let (mut rt, files) = start(&[("ext_lib.wrl", EXT_LIB), ("a.wrl", &a), ("b.wrl", &b)], "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\n");
        let inl = named(&rt, ROOT, "Inl");

        bounded("externproto-cache", &mut rt, |rt, c| {
            swap(rt, inl, alternate(c));

            let inst = named(rt, scene_of(rt, inl), "Inst");
            let size = if c % 2 == 0 { 3.0 } else { 4.0 };

            assert_eq!(body_box(rt, inst), Vector3::new(size, size, size), "cycle {c}");
        });

        assert_eq!(files.fetched("ext_lib.wrl"), 1, "the cached scene is fetched once");
    });
}

/// Late EXTERNPROTO delivery: the Inline goes while its declaration's fetch is in flight.
#[test]
fn w5b_late_externproto_delivery() {
    big_stack(|| {
        let a = uses("lib.wrl?x=1", "Inst", 3);
        let (mut rt, _) = start(&[("lib.wrl", EXT_LIB), ("a.wrl", &a), ("b.wrl", "#VRML V2.0 utf8\nGroup { }\n")], "#VRML V2.0 utf8\nDEF Inl Inline { url \"b.wrl\" }\n");
        let inl = named(&rt, ROOT, "Inl");

        let mut cut = [0usize; 4];

        bounded("externproto-late", &mut rt, |rt, c| {
            set_url(rt, inl, "a.wrl");

            // Replace the scene after 0 … 3 frames: before it attaches, while its declaration's fetch
            // or scene waits, or after.
            for _ in 0..c % 4 {
                frames(rt, 1);
            }

            let d = rt.extern_diagnostics();

            if d.pending_scenes > 0 || d.load_events > 0 || rt.tasks.iter().chain(rt.fetches.iter()).any(|t| matches!(t, Task::ExternLoad(..))) {
                cut[c % 4] += 1;
            }

            set_url(rt, inl, "b.wrl");
            frames(rt, 6);
            assert_eq!(rt.extern_diagnostics().pending_scenes, 0);
            assert!(rt.tasks.is_empty() && rt.fetches.is_empty(), "cycle {c}: work left");
        });

        println!("externproto-late: replaced with EXTERNPROTO work in flight, by frames waited: {cut:?}");
        assert!(cut.iter().sum::<usize>() > 0, "no cycle cut a load in flight");
    });
}

/// Combined: an Inline with a PROTO whose body has an Inline that EXPORTs, an EXTERNPROTO from the
/// cache, an IMPORT of the nested EXPORT, a Script that holds node references, the world's IMPORT
/// of the reloaded Inline's EXPORT.
#[test]
fn w8_combined() {
    big_stack(|| {
        let leaf = "#VRML V2.0 utf8\nDEF LeafT Transform { }\nEXPORT LeafT\n";
        let scene = |x: u32| {
            format!(
                "#VRML V2.0 utf8\nEXTERNPROTO ExtBox [ field SFVec3f sz ] \"ext_lib.wrl\"\nPROTO Holder [ ] {{ Group {{ children [ DEF Nested Inline {{ url \"leaf.wrl\" }} ExtBox {{ sz {x} {x} {x} }} ] }} }}\nDEF H Holder {{ }}\nDEF Leaf Inline {{ url \"leaf.wrl\" }}\nIMPORT Leaf.LeafT AS LT\nDEF ExpT Transform {{ }}\nEXPORT ExpT AS MyExport\nROUTE ExpT.translation_changed TO LT.set_translation\nDEF S Script {{\n  field SFNode h USE H\n  field SFNode e USE ExpT\n  eventIn SFTime tick\n  url \"javascript: function tick (t) {{ e .scale = new SFVec3f ({x}, {x}, {x}); }} function shutdown () {{ print ('down'); }}\"\n}}\n"
            )
        };
        let (a, b) = (scene(1), scene(2));
        let main = "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\nIMPORT Inl.MyExport AS Imp\nDEF Src Transform { }\nROUTE Src.translation_changed TO Imp.set_translation\n";
        let (mut rt, _) = start(&[("leaf.wrl", leaf), ("ext_lib.wrl", EXT_LIB), ("a.wrl", &a), ("b.wrl", &b)], main);
        let (inl, src) = (named(&rt, ROOT, "Inl"), named(&rt, ROOT, "Src"));

        bounded("combined", &mut rt, |rt, c| {
            swap(rt, inl, alternate(c));
            frames(rt, 2);

            let ctx = scene_of(rt, inl);
            let (exp, leaf, s) = (rt.scene.get_exported_node(ctx, "MyExport").expect("MyExport"), named(rt, ctx, "Leaf"), named(rt, ctx, "S"));
            let leaf_t = rt.scene.get_exported_node(scene_of(rt, leaf), "LeafT").expect("LeafT");
            let v = Vector3::new(c as f64, 3.0, 4.0);
            let x = if c % 2 == 0 { 1.0 } else { 2.0 };

            set(rt, src, "translation", FieldValue::SFVec3f(v));
            set(rt, s, "tick", FieldValue::SFTime(c as f64));
            frames(rt, 1);
            assert_eq!(v3(rt, exp, "translation"), v, "cycle {c}: the world's IMPORT");
            assert_eq!(v3(rt, leaf_t, "translation"), v, "cycle {c}: the scene's IMPORT");
            assert_eq!(v3(rt, exp, "scale"), Vector3::new(x, x, x), "cycle {c}: the Script's node reference");
            rt.prints.clear();
        });
    });
}

// --- stale ids across slot reuse ---------------------------------------------------------------

/// Reload until the Inline's new scene takes the context slot of `old` again.
fn until_reused(rt: &mut Runtime, inl: NodeId, old: CtxId) -> usize {
    for c in 1..=8 {
        set_url(rt, inl, alternate(c));
        frames(rt, 6);

        if scene_of(rt, inl) == old {
            return c;
        }
    }

    panic!("the context slot {old} was not reused; contexts {}", rt.scene.contexts.len())
}

/// `CtxId`: a handle taken before the slot was released resolves to nothing; the new scene in the
/// same slot has a new generation and its own handle works.
#[test]
fn a_stale_context_handle_does_not_reach_the_scene_that_took_its_slot() {
    big_stack(|| {
        let (mut rt, _) = start(&[("a.wrl", EXPORT), ("b.wrl", EXPORT)], "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\n");
        let inl = named(&rt, ROOT, "Inl");
        let old = scene_of(&rt, inl);
        let stale = rt.scene.ctx_handle(old);

        until_reused(&mut rt, inl, old);

        let fresh = rt.scene.ctx_handle(old);

        assert_ne!(fresh.generation, stale.generation, "the slot was released and taken again");
        assert_eq!(rt.scene.resolve_ctx(stale), None, "the old handle names the old scene, which is gone");
        assert_eq!(rt.scene.resolve_ctx(fresh), Some(old));
        assert!(!rt.ctx_dead(old) && rt.ctx_live(old), "the new scene in the slot is live");
    });
}

/// The EXTERNPROTO scene cache: an entry that names a context slot that was released and taken by
/// another scene must not deliver that scene. It is evicted and the URL is fetched again.
#[test]
fn a_stale_cache_entry_does_not_resolve_to_the_context_that_took_its_slot() {
    big_stack(|| {
        let other = "#VRML V2.0 utf8\nPROTO ExtBox [ field SFVec3f sz 9 9 9 ] { Shape { geometry Sphere { } } }\n";
        let (mut rt, files) = start(&[("ext_lib.wrl", EXT_LIB), ("a.wrl", other), ("b.wrl", other), ("user.wrl", &uses("ext_lib.wrl", "Inst", 5))], "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\nDEF User Inline { load FALSE url \"user.wrl\" }\n");
        let inl = named(&rt, ROOT, "Inl");
        let old = scene_of(&rt, inl);
        let stale_generation = rt.scene.ctx_slots.generation(old);

        until_reused(&mut rt, inl, old);

        // `old` is now a live scene whose first PROTO is another `ExtBox`. A cache entry from before
        // the slot was reused names it by number.
        assert!(rt.extern_cache_stale(old, stale_generation), "the old generation is stale");
        assert!(!rt.extern_cache_stale(old, rt.scene.ctx_slots.generation(old)), "the current one is not");

        let key = "http://test/ext_lib.wrl".to_string();

        rt.extern_cache.push((key.clone(), CacheEntry::Ready(old, stale_generation)));

        let user = named(&rt, ROOT, "User");

        set_load(&mut rt, user, true);
        frames(&mut rt, 6);

        let inst = named(&rt, scene_of(&rt, user), "Inst");

        assert_eq!(body_box(&rt, inst), Vector3::new(5.0, 5.0, 5.0), "the instance has the ext_lib.wrl body, not the other scene's");
        assert_eq!(files.fetched("ext_lib.wrl"), 1, "the stale entry was evicted and the URL fetched");
        assert!(rt.extern_cache.iter().all(|(k, e)| k != &key || matches!(e, CacheEntry::Ready(c, _) if *c != old)), "no entry names the reused slot");
    });
}

/// `ProtoId`: a PROTO instance of a disposed scene that the world still holds keeps its declaration
/// (and the declaration's context); the slots are released once the world lets go.
#[test]
fn a_held_proto_instance_keeps_its_declaration_slot() {
    big_stack(|| {
        let scene = |s: &str| format!("#VRML V2.0 utf8\nPROTO {s} [ field SFVec3f sz 1 1 1 ] {{ Shape {{ geometry Box {{ size IS sz }} }} }}\nDEF P {s} {{ sz 2 2 2 }}\n");
        let (a, b) = (scene("Pa"), scene("Pb"));
        let (mut rt, _) = start(&[("a.wrl", &a), ("b.wrl", &b)], "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\nDEF Keep Group { }\n");
        let (inl, keep) = (named(&rt, ROOT, "Inl"), named(&rt, ROOT, "Keep"));
        let held = named(&rt, scene_of(&rt, inl), "P");
        let NodeKind::Instance(ProtoRef::Proto(p)) = rt.scene.node(held).kind else { panic!("not an instance") };
        let generation = rt.scene.proto_slots.generation(p);

        set(&mut rt, keep, "children", FieldValue::MFNode(vec![Some(held)]));

        for c in 1..=20 {
            set_url(&mut rt, inl, alternate(c));
            frames(&mut rt, 6);

            assert!(rt.scene.is_live(held), "cycle {c}: the held instance stays");
            assert_eq!(rt.type_name(held), "Pa", "cycle {c}: the held instance is still of its own PROTO");
            assert_eq!(rt.scene.proto_slots.generation(p), generation, "cycle {c}: its declaration slot was not released");
        }

        set(&mut rt, keep, "children", FieldValue::MFNode(vec![]));
        frames(&mut rt, 2);

        assert!(!rt.scene.is_live(held) || rt.scene.node(held).kind != NodeKind::Instance(ProtoRef::Proto(p)), "the instance is released once nothing holds it");
        assert_ne!(rt.scene.proto_slots.generation(p), generation, "and so is its declaration's slot");
        assert_eq!(rt.emptied_contexts(), 0, "no emptied context waits");
    });
}

/// `ExternId`: fetch tasks of a released declaration, still queued, do not reach the declaration
/// that takes the slot while its own load is in flight (request identities are runtime-wide, so no
/// request number the old declaration had can be the new one's).
#[test]
fn a_late_fetch_task_does_not_reach_the_declaration_that_took_its_slot() {
    big_stack(|| {
        let (a, b) = (uses("lib.wrl?v=1", "Inst", 3), uses("lib.wrl?v=2", "Inst", 4));
        let (mut rt, files) = start(&[("lib.wrl", EXT_LIB), ("a.wrl", &a), ("b.wrl", &b)], "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\n");
        let inl = named(&rt, ROOT, "Inl");
        let old_inst = named(&rt, scene_of(&rt, inl), "Inst");
        let NodeKind::Instance(ProtoRef::Extern(e)) = rt.scene.node(old_inst).kind else { panic!("not an extern instance") };
        let generation = rt.scene.extern_slots.generation(e);

        // Reload until the declaration slot is reused.
        let mut c = 0;

        while rt.scene.extern_slots.generation(e) == generation || rt.scene.extern_slots.is_free(e) {
            c += 1;
            assert!(c < 10, "the declaration slot {e} was not reused");
            set_url(&mut rt, inl, alternate(c));
            frames(&mut rt, 6);
        }

        let inst = named(&rt, scene_of(&rt, inl), "Inst");
        let size = if c % 2 == 0 { 3.0 } else { 4.0 };
        let fetched = files.fetched("lib.wrl");

        assert_eq!(body_box(&rt, inst), Vector3::new(size, size, size));

        // A new load of the declaration that has the slot now, up to where its scene is parsed and waits
        // (its fetch is current). Then every request number handed out before it is delivered as if late.
        let bodies = rt.bodies_disposed;
        let before = rt.request_counter;

        rt.extern_load_now(e);

        for _ in 0..4 {
            frames(&mut rt, 1);

            if rt.extern_diagnostics().pending_scenes > 0 {
                break;
            }
        }

        assert!(rt.extern_diagnostics().pending_scenes > 0, "the new load is not in flight");

        for request in 1..=before {
            rt.tasks.push_back(Task::ExternLoad(e, request));
        }

        // The tasks run. A stale one that matched the current request would end the load (FAILED)
        // before its scene arrives at the end of the frame.
        rt.settle();
        assert_eq!(rt.extern_load_state(e), x3d_runtime::nodes::LoadState::InProgress, "a stale task ended the new load");
        frames(&mut rt, 4);

        assert!(rt.log.is_empty(), "a stale task ran the new declaration's load: {:?}", rt.log);
        assert_eq!(rt.extern_load_state(e), x3d_runtime::nodes::LoadState::Complete);
        assert_eq!(files.fetched("lib.wrl"), fetched + 1, "one fetch, the new load's");
        assert_eq!(body_box(&rt, inst), Vector3::new(size, size, size), "the instance's body is its own PROTO's");
        assert_eq!(rt.bodies_disposed, bodies + 1, "the instance's body was replaced once, by the new load (a stale task fails the load first)");
    });
}

/// A live context names its parent: a scene nested in a disposed scene that the world still holds a
/// node of keeps its parent's slot (emptied, not released), so the parent chain never leads into a scene
/// that took the slot. `parent` being set is not proof that the parent is live (R0.3-D, FIX-1 §4).
#[test]
fn a_held_nested_scene_keeps_its_parent_slot() {
    big_stack(|| {
        let leaf = "#VRML V2.0 utf8\nDEF Clock TimeSensor { loop TRUE }\n";
        let mid = "#VRML V2.0 utf8\nDEF Leaf Inline { url \"leaf.wrl\" }\n";
        let (mut rt, _) = start(&[("leaf.wrl", leaf), ("a.wrl", mid), ("b.wrl", mid)], "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\nDEF Keep Group { }\n");
        let (inl, keep) = (named(&rt, ROOT, "Inl"), named(&rt, ROOT, "Keep"));
        let outer = scene_of(&rt, inl);
        let nested = scene_of(&rt, named(&rt, outer, "Leaf"));
        let clock = named(&rt, nested, "Clock");

        set(&mut rt, keep, "children", FieldValue::MFNode(vec![Some(clock)]));
        assert!(rt.ctx_live(nested));

        for c in 1..=20 {
            set_url(&mut rt, inl, alternate(c));
            frames(&mut rt, 6);

            assert!(rt.scene.is_live(clock), "cycle {c}: the held TimeSensor stays");
            assert!(rt.ctx_dead(nested) && !rt.ctx_live(nested), "cycle {c}: the held node's scene is part of a disposed scene");
            assert!(!rt.scene.ctx_slots.is_free(outer), "cycle {c}: its parent's slot is not released");
            assert_ne!(scene_of(&rt, inl), outer, "cycle {c}: no new scene took the parent's slot");
        }

        set(&mut rt, keep, "children", FieldValue::MFNode(vec![]));
        frames(&mut rt, 2);

        assert!(rt.scene.ctx_slots.is_free(outer) || scene_of(&rt, inl) == outer, "the parent's slot is released once nothing holds the nested scene");
        assert_eq!(rt.emptied_contexts(), 0);
    });
}

/// IMPORT and EXPORT records: the records of a released scene are released; the ids the old scene had
/// now belong to the new scene's records, which name the new scene's nodes; no stale record stays.
#[test]
fn stale_import_and_export_records_do_not_stay_active() {
    big_stack(|| {
        let leaf = "#VRML V2.0 utf8\nDEF LeafT Transform { }\nEXPORT LeafT\n";
        let scene = "#VRML V2.0 utf8\nDEF Leaf Inline { url \"leaf.wrl\" }\nIMPORT Leaf.LeafT AS LT\nDEF ExpT Transform { }\nEXPORT ExpT AS MyExport\n";
        let (mut rt, _) = start(&[("leaf.wrl", leaf), ("a.wrl", scene), ("b.wrl", scene)], "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\n");
        let inl = named(&rt, ROOT, "Inl");
        let old = scene_of(&rt, inl);
        let imported = rt.scene.ctx(old).imported.get("LT").unwrap();
        let exported = rt.scene.ctx(old).exported.get("MyExport").unwrap();
        let (gi, ge) = (rt.scene.imported_slots.generation(imported), rt.scene.exported_slots.generation(exported));

        until_reused(&mut rt, inl, old);
        frames(&mut rt, 2);

        assert_ne!(rt.scene.imported_slots.generation(imported), gi, "the IMPORT record slot was released");
        assert_ne!(rt.scene.exported_slots.generation(exported), ge, "the EXPORT record slot was released");

        // Every record that is not released belongs to a context that is live.
        for (i, r) in rt.scene.imported.iter().enumerate().filter(|(i, _)| !rt.scene.imported_slots.is_free(*i)) {
            assert!(!rt.ctx_dead(r.ctx) && !rt.scene.ctx_slots.is_free(r.ctx), "IMPORT record {i} of a gone context");
            assert!(rt.scene.is_live(r.instance) && rt.scene.node(r.instance).kind == NodeKind::Imported(i), "IMPORT record {i} names a node that is not its instance");
        }

        for (x, r) in rt.scene.exported.iter().enumerate().filter(|(x, _)| !rt.scene.exported_slots.is_free(*x)) {
            assert!(!rt.ctx_dead(r.ctx) && !rt.scene.ctx_slots.is_free(r.ctx), "EXPORT record {x} of a gone context");
            assert_eq!(rt.scene.node(r.local).ctx, r.ctx, "EXPORT record {x} names a node of another scene");
        }

        let ctx = scene_of(&rt, inl);
        let exp = rt.scene.get_exported_node(ctx, "MyExport").unwrap();

        assert_eq!(exp, named(&rt, ctx, "ExpT"), "the EXPORT of the new scene names its own node");
    });
}

/// A disposed scene's PROTO body that an EXTERNPROTO declaration names through `extern_loaded`: the
/// uncached EXTERNPROTO scene (a URL with a query) goes with the Inline; nothing names it afterwards.
#[test]
fn uncached_externproto_scenes_are_released_with_their_inline() {
    big_stack(|| {
        let (a, b) = (uses("lib.wrl?q=1", "Inst", 3), uses("lib.wrl?q=1", "Inst", 4));
        let (mut rt, _) = start(&[("lib.wrl", EXT_LIB), ("a.wrl", &a), ("b.wrl", &b)], "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\n");
        let inl = named(&rt, ROOT, "Inl");

        bounded("externproto-uncached", &mut rt, |rt, c| {
            set_url(rt, inl, alternate(c));
            frames(rt, 6);

            let inst = named(rt, scene_of(rt, inl), "Inst");
            let size = if c % 2 == 0 { 3.0 } else { 4.0 };

            assert_eq!(body_box(rt, inst), Vector3::new(size, size, size), "cycle {c}");
            assert_eq!(rt.extern_states.len(), 1, "cycle {c}: one declaration state, the live one's");
        });
    });
}

// --- Scripts during disposal -------------------------------------------------------------------

/// A Script whose scene is disposed while it is in an event cascade (its own output replaces the
/// Inline that holds it): it shuts down once, the cascade ends, nothing it held reaches a new node.
#[test]
fn a_script_disposed_during_its_own_event_cascade() {
    big_stack(|| {
        let scene = |tag: &str, next: &str| {
            format!(
                "#VRML V2.0 utf8\nDEF S Script {{\n  eventIn SFBool go\n  eventOut MFString next\n  field SFVec3f keep 0 0 0\n  url \"javascript:\n    function go (v) {{ keep = new SFVec3f (1, 2, 3); next = new MFString ('{next}'); }}\n    function eventsProcessed () {{ keep = keep .add (new SFVec3f (1, 0, 0)); }}\n    function shutdown () {{ print ('shutdown {tag}'); }}\n  \"\n}}\n"
            )
        };
        let (a, b) = (scene("a", "b.wrl"), scene("b", "a.wrl"));
        let main = "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\nDEF Relay Script { eventIn MFString next eventOut MFString url_changed url \"javascript: function next (v) { url_changed = v; }\" }\nROUTE Relay.url_changed TO Inl.set_url\n";
        let (mut rt, _) = start(&[("a.wrl", &a), ("b.wrl", &b)], main);
        let (inl, relay) = (named(&rt, ROOT, "Inl"), named(&rt, ROOT, "Relay"));
        let mut base = None;

        for c in 1..=300 {
            let ctx = scene_of(&rt, inl);
            let s = named(&rt, ctx, "S");

            // The Script's output goes to the world's relay, which replaces the Inline holding the Script.
            let next = rt.get_field(s, "next").unwrap();
            let relay_in = rt.get_field(relay, "next").unwrap();

            rt.add_field_interest(next, relay_in);
            set(&mut rt, s, "go", FieldValue::SFBool(true));
            frames(&mut rt, 6);

            let tag = if c % 2 == 1 { "a" } else { "b" };

            assert_eq!(rt.prints.iter().filter(|p| **p == format!("shutdown {tag}")).count(), 1, "cycle {c}: {:?}", rt.prints);
            rt.prints.clear();

            if c == WARM_UP {
                base = Some(tables(&rt));
            }

            if let Some(b) = &base {
                assert_eq!(&tables(&rt), b, "cycle {c}");
            }
        }

        assert!(rt.log.iter().all(|l| l.starts_with("warn")), "{:?}", rt.log);
    });
}

/// Rapid replacement: the Inline's URL changes on every frame, so scenes are discarded before they
/// attach, while their Scripts have run `initialize ()`.
#[test]
fn rapid_unload_and_reload_of_scripted_scenes() {
    big_stack(|| {
        let (a, b) = (SCRIPTED.replace("TAG", "1"), SCRIPTED.replace("TAG", "2"));
        let (mut rt, _) = start(&[("a.wrl", &a), ("b.wrl", &b)], "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\n");
        let inl = named(&rt, ROOT, "Inl");

        bounded("rapid-script", &mut rt, |rt, c| {
            for k in 0..3 {
                set_url(rt, inl, alternate(c + k));
                frames(rt, 1);
            }

            set_load(rt, inl, false);
            frames(rt, 1);
            set_load(rt, inl, true);
            frames(rt, 6);
            assert!(rt.inline_scenes.contains_key(&inl), "cycle {c}");
            rt.prints.clear();
        });
    });
}

/// A Script in the world keeps field objects of nodes of a reloaded Inline: the wrappers keep the
/// value they had (tombstone semantics of R0.3-D) and never read the field of the node that takes
/// the slot. When the Script lets them go, their slots are released.
#[test]
fn script_wrappers_of_released_nodes_keep_their_value_and_then_go() {
    big_stack(|| {
        let scene = |x: u32| format!("#VRML V2.0 utf8\nDEF T Transform {{ translation {x} {x} {x} }}\n");
        let (a, b) = (scene(1), scene(2));
        let main = "#VRML V2.0 utf8\nDEF Inl Inline { url \"a.wrl\" }\nDEF S Script {\n  field SFNode t NULL\n  eventIn SFBool grab\n  eventIn SFBool check\n  eventIn SFBool drop\n  url \"javascript:\n    var kept = [];\n    function grab (v) { kept .push (t .translation); t = null; }\n    function check (v) { print ('kept ' + kept [0] + ' ' + kept .length); }\n    function drop (v) { kept = []; }\n  \"\n}\n";
        let (mut rt, _) = start(&[("a.wrl", &a), ("b.wrl", &b)], main);
        let (inl, s) = (named(&rt, ROOT, "Inl"), named(&rt, ROOT, "S"));

        let t = named(&rt, scene_of(&rt, inl), "T");

        set(&mut rt, s, "t", FieldValue::SFNode(Some(t)));
        set(&mut rt, s, "grab", FieldValue::SFBool(true));
        frames(&mut rt, 1);

        for c in 1..=6 {
            set_url(&mut rt, inl, alternate(c));
            frames(&mut rt, 6);
        }

        set(&mut rt, s, "check", FieldValue::SFBool(true));
        frames(&mut rt, 1);
        assert_eq!(rt.prints.last().map(String::as_str), Some("kept 1 1 1 1"), "the wrapper keeps the old value; {:?}", rt.log);

        let held = slot_stats();

        set(&mut rt, s, "drop", FieldValue::SFBool(true));
        frames(&mut rt, 1);

        for c in 1..=4 {
            set_url(&mut rt, inl, alternate(c));
            frames(&mut rt, 6);
        }

        let after = slot_stats();

        assert!(after.len - after.free < held.len - held.free, "the dropped wrapper's slot was released: {held:?} → {after:?}");
    });
}

