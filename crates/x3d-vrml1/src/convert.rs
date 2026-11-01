//! The conversion pass: the extension's `Converter` and the `convert` methods of its node
//! classes. VRML 1.0 properties are kept in stacks, one per kind of state; entering a
//! `Separator`, `Switch`, `LOD` or `WWWAnchor` saves the stack heights, leaving it truncates
//! the stacks back (`save` / `restore`), so a property lasts to the end of its group.
//!
//! State model, as the extension does it:
//!
//! * `transforms` / `groups`: the X3D `Transform` that stands for the VRML 1.0 group, and the
//!   X3D node that takes the children (the same `Transform`, or the `Switch`, `LOD` or `Anchor`
//!   inside it). A node is added to the top group, or to the scene's root nodes when no group
//!   is open.
//! * `materials` (starts with a default `Material`), `textures`, `texture_transforms`,
//!   `tex_coords`, `normals`, `coords` (these start with "none"): the node the next shape
//!   uses. `Material`, `Texture2`, `Texture2Transform`, `TextureCoordinate2`, `Normal` and
//!   `Coordinate3` push; shapes read the top.
//! * `shape_hints`, `normal_bindings`: pushed by `ShapeHints` and `NormalBinding`.
//!   `MaterialBinding` is parsed and ignored.
//! * `Translation`, `Rotation`, `Scale`, `Transform` and `MatrixTransform` do not push. They
//!   fold into the single X3D `Transform` of the innermost group (`transforms.last()`), by
//!   matrix multiplication, so they apply to **every** child of that group, also the ones
//!   before them. Outside any group they do nothing.
//! * Nodes with a `DEF` name: a node class that keeps state (`Material`, `Texture2`,
//!   `Texture2Transform`, `TextureCoordinate2`, `Normal`, `Coordinate3`) reuses the scene's
//!   node of that name when there is one, so `USE` shares. Every other class converts again
//!   (the extension's shared `use` helper has a typo and always fails), so `USE` of a shape,
//!   a `Separator`, a light and so on makes a copy under the current state and moves the DEF
//!   name to it.
//! * Not converted at all: `AsciiText`, `Background`, `CollideStyle`, `DirectedSound`,
//!   `ElevationGrid`, `Environment`, `FontStyle`, `NavigationInfo`, `PointSound`, `Text`,
//!   `TextLanguage`, `MaterialBinding`, and custom nodes.

use crate::parser::{Doc, V1Node, error_at};
use crate::{Options, Vrml1Error};
use x3d_fields::FieldValue as V;
use x3d_fields::math::{Color3, Matrix4, Rotation4, Vector3};
use x3d_fields::{FieldValue, NodeId};
use x3d_scene::{ROOT, Scene};

type R<T> = Result<T, Vrml1Error>;

const COMPONENTS: [&str; 5] = [
    "EnvironmentalSensor",
    "PointingDeviceSensor",
    "Scripting",
    "Sound",
    "Text",
];

#[derive(Clone, Copy)]
enum Material {
    Default,
    Node(NodeId),
}

#[derive(Clone, Copy)]
struct Hints {
    ccw: bool,
    solid: bool,
    convex: bool,
    crease_angle: f64,
}

#[derive(Clone, Copy)]
struct Saved([usize; 10]);

struct Converter<'a> {
    doc: &'a Doc,
    scene: Scene,
    options: &'a Options,
    transforms: Vec<NodeId>,
    groups: Vec<NodeId>,
    materials: Vec<Material>,
    textures: Vec<Option<NodeId>>,
    texture_transforms: Vec<Option<NodeId>>,
    tex_coords: Vec<Option<NodeId>>,
    normals: Vec<Option<NodeId>>,
    coords: Vec<Option<NodeId>>,
    shape_hints: Vec<Hints>,
    normal_bindings: Vec<String>,
    saved: Vec<Saved>,
    default_material: Option<NodeId>,
}

pub(crate) fn convert(doc: &Doc, world_url: &str, options: &Options, _src: &str) -> R<Scene> {
    let mut scene = Scene::new(world_url);

    scene.encoding = "VRML".into();
    scene.profile = Some("Interchange".into());

    for name in COMPONENTS {
        let level = scene.supported_component_level(name).unwrap_or(1);
        scene.update_component(name, level);
    }

    let mut c = Converter {
        doc,
        scene,
        options,
        transforms: vec![],
        groups: vec![],
        materials: vec![Material::Default],
        textures: vec![None],
        texture_transforms: vec![None],
        tex_coords: vec![None],
        normals: vec![None],
        coords: vec![None],
        shape_hints: vec![],
        normal_bindings: vec![],
        saved: vec![],
        default_material: None,
    };

    for &root in &doc.roots {
        c.convert(root)?;
    }

    let mut scene = c.scene;

    scene.setup_nodes();
    Ok(scene)
}

fn bad(message: impl Into<String>) -> Vrml1Error {
    // Conversion errors have no source position; the input is read to the end by then.
    error_at("", 0, message)
}

fn set(scene: &mut Scene, node: NodeId, name: &str, value: FieldValue) {
    let i = scene
        .get_predefined_field(node, name)
        .unwrap_or_else(|| panic!("{} has no field {name}", scene.type_name(node)));
    let f = &mut scene.node_mut(node).fields[i];

    f.value = value;
    f.modified = true;
}

fn get(scene: &Scene, node: NodeId, name: &str) -> FieldValue {
    scene.node(node).fields[scene.get_predefined_field(node, name).unwrap()]
        .value
        .clone()
}

fn push_child(scene: &mut Scene, node: NodeId, child: NodeId) {
    let i = scene.get_predefined_field(node, "children").unwrap();
    let f = &mut scene.node_mut(node).fields[i];

    if let V::MFNode(v) = &mut f.value {
        v.push(Some(child));
    }

    f.modified = true;
}

/// The i-th element of an MFColor value as an SFColor (clamped, as reading an element is).
fn color_at(v: &V1Node, name: &str, i: usize) -> Option<Color3> {
    match v.get(name) {
        V::MFColor(c) if c.len() > 3 * i + 2 => Some(Color3::new(
            c[3 * i] as f64,
            c[3 * i + 1] as f64,
            c[3 * i + 2] as f64,
        )),
        _ => None,
    }
}

fn float_at(v: &V1Node, name: &str, i: usize) -> Option<f64> {
    match v.get(name) {
        V::MFFloat(f) => f.get(i).map(|x| *x as f64),
        _ => None,
    }
}

fn vec3(v: &V1Node, name: &str) -> Vector3 {
    match v.get(name) {
        V::SFVec3f(x) => *x,
        _ => panic!("{name} is not an SFVec3f"),
    }
}

fn rot(v: &V1Node, name: &str) -> Rotation4 {
    match v.get(name) {
        V::SFRotation(x) => *x,
        _ => panic!("{name} is not an SFRotation"),
    }
}

impl Converter<'_> {
    fn save(&mut self) {
        self.saved.push(Saved([
            self.transforms.len(),
            self.groups.len(),
            self.materials.len(),
            self.textures.len(),
            self.texture_transforms.len(),
            self.tex_coords.len(),
            self.normals.len(),
            self.coords.len(),
            self.shape_hints.len(),
            self.normal_bindings.len(),
        ]));
    }

    fn restore(&mut self) {
        let Saved(n) = self.saved.pop().expect("restore without save");

        self.transforms.truncate(n[0]);
        self.groups.truncate(n[1]);
        self.materials.truncate(n[2]);
        self.textures.truncate(n[3]);
        self.texture_transforms.truncate(n[4]);
        self.tex_coords.truncate(n[5]);
        self.normals.truncate(n[6]);
        self.coords.truncate(n[7]);
        self.shape_hints.truncate(n[8]);
        self.normal_bindings.truncate(n[9]);
    }

    fn create(&mut self, type_name: &str) -> R<NodeId> {
        self.scene
            .create_node(ROOT, type_name)
            .ok_or_else(|| bad(format!("Unknown node type '{type_name}'.")))
    }

    /// `scene .updateNamedNode (this .name, node)`.
    fn name(&mut self, def: &str, node: NodeId) {
        if !def.is_empty() {
            self.scene.update_named_node(ROOT, def, node);
        }
    }

    /// The node that a state-keeping class reuses: the scene's node of the `DEF` name.
    fn named(&self, def: &str) -> Option<NodeId> {
        if def.is_empty() {
            None
        } else {
            self.scene.get_named_node(ROOT, def)
        }
    }

    /// Add to the innermost group, or to the root nodes.
    fn add(&mut self, node: NodeId) {
        match self.groups.last() {
            Some(&group) if !self.transforms.is_empty() => push_child(&mut self.scene, group, node),
            _ => self.scene.ctx_mut(ROOT).root_nodes.push(Some(node)),
        }
    }

    fn material_node(&mut self) -> R<NodeId> {
        match *self.materials.last().unwrap() {
            Material::Node(n) => Ok(n),
            Material::Default => match self.default_material {
                Some(n) => Ok(n),
                None => {
                    let n = self.create("Material")?;

                    self.default_material = Some(n);
                    Ok(n)
                }
            },
        }
    }

    /// Shape with Appearance (current material, texture and texture transform) and a geometry.
    fn shape(&mut self, def: &str, geometry_type: &str) -> R<(NodeId, NodeId)> {
        let shape = self.create("Shape")?;
        let appearance = self.create("Appearance")?;
        let geometry = self.create(geometry_type)?;

        self.name(def, shape);

        set(
            &mut self.scene,
            shape,
            "appearance",
            V::SFNode(Some(appearance)),
        );
        set(
            &mut self.scene,
            shape,
            "geometry",
            V::SFNode(Some(geometry)),
        );

        let material = self.material_node()?;

        set(
            &mut self.scene,
            appearance,
            "material",
            V::SFNode(Some(material)),
        );
        set(
            &mut self.scene,
            appearance,
            "texture",
            V::SFNode(*self.textures.last().unwrap()),
        );
        set(
            &mut self.scene,
            appearance,
            "textureTransform",
            V::SFNode(*self.texture_transforms.last().unwrap()),
        );

        Ok((shape, geometry))
    }

    fn solid_from_hints(&mut self, geometry: NodeId) {
        if let Some(h) = self.shape_hints.last() {
            set(&mut self.scene, geometry, "solid", V::SFBool(h.solid));
        }
    }

    fn convert_children(&mut self, node: &V1Node) -> R<()> {
        for &child in &node.children {
            self.convert(child)?;
        }

        Ok(())
    }

    /// Wrap `inner` in a `Transform`, add it to the current group, then convert the children
    /// into `inner` with the state saved.
    fn group(&mut self, node: &V1Node, transform: NodeId, inner: NodeId) -> R<()> {
        self.add(transform);
        self.name(&node.name, transform);

        self.save();
        self.transforms.push(transform);
        self.groups.push(inner);

        let r = self.convert_children(node);

        self.restore();
        r
    }

    fn convert(&mut self, index: usize) -> R<()> {
        let doc = self.doc;
        let node = &doc.nodes[index];

        match node.type_name.as_str() {
            "Group" => self.convert_children(node),
            "Separator" => {
                let transform = self.create("Transform")?;

                self.group(node, transform, transform)
            }
            "Switch" => {
                let transform = self.create("Transform")?;
                let switch = self.create("Switch")?;

                push_child(&mut self.scene, transform, switch);
                set(
                    &mut self.scene,
                    switch,
                    "whichChoice",
                    node.get("whichChild").clone(),
                );
                self.group(node, transform, switch)
            }
            "LOD" => {
                let transform = self.create("Transform")?;
                let lod = self.create("LOD")?;

                push_child(&mut self.scene, transform, lod);
                set(&mut self.scene, lod, "range", node.get("range").clone());
                set(&mut self.scene, lod, "center", node.get("center").clone());
                self.group(node, transform, lod)
            }
            "WWWAnchor" => {
                let transform = self.create("Transform")?;
                let anchor = self.create("Anchor")?;

                push_child(&mut self.scene, transform, anchor);
                set(&mut self.scene, anchor, "url", node.get("name").clone());
                set(
                    &mut self.scene,
                    anchor,
                    "description",
                    node.get("description").clone(),
                );
                self.group(node, transform, anchor)
            }
            "WWWInline" => {
                let inline = self.create("Inline")?;

                self.name(&node.name, inline);
                set(&mut self.scene, inline, "url", node.get("name").clone());
                self.add(inline);
                Ok(())
            }
            "Cube" => {
                let (shape, geometry) = self.shape(&node.name, "Box")?;

                self.solid_from_hints(geometry);
                set(
                    &mut self.scene,
                    geometry,
                    "size",
                    V::SFVec3f(Vector3::new(
                        node.float("width"),
                        node.float("height"),
                        node.float("depth"),
                    )),
                );
                self.add(shape);
                Ok(())
            }
            "Sphere" => {
                let (shape, geometry) = self.shape(&node.name, "Sphere")?;

                self.solid_from_hints(geometry);
                set(
                    &mut self.scene,
                    geometry,
                    "radius",
                    node.get("radius").clone(),
                );
                self.add(shape);
                Ok(())
            }
            "Cone" => {
                let (shape, geometry) = self.shape(&node.name, "Cone")?;
                let parts = node.string("parts");

                self.solid_from_hints(geometry);
                set(
                    &mut self.scene,
                    geometry,
                    "side",
                    V::SFBool(matches_any(parts, &["ALL", "SIDES"])),
                );
                set(
                    &mut self.scene,
                    geometry,
                    "bottom",
                    V::SFBool(matches_any(parts, &["ALL", "BOTTOM"])),
                );
                set(
                    &mut self.scene,
                    geometry,
                    "bottomRadius",
                    node.get("bottomRadius").clone(),
                );
                set(
                    &mut self.scene,
                    geometry,
                    "height",
                    node.get("height").clone(),
                );
                self.add(shape);
                Ok(())
            }
            "Cylinder" => {
                let (shape, geometry) = self.shape(&node.name, "Cylinder")?;
                let parts = node.string("parts");

                self.solid_from_hints(geometry);
                set(
                    &mut self.scene,
                    geometry,
                    "side",
                    V::SFBool(matches_any(parts, &["ALL", "SIDES"])),
                );
                set(
                    &mut self.scene,
                    geometry,
                    "top",
                    V::SFBool(matches_any(parts, &["ALL", "TOP"])),
                );
                set(
                    &mut self.scene,
                    geometry,
                    "bottom",
                    V::SFBool(matches_any(parts, &["ALL", "BOTTOM"])),
                );
                set(
                    &mut self.scene,
                    geometry,
                    "radius",
                    node.get("radius").clone(),
                );
                set(
                    &mut self.scene,
                    geometry,
                    "height",
                    node.get("height").clone(),
                );
                self.add(shape);
                Ok(())
            }
            "GeneralCylinder" => {
                let (shape, geometry) = self.shape(&node.name, "Extrusion")?;
                let parts = node.string("parts");

                if let Some(h) = self.shape_hints.last().copied() {
                    self.hints_to(geometry, h);
                    set(&mut self.scene, geometry, "solid", V::SFBool(h.solid));
                }

                set(
                    &mut self.scene,
                    geometry,
                    "spine",
                    node.get("spine").clone(),
                );
                set(
                    &mut self.scene,
                    geometry,
                    "crossSection",
                    node.get("crossSection").clone(),
                );
                set(
                    &mut self.scene,
                    geometry,
                    "beginCap",
                    V::SFBool(matches_any(parts, &["ALL", "BOTTOM"])),
                );
                set(
                    &mut self.scene,
                    geometry,
                    "endCap",
                    V::SFBool(matches_any(parts, &["ALL", "TOP"])),
                );
                self.add(shape);
                Ok(())
            }
            "IndexedFaceSet" => self.indexed_face_set(node),
            "IndexedLineSet" => {
                let (shape, geometry) = self.shape(&node.name, "IndexedLineSet")?;

                set(
                    &mut self.scene,
                    geometry,
                    "coordIndex",
                    node.get("coordIndex").clone(),
                );
                set(
                    &mut self.scene,
                    geometry,
                    "coord",
                    V::SFNode(*self.coords.last().unwrap()),
                );
                self.add(shape);
                Ok(())
            }
            "PointSet" => self.point_set(node),
            "Coordinate3" => self.state_node(
                node,
                "Coordinate",
                |s| &mut s.coords,
                |s, n| set(&mut s.scene, n, "point", node.get("point").clone()),
            ),
            "Normal" => self.state_node(
                node,
                "Normal",
                |s| &mut s.normals,
                |s, n| set(&mut s.scene, n, "vector", node.get("vector").clone()),
            ),
            "TextureCoordinate2" => self.state_node(
                node,
                "TextureCoordinate",
                |s| &mut s.tex_coords,
                |s, n| set(&mut s.scene, n, "point", node.get("point").clone()),
            ),
            "Texture2Transform" => self.state_node(
                node,
                "TextureTransform",
                |s| &mut s.texture_transforms,
                |s, n| {
                    set(
                        &mut s.scene,
                        n,
                        "translation",
                        node.get("translation").clone(),
                    );
                    set(&mut s.scene, n, "rotation", node.get("rotation").clone());
                    set(&mut s.scene, n, "scale", node.get("scaleFactor").clone());
                    set(&mut s.scene, n, "center", node.get("center").clone());
                },
            ),
            "Texture2" => self.texture2(node),
            "Material" => self.material(node),
            "NormalBinding" => {
                self.normal_bindings.push(node.string("value").to_string());
                Ok(())
            }
            "ShapeHints" => {
                self.shape_hints.push(Hints {
                    ccw: node.string("vertexOrdering") != "CLOCKWISE",
                    solid: node.string("shapeType") != "UNKNOWN_SHAPE_TYPE",
                    // The extension's `convex` getter reads `shapeType`, not `faceType`.
                    convex: node.string("shapeType") != "UNKNOWN_FACE_TYPE",
                    crease_angle: node.float("creaseAngle"),
                });
                Ok(())
            }
            "Translation" => self.fold_transform(|m| {
                m.translate(&vec3(node, "translation"));
            }),
            "Rotation" => self.fold_transform(|m| {
                m.rotate(&rot(node, "rotation"));
            }),
            "Scale" => self.fold_transform(|m| {
                m.scale(&vec3(node, "scaleFactor"));
            }),
            "MatrixTransform" => self.fold_transform(|m| {
                if let V::SFMatrix4f(x) = node.get("matrix") {
                    m.mult_left(x);
                }
            }),
            "Transform" => self.fold_transform(|m| {
                let mut t = Matrix4::IDENTITY;
                let (c, so) = (vec3(node, "center"), rot(node, "scaleOrientation"));

                t.set_transform(
                    Some(&vec3(node, "translation")),
                    Some(&rot(node, "rotation")),
                    Some(&vec3(node, "scaleFactor")),
                    Some(&so),
                    Some(&c),
                );
                m.mult_left(&t);
            }),
            "DirectionalLight" => {
                let light = self.create("DirectionalLight")?;

                self.name(&node.name, light);
                for f in ["on", "intensity", "color", "direction"] {
                    set(&mut self.scene, light, f, node.get(f).clone());
                }
                self.add(light);
                Ok(())
            }
            "PointLight" => {
                let light = self.create("PointLight")?;

                self.name(&node.name, light);
                for f in ["on", "intensity", "color", "location"] {
                    set(&mut self.scene, light, f, node.get(f).clone());
                }
                self.add(light);
                Ok(())
            }
            "SpotLight" => {
                let light = self.create("SpotLight")?;

                self.name(&node.name, light);
                for f in [
                    "on",
                    "intensity",
                    "color",
                    "location",
                    "direction",
                    "cutOffAngle",
                ] {
                    set(&mut self.scene, light, f, node.get(f).clone());
                }
                self.add(light);
                Ok(())
            }
            "PerspectiveCamera" => {
                let vp = self.create("Viewpoint")?;

                self.name(&node.name, vp);
                for (to, from) in [
                    ("position", "position"),
                    ("orientation", "orientation"),
                    ("fieldOfView", "heightAngle"),
                    ("nearDistance", "nearDistance"),
                    ("farDistance", "farDistance"),
                ] {
                    set(&mut self.scene, vp, to, node.get(from).clone());
                }
                self.add(vp);
                Ok(())
            }
            "OrthographicCamera" => {
                let vp = self.create("OrthoViewpoint")?;
                let h = node.float("height");

                self.name(&node.name, vp);
                set(
                    &mut self.scene,
                    vp,
                    "position",
                    node.get("position").clone(),
                );
                set(
                    &mut self.scene,
                    vp,
                    "orientation",
                    node.get("orientation").clone(),
                );
                set(
                    &mut self.scene,
                    vp,
                    "fieldOfView",
                    V::MFFloat(vec![
                        (-h / 2.0) as f32,
                        (-h / 2.0) as f32,
                        (h / 2.0) as f32,
                        (h / 2.0) as f32,
                    ]),
                );
                set(
                    &mut self.scene,
                    vp,
                    "nearDistance",
                    node.get("nearDistance").clone(),
                );
                set(
                    &mut self.scene,
                    vp,
                    "farDistance",
                    node.get("farDistance").clone(),
                );
                self.add(vp);
                Ok(())
            }
            "WorldInfo" => {
                let info = self.create("WorldInfo")?;

                self.name(&node.name, info);
                set(&mut self.scene, info, "title", node.get("title").clone());
                set(&mut self.scene, info, "info", node.get("info").clone());
                self.add(info);
                Ok(())
            }
            "Info" => self.info(node),
            // AsciiText, Background, CollideStyle, DirectedSound, ElevationGrid, Environment,
            // FontStyle, MaterialBinding, NavigationInfo, PointSound, Text, TextLanguage and
            // custom nodes: the extension has no conversion for them.
            _ => Ok(()),
        }
    }

    fn hints_to(&mut self, geometry: NodeId, h: Hints) {
        set(&mut self.scene, geometry, "ccw", V::SFBool(h.ccw));
        set(&mut self.scene, geometry, "solid", V::SFBool(h.solid));
        set(&mut self.scene, geometry, "convex", V::SFBool(h.convex));
        set(
            &mut self.scene,
            geometry,
            "creaseAngle",
            V::SFFloat(h.crease_angle),
        );
    }

    /// `Coordinate3`, `Normal`, `TextureCoordinate2`, `Texture2Transform`: create the X3D node,
    /// or reuse the one of the same `DEF` name, and push it on its stack.
    fn state_node(
        &mut self,
        node: &V1Node,
        x3d_type: &str,
        stack: fn(&mut Self) -> &mut Vec<Option<NodeId>>,
        fill: impl FnOnce(&mut Self, NodeId),
    ) -> R<()> {
        if let Some(existing) = self.named(&node.name) {
            stack(self).push(Some(existing));
            return Ok(());
        }

        let n = self.create(x3d_type)?;

        self.name(&node.name, n);
        fill(self, n);
        stack(self).push(Some(n));
        Ok(())
    }

    fn texture2(&mut self, node: &V1Node) -> R<()> {
        if let Some(existing) = self.named(&node.name) {
            self.textures.push(Some(existing));
            return Ok(());
        }

        let wrap_s = V::SFBool(node.string("wrapS") != "CLAMP");
        let wrap_t = V::SFBool(node.string("wrapT") != "CLAMP");
        let texture = if node.string("filename").is_empty() {
            let t = self.create("PixelTexture")?;

            self.name(&node.name, t);
            set(&mut self.scene, t, "image", node.get("image").clone());
            t
        } else {
            let t = self.create("ImageTexture")?;

            self.name(&node.name, t);
            set(
                &mut self.scene,
                t,
                "url",
                V::MFString(vec![node.string("filename").to_string()]),
            );
            t
        };

        set(&mut self.scene, texture, "repeatS", wrap_s);
        set(&mut self.scene, texture, "repeatT", wrap_t);
        self.textures.push(Some(texture));
        Ok(())
    }

    fn material(&mut self, node: &V1Node) -> R<()> {
        if let Some(existing) = self.named(&node.name) {
            self.materials.push(Material::Node(existing));
            return Ok(());
        }

        let m = self.create("Material")?;

        self.name(&node.name, m);

        if let Some(c) = color_at(node, "ambientColor", 0) {
            set(
                &mut self.scene,
                m,
                "ambientIntensity",
                V::SFFloat(c.get_hsv()[2]),
            );
        }
        for (to, from) in [
            ("diffuseColor", "diffuseColor"),
            ("specularColor", "specularColor"),
            ("emissiveColor", "emissiveColor"),
        ] {
            if let Some(c) = color_at(node, from, 0) {
                set(&mut self.scene, m, to, V::SFColor(c));
            }
        }
        for f in ["shininess", "transparency"] {
            if let Some(x) = float_at(node, f, 0) {
                set(&mut self.scene, m, f, V::SFFloat(x));
            }
        }

        self.materials.push(Material::Node(m));
        Ok(())
    }

    fn indexed_face_set(&mut self, node: &V1Node) -> R<()> {
        let (shape, geometry) = self.shape(&node.name, "IndexedFaceSet")?;

        if let Some(h) = self.shape_hints.last().copied() {
            self.hints_to(geometry, h);
        }

        // XXX in the extension: always set solid to false.
        set(&mut self.scene, geometry, "solid", V::SFBool(false));

        let coord_index = match node.get("coordIndex") {
            V::MFInt32(v) => v.clone(),
            _ => vec![],
        };
        let normal_index = node.get("normalIndex").clone();

        match self.normal_bindings.last().map(String::as_str) {
            None => set(&mut self.scene, geometry, "normalIndex", normal_index),
            Some("OVERALL") => {
                let v = coord_index
                    .iter()
                    .map(|i| if *i < 0 { -1 } else { 0 })
                    .collect();

                set(&mut self.scene, geometry, "normalIndex", V::MFInt32(v));
            }
            Some("PER_FACE") => set(
                &mut self.scene,
                geometry,
                "normalPerVertex",
                V::SFBool(false),
            ),
            Some("PER_FACE_INDEXED") => {
                set(
                    &mut self.scene,
                    geometry,
                    "normalPerVertex",
                    V::SFBool(false),
                );
                set(&mut self.scene, geometry, "normalIndex", normal_index);
            }
            Some("PER_VERTEX") => {}
            Some(_) => set(&mut self.scene, geometry, "normalIndex", normal_index),
        }

        set(
            &mut self.scene,
            geometry,
            "texCoordIndex",
            node.get("textureCoordIndex").clone(),
        );
        set(
            &mut self.scene,
            geometry,
            "coordIndex",
            V::MFInt32(coord_index),
        );
        set(
            &mut self.scene,
            geometry,
            "texCoord",
            V::SFNode(*self.tex_coords.last().unwrap()),
        );
        set(
            &mut self.scene,
            geometry,
            "normal",
            V::SFNode(*self.normals.last().unwrap()),
        );
        set(
            &mut self.scene,
            geometry,
            "coord",
            V::SFNode(*self.coords.last().unwrap()),
        );
        self.add(shape);
        Ok(())
    }

    fn point_set(&mut self, node: &V1Node) -> R<()> {
        let (shape, geometry) = self.shape(&node.name, "PointSet")?;

        if let Some(coord) = *self.coords.last().unwrap() {
            let start_index = match node.get("startIndex") {
                V::SFInt32(v) => *v,
                _ => 0,
            };

            if start_index == 0 {
                set(&mut self.scene, geometry, "coord", V::SFNode(Some(coord)));
            } else {
                let sliced = self.create("Coordinate")?;
                let num_points = match node.get("numPoints") {
                    V::SFInt32(v) => *v,
                    _ => -1,
                };
                let start = start_index.max(0) as usize;
                let length = num_points.max(0) as usize;
                let points = match get(&self.scene, coord, "point") {
                    V::MFVec3f(p) => p,
                    _ => vec![],
                };
                let count = points.len() / 3;
                let from = start.min(count);
                let to = start.saturating_add(length).min(count);

                set(
                    &mut self.scene,
                    sliced,
                    "point",
                    V::MFVec3f(points[3 * from..3 * to].to_vec()),
                );
                set(&mut self.scene, geometry, "coord", V::SFNode(Some(sliced)));
            }
        }

        self.add(shape);
        Ok(())
    }

    fn info(&mut self, node: &V1Node) -> R<()> {
        // No `USE` for Info.
        let string = node.string("string").to_string();

        if let Some(&group) = self.groups.last().filter(|_| !self.transforms.is_empty()) {
            let meta = self.create("MetadataString")?;

            self.name(&node.name, meta);
            set(&mut self.scene, meta, "name", V::SFString("Info".into()));
            set(
                &mut self.scene,
                meta,
                "reference",
                V::SFString(self.options.provider_url.clone()),
            );
            set(&mut self.scene, meta, "value", V::MFString(vec![string]));
            set(&mut self.scene, group, "metadata", V::SFNode(Some(meta)));
        } else {
            let info = self.create("WorldInfo")?;

            set(&mut self.scene, info, "title", V::SFString("Info".into()));
            set(&mut self.scene, info, "info", V::MFString(vec![string]));
            self.scene.ctx_mut(ROOT).root_nodes.push(Some(info));
        }

        Ok(())
    }

    /// Fold an operation into the X3D `Transform` of the innermost group: read its fields into a
    /// matrix, apply the operation, write the factored fields back.
    fn fold_transform(&mut self, op: impl FnOnce(&mut Matrix4)) -> R<()> {
        let Some(&t) = self.transforms.last() else {
            return Ok(());
        };
        let read = |s: &Scene, name: &str| get(s, t, name);
        let (
            V::SFVec3f(mut translation),
            V::SFRotation(mut rotation),
            V::SFVec3f(mut scale),
            V::SFRotation(mut scale_orientation),
            V::SFVec3f(center),
        ) = (
            read(&self.scene, "translation"),
            read(&self.scene, "rotation"),
            read(&self.scene, "scale"),
            read(&self.scene, "scaleOrientation"),
            read(&self.scene, "center"),
        )
        else {
            return Err(bad("Transform fields have unexpected types."));
        };
        let mut matrix = Matrix4::IDENTITY;

        matrix.set_transform(
            Some(&translation),
            Some(&rotation),
            Some(&scale),
            Some(&scale_orientation),
            Some(&center),
        );
        op(&mut matrix);
        matrix.get_transform(
            Some(&mut translation),
            Some(&mut rotation),
            Some(&mut scale),
            Some(&mut scale_orientation),
            Some(&center),
        );

        set(&mut self.scene, t, "translation", V::SFVec3f(translation));
        set(&mut self.scene, t, "rotation", V::SFRotation(rotation));
        set(&mut self.scene, t, "scale", V::SFVec3f(scale));
        set(
            &mut self.scene,
            t,
            "scaleOrientation",
            V::SFRotation(scale_orientation),
        );
        Ok(())
    }
}

/// JS `string .match (/A|B/)`: a match anywhere in the string.
fn matches_any(s: &str, words: &[&str]) -> bool {
    words.iter().any(|w| s.contains(w))
}
