//! InlineGeometry (Components/Networking/InlineGeometry.js, X3D 4.1, experimental): a geometry node whose
//! geometry is the first X3DGeometryNode of a scene it loads by `url` (or the node the url's `#name`
//! fragment names). REA batch 08, lane net.
//!
//! The node loads like an Inline (`FileLoader .createX3DFromURL`), so the load, the loadCount and
//! next-frame waits, `load`, `url`, autoRefresh, loadState and the disposal of the scene are the code of
//! `inline.rs`; this file is what `InlineGeometry .setInternalScene` adds:
//!
//! * the geometry search (`getGeometryFromArray`, `getGeometryFromNode`: depth first through the SFNode and
//!   MFNode fields of every node, `getFields ()` order, PROTO instances through their own fields),
//! * `getInnerNode ()`: a Shape (or anything else that asks for an X3DGeometryNode) gets the geometry of the
//!   loaded scene instead of the InlineGeometry (see [`Runtime::inline_geometry_inner`], used by the
//!   renderer's `Sg::inner`),
//! * `set_solid__` and `set_smooth__`: the node's `solid` and `smooth` are written into the loaded
//!   geometry (`solid` of a 2D or 3D geometry, `creaseAngle` and `normalPerVertex` of a 3D geometry
//!   without a `normal` node),
//! * the errors "Named node 'x' not found." and "No X3DGeometryNode found.": thrown in
//!   `setInternalScene`, they are the error of that candidate url, so the next url is tried.
//!
//! The lock and copy rule (a geometry that another InlineGeometry already uses is `copy ()`ed) exists in X_ITE
//! because the scene cache (browser option Cache) hands the same scene to every node with that url. Here each
//! node parses its own scene, which is the copy; no node ever shares a geometry, so there is nothing to lock.
//! The difference is visible only through the cache itself (the same url fetched once), not in a scene or a picture.

use crate::inline::SceneArg;
use crate::nodes::LoadState;
use crate::*;

impl Runtime {
    /// X3DConstants.X3DGeometryNode: the nodes whose containerField is "geometry".
    fn is_geometry_node(&self, n: NodeId) -> bool {
        match self.scene.node(n).kind {
            NodeKind::Builtin(i) => self.scene.table.types[i].container_field == "geometry",
            _ => false,
        }
    }

    /// `X3DCast (X3DConstants .X3DGeometryNode, node)`: the node itself or, for a PROTO instance, the first root
    /// node of its body (getInnerNode).
    fn cast_geometry(&self, node: NodeId) -> Option<NodeId> {
        self.inner_node(node).filter(|&n| self.is_geometry_node(n) && self.builtin_name(n) != Some("InlineGeometry"))
    }

    /// InlineGeometry.getGeometryFromArray.
    fn geometry_from_array(&self, nodes: &[Option<NodeId>], depth: usize) -> Option<NodeId> {
        nodes.iter().flatten().find_map(|&n| self.geometry_from_node(n, depth))
    }

    /// InlineGeometry.getGeometryFromNode: the node if it is a geometry, else the first geometry found in its
    /// fields (SFNode and MFNode, predefined fields first, then the user-defined ones).
    fn geometry_from_node(&self, node: NodeId, depth: usize) -> Option<NodeId> {
        if depth > 256 {
            return None;
        }

        if let Some(g) = self.cast_geometry(node) {
            return Some(g);
        }

        let n = self.scene.node(node);

        for field in n.fields.iter().chain(n.user_fields.iter()) {
            let found = match &field.value {
                FieldValue::SFNode(Some(c)) => self.geometry_from_node(*c, depth + 1),
                FieldValue::MFNode(v) => self.geometry_from_array(v, depth + 1),
                _ => None,
            };

            if found.is_some() {
                return found;
            }
        }

        None
    }

    /// The geometry `setInternalScene` picks in the scene of root context `root` loaded from `url`: the node named
    /// by the fragment, else the first geometry of the root nodes.
    pub(crate) fn inline_geometry_find(&self, root: CtxId, url: &url::Url) -> Result<NodeId, String> {
        let hash = if url.scheme() == "data" { "" } else { url.fragment().unwrap_or("") };

        let found = if hash.is_empty() {
            self.geometry_from_array(&self.scene.ctx(root).root_nodes, 0)
        } else {
            match self.scene.get_named_node(root, hash) {
                Some(n) => self.cast_geometry(n),
                None => return Err(format!("Error: Named node '{hash}' not found.")),
            }
        };

        found.ok_or_else(|| "Error: No X3DGeometryNode found.".to_string())
    }

    /// `getInnerNode ()`: the geometry of the loaded scene; `None` while there is none ("Geometry node not available.").
    pub fn inline_geometry_inner(&self, node: NodeId) -> Option<NodeId> {
        self.inline_geometries.get(&node).copied()
    }

    /// X3DGeometryNode.getGeometryType () of the nodes `set_solid__` and `set_smooth__` look at: 0 points,
    /// 1 lines, 2 flat 2D geometry, 3 everything else (the default).
    fn geometry_type(&self, n: NodeId) -> u8 {
        match self.builtin_name(n) {
            Some("PointSet" | "Polypoint2D") => 0,
            Some("IndexedLineSet" | "LineSet" | "Arc2D" | "Circle2D" | "Polyline2D") => 1,
            Some("Rectangle2D" | "ArcClose2D" | "TriangleSet2D") => 2,
            // Disk2D.build: a point (both radii 0), a circle (equal radii), else a disk or an annulus.
            Some("Disk2D") => {
                let (a, b) = (self.get_f64(n, "innerRadius").abs(), self.get_f64(n, "outerRadius").abs());
                let (inner, outer) = (a.min(b), a.max(b));

                if inner == outer {
                    if outer == 0.0 { 0 } else { 1 }
                } else {
                    2
                }
            }
            _ => 3,
        }
    }

    /// InlineGeometry.setInternalScene with a scene: false when there is no geometry in it (the load is FAILED).
    pub(crate) fn inline_geometry_set_scene(&mut self, node: NodeId, ctx: CtxId) -> bool {
        // The resolved url of the scene that was loaded: its world URL carries the fragment.
        let world = url::Url::parse(self.scene.world_url_of(ctx)).ok();

        let Some(world) = world else { return false };

        match self.inline_geometry_find(ctx, &world) {
            Ok(g) => {
                self.inline_geometries.insert(node, g);
                self.inline_geometry_set_solid(node);
                self.inline_geometry_set_smooth(node);
                true
            }
            Err(_) => false,
        }
    }

    /// InlineGeometry.set_solid__.
    pub(crate) fn inline_geometry_set_solid(&mut self, node: NodeId) {
        let Some(&g) = self.inline_geometries.get(&node) else { return };

        if self.geometry_type(g) < 2 {
            return;
        }

        let solid = self.get_bool(node, "solid");

        if let Some(k) = self.pre(g, "solid") {
            self.set_value(k, FieldValue::SFBool(solid));
        }
    }

    /// InlineGeometry.set_smooth__, quirks included: a geometry type below 3 or a `normal` node is left alone, and
    /// a `creaseAngle` that already has the wanted value ends the function before `normalPerVertex` is looked at.
    pub(crate) fn inline_geometry_set_smooth(&mut self, node: NodeId) {
        let Some(&g) = self.inline_geometries.get(&node) else { return };

        if self.geometry_type(g) < 3 {
            return;
        }

        if matches!(self.pre_value(g, "normal"), Some(FieldValue::SFNode(Some(_)))) {
            return;
        }

        let smooth = self.get_bool(node, "smooth");

        if let Some(k) = self.pre(g, "creaseAngle") {
            let crease = if smooth { std::f64::consts::PI } else { 0.0 };

            if matches!(self.value(k), FieldValue::SFFloat(v) if *v == crease) {
                return;
            }

            self.set_value(k, FieldValue::SFFloat(crease));
        }

        if let Some(k) = self.pre(g, "normalPerVertex") {
            if matches!(self.value(k), FieldValue::SFBool(v) if *v == smooth) {
                return;
            }

            self.set_value(k, FieldValue::SFBool(smooth));
        }
    }

    /// `this .geometryNode = null` and the state of an unloaded or failed node.
    pub(crate) fn inline_geometry_clear(&mut self, node: NodeId) {
        self.inline_geometries.remove(&node);
    }

    /// The load state InlineGeometry.setInternalScene ends in for `scene`.
    pub(crate) fn inline_geometry_state(&mut self, node: NodeId, scene: SceneArg) -> Option<LoadState> {
        match scene {
            SceneArg::Scene(ctx) => Some(if self.inline_geometry_set_scene(node, ctx) { LoadState::Complete } else { LoadState::Failed }),
            // unloadData: `setInternalScene (null)`, a FAILED state (requestUnload then makes it NOT_STARTED).
            SceneArg::Default | SceneArg::None => Some(LoadState::Failed),
        }
    }
}
