//! X3DNBodyCollidableNode, CollidableShape and CollidableOffset (X_ITE `X3DNBodyCollidableNode.js`,
//! `CollidableShape.js`, `CollidableOffset.js`).
//!
//! # Geometry provider
//!
//! A CollidableShape whose geometry is not a Box, Cone, Cylinder or Sphere (an IndexedFaceSet, an
//! Extrusion, ...) is a `btBvhTriangleMeshShape` of the geometry node's triangles. This crate cannot
//! tessellate; the triangles come from the process-global geometry provider that the embedder installs
//! (`x3d_render::traverse::install_particle_geometry`, called by every `Viewer3d`), see
//! [`crate::nodes::particle_systems`]. Without a provider such a shape has **no collision shape** (bodies
//! fall through it) and the runtime logs one warning per node. First install wins, process wide.

use super::arena::{Home, PWorld, clone_shape_tree};
use super::body::to_transform;
use super::*;
use crate::nodes::particle_systems::{GeometryRequest, geometry_provider};
use crate::physics::bt::math::{Tf, V3};
use crate::physics::bt::shape::ShapeId;
use std::collections::HashMap;

/// The members of an X3DNBodyCollidableNode.
#[derive(Debug)]
pub struct CollidableRt {
    pub(super) home: Home,
    /// `this .compoundShape`.
    pub(super) compound: ShapeId,
    /// `this .collisionShape` (a CollidableShape): the only child of the compound.
    pub(super) collision_shape: Option<ShapeId>,
    pub(super) offset: Vector3,
    pub(super) matrix: Matrix4,
    /// `this .childNode`: the Shape of a CollidableShape, the collidable of a CollidableOffset.
    pub(super) child: Option<NodeId>,
    /// `this .geometryNode` of a CollidableShape.
    pub(super) geometry_node: Option<NodeId>,
    pub(super) is_offset: bool,
    pub(super) disposed: bool,
}

/// `getGeometryType () > 1`: the geometry has surfaces (not points or lines).
fn has_surfaces(name: &str) -> bool {
    !matches!(
        name,
        "PointSet"
            | "IndexedLineSet"
            | "LineSet"
            | "Polypoint2D"
            | "Polyline2D"
            | "Arc2D"
            | "Circle2D"
            | "GeoPoint"
    )
}

fn is_any_node(_: &str) -> bool {
    true
}

impl Runtime {
    fn col_rt(&self, node: NodeId) -> &CollidableRt {
        match self.phys(node) {
            Some(PhysState::Collidable(c)) => c,
            _ => panic!("node {node} is not a collidable"),
        }
    }

    fn col_rt_mut(&mut self, node: NodeId) -> &mut CollidableRt {
        match self.phys_mut(node) {
            Some(PhysState::Collidable(c)) => c,
            _ => panic!("node {node} is not a collidable"),
        }
    }

    pub(super) fn collidable_construct(&mut self, node: NodeId) {
        let is_offset = self.builtin_name(node) == Some("CollidableOffset");
        let compound = self.with_world(None, |w| w.new_compound());

        self.nrt_mut(node).state = State::Physics(PhysState::Collidable(Box::new(CollidableRt {
            home: None,
            compound,
            collision_shape: None,
            offset: Vector3::ZERO,
            matrix: Matrix4::IDENTITY,
            child: None,
            geometry_node: None,
            is_offset,
            disposed: false,
        })));
    }

    pub(super) fn collidable_initialize(&mut self, node: NodeId, name: &str) {
        use PhysicsCb::*;

        // X3DNBodyCollidableNode.initialize
        if let Some(k) = self.private_field(node, "rebuild") {
            self.add_interest(k, node, Cb::Physics(CollidableSetChild));
        }

        self.add_node_interest(node, node, Cb::Physics(CollidableEventsProcessed));
        self.collidable_events_processed(node);

        self.pcb(node, "enabled", CollidableSetCollidableGeometry);

        if name == "CollidableShape" {
            self.pcb(node, "shape", CollidableRequestRebuild);
        } else {
            self.pcb(node, "collidable", CollidableRequestRebuild);
        }

        self.collidable_set_child(node);
    }

    pub(super) fn collidable_compound(&self, node: NodeId) -> ShapeId {
        self.col_rt(node).compound
    }

    /// `getBody ()`.
    pub(super) fn collidable_body(&self, node: NodeId) -> Option<NodeId> {
        self.private_field(node, "body")
            .and_then(|k| self.sfnode_of(k))
    }

    /// `setBody (value)`.
    pub(super) fn collidable_set_body(&mut self, node: NodeId, value: Option<NodeId>) {
        if let Some(k) = self.private_field(node, "body") {
            self.set_value(k, FieldValue::SFNode(value));
        }
    }

    /// `getLocalTransform ()`.
    fn collidable_local_transform(&self, node: NodeId) -> Tf {
        let c = self.col_rt(node);
        let mut m = c.matrix;

        m.translate(&c.offset);
        to_transform(&m.0)
    }

    /// `requestRebuild ()`.
    pub(super) fn collidable_request_rebuild(&mut self, node: NodeId) {
        if let Some(k) = self.private_field(node, "rebuild") {
            self.add_event(k);
        }
    }

    /// `eventsProcessed ()`.
    pub(super) fn collidable_events_processed(&mut self, node: NodeId) {
        let translation = self.pv3(node, "translation");
        let rotation = self.prot(node, "rotation");
        let m = transform_matrix(&translation, &rotation);

        self.col_rt_mut(node).matrix = m;

        let (home, compound) = {
            let c = self.col_rt(node);

            (c.home, c.compound)
        };
        let tf = self.collidable_local_transform(node);

        self.with_world(home, |pw| {
            if pw.w.shapes.num_child_shapes(compound) > 0 {
                pw.w.shapes.update_child_transform(compound, 0, tf, true);
            }
        });
    }

    /// `set_child__ ()` of CollidableShape and CollidableOffset.
    pub(super) fn collidable_set_child(&mut self, node: NodeId) {
        use PhysicsCb::*;

        let is_offset = self.col_rt(node).is_offset;
        let old = self.col_rt(node).child;

        if is_offset {
            if let Some(child) = old {
                self.remove_node_interest(child, node, Cb::Physics(CollidableChildNodeEvent));

                if let (Some(from), Some(to)) = (
                    self.private_field(child, "compoundShape"),
                    self.private_field(node, "compoundShape"),
                ) {
                    self.remove_field_interest(from, to);
                }
            }

            let child = self.phys_cast_field(node, "collidable", is_collidable);

            self.col_rt_mut(node).child = child;

            if let Some(child) = child {
                self.add_node_interest(child, node, Cb::Physics(CollidableChildNodeEvent));

                if let (Some(from), Some(to)) = (
                    self.private_field(child, "compoundShape"),
                    self.private_field(node, "compoundShape"),
                ) {
                    self.add_field_interest(from, to);
                }
            }

            self.collidable_set_collidable_geometry(node);
        } else {
            if let Some(shape) = old {
                if let Some(k) = self.pre(shape, "geometry") {
                    self.remove_interest(k, node, Cb::Physics(CollidableSetGeometry));
                }
            }

            let shape = self.phys_cast_field(node, "shape", is_shape);

            self.col_rt_mut(node).child = shape;

            if let Some(shape) = shape {
                if let Some(k) = self.pre(shape, "geometry") {
                    self.add_interest(k, node, Cb::Physics(CollidableSetGeometry));
                }
            }

            self.collidable_set_geometry(node);
        }
    }

    /// CollidableShape `set_geometry__ ()`.
    pub(super) fn collidable_set_geometry(&mut self, node: NodeId) {
        if let Some(g) = self.col_rt(node).geometry_node {
            self.remove_node_interest(
                g,
                node,
                Cb::Physics(PhysicsCb::CollidableSetCollidableGeometry),
            );
        }

        let geometry = self
            .col_rt(node)
            .child
            .and_then(|shape| self.phys_cast_field(shape, "geometry", is_any_node));

        self.col_rt_mut(node).geometry_node = geometry;

        if let Some(g) = geometry {
            self.add_node_interest(
                g,
                node,
                Cb::Physics(PhysicsCb::CollidableSetCollidableGeometry),
            );

            if matches!(self.builtin_name(g), Some("Sphere" | "Cone" | "Cylinder"))
                && self.physics.geometry_watch.insert(g)
            {
                // A geometry that is live when the collidable first sees it has made its `_rebuild` long ago.
                if self.time_node_live(g) {
                    self.physics.geometry_rebuilt.insert(g);
                }

                self.watch_live(g);
            }
        }

        self.collidable_set_collidable_geometry(node);
    }

    /// `set_collidableGeometry__ ()`.
    pub(super) fn collidable_set_collidable_geometry(&mut self, node: NodeId) {
        if self.col_rt(node).is_offset {
            self.offset_set_collidable_geometry(node);
        } else {
            self.shape_set_collidable_geometry(node);
        }
    }

    fn offset_set_collidable_geometry(&mut self, node: NodeId) {
        let child = self.col_rt(node).child;
        let enabled = self.get_bool(node, "enabled");

        // The child's shapes have to live in the same world.
        if let (Some(child), true) = (child, enabled) {
            let home = self.col_rt(node).home;

            self.collidable_migrate(child, home);
        }

        let (home, compound) = {
            let c = self.col_rt(node);

            (c.home, c.compound)
        };
        let child_compound = child.map(|c| self.collidable_compound(c));
        let tf = self.collidable_local_transform(node);

        self.with_world(home, |pw| {
            if pw.w.shapes.num_child_shapes(compound) > 0 {
                pw.w.shapes.remove_child_shape_by_index(compound, 0);
            }

            if let (Some(cc), true) = (child_compound, enabled) {
                pw.w.shapes.add_child_shape(compound, tf, cc);
            }
        });

        if let Some(k) = self.private_field(node, "compoundShape") {
            self.set_value(k, FieldValue::SFTime(self.now));
        }
    }

    fn shape_set_collidable_geometry(&mut self, node: NodeId) {
        let (home, compound) = {
            let c = self.col_rt(node);

            (c.home, c.compound)
        };

        // `localScaling .setValue (ls .x (), ls .y (), ls .z ())`
        let local_scaling: V3 = self.with_world(home, |pw| pw.w.shapes.get_local_scaling(compound));

        self.shape_remove_collidable_geometry(node);
        self.col_rt_mut(node).offset = Vector3::ZERO;
        self.with_world(home, |pw| {
            pw.w.shapes.set_local_scaling(compound, [1.0, 1.0, 1.0])
        });

        let geometry = self.col_rt(node).geometry_node;
        let enabled = self.get_bool(node, "enabled");
        let mut shape: Option<ShapeId> = None;

        if let (true, Some(g)) = (enabled, geometry) {
            let name = self.builtin_name(g).unwrap_or("");

            if has_surfaces(name) {
                shape = self.shape_create(node, g, name);
            }
        }

        if let Some(cs) = shape {
            let tf = self.collidable_local_transform(node);

            self.with_world(home, |pw| pw.w.shapes.add_child_shape(compound, tf, cs));
        }

        self.col_rt_mut(node).collision_shape = shape;
        self.with_world(home, |pw| {
            pw.w.shapes.set_local_scaling(compound, local_scaling)
        });

        self.add_node_event(node);

        if let Some(k) = self.private_field(node, "compoundShape") {
            self.set_value(k, FieldValue::SFTime(self.now));
        }
    }

    /// The `switch (type [t])` of CollidableShape.set_collidableGeometry__.
    fn shape_create(&mut self, node: NodeId, g: NodeId, name: &str) -> Option<ShapeId> {
        let home = self.col_rt(node).home;

        match name {
            "Box" => {
                let size = self.pv3(g, "size");

                Some(self.with_world(home, |pw| {
                    pw.new_shape(|s| s.new_box([size.x / 2.0, size.y / 2.0, size.z / 2.0]))
                }))
            }
            "Cone" => {
                if self.get_bool(g, "side") && self.get_bool(g, "bottom") {
                    let (radius, height) =
                        (self.get_f64(g, "bottomRadius"), self.get_f64(g, "height"));

                    Some(self.with_world(home, |pw| pw.new_shape(|s| s.new_cone(radius, height))))
                } else {
                    self.shape_create_concave(home, g)
                }
            }
            "Cylinder" => {
                let (radius, height_1_2) =
                    (self.get_f64(g, "radius"), self.get_f64(g, "height") * 0.5);

                if self.get_bool(g, "side") && self.get_bool(g, "top") && self.get_bool(g, "bottom")
                {
                    Some(self.with_world(home, |pw| {
                        pw.new_shape(|s| s.new_cylinder([radius, height_1_2, radius]))
                    }))
                } else {
                    self.shape_create_concave(home, g)
                }
            }
            "ElevationGrid" => {
                let x_dimension = match self.pre_value(g, "xDimension") {
                    Some(FieldValue::SFInt32(v)) => *v,
                    _ => 0,
                };
                let z_dimension = match self.pre_value(g, "zDimension") {
                    Some(FieldValue::SFInt32(v)) => *v,
                    _ => 0,
                };

                if x_dimension > 1 && z_dimension > 1 {
                    let height: Vec<f64> = match self.pre_value(g, "height") {
                        Some(FieldValue::MFFloat(v)) => v.iter().map(|&h| f64::from(h)).collect(),
                        _ => vec![],
                    };
                    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);

                    for &h in &height {
                        min = x3d_fields::jsmath::js_min(min, h);
                        max = x3d_fields::jsmath::js_max(max, h);
                    }

                    // `Ammo ._malloc (4 * xDimension * zDimension)`, filled from `height`.
                    let mut heights = vec![0.0; (x_dimension * z_dimension) as usize];

                    for (i, &h) in height.iter().enumerate().take(heights.len()) {
                        heights[i] = h;
                    }

                    let (x_spacing, z_spacing) =
                        (self.get_f64(g, "xSpacing"), self.get_f64(g, "zSpacing"));
                    let shape = self.with_world(home, |pw| {
                        pw.new_shape(|s| {
                            s.new_heightfield(
                                x_dimension as usize,
                                z_dimension as usize,
                                &heights,
                                min,
                                max,
                                [x_spacing, 1.0, z_spacing],
                            )
                        })
                    });

                    self.col_rt_mut(node).offset = Vector3::new(
                        x_spacing * f64::from(x_dimension - 1) * 0.5,
                        (min + max) * 0.5,
                        z_spacing * f64::from(z_dimension - 1) * 0.5,
                    );
                    Some(shape)
                } else {
                    None
                }
            }
            "Sphere" => {
                let radius = self.get_f64(g, "radius");

                Some(self.with_world(home, |pw| pw.new_shape(|s| s.new_sphere(radius))))
            }
            _ => self.shape_create_concave(home, g),
        }
    }

    /// `createConcaveGeometry ()`: a btBvhTriangleMeshShape of the vertices (3 x 4 floats per triangle).
    fn shape_create_concave(&mut self, home: Home, g: NodeId) -> Option<ShapeId> {
        let Some(provider) = geometry_provider() else {
            self.warn_no_geometry_provider(g, "the concave CollidableShape geometry");
            return None;
        };
        let soup = provider(self, &GeometryRequest::Node(g))?;
        let vertices = soup.vertices;

        if vertices.is_empty() {
            return None;
        }

        let mut triangles: Vec<[V3; 3]> = vec![];
        let mut i = 0;

        while i < vertices.len() {
            let p = |o: usize| {
                [
                    f64::from(vertices[i + o]),
                    f64::from(vertices[i + o + 1]),
                    f64::from(vertices[i + o + 2]),
                ]
            };

            if i + 11 > vertices.len() {
                break;
            }

            triangles.push([p(0), p(4), p(8)]);
            i += 12;
        }

        Some(self.with_world(home, |pw| {
            pw.new_shape(|s| s.new_bvh_triangle_mesh(&triangles))
        }))
    }

    /// `removeCollidableGeometry ()`.
    fn shape_remove_collidable_geometry(&mut self, node: NodeId) {
        let (home, compound, shape) = {
            let c = self.col_rt(node);

            (c.home, c.compound, c.collision_shape)
        };

        if let Some(cs) = shape {
            self.with_world(home, |pw| {
                pw.w.shapes.remove_child_shape_by_index(compound, 0);
                pw.free_shape(cs);
            });
            self.col_rt_mut(node).collision_shape = None;
        }
    }

    // --- worlds ------------------------------------------------------------------------------

    /// The collidables of this collidable's tree (itself first; the child of a CollidableOffset below).
    pub(super) fn collidable_tree(&self, node: NodeId, out: &mut Vec<NodeId>) {
        if out.contains(&node) {
            return;
        }

        out.push(node);

        let c = self.col_rt(node);

        if let (true, Some(child)) = (c.is_offset, c.child) {
            if matches!(self.phys(child), Some(PhysState::Collidable(_))) {
                self.collidable_tree(child, out);
            }
        }
    }

    /// The shapes a collidable owns (its compound and collision shape) go along even when the
    /// compound hierarchy does not reach them (a disabled CollidableOffset holds no child).
    pub(super) fn collidable_clone_own_shapes(
        &self,
        tree: &[NodeId],
        src: &PWorld,
        dst: &mut PWorld,
        map: &mut HashMap<ShapeId, ShapeId>,
    ) {
        for &g in tree {
            let c = self.col_rt(g);

            clone_shape_tree(src, dst, c.compound, map);

            if let Some(cs) = c.collision_shape {
                clone_shape_tree(src, dst, cs, map);
            }
        }
    }

    /// After a copy of the shape tree: the new ids of a collidable of the tree.
    pub(super) fn collidable_remap(
        &mut self,
        node: NodeId,
        to: Home,
        map: &HashMap<ShapeId, ShapeId>,
    ) {
        let c = self.col_rt_mut(node);

        c.home = to;

        if let Some(&n) = map.get(&c.compound) {
            c.compound = n;
        }

        if let Some(cs) = c.collision_shape {
            if let Some(&n) = map.get(&cs) {
                c.collision_shape = Some(n);
            }
        }
    }

    /// Move the shape tree of a collidable (and the collidables below it) into the world `to`.
    pub(super) fn collidable_migrate(&mut self, node: NodeId, to: Home) {
        let from = self.col_rt(node).home;

        if from == to {
            return;
        }

        let mut tree = vec![];

        self.collidable_tree(node, &mut tree);

        let compound = self.col_rt(node).compound;
        let mut src = self.take_world(from);
        let mut dst = self.take_world(to);
        let mut map = HashMap::new();

        clone_shape_tree(&src, &mut dst, compound, &mut map);
        self.collidable_clone_own_shapes(&tree, &src, &mut dst, &mut map);

        for &old in map.keys() {
            src.free_shape(old);
        }

        self.put_world(to, dst);
        self.put_world(from, src);

        for g in tree {
            self.collidable_remap(g, to, &map);
        }
    }

    /// `dispose ()`.
    pub(super) fn collidable_dispose(&mut self, node: NodeId) {
        if self.col_rt(node).disposed {
            return;
        }

        let (home, compound, shape) = {
            let c = self.col_rt(node);

            (c.home, c.compound, c.collision_shape)
        };
        let usable = match home {
            None => true,
            Some(c) => {
                matches!(self.phys(c), Some(PhysState::Collection(rt)) if rt.world.is_some())
            }
        };

        if usable {
            self.with_world(home, |pw| {
                if let Some(cs) = shape {
                    pw.free_shape(cs);
                }

                pw.free_shape(compound);
            });
        }

        let c = self.col_rt_mut(node);

        c.disposed = true;
        c.collision_shape = None;
        c.child = None;
        c.geometry_node = None;
    }
}
