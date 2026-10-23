//! Port of src/x_ite/Parser/GLTF2Parser.js and GLB2Parser.js (X_ITE 16.5.0, pinned fork commit
//! 34cdf9f97): glTF 2.0 JSON and GLB containers into an X3D scene graph.
//!
//! The JS parser mutates the parsed JSON (it hangs `appearanceNode`, `shapeNodes`, `pointers`,
//! cached arrays ... on the input objects). The input is immutable here; the same caches live in
//! tables keyed by array index, or by the address of the JSON object for `pointers` (JS object
//! identity). Where the JS code throws (`TypeError` from a missing property, `RangeError` from a
//! typed array constructor), the error text is `String (error)` of that exception.
//!
//! Missing: `KHR_draco_mesh_compression` (X_ITE loads draco_decoder_gltf.js), BasisU, AVIF and
//! WebP textures (the url is passed on as X_ITE does, decoding is the texture loader's), and
//! video textures.

mod access;
mod animation;
mod bbox;
mod geometry;
mod glb;
mod material;
mod meshopt;
mod scenegraph;
mod scripts;
mod util;

pub use glb::{default_fetch, with_fetcher};
pub(crate) use glb::{is_valid_glb, parse_glb, parse_gltf};

use crate::json::JsValue as J;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use util::*;
use x3d_fields::math::*;
use x3d_fields::{FieldValue, NodeId};
use x3d_scene::*;

pub(super) type R<T> = Result<T, String>;

/// Resolves a URL string (already absolute) to bytes: the parser's FileLoader.
pub(crate) type Fetch<'a> = &'a dyn Fn(&str, &str) -> Result<Vec<u8>, String>;

const KEYS: [&str; 19] = [
    "accessors",
    "animations",
    "asset",
    "buffers",
    "bufferViews",
    "cameras",
    "extensions",
    "extensionsRequired",
    "extensionsUsed",
    "extras",
    "images",
    "materials",
    "meshes",
    "nodes",
    "samplers",
    "scene",
    "scenes",
    "skins",
    "textures",
];

/// GLTF2Parser.isValid on a parsed input.
pub(crate) fn is_valid_gltf(input: &J) -> bool {
    let J::Object(o) = input else { return false };

    if !o.iter().all(|(k, _)| KEYS.contains(&k.as_str())) {
        return false;
    }

    // `this .input .asset ?.version >= 2.0` with JS coercion.
    let version = V::new(input).get("asset").get("version");

    match version.0 {
        Some(J::Number(n)) => *n >= 2.0,
        Some(J::String(s)) => {
            let t = s.trim();
            let n = if t.is_empty() {
                0.0
            } else {
                t.parse::<f64>().unwrap_or(f64::NAN)
            };
            n >= 2.0
        }
        Some(J::Bool(b)) => (*b as u8 as f64) >= 2.0,
        _ => false,
    }
}

/// Everything that hangs on an accessor object in the JS parser.
#[derive(Default)]
pub(super) struct Acc {
    pub array: Option<Rc<TArr>>,
    pub color_node: Option<NodeId>,
    pub tangent_node: Option<NodeId>,
    pub coordinate_node: Option<NodeId>,
    pub normal_node: Option<NodeId>,
    /// `accessor .field`: the base values of a morphed Normal or Coordinate (f32 storage).
    pub field: Option<Vec<f32>>,
    /// `texCoord [mapping]`
    pub tex_coord_nodes: HashMap<String, Option<NodeId>>,
}

pub(super) struct Img {
    pub index: usize,
    pub uri: Option<String>,
    pub name: Option<String>,
}

#[derive(Default)]
pub(super) struct Mat {
    pub appearance: Option<NodeId>,
    pub tex_coord_mappings: Vec<(String, f64)>,
}

/// Per-primitive state (the properties the JS parser writes into the primitive object).
#[derive(Default)]
pub(super) struct Prim {
    /// `attributes` after `attributesObject` ran: name → accessor index.
    pub attributes: Vec<(String, Option<usize>)>,
    pub tex_coord: Vec<Option<usize>>,
    pub color: Vec<Option<usize>>,
    pub joints: Vec<Option<usize>>,
    pub weights: Vec<Option<usize>>,
    /// `texCoords .textureCoordinateNode`
    pub tex_coord_node: Option<NodeId>,
    pub resolved: bool,
    pub indices: Option<usize>,
    pub material: Option<usize>,
    pub appearance: Option<NodeId>,
    pub geometry: Option<NodeId>,
    pub shape: Option<NodeId>,
}

pub(super) struct NodeObj<'j> {
    pub j: V<'j>,
    pub transform: Option<NodeId>,
    pub child: Option<NodeId>,
    pub humanoid: Option<NodeId>,
}

pub(super) struct Skin<'j> {
    pub j: V<'j>,
    pub joints: Vec<V<'j>>,
    pub skeleton: Vec<V<'j>>,
    pub inverse_bind: Vec<Matrix4>,
    pub child: NodeId,
    pub normal: NodeId,
    pub coordinate: NodeId,
    pub humanoid: NodeId,
    pub tex_coord: NodeId,
    pub multi_tex_coord: NodeId,
    pub name: Option<String>,
}

#[derive(Clone, Copy)]
pub(super) struct Commons {
    pub child: NodeId,
    pub normal: NodeId,
    pub coordinate: NodeId,
}

pub(super) enum Ptr {
    Nodes(Vec<NodeId>),
    /// A KHR_texture_transform that reuses the matrix node of an earlier object (its `pointers`
    /// getter returns that object, which `getAnimationPointer` cannot map).
    Existing,
    /// KHR_texture_transform: a Script combining offset, rotation and scale, made on first use.
    TextureTransform {
        node: NodeId,
        translation: [f64; 2],
        rotation: f64,
        scale: [f64; 2],
    },
    /// Orthographic camera: a Script combining xmag and ymag, made on first use.
    Ortho {
        viewpoint: NodeId,
        xmag: f64,
        ymag: f64,
    },
}

pub(super) struct P<'j, 's> {
    pub scene: &'s mut Scene,
    pub root: V<'j>,
    pub base_url: String,
    pub fetch: Fetch<'s>,
    pub extensions: Vec<String>,

    pub env_lights: V<'j>,
    pub lights: V<'j>,
    pub env_light_nodes: HashMap<usize, NodeId>,
    pub light_nodes: HashMap<usize, NodeId>,
    pub material_variants: V<'j>,
    pub material_variant_nodes: Vec<NodeId>,

    pub buffers: Vec<Option<Rc<Vec<u8>>>>,
    pub buffer_views: V<'j>,
    pub bv_cache: Vec<Option<Rc<Vec<u8>>>>,
    pub accessors: V<'j>,
    pub acc: Vec<Acc>,
    pub samplers: V<'j>,
    pub sampler_nodes: Vec<Option<NodeId>>,
    pub images: Option<Vec<Option<Img>>>,
    pub videos: Option<Vec<Option<Img>>>,
    pub textures: Option<V<'j>>,
    pub texture_nodes: Vec<Option<NodeId>>,
    pub texture_cache: HashMap<String, NodeId>,
    pub materials: V<'j>,
    pub mats: Vec<Mat>,

    // Per materialObject run.
    pub texture_transform_nodes: Vec<NodeId>,
    pub tex_coord_mappings: Vec<(String, f64)>,
    pub tex_coord_of_node: HashMap<NodeId, f64>,
    pub tex_coord_ext_of_node: HashMap<NodeId, usize>,
    pub tex_coord_index: f64,

    pub meshes: V<'j>,
    pub mesh_shapes: HashMap<usize, Vec<NodeId>>,
    pub prims: HashMap<(usize, usize), Prim>,
    pub cameras: V<'j>,
    pub camera_nodes: HashMap<usize, Option<NodeId>>,
    pub nodes: Vec<NodeObj<'j>>,
    pub skins_json: V<'j>,
    pub skins: Vec<Option<Skin<'j>>>,
    pub skeletons: HashMap<Option<u64>, Commons>,
    pub joints: HashSet<u64>,
    pub pointer_aliases: HashMap<String, String>,
    pub pointers: HashMap<usize, Ptr>,
    pub animation_scripts: Vec<NodeId>,
    pub animation_nodes: HashMap<usize, NodeId>,
    pub default_appearance: Option<NodeId>,
    pub default_texture_transform: Option<NodeId>,
    pub meshopt: bool,
    pub texture_mappings: HashMap<usize, Option<String>>,
    /// Element type of the keyframe time array being converted (`times .map (...)` keeps it).
    pub times_ct: Ct,
    /// Nodes whose `setup ()` the parser called, to set them up in this order at load.
    pub setup_done: HashSet<NodeId>,
}

pub(super) fn addr(v: V) -> usize {
    v.0.map(|r| r as *const J as usize).unwrap_or(0)
}

/// `Number (x)` as a Set / Map key.
pub(super) fn num_key(v: V) -> Option<u64> {
    v.num().map(|n| {
        if n == 0.0 {
            0.0f64.to_bits()
        } else {
            n.to_bits()
        }
    })
}

impl<'j, 's> P<'j, 's> {
    pub fn new(scene: &'s mut Scene, root: V<'j>, base_url: &str, fetch: Fetch<'s>) -> Self {
        P {
            scene,
            root,
            base_url: base_url.to_string(),
            fetch,
            extensions: vec![],
            env_lights: UNDEF,
            lights: UNDEF,
            env_light_nodes: HashMap::new(),
            light_nodes: HashMap::new(),
            material_variants: UNDEF,
            material_variant_nodes: vec![],
            buffers: vec![],
            buffer_views: UNDEF,
            bv_cache: vec![],
            accessors: UNDEF,
            acc: vec![],
            samplers: UNDEF,
            sampler_nodes: vec![],
            images: None,
            videos: None,
            textures: None,
            texture_nodes: vec![],
            texture_cache: HashMap::new(),
            materials: UNDEF,
            mats: vec![],
            texture_transform_nodes: vec![],
            tex_coord_mappings: vec![],
            tex_coord_of_node: HashMap::new(),
            tex_coord_ext_of_node: HashMap::new(),
            tex_coord_index: -1.0,
            meshes: UNDEF,
            mesh_shapes: HashMap::new(),
            prims: HashMap::new(),
            cameras: UNDEF,
            camera_nodes: HashMap::new(),
            nodes: vec![],
            skins_json: UNDEF,
            skins: vec![],
            skeletons: HashMap::new(),
            joints: HashSet::new(),
            pointer_aliases: HashMap::new(),
            pointers: HashMap::new(),
            animation_scripts: vec![],
            animation_nodes: HashMap::new(),
            default_appearance: None,
            default_texture_transform: None,
            meshopt: false,
            texture_mappings: HashMap::new(),
            times_ct: Ct::F32,
            setup_done: HashSet::new(),
        }
    }

    // --- scene API ------------------------------------------------------------------------------

    /// `createNode (type, false)` whose result is then written to: when the node type is not
    /// available it is null, and the first assignment throws.
    pub fn create_ext(&mut self, type_name: &str, first_field: &str) -> R<NodeId> {
        match self.scene.create_node(ROOT, type_name) {
            Some(n) => Ok(n),
            None => Err(format!(
                "TypeError: Cannot set properties of null (setting '_{first_field}')"
            )),
        }
    }

    /// `node .setup ()`: the node is set up by the runtime when the scene is loaded, in the order
    /// of these calls (a second call does nothing).
    pub fn setup(&mut self, node: NodeId) {
        if self.setup_done.insert(node) {
            self.scene.setup_order.push(node);
        }
    }

    pub fn create(&mut self, type_name: &str) -> R<NodeId> {
        match self.scene.create_node(ROOT, type_name) {
            Some(n) => Ok(n),
            None => Err(format!("Error: Unknown node type '{type_name}'.")),
        }
    }

    pub fn set(&mut self, node: NodeId, name: &str, value: FieldValue) {
        crate::formats::set(self.scene, node, name, value);
    }

    pub fn set_nums(&mut self, node: NodeId, name: &str, values: &[f64]) {
        crate::formats::set_numbers(self.scene, node, name, values);
    }

    pub fn set_node(&mut self, node: NodeId, name: &str, value: Option<NodeId>) {
        crate::formats::set_node(self.scene, node, name, value);
    }

    pub fn push_node(&mut self, node: NodeId, name: &str, child: Option<NodeId>) {
        let i = self.scene.get_predefined_field(node, name).unwrap();
        let f = &mut self.scene.node_mut(node).fields[i];

        if let FieldValue::MFNode(v) = &mut f.value {
            v.push(child);
        }

        f.modified = true;
    }

    pub fn has_field(&self, node: NodeId, name: &str) -> bool {
        self.scene.get_predefined_field(node, name).is_some()
    }

    pub fn get(&self, node: NodeId, name: &str) -> FieldValue {
        crate::formats::node_field(self.scene, node, name)
    }

    pub fn get_node(&self, node: NodeId, name: &str) -> Option<NodeId> {
        match self.get(node, name) {
            FieldValue::SFNode(n) => n,
            _ => None,
        }
    }

    pub fn type_name(&self, node: NodeId) -> String {
        self.scene.type_name(node)
    }

    pub fn node_name(&self, node: NodeId) -> String {
        self.scene.node(node).name.clone()
    }

    /// `scene .addNamedNode (scene .getUniqueName (name), node)`.
    pub fn named(&mut self, name: &str, node: NodeId) -> R<()> {
        let unique = self.scene.get_unique_name(ROOT, name);

        if unique.is_empty() {
            return Err("Error: Couldn't add named node: node name is empty.".into());
        }

        if !self.scene.node(node).name.is_empty() {
            return Err(format!(
                "Error: Couldn't add named node: node named '{}' already has a name.",
                self.scene.node(node).name
            ));
        }

        self.scene.update_named_node(ROOT, &unique, node);
        Ok(())
    }

    /// `scene .addExportedNode (scene .getUniqueExportName (name), node)`.
    pub fn exported(&mut self, name: &str, node: NodeId) -> R<()> {
        let unique = self.scene.get_unique_export_name(name);

        self.scene
            .update_exported_node(ROOT, &unique, node, "")
            .map_err(|e| format!("Error: {e}"))
    }

    pub fn root_push(&mut self, node: NodeId) {
        self.scene.ctx_mut(ROOT).root_nodes.push(Some(node));
    }

    pub fn route(&mut self, src: NodeId, sf: &str, dst: NodeId, df: &str) -> R<()> {
        self.scene
            .add_route(ROOT, src, sf, dst, df)
            .map(|_| ())
            .map_err(|e| format!("Error: {e}"))
    }

    /// `X3DParser.sanitizeName`; `undefined` is the empty string.
    pub fn sanitize(&self, name: V) -> String {
        match name.0 {
            Some(J::String(s)) => crate::formats::sanitize_name(s),
            Some(J::Null) | None => String::new(),
            // `name .replace` on a non-string throws; a number has no `replace`.
            Some(_) => String::new(),
        }
    }

    pub fn sanitize_str(&self, name: &str) -> String {
        crate::formats::sanitize_name(name)
    }

    // --- root ------------------------------------------------------------------------------------

    pub fn root_object(&mut self) -> R<()> {
        let gltf = self.root;

        if !gltf.is_obj() {
            return Ok(());
        }

        self.scene.encoding = "GLTF".into();
        self.scene.profile = Some("Interchange".into());

        if gltf.get("skins").truthy() {
            self.update_component("HAnim", None);
        }

        self.asset_object(gltf.get("asset"), gltf.get("extensions"))?;
        self.extensions_array(gltf.get("extensionsRequired"));
        self.extensions_array(gltf.get("extensionsUsed"));
        self.extensions_object(gltf.get("extensions"));

        self.buffers_array(gltf.get("buffers"))?;
        self.buffer_views_array(gltf.get("bufferViews"));
        self.accessors_array(gltf.get("accessors"));
        self.samplers_array(gltf.get("samplers"));

        self.images_array(gltf.get("images"))?;
        self.videos_array(
            gltf.get("extensions")
                .get("EXT_texture_video")
                .get("videos"),
        )?;

        self.textures_array(gltf.get("textures"));
        self.materials_array(gltf.get("materials"));
        self.meshes_array(gltf.get("meshes"));
        self.cameras_array(gltf.get("cameras"));
        self.skins_array(gltf.get("skins"), gltf.get("nodes"))?;
        self.nodes_array(gltf.get("nodes"))?;
        self.scenes_array(gltf.get("scenes"), gltf.get("scene"))?;
        self.skins_humanoid()?;
        self.skins_bbox()?;
        self.animations_array(gltf.get("animations"))?;

        self.scene.setup_nodes();
        self.viewpoints_center_of_rotation();
        crate::optimizer::Optimizer {
            remove_empty_groups: true,
            combine_grouping_nodes: true,
            optimize_interpolators: true,
        }
        .optimize_scene_graph(self.scene, ROOT);

        self.export_group_lights("EnvironmentLights", true)?;
        self.export_group_lights("Lights", false)?;
        self.export_group_cameras()?;
        self.export_group_animations(gltf.get("animations"))?;

        self.cleanup_animation_scripts();
        self.material_variants_switch()?;

        Ok(())
    }

    pub fn update_component(&mut self, name: &str, level: Option<i32>) {
        let level = level
            .or_else(|| self.scene.supported_component_level(name))
            .unwrap_or(1);

        self.scene.update_component(name, level);
    }

    fn asset_object(&mut self, asset: V<'j>, extensions: V<'j>) -> R<()> {
        if !asset.is_obj() {
            return Ok(());
        }

        for (key, value) in asset.entries() {
            let Some(s) = value.str() else { continue };

            if key == "version" {
                continue;
            }

            self.add_meta(&key, s)?;
        }

        if asset.get("extras").is_obj() {
            for (key, value) in asset.get("extras").entries() {
                let Some(s) = value.str() else { continue };

                self.add_meta(&key, s)?;
            }
        }

        if asset.get("extensions").get("KHR_xmp_json_ld").is_obj() {
            let packet = asset.get("extensions").get("KHR_xmp_json_ld").get("packet");

            self.khr_xmp_json_ld_object(packet, extensions.get("KHR_xmp_json_ld"))?;
        }

        if extensions.get("VRM").get("meta").is_obj() {
            for (key, value) in extensions.get("VRM").get("meta").entries() {
                let Some(s) = value.str() else { continue };

                self.add_meta(&key, s)?;
            }
        }

        Ok(())
    }

    fn add_meta(&mut self, key: &str, value: &str) -> R<()> {
        if key.is_empty() {
            return Err("Error: Couldn't add metadata: name is empty.".into());
        }

        self.scene.meta.push((key.to_string(), value.to_string()));
        Ok(())
    }

    fn khr_xmp_json_ld_object(&mut self, index: V<'j>, ext: V<'j>) -> R<()> {
        if !ext.is_obj() {
            return Ok(());
        }

        let packets = ext.get("packets");

        let Some(packets_arr) = packets.arr() else {
            return Err(format!(
                "TypeError: Cannot read properties of undefined (reading '{}')",
                index.to_js_string()
            ));
        };

        let packet = match index.index(packets_arr.len()) {
            Some(i) => packets.at(i),
            None => return Err("TypeError: Cannot convert undefined or null to object".into()),
        };

        for (key, value) in packet.entries() {
            // key .match (/\w+:(.*)/)
            let Some(k) = xmp_key(&key) else { continue };

            if value.is_obj() {
                let set = value.get("@set");
                let array = if set.nn() { set } else { value.get("@list") };

                if let Some(a) = array.arr() {
                    let v = a
                        .iter()
                        .map(|v| V::new(v).to_js_string())
                        .collect::<Vec<_>>()
                        .join(", ");

                    self.add_meta(&k, &v)?;
                    continue;
                }

                let v = value.get("rdf:_1").get("@value");

                if v.truthy() {
                    let s = v.to_js_string();

                    self.add_meta(&k, &s)?;
                    continue;
                }
            }

            let Some(s) = value.str() else { continue };

            self.add_meta(&k, s)?;
        }

        Ok(())
    }

    fn extensions_array(&mut self, extensions: V<'j>) {
        let Some(list) = extensions.arr() else { return };
        let mut components: Vec<(&str, Option<i32>)> = vec![];

        for extension in list {
            let name = V::new(extension).to_js_string();

            if !self.extensions.contains(&name) {
                self.extensions.push(name.clone());
            }

            match name.as_str() {
                "EXT_lights_image_based" => components.push(("CubeMapTexturing", Some(3))),
                "EXT_mesh_gpu_instancing"
                | "KHR_materials_anisotropy"
                | "KHR_materials_clearcoat"
                | "KHR_materials_diffuse_transmission"
                | "KHR_materials_dispersion"
                | "KHR_materials_emissive_strength"
                | "KHR_materials_ior"
                | "KHR_materials_iridescence"
                | "KHR_materials_pbrSpecularGlossiness"
                | "KHR_materials_retroreflection"
                | "KHR_materials_sheen"
                | "KHR_materials_specular"
                | "KHR_materials_transmission"
                | "KHR_materials_scatter"
                | "KHR_materials_volume_scatter"
                | "KHR_materials_volume" => components.push(("X_ITE", Some(1))),
                "KHR_animation_pointer" => {
                    components.push(("EventUtilities", Some(1)));
                    components.push(("Scripting", Some(1)));
                }
                "KHR_gaussian_splatting" => components.push(("GaussianSplats", Some(1))),
                "KHR_meshopt_compression" | "EXT_meshopt_compression" => self.meshopt = true,
                "KHR_texture_transform" => components.push(("Texturing3D", Some(2))),
                _ => {}
            }
        }

        for (name, level) in components {
            self.update_component(name, level);
        }
    }

    fn extensions_object(&mut self, extensions: V<'j>) {
        if !extensions.is_obj() {
            return;
        }

        for (key, value) in extensions.entries() {
            match key.as_str() {
                "EXT_lights_image_based" => {
                    if value.is_obj() {
                        if value.get("lights").is_array() {
                            self.env_lights = value.get("lights");
                        }
                    }
                }
                "KHR_lights_punctual" => {
                    if value.is_obj() {
                        if value.get("lights").is_array() {
                            self.lights = value.get("lights");
                        }
                    }
                }
                "KHR_materials_variants" => {
                    if value.is_obj() && value.get("variants").is_array() {
                        self.material_variants = value.get("variants");
                    }
                }
                _ => {}
            }
        }
    }

    pub fn has_extension(&self, name: &str) -> bool {
        self.extensions.iter().any(|e| e == name)
    }

    // --- export groups, cleanup, variants ---------------------------------------------------------

    fn export_group(&mut self, name: &str, nodes: Vec<NodeId>) -> R<()> {
        if nodes.is_empty() {
            return Ok(());
        }

        let group = self.create("Group")?;

        self.named(name, group)?;
        self.exported(name, group)?;

        self.set(group, "visible", FieldValue::SFBool(false));
        self.set(
            group,
            "children",
            FieldValue::MFNode(nodes.into_iter().map(Some).collect()),
        );
        self.setup(group);
        self.root_push(group);
        Ok(())
    }

    fn export_group_lights(&mut self, name: &str, env: bool) -> R<()> {
        let (array, cache) = if env {
            (self.env_lights, &self.env_light_nodes)
        } else {
            (self.lights, &self.light_nodes)
        };
        let Some(list) = array.arr() else {
            return Ok(());
        };
        let nodes: Vec<NodeId> = (0..list.len())
            .filter_map(|i| cache.get(&i).copied())
            .collect();

        self.export_group(name, nodes)
    }

    fn export_group_cameras(&mut self) -> R<()> {
        let Some(list) = self.cameras.arr() else {
            return Ok(());
        };

        if list.iter().any(|c| matches!(c, J::Null)) {
            return Err("TypeError: Cannot read properties of null (reading 'node')".into());
        }

        let nodes: Vec<NodeId> = (0..list.len())
            .filter_map(|i| self.camera_nodes.get(&i).copied().flatten())
            .collect();

        self.export_group("Viewpoints", nodes)
    }

    fn export_group_animations(&mut self, animations: V<'j>) -> R<()> {
        let Some(list) = animations.arr() else {
            return Ok(());
        };

        if list.iter().any(|a| matches!(a, J::Null)) {
            return Err("TypeError: Cannot read properties of null (reading 'node')".into());
        }

        let nodes: Vec<NodeId> = (0..list.len())
            .filter_map(|i| self.animation_nodes.get(&i).copied())
            .collect();

        self.export_group("Animations", nodes)
    }

    fn material_variants_switch(&mut self) -> R<()> {
        if self.material_variant_nodes.is_empty() {
            return Ok(());
        }

        let switch = self.create("Switch")?;
        let names: Vec<V> = self
            .material_variants
            .arr()
            .map(|a| a.iter().map(V::new).collect())
            .unwrap_or_default();

        self.named("MaterialVariants", switch)?;
        self.exported("MaterialVariants", switch)?;

        self.set(switch, "whichChoice", FieldValue::SFInt32(0));
        self.set(switch, "visible", FieldValue::SFBool(false));
        self.setup(switch);

        // switchNode .setMetaData ("MaterialVariants/names", new Fields .MFString (... names))
        let names: Vec<String> = names
            .iter()
            .map(|v| {
                v.get("name")
                    .str()
                    .map(str::to_string)
                    .unwrap_or_else(|| "undefined".into())
            })
            .collect();

        self.set_meta_data_strings(switch, "MaterialVariants", "names", &names)?;

        let variants = self.material_variant_nodes.clone();

        for v in variants {
            self.route(switch, "whichChoice", v, "whichChoice")?;
        }

        self.root_push(switch);
        Ok(())
    }

    /// X3DNode.setMetaData ("<set>/<name>", MFString): a MetadataSet holding a MetadataString.
    fn set_meta_data_strings(
        &mut self,
        node: NodeId,
        set_name: &str,
        name: &str,
        values: &[String],
    ) -> R<()> {
        let set = self.create("MetadataSet")?;

        // `createNode (type)` with setup.
        self.setup(set);
        let unique = self.scene.get_unique_name(ROOT, set_name);

        self.scene.update_named_node(ROOT, &unique, set);
        self.set_node(node, "metadata", Some(set));
        self.set(set, "name", FieldValue::SFString(set_name.into()));

        if values.is_empty() {
            return Ok(());
        }

        let meta = self.create("MetadataString")?;

        self.setup(meta);
        let unique = self.scene.get_unique_name(ROOT, name);

        self.scene.update_named_node(ROOT, &unique, meta);
        self.set(meta, "name", FieldValue::SFString(name.into()));
        self.set(meta, "value", FieldValue::MFString(values.to_vec()));
        self.push_node(set, "value", Some(meta));
        Ok(())
    }

    /// X3DNode.dispose of a parsed Script: its DEF name, its routes and its exports go away.
    fn dispose_script(&mut self, node: NodeId) {
        let routes: Vec<usize> = self
            .scene
            .ctx(ROOT)
            .routes
            .iter()
            .enumerate()
            .filter(|(_, r)| r.source_node == node || r.destination_node == node)
            .map(|(i, _)| i)
            .collect();

        for index in routes.into_iter().rev() {
            let route = self.scene.ctx_mut(ROOT).routes.remove(index);

            if let Some(((sn, sl), (dn, dl))) = route.conn {
                let s = self.scene.field_mut(sn, sl);
                s.output_routes = s.output_routes.saturating_sub(1);

                let d = self.scene.field_mut(dn, dl);
                d.input_routes = d.input_routes.saturating_sub(1);
            }
        }

        let name = self.node_name(node);

        if !name.is_empty() {
            self.scene.remove_named_node(ROOT, &name);
        }

        // A disposed node is never set up by the runtime.
        self.scene.setup_order.retain(|n| *n != node);
        self.setup_done.remove(&node);
    }

    fn cleanup_animation_scripts(&mut self) {
        let scripts: Vec<NodeId> = self
            .scene
            .ctx(ROOT)
            .named_nodes
            .values()
            .iter()
            .copied()
            .filter(|n| self.type_name(*n) == "Script")
            .collect();

        for script in scripts {
            let (no_input, no_output) = {
                let n = self.scene.node(script);
                let all: Vec<&Field> = n.fields.iter().chain(n.user_fields.iter()).collect();

                (
                    all.iter().all(|f| f.input_routes == 0),
                    all.iter().all(|f| f.output_routes == 0),
                )
            };

            if no_input || no_output {
                self.dispose_script(script);
            }
        }
    }
}

/// `key .match (/\w+:(.*)/) [1]`.
fn xmp_key(key: &str) -> Option<String> {
    let b: Vec<char> = key.chars().collect();

    for (i, c) in b.iter().enumerate() {
        if *c == ':' && i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == '_') {
            let rest: String = b[i + 1..]
                .iter()
                .take_while(|c| !matches!(**c, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
                .collect();

            return Some(rest);
        }
    }

    None
}
