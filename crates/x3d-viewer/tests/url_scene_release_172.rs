//! Issue #172: the nodes of a scene that a Script loaded with `createX3DFromURL` or `createVrmlFromURL` are released once
//! nothing refers to them, like the nodes of a `createVrmlFromString` scene (release.rs, "Scenes loaded from a URL").
//!
//! X_ITE frees such a scene by JavaScript garbage collection: after `removeLoadingObject (fileLoader)` (Browser/X3DBrowser.js)
//! the browser holds nothing of it, only the promise's value or the field the root nodes went to. A scene that passes the
//! simple-scene gate (one context, no PROTO, EXTERNPROTO, IMPORT or EXPORT) is a sweep candidate once the Script has it;
//! it stays while a field, a JavaScript variable (its X3DScene object or one of its nodes) or a route refers to it.
//! Newly authored worlds; time is virtual.

use std::path::PathBuf;
use x3d_fields::{FieldValue, NodeId};
use x3d_scene::{CtxId, ROOT};
use x3d_viewer::viewer::{ORACLE_PAGE_ATTRIBUTES, Viewer3d};

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 30.0;

/// Four nodes, two roots; `P` names the scene.
const PART: &str =
    "#VRML V2.0 utf8\nDEF P Transform { children Shape { geometry Box { } } }\nDEF Q Group { }\n";

/// A scene with a PROTO: not a sweep candidate (the simple-scene gate).
const PROTO_PART: &str =
    "#VRML V2.0 utf8\nPROTO X [ ] { Group { } }\nDEF P Transform { children X { } }\n";

const SCRIPT: &str = "
  var held = null, heldNode = null;
  function loadKeep (v)  { Browser .createX3DFromURL (new MFString ('part.wrl')) .then (function (s) { held = s; }); }
  function loadDrop (v)  { Browser .createX3DFromURL (new MFString ('part.wrl')) .then (function (s) { }); }
  function loadNode (v)  { Browser .createX3DFromURL (new MFString ('part.wrl')) .then (function (s) { heldNode = s .rootNodes [1]; }); }
  function loadVrml (v)  { Browser .createVrmlFromURL (new MFString ('part.wrl'), grp, 'set_children'); }
  function loadProto (v) { Browser .createX3DFromURL (new MFString ('proto.wrl')) .then (function (s) { }); }
  function clear (v)     { grp .children = new MFNode (); }
  function drop (v)      { held = null; heldNode = null; }
  function loadMark (v)  { Browser .createX3DFromURL (new MFString ('part.wrl')) .then (function (s) { s .setProfile ('stale'); held = s; }); }
  function report (v)    { print ('profile: ' + (held .profile === 'stale' ? 'stale' : held .profile .name)); }
";

fn world() -> String {
    format!(
        "#X3D V3.3 utf8\nPROFILE Full\nDEF GRP Group {{ }}\nDEF SC Script {{ directOutput TRUE
  inputOnly SFBool loadKeep inputOnly SFBool loadDrop inputOnly SFBool loadNode inputOnly SFBool loadVrml
  inputOnly SFBool loadProto inputOnly SFBool clear inputOnly SFBool drop
  inputOnly SFBool loadMark inputOnly SFBool report
  initializeOnly SFNode grp USE GRP
  url \"ecmascript:{SCRIPT}\" }}\n"
    )
}

struct World {
    v: Viewer3d,
    t: f64,
    dir: PathBuf,
}

impl World {
    fn new(tag: &str) -> World {
        let dir = std::env::temp_dir().join(format!("x3d-172-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("part.wrl"), PART).unwrap();
        std::fs::write(dir.join("proto.wrl"), PROTO_PART).unwrap();

        let p = dir.join("w.x3dv");
        std::fs::write(&p, world()).unwrap();

        let mut v = Viewer3d::load_attributes(
            &p,
            Some(&dir),
            T0,
            64,
            64,
            Some(1.0 / DT),
            false,
            ORACLE_PAGE_ATTRIBUTES,
        )
        .expect("load");

        v.frame_rate = 1.0 / DT;

        let mut w = World { v, t: T0, dir };

        w.steps(4);
        w
    }

    fn steps(&mut self, n: usize) {
        for _ in 0..n {
            self.t += DT;
            self.v.date_now = self.t;
            self.v.step(self.t);
        }
    }

    /// Send `true` to an inputOnly of the Script SC, then run `frames` frames.
    fn call(&mut self, name: &str, frames: usize) {
        let sc = self.v.rt.scene.contexts[ROOT]
            .named_nodes
            .get("SC")
            .expect("SC");
        let k = self.v.rt.get_field(sc, name).expect("a Script field");

        self.v.rt.set_value(k, FieldValue::SFBool(true));
        self.steps(frames);
    }

    /// The context of the loaded scene (the one that names `P`), if it still has nodes.
    fn scene(&self) -> Option<CtxId> {
        let rt = &self.v.rt;

        (0..rt.scene.contexts.len()).find(|&c| {
            c != ROOT
                && rt.scene.contexts[c]
                    .named_nodes
                    .get("P")
                    .is_some_and(|n| rt.scene.is_live(n))
        })
    }

    fn nodes_of(&self, c: CtxId) -> Vec<NodeId> {
        let rt = &self.v.rt;

        (0..rt.scene.nodes.len() as NodeId)
            .filter(|&n| rt.scene.is_live(n) && rt.scene.node(n).ctx == c)
            .collect()
    }

    fn live_nodes(&self) -> usize {
        let rt = &self.v.rt;

        (0..rt.scene.nodes.len() as NodeId)
            .filter(|&n| rt.scene.is_live(n))
            .count()
    }

    fn grp_children(&self) -> Vec<NodeId> {
        let grp = self.v.rt.scene.contexts[ROOT]
            .named_nodes
            .get("GRP")
            .unwrap();

        match self.v.rt.pre_value(grp, "children") {
            Some(FieldValue::MFNode(c)) => c.iter().flatten().copied().collect(),
            o => panic!("children: {o:?}"),
        }
    }

    /// The scene is loaded and live, and it has its four nodes.
    fn loaded(&self) -> (CtxId, Vec<NodeId>) {
        let c = self.scene().expect("the scene is loaded");
        let nodes = self.nodes_of(c);

        assert_eq!(nodes.len(), 4, "Transform, Shape, Box, Group: {nodes:?}");
        (c, nodes)
    }

    fn assert_released(&self, c: CtxId, nodes: &[NodeId]) {
        let rt = &self.v.rt;

        for &n in nodes {
            assert!(!rt.scene.is_live(n), "node {n} of the scene was released");
        }

        assert!(rt.scene.contexts[c].named_nodes.is_empty());
        assert!(!rt.dynamic_ctxs.contains(&c));
        assert!(!rt.dynamic_scenes.contains(&c));
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn big(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

/// createX3DFromURL: the Script keeps the X3DScene in a variable. The scene stays (also through idle sweeps); when the
/// Script drops it, the sweep releases every node and empties the context.
#[test]
fn a_held_url_scene_stays_and_goes_when_the_script_drops_it() {
    big(|| {
        let mut w = World::new("keep");

        w.call("loadKeep", 10);

        let (c, nodes) = w.loaded();

        assert!(w.v.rt.dynamic_ctxs.contains(&c), "a sweep candidate");
        assert!(w.v.rt.dynamic_scenes.contains(&c));

        // 30 frames without an event start a sweep: the held scene stays.
        w.steps(70);
        assert_eq!(w.nodes_of(c), nodes, "the Script holds the scene");

        w.call("drop", 3);
        w.assert_released(c, &nodes);
    });
}

/// createX3DFromURL: the promise's value is dropped at once; nothing refers to the scene, it goes in the frame it settles.
#[test]
fn a_dropped_url_scene_is_released() {
    big(|| {
        let mut w = World::new("drop");
        let released = w.v.rt.released_total;

        w.call("loadDrop", 10);

        assert_eq!(w.scene(), None, "no node of the scene is left");
        assert!(
            w.v.rt.released_total >= released + 4,
            "the four nodes of the scene were made and released"
        );

        // 20 more: no node or context stays behind (the first release made the tombstone node, so count from here).
        let base = w.live_nodes();
        let contexts = w.v.rt.scene.contexts.len();

        for _ in 0..20 {
            w.call("loadDrop", 10);
        }

        assert_eq!(w.live_nodes(), base);
        assert_eq!(
            w.v.rt.scene.contexts.len(),
            contexts,
            "context slots are reused"
        );
    });
}

/// A root node of the scene in a JavaScript variable keeps the whole scene (a node keeps its scene).
#[test]
fn a_held_node_keeps_its_url_scene() {
    big(|| {
        let mut w = World::new("node");

        w.call("loadNode", 10);

        let (c, nodes) = w.loaded();

        w.steps(70);
        assert_eq!(w.nodes_of(c), nodes, "the Script holds Q, which keeps P");

        w.call("drop", 3);
        w.assert_released(c, &nodes);
    });
}

/// createVrmlFromURL: the root nodes go into a Group's children. They stay while the Group holds them, and go with the
/// scene when the Group lets them go.
#[test]
fn createvrmlfromurl_nodes_stay_in_a_field_and_go_when_removed() {
    big(|| {
        let mut w = World::new("vrml");

        w.call("loadVrml", 10);

        let (c, nodes) = w.loaded();

        assert_eq!(w.grp_children().len(), 2, "P and Q are GRP's children");

        w.steps(70);
        assert_eq!(w.nodes_of(c), nodes, "GRP holds the scene");

        w.call("clear", 3);
        w.assert_released(c, &nodes);
    });
}

/// The simple-scene gate: a scene with a PROTO is not a candidate, so it stays as before.
#[test]
fn a_url_scene_with_a_proto_is_not_a_candidate() {
    big(|| {
        let mut w = World::new("proto");

        w.call("loadProto", 10);

        let c = w.scene().expect("the scene is loaded");
        let nodes = w.nodes_of(c);

        assert!(!w.v.rt.dynamic_ctxs.contains(&c));

        w.steps(70);
        assert_eq!(w.nodes_of(c), nodes);
    });
}

/// A released scene's context slot is used again: the new scene does not show the profile that `setProfile` gave the
/// released scene, and a scene that stays keeps its own.
#[test]
fn a_reused_context_slot_does_not_keep_the_released_profile() {
    big(|| {
        let mut w = World::new("profile");

        w.v.rt.record_console = true;
        w.call("loadMark", 10);

        let (c, nodes) = w.loaded();

        w.call("report", 2);
        assert_eq!(
            w.v.rt.console.last().map(String::as_str),
            Some("log: profile: stale"),
            "the held scene keeps its profile: {:?}",
            w.v.rt.log
        );

        w.call("drop", 3);
        w.assert_released(c, &nodes);

        w.call("loadKeep", 10);
        assert_eq!(w.scene(), Some(c), "the new scene uses the released slot");

        w.call("report", 2);
        assert_eq!(
            w.v.rt.console.last().map(String::as_str),
            Some("log: profile: Interchange"),
            "{:?}",
            w.v.rt.log
        );
    });
}
