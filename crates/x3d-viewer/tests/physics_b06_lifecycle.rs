//! REA batch 06R + R0.3-E: the lifetime of RigidBodyPhysics state. A RigidBody removed from `bodies`,
//! a joint removed from `joints`, an Inline unloaded: the Bullet world must not keep the body or the
//! constraint, the interests must go, and a node id that is reused must not inherit anything. An Inline
//! holding a physics world (bodies, three joints, a CollisionSensor) is unloaded and loaded again 100
//! times: the tables stay flat after the warm-up. `MEM` lines at cycles 10, 50 and 100.

use std::path::{Path, PathBuf};
use x3d_fields::{FieldValue, NodeId};
use x3d_runtime::{Cb, Runtime};
use x3d_scene::{CtxId, ROOT};
use x3d_viewer::viewer::{ORACLE_PAGE_ATTRIBUTES, Viewer3d};

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 60.0;
const HEADER: &str = "#X3D V3.3 utf8\nPROFILE Full\nCOMPONENT RigidBodyPhysics:2\n";

fn big(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(256 << 20).spawn(f).unwrap().join().unwrap();
}

/// A floor, a resting box R, a falling sphere A and box B, three joints (Ball, Slider, SingleAxisHinge) and a
/// CollisionSensor. With `extra` false only the floor and R exist.
fn physics(extra: bool) -> String {
    let more = if extra {
        "DEF A RigidBody { mass 1 position 0 0.6 0 geometry DEF A_C CollidableShape { shape Shape { geometry Sphere { radius 0.5 } } } }
         DEF B RigidBody { mass 1 position 4 3 0 geometry DEF B_C CollidableShape { shape Shape { geometry Box { size 1 1 1 } } } }
         DEF C RigidBody { mass 1 position 8 3 0 geometry DEF C_C CollidableShape { shape Shape { geometry Box { size 1 1 1 } } } }
         DEF D RigidBody { mass 1 position 12 3 0 geometry DEF D_C CollidableShape { shape Shape { geometry Box { size 1 1 1 } } } }"
    } else {
        ""
    };
    let joints = if extra {
        "joints [ DEF J1 BallJoint { body1 USE B body2 USE C anchorPoint 6 3 0 forceOutput [ \"ALL\" ] }
                  DEF J2 SliderJoint { body1 USE C body2 USE D axis 1 0 0 minSeparation 0 maxSeparation 1 }
                  DEF J3 SingleAxisHingeJoint { body1 USE B body2 USE D anchorPoint 8 3 0 axis 0 0 1 forceOutput [ \"ALL\" ] } ]"
    } else {
        ""
    };
    let collidables = if extra { "USE F_C USE R_C USE A_C USE B_C USE C_C USE D_C" } else { "USE F_C USE R_C" };

    format!(
        "{HEADER}
DEF RBC RigidBodyCollection {{ gravity 0 -9.8 0
  bodies [ DEF F RigidBody {{ fixed TRUE position 0 -0.5 0 geometry DEF F_C CollidableShape {{ shape Shape {{ geometry Box {{ size 40 1 40 }} }} }} }}
           DEF R RigidBody {{ mass 1 position -4 0.5 0 geometry DEF R_C CollidableShape {{ shape Shape {{ geometry Box {{ size 1 1 1 }} }} }} }}
           {more} ]
  {joints}
  collider DEF CC CollisionCollection {{ collidables [ {collidables} ] }} }}
DEF S CollisionSensor {{ collider USE CC }}
"
    )
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("x3d-physics-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(d: &Path, name: &str, text: &str) -> PathBuf {
    let p = d.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

struct World {
    v: Viewer3d,
    t: f64,
}

impl World {
    fn load(d: &Path, main: &str) -> World {
        let p = write(d, "main.x3dv", main);
        let mut w = World { v: Viewer3d::load_attributes(&p, Some(d), T0, 64, 64, Some(60.0), false, ORACLE_PAGE_ATTRIBUTES).expect("load"), t: T0 };

        w.v.frame_rate = 60.0;
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

    fn named(&self, ctx: CtxId, name: &str) -> NodeId {
        self.v.rt.scene.contexts[ctx].named_nodes.get(name).unwrap_or_else(|| panic!("no {name}"))
    }

    fn set(&mut self, node: NodeId, name: &str, value: FieldValue) {
        let k = self.v.rt.get_field(node, name).unwrap();
        self.v.rt.set_value(k, value);
    }

    fn nodes(&self, field: &str, node: NodeId) -> Vec<NodeId> {
        match self.v.rt.pre_value(node, field) {
            Some(FieldValue::MFNode(l)) => l.iter().flatten().copied().collect(),
            other => panic!("{field}: {other:?}"),
        }
    }

    fn vec3(&self, node: NodeId, field: &str) -> [f64; 3] {
        match self.v.rt.pre_value(node, field) {
            Some(FieldValue::SFVec3f(v)) => [v.x, v.y, v.z],
            other => panic!("{field}: {other:?}"),
        }
    }
}

/// Everything the physics keeps in the runtime.
#[derive(Debug, PartialEq, Eq, Clone)]
struct Resources {
    physics_nodes: usize,
    nursery_bodies: usize,
    nursery_shapes: usize,
    nursery_motion_states: usize,
    update_interests: usize,
    live_watchers: usize,
    live_nodes: usize,
    arena: usize,
    arena_cap: usize,
    nrt: usize,
    frt: usize,
    contexts: usize,
}

fn resources(rt: &mut Runtime) -> Resources {
    let n = rt.physics_nursery_counts();
    let sensor = rt.private_field(rt.browser_node, "sensorEvents").unwrap();
    let update_interests = rt.frt.get(&sensor).map_or(0, |f| f.interests.iter().filter(|i| matches!(i.cb, Cb::Physics(_))).count());
    let s = &rt.scene;

    Resources {
        physics_nodes: rt.physics_state_nodes(),
        nursery_bodies: n.bodies,
        nursery_shapes: n.shapes,
        nursery_motion_states: n.motion_states,
        update_interests,
        live_watchers: rt.live_watchers.len(),
        live_nodes: s.nodes.len() - s.free_nodes.len(),
        arena: s.nodes.len(),
        arena_cap: s.nodes.capacity(),
        nrt: rt.nrt.len(),
        frt: rt.frt.len(),
        contexts: s.contexts.len(),
    }
}

fn rss_kib() -> usize {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("VmRSS:").and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok())))
        .unwrap_or(0)
}

#[test]
fn a_body_removed_from_bodies_leaves_the_bullet_world() {
    big(|| {
        let d = dir("body-removed");
        let mut w = World::load(&d, &physics(true));
        let rbc = w.named(ROOT, "RBC");
        let (a, b, c, dd) = (w.named(ROOT, "A"), w.named(ROOT, "B"), w.named(ROOT, "C"), w.named(ROOT, "D"));
        let all = w.nodes("bodies", rbc);
        let counts = w.v.rt.physics_world_counts(Some(rbc));

        assert_eq!((counts.in_world, counts.constraints), (6, 3), "{counts:?}");

        // A leaves: the sphere sits where it is, nothing moves it any more, and the world has one body less.
        let keep: Vec<Option<NodeId>> = all.iter().filter(|&&n| n != a).map(|&n| Some(n)).collect();

        w.set(rbc, "bodies", FieldValue::MFNode(keep));
        w.steps(2);

        let counts = w.v.rt.physics_world_counts(Some(rbc));
        let frozen = w.vec3(a, "position");

        assert_eq!(counts.in_world, 5, "{counts:?}");
        w.steps(60);
        assert_eq!(w.vec3(a, "position"), frozen, "a removed body is not simulated");
        // The Ammo object of A belongs to its node (not to the dynamics world) until the node goes.
        assert_eq!(w.v.rt.physics_world_counts(Some(rbc)).bodies, 6);
        assert!(w.v.rt.physics_stale_interests().is_empty());

        // B leaves: the two joints that name it (J1 and J3) are gone from the world, J2 stays.
        let keep: Vec<Option<NodeId>> = w.nodes("bodies", rbc).into_iter().filter(|&n| n != b).map(Some).collect();

        w.set(rbc, "bodies", FieldValue::MFNode(keep));
        w.steps(2);

        let counts = w.v.rt.physics_world_counts(Some(rbc));

        assert_eq!((counts.in_world, counts.constraints), (4, 1), "{counts:?}");
        let _ = (c, dd);
        let _ = std::fs::remove_dir_all(&d);
    });
}

/// The body a removed body would have hit is not touched: B and the floor behave bit for bit as in a scene
/// where the sphere never existed.
#[test]
fn a_removed_body_no_longer_affects_the_world() {
    big(|| {
        let scene = |with: bool| {
            format!(
                "{HEADER}
DEF RBC RigidBodyCollection {{ gravity 0 -9.8 0
  bodies [ DEF F RigidBody {{ fixed TRUE position 0 -0.5 0 geometry DEF F_C CollidableShape {{ shape Shape {{ geometry Box {{ size 40 1 40 }} }} }} }}
           DEF B RigidBody {{ mass 1 position 0 0.5 0 geometry DEF B_C CollidableShape {{ shape Shape {{ geometry Box {{ size 1 1 1 }} }} }} }}
           {} ]
  collider DEF CC CollisionCollection {{ collidables [ USE F_C USE B_C {} ] }} }}
",
                if with { "DEF A RigidBody { mass 1 position 0.2 1.4 0 geometry DEF A_C CollidableShape { shape Shape { geometry Sphere { radius 0.5 } } } }" } else { "" },
                if with { "USE A_C" } else { "" }
            )
        };
        let (d1, d2) = (dir("removed-1"), dir("removed-2"));
        let load = |d: &Path, text: &str| {
            let mut w = World { v: Viewer3d::load_attributes(&write(d, "main.x3dv", text), Some(d), T0, 64, 64, Some(60.0), false, ORACLE_PAGE_ATTRIBUTES).unwrap(), t: T0 };

            w.v.frame_rate = 60.0;
            w
        };
        // Nothing has stepped yet: A is removed before the first step, so the two worlds are the same world.
        let mut with = load(&d1, &scene(true));
        let mut without = load(&d2, &scene(false));
        let a = with.named(ROOT, "A");
        let rbc = with.named(ROOT, "RBC");
        let keep: Vec<Option<NodeId>> = with.nodes("bodies", rbc).into_iter().filter(|&n| n != a).map(Some).collect();

        with.set(rbc, "bodies", FieldValue::MFNode(keep));

        // The collidable of A stays in the collision collection, but its body is not in the world any more.
        for _ in 0..120 {
            with.steps(1);
            without.steps(1);

            let (bw, bo) = (with.named(ROOT, "B"), without.named(ROOT, "B"));

            assert_eq!(with.vec3(bw, "position"), without.vec3(bo, "position"));
            assert_eq!(with.v.rt.get_bool(bw, "enabled"), without.v.rt.get_bool(bo, "enabled"));
        }

        let _ = (std::fs::remove_dir_all(&d1), std::fs::remove_dir_all(&d2));
    });
}

#[test]
fn a_joint_removed_from_joints_leaves_the_bullet_world() {
    big(|| {
        let d = dir("joint-removed");
        let mut w = World::load(&d, &physics(true));
        let rbc = w.named(ROOT, "RBC");
        let (j1, j2) = (w.named(ROOT, "J1"), w.named(ROOT, "J2"));

        assert_eq!(w.v.rt.physics_world_counts(Some(rbc)).constraints, 3);

        let keep: Vec<Option<NodeId>> = w.nodes("joints", rbc).into_iter().filter(|&n| n != j1).map(Some).collect();

        w.set(rbc, "joints", FieldValue::MFNode(keep));
        w.steps(2);
        assert_eq!(w.v.rt.physics_world_counts(Some(rbc)).constraints, 2);

        w.set(rbc, "joints", FieldValue::MFNode(vec![]));
        w.steps(2);
        assert_eq!(w.v.rt.physics_world_counts(Some(rbc)).constraints, 0);

        // The bodies still move and X_ITE keeps the update interests of a removed joint: the outputs follow
        // the bodies, but the world holds no constraint.
        w.steps(30);
        assert_eq!(w.v.rt.physics_world_counts(Some(rbc)).constraints, 0);
        let _ = (j1, j2);

        // And they come back with `joints`.
        let all: Vec<Option<NodeId>> = ["J1", "J2", "J3"].iter().map(|n| Some(w.named(ROOT, n))).collect();

        w.set(rbc, "joints", FieldValue::MFNode(all));
        w.steps(2);
        assert_eq!(w.v.rt.physics_world_counts(Some(rbc)).constraints, 3);
        let _ = std::fs::remove_dir_all(&d);
    });
}

/// Bodies and joints are made and removed 100 times: the Bullet world and the runtime stay flat, including
/// the constraint table (`constraint_slots`: a removed constraint's slot is reused).
#[test]
fn repeated_body_and_joint_create_remove_stay_flat() {
    big(|| {
        let d = dir("repeat");
        let mut w = World::load(&d, &physics(true));
        let rbc = w.named(ROOT, "RBC");
        let bodies = w.nodes("bodies", rbc);
        let joints = w.nodes("joints", rbc);
        let few: Vec<Option<NodeId>> = bodies.iter().take(2).map(|&n| Some(n)).collect();
        let (all_b, all_j): (Vec<Option<NodeId>>, Vec<Option<NodeId>>) = (bodies.iter().map(|&n| Some(n)).collect(), joints.iter().map(|&n| Some(n)).collect());
        let mut first = None;
        let mut f_slots = 0;

        for c in 1..=100 {
            w.set(rbc, "joints", FieldValue::MFNode(vec![]));
            w.set(rbc, "bodies", FieldValue::MFNode(few.clone()));
            w.steps(2);

            let counts = w.v.rt.physics_world_counts(Some(rbc));

            assert_eq!((counts.in_world, counts.constraints), (2, 0), "cycle {c}: {counts:?}");

            w.set(rbc, "bodies", FieldValue::MFNode(all_b.clone()));
            w.set(rbc, "joints", FieldValue::MFNode(all_j.clone()));
            w.steps(2);

            let counts = w.v.rt.physics_world_counts(Some(rbc));

            assert_eq!((counts.in_world, counts.constraints), (6, 3), "cycle {c}: {counts:?}");
            assert!(w.v.rt.physics_stale_interests().is_empty());

            if [10, 100].contains(&c) {
                println!("MEM\tphysics-joint-churn\t{c}\tconstraint_slots={}\tbody_slots={}\tshape_slots={}", counts.constraint_slots, counts.body_slots, counts.shape_slots);
            }

            let r = (resources(&mut w.v.rt), counts.bodies, counts.shapes, counts.motion_states, counts.body_slots, counts.shape_slots, counts.motion_state_slots);

            if c == 10 {
                f_slots = counts.constraint_slots;
                first = Some(r.clone());
            }

            if let Some(f) = &first {
                assert_eq!(&r.0, &f.0, "cycle {c}: a runtime table grew");
                assert_eq!((r.1, r.2, r.3, r.4, r.5, r.6, counts.constraint_slots), (f.1, f.2, f.3, f.4, f.5, f.6, f_slots), "cycle {c}: the Bullet arenas grew");
            }
        }

        let _ = std::fs::remove_dir_all(&d);
    });
}

fn start(d: &Path) -> World {
    write(d, "a.x3dv", &physics(true));
    write(d, "b.x3dv", &physics(false));

    World::load(d, &format!("{HEADER}\nDEF Inl Inline {{ url \"a.x3dv\" }}\n"))
}

fn inline_ctx(w: &World) -> CtxId {
    w.v.rt.inline_scenes[&w.named(ROOT, "Inl")]
}

#[test]
fn physics_worlds_come_and_go_with_their_inline_100_cycles() {
    big(|| {
        let d = dir("cycles");
        let mut w = start(&d);
        let mut loaded = None;
        let mut unloaded = None;
        let r = resources(&mut w.v.rt);

        // Six bodies + three joints; the collection's and the sensor's update interests.
        assert_eq!(r.update_interests, 2, "{r:?}");
        assert_eq!(r.physics_nodes, 20, "RBC, 6 bodies, 6 collidables, CollisionCollection, 3 joints, the sensor and its 2 Contacts: {r:?}");

        for c in 1..=100 {
            let inl = w.named(ROOT, "Inl");

            w.set(inl, "load", FieldValue::SFBool(false));
            w.steps(3);

            let r = resources(&mut w.v.rt);

            assert_eq!((r.physics_nodes, r.update_interests, r.nursery_bodies, r.nursery_shapes, r.nursery_motion_states), (0, 0, 0, 0, 0), "cycle {c}: an unloaded scene keeps physics state: {r:?}");
            assert!(w.v.rt.physics_stale_interests().is_empty(), "cycle {c}: stale interests");

            if c == 10 {
                unloaded = Some(r.clone());
            }

            if let Some(b) = &unloaded {
                assert_eq!(&r, b, "cycle {c}: a table grew after the warm-up (unloaded)");
            }

            // The other scene (fewer nodes) on odd cycles, the big one on even cycles.
            w.set(inl, "url", FieldValue::MFString(vec![if c % 2 == 0 { "a.x3dv" } else { "b.x3dv" }.into()]));
            w.set(inl, "load", FieldValue::SFBool(true));
            w.steps(8);

            let ctx = inline_ctx(&w);
            let rbc = w.named(ctx, "RBC");
            let r = resources(&mut w.v.rt);
            let counts = w.v.rt.physics_world_counts(Some(rbc));

            if c % 2 == 0 {
                assert_eq!((counts.in_world, counts.constraints), (6, 3), "cycle {c}: {counts:?}");
                // The sensor is live and the sphere A fell onto the floor: contacts exist.
                let s = w.named(ctx, "S");

                assert!(w.v.rt.get_bool(s, "isActive"), "cycle {c}: the sphere rests on the floor");
                assert_eq!(w.nodes("contacts", s).len(), 2, "cycle {c}: the sphere and the box R rest on the floor");
            } else {
                assert_eq!((counts.in_world, counts.constraints), (2, 0), "cycle {c}: {counts:?}");
            }

            assert_eq!(r.update_interests, 2, "cycle {c}: {r:?}");
            assert!(w.v.rt.physics_stale_interests().is_empty(), "cycle {c}: stale interests");

            if [10, 50, 100].contains(&c) {
                println!(
                    "MEM\tphysics-inline-cycle\t{c}\tphysics_nodes={}\tupdate_interests={}\tlive_watchers={}\tlive_nodes={}\tarena={}\tnrt={}\tfrt={}\tbody_slots={}\tshape_slots={}\tconstraint_slots={}\trss_kib={}",
                    r.physics_nodes,
                    r.update_interests,
                    r.live_watchers,
                    r.live_nodes,
                    r.arena,
                    r.nrt,
                    r.frt,
                    counts.body_slots,
                    counts.shape_slots,
                    counts.constraint_slots,
                    rss_kib()
                );
            }

            if c == 10 || c == 11 {
                loaded = Some((c % 2, r.clone(), counts.clone()));
            }

            if let Some((parity, b, bc)) = &loaded {
                if c % 2 == *parity {
                    assert_eq!(&r, b, "cycle {c}: a table grew after the warm-up (loaded)");
                    assert_eq!((counts.shapes, counts.bodies, counts.body_slots, counts.shape_slots, counts.constraint_slots), (bc.shapes, bc.bodies, bc.body_slots, bc.shape_slots, bc.constraint_slots), "cycle {c}: Bullet arenas grew");
                }
            }
        }

        let _ = std::fs::remove_dir_all(&d);
    });
}

/// Node ids are reused by the next load: nothing of the old nodes is left to be found by the new ones.
#[test]
fn reused_node_ids_inherit_nothing() {
    big(|| {
        let d = dir("reuse");
        let mut w = start(&d);
        let inl = w.named(ROOT, "Inl");
        let ctx = inline_ctx(&w);
        let old: Vec<NodeId> = ["RBC", "F", "A", "B", "J1", "J2", "J3", "S"].iter().map(|n| w.named(ctx, n)).collect();

        w.set(inl, "load", FieldValue::SFBool(false));
        w.steps(3);

        for &n in &old {
            assert!(w.v.rt.physics_stale_interests().is_empty());
            assert_eq!(w.v.rt.physics_body_active(n), None, "node {n} still answers as a RigidBody");
        }

        // Load the smaller scene: its nodes take the ids of the freed ones.
        w.set(inl, "url", FieldValue::MFString(vec!["b.x3dv".into()]));
        w.set(inl, "load", FieldValue::SFBool(true));
        w.steps(8);

        let ctx = inline_ctx(&w);
        let new: Vec<NodeId> = ["RBC", "F", "R", "S"].iter().map(|n| w.named(ctx, n)).collect();
        let reused: Vec<&NodeId> = new.iter().filter(|n| old.contains(n)).collect();

        assert!(!reused.is_empty(), "the test needs ids that are reused: old {old:?} new {new:?}");

        let rbc = new[0];

        // Only the new world's bodies are in the new collection's world, and a reused id has exactly one meaning.
        assert_eq!(w.v.rt.physics_bodies_of(rbc), vec![new[1], new[2]]);
        assert_eq!(w.v.rt.physics_world_counts(Some(rbc)).in_world, 2);
        assert!(w.v.rt.physics_stale_interests().is_empty());

        // R rests on the floor: it was simulated by the new world (a fresh body, not a leftover).
        let r = new[2];

        assert!(w.v.rt.physics_body_active(r).is_some());
        let _ = std::fs::remove_dir_all(&d);
    });
}
