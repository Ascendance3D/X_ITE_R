//! Meshes, primitives, geometry nodes, vertex attributes, skinning and morph targets of
//! GLTF2Parser.js.

use super::util::*;
use super::*;

fn undefined_read(key: &str) -> String {
    format!("TypeError: Cannot read properties of undefined (reading '{key}')")
}

/// What `createGeometry` and friends destructure from a primitive.
#[derive(Clone)]
pub(super) struct PrimData<'j> {
    pub attributes: Vec<(String, Option<usize>)>,
    pub tex_coord: Vec<Option<usize>>,
    pub color: Vec<Option<usize>>,
    pub joints: Vec<Option<usize>>,
    pub weights: Vec<Option<usize>>,
    pub indices: Option<usize>,
    pub material: Option<usize>,
    pub targets: V<'j>,
    pub mode: V<'j>,
}

impl PrimData<'_> {
    fn attr(&self, name: &str) -> Option<usize> {
        self.attributes
            .iter()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| *v)
    }
}

impl<'j, 's> P<'j, 's> {
    pub(super) fn meshes_array(&mut self, meshes: V<'j>) {
        if meshes.is_array() {
            self.meshes = meshes;
        }
    }

    fn prim_data(&self, key: (usize, usize), primitive: V<'j>) -> PrimData<'j> {
        let p = self.prims.get(&key);

        PrimData {
            attributes: p.map(|p| p.attributes.clone()).unwrap_or_default(),
            tex_coord: p.map(|p| p.tex_coord.clone()).unwrap_or_default(),
            color: p.map(|p| p.color.clone()).unwrap_or_default(),
            joints: p.map(|p| p.joints.clone()).unwrap_or_default(),
            weights: p.map(|p| p.weights.clone()).unwrap_or_default(),
            indices: p.and_then(|p| p.indices),
            material: p.and_then(|p| p.material),
            targets: primitive.get("targets"),
            mode: primitive.get("mode"),
        }
    }

    /// `meshObject (mesh, skin, EXT_mesh_gpu_instancing)`; `None` is `undefined`.
    pub(super) fn mesh_object(
        &mut self,
        mesh_index: V<'j>,
        skin: Option<usize>,
        instancing: V<'j>,
    ) -> R<Option<Vec<NodeId>>> {
        let Some(list) = self.meshes.arr() else {
            return Ok(None);
        };
        let Some(mi) = mesh_index.index(list.len()) else {
            return Ok(None);
        };
        let mesh = self.meshes.at(mi);

        if !mesh.is_obj() {
            return Ok(None);
        }

        if let Some(shapes) = self.mesh_shapes.get(&mi) {
            if !instancing.truthy() {
                let shapes = shapes.clone();
                let primitives = mesh.get("primitives");

                let Some(prims) = primitives.arr() else {
                    return Ok(Some(shapes));
                };

                for (pi, _) in prims.iter().enumerate() {
                    let p = self.prims.get(&(mi, pi));
                    let (joints, weights) = (
                        p.map(|p| p.joints.clone()).unwrap_or_default(),
                        p.map(|p| p.weights.clone()).unwrap_or_default(),
                    );

                    self.attributes_joints_array(skin, &joints, &weights)?;
                }

                return Ok(Some(shapes));
            }
        }

        // Actually shapeNodes are not always Shape nodes, it also can be Switch nodes from variants
        // or a GaussianSplats node.
        let shapes = self.primitives_array(mi, mesh, skin, instancing)?;

        // Name Shape nodes.
        let name = self.sanitize(mesh.get("name"));

        if !name.is_empty() {
            for (i, shape) in shapes.iter().enumerate() {
                self.named(&name, *shape)?;

                // shapeNode .getGeometry ?.()
                let ty = self.type_name(*shape);

                if matches!(ty.as_str(), "Shape" | "InstancedShape") {
                    if let Some(g) = self.get_node(*shape, "geometry") {
                        if self.node_name(g).is_empty() {
                            self.named(&format!("{name}-Mesh-{i}"), g)?;
                        }
                    }
                }
            }
        }

        self.mesh_shapes.insert(mi, shapes.clone());
        Ok(Some(shapes))
    }

    fn primitives_array(
        &mut self,
        mi: usize,
        mesh: V<'j>,
        skin: Option<usize>,
        instancing: V<'j>,
    ) -> R<Vec<NodeId>> {
        let Some(prims) = mesh.get("primitives").arr() else {
            return Ok(vec![]);
        };
        let weights = mesh.get("weights");
        let mut shapes = vec![];

        for (pi, primitive) in prims.iter().enumerate() {
            self.primitive_object(
                (mi, pi),
                V::new(primitive),
                weights,
                skin,
                instancing,
                &mut shapes,
            )?;
        }

        Ok(shapes)
    }

    fn primitive_object(
        &mut self,
        key: (usize, usize),
        primitive: V<'j>,
        weights: V<'j>,
        skin: Option<usize>,
        instancing: V<'j>,
        shapes: &mut Vec<NodeId>,
    ) -> R<()> {
        if !primitive.is_obj() {
            return Ok(());
        }

        self.attributes_object(key, primitive.get("attributes"));

        if primitive
            .get("extensions")
            .get("KHR_gaussian_splatting")
            .truthy()
        {
            return self.khr_gaussian_splatting(key, primitive, shapes);
        }

        // primitive .indices = this .accessors [primitive .indices]
        let resolved = self.prims.get(&key).map(|p| p.resolved).unwrap_or(false);
        let (indices, material) = if resolved {
            // The primitive was converted before: its properties are objects now, and an object is
            // not an array index.
            (None, None)
        } else {
            let indices = self.accessor(primitive.get("indices"));
            let material = self
                .materials
                .arr()
                .and_then(|a| primitive.get("material").index(a.len()));

            (indices, material)
        };

        {
            let p = self.prims.entry(key).or_default();

            p.indices = indices;
            p.material = material;
            p.resolved = true;
        }

        // primitiveExtensionsObject: KHR_draco_mesh_compression needs the decoder (missing).

        let data = self.prim_data(key, primitive);
        let shape = self.create_shape(key, &data, weights, skin, instancing)?;
        let variants = self.khr_materials_variants_extension(
            primitive.get("extensions").get("KHR_materials_variants"),
            shape,
        )?;
        let result = variants.unwrap_or(shape);

        self.prims.entry(key).or_default().shape = Some(result);
        shapes.push(result);
        Ok(())
    }

    fn khr_gaussian_splatting(
        &mut self,
        key: (usize, usize),
        primitive: V<'j>,
        shapes: &mut Vec<NodeId>,
    ) -> R<()> {
        let node = self.create("GaussianSplats")?;
        let ext = primitive.get("extensions").get("KHR_gaussian_splatting");
        let data = self.prim_data(key, primitive);

        let cs = if ext.get("colorSpace").nn() {
            ext.get("colorSpace").to_js_string()
        } else {
            "SRGB_REC709_DISPLAY".to_string()
        };

        self.set(node, "colorSpace", FieldValue::SFString(cs.to_uppercase()));

        let mut take = |me: &mut Self, name: &str, attr: Option<usize>| -> R<Vec<f64>> {
            let _ = name;

            match attr {
                Some(a) => Ok(me.accessor_array(a)?.v.clone()),
                None => Ok(vec![]),
            }
        };

        let position = data.attr("POSITION");
        let v = take(self, "positions", position)?;
        self.set_nums(node, "positions", &v);
        let v = take(
            self,
            "orientations",
            data.attr("KHR_gaussian_splatting:ROTATION"),
        )?;
        self.set_nums(node, "orientations", &v);
        let v = take(self, "scales", data.attr("KHR_gaussian_splatting:SCALE"))?;
        self.set_nums(node, "scales", &v);
        let v = take(
            self,
            "opacities",
            data.attr("KHR_gaussian_splatting:OPACITY"),
        )?;
        self.set_nums(node, "opacities", &v);

        // Degrees 0,1,2,3
        for (degree, coefs) in [1usize, 3, 5, 7].iter().enumerate() {
            for coef in 0..*coefs {
                let attr = data.attr(&format!(
                    "KHR_gaussian_splatting:SH_DEGREE_{degree}_COEF_{coef}"
                ));
                let v = take(self, "sh", attr)?;

                self.set_nums(
                    node,
                    &format!("sphericalHarmonicsDegree{degree}Coef{coef}"),
                    &v,
                );
            }
        }

        // Bounding Box
        if let Some(p) = position {
            let acc = self.acc_json(p);
            let min = vector_value(acc.get("min"), 3);
            let max = vector_value(acc.get("max"), 3);

            if let (Some(min), Some(max)) = (min, max) {
                self.set(
                    node,
                    "bboxSize",
                    FieldValue::SFVec3f(Vector3::new(
                        max[0] - min[0],
                        max[1] - min[1],
                        max[2] - min[2],
                    )),
                );
                self.set(
                    node,
                    "bboxCenter",
                    FieldValue::SFVec3f(Vector3::new(
                        (min[0] + max[0]) / 2.0,
                        (min[1] + max[1]) / 2.0,
                        (min[2] + max[2]) / 2.0,
                    )),
                );
            }
        }

        self.setup(node);
        self.prims.entry(key).or_default().shape = Some(node);
        shapes.push(node);
        Ok(())
    }

    fn attributes_object(&mut self, key: (usize, usize), attributes: V<'j>) {
        if !attributes.is_obj() {
            return;
        }

        let resolved = self.prims.get(&key).map(|p| p.resolved).unwrap_or(false);
        let mut p = Prim::default();

        // `attributes [key] = this .accessors [attributes [key]]`; on a second pass the values
        // are accessor objects, which index nothing.
        let mut named: Vec<(String, Option<usize>)> = vec![];

        if resolved {
            named = self.prims[&key]
                .attributes
                .iter()
                .map(|(k, _)| (k.clone(), None))
                .collect();

            for k in ["TEXCOORD", "COLOR", "JOINTS", "WEIGHTS"] {
                if !named.iter().any(|(n, _)| n == k) {
                    named.push((k.to_string(), None));
                }
            }
        } else {
            for (k, v) in attributes.entries() {
                named.push((k, self.accessor(v)));
            }
        }

        p.attributes = named;

        let list = |name: &str| -> Vec<Option<usize>> {
            let mut out = vec![];

            for i in 0.. {
                match p
                    .attributes
                    .iter()
                    .find(|(k, _)| *k == format!("{name}_{i}"))
                    .and_then(|(_, v)| *v)
                {
                    Some(a) => out.push(Some(a)),
                    None => break,
                }
            }

            out
        };

        let (tex_coord, color, joints, weights) = (
            list("TEXCOORD"),
            list("COLOR"),
            list("JOINTS"),
            list("WEIGHTS"),
        );

        let existing = self.prims.remove(&key);

        p.tex_coord = tex_coord;
        p.color = color;
        p.joints = joints;
        p.weights = weights;

        if let Some(e) = existing {
            p.tex_coord_node = e.tex_coord_node;
            p.indices = e.indices;
            p.material = e.material;
            p.appearance = e.appearance;
            p.geometry = e.geometry;
            p.shape = e.shape;
            p.resolved = e.resolved;
        }

        self.prims.insert(key, p);
    }

    // --- shapes -------------------------------------------------------------------------------------

    fn create_shape(
        &mut self,
        key: (usize, usize),
        data: &PrimData<'j>,
        weights: V<'j>,
        skin: Option<usize>,
        instancing: V<'j>,
    ) -> R<NodeId> {
        let shape = match self.mesh_instancing(instancing)? {
            Some(s) => s,
            None => self.create("Shape")?,
        };

        let appearance = match self.prims.get(&key).and_then(|p| p.appearance) {
            Some(a) => a,
            None => {
                let a = self.material_object(data.material)?;

                self.prims.entry(key).or_default().appearance = Some(a);
                a
            }
        };

        let geometry = match self.prims.get(&key).and_then(|p| p.geometry) {
            Some(g) => Some(g),
            None => {
                let g = self.create_geometry(key, data, weights, skin)?;

                self.prims.entry(key).or_default().geometry = g;
                g
            }
        };

        self.set_node(shape, "appearance", Some(appearance));
        self.set_node(shape, "geometry", geometry);
        self.setup(shape);
        Ok(shape)
    }

    fn mesh_instancing(&mut self, ext: V<'j>) -> R<Option<NodeId>> {
        if !ext.is_obj() {
            return Ok(None);
        }

        let attributes = ext.get("attributes");
        let translation = self.accessor(attributes.get("TRANSLATION"));
        let rotation = self.accessor(attributes.get("ROTATION"));
        let scale = self.accessor(attributes.get("SCALE"));
        let count_of = |me: &Self, a: Option<usize>| {
            a.map(|a| me.acc_json(a).get("count"))
                .filter(|c| c.nn())
                .map(|c| c.to_number())
                .unwrap_or(0.0)
        };
        let count = x3d_fields::jsmath::js_max(
            x3d_fields::jsmath::js_max(count_of(self, translation), count_of(self, rotation)),
            count_of(self, scale),
        );

        if count == 0.0 || count.is_nan() {
            return Ok(None);
        }

        let kind = |me: &Self, a: Option<usize>, t: &str| {
            a.filter(|a| me.acc_json(*a).get("type").str() == Some(t))
        };
        let translation = kind(self, translation, "VEC3");
        let rotation = kind(self, rotation, "VEC4");
        let scale = kind(self, scale, "VEC3");

        let node = self.create("InstancedShape")?;

        if let Some(a) = translation {
            let arr = self.accessor_array(a)?;

            self.set_nums(node, "translations", &arr.v);
        }

        if let Some(a) = rotation {
            let arr = self.accessor_array(a)?;
            let length = (self.acc_json(a).get("count").to_number() * 4.0) as usize;
            let mut out = vec![];
            let at = |i: usize| arr.v.get(i).copied().unwrap_or(f64::NAN);

            let mut i = 0;

            while i < length {
                let mut r = Rotation4::identity();

                r.set_quaternion(&Quaternion::new(at(i), at(i + 1), at(i + 2), at(i + 3)));
                out.extend(r.components());
                i += 4;
            }

            self.set_nums(node, "rotations", &out);
        }

        if let Some(a) = scale {
            let arr = self.accessor_array(a)?;

            self.set_nums(node, "scales", &arr.v);
        }

        self.setup(node);
        Ok(Some(node))
    }

    fn khr_materials_variants_extension(&mut self, ext: V<'j>, shape: NodeId) -> R<Option<NodeId>> {
        if !ext.is_obj() {
            return Ok(None);
        }

        let mappings = ext.get("mappings");
        let Some(list) = mappings.arr() else {
            return Ok(None);
        };

        if list.is_empty() {
            return Ok(None);
        }

        let variants = self.create("Switch")?;
        let mut children: Vec<Option<NodeId>> = vec![];

        for mapping in list {
            self.khr_materials_variants_mapping(V::new(mapping), shape, &mut children)?;
        }

        let n_variants = self.material_variants.arr().map(|a| a.len()).unwrap_or(0);

        self.set(
            variants,
            "whichChoice",
            FieldValue::SFInt32(n_variants as i32),
        );

        if children.is_empty() {
            return Ok(None);
        }

        // Last child ist default material.
        if children.len() <= n_variants {
            children.resize(n_variants + 1, None);
        }

        children[n_variants] = Some(shape);

        // Fall back to default material if no active variant.
        for c in children.iter_mut() {
            if c.is_none() {
                *c = Some(shape);
            }
        }

        self.set(variants, "children", FieldValue::MFNode(children));
        self.setup(variants);
        self.material_variant_nodes.push(variants);
        Ok(Some(variants))
    }

    fn khr_materials_variants_mapping(
        &mut self,
        mapping: V<'j>,
        shape: NodeId,
        children: &mut Vec<Option<NodeId>>,
    ) -> R<()> {
        if !mapping.is_obj() {
            return Ok(());
        }

        let material = self
            .materials
            .arr()
            .and_then(|a| mapping.get("material").index(a.len()));

        let Some(mi) = material else { return Ok(()) };

        if !self.materials.at(mi).truthy() {
            return Ok(());
        }

        let variant_node = self.create("Shape")?;
        let appearance = self.material_object(Some(mi))?;
        let first = mapping.get("variants").at(0);
        let variant = if first.nn() {
            first
        } else {
            V::new(&J::Number(0.0))
        };
        let vname = {
            let entry = self
                .material_variants
                .arr()
                .and_then(|a| variant.index(a.len()).map(|i| V::new(&a[i])));
            let n = entry.map(|e| e.get("name")).filter(|n| n.nn());

            match n {
                Some(n) => self.sanitize(n),
                None => String::new(),
            }
        };

        if !vname.is_empty() {
            self.named(&vname, variant_node)?;
        }

        self.set_node(variant_node, "appearance", Some(appearance));

        let geometry = self.get_node(shape, "geometry");

        self.set_node(variant_node, "geometry", geometry);
        self.setup(variant_node);

        // variantsNode ._children [variant] = variantNode
        let index = variant.to_number();

        if index >= 0.0 && index.fract() == 0.0 && index < 100_000.0 {
            let index = index as usize;

            if children.len() <= index {
                children.resize(index + 1, None);
            }

            children[index] = Some(variant_node);
        }

        Ok(())
    }

    // --- geometry -------------------------------------------------------------------------------------

    fn create_geometry(
        &mut self,
        key: (usize, usize),
        data: &PrimData<'j>,
        weights: V<'j>,
        skin: Option<usize>,
    ) -> R<Option<NodeId>> {
        let mode = data.mode.num();
        let indexed = data.indices.is_some();

        let node = match mode {
            Some(m) if m == 0.0 => self.create_point_set(key, data, weights, skin)?,
            Some(m) if m == 1.0 => {
                if indexed {
                    self.create_indexed_line_set(key, data, weights, skin, 1)?
                } else {
                    self.create_line_set(key, data, weights, skin)?
                }
            }
            Some(m) if m == 2.0 => self.create_indexed_line_set(key, data, weights, skin, 2)?,
            Some(m) if m == 3.0 => self.create_indexed_line_set(key, data, weights, skin, 3)?,
            Some(m) if m == 5.0 => {
                if indexed {
                    self.create_triangles(key, data, weights, skin, "IndexedTriangleStripSet")?
                } else {
                    self.create_triangles(key, data, weights, skin, "TriangleStripSet")?
                }
            }
            Some(m) if m == 6.0 => {
                if indexed {
                    self.create_triangles(key, data, weights, skin, "IndexedTriangleFanSet")?
                } else {
                    self.create_triangles(key, data, weights, skin, "TriangleFanSet")?
                }
            }
            _ => {
                if indexed {
                    self.create_triangles(key, data, weights, skin, "IndexedTriangleSet")?
                } else {
                    self.create_triangles(key, data, weights, skin, "TriangleSet")?
                }
            }
        };

        Ok(Some(node))
    }

    fn material_json(&self, material: Option<usize>) -> V<'j> {
        match material {
            Some(m) => self.materials.at(m),
            None => UNDEF,
        }
    }

    fn create_point_set(
        &mut self,
        key: (usize, usize),
        data: &PrimData<'j>,
        weights: V<'j>,
        skin: Option<usize>,
    ) -> R<NodeId> {
        let node = self.create("PointSet")?;

        self.common_attributes(node, key, data, weights, false)?;
        self.attributes_joints_array(skin, &data.joints, &data.weights)?;
        self.skin_geometry(skin, node)?;
        let _ = key;
        self.setup(node);
        Ok(node)
    }

    fn create_line_set(
        &mut self,
        key: (usize, usize),
        data: &PrimData<'j>,
        weights: V<'j>,
        skin: Option<usize>,
    ) -> R<NodeId> {
        let node = self.create("LineSet")?;

        self.common_attributes(node, key, data, weights, false)?;
        self.attributes_joints_array(skin, &data.joints, &data.weights)?;
        self.skin_geometry(skin, node)?;
        let _ = key;
        self.setup(node);
        Ok(node)
    }

    /// color, normal, tangent and coord in the order the JS code assigns them.
    fn common_attributes(
        &mut self,
        node: NodeId,
        key: (usize, usize),
        data: &PrimData<'j>,
        weights: V<'j>,
        triangles: bool,
    ) -> R<()> {
        let material = data.material;
        let color = data.color.first().copied().flatten();
        let c = self.create_color(color, material)?;

        self.set_node(node, "color", c);

        if triangles {
            let tc = self.create_multi_texture_coordinate(key, &data.tex_coord, material)?;

            self.set_node(node, "texCoord", tc);
        }

        let normal = self.create_normal(data.attr("NORMAL"), data.targets, weights)?;

        self.set_node(node, "normal", normal);

        let tangent = self.create_tangent(data.attr("TANGENT"), data.attr("NORMAL"))?;

        self.set_node(node, "tangent", tangent);

        let coord = self.create_coordinate(data.attr("POSITION"), data.targets, weights)?;

        self.set_node(node, "coord", coord);
        Ok(())
    }

    fn create_indexed_line_set(
        &mut self,
        key: (usize, usize),
        data: &PrimData<'j>,
        weights: V<'j>,
        skin: Option<usize>,
        mode: u8,
    ) -> R<NodeId> {
        let node = self.create("IndexedLineSet")?;

        self.common_attributes(node, key, data, weights, false)?;

        let indices = match data.indices {
            Some(a) => Some(self.accessor_array(a)?),
            None => None,
        };

        let mut coord_index: Vec<f64> = vec![];

        match mode {
            1 => {
                // LINES: `indices .array` is read with `indices` present.
                let array = indices.map(|a| a.v.clone()).unwrap_or_default();
                let mut i = 0;

                while i < array.len() {
                    coord_index.push(array[i]);
                    coord_index.push(array.get(i + 1).copied().unwrap_or(0.0));
                    coord_index.push(-1.0);
                    i += 2;
                }
            }
            2 | 3 => {
                if let Some(a) = indices {
                    coord_index.extend(a.v.iter().copied());

                    if !coord_index.is_empty() {
                        if mode == 2 {
                            coord_index.push(coord_index[0]);
                        }

                        coord_index.push(-1.0);
                    }
                } else if let Some(coord) = self.get_node(node, "coord") {
                    let n = self.get(coord, "point").len();

                    if n > 0 {
                        coord_index.extend((0..n).map(|i| i as f64));

                        if mode == 2 {
                            coord_index.push(0.0);
                        }

                        coord_index.push(-1.0);
                    }
                }
            }
            _ => {}
        }

        self.set_nums(node, "coordIndex", &coord_index);
        self.attributes_joints_array(skin, &data.joints, &data.weights)?;
        self.skin_geometry(skin, node)?;
        let _ = key;
        self.setup(node);
        Ok(node)
    }

    fn create_triangles(
        &mut self,
        key: (usize, usize),
        data: &PrimData<'j>,
        weights: V<'j>,
        skin: Option<usize>,
        type_name: &str,
    ) -> R<NodeId> {
        let node = self.create(type_name)?;
        let material = self.material_json(data.material);

        self.set(
            node,
            "solid",
            FieldValue::SFBool(!material.get("doubleSided").truthy()),
        );

        if type_name.starts_with("Indexed") {
            let indices = match data.indices {
                Some(a) => self.accessor_array(a)?,
                None => return Err(undefined_read("array")),
            };

            self.set_nums(node, "index", &indices.v);
        }

        self.common_attributes(node, key, data, weights, true)?;

        let has_normal = self.get_node(node, "normal").is_some();

        self.set(node, "normalPerVertex", FieldValue::SFBool(has_normal));

        match type_name {
            "TriangleStripSet" | "TriangleFanSet" => {
                if let Some(coord) = self.get_node(node, "coord") {
                    let n = self.get(coord, "point").len();

                    if n > 0 {
                        self.set_nums(
                            node,
                            if type_name == "TriangleStripSet" {
                                "stripCount"
                            } else {
                                "fanCount"
                            },
                            &[n as f64],
                        );
                    }
                }
            }
            _ => {}
        }

        self.attributes_joints_array(skin, &data.joints, &data.weights)?;
        self.skin_geometry(skin, node)?;
        let _ = key;
        self.setup(node);
        Ok(node)
    }

    // --- attribute nodes ---------------------------------------------------------------------------------

    fn create_color(&mut self, color: Option<usize>, material: Option<usize>) -> R<Option<NodeId>> {
        let Some(c) = color else { return Ok(None) };
        let acc = self.acc_json(c);

        let type_name = match acc.get("type").str() {
            Some("VEC3") => "Color",
            Some("VEC4") => "ColorRGBA",
            _ => return Ok(None),
        };

        if let Some(n) = self.acc[c].color_node {
            return Ok(Some(n));
        }

        let appearance = self.material_object(material)?;
        let opaque =
            matches!(self.get(appearance, "alphaMode"), FieldValue::SFString(s) if s == "OPAQUE");
        let node = self.create(if opaque { "Color" } else { type_name })?;
        let array = self.accessor_array(c)?;

        let values: Vec<f64> = if opaque && type_name != "Color" {
            array
                .v
                .iter()
                .enumerate()
                .filter(|(i, _)| (i + 1) % 4 != 0)
                .map(|(_, v)| *v)
                .collect()
        } else {
            array.v.clone()
        };

        self.set_nums(node, "color", &values);
        self.acc[c].color_node = Some(node);
        self.setup(node);
        Ok(Some(node))
    }

    fn create_multi_texture_coordinate(
        &mut self,
        key: (usize, usize),
        list: &[Option<usize>],
        material: Option<usize>,
    ) -> R<Option<NodeId>> {
        let appearance = self.material_object(material)?;
        let material_node = self.get_node(appearance, "material");

        if !self.has_textures(material_node) {
            return Ok(None);
        }

        if let Some(n) = self.prims.get(&key).and_then(|p| p.tex_coord_node) {
            return Ok(Some(n));
        }

        let mappings: Vec<(String, f64)> = match material {
            Some(m) => self.mats[m].tex_coord_mappings.clone(),
            None => return Err(undefined_read("texCoordMappings")),
        };

        let count = if list.len() == 1 { 1 } else { mappings.len() };

        let tex = |_me: &mut Self, i: f64| -> Option<usize> {
            if i >= 0.0 && i.fract() == 0.0 {
                list.get(i as usize).copied().flatten()
            } else {
                None
            }
        };

        match count {
            0 => Ok(None),
            1 => {
                let mut first: Option<Option<NodeId>> = None;

                for (mapping, i) in &mappings {
                    let t = tex(self, *i);
                    let n = self.create_texture_coordinate(t, mapping)?;

                    if first.is_none() {
                        first = Some(n);
                    }
                }

                let result = first.flatten();

                self.prims.entry(key).or_default().tex_coord_node = result;
                Ok(result)
            }
            _ => {
                let mut sorted = mappings.clone();

                sorted.sort_by(|a, b| a.0.cmp(&b.0));
                sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

                let mut nodes = vec![];

                for (mapping, i) in &sorted {
                    let t = tex(self, *i);

                    nodes.push(self.create_texture_coordinate(t, mapping)?);
                }

                if nodes.is_empty() {
                    return Ok(None);
                }

                let node = self.create("MultiTextureCoordinate")?;

                self.set(node, "texCoord", FieldValue::MFNode(nodes));
                self.prims.entry(key).or_default().tex_coord_node = Some(node);
                self.setup(node);
                Ok(Some(node))
            }
        }
    }

    fn create_texture_coordinate(
        &mut self,
        tex_coord: Option<usize>,
        mapping: &str,
    ) -> R<Option<NodeId>> {
        let Some(t) = tex_coord else { return Ok(None) };

        if self.acc_json(t).get("type").str() != Some("VEC2") {
            return Ok(None);
        }

        if let Some(n) = self.acc[t].tex_coord_nodes.get(mapping) {
            return Ok(*n);
        }

        let node = self.create("TextureCoordinate")?;
        let array = self.accessor_array(t)?;

        self.set(node, "mapping", FieldValue::SFString(mapping.into()));
        self.set_nums(node, "point", &array.v);
        self.acc[t]
            .tex_coord_nodes
            .insert(mapping.to_string(), Some(node));
        self.setup(node);
        Ok(Some(node))
    }

    fn create_normal(
        &mut self,
        normal: Option<usize>,
        targets: V<'j>,
        weights: V<'j>,
    ) -> R<Option<NodeId>> {
        let Some(n) = normal else { return Ok(None) };

        if self.acc_json(n).get("type").str() != Some("VEC3") {
            return Ok(None);
        }

        if let Some(node) = self.acc[n].normal_node {
            return Ok(Some(node));
        }

        let node = self.create("Normal")?;
        let array = self.accessor_array(n)?;

        self.set_nums(node, "vector", &array.v);

        let base = match self.get(node, "vector") {
            FieldValue::MFVec3f(v) => v,
            _ => vec![],
        };

        if targets.is_array() && weights.is_array() {
            let w = json_weights(weights);
            let vectors = self.apply_morph_targets(&base, targets, "NORMAL", &w)?;

            self.set_nums(node, "vector", &vectors);
        }

        self.acc[n].field = Some(base);
        self.acc[n].normal_node = Some(node);
        self.setup(node);
        Ok(Some(node))
    }

    fn create_tangent(
        &mut self,
        tangent: Option<usize>,
        normal: Option<usize>,
    ) -> R<Option<NodeId>> {
        // When the base mesh primitive does not specify normals, client implementations MUST
        // calculate flat normals for each morph target; the provided tangents and their
        // displacements (if present) MUST be ignored.
        let Some(nrm) = normal else { return Ok(None) };

        if self.acc_json(nrm).get("type").str() != Some("VEC3") {
            return Ok(None);
        }

        let Some(t) = tangent else { return Ok(None) };

        if self.acc_json(t).get("type").str() != Some("VEC4") {
            return Ok(None);
        }

        if let Some(n) = self.acc[t].tangent_node {
            return Ok(Some(n));
        }

        let node = self.create("Tangent")?;
        let array = self.accessor_array(t)?;

        self.set_nums(node, "vector", &array.v);
        self.acc[t].tangent_node = Some(node);
        self.setup(node);
        Ok(Some(node))
    }

    fn create_coordinate(
        &mut self,
        position: Option<usize>,
        targets: V<'j>,
        weights: V<'j>,
    ) -> R<Option<NodeId>> {
        let Some(p) = position else { return Ok(None) };

        if self.acc_json(p).get("type").str() != Some("VEC3") {
            return Ok(None);
        }

        if let Some(node) = self.acc[p].coordinate_node {
            return Ok(Some(node));
        }

        let node = self.create("Coordinate")?;
        let array = self.accessor_array(p)?;

        self.set_nums(node, "point", &array.v);

        let base = match self.get(node, "point") {
            FieldValue::MFVec3f(v) => v,
            _ => vec![],
        };

        if targets.is_array() && weights.is_array() {
            let w = json_weights(weights);
            let points = self.apply_morph_targets(&base, targets, "POSITION", &w)?;

            self.set_nums(node, "point", &points);
        }

        self.acc[p].field = Some(base);
        self.acc[p].coordinate_node = Some(node);
        self.setup(node);
        Ok(Some(node))
    }

    /// `applyMorphTargets (array, targets, key, weights)`: the base vectors plus each target's
    /// displacement times its weight, flat.
    pub(super) fn apply_morph_targets(
        &mut self,
        base: &[f32],
        targets: V<'j>,
        key: &str,
        weights: &[f64],
    ) -> R<Vec<f64>> {
        let mut vectors: Vec<[f64; 3]> = base
            .chunks_exact(3)
            .map(|c| [c[0] as f64, c[1] as f64, c[2] as f64])
            .collect();
        let Some(list) = targets.arr() else {
            return Ok(vectors.concat());
        };

        for (i, target) in list.iter().enumerate() {
            let weight = weights.get(i).copied().unwrap_or(f64::NAN);

            if weight == 0.0 || weight.is_nan() {
                continue;
            }

            let target = V::new(target);
            let Some(acc) = self.accessor(target.get(key)) else {
                continue;
            };

            if self.acc_json(acc).get("type").str() != Some("VEC3") {
                continue;
            }

            let array = self.accessor_array(acc)?;
            let mut a = 0;
            let mut p = 0;

            while a < array.v.len() {
                let Some(v) = vectors.get_mut(p) else {
                    return Err(undefined_read("add"));
                };
                let get = |k: usize| array.v.get(k).copied().unwrap_or(f64::NAN);

                v[0] += get(a) * weight;
                v[1] += get(a + 1) * weight;
                v[2] += get(a + 2) * weight;
                a += 3;
                p += 1;
            }
        }

        Ok(vectors.concat())
    }

    // --- skinning ------------------------------------------------------------------------------------------

    pub(super) fn attributes_joints_array(
        &mut self,
        skin: Option<usize>,
        joints: &[Option<usize>],
        weights: &[Option<usize>],
    ) -> R<()> {
        let Some(skin) = skin else { return Ok(()) };

        for (i, j) in joints.iter().enumerate() {
            self.attributes_joints_object(skin, *j, weights.get(i).copied().flatten())?;
        }

        Ok(())
    }

    fn attributes_joints_object(
        &mut self,
        skin: usize,
        joints: Option<usize>,
        weights: Option<usize>,
    ) -> R<()> {
        let (Some(j), Some(w)) = (joints, weights) else {
            return Ok(());
        };

        if self.acc_json(j).get("type").str() != Some("VEC4")
            || self.acc_json(w).get("type").str() != Some("VEC4")
        {
            return Ok(());
        }

        let coordinate = self.skins[skin].as_ref().unwrap().coordinate;
        let start = self.get(coordinate, "point").len();
        let joints_array = self.accessor_array(j)?;
        let weights_array = self.accessor_array(w)?;
        let num_vertices = joints_array.v.len() / 4;

        for v in 0..num_vertices {
            for i in 0..4 {
                let wv = weights_array.v.get(v * 4 + i).copied();

                if wv == Some(0.0) {
                    continue;
                }

                let joint_index = joints_array.v[v * 4 + i];
                let index = {
                    let skin_obj = self.skins[skin].as_ref().unwrap();

                    if joint_index >= 0.0 && joint_index.fract() == 0.0 {
                        skin_obj.joints.get(joint_index as usize).copied()
                    } else {
                        None
                    }
                };

                let node = index
                    .and_then(|ix| ix.index(self.nodes.len()))
                    .and_then(|n| self.nodes[n].transform);

                let Some(node) = node else { continue };

                if self.type_name(node) != "HAnimJoint" {
                    continue;
                }

                push_value(self.scene, node, "skinCoordIndex", (v + start) as f64);
                push_value(self.scene, node, "skinCoordWeight", wv.unwrap_or(f64::NAN));
            }
        }

        Ok(())
    }

    fn skin_geometry(&mut self, skin: Option<usize>, geometry: NodeId) -> R<()> {
        let Some(si) = skin else { return Ok(()) };
        let s = self.skins[si].as_ref().unwrap();
        let (skin_coord, skin_normal, skin_tex, skin_multi) =
            (s.coordinate, s.normal, s.tex_coord, s.multi_tex_coord);
        let start = self.get(skin_coord, "point").len();

        let tex_node = if self.has_field(geometry, "texCoord") {
            self.get_node(geometry, "texCoord")
        } else {
            None
        };
        let normal_node = if self.has_field(geometry, "normal") {
            self.get_node(geometry, "normal")
        } else {
            None
        };
        let coord_node = if self.has_field(geometry, "coord") {
            self.get_node(geometry, "coord")
        } else {
            None
        };

        for field in ["coordIndex", "index"] {
            if self.has_field(geometry, field) {
                if let FieldValue::MFInt32(v) = self.get(geometry, field) {
                    let mapped: Vec<f64> = v
                        .iter()
                        .map(|i| {
                            if *i < 0 {
                                -1.0
                            } else {
                                (*i as f64) + start as f64
                            }
                        })
                        .collect();

                    self.set_nums(geometry, field, &mapped);
                }
            }
        }

        if let Some(t) = tex_node {
            match self.type_name(t).as_str() {
                "TextureCoordinate" => {
                    let points = match self.get(t, "point") {
                        FieldValue::MFVec2f(v) => v,
                        _ => vec![],
                    };

                    set_vec_at(
                        self.scene,
                        skin_tex,
                        "point",
                        start,
                        2,
                        &points.iter().map(|x| *x as f64).collect::<Vec<_>>(),
                    );
                    self.set_node(geometry, "texCoord", Some(skin_tex));
                }
                "MultiTextureCoordinate" => {
                    let children = match self.get(t, "texCoord") {
                        FieldValue::MFNode(v) => v,
                        _ => vec![],
                    };

                    for child in children.into_iter().flatten() {
                        let mapping = match self.get(child, "mapping") {
                            FieldValue::SFString(s) => s,
                            _ => String::new(),
                        };

                        let existing = match self.get(skin_multi, "texCoord") {
                            FieldValue::MFNode(v) => v.into_iter().flatten().find(|n| matches!(self.get(*n, "mapping"), FieldValue::SFString(ref s) if *s == mapping)),
                            _ => None,
                        };

                        let target = match existing {
                            Some(e) => e,
                            None => {
                                let s = if mapping == "TEXCOORD_0" {
                                    skin_tex
                                } else {
                                    let s = self.create("TextureCoordinate")?;

                                    self.set(s, "mapping", FieldValue::SFString(mapping.clone()));
                                    self.setup(s);
                                    s
                                };

                                self.push_node(skin_multi, "texCoord", Some(s));
                                s
                            }
                        };

                        let points = match self.get(child, "point") {
                            FieldValue::MFVec2f(v) => v,
                            _ => vec![],
                        };

                        set_vec_at(
                            self.scene,
                            target,
                            "point",
                            start,
                            2,
                            &points.iter().map(|x| *x as f64).collect::<Vec<_>>(),
                        );
                    }

                    self.set_node(geometry, "texCoord", Some(skin_multi));
                }
                _ => {}
            }
        }

        if let Some(n) = normal_node {
            let vectors = match self.get(n, "vector") {
                FieldValue::MFVec3f(v) => v,
                _ => vec![],
            };

            set_vec_at(
                self.scene,
                skin_normal,
                "vector",
                start,
                3,
                &vectors.iter().map(|x| *x as f64).collect::<Vec<_>>(),
            );
            self.set_node(geometry, "normal", Some(skin_normal));
        }

        if let Some(c) = coord_node {
            let points = match self.get(c, "point") {
                FieldValue::MFVec3f(v) => v,
                _ => vec![],
            };

            set_vec_at(
                self.scene,
                skin_coord,
                "point",
                start,
                3,
                &points.iter().map(|x| *x as f64).collect::<Vec<_>>(),
            );
            self.set_node(geometry, "coord", Some(skin_coord));
        }

        Ok(())
    }
}

/// `weights [i]` of a JSON array as numbers (non-numbers are NaN, which `!weight` skips).
fn json_weights(weights: V) -> Vec<f64> {
    weights
        .arr()
        .map(|a| {
            a.iter()
                .map(|v| match V::new(v).0 {
                    Some(J::Number(n)) => *n,
                    _ => f64::NAN,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `field .push (value)` on a typed MF field.
pub(super) fn push_value(scene: &mut Scene, node: NodeId, name: &str, value: f64) {
    let i = scene.get_predefined_field(node, name).unwrap();
    let f = &mut scene.node_mut(node).fields[i];

    f.value.push_numbers(&[value]);
    f.modified = true;
}

/// `field [index + k] = element` for a typed MF field of `components` per element: writes the
/// elements of `values` from element `start` on, growing the field with zeros.
pub(super) fn set_vec_at(
    scene: &mut Scene,
    node: NodeId,
    name: &str,
    start: usize,
    components: usize,
    values: &[f64],
) {
    let i = scene.get_predefined_field(node, name).unwrap();
    let f = &mut scene.node_mut(node).fields[i];
    let mut current: Vec<f64> = match &f.value {
        FieldValue::MFVec2f(v) | FieldValue::MFVec3f(v) | FieldValue::MFFloat(v) => {
            v.iter().map(|x| *x as f64).collect()
        }
        _ => vec![],
    };

    let need = start * components + values.len();

    if current.len() < need {
        current.resize(need, 0.0);
    }

    current[start * components..need].copy_from_slice(values);
    f.value.set_flat_numbers(&current);
    f.modified = true;
}
