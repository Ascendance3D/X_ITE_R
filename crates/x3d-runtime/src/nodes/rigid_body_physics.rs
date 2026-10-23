//! The RigidBodyPhysics component (X_ITE `Components/RigidBodyPhysics/*.js`, 14 nodes), ported
//! one to one: RigidBodyCollection, RigidBody, CollidableShape, CollidableOffset, CollisionCollection,
//! CollisionSpace, CollisionSensor, Contact, BallJoint, SingleAxisHingeJoint, DoubleAxisHingeJoint,
//! UniversalJoint, SliderJoint and MotorJoint.
//!
//! Every method of the JavaScript classes is a method here (`set_gravity__` is `coll_set_gravity`,
//! and so on), registered as an interest on the same field, node or browser event phase, in the same
//! order. The dynamics are the Bullet 2.82 port of `physics::bt`, called the way X_ITE calls Ammo
//! (`crates/x3d-runtime/tests/bullet_dyn_scenes.rs` replays the same call sequences against recorded
//! Ammo output). The RigidBodyCollection hooks `update` onto the browser's `sensorEvents` while it
//! is live and enabled (`set_enabled__`), exactly like X_ITE.
//!
//! Where the Ammo objects live is described in [`arena`]. Per-node state is a [`PhysState`] in
//! the node's runtime slot ([`State::Physics`]), so it goes with the node when the scene is released
//! (`physics_release` takes the bodies and constraints out of their worlds first).
//!
//! Quirks of X_ITE that are reproduced on purpose (see `oracle/tools/gen_rea_b06.py`): UniversalJoint
//! and MotorJoint make no constraint; Contact never gets geometry1/geometry2/body1/body2 (the sensor
//! sets `geometry1_`); a CollisionSensor keeps its old contact and intersection lists on frames
//! without contacts; its `enabled` field has no interest (only the live state and `collider` do); the
//! collection sets restitution on every body, fixed ones too, from the linear speed; the inertia
//! field is overridden by `calculateLocalInertia`; bodies never sleep because `RigidBody .update`
//! writes the velocity fields (which activates the body) each frame; hinge `minAngle` and `maxAngle`
//! are not applied, `angleRate` of a SingleAxisHingeJoint under ALL is never written (the key is
//! `angularRate`); a DoubleAxisHingeJoint reports angles only for `hinge1Angle`/`hinge2Angle` and rates
//! only for `angleRate`; a SliderJoint never writes `separation` or `separationRate`; BallJoint's
//! `body1AnchorPoint` multiplies the initial inverse matrix twice; CollidableShape has no Capsule.

pub mod arena;
mod body;
mod collection;
mod collidable;
mod joints;
mod sensor;

use crate::nodes::State;
use crate::*;
pub use arena::{Home, PWorld, WorldCounts};
use std::collections::HashSet;
use x3d_fields::math::{Matrix4, Rotation4, Vector3};

pub use body::BodyRt;
pub use collection::{CollectionRt, CollisionCollectionRt, CollisionSpaceRt};
pub use collidable::CollidableRt;
pub use joints::{JointKind, JointRt};
pub use sensor::SensorRt;

/// Interest callbacks of the RigidBodyPhysics nodes. The interest's node is the object (`this`) of
/// `addInterest (callback, this)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PhysicsCb {
    // RigidBodyCollection
    CollSetEnabled,
    CollSetGravity,
    CollSetContactSurfaceThickness,
    CollSetCollider,
    CollSetColliderParameters,
    CollSetBodies,
    CollSetJoints,
    CollSetDynamicsWorld,
    CollUpdate,
    // RigidBody
    BodySetLinearVelocity,
    BodySetAngularVelocity,
    BodySetFiniteRotationAxis,
    BodySetDamping,
    BodySetForces,
    BodySetTorques,
    BodySetDisable,
    BodySetGeometry,
    BodySetMassProps,
    BodySetTransform,
    BodySetCompoundShape,
    BodySetBody,
    /// `geometryNode .addInterest ("addEvent", this ._transform)`.
    BodyTransformEvent,
    // X3DNBodyCollidableNode, CollidableShape, CollidableOffset
    CollidableSetChild,
    CollidableEventsProcessed,
    CollidableSetCollidableGeometry,
    CollidableRequestRebuild,
    CollidableSetGeometry,
    /// `collidableNode .addInterest ("addNodeEvent", this)` of a CollidableOffset.
    CollidableChildNodeEvent,
    // CollisionCollection, CollisionSpace
    CcSetAppliedParameters,
    CcSetCollidables,
    CcCollect,
    CsSetCollidables,
    CsCollect,
    // CollisionSensor
    SensorSetCollider,
    SensorUpdate,
    // X3DRigidJointNode and the joints
    JointSetForceOutput,
    JointSetBody1,
    JointSetBody2,
    JointSetJoint,
    JointUpdate1,
    JointUpdate2,
    JointSetAnchorPoint,
    JointSetSeparation,
}

/// The per-node state of the RigidBodyPhysics nodes.
#[derive(Debug)]
pub enum PhysState {
    Collection(Box<CollectionRt>),
    Body(Box<BodyRt>),
    Collidable(Box<CollidableRt>),
    CollisionCollection(Box<CollisionCollectionRt>),
    CollisionSpace(Box<CollisionSpaceRt>),
    Sensor(Box<SensorRt>),
    Joint(Box<JointRt>),
    /// Contact has no behavior.
    Contact,
}

static RECORD_DT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Runtimes made from now on record every `getTimeStep ()` result ([`Runtime::take_physics_dt_log`]).
pub fn record_time_steps(on: bool) {
    RECORD_DT.store(on, std::sync::atomic::Ordering::Relaxed);
}

impl Default for PhysicsRt {
    fn default() -> Self {
        PhysicsRt {
            nursery: None,
            record_dt: RECORD_DT.load(std::sync::atomic::Ordering::Relaxed),
            dt_log: vec![],
            geometry_watch: HashSet::new(),
            geometry_rebuilt: HashSet::new(),
        }
    }
}

/// What the runtime keeps for the component besides the node states.
#[derive(Debug)]
pub struct PhysicsRt {
    /// The Ammo objects of nodes that no RigidBodyCollection has taken (see [`arena`]).
    nursery: Option<Box<PWorld>>,
    /// Record the result of every `RigidBodyCollection .getTimeStep ()` in [`PhysicsRt::dt_log`].
    pub record_dt: bool,
    pub dt_log: Vec<f64>,
    /// Sphere, Cone and Cylinder nodes below a CollidableShape (X_ITE: `connectOptions` in their
    /// `set_live__`): the first time one becomes live its `_rebuild` fires, once, and the CollidableShape
    /// makes its collision shape again.
    geometry_watch: HashSet<NodeId>,
    geometry_rebuilt: HashSet<NodeId>,
}

/// X_ITE type names handled here.
pub fn is_physics_node(name: &str) -> bool {
    matches!(
        name,
        "RigidBodyCollection"
            | "RigidBody"
            | "CollidableShape"
            | "CollidableOffset"
            | "CollisionCollection"
            | "CollisionSpace"
            | "CollisionSensor"
            | "Contact"
            | "BallJoint"
            | "SingleAxisHingeJoint"
            | "DoubleAxisHingeJoint"
            | "UniversalJoint"
            | "SliderJoint"
            | "MotorJoint"
    )
}

pub(crate) fn is_collidable(name: &str) -> bool {
    matches!(name, "CollidableShape" | "CollidableOffset")
}

pub(crate) fn is_joint(name: &str) -> bool {
    matches!(
        name,
        "BallJoint"
            | "SingleAxisHingeJoint"
            | "DoubleAxisHingeJoint"
            | "UniversalJoint"
            | "SliderJoint"
            | "MotorJoint"
    )
}

pub(crate) fn is_space(name: &str) -> bool {
    name == "CollisionSpace"
}

pub(crate) fn is_rigid_body(name: &str) -> bool {
    name == "RigidBody"
}

pub(crate) fn is_collision_collection(name: &str) -> bool {
    name == "CollisionCollection"
}

pub(crate) fn is_shape(name: &str) -> bool {
    name == "Shape"
}

impl Runtime {
    // --- access helpers ---------------------------------------------------------------------

    pub(crate) fn phys(&self, node: NodeId) -> Option<&PhysState> {
        match &self.nrt(node)?.state {
            State::Physics(p) => Some(p),
            _ => None,
        }
    }

    pub(crate) fn phys_mut(&mut self, node: NodeId) -> Option<&mut PhysState> {
        match &mut self.nrt_mut(node).state {
            State::Physics(p) => Some(p),
            _ => None,
        }
    }

    /// `X3DCast (type, node)`: the inner node when its (builtin) type name passes `test`.
    pub(crate) fn phys_cast(&self, node: Option<NodeId>, test: fn(&str) -> bool) -> Option<NodeId> {
        let inner = self.inner_node(node?)?;

        self.builtin_name(inner).filter(|n| test(n)).map(|_| inner)
    }

    /// `X3DCast (type, this ._field)`.
    pub(crate) fn phys_cast_field(
        &self,
        node: NodeId,
        field: &str,
        test: fn(&str) -> bool,
    ) -> Option<NodeId> {
        let value = match self.pre_value(node, field) {
            Some(FieldValue::SFNode(n)) => *n,
            _ => None,
        };

        self.phys_cast(value, test)
    }

    pub(crate) fn phys_cast_list(
        &self,
        node: NodeId,
        field: &str,
        test: fn(&str) -> bool,
    ) -> Vec<NodeId> {
        self.mfnode(node, field)
            .into_iter()
            .filter_map(|n| self.phys_cast(n, test))
            .collect()
    }

    pub(crate) fn pv3(&self, node: NodeId, name: &str) -> Vector3 {
        match self.pre_value(node, name) {
            Some(FieldValue::SFVec3f(v) | FieldValue::SFVec3d(v)) => *v,
            _ => Vector3::ZERO,
        }
    }

    pub(crate) fn prot(&self, node: NodeId, name: &str) -> Rotation4 {
        match self.pre_value(node, name) {
            Some(FieldValue::SFRotation(r)) => *r,
            _ => Rotation4::identity(),
        }
    }

    pub(crate) fn pstrings(&self, node: NodeId, name: &str) -> Vec<String> {
        match self.pre_value(node, name) {
            Some(FieldValue::MFString(v)) => v.clone(),
            _ => vec![],
        }
    }

    pub(crate) fn pmf_vec3(&self, node: NodeId, name: &str) -> Vec<Vector3> {
        match self.pre_value(node, name) {
            Some(FieldValue::MFVec3f(v)) => v
                .chunks_exact(3)
                .map(|c| Vector3::new(f64::from(c[0]), f64::from(c[1]), f64::from(c[2])))
                .collect(),
            Some(FieldValue::MFVec3d(v)) => v
                .chunks_exact(3)
                .map(|c| Vector3::new(c[0], c[1], c[2]))
                .collect(),
            _ => vec![],
        }
    }

    /// `this ._field = vector` for a predefined SFVec3f field.
    pub(crate) fn set_v3(&mut self, node: NodeId, name: &str, v: Vector3) {
        if let Some(k) = self.pre(node, name) {
            self.set_value(k, FieldValue::SFVec3f(v));
        }
    }

    pub(crate) fn set_f(&mut self, node: NodeId, name: &str, v: f64) {
        if let Some(k) = self.pre(node, name) {
            self.set_value(k, FieldValue::SFFloat(v));
        }
    }

    /// The value of a node-typed private or predefined SFNode field.
    pub(crate) fn sfnode_of(&self, k: FKey) -> Option<NodeId> {
        match self.value(k) {
            FieldValue::SFNode(n) => *n,
            _ => None,
        }
    }

    pub(crate) fn phys_private(&self, node: NodeId, name: &str) -> FKey {
        self.private_field(node, name)
            .unwrap_or_else(|| panic!("private field {name} of node {node}"))
    }

    pub(crate) fn pcb(&mut self, node: NodeId, field: &str, cb: PhysicsCb) {
        if let Some(k) = self.pre(node, field) {
            self.add_interest(k, node, Cb::Physics(cb));
        }
    }

    /// The browser's `sensorEvents` event field.
    pub(crate) fn sensor_events_field(&self) -> FKey {
        self.private_field(self.browser_node, "sensorEvents")
            .expect("browser sensorEvents")
    }

    /// `getLive () .getValue ()` of the node.
    pub(crate) fn phys_live(&self, node: NodeId) -> bool {
        self.ctx_live(self.scene.node(node).ctx)
    }

    // --- worlds -------------------------------------------------------------------------------

    /// Run `f` on the world `home` names. The world is taken out of its owner while `f` runs, so `f`
    /// must not ask for the same world again.
    pub(crate) fn with_world<R>(&mut self, home: Home, f: impl FnOnce(&mut PWorld) -> R) -> R {
        match home {
            None => {
                let mut w = self
                    .physics
                    .nursery
                    .take()
                    .unwrap_or_else(|| Box::new(PWorld::new()));
                let r = f(&mut w);

                self.physics.nursery = Some(w);
                r
            }
            Some(c) => {
                let mut w = match self.phys_mut(c) {
                    Some(PhysState::Collection(rt)) => {
                        rt.world.take().expect("the dynamics world is in use")
                    }
                    _ => panic!("node {c} is not a RigidBodyCollection"),
                };
                let r = f(&mut w);

                if let Some(PhysState::Collection(rt)) = self.phys_mut(c) {
                    rt.world = Some(w);
                }

                r
            }
        }
    }

    // --- construct, initialize, interests ------------------------------------------------------

    /// The constructors (`new Ammo .btCompoundShape ()` ..., `addChildObjects`).
    pub(crate) fn physics_construct(&mut self, node: NodeId, name: &str) {
        use x3d_fields::access;

        match name {
            "RigidBodyCollection" => self.collection_construct(node),
            "RigidBody" => {
                self.add_private_field(
                    node,
                    "collection",
                    access::INPUT_OUTPUT,
                    FieldValue::SFNode(None),
                );
                self.add_private_field(
                    node,
                    "transform",
                    access::OUTPUT_ONLY,
                    FieldValue::SFTime(0.0),
                );
                self.add_private_field(
                    node,
                    "otherGeometry",
                    access::OUTPUT_ONLY,
                    FieldValue::MFNode(vec![]),
                );
                self.body_construct(node);
            }
            "CollidableShape" | "CollidableOffset" => {
                self.add_private_field(node, "body", access::OUTPUT_ONLY, FieldValue::SFNode(None));
                self.add_private_field(
                    node,
                    "compoundShape",
                    access::OUTPUT_ONLY,
                    FieldValue::SFTime(0.0),
                );
                self.add_private_field(
                    node,
                    "rebuild",
                    access::OUTPUT_ONLY,
                    FieldValue::SFTime(0.0),
                );
                self.collidable_construct(node);
            }
            "CollisionCollection" => {
                self.nrt_mut(node).state =
                    State::Physics(PhysState::CollisionCollection(Default::default()))
            }
            "CollisionSpace" => {
                self.nrt_mut(node).state =
                    State::Physics(PhysState::CollisionSpace(Default::default()))
            }
            "CollisionSensor" => {
                self.nrt_mut(node).state = State::Physics(PhysState::Sensor(Default::default()))
            }
            "Contact" => self.nrt_mut(node).state = State::Physics(PhysState::Contact),
            n if is_joint(n) => {
                self.add_private_field(
                    node,
                    "collection",
                    access::INPUT_OUTPUT,
                    FieldValue::SFNode(None),
                );
                self.joint_construct(node, n);
            }
            _ => {}
        }
    }

    /// The `initialize ()` of the node classes.
    pub(crate) fn physics_initialize(&mut self, node: NodeId, name: &str) {
        match name {
            "RigidBodyCollection" => self.collection_initialize(node),
            "RigidBody" => self.body_initialize(node),
            "CollidableShape" | "CollidableOffset" => self.collidable_initialize(node, name),
            "CollisionCollection" => self.cc_initialize(node),
            "CollisionSpace" => self.cs_initialize(node),
            "CollisionSensor" => self.sensor_initialize(node),
            n if is_joint(n) => self.joint_initialize(node),
            _ => {}
        }
    }

    /// Dispatch of an interest callback.
    pub(crate) fn physics_interest(&mut self, node: NodeId, cb: PhysicsCb) {
        use PhysicsCb::*;

        match cb {
            CollSetEnabled => self.coll_set_enabled(node),
            CollSetGravity => self.coll_set_gravity(node),
            CollSetContactSurfaceThickness => self.coll_set_contact_surface_thickness(node),
            CollSetCollider => self.coll_set_collider(node),
            CollSetColliderParameters => self.coll_set_collider_parameters(node),
            CollSetBodies => self.coll_set_bodies(node),
            CollSetJoints => self.coll_set_joints(node),
            CollSetDynamicsWorld => self.coll_set_dynamics_world(node),
            CollUpdate => self.coll_update(node),
            BodySetLinearVelocity => self.body_set_linear_velocity(node),
            BodySetAngularVelocity => self.body_set_angular_velocity(node),
            BodySetFiniteRotationAxis => self.body_set_finite_rotation_axis(node),
            BodySetDamping => self.body_set_damping(node),
            BodySetForces => self.body_set_forces(node),
            BodySetTorques => self.body_set_torques(node),
            BodySetDisable => self.body_set_disable(node),
            BodySetGeometry => self.body_set_geometry(node),
            BodySetMassProps => self.body_set_mass_props(node),
            BodySetTransform => self.body_set_transform(node),
            BodySetCompoundShape => self.body_set_compound_shape(node),
            BodySetBody => self.body_set_body(node),
            BodyTransformEvent => {
                let t = self.phys_private(node, "transform");

                self.add_event(t);
            }
            CollidableSetChild => self.collidable_set_child(node),
            CollidableEventsProcessed => self.collidable_events_processed(node),
            CollidableSetCollidableGeometry => self.collidable_set_collidable_geometry(node),
            CollidableRequestRebuild => self.collidable_request_rebuild(node),
            CollidableSetGeometry => self.collidable_set_geometry(node),
            CollidableChildNodeEvent => self.add_node_event(node),
            CcSetAppliedParameters => self.cc_set_applied_parameters(node),
            CcSetCollidables => self.cc_set_collidables(node),
            CcCollect => self.cc_collect(node),
            CsSetCollidables => self.cs_set_collidables(node),
            CsCollect => self.cs_collect(node),
            SensorSetCollider => self.sensor_set_collider(node),
            SensorUpdate => self.sensor_update(node),
            JointSetForceOutput => self.joint_set_force_output(node),
            JointSetBody1 => self.joint_set_body(node, 1),
            JointSetBody2 => self.joint_set_body(node, 2),
            JointSetJoint => self.joint_set_joint(node),
            JointUpdate1 => self.joint_update(node, 1),
            JointUpdate2 => self.joint_update(node, 2),
            JointSetAnchorPoint => self.joint_set_anchor_point(node),
            JointSetSeparation => self.joint_set_separation(node),
        }
    }

    /// `set_live__` of the nodes that watch their live state (RigidBodyCollection, CollisionSensor).
    pub(crate) fn physics_set_live(&mut self, node: NodeId) {
        match self.builtin_name(node) {
            Some("RigidBodyCollection") => self.coll_set_enabled(node),
            Some("CollisionSensor") => self.sensor_set_live(node),
            Some("Sphere" | "Cone" | "Cylinder") => {
                // X3DGeometryNode.connectOptions: `options .getModificationTime () >= this ._rebuild` holds
                // the first time the geometry is live (its `_rebuild` is still 0).
                if self.time_node_live(node) && self.physics.geometry_rebuilt.insert(node) {
                    self.add_node_event(node);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn physics_watches_live(&self, node: NodeId) -> bool {
        matches!(
            self.builtin_name(node),
            Some("RigidBodyCollection" | "CollisionSensor")
        ) || self.physics.geometry_watch.contains(&node)
    }

    // --- release ------------------------------------------------------------------------------

    /// The nodes in `dead` are about to be released: take their bodies and constraints out of the
    /// worlds that survive, give their Ammo objects back and forget them (X_ITE's `dispose`). Runs before
    /// the node slots are emptied.
    pub(crate) fn physics_release(&mut self, dead: &HashSet<NodeId>) {
        self.physics.geometry_watch.retain(|n| !dead.contains(n));
        self.physics.geometry_rebuilt.retain(|n| !dead.contains(n));

        let mut nodes: Vec<NodeId> = dead
            .iter()
            .copied()
            .filter(|&n| self.phys(n).is_some())
            .collect();

        if nodes.is_empty() {
            return;
        }

        nodes.sort_unstable();

        // Joints first (their constraints live in the worlds of the collections), then bodies,
        // collidables, and last the collections (their worlds are dropped with the state).
        for &n in &nodes {
            if matches!(self.phys(n), Some(PhysState::Joint(_))) {
                self.joint_dispose(n);
            }
        }

        for &n in &nodes {
            if matches!(self.phys(n), Some(PhysState::Body(_))) {
                self.body_dispose(n, dead);
            }
        }

        for &n in &nodes {
            if matches!(self.phys(n), Some(PhysState::Collidable(_))) {
                self.collidable_dispose(n);
            }
        }

        for &n in &nodes {
            if matches!(self.phys(n), Some(PhysState::Collection(_))) {
                self.collection_dispose(n);
            }
        }
    }

    // --- diagnostics -------------------------------------------------------------------------

    /// The counts of the arenas of the world `home` names, with the bodies and constraints that are in
    /// the dynamics world.
    pub fn physics_world_counts(&mut self, home: Home) -> WorldCounts {
        let mut counts = self.with_world(home, |w| w.counts());

        if let Some(c) = home {
            if let Some(PhysState::Collection(rt)) = self.phys(c) {
                counts.in_world = rt.rigid_bodies.len();
                counts.constraints = rt.live_constraints();
            }
        }

        counts
    }

    /// The Ammo objects the runtime still holds for nodes that no collection has taken.
    pub fn physics_nursery_counts(&mut self) -> WorldCounts {
        self.physics_world_counts(None)
    }

    /// Number of nodes that hold RigidBodyPhysics state.
    pub fn physics_state_nodes(&self) -> usize {
        self.nrt
            .iter()
            .filter(|n| matches!(n.state, State::Physics(_)))
            .count()
    }

    /// `btRigidBody .isActive ()` of a RigidBody node.
    pub fn physics_body_active(&mut self, node: NodeId) -> Option<bool> {
        let (home, body) = match self.phys(node)? {
            PhysState::Body(b) => (b.home, b.body),
            _ => return None,
        };

        Some(self.with_world(home, |w| w.w.body(body).is_active()))
    }

    /// The RigidBody nodes whose Ammo body lives in the world of `collection`.
    pub fn physics_bodies_of(&self, collection: NodeId) -> Vec<NodeId> {
        (0..self.nrt.len() as NodeId)
            .filter(
                |&n| matches!(self.phys(n), Some(PhysState::Body(b)) if b.home == Some(collection)),
            )
            .collect()
    }

    /// The `getTimeStep ()` results recorded since the last call.
    pub fn take_physics_dt_log(&mut self) -> Vec<f64> {
        std::mem::take(&mut self.physics.dt_log)
    }

    /// Interests of the RigidBodyPhysics that name a node without physics state (retention check).
    pub fn physics_stale_interests(&self) -> Vec<NodeId> {
        let mut out = vec![];

        for f in self.frt.values() {
            out.extend(
                f.interests
                    .iter()
                    .filter(|i| matches!(i.cb, Cb::Physics(_)) && self.phys(i.node).is_none())
                    .map(|i| i.node),
            );
        }

        for n in self.nrt.iter() {
            out.extend(
                n.interests
                    .iter()
                    .filter(|i| matches!(i.cb, Cb::Physics(_)) && self.phys(i.node).is_none())
                    .map(|i| i.node),
            );
        }

        out
    }
}

/// `Matrix4 .setTransform (position, orientation)`.
pub(crate) fn transform_matrix(position: &Vector3, orientation: &Rotation4) -> Matrix4 {
    let mut m = Matrix4::IDENTITY;

    m.set_transform(Some(position), Some(orientation), None, None, None);
    m
}
