//! Lights, cameras, nodes, skins, scenes and the viewpoint center of rotation of GLTF2Parser.js.

use super::util::*;
use super::*;
use std::f64::consts::PI;

fn undefined_read(key: &str) -> String {
    format!("TypeError: Cannot read properties of undefined (reading '{key}')")
}

fn col3(v: &[f64]) -> FieldValue {
    FieldValue::SFColor(Color3::new(v[0], v[1], v[2]))
}

fn vec3(v: Vector3) -> FieldValue {
    FieldValue::SFVec3f(v)
}

/// `id + 1` as a JS template/concatenation sees it.
fn id_plus_one(id: V) -> String {
    match id.0 {
        Some(J::String(s)) => format!("{s}1"),
        _ => number_to_string_f(id.to_number() + 1.0),
    }
}

fn number_to_string_f(n: f64) -> String {
    x3d_fields::jsmath::number_to_string(n)
}

impl<'j, 's> P<'j, 's> {
    pub(super) fn register_ptr(&mut self, v: V, nodes: Vec<NodeId>) {
        if v.0.is_some() {
            self.pointers.insert(addr(v), Ptr::Nodes(nodes));
        }
    }

    pub(super) fn add_pointer_alias(&mut self, node: NodeId, field: &str, alias: &str) {
        let key = format!("{}.{}", self.type_name(node), field);

        self.pointer_aliases.insert(key, alias.to_string());
    }

    // --- environment lights -----------------------------------------------------------------------

    pub(super) fn env_light_object(&mut self, id: V<'j>) -> R<Option<NodeId>> {
        let Some(list) = self.env_lights.arr() else {
            return Ok(None);
        };
        let Some(i) = id.index(list.len()) else {
            return Ok(None);
        };
        let light = self.env_lights.at(i);

        if !light.is_obj() {
            return Ok(None);
        }

        if let Some(n) = self.env_light_nodes.get(&i) {
            return Ok(Some(*n));
        }

        let node = self.create("EnvironmentLight")?;
        let name = format!("EnvironmentLight{}", id_plus_one(id));

        self.named(&name, node)?;
        self.exported(&name, node)?;

        self.set(node, "global", FieldValue::SFBool(false));
        self.set(
            node,
            "intensity",
            FieldValue::SFFloat(light.get("intensity").number_or(1.0)),
        );

        // `lightNode .rotation` is not a property of a node (fields are `_rotation`): never an Array.
        self.set(
            node,
            "rotation",
            FieldValue::SFRotation(Rotation4::new(0.0, 0.0, 1.0, PI)),
        );

        if let Some(coefficients) = light.get("irradianceCoefficients").arr() {
            let mut values: Vec<f64> = vec![];

            for c in coefficients {
                let Some(a) = V::new(c).arr() else { continue };

                values.extend(a.iter().map(|v| V::new(v).to_number()));
            }

            let current = match self.get(node, "diffuseCoefficients") {
                FieldValue::MFFloat(v) => v.iter().map(|x| *x as f64).collect::<Vec<_>>(),
                _ => vec![],
            };

            let mut all = current;
            all.extend(values);
            self.set_nums(node, "diffuseCoefficients", &all);
        }

        if let Some(specular) = light.get("specularImages").arr() {
            let base_images = V::new(specular.first().unwrap_or(&J::Null));

            let cube = self.create("ComposedCubeMapTexture")?;

            if let Some(base) = base_images.arr() {
                let faces = ["right", "left", "bottom", "top", "front", "back"];
                let mut resolved = vec![];

                for image in base {
                    let image = V::new(image);
                    let Some(images) = &self.images else {
                        return Err(undefined_read(&image.to_js_string()));
                    };

                    resolved.push(image.index(images.len()).and_then(|i| {
                        images[i]
                            .as_ref()
                            .map(|im| (im.name.clone(), im.uri.clone()))
                    }));
                }

                for (i, image) in resolved.into_iter().enumerate() {
                    let texture = self.create("ImageTexture")?;
                    let name = self
                        .sanitize_str(image.as_ref().and_then(|im| im.0.as_deref()).unwrap_or(""));

                    if !name.is_empty() {
                        self.named(&name, texture)?;
                    }

                    let face = faces.get(i).copied();
                    let description = match image.as_ref().and_then(|im| im.0.clone()) {
                        Some(n) => n,
                        None => format!("{} Texture", upper_first(face.unwrap_or("undefined"))),
                    };

                    self.set(texture, "description", FieldValue::SFString(description));
                    self.set(
                        texture,
                        "url",
                        FieldValue::MFString(match &image {
                            Some(im) => vec![im.1.clone().unwrap_or_default()],
                            None => vec![],
                        }),
                    );
                    self.setup(texture);

                    if let Some(face) = face {
                        self.set_node(cube, &format!("{face}Texture"), Some(texture));
                    }
                }

                self.setup(cube);
            }

            self.set_node(node, "specularTexture", Some(cube));
        }

        self.setup(node);
        self.register_ptr(light, vec![node]);
        self.env_light_nodes.insert(i, node);
        Ok(Some(node))
    }

    // --- punctual lights ------------------------------------------------------------------------------

    pub(super) fn light_object(&mut self, id: V<'j>) -> R<Option<NodeId>> {
        let Some(list) = self.lights.arr() else {
            return Ok(None);
        };
        let Some(i) = id.index(list.len()) else {
            return Ok(None);
        };
        let light = self.lights.at(i);

        if !light.is_obj() {
            return Ok(None);
        }

        if let Some(n) = self.light_nodes.get(&i) {
            return Ok(Some(*n));
        }

        let node = match light.get("type").str() {
            Some("directional") => self.create("DirectionalLight")?,
            Some("spot") => {
                let n = self.create("SpotLight")?;
                let range = light.get("range").number_or(0.0);

                self.set(
                    n,
                    "radius",
                    FieldValue::SFFloat(if range != 0.0 && !range.is_nan() {
                        range
                    } else {
                        -1.0
                    }),
                );
                self.set(
                    n,
                    "cutOffAngle",
                    FieldValue::SFFloat(light.get("outerConeAngle").number_or(PI / 4.0)),
                );
                self.set(
                    n,
                    "beamWidth",
                    FieldValue::SFFloat(light.get("innerConeAngle").number_or(0.0)),
                );
                self.add_pointer_alias(n, "range", "radius");
                self.add_pointer_alias(n, "outerConeAngle", "cutOffAngle");
                self.add_pointer_alias(n, "innerConeAngle", "beamWidth");
                n
            }
            Some("point") => {
                let n = self.create("PointLight")?;
                let range = light.get("range").number_or(0.0);

                self.set(
                    n,
                    "radius",
                    FieldValue::SFFloat(if range != 0.0 && !range.is_nan() {
                        range
                    } else {
                        -1.0
                    }),
                );
                self.add_pointer_alias(n, "range", "radius");
                n
            }
            _ => return Ok(None),
        };

        let sanitized = self.sanitize(light.get("name"));
        let name = if sanitized.is_empty() {
            format!("Light{}", id_plus_one(id))
        } else {
            sanitized
        };

        if let Some(c) = vector_value(light.get("color"), 3) {
            self.set(node, "color", col3(&c));
        }

        self.set(node, "global", FieldValue::SFBool(true));
        self.set(
            node,
            "intensity",
            FieldValue::SFFloat(light.get("intensity").number_or(1.0)),
        );

        self.named(&name, node)?;
        self.exported(&name, node)?;

        self.setup(node);
        self.register_ptr(light, vec![node]);
        self.light_nodes.insert(i, node);
        Ok(Some(node))
    }

    // --- cameras ------------------------------------------------------------------------------------------

    pub(super) fn cameras_array(&mut self, cameras: V<'j>) {
        if cameras.is_array() {
            self.cameras = cameras;
        }
    }

    pub(super) fn camera_object(&mut self, id: V<'j>) -> R<Option<NodeId>> {
        let Some(list) = self.cameras.arr() else {
            return Ok(None);
        };
        let Some(i) = id.index(list.len()) else {
            return Ok(None);
        };
        let camera = self.cameras.at(i);

        if !camera.is_obj() {
            return Ok(None);
        }

        if let Some(n) = self.camera_nodes.get(&i) {
            return Ok(*n);
        }

        let viewpoint = match camera.get("type").str() {
            Some("orthographic") => self.orthographic_camera(camera.get("orthographic"))?,
            Some("perspective") => self.perspective_camera(camera.get("perspective"))?,
            _ => None,
        };

        let Some(viewpoint) = viewpoint else {
            self.camera_nodes.insert(i, None);
            return Ok(None);
        };

        let name = self.sanitize(camera.get("name"));

        if !name.is_empty() {
            self.named(&name, viewpoint)?;
            self.exported(&name, viewpoint)?;
        }

        let desc = if camera.get("name").truthy() {
            camera.get("name").to_js_string()
        } else {
            format!("Viewpoint {}", id_plus_one(id))
        };

        self.set(
            viewpoint,
            "description",
            FieldValue::SFString(description(Some(&desc))),
        );
        self.set(viewpoint, "position", vec3(Vector3::ZERO));

        self.camera_nodes.insert(i, Some(viewpoint));
        Ok(Some(viewpoint))
    }

    fn orthographic_camera(&mut self, camera: V<'j>) -> R<Option<NodeId>> {
        if !camera.is_obj() {
            return Ok(None);
        }

        let node = self.create("OrthoViewpoint")?;
        let xmag = camera.get("xmag").num().unwrap_or(1.0);
        let ymag = camera.get("ymag").num().unwrap_or(1.0);

        // viewpointNode ._fieldOfView [0..3] = ...: element assignments into the default array.
        self.set(
            node,
            "fieldOfView",
            FieldValue::SFVec4f(Vector4::new(-xmag, -ymag, xmag, ymag)),
        );

        if let Some(n) = camera.get("znear").num() {
            self.set(node, "nearDistance", FieldValue::SFFloat(n));
        }

        if let Some(n) = camera.get("zfar").num() {
            self.set(node, "farDistance", FieldValue::SFFloat(n));
        }

        self.add_pointer_alias(node, "znear", "nearDistance");
        self.add_pointer_alias(node, "zfar", "farDistance");

        self.setup(node);
        self.pointers.insert(
            addr(camera),
            Ptr::Ortho {
                viewpoint: node,
                xmag,
                ymag,
            },
        );
        Ok(Some(node))
    }

    fn perspective_camera(&mut self, camera: V<'j>) -> R<Option<NodeId>> {
        if !camera.is_obj() {
            return Ok(None);
        }

        let node = self.create("Viewpoint")?;

        if let Some(n) = camera.get("yfov").num() {
            self.set(node, "fieldOfView", FieldValue::SFFloat(n));
        }

        if let Some(n) = camera.get("znear").num() {
            self.set(node, "nearDistance", FieldValue::SFFloat(n));
        }

        if let Some(n) = camera.get("zfar").num() {
            self.set(node, "farDistance", FieldValue::SFFloat(n));
        }

        self.add_pointer_alias(node, "yfov", "fieldOfView");
        self.add_pointer_alias(node, "znear", "nearDistance");
        self.add_pointer_alias(node, "zfar", "farDistance");

        self.setup(node);
        self.register_ptr(camera, vec![node]);
        Ok(Some(node))
    }

    // --- nodes ----------------------------------------------------------------------------------------------

    pub(super) fn nodes_array(&mut self, nodes: V<'j>) -> R<()> {
        let Some(list) = nodes.arr() else {
            return Ok(());
        };
        let mut out = Vec::with_capacity(list.len());

        for (index, node) in list.iter().enumerate() {
            out.push(self.node_object(V::new(node), index)?);
        }

        self.nodes = out;

        // 1. Replace skeleton nodes with humanoid.
        for i in 0..self.nodes.len() {
            self.node_skeleton(i);
        }

        // 2. Add children.
        for i in 0..self.nodes.len() {
            self.node_children(i)?;
        }

        Ok(())
    }

    fn node_object(&mut self, node: V<'j>, index: usize) -> R<NodeObj<'j>> {
        if !node.is_obj() {
            return Ok(NodeObj {
                j: UNDEF,
                transform: None,
                child: None,
                humanoid: None,
            });
        }

        let is_joint = self.joints.contains(&(index as f64).to_bits());
        let t = self.create(if is_joint { "HAnimJoint" } else { "Transform" })?;

        self.register_ptr(node, vec![t]);
        Ok(NodeObj {
            j: node,
            transform: Some(t),
            child: Some(t),
            humanoid: None,
        })
    }

    fn skin_index(&self, skin: V) -> Option<usize> {
        let i = skin.index(self.skins.len())?;

        self.skins[i].as_ref().map(|_| i)
    }

    fn node_skeleton(&mut self, ni: usize) {
        let Some(si) = self.skin_index(self.nodes[ni].j.get("skin")) else {
            return;
        };
        let skin = self.skins[si].as_ref().unwrap();
        let (humanoid, child) = (skin.humanoid, skin.child);
        let skeletons: Vec<usize> = skin
            .skeleton
            .iter()
            .filter_map(|s| s.index(self.nodes.len()))
            .collect();

        self.nodes[ni].transform = Some(child);

        if let Some(first) = skeletons.first() {
            self.nodes[*first].humanoid = Some(humanoid);
            self.nodes[*first].child = Some(child);
        }
    }

    fn node_children(&mut self, ni: usize) -> R<()> {
        let node = self.nodes[ni].j;
        let skin = self.skin_index(node.get("skin"));
        let transform = self.nodes[ni].transform;
        let name = self.sanitize(node.get("name"));

        // Name
        if !name.is_empty() && skin.is_none() {
            let Some(t) = transform else {
                return Err("Error: Couldn't add named node: node must be of type X3DNode.".into());
            };

            self.named(&name, t)?;

            if self.type_name(t) == "HAnimJoint" {
                self.set(
                    t,
                    "name",
                    FieldValue::SFString(node.get("name").str().unwrap_or("").to_string()),
                );
            }
        }

        // Get mesh.
        let instancing = node.get("extensions").get("EXT_mesh_gpu_instancing");
        let shapes = self.mesh_object(node.get("mesh"), skin, instancing)?;

        // Add camera.
        let viewpoint = self.camera_object(node.get("camera"))?;

        if let Some(vp) = viewpoint {
            let Some(t) = transform else {
                return Err(undefined_read("_children"));
            };

            self.push_node(t, "children", Some(vp));
        }

        // Handle extensions.
        self.node_extensions(ni)?;

        // Add children.
        let mut children = self.node_children_array(node.get("children"));

        let Some(t) = transform else {
            return Err(undefined_read("getType"));
        };
        let is_joint = self.type_name(t) == "HAnimJoint";

        if is_joint {
            // Add a HAnimSegment if there are recursive skeletons or Transform nodes as children.
            let mut out = vec![];

            for child in children {
                if self.type_name(child) != "HAnimJoint" {
                    let segment = self.create("HAnimSegment")?;

                    self.push_node(segment, "children", Some(child));
                    self.setup(segment);
                    out.push(segment);
                } else {
                    out.push(child);
                }
            }

            children = out;
        }

        for c in children {
            self.push_node(t, "children", Some(c));
        }

        if let Some(si) = skin {
            let skin = self.skins[si].as_mut().unwrap();

            if skin.name.is_none() {
                skin.name = node.get("name").str().map(str::to_string);
            }

            let humanoid = skin.humanoid;

            let Some(shapes) = shapes.filter(|s| !s.is_empty()) else {
                return Ok(());
            };

            let geometry = self.get_node(shapes[0], "geometry");
            let Some(geometry) = geometry else {
                return Err(undefined_read("normal"));
            };

            let normal = if self.has_field(geometry, "normal") {
                self.get_node(geometry, "normal")
            } else {
                None
            };
            let coord = if self.has_field(geometry, "coord") {
                self.get_node(geometry, "coord")
            } else {
                None
            };

            self.set_node(humanoid, "skinNormal", normal);
            self.set_node(humanoid, "skinCoord", coord);

            for s in shapes {
                self.push_node(humanoid, "skin", Some(s));
            }
        } else {
            // Set transformation matrix.
            if let Some(m) = vector_value(node.get("matrix"), 16) {
                let matrix = Matrix4(m.try_into().unwrap());
                let (mut translation, mut rotation, mut scale, mut so) = (
                    Vector3::ZERO,
                    Rotation4::identity(),
                    Vector3::ONE,
                    Rotation4::identity(),
                );

                matrix.get_transform(
                    Some(&mut translation),
                    Some(&mut rotation),
                    Some(&mut scale),
                    Some(&mut so),
                    None,
                );

                self.set(t, "translation", vec3(translation));
                self.set(t, "rotation", FieldValue::SFRotation(rotation));
                self.set(t, "scale", vec3(scale));
                self.set(t, "scaleOrientation", FieldValue::SFRotation(so));
            } else {
                if let Some(v) = vector_value(node.get("translation"), 3) {
                    self.set(t, "translation", vec3(Vector3::new(v[0], v[1], v[2])));
                }

                if let Some(q) = vector_value(node.get("rotation"), 4) {
                    let mut r = Rotation4::identity();

                    r.set_quaternion(&Quaternion::new(q[0], q[1], q[2], q[3]));
                    self.set(t, "rotation", FieldValue::SFRotation(r));
                }

                if let Some(v) = vector_value(node.get("scale"), 3) {
                    self.set(t, "scale", vec3(Vector3::new(v[0], v[1], v[2])));
                }
            }

            // Add Shape nodes.
            if let Some(shapes) = shapes {
                for s in shapes {
                    self.push_node(t, "children", Some(s));
                }
            }

            self.setup(t);
        }

        Ok(())
    }

    fn node_extensions(&mut self, ni: usize) -> R<()> {
        let node = self.nodes[ni].j;
        let extensions = node.get("extensions");

        if !extensions.is_obj() {
            return Ok(());
        }

        for (key, extension) in extensions.entries() {
            if !extension.is_obj() {
                continue;
            }

            match key.as_str() {
                "KHR_lights_punctual" => {
                    let Some(light) = self.light_object(extension.get("light"))? else {
                        continue;
                    };
                    let Some(t) = self.nodes[ni].transform else {
                        return Err(undefined_read("_children"));
                    };

                    self.push_node(t, "children", Some(light));
                }
                "KHR_node_visibility" => {
                    let Some(child) = self.nodes[ni].child else {
                        return Err(
                            "TypeError: Cannot set properties of undefined (setting '_visible')"
                                .into(),
                        );
                    };

                    self.register_ptr(extension, vec![child]);

                    let visible = if extension.get("visible").nn() {
                        extension.get("visible").truthy()
                    } else {
                        true
                    };

                    self.set(child, "visible", FieldValue::SFBool(visible));
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn node_children_array(&self, children: V<'j>) -> Vec<NodeId> {
        let Some(list) = children.arr() else {
            return vec![];
        };
        let mut out: Vec<NodeId> = vec![];

        for index in list {
            let Some(i) = V::new(index).index(self.nodes.len()) else {
                continue;
            };
            let node = &self.nodes[i];

            // `node ?.skin === undefined`
            if !node.j.get("skin").is_undef() {
                continue;
            }

            if let Some(child) = node.child {
                if !out.contains(&child) {
                    out.push(child);
                }
            }
        }

        out
    }

    // --- skins ----------------------------------------------------------------------------------------------

    pub(super) fn skins_array(&mut self, skins: V<'j>, nodes: V<'j>) -> R<()> {
        let (Some(list), Some(node_list)) = (skins.arr(), nodes.arr()) else {
            return Ok(());
        };

        self.skins_json = skins;
        self.skins = (0..list.len()).map(|_| None).collect();

        for (i, skin) in list.iter().enumerate() {
            let skin = V::new(skin);

            if !skin.is_obj() {
                continue;
            }

            self.skin_object(i, skin, node_list)?;
        }

        Ok(())
    }

    fn skin_object(&mut self, index: usize, skin: V<'j>, nodes: &'j [J]) -> R<()> {
        let skeleton = skin.get("skeleton");

        let used = {
            let mut found = false;

            for n in nodes {
                let n = V::new(n);

                if matches!(n.0, None | Some(J::Null)) {
                    return Err("TypeError: Cannot read properties of null (reading 'skin')".into());
                }

                if n.get("skin").num() == Some(index as f64) {
                    found = true;
                    break;
                }
            }

            found
        };

        // jointsArray: the original array object; `push` below appends to it.
        let mut joints: Vec<V<'j>> = match skin.get("joints").arr() {
            Some(a) => a.iter().map(V::new).collect(),
            None => vec![],
        };

        if used && skin.get("joints").is_array() {
            for j in &joints {
                if let Some(k) = num_key(*j) {
                    self.joints.insert(k);
                }
            }
        }

        let skeleton_list: Vec<V<'j>> = if !skeleton.is_undef() {
            vec![skeleton]
        } else {
            self.skeleton(&joints)
        };

        let inverse_bind = {
            let acc = self.accessor(skin.get("inverseBindMatrices"));

            match acc {
                None => vec![],
                Some(a) => {
                    let array = self.accessor_array(a)?;

                    array
                        .v
                        .chunks_exact(16)
                        .map(|c| Matrix4(c.try_into().unwrap()))
                        .collect()
                }
            }
        };

        if !skeleton.is_undef() {
            let included = joints.iter().any(|j| same_value_zero(*j, skeleton));

            if !included {
                if let Some(k) = num_key(skeleton) {
                    self.joints.insert(k);
                }

                joints.push(skeleton);
            }
        }

        let key = skeleton_list.first().and_then(|s| num_key(*s));
        let commons = match self.skeletons.get(&key) {
            Some(c) => *c,
            None => {
                let c = Commons {
                    child: self.create("Transform")?,
                    normal: self.create("Normal")?,
                    coordinate: self.create("Coordinate")?,
                };

                self.skeletons.insert(key, c);
                c
            }
        };

        let humanoid = self.create("HAnimHumanoid")?;
        let tex_coord = self.create("TextureCoordinate")?;
        let multi_tex_coord = self.create("MultiTextureCoordinate")?;

        self.push_node(commons.child, "children", Some(humanoid));
        self.set(
            tex_coord,
            "mapping",
            FieldValue::SFString("TEXCOORD_0".into()),
        );

        if self.children_of(commons.child, "children").len() == 1 {
            self.setup(commons.child);
            self.setup(commons.normal);
            self.setup(commons.coordinate);
        }

        self.setup(tex_coord);
        self.setup(multi_tex_coord);

        self.skins[index] = Some(Skin {
            j: skin,
            joints,
            skeleton: skeleton_list,
            inverse_bind,
            child: commons.child,
            normal: commons.normal,
            coordinate: commons.coordinate,
            humanoid,
            tex_coord,
            multi_tex_coord,
            name: skin.get("name").str().map(str::to_string),
        });

        Ok(())
    }

    /// `skeleton (joints)`: the joints that are not a descendant of another joint.
    fn skeleton(&self, joints: &[V<'j>]) -> Vec<V<'j>> {
        let mut children: Vec<u64> = vec![];

        for j in joints {
            self.skeleton_children(*j, &mut children, 0);
        }

        joints
            .iter()
            .copied()
            .filter(|j| !num_key(*j).is_some_and(|k| children.contains(&k)))
            .collect()
    }

    fn skeleton_children(&self, index: V<'j>, set: &mut Vec<u64>, depth: usize) {
        if depth > 10_000 {
            return;
        }

        // Must use nodes from input here.
        let nodes = self.root.get("nodes");
        let Some(i) = index.index(nodes.arr().map(|a| a.len()).unwrap_or(0)) else {
            return;
        };
        let node = nodes.at(i);

        if !node.is_obj() {
            return;
        }

        let Some(children) = node.get("children").arr() else {
            return;
        };

        for c in children {
            let c = V::new(c);

            if let Some(k) = num_key(c) {
                if !set.contains(&k) {
                    set.push(k);
                }
            }

            self.skeleton_children(c, set, depth + 1);
        }
    }

    pub(super) fn skins_humanoid(&mut self) -> R<()> {
        for si in 0..self.skins.len() {
            if self.skins[si].is_some() {
                self.skin_humanoid(si)?;
            }
        }

        Ok(())
    }

    fn skin_humanoid(&mut self, si: usize) -> R<()> {
        let skin = self.skins[si].as_ref().unwrap();
        let humanoid = skin.humanoid;
        let skin_name = skin.name.clone();
        let skeleton: Vec<V<'j>> = skin.skeleton.clone();
        let joints: Vec<V<'j>> = skin.joints.clone();
        let inverse: Vec<Matrix4> = skin.inverse_bind.clone();
        let name = self.sanitize_str(skin_name.as_deref().unwrap_or(""));

        if !name.is_empty() {
            self.named(&name, humanoid)?;
        }

        self.set(
            humanoid,
            "name",
            FieldValue::SFString(skin_name.unwrap_or_default()),
        );

        let vrm = self.root.get("extensions").get("VRM").truthy();

        self.set(
            humanoid,
            "skeletalConfiguration",
            FieldValue::SFString(if vrm { "VRM" } else { "GLTF" }.into()),
        );

        for s in skeleton {
            if let Some(i) = s.index(self.nodes.len()) {
                if let Some(t) = self.nodes[i].transform {
                    self.push_node(humanoid, "skeleton", Some(t));
                }
            }
        }

        let (mut positions, mut rotations, mut scales): (Vec<f64>, Vec<f64>, Vec<f64>) =
            (vec![], vec![], vec![]);

        for (i, joint) in joints.iter().enumerate() {
            let joint_node = joint
                .index(self.nodes.len())
                .and_then(|k| self.nodes[k].transform);
            let ibm = inverse.get(i).copied().unwrap_or(Matrix4::IDENTITY);

            let Some(jn) = joint_node else { continue };

            let (mut t, mut r, mut s) = (Vector3::ZERO, Rotation4::identity(), Vector3::ONE);

            ibm.get_transform(Some(&mut t), Some(&mut r), Some(&mut s), None, None);

            self.push_node(humanoid, "joints", Some(jn));
            positions.extend([t.x, t.y, t.z]);
            rotations.extend(r.components());
            scales.extend([s.x, s.y, s.z]);
        }

        // jointBindingPositions .push (translation) ...
        let append = |me: &mut Self, name: &str, add: &[f64]| {
            let current: Vec<f64> = match me.get(humanoid, name) {
                FieldValue::MFVec3f(v) => v.iter().map(|x| *x as f64).collect(),
                FieldValue::MFRotation(v) => v,
                _ => vec![],
            };
            let mut all = current;

            all.extend_from_slice(add);
            me.set_nums(humanoid, name, &all);
        };

        append(self, "jointBindingPositions", &positions);
        append(self, "jointBindingRotations", &rotations);
        append(self, "jointBindingScales", &scales);

        self.setup(humanoid);
        Ok(())
    }

    // --- scenes ------------------------------------------------------------------------------------------------------

    pub(super) fn scenes_array(&mut self, scenes: V<'j>, scene_number: V<'j>) -> R<()> {
        let Some(list) = scenes.arr() else {
            return Ok(());
        };
        let mut children: Vec<NodeId> = vec![];

        for (index, scene) in list.iter().enumerate() {
            if let Some(n) = self.scene_object(V::new(scene), index)? {
                children.push(n);
            }
        }

        // `sceneNumber = 0` replaces `undefined`, not `null`.
        let number = if scene_number.is_undef() {
            Some(0.0)
        } else {
            scene_number.num()
        };

        match children.len() {
            0 => Ok(()),
            1 if number == Some(0.0) => {
                self.root_push(children[0]);
                Ok(())
            }
            _ => {
                let switch = self.create("Switch")?;

                self.named("Scenes", switch)?;
                self.exported("Scenes", switch)?;

                let which = match scene_number.0 {
                    None => 0.0,
                    _ => scene_number.to_number(),
                };

                self.set(
                    switch,
                    "whichChoice",
                    FieldValue::SFInt32(x3d_fields::jsmath::to_int32(which)),
                );
                self.set(
                    switch,
                    "children",
                    FieldValue::MFNode(children.into_iter().map(Some).collect()),
                );
                self.setup(switch);
                self.add_pointer_alias(switch, "scene", "whichChoice");

                self.pointers
                    .insert(addr(self.root), Ptr::Nodes(vec![switch]));
                self.root_push(switch);
                Ok(())
            }
        }
    }

    fn scene_object(&mut self, scene: V<'j>, index: usize) -> R<Option<NodeId>> {
        if !scene.is_obj() {
            return Ok(None);
        }

        let light = self.env_light_object(
            scene
                .get("extensions")
                .get("EXT_lights_image_based")
                .get("light"),
        )?;
        let mut nodes = self.node_children_array(scene.get("nodes"));

        if let Some(l) = light {
            nodes.insert(0, l);
        }

        match nodes.len() {
            0 => Ok(None),
            1 => {
                let node = nodes[0];

                if self.node_name(node).is_empty() {
                    self.named(&format!("Scene{index}"), node)?;
                }

                Ok(Some(node))
            }
            _ => {
                let group = self.create("Group")?;
                // `scene .name` is the name of the X3DScene here (the block shadows the glTF scene).
                let name = format!("Scene{index}");

                self.named(&name, group)?;
                self.set(
                    group,
                    "children",
                    FieldValue::MFNode(nodes.into_iter().map(Some).collect()),
                );
                self.setup(group);
                Ok(Some(group))
            }
        }
    }
}

/// `array .includes (value)` equality (SameValueZero) for the joint indices.
fn same_value_zero(a: V, b: V) -> bool {
    match (a.0, b.0) {
        (Some(J::Number(x)), Some(J::Number(y))) => x == y || (x.is_nan() && y.is_nan()),
        (Some(J::String(x)), Some(J::String(y))) => x == y,
        (Some(J::Bool(x)), Some(J::Bool(y))) => x == y,
        (Some(J::Null), Some(J::Null)) => true,
        _ => false,
    }
}
