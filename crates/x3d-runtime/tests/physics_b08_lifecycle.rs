//! Batch 08 (physics lane): the engine's lifecycle without the node layer. Worlds, shapes, motion states, bodies
//! and constraints are made and destroyed 100 times; the table sizes of the world (`World::stats`, `PWorld::counts`)
//! and the bytes the process holds (a counting global allocator) must be the same at cycle 10, 50 and 100. A scene
//! rebuilt in the same process gives bit-identical results (no state outside the world).
//!
//! The node-level counterpart (RigidBody removed from `bodies`, Inline unload and load, 100 cycles) is
//! x3d-viewer/tests/physics_b06_lifecycle.rs.
//!
//!   cargo test --release -p x3d-runtime --test physics_b08_lifecycle -- --nocapture

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};
use x3d_runtime::nodes::rigid_body_physics::{PWorld, WorldCounts};
use x3d_runtime::physics::bt::body::{BodyId, ConstructionInfo, MotionStateId};
use x3d_runtime::physics::bt::constraint::Constraint;
use x3d_runtime::physics::bt::math::{Tf, V3};
use x3d_runtime::physics::bt::shape::ShapeId;
use x3d_runtime::physics::bt::world::{World, WorldStats};

struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        LIVE.fetch_add(l.size() as isize, Ordering::Relaxed);
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size() as isize, Ordering::Relaxed);
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        LIVE.fetch_add(n as isize - l.size() as isize, Ordering::Relaxed);
        System.realloc(p, l, n)
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// The tests measure the whole process; they run one at a time.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn live() -> isize {
    LIVE.load(Ordering::Relaxed)
}

/// Bytes held at cycles 10, 50 and 100. A Vec that reaches its working size grows by doubling, so cycle 10 may
/// sit under the plateau (measured 64 B to 736 B in the four tests); from cycle 50 on the heap is flat to the byte.
fn assert_flat_heap(at: &[isize]) {
    assert_eq!(at[1], at[2], "heap grew between cycle 50 and 100: {at:?}");
    assert!(
        (at[1] - at[0]).abs() <= 1024,
        "heap grew between cycle 10 and 50: {at:?}"
    );
}

fn tf(o: V3) -> Tf {
    Tf {
        b: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        o,
    }
}

/// Everything one cycle creates, in the order X_ITE creates it.
#[derive(Default)]
struct Cluster {
    bodies: Vec<BodyId>,
    motion_states: Vec<MotionStateId>,
    /// Every shape (collision shapes and compounds), to free again.
    shapes: Vec<ShapeId>,
    /// Constraint ids in the world.
    joints: Vec<usize>,
}

/// A RigidBody the way RigidBody.js makes it: an empty compound, a default motion state, a construction info with
/// mass 0, a btRigidBody; the collidables (a compound with one collision shape each) go below the compound.
fn make_body(
    pw: &mut PWorld,
    c: &mut Cluster,
    shapes: &[Box<dyn Fn(&mut x3d_runtime::physics::bt::shape::Shapes) -> ShapeId>],
    offsets: &[V3],
    pos: V3,
    mass: f64,
) -> BodyId {
    let compound = pw.new_compound();
    c.shapes.push(compound);
    let ms = pw.new_motion_state(Tf::IDENTITY);
    c.motion_states.push(ms);
    let info = ConstructionInfo::new(0.0, Some(ms), compound);
    let rb = pw.new_rigid_body(&info);
    c.bodies.push(rb);

    for (make, off) in shapes.iter().zip(offsets) {
        let cc = pw.new_compound();
        c.shapes.push(cc);
        let prim = pw.new_shape(|s| make(s));
        c.shapes.push(prim);
        pw.w.shapes.add_child_shape(cc, tf(*off), prim);
        pw.w.shapes.set_local_scaling(cc, [1.0; 3]);
        pw.w.shapes.add_child_shape(
            compound,
            Tf {
                b: [[0.0; 3]; 3],
                o: [0.0; 3],
            },
            cc,
        );
    }

    let inv = Tf {
        b: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        o: [-pos[0], -pos[1], -pos[2]],
    };
    for i in 0..pw.w.shapes.num_child_shapes(compound) {
        pw.w.shapes.update_child_transform(compound, i, inv, false);
    }
    pw.w.shapes.recalculate_local_aabb(compound);
    pw.w.set_motion_state_world_transform(ms, tf(pos));
    pw.w.set_motion_state(rb, ms);

    let li = pw.w.shapes.calculate_local_inertia(compound, mass);
    pw.w.body_mut(rb).set_mass_props(mass, li);
    rb
}

type Maker = Box<dyn Fn(&mut x3d_runtime::physics::bt::shape::Shapes) -> ShapeId>;

fn terrain() -> (Vec<f64>, usize, usize) {
    let (xd, zd) = (5, 5);
    (
        (0..xd * zd)
            .map(|i| {
                (0.3 * ((i % xd) as f64 * 0.8).sin() * ((i / xd) as f64 * 0.7).cos() + 0.15) as f32
                    as f64
            })
            .collect(),
        xd,
        zd,
    )
}

/// Floor, sphere, box, cylinder, cone, hull, a two-collidable body, a mesh and a height field; a ball joint, a
/// hinge and a slider.
fn build(pw: &mut PWorld) -> Cluster {
    let mut c = Cluster::default();
    let floor: Vec<Maker> = vec![Box::new(|s| s.new_box([20.0, 0.5, 20.0]))];
    make_body(pw, &mut c, &floor, &[[0.0; 3]], [0.0, -0.5, 0.0], 0.0);
    let sphere: Vec<Maker> = vec![Box::new(|s| s.new_sphere(0.4))];
    let a = make_body(pw, &mut c, &sphere, &[[0.0; 3]], [0.0, 1.5, 0.0], 1.0);
    let boxes: Vec<Maker> = vec![Box::new(|s| s.new_box([0.4, 0.3, 0.3]))];
    let b = make_body(pw, &mut c, &boxes, &[[0.0; 3]], [1.5, 1.5, 0.0], 1.0);
    let cyl: Vec<Maker> = vec![Box::new(|s| s.new_cylinder([0.3, 0.5, 0.3]))];
    let d = make_body(pw, &mut c, &cyl, &[[0.0; 3]], [3.0, 1.5, 0.0], 1.0);
    let cone: Vec<Maker> = vec![Box::new(|s| s.new_cone(0.4, 0.9))];
    let e = make_body(pw, &mut c, &cone, &[[0.0; 3]], [4.2, 1.5, 0.0], 1.0);
    let hull: Vec<Maker> = vec![Box::new(|s| {
        s.new_convex_hull(&[
            [-0.4, -0.4, -0.4],
            [0.4, -0.4, -0.4],
            [0.0, 0.4, -0.4],
            [0.0, 0.0, 0.5],
            [0.3, 0.2, 0.1],
        ])
    })];
    let h = make_body(pw, &mut c, &hull, &[[0.0; 3]], [5.5, 1.5, 0.0], 1.0);
    let two: Vec<Maker> = vec![
        Box::new(|s| s.new_box([0.3, 0.2, 0.3])),
        Box::new(|s| s.new_sphere(0.25)),
    ];
    make_body(
        pw,
        &mut c,
        &two,
        &[[0.0; 3], [0.6, 0.2, 0.0]],
        [7.0, 1.5, 0.0],
        2.0,
    );
    let tris: Vec<[V3; 3]> = (0..8)
        .flat_map(|i| (0..8).map(move |j| (i, j)))
        .flat_map(|(i, j)| {
            let (x0, z0) = (i as f64 * 1.0 - 4.0, j as f64 * 1.0 - 4.0 + 10.0);
            let p = |x: f64, z: f64| {
                [
                    x as f32 as f64,
                    (0.1 * (x * 0.9).sin()) as f32 as f64,
                    z as f32 as f64,
                ]
            };
            [
                [p(x0, z0), p(x0, z0 + 1.0), p(x0 + 1.0, z0)],
                [p(x0 + 1.0, z0), p(x0, z0 + 1.0), p(x0 + 1.0, z0 + 1.0)],
            ]
        })
        .collect();
    let mesh: Vec<Maker> = vec![Box::new(move |s| s.new_bvh_triangle_mesh(&tris))];
    make_body(pw, &mut c, &mesh, &[[0.0; 3]], [0.0, 0.0, 0.0], 0.0);
    let (hs, xd, zd) = terrain();
    let (min, max) = hs
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), h| {
            (lo.min(*h), hi.max(*h))
        });
    let height: Vec<Maker> = vec![Box::new(move |s| {
        s.new_heightfield(xd, zd, &hs, min, max, [1.2, 1.0, 0.8])
    })];
    make_body(
        pw,
        &mut c,
        &height,
        &[[2.4, (min + max) * 0.5, 1.6]],
        [10.0, 0.0, 10.0],
        0.0,
    );

    for &rb in &c.bodies {
        pw.w.body_mut(rb).friction = 0.5f32 as f64;
    }
    for &rb in &c.bodies {
        pw.w.add_rigid_body(rb);
    }

    let ball = Constraint::point2point(a, b, [0.75, 0.0, 0.0], [-0.75, 0.0, 0.0]);
    c.joints.push(pw.w.add_constraint(ball, true));
    let hinge = Constraint::hinge(
        &pw.w.bodies,
        d,
        e,
        [0.6, 0.0, 0.0],
        [-0.6, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        false,
    );
    c.joints.push(pw.w.add_constraint(hinge, true));
    let slider = Constraint::slider(
        &pw.w.bodies,
        e,
        h,
        tf([4.8, 1.5, 0.0]),
        tf([4.8, 1.5, 0.0]),
        true,
    );
    c.joints.push(pw.w.add_constraint(slider, true));
    c
}

fn teardown(pw: &mut PWorld, c: Cluster) {
    for j in c.joints.into_iter().rev() {
        pw.w.remove_constraint(j);
    }
    for &rb in &c.bodies {
        pw.w.remove_rigid_body(rb);
    }
    for rb in c.bodies {
        pw.free_body(rb);
    }
    for ms in c.motion_states {
        pw.free_motion_state(ms);
    }
    for s in c.shapes {
        pw.free_shape(s);
    }
}

fn run(w: &mut World, steps: usize) {
    for _ in 0..steps {
        w.step_simulation(1.0 / 60.0, 12, 1.0 / 600.0);
    }
}

/// The sizes that must not grow. `pairs` and `manifolds` follow the physical state of the bodies (which AABBs
/// overlap, which shapes touch) and are checked where the state is known (zero after a teardown).
fn structure(s: WorldStats) -> WorldStats {
    WorldStats {
        pairs: 0,
        manifolds: 0,
        ..s
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    stats: WorldStats,
    counts: WorldCounts,
}

fn snapshot(pw: &PWorld) -> Snapshot {
    Snapshot {
        stats: structure(pw.w.stats()),
        counts: pw.counts(),
    }
}

/// Bodies, shapes and constraints come and go in one world 100 times, as X_ITE does when `bodies` or `joints`
/// change or a CollidableShape is rebuilt: the tables and the heap stay flat.
#[test]
fn in_world_churn_100_cycles() {
    let _g = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let mut pw = PWorld::new();
    pw.w.set_gravity([0.0, -9.8, 0.0]);
    let mut at: Vec<(usize, Snapshot, isize)> = vec![];

    for cycle in 1..=100 {
        let c = build(&mut pw);
        run(&mut pw.w, 20);
        let during = pw.w.stats();
        assert!(
            during.manifolds > 0,
            "cycle {cycle}: the scene made no contact"
        );
        assert_eq!(during.collision_objects, 9);
        assert_eq!(during.constraints, 3);
        teardown(&mut pw, c);

        let after = pw.w.stats();
        assert_eq!(
            (
                after.collision_objects,
                after.non_static,
                after.constraints,
                after.proxies,
                after.pairs,
                after.manifolds
            ),
            (0, 0, 0, 0, 0, 0),
            "cycle {cycle}: the world kept something: {after:?}"
        );
        assert_eq!(
            pw.counts().shapes + pw.counts().bodies + pw.counts().motion_states,
            0,
            "cycle {cycle}: arena slots in use"
        );

        if matches!(cycle, 10 | 50 | 100) {
            at.push((cycle, snapshot(&pw), live()));
        }
    }

    for (cycle, s, bytes) in &at {
        println!(
            "LIFECYCLE in_world cycle {cycle}: {:?} {:?} heap {bytes} B",
            s.stats, s.counts
        );
    }

    assert_eq!(at[0].1, at[1].1, "tables grew between cycle 10 and 50");
    assert_eq!(at[1].1, at[2].1, "tables grew between cycle 50 and 100");
    assert_flat_heap(&at.iter().map(|a| a.2).collect::<Vec<_>>());
}

/// A whole world made, stepped and dropped 100 times: nothing stays in the process.
#[test]
fn world_churn_100_cycles() {
    let _g = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let mut at = vec![];

    for cycle in 1..=100 {
        {
            let mut pw = PWorld::new();
            pw.w.set_gravity([0.0, -9.8, 0.0]);
            let _c = build(&mut pw);
            run(&mut pw.w, 20);
        }

        if matches!(cycle, 10 | 50 | 100) {
            at.push((cycle, live()));
        }
    }

    for (cycle, bytes) in &at {
        println!("LIFECYCLE world cycle {cycle}: heap {bytes} B");
    }

    assert_flat_heap(&at.iter().map(|a| a.1).collect::<Vec<_>>());
}

/// What a body does when it leaves the world while it has contacts and a joint, and joins again: the pair cache,
/// the manifolds and the proxies go with it.
#[test]
fn body_removed_with_contacts_and_joint_100_cycles() {
    let _g = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let mut pw = PWorld::new();
    pw.w.set_gravity([0.0, -9.8, 0.0]);
    let c = build(&mut pw);
    run(&mut pw.w, 60);
    let mut at = vec![];

    for cycle in 1..=100 {
        // set_dynamicsWorld__: remove every body, add the enabled ones again.
        for &rb in &c.bodies {
            pw.w.remove_rigid_body(rb);
        }
        let mid = pw.w.stats();
        assert_eq!(
            (mid.proxies, mid.pairs, mid.manifolds),
            (0, 0, 0),
            "cycle {cycle}: {mid:?}"
        );
        for &rb in &c.bodies {
            pw.w.add_rigid_body(rb);
        }
        run(&mut pw.w, 5);

        if matches!(cycle, 10 | 50 | 100) {
            at.push((cycle, structure(pw.w.stats()), live()));
        }
    }

    for (cycle, s, bytes) in &at {
        println!("LIFECYCLE remove_add cycle {cycle}: {s:?} heap {bytes} B");
    }

    assert_eq!(at[0].1, at[1].1, "tables grew between cycle 10 and 50");
    assert_eq!(at[1].1, at[2].1, "tables grew between cycle 50 and 100");
    assert_flat_heap(&at.iter().map(|a| a.2).collect::<Vec<_>>());
}

/// A constraint removed and made again 100 times in a live world reuses its slot.
#[test]
fn constraint_churn_100_cycles() {
    let _g = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let mut pw = PWorld::new();
    pw.w.set_gravity([0.0, -9.8, 0.0]);
    let c = build(&mut pw);
    run(&mut pw.w, 10);
    let (a, b) = (c.bodies[1], c.bodies[2]);
    let mut at = vec![];
    let mut id = c.joints[0];

    for cycle in 1..=100 {
        pw.w.remove_constraint(id);
        run(&mut pw.w, 2);
        let ball = Constraint::point2point(a, b, [0.75, 0.0, 0.0], [-0.75, 0.0, 0.0]);
        id = pw.w.add_constraint(ball, true);
        run(&mut pw.w, 2);

        if matches!(cycle, 10 | 50 | 100) {
            at.push((cycle, structure(pw.w.stats()), live()));
        }
    }

    for (cycle, s, bytes) in &at {
        println!("LIFECYCLE constraint cycle {cycle}: {s:?} heap {bytes} B");
    }

    assert_eq!(at[0].1, at[1].1);
    assert_eq!(at[1].1, at[2].1);
    assert_eq!(at[0].1.constraints, 3);
    assert_flat_heap(&at.iter().map(|a| a.2).collect::<Vec<_>>());
}

fn trace(w: &mut World, bodies: &[BodyId], steps: usize) -> Vec<u64> {
    let mut out = vec![];
    for _ in 0..steps {
        w.step_simulation(1.0 / 60.0, 12, 1.0 / 600.0);
        for &b in bodies {
            let body = w.body(b);
            out.extend(
                body.world_tf
                    .o
                    .iter()
                    .chain(body.lin_vel.iter())
                    .chain(body.ang_vel.iter())
                    .map(|v| v.to_bits()),
            );
        }
        out.push(w.num_manifolds() as u64);
    }
    out
}

/// A scene built again in the same process (a reload), also while another world exists and is stepped between,
/// gives the same bits: the engine keeps no state outside the world.
#[test]
fn reload_is_bit_identical() {
    let _g = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let mut first: Option<Vec<u64>> = None;
    let mut other = PWorld::new();
    other.w.set_gravity([0.0, -3.0, 0.0]);
    let oc = build(&mut other);

    for reload in 0..5 {
        let mut pw = PWorld::new();
        pw.w.set_gravity([0.0, -9.8, 0.0]);
        let c = build(&mut pw);
        // another world is stepped in between, with different gravity
        run(&mut other.w, 7);
        let t = trace(&mut pw.w, &c.bodies, 120);
        run(&mut other.w, 3);
        match &first {
            None => first = Some(t),
            Some(f) => assert_eq!(f, &t, "reload {reload} differs from the first load"),
        }
    }

    drop(oc);
    assert!(first.unwrap().len() > 1000);
}
