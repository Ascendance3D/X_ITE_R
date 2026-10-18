//! Materials, texture infos, KHR_texture_transform and the KHR_materials_* extensions of
//! GLTF2Parser.js.

use super::util::*;
use super::*;
use x3d_fields::math::Vector2;

fn tc_name(i: f64) -> String {
    format!("TEXCOORD_{}", x3d_fields::jsmath::number_to_string(i))
}

fn col3(v: &[f64]) -> FieldValue {
    FieldValue::SFColor(Color3::new(v[0], v[1], v[2]))
}

/// `texCoord || 0`
fn tex_coord_or_zero(v: V) -> f64 {
    if v.truthy() { v.to_number() } else { 0.0 }
}

impl<'j, 's> P<'j, 's> {
    pub(super) fn materials_array(&mut self, materials: V<'j>) {
        if let Some(list) = materials.arr() {
            self.materials = materials;
            self.mats = (0..list.len()).map(|_| Mat::default()).collect();
        }
    }

    /// `set` that ignores a field the node does not have, like assigning a plain JS property.
    fn set_f(&mut self, node: NodeId, name: &str, value: FieldValue) {
        if self.has_field(node, name) {
            self.set(node, name, value);
        }
    }

    fn set_mapping(&mut self, node: NodeId, name: &str, mapping: Option<String>) {
        // `undefined` assigned to an SFString.
        self.set_f(node, name, FieldValue::SFString(mapping.unwrap_or_else(|| "undefined".into())));
    }

    pub(super) fn get_default_appearance(&mut self) -> R<NodeId> {
        if let Some(a) = self.default_appearance {
            return Ok(a);
        }

        let appearance = self.create("Appearance")?;
        let material = self.create("PhysicalMaterial")?;

        self.set(appearance, "alphaMode", FieldValue::SFString("OPAQUE".into()));
        self.set_node(appearance, "material", Some(material));
        self.set(material, "metallic", FieldValue::SFFloat(0.0));

        self.setup(material);
        self.setup(appearance);
        self.default_appearance = Some(appearance);
        Ok(appearance)
    }

    /// `materialObject ({ material })`
    pub(super) fn material_object(&mut self, mi: Option<usize>) -> R<NodeId> {
        let Some(mi) = mi else { return self.get_default_appearance() };
        let material = self.materials.at(mi);

        if !material.is_obj() {
            return self.get_default_appearance();
        }

        if let Some(a) = self.mats[mi].appearance {
            return Ok(a);
        }

        let indices = self.get_tex_coord_indices("", material, &mut vec![]);

        self.tex_coord_index = indices.iter().copied().fold(-1.0, x3d_fields::jsmath::js_max);
        self.texture_transform_nodes = vec![];
        self.tex_coord_mappings = vec![];
        self.tex_coord_of_node = HashMap::new();
        self.tex_coord_ext_of_node = HashMap::new();

        let appearance = self.create("Appearance")?;
        let material_node = self.create_material(material)?;
        let name = self.sanitize(material.get("name"));

        if let Some(c) = vector_value(material.get("emissiveFactor"), 3) {
            self.set_f(material_node, "emissiveColor", col3(&c));
        }

        let tex = self.texture_info(material.get("emissiveTexture"))?;

        self.set_node(material_node, "emissiveTexture", tex);

        let mapping = self.texture_mapping(material.get("emissiveTexture"));

        self.set_mapping(material_node, "emissiveTextureMapping", mapping);

        self.occlusion_texture_info(material.get("occlusionTexture"), material_node)?;
        self.normal_texture_info(material.get("normalTexture"), material_node)?;
        self.material_extensions(material.get("extensions"), material_node)?;
        self.setup(material_node);

        for i in &indices {
            let mapping = tc_name(*i);

            if !self.texture_transform_nodes.is_empty() {
                // If there are texture transform nodes, create a TextureTransform node for missing mappings.
                let missing = self.texture_transform_nodes.iter().all(|n| !matches!(self.get(*n, "mapping"), FieldValue::SFString(ref s) if *s == mapping));

                if missing {
                    let tt = self.create("TextureTransform")?;

                    // Flip Y
                    self.set(tt, "mapping", FieldValue::SFString(mapping.clone()));
                    self.set(tt, "translation", FieldValue::SFVec2f(Vector2::new(0.0, -1.0)));
                    self.set(tt, "scale", FieldValue::SFVec2f(Vector2::new(1.0, -1.0)));
                    self.setup(tt);
                    self.texture_transform_nodes.push(tt);
                }
            }

            self.set_mapping_entry(mapping, *i);
        }

        if !name.is_empty() {
            self.named(&name, appearance)?;
        }

        let alpha_mode = material.get("alphaMode").str().unwrap_or("OPAQUE").to_string();

        self.set(appearance, "alphaMode", FieldValue::SFString(alpha_mode));
        self.set(appearance, "alphaCutoff", FieldValue::SFFloat(material.get("alphaCutoff").number_or(0.5)));

        let final_material = self.khr_materials_unlit_object(material.get("extensions").get("KHR_materials_unlit"), material_node)?;

        self.set_node(appearance, "material", Some(final_material));

        let tt = self.create_multi_texture_transform(Some(final_material))?;

        self.set_node(appearance, "textureTransform", tt);
        self.setup(appearance);

        self.register_ptr(material, vec![appearance, material_node]);
        self.mats[mi].tex_coord_mappings = self.tex_coord_mappings.clone();
        self.mats[mi].appearance = Some(appearance);
        Ok(appearance)
    }

    fn set_mapping_entry(&mut self, mapping: String, i: f64) {
        match self.tex_coord_mappings.iter_mut().find(|(k, _)| *k == mapping) {
            Some(e) => e.1 = i,
            None => self.tex_coord_mappings.push((mapping, i)),
        }
    }

    fn get_tex_coord_indices(&self, key: &str, object: V<'j>, indices: &mut Vec<f64>) -> Vec<f64> {
        if !object.is_obj() {
            return indices.clone();
        }

        if key.ends_with("Texture") && !object.get("extensions").get("KHR_texture_transform").truthy() {
            let i = tex_coord_or_zero(object.get("texCoord"));

            if !indices.iter().any(|x| *x == i) {
                indices.push(i);
            }
        }

        for (k, v) in object.entries() {
            self.get_tex_coord_indices(&k, v, indices);
        }

        indices.clone()
    }

    fn create_material(&mut self, material: V<'j>) -> R<NodeId> {
        if let Some(n) = self.pbr_metallic_roughness_object(material.get("pbrMetallicRoughness"))? {
            return Ok(n);
        }

        if let Some(n) = self.pbr_specular_glossiness_object(material.get("extensions").get("KHR_materials_pbrSpecularGlossiness"))? {
            return Ok(n);
        }

        Ok(self.pbr_metallic_roughness_object_empty()?)
    }

    fn pbr_metallic_roughness_object_empty(&mut self) -> R<NodeId> {
        let node = self.create("PhysicalMaterial")?;

        self.set(node, "metallic", FieldValue::SFFloat(1.0));
        self.set(node, "roughness", FieldValue::SFFloat(1.0));
        self.set_mapping(node, "baseTextureMapping", Some(String::new()));
        self.set_mapping(node, "metallicRoughnessTextureMapping", Some(String::new()));
        // textureMapping (undefined) is "".
        self.set_node(node, "baseTexture", None);
        Ok(node)
    }

    fn pbr_metallic_roughness_object(&mut self, pbr: V<'j>) -> R<Option<NodeId>> {
        if !pbr.is_obj() {
            return Ok(None);
        }

        let node = self.create("PhysicalMaterial")?;

        if let Some(c) = vector_value(pbr.get("baseColorFactor"), 4) {
            let color = Color4::new(c[0], c[1], c[2], c[3]);
            let [r, g, b, a] = color.components();

            self.set(node, "baseColor", col3(&[r, g, b]));
            self.set(node, "transparency", FieldValue::SFFloat(1.0 - a));
        }

        self.set(node, "metallic", FieldValue::SFFloat(pbr.get("metallicFactor").number_or(1.0)));
        self.set(node, "roughness", FieldValue::SFFloat(pbr.get("roughnessFactor").number_or(1.0)));

        let t = self.texture_info(pbr.get("baseColorTexture"))?;

        self.set_node(node, "baseTexture", t);

        let m = self.texture_mapping(pbr.get("baseColorTexture"));

        self.set_mapping(node, "baseTextureMapping", m);

        let t = self.texture_info(pbr.get("metallicRoughnessTexture"))?;

        self.set_node(node, "metallicRoughnessTexture", t);

        let m = self.texture_mapping(pbr.get("metallicRoughnessTexture"));

        self.set_mapping(node, "metallicRoughnessTextureMapping", m);

        self.register_ptr(pbr, vec![node]);
        Ok(Some(node))
    }

    fn pbr_specular_glossiness_object(&mut self, sg: V<'j>) -> R<Option<NodeId>> {
        if !sg.is_obj() {
            return Ok(None);
        }

        let node = self.create("SpecularGlossinessMaterial")?;

        if let Some(c) = vector_value(sg.get("diffuseFactor"), 4) {
            let color = Color4::new(c[0], c[1], c[2], c[3]);
            let [r, g, b, a] = color.components();

            self.set(node, "diffuseColor", col3(&[r, g, b]));
            self.set(node, "transparency", FieldValue::SFFloat(1.0 - a));
        }

        let t = self.texture_info(sg.get("diffuseTexture"))?;

        self.set_node(node, "diffuseTexture", t);

        let m = self.texture_mapping(sg.get("diffuseTexture"));

        self.set_mapping(node, "diffuseTextureMapping", m);

        match vector_value(sg.get("specularFactor"), 3) {
            Some(c) => self.set(node, "specularColor", col3(&c)),
            None => self.set(node, "specularColor", FieldValue::SFColor(Color3::WHITE)),
        }

        self.set(node, "glossiness", FieldValue::SFFloat(sg.get("glossinessFactor").number_or(1.0)));

        let t = self.texture_info(sg.get("specularGlossinessTexture"))?;

        self.set_node(node, "specularGlossinessTexture", t);

        let m = self.texture_mapping(sg.get("specularGlossinessTexture"));

        self.set_mapping(node, "specularGlossinessTextureMapping", m);

        self.register_ptr(sg, vec![node]);
        Ok(Some(node))
    }

    fn occlusion_texture_info(&mut self, tex: V<'j>, material: NodeId) -> R<()> {
        if !tex.is_obj() {
            return Ok(());
        }

        self.set_f(material, "occlusionStrength", FieldValue::SFFloat(tex.get("strength").number_or(1.0)));

        let t = self.texture_info(tex)?;

        self.set_f(material, "occlusionTexture", FieldValue::SFNode(t));

        let m = self.texture_mapping(tex);

        self.set_mapping(material, "occlusionTextureMapping", m);
        Ok(())
    }

    fn normal_texture_info(&mut self, tex: V<'j>, material: NodeId) -> R<()> {
        if !tex.is_obj() {
            return Ok(());
        }

        self.set_f(material, "normalScale", FieldValue::SFFloat(tex.get("scale").number_or(1.0)));

        let t = self.texture_info(tex)?;

        self.set_f(material, "normalTexture", FieldValue::SFNode(t));

        let m = self.texture_mapping(tex);

        self.set_mapping(material, "normalTextureMapping", m);
        Ok(())
    }

    /// `textureInfo (texture)`: sets `texture .mapping` and returns the texture node.
    pub(super) fn texture_info(&mut self, texture: V<'j>) -> R<Option<NodeId>> {
        if !texture.is_obj() {
            return Ok(None);
        }

        let tex_coord = tex_coord_or_zero(texture.get("texCoord"));
        let mapping = if texture.get("extensions").is_obj() {
            self.texture_transform_object(texture.get("extensions").get("KHR_texture_transform"), tex_coord)?
        } else {
            Some(tc_name(tex_coord))
        };

        self.texture_mappings.insert(addr(texture), mapping);
        self.texture_object(texture.get("index"))
    }

    /// `textureMapping (texture)`
    pub(super) fn texture_mapping(&self, texture: V<'j>) -> Option<String> {
        if !texture.is_obj() {
            return Some(String::new());
        }

        self.texture_mappings.get(&addr(texture)).cloned().flatten()
    }

    fn texture_transform_object(&mut self, ext: V<'j>, tex_coord: f64) -> R<Option<String>> {
        if !ext.is_obj() {
            return Ok(None);
        }

        if !self.has_extension("KHR_texture_transform") {
            return Ok(None);
        }

        let tex_coord = if ext.get("texCoord").nn() { ext.get("texCoord").to_number() } else { tex_coord };

        let mut translation = Vector2::new(0.0, 0.0);
        let mut scale = Vector2::new(1.0, 1.0);

        // Reset matrix.
        let mut matrix = Matrix4::IDENTITY;

        // Flip Y
        matrix.scale(&Vector3::new(1.0, -1.0, 1.0));
        matrix.translate(&Vector3::new(0.0, -1.0, 0.0));

        if let Some(o) = vector_value(ext.get("offset"), 2) {
            translation.set(o[0], o[1]);
            matrix.translate(&Vector3::new(translation.x, translation.y, 0.0));
        }

        let angle = ext.get("rotation").number_or(0.0);

        matrix.rotate(&Rotation4::new(0.0, 0.0, -1.0, angle));

        if let Some(s) = vector_value(ext.get("scale"), 2) {
            scale.set(s[0], s[1]);
            matrix.scale(&Vector3::new(scale.x, scale.y, 1.0));
        }

        // Check for existing node.
        let existing = self.texture_transform_nodes.iter().copied().find(|n| {
            self.tex_coord_of_node.get(n) == Some(&tex_coord) && matches!(self.get(*n, "matrix"), FieldValue::SFMatrix4f(ref m) if m.equals(&matrix))
        });

        if let Some(existing) = existing {
            self.pointers.insert(addr(ext), Ptr::Existing);

            return Ok(match self.get(existing, "mapping") {
                FieldValue::SFString(s) => Some(s),
                _ => None,
            });
        }

        // Create new TextureTransformMatrix3D.
        let node = self.create("TextureTransformMatrix3D")?;
        let mapping = tc_name(self.tex_coord_index + self.texture_transform_nodes.len() as f64 + 1.0);

        self.set(node, "mapping", FieldValue::SFString(mapping.clone()));
        self.set(node, "matrix", FieldValue::SFMatrix4f(matrix));
        self.setup(node);

        self.texture_transform_nodes.push(node);
        self.set_mapping_entry(mapping.clone(), tex_coord);
        self.tex_coord_of_node.insert(node, tex_coord);
        self.tex_coord_ext_of_node.insert(node, addr(ext));

        self.pointers.insert(addr(ext), Ptr::TextureTransform { node, translation: [translation.x, translation.y], rotation: angle, scale: [scale.x, scale.y] });

        Ok(Some(mapping))
    }

    fn material_extensions(&mut self, extensions: V<'j>, material: NodeId) -> R<()> {
        if !extensions.is_obj() {
            return Ok(());
        }

        if self.type_name(material) != "PhysicalMaterial" {
            return Ok(());
        }

        for (key, value) in extensions.entries() {
            match key.as_str() {
                "KHR_materials_anisotropy" => self.ext_anisotropy(value, material)?,
                "KHR_materials_clearcoat" => self.ext_clearcoat(value, material)?,
                "KHR_materials_diffuse_transmission" => self.ext_diffuse_transmission(value, material)?,
                "KHR_materials_dispersion" => self.ext_dispersion(value, material)?,
                "KHR_materials_emissive_strength" => self.ext_emissive_strength(value, material)?,
                "KHR_materials_ior" => self.ext_ior(value, material)?,
                "KHR_materials_iridescence" => self.ext_iridescence(value, material)?,
                "KHR_materials_retroreflection" => self.ext_retroreflection(value, material)?,
                "KHR_materials_sheen" => self.ext_sheen(value, material)?,
                "KHR_materials_specular" => self.ext_specular(value, material)?,
                "KHR_materials_transmission" => self.ext_transmission(value, material)?,
                "KHR_materials_volume" => self.ext_volume(value, material)?,
                "KHR_materials_scatter" | "KHR_materials_volume_scatter" => self.ext_scatter(value, material)?,
                _ => {}
            }
        }

        Ok(())
    }

    fn finish_extension(&mut self, json: V<'j>, extension: NodeId, material: NodeId) {
        self.setup(extension);
        self.register_ptr(json, vec![extension]);
        self.push_node(material, "extensions", Some(extension));
    }

    /// `texture` into `<field>` and `<field>Mapping` of an extension node.
    fn tex(&mut self, ext: NodeId, field: &str, texture: V<'j>) -> R<()> {
        let t = self.texture_info(texture)?;

        self.set_node(ext, field, t);

        let m = self.texture_mapping(texture);

        self.set_mapping(ext, &format!("{field}Mapping"), m);
        Ok(())
    }

    fn null_read(v: V, prop: &str) -> R<()> {
        if matches!(v.0, Some(J::Null)) {
            return Err(format!("TypeError: Cannot read properties of null (reading '{prop}')"));
        }

        Ok(())
    }

    fn ext_anisotropy(&mut self, v: V<'j>, material: NodeId) -> R<()> {
        if !v.is_obj() {
            return Ok(());
        }

        let e = self.create_ext("AnisotropyMaterialExtension", "anisotropyStrength")?;

        self.set(e, "anisotropyStrength", FieldValue::SFFloat(v.get("anisotropyStrength").number_or(0.0)));
        self.set(e, "anisotropyRotation", FieldValue::SFFloat(v.get("anisotropyRotation").number_or(0.0)));
        self.tex(e, "anisotropyTexture", v.get("anisotropyTexture"))?;
        self.finish_extension(v, e, material);
        Ok(())
    }

    fn ext_clearcoat(&mut self, v: V<'j>, material: NodeId) -> R<()> {
        if !v.is_obj() {
            return Ok(());
        }

        let e = self.create_ext("ClearcoatMaterialExtension", "clearcoat")?;

        self.set(e, "clearcoat", FieldValue::SFFloat(v.get("clearcoatFactor").number_or(0.0)));
        self.tex(e, "clearcoatTexture", v.get("clearcoatTexture"))?;
        self.set(e, "clearcoatRoughness", FieldValue::SFFloat(v.get("clearcoatRoughnessFactor").number_or(0.0)));
        self.tex(e, "clearcoatRoughnessTexture", v.get("clearcoatRoughnessTexture"))?;
        self.tex(e, "clearcoatNormalTexture", v.get("clearcoatNormalTexture"))?;
        self.finish_extension(v, e, material);
        Ok(())
    }

    fn ext_diffuse_transmission(&mut self, v: V<'j>, material: NodeId) -> R<()> {
        if !v.is_obj() {
            return Ok(());
        }

        let e = self.create_ext("DiffuseTransmissionMaterialExtension", "diffuseTransmission")?;

        self.set(e, "diffuseTransmission", FieldValue::SFFloat(v.get("diffuseTransmissionFactor").number_or(0.0)));
        self.tex(e, "diffuseTransmissionTexture", v.get("diffuseTransmissionTexture"))?;

        if let Some(c) = vector_value(v.get("diffuseTransmissionColorFactor"), 3) {
            self.set(e, "diffuseTransmissionColor", col3(&c));
        }

        self.tex(e, "diffuseTransmissionColorTexture", v.get("diffuseTransmissionColorTexture"))?;
        self.finish_extension(v, e, material);
        Ok(())
    }

    fn ext_dispersion(&mut self, v: V<'j>, material: NodeId) -> R<()> {
        if !v.is_obj() {
            return Ok(());
        }

        let e = self.create_ext("DispersionMaterialExtension", "dispersion")?;

        self.set(e, "dispersion", FieldValue::SFFloat(v.get("dispersion").number_or(0.0)));
        self.finish_extension(v, e, material);
        Ok(())
    }

    fn ext_emissive_strength(&mut self, v: V<'j>, material: NodeId) -> R<()> {
        if !v.is_obj() {
            return Ok(());
        }

        let e = self.create_ext("EmissiveStrengthMaterialExtension", "emissiveStrength")?;

        self.set(e, "emissiveStrength", FieldValue::SFFloat(v.get("emissiveStrength").number_or(1.0)));
        self.finish_extension(v, e, material);
        Ok(())
    }

    fn ext_ior(&mut self, v: V<'j>, material: NodeId) -> R<()> {
        Self::null_read(v, "ior")?;
        let e = self.create_ext("IORMaterialExtension", "indexOfRefraction")?;

        self.set(e, "indexOfRefraction", FieldValue::SFFloat(v.get("ior").number_or(1.5)));
        self.finish_extension(v, e, material);
        Ok(())
    }

    fn ext_iridescence(&mut self, v: V<'j>, material: NodeId) -> R<()> {
        Self::null_read(v, "iridescenceFactor")?;
        let e = self.create_ext("IridescenceMaterialExtension", "iridescence")?;

        self.set(e, "iridescence", FieldValue::SFFloat(v.get("iridescenceFactor").number_or(0.0)));
        self.tex(e, "iridescenceTexture", v.get("iridescenceTexture"))?;
        self.set(e, "iridescenceIndexOfRefraction", FieldValue::SFFloat(v.get("iridescenceIor").number_or(1.3)));
        self.set(e, "iridescenceThicknessMinimum", FieldValue::SFFloat(v.get("iridescenceThicknessMinimum").number_or(100.0)));
        self.set(e, "iridescenceThicknessMaximum", FieldValue::SFFloat(v.get("iridescenceThicknessMaximum").number_or(400.0)));
        self.tex(e, "iridescenceThicknessTexture", v.get("iridescenceThicknessTexture"))?;
        self.finish_extension(v, e, material);
        Ok(())
    }

    fn ext_retroreflection(&mut self, v: V<'j>, material: NodeId) -> R<()> {
        if !v.is_obj() {
            return Ok(());
        }

        let e = self.create_ext("RetroreflectionMaterialExtension", "retroreflection")?;

        self.set(e, "retroreflection", FieldValue::SFFloat(v.get("retroreflectionFactor").number_or(0.0)));
        self.tex(e, "retroreflectionTexture", v.get("retroreflectionTexture"))?;
        self.finish_extension(v, e, material);
        Ok(())
    }

    fn ext_sheen(&mut self, v: V<'j>, material: NodeId) -> R<()> {
        if !v.is_obj() {
            return Ok(());
        }

        let e = self.create_ext("SheenMaterialExtension", "sheenColorTexture")?;

        if let Some(c) = vector_value(v.get("sheenColorFactor"), 3) {
            self.set(e, "sheenColor", col3(&c));
        }

        self.tex(e, "sheenColorTexture", v.get("sheenColorTexture"))?;
        self.set(e, "sheenRoughness", FieldValue::SFFloat(v.get("sheenRoughnessFactor").number_or(0.0)));
        self.tex(e, "sheenRoughnessTexture", v.get("sheenRoughnessTexture"))?;
        self.finish_extension(v, e, material);
        Ok(())
    }

    fn ext_specular(&mut self, v: V<'j>, material: NodeId) -> R<()> {
        if !v.is_obj() {
            return Ok(());
        }

        let e = self.create_ext("SpecularMaterialExtension", "specular")?;

        self.set(e, "specular", FieldValue::SFFloat(v.get("specularFactor").number_or(1.0)));
        self.tex(e, "specularTexture", v.get("specularTexture"))?;

        if let Some(c) = vector_value(v.get("specularColorFactor"), 3) {
            let strength = c.iter().copied().fold(f64::NEG_INFINITY, x3d_fields::jsmath::js_max);
            let color: Vec<f64> = c.iter().map(|x| x / strength).collect();
            let color = if strength != 0.0 && !strength.is_nan() { color } else { vec![0.0, 0.0, 0.0] };

            self.set(e, "specularColor", col3(&color));
            self.set(e, "specularStrength", FieldValue::SFFloat(strength));
        }

        self.tex(e, "specularColorTexture", v.get("specularColorTexture"))?;
        self.finish_extension(v, e, material);
        Ok(())
    }

    fn ext_transmission(&mut self, v: V<'j>, material: NodeId) -> R<()> {
        if !v.is_obj() {
            return Ok(());
        }

        let e = self.create_ext("TransmissionMaterialExtension", "transmission")?;

        self.set(e, "transmission", FieldValue::SFFloat(v.get("transmissionFactor").number_or(0.0)));
        self.tex(e, "transmissionTexture", v.get("transmissionTexture"))?;
        self.finish_extension(v, e, material);
        Ok(())
    }

    fn ext_volume(&mut self, v: V<'j>, material: NodeId) -> R<()> {
        Self::null_read(v, "thicknessFactor")?;
        let e = self.create_ext("VolumeMaterialExtension", "thickness")?;

        self.set(e, "thickness", FieldValue::SFFloat(v.get("thicknessFactor").number_or(0.0)));
        self.tex(e, "thicknessTexture", v.get("thicknessTexture"))?;
        self.set(e, "attenuationDistance", FieldValue::SFFloat(v.get("attenuationDistance").number_or(1_000_000.0)));

        if let Some(c) = vector_value(v.get("attenuationColor"), 3) {
            self.set(e, "attenuationColor", col3(&c));
        }

        self.finish_extension(v, e, material);
        Ok(())
    }

    fn ext_scatter(&mut self, v: V<'j>, material: NodeId) -> R<()> {
        if !v.is_obj() {
            return Ok(());
        }

        let e = self.create_ext("VolumeScatterMaterialExtension", "scatterStrength")?;

        self.set(e, "scatterStrength", FieldValue::SFFloat(v.get("scatterStrengthFactor").number_or(0.0)));

        // extension ._scatterStrengthTexture = textureInfo (scatterStrengthTexture)
        let t = self.texture_info(v.get("scatterStrengthTexture"))?;

        self.set_f(e, "scatterStrengthTexture", FieldValue::SFNode(t));

        // extension ._scatterTextureMapping = textureMapping (scatterTexture)
        let m = self.texture_mapping(v.get("scatterTexture"));

        self.set_f(e, "scatterTextureMapping", FieldValue::SFString(m.unwrap_or_default()));

        if let Some(c) = vector_value(v.get("multiscatterColorFactor"), 3) {
            self.set(e, "multiscatterColor", col3(&c));
        }

        self.tex(e, "multiscatterColorTexture", v.get("multiscatterColorTexture"))?;
        self.set(e, "scatterAnisotropy", FieldValue::SFFloat(v.get("scatterAnisotropy").number_or(0.0)));
        self.finish_extension(v, e, material);
        Ok(())
    }

    fn khr_materials_unlit_object(&mut self, unlit: V<'j>, material: NodeId) -> R<NodeId> {
        if !unlit.truthy() {
            return Ok(material);
        }

        let node = self.create("UnlitMaterial")?;

        for (to, from) in [("emissiveColor", "baseColor"), ("emissiveTextureMapping", "baseTextureMapping"), ("emissiveTexture", "baseTexture"), ("normalScale", "normalScale"), ("normalTextureMapping", "normalTextureMapping"), ("normalTexture", "normalTexture"), ("transparency", "transparency")] {
            if self.has_field(material, from) {
                let value = self.get(material, from);

                self.set(node, to, value);
            }
        }

        self.setup(node);
        self.add_pointer_alias(node, "baseColor", "emissiveColor");

        // `materialNode .dispose ()`: the replaced material is never set up by the runtime.
        self.scene.setup_order.retain(|n| *n != material);
        self.setup_done.remove(&material);
        Ok(node)
    }

    /// `hasTextures (materialNode)`: `getTextureBits` of the material or any of its extensions.
    pub(super) fn has_textures(&self, material: Option<NodeId>) -> bool {
        let Some(m) = material else { return false };
        let has = |n: NodeId| -> bool {
            self.scene.node(n).fields.iter().any(|f| f.name.ends_with("Texture") && matches!(f.value, FieldValue::SFNode(Some(_))))
        };

        if has(m) {
            return true;
        }

        if self.has_field(m, "extensions") {
            if let FieldValue::MFNode(list) = self.get(m, "extensions") {
                return list.into_iter().flatten().any(has);
            }
        }

        false
    }

    pub(super) fn create_multi_texture_transform(&mut self, material: Option<NodeId>) -> R<Option<NodeId>> {
        if !self.has_textures(material) {
            return Ok(None);
        }

        let mut nodes = self.texture_transform_nodes.clone();

        nodes.sort_by_key(|n| match self.get(*n, "mapping") {
            FieldValue::SFString(s) => s,
            _ => String::new(),
        });
        self.texture_transform_nodes = nodes.clone();

        match nodes.len() {
            0 => {
                if let Some(n) = self.default_texture_transform {
                    return Ok(Some(n));
                }

                let tt = self.create("TextureTransform")?;

                // Flip Y
                self.set(tt, "translation", FieldValue::SFVec2f(Vector2::new(0.0, -1.0)));
                self.set(tt, "scale", FieldValue::SFVec2f(Vector2::new(1.0, -1.0)));
                self.setup(tt);
                self.default_texture_transform = Some(tt);
                Ok(Some(tt))
            }
            1 => Ok(Some(nodes[0])),
            _ => {
                let tt = self.create("MultiTextureTransform")?;

                self.set(tt, "textureTransform", FieldValue::MFNode(nodes.into_iter().map(Some).collect()));
                self.setup(tt);
                Ok(Some(tt))
            }
        }
    }
}
