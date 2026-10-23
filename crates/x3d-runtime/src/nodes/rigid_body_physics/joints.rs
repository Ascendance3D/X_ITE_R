//! X3DRigidJointNode and the six joints (X_ITE `X3DRigidJointNode.js`, `BallJoint.js`,
//! `SingleAxisHingeJoint.js`, `DoubleAxisHingeJoint.js`, `UniversalJoint.js`, `SliderJoint.js`,
//! `MotorJoint.js`). UniversalJoint and MotorJoint are not supported upstream: they have no
//! constraint, no outputs and no interests besides the base class's.

use super::*;
use crate::physics::bt::constraint::Constraint;
use crate::physics::bt::math::{Tf, V3, rt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JointKind {
    Ball,
    SingleAxisHinge,
    DoubleAxisHinge,
    Universal,
    Slider,
    Motor,
}

/// The members of an X3DRigidJointNode and its subclasses.
#[derive(Debug)]
pub struct JointRt {
    pub(super) kind: JointKind,
    /// `this .joint`: the collection whose dynamics world holds the constraint, and its id there.
    pub(super) constraint: Option<(NodeId, usize)>,
    /// `this .outputs`, in insertion order.
    pub(super) outputs: Vec<String>,
    pub(super) body_node1: Option<NodeId>,
    pub(super) body_node2: Option<NodeId>,
    pub(super) initial_inverse1: Matrix4,
    pub(super) initial_inverse2: Matrix4,
    pub(super) output: bool,
    pub(super) local_anchor1: Vector3,
    pub(super) local_anchor2: Vector3,
    pub(super) local_axis1: Vector3,
    pub(super) local_axis2: Vector3,
}

fn v3a(v: &Vector3) -> V3 {
    [v.x, v.y, v.z]
}

impl Runtime {
    pub(super) fn joint_rt(&self, node: NodeId) -> &JointRt {
        match self.phys(node) {
            Some(PhysState::Joint(j)) => j,
            _ => panic!("node {node} is not a joint"),
        }
    }

    pub(super) fn joint_rt_mut(&mut self, node: NodeId) -> &mut JointRt {
        match self.phys_mut(node) {
            Some(PhysState::Joint(j)) => j,
            _ => panic!("node {node} is not a joint"),
        }
    }

    pub(super) fn joint_construct(&mut self, node: NodeId, name: &str) {
        let kind = match name {
            "BallJoint" => JointKind::Ball,
            "SingleAxisHingeJoint" => JointKind::SingleAxisHinge,
            "DoubleAxisHingeJoint" => JointKind::DoubleAxisHinge,
            "UniversalJoint" => JointKind::Universal,
            "SliderJoint" => JointKind::Slider,
            _ => JointKind::Motor,
        };

        self.nrt_mut(node).state = State::Physics(PhysState::Joint(Box::new(JointRt {
            kind,
            constraint: None,
            outputs: vec![],
            body_node1: None,
            body_node2: None,
            initial_inverse1: Matrix4::IDENTITY,
            initial_inverse2: Matrix4::IDENTITY,
            output: false,
            local_anchor1: Vector3::ZERO,
            local_anchor2: Vector3::ZERO,
            local_axis1: Vector3::ZERO,
            local_axis2: Vector3::ZERO,
        })));
    }

    pub(super) fn joint_initialize(&mut self, node: NodeId) {
        use PhysicsCb::*;

        // X3DRigidJointNode.initialize
        self.pcb(node, "forceOutput", JointSetForceOutput);
        self.pcb(node, "body1", JointSetBody1);
        self.pcb(node, "body2", JointSetBody2);

        self.joint_set_force_output(node);
        self.joint_set_body(node, 1);
        self.joint_set_body(node, 2);

        match self.joint_rt(node).kind {
            JointKind::Ball => self.pcb(node, "anchorPoint", JointSetAnchorPoint),
            JointKind::SingleAxisHinge => {
                self.pcb(node, "anchorPoint", JointSetJoint);
                self.pcb(node, "axis", JointSetJoint);
            }
            JointKind::DoubleAxisHinge => {
                self.pcb(node, "anchorPoint", JointSetJoint);
                self.pcb(node, "axis1", JointSetJoint);
                self.pcb(node, "axis2", JointSetJoint);
            }
            JointKind::Slider => {
                self.pcb(node, "axis", JointSetJoint);
                self.pcb(node, "minSeparation", JointSetSeparation);
                self.pcb(node, "maxSeparation", JointSetSeparation);
            }
            JointKind::Universal | JointKind::Motor => {}
        }
    }

    /// `getCollection ()`.
    pub(super) fn joint_collection(&self, node: NodeId) -> Option<NodeId> {
        self.sfnode_of(self.phys_private(node, "collection"))
    }

    /// `setCollection (value)`.
    pub(super) fn joint_set_collection(&mut self, node: NodeId, value: Option<NodeId>) {
        self.joint_remove(node);

        let k = self.phys_private(node, "collection");

        self.set_value(k, FieldValue::SFNode(value));
        self.joint_add(node);
    }

    /// `set_joint__ ()`.
    pub(super) fn joint_set_joint(&mut self, node: NodeId) {
        self.joint_remove(node);
        self.joint_add(node);
    }

    /// `setOutput (value)`.
    fn joint_set_output(&mut self, node: NodeId, value: bool) {
        let (b1, b2) = {
            let j = self.joint_rt_mut(node);

            j.output = value;
            (j.body_node1, j.body_node2)
        };

        if value {
            if let Some(b) = b1 {
                self.add_node_interest(b, node, Cb::Physics(PhysicsCb::JointUpdate1));
            }

            if let Some(b) = b2 {
                self.add_node_interest(b, node, Cb::Physics(PhysicsCb::JointUpdate2));
            }
        } else {
            if let Some(b) = b1 {
                self.remove_node_interest(b, node, Cb::Physics(PhysicsCb::JointUpdate1));
            }

            if let Some(b) = b2 {
                self.remove_node_interest(b, node, Cb::Physics(PhysicsCb::JointUpdate2));
            }
        }
    }

    pub(super) fn joint_set_force_output(&mut self, node: NodeId) {
        let kind = self.joint_rt(node).kind;
        let all: &[&str] = match kind {
            JointKind::Ball => &["body1AnchorPoint", "body2AnchorPoint"],
            JointKind::SingleAxisHinge => &[
                "body1AnchorPoint",
                "body2AnchorPoint",
                "angle",
                "angularRate",
            ],
            JointKind::DoubleAxisHinge => &[
                "body1AnchorPoint",
                "body2AnchorPoint",
                "body1Axis",
                "body2Axis",
                "hinge1Angle",
                "hinge2Angle",
                "hinge1AngleRate",
                "hinge2AngleRate",
            ],
            JointKind::Slider => &["separation", "separationRate"],
            // The base class's set_forceOutput__ does nothing.
            JointKind::Universal | JointKind::Motor => return,
        };
        let mut outputs: Vec<String> = vec![];

        for value in self.pstrings(node, "forceOutput") {
            if value == "ALL" {
                for a in all {
                    if !outputs.iter().any(|o| o == a) {
                        outputs.push((*a).to_string());
                    }
                }
            } else if !outputs.contains(&value) {
                outputs.push(value);
            }
        }

        let any = !outputs.is_empty();

        self.joint_rt_mut(node).outputs = outputs;
        self.joint_set_output(node, any);
    }

    /// `set_body1__ ()` and `set_body2__ ()`.
    pub(super) fn joint_set_body(&mut self, node: NodeId, which: u8) {
        self.joint_remove(node);

        let (update_cb, old) = if which == 1 {
            (PhysicsCb::JointUpdate1, self.joint_rt(node).body_node1)
        } else {
            (PhysicsCb::JointUpdate2, self.joint_rt(node).body_node2)
        };

        if let Some(b) = old {
            self.remove_node_interest(b, node, Cb::Physics(update_cb));

            if let Some(k) = self.private_field(b, "collection") {
                self.remove_interest(k, node, Cb::Physics(PhysicsCb::JointSetJoint));
            }
        }

        let body = self.phys_cast_field(
            node,
            if which == 1 { "body1" } else { "body2" },
            is_rigid_body,
        );

        {
            let j = self.joint_rt_mut(node);

            if which == 1 {
                j.body_node1 = body;
            } else {
                j.body_node2 = body;
            }
        }

        if let Some(b) = body {
            if let Some(k) = self.private_field(b, "collection") {
                self.add_interest(k, node, Cb::Physics(PhysicsCb::JointSetJoint));
            }

            // initialize1 () / initialize2 ()
            let position = self.pv3(b, "position");
            let orientation = self.prot(b, "orientation");
            let mut m = transform_matrix(&position, &orientation);

            m.inverse();

            {
                let j = self.joint_rt_mut(node);

                if which == 1 {
                    j.initial_inverse1 = m;
                } else {
                    j.initial_inverse2 = m;
                }
            }

            self.joint_add(node);

            let output = self.joint_rt(node).output;

            self.joint_set_output(node, output);
        }
    }

    /// The collection and both bodies when `addJoint` gets past its guards.
    fn joint_ready(&self, node: NodeId) -> Option<(NodeId, NodeId, NodeId)> {
        let collection = self.joint_collection(node)?;
        let j = self.joint_rt(node);
        let (b1, b2) = (j.body_node1?, j.body_node2?);

        if self.body_collection(b1) != Some(collection)
            || self.body_collection(b2) != Some(collection)
        {
            return None;
        }

        Some((collection, b1, b2))
    }

    /// `addJoint ()`.
    pub(super) fn joint_add(&mut self, node: NodeId) {
        let kind = self.joint_rt(node).kind;

        if matches!(kind, JointKind::Universal | JointKind::Motor) {
            return;
        }

        let Some((collection, b1, b2)) = self.joint_ready(node) else {
            return;
        };
        let (body_a, body_b) = (self.body_rt(b1).body, self.body_rt(b2).body);
        let (inv1, inv2) = {
            let j = self.joint_rt(node);

            (j.initial_inverse1, j.initial_inverse2)
        };

        let constraint = match kind {
            JointKind::Ball => {
                let mut c = Constraint::point2point(body_a, body_b, [0.0; 3], [0.0; 3]);

                // set_anchorPoint__ ()
                let (p1, p2) = self.joint_ball_pivots(node);

                c.set_pivot_a(v3a(&p1));
                c.set_pivot_b(v3a(&p2));
                c
            }
            JointKind::SingleAxisHinge => {
                let anchor = self.pv3(node, "anchorPoint");
                let axis = self.pv3(node, "axis");
                let (mut a1, mut a2) = (anchor, anchor);
                let (mut x1, mut x2) = (axis, axis);

                inv1.mult_vec_matrix3(&mut a1);
                inv2.mult_vec_matrix3(&mut a2);
                inv1.mult_dir_matrix(&mut x1);
                x1.normalize();
                inv2.mult_dir_matrix(&mut x2);
                x2.normalize();

                // `localAnchorPoint1` is the member itself: it holds the converted anchor.
                {
                    let j = self.joint_rt_mut(node);

                    j.local_anchor1 = a1;
                    j.local_anchor2 = a2;
                }

                self.with_world(Some(collection), |pw| {
                    Constraint::hinge(
                        &pw.w.bodies,
                        body_a,
                        body_b,
                        v3a(&a1),
                        v3a(&a2),
                        v3a(&x1),
                        v3a(&x2),
                        false,
                    )
                })
            }
            JointKind::DoubleAxisHinge => {
                let anchor = self.pv3(node, "anchorPoint");
                let (mut a1, mut a2) = (anchor, anchor);
                let (mut x1, mut x2) = (self.pv3(node, "axis1"), self.pv3(node, "axis2"));

                inv1.mult_vec_matrix3(&mut a1);
                inv2.mult_vec_matrix3(&mut a2);
                inv1.mult_dir_matrix(&mut x1);
                x1.normalize();
                inv2.mult_dir_matrix(&mut x2);
                x2.normalize();

                self.with_world(Some(collection), |pw| {
                    Constraint::hinge(
                        &pw.w.bodies,
                        body_a,
                        body_b,
                        v3a(&a1),
                        v3a(&a2),
                        v3a(&x1),
                        v3a(&x2),
                        false,
                    )
                })
            }
            JointKind::Slider => {
                let axis = self.pv3(node, "axis");
                let mut axis_rotation = Rotation4::identity();

                axis_rotation.set_vectors(&Vector3::X_AXIS, &axis);

                let position = self.pv3(b1, "position");
                let mut orientation = self.prot(b1, "orientation");

                orientation.mult_right(&axis_rotation);

                let matrix_a = transform_matrix(&position, &orientation);
                let matrix_b = transform_matrix(&position, &orientation);
                // frameInA gets the basis of matrixA, then (a typo upstream) the basis of matrixB; frameInB
                // only gets an origin, its basis stays the identity.
                let frame_a = rt(Tf {
                    b: [
                        [matrix_b.0[0], matrix_b.0[4], matrix_b.0[8]],
                        [matrix_b.0[1], matrix_b.0[5], matrix_b.0[9]],
                        [matrix_b.0[2], matrix_b.0[6], matrix_b.0[10]],
                    ],
                    o: [matrix_a.0[12], matrix_a.0[13], matrix_a.0[14]],
                });
                let frame_b = rt(Tf {
                    b: [[0.0; 3]; 3],
                    o: [matrix_b.0[12], matrix_b.0[13], matrix_b.0[14]],
                });
                let (lower, upper) = (
                    self.get_f64(node, "minSeparation"),
                    self.get_f64(node, "maxSeparation"),
                );
                let mut c = self.with_world(Some(collection), |pw| {
                    Constraint::slider(&pw.w.bodies, body_a, body_b, frame_a, frame_b, true)
                });

                c.set_lower_ang_limit(0.0);
                c.set_upper_ang_limit(0.0);
                // set_separation__ ()
                c.set_lower_lin_limit(lower);
                c.set_upper_lin_limit(upper);
                c
            }
            JointKind::Universal | JointKind::Motor => return,
        };

        let id = self.with_world(Some(collection), |pw| pw.w.add_constraint(constraint, true));

        self.joint_rt_mut(node).constraint = Some((collection, id));
        self.coll_rt_mut(collection).constraint_ids.push(id);
    }

    /// `removeJoint ()`.
    pub(super) fn joint_remove(&mut self, node: NodeId) {
        let Some((collection, id)) = self.joint_rt_mut(node).constraint.take() else {
            return;
        };

        if matches!(self.phys(collection), Some(PhysState::Collection(rt)) if rt.world.is_some()) {
            self.with_world(Some(collection), |pw| pw.w.remove_constraint(id));
            self.coll_rt_mut(collection)
                .constraint_ids
                .retain(|&c| c != id);
        }
    }

    /// BallJoint `set_anchorPoint__ ()`: the pivots in the bodies' initial local frames.
    fn joint_ball_pivots(&mut self, node: NodeId) -> (Vector3, Vector3) {
        let anchor = self.pv3(node, "anchorPoint");
        let (mut p1, mut p2) = (anchor, anchor);
        let j = self.joint_rt_mut(node);

        j.initial_inverse1.mult_vec_matrix3(&mut p1);
        j.initial_inverse2.mult_vec_matrix3(&mut p2);
        j.local_anchor1 = p1;
        j.local_anchor2 = p2;
        (p1, p2)
    }

    pub(super) fn joint_set_anchor_point(&mut self, node: NodeId) {
        let Some((collection, id)) = self.joint_rt(node).constraint else {
            return;
        };
        let (p1, p2) = self.joint_ball_pivots(node);

        self.with_world(Some(collection), |pw| {
            pw.w.joints[id].set_pivot_a(v3a(&p1));
            pw.w.joints[id].set_pivot_b(v3a(&p2));
        });
    }

    pub(super) fn joint_set_separation(&mut self, node: NodeId) {
        let Some((collection, id)) = self.joint_rt(node).constraint else {
            return;
        };
        let (lower, upper) = (
            self.get_f64(node, "minSeparation"),
            self.get_f64(node, "maxSeparation"),
        );

        self.with_world(Some(collection), |pw| {
            pw.w.joints[id].set_lower_lin_limit(lower);
            pw.w.joints[id].set_upper_lin_limit(upper);
        });
    }

    fn joint_has(&self, node: NodeId, output: &str) -> bool {
        self.joint_rt(node).outputs.iter().any(|o| o == output)
    }

    /// `update1 ()` and `update2 ()`: the body moved (`which` names the body).
    pub(super) fn joint_update(&mut self, node: NodeId, which: u8) {
        let kind = self.joint_rt(node).kind;
        let body = if which == 1 {
            self.joint_rt(node).body_node1
        } else {
            self.joint_rt(node).body_node2
        };
        let Some(body) = body else {
            return;
        };
        let body_matrix = self.body_rt(body).matrix;
        let (inv, local_anchor, local_axis) = {
            let j = self.joint_rt(node);

            if which == 1 {
                (j.initial_inverse1, j.local_anchor1, j.local_axis1)
            } else {
                (j.initial_inverse2, j.local_anchor2, j.local_axis2)
            }
        };
        let (anchor_name, axis_name, angle_name, rate_name) = if which == 1 {
            (
                "body1AnchorPoint",
                "body1Axis",
                "hinge1Angle",
                "hinge1AngleRate",
            )
        } else {
            (
                "body2AnchorPoint",
                "body2Axis",
                "hinge2Angle",
                "hinge2AngleRate",
            )
        };

        // body .getMatrix () .multVecMatrix (initialInverseMatrix .multVecMatrix (localAnchorPoint))
        let anchor = || {
            let mut v = local_anchor;

            inv.mult_vec_matrix3(&mut v);
            body_matrix.mult_vec_matrix3(&mut v);
            v
        };
        let angle_of = || {
            let mut difference = inv;
            let mut rotation = Rotation4::identity();

            difference.mult_right(&body_matrix);
            difference.get_transform(None, Some(&mut rotation), None, None, None);
            rotation.angle()
        };

        match kind {
            JointKind::Ball => {
                if self.joint_has(node, anchor_name) {
                    self.set_v3(node, anchor_name, anchor());
                }
            }
            JointKind::SingleAxisHinge => {
                if self.joint_has(node, anchor_name) {
                    self.set_v3(node, anchor_name, anchor());
                }

                // Only body 2 reports the angle.
                if which == 2 && self.joint_has(node, "angle") {
                    let last_angle = self.get_f64(node, "angle");
                    let angle = angle_of();

                    self.set_f(node, "angle", angle);

                    if self.joint_has(node, "angleRate") {
                        let rate = (self.get_f64(node, "angle") - last_angle) * self.frame_rate;

                        self.set_f(node, "angleRate", rate);
                    }
                }
            }
            JointKind::DoubleAxisHinge => {
                if self.joint_has(node, anchor_name) {
                    self.set_v3(node, anchor_name, anchor());
                }

                if self.joint_has(node, axis_name) {
                    // initialInverseMatrix .multDirMatrix (body .getMatrix () .multDirMatrix (localAxis)) .normalize ()
                    let mut v = local_axis;

                    body_matrix.mult_dir_matrix(&mut v);
                    inv.mult_dir_matrix(&mut v);
                    v.normalize();
                    self.set_v3(node, axis_name, v);
                }

                if self.joint_has(node, angle_name) {
                    let last_angle = self.get_f64(node, angle_name);
                    let angle = angle_of();

                    self.set_f(node, angle_name, angle);

                    if self.joint_has(node, "angleRate") {
                        let rate = (self.get_f64(node, angle_name) - last_angle) * self.frame_rate;

                        self.set_f(node, rate_name, rate);
                    }
                }
            }
            // SliderJoint, UniversalJoint and MotorJoint have no update1 or update2.
            _ => {}
        }
    }

    /// `dispose ()`.
    pub(super) fn joint_dispose(&mut self, node: NodeId) {
        self.joint_remove(node);

        let (b1, b2) = {
            let j = self.joint_rt(node);

            (j.body_node1, j.body_node2)
        };

        for (b, cb) in [(b1, PhysicsCb::JointUpdate1), (b2, PhysicsCb::JointUpdate2)] {
            if let Some(b) = b {
                self.remove_node_interest(b, node, Cb::Physics(cb));

                if let Some(k) = self.private_field(b, "collection") {
                    self.remove_interest(k, node, Cb::Physics(PhysicsCb::JointSetJoint));
                }
            }
        }

        let j = self.joint_rt_mut(node);

        j.body_node1 = None;
        j.body_node2 = None;
        j.outputs.clear();
    }
}
