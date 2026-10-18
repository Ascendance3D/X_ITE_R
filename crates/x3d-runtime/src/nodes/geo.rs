//! Components/Geospatial runtime parts: X3DGeospatialObject state read from the node's fields and
//! GeoPositionInterpolator.interpolate.

use crate::*;
use x3d_fields::math::Vector3;
use crate::geo::{GeoOriginInfo, GeospatialObject, geocentric_slerp, geo_system};
use crate::nodes::LoadState;

impl Runtime {
    fn geo_strings(&self, node: NodeId, name: &str) -> Vec<String> {
        match self.pre_value(node, name) {
            Some(FieldValue::MFString(v)) => v.clone(),
            _ => vec![],
        }
    }

    fn geo_vec3(&self, node: NodeId, name: &str) -> Vector3 {
        match self.pre_value(node, name) {
            Some(FieldValue::SFVec3d(v) | FieldValue::SFVec3f(v)) => *v,
            _ => Vector3::ZERO,
        }
    }

    /// X3DGeospatialObject: reference frames from `geoSystem`, origin from the `geoOrigin` GeoOrigin.
    pub fn geo_object(&self, node: NodeId) -> GeospatialObject {
        let origin = match self.pre_value(node, "geoOrigin") {
            Some(FieldValue::SFNode(Some(o))) if self.builtin_name(*o) == Some("GeoOrigin") => Some(GeoOriginInfo {
                origin: geo_system::reference_frame(&self.geo_strings(*o, "geoSystem"), false).convert(&self.geo_vec3(*o, "geoCoords")),
                rotate_y_up: self.get_bool(*o, "rotateYUp"),
            }),
            _ => None,
        };

        GeospatialObject::new(&self.geo_strings(node, "geoSystem"), origin, false)
    }

    /// X3DViewpointNode.set_navigationInfo__: X3DCast (NavigationInfo, navigationInfo), through
    /// PROTO instances.
    pub fn viewpoint_navigation_info(&self, node: NodeId) -> Option<NodeId> {
        let Some(FieldValue::SFNode(Some(mut n))) = self.pre_value(node, "navigationInfo").cloned() else {
            return None;
        };

        for _ in 0..256 {
            if let NodeKind::Instance(_) = self.scene.node(n).kind {
                n = self.scene.ctx(self.instance_body(n)?).root_nodes.first().copied().flatten()?;
                continue;
            }

            return (self.builtin_name(n) == Some("NavigationInfo")).then_some(n);
        }

        None
    }

    /// The private NavigationInfo of a GeoViewpoint up to specification version 3.2 (its
    /// traverse adds it to the layer's NavigationInfo list).
    pub fn geo_navigation_info(&self, node: NodeId) -> Option<NodeId> {
        match self.private_field(node, "geoNavigationInfo").map(|k| self.value(k)) {
            Some(FieldValue::SFNode(n)) => *n,
            _ => None,
        }
    }

    /// GeoViewpoint constructor and initialize for specification versions up to 3.2: the private
    /// NavigationInfo gets navType and headlight (`addFieldInterest` and the initial values),
    /// is set up, and becomes the `navigationInfo` field.
    pub(crate) fn geo_viewpoint_initialize(&mut self, node: NodeId) {
        if !self.scene.is_legacy_geo_viewpoint(node) {
            return;
        }

        let Some(&index) = self.scene.table.by_name.get("NavigationInfo") else {
            return;
        };

        let ctx = self.scene.node(node).ctx;
        let nav = self.scene.instantiate_builtin(ctx, index);
        self.nrt_mut(nav);
        self.construct_node(nav);

        let holder = self.add_private_field(node, "geoNavigationInfo", access::INITIALIZE_ONLY, FieldValue::SFNode(None));
        self.set_raw(holder, FieldValue::SFNode(Some(nav)));

        for (from, to) in [("navType", "type"), ("headlight", "headlight")] {
            if let (Some(from), Some(to)) = (self.private_field(node, from), self.pre(nav, to)) {
                self.add_field_interest(from, to);
                let value = self.value(from).clone();
                self.set_raw(to, value);
            }
        }

        self.setup_node(nav);

        if let Some(k) = self.pre(node, "navigationInfo") {
            self.set_value(k, FieldValue::SFNode(Some(nav)));
        }
    }

    /// GeoLOD: its private root Inline and four child Inlines.
    pub fn geo_lod_inlines(&self, node: NodeId) -> Option<(NodeId, [NodeId; 4])> {
        let root = match self.private_field(node, "rootInlineNode").map(|k| self.value(k)) {
            Some(FieldValue::SFNode(Some(n))) => *n,
            _ => return None,
        };
        let children = match self.private_field(node, "childInlineNodes").map(|k| self.value(k)) {
            Some(FieldValue::MFNode(v)) if v.len() == 4 => [v[0]?, v[1]?, v[2]?, v[3]?],
            _ => return None,
        };

        Some((root, children))
    }

    /// GeoLOD `level_changed` (-1 before the first DISPLAY traversal).
    pub fn geo_lod_level(&self, node: NodeId) -> i32 {
        match self.pre_value(node, "level_changed") {
            Some(FieldValue::SFInt32(v)) => *v,
            _ => -1,
        }
    }

    /// GeoLOD.traverse: `childrenLoaded ? level_changed : 0`. With `display`, the value of the
    /// last DISPLAY traversal ([`Runtime::geo_lod_display`]): X_ITE draws during that traversal,
    /// before the displayFinishEvents and finishEvents of the frame change the state again.
    pub fn geo_lod_mode(&self, node: NodeId, display: bool) -> i32 {
        if display {
            if let Some(FieldValue::SFInt32(v)) = self.private_field(node, "displayLevel").map(|k| self.value(k)) {
                return *v;
            }
        }

        if self.geo_lod_children_loaded(node) { self.geo_lod_level(node) } else { 0 }
    }

    /// Record the traversal mode of a GeoLOD the DISPLAY traversal visited (after changeLevel).
    pub fn geo_lod_display(&mut self, node: NodeId) {
        let mode = self.geo_lod_mode(node, false);
        let k = self.add_private_field(node, "displayLevel", access::INITIALIZE_ONLY, FieldValue::SFInt32(mode));
        self.set_raw(k, FieldValue::SFInt32(mode));
    }

    /// GeoLOD `childrenLoaded`.
    pub fn geo_lod_children_loaded(&self, node: NodeId) -> bool {
        self.private_field(node, "childrenLoaded").is_some_and(|k| matches!(self.value(k), FieldValue::SFBool(true)))
    }

    fn geo_lod_set_children_loaded(&mut self, node: NodeId, value: bool) {
        if let Some(k) = self.private_field(node, "childrenLoaded") {
            self.set_raw(k, FieldValue::SFBool(value));
        }
    }

    /// `inlineNode .getInternalScene () .getRootNodes ()` (no roots before a scene is attached).
    fn inline_root_nodes(&self, inline: NodeId) -> Vec<Option<NodeId>> {
        self.inline_scenes.get(&inline).map(|ctx| self.scene.ctx(*ctx).root_nodes.clone()).unwrap_or_default()
    }

    fn geo_lod_set_children(&mut self, node: NodeId, nodes: Vec<Option<NodeId>>) {
        if let Some(k) = self.pre(node, "children") {
            self.set_value(k, FieldValue::MFNode(nodes));
        }
    }

    fn geo_lod_root_node(&self, node: NodeId) -> Vec<Option<NodeId>> {
        match self.pre_value(node, "rootNode") {
            Some(FieldValue::MFNode(v)) => v.clone(),
            _ => vec![],
        }
    }

    /// GeoLOD constructor and initialize: the root Inline (load TRUE) and the child Inlines
    /// (load FALSE) follow rootUrl and child1Url … child4Url (`addFieldInterest` and the initial
    /// values) and are set up; their loadState changes call set_rootLoadState__ and
    /// set_childLoadState__. The private Group of `rootNode` is not a node here: traversing
    /// the GeoLOD at level 0 visits `rootNode` like the Group's children.
    pub(crate) fn geo_lod_initialize(&mut self, node: NodeId) {
        let Some(&index) = self.scene.table.by_name.get("Inline") else {
            return;
        };

        let ctx = self.scene.node(node).ctx;
        let inlines: Vec<NodeId> = (0..5)
            .map(|_| {
                let inline = self.scene.instantiate_builtin(ctx, index);
                self.nrt_mut(inline);
                self.construct_node(inline);
                inline
            })
            .collect();

        let root = self.add_private_field(node, "rootInlineNode", access::INITIALIZE_ONLY, FieldValue::SFNode(None));
        self.set_raw(root, FieldValue::SFNode(Some(inlines[0])));
        let children = self.add_private_field(node, "childInlineNodes", access::INITIALIZE_ONLY, FieldValue::MFNode(vec![]));
        self.set_raw(children, FieldValue::MFNode(inlines[1..].iter().map(|n| Some(*n)).collect()));
        self.add_private_field(node, "childrenLoaded", access::INITIALIZE_ONLY, FieldValue::SFBool(false));

        for (i, &inline) in inlines.iter().enumerate() {
            let state = self.add_private_field(inline, "loadState", access::OUTPUT_ONLY, FieldValue::SFInt32(0));
            self.add_interest(state, node, if i == 0 { Cb::GeoLodRootLoadState } else { Cb::GeoLodChildLoadState });
        }

        for (i, &inline) in inlines.iter().enumerate() {
            let url = ["rootUrl", "child1Url", "child2Url", "child3Url", "child4Url"][i];

            if let (Some(from), Some(to)) = (self.pre(node, url), self.pre(inline, "url")) {
                self.add_field_interest(from, to);
            }
        }

        for (i, &inline) in inlines.iter().enumerate() {
            if let Some(k) = self.pre(inline, "load") {
                self.set_raw(k, FieldValue::SFBool(i == 0));
            }
        }

        for (i, &inline) in inlines.iter().enumerate() {
            let url = ["rootUrl", "child1Url", "child2Url", "child3Url", "child4Url"][i];

            if let (Some(from), Some(to)) = (self.pre(node, url), self.pre(inline, "url")) {
                let value = self.value(from).clone();
                self.set_raw(to, value);
            }
        }

        for inline in inlines {
            self.setup_node(inline);
        }
    }

    /// GeoLOD.set_rootLoadState__.
    pub(crate) fn geo_lod_set_root_load_state(&mut self, node: NodeId) {
        let Some((root, _)) = self.geo_lod_inlines(node) else { return };

        if self.geo_lod_level(node) != 0 || !self.geo_lod_root_node(node).is_empty() || self.url_load_state(root) != LoadState::Complete {
            return;
        }

        self.geo_lod_set_children_loaded(node, false);
        let roots = self.inline_root_nodes(root);
        self.geo_lod_set_children(node, roots);
    }

    /// GeoLOD.set_childLoadState__: once the four child Inlines are COMPLETE or FAILED, the
    /// children are their scenes' root nodes.
    pub(crate) fn geo_lod_set_child_load_state(&mut self, node: NodeId) {
        let Some((_, children)) = self.geo_lod_inlines(node) else { return };

        if self.geo_lod_level(node) != 1 {
            return;
        }

        let loaded = children.iter().filter(|c| matches!(self.url_load_state(**c), LoadState::Complete | LoadState::Failed)).count();

        if loaded != 4 {
            return;
        }

        self.geo_lod_set_children_loaded(node, true);
        let roots = children.iter().flat_map(|c| self.inline_root_nodes(*c)).collect();
        self.geo_lod_set_children(node, roots);
    }

    /// GeoLOD.changeLevel (DISPLAY traversal) with the level of getLevel.
    pub fn geo_lod_change_level(&mut self, node: NodeId, level: i32) {
        if level == self.geo_lod_level(node) {
            return;
        }

        let Some((root, children)) = self.geo_lod_inlines(node) else { return };

        if let Some(k) = self.pre(node, "level_changed") {
            self.set_value(k, FieldValue::SFInt32(level));
        }

        match level {
            0 => {
                let root_node = self.geo_lod_root_node(node);

                if !root_node.is_empty() {
                    self.geo_lod_set_children(node, root_node);
                    self.geo_lod_set_children_loaded(node, false);
                } else if self.url_load_state(root) == LoadState::Complete {
                    let roots = self.inline_root_nodes(root);
                    self.geo_lod_set_children(node, roots);
                    self.geo_lod_set_children_loaded(node, false);
                }

                // `this .unload` is never set: the child Inlines stay loaded.
            }
            _ => {
                if self.get_bool(children[0], "load") {
                    self.geo_lod_set_child_load_state(node);
                } else {
                    for c in children {
                        if let Some(k) = self.pre(c, "load") {
                            self.set_value(k, FieldValue::SFBool(true));
                        }
                    }
                }
            }
        }
    }

    /// GeoPositionInterpolator.interpolate.
    pub(crate) fn geo_position_interpolate(&mut self, node: NodeId, index0: usize, index1: usize, weight: f64) {
        let Some(FieldValue::MFVec3d(a)) = self.pre_value(node, "keyValue").cloned() else {
            return;
        };

        let geo = self.geo_object(node);
        let get = |i: usize| geo.get_coord(&Vector3::new(a[i * 3], a[i * 3 + 1], a[i * 3 + 2]));
        let coord = geocentric_slerp(get(index0), get(index1), weight);
        let geovalue = geo.get_geo_coord(&coord);

        if let Some(k) = self.pre(node, "geovalue_changed") {
            self.set_value(k, FieldValue::SFVec3d(geovalue));
        }

        if let Some(k) = self.pre(node, "value_changed") {
            self.set_value(k, FieldValue::SFVec3f(coord));
        }
    }
}
