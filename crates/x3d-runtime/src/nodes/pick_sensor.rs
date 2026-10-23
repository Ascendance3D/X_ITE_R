//! Components/Picking/X3DPickSensorNode.js: the browser keeps the set of live pick sensors
//! (X3DPickingContext [_pickSensorNodes] [0]); the PICKING traversal, which collects the targets,
//! and `process`, which needs their geometry, run in the viewer (x3d-viewer picking.rs and
//! pick_shapes.rs, the Ammo contact test of Point-, Primitive- and VolumePickSensor).

use crate::*;
use std::collections::HashSet;

impl Runtime {
    /// X3DPickSensorNode.initialize.
    pub(crate) fn pick_sensor_initialize(&mut self, node: NodeId) {
        self.watch_live(node);
        self.interest(node, "enabled", Cb::PickSensorSetLive);
        self.interest(node, "objectType", Cb::PickSensorSetLive);
        self.pick_sensor_set_live(node);
    }

    /// set_live__ (also after set_objectType__): live, enabled and objectType without "NONE".
    pub(crate) fn pick_sensor_set_live(&mut self, node: NodeId) {
        let on = self.time_node_live(node)
            && self.get_bool(node, "enabled")
            && !self
                .pick_sensor_object_type(node)
                .iter()
                .any(|t| t == "NONE");

        if on {
            if !self.pick_sensors.contains(&node) {
                self.pick_sensors.push(node);
            }
        } else {
            self.pick_sensors.retain(|&n| n != node);
        }
    }

    /// `this .objectType`: the Set of the objectType strings (X3DPickSensorNode and X3DPickableObject).
    pub fn pick_sensor_object_type(&self, node: NodeId) -> Vec<String> {
        let mut out: Vec<String> = vec![];

        if let Some(FieldValue::MFString(v)) = self.pre_value(node, "objectType") {
            for t in v {
                if !out.contains(t) {
                    out.push(t.clone());
                }
            }
        }

        out
    }

    /// set_pickTarget__: the inner nodes of `pickTarget` that are an Inline, a Shape or an
    /// X3DGroupingNode.
    pub fn pick_sensor_targets(&self, node: NodeId) -> HashSet<NodeId> {
        let Some(FieldValue::MFNode(v)) = self.pre_value(node, "pickTarget").cloned() else {
            return HashSet::new();
        };

        v.into_iter()
            .flatten()
            .filter_map(|n| self.inner_node(n))
            .filter(|&n| {
                self.builtin_name(n).is_some_and(|t| {
                    t == "Inline"
                        || t == "Shape"
                        || (t != "ParticleSystem"
                            && crate::nodes::transform_sensor::is_grouping_or_shape(t))
                })
            })
            .collect()
    }

    /// `getInnerNode ()`: through PROTO instances to the first root node of the body.
    pub fn inner_node(&self, node: NodeId) -> Option<NodeId> {
        let mut n = node;

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

            return Some(n);
        }

        None
    }
}
