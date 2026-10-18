//! REA batch 06R repair, defect 1: a ParticleSystem that a Script created at runtime and that nothing
//! refers to any more is released, with its simulation state, its `sensorEvents` interest and its watcher
//! (X_ITE: the interests are WeakRefs, so an unreachable node is collected and stops animating).
//!
//! The reachability sweep is `Runtime::release_disposed` ("Runtime-created nodes" in release.rs). A runtime
//! created node stays while a live field (a Group's `children`, a Script's field), a name, a ROUTE of a live
//! context, a pending event, a live JavaScript object (variable, `MFNode`), or another node that stays refers
//! to it. Newly authored worlds; time is virtual. All tests share one process, so they take a lock: the
//! RSS columns would otherwise measure each other.

mod particles_b06_common;

use particles_b06_common::big;
use std::path::PathBuf;
use std::sync::Mutex;
use x3d_fields::{FieldValue, NodeId};
use x3d_runtime::nodes::State;
use x3d_runtime::{Cb, Runtime};
use x3d_scene::ROOT;
use x3d_viewer::viewer::{ORACLE_PAGE_ATTRIBUTES, Viewer3d};

static LOCK: Mutex<()> = Mutex::new(());

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 30.0;
const HEADER: &str = "#X3D V3.3 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:2\n";

/// The Script of the worlds: `mk ()` builds the seven nodes of the lane's cycles case (Shape,
/// ParticleSystem, SurfaceEmitter, Box, ForcePhysicsModel, BoundedPhysicsModel, Box).
const SCRIPT: &str = "
  var held = null;
  function mk () { var S = Browser .currentScene, sh = S .createNode ('Shape'), ps = S .createNode ('ParticleSystem'), em = S .createNode ('SurfaceEmitter'), bx = S .createNode ('Box'), fm = S .createNode ('ForcePhysicsModel'), bm = S .createNode ('BoundedPhysicsModel'), bb = S .createNode ('Box'); ps .maxParticles = 30; ps .particleLifetime = 1; em .speed = 1; bx .size = new SFVec3f (2, 2, 2); em .surface = bx; fm .force = new SFVec3f (0, -9.8, 0); bm .damping = 0.5; bb .size = new SFVec3f (4, 4, 4); bm .geometry = bb; ps .emitter = em; ps .physics = new MFNode (fm, bm); sh .geometry = ps; return sh; }
  function add (v)      { grp .children = new MFNode (mk ()); }
  function addBoth (v)  { var sh = mk (); grp .children = new MFNode (sh); g2 .children = new MFNode (sh); }
  function addHeld (v)  { held = mk (); grp .children = new MFNode (held); }
  function addMF (v)    { held = new MFNode (mk ()); grp .children = held; }
  function addRoute (v) { var sh = mk (); grp .children = new MFNode (sh); Browser .currentScene .addRoute (sh .geometry, 'isActive', ts, 'enabled'); }
  function addVrml (v)  { var nodes = Browser .createVrmlFromString ('Shape { geometry Box { } } Transform { children Shape { geometry Sphere { } } }'); grp .children = new MFNode (nodes [0]); }
  function clear (v)    { grp .children = new MFNode (); }
  function clear2 (v)   { g2 .children = new MFNode (); }
  function drop (v)     { held = null; }
";

const WORLD_FIELDS: &str = "
  inputOnly SFBool add inputOnly SFBool addBoth inputOnly SFBool addHeld inputOnly SFBool addMF inputOnly SFBool addRoute inputOnly SFBool addVrml
  inputOnly SFBool clear inputOnly SFBool clear2 inputOnly SFBool drop
  initializeOnly SFNode grp USE GRP initializeOnly SFNode g2 USE G2 initializeOnly SFNode ts USE TS";

fn world() -> String {
    format!(
        "{HEADER}\nDEF GRP Group {{ }}\nDEF G2 Group {{ }}\nDEF TS TimeSensor {{ enabled TRUE }}\nDEF SC Script {{ directOutput TRUE {WORLD_FIELDS}\n url \"ecmascript:{SCRIPT}\" }}\n"
    )
}

fn rss_kib() -> usize {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("VmRSS:").and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok())))
        .unwrap_or(0)
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct Resources {
    systems: usize,
    buffer_floats: usize,
    animate_interests: usize,
    live_watchers: usize,
    live_nodes: usize,
    arena: usize,
    nrt: usize,
    frt: usize,
    contexts: usize,
    dynamic: usize,
}

fn resources(rt: &Runtime) -> Resources {
    let mut systems = 0;
    let mut floats = 0;

    for (i, n) in rt.nrt.iter().enumerate() {
        if let State::ParticleSystem(_) = n.state {
            systems += 1;
            floats += rt.particle_sim(i as NodeId).map_or(0, |s| s.particles.len() * x3d_runtime::particles::STRIDE);
        }
    }

    let sensor = rt.private_field(rt.browser_node, "sensorEvents").unwrap();
    let s = &rt.scene;

    Resources {
        systems,
        buffer_floats: floats,
        animate_interests: rt.frt.get(&sensor).map_or(0, |f| f.interests.iter().filter(|i| matches!(i.cb, Cb::Particle(_))).count()),
        live_watchers: rt.live_watchers.len(),
        live_nodes: s.nodes.len() - s.free_nodes.len(),
        arena: s.nodes.len(),
        nrt: rt.nrt.len(),
        frt: rt.frt.len(),
        contexts: s.contexts.len(),
        dynamic: rt.dynamic_nodes.len(),
    }
}

/// No interest of a ParticleSystem names a node that holds no ParticleSystem state.
fn assert_no_stale_ids(rt: &Runtime, when: &str) {
    for (k, f) in &rt.frt {
        for i in f.interests.iter().filter(|i| matches!(i.cb, Cb::Particle(_))) {
            assert!(rt.particle_sim(i.node).is_some(), "{when}: an interest of field {k:?} names node {} which holds no ParticleSystem state", i.node);
        }
    }
}

struct World {
    v: Viewer3d,
    t: f64,
    dir: PathBuf,
}

impl World {
    fn new(tag: &str, text: &str) -> World {
        let dir = std::env::temp_dir().join(format!("x3d-unreach-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let p = dir.join("w.x3dv");
        std::fs::write(&p, text).unwrap();

        let mut v = Viewer3d::load_attributes(&p, Some(&dir), T0, 64, 64, Some(1.0 / DT), false, ORACLE_PAGE_ATTRIBUTES).expect("load");

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

    fn named(&self, name: &str) -> NodeId {
        self.v.rt.scene.contexts[ROOT].named_nodes.get(name).unwrap_or_else(|| panic!("no {name}"))
    }

    /// Send `true` to an inputOnly of the Script SC, then run two frames.
    fn call(&mut self, name: &str) {
        let sc = self.named("SC");
        let k = self.v.rt.get_field(sc, name).unwrap_or_else(|| panic!("no field {name}"));

        self.v.rt.set_value(k, FieldValue::SFBool(true));
        self.steps(2);
    }

    fn children(&self, group: &str) -> Vec<NodeId> {
        match self.v.rt.pre_value(self.named(group), "children") {
            Some(FieldValue::MFNode(c)) => c.iter().flatten().copied().collect(),
            o => panic!("children: {o:?}"),
        }
    }

    /// The ParticleSystem of the Shape in a Group.
    fn system_in(&self, group: &str) -> NodeId {
        let shape = self.children(group)[0];

        match self.v.rt.pre_value(shape, "geometry") {
            Some(FieldValue::SFNode(Some(ps))) => *ps,
            o => panic!("geometry: {o:?}"),
        }
    }

    fn live(&self, n: NodeId) -> bool {
        self.v.rt.scene.is_live(n)
    }

    fn animating(&self, n: NodeId) -> bool {
        self.live(n) && self.v.rt.particle_sim(n).is_some()
    }

    fn res(&self) -> Resources {
        resources(&self.v.rt)
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn locked(f: impl FnOnce() + Send + 'static) {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());

    big(f);
}

// --- the defect ----------------------------------------------------------------------------------

/// The lane's `cycles-new`: a new system is put into a Group and the children are cleared, 100 times.
/// Before the repair: +840 floats, +7 nodes, +54 field records and ~32 KiB per cycle.
#[test]
fn unreferenced_runtime_created_systems_are_released_100_cycles() {
    locked(|| {
        let mut w = World::new("cycles", &world());

        // The first release makes the runtime's tombstone node (once): that is the baseline.
        w.call("add");
        w.call("clear");

        let base = w.res();
        let mut at10 = None;
        let mut rss10 = 0;

        for c in 1..=100 {
            w.call("add");

            let live = w.res();

            assert_eq!(live.systems, base.systems + 1, "cycle {c}: the system in the Group runs");
            assert_eq!((live.animate_interests, live.live_watchers), (base.animate_interests + 1, base.live_watchers + 1));

            let ps = w.system_in("GRP");

            w.call("clear");

            assert!(!w.live(ps) && w.v.rt.particle_sim(ps).is_none(), "cycle {c}: the unreferenced system {ps} still lives");
            assert_no_stale_ids(&w.v.rt, "cleared");

            if c == 10 {
                at10 = Some(w.res());
                rss10 = rss_kib();
            }

            if [10, 50, 100].contains(&c) {
                let r = w.res();

                println!(
                    "MEM\tcycles-new\t{c}\tsystems={}\tbuffer_floats={}\tanimate_interests={}\tlive_watchers={}\tlive_nodes={}\tarena={}\tnrt={}\tfrt={}\tcontexts={}\tdynamic={}\trss_kib={}",
                    r.systems, r.buffer_floats, r.animate_interests, r.live_watchers, r.live_nodes, r.arena, r.nrt, r.frt, r.contexts, r.dynamic, rss_kib()
                );
            }

            if let Some(b) = &at10 {
                assert_eq!(&w.res(), b, "cycle {c}: a counter grew after the warm-up");
            }
        }

        let b = at10.unwrap();

        assert_eq!((b.systems, b.buffer_floats, b.animate_interests, b.live_watchers), (base.systems, base.buffer_floats, base.animate_interests, base.live_watchers), "nothing of the removed systems is left");
        assert_eq!(b.live_nodes, base.live_nodes, "the seven nodes of every cycle are gone");

        let grown = rss_kib().saturating_sub(rss10);

        assert!(grown <= 1024, "RSS grew by {grown} KiB over the last 90 cycles (was ~2.9 MiB before the repair)");
    });
}

#[test]
fn a_system_held_by_a_group_stays_and_goes_with_the_children() {
    locked(|| {
        let mut w = World::new("group", &world());

        w.call("add");

        let ps = w.system_in("GRP");

        w.steps(40);
        assert!(w.animating(ps), "referenced by the Group's children");
        assert!(w.v.rt.particle_sim(ps).unwrap().num_particles > 0);

        w.call("clear");
        assert!(!w.animating(ps));
    });
}

// --- what keeps a node ------------------------------------------------------------------------

/// DEF/USE: one node in two Groups. Removed from one it stays; removed from both it goes.
#[test]
fn a_node_used_elsewhere_survives_removal_from_one_parent() {
    locked(|| {
        let mut w = World::new("use", &world());

        w.call("addBoth");

        let ps = w.system_in("GRP");

        assert_eq!(w.system_in("G2"), ps);

        w.call("clear");
        w.steps(10);
        assert!(w.animating(ps), "still a child of the second Group");
        assert_eq!(w.res().systems, 1);

        w.call("clear2");
        assert!(!w.live(ps), "last reference gone");
        assert_eq!(w.res().systems, 0);
        assert_eq!(w.res().animate_interests, 0);
    });
}

/// A ROUTE of a live context keeps both of its ends; deleting it lets the node go.
#[test]
fn a_node_reachable_only_through_a_route_survives_until_the_route_is_deleted() {
    locked(|| {
        let mut w = World::new("route", &world());

        w.call("addRoute");

        let ps = w.system_in("GRP");
        let ts = w.named("TS");

        w.call("clear");
        w.steps(40);
        assert!(w.animating(ps), "the ROUTE from isActive holds the system");
        assert!(w.live(w.children_of_system(ps)), "and what it refers to");

        w.v.rt.delete_route(ROOT, ps, "isActive", ts, "enabled");
        w.steps(3);
        assert!(!w.live(ps), "released after the ROUTE is deleted");
        assert_eq!(w.res().systems, 0);
    });
}

impl World {
    /// The emitter of a system (a node it refers to).
    fn children_of_system(&self, ps: NodeId) -> NodeId {
        match self.v.rt.pre_value(ps, "emitter") {
            Some(FieldValue::SFNode(Some(e))) => *e,
            o => panic!("emitter: {o:?}"),
        }
    }
}

/// A node a Script variable holds stays while the variable holds it, and goes after the variable is
/// cleared and JavaScript has collected it. The host reports the live wrappers (`ScriptHost::held_nodes`).
#[test]
fn a_node_held_by_a_script_variable_survives_until_the_script_drops_it() {
    locked(|| {
        let mut w = World::new("held", &world());

        w.call("addHeld");

        let ps = w.system_in("GRP");

        w.call("clear");
        w.steps(60);
        assert!(w.animating(ps), "the Script variable `held` holds the Shape");

        w.call("drop");
        w.steps(2);
        assert!(!w.live(ps), "released once the variable is null");
        assert_eq!(w.res().systems, 0);
    });
}

/// The same with an `MFNode` value (it holds node ids in the Rust value table, not as wrappers).
#[test]
fn a_node_in_a_script_mfnode_survives_until_the_script_drops_it() {
    locked(|| {
        let mut w = World::new("heldmf", &world());

        w.call("addMF");

        let ps = w.system_in("GRP");

        w.call("clear");
        w.steps(60);
        assert!(w.animating(ps), "the Script variable holds an MFNode of the Shape");

        w.call("drop");
        w.steps(2);
        assert!(!w.live(ps));
    });
}

/// A host that cannot report held nodes is conservative: nothing a Script created is released.
#[test]
fn a_script_host_that_cannot_report_keeps_every_created_node() {
    locked(|| {
        struct Silent(Box<dyn x3d_runtime::script::ScriptHost>);

        impl x3d_runtime::script::ScriptHost for Silent {
            fn create_context(&mut self, rt: &mut Runtime, node: NodeId, source: &str, callbacks: &[String]) -> Result<Vec<bool>, x3d_runtime::script::ScriptError> {
                self.0.create_context(rt, node, source, callbacks)
            }

            fn call(&mut self, rt: &mut Runtime, node: NodeId, call: x3d_runtime::script::ScriptCall) -> Result<(), x3d_runtime::script::ScriptError> {
                self.0.call(rt, node, call)
            }
        }

        let mut w = World::new("silent", &world());
        let real = w.v.rt.script_host.take().unwrap();

        w.v.rt.script_host = Some(Box::new(Silent(real)));
        w.call("add");

        let ps = w.system_in("GRP");

        w.call("clear");
        w.steps(60);
        assert!(w.animating(ps), "unknown Script references: the node stays, as it did before the repair");
    });
}

/// `createVrmlFromString`: the scene is one unit. A node of it in a Group keeps the other root; both go
/// together, and the scene's execution context slot is emptied.
#[test]
fn a_created_scene_goes_as_a_whole() {
    locked(|| {
        let mut w = World::new("vrml", &world());

        w.call("addVrml");
        w.call("clear");

        let base = w.res();

        w.call("addVrml");

        let shape = w.children("GRP")[0];
        let boxn = match w.v.rt.pre_value(shape, "geometry") {
            Some(FieldValue::SFNode(Some(p))) => *p,
            o => panic!("{o:?}"),
        };

        w.steps(20);
        assert!(w.live(shape) && w.live(boxn));
        assert_eq!(w.res().live_nodes, base.live_nodes + 5, "both roots of the created scene exist (Shape, Box, Transform, Shape, Sphere)");

        w.call("clear");
        assert!(!w.live(shape) && !w.live(boxn));
        assert_eq!(w.res().live_nodes, base.live_nodes, "the unused second root went with the scene");
        assert_eq!(w.res().contexts, base.contexts, "{:?}", w.res());

        // 100 more: the context table is flat too.
        let mut first = None;

        for c in 0..100 {
            w.call("addVrml");
            w.call("clear");

            if c == 10 {
                first = Some(w.res());
            }

            if let Some(f) = &first {
                assert_eq!(&w.res(), f, "cycle {c}: a counter grew");
            }
        }
    });
}

// --- NodeId reuse -----------------------------------------------------------------------------

/// A released id is handed out again; the old node's wrapper does not reach the new one, and the next
/// system runs with its own state.
#[test]
fn released_ids_are_reused_safely() {
    locked(|| {
        let mut w = World::new("reuse", &world());

        // Warm-up: the runtime makes its tombstone node on the first release.
        w.call("add");
        w.call("clear");
        w.call("add");

        let _first = w.system_in("GRP");
        let first_nodes = w.res().live_nodes;
        let total = w.v.rt.released_total;

        w.call("clear");

        assert!(w.v.rt.released_total >= total + 7, "the sweep released the seven nodes of the cycle");

        w.call("add");

        let second = w.system_in("GRP");

        assert_eq!(w.res().live_nodes, first_nodes, "the arena is reused, not grown");
        assert!(w.animating(second));
        assert_eq!(w.v.rt.particle_sim(second).unwrap().max_particles, 30);
        assert_no_stale_ids(&w.v.rt, "reused");

        // A node that is held keeps its id: no other node takes it.
        w.call("addHeld");

        let held = w.system_in("GRP");

        w.call("clear");
        w.call("add");
        w.call("clear");
        assert!(w.live(held) && w.animating(held), "id {held} still names the held node");
        assert_eq!(w.v.rt.builtin_name(held), Some("ParticleSystem"));
    });
}

// --- Inline -----------------------------------------------------------------------------------

/// A Script inside an Inline creates systems. Unloading the Inline releases all of them, referenced or
/// not (scene dispose), and the runtime-created bookkeeping goes with them.
#[test]
fn unloading_an_inline_releases_what_its_script_created() {
    locked(|| {
        let d = std::env::temp_dir().join(format!("x3d-unreach-{}-inline", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();

        let inner = format!("{HEADER}\nDEF GRP Group {{ }}\nDEF G2 Group {{ }}\nDEF TS TimeSensor {{ }}\nDEF SC Script {{ directOutput TRUE {WORLD_FIELDS}\n url \"ecmascript:{SCRIPT}\" }}\n");

        std::fs::write(d.join("inner.x3dv"), inner).unwrap();
        std::fs::write(d.join("w.x3dv"), format!("{HEADER}\nDEF Inl Inline {{ url \"inner.x3dv\" }}\n")).unwrap();

        let mut v = Viewer3d::load_attributes(&d.join("w.x3dv"), Some(&d), T0, 64, 64, Some(1.0 / DT), false, ORACLE_PAGE_ATTRIBUTES).expect("load");

        v.frame_rate = 1.0 / DT;

        let mut t = T0;
        let mut step = |v: &mut Viewer3d, n: usize| {
            for _ in 0..n {
                t += DT;
                v.date_now = t;
                v.step(t);
            }
        };

        step(&mut v, 8);

        let inl = v.rt.scene.contexts[ROOT].named_nodes.get("Inl").unwrap();
        let ctx = v.rt.inline_scenes[&inl];
        let sc = v.rt.scene.contexts[ctx].named_nodes.get("SC").unwrap();
        let base = resources(&v.rt);

        for _ in 0..3 {
            let k = v.rt.get_field(sc, "addHeld").unwrap();

            v.rt.set_value(k, FieldValue::SFBool(true));
            step(&mut v, 2);
        }

        assert_eq!(resources(&v.rt).systems, base.systems + 1, "one system in the Inline's Group, the earlier ones are gone");
        assert!(resources(&v.rt).dynamic > 0);

        let k = v.rt.get_field(inl, "load").unwrap();

        v.rt.set_value(k, FieldValue::SFBool(false));
        step(&mut v, 6);

        let r = resources(&v.rt);

        assert_eq!((r.systems, r.animate_interests, r.live_watchers, r.dynamic), (0, 0, 0, 0), "{r:?}");
        assert_no_stale_ids(&v.rt, "unloaded");
        let _ = std::fs::remove_dir_all(&d);
    });
}

/// Event interest and particle buffer of a released system (the sweep, not the scene dispose).
#[test]
fn a_released_system_leaves_no_interest_no_watcher_and_no_buffer() {
    locked(|| {
        let mut w = World::new("clean", &world());

        w.call("add");
        w.call("clear");

        let base = w.res();

        w.call("add");
        w.steps(10);

        let ps = w.system_in("GRP");
        let busy = w.res();

        assert!(busy.buffer_floats > base.buffer_floats && busy.animate_interests > base.animate_interests);

        w.call("clear");

        let sensor = w.v.rt.private_field(w.v.rt.browser_node, "sensorEvents").unwrap();

        assert!(w.v.rt.frt.get(&sensor).unwrap().interests.iter().all(|i| i.node != ps), "interest of the released system");
        assert!(!w.v.rt.live_watchers.iter().any(|(n, _)| *n == ps));
        assert!(w.v.rt.particle_sim(ps).is_none());
        assert_eq!(w.res().buffer_floats, base.buffer_floats);
        w.steps(30);
        assert_eq!(w.res(), base, "and nothing comes back");
    });
}
