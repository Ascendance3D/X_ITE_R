//! The contact test of Browser/Picking/VolumePicker.js for PointPickSensor, PrimitivePickSensor and
//! VolumePickSensor. X_ITE builds Ammo (Bullet) shapes with CollidableShape (`convex TRUE`) and
//! asks `contactTest`: does one manifold point have `distance <= 0`. This module builds the shapes
//! and keeps the Ammo state between frames; [`crate::bullet`] is the collision pipeline.
//!
//! - CollidableShape.set_collidableGeometry__: Box → btBoxShape, Sphere → btSphereShape, Cone
//!   with side and bottom → btConeShape, Cylinder with side, top and bottom → btCylinderShape,
//!   other Cone / Cylinder → btBvhTriangleMeshShape of the triangles, ElevationGrid (both
//!   dimensions > 1) → btHeightfieldTerrainShape (flipQuadEdges), other geometry → btConvexHullShape
//!   of the vertices. Point and line geometry (getGeometryType () <= 1) get no shape.
//! - Margins: btBoxShape / btCylinderShape keep the margin inside the size (setSafeMargin: at
//!   most a tenth of the smallest half extent), btConeShape / btConvexHullShape add 0.04 outside,
//!   a sphere is its center with the radius as margin, triangle meshes have none.
//! - setLocalScaling: box and cylinder scale the size with margin, a sphere uses the x scale, a
//!   cone scales its height by y and its radius by the mean ratio of x and z to the previous
//!   scaling (state kept per geometry node, as Ammo keeps the shape: X3DPickSensorNode.getPickShape
//!   caches one CollidableShape per geometry node for all sensors), hulls scale their points, a
//!   triangle mesh rebuilds its BVH only when its scaling changes by more than SIMD_EPSILON.
//! - Each sensor has its own VolumePicker. removeChildShapeByIndex never shrinks a btCompoundShape's
//!   local AABB, so each of the two compounds' AABB holds every shape the sensor ever placed in it,
//!   and the contact breaking threshold of the manifold (0.02 × the compounds' angular motion
//!   disc, the smaller one) grows with that history. The threshold decides which triangle contacts
//!   replace each other in the manifold.

use std::collections::HashMap;
use std::rc::Rc;
use x3d_fields::math::{Matrix4, Rotation4, Vector3};
use x3d_fields::{FieldValue, NodeId};
use x3d_render::geometry::Geometry;
use x3d_runtime::Runtime;

use crate::bullet::{self, Body, BvhMesh, Convex, HeightField, Transform, V, add, f, mul, sub};

/// CONVEX_DISTANCE_MARGIN.
const MARGIN: f64 = 0.04;

/// The shape CollidableShape builds for a geometry node, before scaling.
#[derive(Clone, Debug)]
pub enum BaseShape {
    Sphere {
        radius: f64,
    },
    /// btBoxShape: implicit (core) half extents and the margin (setSafeMargin).
    Box {
        implicit: V,
        margin: f64,
    },
    /// btCylinderShape (Y axis), like the box.
    Cylinder {
        implicit: V,
        margin: f64,
    },
    Cone {
        radius: f64,
        height: f64,
    },
    Hull {
        points: Vec<V>,
    },
    /// btBvhTriangleMeshShape of the triangles.
    Mesh {
        triangles: Vec<[V; 3]>,
    },
    /// btHeightfieldTerrainShape (xDimension, zDimension, heights, 1, min, max, 1, PHY_FLOAT,
    /// flipQuadEdges) with local scaling (xSpacing, 1, zSpacing) and CollidableShape's offset.
    HeightField {
        field: HeightField,
        offset: V,
    },
}

/// btBoxShape / btCylinderShape constructor (2.82): setSafeMargin (halfExtents) sets the margin to
/// min (0.04, 0.1 * smallest half extent), then implicit = halfExtents - margin.
fn safe(half: V) -> (V, f64) {
    let min = half[0].min(half[1]).min(half[2]);
    let safe = f(0.1 * min);
    let margin = if safe < f(MARGIN) { safe } else { f(MARGIN) };
    (half.map(|h| f(h - margin)), margin)
}

fn value<'a>(rt: &'a Runtime, n: NodeId, name: &str) -> Option<&'a FieldValue> {
    rt.pre_value(n, name)
}

fn num(rt: &Runtime, n: NodeId, name: &str, default: f64) -> f64 {
    match value(rt, n, name) {
        Some(FieldValue::SFFloat(v)) => *v,
        Some(FieldValue::SFDouble(v)) => *v,
        Some(FieldValue::SFInt32(v)) => *v as f64,
        _ => default,
    }
}

fn boolean(rt: &Runtime, n: NodeId, name: &str) -> bool {
    !matches!(value(rt, n, name), Some(FieldValue::SFBool(false)))
}

fn triangles(g: &Geometry) -> Vec<[V; 3]> {
    let vs = &g.vertices;
    (0..vs.len() / 12)
        .map(|t| {
            let p = |k: usize| {
                [
                    vs[t * 12 + k * 4] as f64,
                    vs[t * 12 + k * 4 + 1] as f64,
                    vs[t * 12 + k * 4 + 2] as f64,
                ]
            };
            [p(0), p(1), p(2)]
        })
        .collect()
}

impl BaseShape {
    /// btBoxShape (size / 2).
    pub fn new_box(size: V) -> BaseShape {
        let (implicit, margin) = safe([f(size[0] / 2.0), f(size[1] / 2.0), f(size[2] / 2.0)]);
        BaseShape::Box { implicit, margin }
    }

    /// btCylinderShape (radius, height / 2, radius).
    pub fn new_cylinder(radius: f64, height: f64) -> BaseShape {
        let r = f(radius);
        let (implicit, margin) = safe([r, f(height * 0.5), r]);
        BaseShape::Cylinder { implicit, margin }
    }

    /// btHeightfieldTerrainShape for an ElevationGrid (both dimensions > 1). Missing heights are 0.
    pub fn new_height_field(
        xd: i64,
        zd: i64,
        xs: f64,
        zs: f64,
        heights: &[f64],
    ) -> Option<BaseShape> {
        if xd <= 1 || zd <= 1 {
            return None;
        }

        let (min, max) = heights
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &h| {
                (a.min(h), b.max(h))
            });
        let (min, max) = (f(min), f(max));
        let (xs, zs) = (f(xs), f(zs));
        let (w, l) = ((xd - 1) as f64, (zd - 1) as f64);
        let field = HeightField {
            width: xd as usize,
            length: zd as usize,
            heights: (0..(xd * zd) as usize)
                .map(|i| f(heights.get(i).copied().unwrap_or(0.0)))
                .collect(),
            min,
            max,
            scaling: [xs, 1.0, zs],
        };
        Some(BaseShape::HeightField {
            field,
            offset: [f(xs * w * 0.5), f((min + max) * 0.5), f(zs * l * 0.5)],
        })
    }
}

/// CollidableShape.set_collidableGeometry__ with `convex TRUE` (X3DPickSensorNode.getPickShape).
/// `geometry` is the built geometry of the node (its vertices), `None` without vertices.
pub fn base_shape(rt: &Runtime, n: NodeId, geometry: Option<&Geometry>) -> Option<BaseShape> {
    let ty = rt.builtin_name(n)?;
    let g = geometry?;

    // getGeometryType () > 1 (the shape node is set up only for 2D and 3D geometry).
    if g.geometry_type <= 1 {
        return None;
    }

    match ty {
        "Box" => Some(BaseShape::new_box(match value(rt, n, "size") {
            Some(FieldValue::SFVec3f(v)) => [v.x, v.y, v.z],
            _ => [2.0, 2.0, 2.0],
        })),
        "Sphere" => Some(BaseShape::Sphere {
            radius: f(num(rt, n, "radius", 1.0)),
        }),
        "Cone" if boolean(rt, n, "side") && boolean(rt, n, "bottom") => Some(BaseShape::Cone {
            radius: f(num(rt, n, "bottomRadius", 1.0)),
            height: f(num(rt, n, "height", 2.0)),
        }),
        "Cylinder"
            if boolean(rt, n, "side") && boolean(rt, n, "top") && boolean(rt, n, "bottom") =>
        {
            Some(BaseShape::new_cylinder(
                num(rt, n, "radius", 1.0),
                num(rt, n, "height", 2.0),
            ))
        }
        "Cone" | "Cylinder" => {
            let t = triangles(g);
            (!t.is_empty()).then_some(BaseShape::Mesh { triangles: t })
        }
        "ElevationGrid" => {
            let heights: Vec<f64> = match value(rt, n, "height") {
                Some(FieldValue::MFFloat(v)) => v.iter().map(|&h| h as f64).collect(),
                _ => vec![],
            };
            BaseShape::new_height_field(
                num(rt, n, "xDimension", 0.0) as i64,
                num(rt, n, "zDimension", 0.0) as i64,
                num(rt, n, "xSpacing", 1.0),
                num(rt, n, "zSpacing", 1.0),
                &heights,
            )
        }
        _ => {
            let vs = &g.vertices;
            let points: Vec<V> = (0..vs.len() / 4)
                .map(|i| [vs[i * 4] as f64, vs[i * 4 + 1] as f64, vs[i * 4 + 2] as f64])
                .collect();
            (!points.is_empty()).then_some(BaseShape::Hull { points })
        }
    }
}

/// A btConeShape as Ammo keeps it between calls: setLocalScaling multiplies the radius by the
/// mean ratio of the x and z scalings to the previous ones.
#[derive(Clone, Debug)]
struct ConeState {
    base: (f64, f64),
    radius: f64,
    height: f64,
    scaling: V,
}

/// A btBvhTriangleMeshShape in its compound: the compound's scaling, the mesh's scaling (the last
/// one it accepted) and the BVH of the scaled triangles.
#[derive(Clone, Debug)]
struct MeshState {
    base: Vec<[V; 3]>,
    compound: V,
    child: V,
    mesh: Rc<BvhMesh>,
}

impl MeshState {
    fn new(base: &[[V; 3]]) -> MeshState {
        MeshState {
            base: base.to_vec(),
            compound: [1.0; 3],
            child: [1.0; 3],
            mesh: Rc::new(BvhMesh::new(scaled(base, [1.0; 3]))),
        }
    }

    /// btCompoundShape.setLocalScaling: the child gets childScale * scaling / m_localScaling, and
    /// btBvhTriangleMeshShape.setLocalScaling takes it (and rebuilds the BVH) only when it differs
    /// by more than SIMD_EPSILON (squared length).
    fn set_scaling(&mut self, s: V) {
        let child: V = std::array::from_fn(|i| f(self.child[i] * s[i] / self.compound[i]));

        if bullet::len2(sub(self.child, child)) > bullet::SIMD_EPSILON {
            self.child = child;
            self.mesh = Rc::new(BvhMesh::new(scaled(&self.base, child)));
        }

        self.compound = s;
    }
}

fn scaled(triangles: &[[V; 3]], s: V) -> Vec<[V; 3]> {
    triangles
        .iter()
        .map(|t| t.map(|p| [f(p[0] * s[0]), f(p[1] * s[1]), f(p[2] * s[2])]))
        .collect()
}

/// The two compounds of a sensor's VolumePicker: their local AABBs (never shrinking).
#[derive(Clone, Copy, Debug, Default)]
struct PickerState {
    compounds: [Option<(V, V)>; 2],
}

/// btCollisionShape.getContactBreakingThreshold (gContactBreakingThreshold) of a compound at the
/// identity: 0.02 × (bounding sphere radius + distance of its center).
fn threshold((lo, hi): (V, V)) -> f64 {
    let radius = bullet::len2(sub(hi, lo)).sqrt() * 0.5;
    let center = mul(add(lo, hi), 0.5);
    f((radius + bullet::len2(center).sqrt()) * bullet::CONTACT_BREAKING_THRESHOLD)
}

/// The pick shapes' and the pickers' Ammo state.
#[derive(Default)]
pub struct PickShapes {
    cones: HashMap<NodeId, ConeState>,
    meshes: HashMap<NodeId, MeshState>,
    pickers: HashMap<NodeId, PickerState>,
}

impl PickShapes {
    /// Drop the state of nodes that are gone (generation-safe ids are never reused).
    pub fn retain(&mut self, keep: impl Fn(NodeId) -> bool) {
        self.cones.retain(|n, _| keep(*n));
        self.meshes.retain(|n, _| keep(*n));
        self.pickers.retain(|n, _| keep(*n));
    }

    pub fn len(&self) -> usize {
        self.cones.len() + self.meshes.len() + self.pickers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// VolumePicker.setChildShape (matrix, compound): the compound gets the scale of `matrix`, the
    /// shape is placed at its translation and rotation.
    pub fn place(&mut self, node: NodeId, base: &BaseShape, matrix: &Matrix4) -> Placed {
        let mut t = Vector3::ZERO;
        let mut r = Rotation4::identity();
        let mut s = Vector3::ONE;
        matrix.get_transform(Some(&mut t), Some(&mut r), Some(&mut s), None, None);
        self.place_scaled(node, base, frame(&t, &r), [f(s.x), f(s.y), f(s.z)])
    }

    fn place_scaled(
        &mut self,
        node: NodeId,
        base: &BaseShape,
        frame: Transform,
        scale: V,
    ) -> Placed {
        let a = [scale[0].abs(), scale[1].abs(), scale[2].abs()];

        let (kind, child) = match base {
            BaseShape::Sphere { radius } => (
                Kind::Convex(Convex::Sphere {
                    radius: f(radius * a[0]),
                }),
                Transform::IDENTITY,
            ),
            BaseShape::Box { implicit, margin } => (
                Kind::Convex(Convex::Box {
                    half: scaled_implicit(*implicit, *margin, a),
                    margin: *margin,
                }),
                Transform::IDENTITY,
            ),
            BaseShape::Cylinder { implicit, margin } => (
                Kind::Convex(Convex::Cylinder {
                    half: scaled_implicit(*implicit, *margin, a),
                    margin: *margin,
                }),
                Transform::IDENTITY,
            ),
            BaseShape::Cone { radius, height } => {
                let st = self.cones.entry(node).or_insert(ConeState {
                    base: (*radius, *height),
                    radius: *radius,
                    height: *height,
                    scaling: [1.0; 3],
                });

                if st.base != (*radius, *height) {
                    *st = ConeState {
                        base: (*radius, *height),
                        radius: *radius,
                        height: *height,
                        scaling: [1.0; 3],
                    };
                }

                // btConeShape.setLocalScaling.
                st.height = f(st.height * (a[1] / st.scaling[1]));
                st.radius = f(st.radius * ((a[0] / st.scaling[0] + a[2] / st.scaling[2]) / 2.0));
                st.scaling = a;
                (
                    Kind::Convex(Convex::Cone {
                        radius: st.radius,
                        height: st.height,
                        margin: MARGIN,
                    }),
                    Transform::IDENTITY,
                )
            }
            BaseShape::Hull { points } => (
                Kind::Convex(Convex::Hull {
                    points: points
                        .iter()
                        .map(|p| [f(p[0] * a[0]), f(p[1] * a[1]), f(p[2] * a[2])])
                        .collect(),
                    margin: MARGIN,
                }),
                Transform::IDENTITY,
            ),
            BaseShape::Mesh { triangles } => {
                let st = self
                    .meshes
                    .entry(node)
                    .or_insert_with(|| MeshState::new(triangles));

                if st.base != *triangles {
                    // set_collidableGeometry__: a new shape at scaling 1, then the compound's
                    // previous scaling.
                    let previous = st.compound;
                    *st = MeshState::new(triangles);
                    st.set_scaling(previous);
                }

                st.set_scaling(scale);
                (Kind::Mesh(st.mesh.clone()), Transform::IDENTITY)
            }
            BaseShape::HeightField { field, offset } => {
                let mut field = field.clone();
                field.scaling = [
                    f(field.scaling[0] * scale[0]),
                    f(field.scaling[1] * scale[1]),
                    f(field.scaling[2] * scale[2]),
                ];
                (
                    Kind::Height(field),
                    Transform::translation([
                        f(offset[0] * scale[0]),
                        f(offset[1] * scale[1]),
                        f(offset[2] * scale[2]),
                    ]),
                )
            }
        };

        Placed::new(kind, frame, child)
    }

    /// PointPickSensor: a btSphereShape (0) at `point` in a compound with the scale of the
    /// sensor's model matrix.
    pub fn place_point(point: &Vector3, matrix: &Matrix4) -> Placed {
        let mut t = Vector3::ZERO;
        let mut r = Rotation4::identity();
        let mut s = Vector3::ONE;
        matrix.get_transform(Some(&mut t), Some(&mut r), Some(&mut s), None, None);
        let p = [f(point.x), f(point.y), f(point.z)];
        let center = [f(p[0] * f(s.x)), f(p[1] * f(s.y)), f(p[2] * f(s.z))];
        Placed::new(
            Kind::Convex(Convex::Sphere { radius: 0.0 }),
            frame(&t, &r),
            Transform::translation(center),
        )
    }

    /// VolumePicker.setChildShape1 (`slot` 0) / setChildShape2 (`slot` 1) of `sensor`'s picker: the
    /// compound's local AABB grows by the new child (`None`: an empty pick shape, nothing added).
    pub fn set_child(&mut self, sensor: NodeId, slot: usize, placed: Option<&Placed>) {
        let Some(p) = placed else { return };
        let (lo, hi) = p.outer_aabb();
        let c = &mut self.pickers.entry(sensor).or_default().compounds[slot];
        *c = Some(match *c {
            Some((l, h)) => (
                std::array::from_fn(|i| l[i].min(lo[i])),
                std::array::from_fn(|i| h[i].max(hi[i])),
            ),
            None => (lo, hi),
        });
    }

    /// VolumePicker.contactTest of `sensor`'s picker holding `a` and `b` (after set_child).
    pub fn contact(&self, sensor: NodeId, a: &Placed, b: &Placed) -> bool {
        self.manifold(sensor, a, b).is_some_and(|m| m.hit())
    }

    /// The manifold of the pair (`None` when the children's AABBs do not overlap).
    pub fn manifold(&self, sensor: NodeId, a: &Placed, b: &Placed) -> Option<bullet::Manifold> {
        let (ba, bb) = (a.body(), b.body());

        if !bullet::aabb_overlap(&ba.aabb(), &bb.aabb()) || !a.trees_overlap(b) {
            return None;
        }

        let st = self.pickers.get(&sensor).copied().unwrap_or_default();
        let th = |i: usize| st.compounds[i].map(threshold).unwrap_or(0.0);
        Some(bullet::pair_manifold(&ba, &bb, th(0).min(th(1)), box_box))
    }
}

/// btBoxShape / btCylinderShape.setLocalScaling from scaling 1: (implicit + margin) * s - margin.
fn scaled_implicit(implicit: V, margin: f64, s: V) -> V {
    [
        f((implicit[0] + margin) * s[0] - margin),
        f((implicit[1] + margin) * s[1] - margin),
        f((implicit[2] + margin) * s[2] - margin),
    ]
}

/// btTransform of VolumePicker.getTransform: the basis of `setTransform (translation, rotation)`.
fn frame(t: &Vector3, r: &Rotation4) -> Transform {
    let mut m = Matrix4::IDENTITY;
    m.set_transform(Some(t), Some(r), None, None, None);
    let m = m.0.map(f);
    Transform {
        basis: [[m[0], m[4], m[8]], [m[1], m[5], m[9]], [m[2], m[6], m[10]]],
        origin: [m[12], m[13], m[14]],
    }
}

#[derive(Clone, Debug)]
enum Kind {
    Convex(Convex),
    Mesh(Rc<BvhMesh>),
    Height(HeightField),
}

/// A shape placed in a picker: the outer compound's child transform (`frame`), the shape's
/// transform in the world, and the inner compound's local AABB.
#[derive(Clone, Debug)]
pub struct Placed {
    kind: Kind,
    frame: Transform,
    t: Transform,
    inner: (V, V),
}

impl Placed {
    fn new(kind: Kind, frame: Transform, child: Transform) -> Placed {
        let inner = match &kind {
            Kind::Convex(c) => c.aabb(&child),
            Kind::Mesh(m) => m.aabb(&child),
            Kind::Height(h) => h.aabb(&child),
        };
        let inner = (bullet::fv(inner.0), bullet::fv(inner.1));
        Placed {
            kind,
            frame,
            t: frame.mul(&child),
            inner,
        }
    }

    fn body(&self) -> Body<'_> {
        match &self.kind {
            Kind::Convex(c) => Body::Convex(c, self.t),
            Kind::Mesh(m) => Body::Mesh(m, self.t),
            Kind::Height(h) => Body::Height(h, self.t),
        }
    }

    /// btCompoundCompoundCollisionAlgorithm's dbvt walks: the outer compounds' leaves (the inner
    /// compounds at their frames, in the identity bodies' space), then the inner compounds' leaves
    /// (the shape's AABB in the inner space, `b`'s transformed by `a.frame^-1 * b.frame` with
    /// btTransformAabb). A non-uniformly scaled cylinder's AABB keeps its scaled z extent while
    /// its support function uses the x radius, so this walk can drop a pair whose shapes overlap.
    fn trees_overlap(&self, other: &Placed) -> bool {
        let (oa, ob) = (self.outer_aabb(), other.outer_aabb());

        if !bullet::aabb_overlap(&oa, &ob) {
            return false;
        }

        let xform = self.frame.inverse_times(&other.frame);
        let xform = Transform {
            basis: xform.basis.map(bullet::fv),
            origin: bullet::fv(xform.origin),
        };
        let (lo, hi) = xform.transform_aabb(other.inner.0, other.inner.1, 0.0);
        bullet::aabb_overlap(&self.inner, &(bullet::fv(lo), bullet::fv(hi)))
    }

    /// btCompoundShape.getAabb of the inner compound at the outer child transform (what
    /// addChildShape adds to the outer compound's local AABB).
    fn outer_aabb(&self) -> (V, V) {
        let (lo, hi) = self.frame.transform_aabb(self.inner.0, self.inner.1, 0.0);
        (bullet::fv(lo), bullet::fv(hi))
    }
}

/// dBoxBox2's separating axis tests on the boxes with margin: true when no axis separates them.
fn box_box(f1: &Transform, a: V, f2: &Transform, b: V) -> bool {
    let p = sub(f2.origin, f1.origin);
    let u = [f1.axis(0), f1.axis(1), f1.axis(2)];
    let v = [f2.axis(0), f2.axis(1), f2.axis(2)];
    let dot = bullet::dot;
    let pp = [dot(u[0], p), dot(u[1], p), dot(u[2], p)];
    let mut r = [[0.0; 3]; 3];

    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = dot(u[i], v[j]);
        }
    }

    let mut q = r.map(|row| row.map(f64::abs));

    for i in 0..3 {
        if pp[i].abs() - (a[i] + b[0] * q[i][0] + b[1] * q[i][1] + b[2] * q[i][2]) > 0.0 {
            return false;
        }
    }

    for j in 0..3 {
        if dot(v[j], p).abs() - (a[0] * q[0][j] + a[1] * q[1][j] + a[2] * q[2][j] + b[j]) > 0.0 {
            return false;
        }
    }

    for row in q.iter_mut() {
        for x in row.iter_mut() {
            *x += 1.0e-5;
        }
    }

    let (r, q) = (&r, &q);
    let edge = |e1: f64, e2: f64| e1.abs() - e2 > bullet::SIMD_EPSILON;
    // u1 x (v1, v2, v3), u2 x …, u3 x …
    let tests = [
        (
            pp[2] * r[1][0] - pp[1] * r[2][0],
            a[1] * q[2][0] + a[2] * q[1][0] + b[1] * q[0][2] + b[2] * q[0][1],
        ),
        (
            pp[2] * r[1][1] - pp[1] * r[2][1],
            a[1] * q[2][1] + a[2] * q[1][1] + b[0] * q[0][2] + b[2] * q[0][0],
        ),
        (
            pp[2] * r[1][2] - pp[1] * r[2][2],
            a[1] * q[2][2] + a[2] * q[1][2] + b[0] * q[0][1] + b[1] * q[0][0],
        ),
        (
            pp[0] * r[2][0] - pp[2] * r[0][0],
            a[0] * q[2][0] + a[2] * q[0][0] + b[1] * q[1][2] + b[2] * q[1][1],
        ),
        (
            pp[0] * r[2][1] - pp[2] * r[0][1],
            a[0] * q[2][1] + a[2] * q[0][1] + b[0] * q[1][2] + b[2] * q[1][0],
        ),
        (
            pp[0] * r[2][2] - pp[2] * r[0][2],
            a[0] * q[2][2] + a[2] * q[0][2] + b[0] * q[1][1] + b[1] * q[1][0],
        ),
        (
            pp[1] * r[0][0] - pp[0] * r[1][0],
            a[0] * q[1][0] + a[1] * q[0][0] + b[1] * q[2][2] + b[2] * q[2][1],
        ),
        (
            pp[1] * r[0][1] - pp[0] * r[1][1],
            a[0] * q[1][1] + a[1] * q[0][1] + b[0] * q[2][2] + b[2] * q[2][0],
        ),
        (
            pp[1] * r[0][2] - pp[0] * r[1][2],
            a[0] * q[1][2] + a[1] * q[0][2] + b[0] * q[2][1] + b[1] * q[2][0],
        ),
    ];

    !tests.iter().any(|&(e1, e2)| edge(e1, e2))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Away from the origin: a picker that only ever held a point at the origin has a contact
    /// breaking threshold of 0, and refreshContactPoints then drops every point.
    fn at(kind: Kind, t: V) -> Placed {
        Placed::new(
            kind,
            Transform::translation(add(t, [5.0, 0.0, 0.0])),
            Transform::IDENTITY,
        )
    }

    fn point(p: V) -> Placed {
        at(Kind::Convex(Convex::Sphere { radius: 0.0 }), p)
    }

    fn contact(a: &Placed, b: &Placed) -> bool {
        let mut shapes = PickShapes::default();
        shapes.set_child(1, 0, Some(a));
        shapes.set_child(1, 1, Some(b));
        shapes.contact(1, a, b)
    }

    fn convex(c: Convex) -> Kind {
        Kind::Convex(c)
    }

    #[test]
    fn point_against_margins() {
        let (implicit, margin) = safe([1.0, 1.0, 1.0]);
        let bx = at(
            convex(Convex::Box {
                half: implicit,
                margin,
            }),
            [0.0; 3],
        );
        assert!(contact(&point([0.5, 0.5, 0.5]), &bx));
        assert!(contact(&point([0.999, 0.0, 0.0]), &bx));
        assert!(!contact(&point([1.01, 0.0, 0.0]), &bx));
        // Rounded corner: (1, 1, 1) is outside the core (0.96) plus 0.04.
        assert!(!contact(&point([0.995, 0.995, 0.995]), &bx));

        let hull = at(
            convex(Convex::Hull {
                points: vec![
                    [-1.0, -1.0, -1.0],
                    [1.0, -1.0, -1.0],
                    [-1.0, 1.0, -1.0],
                    [1.0, 1.0, -1.0],
                    [-1.0, -1.0, 1.0],
                    [1.0, -1.0, 1.0],
                    [-1.0, 1.0, 1.0],
                    [1.0, 1.0, 1.0],
                ],
                margin: MARGIN,
            }),
            [0.0; 3],
        );
        assert!(contact(&point([1.03, 0.0, 0.0]), &hull));
        assert!(!contact(&point([1.05, 0.0, 0.0]), &hull));
        assert!(contact(&point([0.2, -0.3, 0.1]), &hull));

        let cone = at(
            convex(Convex::Cone {
                radius: 1.0,
                height: 2.0,
                margin: MARGIN,
            }),
            [0.0; 3],
        );
        assert!(contact(&point([0.0, 1.03, 0.0]), &cone));
        assert!(!contact(&point([0.0, 1.05, 0.0]), &cone));
        assert!(contact(&point([0.0, 0.0, 0.0]), &cone));
        assert!(!contact(&point([0.9, 0.9, 0.0]), &cone));
    }

    #[test]
    fn point_against_triangles() {
        let mesh = at(
            Kind::Mesh(Rc::new(BvhMesh::new(vec![[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
            ]]))),
            [0.0; 3],
        );
        assert!(contact(&point([0.2, 0.0, 0.2]), &mesh));
        // The flat mesh's AABB has no height: a point above it is not tested.
        assert!(!contact(&point([0.2, 0.0003, 0.2]), &mesh));
        let tilted = at(
            Kind::Mesh(Rc::new(BvhMesh::new(vec![[
                [0.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ]]))),
            [0.0; 3],
        );
        assert!(contact(&point([0.5, 0.5003, 0.2]), &tilted));
        assert!(!contact(&point([0.5, 0.501, 0.2]), &tilted));
        let sphere = at(convex(Convex::Sphere { radius: 0.5 }), [0.2, 0.4, 0.2]);
        assert!(contact(&sphere, &mesh));
    }

    #[test]
    fn boxes_and_gjk() {
        let (i1, m1) = safe([1.0, 1.0, 1.0]);
        let a = at(
            convex(Convex::Box {
                half: i1,
                margin: m1,
            }),
            [0.0; 3],
        );
        let b = at(
            convex(Convex::Box {
                half: i1,
                margin: m1,
            }),
            [1.99, 0.0, 0.0],
        );
        let c = at(
            convex(Convex::Box {
                half: i1,
                margin: m1,
            }),
            [2.01, 0.0, 0.0],
        );
        assert!(contact(&a, &b));
        assert!(!contact(&a, &c));
        let cyl = at(
            convex(Convex::Cylinder {
                half: i1,
                margin: m1,
            }),
            [2.05, 0.0, 0.0],
        );
        // Box core 0.96 + cylinder core 0.96 + margins 0.08 = 2.0 < 2.05.
        assert!(!contact(&a, &cyl));
        let cyl = at(
            convex(Convex::Cylinder {
                half: i1,
                margin: m1,
            }),
            [1.95, 0.0, 0.0],
        );
        assert!(contact(&a, &cyl));
        // Deep penetration goes through EPA.
        let cyl = at(
            convex(Convex::Cylinder {
                half: i1,
                margin: m1,
            }),
            [0.3, 0.1, 0.0],
        );
        assert!(contact(&a, &cyl));
    }

    #[test]
    fn manifold_keeps_four_points() {
        let mut m = bullet::Manifold::new(0.5);
        for i in 0..6 {
            m.add_contact_point(
                [0.0, 1.0, 0.0],
                [i as f64, 0.0, 0.0],
                0.01 * i as f64 - 0.02,
            );
        }
        assert_eq!(m.distances().len(), 4);
        // A point closer than the threshold to a cached one replaces it.
        m.add_contact_point([0.0, 1.0, 0.0], [0.1, 0.0, 0.0], 0.3);
        assert_eq!(m.distances().len(), 4);
    }
}
