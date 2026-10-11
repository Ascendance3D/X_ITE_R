//! Scene traversal: the CAMERA pass that finds the bound bindables (X3DLayerNode.bindBindables,
//! BindableList.getBound) and the DISPLAY pass that collects shapes and lights
//! (X3DLayerNode.display, X3DGroupingNode.traverse, X3DTransformMatrix3DNode, Switch, LOD,
//! Billboard, Inline, X3DShapeNode, X3DRenderObject.addDisplayShape, X3DLightNode.push).

use crate::fog::{self, FogParams};
use crate::geometry::{self, Colors, ElevationInput, ExtrusionInput, Geometry, IfsInput};
use crate::sg::Sg;
use crate::text;
use crate::texture::{self, TextureImage};
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;
use x3d_fields::{FieldValue, NodeId};
use x3d_fields::math::{Matrix3, Matrix4, Rotation4, Vector2, Vector3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shading {
    /// Material with normals: Material2.glsl X3D_GOURAUD_MATERIAL (Shading GOURAUD default).
    Gouraud,
    /// UnlitMaterial / no material, or a Material on points and lines (X3D_UNLIT_MATERIAL).
    Unlit,
}

#[derive(Clone, Debug)]
pub struct MaterialParams {
    pub shading: Shading,
    pub diffuse: [f64; 3],
    pub ambient_intensity: f64,
    pub specular: [f64; 3],
    pub shininess: f64,
    pub emissive: [f64; 3],
    pub transparency: f64,
}

#[derive(Clone, Debug)]
pub struct DrawItem {
    pub geometry_node: NodeId,
    pub geometry: Rc<Geometry>,
    pub model_view: Matrix4,
    pub material: MaterialParams,
    pub texture: Option<Rc<TextureImage>>,
    /// x3d_TextureMatrix [0] (TextureTransform, column-major for GLSL).
    pub texture_matrix: [f32; 16],
    pub transparent: bool,
    /// Sort key for transparent shapes: bbox center z in view space.
    pub distance: f64,
}

#[derive(Clone, Debug)]
pub struct LightParams {
    /// 1 directional, 2 point, 3 spot (x3d_DirectionalLight …).
    pub kind: u32,
    pub color: [f64; 3],
    pub intensity: f64,
    pub ambient_intensity: f64,
    pub direction: Vector3,
    pub location: Vector3,
    pub attenuation: [f64; 3],
    pub radius: f64,
    pub cut_off_angle: f64,
    pub beam_width: f64,
    /// Inverse of the light's model-view 3×3 (point and spot lights).
    pub matrix: [f32; 9],
}

#[derive(Clone, Debug)]
pub struct BackgroundParams {
    /// Vertex colors (rgba) and positions (xyzw) of the sphere or cube, X3DBackgroundNode.build.
    pub colors: Vec<f32>,
    pub sphere: Vec<f32>,
    pub transparency: f64,
    pub model_view: Matrix4,
    /// Face textures in draw order (X3DBackgroundNode.drawCube); empty without textures.
    pub faces: Vec<BackgroundFace>,
}

/// One loaded Background / TextureBackground face (X3DBackgroundNode.textureBits).
#[derive(Clone, Debug)]
pub struct BackgroundFace {
    /// Six vertices, xyzw (X3DBackgroundNode.transferRectangle).
    pub vertices: [f32; 24],
    pub texture: Rc<TextureImage>,
}

pub struct Frame {
    pub projection: Matrix4,
    pub view: Matrix4,
    pub items: Vec<DrawItem>,
    pub lights: Vec<LightParams>,
    pub background: Option<BackgroundParams>,
    /// The bound Fog, if it draws fog (fogType != 0).
    pub fog: Option<FogParams>,
    /// Node types encountered that the renderer does not draw.
    pub unsupported: BTreeSet<String>,
    pub viewpoint: Option<NodeId>,
}

pub const MAX_LIGHTS: usize = 8;

/// Camera and bindable state supplied by an interactive viewer (x3d-viewer) instead of the
/// "first node found" choice of the static render: the bound viewpoint's cameraSpaceMatrix
/// (user position, orientation and model matrix), its user field of view, near/far values, the
/// bound NavigationInfo headlight, and the bound Background and Fog.
#[derive(Clone, Debug)]
pub struct CameraOverride {
    pub camera_space: Matrix4,
    pub fov: f64,
    pub near: f64,
    pub far: f64,
    pub headlight: bool,
    pub viewpoint: Option<NodeId>,
    pub navigation_info: Option<NodeId>,
    pub background: Option<(NodeId, Matrix4)>,
    pub fog: Option<NodeId>,
}

struct Bound {
    viewpoint: Option<(NodeId, Matrix4)>,
    navigation_info: Option<NodeId>,
    background: Option<(NodeId, Matrix4)>,
    fog: Option<NodeId>,
    /// Model matrix of every Fog met by the CAMERA pass (Fog.traverse: modelMatrix).
    fog_matrices: HashMap<NodeId, Matrix4>,
}

/// Runtime identity + node: Inline scenes live in their own runtimes with their own node ids.
type Key = (usize, NodeId);

fn key(sg: &Sg, n: NodeId) -> Key {
    (sg.rt as *const x3d_runtime::Runtime as usize, n)
}

/// Geometry and texture results kept between frames by an interactive viewer, so an unchanged
/// node is not rebuilt (X_ITE rebuilds a geometry only when one of its fields changes:
/// X3DGeometryNode.set_live / addInterest rebuild). A node is rebuilt when the write stamp
/// ([`x3d_runtime::Runtime::stamp`]) of the node or of any node its fields reference changed.
#[derive(Default)]
pub struct RenderCache {
    geometries: HashMap<(u64, NodeId), CacheEntry<Geometry>>,
    textures: HashMap<(u64, NodeId), CacheEntry<TextureImage>>,
    /// Background face images (the six `*Url` fields of a Background), by node and face.
    faces: HashMap<(u64, NodeId, usize), CacheEntry<TextureImage>>,
    /// X_ITE's 1x1 white default texture, shared so the GPU keeps one bind group for it.
    white: Option<Rc<TextureImage>>,
    frame: u64,
    /// Counters for tests and tools: geometries (re)built / reused in the last frame.
    pub built: usize,
    pub reused: usize,
}

struct CacheEntry<T> {
    fingerprint: u64,
    value: Option<Rc<T>>,
    used: u64,
}

/// How many frames an unused entry is kept (a Switch or LOD may bring the node back).
const CACHE_KEEP_FRAMES: u64 = 600;

impl RenderCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop entries that have not been used for a while; call once per frame after the build.
    fn end_frame(&mut self) {
        let frame = self.frame;
        self.geometries.retain(|_, e| e.used + CACHE_KEEP_FRAMES >= frame);
        self.textures.retain(|_, e| e.used + CACHE_KEEP_FRAMES >= frame);
        self.faces.retain(|_, e| e.used + CACHE_KEEP_FRAMES >= frame);
    }
}

/// FNV-style mix of the write stamps of `n` and of the nodes its fields reference.
fn fingerprint(sg: &Sg, n: NodeId) -> u64 {
    fn mix(h: &mut u64, v: u64) {
        *h = (*h ^ v).wrapping_mul(0x100000001b3).rotate_left(23);
    }

    fn walk(sg: &Sg, n: NodeId, depth: usize, h: &mut u64) {
        mix(h, n as u64);
        mix(h, sg.rt.stamp(n));

        if depth >= 3 {
            return;
        }

        for f in &sg.rt.scene.node(n).fields {
            match &f.value {
                FieldValue::SFNode(Some(c)) => {
                    if let Some(c) = sg.inner(*c) {
                        walk(sg, c, depth + 1, h);
                    }
                }
                FieldValue::MFNode(v) => {
                    for c in v.iter().flatten() {
                        if let Some(c) = sg.inner(*c) {
                            walk(sg, c, depth + 1, h);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let mut h = 0xcbf29ce484222325u64;
    walk(sg, n, 0, &mut h);
    h
}

pub struct Builder<'a> {
    /// The world's runtime: its fetcher serves every load (Inline runtimes are detached).
    root: &'a x3d_runtime::Runtime,
    geometries: HashMap<Key, Option<Rc<Geometry>>>,
    textures: HashMap<Key, Option<Rc<TextureImage>>>,
    pub unsupported: BTreeSet<String>,
    items: Vec<DrawItem>,
    lights: Vec<LightParams>,
    /// Interactive camera (see [`CameraOverride`]); `None` for the oracle renders.
    pub camera: Option<CameraOverride>,
    /// Results kept between frames (interactive viewer); `None` rebuilds everything per frame.
    cache: Option<&'a mut RenderCache>,
}

/// Children-like fields of the grouping nodes.
fn children_of(sg: &Sg, n: NodeId, ty: &str) -> Vec<NodeId> {
    match ty {
        "Switch" => {
            let choice = sg.i32(n, "whichChoice", -1);
            // VRML 2.0 Switch has `choice`, X3D `children`; X_ITE aliases them.
            let mut kids = sg.nodes(n, "children");
            if kids.is_empty() {
                kids = sg.nodes(n, "choice");
            }
            let raw = {
                let mut r = sg.raw_nodes(n, "children");
                if r.is_empty() {
                    r = sg.raw_nodes(n, "choice");
                }
                r
            };
            let _ = kids;
            if choice >= 0 && (choice as usize) < raw.len() { raw[choice as usize].and_then(|c| sg.inner(c)).into_iter().collect() } else { vec![] }
        }
        _ => {
            let mut kids = sg.nodes(n, "children");
            if kids.is_empty() && ty == "LOD" {
                kids = sg.nodes(n, "level");
            }
            kids
        }
    }
}

const GROUPS: &[&str] = &["Group", "Transform", "Anchor", "Collision", "Billboard", "Switch", "LOD", "StaticGroup"];

impl<'a> Builder<'a> {
    pub fn new(root: &'a x3d_runtime::Runtime) -> Self {
        Builder { root, geometries: HashMap::new(), textures: HashMap::new(), unsupported: BTreeSet::new(), items: vec![], lights: vec![], camera: None, cache: None }
    }

    /// Keep geometry and texture results in `cache` between frames.
    pub fn with_cache(mut self, cache: &'a mut RenderCache) -> Self {
        cache.frame += 1;
        cache.built = 0;
        cache.reused = 0;
        self.cache = Some(cache);
        self
    }

    /// Transform: Matrix4.setTransform (translation, rotation, scale, scaleOrientation, center).
    fn transform_matrix(&self, sg: &Sg, n: NodeId) -> Matrix4 {
        let t = sg.vec3(n, "translation", Vector3::ZERO);
        let r = sg.rotation(n, "rotation");
        let s = sg.vec3(n, "scale", Vector3::ONE);
        let so = sg.rotation(n, "scaleOrientation");
        let c = sg.vec3(n, "center", Vector3::ZERO);
        let mut m = Matrix4::IDENTITY;
        m.set_transform(Some(&t), Some(&r), Some(&s), Some(&so), Some(&c));
        m
    }

    /// Billboard.rotate.
    fn billboard_matrix(&self, sg: &Sg, n: NodeId, model_view: &Matrix4) -> Matrix4 {
        let mut inv = *model_view;
        inv.inverse();
        let mut to_viewer = Vector3::new(inv.0[12], inv.0[13], inv.0[14]);
        to_viewer.normalize();
        let axis = sg.vec3(n, "axisOfRotation", Vector3::new(0.0, 1.0, 0.0));

        if axis.equals(&Vector3::ZERO) {
            let mut viewer_y = Vector3::Y_AXIS;
            inv.mult_dir_matrix(&mut viewer_y);
            viewer_y.normalize();
            let z = to_viewer;
            let mut x = viewer_y;
            x.cross(&to_viewer);
            let mut y = to_viewer;
            y.cross(&x);
            x.normalize();
            y.normalize();
            Matrix4([x.x, x.y, x.z, 0.0, y.x, y.y, y.z, 0.0, z.x, z.y, z.z, 0.0, 0.0, 0.0, 0.0, 1.0])
        } else {
            let mut n1 = axis;
            n1.cross(&to_viewer);
            let mut n2 = axis;
            n2.cross(&Vector3::Z_AXIS);
            let mut r = Rotation4::identity();
            r.set_vectors(&n2, &n1);
            let m3 = r.get_matrix();
            let m = m3.0;
            Matrix4([m[0], m[1], m[2], 0.0, m[3], m[4], m[5], 0.0, m[6], m[7], m[8], 0.0, 0.0, 0.0, 0.0, 1.0])
        }
    }

    /// LOD.getLevel with a range.
    fn lod_level(&self, sg: &Sg, n: NodeId, model_view: &Matrix4, count: usize) -> Option<usize> {
        if count == 0 {
            return None;
        }
        let range = sg.f64s(n, "range");
        if range.is_empty() {
            return Some(0);
        }
        let mut c = sg.vec3(n, "center", Vector3::ZERO);
        model_view.mult_vec_matrix3(&mut c);
        let distance = c.norm();
        let level = range.iter().position(|r| distance < *r).unwrap_or(range.len());
        Some(level.min(count - 1))
    }

    // --- CAMERA pass ------------------------------------------------------------------------

    fn camera(&self, sg: &Sg, n: NodeId, m: &Matrix4, bound: &mut Bound, depth: usize) {
        if depth > 256 {
            return;
        }
        let Some(ty) = sg.type_name(n) else { return };
        let inline = sg.in_inline(n);

        match ty {
            "Viewpoint" if bound.viewpoint.is_none() && !inline => bound.viewpoint = Some((n, *m)),
            "NavigationInfo" if bound.navigation_info.is_none() && !inline => bound.navigation_info = Some(n),
            "Background" | "TextureBackground" if bound.background.is_none() && !inline => bound.background = Some((n, *m)),
            "Fog" if !inline => {
                bound.fog_matrices.insert(n, *m);

                if bound.fog.is_none() {
                    bound.fog = Some(n);
                }
            }
            _ if GROUPS.contains(&ty) => {
                let mut mm = *m;
                if ty == "Transform" {
                    mm.mult_left(&self.transform_matrix(sg, n));
                }
                let kids = children_of(sg, n, ty);
                let kids = if ty == "LOD" { self.lod_level(sg, n, &mm, kids.len()).map(|l| vec![kids[l]]).unwrap_or_default() } else { kids };
                for c in kids {
                    self.camera(sg, c, &mm, bound, depth + 1);
                }
            }
            _ => {}
        }
    }

    // --- DISPLAY pass -----------------------------------------------------------------------

    fn display(&mut self, sg: &Sg, n: NodeId, mv: &Matrix4, depth: usize) {
        if depth > 256 {
            return;
        }
        let Some(ty) = sg.type_name(n) else { return };

        match ty {
            "Shape" => self.shape(sg, n, mv),
            "DirectionalLight" | "PointLight" | "SpotLight" => self.light(sg, n, ty, mv),
            "Inline" => {
                // Inline.getInternalScene: the loaded scene's root nodes, traversed in its runtime.
                if let Some(child) = sg.rt.inline_runtimes.get(&n) {
                    if sg.bool(n, "load", true) {
                        let csg = Sg::new(child);
                        for c in csg.roots() {
                            self.display(&csg, c, mv, depth + 1);
                        }
                    }
                }
            }
            _ if GROUPS.contains(&ty) => {
                let mut m = *mv;
                if ty == "Transform" {
                    m.mult_left(&self.transform_matrix(sg, n));
                } else if ty == "Billboard" {
                    let b = self.billboard_matrix(sg, n, &m);
                    m.mult_left(&b);
                }
                let kids = children_of(sg, n, ty);
                let kids = if ty == "LOD" { self.lod_level(sg, n, &m, kids.len()).map(|l| vec![kids[l]]).unwrap_or_default() } else { kids };

                // Lights first (X3DGroupingNode: displayNodes push, then visibleObjects).
                let (lights, rest): (Vec<NodeId>, Vec<NodeId>) =
                    kids.into_iter().partition(|c| matches!(sg.type_name(*c), Some("DirectionalLight" | "PointLight" | "SpotLight")));
                for c in lights.into_iter().chain(rest) {
                    self.display(sg, c, &m, depth + 1);
                }
            }
            // Nodes with no visual output.
            "WorldInfo" | "Viewpoint" | "NavigationInfo" | "Background" | "TextureBackground" | "Fog" | "TimeSensor" | "TouchSensor" | "PlaneSensor" | "SphereSensor"
            | "CylinderSensor" | "ProximitySensor" | "VisibilitySensor" | "Script" | "PositionInterpolator" | "OrientationInterpolator"
            | "ColorInterpolator" | "ScalarInterpolator" | "CoordinateInterpolator" | "NormalInterpolator" | "Sound" | "AudioClip"
            | "BooleanFilter" | "BooleanToggle" | "BooleanTrigger" | "BooleanSequencer" | "IntegerSequencer" | "IntegerTrigger"
            | "TimeTrigger" | "MetadataString" | "MetadataFloat" | "MetadataInteger" | "MetadataSet" | "MetadataDouble" | "MetadataBoolean" => {}
            // Not X3DChildNode: X3DGroupingNode.add ignores these in children.
            "Appearance" | "Material" | "ImageTexture" | "PixelTexture" | "MovieTexture" | "TextureTransform" | "Coordinate" | "Color"
            | "Normal" | "TextureCoordinate" | "FontStyle" | "Box" | "Sphere" | "Cone" | "Cylinder" | "IndexedFaceSet" | "IndexedLineSet"
            | "PointSet" | "ElevationGrid" | "Extrusion" | "Text" => {}
            other => {
                self.unsupported.insert(other.to_string());
            }
        }
    }

    fn light(&mut self, sg: &Sg, n: NodeId, ty: &str, mv: &Matrix4) {
        // X3DLightNode.set_on__: no push when off or intensity <= 0.
        let intensity = sg.f64(n, "intensity", 1.0).max(0.0);
        if !sg.bool(n, "on", true) || intensity <= 0.0 {
            return;
        }

        let color = sg.color(n, "color", [1.0, 1.0, 1.0]);
        let ambient_intensity = sg.f64(n, "ambientIntensity", 0.0).clamp(0.0, 1.0);

        match ty {
            "DirectionalLight" => {
                // DirectionalLightContainer.setGlobalVariables.
                let mut d = sg.vec3(n, "direction", Vector3::new(0.0, 0.0, -1.0));
                mv.mult_dir_matrix(&mut d);
                d.normalize();
                self.lights.push(LightParams {
                    kind: 1,
                    color,
                    intensity,
                    ambient_intensity,
                    direction: d,
                    location: Vector3::ZERO,
                    attenuation: [0.0; 3],
                    radius: -1.0,
                    cut_off_angle: 0.0,
                    beam_width: 0.0,
                    matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                });
            }
            _ => {
                // PointLightContainer / SpotLightContainer.setGlobalVariables.
                let mut inv = mv.submatrix();
                inv.inverse();
                let matrix = inv.0.map(|v| v as f32);
                let mut location = sg.vec3(n, "location", Vector3::ZERO);
                mv.mult_vec_matrix3(&mut location);
                let a = sg.vec3(n, "attenuation", Vector3::new(1.0, 0.0, 0.0));
                let mut direction = sg.vec3(n, "direction", Vector3::new(0.0, 0.0, -1.0));
                mv.mult_dir_matrix(&mut direction);
                direction.normalize();
                let spot = ty == "SpotLight";
                self.lights.push(LightParams {
                    kind: if spot { 3 } else { 2 },
                    color,
                    intensity,
                    ambient_intensity,
                    direction,
                    location,
                    attenuation: [a.x.max(0.0), a.y.max(0.0), a.z.max(0.0)],
                    radius: sg.f64(n, "radius", 100.0),
                    cut_off_angle: if spot { sg.f64(n, "cutOffAngle", 0.785398).clamp(0.0, std::f64::consts::FRAC_PI_2) } else { 0.0 },
                    beam_width: if spot { sg.f64(n, "beamWidth", 1.5708).clamp(0.0, std::f64::consts::FRAC_PI_2) } else { 0.0 },
                    matrix,
                });
            }
        }
    }

    fn white_texture(&mut self) -> Rc<TextureImage> {
        match self.cache.as_deref_mut() {
            Some(c) => c.white.get_or_insert_with(|| Rc::new(TextureImage::white())).clone(),
            None => Rc::new(TextureImage::white()),
        }
    }

    fn texture(&mut self, sg: &Sg, n: NodeId) -> Option<Rc<TextureImage>> {
        if let Some(t) = self.textures.get(&key(sg, n)) {
            return t.clone();
        }

        let persistent = self.cache.is_some();
        let (ckey, fp) = if persistent { ((sg.rt.uid, n), fingerprint(sg, n)) } else { ((0, 0), 0) };

        if let Some(c) = self.cache.as_deref_mut() {
            let frame = c.frame;

            if let Some(e) = c.textures.get_mut(&ckey) {
                if e.fingerprint == fp {
                    e.used = frame;
                    let t = e.value.clone();
                    self.textures.insert(key(sg, n), t.clone());
                    return t;
                }
            }
        }
        let ty = sg.type_name(n).unwrap_or("");
        let t = match ty {
            "ImageTexture" => {
                // X3DExecutionContext.getBaseURL: the world URL of the node's own scene (an
                // Inline's scene has the Inline's resolved URL).
                let base = sg.rt.scene.world_url.clone();
                texture::load_image_texture(sg, n, &base, self.root.fetcher.as_ref()).map(Rc::new)
            }
            "PixelTexture" => texture::pixel_texture(sg, n).map(Rc::new),
            other => {
                self.unsupported.insert(other.to_string());
                None
            }
        };
        self.textures.insert(key(sg, n), t.clone());

        if let Some(c) = self.cache.as_deref_mut() {
            let frame = c.frame;
            c.textures.insert(ckey, CacheEntry { fingerprint: fp, value: t.clone(), used: frame });
        }

        t
    }

    fn geometry(&mut self, sg: &Sg, n: NodeId) -> Option<Rc<Geometry>> {
        if let Some(g) = self.geometries.get(&key(sg, n)) {
            return g.clone();
        }
        let persistent = self.cache.is_some();
        let (ckey, fp) = if persistent { ((sg.rt.uid, n), fingerprint(sg, n)) } else { ((0, 0), 0) };

        if let Some(c) = self.cache.as_deref_mut() {
            let frame = c.frame;

            if let Some(e) = c.geometries.get_mut(&ckey) {
                if e.fingerprint == fp {
                    e.used = frame;
                    c.reused += 1;
                    let g = e.value.clone();
                    self.geometries.insert(key(sg, n), g.clone());
                    return g;
                }
            }
        }

        let g = self.build_geometry(sg, n).filter(|g| g.vertex_count() > 0).map(Rc::new);
        self.geometries.insert(key(sg, n), g.clone());

        if let Some(c) = self.cache.as_deref_mut() {
            let frame = c.frame;
            c.built += 1;
            c.geometries.insert(ckey, CacheEntry { fingerprint: fp, value: g.clone(), used: frame });
        }

        g
    }

    fn colors(&self, sg: &Sg, n: NodeId) -> Option<(Vec<f32>, usize)> {
        let c = sg.node(n, "color")?;
        match sg.type_name(c)? {
            "Color" => Some((sg.f32s(c, "color").to_vec(), 3)),
            "ColorRGBA" => Some((sg.f32s(c, "color").to_vec(), 4)),
            _ => None,
        }
    }

    fn coord(&self, sg: &Sg, n: NodeId) -> Option<Vec<f32>> {
        let c = sg.node(n, "coord")?;
        match sg.type_name(c)? {
            "Coordinate" | "CoordinateDouble" => Some(sg.f32s(c, "point").to_vec()),
            _ => None,
        }
    }

    fn build_geometry(&mut self, sg: &Sg, n: NodeId) -> Option<Geometry> {
        let ty = sg.type_name(n)?;

        let g = match ty {
            "IndexedFaceSet" => {
                let coord = self.coord(sg, n);
                let colors = self.colors(sg, n);
                let tex = sg.node(n, "texCoord").filter(|t| sg.type_name(*t) == Some("TextureCoordinate")).map(|t| sg.f32s(t, "point").to_vec());
                let normal = sg.node(n, "normal").filter(|t| sg.type_name(*t) == Some("Normal")).map(|t| sg.f32s(t, "vector").to_vec());
                let input = IfsInput {
                    coord: coord.as_deref(),
                    coord_index: sg.i32s(n, "coordIndex"),
                    tex_coord: tex.as_deref(),
                    tex_coord_index: sg.i32s(n, "texCoordIndex"),
                    normal: normal.as_deref(),
                    normal_index: sg.i32s(n, "normalIndex"),
                    color: colors.as_ref().map(|(v, c)| Colors { values: v, components: *c }),
                    color_index: sg.i32s(n, "colorIndex"),
                    ccw: sg.bool(n, "ccw", true),
                    solid: sg.bool(n, "solid", true),
                    convex: sg.bool(n, "convex", true),
                    crease_angle: sg.f64(n, "creaseAngle", 0.0),
                    color_per_vertex: sg.bool(n, "colorPerVertex", true),
                    normal_per_vertex: sg.bool(n, "normalPerVertex", true),
                };
                geometry::indexed_face_set(&input)
            }
            "IndexedLineSet" => {
                let coord = self.coord(sg, n);
                let colors = self.colors(sg, n);
                geometry::indexed_line_set(
                    coord.as_deref(),
                    sg.i32s(n, "coordIndex"),
                    colors.as_ref().map(|(v, c)| Colors { values: v, components: *c }),
                    sg.i32s(n, "colorIndex"),
                    sg.bool(n, "colorPerVertex", true),
                )
            }
            "PointSet" => {
                let coord = self.coord(sg, n);
                let colors = self.colors(sg, n);
                geometry::point_set(coord.as_deref(), colors.as_ref().map(|(v, c)| Colors { values: v, components: *c }))
            }
            "Box" => geometry::make_box(sg.vec3(n, "size", Vector3::new(2.0, 2.0, 2.0)), sg.bool(n, "solid", true)),
            "Sphere" => geometry::make_sphere(sg.f64(n, "radius", 1.0), sg.bool(n, "solid", true)),
            "Cone" => geometry::make_cone(
                sg.f64(n, "height", 2.0),
                sg.f64(n, "bottomRadius", 1.0),
                sg.bool(n, "side", true),
                sg.bool(n, "bottom", true),
                sg.bool(n, "solid", true),
            ),
            "Cylinder" => geometry::make_cylinder(
                sg.f64(n, "height", 2.0),
                sg.f64(n, "radius", 1.0),
                sg.bool(n, "side", true),
                sg.bool(n, "top", true),
                sg.bool(n, "bottom", true),
                sg.bool(n, "solid", true),
            ),
            "ElevationGrid" => {
                let colors = self.colors(sg, n);
                let tex = sg.node(n, "texCoord").filter(|t| sg.type_name(*t) == Some("TextureCoordinate")).map(|t| sg.f32s(t, "point").to_vec());
                let normal = sg.node(n, "normal").filter(|t| sg.type_name(*t) == Some("Normal")).map(|t| sg.f32s(t, "vector").to_vec());
                geometry::elevation_grid(&ElevationInput {
                    x_dimension: sg.i32(n, "xDimension", 0),
                    z_dimension: sg.i32(n, "zDimension", 0),
                    x_spacing: sg.f64(n, "xSpacing", 1.0),
                    z_spacing: sg.f64(n, "zSpacing", 1.0),
                    height: sg.f32s(n, "height"),
                    color: colors.as_ref().map(|(v, c)| Colors { values: v, components: *c }),
                    tex_coord: tex.as_deref(),
                    normal: normal.as_deref(),
                    ccw: sg.bool(n, "ccw", true),
                    solid: sg.bool(n, "solid", true),
                    crease_angle: sg.f64(n, "creaseAngle", 0.0),
                    color_per_vertex: sg.bool(n, "colorPerVertex", true),
                    normal_per_vertex: sg.bool(n, "normalPerVertex", true),
                })
            }
            "Text" => return text::text_geometry(sg, n, &mut self.unsupported),
            "Extrusion" => {
                let orientation = sg.f64s(n, "orientation");
                geometry::extrusion(&ExtrusionInput {
                    cross_section: sg.f32s(n, "crossSection"),
                    orientation: &orientation,
                    scale: sg.f32s(n, "scale"),
                    spine: sg.f32s(n, "spine"),
                    begin_cap: sg.bool(n, "beginCap", true),
                    end_cap: sg.bool(n, "endCap", true),
                    solid: sg.bool(n, "solid", true),
                    ccw: sg.bool(n, "ccw", true),
                    convex: sg.bool(n, "convex", true),
                    crease_angle: sg.f64(n, "creaseAngle", 0.0),
                })
            }
            other => {
                self.unsupported.insert(other.to_string());
                return None;
            }
        };

        Some(g)
    }

    fn shape(&mut self, sg: &Sg, n: NodeId, mv: &Matrix4) {
        let Some(gnode) = sg.node(n, "geometry") else { return };
        let Some(geometry) = self.geometry(sg, gnode) else { return };

        let appearance = sg.node(n, "appearance").filter(|a| sg.type_name(*a) == Some("Appearance"));
        let material_node = appearance.and_then(|a| sg.node(a, "material")).filter(|m| sg.type_name(*m) == Some("Material"));
        let texture_node = appearance.and_then(|a| sg.node(a, "texture"));
        let transform_node = appearance.and_then(|a| sg.node(a, "textureTransform")).filter(|t| sg.type_name(*t) == Some("TextureTransform"));

        if let Some(a) = appearance {
            if let Some(m) = sg.node(a, "material") {
                if sg.type_name(m) != Some("Material") {
                    self.unsupported.insert(sg.type_name(m).unwrap_or("?").to_string());
                }
            }
        }

        let has_normals = !geometry.normals.is_empty();

        let material = match material_node {
            Some(m) => {
                        MaterialParams {
                    shading: if has_normals { Shading::Gouraud } else { Shading::Unlit },
                    diffuse: sg.color(m, "diffuseColor", [0.8, 0.8, 0.8]),
                    ambient_intensity: sg.f64(m, "ambientIntensity", 0.2).clamp(0.0, 1.0),
                    specular: sg.color(m, "specularColor", [0.0, 0.0, 0.0]),
                    shininess: sg.f64(m, "shininess", 0.2).clamp(0.0, 1.0),
                    emissive: sg.color(m, "emissiveColor", [0.0, 0.0, 0.0]),
                    transparency: sg.f64(m, "transparency", 0.0).clamp(0.0, 1.0),
                }
            }
            // Browser default material: UnlitMaterial, emissiveColor 1 1 1.
            None => MaterialParams {
                shading: Shading::Unlit,
                diffuse: [0.8; 3],
                ambient_intensity: 0.2,
                specular: [0.0; 3],
                shininess: 0.2,
                emissive: [1.0, 1.0, 1.0],
                transparency: 0.0,
            },
        };

        let texture = texture_node.and_then(|t| self.texture(sg, t));
        // An X3DTextureNode that failed to load still binds X_ITE's 1×1 white default texture.
        let texture = match (texture_node, texture) {
            (Some(t), None) if matches!(sg.type_name(t), Some("ImageTexture" | "PixelTexture")) => Some(self.white_texture()),
            (_, t) => t,
        };

        let texture_matrix = match transform_node {
            Some(t) => texture_transform_matrix(sg, t),
            None => Matrix4::IDENTITY.0.map(|v| v as f32),
        };

        let transparent = material.transparency > 0.0 || texture.as_ref().is_some_and(|t| t.transparent);

        // addDisplayShape: distance = bbox center z in view space.
        let mut center = geometry.bbox_center();
        mv.mult_vec_matrix3(&mut center);

        self.items.push(DrawItem {
            geometry_node: gnode,
            geometry,
            model_view: *mv,
            material,
            texture,
            texture_matrix,
            transparent,
            distance: center.z,
        });
    }

    /// X3DBackgroundNode.textureBits: the faces whose texture loaded. Background loads its six
    /// private ImageTextures (backgroundTextureProperties); TextureBackground uses its
    /// X3DTexture2DNode fields.
    fn background_faces(&mut self, sg: &Sg, b: NodeId) -> Vec<BackgroundFace> {
        let mut faces = vec![];
        let textured = sg.type_name(b) == Some("TextureBackground");
        let names = ["front", "back", "left", "right", "top", "bottom"];

        for (i, name) in names.iter().enumerate() {
            let texture = if textured {
                match sg.node(b, &format!("{name}Texture")) {
                    Some(t) => match sg.type_name(t) {
                        Some("ImageTexture" | "PixelTexture") => self.texture(sg, t),
                        Some(other) => {
                            self.unsupported.insert(other.to_string());
                            None
                        }
                        None => None,
                    },
                    None => None,
                }
            } else {
                let ckey = (sg.rt.uid, b, i);
                let fp = if self.cache.is_some() { fingerprint(sg, b) } else { 0 };
                let hit = self.cache.as_deref_mut().and_then(|c| {
                    let frame = c.frame;
                    c.faces.get_mut(&ckey).filter(|e| e.fingerprint == fp).map(|e| {
                        e.used = frame;
                        e.value.clone()
                    })
                });

                match hit {
                    Some(t) => t,
                    None => {
                        let urls = sg.strings(b, &format!("{name}Url"));
                        let t = texture::load_background_texture(&urls, &sg.rt.scene.world_url, self.root.fetcher.as_ref()).map(Rc::new);

                        if let Some(c) = self.cache.as_deref_mut() {
                            let frame = c.frame;
                            c.faces.insert(ckey, CacheEntry { fingerprint: fp, value: t.clone(), used: frame });
                        }

                        t
                    }
                }
            };

            if let Some(texture) = texture {
                faces.push(BackgroundFace { vertices: FACE_VERTICES[i], texture });
            }
        }

        faces
    }

    pub fn build(mut self, width: u32, height: u32) -> Frame {
        let root_sg = Sg::new(self.root);
        let sg = &root_sg;
        let roots = sg.roots();

        // CAMERA pass.
        let mut bound = Bound { viewpoint: None, navigation_info: None, background: None, fog: None, fog_matrices: HashMap::new() };
        for r in &roots {
            self.camera(sg, *r, &Matrix4::IDENTITY, &mut bound, 0);
        }

        if let Some(o) = &self.camera {
            bound.viewpoint = o.viewpoint.map(|v| (v, Matrix4::IDENTITY));
            bound.navigation_info = o.navigation_info;
            bound.background = o.background;
            bound.fog = o.fog;
        }

        // NavigationInfo.set_type__: the active viewer.
        let viewer = match bound.navigation_info {
            Some(ni) => navigation_viewer(&sg.strings(ni, "type")),
            None => "EXAMINE".to_string(),
        };

        // Viewpoint (default: position 0 0 10, fieldOfView π/4).
        let (position, orientation, fov, vp_model) = match bound.viewpoint {
            Some((v, m)) => {
                let fov = sg.f64(v, "fieldOfView", std::f64::consts::FRAC_PI_4);
                let fov = if fov > 0.0 && fov < std::f64::consts::PI { fov } else { std::f64::consts::FRAC_PI_4 };
                (sg.vec3(v, "position", Vector3::new(0.0, 0.0, 10.0)), sg.rotation(v, "orientation"), fov, m)
            }
            None => (Vector3::new(0.0, 0.0, 10.0), Rotation4::identity(), std::f64::consts::FRAC_PI_4, Matrix4::IDENTITY),
        };

        // ExamineViewer.set_activeViewpoint__: with StraightenHorizon (default TRUE) the EXAMINE
        // viewer sets an orientation offset that straightens the bound viewpoint's horizon.
        let orientation = if viewer == "EXAMINE" { straighten(&orientation, &Vector3::Y_AXIS) } else { orientation };

        // X3DViewpointNode.update: cameraSpaceMatrix = T(position) R(orientation) × modelMatrix.
        let mut camera_space = Matrix4::IDENTITY;
        camera_space.set_transform(Some(&position), Some(&orientation), Some(&Vector3::ONE), Some(&Rotation4::identity()), None);
        camera_space.mult_right(&vp_model);
        let mut view = camera_space;
        view.inverse();

        // NavigationInfo: near = avatarSize [0] / 2 (0.125 if 0), far = visibilityLimit or 1e5.
        let (headlight, near, far) = match bound.navigation_info {
            Some(ni) => {
                let avatar = sg.f64s(ni, "avatarSize");
                let radius = avatar.first().copied().unwrap_or(0.25).max(0.0);
                let near = if radius == 0.0 { 0.125 } else { radius / 2.0 };
                let limit = sg.f64(ni, "visibilityLimit", 0.0).max(0.0);
                (sg.bool(ni, "headlight", true), near, if limit != 0.0 { limit } else { far_max() })
            }
            None => (true, 0.125, far_max()),
        };

        let projection = perspective(fov, near, far, width as f64, height as f64);

        let (view, projection, headlight, _far) = match &self.camera {
            Some(o) => {
                let mut view = o.camera_space;
                view.inverse();
                (view, perspective(o.fov, o.near, o.far, width as f64, height as f64), o.headlight, o.far)
            }
            None => (view, projection, headlight, far),
        };

        // DISPLAY pass. NavigationInfo.enable pushes the headlight first.
        if headlight {
            self.lights.push(LightParams {
                kind: 1,
                color: [1.0, 1.0, 1.0],
                intensity: 1.0,
                ambient_intensity: 0.0,
                direction: Vector3::new(0.0, 0.0, -1.0),
                location: Vector3::ZERO,
                attenuation: [0.0; 3],
                radius: -1.0,
                cut_off_angle: 0.0,
                beam_width: 0.0,
                matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            });
        }

        for r in &roots {
            self.display(sg, *r, &view, 0);
        }

        // X3DRenderObject.setGlobalFog: modelViewMatrix = fog modelMatrix × viewMatrix, inverted 3×3.
        let fog = bound.fog.and_then(|f| fog_params(sg, f, bound.fog_matrices.get(&f), &view));

        let faces = bound.background.map(|(b, _)| self.background_faces(sg, b)).unwrap_or_default();
        let background = bound.background.and_then(|(b, m)| background_params(sg, b, &m, &view, far_max(), faces));

        if let Some(c) = self.cache.as_deref_mut() {
            c.end_frame();
        }

        Frame {
            projection,
            view,
            items: self.items,
            lights: self.lights,
            background,
            fog,
            unsupported: self.unsupported,
            viewpoint: bound.viewpoint.map(|v| v.0),
        }
    }
}

/// The arrays X_ITE would upload for a geometry node (X3DGeometryNode after rebuild), for tests
/// and tools. Returns None for unsupported node types.
pub fn geometry_of(rt: &x3d_runtime::Runtime, n: NodeId) -> Option<Geometry> {
    let sg = Sg::new(rt);
    let n = sg.inner(n)?;
    Builder::new(rt).build_geometry(&sg, n)
}

/// NavigationInfo.set_type__: first recognized type, EXAMINE by default.
fn navigation_viewer(types: &[String]) -> String {
    for t in types {
        match t.as_str() {
            "EXAMINE" | "WALK" | "FLY" | "LOOKAT" | "PLANE" | "NONE" => return t.clone(),
            "EXPLORE" => return "EXAMINE".into(),
            "PLANE_create3000.github.io" | "PLANE_create3000.de" => return "PLANE".into(),
            _ => continue,
        }
    }
    "EXAMINE".into()
}

/// Rotation4.straighten (upVector).
pub fn straighten(r: &Rotation4, up: &Vector3) -> Rotation4 {
    let mut r = *r;
    let mut up_normal = *up;
    up_normal.normalize();
    let mut local_x = Vector3::X_AXIS;
    local_x.negate();
    r.mult_vec_rot(&mut local_x);
    let mut local_z = Vector3::Z_AXIS;
    r.mult_vec_rot(&mut local_z);

    if local_z.dot(&up_normal).abs() >= 1.0 {
        return r;
    }

    let mut new_x = local_z;
    new_x.cross(&up_normal).normalize();

    if new_x.dot(&local_x) <= -1.0 {
        let rot = Rotation4::new(0.0, 0.0, 1.0, std::f64::consts::PI);
        r.mult_left(&rot);
    } else {
        let mut rot = Rotation4::identity();
        rot.set_vectors(&local_x, &new_x);
        r.mult_right(&rot);
    }

    r
}

static LOGARITHMIC_DEPTH_BUFFER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// X3D `LogarithmicDepthBuffer` rendering property (a browser-wide property in X_ITE, so a
/// process-wide switch here). Set through `gpu::Renderer::set_depth_mode`.
pub fn set_logarithmic_depth_buffer(on: bool) {
    LOGARITHMIC_DEPTH_BUFFER.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn logarithmic_depth_buffer() -> bool {
    LOGARITHMIC_DEPTH_BUFFER.load(std::sync::atomic::Ordering::Relaxed)
}

/// X3DViewpointNode.getMaxFarValue: 1e10 with the LogarithmicDepthBuffer rendering property, else 1e5.
pub fn far_max() -> f64 {
    if logarithmic_depth_buffer() { 1e10 } else { 1e5 }
}

/// Camera.perspective → Camera.frustum (column-major array, X_ITE layout).
pub fn perspective(fov: f64, near: f64, far: f64, width: f64, height: f64) -> Matrix4 {
    let ratio = x3d_fields::jsmath::tan(fov / 2.0) * near;

    let (l, r, b, t) = if width > height {
        let aspect = width * ratio / height;
        (-aspect, aspect, -ratio, ratio)
    } else {
        let aspect = height * ratio / width;
        (-ratio, ratio, -aspect, aspect)
    };

    let (r_l, t_b, f_n, n_2) = (r - l, t - b, far - near, 2.0 * near);
    let a = (r + l) / r_l;
    let bb = (t + b) / t_b;
    let c = -(far + near) / f_n;
    let d = -(n_2 * far) / f_n;
    let e = n_2 / r_l;
    let f = n_2 / t_b;

    Matrix4([e, 0.0, 0.0, 0.0, 0.0, f, 0.0, 0.0, a, bb, c, -1.0, 0.0, 0.0, d, 0.0])
}

/// TextureTransform.eventsProcessed.
pub fn texture_transform_matrix(sg: &Sg, n: NodeId) -> [f32; 16] {
    let translation = sg.vec2(n, "translation", Vector2::ZERO);
    let rotation = sg.f64(n, "rotation", 0.0);
    let scale = sg.vec2(n, "scale", Vector2::ONE);
    let center = sg.vec2(n, "center", Vector2::ZERO);
    let mut m3 = Matrix3([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);

    if !center.equals(&Vector2::ZERO) {
        let mut c = center;
        c.negate();
        m3.translate(&c);
    }
    if !scale.equals(&Vector2::ONE) {
        m3.scale(&scale);
    }
    if rotation != 0.0 {
        m3.rotate(rotation);
    }
    if !center.equals(&Vector2::ZERO) {
        m3.translate(&center);
    }
    if !translation.equals(&Vector2::ZERO) {
        m3.translate(&translation);
    }

    let m = m3.0;
    let mut m4 = Matrix4::IDENTITY.0;
    m4[0] = m[0];
    m4[1] = m[1];
    m4[4] = m[3];
    m4[5] = m[4];
    m4[12] = m[6];
    m4[13] = m[7];
    m4.map(|v| v as f32)
}

/// Algorithm.upperBound (array, first, last, value): first index with array [i] > value.
fn upper_bound(array: &[f64], value: f64) -> usize {
    array.iter().position(|a| *a > value).unwrap_or(array.len())
}

/// X3DBackgroundNode.build + display matrices.
fn background_params(sg: &Sg, n: NodeId, model: &Matrix4, view: &Matrix4, far: f64, faces: Vec<BackgroundFace>) -> Option<BackgroundParams> {
    // drawSphere returns early for transparency 1; drawCube still draws the loaded textures.
    let transparency = sg.f64(n, "transparency", 0.0).clamp(0.0, 1.0);
    if transparency >= 1.0 && faces.is_empty() {
        return None;
    }

    let sky_color = sg.f32s(n, "skyColor").chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect::<Vec<_>>();
    let ground_color = sg.f32s(n, "groundColor").chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect::<Vec<_>>();
    let sky_angle = sg.f64s(n, "skyAngle");
    let ground_angle = sg.f64s(n, "groundAngle");

    let mut colors: Vec<f32> = vec![];
    let mut sphere: Vec<f32> = vec![];
    let pi = std::f64::consts::PI;

    if ground_color.is_empty() && sky_color.len() == 1 {
        #[rustfmt::skip]
        let cube: [f32; 144] = [
             1.,  1., -1., 1., -1.,  1., -1., 1., -1., -1., -1., 1.,
             1.,  1., -1., 1., -1., -1., -1., 1.,  1., -1., -1., 1.,
            -1.,  1.,  1., 1.,  1.,  1.,  1., 1., -1., -1.,  1., 1.,
            -1., -1.,  1., 1.,  1.,  1.,  1., 1.,  1., -1.,  1., 1.,
            -1.,  1., -1., 1., -1.,  1.,  1., 1., -1., -1.,  1., 1.,
            -1.,  1., -1., 1., -1., -1.,  1., 1., -1., -1., -1., 1.,
             1.,  1.,  1., 1.,  1.,  1., -1., 1.,  1., -1.,  1., 1.,
             1., -1.,  1., 1.,  1.,  1., -1., 1.,  1., -1., -1., 1.,
             1.,  1.,  1., 1., -1.,  1.,  1., 1., -1.,  1., -1., 1.,
             1.,  1.,  1., 1., -1.,  1., -1., 1.,  1.,  1., -1., 1.,
            -1., -1.,  1., 1.,  1., -1.,  1., 1., -1., -1., -1., 1.,
            -1., -1., -1., 1.,  1., -1.,  1., 1.,  1., -1., -1., 1.,
        ];
        sphere.extend_from_slice(&cube);
        let c = sky_color[0];
        for _ in 0..36 {
            colors.extend_from_slice(&[c[0], c[1], c[2], 1.0]);
        }
    } else {
        let mut build = |v_angle: &[f64], angle: &[f64], color: &[[f32; 3]], bottom: bool| {
            const U_DIMENSION: usize = 20;
            let v_angle_max = if bottom { pi / 2.0 } else { pi };
            let get_color = |theta: f64| color[upper_bound(angle, theta).min(color.len() - 1)];

            for v in 0..v_angle.len() - 1 {
                let mut theta1 = v_angle[v].clamp(0.0, v_angle_max);
                let mut theta2 = v_angle[v + 1].clamp(0.0, v_angle_max);
                if bottom {
                    theta1 = pi - theta1;
                    theta2 = pi - theta2;
                }
                let z1 = (x3d_fields::jsmath::cos(theta1), x3d_fields::jsmath::sin(theta1));
                let z2 = (x3d_fields::jsmath::cos(theta2), x3d_fields::jsmath::sin(theta2));
                let c1 = get_color(v_angle[v]);
                let c2 = get_color(v_angle[v + 1]);

                for u in 0..U_DIMENSION {
                    let u1 = if u < U_DIMENSION - 1 { u + 1 } else { 0 };
                    let polar = |m: f64, a: f64| (m * x3d_fields::jsmath::cos(a), m * x3d_fields::jsmath::sin(a));
                    let phi = 2.0 * pi * (u as f64 / U_DIMENSION as f64);
                    let y1 = polar(-z1.1, phi);
                    let y2 = polar(-z2.1, phi);
                    let phi = 2.0 * pi * (u1 as f64 / U_DIMENSION as f64);
                    let y3 = polar(-z2.1, phi);
                    let y4 = polar(-z1.1, phi);

                    for c in [c1, c2, c2, c1, c1, c2] {
                        colors.extend_from_slice(&[c[0], c[1], c[2], 1.0]);
                    }
                    for p in [(y1, z1.0), (y3, z2.0), (y2, z2.0), (y1, z1.0), (y4, z1.0), (y3, z2.0)] {
                        sphere.extend_from_slice(&[(p.0).1 as f32, p.1 as f32, (p.0).0 as f32, 1.0]);
                    }
                }
            }
        };

        if sky_color.len() > sky_angle.len() {
            let mut v_angle = sky_angle.clone();
            if v_angle.is_empty() || v_angle[0] > 0.0 {
                v_angle.insert(0, 0.0);
            }
            if *v_angle.last().unwrap() < pi {
                v_angle.push(pi);
            }
            if v_angle.len() == 2 {
                let mid = (v_angle[0] + v_angle[1]) / 2.0;
                v_angle.insert(1, mid);
            }
            build(&v_angle, &sky_angle, &sky_color, false);
        }

        if ground_color.len() > ground_angle.len() {
            let mut v_angle: Vec<f64> = ground_angle.iter().rev().copied().collect();
            if v_angle.is_empty() || v_angle[0] < pi / 2.0 {
                v_angle.insert(0, pi / 2.0);
            }
            if *v_angle.last().unwrap() > 0.0 {
                v_angle.push(0.0);
            }
            build(&v_angle, &ground_angle, &ground_color, true);
        }
    }

    // display: rotation of (modelMatrix × viewMatrix), scaled by the max far value.
    let mut m = *model;
    m.mult_right(view);
    let rotation = rotation_of(&m);
    let mut mv = Matrix4::IDENTITY;
    mv.rotate(&rotation);
    mv.scale(&Vector3::new(far, far, far));

    if transparency >= 1.0 {
        colors.clear();
        sphere.clear();
    }

    Some(BackgroundParams { colors, sphere, transparency, model_view: mv, faces })
}

/// X3DBackgroundNode.transferRectangle: the six face quads (front, back, left, right, top, bottom).
#[rustfmt::skip]
const FACE_VERTICES: [[f32; 24]; 6] = [
    [ 1.,  1., -1., 1., -1.,  1., -1., 1., -1., -1., -1., 1.,  1.,  1., -1., 1., -1., -1., -1., 1.,  1., -1., -1., 1.],
    [-1.,  1.,  1., 1.,  1.,  1.,  1., 1.,  1., -1.,  1., 1., -1.,  1.,  1., 1.,  1., -1.,  1., 1., -1., -1.,  1., 1.],
    [-1.,  1., -1., 1., -1.,  1.,  1., 1., -1., -1.,  1., 1., -1.,  1., -1., 1., -1., -1.,  1., 1., -1., -1., -1., 1.],
    [ 1.,  1.,  1., 1.,  1.,  1., -1., 1.,  1., -1., -1., 1.,  1.,  1.,  1., 1.,  1., -1., -1., 1.,  1., -1.,  1., 1.],
    [ 1.,  1.,  1., 1., -1.,  1.,  1., 1., -1.,  1., -1., 1.,  1.,  1.,  1., 1., -1.,  1., -1., 1.,  1.,  1., -1., 1.],
    [ 1., -1., -1., 1., -1., -1., -1., 1., -1., -1.,  1., 1.,  1., -1., -1., 1., -1., -1.,  1., 1.,  1., -1.,  1., 1.],
];

/// X3DBackgroundNode.drawCube texture coordinates for every face.
pub const FACE_TEX_COORDS: [f32; 24] = [1., 1., 0., 1., 0., 1., 0., 1., 0., 0., 0., 1., 1., 1., 0., 1., 0., 0., 0., 1., 1., 0., 0., 1.];

/// FogContainer.set / X3DFogObject: None when the fog does not draw (fogType 0).
fn fog_params(sg: &Sg, f: NodeId, model: Option<&Matrix4>, view: &Matrix4) -> Option<FogParams> {
    let type_field = match sg.value(f, "fogType") {
        Some(x3d_fields::FieldValue::SFString(s)) => s.clone(),
        _ => "LINEAR".to_string(),
    };
    let (fog_type, start, range) = fog::fog_type(&type_field, sg.f64(f, "visibilityStart", 0.0), sg.f64(f, "visibilityRange", 0.0), false);

    if fog_type == 0 {
        return None;
    }

    let mut mv = model.copied().unwrap_or(Matrix4::IDENTITY);
    mv.mult_right(view);

    let mut inv = mv.submatrix();
    inv.inverse();

    let c = sg.color(f, "color", [1.0, 1.0, 1.0]);

    Some(FogParams { fog_type, color: [c[0] as f32, c[1] as f32, c[2] as f32], visibility_start: start as f32, visibility_range: range as f32, matrix: inv.0.map(|v| v as f32) })
}

/// Matrix4.getTransform (null, rotation): the rotation part of a matrix (polar decomposition
/// via X_ITE's Matrix3 factor). Uses the normalized rows for an orthogonal-ish matrix.
fn rotation_of(m: &Matrix4) -> Rotation4 {
    let mut r = Rotation4::identity();
    let mut m3 = m.submatrix();
    // Normalize rows to remove scale (exact for the rigid view matrices used here).
    for row in 0..3 {
        let v = Vector3::new(m3.0[row * 3], m3.0[row * 3 + 1], m3.0[row * 3 + 2]);
        let n = v.norm();
        if n != 0.0 {
            for k in 0..3 {
                m3.0[row * 3 + k] /= n;
            }
        }
    }
    r.set_matrix(&m3);
    r
}
