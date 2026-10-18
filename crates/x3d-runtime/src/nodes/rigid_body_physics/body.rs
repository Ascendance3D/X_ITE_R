//! RigidBody (X_ITE `RigidBody.js`).

use super::arena::{clone_body, clone_shape_tree, Home, PWorld};
use super::*;
use crate::physics::bt::body::{BodyId, ConstructionInfo, MotionStateId};
use crate::physics::bt::math::{rq, rt, Tf, IDENTITY3, V3, ZERO};
use crate::physics::bt::shape::ShapeId;
use std::collections::HashMap;
use x3d_fields::math::Quaternion;

/// The members of a RigidBody (`this .compoundShape`, `this .rigidBody`, ...).
#[derive(Debug)]
pub struct BodyRt {
    /// The world the Ammo objects below live in (see `arena`).
    pub(super) home: Home,
    pub(super) compound: ShapeId,
    pub(super) motion_state: MotionStateId,
    pub(super) body: BodyId,
    pub(super) geometry_nodes: Vec<NodeId>,
    pub(super) other_geometry_nodes: Vec<NodeId>,
    pub(super) matrix: Matrix4,
    pub(super) force: Vector3,
    pub(super) torque: Vector3,
    /// The node was released; its Ammo objects are gone.
    pub(super) disposed: bool,
}

fn v3a(v: &Vector3) -> V3 {
    [v.x, v.y, v.z]
}

/// A column-major Matrix4 to a btTransform (`t .getBasis () .setValue (m [0], m [4], m [8], ...)`).
pub(super) fn to_transform(m: &[f64; 16]) -> Tf {
    rt(Tf { b: [[m[0], m[4], m[8]], [m[1], m[5], m[9]], [m[2], m[6], m[10]]], o: [m[12], m[13], m[14]] })
}

impl Runtime {
    pub(super) fn body_rt(&self, node: NodeId) -> &BodyRt {
        match self.phys(node) {
            Some(PhysState::Body(b)) => b,
            _ => panic!("node {node} is not a RigidBody"),
        }
    }

    pub(super) fn body_rt_mut(&mut self, node: NodeId) -> &mut BodyRt {
        match self.phys_mut(node) {
            Some(PhysState::Body(b)) => b,
            _ => panic!("node {node} is not a RigidBody"),
        }
    }

    /// `home, compound, motion state, body` of a RigidBody.
    fn body_ids(&self, node: NodeId) -> (Home, ShapeId, MotionStateId, BodyId) {
        let b = self.body_rt(node);

        (b.home, b.compound, b.motion_state, b.body)
    }

    /// RigidBody constructor members.
    pub(super) fn body_construct(&mut self, node: NodeId) {
        let (compound, motion_state, body) = self.with_world(None, |w| {
            let compound = w.new_compound();
            let motion_state = w.new_motion_state(Tf::IDENTITY);
            let info = ConstructionInfo::new(0.0, Some(motion_state), compound);
            let body = w.new_rigid_body(&info);

            (compound, motion_state, body)
        });

        self.nrt_mut(node).state = State::Physics(PhysState::Body(Box::new(BodyRt {
            home: None,
            compound,
            motion_state,
            body,
            geometry_nodes: vec![],
            other_geometry_nodes: vec![],
            matrix: Matrix4::IDENTITY,
            force: Vector3::ZERO,
            torque: Vector3::ZERO,
            disposed: false,
        })));
    }

    pub(super) fn body_initialize(&mut self, node: NodeId) {
        use PhysicsCb::*;

        self.pcb(node, "linearVelocity", BodySetLinearVelocity);
        self.pcb(node, "angularVelocity", BodySetAngularVelocity);
        self.pcb(node, "useFiniteRotation", BodySetFiniteRotationAxis);
        self.pcb(node, "finiteRotationAxis", BodySetFiniteRotationAxis);
        self.pcb(node, "autoDamp", BodySetDamping);
        self.pcb(node, "linearDampingFactor", BodySetDamping);
        self.pcb(node, "angularDampingFactor", BodySetDamping);
        self.pcb(node, "forces", BodySetForces);
        self.pcb(node, "torques", BodySetTorques);
        // `_disableTime` twice, no `_autoDisable`: as upstream.
        self.pcb(node, "disableTime", BodySetDisable);
        self.pcb(node, "disableLinearSpeed", BodySetDisable);
        self.pcb(node, "disableAngularSpeed", BodySetDisable);
        self.pcb(node, "geometry", BodySetGeometry);

        if let Some(k) = self.private_field(node, "otherGeometry") {
            self.add_interest(k, node, Cb::Physics(BodySetGeometry));
        }

        self.pcb(node, "fixed", BodySetMassProps);
        self.pcb(node, "mass", BodySetMassProps);
        self.pcb(node, "inertia", BodySetMassProps);

        if let Some(k) = self.private_field(node, "transform") {
            self.add_interest(k, node, Cb::Physics(BodySetTransform));
        }

        self.body_set_forces(node);
        self.body_set_torques(node);
        self.body_set_geometry(node);
    }

    /// `setCollection (value)`: a RigidBodyCollection takes the body (`value`) or lets it go (`null`).
    pub(super) fn body_set_collection(&mut self, node: NodeId, value: Option<NodeId>) {
        if let Some(c) = value {
            self.body_adopt(node, c);
        }

        let k = self.phys_private(node, "collection");

        self.set_value(k, FieldValue::SFNode(value));
    }

    /// `getCollection ()`.
    pub(super) fn body_collection(&self, node: NodeId) -> Option<NodeId> {
        self.sfnode_of(self.phys_private(node, "collection"))
    }

    fn body_set_position(&mut self, node: NodeId) {
        let position = self.pv3(node, "position");

        for g in self.body_rt(node).geometry_nodes.clone() {
            self.set_v3(g, "translation", position);
        }
    }

    fn body_set_orientation(&mut self, node: NodeId) {
        let orientation = self.prot(node, "orientation");

        for g in self.body_rt(node).geometry_nodes.clone() {
            if let Some(k) = self.pre(g, "rotation") {
                self.set_value(k, FieldValue::SFRotation(orientation));
            }
        }
    }

    pub(super) fn body_set_transform(&mut self, node: NodeId) {
        let (home, compound, ms, body) = self.body_ids(node);
        let position = self.pv3(node, "position");
        let orientation = self.prot(node, "orientation");
        let m = transform_matrix(&position, &orientation);
        let t = to_transform(&m.0);
        let mut im = m;

        im.inverse();

        let it = to_transform(&im.0);

        self.body_rt_mut(node).matrix = m;
        self.with_world(home, |pw| {
            let n = pw.w.shapes.num_child_shapes(compound);

            for i in 0..n {
                pw.w.shapes.update_child_transform(compound, i, it, false);
            }

            pw.w.shapes.recalculate_local_aabb(compound);
            pw.w.set_motion_state_world_transform(ms, t);
            pw.w.set_motion_state(body, ms);
        });
    }

    pub(super) fn body_set_linear_velocity(&mut self, node: NodeId) {
        let (home, _, _, body) = self.body_ids(node);
        let v = if self.get_bool(node, "fixed") { ZERO } else { v3a(&self.pv3(node, "linearVelocity")) };

        self.with_world(home, |pw| {
            let b = pw.w.body_mut(body);

            b.set_linear_velocity(v);
            b.activate(false);
        });
    }

    pub(super) fn body_set_angular_velocity(&mut self, node: NodeId) {
        let (home, _, _, body) = self.body_ids(node);
        let v = if self.get_bool(node, "fixed") { ZERO } else { v3a(&self.pv3(node, "angularVelocity")) };

        self.with_world(home, |pw| {
            let b = pw.w.body_mut(body);

            b.set_angular_velocity(v);
            b.activate(false);
        });
    }

    pub(super) fn body_set_finite_rotation_axis(&mut self, node: NodeId) {
        let (home, _, _, body) = self.body_ids(node);
        let v = if self.get_bool(node, "useFiniteRotation") { v3a(&self.pv3(node, "finiteRotationAxis")) } else { [1.0, 1.0, 1.0] };

        self.with_world(home, |pw| pw.w.body_mut(body).set_angular_factor(v));
    }

    pub(super) fn body_set_damping(&mut self, node: NodeId) {
        let (home, _, _, body) = self.body_ids(node);
        let (l, a) = if self.get_bool(node, "autoDamp") { (self.get_f64(node, "linearDampingFactor"), self.get_f64(node, "angularDampingFactor")) } else { (0.0, 0.0) };

        self.with_world(home, |pw| {
            let b = pw.w.body_mut(body);

            b.set_damping(l, a);
            b.activate(false);
        });
    }

    fn body_set_center_of_mass(&mut self, node: NodeId) {
        let (home, _, _, body) = self.body_ids(node);
        let com = v3a(&self.pv3(node, "centerOfMass"));

        self.with_world(home, |pw| pw.w.body_mut(body).set_center_of_mass_transform(Tf { b: IDENTITY3, o: com }));
    }

    pub(super) fn body_set_mass_props(&mut self, node: NodeId) {
        let (home, compound, _, body) = self.body_ids(node);
        let mass = if self.get_bool(node, "fixed") { 0.0 } else { self.get_f64(node, "mass") };

        // `localInertia` holds the row sums of `inertia`, and `calculateLocalInertia` overwrites it.
        self.with_world(home, |pw| {
            let inertia = pw.w.shapes.calculate_local_inertia(compound, mass);

            pw.w.body_mut(body).set_mass_props(mass, inertia);
        });
    }

    pub(super) fn body_set_forces(&mut self, node: NodeId) {
        let mut force = Vector3::ZERO;

        for f in self.pmf_vec3(node, "forces") {
            force.add(&f);
        }

        self.body_rt_mut(node).force = force;
    }

    pub(super) fn body_set_torques(&mut self, node: NodeId) {
        let mut torque = Vector3::ZERO;

        for t in self.pmf_vec3(node, "torques") {
            torque.add(&t);
        }

        self.body_rt_mut(node).torque = torque;
    }

    pub(super) fn body_set_disable(&mut self, node: NodeId) {
        let (home, _, _, body) = self.body_ids(node);
        let (l, a) = if self.get_bool(node, "autoDisable") { (self.get_f64(node, "disableLinearSpeed"), self.get_f64(node, "disableAngularSpeed")) } else { (0.0, 0.0) };

        self.with_world(home, |pw| {
            let b = pw.w.body_mut(body);

            b.lin_sleeping_threshold = crate::physics::bt::math::f(l);
            b.ang_sleeping_threshold = crate::physics::bt::math::f(a);
        });
    }

    pub(super) fn body_set_geometry(&mut self, node: NodeId) {
        use PhysicsCb::*;

        let transform = self.phys_private(node, "transform");
        let position = self.pre(node, "position");
        let orientation = self.pre(node, "orientation");
        let other = self.phys_private(node, "otherGeometry");

        for g in self.body_rt(node).geometry_nodes.clone() {
            self.remove_node_interest(g, node, Cb::Physics(BodyTransformEvent));

            if let Some(k) = self.private_field(g, "compoundShape") {
                self.remove_interest(k, node, Cb::Physics(BodySetCompoundShape));
            }

            self.collidable_set_body(g, None);

            if let (Some(t), Some(p)) = (self.pre(g, "translation"), position) {
                self.remove_field_interest(t, p);
                self.remove_field_interest(p, t);
            }

            if let (Some(r), Some(o)) = (self.pre(g, "rotation"), orientation) {
                self.remove_field_interest(r, o);
                self.remove_field_interest(o, r);
            }
        }

        for g in self.body_rt(node).other_geometry_nodes.clone() {
            if let Some(k) = self.private_field(g, "body") {
                self.remove_interest(k, node, Cb::Physics(BodySetBody));
            }
        }

        self.body_rt_mut(node).geometry_nodes.clear();

        let mut geometry_nodes = vec![];

        for g in self.phys_cast_list(node, "geometry", is_collidable) {
            if self.collidable_body(g).is_some() {
                if let Some(k) = self.private_field(g, "body") {
                    self.add_interest(k, node, Cb::Physics(BodySetBody));
                }

                let rt = self.body_rt_mut(node);

                if !rt.other_geometry_nodes.contains(&g) {
                    rt.other_geometry_nodes.push(g);
                }

                continue;
            }

            self.collidable_set_body(g, Some(node));
            geometry_nodes.push(g);
        }

        self.body_rt_mut(node).geometry_nodes = geometry_nodes.clone();

        // The collidables follow the body into its world.
        let home = self.body_rt(node).home;

        for &g in &geometry_nodes {
            self.collidable_migrate(g, home);
        }

        for g in geometry_nodes {
            self.add_node_interest(g, node, Cb::Physics(BodyTransformEvent));

            if let Some(k) = self.private_field(g, "compoundShape") {
                self.add_interest(k, node, Cb::Physics(BodySetCompoundShape));
            }

            if let (Some(t), Some(p)) = (self.pre(g, "translation"), position) {
                self.add_field_interest(t, p);
                self.add_field_interest(p, t);
            }

            if let (Some(r), Some(o)) = (self.pre(g, "rotation"), orientation) {
                self.add_field_interest(r, o);
                self.add_field_interest(o, r);
            }
        }

        let _ = (transform, other);
        self.body_set_compound_shape(node);
    }

    /// `set_body__`: `this ._otherGeometry .addEvent ()`.
    pub(super) fn body_set_body(&mut self, node: NodeId) {
        let k = self.phys_private(node, "otherGeometry");

        self.add_event(k);
    }

    pub(super) fn body_set_compound_shape(&mut self, node: NodeId) {
        let (home, compound, _, _) = self.body_ids(node);
        let children: Vec<ShapeId> = self.body_rt(node).geometry_nodes.iter().map(|&g| self.collidable_compound(g)).collect();

        self.with_world(home, |pw| {
            for i in (0..pw.w.shapes.num_child_shapes(compound)).rev() {
                pw.w.shapes.remove_child_shape_by_index(compound, i);
            }

            for c in children {
                // `new Ammo .btTransform ()`: nothing initializes the basis.
                pw.w.shapes.add_child_shape(compound, Tf { b: [[0.0; 3]; 3], o: ZERO }, c);
            }
        });

        self.body_set_position(node);
        self.body_set_orientation(node);
        self.body_set_transform(node);
        self.body_set_linear_velocity(node);
        self.body_set_angular_velocity(node);
        self.body_set_finite_rotation_axis(node);
        self.body_set_damping(node);
        self.body_set_center_of_mass(node);
        self.body_set_mass_props(node);
        self.body_set_disable(node);
    }

    /// `applyForces (gravity)`.
    pub(super) fn body_apply_forces(&mut self, node: NodeId, gravity: &Vector3) {
        if self.get_bool(node, "fixed") {
            return;
        }

        let g = if self.get_bool(node, "useGlobalGravity") { v3a(gravity) } else { ZERO };
        let (home, _, _, body) = self.body_ids(node);
        let (f, t) = {
            let b = self.body_rt(node);

            (v3a(&b.force), v3a(&b.torque))
        };

        self.with_world(home, |pw| {
            let b = pw.w.body_mut(body);

            b.set_gravity(g);
            b.apply_force(f, ZERO);
            b.apply_torque(t);
        });
    }

    /// `update ()`: the simulation result into the fields.
    pub(super) fn body_update(&mut self, node: NodeId) {
        let (home, _, ms, body) = self.body_ids(node);
        let (origin, q, lin, ang) = self.with_world(home, |pw| {
            let tf = pw.w.motion_state_world_transform(ms);
            let b = pw.w.body(body);

            (tf.o, rq(tf.rotation()), b.lin_vel, b.ang_vel)
        });
        let mut quaternion = Quaternion::IDENTITY;

        quaternion.set(q[0], q[1], q[2], q[3]);

        let mut orientation = Rotation4::identity();

        orientation.set_quaternion(&quaternion);

        self.set_v3(node, "position", Vector3::new(origin[0], origin[1], origin[2]));

        if let Some(k) = self.pre(node, "orientation") {
            self.set_value(k, FieldValue::SFRotation(orientation));
        }

        self.set_v3(node, "linearVelocity", Vector3::new(lin[0], lin[1], lin[2]));
        self.set_v3(node, "angularVelocity", Vector3::new(ang[0], ang[1], ang[2]));
    }

    // --- worlds ------------------------------------------------------------------------------

    /// A collection takes the body: the body and the shape tree below it move into its world.
    fn body_adopt(&mut self, node: NodeId, collection: NodeId) {
        if self.body_rt(node).home == Some(collection) {
            return;
        }

        self.body_migrate(node, Some(collection));
    }

    /// The collidables below a body (its geometry and what the CollidableOffsets hold).
    pub(super) fn body_collidable_tree(&self, node: NodeId) -> Vec<NodeId> {
        let mut out = vec![];

        for &g in &self.body_rt(node).geometry_nodes {
            self.collidable_tree(g, &mut out);
        }

        out
    }

    /// Copy a body and its shape tree from its world into `to`; the old slots are given back.
    pub(super) fn body_migrate(&mut self, node: NodeId, to: Home) {
        let (from, compound, _, body) = self.body_ids(node);

        if from == to {
            return;
        }

        let tree = self.body_collidable_tree(node);
        let mut src = self.take_world(from);
        let mut dst = self.take_world(to);
        let mut map = HashMap::new();

        src.w.remove_rigid_body(body);

        let new_compound = clone_shape_tree(&src, &mut dst, compound, &mut map);

        self.collidable_clone_own_shapes(&tree, &src, &mut dst, &mut map);

        let (new_body, new_ms) = clone_body(&src, &mut dst, body, new_compound);
        let old_ms = src.w.bodies[body].motion_state;

        for &old in map.keys() {
            src.free_shape(old);
        }

        src.free_body(body);

        if let Some(ms) = old_ms {
            src.free_motion_state(ms);
        }

        self.put_world(to, dst);
        self.put_world(from, src);

        let b = self.body_rt_mut(node);

        b.home = to;
        b.compound = new_compound;
        b.motion_state = new_ms;
        b.body = new_body;

        for g in tree {
            self.collidable_remap(g, to, &map);
        }
    }

    /// `dispose ()` of a released RigidBody: out of the world, the Ammo objects given back.
    pub(super) fn body_dispose(&mut self, node: NodeId, dead: &HashSet<NodeId>) {
        let (home, compound, ms, body) = self.body_ids(node);

        if self.body_rt(node).disposed {
            return;
        }

        // The collidables of a body that goes away let go of it.
        for g in self.body_rt(node).geometry_nodes.clone() {
            if !dead.contains(&g) {
                if let Some(k) = self.private_field(g, "body") {
                    self.set_raw(k, FieldValue::SFNode(None));
                }
            }
        }

        let world_alive = match home {
            None => true,
            Some(c) => !dead.contains(&c) && matches!(self.phys(c), Some(PhysState::Collection(_))),
        };

        if world_alive {
            if let Some(c) = home {
                if let Some(PhysState::Collection(rt)) = self.phys_mut(c) {
                    rt.rigid_bodies.retain(|&b| b != body);
                    rt.body_nodes.retain(|&n| n != node);
                    rt.other_body_nodes.retain(|&n| n != node);
                }
            }

            self.with_world(home, |pw| {
                pw.w.remove_rigid_body(body);
                pw.free_body(body);
                pw.free_motion_state(ms);
                pw.free_shape(compound);
            });
        }

        let b = self.body_rt_mut(node);

        b.disposed = true;
        b.geometry_nodes.clear();
        b.other_geometry_nodes.clear();
    }
}

impl Runtime {
    /// Take the world `home` names out of its owner (see [`Runtime::with_world`]).
    pub(super) fn take_world(&mut self, home: Home) -> Box<PWorld> {
        match home {
            None => self.physics.nursery.take().unwrap_or_else(|| Box::new(PWorld::new())),
            Some(c) => match self.phys_mut(c) {
                Some(PhysState::Collection(rt)) => rt.world.take().expect("the dynamics world is in use"),
                _ => panic!("node {c} is not a RigidBodyCollection"),
            },
        }
    }

    pub(super) fn put_world(&mut self, home: Home, w: Box<PWorld>) {
        match home {
            None => self.physics.nursery = Some(w),
            Some(c) => {
                if let Some(PhysState::Collection(rt)) = self.phys_mut(c) {
                    rt.world = Some(w);
                }
            }
        }
    }
}
