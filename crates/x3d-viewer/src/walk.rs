//! Scene walk for the interactive viewer: one traversal that collects what X_ITE gathers in its
//! CAMERA, POINTER, COLLISION and DISPLAY passes (X3DLayerNode.camera / pointer / collision,
//! X3DGroupingNode.traverse, X3DShapeNode.traverse, Billboard / LOD / Switch / Transform /
//! Anchor / Collision / ProximitySensor / VisibilitySensor traverse).
//!
//! The renderer (x3d-render) has its own traversal for drawing; this one produces the data the
//! viewer logic needs: bindable lists with model matrices, shape instances with their enclosing
//! pointing device sensors, collision shapes, and the environmental sensors.

use std::rc::Rc;
use x3d_fields::NodeId;
use x3d_fields::math::{Matrix4, Rotation4, Vector3};
use x3d_render::layer::{self, LayerKey, LayoutCamera, LayoutNode, LayoutStack};
use x3d_render::sg::Sg;
use x3d_runtime::Runtime;

/// Path of Inline nodes from the world's runtime to the runtime holding a node. Inline scenes
/// are contexts of the world's runtime now (one browser), so the path is always empty; it stays
/// in the types the viewer keys its caches with.
pub type RtPath = Rc<Vec<NodeId>>;

/// A node together with the runtime that owns it.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeRef {
    pub path: RtPath,
    pub node: NodeId,
}

/// A pointing device sensor in scope of a shape, with the model-view matrix of the grouping
/// node it was pushed from (X3DPointingDeviceSensorNode.push).
#[derive(Clone, Debug)]
pub struct SensorRef {
    pub node: NodeRef,
    pub model_view: Matrix4,
}

#[derive(Clone, Debug)]
pub struct ShapeInst {
    pub shape: NodeRef,
    pub geometry: NodeId,
    /// X3DRenderObject.getModelViewMatrix () for this shape.
    pub model_view: Matrix4,
    /// The shape's own model-view matrix and, for an InstancedShape, the instance matrix that
    /// `model_view` has folded in (the pointing shader applies it to the vertex first).
    pub shape_model_view: Matrix4,
    pub instance: Option<Matrix4>,
    /// Every instance matrix of the InstancedShape (the shape's bbox is their union).
    pub all_instances: Option<Rc<Vec<Matrix4>>>,
    /// The shape's explicit `bboxSize` / `bboxCenter` as min and max (X3DShapeNode.getBBox uses them
    /// instead of the geometry's box unless bboxSize is -1 -1 -1).
    pub explicit_bbox: Option<(Vector3, Vector3)>,
    /// Pointing device sensors that apply (X3DGroupingNode pointingDeviceSensorNodes of every
    /// enclosing group), outermost first.
    pub sensors: Vec<SensorRef>,
    /// Inside no disabled Collision node (Collision.set_collisionObjects__).
    pub collidable: bool,
    /// The enabled Collision nodes around the shape in the COLLISION traversal (renderContext .collisions),
    /// outermost first.
    pub collisions: Vec<NodeId>,
    /// Anchor nodes of the sensors list (their hidden TouchSensor), for activation reports.
    pub anchors: Vec<NodeRef>,
    /// renderContext .clipPlanes: the enabled ClipPlanes in scope, in the space of the walk's
    /// start matrix (view space for POINTER / COLLISION / DISPLAY), outermost first.
    pub clip_planes: Vec<[f64; 4]>,
    /// The viewport rectangle the shape is drawn in (its layer's, or that of the Viewport nodes
    /// around it): the pointer must be inside, and the hit ray goes through it.
    pub viewport: [f64; 4],
    /// renderObject.getHAnimNode (): the skin tables and joint matrices of the HAnimHumanoid or
    /// HAnimSegment around the shape (the pointing shader skins it; collision does not).
    pub skin: Option<Rc<(x3d_render::hanim::SkinTables, Vec<f32>)>>,
    /// Appearance.getStyleProperties (geometryType) with ContentScale 1 (PointProperties for points, an applied
    /// LineProperties for lines): the pointing pass draws points and lines with it.
    pub style: Option<x3d_render::traverse::StyleParams>,
    /// X3D_TEXTURE and X3D_MATERIAL_TEXTURES of the pointing shader (appearance and material texture nodes).
    pub textures: (bool, bool),
}

/// The layer a walk covers (X3DLayerNode.traverse): its children, its viewport rectangle, and the
/// bound viewpoint and navigation info its Layout nodes read.
#[derive(Clone, Copy, Debug)]
pub struct LayerWalk {
    pub key: LayerKey,
    pub viewport: [f64; 4],
    pub layout: LayoutCamera,
}

impl LayerWalk {
    /// Layer 0 over the whole `width` × `height` canvas with the default viewpoint.
    pub fn root(width: f64, height: f64) -> LayerWalk {
        LayerWalk { key: None, viewport: [0.0, 0.0, width, height], layout: LayoutCamera { ortho: None, fov: std::f64::consts::FRAC_PI_4, near: 0.125, content_scale: 1.0 } }
    }
}

#[derive(Clone, Debug)]
pub struct EnvInst {
    pub node: NodeRef,
    pub model_view: Matrix4,
}

#[derive(Default)]
pub struct Scan {
    pub viewpoints: Vec<(NodeRef, Matrix4)>,
    pub navigation_infos: Vec<NodeRef>,
    pub backgrounds: Vec<(NodeRef, Matrix4)>,
    pub fogs: Vec<NodeRef>,
    /// CAMERA: X3DViewpointNode.traverse computes the descriptions of each viewpoint reached.
    pub viewpoint_descriptions: Vec<(NodeId, Vec<String>)>,
    pub shapes: Vec<ShapeInst>,
    /// The shapes of enabled Collision nodes' `proxy` nodes: collision objects only (Collision.set_collisionObjects__),
    /// never drawn, picked or measured.
    pub proxy_shapes: Vec<ShapeInst>,
    pub proximity: Vec<EnvInst>,
    pub visibility: Vec<EnvInst>,
    /// Enabled Collision nodes seen (browser.getCollisionCount).
    pub collisions: usize,
    /// PICKING: TransformSensor children with the model matrix of their group (TransformSensor.traverse).
    pub transform_sensors: Vec<EnvInst>,
    /// PICKING: the model matrices of the requested target objects (TransformSensor.collect).
    pub pick_targets: Vec<(NodeId, Matrix4)>,
    /// PICKING: the model matrices of the live pick sensors (X3DPickSensorNode.traverse).
    pub pick_sensor_matrices: Vec<(NodeId, Matrix4)>,
    /// PICKING: X3DPickSensorNode.collect: (sensor, geometry, model matrix) of each shape below one
    /// of the sensor's pickTarget nodes, in traversal order.
    pub pick_collected: Vec<(NodeId, NodeId, Matrix4)>,
    /// DISPLAY: SpatialSound and ListenerPointSource with their model-view matrix (their traverse).
    pub sounds: Vec<EnvInst>,
}

const GROUPS: &[&str] =
    &["Group", "Transform", "Anchor", "Collision", "Billboard", "Switch", "LOD", "GeoLocation", "GeoTransform", "GeoLOD", "CADAssembly", "CADLayer", "CADPart", "CADFace", "PickableGroup", "GroupAnnotation", "ScreenGroup", "HAnimHumanoid", "HAnimJoint", "HAnimSegment", "HAnimSite"];
/// X3DCast (X3DChildNode, node) for the node types that can be a Collision's `proxy`: everything but the
/// geometry, appearance, material, texture and property nodes.
fn is_child_node(sg: &Sg, n: NodeId) -> bool {
    let Some(ty) = sg.type_name(n) else { return false };
    !(matches!(
        ty,
        "Appearance" | "Material" | "TwoSidedMaterial" | "UnlitMaterial" | "PhysicalMaterial" | "SpecularGlossinessMaterial" | "Coordinate" | "Normal" | "Color" | "ColorRGBA" | "TextureCoordinate" | "ImageTexture" | "PixelTexture"
            | "MovieTexture" | "TextureTransform" | "Box" | "Sphere" | "Cone" | "Cylinder" | "IndexedFaceSet" | "IndexedLineSet" | "PointSet" | "LineSet" | "ElevationGrid" | "Extrusion" | "Text" | "FontStyle" | "Rectangle2D"
            | "Circle2D" | "Arc2D" | "Disk2D" | "Polyline2D" | "Polypoint2D" | "TriangleSet2D" | "TriangleSet" | "IndexedTriangleSet" | "FillProperties" | "LineProperties" | "PointProperties"
    ))
}

const SENSORS: &[&str] = &["TouchSensor", "GeoTouchSensor", "PlaneSensor", "SphereSensor", "CylinderSensor"];

/// Resolve a runtime path from the root runtime (all nodes live in the root runtime).
pub fn rt_at<'a>(root: &'a Runtime, path: &[NodeId]) -> Option<&'a Runtime> {
    path.is_empty().then_some(root)
}

pub fn rt_at_mut<'a>(root: &'a mut Runtime, path: &[NodeId]) -> Option<&'a mut Runtime> {
    path.is_empty().then_some(root)
}

/// Every runtime path: only the root.
pub fn all_paths(_root: &Runtime) -> Vec<Vec<NodeId>> {
    vec![vec![]]
}

/// Transform: Matrix4.setTransform (translation, rotation, scale, scaleOrientation, center).
pub fn transform_matrix(sg: &Sg, n: NodeId) -> Matrix4 {
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
pub fn billboard_matrix(sg: &Sg, n: NodeId, model_view: &Matrix4) -> Matrix4 {
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
        let m = r.get_matrix().0;
        Matrix4([m[0], m[1], m[2], 0.0, m[3], m[4], m[5], 0.0, m[6], m[7], m[8], 0.0, 0.0, 0.0, 0.0, 1.0])
    }
}

/// LOD state the DISPLAY traversal changes (LOD.changeLevel): the levels set during this walk and
/// each LOD's frame rate average, which persists between frames.
#[derive(Default)]
pub struct LodPass {
    /// level_changed set during this walk, by node (a USEd LOD changes it once per visit).
    pub levels: std::collections::HashMap<(Vec<NodeId>, NodeId), i32>,
    /// LOD [frameRate] (starts at 60), by node.
    pub frame_rates: std::collections::HashMap<(Vec<NodeId>, NodeId), f64>,
    /// browser .currentFrameRate.
    pub current_frame_rate: f64,
    /// The level_changed events to send, in traversal order.
    pub changes: Vec<(NodeRef, i32)>,
    /// GeoLOD.changeLevel calls with the new level, in traversal order.
    pub geo_changes: Vec<(NodeRef, i32)>,
    /// The GeoLODs this DISPLAY walk visited.
    pub geo_visited: Vec<NodeRef>,
    /// StaticGroups whose optimized group exists. StaticGroup.traverse builds it on the first
    /// visit and the build finishes after that frame, so the first frame still traverses the
    /// plain Group (all children).
    pub static_done: std::collections::HashSet<(Vec<NodeId>, NodeId)>,
    /// StaticGroups first reached during this walk.
    pub static_reached: Vec<(Vec<NodeId>, NodeId)>,
    /// GeneratedCubeMapTextures with update NEXT_FRAME_ONLY that this frame renders (their shapes'
    /// Appearance.traverse): renderTexture sets update to NONE.
    pub generated_once: Vec<NodeRef>,
    /// RenderedTextures reached by this frame's DISPLAY traversal (Appearance.traverse), in order;
    /// RenderedTexture.traverse decides which render.
    pub rendered_reached: Vec<NodeRef>,
}

/// LOD.getLevel.
fn lod_get_level(sg: &Sg, n: NodeId, model_view: &Matrix4, frame_rate: &mut f64, current_frame_rate: f64) -> i32 {
    const FRAMES: f64 = 180.0;
    const FRAME_RATE_MIN: f64 = 20.0;
    const FRAME_RATE_MAX: f64 = 55.0;

    let range = sg.f64s(n, "range");

    if range.is_empty() {
        *frame_rate = ((FRAMES - 1.0) * *frame_rate + current_frame_rate) / FRAMES;

        let size = sg.raw_nodes(n, "children").len() as i32;

        return match size {
            0 => -1,
            1 => 0,
            2 => (*frame_rate > FRAME_RATE_MAX) as i32,
            _ => {
                let fraction = 1.0 - x3d_fields::jsmath::clamp((*frame_rate - FRAME_RATE_MIN) / (FRAME_RATE_MAX - FRAME_RATE_MIN), 0.0, 1.0);
                ((fraction * size as f64).floor() as i32).min(size - 1)
            }
        };
    }

    // modelViewMatrix .translate (center) .origin .norm (); Algorithm .upperBound (range, distance).
    let mut c = sg.vec3(n, "center", Vector3::ZERO);
    model_view.mult_vec_matrix3(&mut c);
    let distance = c.norm();

    range.iter().position(|r| distance < *r).unwrap_or(range.len()) as i32
}

/// LOD.set_children__: the one child, `children [min (level_changed, length - 1)]`.
fn lod_child(sg: &Sg, n: NodeId, level: i32) -> Vec<NodeId> {
    let raw = sg.raw_nodes(n, "children");
    let level = level.min(raw.len() as i32 - 1);

    if level >= 0 { raw[level as usize].and_then(|c| sg.inner(c)).into_iter().collect() } else { vec![] }
}

fn children_of(sg: &Sg, n: NodeId, ty: &str) -> Vec<NodeId> {
    match ty {
        "Switch" => {
            let choice = sg.i32(n, "whichChoice", -1);
            let mut raw = sg.raw_nodes(n, "children");
            if raw.is_empty() {
                raw = sg.raw_nodes(n, "choice");
            }
            if choice >= 0 && (choice as usize) < raw.len() { raw[choice as usize].and_then(|c| sg.inner(c)).into_iter().collect() } else { vec![] }
        }
        "CADFace" => sg.cad_face_child(n).into_iter().collect(),
        "GeoLOD" => sg.geo_lod_kids(n, false),
        "HAnimHumanoid" | "HAnimJoint" | "HAnimSegment" | "HAnimSite" => sg.hanim_children(n, ty).map(|g| g.concat()).unwrap_or_default(),
        _ => {
            let mut kids = sg.nodes(n, "children");
            if kids.is_empty() && ty == "LOD" {
                kids = sg.nodes(n, "level");
            }
            kids
        }
    }
}

/// X3DGroupingNode.traverse: the enabled pointing device sensors among `kids` apply to all the
/// children instead of the enclosing ones (renderObject.getSensors () is a stack, and
/// addPointingShape takes its top: the sensors of the innermost group that has any). A disabled
/// sensor pushes nothing (`push = Function .prototype`).
fn group_sensors(sg: &Sg, path: &RtPath, kids: &[NodeId], m: &Matrix4, sensors: &[SensorRef]) -> Vec<SensorRef> {
    let mut local: Vec<SensorRef> = vec![];

    for c in kids {
        if let Some(t) = sg.type_name(*c) {
            if SENSORS.contains(&t) && sg.bool(*c, "enabled", true) {
                local.push(SensorRef { node: NodeRef { path: path.clone(), node: *c }, model_view: *m });
            }
        }
    }

    if local.is_empty() { sensors.to_vec() } else { local }
}

/// X3DPickingContext state of a PICKING traversal: getPickable (), getPickSensors () (indices into
/// `input.sensors`) and getPickingHierarchy ().
struct PickWalk<'a> {
    input: &'a crate::picking::PickInput,
    pickable: Vec<bool>,
    sensors: Vec<Vec<usize>>,
    hierarchy: Vec<NodeId>,
    visible: std::collections::HashMap<NodeId, bool>,
}

impl PickWalk<'_> {
    fn top(&self) -> bool {
        *self.pickable.last().unwrap()
    }

    /// PickableGroup.traverse: `None` with objectType "ALL" (the sensors stay), else the sensors of
    /// the current set whose objectType and matchCriterion accept the group's objectType.
    fn filter_sensors(&self, types: &[String]) -> Option<Vec<usize>> {
        if types.iter().any(|t| t == "ALL") {
            return None;
        }

        let mut out = vec![];

        for &i in self.sensors.last().unwrap() {
            let s = &self.input.sensors[i];

            if !s.object_type.iter().any(|t| t == "ALL") {
                // `++ intersection; break;`: at most 1.
                let intersection = usize::from(types.iter().any(|t| s.object_type.contains(t)));

                let keep = match s.match_criterion {
                    crate::picking::MatchCriterion::Any => intersection != 0,
                    crate::picking::MatchCriterion::Every => intersection == s.object_type.len(),
                    crate::picking::MatchCriterion::OnlyOne => intersection == 1,
                };

                if !keep {
                    continue;
                }
            }

            out.push(i);
        }

        Some(out)
    }
}

/// X3DChildNode.isVisibleObject (): the children a grouping node traverses in PICKING inside a
/// PickableGroup (`visibleObjects`).
fn visible_object(sg: &Sg, n: NodeId, memo: &mut std::collections::HashMap<NodeId, bool>, depth: usize) -> bool {
    if depth > 256 || !sg.bool(n, "visible", true) {
        return false;
    }

    if let Some(&v) = memo.get(&n) {
        return v;
    }

    let Some(ty) = sg.type_name(n) else { return false };
    let any = |kids: Vec<NodeId>, memo: &mut std::collections::HashMap<NodeId, bool>| kids.into_iter().any(|c| visible_object(sg, c, memo, depth + 1));

    let v = match ty {
        // X3DShapeNode.isEnabled ().
        "Shape" => sg.node(n, "geometry").is_some(),
        // InstancedShape.isEnabled (): instances and a geometry.
        "InstancedShape" => sg.node(n, "geometry").is_some() && !x3d_render::traverse::instance_data(sg, n).matrices.is_empty(),
        "LOD" | "GeoLOD" | "HAnimJoint" | "Background" | "TextureBackground" | "ListenerPointSource" => true,
        "DirectionalLight" | "PointLight" | "SpotLight" => sg.bool(n, "global", false),
        "ProximitySensor" | "VisibilitySensor" => sg.bool(n, "enabled", true),
        "Sound" => sg.node(n, "source").is_some(),
        // SpatialSound: setVisibleObject (this .childNodes .length).
        "SpatialSound" => sg.rt.spatial_sound_visible(n),
        "Inline" => sg.bool(n, "load", true) && sg.inline_scene(n).is_some_and(|ctx| any(sg.roots_of(ctx), memo)),
        "StaticGroup" => any(sg.nodes(n, "children"), memo),
        _ if GROUPS.contains(&ty) || matches!(ty, "Viewport" | "LayoutGroup") => any(children_of(sg, n, ty), memo),
        _ => false,
    };

    memo.insert(n, v);
    v
}

const PICK_SENSORS: &[&str] = &["LinePickSensor", "PointPickSensor", "PrimitivePickSensor", "VolumePickSensor"];

struct Walker<'a> {
    scan: &'a mut Scan,
    /// DISPLAY: LOD.traverse calls changeLevel first. Other passes use the level of the last DISPLAY.
    lod: Option<&'a mut LodPass>,
    /// PICKING: the target objects whose model matrices are collected.
    picking: Option<&'a std::collections::HashSet<NodeId>>,
    /// PICKING: the pick sensors' state.
    pick: Option<PickWalk<'a>>,
    /// `camera`: CAMERA pass (matrices start at identity, no Billboard rotation).
    camera: bool,
    /// renderObject.getLocalObjects (): clip planes pushed by the enclosing grouping nodes.
    clip_planes: Vec<[f64; 4]>,
    layer: LayerWalk,
    /// renderObject.getViewVolumes (): the viewport rectangles, the layer's first.
    viewports: Vec<[f64; 4]>,
    /// renderObject.getLayouts () and getLayoutRectangles ().
    layouts: LayoutStack,
    /// renderObject.getViewpointGroups (): the ViewpointGroup nodes around the CAMERA traversal.
    viewpoint_groups: Vec<NodeId>,
    /// renderObject.getInvHumanoidMatrix () (starts with the identity) and getHAnimNode ().
    inv_humanoid: Vec<Matrix4>,
    hanim: Vec<Option<Rc<(x3d_render::hanim::SkinTables, Vec<f32>)>>>,
    /// HAnimJoint model-view matrices of this walk.
    joint_model_views: std::collections::HashMap<NodeId, Matrix4>,
    /// ScreenGroup.matrix of the last DISPLAY traversal (ScreenGroup.traverse multiplies it in for every pass).
    screens: std::collections::HashMap<NodeId, Matrix4>,
    /// renderObject .getCollisions (): the enabled Collision nodes around the walk's position.
    collision_stack: Vec<NodeId>,
}

/// Walk the world starting with `start` as the model-view matrix (identity for the CAMERA
/// pass, the view matrix for POINTER / COLLISION / DISPLAY).
pub fn scan(root: &Runtime, layer: &LayerWalk, start: &Matrix4, camera: bool) -> Scan {
    scan_screens(root, layer, start, camera, &Default::default())
}

/// [`scan`] with the ScreenGroup.matrix values of the last DISPLAY traversal (identity for a ScreenGroup that has none).
pub fn scan_screens(root: &Runtime, layer: &LayerWalk, start: &Matrix4, camera: bool, screens: &std::collections::HashMap<NodeId, Matrix4>) -> Scan {
    let mut scan = Scan::default();
    let mut w = Walker::new(&mut scan, None, None, camera, layer);
    w.screens = screens.clone();
    w.runtime(root, &Rc::new(vec![]), start, &[], &[], true, 0);
    scan
}

/// The PICKING walk (X3DPickingContext.picking): model matrices from identity; the TransformSensors
/// reached and the matrices of the `targets`. Billboards keep no rotation here (X_ITE uses the one
/// the last DISPLAY computed).
///
/// The layer's group is a PickableGroup when the layer's scene has the Picking component
/// (X3DLayerNode constructor), with objectType "ALL" until the layer's objectType changes
/// (`layer_object_type`): then everything visible in the layer is pickable. Otherwise only
/// PickableGroup nodes make their children pickable.
pub fn scan_picking(root: &Runtime, layer: &LayerWalk, targets: &std::collections::HashSet<NodeId>, pick: &crate::picking::PickInput, pickable_layer: bool, layer_object_type: Option<&[String]>) -> Scan {
    let mut scan = Scan::default();

    if pickable_layer && layer_object_type.is_some_and(|t| t.iter().any(|t| t == "NONE")) {
        return scan;
    }

    let mut w = Walker::new(&mut scan, None, Some(targets), true, layer);
    let mut walk = PickWalk { input: pick, pickable: vec![false], sensors: vec![(0..pick.sensors.len()).collect()], hierarchy: vec![], visible: Default::default() };

    if pickable_layer {
        if let Some(filtered) = layer_object_type.and_then(|t| walk.filter_sensors(t)) {
            walk.sensors.push(filtered);
        }

        walk.pickable.push(true);
    }

    w.pick = Some(walk);
    w.runtime(root, &Rc::new(vec![]), &Matrix4::IDENTITY, &[], &[], true, 0);
    scan
}

/// The DISPLAY walk: like [`scan`] (not CAMERA), and every LOD visited changes its level first.
pub fn scan_display(root: &Runtime, layer: &LayerWalk, start: &Matrix4, lod: &mut LodPass) -> Scan {
    let mut scan = Scan::default();
    let mut w = Walker::new(&mut scan, Some(lod), None, false, layer);
    w.runtime(root, &Rc::new(vec![]), start, &[], &[], true, 0);
    scan
}

impl<'a> Walker<'a> {
    fn new(scan: &'a mut Scan, lod: Option<&'a mut LodPass>, picking: Option<&'a std::collections::HashSet<NodeId>>, camera: bool, layer: &LayerWalk) -> Walker<'a> {
        Walker { scan, lod, picking, pick: None, camera, clip_planes: vec![], layer: *layer, viewports: vec![layer.viewport], layouts: LayoutStack::default(), viewpoint_groups: vec![], inv_humanoid: vec![Matrix4::IDENTITY], hanim: vec![None], joint_model_views: Default::default(), screens: Default::default(), collision_stack: vec![] }
    }

    fn viewport(&self) -> [f64; 4] {
        *self.viewports.last().unwrap()
    }

    /// GeoLOD.traverse in the DISPLAY walk: changeLevel with getLevel (the viewer applies it).
    fn geo_lod_level(&mut self, sg: &Sg, path: &RtPath, n: NodeId, mv: &Matrix4) {
        let Some(lod) = self.lod.as_deref_mut() else { return };

        lod.geo_visited.push(NodeRef { path: path.clone(), node: n });
        let key = (path.to_vec(), n);
        let current = lod.levels.get(&key).copied().unwrap_or_else(|| sg.rt.geo_lod_level(n));

        // GeoLOD.getLevel: modelViewMatrix .translate (getCoord (center)) .origin .norm ().
        let mut c = sg.geo_object(n).get_coord(&sg.vec3(n, "center", Vector3::ZERO));
        mv.mult_vec_matrix3(&mut c);
        let level = if c.norm() < sg.f64(n, "range", 10.0) { 1 } else { 0 };

        if level != current {
            lod.levels.insert(key, level);
            lod.geo_changes.push((NodeRef { path: path.clone(), node: n }, level));
        }
    }

    /// The LOD's level_changed for this visit: LOD.changeLevel in the DISPLAY walk, else the stored one.
    fn lod_level(&mut self, sg: &Sg, path: &RtPath, n: NodeId, mv: &Matrix4) -> i32 {
        let stored = sg.i32(n, "level_changed", -1);
        let Some(lod) = self.lod.as_deref_mut() else {
            return stored;
        };

        let key = (path.to_vec(), n);
        let current = lod.levels.get(&key).copied().unwrap_or(stored);
        let rate = lod.current_frame_rate;
        let frame_rate = lod.frame_rates.entry(key.clone()).or_insert(60.0);
        let mut level = lod_get_level(sg, n, mv, frame_rate, rate);

        if sg.bool(n, "forceTransitions", false) {
            if level > current {
                level = current + 1;
            } else if level < current {
                level = current - 1;
            }
        }

        if level != current {
            lod.levels.insert(key, level);
            lod.changes.push((NodeRef { path: path.clone(), node: n }, level));
        }

        level
    }

    #[allow(clippy::too_many_arguments)]
    fn runtime(&mut self, rt: &Runtime, path: &RtPath, mv: &Matrix4, sensors: &[SensorRef], anchors: &[NodeRef], collidable: bool, depth: usize) {
        let sg = Sg::new(rt);
        let key = self.layer.key;
        let roots = layer::layer_children(&sg, key);

        // X3DLayerNode: the root nodes are the children of the layer's Group (a LayoutLayer's is a
        // LayoutGroup with the layer's layout), whose pointing device sensors apply to all of them.
        if layer::is_layout_layer(&sg, key) {
            let n = key.unwrap();
            self.layout_group(&sg, path, sg.node(n, "layout"), None, roots, mv, sensors, anchors, collidable, depth);
        } else {
            self.group(&sg, path, "Group", NodeId::MAX, roots, mv, sensors, anchors, collidable, depth);
        }
    }

    /// X3DGroupingNode.traverse: the pointing device sensors among the children apply to all
    /// children (pointingDeviceSensorNodes → sensors, then renderObject.getSensors ()), the
    /// ClipPlanes push before the children and pop after.
    #[allow(clippy::too_many_arguments)]
    fn group(&mut self, sg: &Sg, path: &RtPath, ty: &str, n: NodeId, kids: Vec<NodeId>, m: &Matrix4, sensors: &[SensorRef], anchors: &[NodeRef], collidable: bool, depth: usize) {
        let mut local_anchors = anchors.to_vec();
        let me = || NodeRef { path: path.clone(), node: n };

        // Anchor.traverse: its own TouchSensor (enabled = load) is pushed first, then the group's.
        let anchor = ty == "Anchor" && sg.bool(n, "load", true);
        let inherited = if anchor { vec![SensorRef { node: me(), model_view: *m }] } else { sensors.to_vec() };
        let local = group_sensors(sg, path, &kids, m, &inherited);

        if anchor {
            local_anchors.push(me());
        }

        let mut collidable = collidable;

        let mut proxy = None;
        let collision = ty == "Collision" && sg.bool(n, "enabled", true);

        if ty == "Collision" {
            if collision {
                self.scan.collisions += 1;
                // Collision.set_collisionObjects__: an X3DChildNode proxy replaces the children as the collision objects.
                proxy = sg.node(n, "proxy").filter(|p| is_child_node(sg, *p));
                collidable = collidable && proxy.is_none();
                self.collision_stack.push(n);
            } else {
                collidable = false;
            }
        }

        let pushed = self.push_clip_planes(sg, &kids, m);

        for c in kids {
            self.node(sg, path, c, m, &local, &local_anchors, collidable, depth + 1);
        }

        if let Some(p) = proxy {
            // Only the proxy's shapes are kept: it is traversed for COLLISION only.
            let saved = std::mem::take(&mut *self.scan);
            self.node(sg, path, p, m, &[], &[], true, depth + 1);
            let walked = std::mem::replace(&mut *self.scan, saved);
            self.scan.proxy_shapes.extend(walked.shapes);
        }

        if collision {
            self.collision_stack.pop();
        }

        self.clip_planes.truncate(self.clip_planes.len() - pushed);
    }

    /// LayoutGroup.traverse (also the group of a LayoutLayer): the Viewport pushed, then the
    /// Layout's matrix around the children.
    #[allow(clippy::too_many_arguments)]
    fn layout_group(&mut self, sg: &Sg, path: &RtPath, layout: Option<NodeId>, viewport: Option<NodeId>, kids: Vec<NodeId>, mv: &Matrix4, sensors: &[SensorRef], anchors: &[NodeRef], collidable: bool, depth: usize) {
        let viewport = viewport.filter(|v| sg.type_name(*v) == Some("Viewport"));

        if let Some(v) = viewport {
            let rectangle = layer::viewport_rectangle(sg, v, self.viewport());
            self.viewports.push(rectangle);
        }

        match layout.filter(|l| sg.type_name(*l) == Some("Layout")) {
            Some(l) => {
                let node = LayoutNode::read(sg, l);
                let current = self.viewport();
                let m = self.layouts.push(&node, &self.layer.layout, &current, mv);
                self.group(sg, path, "LayoutGroup", NodeId::MAX, kids, &m, sensors, anchors, collidable, depth);
                self.layouts.pop();
            }
            None => self.group(sg, path, "LayoutGroup", NodeId::MAX, kids, mv, sensors, anchors, collidable, depth),
        }

        if viewport.is_some() {
            self.viewports.pop();
        }
    }

    /// ClipPlane.push for the ClipPlane children of a grouping node; returns how many were pushed.
    fn push_clip_planes(&mut self, sg: &Sg, kids: &[NodeId], mv: &Matrix4) -> usize {
        let before = self.clip_planes.len();

        for &c in kids {
            if sg.type_name(c) == Some("ClipPlane") {
                if let Some(p) = x3d_render::traverse::clip_plane(sg, c, mv) {
                    self.clip_planes.push(p);
                }
            }
        }

        self.clip_planes.len() - before
    }

    #[allow(clippy::too_many_arguments)]
    fn node(&mut self, sg: &Sg, path: &RtPath, n: NodeId, mv: &Matrix4, sensors: &[SensorRef], anchors: &[NodeRef], collidable: bool, depth: usize) {
        if depth > 256 {
            return;
        }

        let Some(ty) = sg.type_name(n) else { return };
        let me = || NodeRef { path: path.clone(), node: n };

        // X3DGroupingNode: a child whose `visible` is FALSE (X3DBoundedObject._display) is in none
        // of the group's object sets (pointing, camera, pickable, collision, visible).
        if !sg.bool(n, "visible", true) {
            return;
        }

        // X3DGroupingNode PICKING: the pick and transform sensors are traversed (pickableSensorNodes);
        // inside a PickableGroup the other children only if they are visible objects.
        if let Some(p) = self.pick.as_mut() {
            if p.top() && !PICK_SENSORS.contains(&ty) && ty != "TransformSensor" && !visible_object(sg, n, &mut p.visible, depth) {
                return;
            }
        }

        // getPickingHierarchy (): the grouping nodes, Inline nodes and the shape around a shape.
        let hierarchy = self.pick.is_some() && (GROUPS.contains(&ty) || matches!(ty, "Viewport" | "LayoutGroup" | "Inline" | "StaticGroup"));

        if hierarchy {
            self.pick.as_mut().unwrap().hierarchy.push(n);
        }

        // The bindables of Inline scenes are collected too (X3DBindableNode traverse pushes them
        // on the layer's lists); BindableList.getBound does not bind them.
        match ty {
            "Viewpoint" | "OrthoViewpoint" | "GeoViewpoint" => {
                self.scan.viewpoints.push((me(), *mv));

                // X3DViewpointNode.traverse: the descriptions, if every enclosing ViewpointGroup
                // is displayed.
                if self.camera && self.pick.is_none() {
                    let mut descriptions = vec![];
                    let own = sg.string(n, "description", "");

                    if !own.is_empty() && self.viewpoint_groups.iter().all(|g| sg.rt.viewpoint_group_state(*g).1) {
                        descriptions.extend(self.viewpoint_groups.iter().map(|g| sg.string(*g, "description", "")).filter(|d| !d.is_empty()));
                        descriptions.push(own);
                    }

                    self.scan.viewpoint_descriptions.push((n, descriptions));
                }

                // GeoViewpoint traverse up to 3.2: `this .geoNavigationInfoNode .traverse (...)`.
                if let Some(nav) = sg.rt.geo_navigation_info(n) {
                    self.scan.navigation_infos.push(NodeRef { path: path.clone(), node: nav });
                }
            }
            "NavigationInfo" => self.scan.navigation_infos.push(me()),
            "Background" | "TextureBackground" => self.scan.backgrounds.push((me(), *mv)),
            "Fog" => self.scan.fogs.push(me()),
            "ProximitySensor" | "GeoProximitySensor" => self.scan.proximity.push(EnvInst { node: me(), model_view: *mv }),
            "VisibilitySensor" => self.scan.visibility.push(EnvInst { node: me(), model_view: *mv }),
            "SpatialSound" | "ListenerPointSource" => self.scan.sounds.push(EnvInst { node: me(), model_view: *mv }),
            "TransformSensor" if self.picking.is_some() => self.scan.transform_sensors.push(EnvInst { node: me(), model_view: *mv }),
            _ if PICK_SENSORS.contains(&ty) => {
                if self.pick.as_ref().is_some_and(|p| p.input.sensors.iter().any(|s| s.node == n)) {
                    self.scan.pick_sensor_matrices.push((n, *mv));
                }
            }
            // EnvironmentLight.traverse: a GeneratedCubeMapTexture specularTexture renders (and
            // NEXT_FRAME_ONLY becomes NONE) like a shape's.
            "EnvironmentLight" => {
                if let Some(lod) = self.lod.as_deref_mut() {
                    if let Some(g) = sg.node(n, "specularTexture").filter(|t| sg.type_name(*t) == Some("GeneratedCubeMapTexture")) {
                        if sg.string(g, "update", "NONE") == "NEXT_FRAME_ONLY" && sg.i32(g, "size", 128) > 0 && !lod.generated_once.iter().any(|r| r.node == g) {
                            lod.generated_once.push(NodeRef { path: path.clone(), node: g });
                        }
                    }
                }
            }
            "Shape" | "InstancedShape" => {
                if self.picking.is_some_and(|t| t.contains(&n)) {
                    self.scan.pick_targets.push((n, *mv));
                }

                // X3DShapeNode.picking: outside a PickableGroup a shape is traversed only as a
                // TransformSensor target (isPickableObject).
                if let (Some(p), Some(g)) = (self.pick.as_mut(), sg.node(n, "geometry")) {
                    if p.top() || self.picking.is_some_and(|t| t.contains(&n)) {
                        p.hierarchy.push(n);

                        for &i in p.sensors.last().unwrap() {
                            let s = &p.input.sensors[i];

                            if p.hierarchy.iter().any(|h| s.targets.contains(h)) {
                                self.scan.pick_collected.push((s.node, g, *mv));
                            }
                        }

                        p.hierarchy.pop();
                    }
                }

                // Appearance.traverse (DISPLAY): the rendered textures of an independent renderer.
                if let Some(lod) = self.lod.as_deref_mut() {
                    let texture = sg.node(n, "appearance").and_then(|a| sg.node(a, "texture"));
                    let generated: Vec<NodeId> = match texture.and_then(|t| sg.type_name(t).map(|ty| (t, ty))) {
                        Some((t, "GeneratedCubeMapTexture")) => vec![t],
                        Some((t, "MultiTexture")) => sg.nodes(t, "texture").into_iter().filter(|u| sg.type_name(*u) == Some("GeneratedCubeMapTexture")).collect(),
                        _ => vec![],
                    };

                    for g in generated {
                        if sg.string(g, "update", "NONE") == "NEXT_FRAME_ONLY" && sg.i32(g, "size", 128) > 0 && !lod.generated_once.iter().any(|r| r.node == g) {
                            lod.generated_once.push(NodeRef { path: path.clone(), node: g });
                        }
                    }

                    for r in x3d_render::traverse::rendered_textures(sg, sg.node(n, "appearance")) {
                        if !lod.rendered_reached.iter().any(|x| x.node == r && x.path == *path) {
                            lod.rendered_reached.push(NodeRef { path: path.clone(), node: r });
                        }
                    }
                }

                // InstancedShape: the pointing shader draws every instance, so each is a hit
                // candidate under its own matrix (the sensors are those of the shape).
                let explicit_bbox = {
                    let size = sg.vec3(n, "bboxSize", Vector3::new(-1.0, -1.0, -1.0));
                    let center = sg.vec3(n, "bboxCenter", Vector3::ZERO);

                    // isDefaultBBoxSize: every component -1.
                    (!(size.x == -1.0 && size.y == -1.0 && size.z == -1.0)).then(|| {
                        let half = Vector3::new(size.x / 2.0, size.y / 2.0, size.z / 2.0);
                        (Vector3::new(center.x - half.x, center.y - half.y, center.z - half.z), Vector3::new(center.x + half.x, center.y + half.y, center.z + half.z))
                    })
                };
                let all_instances: Option<Rc<Vec<Matrix4>>> = (ty == "InstancedShape").then(|| Rc::new(x3d_render::traverse::instance_data(sg, n).matrices.to_vec()));
                let instance_matrices: Vec<(Matrix4, Option<Matrix4>)> = if ty == "InstancedShape" {
                    x3d_render::traverse::instance_data(sg, n).matrices.iter().map(|m| {
                        let mut c = *m;
                        c.mult_right(mv);
                        (c, Some(*m))
                    }).collect()
                } else {
                    vec![(*mv, None)]
                };

                let appearance = sg.node(n, "appearance").filter(|a| sg.type_name(*a) == Some("Appearance"));
                let style = sg.node(n, "geometry").and_then(|g| {
                    let geometry_type = match sg.type_name(g) {
                        Some("PointSet") => 0,
                        Some("IndexedLineSet" | "LineSet" | "Polyline2D" | "Arc2D" | "Circle2D" | "ArcClose2D") => 1,
                        _ => 2,
                    };
                    appearance.and_then(|a| x3d_render::traverse::style_params(sg, a, geometry_type))
                });
                let textures = appearance.map_or((false, false), |a| {
                    let material = sg.node(a, "material");
                    let material_texture = material.is_some_and(|m| {
                        ["ambientTexture", "diffuseTexture", "specularTexture", "emissiveTexture", "shininessTexture", "occlusionTexture", "normalTexture", "baseTexture", "metallicRoughnessTexture"]
                            .iter()
                            .any(|field| sg.node(m, field).is_some())
                    });
                    (sg.node(a, "texture").is_some(), material_texture)
                });

                for (model_view, instance) in instance_matrices.iter().filter(|_| sg.node(n, "geometry").is_some()) {
                    let g = sg.node(n, "geometry").unwrap();
                    self.scan.shapes.push(ShapeInst {
                        shape: me(),
                        geometry: g,
                        model_view: *model_view,
                        shape_model_view: *mv,
                        instance: *instance,
                        all_instances: all_instances.clone(),
                        explicit_bbox,
                        sensors: sensors.to_vec(),
                        collidable,
                        collisions: self.collision_stack.clone(),
                        anchors: anchors.to_vec(),
                        clip_planes: self.clip_planes.clone(),
                        viewport: self.viewport(),
                        skin: self.hanim.last().cloned().flatten(),
                        style,
                        textures,
                    });
                }
            }
            // ViewpointGroup.traverse: its private ProximitySensor in every pass; CAMERA the
            // viewpoints and ViewpointGroup children (cameraObjects); DISPLAY, while active, the
            // ViewpointGroup children. It is a camera and visible object only with such children;
            // it is no pointing, collision or pickable object.
            "ViewpointGroup" => {
                let display = self.lod.is_some();

                if self.pick.is_some() || !(self.camera || display) {
                    return;
                }

                let children: Vec<(NodeId, bool)> = sg
                    .nodes(n, "children")
                    .into_iter()
                    .filter_map(|c| match sg.type_name(c) {
                        Some("Viewpoint" | "OrthoViewpoint" | "GeoViewpoint") => Some((c, false)),
                        Some("ViewpointGroup") => Some((c, true)),
                        _ => None,
                    })
                    .collect();

                if children.is_empty() {
                    return;
                }

                if let Some(s) = sg.rt.viewpoint_group_sensor(n) {
                    self.scan.proximity.push(EnvInst { node: NodeRef { path: path.clone(), node: s }, model_view: *mv });
                }

                if self.camera {
                    self.viewpoint_groups.push(n);
                    for (c, _) in children {
                        self.node(sg, path, c, mv, sensors, anchors, collidable, depth + 1);
                    }
                    self.viewpoint_groups.pop();
                } else if sg.rt.viewpoint_group_state(n).0 {
                    for (c, _) in children.into_iter().filter(|(_, group)| *group) {
                        self.node(sg, path, c, mv, sensors, anchors, collidable, depth + 1);
                    }
                }
            }
            "Inline" => {
                if sg.bool(n, "load", true) {
                    if let Some(ctx) = sg.inline_scene(n) {
                        let roots = sg.roots_of(ctx);
                        let pushed = self.push_clip_planes(sg, &roots, mv);
                        // The Inline's scene group: its root sensors apply to its root nodes.
                        let sensors = group_sensors(sg, path, &roots, mv, sensors);
                        for r in roots {
                            self.node(sg, path, r, mv, &sensors, anchors, collidable, depth + 1);
                        }
                        self.clip_planes.truncate(self.clip_planes.len() - pushed);
                    }
                }
            }
            // StaticGroup: the plain Group in the frame that starts the optimization.
            "StaticGroup" if self.lod.as_ref().is_some_and(|l| !l.static_done.contains(&(path.to_vec(), n))) => {
                if let Some(l) = self.lod.as_mut() {
                    l.static_reached.push((path.to_vec(), n));
                }

                for c in sg.nodes(n, "children") {
                    self.node(sg, path, c, mv, sensors, anchors, collidable, depth + 1);
                }
            }
            // StaticGroup traverses its optimized group: only the shapes, with static matrices.
            "StaticGroup" => {
                let mut shapes = vec![];
                sg.static_shapes(n, &Matrix4::IDENTITY, &mut shapes, depth);
                for (s, m) in shapes {
                    let mut m = m;
                    m.mult_right(mv);
                    self.node(sg, path, s, &m, sensors, anchors, collidable, depth + 1);
                }
            }
            // HAnimHumanoid.traverse: its private Transform's skeleton, viewpoints and skin groups;
            // the skin's shapes are skinned for pointing (the joint matrices of this walk).
            "HAnimHumanoid" if self.pick.is_none() => {
                let mut m = *mv;
                m.mult_left(&sg.group_matrix(n, ty).unwrap());
                let mut inv = m;
                inv.inverse();
                let Some([skeleton, viewpoints, skin]) = sg.hanim_children(n, ty) else { return };

                self.inv_humanoid.push(inv);
                self.group(sg, path, ty, n, skeleton, &m, sensors, anchors, collidable, depth);
                self.group(sg, path, ty, n, viewpoints, &m, sensors, anchors, collidable, depth);

                let source = (!self.camera).then(|| {
                    let (joints, bindings) = x3d_render::hanim::humanoid_joints(sg, n);
                    let tables = x3d_render::hanim::humanoid_tables(sg, n, &joints);
                    let skin_coord = sg.node(n, "skinCoord").is_some_and(|c| matches!(sg.type_name(c), Some("Coordinate" | "CoordinateDouble")));
                    let matrices = if skin_coord {
                        let model_views: Vec<Matrix4> = joints.iter().map(|j| self.joint_model_views.get(j).copied().unwrap_or(Matrix4::IDENTITY)).collect();
                        x3d_render::hanim::joint_matrices(&bindings, &model_views, &inv)
                    } else {
                        vec![0.0; joints.len() * 32]
                    };
                    Rc::new((tables, matrices))
                });

                self.hanim.push(source);
                self.group(sg, path, ty, n, skin, &m, sensors, anchors, collidable, depth);
                self.hanim.pop();
                self.inv_humanoid.pop();
            }
            _ if GROUPS.contains(&ty) => {
                let mut m = *mv;

                match ty {
                    "Transform" | "CADPart" | "GeoLocation" | "GeoTransform" | "HAnimHumanoid" | "HAnimJoint" | "HAnimSite" => {
                        m.mult_left(&sg.group_matrix(n, ty).unwrap());

                        if ty == "HAnimJoint" {
                            self.joint_model_views.insert(n, m);
                        }
                    }
                    "Billboard" if !self.camera => {
                        let b = billboard_matrix(sg, n, &m);
                        m.mult_left(&b);
                    }
                    "ScreenGroup" => {
                        if let Some(s) = self.screens.get(&n) {
                            m.mult_left(s);
                        }
                    }
                    _ => {}
                }

                // PickableGroup.traverse (PICKING): not pickable, only its TransformSensors; objectType
                // "NONE", nothing; else its children are pickable, for the sensors that match.
                let pick_group = if ty == "PickableGroup" && self.pick.is_some() {
                    if !sg.bool(n, "pickable", true) {
                        Some(None)
                    } else if sg.rt.pick_sensor_object_type(n).iter().any(|t| t == "NONE") {
                        Some(Some(None))
                    } else {
                        Some(Some(Some(self.pick.as_ref().unwrap().filter_sensors(&sg.rt.pick_sensor_object_type(n)))))
                    }
                } else {
                    None
                };

                if self.picking.is_some_and(|t| t.contains(&n)) && !matches!(pick_group, Some(Some(None))) {
                    self.scan.pick_targets.push((n, m));
                }

                if ty == "GeoLOD" {
                    self.geo_lod_level(sg, path, n, &m);
                }

                match pick_group {
                    Some(Some(Some(filtered))) => {
                        let p = self.pick.as_mut().unwrap();
                        p.pickable.push(true);
                        let pushed = filtered.is_some();

                        if let Some(f) = filtered {
                            p.sensors.push(f);
                        }

                        self.group(sg, path, ty, n, children_of(sg, n, ty), &m, sensors, anchors, collidable, depth);
                        let p = self.pick.as_mut().unwrap();
                        p.pickable.pop();

                        if pushed {
                            p.sensors.pop();
                        }
                    }
                    Some(_) => {}
                    None => {
                        let kids = if ty == "LOD" { lod_child(sg, n, self.lod_level(sg, path, n, &m)) } else { children_of(sg, n, ty) };
                        // HAnimSegment with a coord and displacements: the hAnimNode of its children.
                        let segment = (ty == "HAnimSegment" && !self.camera)
                            .then(|| x3d_render::hanim::segment_tables(sg, n))
                            .flatten()
                            .map(|tables| Rc::new((tables, x3d_render::hanim::segment_matrices(&m, self.inv_humanoid.last().unwrap()))));
                        let pushed = segment.is_some();

                        if pushed {
                            self.hanim.push(segment);
                        }

                        self.group(sg, path, ty, n, kids, &m, sensors, anchors, collidable, depth);

                        if pushed {
                            self.hanim.pop();
                        }
                    }
                }
            }
            // Viewport.traverse: its rectangle in the enclosing one around the children.
            "Viewport" => {
                let rectangle = layer::viewport_rectangle(sg, n, self.viewport());
                self.viewports.push(rectangle);
                self.group(sg, path, ty, n, children_of(sg, n, ty), mv, sensors, anchors, collidable, depth);
                self.viewports.pop();
            }
            "LayoutGroup" => self.layout_group(sg, path, sg.node(n, "layout"), sg.node(n, "viewport"), children_of(sg, n, ty), mv, sensors, anchors, collidable, depth),
            _ => {}
        }

        if hierarchy {
            self.pick.as_mut().unwrap().hierarchy.pop();
        }
    }
}
