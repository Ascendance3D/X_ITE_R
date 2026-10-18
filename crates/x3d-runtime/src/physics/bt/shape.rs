//! Collision shapes of Bullet 2.82 as X_ITE's RigidBodyPhysics creates them: btSphereShape,
//! btBoxShape, btCylinderShape (Y), btConeShape (Y), btCapsuleShape (Y), btConvexHullShape,
//! btBvhTriangleMeshShape, btHeightfieldTerrainShape and btCompoundShape (with its dynamic AABB
//! tree). Shapes live in an arena ([`Shapes`]) and are referenced by [`ShapeId`], like the raw
//! pointers of the C++ API.
//!
//! Members that Bullet stores in a `btScalar`/`btVector3` are rounded to float here.

use super::dbvt::{Aabb, Dbvt};
use super::math::*;
use crate::bullet::{BvhMesh, Convex, HeightField, Transform};

pub type ShapeId = usize;

/// CONVEX_DISTANCE_MARGIN.
pub const CONVEX_DISTANCE_MARGIN: f64 = 0.04_f32 as f64;
/// btScalar (0.1f), the default margin multiplier of setSafeMargin.
const SAFE_MARGIN_MULTIPLIER: f64 = 0.1_f32 as f64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeType {
    Box,
    Triangle,
    ConvexHull,
    Sphere,
    Capsule,
    Cone,
    Cylinder,
    TriangleMesh,
    Heightfield,
    Compound,
}

impl ShapeType {
    pub fn is_convex(self) -> bool {
        matches!(self, ShapeType::Box | ShapeType::Triangle | ShapeType::ConvexHull | ShapeType::Sphere | ShapeType::Capsule | ShapeType::Cone | ShapeType::Cylinder)
    }

    pub fn is_concave(self) -> bool {
        matches!(self, ShapeType::TriangleMesh | ShapeType::Heightfield)
    }

    pub fn is_compound(self) -> bool {
        self == ShapeType::Compound
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConvTy {
    Sphere,
    Box,
    Cylinder,
    Cone,
    Capsule,
    Hull,
}

/// A btConvexInternalShape.
#[derive(Clone, Debug)]
pub struct Conv {
    pub ty: ConvTy,
    /// m_localScaling.
    pub scaling: V3,
    /// m_collisionMargin.
    pub margin: f64,
    /// m_implicitShapeDimensions.
    pub implicit: V3,
    /// btConeShape: m_radius, m_height, m_sinAngle.
    pub cone: (f64, f64, f64),
    /// btConvexHullShape: unscaled points.
    pub points: Vec<V3>,
    /// btPolyhedralConvexAabbCachingShape: cached local AABB.
    pub laabb: (V3, V3),
}

impl Conv {
    fn base(ty: ConvTy) -> Conv {
        Conv { ty, scaling: [1.0; 3], margin: CONVEX_DISTANCE_MARGIN, implicit: [0.0; 3], cone: (0.0, 0.0, 0.0), points: vec![], laabb: ([1.0; 3], [-1.0; 3]) }
    }

    pub fn sphere(radius: f64) -> Conv {
        let mut c = Conv::base(ConvTy::Sphere);
        c.implicit[0] = f(radius);
        c.margin = f(radius);
        c
    }

    /// setSafeMargin (halfExtents).
    fn set_safe_margin(&mut self, half: V3) {
        let min_dimension = half[min_axis(half)];
        let safe = SAFE_MARGIN_MULTIPLIER * min_dimension;

        if safe < self.margin {
            self.margin = f(safe);
        }
    }

    pub fn box_(half: V3) -> Conv {
        let half = r3(half);
        let mut c = Conv::base(ConvTy::Box);
        c.set_safe_margin(half);
        let m = [c.margin; 3];
        c.implicit = r3(sub(mulv(half, c.scaling), m));
        c
    }

    pub fn cylinder(half: V3) -> Conv {
        let half = r3(half);
        let mut c = Conv::base(ConvTy::Cylinder);
        c.set_safe_margin(half);
        let m = [c.margin; 3];
        c.implicit = r3(sub(mulv(half, c.scaling), m));
        c
    }

    pub fn cone(radius: f64, height: f64) -> Conv {
        let (radius, height) = (f(radius), f(height));
        let mut c = Conv::base(ConvTy::Cone);
        let sin = f(radius / (radius * radius + height * height).sqrt());
        c.cone = (radius, height, sin);
        c.implicit = [radius, height, radius];
        c
    }

    pub fn capsule(radius: f64, height: f64) -> Conv {
        let mut c = Conv::base(ConvTy::Capsule);
        c.implicit = r3([radius, f(0.5_f32 as f64 * height), radius]);
        c
    }

    pub fn hull() -> Conv {
        Conv::base(ConvTy::Hull)
    }

    pub fn get_margin(&self) -> f64 {
        match self.ty {
            ConvTy::Sphere => f(self.implicit[0] * self.scaling[0]),
            _ => self.margin,
        }
    }

    /// The Convex of bullet.rs for GJK: scaled/implicit data as stored.
    pub fn core(&self) -> Convex {
        match self.ty {
            ConvTy::Sphere => Convex::Sphere { radius: self.get_margin() },
            ConvTy::Box => Convex::Box { half: self.implicit, margin: self.margin },
            ConvTy::Cylinder => Convex::Cylinder { half: self.implicit, margin: self.margin },
            ConvTy::Cone => Convex::Cone { radius: self.cone.0, height: self.cone.1, margin: self.margin },
            ConvTy::Capsule => Convex::Capsule { half: self.implicit, margin: self.margin },
            ConvTy::Hull => Convex::Hull { points: self.points.iter().map(|p| r3(mulv(*p, self.scaling))).collect(), margin: self.margin },
        }
    }

    /// btConvexHullShape/addPoint, then recalcLocalAabb.
    pub fn hull_add_point(&mut self, p: V3) {
        self.points.push(r3(p));
    }

    pub fn recalc_local_aabb(&mut self) {
        let core = self.core();
        let support = |d: V3| core.support(d);
        let lo = std::array::from_fn(|i| {
            let mut d = [0.0; 3];
            d[i] = -1.0;
            f(support(d)[i] - self.margin)
        });
        let hi = std::array::from_fn(|i| {
            let mut d = [0.0; 3];
            d[i] = 1.0;
            f(support(d)[i] + self.margin)
        });
        self.laabb = (lo, hi);
    }

    pub fn set_margin(&mut self, m: f64) {
        let m = f(m);
        match self.ty {
            ConvTy::Box | ConvTy::Cylinder | ConvTy::Capsule => {
                let old = [self.get_margin(); 3];
                let with_margin = add(self.implicit, old);
                self.margin = m;
                self.implicit = r3(sub(with_margin, [self.margin; 3]));
            }
            _ => self.margin = m,
        }
    }

    pub fn set_local_scaling(&mut self, s: V3) {
        let s = r3(s);
        match self.ty {
            ConvTy::Sphere => self.scaling = r3(abs3(s)),
            ConvTy::Box | ConvTy::Cylinder | ConvTy::Capsule => {
                let old = [self.get_margin(); 3];
                let with_margin = add(self.implicit, old);
                let unscaled = r3([with_margin[0] / self.scaling[0], with_margin[1] / self.scaling[1], with_margin[2] / self.scaling[2]]);
                self.scaling = r3(abs3(s));
                self.implicit = r3(sub(mulv(unscaled, self.scaling), old));
            }
            ConvTy::Cone => {
                // axis = 1, r1 = 0, r2 = 2
                let (mut radius, mut height, _) = self.cone;
                height = f(height * (s[1] / self.scaling[1]));
                radius = f(radius * ((s[0] / self.scaling[0] + s[2] / self.scaling[2]) / 2.0));
                let sin = f(radius / (radius * radius + height * height).sqrt());
                self.cone = (radius, height, sin);
                self.scaling = r3(abs3(s));
            }
            ConvTy::Hull => {
                self.scaling = s;
                self.recalc_local_aabb();
            }
        }
    }

    pub fn aabb(&self, t: &Tf) -> (V3, V3) {
        match self.ty {
            ConvTy::Sphere => {
                let e = [self.get_margin(); 3];
                (r3(sub(t.o, e)), r3(add(t.o, e)))
            }
            ConvTy::Box | ConvTy::Cylinder => transform_aabb_half(self.implicit, self.margin, t),
            ConvTy::Capsule => {
                let radius = self.implicit[0];
                let mut half = [radius; 3];
                // halfExtents lives in memory: its Y is rounded before the margin is added
                half[1] = f(radius + self.implicit[1]);
                let half = add(half, [self.margin; 3]);
                let ab = absolute(&t.b);
                let extent = m_mul_v(&ab, half);
                (r3(sub(t.o, extent)), r3(add(t.o, extent)))
            }
            ConvTy::Hull => transform_aabb_local(self.laabb.0, self.laabb.1, self.margin, t),
            ConvTy::Cone => self.cone_aabb_slow(t),
        }
    }

    /// btConvexInternalShape::getAabbSlow for the cone: the supporting vertices (with margin) are
    /// stored as floats, and so are their transformed positions.
    fn cone_aabb_slow(&self, t: &Tf) -> (V3, V3) {
        let core = self.core();
        let margin = core.margin();
        let tb = to_bt(t);
        let mut lo = ZERO;
        let mut hi = ZERO;

        for i in 0..3 {
            for (sign, out) in [(1.0, &mut hi), (-1.0, &mut lo)] {
                let mut d = [0.0; 3];
                d[i] = sign;
                let v = r3(tb.inv_rotate(d));
                let mut sv = r3(core.support(v));

                // localGetSupportingVertex: the margin along the normalized direction
                if margin != 0.0 {
                    let l2 = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
                    let n = if l2 < f32::EPSILON as f64 * f32::EPSILON as f64 { [-1.0; 3] } else { v };
                    let k = 1.0 / (n[2] * n[2] + (n[0] * n[0] + n[1] * n[1])).sqrt();
                    sv = r3([sv[0] + margin * (n[0] * k), sv[1] + margin * (n[1] * k), sv[2] + margin * (n[2] * k)]);
                }

                let tmp = r3(tb.apply(sv));
                out[i] = if sign > 0.0 { f(margin + tmp[i]) } else { f(tmp[i] - margin) };
            }
        }

        (lo, hi)
    }

    /// calculateLocalInertia.
    pub fn calculate_local_inertia(&self, mass: f64) -> V3 {
        match self.ty {
            ConvTy::Sphere => {
                let elem = 0.4_f32 as f64 * mass * self.get_margin() * self.get_margin();
                r3([elem; 3])
            }
            ConvTy::Box => {
                let half = add(self.implicit, [self.margin; 3]);
                let (lx, ly, lz) = (2.0 * half[0], 2.0 * half[1], 2.0 * half[2]);
                r3([mass / 12.0 * (ly * ly + lz * lz), mass / 12.0 * (lx * lx + lz * lz), mass / 12.0 * (lx * lx + ly * ly)])
            }
            ConvTy::Cylinder => {
                let half = add(self.implicit, [self.margin; 3]);
                let div12 = mass / 12.0;
                let div4 = mass / 4.0;
                let div2 = mass / 2.0;
                let radius2 = half[0] * half[0];
                let height2 = 4.0 * half[1] * half[1];
                let t1 = div12 * height2 + div4 * radius2;
                let t2 = div2 * radius2;
                r3([t1, t2, t1])
            }
            ConvTy::Capsule => {
                let radius = self.implicit[0];
                let mut half = [radius; 3];
                half[1] += self.implicit[1];
                let margin = CONVEX_DISTANCE_MARGIN;
                let (lx, ly, lz) = (2.0 * (half[0] + margin), 2.0 * (half[1] + margin), 2.0 * (half[2] + margin));
                let (x2, y2, z2) = (lx * lx, ly * ly, lz * lz);
                let scaled = mass * (0.08333333_f32 as f64);
                r3([scaled * (y2 + z2), scaled * (x2 + z2), scaled * (x2 + y2)])
            }
            ConvTy::Cone | ConvTy::Hull => {
                // btConeShape and btPolyhedralConvexShape::calculateLocalInertia.
                let margin = self.margin;
                let (lo, hi) = self.aabb(&Tf::IDENTITY);
                let half = scale(sub(hi, lo), 0.5);
                let (lx, ly, lz) = (2.0 * (half[0] + margin), 2.0 * (half[1] + margin), 2.0 * (half[2] + margin));
                let (x2, y2, z2) = (lx * lx, ly * ly, lz * lz);
                let scaled = mass * (0.08333333_f32 as f64);
                r3([scaled * (y2 + z2), scaled * (x2 + z2), scaled * (x2 + y2)])
            }
        }
    }

    pub fn shape_type(&self) -> ShapeType {
        match self.ty {
            ConvTy::Sphere => ShapeType::Sphere,
            ConvTy::Box => ShapeType::Box,
            ConvTy::Cylinder => ShapeType::Cylinder,
            ConvTy::Cone => ShapeType::Cone,
            ConvTy::Capsule => ShapeType::Capsule,
            ConvTy::Hull => ShapeType::ConvexHull,
        }
    }
}

/// btTransformAabb (halfExtents, margin, t).
pub fn transform_aabb_half(half: V3, margin: f64, t: &Tf) -> (V3, V3) {
    let with_margin = add(half, [margin; 3]);
    let ab = absolute(&t.b);
    let extent = m_mul_v(&ab, with_margin);
    (r3(sub(t.o, extent)), r3(add(t.o, extent)))
}

/// btTransformAabb (localAabbMin, localAabbMax, margin, trans).
pub fn transform_aabb_local(lmin: V3, lmax: V3, margin: f64, t: &Tf) -> (V3, V3) {
    let half = add(scale(sub(lmax, lmin), 0.5), [margin; 3]);
    let center = t.apply(scale(add(lmax, lmin), 0.5));
    let ab = absolute(&t.b);
    let extent = m_mul_v(&ab, half);
    (r3(sub(center, extent)), r3(add(center, extent)))
}

pub fn to_bt(t: &Tf) -> Transform {
    Transform { basis: t.b, origin: t.o }
}

pub fn from_bt(t: &Transform) -> Tf {
    Tf { b: t.basis, o: t.origin }
}

/// btCompoundShapeChild.
#[derive(Clone, Debug)]
pub struct Child {
    pub tf: Tf,
    pub shape: ShapeId,
    /// The dynamic tree leaf.
    pub node: usize,
    pub ty: ShapeType,
    pub margin: f64,
}

/// btCompoundShape.
#[derive(Clone, Debug)]
pub struct Compound {
    pub children: Vec<Child>,
    pub lmin: V3,
    pub lmax: V3,
    pub tree: Dbvt,
    pub revision: i32,
    pub margin: f64,
    pub scaling: V3,
}

#[derive(Clone, Debug)]
pub enum Shape {
    Conv(Conv),
    Compound(Compound),
    Bvh(BvhMesh),
    Height(HeightField),
}

/// The shape arena.
#[derive(Clone, Debug, Default)]
pub struct Shapes {
    pub v: Vec<Shape>,
}

impl Shapes {
    pub fn add(&mut self, s: Shape) -> ShapeId {
        self.v.push(s);
        self.v.len() - 1
    }

    pub fn new_sphere(&mut self, radius: f64) -> ShapeId {
        self.add(Shape::Conv(Conv::sphere(radius)))
    }

    pub fn new_box(&mut self, half: V3) -> ShapeId {
        self.add(Shape::Conv(Conv::box_(half)))
    }

    pub fn new_cylinder(&mut self, half: V3) -> ShapeId {
        self.add(Shape::Conv(Conv::cylinder(half)))
    }

    pub fn new_cone(&mut self, radius: f64, height: f64) -> ShapeId {
        self.add(Shape::Conv(Conv::cone(radius, height)))
    }

    pub fn new_capsule(&mut self, radius: f64, height: f64) -> ShapeId {
        self.add(Shape::Conv(Conv::capsule(radius, height)))
    }

    /// btConvexHullShape: addPoint (p, false) for each point, then recalcLocalAabb.
    pub fn new_convex_hull(&mut self, points: &[V3]) -> ShapeId {
        let mut c = Conv::hull();
        for p in points {
            c.hull_add_point(*p);
        }
        c.recalc_local_aabb();
        self.add(Shape::Conv(c))
    }

    /// btBvhTriangleMeshShape (btTriangleMesh, false): the triangles as added with addTriangle.
    pub fn new_bvh_triangle_mesh(&mut self, triangles: &[[V3; 3]]) -> ShapeId {
        for t in triangles {
            super::trace(|| format!("[\"btTriangleMesh\",\"addTriangle\",{},{},{}]", super::vj(&t[0]), super::vj(&t[1]), super::vj(&t[2])));
        }

        let tris = triangles.iter().map(|t| [r3(t[0]), r3(t[1]), r3(t[2])]).collect();
        self.add(Shape::Bvh(BvhMesh::new(tris)))
    }

    /// btHeightfieldTerrainShape (width, length, heights, 1, min, max, 1, PHY_FLOAT, true) followed by
    /// setLocalScaling (sx, 1, sz).
    pub fn new_heightfield(&mut self, width: usize, length: usize, heights: &[f64], min: f64, max: f64, scaling: V3) -> ShapeId {
        let heights: Vec<f64> = heights.iter().map(|h| f(*h)).collect();
        self.add(Shape::Height(HeightField { width, length, heights, min: f(min), max: f(max), scaling: r3(scaling) }))
    }

    /// btCompoundShape (enableDynamicAabbTree = true).
    pub fn new_compound(&mut self) -> ShapeId {
        self.add(Shape::Compound(Compound {
            children: vec![],
            lmin: [BT_LARGE_FLOAT; 3],
            lmax: [-BT_LARGE_FLOAT; 3],
            tree: Dbvt::new(),
            revision: 1,
            margin: 0.0,
            scaling: [1.0; 3],
        }))
    }

    pub fn shape_type(&self, id: ShapeId) -> ShapeType {
        match &self.v[id] {
            Shape::Conv(c) => c.shape_type(),
            Shape::Compound(_) => ShapeType::Compound,
            Shape::Bvh(_) => ShapeType::TriangleMesh,
            Shape::Height(_) => ShapeType::Heightfield,
        }
    }

    pub fn get_margin(&self, id: ShapeId) -> f64 {
        match &self.v[id] {
            Shape::Conv(c) => c.get_margin(),
            Shape::Compound(c) => c.margin,
            Shape::Bvh(_) | Shape::Height(_) => 0.0,
        }
    }

    pub fn set_margin(&mut self, id: ShapeId, m: f64) {
        super::trace(|| format!("[\"btCollisionShape\",\"setMargin\",{m}]"));
        match &mut self.v[id] {
            Shape::Conv(c) => {
                c.set_margin(m);
                if c.ty == ConvTy::Hull {
                    // btConvexHullShape does not override setMargin; the cached AABB is stale until
                    // the next recalcLocalAabb, exactly like Bullet.
                }
            }
            Shape::Compound(c) => c.margin = f(m),
            Shape::Bvh(_) | Shape::Height(_) => {}
        }
    }

    pub fn get_local_scaling(&self, id: ShapeId) -> V3 {
        match &self.v[id] {
            Shape::Conv(c) => c.scaling,
            Shape::Compound(c) => c.scaling,
            Shape::Bvh(_) => [1.0; 3],
            Shape::Height(h) => h.scaling,
        }
    }

    pub fn set_local_scaling(&mut self, id: ShapeId, s: V3) {
        super::trace(|| format!("[\"btCompoundShape\",\"setLocalScaling\",{}]", super::vj(&s)));
        let s = r3(s);

        if matches!(self.v[id], Shape::Compound(_)) {
            // btCompoundShape::setLocalScaling.
            let (n, old) = match &self.v[id] {
                Shape::Compound(c) => (c.children.len(), c.scaling),
                _ => unreachable!(),
            };

            for i in 0..n {
                let (child_tf, child_shape) = match &self.v[id] {
                    Shape::Compound(c) => (c.children[i].tf, c.children[i].shape),
                    _ => unreachable!(),
                };
                let cs = self.get_local_scaling(child_shape);
                // childScale = childScale * scaling / m_localScaling
                let new_scale = r3(std::array::from_fn(|k| cs[k] * s[k] / old[k]));
                self.set_local_scaling(child_shape, new_scale);
                let mut tf = child_tf;
                tf.o = r3(std::array::from_fn(|k| tf.o[k] * s[k] / old[k]));
                self.update_child_transform(id, i, tf, false);
            }

            if let Shape::Compound(c) = &mut self.v[id] {
                c.scaling = s;
            }
            self.recalculate_local_aabb(id);
            return;
        }

        match &mut self.v[id] {
            Shape::Conv(c) => c.set_local_scaling(s),
            Shape::Bvh(_) => {}
            Shape::Height(h) => h.scaling = s,
            Shape::Compound(_) => unreachable!(),
        }
    }

    /// getAabb.
    pub fn aabb(&self, id: ShapeId, t: &Tf) -> (V3, V3) {
        match &self.v[id] {
            Shape::Conv(c) => c.aabb(t),
            Shape::Compound(c) => {
                let mut half = scale(sub(c.lmax, c.lmin), 0.5);
                let mut center = scale(add(c.lmax, c.lmin), 0.5);

                // avoid an illegal AABB when there are no children
                if c.children.is_empty() {
                    half = ZERO;
                    center = ZERO;
                }

                half = add(half, [c.margin; 3]);
                let ab = absolute(&t.b);
                let center = t.apply(center);
                let extent = m_mul_v(&ab, half);
                (r3(sub(center, extent)), r3(add(center, extent)))
            }
            Shape::Bvh(m) => {
                let (lo, hi) = m.aabb(&to_bt(t));
                (r3(lo), r3(hi))
            }
            Shape::Height(h) => {
                let (lo, hi) = h.aabb(&to_bt(t));
                (r3(lo), r3(hi))
            }
        }
    }

    /// calculateLocalInertia.
    pub fn calculate_local_inertia(&self, id: ShapeId, mass: f64) -> V3 {
        super::trace(|| format!("[\"btCompoundShape\",\"calculateLocalInertia\",{mass}]"));
        match &self.v[id] {
            Shape::Conv(c) => c.calculate_local_inertia(mass),
            Shape::Compound(_) => {
                // approximation: take the inertia from the aabb for now
                let (lo, hi) = self.aabb(id, &Tf::IDENTITY);
                let half = scale(sub(hi, lo), 0.5);
                let (lx, ly, lz) = (2.0 * half[0], 2.0 * half[1], 2.0 * half[2]);
                r3([mass / 12.0 * (ly * ly + lz * lz), mass / 12.0 * (lx * lx + lz * lz), mass / 12.0 * (lx * lx + ly * ly)])
            }
            Shape::Bvh(_) | Shape::Height(_) => ZERO,
        }
    }

    /// btCollisionShape::getBoundingSphere.
    pub fn bounding_sphere(&self, id: ShapeId) -> (V3, f64) {
        let (lo, hi) = self.aabb(id, &Tf::IDENTITY);
        (scale(add(lo, hi), 0.5), len(sub(hi, lo)) * 0.5)
    }

    /// btCollisionShape::getAngularMotionDisc.
    pub fn angular_motion_disc(&self, id: ShapeId) -> f64 {
        let (center, radius) = self.bounding_sphere(id);
        radius + len(center)
    }

    /// btCollisionShape::getContactBreakingThreshold.
    pub fn contact_breaking_threshold(&self, id: ShapeId, default: f64) -> f64 {
        self.angular_motion_disc(id) * default
    }

    // --- btCompoundShape ---

    fn comp(&self, id: ShapeId) -> &Compound {
        match &self.v[id] {
            Shape::Compound(c) => c,
            _ => panic!("not a compound shape"),
        }
    }

    fn comp_mut(&mut self, id: ShapeId) -> &mut Compound {
        match &mut self.v[id] {
            Shape::Compound(c) => c,
            _ => panic!("not a compound shape"),
        }
    }

    pub fn num_child_shapes(&self, id: ShapeId) -> usize {
        self.comp(id).children.len()
    }

    pub fn child_shape(&self, id: ShapeId, i: usize) -> ShapeId {
        self.comp(id).children[i].shape
    }

    pub fn child_transform(&self, id: ShapeId, i: usize) -> Tf {
        self.comp(id).children[i].tf
    }

    pub fn update_revision(&self, id: ShapeId) -> i32 {
        self.comp(id).revision
    }

    /// addChildShape (localTransform, shape).
    pub fn add_child_shape(&mut self, id: ShapeId, tf: Tf, shape: ShapeId) {
        super::trace(|| format!("[\"btCompoundShape\",\"addChildShape\",{}]", super::tj(&tf)));
        let tf = rt(tf);
        let ty = self.shape_type(shape);
        let margin = self.get_margin(shape);
        let (lo, hi) = self.aabb(shape, &tf);
        let c = self.comp_mut(id);
        c.revision += 1;

        for i in 0..3 {
            if c.lmin[i] > lo[i] {
                c.lmin[i] = lo[i];
            }
            if c.lmax[i] < hi[i] {
                c.lmax[i] = hi[i];
            }
        }

        let index = c.children.len();
        let node = c.tree.insert(Aabb::from_mm(lo, hi), index);
        c.children.push(Child { tf, shape, node, ty, margin: f(margin) });
    }

    /// updateChildTransform (childIndex, newChildTransform, shouldRecalculateLocalAabb).
    pub fn update_child_transform(&mut self, id: ShapeId, i: usize, tf: Tf, recalc: bool) {
        super::trace(|| format!("[\"btCompoundShape\",\"updateChildTransform\",{i},{},{recalc}]", super::tj(&tf)));
        let tf = rt(tf);
        let shape = self.comp(id).children[i].shape;
        let (lo, hi) = self.aabb(shape, &tf);
        let c = self.comp_mut(id);
        c.children[i].tf = tf;
        let node = c.children[i].node;
        c.tree.update(node, Aabb::from_mm(lo, hi));

        if recalc {
            self.recalculate_local_aabb(id);
        }
    }

    /// removeChildShapeByIndex.
    pub fn remove_child_shape_by_index(&mut self, id: ShapeId, i: usize) {
        super::trace(|| format!("[\"btCompoundShape\",\"removeChildShapeByIndex\",{i}]"));
        let c = self.comp_mut(id);
        c.revision += 1;
        let node = c.children[i].node;
        c.tree.remove(node);
        let last = c.children.len() - 1;
        c.children.swap(i, last);
        let moved = c.children[i].node;
        c.tree.nodes[moved].data = i;
        c.children.pop();
    }

    /// recalculateLocalAabb.
    pub fn recalculate_local_aabb(&mut self, id: ShapeId) {
        super::trace(|| "[\"btCompoundShape\",\"recalculateLocalAabb\"]".to_string());
        let boxes: Vec<(V3, V3)> = self.comp(id).children.iter().map(|ch| self.aabb(ch.shape, &ch.tf)).collect();
        let c = self.comp_mut(id);
        c.lmin = [BT_LARGE_FLOAT; 3];
        c.lmax = [-BT_LARGE_FLOAT; 3];

        for (lo, hi) in boxes {
            for i in 0..3 {
                if c.lmin[i] > lo[i] {
                    c.lmin[i] = lo[i];
                }
                if c.lmax[i] < hi[i] {
                    c.lmax[i] = hi[i];
                }
            }
        }
    }
}
