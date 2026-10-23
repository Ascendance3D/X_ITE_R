//! Replays the scenes of `oracle/tools/ammo_dyn_vectors.mjs` (Ammo, Bullet 2.82, as X_ITE builds
//! and steps them) through `x3d_runtime::physics::bt` and prints the maximum absolute error of every
//! recorded quantity per scene. The target, and the assertion, is zero.

use std::collections::BTreeMap;
use x3d_runtime::physics::bt::body::{BodyId, ConstructionInfo};
use x3d_runtime::physics::bt::constraint::Constraint;
use x3d_runtime::physics::bt::math::*;
use x3d_runtime::physics::bt::shape::ShapeId;
use x3d_runtime::physics::bt::world::World;

// --- a small JSON reader (no external crates) ------------------------------------------------

#[derive(Debug, Clone)]
enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(BTreeMap<String, J>),
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && (self.s[self.i] as char).is_whitespace() {
            self.i += 1;
        }
    }

    fn val(&mut self) -> J {
        self.ws();
        match self.s[self.i] {
            b'{' => {
                self.i += 1;
                let mut m = BTreeMap::new();
                loop {
                    self.ws();
                    if self.s[self.i] == b'}' {
                        self.i += 1;
                        break;
                    }
                    if self.s[self.i] == b',' {
                        self.i += 1;
                        continue;
                    }
                    let J::Str(k) = self.val() else { panic!("key") };
                    self.ws();
                    assert_eq!(self.s[self.i], b':');
                    self.i += 1;
                    m.insert(k, self.val());
                }
                J::Obj(m)
            }
            b'[' => {
                self.i += 1;
                let mut v = vec![];
                loop {
                    self.ws();
                    if self.s[self.i] == b']' {
                        self.i += 1;
                        break;
                    }
                    if self.s[self.i] == b',' {
                        self.i += 1;
                        continue;
                    }
                    v.push(self.val());
                }
                J::Arr(v)
            }
            b'"' => {
                self.i += 1;
                let st = self.i;
                while self.s[self.i] != b'"' {
                    if self.s[self.i] == b'\\' {
                        self.i += 1;
                    }
                    self.i += 1;
                }
                let t = String::from_utf8_lossy(&self.s[st..self.i]).into_owned();
                self.i += 1;
                J::Str(t)
            }
            b't' => {
                self.i += 4;
                J::Bool(true)
            }
            b'f' => {
                self.i += 5;
                J::Bool(false)
            }
            b'n' => {
                self.i += 4;
                J::Null
            }
            _ => {
                let st = self.i;
                while self.i < self.s.len()
                    && matches!(
                        self.s[self.i],
                        b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                    )
                {
                    self.i += 1;
                }
                J::Num(
                    std::str::from_utf8(&self.s[st..self.i])
                        .unwrap()
                        .parse()
                        .unwrap(),
                )
            }
        }
    }
}

impl J {
    fn get(&self, k: &str) -> &J {
        match self {
            J::Obj(m) => m.get(k).unwrap_or(&J::Null),
            _ => &J::Null,
        }
    }
    fn f(&self) -> f64 {
        match self {
            J::Num(n) => *n,
            _ => panic!("number expected, got {self:?}"),
        }
    }
    fn b(&self) -> bool {
        matches!(self, J::Bool(true))
    }
    fn s(&self) -> &str {
        match self {
            J::Str(s) => s,
            _ => "",
        }
    }
    fn a(&self) -> &[J] {
        match self {
            J::Arr(a) => a,
            _ => &[],
        }
    }
    fn v(&self) -> Vec<f64> {
        self.a().iter().map(|x| x.f()).collect()
    }
    fn v3(&self) -> V3 {
        let v = self.v();
        [v[0], v[1], v[2]]
    }
    fn is_null(&self) -> bool {
        matches!(self, J::Null)
    }
}

// --- building the scene the way X_ITE does ---------------------------------------------------

/// RigidBody.js / CollidableShape.js toTransform: a column-major Matrix4 to a btTransform.
fn to_transform(m: &[f64]) -> Tf {
    rt(Tf {
        b: [[m[0], m[4], m[8]], [m[1], m[5], m[9]], [m[2], m[6], m[10]]],
        o: [m[12], m[13], m[14]],
    })
}

fn make_primitive(w: &mut World, s: &J) -> ShapeId {
    match s.get("type").s() {
        "sphere" => w.shapes.new_sphere(s.get("radius").f()),
        "box" => w.shapes.new_box(s.get("half").v3()),
        "cylinder" => w.shapes.new_cylinder([
            s.get("radius").f(),
            s.get("half_height").f(),
            s.get("radius").f(),
        ]),
        "cone" => w.shapes.new_cone(s.get("radius").f(), s.get("height").f()),
        "capsule" => w
            .shapes
            .new_capsule(s.get("radius").f(), s.get("height").f()),
        "hull" => {
            let pts: Vec<V3> = s.get("points").a().iter().map(|p| p.v3()).collect();
            w.shapes.new_convex_hull(&pts)
        }
        "mesh" => {
            let tris: Vec<[V3; 3]> = s
                .get("triangles")
                .a()
                .iter()
                .map(|t| [t.a()[0].v3(), t.a()[1].v3(), t.a()[2].v3()])
                .collect();
            w.shapes.new_bvh_triangle_mesh(&tris)
        }
        t => panic!("shape {t}"),
    }
}

struct Built {
    rb: BodyId,
    ms: usize,
    compound: ShapeId,
    /// The collidable compounds (one per shape of the body) below `compound`.
    ccs: Vec<ShapeId>,
    spec: J,
}

fn set_transform(w: &mut World, b: &Built) {
    set_transform_with(w, b, &b.spec.get("m").v(), &b.spec.get("im").v());
}

fn set_transform_with(w: &mut World, b: &Built, m: &[f64], im: &[f64]) {
    let (m, im) = (m.to_vec(), im.to_vec());
    let (t, it) = (to_transform(&m), to_transform(&im));
    let n = w.shapes.num_child_shapes(b.compound);
    for i in 0..n {
        w.shapes.update_child_transform(b.compound, i, it, false);
    }
    w.shapes.recalculate_local_aabb(b.compound);
    w.set_motion_state_world_transform(b.ms, t);
    w.set_motion_state(b.rb, b.ms);
}

/// What X_ITE does after every RigidBodyCollection.update (): RigidBody.update () writes position,
/// orientation and the velocities from the motion state, and the events they cause run in this order:
/// every body's set_linearVelocity__ / set_angularVelocity__, then the collidables' eventsProcessed
/// (the child transform below the collidable compound), then every body's set_transform__.
fn feedback(w: &mut World, built: &[Built]) {
    use x3d_fields::math::{Matrix4, Quaternion, Rotation4, Vector3};

    let next: Vec<(Matrix4, Matrix4, V3, V3)> = built
        .iter()
        .map(|b| {
            let tf = w.motion_state_world_transform(b.ms);
            let q = rq(tf.rotation());
            let mut quaternion = Quaternion::IDENTITY;
            quaternion.set(q[0], q[1], q[2], q[3]);
            let mut orientation = Rotation4::identity();
            orientation.set_quaternion(&quaternion);
            let mut m = Matrix4::IDENTITY;
            m.set_transform(
                Some(&Vector3::new(tf.o[0], tf.o[1], tf.o[2])),
                Some(&orientation),
                None,
                None,
                None,
            );
            let mut im = m;
            im.inverse();
            let body = w.body(b.rb);
            (m, im, body.lin_vel, body.ang_vel)
        })
        .collect();

    for (b, n) in built.iter().zip(&next) {
        let fixed = b.spec.get("fixed").b();
        let body = w.body_mut(b.rb);
        body.set_linear_velocity(if fixed { ZERO } else { n.2 });
        body.activate(false);
        body.set_angular_velocity(if fixed { ZERO } else { n.3 });
        body.activate(false);
    }

    for (b, n) in built.iter().zip(&next) {
        for (k, s) in b.spec.get("shapes").a().iter().enumerate() {
            let mut lm = n.0;
            if !s.get("offset").is_null() {
                let o = s.get("offset").v3();
                lm.translate(&Vector3::new(o[0], o[1], o[2]));
            }
            w.shapes
                .update_child_transform(b.ccs[k], 0, to_transform(&lm.0), true);
        }
    }

    for (b, n) in built.iter().zip(&next) {
        set_transform_with(w, b, &n.0.0, &n.1.0);
    }
}

#[derive(Default, Clone)]
struct Errors(BTreeMap<&'static str, f64>);

impl Errors {
    fn upd(&mut self, k: &'static str, a: &[f64], b: &[f64]) {
        let e = self.0.entry(k).or_insert(0.0);
        for (x, y) in a.iter().zip(b) {
            let d = (x - y).abs();
            if d.is_nan() || d > *e {
                *e = if d.is_nan() { f64::INFINITY } else { d };
            }
        }
    }
    fn count(&mut self, k: &'static str, bad: bool) {
        let e = self.0.entry(k).or_insert(0.0);
        if bad {
            *e += 1.0;
        }
    }
}

fn load(name: &str) -> J {
    let path = format!(
        "{}/tests/data/bullet_dyn/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    P { s: &text, i: 0 }.val()
}

fn make_ccs(w: &mut World, b: &J, xflow: bool) -> Vec<ShapeId> {
    b.get("shapes")
        .a()
        .iter()
        .map(|s| {
            let cc = w.shapes.new_compound();
            let prim = make_primitive(w, s);
            if !s.get("margin").is_null() {
                w.shapes.set_margin(prim, s.get("margin").f());
            }
            if xflow {
                w.shapes.set_local_scaling(cc, [1.0; 3]);
            }
            w.shapes.add_child_shape(
                cc,
                if xflow {
                    Tf::IDENTITY
                } else {
                    to_transform(&s.get("local").v())
                },
                prim,
            );
            w.shapes.set_local_scaling(cc, [1.0; 3]);
            cc
        })
        .collect()
}

/// The velocities, damping, centre of mass, mass properties and sleeping thresholds of a RigidBody
/// (RigidBody.initialize and set_compoundShape__); returns the local inertia.
fn init_body(w: &mut World, bt: &Built) -> V3 {
    let b = &bt.spec;
    let fixed = b.get("fixed").b();
    let body = w.body_mut(bt.rb);
    body.set_linear_velocity(if fixed {
        ZERO
    } else {
        b.get("linear_velocity").v3()
    });
    body.activate(false);
    body.set_angular_velocity(if fixed {
        ZERO
    } else {
        b.get("angular_velocity").v3()
    });
    body.activate(false);
    body.set_angular_factor(if b.get("use_finite_rotation").b() {
        b.get("finite_rotation_axis").v3()
    } else {
        [1.0; 3]
    });

    if b.get("auto_damp").b() {
        body.set_damping(b.get("linear_damping").f(), b.get("angular_damping").f());
    } else {
        body.set_damping(0.0, 0.0);
    }
    body.activate(false);
    body.set_center_of_mass_transform(Tf {
        b: IDENTITY3,
        o: b.get("center_of_mass").v3(),
    });

    let mass = if fixed { 0.0 } else { b.get("mass").f() };
    let li = w.shapes.calculate_local_inertia(bt.compound, mass);
    let body = w.body_mut(bt.rb);
    body.set_mass_props(mass, li);

    if b.get("auto_disable").b() {
        body.lin_sleeping_threshold = f(b.get("disable_linear_speed").f());
        body.ang_sleeping_threshold = f(b.get("disable_angular_speed").f());
    } else {
        body.lin_sleeping_threshold = 0.0;
        body.ang_sleeping_threshold = 0.0;
    }

    li
}

fn run_scene(name: &str) -> Errors {
    let root = load(name);
    run_one(
        root.get("scene"),
        root.get("steps").a(),
        &mut Errors::default(),
    )
}

fn run_one(spec: &J, steps: &[J], errors: &mut Errors) -> Errors {
    let mut w = World::new();
    let mut built: Vec<Built> = vec![];

    let xflow = spec.get("xflow").b();
    let zero = Tf {
        b: [[0.0; 3]; 3],
        o: ZERO,
    };

    // X_ITE makes every CollidableShape before the RigidBodies that hold them.
    let pre: Vec<Vec<ShapeId>> = if xflow {
        spec.get("bodies")
            .a()
            .iter()
            .map(|b| make_ccs(&mut w, b, true))
            .collect()
    } else {
        vec![]
    };

    for (bi, b) in spec.get("bodies").a().iter().enumerate() {
        let compound = w.shapes.new_compound();
        let ms = w.create_motion_state(Tf {
            b: [[0.0; 3]; 3],
            o: ZERO,
        });
        // new btDefaultMotionState (): identity
        w.set_motion_state_world_transform(ms, Tf::IDENTITY);
        let info = ConstructionInfo::new(0.0, Some(ms), compound);
        let rb = w.create_rigid_body(&info);
        let mut bt = Built {
            rb,
            ms,
            compound,
            ccs: if xflow {
                pre[bi].clone()
            } else {
                make_ccs(&mut w, b, false)
            },
            spec: b.clone(),
        };

        // RigidBody.set_compoundShape__: the transform is replaced by updateChildTransform
        for cc in bt.ccs.clone() {
            w.shapes.add_child_shape(compound, zero, cc);
        }

        set_transform(&mut w, &bt);
        init_body(&mut w, &bt);

        if !xflow {
            set_transform(&mut w, &bt);
        }

        built.push(bt);
    }

    // RigidBodyCollection
    let gravity = spec.get("gravity").v3();
    w.set_gravity(gravity);
    let collider = spec.get("collider");

    for b in &built {
        let (fr, rfr) = if collider.is_null() {
            (0.5, 0.0)
        } else {
            (
                collider.get("friction").v()[0],
                collider.get("friction").v()[1],
            )
        };
        let body = w.body_mut(b.rb);
        body.friction = f(fr);
        body.rolling_friction = f(rfr);
        let shape = body.shape;
        w.shapes
            .set_margin(shape, spec.get("contact_surface_thickness").f());
    }

    for b in &built {
        if !matches!(b.spec.get("enabled"), J::Bool(false)) {
            w.add_rigid_body(b.rb);
        }
    }

    // X_ITE's load, as the call log of the real run shows it: see ammo_dyn_vectors.mjs.
    if xflow {
        let rebuilds = |s: &J| {
            s.get("rebuild").b() || matches!(s.get("type").s(), "sphere" | "cone" | "cylinder")
        };
        let local_of = |b: &Built, k: usize| -> Vec<f64> {
            let mut m = x3d_fields::math::Matrix4::IDENTITY;
            m.0.copy_from_slice(&b.spec.get("m").v());
            let s = &b.spec.get("shapes").a()[k];
            if !s.get("offset").is_null() {
                let o = s.get("offset").v3();
                m.translate(&x3d_fields::math::Vector3::new(o[0], o[1], o[2]));
            }
            m.0.to_vec()
        };
        let remake = |w: &mut World, bt: &Built| {
            w.shapes.remove_child_shape_by_index(bt.compound, 0);
            for cc in &bt.ccs {
                w.shapes.add_child_shape(bt.compound, zero, *cc);
            }
            set_transform(w, bt);
            init_body(w, bt);
        };

        for bt in &built {
            remake(&mut w, bt);
        }
        for bt in &built {
            for k in 0..bt.ccs.len() {
                w.shapes
                    .update_child_transform(bt.ccs[k], 0, to_transform(&local_of(bt, k)), true);
            }
        }
        for bt in &built {
            set_transform(&mut w, bt);
        }

        for bt in &built {
            let shapes = bt.spec.get("shapes").a();

            if !shapes.iter().any(rebuilds) {
                continue;
            }

            for (k, s) in shapes.iter().enumerate() {
                if !rebuilds(s) {
                    continue;
                }
                let cc = bt.ccs[k];
                w.shapes.remove_child_shape_by_index(cc, 0);
                w.shapes.set_local_scaling(cc, [1.0; 3]);
                let prim = make_primitive(&mut w, s);
                if !s.get("margin").is_null() {
                    w.shapes.set_margin(prim, s.get("margin").f());
                }
                w.shapes
                    .add_child_shape(cc, to_transform(&s.get("local").v()), prim);
                w.shapes.set_local_scaling(cc, [1.0; 3]);
            }

            remake(&mut w, bt);
            for k in 0..bt.ccs.len() {
                w.shapes
                    .update_child_transform(bt.ccs[k], 0, to_transform(&local_of(bt, k)), true);
            }
            set_transform(&mut w, bt);
        }
    }

    for (i, bt) in built.iter().enumerate() {
        // the inertia of the last init_body
        let fixed = bt.spec.get("fixed").b();
        let mass = if fixed { 0.0 } else { bt.spec.get("mass").f() };
        let li = w.shapes.calculate_local_inertia(bt.compound, mass);
        errors.upd(
            "inertia",
            &li,
            &steps[0].get("bodies").a()[i].get("inertia").v(),
        );
    }

    if !spec.get("joints").is_null() {
        for j in spec.get("joints").a() {
            let find = |n: &str| {
                built
                    .iter()
                    .find(|b| b.spec.get("name").s() == n)
                    .unwrap()
                    .rb
            };
            let (b1, b2) = (find(j.get("body1").s()), find(j.get("body2").s()));
            let mut c = match j.get("type").s() {
                "ball" => {
                    let mut c = Constraint::point2point(b1, b2, ZERO, ZERO);
                    c.set_pivot_a(j.get("pivot_a").v3());
                    c.set_pivot_b(j.get("pivot_b").v3());
                    c
                }
                "hinge" => Constraint::hinge(
                    &w.bodies,
                    b1,
                    b2,
                    j.get("pivot_a").v3(),
                    j.get("pivot_b").v3(),
                    j.get("axis_a").v3(),
                    j.get("axis_b").v3(),
                    false,
                ),
                "slider" => {
                    let mut c = Constraint::slider(
                        &w.bodies,
                        b1,
                        b2,
                        to_transform(&j.get("frame_a").v()),
                        to_transform(&j.get("frame_b").v()),
                        true,
                    );
                    c.set_lower_ang_limit(0.0);
                    c.set_upper_ang_limit(0.0);
                    c.set_lower_lin_limit(j.get("lower").f());
                    c.set_upper_lin_limit(j.get("upper").f());
                    c
                }
                t => panic!("joint {t}"),
            };
            c.enabled = true;
            w.add_constraint(c, true);
        }
    }

    let iterations = spec.get("iterations").f();
    let prefer_accuracy = spec.get("prefer_accuracy").b();
    let compare = |w: &World, built: &[Built], step: &J, errors: &mut Errors, step_no: usize| {
        let mut debug_hit = false;
        for (i, b) in built.iter().enumerate() {
            if std::env::var("BT_DEBUG").is_ok() {
                let sb = &step.get("bodies").a()[i];
                let t = w.motion_state_world_transform(b.ms);
                let body = w.body(b.rb);
                let pairs: [(&str, Vec<f64>, Vec<f64>); 7] = [
                    ("pos", t.o.to_vec(), sb.get("pos").v()),
                    ("lin", body.lin_vel.to_vec(), sb.get("lin").v()),
                    ("ang", body.ang_vel.to_vec(), sb.get("ang").v()),
                    ("wpos", body.world_tf.o.to_vec(), sb.get("wpos").v()),
                    (
                        "wbasis",
                        body.world_tf
                            .b
                            .iter()
                            .flatten()
                            .copied()
                            .collect::<Vec<f64>>(),
                        sb.get("wbasis").v(),
                    ),
                    (
                        "wquat",
                        rq(body.world_tf.rotation()).to_vec(),
                        sb.get("wquat").v(),
                    ),
                    ("quat", rq(t.rotation()).to_vec(), sb.get("quat").v()),
                ];
                let (lo, hi) = w.shapes.aabb(body.shape, &body.world_tf);
                let aabb: Vec<f64> = lo.iter().chain(hi.iter()).copied().collect();
                if aabb != sb.get("aabb").v() {
                    eprintln!(
                        "step {step_no} body {i} aabb: ours {aabb:?} want {:?}",
                        sb.get("aabb").v()
                    );
                    debug_hit = true;
                }

                for (nm, o, wnt) in pairs {
                    if o != wnt {
                        eprintln!("step {step_no} body {i} {nm}: ours {o:?} want {wnt:?}");
                        debug_hit = true;
                    }
                }
            }
            let sb = &step.get("bodies").a()[i];
            let t = w.motion_state_world_transform(b.ms);
            let body = w.body(b.rb);
            errors.upd("pos", &t.o, &sb.get("pos").v());
            errors.upd("quat", &rq(t.rotation()), &sb.get("quat").v());
            errors.upd("lin", &body.lin_vel, &sb.get("lin").v());
            errors.upd("ang", &body.ang_vel, &sb.get("ang").v());
            errors.upd("wpos", &body.world_tf.o, &sb.get("wpos").v());
            errors.upd(
                "wbasis",
                &body
                    .world_tf
                    .b
                    .iter()
                    .flatten()
                    .copied()
                    .collect::<Vec<f64>>(),
                &sb.get("wbasis").v(),
            );
            errors.upd("wquat", &rq(body.world_tf.rotation()), &sb.get("wquat").v());
            errors.count("active", body.is_active() != sb.get("active").b());
            let (lo, hi) = w.shapes.aabb(body.shape, &body.world_tf);
            errors.upd(
                "aabb",
                &[lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]],
                &sb.get("aabb").v(),
            );
        }

        let ms = step.get("manifolds").a();
        let n = w.num_manifolds();
        if std::env::var("BT_DEBUG").is_ok() && n != ms.len() {
            eprintln!("step {step_no}: {n} manifolds, want {}", ms.len());
            debug_hit = true;
        }
        errors.count("manifold_count", n != ms.len());

        for (k, m) in ms.iter().enumerate().take(n) {
            let mf = w.manifold_by_index(k);
            errors.count(
                "manifold_bodies",
                mf.body0 != m.get("b0").f() as usize || mf.body1 != m.get("b1").f() as usize,
            );
            let pts = m.get("points").a();
            errors.count("point_count", mf.points.len() != pts.len());

            if std::env::var("BT_DEBUG").is_ok() {
                for (pi, (p, jp)) in mf.points.iter().zip(pts).enumerate() {
                    let pairs: [(&str, Vec<f64>, Vec<f64>); 7] = [
                        ("local_a", p.local_a.to_vec(), jp.get("la").v()),
                        ("local_b", p.local_b.to_vec(), jp.get("lb").v()),
                        ("a", p.position_world_on_a.to_vec(), jp.get("a").v()),
                        ("b", p.position_world_on_b.to_vec(), jp.get("b").v()),
                        ("n", p.normal_world_on_b.to_vec(), jp.get("n").v()),
                        ("distance", vec![p.distance1], vec![jp.get("distance").f()]),
                        (
                            "impulse",
                            vec![p.applied_impulse],
                            vec![jp.get("impulse").f()],
                        ),
                    ];
                    for (nm, o, wnt) in pairs {
                        if o != wnt {
                            eprintln!(
                                "step {step_no} manifold {k} point {pi} {nm}: ours {o:?} want {wnt:?}"
                            );
                            debug_hit = true;
                        }
                    }
                }
                if mf.points.len() != pts.len() {
                    eprintln!(
                        "step {step_no} manifold {k}: {} points, want {}",
                        mf.points.len(),
                        pts.len()
                    );
                    debug_hit = true;
                }
            }

            for (p, jp) in mf.points.iter().zip(pts) {
                errors.upd("pt_a", &p.position_world_on_a, &jp.get("a").v());
                errors.upd("pt_b", &p.position_world_on_b, &jp.get("b").v());
                errors.upd("pt_n", &p.normal_world_on_b, &jp.get("n").v());
                errors.upd("pt_distance", &[p.distance1], &[jp.get("distance").f()]);
                errors.upd("pt_impulse", &[p.applied_impulse], &[jp.get("impulse").f()]);
            }
        }

        if debug_hit {
            for k in 0..n {
                let mf = w.manifold_by_index(k);
                for p in &mf.points {
                    eprintln!(
                        "  ours manifold {k} b{}-b{}: a {:?} b {:?} n {:?} d {:?} imp {:?} lat {:?} life {}",
                        mf.body0,
                        mf.body1,
                        p.position_world_on_a,
                        p.position_world_on_b,
                        p.normal_world_on_b,
                        p.distance1,
                        p.applied_impulse,
                        p.applied_impulse_lateral1,
                        p.life_time
                    );
                }
            }
            for (k, m) in ms.iter().enumerate() {
                for jp in m.get("points").a() {
                    eprintln!(
                        "  want manifold {k}: a {:?} b {:?} n {:?} d {:?} imp {:?}",
                        jp.get("a").v(),
                        jp.get("b").v(),
                        jp.get("n").v(),
                        jp.get("distance").f(),
                        jp.get("impulse").f()
                    );
                }
            }
            std::process::exit(3);
        }
    };

    if spec.get("detect").b() {
        w.perform_discrete_collision_detection();
    }

    compare(&w, &built, &steps[0], errors, 0);

    let collider_bounce = !collider.is_null() && collider.get("bounce_enabled").b();

    for (k, dt) in spec.get("dts").v().into_iter().enumerate() {
        if collider_bounce {
            for b in &built {
                let v = w.body(b.rb).lin_vel;
                let r = if (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
                    >= collider.get("min_bounce_speed").f()
                {
                    collider.get("bounce").f()
                } else {
                    0.0
                };
                w.body_mut(b.rb).restitution = f(r);
            }
        } else {
            for b in &built {
                w.body_mut(b.rb).restitution = 0.0;
            }
        }

        let apply_forces = |w: &mut World| {
            for b in &built {
                if b.spec.get("fixed").b() {
                    continue;
                }
                let g = if b.spec.get("use_global_gravity").b() {
                    gravity
                } else {
                    ZERO
                };
                let body = w.body_mut(b.rb);
                body.set_gravity(g);
                body.apply_force(b.spec.get("force").v3(), ZERO);
                body.apply_torque(b.spec.get("torque").v3());
            }
        };

        if prefer_accuracy {
            let d = dt / iterations;
            for _ in 0..iterations as usize {
                apply_forces(&mut w);
                w.step_simulation(d, 0, 0.0);
            }
        } else if !spec.get("sub_at").is_null() && spec.get("sub_at").f() as usize == k + 1 {
            apply_forces(&mut w);
            let d = dt / iterations;
            let subs = steps[k + 1].get("subs").a().to_vec();
            for (i, sub) in subs.iter().enumerate() {
                w.step_simulation(d, 0, 0.0);
                compare(&w, &built, sub, errors, 1000 * (k + 1) + i);
            }
            if spec.get("feedback").b() {
                feedback(&mut w, &built);
            }
        } else {
            apply_forces(&mut w);
            let n = w.step_simulation(dt, iterations as i32 + 2, dt / iterations);
            if spec.get("feedback").b() {
                feedback(&mut w, &built);
            }
            let want = steps[k + 1].get("substeps");
            if !want.is_null() {
                errors.count("substeps", want.f() as i32 != n);
                if std::env::var("BT_DEBUG").is_ok() && want.f() as i32 != n {
                    eprintln!("step {}: {n} substeps, want {}", k + 1, want.f());
                    std::process::exit(3);
                }
            }
        }

        compare(&w, &built, &steps[k + 1], errors, k + 1);
    }

    errors.clone()
}

/// Penetration depth beyond which a pair is classified as deep (the EPA penetration solver).
const DEEP: f64 = -0.03;

/// The narrow phase corpus: many two body cases, one detection pass each.
fn check_pairs(name: &str) {
    let root = load(name);
    let mut total = Errors::default();
    let mut bad = 0;
    let (mut deep_bad, mut deep_total) = (0, 0);
    let mut deep_err = Errors::default();
    let cases = root.get("cases").a();

    for (ci, c) in cases.iter().enumerate() {
        if let Ok(only) = std::env::var("BT_CASE") {
            if only.parse::<usize>().unwrap() != ci {
                continue;
            }
        }
        let depth0 = c.get("steps").a()[0]
            .get("manifolds")
            .a()
            .iter()
            .flat_map(|m| {
                m.get("points")
                    .a()
                    .iter()
                    .map(|p| p.get("distance").f())
                    .collect::<Vec<_>>()
            })
            .fold(0.0, f64::min);
        if std::env::var("BT_DEBUG").is_ok() && depth0 < DEEP {
            continue;
        }
        if std::env::var("BT_DEBUG").is_ok() {
            eprintln!(
                "case {ci}: {}",
                c.get("scene")
                    .get("bodies")
                    .a()
                    .iter()
                    .map(|b| format!(
                        "{:?} {:?} {:?}",
                        b.get("position").v(),
                        b.get("rotation").v(),
                        b.get("shapes").a()[0].get("type").s()
                    ))
                    .collect::<Vec<_>>()
                    .join(" | ")
            );
        }
        let mut e = Errors::default();
        run_one(c.get("scene"), c.get("steps").a(), &mut e);
        let worst = e.0.values().cloned().fold(0.0, f64::max);
        let depth = c.get("steps").a()[0]
            .get("manifolds")
            .a()
            .iter()
            .flat_map(|m| {
                m.get("points")
                    .a()
                    .iter()
                    .map(|p| p.get("distance").f())
                    .collect::<Vec<_>>()
            })
            .fold(0.0, f64::min);
        let deep = depth < DEEP;
        if worst > 0.0 {
            println!(
                "  case {ci} differs{}: worst {worst:e} {:?}",
                if deep { " (deep)" } else { "" },
                e.0.iter().filter(|(_, v)| **v > 0.0).collect::<Vec<_>>()
            );
            if deep {
                deep_bad += 1;
            } else {
                bad += 1;
            }
        }
        deep_total += deep as usize;
        for (k, v) in e.0 {
            if deep {
                let t = deep_err.0.entry(k).or_insert(0.0);
                if v > *t {
                    *t = v;
                }
                continue;
            }
            let t = total.0.entry(k).or_insert(0.0);
            if v > *t {
                *t = v;
            }
        }
    }

    println!(
        "pairs {name}: {bad} of {} shallow cases differ; {deep_bad} of {deep_total} deep (penetration beyond {DEEP}, EPA) cases differ",
        cases.len() - deep_total
    );
    for (k, v) in &deep_err.0 {
        println!("  deep MAX ABS ERROR {k:>16} = {v:e}");
    }
    let mut failed = vec![];

    for (k, v) in &total.0 {
        println!("  MAX ABS ERROR {k:>16} = {v:e}");
        if *v > 0.0 {
            failed.push(format!("{k}: {v:e} > 0"));
        }
    }

    assert!(failed.is_empty(), "{name}: {bad} cases differ: {failed:?}");
}

fn check(name: &str) {
    let e = run_scene(name);
    println!("scene {name}");
    let mut failed = vec![];

    for (k, v) in &e.0 {
        println!("  MAX ABS ERROR {k:>16} = {v:e}");
        if *v > 0.0 {
            failed.push(format!("{k}: {v:e} > 0"));
        }
    }

    assert!(failed.is_empty(), "{name}: {failed:?}");
}

macro_rules! scenes {
    ($($test:ident => $scene:literal),* $(,)?) => {
        $(
            #[test]
            fn $test() {
                check($scene);
            }
        )*
    };
}

scenes! {
    bullet_dyn_s01_sphere_free_fall => "s01_sphere_free_fall",
    bullet_dyn_s01b_sphere_fixed_dt => "s01b_sphere_fixed_dt",
    bullet_dyn_s01c_sphere_prefer_accuracy => "s01c_sphere_prefer_accuracy",
    bullet_dyn_s02_box_tumbling => "s02_box_tumbling",
    bullet_dyn_s03_sphere_bounce_rest => "s03_sphere_bounce_rest",
    bullet_dyn_s03d_sphere_bounce_single_steps => "s03d_sphere_bounce_single_steps",
    bullet_dyn_s06c_cylinder_cone_hull_single_steps => "s06c_cylinder_cone_hull_single_steps",
    bullet_dyn_s03b_sphere_roll_friction => "s03b_sphere_roll_friction",
    bullet_dyn_s03c_box_tumble_land => "s03c_box_tumble_land",
    bullet_dyn_s04_box_stack3 => "s04_box_stack3",
    bullet_dyn_s05_sphere_mesh => "s05_sphere_mesh",
    bullet_dyn_s05b_box_mesh => "s05b_box_mesh",
    bullet_dyn_s06_box_incline_friction => "s06_box_incline_friction",
    bullet_dyn_s06b_cylinder_cone_hull => "s06b_cylinder_cone_hull",
    bullet_dyn_s07_ball_pendulum => "s07_ball_pendulum",
    bullet_dyn_s07b_ball_chain => "s07b_ball_chain",
    bullet_dyn_s08_hinge_pendulum => "s08_hinge_pendulum",
    bullet_dyn_s08b_hinge_two_dynamic => "s08b_hinge_two_dynamic",
    bullet_dyn_s08c_hinge_antiparallel => "s08c_hinge_antiparallel",
    bullet_dyn_s09_universal_joint_unconnected => "s09_universal_joint_unconnected",
    bullet_dyn_s10_slider_limits => "s10_slider_limits",
    bullet_dyn_s10b_slider_two_dynamic => "s10b_slider_two_dynamic",
    bullet_dyn_s11_damping => "s11_damping",
    bullet_dyn_s12_sleep_wake => "s12_sleep_wake",
    bullet_dyn_s12b_box_kicked_awake => "s12b_box_kicked_awake",
    bullet_dyn_s12c_box_sleeps_then_kicked => "s12c_box_sleeps_then_kicked",
    bullet_dyn_s12d_sleep_wake_single_steps => "s12d_sleep_wake_single_steps",
    bullet_dyn_s12e_awake_box_fast_sphere => "s12e_awake_box_fast_sphere",
    bullet_dyn_s12f_sleeping_box_fast_sphere_8m => "s12f_sleeping_box_fast_sphere_8m",
    bullet_dyn_s12g_fast_sphere_hits_box_substeps => "s12g_fast_sphere_hits_box_substeps",
    bullet_dyn_s12h_fast_sphere_hits_box_single_steps => "s12h_fast_sphere_hits_box_single_steps",
    bullet_dyn_s12i_sleep_wake_fine_steps => "s12i_sleep_wake_fine_steps",
    bullet_dyn_s13_compound_offset => "s13_compound_offset",
    bullet_dyn_s13b_capsule => "s13b_capsule",
    bullet_dyn_s14_rolling_friction_sphere => "s14_rolling_friction_sphere",
    bullet_dyn_s14b_rolling_friction_box => "s14b_rolling_friction_box",
    bullet_dyn_s14c_rolling_friction_single_axis => "s14c_rolling_friction_single_axis",
    bullet_dyn_b06_shape_cylinder => "b06_shape_cylinder",
    bullet_dyn_b06_shape_cone => "b06_shape_cone",
    bullet_dyn_b06_shape_cone_open => "b06_shape_cone_open",
    bullet_dyn_b06_shape_cylinder_open => "b06_shape_cylinder_open",
    bullet_dyn_b06_collider_friction_rolling => "b06_collider_friction_rolling",
}

macro_rules! pair_scenes {
    ($($test:ident => $scene:literal),* $(,)?) => {
        $(
            #[test]
            fn $test() {
                check_pairs($scene);
            }
        )*
    };
}

pair_scenes! {
    bullet_dyn_pairs_sphere_sphere => "pairs_sphere_sphere",
    bullet_dyn_pairs_sphere_box => "pairs_sphere_box",
    bullet_dyn_pairs_sphere_cylinder => "pairs_sphere_cylinder",
    bullet_dyn_pairs_sphere_cone => "pairs_sphere_cone",
    bullet_dyn_pairs_sphere_hull => "pairs_sphere_hull",
    bullet_dyn_pairs_box_box => "pairs_box_box",
    bullet_dyn_pairs_box_flat => "pairs_box_flat",
    bullet_dyn_pairs_box_aligned => "pairs_box_aligned",
    bullet_dyn_pairs_box_touching => "pairs_box_touching",
    bullet_dyn_pairs_box_touching_aligned => "pairs_box_touching_aligned",
    bullet_dyn_pairs_box_cylinder => "pairs_box_cylinder",
    bullet_dyn_pairs_box_cone => "pairs_box_cone",
    bullet_dyn_pairs_box_hull => "pairs_box_hull",
    bullet_dyn_pairs_cylinder_cylinder => "pairs_cylinder_cylinder",
    bullet_dyn_pairs_cylinder_cone => "pairs_cylinder_cone",
    bullet_dyn_pairs_cylinder_hull => "pairs_cylinder_hull",
    bullet_dyn_pairs_cone_cone => "pairs_cone_cone",
    bullet_dyn_pairs_cone_hull => "pairs_cone_hull",
    bullet_dyn_pairs_hull_hull => "pairs_hull_hull",
}

pair_scenes! {
    bullet_dyn_solve_sphere => "solve_sphere",
    bullet_dyn_solve_box => "solve_box",
    bullet_dyn_solve_cylinder => "solve_cylinder",
    bullet_dyn_solve_cone => "solve_cone",
    bullet_dyn_solve_hull => "solve_hull",
}
