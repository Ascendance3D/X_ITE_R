//! `X3DOptimizer.viewpointsCenterOfRotation` and `GLTF2Parser.skinsBBox`: both need the bounding
//! boxes and joint matrices X_ITE reads from a scene it just parsed. The boxes are X_ITE's Box3
//! (an oriented box held as a matrix; empty when m[15] is 0).

use super::*;
use x3d_fields::math::Vector3;

#[derive(Clone, Copy)]
pub(super) struct Box3 {
    m: Matrix4,
}

impl Box3 {
    pub fn empty() -> Self {
        Box3 {
            m: Matrix4([0.0; 16]),
        }
    }

    pub fn from_size_center(size: &Vector3, center: &Vector3) -> Self {
        Box3 {
            m: Matrix4([
                size.x / 2.0,
                0.0,
                0.0,
                0.0,
                0.0,
                size.y / 2.0,
                0.0,
                0.0,
                0.0,
                0.0,
                size.z / 2.0,
                0.0,
                center.x,
                center.y,
                center.z,
                1.0,
            ]),
        }
    }

    pub fn from_extents(min: &Vector3, max: &Vector3) -> Self {
        let (sx, sy, sz) = (
            (max.x - min.x) / 2.0,
            (max.y - min.y) / 2.0,
            (max.z - min.z) / 2.0,
        );
        let (cx, cy, cz) = (
            (max.x + min.x) / 2.0,
            (max.y + min.y) / 2.0,
            (max.z + min.z) / 2.0,
        );

        Box3 {
            m: Matrix4([
                sx, 0.0, 0.0, 0.0, 0.0, sy, 0.0, 0.0, 0.0, 0.0, sz, 0.0, cx, cy, cz, 1.0,
            ]),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.m.0[15] == 0.0
    }

    pub fn center(&self) -> Vector3 {
        Vector3::new(self.m.0[12], self.m.0[13], self.m.0[14])
    }

    fn absolute_extents(&self) -> (Vector3, Vector3) {
        let m = &self.m.0;
        let x = Vector3::new(m[0], m[1], m[2]);
        let y = Vector3::new(m[4], m[5], m[6]);
        let z = Vector3::new(m[8], m[9], m[10]);
        let r1 = Vector3::new(y.x + z.x, y.y + z.y, y.z + z.z);
        let r2 = Vector3::new(z.x - y.x, z.y - y.y, z.z - y.z);
        let p1 = Vector3::new(x.x + r1.x, x.y + r1.y, x.z + r1.z);
        let p4 = Vector3::new(x.x + r2.x, x.y + r2.y, x.z + r2.z);
        let p2 = Vector3::new(r1.x - x.x, r1.y - x.y, r1.z - x.z);
        let p3 = Vector3::new(r2.x - x.x, r2.y - x.y, r2.z - x.z);

        let mut min = p1;
        let mut max = p1;

        min.min(&p2).min(&p3).min(&p4);
        max.max(&p2).max(&p3).max(&p4);

        for p in [p1, p2, p3, p4] {
            let n = Vector3::new(-p.x, -p.y, -p.z);

            min.min(&n);
            max.max(&n);
        }

        (min, max)
    }

    pub fn extents(&self) -> (Vector3, Vector3) {
        let (mut min, mut max) = self.absolute_extents();
        let c = self.center();

        min.add(&c);
        max.add(&c);
        (min, max)
    }

    pub fn size(&self) -> Vector3 {
        let (min, mut max) = self.absolute_extents();

        max.subtract(&min);
        max
    }

    pub fn add(&mut self, other: &Box3) {
        if self.is_empty() {
            *self = *other;
            return;
        }

        if other.is_empty() {
            return;
        }

        let (mut lmin, mut lmax) = self.extents();
        let (rmin, rmax) = other.extents();

        lmin.min(&rmin);
        lmax.max(&rmax);
        *self = Box3::from_extents(&lmin, &lmax);
    }

    pub fn mult_right(&mut self, matrix: &Matrix4) {
        self.m.mult_right(matrix);
    }
}

fn vec3_of(v: FieldValue) -> Vector3 {
    match v {
        FieldValue::SFVec3f(v) | FieldValue::SFVec3d(v) => v,
        _ => Vector3::ZERO,
    }
}

fn rot_of(v: FieldValue) -> Rotation4 {
    match v {
        FieldValue::SFRotation(r) => r,
        _ => Rotation4::identity(),
    }
}

impl<'j, 's> P<'j, 's> {
    fn matrix_of(&self, node: NodeId) -> Matrix4 {
        let mut m = Matrix4::IDENTITY;

        m.set_transform(
            Some(&vec3_of(self.get(node, "translation"))),
            Some(&rot_of(self.get(node, "rotation"))),
            Some(&vec3_of(self.get(node, "scale"))),
            Some(&rot_of(self.get(node, "scaleOrientation"))),
            Some(&vec3_of(self.get(node, "center"))),
        );
        m
    }

    pub(super) fn children_of(&self, node: NodeId, field: &str) -> Vec<NodeId> {
        match self.get(node, field) {
            FieldValue::MFNode(v) => v.into_iter().flatten().collect(),
            _ => vec![],
        }
    }

    fn default_bbox_size(&self, node: NodeId) -> bool {
        self.has_field(node, "bboxSize")
            && vec3_of(self.get(node, "bboxSize")).equals(&Vector3::new(-1.0, -1.0, -1.0))
    }

    fn points_of(&self, coord: Option<NodeId>) -> Vec<f32> {
        match coord.map(|c| self.get(c, "point")) {
            Some(FieldValue::MFVec3f(v)) => v,
            _ => vec![],
        }
    }

    fn ints_of(&self, node: NodeId, field: &str) -> Vec<i32> {
        match self.get(node, field) {
            FieldValue::MFInt32(v) => v,
            _ => vec![],
        }
    }

    /// X3DGeometryNode.updateBBox: the box of the vertices the geometry builds.
    fn geometry_bbox(&self, geometry: NodeId) -> Box3 {
        let ty = self.type_name(geometry);
        let coord = if self.has_field(geometry, "coord") {
            self.get_node(geometry, "coord")
        } else {
            None
        };
        let points = self.points_of(coord);
        let count = points.len() / 3;
        let mut used: Vec<usize> = vec![];

        let strips = |index: &[i32]| -> Vec<Vec<usize>> {
            let mut out = vec![];
            let mut cur = vec![];

            for i in index {
                if *i < 0 {
                    out.push(std::mem::take(&mut cur));
                } else {
                    cur.push(*i as usize);
                }
            }

            if !cur.is_empty() {
                out.push(cur);
            }

            out
        };

        match ty.as_str() {
            "PointSet" => used.extend(0..count),
            "LineSet" => {
                let mut start = 0;

                for c in self.ints_of(geometry, "vertexCount") {
                    if c >= 2 {
                        used.extend(start..start + c as usize);
                    }

                    start += c.max(0) as usize;
                }
            }
            "IndexedLineSet" => {
                for s in strips(&self.ints_of(geometry, "coordIndex")) {
                    if s.len() >= 2 {
                        used.extend(s);
                    }
                }
            }
            "IndexedTriangleSet" => {
                let index = self.ints_of(geometry, "index");

                used.extend(
                    index[..index.len() - index.len() % 3]
                        .iter()
                        .filter(|i| **i >= 0)
                        .map(|i| *i as usize),
                );
            }
            "TriangleSet" => used.extend(0..count - count % 3),
            "TriangleStripSet" | "TriangleFanSet" => {
                let field = if ty == "TriangleStripSet" {
                    "stripCount"
                } else {
                    "fanCount"
                };
                let mut start = 0;

                for c in self.ints_of(geometry, field) {
                    if c >= 3 {
                        used.extend(start..start + c as usize);
                    }

                    start += c.max(0) as usize;
                }
            }
            "IndexedTriangleStripSet" | "IndexedTriangleFanSet" => {
                // These index lists have no separator in the glTF output: one strip.
                for s in strips(&self.ints_of(geometry, "index")) {
                    if s.len() >= 3 {
                        used.extend(s);
                    }
                }
            }
            _ => {}
        }

        let used: Vec<usize> = used.into_iter().filter(|i| *i < count).collect();

        if used.is_empty() {
            return Box3::from_extents(&Vector3::ZERO, &Vector3::ZERO);
        }

        let mut min = Vector3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut max = Vector3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);

        for i in used {
            let p = Vector3::new(
                points[i * 3] as f64,
                points[i * 3 + 1] as f64,
                points[i * 3 + 2] as f64,
            );

            min.min(&p);
            max.max(&p);
        }

        Box3::from_extents(&min, &max)
    }

    /// `node .getBBox (bbox)` for the nodes of a glTF scene; `None` is "not an X3DBoundedObject".
    pub(super) fn node_bbox(&self, node: NodeId, depth: usize) -> Option<Box3> {
        if depth > 1000 {
            return Some(Box3::empty());
        }

        let ty = self.type_name(node);

        match ty.as_str() {
            "Group" | "Transform" | "HAnimJoint" | "HAnimSegment" | "Switch" => {
                if !self.default_bbox_size(node) {
                    return Some(Box3::from_size_center(
                        &vec3_of(self.get(node, "bboxSize")),
                        &vec3_of(self.get(node, "bboxCenter")),
                    ));
                }

                let mut bbox = Box3::empty();
                let children = self.children_of(node, "children");

                let chosen: Vec<NodeId> = if ty == "Switch" {
                    match self.get(node, "whichChoice") {
                        FieldValue::SFInt32(i) if i >= 0 => {
                            children.get(i as usize).copied().into_iter().collect()
                        }
                        _ => vec![],
                    }
                } else {
                    children
                };

                for c in chosen {
                    let b = self.node_bbox(c, depth + 1).unwrap_or_else(Box3::empty);

                    bbox.add(&b);
                }

                if matches!(ty.as_str(), "Transform" | "HAnimJoint") {
                    bbox.mult_right(&self.matrix_of(node));
                }

                Some(bbox)
            }
            "HAnimHumanoid" => {
                if !self.default_bbox_size(node) {
                    return Some(Box3::from_size_center(
                        &vec3_of(self.get(node, "bboxSize")),
                        &vec3_of(self.get(node, "bboxCenter")),
                    ));
                }

                // Verified against X_ITE: while the parser runs, the humanoid's internal groups
                // have not processed their children yet, so its bounding box is empty (even with
                // Shape nodes below the skeleton).
                Some(Box3::empty())
            }
            "Shape" => {
                if !self.default_bbox_size(node) {
                    return Some(Box3::from_size_center(
                        &vec3_of(self.get(node, "bboxSize")),
                        &vec3_of(self.get(node, "bboxCenter")),
                    ));
                }

                match self.get_node(node, "geometry") {
                    Some(g) => Some(self.geometry_bbox(g)),
                    None => Some(Box3::empty()),
                }
            }
            "InstancedShape" => {
                if !self.default_bbox_size(node) {
                    return Some(Box3::from_size_center(
                        &vec3_of(self.get(node, "bboxSize")),
                        &vec3_of(self.get(node, "bboxCenter")),
                    ));
                }

                let Some(g) = self.get_node(node, "geometry") else {
                    return Some(Box3::empty());
                };
                let geometry = self.geometry_bbox(g);
                let mut bbox = Box3::empty();

                for m in self.instance_matrices(node) {
                    let mut sub = geometry;

                    sub.mult_right(&m);
                    bbox.add(&sub);
                }

                Some(bbox)
            }
            "GaussianSplats" => {
                if !self.default_bbox_size(node) {
                    return Some(Box3::from_size_center(
                        &vec3_of(self.get(node, "bboxSize")),
                        &vec3_of(self.get(node, "bboxCenter")),
                    ));
                }

                let points = match self.get(node, "positions") {
                    FieldValue::MFVec3f(v) => v,
                    _ => vec![],
                };

                if points.is_empty() {
                    return Some(Box3::empty());
                }

                let mut min = Vector3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
                let mut max = Vector3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);

                for p in points.chunks_exact(3) {
                    let p = Vector3::new(p[0] as f64, p[1] as f64, p[2] as f64);

                    min.min(&p);
                    max.max(&p);
                }

                Some(Box3::from_extents(&min, &max))
            }
            _ => None,
        }
    }

    /// InstancedShape.set_matrices__: one matrix per instance.
    fn instance_matrices(&self, node: NodeId) -> Vec<Matrix4> {
        let f32s = |name: &str, c: usize| -> Vec<Vec<f64>> {
            match self.get(node, name) {
                FieldValue::MFVec3f(v) | FieldValue::MFVec2f(v) => v
                    .chunks_exact(c)
                    .map(|x| x.iter().map(|y| *y as f64).collect())
                    .collect(),
                FieldValue::MFRotation(v) => v.chunks_exact(c).map(|x| x.to_vec()).collect(),
                _ => vec![],
            }
        };

        let translations = f32s("translations", 3);
        let rotations = f32s("rotations", 4);
        let scales = f32s("scales", 3);
        let scale_orientations = f32s("scaleOrientations", 4);
        let centers = f32s("centers", 3);
        let n = translations
            .len()
            .max(rotations.len())
            .max(scales.len())
            .max(scale_orientations.len())
            .max(centers.len());
        let mut out = vec![];

        for i in 0..n {
            let get3 = |a: &Vec<Vec<f64>>, d: Vector3| {
                a.get(i)
                    .map(|v| Vector3::new(v[0], v[1], v[2]))
                    .unwrap_or(d)
            };
            let get_r = |a: &Vec<Vec<f64>>| {
                a.get(i)
                    .map(|v| Rotation4::new(v[0], v[1], v[2], v[3]))
                    .unwrap_or_else(Rotation4::identity)
            };
            let mut m = Matrix4::IDENTITY;

            m.set_transform(
                Some(&get3(&translations, Vector3::ZERO)),
                Some(&get_r(&rotations)),
                Some(&get3(&scales, Vector3::ONE)),
                Some(&get_r(&scale_orientations)),
                Some(&get3(&centers, Vector3::ZERO)),
            );
            out.push(m);
        }

        out
    }

    /// `X3DOptimizer.viewpointsCenterOfRotation (scene)`
    pub(super) fn viewpoints_center_of_rotation(&mut self) {
        let mut bbox = Box3::empty();
        let roots: Vec<Option<NodeId>> = self.scene.ctx(ROOT).root_nodes.clone();
        let mut child = Box3::empty();

        for r in roots.iter().flatten() {
            if let Some(b) = self.node_bbox(*r, 0) {
                child = b;
            } else {
                child = Box3::empty();
            }

            bbox.add(&child);
        }

        let _ = child;
        let mut seen: HashSet<NodeId> = HashSet::new();
        let center = bbox.center();

        for r in roots.iter().flatten() {
            self.vcor_node(*r, &center, Matrix4::IDENTITY, &mut seen);
        }
    }

    fn vcor_node(
        &mut self,
        node: NodeId,
        center: &Vector3,
        model: Matrix4,
        seen: &mut HashSet<NodeId>,
    ) {
        if !seen.insert(node) {
            return;
        }

        let ty = self.type_name(node);
        let mut model = model;

        // `if (node .getMatrix) modelMatrix = modelMatrix .copy () .multLeft (node .getMatrix ())`
        if matches!(ty.as_str(), "Transform" | "HAnimJoint" | "HAnimHumanoid") {
            model.mult_left(&self.matrix_of(node));
        }

        if matches!(ty.as_str(), "Viewpoint" | "OrthoViewpoint") {
            let mut inv = model;

            inv.inverse();

            let mut p = *center;

            inv.mult_vec_matrix3(&mut p);
            self.set(node, "centerOfRotation", FieldValue::SFVec3f(p));
        }

        // for (const field of node .getFields ()) SFNode / MFNode
        let children: Vec<NodeId> = {
            let n = self.scene.node(node);

            n.fields
                .iter()
                .chain(n.user_fields.iter())
                .flat_map(|f| match &f.value {
                    FieldValue::SFNode(Some(c)) => vec![*c],
                    FieldValue::MFNode(v) => v.iter().flatten().copied().collect(),
                    _ => vec![],
                })
                .collect()
        };

        for c in children {
            self.vcor_node(c, center, model, seen);
        }
    }

    // --- skins -------------------------------------------------------------------------------------------------------

    pub(super) fn skins_bbox(&mut self) -> R<()> {
        for si in 0..self.skins.len() {
            if let Some(s) = &self.skins[si] {
                let humanoid = s.humanoid;

                self.skin_bbox(humanoid)?;
            }
        }

        Ok(())
    }

    /// Joint model view matrices of a traversal from the humanoid: parent chain times own matrix.
    fn joint_matrices(
        &self,
        node: NodeId,
        mv: Matrix4,
        out: &mut HashMap<NodeId, Matrix4>,
        depth: usize,
    ) {
        if depth > 1000 {
            return;
        }

        let ty = self.type_name(node);
        let mut mv = mv;

        match ty.as_str() {
            "Transform" | "HAnimJoint" => {
                mv.mult_left(&self.matrix_of(node));

                if ty == "HAnimJoint" {
                    out.insert(node, mv);
                }
            }
            "Group" | "HAnimSegment" => {}
            "Switch" => {
                if let FieldValue::SFInt32(i) = self.get(node, "whichChoice") {
                    if i >= 0 {
                        if let Some(c) = self.children_of(node, "children").get(i as usize) {
                            self.joint_matrices(*c, mv, out, depth + 1);
                        }
                    }
                }

                return;
            }
            _ => return,
        }

        for c in self.children_of(node, "children") {
            self.joint_matrices(c, mv, out, depth + 1);
        }
    }

    fn skin_bbox(&mut self, humanoid: NodeId) -> R<()> {
        let skin_coord = self.get_node(humanoid, "skinCoord");

        let Some(skin_coord) = skin_coord else {
            return Ok(());
        };

        // humanoidNode .traverse (DISPLAY, layerNode)
        let mut matrices: HashMap<NodeId, Matrix4> = HashMap::new();
        let mut mv = Matrix4::IDENTITY;

        mv.mult_left(&self.matrix_of(humanoid));

        for field in ["skeleton"] {
            for c in self.children_of(humanoid, field) {
                self.joint_matrices(c, mv, &mut matrices, 0);
            }
        }

        let positions = match self.get(humanoid, "jointBindingPositions") {
            FieldValue::MFVec3f(v) => v,
            _ => vec![],
        };
        let rotations = match self.get(humanoid, "jointBindingRotations") {
            FieldValue::MFRotation(v) => v,
            _ => vec![],
        };
        let scales = match self.get(humanoid, "jointBindingScales") {
            FieldValue::MFVec3f(v) => v,
            _ => vec![],
        };

        let points: Vec<Vector3> = self
            .points_of(Some(skin_coord))
            .chunks_exact(3)
            .map(|p| Vector3::new(p[0] as f64, p[1] as f64, p[2] as f64))
            .collect();
        let mut skin_points: std::collections::BTreeMap<usize, Vector3> =
            std::collections::BTreeMap::new();

        for (j, joint) in self.children_of(humanoid, "joints").into_iter().enumerate() {
            let weights = match self.get(joint, "skinCoordWeight") {
                FieldValue::MFFloat(v) => v,
                _ => vec![],
            };
            let indices = self.ints_of(joint, "skinCoordIndex");
            let t = positions
                .get(j * 3..j * 3 + 3)
                .map(|p| Vector3::new(p[0] as f64, p[1] as f64, p[2] as f64));
            let r = rotations
                .get(j * 4..j * 4 + 4)
                .map(|p| Rotation4::new(p[0], p[1], p[2], p[3]));
            let s = scales
                .get(j * 3..j * 3 + 3)
                .map(|p| Vector3::new(p[0] as f64, p[1] as f64, p[2] as f64));

            let mut joint_matrix = Matrix4::IDENTITY;

            joint_matrix.set_transform(t.as_ref(), r.as_ref(), s.as_ref(), None, None);
            joint_matrix.mult_right(matrices.get(&joint).unwrap_or(&Matrix4::IDENTITY));

            for (c, index) in indices.iter().enumerate() {
                let Some(point) = (*index >= 0).then(|| points.get(*index as usize)).flatten()
                else {
                    return Err(
                        "TypeError: Cannot read properties of undefined (reading 'copy')".into(),
                    );
                };

                let entry = skin_points.entry(*index as usize).or_insert(*point);
                let mut moved = *point;

                joint_matrix.mult_vec_matrix3(&mut moved);
                moved.subtract(point);
                moved.multiply(weights.get(c).copied().unwrap_or(f32::NAN) as f64);
                entry.add(&moved);
            }
        }

        let bbox = if skin_points.is_empty() {
            Box3::empty()
        } else {
            let mut min = Vector3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
            let mut max = Vector3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);

            for p in skin_points.values() {
                min.min(p);
                max.max(p);
            }

            Box3::from_extents(&min, &max)
        };

        let (size, center) = (bbox.size(), bbox.center());

        for shape in self.children_of(humanoid, "skin") {
            if self.has_field(shape, "bboxSize") {
                self.set(shape, "bboxSize", FieldValue::SFVec3f(size));
                self.set(shape, "bboxCenter", FieldValue::SFVec3f(center));
            }
        }

        Ok(())
    }
}
