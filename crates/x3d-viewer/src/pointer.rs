//! Pointing device sensors: Browser/PointingDeviceSensor/X3DPointingDeviceSensorContext.js,
//! PointingDevice.js, Components/PointingDeviceSensor/{X3DPointingDeviceSensorNode,
//! X3DTouchSensorNode, X3DDragSensorNode, TouchSensor, PlaneSensor, CylinderSensor,
//! SphereSensor}.js and Components/Networking/Anchor.js (its hidden TouchSensor).
//!
//! X_ITE picks by rendering the pointer's pixel into a 1x1 float buffer (vertex position,
//! normal, texCoord in view space). Here the pointer ray is cast against the triangles of the
//! visible shapes instead; the hit values are the same (up to float precision).

use crate::geom::{Cylinder3, Line2, Line3, Plane3, Sphere3, Triangle3, ViewVolume, interval};
use crate::walk::{NodeRef, SensorRef, ShapeInst, rt_at_mut};
use std::rc::Rc;
use x3d_fields::math::{Matrix3, Matrix4, Rotation4, Vector2, Vector3, Vector4};
use x3d_fields::{FieldValue, NodeId};
use x3d_render::geometry::Geometry;
use x3d_render::pointing_gpu::{PointingGpu, PointingTriangle};
use x3d_runtime::Runtime;

#[derive(Clone, Debug)]
pub struct Hit {
    pub id: bool,
    pub pointer: Vector2,
    /// The pointer ray in view space (X3DRenderObject.getHitRay).
    pub ray: Line3,
    pub sensors: Vec<SensorRef>,
    pub view_matrix: Matrix4,
    pub model_view: Matrix4,
    pub point: Vector3,
    pub normal: Vector3,
    pub tex_coord: Vector4,
    pub shape: Option<NodeRef>,
    /// The hit shape's geometry bounding box in its local space (`hit .shapeNode .getBBox ()`).
    pub bbox: Option<(Vector3, Vector3)>,
    pub anchors: Vec<NodeRef>,
    /// hit .layerNode: the layer the pick ran in.
    pub layer: x3d_render::layer::LayerKey,
}

impl Default for Hit {
    fn default() -> Hit {
        Hit {
            id: false,
            pointer: Vector2::ZERO,
            ray: Line3::new(Vector3::ZERO, Vector3::new(0.0, 0.0, -1.0)),
            sensors: vec![],
            view_matrix: Matrix4::IDENTITY,
            model_view: Matrix4::IDENTITY,
            point: Vector3::ZERO,
            normal: Vector3::ZERO,
            tex_coord: Vector4::ZERO,
            shape: None,
            bbox: None,
            layer: None,
            anchors: vec![],
        }
    }
}

/// What the viewer needs to know about an Anchor click (the UI reports it, nothing is opened).
#[derive(Clone, Debug)]
pub struct AnchorActivation {
    pub node: NodeRef,
    pub description: String,
    pub urls: Vec<String>,
    pub parameters: Vec<String>,
}

/// Camera for picking: projection matrix and viewport (x, y, w, h).
#[derive(Clone, Copy, Debug)]
pub struct PickCamera {
    pub projection: Matrix4,
    pub viewport: Vector4,
    pub view: Matrix4,
    pub near: f64,
    pub far: f64,
}

/// X3DRenderObject.setHitRay.
pub fn hit_ray(cam: &PickCamera, x: f64, y: f64) -> Line3 {
    let mut ray = Line3::new(Vector3::ZERO, Vector3::new(0.0, 0.0, -1.0));
    ViewVolume::unproject_ray(x, y, &Matrix4::IDENTITY, &cam.projection, &cam.viewport, &mut ray);
    ray
}

struct TriHit {
    /// The ray is inside the triangle (within [`EDGE`]); the others are near enough that the
    /// GPU's snapped vertices might still cover the pixel centre.
    strict: bool,
    t: f64,
    u: f64,
    v: f64,
    tri: usize,
}

/// Möller-Trumbore in object space; both faces.
/// The nearest triangle hit along the ray whose point `keep` accepts (the pointing shader
/// discards clipped fragments, so a clipped hit lets the ray reach the triangles behind it).
fn ray_triangles(g: &Geometry, origin: &Vector3, dir: &Vector3, keep: &dyn Fn(f64) -> bool) -> Vec<TriHit> {
    if g.geometry_type < 2 {
        return vec![];
    }

    let mut hits: Vec<TriHit> = vec![];

    let vert = |k: usize| {
        let v = &g.vertices[k * 4..k * 4 + 3];
        Vector3::new(v[0] as f64, v[1] as f64, v[2] as f64)
    };

    for tri in 0..g.vertex_count() / 3 {
        let (a, b, c) = (vert(tri * 3), vert(tri * 3 + 1), vert(tri * 3 + 2));
        let mut e1 = b;
        e1.subtract(&a);
        let mut e2 = c;
        e2.subtract(&a);
        let mut p = *dir;
        p.cross(&e2);
        let det = e1.dot(&p);

        if det.abs() < 1e-14 {
            continue;
        }

        let inv = 1.0 / det;
        let mut s = *origin;
        s.subtract(&a);
        let u = s.dot(&p) * inv;

        // A ray through an edge belongs to both neighbours; the GPU's fill rule picks one.
        if !(-NEAR..=1.0 + NEAR).contains(&u) {
            continue;
        }

        let mut q = s;
        q.cross(&e1);
        let v = dir.dot(&q) * inv;

        if v < -NEAR || u + v > 1.0 + NEAR {
            continue;
        }

        let t = e2.dot(&q) * inv;

        if keep(t) {
            hits.push(TriHit { strict: (-EDGE..=1.0 + EDGE).contains(&u) && v >= -EDGE && u + v <= 1.0 + EDGE, t, u, v, tri });
        }
    }

    hits
}

/// Barycentric slack of the ray cast: the pointing shader decides the triangles on an edge.
const EDGE: f64 = 1e-9;

/// Candidates for the GPU: triangles whose edge the ray passes within this barycentric distance
/// (the rasterizer snaps vertices to 1/256 pixel).
const NEAR: f64 = 0.05;

/// Pick: the nearest shape under the pointer ray (X3DPointingDeviceSensorContext.touch).
/// A shape inside Viewport nodes is drawn into the pointing buffer with its own viewport: it is
/// skipped when the pointer is outside that rectangle (Viewport.traverse, unless `pointing`, a
/// layer holds the pointer), and its ray goes through that viewport.
pub fn pick(shapes: &[ShapeInst], geometry: &mut dyn FnMut(&ShapeInst) -> Option<Rc<Geometry>>, cam: &PickCamera, x: f64, y: f64, pointing: bool, mut gpu: Option<&mut PointingGpu>) -> Hit {
    let ray = hit_ray(cam, x, y);
    // The hit point, normal and texCoord come from the pointing buffer: a 1x1 float buffer drawn
    // with the viewport shifted so that it covers the pixel under the pointer
    // (X3DRenderObject.pointing), i.e. they are sampled at that pixel's center. The hit ray
    // itself (setHitRay) uses the exact pointer.
    let sray = hit_ray(cam, x.floor() + 0.5, y.floor() + 0.5);
    let mut hit = Hit { pointer: Vector2::new(x, y), ray: ray.copy(), view_matrix: cam.view, ..Default::default() };

    let mut best: Option<(f64, usize, Vec<TriHit>, Rc<Geometry>)> = None;

    let shape_ray = |s: &ShapeInst| {
        if s.viewport == [cam.viewport.x, cam.viewport.y, cam.viewport.z, cam.viewport.w] {
            sray.copy()
        } else {
            let v = s.viewport;
            hit_ray(&PickCamera { viewport: Vector4::new(v[0], v[1], v[2], v[3]), ..cam.clone() }, x.floor() + 0.5, y.floor() + 0.5)
        }
    };

    for (i, s) in shapes.iter().enumerate() {
        if !pointing && !x3d_render::layer::pointer_in_rectangle(&s.viewport, x, y) {
            continue;
        }

        let Some(g) = geometry(s) else { continue };
        // The pointing shader skins the shapes of an HAnimHumanoid or HAnimSegment.
        let g = match &s.skin {
            Some(skin) => Rc::new(x3d_render::hanim::skin(&g, &skin.0, &skin.1)),
            None => g,
        };
        let sray = shape_ray(s);
        let mut inv = s.model_view;
        inv.inverse();
        let mut o = sray.point;
        inv.mult_vec_matrix3(&mut o);
        let mut d = sray.direction;
        inv.mult_dir_matrix(&mut d);

        // Quick reject with the object's bounding box.
        if !ray_box(&g.min, &g.max, &o, &d) {
            continue;
        }

        // ClipPlanes2.glsl clip () in the pointing shader: view-space hit points.
        let clipped = |t: f64| {
            let mut p = d;
            p.multiply(t);
            p.add(&o);
            s.model_view.mult_vec_matrix3(&mut p);
            x3d_render::traverse::is_clipped(&s.clip_planes, &p)
        };

        let hits = ray_triangles(&g, &o, &d, &|t| !clipped(t));
        let Some(th) = hits.iter().filter(|h| h.strict).min_by(|a, b| a.t.total_cmp(&b.t)) else { continue };

        // View space hit point and its distance along the view ray.
        let mut p = o;
        let mut dd = d;
        dd.multiply(th.t);
        p.add(&dd);
        s.model_view.mult_vec_matrix3(&mut p);

        // Depth range of the pointing buffer: between near and far plane.
        if -p.z < cam.near || -p.z > cam.far {
            continue;
        }

        // The depth test of the pointing buffer: view space depth (the same projection for all).
        let dist = -p.z;

        if best.as_ref().is_none_or(|b| dist < b.0) {
            best = Some((dist, i, hits, g));
        }
    }

    if let Some((_, i, mut candidates, g)) = best {
        // The nearest triangle is the ray cast's own hit; every candidate goes to the GPU, far to
        // near (the pointing buffer's depth test keeps the nearest fragment).
        candidates.sort_by(|a, b| b.t.total_cmp(&a.t));
        let th = candidates.iter().rev().find(|h| h.strict).unwrap();
        let s = &shapes[i];
        let sray = shape_ray(s);
        let mut inv = s.model_view;
        inv.inverse();
        let mut o = sray.point;
        inv.mult_vec_matrix3(&mut o);
        let mut d = sray.direction;
        inv.mult_dir_matrix(&mut d);

        let mut p = o;
        let mut dd = d;
        dd.multiply(th.t);
        p.add(&dd);
        s.model_view.mult_vec_matrix3(&mut p);

        let w0 = 1.0 - th.u - th.v;
        let k = th.tri * 3;

        // Interpolated normal (view space, normalMatrix = inverse transpose).
        let has_normals = !g.normals.is_empty();
        let mut normal = Vector3::Z_AXIS;

        if has_normals {
            let nv = |j: usize| Vector3::new(g.normals[j * 3] as f64, g.normals[j * 3 + 1] as f64, g.normals[j * 3 + 2] as f64);
            let (n0, n1, n2) = (nv(k), nv(k + 1), nv(k + 2));
            let mut n = Vector3::new(n0.x * w0 + n1.x * th.u + n2.x * th.v, n0.y * w0 + n1.y * th.u + n2.y * th.v, n0.z * w0 + n1.z * th.u + n2.z * th.v);
            // normalMatrix * normal
            let mut nm = s.model_view.submatrix();
            nm.inverse();
            nm.transpose();
            nm.mult_matrix_vec3(&mut n);
            n.normalize();
            // touch (): hit .modelViewMatrix .submatrix .inverse () .multMatrixVec (hit .normal)
            let mut inv3 = s.model_view.submatrix();
            inv3.inverse();
            inv3.mult_matrix_vec3(&mut n);
            normal = n;
        }

        let mut tc = Vector4::new(0.0, 0.0, 0.0, 1.0);

        if g.tex_coords.len() >= (k + 3) * 4 {
            let tv = |j: usize| [g.tex_coords[j * 4] as f64, g.tex_coords[j * 4 + 1] as f64, g.tex_coords[j * 4 + 2] as f64, g.tex_coords[j * 4 + 3] as f64];
            let (t0, t1, t2) = (tv(k), tv(k + 1), tv(k + 2));
            tc = Vector4::new(
                t0[0] * w0 + t1[0] * th.u + t2[0] * th.v,
                t0[1] * w0 + t1[1] * th.u + t2[1] * th.v,
                t0[2] * w0 + t1[2] * th.u + t2[2] * th.v,
                t0[3] * w0 + t1[3] * th.u + t2[3] * th.v,
            );
        }

        let mut p = p;
        let mut normal = normal;
        let mut tc = tc;

        // The pointing buffer's own float32 values: the same triangle through the pointing shader.
        if let Some(gpu) = gpu.as_deref_mut() {
            let f32x16 = |m: &Matrix4| m.0.map(|v| v as f32);
            let vert = |j: usize| [g.vertices[j * 4], g.vertices[j * 4 + 1], g.vertices[j * 4 + 2], g.vertices[j * 4 + 3]];
            let norm = |j: usize| if has_normals { [g.normals[j * 3], g.normals[j * 3 + 1], g.normals[j * 3 + 2]] } else { [0.0, 0.0, 1.0] };
            let has_tex = g.tex_coords.len() >= g.vertex_count() * 4;
            let tex = |j: usize| if has_tex { [g.tex_coords[j * 4], g.tex_coords[j * 4 + 1], g.tex_coords[j * 4 + 2], g.tex_coords[j * 4 + 3]] } else { [0.0, 0.0, 0.0, 1.0] };
            let v = s.viewport;
            let corners: Vec<usize> = candidates.iter().flat_map(|c| [c.tri * 3, c.tri * 3 + 1, c.tri * 3 + 2]).collect();
            // gl .viewport (viewport .x - x, viewport .y - y, viewport .z, viewport .w): GLint arguments.
            let tri = PointingTriangle {
                vertices: corners.iter().map(|&j| vert(j)).collect(),
                normals: corners.iter().map(|&j| norm(j)).collect(),
                tex_coords: corners.iter().map(|&j| tex(j)).collect(),
                projection: f32x16(&cam.projection),
                model_view: f32x16(&s.shape_model_view),
                instance: s.instance.as_ref().map_or(x3d_render::pointing_gpu::IDENTITY, f32x16),
                instance_normal: s.instance.as_ref().map_or(x3d_render::pointing_gpu::IDENTITY3, |m| {
                    let mut n = m.submatrix();
                    n.transpose();
                    n.inverse();
                    n.0.map(|v| v as f32)
                }),
                viewport: [(v[0] - x) as i32, (v[1] - y) as i32, v[2] as i32, v[3] as i32],
                id: 1.0,
            };

            if let Some(sample) = gpu.sample(&tri) {
                p = Vector3::new(sample.point[0] as f64, sample.point[1] as f64, sample.point[2] as f64);
                tc = Vector4::new(sample.tex_coord[0] as f64, sample.tex_coord[1] as f64, sample.tex_coord[2] as f64, sample.tex_coord[3] as f64);

                // touch (): hit .modelViewMatrix .submatrix .inverse () .multMatrixVec (hit .normal)
                // when the geometry has normals, else the Z axis.
                normal = Vector3::Z_AXIS;

                if has_normals {
                    let mut n = Vector3::new(sample.normal[0] as f64, sample.normal[1] as f64, sample.normal[2] as f64);
                    let mut mv = s.shape_model_view;
                    mv.0 = mv.0.map(|v| v as f32 as f64);
                    let mut inv3 = mv.submatrix();
                    inv3.inverse();
                    inv3.mult_matrix_vec3(&mut n);
                    normal = n;
                }
            }
        }

        hit.id = true;
        hit.point = p;
        hit.normal = normal;
        hit.tex_coord = tc;
        // hit .modelViewMatrix is the shape's own (an InstancedShape's instance matrices are in its
        // vertex shader) and hit .shapeNode .getBBox () the union of the instances' boxes
        // (InstancedShape.set_bbox__: `bbox .add (subBBox .assign (geometryBBox) .multRight (matrix))`).
        hit.model_view = s.shape_model_view;
        hit.sensors = s.sensors.clone();
        hit.shape = Some(s.shape.clone());
        hit.bbox = Some(match (&s.explicit_bbox, &s.all_instances) {
            (Some(explicit), _) => *explicit,
            (None, Some(all)) => {
                let mut union: Option<(Vector3, Vector3)> = None;

                for m in all.iter() {
                    let mut b = crate::lookat::Box3::from_extents(&g.min, &g.max);
                    b.mult_right(m);
                    let (lo, hi) = b.extents();

                    union = Some(match union {
                        None => (lo, hi),
                        Some((mut a, mut c)) => (*a.min(&lo), *c.max(&hi)),
                    });
                }

                union.unwrap_or((g.min, g.max))
            }
            (None, None) => (g.min, g.max),
        });
        hit.anchors = s.anchors.clone();
    }

    hit
}

fn ray_box(min: &Vector3, max: &Vector3, o: &Vector3, d: &Vector3) -> bool {
    let mut t0 = f64::NEG_INFINITY;
    let mut t1 = f64::INFINITY;

    for (mn, mx, oo, dd) in [(min.x, max.x, o.x, d.x), (min.y, max.y, o.y, d.y), (min.z, max.z, o.z, d.z)] {
        if dd.abs() < 1e-300 {
            if oo < mn || oo > mx {
                return false;
            }
        } else {
            let (a, b) = ((mn - oo) / dd, (mx - oo) / dd);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
    }

    t1 >= t0.max(f64::NEG_INFINITY) && t1 >= -1e9
}

// --- sensor behavior -------------------------------------------------------------------------

#[derive(Clone)]
struct SensorState {
    mv: Matrix4,
    inv_mv: Matrix4,
    projection: Matrix4,
    viewport: Vector4,
    activate_hit: Option<Hit>,
    motion_hit: Option<Hit>,
    // PlaneSensor
    plane_sensor: bool,
    plane: Plane3,
    line: Line3,
    start_offset_v: Vector3,
    start_point: Vector3,
    // CylinderSensor
    cylinder: Cylinder3,
    disk: bool,
    y_plane: Plane3,
    z_plane: Plane3,
    sx_plane: Plane3,
    sz_normal: Vector3,
    behind: bool,
    from_vector: Vector3,
    start_offset_r: Rotation4,
    angle: f64,
    start_vector: Vector3,
    // SphereSensor
    sphere: Sphere3,
}

impl SensorState {
    fn new() -> SensorState {
        SensorState {
            mv: Matrix4::IDENTITY,
            inv_mv: Matrix4::IDENTITY,
            projection: Matrix4::IDENTITY,
            viewport: Vector4::ZERO,
            activate_hit: None,
            motion_hit: None,
            plane_sensor: true,
            plane: Plane3::new(Vector3::ZERO, Vector3::Z_AXIS),
            line: Line3::new(Vector3::ZERO, Vector3::Y_AXIS),
            start_offset_v: Vector3::ZERO,
            start_point: Vector3::ZERO,
            cylinder: Cylinder3::new(Line3::new(Vector3::ZERO, Vector3::Y_AXIS), 0.0),
            disk: false,
            y_plane: Plane3::new(Vector3::ZERO, Vector3::Y_AXIS),
            z_plane: Plane3::new(Vector3::ZERO, Vector3::Z_AXIS),
            sx_plane: Plane3::new(Vector3::ZERO, Vector3::X_AXIS),
            sz_normal: Vector3::Z_AXIS,
            behind: false,
            from_vector: Vector3::ZERO,
            start_offset_r: Rotation4::identity(),
            angle: 0.0,
            start_vector: Vector3::ZERO,
            sphere: Sphere3::new(0.0, Vector3::ZERO),
        }
    }
}

/// Output of the browser for the UI.
#[derive(Default)]
pub struct PointerOutput {
    pub anchors: Vec<AnchorActivation>,
    pub description: Option<String>,
}

pub struct Pointer {
    over: Vec<(NodeRef, SensorRef)>,
    active: Vec<(NodeRef, SensorRef)>,
    states: Vec<(NodeRef, SensorState)>,
    anchor_over: std::collections::HashMap<(Vec<NodeId>, NodeId), bool>,
    anchor_active: std::collections::HashMap<(Vec<NodeId>, NodeId), bool>,
    pub hit: Hit,
    pub out: PointerOutput,
}

type FKey = (NodeId, x3d_scene::FieldLoc);

fn field(rt: &Runtime, n: NodeId, name: &str) -> Option<FKey> {
    rt.pre(n, name)
}

fn set(rt: &mut Runtime, n: NodeId, name: &str, v: FieldValue) {
    if let Some(k) = field(rt, n, name) {
        rt.set_value(k, v);
    }
}

fn get_v3(rt: &Runtime, n: NodeId, name: &str) -> Vector3 {
    match rt.pre_value(n, name) {
        Some(FieldValue::SFVec3f(v)) => *v,
        _ => Vector3::ZERO,
    }
}

fn get_v2(rt: &Runtime, n: NodeId, name: &str) -> Vector2 {
    match rt.pre_value(n, name) {
        Some(FieldValue::SFVec2f(v)) => *v,
        _ => Vector2::ZERO,
    }
}

fn get_rot(rt: &Runtime, n: NodeId, name: &str) -> Rotation4 {
    match rt.pre_value(n, name) {
        Some(FieldValue::SFRotation(v)) => *v,
        _ => Rotation4::identity(),
    }
}

fn get_f(rt: &Runtime, n: NodeId, name: &str) -> f64 {
    rt.get_f64(n, name)
}

fn rot_vec(r: &Rotation4, v: Vector3) -> Vector3 {
    let mut v = v;
    r.mult_vec_rot(&mut v);
    v
}

fn mult_vec_matrix(m: &Matrix4, v: Vector3) -> Vector3 {
    let mut v = v;
    m.mult_vec_matrix3(&mut v);
    v
}

fn mult_dir_matrix(m: &Matrix4, v: Vector3) -> Vector3 {
    let mut v = v;
    m.mult_dir_matrix(&mut v);
    v
}

impl Pointer {
    pub fn new() -> Pointer {
        Pointer { over: vec![], active: vec![], states: vec![], anchor_over: Default::default(), anchor_active: Default::default(), hit: Hit::default(), out: PointerOutput::default() }
    }

    fn state(&mut self, n: &NodeRef) -> &mut SensorState {
        if let Some(i) = self.states.iter().position(|(k, _)| k == n) {
            return &mut self.states[i].1;
        }

        self.states.push((n.clone(), SensorState::new()));
        &mut self.states.last_mut().unwrap().1
    }

    pub fn is_active(&self) -> bool {
        !self.active.is_empty()
    }

    /// The runtime released `released` ([`Runtime::take_released`]): their ids may name nodes of
    /// another scene from now on. Every sensor, Anchor and hit this pointer remembers of them is
    /// dropped, without events: a drag or a press on a node that is gone ends with it, and the
    /// node that takes the slot starts inactive and not under the pointer. The other sensors keep
    /// their state.
    pub fn forget(&mut self, released: &[NodeId]) {
        if released.is_empty() {
            return;
        }

        let gone = |path: &[NodeId], n: NodeId| released.contains(&n) || path.iter().any(|p| released.contains(p));
        let dead = |r: &NodeRef| gone(&r.path, r.node);

        self.over.retain(|(n, _)| !dead(n));
        self.active.retain(|(n, _)| !dead(n));
        self.states.retain(|(n, _)| !dead(n));
        self.anchor_over.retain(|(path, n), _| !gone(path, *n));
        self.anchor_active.retain(|(path, n), _| !gone(path, *n));

        // The last pick is reused while navigating: it must not name a released node either.
        if self.hit.shape.as_ref().is_some_and(dead) {
            self.hit = Hit::default();
        } else {
            self.hit.sensors.retain(|s| !dead(&s.node));
            self.hit.anchors.retain(|a| !dead(a));
        }
    }

    /// X3DPointingDeviceSensorContext.buttonPressEvent. `hit` is the fresh pick.
    pub fn button_press(&mut self, root: &mut Runtime, hit: Hit, cam: &PickCamera, now: f64) -> bool {
        self.hit = hit;

        if !self.hit.id {
            return false;
        }

        let sensors = self.hit.sensors.clone();

        for s in sensors {
            if self.active.iter().any(|(k, _)| *k == s.node) {
                continue;
            }

            if !self.over.iter().any(|(k, _)| *k == s.node) {
                self.over.push((s.node.clone(), s.clone()));
            }

            self.active.push((s.node.clone(), s.clone()));
            self.set_active(root, &s, true, cam, now);
        }

        root.process_events();
        !self.hit.sensors.is_empty()
    }

    /// buttonReleaseEvent.
    pub fn button_release(&mut self, root: &mut Runtime, cam: &PickCamera, now: f64) {
        let active = std::mem::take(&mut self.active);

        for (_, s) in active {
            self.set_active(root, &s, false, cam, now);
        }

        root.process_events();
    }

    /// motionNotifyEvent: `hit` is the fresh pick (or, while a button is held, a pick whose ray
    /// still drives the active drag sensors).
    pub fn motion(&mut self, root: &mut Runtime, hit: Hit, cam: &PickCamera, now: f64) -> bool {
        self.hit = hit;
        self.motion_hit(root, cam, now);
        root.process_events();
        !self.hit.sensors.is_empty()
    }

    fn motion_hit(&mut self, root: &mut Runtime, cam: &PickCamera, now: f64) {
        let hit = self.hit.clone();

        // isOver FALSE for sensors no longer under the pointer.
        let over = std::mem::take(&mut self.over);
        let mut keep = vec![];

        for (n, s) in over {
            if hit.id && hit.sensors.iter().any(|h| h.node == n) {
                keep.push((n, s));
                continue;
            }

            self.set_over(root, &s, false, &hit, now);
        }

        self.over = keep;

        // isOver TRUE for the sensors under the pointer: `overSensors .set (node, sensor);
        // sensor .set_over__ (true, hit)` for every one, also those already over (a TouchSensor
        // sends its hit outputs again for the new hit).
        for s in &hit.sensors {
            match self.over.iter_mut().find(|(k, _)| *k == s.node) {
                Some(entry) => entry.1 = s.clone(),
                None => self.over.push((s.node.clone(), s.clone())),
            }

            self.set_over(root, s, true, &hit, now);
        }

        // Motion for the active drag sensors.
        for (_, s) in self.active.clone() {
            self.set_motion(root, &s, &hit, cam);
        }
    }

    fn ty(root: &Runtime, s: &SensorRef) -> Option<&'static str> {
        let rt = crate::walk::rt_at(root, &s.node.path)?;
        rt.builtin_name(s.node.node)
    }

    // X3DPointingDeviceSensorNode.set_over__ and the X3DTouchSensorNode override.
    fn set_over(&mut self, root: &mut Runtime, s: &SensorRef, over: bool, hit: &Hit, _now: f64) {
        let ty = Self::ty(root, s);
        let Some(rt) = rt_at_mut(root, &s.node.path) else { return };
        let n = s.node.node;
        let is_anchor = ty == Some("Anchor");

        // Anchor: its hidden TouchSensor has the Anchor's description as description.
        if is_anchor {
            let cur = self.anchor_over.get(&(s.node.path.to_vec(), n)).copied().unwrap_or(false);
            if cur == over {
                return;
            }
            self.anchor_over.insert((s.node.path.to_vec(), n), over);

            if over {
                if let Some(FieldValue::SFString(d)) = rt.pre_value(n, "description") {
                    self.out.description = Some(d.clone());
                }
            }
            return;
        }

        if rt.get_bool(n, "isOver") != over {
            set(rt, n, "isOver", FieldValue::SFBool(over));

            if over {
                if let Some(FieldValue::SFString(d)) = rt.pre_value(n, "description") {
                    self.out.description = Some(d.clone());
                }
            }
        }

        if matches!(ty, Some("TouchSensor" | "GeoTouchSensor")) && over && rt.get_bool(n, "isOver") {
            let mut inv = s.model_view;
            inv.inverse();
            let w = hit.tex_coord.w;
            let tc = Vector2::new(hit.tex_coord.x / w, hit.tex_coord.y / w);
            let mut normal = hit.normal;
            s.model_view.mult_matrix_dir(&mut normal);
            normal.normalize();
            let point = mult_vec_matrix(&inv, hit.point);

            set(rt, n, "hitTexCoord_changed", FieldValue::SFVec2f(tc));
            set(rt, n, "hitNormal_changed", FieldValue::SFVec3f(normal));
            set(rt, n, "hitPoint_changed", FieldValue::SFVec3f(point));
        }

        // GeoTouchSensor.set_over__: `if (this ._isOver .getValue ()) hitGeoCoord_changed = getGeoCoord (hitPoint_changed)`.
        if ty == Some("GeoTouchSensor") && rt.get_bool(n, "isOver") {
            let hit_point = match rt.pre_value(n, "hitPoint_changed") {
                Some(FieldValue::SFVec3f(v)) => *v,
                _ => Vector3::ZERO,
            };
            let geo = rt.geo_object(n).get_geo_coord(&hit_point);
            set(rt, n, "hitGeoCoord_changed", FieldValue::SFVec3d(geo));
        }
    }

    // set_active__ for all sensor types.
    fn set_active(&mut self, root: &mut Runtime, s: &SensorRef, active: bool, cam: &PickCamera, now: f64) {
        let ty = Self::ty(root, s);
        let hit = self.hit.clone();
        let key = (s.node.path.to_vec(), s.node.node);

        if ty == Some("Anchor") {
            let over = self.anchor_over.get(&key).copied().unwrap_or(false);
            let was_active = self.anchor_active.get(&key).copied().unwrap_or(false);

            if was_active == active {
                return;
            }

            self.anchor_active.insert(key, active);

            // The hidden TouchSensor: on release over the anchor, request the load.
            if over && !active {
                if let Some(rt) = crate::walk::rt_at(root, &s.node.path) {
                    let n = s.node.node;
                    let strings = |name: &str| match rt.pre_value(n, name) {
                        Some(FieldValue::MFString(v)) => v.clone(),
                        _ => vec![],
                    };
                    let description = match rt.pre_value(n, "description") {
                        Some(FieldValue::SFString(d)) => d.clone(),
                        _ => String::new(),
                    };
                    self.out.anchors.push(AnchorActivation { node: s.node.clone(), description, urls: strings("url"), parameters: strings("parameter") });
                }
            }
            return;
        }

        let Some(rt) = rt_at_mut(root, &s.node.path) else { return };
        let n = s.node.node;

        if rt.get_bool(n, "isActive") == active {
            return;
        }

        set(rt, n, "isActive", FieldValue::SFBool(active));

        match ty {
            Some("TouchSensor" | "GeoTouchSensor") => {
                if rt.get_bool(n, "enabled") && rt.get_bool(n, "isOver") && !active {
                    set(rt, n, "touchTime", FieldValue::SFTime(now));
                }
            }
            Some("PlaneSensor") | Some("CylinderSensor") | Some("SphereSensor") => {
                if active {
                    let st = self.state(&s.node);
                    st.activate_hit = Some(hit.clone());
                    st.motion_hit = None;
                    st.mv = s.model_view;
                    st.projection = cam.projection;
                    st.viewport = cam.viewport;
                    let mut inv = s.model_view;
                    inv.inverse();
                    st.inv_mv = inv;

                    let mut st = self.state(&s.node).clone();
                    match ty {
                        Some("PlaneSensor") => Self::plane_activate(rt, n, &mut st, &hit),
                        Some("CylinderSensor") => Self::cylinder_activate(rt, n, &mut st, &hit),
                        _ => Self::sphere_activate(rt, n, &mut st, &hit),
                    }
                    *self.state(&s.node) = st;
                } else if rt.get_bool(n, "autoOffset") {
                    match ty {
                        Some("PlaneSensor") => {
                            let v = get_v3(rt, n, "translation_changed");
                            set(rt, n, "offset", FieldValue::SFVec3f(v));
                        }
                        Some("CylinderSensor") => {
                            let mut r = get_rot(rt, n, "rotation_changed");
                            let st = self.state(&s.node).clone();
                            let a = Self::cylinder_angle(&st, &mut r);
                            set(rt, n, "offset", FieldValue::SFFloat(a));
                        }
                        _ => {
                            let r = get_rot(rt, n, "rotation_changed");
                            set(rt, n, "offset", FieldValue::SFRotation(r));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn set_motion(&mut self, root: &mut Runtime, s: &SensorRef, hit: &Hit, _cam: &PickCamera) {
        let ty = Self::ty(root, s);
        let Some(rt) = rt_at_mut(root, &s.node.path) else { return };
        let n = s.node.node;
        let mut st = self.state(&s.node).clone();

        match ty {
            Some("PlaneSensor") => Self::plane_motion(rt, n, &mut st, hit),
            Some("CylinderSensor") => Self::cylinder_motion(rt, n, &mut st, hit),
            Some("SphereSensor") => Self::sphere_motion(rt, n, &mut st, hit),
            _ => return,
        }

        *self.state(&s.node) = st;
    }

    // --- PlaneSensor ---------------------------------------------------------------------------

    fn line_track_point(st: &SensorState, pointer: &Vector2, line: &Line3, track_point: &mut Vector3) -> bool {
        let mut screen_line = Line2::new(Vector2::ZERO, Vector2::new(1.0, 0.0));
        ViewVolume::project_line(line, &st.mv, &st.projection, &st.viewport, &mut screen_line);
        let tp1 = screen_line.get_closest_point_to_point(pointer);
        let mut track_line = Line3::new(Vector3::ZERO, Vector3::new(0.0, 0.0, -1.0));
        ViewVolume::unproject_ray(tp1.x, tp1.y, &st.mv, &st.projection, &st.viewport, &mut track_line);
        line.get_closest_point_to_line(&track_line, track_point)
    }

    fn plane_activate(rt: &mut Runtime, n: NodeId, st: &mut SensorState, hit: &Hit) {
        let mut hit_ray = hit.ray.copy();
        hit_ray.mult_line_matrix(&st.inv_mv);
        let hit_point = mult_vec_matrix(&st.inv_mv, hit.point);
        let axis_rotation = get_rot(rt, n, "axisRotation");
        let (min, max) = (get_v2(rt, n, "minPosition"), get_v2(rt, n, "maxPosition"));

        if min.x == max.x {
            st.plane_sensor = false;
            let direction = rot_vec(&axis_rotation, Vector3::new(0.0, (max.y - min.y).abs(), 0.0));
            let mut d = direction;
            d.normalize();
            st.line = Line3::new(hit_point, d);
        } else if min.y == max.y {
            st.plane_sensor = false;
            let direction = rot_vec(&axis_rotation, Vector3::new((max.x - min.x).abs(), 0.0, 0.0));
            let mut d = direction;
            d.normalize();
            st.line = Line3::new(hit_point, d);
        } else {
            st.plane_sensor = true;
            st.plane = Plane3::new(hit_point, rot_vec(&axis_rotation, Vector3::new(0.0, 0.0, 1.0)));
        }

        if st.plane_sensor {
            let mut sp = Vector3::ZERO;

            if st.plane.intersects_line(&hit_ray, &mut sp) {
                st.start_point = sp;
                Self::plane_track_start(rt, n, st, sp);
            }
        } else {
            let mut sp = Vector3::ZERO;

            if Self::line_track_point(st, &hit.pointer, &st.line.copy(), &mut sp) {
                st.start_point = sp;
                let mut track_point = Vector3::ZERO;
                let l2 = Line3::new(st.line.direction, st.line.direction);

                if !Self::line_track_point(st, &hit.pointer, &l2, &mut track_point) {
                    track_point = sp;
                }

                Self::plane_track_start(rt, n, st, track_point);
            }
        }
    }

    fn plane_track_start(rt: &mut Runtime, n: NodeId, st: &mut SensorState, track_point: Vector3) {
        st.start_offset_v = get_v3(rt, n, "offset");
        set(rt, n, "trackPoint_changed", FieldValue::SFVec3f(track_point));
        let off = get_v3(rt, n, "offset");
        set(rt, n, "translation_changed", FieldValue::SFVec3f(off));
    }

    fn plane_motion(rt: &mut Runtime, n: NodeId, st: &mut SensorState, hit: &Hit) {
        st.motion_hit = Some(hit.clone());

        let result: Option<(Vector3, Vector3)> = if st.plane_sensor {
            let mut hit_ray = hit.ray.copy();
            hit_ray.mult_line_matrix(&st.inv_mv);
            let mut end = Vector3::ZERO;

            if st.plane.intersects_line(&hit_ray, &mut end) { Some((end, end)) } else { None }
        } else {
            let mut end = Vector3::ZERO;

            if Self::line_track_point(st, &hit.pointer, &st.line.copy(), &mut end) {
                let mut tp = Vector3::ZERO;
                let l2 = Line3::new(Vector3::ZERO, st.line.direction);

                if !Self::line_track_point(st, &hit.pointer, &l2, &mut tp) {
                    tp = end;
                }

                Some((end, tp))
            } else {
                None
            }
        };

        match result {
            Some((end, track_point)) => Self::plane_track(rt, n, st, end, track_point),
            None => {
                // this ._trackPoint_changed .addEvent (); this ._translation_changed .addEvent ();
                if let Some(k) = field(rt, n, "trackPoint_changed") {
                    rt.add_event(k);
                }
                if let Some(k) = field(rt, n, "translation_changed") {
                    rt.add_event(k);
                }
            }
        }
    }

    fn plane_track(rt: &mut Runtime, n: NodeId, st: &mut SensorState, end_point: Vector3, track_point: Vector3) {
        let axis_rotation = get_rot(rt, n, "axisRotation");
        let mut inv_ar = axis_rotation;
        inv_ar.inverse();
        let mut e = end_point;
        e.add(&st.start_offset_v).subtract(&st.start_point);
        let mut translation = rot_vec(&inv_ar, e);
        let (min, max) = (get_v2(rt, n, "minPosition"), get_v2(rt, n, "maxPosition"));

        if !(min.x > max.x) {
            translation.x = translation.x.max(min.x).min(max.x);
        }

        if !(min.y > max.y) {
            translation.y = translation.y.max(min.y).min(max.y);
        }

        let translation = rot_vec(&axis_rotation, translation);

        if !get_v3(rt, n, "trackPoint_changed").equals(&track_point) {
            set(rt, n, "trackPoint_changed", FieldValue::SFVec3f(track_point));
        }

        if !get_v3(rt, n, "translation_changed").equals(&translation) {
            set(rt, n, "translation_changed", FieldValue::SFVec3f(translation));
        }
    }

    // --- CylinderSensor ------------------------------------------------------------------------

    fn cylinder_is_behind(st: &SensorState, hit_ray: &Line3, hit_point: &Vector3) -> bool {
        let (mut enter, mut exit) = (Vector3::ZERO, Vector3::ZERO);
        st.cylinder.intersects_line(hit_ray, &mut enter, &mut exit);
        hit_point.distance(&enter) > hit_point.distance(&exit)
    }

    fn cylinder_track_point(st: &SensorState, hit_ray: &Line3) -> Vector3 {
        let mut z_point = Vector3::ZERO;
        st.z_plane.intersects_line(hit_ray, &mut z_point);
        let mut axis_point = z_point;
        axis_point.add(&st.cylinder.axis.get_perpendicular_vector_to_point(&z_point));
        let distance = st.sx_plane.get_distance_to_point(&z_point) / st.cylinder.radius;
        let section = ((distance + 1.0) / 2.0).floor();
        let sinp = interval(distance, -1.0, 1.0);
        let phi = if section == 0.0 { sinp.asin() } else { sinp * std::f64::consts::PI / 2.0 };
        let angle = phi + section * std::f64::consts::PI;
        let d = st.cylinder.axis.direction;
        let rotation = Rotation4::new(d.x, d.y, d.z, angle);
        let mut tp = st.sz_normal;
        tp.multiply(st.cylinder.radius);
        rotation.mult_vec_rot(&mut tp);
        tp.add(&axis_point);
        tp
    }

    fn cylinder_angle(st: &SensorState, rotation: &mut Rotation4) -> f64 {
        if rotation.get_axis().dot(&st.cylinder.axis.direction) > 0.0 { rotation.angle() } else { -rotation.angle() }
    }

    fn cylinder_activate(rt: &mut Runtime, n: NodeId, st: &mut SensorState, hit: &Hit) {
        let mut hit_ray = hit.ray.copy();
        hit_ray.mult_line_matrix(&st.inv_mv);
        let hit_point = mult_vec_matrix(&st.inv_mv, hit.point);
        let axis_rotation = get_rot(rt, n, "axisRotation");
        let y_axis = rot_vec(&axis_rotation, Vector3::new(0.0, 1.0, 0.0));
        let mut camera_back = mult_dir_matrix(&st.inv_mv, Vector3::new(0.0, 0.0, 1.0));
        camera_back.normalize();

        let mut ya = y_axis;
        ya.normalize();
        let axis = Line3::new(Vector3::ZERO, ya);
        let radius = axis.get_perpendicular_vector_to_point(&hit_point).norm();

        st.cylinder = Cylinder3::new(axis, radius);
        st.disk = camera_back.dot(&y_axis).abs() > get_f(rt, n, "diskAngle").cos();
        st.behind = Self::cylinder_is_behind(st, &hit_ray, &hit_point);
        st.y_plane = Plane3::new(hit_point, y_axis);
        st.z_plane = Plane3::new(hit_point, camera_back);

        let billboard_to_viewer = Vector3::new(st.inv_mv.0[12], st.inv_mv.0[13], st.inv_mv.0[14]);
        let mut sx_normal = y_axis;
        sx_normal.cross(&billboard_to_viewer).normalize();
        st.sx_plane = Plane3::new(Vector3::ZERO, sx_normal);
        let mut sz = sx_normal;
        sz.cross(&y_axis).normalize();
        st.sz_normal = sz;

        let track_point = if st.disk {
            let mut tp = Vector3::ZERO;
            st.y_plane.intersects_line(&hit_ray, &mut tp);
            tp
        } else {
            Self::cylinder_track_point(st, &hit_ray)
        };

        let mut fv = st.cylinder.axis.get_perpendicular_vector_to_point(&track_point);
        fv.negate();
        st.from_vector = fv;
        let offset = get_f(rt, n, "offset");
        st.start_offset_r = Rotation4::new(y_axis.x, y_axis.y, y_axis.z, offset);

        set(rt, n, "trackPoint_changed", FieldValue::SFVec3f(track_point));
        set(rt, n, "rotation_changed", FieldValue::SFRotation(st.start_offset_r));

        st.angle = offset;
        let rc = get_rot(rt, n, "rotation_changed");
        st.start_vector = rot_vec(&rc, rot_vec(&axis_rotation, Vector3::new(0.0, 0.0, 1.0)));
    }

    fn cylinder_motion(rt: &mut Runtime, n: NodeId, st: &mut SensorState, hit: &Hit) {
        st.motion_hit = Some(hit.clone());

        let mut hit_ray = hit.ray.copy();
        hit_ray.mult_line_matrix(&st.inv_mv);

        let track_point = if st.disk {
            let mut tp = Vector3::ZERO;
            st.y_plane.intersects_line(&hit_ray, &mut tp);
            tp
        } else {
            Self::cylinder_track_point(st, &hit_ray)
        };

        set(rt, n, "trackPoint_changed", FieldValue::SFVec3f(track_point));

        let mut to_vector = st.cylinder.axis.get_perpendicular_vector_to_point(&track_point);
        to_vector.negate();
        let mut rotation = Rotation4::identity();
        rotation.set_vectors(&st.from_vector, &to_vector);

        if st.disk {
            let tp = mult_vec_matrix(&st.mv, track_point);

            if tp.z > 0.0 {
                let pn = st.y_plane.normal;
                rotation.mult_right(&Rotation4::new(pn.x, pn.y, pn.z, std::f64::consts::PI));
            }
        } else if st.behind {
            rotation.inverse();
        }

        rotation.mult_left(&st.start_offset_r);

        let (min_a, max_a) = (get_f(rt, n, "minAngle"), get_f(rt, n, "maxAngle"));

        if min_a > max_a {
            set(rt, n, "rotation_changed", FieldValue::SFRotation(rotation));
        } else {
            let axis_rotation = get_rot(rt, n, "axisRotation");
            let end_vector = rot_vec(&rotation, rot_vec(&axis_rotation, Vector3::new(0.0, 0.0, 1.0)));
            let mut delta = Rotation4::identity();
            delta.set_vectors(&st.start_vector, &end_vector);
            let axis = rot_vec(&axis_rotation, Vector3::new(0.0, 1.0, 0.0));
            let sign = if axis.dot(&delta.get_axis()) > 0.0 { 1.0 } else { -1.0 };

            st.angle += sign * delta.angle();
            st.start_vector = end_vector;

            let d = st.cylinder.axis.direction;
            let a = if st.angle < min_a {
                min_a
            } else if st.angle > max_a {
                max_a
            } else {
                st.angle
            };
            rotation = Rotation4::new(d.x, d.y, d.z, a);

            if !get_rot(rt, n, "rotation_changed").equals(&rotation) {
                set(rt, n, "rotation_changed", FieldValue::SFRotation(rotation));
            }
        }
    }

    // --- SphereSensor --------------------------------------------------------------------------

    fn sphere_track_point(st: &SensorState, hit_ray: &Line3, track_point: &mut Vector3, behind: bool) -> bool {
        let mut exit = Vector3::ZERO;

        if st.sphere.intersects_line(hit_ray, track_point, &mut exit) {
            let closer_exit = hit_ray.point.distance(&exit) < hit_ray.point.distance(track_point);

            if (closer_exit as i32 - behind as i32) != 0 {
                *track_point = exit;
            }

            return true;
        }

        false
    }

    fn sphere_activate(rt: &mut Runtime, n: NodeId, st: &mut SensorState, hit: &Hit) {
        let hit_point = mult_vec_matrix(&st.inv_mv, hit.point);
        let center = Vector3::ZERO;
        let mut zn = mult_dir_matrix(&st.inv_mv, Vector3::new(0.0, 0.0, 1.0));
        zn.normalize();

        st.z_plane = Plane3::new(center, zn);
        st.sphere = Sphere3::new(hit_point.norm(), center);
        st.behind = st.z_plane.get_distance_to_point(&hit_point) < 0.0;
        st.from_vector = hit_point;
        st.start_point = hit_point;
        st.start_offset_r = get_rot(rt, n, "offset");

        set(rt, n, "trackPoint_changed", FieldValue::SFVec3f(hit_point));
        let off = get_rot(rt, n, "offset");
        set(rt, n, "rotation_changed", FieldValue::SFRotation(off));
    }

    fn sphere_motion(rt: &mut Runtime, n: NodeId, st: &mut SensorState, hit: &Hit) {
        let mut hit_ray = hit.ray.copy();
        hit_ray.mult_line_matrix(&st.inv_mv);
        let mut track_point = Vector3::ZERO;

        if Self::sphere_track_point(st, &hit_ray, &mut track_point, st.behind) {
            let mut z_axis = mult_dir_matrix(&st.inv_mv, Vector3::new(0.0, 0.0, 1.0));
            z_axis.normalize();
            st.z_plane = Plane3::new(track_point, z_axis);
        } else {
            let mut tangent = Vector3::ZERO;
            st.z_plane.intersects_line(&hit_ray, &mut tangent);

            let mut dir = st.sphere.center;
            dir.subtract(&tangent).normalize();
            hit_ray.set(&tangent, &dir);

            Self::sphere_track_point(st, &hit_ray, &mut track_point, false);

            let mut tri_normal = Vector3::ZERO;
            Triangle3::normal(&st.sphere.center, &track_point, &st.start_point, &mut tri_normal);
            let mut dir_from_center = track_point;
            dir_from_center.subtract(&st.sphere.center).normalize();
            let mut normal = tri_normal;
            normal.cross(&dir_from_center).normalize();

            let mut tmp = tangent;
            tmp.subtract(&track_point);
            let mut off = normal;
            off.multiply(tmp.norm());
            let mut point1 = track_point;
            point1.subtract(&off);

            let mut dir = st.sphere.center;
            dir.subtract(&point1).normalize();
            hit_ray.set(&point1, &dir);

            Self::sphere_track_point(st, &hit_ray, &mut track_point, false);
        }

        set(rt, n, "trackPoint_changed", FieldValue::SFVec3f(track_point));

        let mut to_vector = track_point;
        to_vector.subtract(&st.sphere.center);
        let mut rotation = Rotation4::identity();
        rotation.set_vectors(&st.from_vector, &to_vector);

        if st.behind {
            rotation.inverse();
        }

        let mut r = st.start_offset_r;
        r.mult_right(&rotation);
        set(rt, n, "rotation_changed", FieldValue::SFRotation(r));
    }
}

#[allow(dead_code)]
fn _m3(_: &Matrix3) {}
