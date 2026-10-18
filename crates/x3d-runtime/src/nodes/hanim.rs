//! Components/HAnim runtime parts (REA batch 04): HAnimHumanoid's pose, motion and joint
//! bookkeeping, HAnimPose and HAnimMotion with their private TimeSensor and interpolators, and the
//! humanoid's update Lock as a count of unlocks (x3d-render reads it to know when to make the joint
//! matrices again). The skin tables and the skinning are in x3d-render (hanim.rs).

use crate::*;
use x3d_fields::math::{Rotation4, Vector3};

#[derive(Clone, Debug, Default)]
pub struct HumanoidState {
    /// jointNodes: the HAnimJoint nodes of `joints`.
    pub joints: Vec<NodeId>,
    /// poseNodes and motionNodes.
    pub poses: Vec<NodeId>,
    pub motions: Vec<NodeId>,
    /// skinCoordNode.
    pub skin_coord: Option<NodeId>,
    /// update.unlock () calls so far.
    pub unlocks: u64,
}

#[derive(Clone, Debug, Default)]
pub struct PoseState {
    /// `joints`: the humanoids whose jointNodes were added (a Set, in add order).
    pub humanoids: Vec<NodeId>,
    pub interpolators: Vec<NodeId>,
    /// poseJointNodes: name → HAnimJoint child.
    pub pose_joints: Vec<(String, NodeId)>,
    pub needs_update: bool,
}

#[derive(Clone, Debug, Default)]
pub struct MotionState {
    /// `joints` split into names.
    pub joints: Vec<String>,
    /// Per channel group: translation, rotation, scale interpolators.
    pub interpolators: Vec<[Option<NodeId>; 3]>,
    /// startFrame and endFrame as set_start_or_endFrame__ keeps them (undefined before).
    pub start_frame: Option<i32>,
    pub end_frame: Option<i32>,
}

const JOINT_FIELDS: [&str; 3] = ["translation", "rotation", "scale"];

/// JavaScript `\s`.
fn js_space(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

fn separator(c: char) -> bool {
    js_space(c) || c == ','
}

/// `string .split (/[\s,]+/s)`.
fn split_separators(s: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut in_run = false;

    for c in s.chars() {
        if separator(c) {
            if !in_run {
                out.push(String::new());
                in_run = true;
            }
        } else {
            out.last_mut().unwrap().push(c);
            in_run = false;
        }
    }

    out
}

/// HAnimMotion channels: `.replace (/^[\s,\d]+|[\s,\d]+$/sg, "") .split (/[\s,]+\d+[\s,]+/s)
/// .map (string => string .split (/[\s,]+/s))`.
fn parse_channels(s: &str) -> Vec<Vec<String>> {
    let trim = |c: char| separator(c) || c.is_ascii_digit();
    let s: Vec<char> = s.trim_start_matches(trim).trim_end_matches(trim).chars().collect();
    let mut parts = vec![];
    let mut start = 0;
    let mut i = 0;

    while i < s.len() {
        let run = |from: usize, f: &dyn Fn(char) -> bool| -> usize {
            let mut k = from;
            while k < s.len() && f(s[k]) {
                k += 1;
            }
            k
        };
        let j = run(i, &separator);
        let k = if j > i { run(j, &|c: char| c.is_ascii_digit()) } else { j };
        let m = if k > j { run(k, &separator) } else { k };

        if j > i && k > j && m > k {
            parts.push(s[start..i].iter().collect::<String>());
            start = m;
            i = m;
        } else {
            i += 1;
        }
    }

    parts.push(s[start..].iter().collect::<String>());
    parts.iter().map(|p| split_separators(p)).collect()
}

impl Runtime {
    /// X3DCast (type, node): through PROTO instances.
    fn hanim_cast(&self, node: Option<NodeId>, type_name: &str) -> Option<NodeId> {
        node.and_then(|n| self.inner_node(n)).filter(|n| self.builtin_name(*n) == Some(type_name))
    }

    fn hanim_nodes(&self, node: NodeId, field: &str) -> Vec<Option<NodeId>> {
        match self.pre_value(node, field) {
            Some(FieldValue::MFNode(v)) => v.clone(),
            _ => vec![],
        }
    }

    fn hanim_string(&self, node: NodeId, field: &str) -> String {
        match self.pre_value(node, field) {
            Some(FieldValue::SFString(s)) => s.clone(),
            _ => String::new(),
        }
    }

    fn hanim_i32(&self, node: NodeId, field: &str) -> i32 {
        match self.pre_value(node, field) {
            Some(FieldValue::SFInt32(v)) => *v,
            _ => 0,
        }
    }

    /// A private node like `new Type (executionContext)` / createNode (type, false): constructed,
    /// not set up.
    fn hanim_create(&mut self, ctx: CtxId, type_name: &str) -> Option<NodeId> {
        let &index = self.scene.table.by_name.get(type_name)?;
        let n = self.scene.instantiate_builtin(ctx, index);
        self.nrt_mut(n);
        self.construct_node(n);
        Some(n)
    }

    /// The count of HAnimHumanoid update.unlock () calls (0 for other nodes).
    pub fn hanim_unlocks(&self, humanoid: NodeId) -> u64 {
        self.hanim_humanoids.get(&humanoid).map_or(0, |h| h.unlocks)
    }

    fn hanim_unlock(&mut self, humanoid: NodeId) {
        if let Some(h) = self.hanim_humanoids.get_mut(&humanoid) {
            h.unlocks += 1;
        }
    }

    fn humanoid(&self, node: NodeId) -> HumanoidState {
        self.hanim_humanoids.get(&node).cloned().unwrap_or_default()
    }

    // --- HAnimHumanoid ----------------------------------------------------------------------

    pub(crate) fn hanim_humanoid_construct(&mut self, node: NodeId) {
        self.add_private_field(node, "jointName", access::OUTPUT_ONLY, FieldValue::SFTime(0.0));
    }

    pub(crate) fn hanim_humanoid_initialize(&mut self, node: NodeId) {
        self.hanim_humanoids.insert(node, HumanoidState::default());

        self.interest(node, "children", Cb::HAnimHumanoidSetChildren);
        self.interest(node, "motionsEnabled", Cb::HAnimHumanoidSetMotions);
        self.interest(node, "motions", Cb::HAnimHumanoidSetMotions);
        self.interest(node, "jointBindingPositions", Cb::HAnimHumanoidSetJoints);
        self.interest(node, "jointBindingRotations", Cb::HAnimHumanoidSetJoints);
        self.interest(node, "jointBindingScales", Cb::HAnimHumanoidSetJoints);
        self.interest(node, "joints", Cb::HAnimHumanoidSetJoints);

        if let Some(k) = self.private_field(node, "jointName") {
            self.add_interest(k, node, Cb::HAnimHumanoidConnectAllJoints);
        }

        self.interest(node, "skinCoord", Cb::HAnimHumanoidSetSkinCoord);

        self.hanim_humanoid_set_children(node);
        self.hanim_humanoid_set_motions(node);
        self.hanim_humanoid_set_joints(node);
        self.hanim_humanoid_set_skin_coord(node);

        // Depreciated
        self.interest(node, "segments", Cb::HAnimHumanoidDeprecated);
        self.interest(node, "sites", Cb::HAnimHumanoidDeprecated);

        let version = self.scene.spec_version_of(self.scene.node(node).ctx).parse::<f64>().unwrap_or(4.0);

        for field in ["segments", "sites"] {
            if version >= 4.1 {
                self.hanim_humanoid_deprecated(node, field);
            }
        }
    }

    /// set_depreciated__: a warning when the field holds nodes.
    pub(crate) fn hanim_humanoid_deprecated(&mut self, node: NodeId, field: &str) {
        if self.hanim_nodes(node, field).is_empty() {
            return;
        }

        let type_name = self.builtin_name(node).unwrap_or("HAnimHumanoid").to_string();
        self.log_warn(format!("Use of field {type_name}.{field} is deprecated. This field may be removed in future versions of X3D."));
    }

    pub(crate) fn hanim_humanoid_set_children(&mut self, node: NodeId) {
        let old = self.humanoid(node).poses;

        for pose in old {
            if let Some(k) = self.pre(pose, "set_fraction") {
                self.remove_interest(k, node, Cb::HAnimHumanoidPoseFraction(pose));
            }

            if let Some(k) = self.pre(pose, "isActive") {
                self.remove_interest(k, node, Cb::HAnimHumanoidPoseActive);
            }

            self.hanim_pose_remove_joints(pose, node);
        }

        let poses: Vec<NodeId> = self.hanim_nodes(node, "children").into_iter().filter_map(|c| self.hanim_cast(c, "HAnimPose")).collect();

        if let Some(h) = self.hanim_humanoids.get_mut(&node) {
            h.poses = poses.clone();
        }

        for pose in poses {
            if let Some(k) = self.pre(pose, "set_fraction") {
                self.add_interest(k, node, Cb::HAnimHumanoidPoseFraction(pose));
            }

            if let Some(k) = self.pre(pose, "isActive") {
                self.add_interest(k, node, Cb::HAnimHumanoidPoseActive);
            }

            self.hanim_pose_add_joints(pose, node);
        }
    }

    /// set_pose_fraction__: every other pose updates its interpolators at its next fraction.
    pub(crate) fn hanim_humanoid_pose_fraction(&mut self, node: NodeId, current: NodeId) {
        for pose in self.humanoid(node).poses {
            if pose != current {
                if let Some(p) = self.hanim_poses.get_mut(&pose) {
                    p.needs_update = true;
                }
            }
        }
    }

    /// set_pose_active__: when a pose finished, all poses update their interpolators.
    pub(crate) fn hanim_humanoid_pose_active(&mut self, node: NodeId, k: FKey) {
        if matches!(self.value(k), FieldValue::SFBool(true)) {
            return;
        }

        for pose in self.humanoid(node).poses {
            if let Some(p) = self.hanim_poses.get_mut(&pose) {
                p.needs_update = true;
            }
        }
    }

    const MOTION_CONNECT_FIELDS: [&'static str; 4] = ["joints", "channelsEnabled", "channels", "values"];

    pub(crate) fn hanim_humanoid_set_motions(&mut self, node: NodeId) {
        for motion in self.humanoid(node).motions {
            for field in Self::MOTION_CONNECT_FIELDS {
                if let Some(k) = self.pre(motion, field) {
                    self.remove_interest(k, node, Cb::HAnimHumanoidConnectJoints(motion));
                }
            }

            self.hanim_motion_disconnect(motion);
        }

        let enabled = match self.pre_value(node, "motionsEnabled") {
            Some(FieldValue::MFBool(v)) => v.clone(),
            _ => vec![],
        };
        let motions: Vec<NodeId> = self
            .hanim_nodes(node, "motions")
            .into_iter()
            .enumerate()
            .filter(|(i, _)| !(*i < enabled.len() && !enabled[*i]))
            .filter_map(|(_, c)| self.hanim_cast(c, "HAnimMotion"))
            .collect();

        if let Some(h) = self.hanim_humanoids.get_mut(&node) {
            h.motions = motions.clone();
        }

        let joints = self.humanoid(node).joints;

        for motion in motions {
            for field in Self::MOTION_CONNECT_FIELDS {
                if let Some(k) = self.pre(motion, field) {
                    self.add_interest(k, node, Cb::HAnimHumanoidConnectJoints(motion));
                }
            }

            self.hanim_motion_connect(motion, &joints);
        }
    }

    pub(crate) fn hanim_humanoid_connect_all_joints(&mut self, node: NodeId) {
        for motion in self.humanoid(node).motions {
            self.hanim_humanoid_connect_joints(node, motion);
        }
    }

    pub(crate) fn hanim_humanoid_connect_joints(&mut self, node: NodeId, motion: NodeId) {
        self.hanim_motion_disconnect(motion);
        let joints = self.humanoid(node).joints;
        self.hanim_motion_connect(motion, &joints);
    }

    pub(crate) fn hanim_humanoid_set_joints(&mut self, node: NodeId) {
        let state = self.humanoid(node);

        for &pose in &state.poses {
            self.hanim_pose_remove_joints(pose, node);
        }

        for &motion in &state.motions {
            self.hanim_motion_disconnect(motion);
        }

        for &joint in &state.joints {
            self.remove_node_interest(joint, node, Cb::HAnimHumanoidUnlock);

            if let Some(k) = self.pre(joint, "name") {
                self.remove_interest(k, node, Cb::HAnimHumanoidJointName);
            }
        }

        let joints: Vec<NodeId> = self.hanim_nodes(node, "joints").into_iter().filter_map(|c| self.hanim_cast(c, "HAnimJoint")).collect();

        if let Some(h) = self.hanim_humanoids.get_mut(&node) {
            h.joints = joints.clone();
        }

        for &joint in &joints {
            self.add_node_interest(joint, node, Cb::HAnimHumanoidUnlock);

            if let Some(k) = self.pre(joint, "name") {
                self.add_interest(k, node, Cb::HAnimHumanoidJointName);
            }
        }

        for &pose in &state.poses {
            self.hanim_pose_add_joints(pose, node);
        }

        for &motion in &state.motions {
            self.hanim_motion_connect(motion, &joints);
        }

        // jointTextures and displacementsTexture events: both unlock.
        self.hanim_unlock(node);
    }

    /// `jointNode ._name .addInterest ("addEvent", this ._jointName)`.
    pub(crate) fn hanim_humanoid_joint_name(&mut self, node: NodeId) {
        if let Some(k) = self.private_field(node, "jointName") {
            self.add_event(k);
        }
    }

    pub(crate) fn hanim_humanoid_set_skin_coord(&mut self, node: NodeId) {
        if let Some(old) = self.humanoid(node).skin_coord {
            self.remove_node_interest(old, node, Cb::HAnimHumanoidUnlock);
        }

        let coord = match self.pre_value(node, "skinCoord") {
            Some(FieldValue::SFNode(c)) => *c,
            _ => None,
        };
        let coord = self.hanim_cast(coord, "Coordinate").or_else(|| self.hanim_cast(coord, "CoordinateDouble"));

        if let Some(h) = self.hanim_humanoids.get_mut(&node) {
            h.skin_coord = coord;
        }

        if let Some(c) = coord {
            self.add_node_interest(c, node, Cb::HAnimHumanoidUnlock);
        }

        self.hanim_unlock(node);
    }

    pub(crate) fn hanim_humanoid_unlock(&mut self, node: NodeId) {
        self.hanim_unlock(node);
    }

    // --- HAnimPose --------------------------------------------------------------------------

    fn hanim_time_sensor(&self, node: NodeId) -> Option<NodeId> {
        match self.private_field(node, "timeSensor").map(|k| self.value(k)) {
            Some(FieldValue::SFNode(n)) => *n,
            _ => None,
        }
    }

    /// The private TimeSensor, held by a private field.
    fn hanim_make_time_sensor(&mut self, node: NodeId) -> Option<NodeId> {
        let ts = self.hanim_create(self.scene.node(node).ctx, "TimeSensor")?;
        let holder = self.add_private_field(node, "timeSensor", access::INITIALIZE_ONLY, FieldValue::SFNode(None));
        self.set_raw(holder, FieldValue::SFNode(Some(ts)));
        Some(ts)
    }

    fn hanim_set(&mut self, node: NodeId, field: &str, value: FieldValue) {
        if let Some(k) = self.pre(node, field) {
            self.set_value(k, value);
        }
    }

    fn hanim_field_interest(&mut self, from: (NodeId, &str), to: (NodeId, &str)) {
        if let (Some(a), Some(b)) = (self.pre(from.0, from.1), self.pre(to.0, to.1)) {
            self.add_field_interest(a, b);
        }
    }

    pub(crate) fn hanim_pose_initialize(&mut self, node: NodeId) {
        self.hanim_poses.insert(node, PoseState::default());

        let Some(ts) = self.hanim_make_time_sensor(node) else { return };

        self.hanim_field_interest((node, "enabled"), (ts, "enabled"));
        self.hanim_field_interest((node, "transitionDuration"), (ts, "cycleInterval"));
        self.hanim_field_interest((ts, "isActive"), (node, "isActive"));

        for (from, to) in [("enabled", "enabled"), ("transitionDuration", "cycleInterval")] {
            if let (Some(a), Some(b)) = (self.pre(node, from), self.pre(ts, to)) {
                let v = self.value(a).clone();
                self.set_raw(b, v);
            }
        }

        self.setup_node(ts);

        self.interest(node, "resetAllJoints", Cb::HAnimPoseResetAllJoints);
        self.interest(node, "commencePose", Cb::HAnimPoseCommence);
        self.interest(node, "set_startTime", Cb::HAnimPoseStartTime);
        self.interest(node, "set_fraction", Cb::HAnimPoseFraction);
        self.interest(node, "children", Cb::HAnimPoseChildren);

        self.hanim_pose_set_children(node);
    }

    fn hanim_pose_add_joints(&mut self, pose: NodeId, humanoid: NodeId) {
        if let Some(p) = self.hanim_poses.get_mut(&pose) {
            if !p.humanoids.contains(&humanoid) {
                p.humanoids.push(humanoid);
            }

            p.needs_update = true;
        }
    }

    fn hanim_pose_remove_joints(&mut self, pose: NodeId, humanoid: NodeId) {
        if let Some(p) = self.hanim_poses.get_mut(&pose) {
            p.humanoids.retain(|h| *h != humanoid);
        }
    }

    fn hanim_pose_update_interpolators(&mut self, pose: NodeId, reset: bool) {
        let Some(state) = self.hanim_poses.get(&pose).cloned() else { return };
        let Some(ts) = self.hanim_time_sensor(pose) else { return };
        let fraction_changed = self.pre(ts, "fraction_changed");

        if let Some(p) = self.hanim_poses.get_mut(&pose) {
            p.needs_update = false;
        }

        for &i in &state.interpolators {
            if let (Some(f), Some(s)) = (fraction_changed, self.pre(i, "set_fraction")) {
                self.remove_field_interest(f, s);
            }
        }

        let mut interpolators = vec![];

        for humanoid in state.humanoids {
            for joint in self.humanoid(humanoid).joints {
                self.hanim_pose_process_joint(pose, &state.pose_joints, joint, reset, &mut interpolators);
            }
        }

        for &i in &interpolators {
            if let (Some(f), Some(s)) = (fraction_changed, self.pre(i, "set_fraction")) {
                self.add_field_interest(f, s);
            }
        }

        if let Some(p) = self.hanim_poses.get_mut(&pose) {
            p.interpolators = interpolators;
        }
    }

    fn hanim_pose_process_joint(&mut self, pose: NodeId, pose_joints: &[(String, NodeId)], joint: NodeId, reset: bool, out: &mut Vec<NodeId>) {
        let mut pose_joint = None;

        if !reset {
            let name = self.hanim_string(joint, "name");
            pose_joint = pose_joints.iter().find(|(n, _)| *n == name).map(|(_, j)| *j);

            if pose_joint.is_none() && !pose_joints.is_empty() {
                return;
            }
        }

        let ctx = self.scene.node(pose).ctx;

        for (name, interpolator_name) in [("translation", "PositionInterpolator"), ("rotation", "OrientationInterpolator"), ("scale", "PositionInterpolator")] {
            let Some(joint_field) = self.pre(joint, name) else { continue };
            let current = self.value(joint_field).clone();
            let destination = match pose_joint.and_then(|p| self.pre_value(p, name)) {
                Some(v) => v.clone(),
                None => match name {
                    "rotation" => FieldValue::SFRotation(Rotation4::identity()),
                    "scale" => FieldValue::SFVec3f(Vector3::ONE),
                    _ => FieldValue::SFVec3f(Vector3::ZERO),
                },
            };

            let equal = match (&current, &destination) {
                (FieldValue::SFVec3f(a), FieldValue::SFVec3f(b)) => a.equals(b),
                (FieldValue::SFRotation(a), FieldValue::SFRotation(b)) => a.equals(b),
                _ => false,
            };

            if equal {
                continue;
            }

            let Some(interpolator) = self.hanim_create(ctx, interpolator_name) else { continue };

            if let Some(v) = self.pre(interpolator, "value_changed") {
                self.add_field_interest(v, joint_field);
            }

            let key_value = match (current, destination) {
                (FieldValue::SFVec3f(a), FieldValue::SFVec3f(b)) => FieldValue::MFVec3f([a.x, a.y, a.z, b.x, b.y, b.z].map(|v| v as f32).to_vec()),
                (FieldValue::SFRotation(mut a), FieldValue::SFRotation(mut b)) => FieldValue::MFRotation([a.components(), b.components()].concat()),
                _ => continue,
            };

            if let Some(k) = self.pre(interpolator, "key") {
                self.set_raw(k, FieldValue::MFFloat(vec![0.0, 1.0]));
            }

            if let Some(k) = self.pre(interpolator, "keyValue") {
                self.set_raw(k, key_value);
            }

            self.setup_node(interpolator);
            out.push(interpolator);
        }
    }

    pub(crate) fn hanim_pose_reset_all_joints(&mut self, node: NodeId) {
        if !self.get_bool(node, "resetAllJoints") {
            return;
        }

        self.hanim_pose_update_interpolators(node, true);
        self.hanim_pose_start(node);
    }

    fn hanim_pose_start(&mut self, node: NodeId) {
        let now = self.now;

        if let Some(ts) = self.hanim_time_sensor(node) {
            self.hanim_set(ts, "startTime", FieldValue::SFTime(now));
        }
    }

    pub(crate) fn hanim_pose_commence(&mut self, node: NodeId) {
        if !self.get_bool(node, "commencePose") {
            return;
        }

        self.hanim_pose_set_start_time(node);
    }

    /// set_startTime__: the event's value is not used, the TimeSensor starts now.
    pub(crate) fn hanim_pose_set_start_time(&mut self, node: NodeId) {
        self.hanim_pose_update_interpolators(node, false);
        self.hanim_pose_start(node);
    }

    pub(crate) fn hanim_pose_set_fraction(&mut self, node: NodeId) {
        if self.hanim_poses.get(&node).is_some_and(|p| p.needs_update) {
            self.hanim_pose_update_interpolators(node, false);
        }

        let fraction = self.get_f64(node, "set_fraction");

        for i in self.hanim_poses.get(&node).map(|p| p.interpolators.clone()).unwrap_or_default() {
            self.hanim_set(i, "set_fraction", FieldValue::SFFloat(fraction));
        }
    }

    pub(crate) fn hanim_pose_set_children(&mut self, node: NodeId) {
        let mut pose_joints: Vec<(String, NodeId)> = vec![];

        for c in self.hanim_nodes(node, "children") {
            if let Some(joint) = self.hanim_cast(c, "HAnimJoint") {
                let name = self.hanim_string(joint, "name");

                match pose_joints.iter_mut().find(|(n, _)| *n == name) {
                    Some(e) => e.1 = joint,
                    None => pose_joints.push((name, joint)),
                }
            }
        }

        if let Some(p) = self.hanim_poses.get_mut(&node) {
            p.pose_joints = pose_joints;
        }
    }

    // --- HAnimMotion ------------------------------------------------------------------------

    pub(crate) fn hanim_motion_initialize(&mut self, node: NodeId) {
        self.hanim_motions.insert(node, MotionState::default());

        let Some(ts) = self.hanim_make_time_sensor(node) else { return };

        self.hanim_field_interest((node, "loop"), (ts, "loop"));
        self.hanim_field_interest((ts, "cycleTime"), (node, "cycleTime"));
        self.hanim_field_interest((ts, "elapsedTime"), (node, "elapsedTime"));

        let looping = self.pre_value(node, "loop").cloned().unwrap_or(FieldValue::SFBool(false));

        for (field, value) in [("enabled", FieldValue::SFBool(false)), ("loop", looping), ("stopTime", FieldValue::SFTime(1.0))] {
            if let Some(k) = self.pre(ts, field) {
                self.set_raw(k, value);
            }
        }

        self.setup_node(ts);

        self.interest(node, "enabled", Cb::HAnimMotionEnabled);
        self.interest(node, "joints", Cb::HAnimMotionJoints);
        self.interest(node, "channels", Cb::HAnimMotionInterpolators);
        self.interest(node, "values", Cb::HAnimMotionInterpolators);
        self.interest(node, "next", Cb::HAnimMotionNextOrPrevious(1));
        self.interest(node, "previous", Cb::HAnimMotionNextOrPrevious(-1));
        self.interest(node, "frameIndex", Cb::HAnimMotionFrameIndex);
        self.interest(node, "frameDuration", Cb::HAnimMotionFrameDuration);
        self.interest(node, "frameIncrement", Cb::HAnimMotionFrameIncrement);
        self.interest(node, "startFrame", Cb::HAnimMotionStartOrEndFrame);
        self.interest(node, "endFrame", Cb::HAnimMotionStartOrEndFrame);

        self.hanim_motion_set_enabled(node);
        self.hanim_motion_set_joints(node);
        self.hanim_motion_set_frame_increment(node);
        self.hanim_motion_set_interpolators(node);
    }

    fn motion(&self, node: NodeId) -> MotionState {
        self.hanim_motions.get(&node).cloned().unwrap_or_default()
    }

    /// connectJoints: each enabled channel group's interpolators drive the joint of that name.
    fn hanim_motion_connect(&mut self, motion: NodeId, joint_nodes: &[NodeId]) {
        let channels_enabled = match self.pre_value(motion, "channelsEnabled") {
            Some(FieldValue::MFBool(v)) => v.clone(),
            _ => vec![],
        };
        let state = self.motion(motion);

        // getJointsIndex: name (trimmed) → joint; IGNORED removed; HumanoidRoot = humanoid_root.
        let mut index: Vec<(String, Option<NodeId>)> = vec![];

        for &j in joint_nodes {
            let name = self.hanim_string(j, "name").trim_matches(js_space).to_string();

            match index.iter_mut().find(|(n, _)| *n == name) {
                Some(e) => e.1 = Some(j),
                None => index.push((name, Some(j))),
            }
        }

        index.retain(|(n, _)| n != "IGNORED");
        let root = index.iter().find(|(n, _)| n == "humanoid_root").and_then(|(_, j)| *j);

        match index.iter_mut().find(|(n, _)| n == "HumanoidRoot") {
            Some(e) => e.1 = root,
            None => index.push(("HumanoidRoot".into(), root)),
        }

        for (j, interpolators) in state.interpolators.iter().enumerate() {
            if j < channels_enabled.len() && !channels_enabled[j] {
                continue;
            }

            let Some(joint) = state.joints.get(j).and_then(|name| index.iter().find(|(n, _)| n == name)).and_then(|(_, j)| *j) else { continue };

            for (field, interpolator) in JOINT_FIELDS.iter().zip(interpolators) {
                if let (Some(i), Some(to)) = (interpolator.and_then(|i| self.pre(i, "value_changed")), self.pre(joint, field)) {
                    self.add_field_interest(i, to);
                }
            }
        }
    }

    /// disconnectJoints: no interpolator drives a joint any more.
    fn hanim_motion_disconnect(&mut self, motion: NodeId) {
        for interpolators in self.motion(motion).interpolators {
            for i in interpolators.into_iter().flatten() {
                if let Some(k) = self.pre(i, "value_changed") {
                    if let Some(f) = self.frt.get_mut(&k) {
                        f.field_interests.clear();
                    }
                }
            }
        }
    }

    pub(crate) fn hanim_motion_set_enabled(&mut self, node: NodeId) {
        let Some(ts) = self.hanim_time_sensor(node) else { return };
        let now = self.now;

        if self.get_bool(node, "enabled") {
            self.hanim_set(ts, "startTime", FieldValue::SFTime(now));
        } else {
            self.hanim_set(ts, "stopTime", FieldValue::SFTime(now));
        }
    }

    pub(crate) fn hanim_motion_set_joints(&mut self, node: NodeId) {
        let joints = self.hanim_string(node, "joints");
        let joints = split_separators(joints.trim_matches(separator));

        if let Some(m) = self.hanim_motions.get_mut(&node) {
            m.joints = joints;
        }

        self.hanim_motion_disconnect(node);
    }

    pub(crate) fn hanim_motion_set_interpolators(&mut self, node: NodeId) {
        let Some(ts) = self.hanim_time_sensor(node) else { return };
        let Some(fraction_changed) = self.pre(ts, "fraction_changed") else { return };

        if let Some(f) = self.frt.get_mut(&fraction_changed) {
            f.field_interests.clear();
        }

        let channels = parse_channels(&self.hanim_string(node, "channels"));
        let values = match self.pre_value(node, "values") {
            Some(FieldValue::MFFloat(v)) => v.clone(),
            _ => vec![],
        };
        let num_channels: usize = channels.iter().map(Vec::len).sum();
        let frame_count = if num_channels > 0 { values.len() / num_channels } else { 0 };
        let ctx = self.scene.node(node).ctx;
        let mut interpolators: Vec<[Option<NodeId>; 3]> = vec![[None; 3]; channels.len()];
        let mut keys: Vec<[Vec<f32>; 3]> = vec![Default::default(); channels.len()];
        let mut key_values: Vec<[Vec<f64>; 3]> = vec![Default::default(); channels.len()];
        // The Rotation4 setEuler writes into (kept between groups and frames).
        let mut rotation = Rotation4::identity();
        let mut v = 0;

        for frame in 0..frame_count {
            let key = (frame as f64 / (frame_count as f64 - 1.0)) as f32;

            for (j, joint) in channels.iter().enumerate() {
                let (mut position, mut position_channels) = ([0.0f64; 3], false);
                let (mut euler, mut order) = ([0.0f64; 3], String::new());
                let (mut scale, mut scale_channels) = ([1.0f64; 3], false);
                let mut next = || {
                    let x = values.get(v).map_or(f64::NAN, |x| *x as f64);
                    v += 1;
                    x
                };

                for channel in joint {
                    match channel.as_str() {
                        "Xposition" => (position_channels, position[0]) = (true, next()),
                        "Yposition" => (position_channels, position[1]) = (true, next()),
                        "Zposition" => (position_channels, position[2]) = (true, next()),
                        "Xrotation" => {
                            order.push('X');
                            euler[0] = next().to_radians();
                        }
                        "Yrotation" => {
                            order.push('Y');
                            euler[1] = next().to_radians();
                        }
                        "Zrotation" => {
                            order.push('Z');
                            euler[2] = next().to_radians();
                        }
                        "Xscale" => (scale_channels, scale[0]) = (true, next()),
                        "Yscale" => (scale_channels, scale[1]) = (true, next()),
                        "Zscale" => (scale_channels, scale[2]) = (true, next()),
                        _ => {
                            next();
                        }
                    }
                }

                let mut slot = |s: usize, type_name: &str, rt: &mut Runtime| {
                    if interpolators[j][s].is_none() {
                        if let Some(i) = rt.hanim_create(ctx, type_name) {
                            if let Some(k) = rt.pre(i, "set_fraction") {
                                rt.add_field_interest(fraction_changed, k);
                            }

                            interpolators[j][s] = Some(i);
                        }
                    }
                };

                if position_channels {
                    slot(0, "PositionInterpolator", self);
                    keys[j][0].push(key);
                    key_values[j][0].extend_from_slice(&position);
                }

                if !order.is_empty() {
                    slot(1, "OrientationInterpolator", self);

                    if order.len() != 3 {
                        if order.len() < 3 {
                            for o in ["X", "Y", "Z"] {
                                if !order.contains(o) {
                                    order.push_str(o);
                                }
                            }
                        } else {
                            order.truncate(3);
                        }
                    }

                    rotation.set_euler_order(euler[0], euler[1], euler[2], &order);
                    keys[j][1].push(key);
                    key_values[j][1].extend_from_slice(&rotation.components());
                }

                if scale_channels {
                    slot(2, "PositionInterpolator", self);
                    keys[j][2].push(key);
                    key_values[j][2].extend_from_slice(&scale);
                }
            }
        }

        for (j, group) in interpolators.iter().enumerate() {
            for (s, i) in group.iter().enumerate() {
                let Some(i) = *i else { continue };

                if let Some(k) = self.pre(i, "key") {
                    self.set_raw(k, FieldValue::MFFloat(keys[j][s].clone()));
                }

                if let Some(k) = self.pre(i, "keyValue") {
                    let value = if s == 1 { FieldValue::MFRotation(key_values[j][s].clone()) } else { FieldValue::MFVec3f(key_values[j][s].iter().map(|v| *v as f32).collect()) };
                    self.set_raw(k, value);
                }
            }
        }

        for group in &interpolators {
            for i in group.iter().flatten() {
                self.setup_node(*i);
            }
        }

        if let Some(m) = self.hanim_motions.get_mut(&node) {
            m.interpolators = interpolators;
        }

        self.hanim_set(node, "frameIndex", FieldValue::SFInt32(0));
        self.hanim_set(node, "startFrame", FieldValue::SFInt32(0));
        self.hanim_set(node, "endFrame", FieldValue::SFInt32((frame_count as i32 - 1).max(0)));
        self.hanim_set(node, "frameCount", FieldValue::SFInt32(frame_count as i32));

        self.hanim_motion_set_frame_duration(node);
    }

    /// getFraction: the set_fraction value of the first interpolator the TimeSensor drives.
    fn hanim_motion_fraction(&self, node: NodeId) -> f64 {
        let Some(k) = self.hanim_time_sensor(node).and_then(|ts| self.pre(ts, "fraction_changed")) else { return 0.0 };

        match self.frt.get(&k).and_then(|f| f.field_interests.first()) {
            Some(f) => match self.value(*f) {
                FieldValue::SFFloat(v) => *v,
                _ => 0.0,
            },
            None => 0.0,
        }
    }

    pub(crate) fn hanim_motion_next_or_previous(&mut self, node: NodeId, direction: i32, k: FKey) {
        if !matches!(self.value(k), FieldValue::SFBool(true)) {
            return;
        }

        let fraction = self.hanim_motion_fraction(node);
        let frame_count = self.hanim_i32(node, "frameCount");
        let frame_increment = self.hanim_i32(node, "frameIncrement");
        let frame_index = (if frame_count > 1 { (fraction * (frame_count - 1) as f64).floor() } else { 0.0 }) + (frame_increment * direction) as f64;
        let state = self.motion(node);
        let looping = self.get_bool(node, "loop");

        if state.end_frame.is_some_and(|e| frame_index > e as f64) {
            if !looping {
                return;
            }

            self.hanim_set(node, "frameIndex", FieldValue::SFInt32(state.start_frame.unwrap_or(0)));
        } else if state.start_frame.is_some_and(|s| frame_index < s as f64) {
            if !looping {
                return;
            }

            self.hanim_set(node, "frameIndex", FieldValue::SFInt32(state.end_frame.unwrap_or(0)));
        } else {
            self.hanim_set(node, "frameIndex", FieldValue::SFInt32(frame_index as i32));
        }
    }

    fn hanim_motion_range(&mut self, ts: NodeId, values: &[(usize, f64)]) {
        let Some(k) = self.private_field(ts, "range") else { return };
        let mut range = match self.value(k) {
            FieldValue::MFFloat(v) => v.clone(),
            _ => vec![0.0, 0.0, 1.0],
        };

        for &(i, v) in values {
            if i < range.len() {
                range[i] = v as f32;
            }
        }

        self.set_value(k, FieldValue::MFFloat(range));
    }

    pub(crate) fn hanim_motion_set_frame_index(&mut self, node: NodeId) {
        let Some(ts) = self.hanim_time_sensor(node) else { return };
        let frame_count = self.hanim_i32(node, "frameCount");
        let frame_index = self.hanim_i32(node, "frameIndex").clamp(0, frame_count.max(0));
        let fraction = if frame_count > 1 { frame_index as f64 / (frame_count - 1) as f64 } else { 0.0 };

        self.hanim_motion_range(ts, &[(0, fraction)]);

        if self.get_bool(ts, "isActive") {
            return;
        }

        let Some(k) = self.pre(ts, "fraction_changed") else { return };

        for f in self.frt.get(&k).map(|f| f.field_interests.clone()).unwrap_or_default() {
            self.set_value(f, FieldValue::SFFloat(fraction));
        }
    }

    pub(crate) fn hanim_motion_set_frame_duration(&mut self, node: NodeId) {
        let Some(ts) = self.hanim_time_sensor(node) else { return };
        let frame_count = self.hanim_i32(node, "frameCount");
        let frame_duration = self.get_f64(node, "frameDuration").max(0.0);

        self.hanim_set(ts, "cycleInterval", FieldValue::SFTime(if frame_count > 1 { (frame_count - 1) as f64 * frame_duration } else { 0.0 }));
    }

    pub(crate) fn hanim_motion_set_frame_increment(&mut self, node: NodeId) {
        let Some(ts) = self.hanim_time_sensor(node) else { return };
        let increment = self.hanim_i32(node, "frameIncrement");

        self.hanim_set(ts, "enabled", FieldValue::SFBool(increment != 0));
    }

    pub(crate) fn hanim_motion_set_start_or_end_frame(&mut self, node: NodeId) {
        let Some(ts) = self.hanim_time_sensor(node) else { return };
        let frame_count = self.hanim_i32(node, "frameCount");
        let start = self.hanim_i32(node, "startFrame").clamp(0, frame_count.max(0));
        let end = self.hanim_i32(node, "endFrame").clamp(0, frame_count.max(0));
        let (start, end) = (start.min(end), start.max(end));

        if let Some(m) = self.hanim_motions.get_mut(&node) {
            m.start_frame = Some(start);
            m.end_frame = Some(end);
        }

        let fraction = |f: i32| if frame_count > 1 { f as f64 / (frame_count - 1) as f64 } else { 0.0 };
        self.hanim_motion_range(ts, &[(1, fraction(start)), (2, fraction(end))]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_split_like_the_regular_expressions() {
        let c = parse_channels("6 Xposition Yposition Zposition Zrotation Xrotation Yrotation 3 Zrotation Xrotation Yrotation");
        assert_eq!(c, vec![vec!["Xposition", "Yposition", "Zposition", "Zrotation", "Xrotation", "Yrotation"], vec!["Zrotation", "Xrotation", "Yrotation"]]);
        assert_eq!(parse_channels(""), vec![vec![""]]);
        assert_eq!(parse_channels("3,Xrotation,Yrotation,Zrotation, 1 Xposition 2"), vec![vec!["Xrotation", "Yrotation", "Zrotation"], vec!["Xposition"]]);
        assert_eq!(split_separators("a, b  c"), vec!["a", "b", "c"]);
        assert_eq!(split_separators(""), vec![""]);
    }
}
