//! RigidBodyCollection, CollisionCollection and CollisionSpace (X_ITE `RigidBodyCollection.js`,
//! `CollisionCollection.js`, `CollisionSpace.js`, `X3DNBodyCollisionSpaceNode.js`).

use super::arena::PWorld;
use super::*;
use crate::physics::bt::body::BodyId;
use crate::physics::bt::math::f;

/// `AppliedParametersType`.
const BOUNCE: i32 = 0;
const FRICTION_COEFFICIENT_2: i32 = 2;

fn applied_parameter(name: &str) -> Option<i32> {
    [
        "BOUNCE",
        "USER_FRICTION",
        "FRICTION_COEFFICIENT_2",
        "ERROR_REDUCTION",
        "CONSTANT_FORCE",
        "SPEED_1",
        "SPEED_2",
        "SLIP_1",
        "SLIP_2",
    ]
    .iter()
    .position(|n| *n == name)
    .map(|i| i as i32)
}

/// The members of a RigidBodyCollection.
#[derive(Debug)]
pub struct CollectionRt {
    /// `this .dynamicsWorld` and the objects of the bodies in it.
    pub(super) world: Option<Box<PWorld>>,
    pub(super) delta_time: f64,
    pub(super) body_nodes: Vec<NodeId>,
    pub(super) other_body_nodes: Vec<NodeId>,
    /// `this .rigidBodies`: the bodies that are in the dynamics world.
    pub(super) rigid_bodies: Vec<BodyId>,
    pub(super) joint_nodes: Vec<NodeId>,
    pub(super) other_joint_nodes: Vec<NodeId>,
    pub(super) collider: Option<NodeId>,
    /// Ids of the constraints in the dynamics world (added by the joints).
    pub(super) constraint_ids: Vec<usize>,
    pub(super) disposed: bool,
}

impl CollectionRt {
    pub fn live_constraints(&self) -> usize {
        self.constraint_ids.len()
    }
}

/// The members of a CollisionCollection.
#[derive(Debug, Default)]
pub struct CollisionCollectionRt {
    pub(super) applied_parameters: Vec<i32>,
    pub(super) collidable_nodes: Vec<NodeId>,
    pub(super) collision_space_nodes: Vec<NodeId>,
}

/// The members of a CollisionSpace.
#[derive(Debug, Default)]
pub struct CollisionSpaceRt {
    pub(super) collidable_nodes: Vec<NodeId>,
    pub(super) collision_space_nodes: Vec<NodeId>,
}

impl Runtime {
    pub(super) fn coll_rt(&self, node: NodeId) -> &CollectionRt {
        match self.phys(node) {
            Some(PhysState::Collection(c)) => c,
            _ => panic!("node {node} is not a RigidBodyCollection"),
        }
    }

    pub(super) fn coll_rt_mut(&mut self, node: NodeId) -> &mut CollectionRt {
        match self.phys_mut(node) {
            Some(PhysState::Collection(c)) => c,
            _ => panic!("node {node} is not a RigidBodyCollection"),
        }
    }

    pub(super) fn collection_construct(&mut self, node: NodeId) {
        self.nrt_mut(node).state = State::Physics(PhysState::Collection(Box::new(CollectionRt {
            world: Some(Box::new(PWorld::new())),
            delta_time: 0.0,
            body_nodes: vec![],
            other_body_nodes: vec![],
            rigid_bodies: vec![],
            joint_nodes: vec![],
            other_joint_nodes: vec![],
            collider: None,
            constraint_ids: vec![],
            disposed: false,
        })));
    }

    pub(super) fn collection_initialize(&mut self, node: NodeId) {
        use PhysicsCb::*;

        // this .getLive () .addInterest ("set_enabled__", this)
        self.watch_live(node);

        self.pcb(node, "enabled", CollSetEnabled);
        self.pcb(node, "gravity", CollSetGravity);
        self.pcb(
            node,
            "contactSurfaceThickness",
            CollSetContactSurfaceThickness,
        );
        self.pcb(node, "collider", CollSetCollider);
        self.pcb(node, "bodies", CollSetBodies);
        self.pcb(node, "joints", CollSetJoints);

        self.coll_set_enabled(node);
        self.coll_set_gravity(node);
        self.coll_set_collider(node);
        self.coll_set_bodies(node);
    }

    /// `getTimeStep ()`: a moving average of 1 / frame rate over DELAY frames.
    fn coll_time_step(&mut self, node: NodeId) -> f64 {
        const DELAY: f64 = 15.0;

        let dt = 1.0 / x3d_fields::jsmath::js_max(10.0, self.frame_rate);
        let rt = self.coll_rt_mut(node);
        let delta_time = (((DELAY - 1.0) * rt.delta_time) + dt) / DELAY;

        rt.delta_time = delta_time;

        if self.physics.record_dt {
            self.physics.dt_log.push(delta_time);
        }

        delta_time
    }

    pub(super) fn coll_set_enabled(&mut self, node: NodeId) {
        let sensor = self.sensor_events_field();

        if self.phys_live(node) && self.get_bool(node, "enabled") {
            self.add_interest(sensor, node, Cb::Physics(PhysicsCb::CollUpdate));
        } else {
            self.remove_interest(sensor, node, Cb::Physics(PhysicsCb::CollUpdate));
        }
    }

    pub(super) fn coll_set_gravity(&mut self, node: NodeId) {
        let g = self.pv3(node, "gravity");

        self.with_world(Some(node), |pw| pw.w.set_gravity([g.x, g.y, g.z]));
    }

    pub(super) fn coll_set_contact_surface_thickness(&mut self, node: NodeId) {
        let thickness = self.get_f64(node, "contactSurfaceThickness");
        let bodies = self.coll_body_ids(node);

        self.with_world(Some(node), |pw| {
            for (_, b) in bodies {
                let shape = pw.w.body(b).shape;

                pw.w.shapes.set_margin(shape, thickness);
            }
        });
    }

    /// The (node, body id) pairs of `this .bodyNodes`.
    fn coll_body_ids(&self, node: NodeId) -> Vec<(NodeId, BodyId)> {
        self.coll_rt(node)
            .body_nodes
            .iter()
            .map(|&b| (b, self.body_rt(b).body))
            .collect()
    }

    pub(super) fn coll_set_collider(&mut self, node: NodeId) {
        if let Some(old) = self.coll_rt(node).collider {
            self.remove_node_interest(old, node, Cb::Physics(PhysicsCb::CollSetColliderParameters));
        }

        let collider = self.phys_cast_field(node, "collider", is_collision_collection);

        self.coll_rt_mut(node).collider = collider;

        if let Some(c) = collider {
            self.add_node_interest(c, node, Cb::Physics(PhysicsCb::CollSetColliderParameters));
        }

        self.coll_set_collider_parameters(node);
    }

    pub(super) fn coll_set_collider_parameters(&mut self, node: NodeId) {
        let collider = self.coll_rt(node).collider;
        let bodies = self.coll_body_ids(node);

        self.with_world(Some(node), |pw| {
            for &(_, b) in &bodies {
                let body = pw.w.body_mut(b);

                body.friction = f(0.5);
                body.rolling_friction = f(0.0);
            }
        });

        let Some(c) = collider else {
            return;
        };

        for parameter in self.cc_rt(c).applied_parameters.clone() {
            if parameter == FRICTION_COEFFICIENT_2 {
                let (x, y) = match self.pre_value(c, "frictionCoefficients") {
                    Some(FieldValue::SFVec2f(v)) => (v.x, v.y),
                    _ => (0.0, 0.0),
                };

                self.with_world(Some(node), |pw| {
                    for &(_, b) in &bodies {
                        let body = pw.w.body_mut(b);

                        body.friction = f(x);
                        body.rolling_friction = f(y);
                    }
                });
            }
        }
    }

    fn coll_set_bounce(&mut self, node: NodeId) {
        let bodies = self.coll_body_ids(node);
        let mut bounce: Option<(f64, f64)> = None;

        if let Some(c) = self.coll_rt(node).collider {
            if self.get_bool(c, "enabled") && self.cc_rt(c).applied_parameters.contains(&BOUNCE) {
                bounce = Some((self.get_f64(c, "minBounceSpeed"), self.get_f64(c, "bounce")));
            }
        }

        self.with_world(Some(node), |pw| {
            for &(_, b) in &bodies {
                let body = pw.w.body_mut(b);

                match bounce {
                    Some((min_bounce_speed, bounce)) => {
                        let v = body.lin_vel;

                        if (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() >= min_bounce_speed {
                            body.restitution = f(bounce);
                        } else {
                            body.restitution = f(0.0);
                        }
                    }
                    None => body.restitution = f(0.0),
                }
            }
        });
    }

    pub(super) fn coll_set_bodies(&mut self, node: NodeId) {
        use PhysicsCb::*;

        for b in self.coll_rt(node).body_nodes.clone() {
            if let Some(k) = self.pre(b, "enabled") {
                self.remove_interest(k, node, Cb::Physics(CollSetDynamicsWorld));
            }

            self.body_set_collection(b, None);
        }

        for ob in self.coll_rt(node).other_body_nodes.clone() {
            if let Some(k) = self.private_field(ob, "collection") {
                self.remove_interest(k, node, Cb::Physics(CollSetBodies));
            }
        }

        self.coll_rt_mut(node).body_nodes.clear();

        for b in self.phys_cast_list(node, "bodies", is_rigid_body) {
            if self.body_collection(b).is_some() {
                if let Some(k) = self.private_field(b, "collection") {
                    self.add_interest(k, node, Cb::Physics(CollSetBodies));
                }

                let rt = self.coll_rt_mut(node);

                if !rt.other_body_nodes.contains(&b) {
                    rt.other_body_nodes.push(b);
                }

                continue;
            }

            self.body_set_collection(b, Some(node));
            self.coll_rt_mut(node).body_nodes.push(b);
        }

        for b in self.coll_rt(node).body_nodes.clone() {
            if let Some(k) = self.pre(b, "enabled") {
                self.add_interest(k, node, Cb::Physics(CollSetDynamicsWorld));
            }
        }

        self.coll_set_collider_parameters(node);
        self.coll_set_contact_surface_thickness(node);
        self.coll_set_dynamics_world(node);
        self.coll_set_joints(node);
    }

    pub(super) fn coll_set_dynamics_world(&mut self, node: NodeId) {
        let old = std::mem::take(&mut self.coll_rt_mut(node).rigid_bodies);
        let mut new = vec![];

        for &b in &self.coll_rt(node).body_nodes.clone() {
            if !self.get_bool(b, "enabled") {
                continue;
            }

            new.push(self.body_rt(b).body);
        }

        self.with_world(Some(node), |pw| {
            for &rb in &old {
                pw.w.remove_rigid_body(rb);
            }

            for &rb in &new {
                pw.w.add_rigid_body(rb);
            }
        });

        self.coll_rt_mut(node).rigid_bodies = new;
    }

    pub(super) fn coll_set_joints(&mut self, node: NodeId) {
        for j in self.coll_rt(node).joint_nodes.clone() {
            self.joint_set_collection(j, None);
        }

        self.coll_rt_mut(node).joint_nodes.clear();

        for oj in self.coll_rt(node).other_joint_nodes.clone() {
            if let Some(k) = self.private_field(oj, "collection") {
                self.remove_interest(k, node, Cb::Physics(PhysicsCb::CollSetJoints));
            }
        }

        self.coll_rt_mut(node).other_joint_nodes.clear();

        for j in self.phys_cast_list(node, "joints", is_joint) {
            if self.joint_collection(j).is_some() {
                if let Some(k) = self.private_field(j, "collection") {
                    self.add_interest(k, node, Cb::Physics(PhysicsCb::CollSetJoints));
                }

                self.coll_rt_mut(node).other_joint_nodes.push(j);
                continue;
            }

            self.joint_set_collection(j, Some(node));
            self.coll_rt_mut(node).joint_nodes.push(j);
        }
    }

    /// `update ()`: one step of the simulation (the browser's `sensorEvents`).
    pub(super) fn coll_update(&mut self, node: NodeId) {
        let iterations = match self.pre_value(node, "iterations") {
            Some(FieldValue::SFInt32(v)) => *v,
            _ => 10,
        };
        let gravity = self.pv3(node, "gravity");

        self.coll_set_bounce(node);

        let bodies = self.coll_rt(node).body_nodes.clone();

        if self.get_bool(node, "preferAccuracy") {
            let delta_time = self.coll_time_step(node) / f64::from(iterations);

            for _ in 0..iterations {
                for &b in &bodies {
                    self.body_apply_forces(b, &gravity);
                }

                self.with_world(Some(node), |pw| {
                    pw.w.step_simulation(delta_time, 0, 0.0);
                });
            }
        } else {
            let delta_time = self.coll_time_step(node);

            for &b in &bodies {
                self.body_apply_forces(b, &gravity);
            }

            self.with_world(Some(node), |pw| {
                pw.w.step_simulation(
                    delta_time,
                    iterations + 2,
                    delta_time / f64::from(iterations),
                );
            });
        }

        for &b in &bodies {
            self.body_update(b);
        }
    }

    /// `dispose ()`: the world goes, the bodies that stay get their objects back.
    pub(super) fn collection_dispose(&mut self, node: NodeId) {
        if self.coll_rt(node).disposed {
            return;
        }

        let sensor = self.sensor_events_field();

        self.remove_interest(sensor, node, Cb::Physics(PhysicsCb::CollUpdate));

        // Bodies of nodes that stay: back into the nursery.
        for b in self.physics_bodies_of(node) {
            if !self.body_rt(b).disposed {
                self.body_migrate(b, None);
            }
        }

        let rt = self.coll_rt_mut(node);

        rt.disposed = true;
        rt.world = None;
        rt.rigid_bodies.clear();
        rt.constraint_ids.clear();
        rt.body_nodes.clear();
        rt.other_body_nodes.clear();
        rt.joint_nodes.clear();
        rt.other_joint_nodes.clear();
    }

    // --- CollisionCollection ----------------------------------------------------------------

    pub(super) fn cc_rt(&self, node: NodeId) -> &CollisionCollectionRt {
        match self.phys(node) {
            Some(PhysState::CollisionCollection(c)) => c,
            _ => panic!("node {node} is not a CollisionCollection"),
        }
    }

    fn cc_rt_mut(&mut self, node: NodeId) -> &mut CollisionCollectionRt {
        match self.phys_mut(node) {
            Some(PhysState::CollisionCollection(c)) => c,
            _ => panic!("node {node} is not a CollisionCollection"),
        }
    }

    pub(super) fn cc_initialize(&mut self, node: NodeId) {
        self.pcb(node, "appliedParameters", PhysicsCb::CcSetAppliedParameters);
        self.pcb(node, "collidables", PhysicsCb::CcSetCollidables);

        self.cc_set_applied_parameters(node);
        self.cc_set_collidables(node);
    }

    pub(super) fn cc_set_applied_parameters(&mut self, node: NodeId) {
        let mut set: Vec<i32> = vec![];

        for name in self.pstrings(node, "appliedParameters") {
            if let Some(v) = applied_parameter(&name) {
                if !set.contains(&v) {
                    set.push(v);
                }
            }
        }

        self.cc_rt_mut(node).applied_parameters = set;
    }

    pub(super) fn cc_set_collidables(&mut self, node: NodeId) {
        for s in std::mem::take(&mut self.cc_rt_mut(node).collision_space_nodes) {
            self.remove_node_interest(s, node, Cb::Physics(PhysicsCb::CcCollect));
        }

        let spaces = self.phys_cast_list(node, "collidables", is_space);

        self.cc_rt_mut(node).collision_space_nodes = spaces.clone();

        for s in spaces {
            self.add_node_interest(s, node, Cb::Physics(PhysicsCb::CcCollect));
        }

        self.cc_collect(node);
    }

    pub(super) fn cc_collect(&mut self, node: NodeId) {
        let mut collidables = vec![];

        for n in self.mfnode(node, "collidables") {
            if let Some(c) = self.phys_cast(n, is_collidable) {
                collidables.push(c);
                continue;
            }

            if let Some(s) = self.phys_cast(n, is_space) {
                collidables.extend(self.cs_rt(s).collidable_nodes.iter().copied());
            }
        }

        let rt = self.cc_rt_mut(node);

        rt.collidable_nodes = collidables;
        rt.collision_space_nodes.clear();

        self.add_node_event(node);
    }

    /// `getCollidables ()`.
    pub(super) fn cc_collidables(&self, node: NodeId) -> Vec<NodeId> {
        self.cc_rt(node).collidable_nodes.clone()
    }

    // --- CollisionSpace ---------------------------------------------------------------------

    fn cs_rt(&self, node: NodeId) -> &CollisionSpaceRt {
        match self.phys(node) {
            Some(PhysState::CollisionSpace(c)) => c,
            _ => panic!("node {node} is not a CollisionSpace"),
        }
    }

    fn cs_rt_mut(&mut self, node: NodeId) -> &mut CollisionSpaceRt {
        match self.phys_mut(node) {
            Some(PhysState::CollisionSpace(c)) => c,
            _ => panic!("node {node} is not a CollisionSpace"),
        }
    }

    pub(super) fn cs_initialize(&mut self, node: NodeId) {
        self.pcb(node, "collidables", PhysicsCb::CsSetCollidables);
        self.cs_set_collidables(node);
    }

    pub(super) fn cs_set_collidables(&mut self, node: NodeId) {
        for s in std::mem::take(&mut self.cs_rt_mut(node).collision_space_nodes) {
            self.remove_node_interest(s, node, Cb::Physics(PhysicsCb::CsCollect));
        }

        let spaces = self.phys_cast_list(node, "collidables", is_space);

        self.cs_rt_mut(node).collision_space_nodes = spaces.clone();

        for s in spaces {
            self.add_node_interest(s, node, Cb::Physics(PhysicsCb::CsCollect));
        }

        self.cs_collect(node);
    }

    pub(super) fn cs_collect(&mut self, node: NodeId) {
        let mut collidables = vec![];

        for n in self.mfnode(node, "collidables") {
            if let Some(c) = self.phys_cast(n, is_collidable) {
                collidables.push(c);
                continue;
            }

            if let Some(s) = self.phys_cast(n, is_space) {
                collidables.extend(self.cs_rt(s).collidable_nodes.iter().copied());
            }
        }

        let rt = self.cs_rt_mut(node);

        rt.collidable_nodes = collidables;
        rt.collision_space_nodes.clear();

        self.add_node_event(node);
    }
}
