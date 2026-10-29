//! Components/Picking: the pick sensors of the PICKING traversal ([`crate::walk::scan_picking`]
//! collects their model matrices and targets) and the sensors' `process`, which needs the geometry
//! of the targets. PointPickSensor, PrimitivePickSensor and VolumePickSensor use Ammo's contact
//! test in X_ITE ([`crate::pick_shapes`]).

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use x3d_fields::math::{Matrix4, Vector3};
use x3d_fields::{FieldValue, NodeId};
use x3d_render::geometry::Geometry;
use x3d_runtime::Runtime;

/// Browser/Picking/MatchCriterion.js.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MatchCriterion {
    Any,
    Every,
    OnlyOne,
}

/// A live pick sensor as the PICKING traversal sees it.
pub struct PickSensorInfo {
    pub node: NodeId,
    /// `this .objectType` (a Set).
    pub object_type: Vec<String>,
    pub match_criterion: MatchCriterion,
    /// `this .pickTargetNodes`.
    pub targets: HashSet<NodeId>,
}

/// X3DPickingContext [_pickSensorNodes] [0], read once per PICKING traversal.
#[derive(Default)]
pub struct PickInput {
    pub sensors: Vec<PickSensorInfo>,
}

fn string(rt: &Runtime, n: NodeId, name: &str) -> String {
    match rt.pre_value(n, name) {
        Some(FieldValue::SFString(s)) => s.clone(),
        _ => String::new(),
    }
}

impl PickInput {
    pub fn new(rt: &Runtime) -> PickInput {
        let sensors = rt
            .pick_sensors
            .iter()
            .map(|&s| PickSensorInfo {
                node: s,
                object_type: rt.pick_sensor_object_type(s),
                // set_matchCriterion__: unknown names are MATCH_ANY.
                match_criterion: match string(rt, s, "matchCriterion").as_str() {
                    "MATCH_EVERY" => MatchCriterion::Every,
                    "MATCH_ONLY_ONE" => MatchCriterion::OnlyOne,
                    _ => MatchCriterion::Any,
                },
                targets: rt.pick_sensor_targets(s),
            })
            .collect();

        PickInput { sensors }
    }
}

// --- vector helpers (X_ITE Vector3, double precision) -------------------------------------------

fn v(x: f64, y: f64, z: f64) -> Vector3 {
    Vector3::new(x, y, z)
}

fn add(a: &Vector3, b: &Vector3) -> Vector3 {
    v(a.x + b.x, a.y + b.y, a.z + b.z)
}

fn sub(a: &Vector3, b: &Vector3) -> Vector3 {
    v(a.x - b.x, a.y - b.y, a.z - b.z)
}

fn neg(a: &Vector3) -> Vector3 {
    v(-a.x, -a.y, -a.z)
}

fn dot(a: &Vector3, b: &Vector3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn cross(a: &Vector3, b: &Vector3) -> Vector3 {
    v(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

fn norm(a: &Vector3) -> f64 {
    dot(a, a).sqrt()
}

/// Vector3.normalize: a zero vector stays zero.
fn normalize(a: &Vector3) -> Vector3 {
    let l = norm(a);
    if l != 0.0 {
        v(a.x / l, a.y / l, a.z / l)
    } else {
        *a
    }
}

fn squared(a: &Vector3) -> f64 {
    dot(a, a)
}

fn mult_vec_matrix(m: &Matrix4, p: &Vector3) -> Vector3 {
    let mut p = *p;
    m.mult_vec_matrix3(&mut p);
    p
}

// --- Box3 (standard/Math/Geometry/Box3.js) and SAT ------------------------------------------------

/// Box3: an oriented box as a matrix (half axes in rows 0..2, center in row 3).
#[derive(Clone, Copy)]
struct Box3(Matrix4);

const AXES: [Vector3; 3] = [
    Vector3 {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    },
    Vector3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    },
    Vector3 {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    },
];

impl Box3 {
    /// setExtents.
    fn extents(min: &Vector3, max: &Vector3) -> Box3 {
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
        Box3(Matrix4([
            sx, 0.0, 0.0, 0.0, 0.0, sy, 0.0, 0.0, 0.0, 0.0, sz, 0.0, cx, cy, cz, 1.0,
        ]))
    }

    fn mult_right(mut self, m: &Matrix4) -> Box3 {
        self.0.mult_right(m);
        self
    }

    fn is_empty(&self) -> bool {
        self.0.0[15] == 0.0
    }

    fn axis(&self, i: usize) -> Vector3 {
        let m = &self.0.0;
        v(m[i * 4], m[i * 4 + 1], m[i * 4 + 2])
    }

    /// `center`: matrix.origin.
    fn center(&self) -> Vector3 {
        self.axis(3)
    }

    /// getPoints.
    fn points(&self) -> [Vector3; 8] {
        let (x, y, z) = (self.axis(0), self.axis(1), self.axis(2));
        let r1 = add(&y, &z);
        let r2 = sub(&z, &y);
        let p0 = add(&x, &r1);
        let p1 = sub(&r1, &x);
        let p2 = sub(&r2, &x);
        let p3 = add(&x, &r2);
        let c = self.center();
        [p0, p1, p2, p3, neg(&p2), neg(&p3), neg(&p0), neg(&p1)].map(|p| add(&p, &c))
    }

    /// getNormals: a zero axis is replaced by a perpendicular one first.
    fn normals(&self) -> [Vector3; 3] {
        let (mut x, mut y, mut z) = (self.axis(0), self.axis(1), self.axis(2));

        let fix = |a: &mut Vector3,
                   b: &Vector3,
                   c: &Vector3,
                   first: &Vector3,
                   second: &Vector3,
                   fallback: Vector3| {
            if squared(a) == 0.0 {
                *a = cross(b, c);

                if squared(a) == 0.0 {
                    for axis in &AXES {
                        *a = cross(axis, first);
                        if squared(a) != 0.0 {
                            break;
                        }
                    }

                    if squared(a) == 0.0 {
                        for axis in &AXES {
                            *a = cross(axis, second);
                            if squared(a) != 0.0 {
                                break;
                            }
                        }

                        if squared(a) == 0.0 {
                            *a = fallback;
                        }
                    }
                }
            }
        };

        let (y0, z0) = (y, z);
        fix(&mut x, &y0, &z0, &y0, &z0, AXES[0]);
        let (x1, z1) = (x, z);
        fix(&mut y, &z1, &x1, &z1, &x1, AXES[1]);
        let (x2, y2) = (x, y);
        fix(&mut z, &x2, &y2, &x2, &y2, AXES[2]);

        [
            normalize(&cross(&y, &z)),
            normalize(&cross(&z, &x)),
            normalize(&cross(&x, &y)),
        ]
    }

    /// intersectsBox.
    fn intersects_box(&self, other: &Box3) -> bool {
        if self.is_empty() || other.is_empty() {
            return false;
        }

        let (p1, p2) = (self.points(), other.points());

        if separated(&self.normals(), &p1, &p2) || separated(&other.normals(), &p1, &p2) {
            return false;
        }

        let mut axes9 = vec![];

        for i1 in 0..3 {
            for i2 in 0..3 {
                axes9.push(cross(&self.axis(i1), &other.axis(i2)));
            }
        }

        !separated(&axes9, &p1, &p2)
    }
}

/// SAT.isSeparated.
fn separated(axes: &[Vector3], p1: &[Vector3; 8], p2: &[Vector3; 8]) -> bool {
    let project = |points: &[Vector3; 8], axis: &Vector3| {
        let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
        for p in points {
            let d = dot(p, axis);
            if d < min {
                min = d;
            }
            if d > max {
                max = d;
            }
        }
        (min, max)
    };
    let between = |value: f64, lower: f64, upper: f64| lower <= value && value <= upper;

    for axis in axes {
        let (min1, max1) = project(p1, axis);
        let (min2, max2) = project(p2, axis);

        if !(between(min2, min1, max1) || between(min1, min2, max2)) {
            return true;
        }
    }

    false
}

// --- X3DPickSensorNode / LinePickSensor ------------------------------------------------------------

struct Intersection {
    tex_coord: [f64; 4],
    normal: Vector3,
    point: Vector3,
}

struct Target {
    geometry: NodeId,
    matrix: Matrix4,
    intersected: bool,
    distance: f64,
    intersections: Vec<Intersection>,
}

/// QuickSort (standard/Math/Algorithms/QuickSort.js) with `lhs .distance < rhs .distance`: the order
/// of equal distances is the one X_ITE's sort leaves.
fn quick_sort(a: &mut [usize], d: &dyn Fn(usize) -> f64) {
    fn qs(a: &mut [usize], d: &dyn Fn(usize) -> f64, lo: isize, hi: isize) {
        let (mut i, mut j) = (lo, hi);
        let x = d(a[((lo + hi) as usize) >> 1]);

        loop {
            while d(a[i as usize]) < x {
                i += 1;
            }
            while x < d(a[j as usize]) {
                j -= 1;
            }

            if i < j {
                a.swap(i as usize, j as usize);
                i += 1;
                j -= 1;
            } else {
                if i == j {
                    i += 1;
                    j -= 1;
                }
                break;
            }
        }

        if lo < j {
            qs(a, d, lo, j);
        }
        if i < hi {
            qs(a, d, i, hi);
        }
    }

    if a.len() > 1 {
        let hi = a.len() as isize - 1;
        qs(a, d, 0, hi);
    }
}

/// X3DGeometryNode.intersectsBBox: five of the box's planes (X_ITE does not test the left one),
/// infinite line.
fn intersects_bbox(g: &Geometry, point: &Vector3, dir: &Vector3) -> bool {
    let (min, max) = (g.min, g.max);
    let planes = [
        (max, v(0.0, 0.0, 1.0)),
        (min, v(0.0, 0.0, -1.0)),
        (max, v(0.0, 1.0, 0.0)),
        (min, v(0.0, -1.0, 0.0)),
        (max, v(1.0, 0.0, 0.0)),
    ];

    for (i, (p, normal)) in planes.iter().enumerate() {
        // Plane3.intersectsLine.
        let theta = dot(dir, normal);

        if theta == 0.0 {
            continue;
        }

        let t = (dot(normal, p) - dot(normal, point)) / theta;
        let x = v(
            dir.x * t + point.x,
            dir.y * t + point.y,
            dir.z * t + point.z,
        );

        let inside = match i {
            0 | 1 => x.x >= min.x && x.x <= max.x && x.y >= min.y && x.y <= max.y,
            2 | 3 => x.x >= min.x && x.x <= max.x && x.z >= min.z && x.z <= max.z,
            _ => x.y >= min.y && x.y <= max.y && x.z >= min.z && x.z <= max.z,
        };

        if inside {
            return true;
        }
    }

    false
}

/// X3DGeometryNode.intersectsLine (LinePickSensor.js): every triangle the infinite line meets,
/// with the barycentric point, normal and texture coordinate.
fn intersects_line(g: &Geometry, point: &Vector3, dir: &Vector3, out: &mut Vec<Intersection>) {
    if !intersects_bbox(g, point, dir) {
        return;
    }

    let vert = |k: usize| {
        v(
            g.vertices[k * 4] as f64,
            g.vertices[k * 4 + 1] as f64,
            g.vertices[k * 4 + 2] as f64,
        )
    };
    let tex = |k: usize, c: usize| g.tex_coords.get(k * 4 + c).copied().unwrap_or(0.0) as f64;
    let nor = |k: usize, c: usize| g.normals.get(k * 3 + c).copied().unwrap_or(0.0) as f64;

    for i in (0..g.vertex_count().saturating_sub(2)).step_by(3) {
        let (a, b, c) = (vert(i), vert(i + 1), vert(i + 2));

        // Line3.intersectsTriangle.
        let edge1 = sub(&b, &a);
        let edge2 = sub(&c, &a);
        let pvec = cross(dir, &edge2);
        let det = dot(&edge1, &pvec);

        if det == 0.0 {
            continue;
        }

        let inv = 1.0 / det;
        let tvec = sub(point, &a);
        let u = dot(&tvec, &pvec) * inv;

        if !(0.0..=1.0).contains(&u) {
            continue;
        }

        let qvec = cross(&tvec, &edge1);
        let vv = dot(dir, &qvec) * inv;

        if vv < 0.0 || u + vv > 1.0 {
            continue;
        }

        let (w0, w1, w2) = (1.0 - u - vv, u, vv);
        let point = v(
            w0 * a.x + w1 * b.x + w2 * c.x,
            w0 * a.y + w1 * b.y + w2 * c.y,
            w0 * a.z + w1 * b.z + w2 * c.z,
        );
        let tex_coord =
            [0, 1, 2, 3].map(|k| w0 * tex(i, k) + w1 * tex(i + 1, k) + w2 * tex(i + 2, k));
        let normal = v(
            w0 * nor(i, 0) + w1 * nor(i + 1, 0) + w2 * nor(i + 2, 0),
            w0 * nor(i, 1) + w1 * nor(i + 1, 1) + w2 * nor(i + 2, 1),
            w0 * nor(i, 2) + w1 * nor(i + 1, 2) + w2 * nor(i + 2, 2),
        );

        out.push(Intersection {
            tex_coord,
            normal,
            point,
        });
    }
}

fn set_if_changed(rt: &mut Runtime, n: NodeId, name: &str, value: FieldValue) {
    // X3DArrayField.equals: the same elements (float32 values, node identity).
    let same = match (rt.pre_value(n, name), &value) {
        (Some(FieldValue::MFVec3f(a)), FieldValue::MFVec3f(b)) => a == b,
        (Some(FieldValue::MFNode(a)), FieldValue::MFNode(b)) => a == b,
        _ => false,
    };

    if same {
        return;
    }

    if let Some(k) = rt.get_field(n, name) {
        rt.set_value(k, value);
    }
}

fn mf_vec3f(values: impl Iterator<Item = Vector3>) -> FieldValue {
    FieldValue::MFVec3f(
        values
            .flat_map(|p| [p.x as f32, p.y as f32, p.z as f32])
            .collect(),
    )
}

/// The geometry nodes LinePickSensor.process reads: the picking geometry of each sensor and every
/// collected target.
pub fn geometry_nodes(
    rt: &Runtime,
    sensors: &[PickSensorInfo],
    collected: &[(NodeId, NodeId, Matrix4)],
) -> Vec<NodeId> {
    let mut out: Vec<NodeId> = sensors
        .iter()
        .filter_map(|s| picking_geometry(rt, s.node))
        .collect();
    out.extend(collected.iter().map(|(_, g, _)| *g));
    out.sort_unstable();
    out.dedup();
    out
}

/// set_pickingGeometry__: LinePickSensor X3DCast (IndexedLineSet) ?? X3DCast (LineSet),
/// PointPickSensor X3DCast (PointSet), PrimitivePickSensor a Box, Cone, Cylinder or Sphere,
/// VolumePickSensor X3DCast (X3DGeometryNode).
fn picking_geometry(rt: &Runtime, sensor: NodeId) -> Option<NodeId> {
    let Some(FieldValue::SFNode(Some(n))) = rt.pre_value(sensor, "pickingGeometry") else {
        return None;
    };
    let n = rt.inner_node(*n)?;
    let ty = rt.builtin_name(n)?;

    let ok = match rt.builtin_name(sensor)? {
        "LinePickSensor" => matches!(ty, "IndexedLineSet" | "LineSet"),
        "PointPickSensor" => ty == "PointSet",
        "PrimitivePickSensor" => matches!(ty, "Box" | "Cone" | "Cylinder" | "Sphere"),
        // X3DGeometryNode: the node types whose container field is "geometry".
        _ => rt
            .scene
            .table
            .by_name
            .get(ty)
            .is_some_and(|&i| rt.scene.table.types[i].container_field == "geometry"),
    };

    ok.then_some(n)
}

/// LinePickSensor.process with the model matrices of the sensor and its collected targets
/// (geometry, model matrix), in traversal order. `geometry` maps a geometry node to its built
/// geometry (None: no vertices).
pub fn line_pick_process(
    rt: &mut Runtime,
    geometry: &HashMap<NodeId, Option<Rc<Geometry>>>,
    sensor: NodeId,
    model_matrices: &[Matrix4],
    collected: Vec<(NodeId, Matrix4)>,
) {
    let Some(pg_node) = picking_geometry(rt, sensor) else {
        return;
    };

    let empty = Geometry::default();
    let get = |n: NodeId| geometry.get(&n).cloned().flatten();
    let bounds = |g: &Option<Rc<Geometry>>| {
        g.as_ref()
            .map(|g| (g.min, g.max))
            .unwrap_or((Vector3::ZERO, Vector3::ZERO))
    };

    let pg = get(pg_node);
    let (pg_min, pg_max) = bounds(&pg);
    let bounds_mode = string(rt, sensor, "intersectionType") != "GEOMETRY";

    let mut targets: Vec<Target> = collected
        .into_iter()
        .map(|(geometry, matrix)| Target {
            geometry,
            matrix,
            intersected: false,
            distance: 0.0,
            intersections: vec![],
        })
        .collect();

    for model in model_matrices {
        let picking_box = Box3::extents(&pg_min, &pg_max).mult_right(model);

        for target in targets.iter_mut() {
            let tg = get(target.geometry);
            let (t_min, t_max) = bounds(&tg);
            let target_box = Box3::extents(&t_min, &t_max).mult_right(&target.matrix);

            if bounds_mode {
                if picking_box.intersects_box(&target_box) {
                    target.intersected = true;
                    target.distance = norm(&sub(&picking_box.center(), &target_box.center()));
                }

                continue;
            }

            // matrix .assign (target .modelMatrix) .inverse () .multLeft (modelMatrix).
            let mut matrix = target.matrix;
            matrix.inverse();
            matrix.mult_left(model);

            let lines = pg.as_deref().unwrap_or(&empty);
            let tg_geometry = tg.as_deref().unwrap_or(&empty);

            for s in (0..lines.vertices.len() / 8).map(|k| k * 8) {
                let vs = &lines.vertices;
                let point1 = mult_vec_matrix(
                    &matrix,
                    &v(vs[s] as f64, vs[s + 1] as f64, vs[s + 2] as f64),
                );
                let point2 = mult_vec_matrix(
                    &matrix,
                    &v(vs[s + 4] as f64, vs[s + 5] as f64, vs[s + 6] as f64),
                );
                let dir = normalize(&sub(&point2, &point1));
                let mut intersections = vec![];

                intersects_line(tg_geometry, &point1, &dir, &mut intersections);

                // The intersection point lies between point1 and point2.
                for intersection in intersections {
                    let a = sub(&intersection.point, &point1);
                    let b = sub(&intersection.point, &point2);
                    let c = norm(&add(&a, &b));
                    let s = norm(&sub(&point1, &point2));

                    if c <= s {
                        target.intersections.push(intersection);
                    }
                }
            }

            if !target.intersections.is_empty() {
                target.intersected = true;
                target.distance = norm(&sub(&picking_box.center(), &target_box.center()));
            }
        }
    }

    // getPickedGeometries.
    let mut picked: Vec<usize> = (0..targets.len())
        .filter(|&i| targets[i].intersected)
        .collect();
    let sort_order = string(rt, sensor, "sortOrder");
    let sort_order = match sort_order.as_str() {
        "ANY" | "CLOSEST" | "ALL" | "ALL_SORTED" => sort_order.as_str(),
        _ => "CLOSEST",
    };

    let geometries: Vec<Option<NodeId>> = if picked.is_empty() {
        vec![]
    } else {
        if matches!(sort_order, "CLOSEST" | "ALL_SORTED") {
            quick_sort(&mut picked, &|i| targets[i].distance);
        }

        match sort_order {
            "ANY" | "CLOSEST" => vec![Some(targets[picked[0]].geometry)],
            _ => picked.iter().map(|&i| Some(targets[i].geometry)).collect(),
        }
    };

    let active = !geometries.is_empty();

    if active != rt.get_bool(sensor, "isActive")
        && let Some(k) = rt.get_field(sensor, "isActive")
    {
        rt.set_value(k, FieldValue::SFBool(active));
    }

    set_if_changed(rt, sensor, "pickedGeometry", FieldValue::MFNode(geometries));

    if bounds_mode {
        return;
    }

    let all = || picked.iter().flat_map(|&i| targets[i].intersections.iter());
    let tex_coords = mf_vec3f(all().map(|x| {
        v(
            x.tex_coord[0] / x.tex_coord[3],
            x.tex_coord[1] / x.tex_coord[3],
            x.tex_coord[2] / x.tex_coord[3],
        )
    }));
    let normals = mf_vec3f(all().map(|x| x.normal));
    let points = mf_vec3f(all().map(|x| x.point));

    set_if_changed(rt, sensor, "pickedTextureCoordinate", tex_coords);
    set_if_changed(rt, sensor, "pickedNormal", normals);
    set_if_changed(rt, sensor, "pickedPoint", points);
}

/// getPickedGeometries: the intersected targets (sorted for CLOSEST and ALL_SORTED, in place as
/// X_ITE sorts `pickedTargets`), and the picked geometry nodes the sort order selects.
fn picked(
    rt: &Runtime,
    sensor: NodeId,
    intersected: Vec<usize>,
    distance: &dyn Fn(usize) -> f64,
    geometry: &dyn Fn(usize) -> NodeId,
) -> (Vec<usize>, Vec<Option<NodeId>>) {
    let mut picked = intersected;
    let sort_order = string(rt, sensor, "sortOrder");
    let sort_order = match sort_order.as_str() {
        "ANY" | "CLOSEST" | "ALL" | "ALL_SORTED" => sort_order.as_str(),
        _ => "CLOSEST",
    };

    if picked.is_empty() {
        return (picked, vec![]);
    }

    if matches!(sort_order, "CLOSEST" | "ALL_SORTED") {
        quick_sort(&mut picked, distance);
    }

    let geometries = match sort_order {
        "ANY" | "CLOSEST" => vec![Some(geometry(picked[0]))],
        _ => picked.iter().map(|&i| Some(geometry(i))).collect(),
    };

    (picked, geometries)
}

fn send_picked(rt: &mut Runtime, sensor: NodeId, geometries: Vec<Option<NodeId>>) {
    let active = !geometries.is_empty();

    if active != rt.get_bool(sensor, "isActive")
        && let Some(k) = rt.get_field(sensor, "isActive")
    {
        rt.set_value(k, FieldValue::SFBool(active));
    }

    set_if_changed(rt, sensor, "pickedGeometry", FieldValue::MFNode(geometries));
}

/// The points of the PointSet's coord (`coord .get1Point (i)` for `i < coord .getSize ()`).
fn coord_points(rt: &Runtime, point_set: NodeId) -> Vec<Vector3> {
    let Some(FieldValue::SFNode(Some(c))) = rt.pre_value(point_set, "coord") else {
        return vec![];
    };
    let Some(c) = rt.inner_node(*c) else {
        return vec![];
    };

    match rt.pre_value(c, "point") {
        Some(FieldValue::MFVec3f(p)) => p
            .chunks_exact(3)
            .map(|p| v(p[0] as f64, p[1] as f64, p[2] as f64))
            .collect(),
        Some(FieldValue::MFVec3d(p)) => p.chunks_exact(3).map(|p| v(p[0], p[1], p[2])).collect(),
        _ => vec![],
    }
}

/// PointPickSensor, PrimitivePickSensor and VolumePickSensor `process` with the model matrices
/// of the sensor and its collected targets (geometry, model matrix), in traversal order.
pub fn volume_pick_process(
    rt: &mut Runtime,
    shapes: &mut crate::pick_shapes::PickShapes,
    geometry: &HashMap<NodeId, Option<Rc<Geometry>>>,
    sensor: NodeId,
    model_matrices: &[Matrix4],
    collected: Vec<(NodeId, Matrix4)>,
) {
    use crate::pick_shapes::{PickShapes, base_shape};

    let Some(pg_node) = picking_geometry(rt, sensor) else {
        return;
    };
    let points_sensor = rt.builtin_name(sensor) == Some("PointPickSensor");

    let get = |n: NodeId| geometry.get(&n).cloned().flatten();
    let bounds = |g: &Option<Rc<Geometry>>| {
        g.as_ref()
            .map(|g| (g.min, g.max))
            .unwrap_or((Vector3::ZERO, Vector3::ZERO))
    };

    let pg = get(pg_node);
    let (pg_min, pg_max) = bounds(&pg);
    let bounds_mode = string(rt, sensor, "intersectionType") != "GEOMETRY";

    struct VTarget {
        geometry: NodeId,
        matrix: Matrix4,
        intersected: bool,
        distance: f64,
        picked_point: Vec<Vector3>,
    }

    let mut targets: Vec<VTarget> = collected
        .into_iter()
        .map(|(geometry, matrix)| VTarget {
            geometry,
            matrix,
            intersected: false,
            distance: 0.0,
            picked_point: vec![],
        })
        .collect();
    let points = if points_sensor {
        coord_points(rt, pg_node)
    } else {
        vec![]
    };
    let picking_base = if points_sensor {
        None
    } else {
        base_shape(rt, pg_node, pg.as_deref())
    };
    let target_bases: Vec<_> = targets
        .iter()
        .map(|t| base_shape(rt, t.geometry, get(t.geometry).as_deref()))
        .collect();

    for model in model_matrices {
        let picking_box = Box3::extents(&pg_min, &pg_max).mult_right(model);

        if bounds_mode {
            for target in targets.iter_mut() {
                let (t_min, t_max) = bounds(&get(target.geometry));
                let target_box = Box3::extents(&t_min, &t_max).mult_right(&target.matrix);

                if picking_box.intersects_box(&target_box) {
                    target.intersected = true;
                    target.distance = norm(&sub(&picking_box.center(), &target_box.center()));
                }
            }

            continue;
        }

        if points_sensor {
            for point in &points {
                let placed_point = PickShapes::place_point(point, model);
                shapes.set_child(sensor, 0, Some(&placed_point));

                for (target, base) in targets.iter_mut().zip(&target_bases) {
                    let Some(base) = base else { continue };
                    let placed = shapes.place(target.geometry, base, &target.matrix);
                    shapes.set_child(sensor, 1, Some(&placed));

                    if shapes.contact(sensor, &placed_point, &placed) {
                        let (t_min, t_max) = bounds(&get(target.geometry));
                        let target_box = Box3::extents(&t_min, &t_max).mult_right(&target.matrix);
                        target.intersected = true;
                        target.distance = norm(&sub(&picking_box.center(), &target_box.center()));
                        target.picked_point.push(*point);
                    }
                }
            }

            continue;
        }

        // setChildShape1 / setChildShape2 run (and scale the shapes) even when the picking
        // shape is empty.
        let placed_picker = picking_base
            .as_ref()
            .map(|b| shapes.place(pg_node, b, model));
        shapes.set_child(sensor, 0, placed_picker.as_ref());

        for (target, base) in targets.iter_mut().zip(&target_bases) {
            let Some(base) = base else { continue };
            let placed = shapes.place(target.geometry, base, &target.matrix);
            shapes.set_child(sensor, 1, Some(&placed));

            if placed_picker
                .as_ref()
                .is_some_and(|p| shapes.contact(sensor, p, &placed))
            {
                let (t_min, t_max) = bounds(&get(target.geometry));
                let target_box = Box3::extents(&t_min, &t_max).mult_right(&target.matrix);
                target.intersected = true;
                target.distance = norm(&sub(&picking_box.center(), &target_box.center()));
            }
        }
    }

    let intersected: Vec<usize> = (0..targets.len())
        .filter(|&i| targets[i].intersected)
        .collect();
    let (picked_targets, geometries) =
        picked(rt, sensor, intersected, &|i| targets[i].distance, &|i| {
            targets[i].geometry
        });

    send_picked(rt, sensor, geometries);

    if points_sensor && !bounds_mode {
        let points = mf_vec3f(
            picked_targets
                .iter()
                .flat_map(|&i| targets[i].picked_point.iter().copied()),
        );
        set_if_changed(rt, sensor, "pickedPoint", points);
    }
}
