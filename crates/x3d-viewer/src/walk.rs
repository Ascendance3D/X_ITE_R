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
use x3d_render::sg::Sg;
use x3d_runtime::Runtime;

/// Path of Inline nodes from the world's runtime to the runtime holding a node.
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
    /// Pointing device sensors that apply (X3DGroupingNode pointingDeviceSensorNodes of every
    /// enclosing group), outermost first.
    pub sensors: Vec<SensorRef>,
    /// Inside no disabled Collision node (Collision.set_collisionObjects__).
    pub collidable: bool,
    /// Anchor nodes of the sensors list (their hidden TouchSensor), for activation reports.
    pub anchors: Vec<NodeRef>,
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
    pub shapes: Vec<ShapeInst>,
    pub proximity: Vec<EnvInst>,
    pub visibility: Vec<EnvInst>,
    /// Enabled Collision nodes seen (browser.getCollisionCount).
    pub collisions: usize,
}

const GROUPS: &[&str] = &["Group", "Transform", "Anchor", "Collision", "Billboard", "Switch", "LOD", "StaticGroup"];
const SENSORS: &[&str] = &["TouchSensor", "PlaneSensor", "SphereSensor", "CylinderSensor"];

/// Resolve a runtime path (Inline nodes) from the root runtime.
pub fn rt_at<'a>(root: &'a Runtime, path: &[NodeId]) -> Option<&'a Runtime> {
    let mut rt = root;

    for n in path {
        rt = rt.inline_runtimes.get(n)?;
    }

    Some(rt)
}

pub fn rt_at_mut<'a>(root: &'a mut Runtime, path: &[NodeId]) -> Option<&'a mut Runtime> {
    let mut rt = root;

    for n in path {
        rt = rt.inline_runtimes.get_mut(n)?;
    }

    Some(rt)
}

/// Every runtime path (root and nested Inlines), parents before children.
pub fn all_paths(root: &Runtime) -> Vec<Vec<NodeId>> {
    fn go(rt: &Runtime, prefix: &mut Vec<NodeId>, out: &mut Vec<Vec<NodeId>>) {
        out.push(prefix.clone());
        let mut keys: Vec<NodeId> = rt.inline_runtimes.keys().copied().collect();
        keys.sort();
        for k in keys {
            prefix.push(k);
            go(&rt.inline_runtimes[&k], prefix, out);
            prefix.pop();
        }
    }

    let mut out = vec![];
    go(root, &mut vec![], &mut out);
    out
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

fn lod_level(sg: &Sg, n: NodeId, model_view: &Matrix4, count: usize) -> Option<usize> {
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
        _ => {
            let mut kids = sg.nodes(n, "children");
            if kids.is_empty() && ty == "LOD" {
                kids = sg.nodes(n, "level");
            }
            kids
        }
    }
}

struct Walker<'a> {
    scan: &'a mut Scan,
    /// `camera`: CAMERA pass (matrices start at identity, no Billboard rotation).
    camera: bool,
}

/// Walk the world starting with `start` as the model-view matrix (identity for the CAMERA
/// pass, the view matrix for POINTER / COLLISION / DISPLAY).
pub fn scan(root: &Runtime, start: &Matrix4, camera: bool) -> Scan {
    let mut scan = Scan::default();
    let mut w = Walker { scan: &mut scan, camera };
    w.runtime(root, &Rc::new(vec![]), start, &[], &[], true, 0);
    scan
}

impl Walker<'_> {
    #[allow(clippy::too_many_arguments)]
    fn runtime(&mut self, rt: &Runtime, path: &RtPath, mv: &Matrix4, sensors: &[SensorRef], anchors: &[NodeRef], collidable: bool, depth: usize) {
        let sg = Sg::new(rt);

        for r in sg.roots() {
            self.node(&sg, path, r, mv, sensors, anchors, collidable, depth);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn node(&mut self, sg: &Sg, path: &RtPath, n: NodeId, mv: &Matrix4, sensors: &[SensorRef], anchors: &[NodeRef], collidable: bool, depth: usize) {
        if depth > 256 {
            return;
        }

        let Some(ty) = sg.type_name(n) else { return };
        let me = || NodeRef { path: path.clone(), node: n };
        // Bindables of Inline scenes are not bound (BindableList.getBound enableInlineBindables).
        let inline = !path.is_empty();

        match ty {
            "Viewpoint" if !inline => self.scan.viewpoints.push((me(), *mv)),
            "NavigationInfo" if !inline => self.scan.navigation_infos.push(me()),
            "Background" if !inline => self.scan.backgrounds.push((me(), *mv)),
            "Fog" if !inline => self.scan.fogs.push(me()),
            "ProximitySensor" => self.scan.proximity.push(EnvInst { node: me(), model_view: *mv }),
            "VisibilitySensor" => self.scan.visibility.push(EnvInst { node: me(), model_view: *mv }),
            "Shape" => {
                if let Some(g) = sg.node(n, "geometry") {
                    self.scan.shapes.push(ShapeInst {
                        shape: me(),
                        geometry: g,
                        model_view: *mv,
                        sensors: sensors.to_vec(),
                        collidable,
                        anchors: anchors.to_vec(),
                    });
                }
            }
            "Inline" => {
                if sg.bool(n, "load", true) {
                    if let Some(child) = sg.rt.inline_runtimes.get(&n) {
                        let mut p = (**path).clone();
                        p.push(n);
                        self.runtime(child, &Rc::new(p), mv, sensors, anchors, collidable, depth + 1);
                    }
                }
            }
            _ if GROUPS.contains(&ty) => {
                let mut m = *mv;

                match ty {
                    "Transform" => {
                        m.mult_left(&transform_matrix(sg, n));
                    }
                    "Billboard" if !self.camera => {
                        let b = billboard_matrix(sg, n, &m);
                        m.mult_left(&b);
                    }
                    _ => {}
                }

                let kids = children_of(sg, n, ty);
                let kids = if ty == "LOD" { lod_level(sg, n, &m, kids.len()).map(|l| vec![kids[l]]).unwrap_or_default() } else { kids };

                // X3DGroupingNode: the pointing device sensors among the children apply to all
                // children (pointingDeviceSensorNodes → sensors, then renderObject.getSensors ()).
                let mut local: Vec<SensorRef> = sensors.to_vec();
                let mut local_anchors = anchors.to_vec();

                for c in &kids {
                    if let Some(t) = sg.type_name(*c) {
                        if SENSORS.contains(&t) && sg.bool(*c, "enabled", true) {
                            local.push(SensorRef { node: NodeRef { path: path.clone(), node: *c }, model_view: m });
                        }
                    }
                }

                // Anchor.traverse: its own TouchSensor (enabled = load).
                if ty == "Anchor" && sg.bool(n, "load", true) {
                    local.push(SensorRef { node: me(), model_view: m });
                    local_anchors.push(me());
                }

                let mut collidable = collidable;

                if ty == "Collision" {
                    if sg.bool(n, "enabled", true) {
                        self.scan.collisions += 1;
                    } else {
                        collidable = false;
                    }
                }

                for c in kids {
                    self.node(sg, path, c, &m, &local, &local_anchors, collidable, depth + 1);
                }
            }
            _ => {}
        }
    }
}
