//! Script `shutdown ()` on world disposal: a kept, owner-approved DIFFERENCE from X_ITE.
//!
//! X_ITE 16.5.0 does NOT call a Script's `shutdown ()` when the world is replaced (`replaceWorld`) or when an
//! Inline's scene is replaced (verified against the pinned build with `console.log` in `shutdown`). The Rust port
//! calls it (ISO/IEC 14772-1 4.12.3: `shutdown ()` runs when the Script node is deleted or the world containing it
//! is unloaded or replaced): `Runtime::dispose_scene` closes every Script of the disposed contexts, once, while the
//! scene is still connected. The owner decided to keep that behavior, so these tests lock it; they do not compare
//! with X_ITE and must not be "fixed" by removing the call. Ledger status is unchanged by this file.
//!
//! What is locked:
//! * `shutdown ()` runs exactly once per Script when the world is unloaded (a second unload adds nothing);
//! * the same for the Scripts of an Inline's scene that is switched off or replaced (the load/unload cycle);
//! * a Script that is only removed from a live Group's `children` (no scene is disposed) gets no `shutdown ()`
//!   (the port has no per-node deletion hook; the node is released by the sweep, see release.rs);
//! * the nodes a Script created are released after the disposal, and 20 load/unload cycles leave the node arena,
//!   the release candidates and the Script-created nodes bounded.

use std::collections::HashMap;
use std::rc::Rc;
use x3d_fields::FieldValue;
use x3d_runtime::{Fetcher, Runtime};

const BASE: &str = "http://oracle/shutdown-kept/";

struct Files(Rc<HashMap<String, Vec<u8>>>);

impl Fetcher for Files {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        self.0.get(url.path().trim_start_matches("/shutdown-kept/")).cloned().ok_or_else(|| "Error: Not Found".to_string())
    }
}

/// The Script of the nested scene: logs initialize and shutdown and creates nodes at run time (one of them a Script).
const CONTENT: &str = r#"#VRML V2.0 utf8
DEF Holder Group { }
DEF Maker Script {
  directOutput TRUE
  field SFNode holder USE Holder
  field MFNode made [ ]
  url "javascript:
    function initialize () {
      print ('Maker initialize');
      made = Browser .createVrmlFromString ('Group { children [ Transform { children Shape { geometry Box { } } } Transform { } ] }');
      holder .children = made;
    }
    function shutdown () { print ('Maker shutdown'); }
  "
}
"#;

fn world(files: &[(&str, &str)], body: &str) -> Runtime {
    let text = format!("#VRML V2.0 utf8\n{body}");
    let scene = x3d_parse::parse_bytes(text.as_bytes(), &format!("{BASE}main.wrl")).unwrap();
    let map: HashMap<String, Vec<u8>> = files.iter().map(|(n, c)| (n.to_string(), c.as_bytes().to_vec())).collect();
    let mut rt = Runtime::new(scene, Box::new(Files(Rc::new(map))));

    rt.script_host = Some(Box::new(x3d_script::QuickJsHost::new()));
    rt.record_console = true;
    rt.now = 946684800.0;
    rt.load_world();
    rt.settle();
    rt.frame();
    rt.settle();
    rt
}

fn count(rt: &Runtime, line: &str) -> usize {
    rt.console.iter().filter(|l| l.as_str() == line).count()
}

fn step(rt: &mut Runtime) {
    rt.now += 0.1;
    rt.frame();
    rt.settle();
}

fn set_load(rt: &mut Runtime, on: bool) {
    let inl = rt.scene.get_named_node(x3d_scene::ROOT, "Inl").unwrap();
    let k = rt.get_field(inl, "load").unwrap();

    rt.set_value(k, FieldValue::SFBool(on));
}

fn arena_live(rt: &Runtime) -> usize {
    rt.scene.nodes.len() - rt.scene.free_nodes.len()
}

const MAIN: &str = r#"DEF Outer Script { url "javascript:
    function initialize () { print ('Outer initialize'); }
    function shutdown () { print ('Outer shutdown'); }
  " }
DEF Inl Inline { url "content.wrl" }
"#;

fn big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new().stack_size(64 << 20).spawn(f).unwrap().join().unwrap()
}

/// Unloading the world (what replacing it does) runs `shutdown ()` of every Script once, nested scene included.
#[test]
fn unloading_the_world_runs_shutdown_once_per_script() {
    big_stack(|| {
        let mut rt = world(&[("content.wrl", CONTENT)], MAIN);

        assert_eq!((count(&rt, "log: Outer initialize"), count(&rt, "log: Maker initialize")), (1, 1), "{:?}", rt.console);
        assert_eq!(count(&rt, "log: Outer shutdown") + count(&rt, "log: Maker shutdown"), 0);

        rt.unload_world();
        rt.settle();
        step(&mut rt);
        rt.unload_world();
        rt.settle();
        step(&mut rt);

        assert_eq!(count(&rt, "log: Outer shutdown"), 1, "{:?}", rt.console);
        assert_eq!(count(&rt, "log: Maker shutdown"), 1, "{:?}", rt.console);
    });
}

/// Switching the Inline off, then on, then off again: each disposal of its scene shuts the nested Script down once.
#[test]
fn replacing_an_inline_scene_runs_shutdown_once_each_time() {
    big_stack(|| {
        let mut rt = world(&[("content.wrl", CONTENT)], MAIN);

        for round in 1..=3usize {
            set_load(&mut rt, false);
            rt.settle();
            step(&mut rt);
            step(&mut rt);
            assert_eq!(count(&rt, "log: Maker shutdown"), round, "round {round}: {:?}", rt.console);

            set_load(&mut rt, true);
            rt.settle();
            step(&mut rt);
            step(&mut rt);
            assert_eq!(count(&rt, "log: Maker initialize"), round + 1, "round {round}: {:?}", rt.console);
        }

        assert_eq!(count(&rt, "log: Outer shutdown"), 0, "the world itself is still there");
    });
}

/// A Script that is only removed from a live Group (no scene is disposed) gets no `shutdown ()` in this port.
#[test]
fn removal_from_a_live_group_alone_does_not_run_shutdown() {
    big_stack(|| {
        let body = r#"DEF G Group { children DEF Q Script { url "javascript:
            function initialize () { print ('Q initialize'); }
            function shutdown () { print ('Q shutdown'); }
          " } }
        "#;
        let mut rt = world(&[], body);
        let g = rt.scene.get_named_node(x3d_scene::ROOT, "G").unwrap();
        let k = rt.get_field(g, "children").unwrap();

        rt.set_value(k, FieldValue::MFNode(vec![]));
        rt.settle();

        for _ in 0..40 {
            step(&mut rt);
        }

        assert_eq!(count(&rt, "log: Q initialize"), 1, "{:?}", rt.console);
        assert_eq!(count(&rt, "log: Q shutdown"), 0, "{:?}", rt.console);

        // The unload is the disposal that shuts it down (the removed node's context is still part of the world), once.
        rt.unload_world();
        rt.settle();
        step(&mut rt);
        assert_eq!(count(&rt, "log: Q shutdown"), 1, "{:?}", rt.console);
    });
}

/// 20 load/unload cycles of an Inline whose Script creates nodes: one `shutdown ()` per cycle, and afterwards the node
/// arena, the release candidates and the Script-created nodes are back at the size after the first cycle.
#[test]
fn twenty_load_unload_cycles_release_everything() {
    big_stack(|| {
        let mut rt = world(&[("content.wrl", CONTENT)], MAIN);
        let mut live_off = vec![];
        let mut arena = vec![];

        for cycle in 1..=20usize {
            set_load(&mut rt, false);
            rt.settle();

            // The sweep runs at the end of a frame; give a candidate that waits for an event a few frames.
            for _ in 0..4 {
                step(&mut rt);
            }

            assert_eq!(count(&rt, "log: Maker shutdown"), cycle, "cycle {cycle}: {:?}", rt.console);
            live_off.push(arena_live(&rt));
            arena.push(rt.scene.nodes.len());

            set_load(&mut rt, true);
            rt.settle();

            for _ in 0..4 {
                step(&mut rt);
            }

            assert_eq!(count(&rt, "log: Maker initialize"), cycle + 1, "cycle {cycle}");
        }

        // With the Inline off, what is live is the same after every cycle (the nodes the Script made went with its scene).
        assert!(live_off.windows(2).all(|w| w[0] == w[1]), "live nodes while unloaded: {live_off:?}");
        assert!(arena[2..].windows(2).all(|w| w[0] == w[1]), "the arena keeps growing: {arena:?}");
        assert!(rt.released_total > 20, "nothing was released: {}", rt.released_total);
        assert!(rt.release_pending() < 64, "candidates pile up: {}", rt.release_pending());
        assert!(rt.dynamic_nodes.len() < 64, "Script-created nodes pile up: {}", rt.dynamic_nodes.len());

        rt.unload_world();
        rt.settle();
        step(&mut rt);
        assert_eq!(count(&rt, "log: Outer shutdown"), 1);
        assert_eq!(count(&rt, "log: Maker shutdown"), 20 + 1, "the last load is shut down by the unload: {:?}", rt.console);
    });
}
