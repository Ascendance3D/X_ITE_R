//! Read-only access to the runtime's scene graph: typed field getters, PROTO inner nodes
//! (X3DPrototypeInstance.getInnerNode) and Inline scenes.

use std::collections::HashMap;
use x3d_fields::FieldValue;
use x3d_fields::NodeId;
use x3d_fields::math::{Rotation4, Vector2, Vector3, Vector4};
use x3d_runtime::Runtime;
use x3d_scene::{CtxId, NodeKind, Outer};

pub struct Sg<'a> {
    pub rt: &'a Runtime,
    /// Instance node → body context.
    bodies: HashMap<NodeId, CtxId>,
}

impl<'a> Sg<'a> {
    pub fn new(rt: &'a Runtime) -> Self {
        // The current body of each instance; a replaced body is not drawn.
        let bodies: HashMap<NodeId, CtxId> = rt.instance_bodies.clone();

        Sg { rt, bodies }
    }

    /// Builtin type name of a node (None for placeholders, internal nodes, instances).
    pub fn type_name(&self, n: NodeId) -> Option<&'static str> {
        self.rt.builtin_name(n)
    }

    /// X3DPrototypeInstance.getInnerNode, recursively; builtin nodes map to themselves.
    pub fn inner(&self, n: NodeId) -> Option<NodeId> {
        let mut n = n;

        for _ in 0..64 {
            match self.rt.scene.node(n).kind {
                // InlineGeometry.getInnerNode: the geometry of the scene it loaded (none while it has none).
                NodeKind::Builtin(_) if self.rt.builtin_name(n) == Some("InlineGeometry") => return self.rt.inline_geometry_inner(n),
                NodeKind::Builtin(_) => return Some(n),
                NodeKind::Instance(_) => {
                    let body = *self.bodies.get(&n)?;
                    n = (*self.rt.scene.ctx(body).root_nodes.first()?)?;
                }
                _ => return None,
            }
        }

        None
    }

    /// Root nodes of the runtime's scene (inner nodes of PROTO instances).
    pub fn roots(&self) -> Vec<NodeId> {
        self.roots_of(self.rt.scene.world)
    }

    /// Root nodes of the scene whose root context is `ctx` (the world's, or the one an Inline
    /// loaded: Inline.getInternalScene () .rootNodes).
    pub fn roots_of(&self, ctx: CtxId) -> Vec<NodeId> {
        self.rt.scene.ctx(ctx).root_nodes.iter().flatten().filter_map(|n| self.inner(*n)).collect()
    }

    /// Inline.getInternalScene: the root context of the scene an Inline attached.
    pub fn inline_scene(&self, inline: NodeId) -> Option<CtxId> {
        self.rt.inline_scenes.get(&inline).copied()
    }

    pub fn value(&self, n: NodeId, name: &str) -> Option<&FieldValue> {
        self.rt.pre_value(n, name)
    }

    pub fn bool(&self, n: NodeId, name: &str, default: bool) -> bool {
        match self.value(n, name) {
            Some(FieldValue::SFBool(b)) => *b,
            _ => default,
        }
    }

    pub fn f64(&self, n: NodeId, name: &str, default: f64) -> f64 {
        match self.value(n, name) {
            Some(FieldValue::SFFloat(v) | FieldValue::SFDouble(v) | FieldValue::SFTime(v)) => *v,
            Some(FieldValue::SFInt32(v)) => *v as f64,
            _ => default,
        }
    }

    pub fn i32(&self, n: NodeId, name: &str, default: i32) -> i32 {
        match self.value(n, name) {
            Some(FieldValue::SFInt32(v)) => *v,
            _ => default,
        }
    }

    pub fn vec3(&self, n: NodeId, name: &str, default: Vector3) -> Vector3 {
        match self.value(n, name) {
            Some(FieldValue::SFVec3f(v) | FieldValue::SFVec3d(v)) => *v,
            _ => default,
        }
    }

    pub fn vec2(&self, n: NodeId, name: &str, default: Vector2) -> Vector2 {
        match self.value(n, name) {
            Some(FieldValue::SFVec2f(v) | FieldValue::SFVec2d(v)) => *v,
            _ => default,
        }
    }

    pub fn color(&self, n: NodeId, name: &str, default: [f64; 3]) -> [f64; 3] {
        match self.value(n, name) {
            Some(FieldValue::SFColor(c)) => c.components(),
            _ => default,
        }
    }

    pub fn color4(&self, n: NodeId, name: &str, default: [f64; 4]) -> [f64; 4] {
        match self.value(n, name) {
            Some(FieldValue::SFColorRGBA(c)) => c.components(),
            _ => default,
        }
    }

    pub fn vec4(&self, n: NodeId, name: &str, default: Vector4) -> Vector4 {
        match self.value(n, name) {
            Some(FieldValue::SFVec4f(v) | FieldValue::SFVec4d(v)) => *v,
            _ => default,
        }
    }

    pub fn string(&self, n: NodeId, name: &str, default: &str) -> String {
        match self.value(n, name) {
            Some(FieldValue::SFString(s)) => s.clone(),
            _ => default.to_string(),
        }
    }

    pub fn rotation(&self, n: NodeId, name: &str) -> Rotation4 {
        match self.value(n, name) {
            Some(FieldValue::SFRotation(r)) => *r,
            _ => Rotation4::identity(),
        }
    }

    pub fn node(&self, n: NodeId, name: &str) -> Option<NodeId> {
        match self.value(n, name) {
            Some(FieldValue::SFNode(Some(c))) => self.inner(*c),
            _ => None,
        }
    }

    pub fn nodes(&self, n: NodeId, name: &str) -> Vec<NodeId> {
        match self.value(n, name) {
            Some(FieldValue::MFNode(v)) => v.iter().flatten().filter_map(|c| self.inner(*c)).collect(),
            _ => vec![],
        }
    }

    /// Like `nodes`, but keeps the raw (uninnered) node ids, so callers can detect instances.
    pub fn raw_nodes(&self, n: NodeId, name: &str) -> Vec<Option<NodeId>> {
        match self.value(n, name) {
            Some(FieldValue::MFNode(v)) => v.clone(),
            _ => vec![],
        }
    }

    pub fn f32s(&self, n: NodeId, name: &str) -> &[f32] {
        match self.value(n, name) {
            Some(
                FieldValue::MFVec3f(v)
                | FieldValue::MFVec2f(v)
                | FieldValue::MFColor(v)
                | FieldValue::MFColorRGBA(v)
                | FieldValue::MFFloat(v)
                | FieldValue::MFVec4f(v)
                // Matrix3VertexAttribute / Matrix4VertexAttribute (Batch 08).
                | FieldValue::MFMatrix3f(v)
                | FieldValue::MFMatrix4f(v),
            ) => v,
            _ => &[],
        }
    }

    pub fn f64s(&self, n: NodeId, name: &str) -> Vec<f64> {
        match self.value(n, name) {
            Some(FieldValue::MFRotation(v) | FieldValue::MFDouble(v) | FieldValue::MFVec3d(v) | FieldValue::MFVec2d(v)) => v.clone(),
            Some(FieldValue::MFFloat(v) | FieldValue::MFVec3f(v) | FieldValue::MFVec2f(v)) => v.iter().map(|x| *x as f64).collect(),
            _ => vec![],
        }
    }

    pub fn i32s(&self, n: NodeId, name: &str) -> &[i32] {
        match self.value(n, name) {
            Some(FieldValue::MFInt32(v)) => v,
            _ => &[],
        }
    }

    pub fn strings(&self, n: NodeId, name: &str) -> Vec<String> {
        match self.value(n, name) {
            Some(FieldValue::MFString(v)) => v.clone(),
            Some(FieldValue::SFString(s)) => vec![s.clone()],
            _ => vec![],
        }
    }

    pub fn image(&self, n: NodeId, name: &str) -> Option<&x3d_fields::Image> {
        match self.value(n, name) {
            Some(FieldValue::SFImage(i)) => Some(i),
            _ => None,
        }
    }

    /// The node's own context is inside an Inline'd scene (BindableList.getBound skips those).
    pub fn in_inline(&self, n: NodeId) -> bool {
        let mut c = self.rt.scene.node(n).ctx;

        loop {
            let ctx = self.rt.scene.ctx(c);

            if matches!(ctx.outer, Outer::Inline(_)) {
                return true;
            }

            match ctx.parent {
                Some(p) => c = p,
                None => return false,
            }
        }
    }

    /// CADFace.set_children__: `shape` when it is an LOD, a Transform or an X3DShapeNode
    /// (X3DCast (X3DChildNode), then the type switch of setChild); anything else is ignored.
    pub fn cad_face_child(&self, n: NodeId) -> Option<NodeId> {
        let c = self.node(n, "shape")?;
        matches!(self.type_name(c)?, "LOD" | "Transform" | "Shape" | "ParticleSystem").then_some(c)
    }

    /// GeoLOD.traverse: with `childrenLoaded ? level_changed : 0` at 0 the `rootNode` nodes (its
    /// private Group's children) or else the root Inline, at 1 the four child Inlines. `display`:
    /// the mode of the last DISPLAY traversal (what that frame drew).
    pub fn geo_lod_kids(&self, n: NodeId, display: bool) -> Vec<NodeId> {
        let Some((root, children)) = self.rt.geo_lod_inlines(n) else { return vec![] };

        match self.rt.geo_lod_mode(n, display) {
            0 => {
                let root_node = self.nodes(n, "rootNode");

                if self.raw_nodes(n, "rootNode").is_empty() { vec![root] } else { root_node }
            }
            1 => children.to_vec(),
            _ => vec![],
        }
    }

    /// X3DGeospatialObject: the reference frames of `geoSystem` and the GeoOrigin of `geoOrigin`.
    pub fn geo_object(&self, n: NodeId) -> x3d_runtime::geo::GeospatialObject {
        use x3d_runtime::geo::{GeoOriginInfo, GeospatialObject, geo_system};

        let origin = self.node(n, "geoOrigin").filter(|o| self.type_name(*o) == Some("GeoOrigin")).map(|o| GeoOriginInfo {
            origin: geo_system::reference_frame(&self.strings(o, "geoSystem"), false).convert(&self.vec3(o, "geoCoords", Vector3::ZERO)),
            rotate_y_up: self.bool(o, "rotateYUp", false),
        });

        GeospatialObject::new(&self.strings(n, "geoSystem"), origin, false)
    }

    /// The matrix an X3DTransformMatrix3DNode applies to its children: Transform, GeoLocation
    /// (getLocationMatrix (geoCoords)), GeoTransform (L⁻¹ · setTransform (…) · L at geoCenter).
    pub fn group_matrix(&self, n: NodeId, ty: &str) -> Option<x3d_fields::math::Matrix4> {
        use x3d_fields::math::Matrix4;

        match ty {
            // CADPart, HAnimJoint and HAnimSite are X3DTransformNodes with Transform's fields;
            // HAnimHumanoid applies them through its private Transform.
            "Transform" | "CADPart" | "HAnimHumanoid" | "HAnimJoint" | "HAnimSite" => {
                let mut m = Matrix4::IDENTITY;
                m.set_transform(
                    Some(&self.vec3(n, "translation", Vector3::ZERO)),
                    Some(&self.rotation(n, "rotation")),
                    Some(&self.vec3(n, "scale", Vector3::ONE)),
                    Some(&self.rotation(n, "scaleOrientation")),
                    Some(&self.vec3(n, "center", Vector3::ZERO)),
                );
                Some(m)
            }
            "GeoLocation" => Some(self.geo_object(n).get_location_matrix(&self.vec3(n, "geoCoords", Vector3::ZERO))),
            "GeoTransform" => {
                let mut location = self.geo_object(n).get_location_matrix(&self.vec3(n, "geoCenter", Vector3::ZERO));
                let mut m = Matrix4::IDENTITY;
                m.set_transform(
                    Some(&self.vec3(n, "translation", Vector3::ZERO)),
                    Some(&self.rotation(n, "rotation")),
                    Some(&self.vec3(n, "scale", Vector3::ONE)),
                    Some(&self.rotation(n, "scaleOrientation")),
                    None,
                );
                m.mult_right(&location);
                location.inverse();
                m.mult_left(&location);
                Some(m)
            }
            _ => None,
        }
    }

    /// GeoCoordinate.get1Point of every point, in double precision.
    pub fn geo_coord_points(&self, c: NodeId) -> Option<Vec<Vector3>> {
        (self.type_name(c)? == "GeoCoordinate").then(|| {
            let geo = self.geo_object(c);
            self.f64s(c, "point").chunks_exact(3).map(|v| geo.get_coord(&Vector3::new(v[0], v[1], v[2]))).collect()
        })
    }

    /// X3DCoordinateNode points: Coordinate, CoordinateDouble, GeoCoordinate (getCoord of each point).
    /// The children an HAnim node traverses (X3DGroupingNode.addChild with allowedTypes):
    /// HAnimHumanoid its private Transform's groups: skeleton (HAnimJoint, HAnimSite), viewpoints
    /// (HAnimSite), skin; HAnimJoint its HAnimJoint and HAnimSegment children (and HAnimSite up to
    /// specification version 3.3); HAnimSegment and HAnimSite all children.
    pub fn hanim_children(&self, n: NodeId, ty: &str) -> Option<[Vec<NodeId>; 3]> {
        let only = |field: &str, types: &[&str]| -> Vec<NodeId> { self.nodes(n, field).into_iter().filter(|c| self.type_name(*c).is_some_and(|t| types.contains(&t))).collect() };

        match ty {
            "HAnimHumanoid" => Some([only("skeleton", &["HAnimJoint", "HAnimSite"]), only("viewpoints", &["HAnimSite"]), self.nodes(n, "skin")]),
            "HAnimJoint" => {
                let legacy = self.rt.scene.spec_version_of(self.rt.scene.node(n).ctx).parse::<f64>().is_ok_and(|v| v <= 3.3);
                let types: &[&str] = if legacy { &["HAnimJoint", "HAnimSegment", "HAnimSite"] } else { &["HAnimJoint", "HAnimSegment"] };
                Some([only("children", types), vec![], vec![]])
            }
            "HAnimSegment" | "HAnimSite" => Some([self.nodes(n, "children"), vec![], vec![]]),
            _ => None,
        }
    }

    pub fn coord_points(&self, c: NodeId) -> Option<Vec<f32>> {
        match self.type_name(c)? {
            "Coordinate" | "CoordinateDouble" => Some(self.f32s(c, "point").to_vec()),
            "GeoCoordinate" => Some(self.geo_coord_points(c)?.iter().flat_map(|g| [g.x as f32, g.y as f32, g.z as f32]).collect()),
            _ => None,
        }
    }

    /// StaticGroup.createGroups: `groupNode .getShapes ([ ], IDENTITY)`, the visible shapes of the
    /// children with their model matrices. X3DGroupingNode.getShapes visits its visibleObjects
    /// (Switch: the chosen child; LOD: the child of the current level_changed), transform nodes
    /// apply their matrix, Inline uses its scene; Billboard rotation, sensors and lights are lost.
    pub fn static_shapes(&self, n: NodeId, matrix: &x3d_fields::math::Matrix4, out: &mut Vec<(NodeId, x3d_fields::math::Matrix4)>, depth: usize) {
        if depth > 256 {
            return;
        }

        let Some(ty) = self.type_name(n) else { return };

        if !self.bool(n, "visible", true) {
            return;
        }

        match ty {
            "Shape" => {
                if self.node(n, "geometry").is_some() {
                    out.push((n, *matrix));
                }
            }
            "Inline" => {
                if let Some(ctx) = self.inline_scene(n).filter(|_| self.bool(n, "load", true)) {
                    for r in self.roots_of(ctx) {
                        self.static_shapes(r, matrix, out, depth + 1);
                    }
                }
            }
            "Switch" => {
                let choice = self.i32(n, "whichChoice", -1);
                let mut kids = self.raw_nodes(n, "children");
                if kids.is_empty() {
                    kids = self.raw_nodes(n, "choice");
                }
                if choice >= 0 && (choice as usize) < kids.len() {
                    if let Some(c) = kids[choice as usize].and_then(|c| self.inner(c)) {
                        self.static_shapes(c, matrix, out, depth + 1);
                    }
                }
            }
            "LOD" => {
                let mut kids = self.nodes(n, "children");
                if kids.is_empty() {
                    kids = self.nodes(n, "level");
                }
                let level = self.i32(n, "level_changed", -1).min(kids.len() as i32 - 1);
                if level >= 0 {
                    self.static_shapes(kids[level as usize], matrix, out, depth + 1);
                }
            }
            "CADFace" => {
                if let Some(c) = self.cad_face_child(n) {
                    self.static_shapes(c, matrix, out, depth + 1);
                }
            }
            "GeoLOD" => {
                for c in self.geo_lod_kids(n, true) {
                    self.static_shapes(c, matrix, out, depth + 1);
                }
            }
            "Group" | "Transform" | "Anchor" | "Collision" | "Billboard" | "StaticGroup" | "GeoLocation" | "GeoTransform" | "CADAssembly" | "CADLayer" | "CADPart" | "PickableGroup" | "GroupAnnotation" => {
                let mut m = *matrix;
                if let Some(t) = self.group_matrix(n, ty) {
                    m.mult_left(&t);
                }
                for c in self.nodes(n, "children") {
                    self.static_shapes(c, &m, out, depth + 1);
                }
            }
            _ => {}
        }
    }
}
