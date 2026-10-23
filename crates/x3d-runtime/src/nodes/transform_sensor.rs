//! Components/EnvironmentalSensor/TransformSensor.js: reports the position and orientation of its
//! target object (a grouping node or a shape) inside its box. The browser keeps the set of
//! sensors that can fire (X3DPickingContext [_transformSensorNodes]); the PICKING traversal and
//! `process` run in the viewer, which walks the scene in world coordinates.

use crate::*;
use x3d_fields::math::{Matrix4, Rotation4, Vector3};

/// X3DGroupingNode and X3DShapeNode types (X3DCast of `targetObject`).
pub(crate) fn is_grouping_or_shape(name: &str) -> bool {
    matches!(
        name,
        "Group"
            | "Transform"
            | "Anchor"
            | "Billboard"
            | "Collision"
            | "LOD"
            | "Switch"
            | "StaticGroup"
            | "CADAssembly"
            | "CADFace"
            | "CADLayer"
            | "CADPart"
            | "GeoLocation"
            | "GeoTransform"
            | "LayoutGroup"
            | "ScreenGroup"
            | "PickableGroup"
            | "GroupAnnotation"
            | "EspduTransform"
            | "HAnimJoint"
            | "HAnimSegment"
            | "HAnimSite"
            | "Shape"
            | "ParticleSystem"
    )
}

impl Runtime {
    fn ts_set(&mut self, node: NodeId, name: &str, value: FieldValue) {
        if let Some(k) = self.pre(node, name) {
            self.set_value(k, value);
        }
    }

    /// TransformSensor.initialize.
    pub(crate) fn transform_sensor_initialize(&mut self, node: NodeId) {
        self.watch_live(node);
        self.interest(node, "enabled", Cb::TransformSensorSetEnabled);
        self.interest(node, "size", Cb::TransformSensorSetEnabled);
        self.interest(node, "targetObject", Cb::TransformSensorSetEnabled);
        self.transform_sensor_set_enabled(node);
    }

    /// `this .targetObjectNode`: X3DCast (X3DGroupingNode) ?? X3DCast (X3DShapeNode), through PROTO instances.
    pub fn transform_sensor_target(&self, node: NodeId) -> Option<NodeId> {
        let Some(FieldValue::SFNode(Some(mut n))) = self.pre_value(node, "targetObject").cloned()
        else {
            return None;
        };

        for _ in 0..256 {
            if let NodeKind::Instance(_) = self.scene.node(n).kind {
                n = self
                    .scene
                    .ctx(self.instance_body(n)?)
                    .root_nodes
                    .first()
                    .copied()
                    .flatten()?;
                continue;
            }

            return self
                .builtin_name(n)
                .is_some_and(is_grouping_or_shape)
                .then_some(n);
        }

        None
    }

    fn ts_vec3(&self, node: NodeId, name: &str) -> Vector3 {
        match self.pre_value(node, name) {
            Some(FieldValue::SFVec3f(v)) => *v,
            _ => Vector3::ZERO,
        }
    }

    /// set_enabled__ (also set_targetObject__ and set_live__).
    pub(crate) fn transform_sensor_set_enabled(&mut self, node: NodeId) {
        let on = self.time_node_live(node)
            && self.transform_sensor_target(node).is_some()
            && self.get_bool(node, "enabled")
            && !self.ts_vec3(node, "size").equals(&Vector3::ZERO);

        if on {
            if !self.transform_sensors.contains(&node) {
                self.transform_sensors.push(node);
            }
        } else {
            self.transform_sensors.retain(|&n| n != node);

            if self.get_bool(node, "isActive") {
                let now = self.now;
                self.ts_set(node, "isActive", FieldValue::SFBool(false));
                self.ts_set(node, "exitTime", FieldValue::SFTime(now));
            }
        }
    }

    /// TransformSensor.process with the model matrices of the sensor and of its target that the
    /// PICKING traversal collected.
    pub fn transform_sensor_process(
        &mut self,
        node: NodeId,
        mut model_matrices: Vec<Matrix4>,
        mut target_matrices: Vec<Matrix4>,
    ) {
        let size = self.ts_vec3(node, "size");
        let center = self.ts_vec3(node, "center");
        let always = size.equals(&Vector3::new(-1.0, -1.0, -1.0));
        let (min, max) = (
            Vector3::new(
                center.x - size.x / 2.0,
                center.y - size.y / 2.0,
                center.z - size.z / 2.0,
            ),
            Vector3::new(
                center.x + size.x / 2.0,
                center.y + size.y / 2.0,
                center.z + size.z / 2.0,
            ),
        );
        let contains = |p: &Vector3| {
            min.x <= p.x
                && max.x >= p.x
                && min.y <= p.y
                && max.y >= p.y
                && min.z <= p.z
                && max.z >= p.z
        };

        // intersects (): `modelMatrix .inverse ()` and `targetMatrix .multRight (invModelMatrix)` change
        // the collected matrices in place, as in X_ITE.
        let mut found = None;

        'outer: for model in model_matrices.iter_mut() {
            model.inverse();
            let inv = *model;

            for target in target_matrices.iter_mut() {
                target.mult_right(&inv);

                let origin = Vector3::new(target.0[12], target.0[13], target.0[14]);

                if always || contains(&origin) {
                    found = Some(*target);
                    break 'outer;
                }
            }
        }

        let now = self.now;

        match found {
            Some(matrix) => {
                let mut position = Vector3::ZERO;
                let mut orientation = Rotation4::identity();
                matrix.get_transform(
                    Some(&mut position),
                    Some(&mut orientation),
                    None,
                    None,
                    None,
                );

                if self.get_bool(node, "isActive") {
                    if !matches!(self.pre_value(node, "position_changed"), Some(FieldValue::SFVec3f(p)) if p.equals(&position))
                    {
                        self.ts_set(node, "position_changed", FieldValue::SFVec3f(position));
                    }

                    if !matches!(self.pre_value(node, "orientation_changed"), Some(FieldValue::SFRotation(o)) if o.equals(&orientation))
                    {
                        self.ts_set(
                            node,
                            "orientation_changed",
                            FieldValue::SFRotation(orientation),
                        );
                    }
                } else {
                    self.ts_set(node, "isActive", FieldValue::SFBool(true));
                    self.ts_set(node, "enterTime", FieldValue::SFTime(now));
                    self.ts_set(node, "position_changed", FieldValue::SFVec3f(position));
                    self.ts_set(
                        node,
                        "orientation_changed",
                        FieldValue::SFRotation(orientation),
                    );
                }
            }
            None => {
                if self.get_bool(node, "isActive") {
                    self.ts_set(node, "isActive", FieldValue::SFBool(false));
                    self.ts_set(node, "exitTime", FieldValue::SFTime(now));
                }
            }
        }
    }
}
