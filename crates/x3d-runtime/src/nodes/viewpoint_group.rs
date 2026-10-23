//! Components/Navigation/ViewpointGroup runtime parts: the private ProximitySensor and the
//! `active` and `displayed` state its traversal and the user viewpoint list read.

use crate::*;
use x3d_fields::math::Vector3;

impl Runtime {
    /// ViewpointGroup constructor and initialize: the private ProximitySensor gets size and center
    /// (`addFieldInterest` and the initial values) and is set up; then set_size__.
    pub(crate) fn viewpoint_group_initialize(&mut self, node: NodeId) {
        let Some(&index) = self.scene.table.by_name.get("ProximitySensor") else {
            return;
        };

        let ctx = self.scene.node(node).ctx;
        let sensor = self.scene.instantiate_builtin(ctx, index);
        self.nrt_mut(sensor);
        self.construct_node(sensor);

        let holder = self.add_private_field(
            node,
            "proximitySensor",
            access::INITIALIZE_ONLY,
            FieldValue::SFNode(None),
        );
        self.set_raw(holder, FieldValue::SFNode(Some(sensor)));

        self.interest(node, "size", Cb::ViewpointGroupSetSize);

        for name in ["size", "center"] {
            if let (Some(from), Some(to)) = (self.pre(node, name), self.pre(sensor, name)) {
                self.add_field_interest(from, to);
            }
        }

        if let Some(k) = self.pre(sensor, "isActive") {
            self.add_interest(k, node, Cb::ViewpointGroupSetActive);
        }

        for name in ["size", "center"] {
            if let (Some(from), Some(to)) = (self.pre(node, name), self.pre(sensor, name)) {
                let value = self.value(from).clone();
                self.set_raw(to, value);
            }
        }

        self.setup_node(sensor);
        self.viewpoint_group_set_size(node);
    }

    /// The private ProximitySensor of a ViewpointGroup.
    pub fn viewpoint_group_sensor(&self, node: NodeId) -> Option<NodeId> {
        match self
            .private_field(node, "proximitySensor")
            .map(|k| self.value(k))
        {
            Some(FieldValue::SFNode(n)) => *n,
            _ => None,
        }
    }

    /// ViewpointGroup.set_size__: the sensor is enabled unless size is 0 0 0.
    pub(crate) fn viewpoint_group_set_size(&mut self, node: NodeId) {
        let size = match self.pre_value(node, "size") {
            Some(FieldValue::SFVec3f(v)) => *v,
            _ => Vector3::ZERO,
        };

        if let Some(k) = self
            .viewpoint_group_sensor(node)
            .and_then(|s| self.pre(s, "enabled"))
        {
            self.set_value(k, FieldValue::SFBool(size != Vector3::ZERO));
        }

        self.viewpoint_group_set_active(node);
    }

    /// ViewpointGroup.set_active__: `active` (the sensor is active, or not enabled) and
    /// `displayed` (the field and `active`), read when they are computed: a later change of the
    /// `displayed` field alone does not change them.
    pub(crate) fn viewpoint_group_set_active(&mut self, node: NodeId) {
        let Some(sensor) = self.viewpoint_group_sensor(node) else {
            return;
        };
        let active = self.get_bool(sensor, "isActive") || !self.get_bool(sensor, "enabled");
        let displayed = self.get_bool(node, "displayed") && active;

        self.viewpoint_groups.insert(node, (active, displayed));
    }

    /// ViewpointGroup (`active`, getDisplayed ()).
    pub fn viewpoint_group_state(&self, node: NodeId) -> (bool, bool) {
        self.viewpoint_groups
            .get(&node)
            .copied()
            .unwrap_or((false, false))
    }
}
