//! REA batch 08 (svg lane): an Inline that holds an SVG scene (gradient textures included) is unloaded and loaded
//! again 100 times, alternating two SVG documents, and the runtime tables and the process heap stay flat after the
//! warm-up. A second test parses and drops SVG documents 100 times (the parser keeps nothing between runs).
//! `MEM` lines at cycles 10, 50 and 100.
//!
//!   cargo test --release -p x3d-viewer --test svg_b08_lifecycle -- --nocapture

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicIsize, Ordering};
use x3d_fields::{FieldValue, NodeId};
use x3d_runtime::Runtime;
use x3d_scene::{CtxId, ROOT};
use x3d_viewer::viewer::{ORACLE_PAGE_ATTRIBUTES, Viewer3d};

struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        LIVE.fetch_add(l.size() as isize, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size() as isize, Ordering::Relaxed);
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        LIVE.fetch_add(n as isize - l.size() as isize, Ordering::Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// Both tests measure the whole process heap, so they run one at a time.
static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 60.0;

const BIG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="200" height="200" viewBox="0 0 200 200">
<defs><linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient>
<radialGradient id="r" fx="0.3"><stop offset="0" stop-color="white"/><stop offset="1" stop-color="black"/></radialGradient></defs>
<circle cx="100" cy="100" r="80" fill="url(#g)" stroke="black"/>
<path d="M 10 100 C 10 10 190 10 190 100 S 100 190 10 100 Z" fill="url(#r)" fill-rule="evenodd"/>
<polygon points="10,10 190,10 100,190 10,10 60,60 140,60 100,140" fill="orange" stroke="navy" stroke-width="3"/>
<g transform="rotate(20 100 100)"><rect x="20" y="20" width="60" height="40" rx="8" fill="#0a0"/><use xlink:href="#g" /></g>
</svg>"##;

const SMALL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect width="40" height="40" fill="green"/></svg>"##;

fn big(f: impl FnOnce() + Send + 'static) {
    let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());

    std::thread::Builder::new().stack_size(256 << 20).spawn(f).unwrap().join().unwrap();
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("x3d-svg-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(d: &Path, name: &str, text: &str) -> PathBuf {
    let p = d.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

fn rss_kib() -> usize {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("VmRSS:").and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok())))
        .unwrap_or(0)
}

struct World {
    v: Viewer3d,
    t: f64,
}

impl World {
    fn steps(&mut self, n: usize) {
        for _ in 0..n {
            self.t += DT;
            self.v.date_now = self.t;
            self.v.step(self.t);
        }
    }

    fn named(&self, ctx: CtxId, name: &str) -> NodeId {
        self.v.rt.scene.contexts[ctx].named_nodes.get(name).unwrap_or_else(|| panic!("no {name}"))
    }

    fn set(&mut self, node: NodeId, name: &str, value: FieldValue) {
        let k = self.v.rt.get_field(node, name).unwrap();
        self.v.rt.set_value(k, value);
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct Resources {
    live_nodes: usize,
    arena: usize,
    arena_cap: usize,
    nrt: usize,
    frt: usize,
    contexts: usize,
    live_watchers: usize,
}

fn resources(rt: &Runtime) -> Resources {
    let s = &rt.scene;

    Resources { live_nodes: s.nodes.len() - s.free_nodes.len(), arena: s.nodes.len(), arena_cap: s.nodes.capacity(), nrt: rt.nrt.len(), frt: rt.frt.len(), contexts: s.contexts.len(), live_watchers: rt.live_watchers.len() }
}

#[test]
fn an_svg_inline_comes_and_goes_100_cycles() {
    big(|| {
        let d = dir("cycles");

        write(&d, "a.svg", BIG);
        write(&d, "b.svg", SMALL);

        let main = write(&d, "main.x3dv", "#X3D V3.3 utf8\nPROFILE Full\nDEF Inl Inline { url \"a.svg\" }\n");
        let mut w = World { v: Viewer3d::load_attributes(&main, Some(&d), T0, 64, 64, Some(60.0), false, ORACLE_PAGE_ATTRIBUTES).expect("load"), t: T0 };

        w.v.frame_rate = 60.0;
        w.steps(6);

        let ctx = w.v.rt.inline_scenes[&w.named(ROOT, "Inl")];
        let first = resources(&w.v.rt);

        assert!(w.v.rt.scene.contexts[ctx].named_nodes.get("ViewBox").is_some(), "the SVG scene is the Inline's: {first:?}");
        assert!(first.live_nodes > 30, "{first:?}");

        let (mut unloaded, mut loaded_big, mut loaded_small): (Option<Resources>, Option<Resources>, Option<Resources>) = (None, None, None);
        let mut heap = vec![];

        for c in 1..=100 {
            let inl = w.named(ROOT, "Inl");

            w.set(inl, "load", FieldValue::SFBool(false));
            w.steps(3);

            let r = resources(&w.v.rt);

            if c == 10 {
                unloaded = Some(r.clone());
            }

            if let Some(b) = &unloaded {
                assert_eq!(&r, b, "cycle {c}: a table grew after the warm-up (unloaded)");
            }

            let big_doc = c % 2 == 0;

            w.set(inl, "url", FieldValue::MFString(vec![if big_doc { "a.svg" } else { "b.svg" }.into()]));
            w.set(inl, "load", FieldValue::SFBool(true));
            w.steps(8);

            let ctx = w.v.rt.inline_scenes[&w.named(ROOT, "Inl")];

            // Background, OrthoViewpoint and the (possibly optimized) ViewBox Transform; the big document
            // keeps its ViewBox name, the small one is combined into its Transform like X_ITE's.
            assert_eq!(w.v.rt.scene.contexts[ctx].root_nodes.len(), 3, "cycle {c}: the SVG scene loaded");
            assert_eq!(w.v.rt.scene.contexts[ctx].named_nodes.get("ViewBox").is_some(), big_doc, "cycle {c}: ViewBox name");

            let r = resources(&w.v.rt);
            let slot = if big_doc { &mut loaded_big } else { &mut loaded_small };

            if c == 10 || c == 11 {
                *slot = Some(r.clone());
            }

            if let Some(b) = slot {
                assert_eq!(&*&r, &*b, "cycle {c}: a table grew after the warm-up (loaded, big {big_doc})");
            }

            if [10, 50, 100].contains(&c) {
                let live = LIVE.load(Ordering::Relaxed);

                heap.push(live);
                println!("MEM\tsvg-inline-cycle\t{c}\tlive_nodes={}\tarena={}\tnrt={}\tfrt={}\tcontexts={}\theap_bytes={live}\trss_kib={}", r.live_nodes, r.arena, r.nrt, r.frt, r.contexts, rss_kib());
            }
        }

        // Even cycles end with the big document loaded, so cycles 10, 50 and 100 are comparable.
        // Exactly flat when run alone (--nocapture --test-threads=1: 3174409 B at cycles 50 and 100); under the
        // harness, the captured MEM lines and the other test thread add a few hundred bytes.
        assert!((heap[2] - heap[1]).abs() <= 2048, "heap grew between cycle 50 and 100: {heap:?}");
        assert!((heap[1] - heap[0]).abs() <= 64 * 1024, "heap grew between cycle 10 and 50: {heap:?}");

        let _ = std::fs::remove_dir_all(&d);
    });
}

#[test]
fn parsing_svg_100_times_keeps_nothing() {
    big(|| {
        let mut heap = vec![];

        for c in 1..=100 {
            for doc in [BIG, SMALL] {
                let scene = x3d_parse::parse_bytes(doc.as_bytes(), "http://oracle/x.svg").unwrap();

                assert!(scene.nodes.len() > 4);
            }

            if [10, 50, 100].contains(&c) {
                let live = LIVE.load(Ordering::Relaxed);

                heap.push(live);
                println!("MEM\tsvg-parse-cycle\t{c}\theap_bytes={live}\trss_kib={}", rss_kib());
            }
        }

        // Exactly flat when run alone (--nocapture --test-threads=1: 3174409 B at cycles 50 and 100); under the
        // harness, the captured MEM lines and the other test thread add a few hundred bytes.
        assert!((heap[2] - heap[1]).abs() <= 2048, "heap grew between cycle 50 and 100: {heap:?}");
        assert!((heap[1] - heap[0]).abs() <= 4096, "heap grew between cycle 10 and 50: {heap:?}");
    });
}
