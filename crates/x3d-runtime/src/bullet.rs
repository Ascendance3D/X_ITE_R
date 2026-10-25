//! The parts of Bullet 2.82 (zlib license, Erwin Coumans; GJK-EPA by Nathanael Presson) that Ammo
//! runs for Browser/Picking/VolumePicker.contactTest, ported to Rust. ammo.js (kripken/ammo.js,
//! bullet/VERSION 2.82, with a raycast patch from 2.83) is the version X_ITE ships. X_ITE asks one question: does
//! a manifold point have `distance <= 0` after `performDiscreteCollisionDetection`. The answer
//! depends on more than the shapes' distance, because a convex shape against a triangle mesh or a
//! height field shares one btPersistentManifold between all triangles: a new point replaces the
//! cached point nearest to it (closer than the contact breaking threshold), and the cache holds at
//! most four points. So this module ports the pieces that decide which points survive:
//!
//! - btGjkPairDetector.getClosestPointsNonVirtual (the btVoronoiSimplexSolver loop,
//!   btGjkEpaPenetrationDepthSolver with btGjkEpaSolver2, m_fixContactNormalDirection),
//! - SphereTriangleDetector (btSphereTriangleCollisionAlgorithm),
//! - btConvexConcaveCollisionAlgorithm with btBvhTriangleMeshShape (btOptimizedBvh, not quantized,
//!   walkStacklessTree) and btHeightfieldTerrainShape.processAllTriangles,
//! - btManifoldResult.addContactPoint, btPersistentManifold (getCacheEntry, sortCachedPoints,
//!   refreshContactPoints).
//!
//! Ammo is asm.js without Math.fround: arithmetic is double, values are rounded to float where
//! Bullet stores them in memory. This port computes in f64 and rounds the stored data (shapes,
//! vertices, manifold points) to f32.

pub type V = [f64; 3];

/// SIMD_EPSILON of the float build (FLT_EPSILON).
pub const SIMD_EPSILON: f64 = f32::EPSILON as f64;
const BT_LARGE_FLOAT: f64 = 1e18;
const REL_ERROR2: f64 = 1.0e-6;
const EQUAL_VERTEX_THRESHOLD: f64 = 0.0001;
const EPA_MAX_VERTICES: usize = 64;
const EPA_MAX_ITERATIONS: usize = 255;
const EPA_MAX_FACES: usize = EPA_MAX_VERTICES * 2;
/// gContactBreakingThreshold.
pub const CONTACT_BREAKING_THRESHOLD: f64 = 0.02;

pub fn f(x: f64) -> f64 {
    x as f32 as f64
}

pub fn fv(v: V) -> V {
    v.map(f)
}

pub fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn mul(a: V, s: f64) -> V {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn neg(a: V) -> V {
    [-a[0], -a[1], -a[2]]
}

pub fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn len2(a: V) -> f64 {
    dot(a, a)
}

fn len(a: V) -> f64 {
    len2(a).sqrt()
}

fn normalize(a: V) -> V {
    mul(a, 1.0 / len(a))
}

/// btVector3.maxAxis.
fn max_axis(v: V) -> usize {
    if v[0] < v[1] {
        if v[1] < v[2] { 2 } else { 1 }
    } else if v[0] < v[2] {
        2
    } else {
        0
    }
}

/// btTransform: the basis rows and the origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub basis: [V; 3],
    pub origin: V,
}

impl Transform {
    pub const IDENTITY: Transform = Transform {
        basis: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        origin: [0.0; 3],
    };

    pub fn translation(origin: V) -> Transform {
        Transform {
            origin,
            ..Transform::IDENTITY
        }
    }

    pub fn rotate(&self, d: V) -> V {
        [
            dot(self.basis[0], d),
            dot(self.basis[1], d),
            dot(self.basis[2], d),
        ]
    }

    /// `d * basis` (basis^T d).
    pub fn inv_rotate(&self, d: V) -> V {
        let b = &self.basis;
        [
            b[0][0] * d[0] + b[1][0] * d[1] + b[2][0] * d[2],
            b[0][1] * d[0] + b[1][1] * d[1] + b[2][1] * d[2],
            b[0][2] * d[0] + b[1][2] * d[1] + b[2][2] * d[2],
        ]
    }

    pub fn apply(&self, p: V) -> V {
        add(self.rotate(p), self.origin)
    }

    pub fn inv_xform(&self, p: V) -> V {
        self.inv_rotate(sub(p, self.origin))
    }

    pub fn mul(&self, o: &Transform) -> Transform {
        let basis = std::array::from_fn(|i| {
            std::array::from_fn(|j| (0..3).map(|k| self.basis[i][k] * o.basis[k][j]).sum())
        });
        Transform {
            basis,
            origin: self.apply(o.origin),
        }
    }

    /// inverseTimes: this^-1 * o.
    pub fn inverse_times(&self, o: &Transform) -> Transform {
        let basis = std::array::from_fn(|i| {
            std::array::from_fn(|j| (0..3).map(|k| self.basis[k][i] * o.basis[k][j]).sum())
        });
        Transform {
            basis,
            origin: self.inv_xform(o.origin),
        }
    }

    /// Column `i` of the basis.
    pub fn axis(&self, i: usize) -> V {
        [self.basis[0][i], self.basis[1][i], self.basis[2][i]]
    }

    /// halfExtents.dot3 (abs_b[0], abs_b[1], abs_b[2]).
    fn abs_extent(&self, h: V) -> V {
        std::array::from_fn(|i| {
            self.basis[i][0].abs() * h[0]
                + self.basis[i][1].abs() * h[1]
                + self.basis[i][2].abs() * h[2]
        })
    }

    /// btTransformAabb (localAabbMin, localAabbMax, margin, trans).
    pub fn transform_aabb(&self, lo: V, hi: V, margin: f64) -> (V, V) {
        let half = add(mul(sub(hi, lo), 0.5), [margin; 3]);
        let center = self.apply(mul(add(hi, lo), 0.5));
        let e = self.abs_extent(half);
        (sub(center, e), add(center, e))
    }
}

/// A btConvexShape after setLocalScaling: the core without margin and the margin.
#[derive(Clone, Debug, PartialEq)]
pub enum Convex {
    /// btSphereShape: a point with the radius as margin.
    Sphere { radius: f64 },
    /// btBoxShape: implicit half extents.
    Box { half: V, margin: f64 },
    /// btCylinderShape (Y axis): implicit half extents.
    Cylinder { half: V, margin: f64 },
    /// btConeShape (Y axis).
    Cone {
        radius: f64,
        height: f64,
        margin: f64,
    },
    /// btConvexHullShape: scaled points.
    Hull { points: Vec<V>, margin: f64 },
    /// btTriangleShape.
    Triangle { v: [V; 3], margin: f64 },
    /// btCapsuleShape (Y axis): implicit shape dimensions (radius, half height, radius).
    Capsule { half: V, margin: f64 },
}

impl Convex {
    pub fn margin(&self) -> f64 {
        match self {
            Convex::Sphere { radius } => *radius,
            Convex::Box { margin, .. }
            | Convex::Cylinder { margin, .. }
            | Convex::Cone { margin, .. }
            | Convex::Hull { margin, .. }
            | Convex::Triangle { margin, .. }
            | Convex::Capsule { margin, .. } => *margin,
        }
    }

    /// localGetSupportVertexWithoutMarginNonVirtual.
    pub fn support(&self, d: V) -> V {
        match self {
            Convex::Sphere { .. } => [0.0; 3],
            Convex::Box { half, .. } => {
                std::array::from_fn(|i| if d[i] >= 0.0 { half[i] } else { -half[i] })
            }
            Convex::Cylinder { half, .. } => {
                // CylinderLocalSupportY.
                let (radius, half_height) = (half[0], half[1]);
                let s = (d[0] * d[0] + d[2] * d[2]).sqrt();
                let y = if d[1] < 0.0 {
                    -half_height
                } else {
                    half_height
                };
                if s != 0.0 {
                    let k = radius / s;
                    [d[0] * k, y, d[2] * k]
                } else {
                    [radius, y, 0.0]
                }
            }
            Convex::Cone { radius, height, .. } => {
                // btConeShape.coneLocalSupport.
                let half = height * 0.5;
                let sin_angle = f(radius / (radius * radius + height * height).sqrt());
                if d[1] > len(d) * sin_angle {
                    [0.0, half, 0.0]
                } else {
                    let s = (d[0] * d[0] + d[2] * d[2]).sqrt();
                    if s > SIMD_EPSILON {
                        let k = radius / s;
                        [d[0] * k, -half, d[2] * k]
                    } else {
                        [0.0, -half, 0.0]
                    }
                }
            }
            Convex::Hull { points, .. } => {
                let mut best = (-BT_LARGE_FLOAT, [0.0; 3]);
                for p in points {
                    let x = dot(*p, d);
                    if x > best.0 {
                        best = (x, *p);
                    }
                }
                best.1
            }
            Convex::Triangle { v, .. } => v[max_axis([dot(d, v[0]), dot(d, v[1]), dot(d, v[2])])],
            Convex::Capsule { half, margin } => {
                // btCapsuleShape.localGetSupportingVertexWithoutMargin.
                let mut vec = d;
                let l2 = len2(vec);
                if l2 < 0.0001 {
                    vec = [1.0, 0.0, 0.0];
                } else {
                    vec = mul(vec, 1.0 / l2.sqrt());
                }
                let mut best = (-BT_LARGE_FLOAT, [0.0; 3]);
                for sign in [1.0, -1.0] {
                    let pos = [0.0, sign * half[1], 0.0];
                    let vtx = sub(add(pos, mul(vec, half[0])), mul(vec, *margin));
                    let nd = dot(vec, vtx);
                    if nd > best.0 {
                        best = (nd, vtx);
                    }
                }
                best.1
            }
        }
    }

    /// localGetSupportVertexNonVirtual (with margin).
    fn support_margin(&self, d: V) -> V {
        let n = normalize(if len2(d) < SIMD_EPSILON * SIMD_EPSILON {
            [-1.0; 3]
        } else {
            d
        });
        add(self.support(n), mul(n, self.margin()))
    }

    /// getAabb.
    pub fn aabb(&self, t: &Transform) -> (V, V) {
        match self {
            Convex::Sphere { radius } => (sub(t.origin, [*radius; 3]), add(t.origin, [*radius; 3])),
            Convex::Box { half, margin } | Convex::Cylinder { half, margin } => {
                t.transform_aabb(neg(*half), *half, *margin)
            }
            Convex::Hull { points, margin } => {
                // recalcLocalAabb (supporting vertices without margin, plus the margin), then
                // getNonvirtualAabb adds the margin again.
                let lo = std::array::from_fn(|i| {
                    self.support(std::array::from_fn(|k| if k == i { -1.0 } else { 0.0 }))[i]
                        - margin
                });
                let hi = std::array::from_fn(|i| {
                    self.support(std::array::from_fn(|k| if k == i { 1.0 } else { 0.0 }))[i]
                        + margin
                });
                let _ = points;
                t.transform_aabb(lo, hi, *margin)
            }
            Convex::Capsule { half, margin } => {
                let he = [half[0], half[0] + half[1], half[0]];
                t.transform_aabb(neg(he), he, *margin)
            }
            Convex::Cone { margin, .. } | Convex::Triangle { margin, .. } => {
                // btConvexInternalShape.getAabbSlow.
                let mut lo = [0.0; 3];
                let mut hi = [0.0; 3];
                for i in 0..3 {
                    let mut d = [0.0; 3];
                    d[i] = 1.0;
                    hi[i] = t.apply(self.support_margin(t.inv_rotate(d)))[i] + margin;
                    d[i] = -1.0;
                    lo[i] = t.apply(self.support_margin(t.inv_rotate(d)))[i] - margin;
                }
                (lo, hi)
            }
        }
    }
}

/// TestAabbAgainstAabb2.
pub fn aabb_overlap(a: &(V, V), b: &(V, V)) -> bool {
    !(a.0[0] > b.1[0]
        || a.1[0] < b.0[0]
        || a.0[2] > b.1[2]
        || a.1[2] < b.0[2]
        || a.0[1] > b.1[1]
        || a.1[1] < b.0[1])
}

pub fn triangle_overlaps_aabb(t: &[V; 3], lo: V, hi: V) -> bool {
    for i in [0, 2, 1] {
        if t[0][i].min(t[1][i]).min(t[2][i]) > hi[i] || t[0][i].max(t[1][i]).max(t[2][i]) < lo[i] {
            return false;
        }
    }

    true
}

// ---------------------------------------------------------------------------------------------
// btGjkPairDetector

#[derive(Clone, Copy, Default)]
pub struct SubResult {
    pub used: [bool; 4],
    pub bary: [f64; 4],
    pub degenerate: bool,
    pub closest: V,
}

impl SubResult {
    fn reset(&mut self) {
        self.degenerate = false;
        self.bary = [0.0; 4];
        self.used = [false; 4];
    }

    fn is_valid(&self) -> bool {
        self.bary.iter().all(|&b| b >= 0.0)
    }
}

/// btVoronoiSimplexSolver.
struct Voronoi {
    n: usize,
    w: [V; 5],
    p: [V; 5],
    q: [V; 5],
    cp1: V,
    cp2: V,
    cv: V,
    last_w: V,
    valid: bool,
    bc: SubResult,
    needs_update: bool,
}

impl Voronoi {
    fn new() -> Voronoi {
        Voronoi {
            n: 0,
            w: [[0.0; 3]; 5],
            p: [[0.0; 3]; 5],
            q: [[0.0; 3]; 5],
            cp1: [0.0; 3],
            cp2: [0.0; 3],
            cv: [0.0; 3],
            last_w: [BT_LARGE_FLOAT; 3],
            valid: false,
            bc: SubResult::default(),
            needs_update: true,
        }
    }

    fn remove_vertex(&mut self, i: usize) {
        self.n -= 1;
        self.w[i] = self.w[self.n];
        self.p[i] = self.p[self.n];
        self.q[i] = self.q[self.n];
    }

    fn reduce(&mut self, used: [bool; 4]) {
        for i in (0..4).rev() {
            if self.n > i && !used[i] {
                self.remove_vertex(i);
            }
        }
    }

    fn add_vertex(&mut self, w: V, p: V, q: V) {
        self.last_w = w;
        self.needs_update = true;
        self.w[self.n] = w;
        self.p[self.n] = p;
        self.q[self.n] = q;
        self.n += 1;
    }

    fn update(&mut self) -> bool {
        if !self.needs_update {
            return self.valid;
        }

        self.bc.reset();
        self.needs_update = false;

        match self.n {
            0 => self.valid = false,
            1 => {
                self.cp1 = self.p[0];
                self.cp2 = self.q[0];
                self.cv = sub(self.cp1, self.cp2);
                self.bc.reset();
                self.bc.bary = [1.0, 0.0, 0.0, 0.0];
                self.valid = self.bc.is_valid();
            }
            2 => {
                let from = self.w[0];
                let v = sub(self.w[1], from);
                let mut t = dot(v, neg(from));

                if t > 0.0 {
                    let vv = dot(v, v);
                    if t < vv {
                        t /= vv;
                        self.bc.used[0] = true;
                        self.bc.used[1] = true;
                    } else {
                        t = 1.0;
                        self.bc.used[1] = true;
                    }
                } else {
                    t = 0.0;
                    self.bc.used[0] = true;
                }

                self.bc.bary = [1.0 - t, t, 0.0, 0.0];
                self.cp1 = add(self.p[0], mul(sub(self.p[1], self.p[0]), t));
                self.cp2 = add(self.q[0], mul(sub(self.q[1], self.q[0]), t));
                self.cv = sub(self.cp1, self.cp2);
                self.reduce(self.bc.used);
                self.valid = self.bc.is_valid();
            }
            3 => {
                self.bc = closest_point_triangle([0.0; 3], self.w[0], self.w[1], self.w[2]);
                let b = self.bc.bary;
                self.cp1 = add(
                    add(mul(self.p[0], b[0]), mul(self.p[1], b[1])),
                    mul(self.p[2], b[2]),
                );
                self.cp2 = add(
                    add(mul(self.q[0], b[0]), mul(self.q[1], b[1])),
                    mul(self.q[2], b[2]),
                );
                self.cv = sub(self.cp1, self.cp2);
                self.reduce(self.bc.used);
                self.valid = self.bc.is_valid();
            }
            4 => {
                let (separated, r) =
                    closest_point_tetrahedron([0.0; 3], self.w[0], self.w[1], self.w[2], self.w[3]);
                self.bc = r;

                if separated {
                    let b = self.bc.bary;
                    self.cp1 = (0..4).fold([0.0; 3], |s, i| add(s, mul(self.p[i], b[i])));
                    self.cp2 = (0..4).fold([0.0; 3], |s, i| add(s, mul(self.q[i], b[i])));
                    self.cv = sub(self.cp1, self.cp2);
                    self.reduce(self.bc.used);
                } else {
                    if self.bc.degenerate {
                        self.valid = false;
                    } else {
                        self.valid = true;
                        self.cv = [0.0; 3];
                    }
                    return self.valid;
                }

                self.valid = self.bc.is_valid();
            }
            _ => self.valid = false,
        }

        self.valid
    }

    fn closest(&mut self) -> (bool, V) {
        let ok = self.update();
        (ok, self.cv)
    }

    fn in_simplex(&self, w: V) -> bool {
        let found = (0..self.n).any(|i| len2(sub(self.w[i], w)) <= EQUAL_VERTEX_THRESHOLD);
        w == self.last_w || found
    }

    fn compute_points(&mut self) -> (V, V) {
        self.update();
        (self.cp1, self.cp2)
    }
}

/// btVoronoiSimplexSolver.closestPtPointTriangle.
pub fn closest_point_triangle(p: V, a: V, b: V, c: V) -> SubResult {
    let mut r = SubResult::default();
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);

    if d1 <= 0.0 && d2 <= 0.0 {
        r.closest = a;
        r.used[0] = true;
        r.bary = [1.0, 0.0, 0.0, 0.0];
        return r;
    }

    let bp = sub(p, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);

    if d3 >= 0.0 && d4 <= d3 {
        r.closest = b;
        r.used[1] = true;
        r.bary = [0.0, 1.0, 0.0, 0.0];
        return r;
    }

    let vc = d1 * d4 - d3 * d2;

    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        r.closest = add(a, mul(ab, v));
        r.used[0] = true;
        r.used[1] = true;
        r.bary = [1.0 - v, v, 0.0, 0.0];
        return r;
    }

    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);

    if d6 >= 0.0 && d5 <= d6 {
        r.closest = c;
        r.used[2] = true;
        r.bary = [0.0, 0.0, 1.0, 0.0];
        return r;
    }

    let vb = d5 * d2 - d1 * d6;

    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        r.closest = add(a, mul(ac, w));
        r.used[0] = true;
        r.used[2] = true;
        r.bary = [1.0 - w, 0.0, w, 0.0];
        return r;
    }

    let va = d3 * d6 - d5 * d4;

    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        r.closest = add(b, mul(sub(c, b), w));
        r.used[1] = true;
        r.used[2] = true;
        r.bary = [0.0, 1.0 - w, w, 0.0];
        return r;
    }

    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    r.closest = add(add(a, mul(ab, v)), mul(ac, w));
    r.used = [true, true, true, false];
    r.bary = [1.0 - v - w, v, w, 0.0];
    r
}

/// btVoronoiSimplexSolver.pointOutsideOfPlane (CATCH_DEGENERATE_TETRAHEDRON, float build).
fn point_outside_of_plane(p: V, a: V, b: V, c: V, d: V) -> i32 {
    let normal = cross(sub(b, a), sub(c, a));
    let signp = dot(sub(p, a), normal);
    let signd = dot(sub(d, a), normal);

    if signd * signd < 1e-4 * 1e-4 {
        return -1;
    }

    (signp * signd < 0.0) as i32
}

/// btVoronoiSimplexSolver.closestPtPointTetrahedron: (separated, result).
pub fn closest_point_tetrahedron(p: V, a: V, b: V, c: V, d: V) -> (bool, SubResult) {
    let mut fin = SubResult {
        closest: p,
        used: [true; 4],
        ..SubResult::default()
    };
    let outside = [
        point_outside_of_plane(p, a, b, c, d),
        point_outside_of_plane(p, a, c, d, b),
        point_outside_of_plane(p, a, d, b, c),
        point_outside_of_plane(p, b, d, c, a),
    ];

    if outside.iter().any(|&o| o < 0) {
        fin.degenerate = true;
        return (false, fin);
    }

    if outside.iter().all(|&o| o == 0) {
        return (false, fin);
    }

    let mut best = f32::MAX as f64;
    // (triangle, which tetrahedron vertex each triangle vertex is)
    let faces = [
        ([a, b, c], [0, 1, 2]),
        ([a, c, d], [0, 2, 3]),
        ([a, d, b], [0, 3, 1]),
        ([b, d, c], [1, 3, 2]),
    ];

    for (k, (t, map)) in faces.iter().enumerate() {
        if outside[k] == 0 {
            continue;
        }

        let r = closest_point_triangle(p, t[0], t[1], t[2]);
        let q = r.closest;
        let dist = dot(sub(q, p), sub(q, p));

        if dist < best {
            best = dist;
            fin.closest = q;
            fin.used = [false; 4];
            fin.bary = [0.0; 4];
            for j in 0..3 {
                fin.used[map[j]] = r.used[j];
                fin.bary[map[j]] = r.bary[j];
            }
        }
    }

    (true, fin)
}

/// A convex shape at a transform.
#[derive(Clone, Copy)]
pub struct Placed<'a> {
    pub shape: &'a Convex,
    pub t: Transform,
}

/// The result of btGjkPairDetector: normalOnB, pointOnB (world) and the distance.
#[derive(Clone, Copy, Debug)]
pub struct ClosestPoints {
    pub normal: V,
    pub point_on_b: V,
    pub distance: f64,
}

/// btGjkPairDetector.getClosestPointsNonVirtual (with btGjkEpaPenetrationDepthSolver).
pub fn gjk_pair(a: Placed, b: Placed, max_distance2: f64) -> Option<ClosestPoints> {
    let offset = mul(add(a.t.origin, b.t.origin), 0.5);
    let ta = Transform {
        origin: sub(a.t.origin, offset),
        ..a.t
    };
    let tb = Transform {
        origin: sub(b.t.origin, offset),
        ..b.t
    };
    let (sa, sb) = (a.shape, b.shape);
    let margin_a = sa.margin();
    let margin_b = sb.margin();
    let margin = margin_a + margin_b;
    let mut axis = [0.0, 1.0, 0.0];
    let mut distance = 0.0;
    let mut normal_in_b = [0.0; 3];
    let mut point_on_a = [0.0; 3];
    let mut point_on_b = [0.0; 3];
    let mut is_valid = false;
    let mut check_simplex = false;
    let mut degenerate = 0;
    let mut solver = Voronoi::new();
    let mut squared_distance = BT_LARGE_FLOAT;

    for iteration in 0.. {
        let p_world = ta.apply(sa.support(ta.inv_rotate(neg(axis))));
        let q_world = tb.apply(sb.support(tb.inv_rotate(axis)));
        let w = sub(p_world, q_world);
        let delta = dot(axis, w);

        if delta > 0.0 && delta * delta > squared_distance * max_distance2 {
            degenerate = 10;
            check_simplex = true;
            break;
        }

        if solver.in_simplex(w) {
            degenerate = 1;
            check_simplex = true;
            break;
        }

        let f0 = squared_distance - delta;
        let f1 = squared_distance * REL_ERROR2;

        if f0 <= f1 {
            degenerate = if f0 <= 0.0 { 2 } else { 11 };
            check_simplex = true;
            break;
        }

        solver.add_vertex(w, p_world, q_world);
        let (ok, new_axis) = solver.closest();

        if !ok {
            degenerate = 3;
            check_simplex = true;
            break;
        }

        if len2(new_axis) < REL_ERROR2 {
            axis = new_axis;
            degenerate = 6;
            check_simplex = true;
            break;
        }

        let previous = squared_distance;
        squared_distance = len2(new_axis);

        if previous - squared_distance <= SIMD_EPSILON * previous {
            check_simplex = true;
            degenerate = 12;
            break;
        }

        axis = new_axis;

        if iteration > 1000 {
            break;
        }

        if solver.n == 4 {
            degenerate = 13;
            break;
        }
    }

    if check_simplex {
        (point_on_a, point_on_b) = solver.compute_points();
        normal_in_b = axis;
        let l2 = len2(axis);

        if l2 < REL_ERROR2 {
            degenerate = 5;
        }

        if l2 > SIMD_EPSILON * SIMD_EPSILON {
            let rlen = 1.0 / l2.sqrt();
            normal_in_b = mul(normal_in_b, rlen);
            let s = squared_distance.sqrt();
            point_on_a = sub(point_on_a, mul(axis, margin_a / s));
            point_on_b = add(point_on_b, mul(axis, margin_b / s));
            distance = 1.0 / rlen - margin;
            is_valid = true;
        }
    }

    let catch_degenerate = degenerate != 0 && (distance + margin) < 0.01;

    if !is_valid || catch_degenerate {
        let (valid2, pen_axis, tmp_a, tmp_b) = calc_pen_depth(sa, &ta, sb, &tb);

        {
            if valid2 {
                let mut tmp_normal = sub(tmp_b, tmp_a);
                let mut l2 = len2(tmp_normal);

                if l2 <= SIMD_EPSILON * SIMD_EPSILON {
                    tmp_normal = pen_axis;
                    l2 = len2(pen_axis);
                }

                if l2 > SIMD_EPSILON * SIMD_EPSILON {
                    tmp_normal = mul(tmp_normal, 1.0 / l2.sqrt());
                    let distance2 = -len(sub(tmp_a, tmp_b));

                    if !is_valid || distance2 < distance {
                        distance = distance2;
                        point_on_a = tmp_a;
                        point_on_b = tmp_b;
                        normal_in_b = tmp_normal;
                        is_valid = true;
                    }
                }
            } else if len2(pen_axis) > 0.0 {
                let distance2 = len(sub(tmp_a, tmp_b)) - margin;

                if !is_valid || distance2 < distance {
                    distance = distance2;
                    point_on_a = sub(tmp_a, mul(pen_axis, margin_a));
                    point_on_b = add(tmp_b, mul(pen_axis, margin_b));
                    normal_in_b = normalize(pen_axis);
                    is_valid = true;
                }
            }
        }
    }

    let _ = point_on_a;

    if !(is_valid && (distance < 0.0 || distance * distance < max_distance2)) {
        return None;
    }

    // m_fixContactNormalDirection: the normal points from B's AABB center to A's.
    let center = |s: &Convex, t: &Transform| {
        let (lo, hi) = s.aabb(t);
        mul(add(hi, lo), 0.5)
    };

    if dot(sub(center(sa, &ta), center(sb, &tb)), normal_in_b) < 0.0 {
        normal_in_b = neg(normal_in_b);
    }

    Some(ClosestPoints {
        normal: normal_in_b,
        point_on_b: add(point_on_b, offset),
        distance,
    })
}

// ---------------------------------------------------------------------------------------------
// btGjkEpaSolver2
//
// Transcribed from the asm.js of the pinned Ammo (Bullet 2.82), function by function, in the rounding
// model of that build: locals are doubles, every value that Bullet stores in a btScalar member or
// a stack object (rays, weights, vertices, face normals and distances, witnesses) is rounded to a
// float at the store, and expressions keep the association order the compiler emitted (for example
// `z*z + (x*x + y*y)` where Bullet writes `length2()`). The names of the locals follow the asm.js.
//
//   Bb  GJK::Evaluate                  zb  Penetration (EPA::Evaluate inlined)
//   $c  GJK::projectorigin (3 points)  Pb  GJK::EncloseOrigin
//   Jg  GJK::getsupport                Rd  EPA::newface
//   tg  EPA::expand                    Xj  EPA::getedgedist
//   Ec  Distance                       dh/zc  localGetSupportVertexNonVirtual (with / without margin)

/// GJK_MIN_DISTANCE, GJK_DUPLICATED_EPS, GJK_ACCURARY and EPA_ACCURACY as floats.
const F_1E4: f64 = 1e-4_f32 as f64;
/// EPA_PLANE_EPS as a float.
const F_1E5: f64 = 1e-5_f32 as f64;

/// localGetSupportVertexNonVirtual / localGetSupportVertexWithoutMarginNonVirtual: the result is stored
/// in a btVector3 (floats).
fn ls(s: &Convex, d: V, margins: bool) -> V {
    if !margins {
        return fv(s.support(d));
    }

    let (mut g, mut h, mut e) = (d[0], d[1], d[2]);

    if g * g + h * h + e * e < SIMD_EPSILON * SIMD_EPSILON {
        g = -1.0;
        h = -1.0;
        e = -1.0;
    }

    let j = 1.0 / (g * g + h * h + e * e).sqrt();
    let n = [f(g * j), f(h * j), f(e * j)];
    let sup = fv(s.support(n));
    let m = s.margin();
    [
        f(m * n[0] + sup[0]),
        f(m * n[1] + sup[1]),
        f(m * n[2] + sup[2]),
    ]
}

/// MinkowskiDiff: m_toshape1 (B^T A), m_toshape0 (A^-1 B) and the shapes, stored as floats.
struct Mink<'a> {
    s0: &'a Convex,
    s1: &'a Convex,
    to1: [V; 3],
    to0: [V; 3],
    to0_origin: V,
    margins: bool,
}

impl<'a> Mink<'a> {
    fn new(
        s0: &'a Convex,
        t0: &Transform,
        s1: &'a Convex,
        t1: &Transform,
        margins: bool,
    ) -> Mink<'a> {
        let (e, h) = (&t0.basis, &t1.basis);
        let to1 = std::array::from_fn(|i| {
            std::array::from_fn(|j| f(h[0][i] * e[0][j] + h[1][i] * e[1][j] + h[2][i] * e[2][j]))
        });
        let to0 = std::array::from_fn(|i| {
            std::array::from_fn(|j| f(e[0][i] * h[0][j] + e[1][i] * h[1][j] + e[2][i] * h[2][j]))
        });
        let (n, m, q) = (
            t1.origin[0] - t0.origin[0],
            t1.origin[1] - t0.origin[1],
            t1.origin[2] - t0.origin[2],
        );
        let to0_origin = std::array::from_fn(|j| f(n * e[0][j] + m * e[1][j] + q * e[2][j]));
        Mink {
            s0,
            s1,
            to1,
            to0,
            to0_origin,
            margins,
        }
    }

    /// Support0(d), d is a stored vector.
    fn support0(&self, d: V) -> V {
        ls(self.s0, d, self.margins)
    }

    /// Support1(d) = m_toshape0 * Ls(m_toshape1 * d). The transformed point is not stored: it is
    /// subtracted in the same expression.
    fn support1(&self, d: V) -> V {
        let t = &self.to1;
        let dir = fv(std::array::from_fn(|i| {
            t[i][0] * d[0] + t[i][1] * d[1] + t[i][2] * d[2]
        }));
        let s = ls(self.s1, dir, self.margins);
        let (r, o) = (&self.to0, &self.to0_origin);
        std::array::from_fn(|i| s[0] * r[i][0] + s[1] * r[i][1] + s[2] * r[i][2] + o[i])
    }
}

/// sSV: the support direction and the Minkowski difference point (both floats).
#[derive(Clone, Copy, Default)]
struct Sv {
    d: V,
    w: V,
}

#[derive(Clone, Copy, Default)]
struct Simplex2 {
    c: [usize; 4],
    p: [f64; 4],
    rank: usize,
}

/// z*z + (x*x + y*y), the order in which the asm.js sums a length2 of a cross product.
fn l2z(v: V) -> f64 {
    v[2] * v[2] + (v[0] * v[0] + v[1] * v[1])
}

#[allow(non_snake_case)]
/// GJK::projectorigin (a, b, c): (squared distance, weights, mask). The weights are floats.
fn project3(a: V, b: V, c: V) -> (f64, [f64; 3], u32) {
    let (G, H, I, x, y, z, A, B, C) = (a[0], b[0], a[1], b[1], a[2], b[2], c[0], c[1], c[2]);
    let D = (I - x) * (z - C) - (y - z) * (x - B);
    let E = (y - z) * (H - A) - (G - H) * (z - C);
    let F = (G - H) * (x - B) - (I - x) * (H - A);
    let mut w = [0.0; 3];
    let mut mask = 0;

    if !(F * F + (D * D + E * E) > 0.0) {
        return (-1.0, w, 0);
    }

    // m: the first weight, k: the second, l: the sub mask
    let (mut j, mut l, mut m, mut k);
    let (q, r, t);

    if G * (F * (I - x) - E * (y - z))
        + I * (D * (y - z) - F * (G - H))
        + (E * (G - H) - D * (I - x)) * y
        > 0.0
    {
        if (H - G) * (H - G) + (x - I) * (x - I) + (z - y) * (z - y) > 0.0 {
            let i = -(G * (H - G) + I * (x - I) + y * (z - y))
                / ((H - G) * (H - G) + (x - I) * (x - I) + (z - y) * (z - y));

            if i >= 1.0 {
                l = 2;
                j = H * H + x * x + z * z;
                m = 0.0;
                k = 1.0;
            } else if !(i <= 0.0) {
                k = f(i);
                l = 3;
                j = (y + (z - y) * i) * (y + (z - y) * i)
                    + ((G + (H - G) * i) * (G + (H - G) * i)
                        + (I + (x - I) * i) * (I + (x - I) * i));
                m = f(1.0 - i);
            } else {
                l = 1;
                j = G * G + I * I + y * y;
                m = 1.0;
                k = 0.0;
            }
        } else {
            l = 0;
            j = -1.0;
            m = 0.0;
            k = 0.0;
        }

        mask = l;
        w = [m, k, 0.0];
        q = b[0];
        r = b[1];
        t = b[2];
    } else {
        j = -1.0;
        l = 0;
        m = 0.0;
        k = 0.0;
        q = H;
        r = x;
        t = z;
    }

    if q * (F * (x - B) - E * (z - C))
        + r * (D * (z - C) - F * (H - A))
        + (E * (H - A) - D * (x - B)) * t
        > 0.0
    {
        let (u, v, ww) = (c[0], c[1], c[2]);
        let mut i = u - q;
        let o = v - r;
        let p = ww - t;

        if i * i + o * o + p * p > 0.0 {
            let n = -(q * i + r * o + t * p) / (i * i + o * o + p * p);

            if n >= 1.0 {
                l = 2;
                i = u * u + v * v + ww * ww;
                m = 0.0;
                k = 1.0;
            } else if !(n <= 0.0) {
                k = f(n);
                let w2 = q + i * n;
                let i2 = r + o * n;
                let v2 = t + p * n;
                l = 3;
                i = v2 * v2 + (w2 * w2 + i2 * i2);
                m = f(1.0 - n);
            } else {
                l = 1;
                i = q * q + r * r + t * t;
                m = 1.0;
                k = 0.0;
            }
        } else {
            i = -1.0;
        }

        if j < 0.0 || i < j {
            mask = (l << 1) & 6;
            w[1] = m;
            w[2] = k;
            w[0] = 0.0;
            j = i;
        }
    }

    let i = c[0];
    let o = c[1];
    let p = c[2];
    let mut ii;

    if i * (F * (B - I) - E * (C - y))
        + o * (D * (C - y) - F * (A - G))
        + (E * (A - G) - D * (B - I)) * p
        > 0.0
    {
        let (q, r, t) = (a[0], a[1], a[2]);
        if (q - i) * (q - i) + (r - o) * (r - o) + (t - p) * (t - p) > 0.0 {
            let n = -(i * (q - i) + o * (r - o) + p * (t - p))
                / ((q - i) * (q - i) + (r - o) * (r - o) + (t - p) * (t - p));

            if n >= 1.0 {
                l = 2;
                ii = q * q + r * r + t * t;
                m = 0.0;
                k = 1.0;
            } else if !(n <= 0.0) {
                k = f(n);
                l = 3;
                ii = (p + (t - p) * n) * (p + (t - p) * n)
                    + ((i + (q - i) * n) * (i + (q - i) * n)
                        + (o + (r - o) * n) * (o + (r - o) * n));
                m = f(1.0 - n);
            } else {
                l = 1;
                ii = i * i + o * o + p * p;
                m = 1.0;
                k = 0.0;
            }
        } else {
            ii = -1.0;
        }

        if j < 0.0 || ii < j {
            mask = ((l << 2) & 4) | ((l >> 1) & 1);
            w[2] = m;
            w[0] = k;
            w[1] = 0.0;
        } else {
            ii = j;
        }
    } else {
        ii = j;
    }

    if !(ii < 0.0) {
        return (ii, w, mask);
    }

    let v = (F * F + (D * D + E * E)).sqrt();
    let ww = (D * a[0] + E * a[1] + F * a[2]) / (F * F + (D * D + E * E));
    mask = 7;
    let t = b[0] - D * ww;
    let r = b[1] - E * ww;
    let u = b[2] - F * ww;
    let h2 = (((H - A) * r - (x - B) * t) * ((H - A) * r - (x - B) * t)
        + (((x - B) * u - (z - C) * r) * ((x - B) * u - (z - C) * r)
            + ((z - C) * t - (H - A) * u) * ((z - C) * t - (H - A) * u)))
        .sqrt()
        / v;
    w[0] = f(h2);
    let x2 = c[0] - D * ww;
    let u2 = c[1] - E * ww;
    let z2 = c[2] - F * ww;
    let i2 = (((A - G) * u2 - (B - I) * x2) * ((A - G) * u2 - (B - I) * x2)
        + (((B - I) * z2 - (C - y) * u2) * ((B - I) * z2 - (C - y) * u2)
            + ((C - y) * x2 - (A - G) * z2) * ((C - y) * x2 - (A - G) * z2)))
        .sqrt()
        / v;
    w[1] = f(i2);
    w[2] = f(1.0 - (h2 + i2));
    (
        F * ww * (F * ww) + (D * ww * (D * ww) + E * ww * (E * ww)),
        w,
        mask,
    )
}

#[derive(Clone, Copy, Default)]
struct Face {
    n: V,
    d: f64,
    c: [usize; 3],
    f: [usize; 3],
    l: [Option<usize>; 2],
    e: [u8; 3],
    pass: u8,
}

#[derive(Default)]
struct List {
    root: Option<usize>,
    count: usize,
}

#[derive(Default)]
struct Horizon {
    cf: Option<usize>,
    ff: Option<usize>,
    nf: usize,
}

const EPA_VALID: u8 = 0;
const EPA_DEGENERATED: u8 = 2;
const EPA_NON_CONVEX: u8 = 3;
const EPA_INVALID_HULL: u8 = 4;
const EPA_OUT_OF_FACES: u8 = 5;
const EPA_OUT_OF_VERTICES: u8 = 6;
const EPA_ACCURARY_REACHED: u8 = 7;
const EPA_FALL_BACK: u8 = 8;
const EPA_FAILED: u8 = 9;

/// GJK and EPA state of one query. `store` holds the four GJK vertices, then the EPA vertices.
struct Gjk<'a> {
    m: Mink<'a>,
    ray: V,
    simp: [Simplex2; 2],
    store: Vec<Sv>,
    free: [usize; 4],
    nfree: usize,
    cur: usize,
    msimplex: usize,
    /// 0 Valid, 1 Inside, 2 Failed
    status: u8,
    // EPA
    estatus: u8,
    result: Simplex2,
    normal: V,
    depth: f64,
    faces: Vec<Face>,
    nextsv: usize,
    hull: List,
    stock: List,
}

#[allow(non_snake_case)]
impl<'a> Gjk<'a> {
    fn new(m: Mink<'a>) -> Gjk<'a> {
        Gjk {
            m,
            ray: [0.0; 3],
            simp: [Simplex2::default(); 2],
            store: vec![Sv::default(); 4 + EPA_MAX_VERTICES],
            free: [0, 1, 2, 3],
            nfree: 4,
            cur: 0,
            msimplex: 0,
            status: 2,
            estatus: EPA_FAILED,
            result: Simplex2::default(),
            normal: [0.0; 3],
            depth: 0.0,
            faces: vec![Face::default(); EPA_MAX_FACES],
            nextsv: 0,
            hull: List::default(),
            stock: List::default(),
        }
    }

    /// Jg: getsupport.
    fn getsupport(&mut self, v: V, sv: usize) {
        let l = 1.0 / (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        let d = [f(l * v[0]), f(l * v[1]), f(l * v[2])];
        let s0 = self.m.support0(d);
        let s1 = self.m.support1(neg(d));
        self.store[sv] = Sv {
            d,
            w: fv(sub(s0, s1)),
        };
    }

    fn remove_vertice(&mut self, s: usize) {
        self.simp[s].rank -= 1;
        self.free[self.nfree] = self.simp[s].c[self.simp[s].rank];
        self.nfree += 1;
    }

    fn append_vertice(&mut self, s: usize, v: V) {
        let rank = self.simp[s].rank;
        self.simp[s].p[rank] = 0.0;
        self.nfree -= 1;
        let sv = self.free[self.nfree];
        self.simp[s].c[rank] = sv;
        self.simp[s].rank = rank + 1;
        self.getsupport(v, sv);
    }

    fn w(&self, s: usize, i: usize) -> V {
        self.store[self.simp[s].c[i]].w
    }

    /// Bb: GJK::Evaluate(shape, guess). Returns the status: 0 Valid, 1 Inside, 2 Failed.
    #[allow(non_snake_case)]
    fn evaluate(&mut self, guess: V) -> u8 {
        self.free = [0, 1, 2, 3];
        self.nfree = 4;
        self.cur = 0;
        self.status = 0;
        self.simp[0].rank = 0;
        self.ray = guess;
        let sqrl = guess[0] * guess[0] + guess[1] * guess[1] + guess[2] * guess[2];
        self.append_vertice(
            0,
            if sqrl > 0.0 {
                neg(guess)
            } else {
                [1.0, 0.0, 0.0]
            },
        );
        self.simp[0].p[0] = 1.0;
        self.ray = self.w(0, 0);
        let mut lastw = [self.ray; 4];
        let mut iterations = 0;
        let mut clastw = 0;
        let mut alpha = 0.0_f64;
        let mut C = 0usize;
        let mut wts = [0.0_f64; 4];
        // the simplex the loop ends on
        let mut simplex_on_exit = None;
        // The ray the asm.js keeps in its locals g, k, i across the iterations: the accumulation below is stored to
        // m_ray as floats, but the next iteration reads the unrounded sums from the locals (its length, the
        // direction of the new vertex). `ray_locals` is the unrounded value of the last accumulation step.
        let mut ray_locals: V = self.ray;

        loop {
            let b = 1 - C;
            let (g, k, i) = (ray_locals[0], ray_locals[1], ray_locals[2]);
            let l = (g * g + k * k + i * i).sqrt();

            if l < F_1E4 {
                self.status = 1;
                simplex_on_exit = Some(C);
                break;
            }

            self.append_vertice(C, neg(ray_locals));
            let rank = self.simp[C].rank;
            let w = self.w(C, rank - 1);

            if lastw.iter().any(|lw| {
                let (z, A, B) = (w[0] - lw[0], w[1] - lw[1], w[2] - lw[2]);
                z * z + A * A + B * B < F_1E4
            }) {
                self.remove_vertice(self.cur);
                simplex_on_exit = Some(self.cur);
                break;
            }

            clastw = (clastw + 1) & 3;
            lastw[clastw] = w;
            let B = (self.ray[0] * w[0] + self.ray[1] * w[1] + self.ray[2] * w[2]) / l;
            alpha = if B > alpha { B } else { alpha };

            if l - alpha - l * F_1E4 <= 0.0 {
                self.remove_vertice(self.cur);
                simplex_on_exit = Some(self.cur);
                break;
            }

            let mut mask: u32 = 0;
            let e: f64;

            match rank {
                2 => {
                    let (p, o) = (self.w(C, 0), self.w(C, 1));
                    let (e0, g0, i0, k0, l0, m0) = (o[0], p[0], o[1], p[1], o[2], p[2]);

                    if !((e0 - g0) * (e0 - g0) + (i0 - k0) * (i0 - k0) + (l0 - m0) * (l0 - m0)
                        > 0.0)
                    {
                        e = -1.0;
                    } else {
                        let n = -(g0 * (e0 - g0) + k0 * (i0 - k0) + m0 * (l0 - m0))
                            / ((e0 - g0) * (e0 - g0)
                                + (i0 - k0) * (i0 - k0)
                                + (l0 - m0) * (l0 - m0));

                        if n >= 1.0 {
                            wts[0] = 0.0;
                            wts[1] = 1.0;
                            mask = 2;
                            e = e0 * e0 + i0 * i0 + l0 * l0;
                        } else if !(n <= 0.0) {
                            wts[1] = f(n);
                            wts[0] = f(1.0 - n);
                            mask = 3;
                            e = (m0 + (l0 - m0) * n) * (m0 + (l0 - m0) * n)
                                + ((g0 + (e0 - g0) * n) * (g0 + (e0 - g0) * n)
                                    + (k0 + (i0 - k0) * n) * (k0 + (i0 - k0) * n));
                        } else {
                            wts[0] = 1.0;
                            wts[1] = 0.0;
                            mask = 1;
                            e = g0 * g0 + k0 * k0 + m0 * m0;
                        }
                    }
                }
                3 => {
                    let (ee, w3, mk) = project3(self.w(C, 0), self.w(C, 1), self.w(C, 2));
                    e = ee;
                    mask = mk;
                    wts[..3].copy_from_slice(&w3);
                }
                _ => {
                    let (d, j, o, p) = (self.w(C, 0), self.w(C, 1), self.w(C, 2), self.w(C, 3));
                    let (r, t, u, v, w_, x) = (d[0], p[0], d[1], p[1], d[2], p[2]);
                    let (g, i, k) = (j[0], j[1], j[2]);
                    let (y, z, A) = (o[0], o[1], o[2]);
                    let B = (r - t) * (i - v) * (A - x)
                        + ((u - v) * (k - x) * (y - t) + (w_ - x) * (g - t) * (z - v)
                            - (r - t) * (k - x) * (z - v)
                            - (u - v) * (g - t) * (A - x))
                        - (w_ - x) * (i - v) * (y - t);

                    // `B == B` is btVoronoiSimplexSolver's own "is not NaN" test, kept as
                    // the C++ writes it: the determinant is built from the simplex in the same
                    // association order, so when it degenerates the self-comparison is what
                    // Bullet branches on. `!B.is_nan()` is the same predicate, but it is our
                    // spelling, not the source's.
                    #[allow(clippy::eq_op, reason = "Bullet's NaN test, ported verbatim")]
                    if !(B == B
                        && B != 0.0
                        && B * (w_ * ((u - i) * (g - y) - (r - g) * (i - z))
                            + (r * ((w_ - k) * (i - z) - (u - i) * (k - A))
                                + u * ((r - g) * (k - A) - (w_ - k) * (g - y))))
                            <= 0.0)
                    {
                        e = -1.0;
                    } else {
                        let mut ee;
                        let (mut l1, mut m1, mut n1);

                        if B * (((u - v) * (k - x) - (w_ - x) * (i - v)) * t
                            + v * ((w_ - x) * (g - t) - (r - t) * (k - x))
                            + ((r - t) * (i - v) - (u - v) * (g - t)) * x)
                            > 0.0
                        {
                            let (e1, sw, sm) = project3(d, j, p);
                            ee = e1;
                            mask = (sm & 2) | ((sm << 1) & 8) | (sm & 1);
                            wts[0] = sw[0];
                            wts[1] = sw[1];
                            wts[2] = 0.0;
                            wts[3] = sw[2];
                            l1 = p[0];
                            m1 = p[1];
                            n1 = p[2];
                        } else {
                            ee = -1.0;
                            l1 = t;
                            m1 = v;
                            n1 = x;
                        }

                        if B * (((i - v) * (A - x) - (k - x) * (z - v)) * l1
                            + m1 * ((k - x) * (y - t) - (A - x) * (g - t))
                            + ((z - v) * (g - t) - (i - v) * (y - t)) * n1)
                            > 0.0
                        {
                            let (gg, sw, sm) = project3(j, o, p);

                            if ee < 0.0 || gg < ee {
                                mask = (sm << 1) & 14;
                                wts[1] = sw[0];
                                wts[2] = sw[1];
                                wts[0] = 0.0;
                                wts[3] = sw[2];
                                ee = gg;
                            }
                        }

                        if B * (((z - v) * (w_ - x) - (A - x) * (u - v)) * p[0]
                            + p[1] * ((A - x) * (r - t) - (w_ - x) * (y - t))
                            + ((u - v) * (y - t) - (z - v) * (r - t)) * p[2])
                            > 0.0
                        {
                            let (hh, sw, sm) = project3(o, d, p);

                            if ee < 0.0 || hh < ee {
                                mask = ((sm >> 1) & 1) | ((sm << 1) & 8) | ((sm << 2) & 4);
                                wts[2] = sw[0];
                                wts[0] = sw[1];
                                wts[1] = 0.0;
                                wts[3] = sw[2];
                                ee = hh;
                            }
                        }

                        if ee < 0.0 {
                            mask = 15;
                            let r2 = o[1];
                            let A2 = j[2];
                            let z2 = p[0];
                            let m2 = o[2];
                            let y2 = j[0];
                            let t2 = p[1];
                            let n2 = o[0];
                            let x2 = p[2];
                            let v2 = j[1];
                            wts[0] =
                                f((r2 * A2 * z2 + m2 * y2 * t2 - t2 * (A2 * n2) - r2 * y2 * x2
                                    + x2 * (n2 * v2)
                                    - z2 * (m2 * v2))
                                    / B);
                            let e2 = d[1];
                            let u2 = d[2];
                            let w2 = d[0];
                            wts[1] =
                                f((e2 * m2 * z2 + u2 * n2 * t2 - t2 * (m2 * w2) - e2 * n2 * x2
                                    + x2 * (w2 * r2)
                                    - z2 * (u2 * r2))
                                    / B);
                            wts[2] =
                                f((v2 * u2 * z2 + A2 * w2 * t2 - t2 * (u2 * y2) - v2 * w2 * x2
                                    + x2 * (y2 * e2)
                                    - z2 * (A2 * e2))
                                    / B);
                            wts[3] = f(1.0
                                - ((r2 * A2 * z2 + m2 * y2 * t2 - t2 * (A2 * n2) - r2 * y2 * x2
                                    + x2 * (n2 * v2)
                                    - z2 * (m2 * v2))
                                    / B
                                    + (e2 * m2 * z2 + u2 * n2 * t2
                                        - t2 * (m2 * w2)
                                        - e2 * n2 * x2
                                        + x2 * (w2 * r2)
                                        - z2 * (u2 * r2))
                                        / B
                                    + (v2 * u2 * z2 + A2 * w2 * t2
                                        - t2 * (u2 * y2)
                                        - v2 * w2 * x2
                                        + x2 * (y2 * e2)
                                        - z2 * (A2 * e2))
                                        / B));
                            ee = 0.0;
                        }

                        let _ = (&mut l1, &mut m1, &mut n1);
                        e = ee;
                    }
                }
            }

            if !(e >= 0.0) {
                self.remove_vertice(self.cur);
                simplex_on_exit = Some(self.cur);
                break;
            }

            self.simp[b].rank = 0;
            self.ray = [0.0; 3];
            ray_locals = [0.0; 3];
            self.cur = b;
            let cs = self.simp[C];

            for q in 0..cs.rank {
                if mask & (1 << q) != 0 {
                    let r = self.simp[b].rank;
                    self.simp[b].c[r] = cs.c[q];
                    self.simp[b].p[r] = wts[q];
                    self.simp[b].rank += 1;
                    let sw = self.store[cs.c[q]].w;
                    ray_locals = [
                        sw[0] * wts[q] + self.ray[0],
                        sw[1] * wts[q] + self.ray[1],
                        sw[2] * wts[q] + self.ray[2],
                    ];
                    self.ray = ray_locals.map(f);
                } else {
                    self.free[self.nfree] = cs.c[q];
                    self.nfree += 1;
                }
            }

            if mask == 15 {
                self.status = 1;
            }

            iterations += 1;

            if iterations >= 128 {
                self.status = 2;
                self.msimplex = b;
                return 2;
            }

            if self.status != 0 {
                break;
            }

            C = b;
        }

        self.msimplex = match simplex_on_exit {
            Some(s) => s,
            None => self.cur,
        };
        self.status
    }

    /// Pb: GJK::EncloseOrigin.
    fn enclose_origin(&mut self) -> bool {
        let s = self.msimplex;

        match self.simp[s].rank {
            1 => {
                for v in [
                    [1.0, 0.0, 0.0],
                    [-1.0, -0.0, -0.0],
                    [0.0, 1.0, 0.0],
                    [-0.0, -1.0, -0.0],
                    [0.0, 0.0, 1.0],
                    [-0.0, -0.0, -1.0],
                ] {
                    self.append_vertice(s, v);

                    if self.enclose_origin() {
                        return true;
                    }

                    self.remove_vertice(s);
                }
            }
            2 => {
                let (a, b) = (self.w(s, 0), self.w(s, 1));
                let d = sub(b, a);

                for axis in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
                    let p = cross(d, axis);

                    if l2z(p) > 0.0 {
                        self.append_vertice(s, p);

                        if self.enclose_origin() {
                            return true;
                        }

                        self.remove_vertice(s);
                        self.append_vertice(s, neg(p));

                        if self.enclose_origin() {
                            return true;
                        }

                        self.remove_vertice(s);
                    }
                }
            }
            3 => {
                let (a, b, c) = (self.w(s, 0), self.w(s, 1), self.w(s, 2));
                let n = cross(sub(b, a), sub(c, a));

                if l2z(n) > 0.0 {
                    self.append_vertice(s, n);

                    if self.enclose_origin() {
                        return true;
                    }

                    self.remove_vertice(s);
                    self.append_vertice(s, neg(n));

                    if self.enclose_origin() {
                        return true;
                    }

                    self.remove_vertice(s);
                }
            }
            4 => {
                let (w0, w1, w2, w3) = (self.w(s, 0), self.w(s, 1), self.w(s, 2), self.w(s, 3));
                let (o, b, g) = (w0[0] - w3[0], w0[1] - w3[1], w0[2] - w3[2]);
                let (d, h, n) = (w1[0] - w3[0], w1[1] - w3[1], w1[2] - w3[2]);
                let (i, m, e) = (w2[0] - w3[0], w2[1] - w3[1], w2[2] - w3[2]);
                let x = o * h * e + (b * n * i + g * d * m - o * n * m - b * d * e) - g * h * i;

                // Bullet's NaN test again, same reason as the determinant above.
                #[allow(clippy::eq_op, reason = "Bullet's NaN test, ported verbatim")]
                if x == x && x != 0.0 {
                    return true;
                }
            }
            _ => {}
        }

        false
    }

    // ---- EPA lists

    fn bind(&mut self, fa: usize, ea: usize, fb: usize, eb: usize) {
        self.faces[fa].e[ea] = eb as u8;
        self.faces[fa].f[ea] = fb;
        self.faces[fb].e[eb] = ea as u8;
        self.faces[fb].f[eb] = fa;
    }

    fn list_append(faces: &mut [Face], list: &mut List, face: usize) {
        faces[face].l[0] = None;
        faces[face].l[1] = list.root;

        if let Some(r) = list.root {
            faces[r].l[0] = Some(face);
        }

        list.root = Some(face);
        list.count += 1;
    }

    fn list_remove(faces: &mut [Face], list: &mut List, face: usize) {
        let [l0, l1] = faces[face].l;

        if let Some(n) = l1 {
            faces[n].l[0] = l0;
        }

        if let Some(p) = l0 {
            faces[p].l[1] = l1;
        }

        if list.root == Some(face) {
            list.root = l1;
        }

        list.count -= 1;
    }

    fn hull_to_stock(&mut self, face: usize) {
        Gjk::list_remove(&mut self.faces, &mut self.hull, face);
        Gjk::list_append(&mut self.faces, &mut self.stock, face);
    }

    /// Xj: EPA::getedgedist, stores the distance (a float) into `d`.
    #[allow(non_snake_case)]
    fn edge_dist(n: V, a: V, b: V) -> Option<f64> {
        let (aa, bb, c, d, e, g, h, i, j) = (n[0], n[1], n[2], a[0], a[1], a[2], b[0], b[1], b[2]);

        if !(((h - d) * bb - (i - e) * aa) * g
            + (((i - e) * c - (j - g) * bb) * d + ((j - g) * aa - (h - d) * c) * e)
            < 0.0)
        {
            return None;
        }

        let x;

        if !((h - d) * d + (i - e) * e + (j - g) * g > 0.0) {
            if (h - d) * h + (i - e) * i + (j - g) * j < 0.0 {
                x = h * h + i * i + j * j;
            } else {
                let y = ((h * h + i * i + j * j) * (d * d + e * e + g * g)
                    - (h * d + i * e + j * g) * (h * d + i * e + j * g))
                    / ((h - d) * (h - d) + (i - e) * (i - e) + (j - g) * (j - g));
                x = if y > 0.0 { y } else { 0.0 };
            }
        } else {
            x = d * d + e * e + g * g;
        }

        Some(f(x.sqrt()))
    }

    /// Rd: EPA::newface(a, b, c, forced).
    fn newface(&mut self, a: usize, b: usize, c: usize, forced: bool) -> Option<usize> {
        let Some(k) = self.stock.root else {
            self.estatus = EPA_OUT_OF_FACES;
            return None;
        };

        Gjk::list_remove(&mut self.faces, &mut self.stock, k);
        Gjk::list_append(&mut self.faces, &mut self.hull, k);
        self.faces[k].pass = 0;
        self.faces[k].c = [a, b, c];
        let (wa, wb, wc) = (self.store[a].w, self.store[b].w, self.store[c].w);
        let (n, l, p) = (wb[0] - wa[0], wb[1] - wa[1], wb[2] - wa[2]);
        let (j, m, o) = (wc[0] - wa[0], wc[1] - wa[1], wc[2] - wa[2]);
        let nx = l * o - p * m;
        let ny = p * j - n * o;
        let nz = n * m - l * j;
        self.faces[k].n = [f(nx), f(ny), f(nz)];
        let j = (nx * nx + ny * ny + nz * nz).sqrt();

        if !(j > F_1E4) {
            self.estatus = EPA_DEGENERATED;
            self.hull_to_stock(k);
            return None;
        }

        let sn = self.faces[k].n;

        let d = if let Some(d) = Gjk::edge_dist(sn, wa, wb) {
            d
        } else if let Some(d) = Gjk::edge_dist(sn, wb, wc) {
            d
        } else if let Some(d) = Gjk::edge_dist(sn, wc, wa) {
            d
        } else {
            f((wa[0] * sn[0] + wa[1] * sn[1] + wa[2] * sn[2]) / j)
        };

        self.faces[k].d = d;
        self.faces[k].n = [f(1.0 / j * sn[0]), f(1.0 / j * sn[1]), f(1.0 / j * sn[2])];

        if forced || self.faces[k].d >= -F_1E5 {
            return Some(k);
        }

        self.estatus = EPA_NON_CONVEX;
        self.hull_to_stock(k);
        None
    }

    /// tg: EPA::expand.
    fn expand(&mut self, pass: u8, w: usize, fc: usize, e: usize, horizon: &mut Horizon) -> bool {
        const I1M3: [usize; 3] = [1, 2, 0];
        const I2M3: [usize; 3] = [2, 0, 1];

        if self.faces[fc].pass == pass {
            return false;
        }

        let k = I1M3[e];
        let (n, d) = (self.faces[fc].n, self.faces[fc].d);
        let ww = self.store[w].w;

        if n[0] * ww[0] + n[1] * ww[1] + n[2] * ww[2] - d < -F_1E5 {
            let Some(nf) = self.newface(self.faces[fc].c[k], self.faces[fc].c[e], w, false) else {
                return false;
            };

            self.faces[nf].e[0] = e as u8;
            self.faces[nf].f[0] = fc;
            self.faces[fc].e[e] = 0;
            self.faces[fc].f[e] = nf;

            match horizon.cf {
                None => horizon.ff = Some(nf),
                Some(cf) => {
                    self.faces[cf].e[1] = 2;
                    self.faces[cf].f[1] = nf;
                    self.faces[nf].e[2] = 1;
                    self.faces[nf].f[2] = cf;
                }
            }

            horizon.cf = Some(nf);
            horizon.nf += 1;
            return true;
        }

        let i2 = I2M3[e];
        self.faces[fc].pass = pass;

        if !self.expand(
            pass,
            w,
            self.faces[fc].f[k],
            self.faces[fc].e[k] as usize,
            horizon,
        ) {
            return false;
        }

        if !self.expand(
            pass,
            w,
            self.faces[fc].f[i2],
            self.faces[fc].e[i2] as usize,
            horizon,
        ) {
            return false;
        }

        self.hull_to_stock(fc);
        true
    }

    fn findbest(&self) -> usize {
        let mut g = self.hull.root.unwrap();
        let mut m = self.faces[g].d * self.faces[g].d;
        let mut cur = self.faces[g].l[1];

        while let Some(b) = cur {
            let l = self.faces[b].d;

            if l * l < m {
                g = b;
                m = l * l;
            }

            cur = self.faces[b].l[1];
        }

        g
    }

    /// The EPA half of zb (Penetration), after GJK returned Inside. `guess` is the caller's guess.
    fn epa_evaluate(&mut self, guess: V) {
        self.estatus = EPA_FAILED;
        self.nextsv = 0;
        self.result = Simplex2::default();
        self.hull = List::default();
        self.stock = List::default();

        for i in 0..EPA_MAX_FACES {
            Gjk::list_append(&mut self.faces, &mut self.stock, EPA_MAX_FACES - i - 1);
        }

        let t = self.msimplex;
        let mut enclosed = false;

        if self.simp[t].rank > 1 && self.enclose_origin() {
            while let Some(fc) = self.hull.root {
                self.hull_to_stock(fc);
            }

            self.estatus = EPA_VALID;
            self.nextsv = 0;
            let (w0, w1, w2, w3) = (self.w(t, 0), self.w(t, 1), self.w(t, 2), self.w(t, 3));
            let (F, I, L) = (w0[0] - w3[0], w0[1] - w3[1], w0[2] - w3[2]);
            let (J, M, G) = (w1[0] - w3[0], w1[1] - w3[1], w1[2] - w3[2]);
            let (N, H, K) = (w2[0] - w3[0], w2[1] - w3[1], w2[2] - w3[2]);

            if F * M * K + (I * G * N + L * J * H - F * G * H - I * J * K) - L * M * N < 0.0 {
                self.simp[t].c.swap(0, 1);
                self.simp[t].p.swap(0, 1);
            }

            let c = self.simp[t].c;
            let k = self.newface(c[0], c[1], c[2], true);
            let o = self.newface(c[1], c[0], c[3], true);
            let p = self.newface(c[2], c[1], c[3], true);
            let i = self.newface(c[0], c[2], c[3], true);

            if self.hull.count == 4 {
                let (k, o, p, i) = (k.unwrap(), o.unwrap(), p.unwrap(), i.unwrap());
                enclosed = true;
                let mut best = self.findbest();
                let mut outer = self.faces[best];
                self.bind(k, 0, o, 0);
                self.bind(k, 1, p, 0);
                self.bind(k, 2, i, 0);
                self.bind(o, 1, i, 2);
                self.bind(o, 2, p, 1);
                self.bind(p, 2, i, 1);
                self.estatus = EPA_VALID;
                let mut pass: u8 = 0;

                loop {
                    let g = self.nextsv;

                    if g >= EPA_MAX_VERTICES {
                        self.estatus = EPA_OUT_OF_VERTICES;
                        break;
                    }

                    let mut horizon = Horizon::default();
                    self.nextsv = g + 1;
                    pass = pass.wrapping_add(1);
                    self.faces[best].pass = pass;
                    let w = 4 + g;
                    self.getsupport(self.faces[best].n, w);
                    let (bn, bd, ww) = (self.faces[best].n, self.faces[best].d, self.store[w].w);

                    if !(bn[0] * ww[0] + bn[1] * ww[1] + bn[2] * ww[2] - bd > F_1E4) {
                        self.estatus = EPA_ACCURARY_REACHED;
                        break;
                    }

                    let mut valid;
                    let mut b = 0;

                    loop {
                        let (fj, ej) = (self.faces[best].f[b], self.faces[best].e[b] as usize);
                        valid = self.expand(pass, w, fj, ej, &mut horizon);
                        b += 1;

                        if !(valid && b < 3) {
                            break;
                        }
                    }

                    if !(valid && horizon.nf > 2) {
                        self.estatus = EPA_INVALID_HULL;
                        break;
                    }

                    let (cf, ff) = (horizon.cf.unwrap(), horizon.ff.unwrap());
                    self.faces[cf].e[1] = 2;
                    self.faces[cf].f[1] = ff;
                    self.faces[ff].e[2] = 1;
                    self.faces[ff].f[2] = cf;
                    self.hull_to_stock(best);
                    best = self.findbest();
                    outer = self.faces[best];

                    if pass as usize >= EPA_MAX_ITERATIONS {
                        break;
                    }
                }

                let (n, r, q) = (outer.n[0], outer.n[1], outer.n[2]);
                let l = outer.d;
                let (J, H, N) = (n * l, r * l, q * l);
                self.normal = outer.n;
                self.depth = l;
                self.result.rank = 3;
                self.result.c[..3].copy_from_slice(&outer.c);
                let wc = |i: usize| self.store[outer.c[i]].w;
                let (c1, c2, c0) = (wc(1), wc(2), wc(0));
                let (K, F, I) = (c1[0] - J, c1[1] - H, c1[2] - N);
                let (E, G, L) = (c2[0] - J, c2[1] - H, c2[2] - N);
                let L2 = ((K * G - F * E) * (K * G - F * E)
                    + ((F * L - I * G) * (F * L - I * G) + (I * E - K * L) * (I * E - K * L)))
                    .sqrt();
                let (K2, G2, I2) = (c2[0] - J, c2[1] - H, c2[2] - N);
                let (E2, F2, M2) = (c0[0] - J, c0[1] - H, c0[2] - N);
                let M3 = ((K2 * F2 - G2 * E2) * (K2 * F2 - G2 * E2)
                    + ((G2 * M2 - I2 * F2) * (G2 * M2 - I2 * F2)
                        + (I2 * E2 - K2 * M2) * (I2 * E2 - K2 * M2)))
                    .sqrt();
                let (K3, G3, I3) = (c0[0] - J, c0[1] - H, c0[2] - N);
                let (J3, H3, N3) = (c1[0] - J, c1[1] - H, c1[2] - N);
                let N4 = ((K3 * H3 - G3 * J3) * (K3 * H3 - G3 * J3)
                    + ((G3 * N3 - I3 * H3) * (G3 * N3 - I3 * H3)
                        + (I3 * J3 - K3 * N3) * (I3 * J3 - K3 * N3)))
                    .sqrt();
                self.result.p[0] = f(L2 / (L2 + M3 + N4));
                self.result.p[1] = f(M3 / (L2 + M3 + N4));
                self.result.p[2] = f(N4 / (L2 + M3 + N4));
            }
        }

        if !enclosed {
            self.estatus = EPA_FALL_BACK;
            let (n, q, m) = (guess[0], guess[1], guess[2]);
            let l = (n * n + q * q + m * m).sqrt();
            self.normal = if l > 0.0 {
                [f(1.0 / l * n), f(1.0 / l * q), f(1.0 / l * m)]
            } else {
                [1.0, 0.0, 0.0]
            };
            self.depth = 0.0;
            self.result.rank = 1;
            self.result.c[0] = self.simp[t].c[0];
            self.result.p[0] = 1.0;
        }
    }
}

/// zb: btGjkEpaSolver2::Penetration. (witness on A, witness on B, normal) in world space.
fn epa_penetration(
    s0: &Convex,
    t0: &Transform,
    s1: &Convex,
    t1: &Transform,
    guess: V,
) -> Option<(V, V, V)> {
    let mut gjk = Gjk::new(Mink::new(s0, t0, s1, t1, true));

    if gjk.evaluate(neg(guess)) != 1 {
        return None;
    }

    gjk.epa_evaluate(guess);

    if gjk.estatus == EPA_FAILED {
        return None;
    }

    let (mut l, mut m, mut n) = (0.0, 0.0, 0.0);

    for h in 0..gjk.result.rank {
        let o = gjk.m.support0(gjk.store[gjk.result.c[h]].d);
        let nn = gjk.result.p[h];
        l += o[0] * nn;
        m += nn * o[1];
        n += nn * o[2];
    }

    let wp = |p: V| fv(t0.apply(p));
    let (kx, ky, kz) = (gjk.normal[0], gjk.normal[1], gjk.normal[2]);
    let nd = gjk.depth;
    let (fx, fy, fz) = (l - kx * nd, m - nd * ky, n - nd * kz);
    Some((wp([l, m, n]), wp([fx, fy, fz]), [-kx, -ky, -kz]))
}

#[allow(non_snake_case)]
/// Ec: btGjkEpaSolver2::Distance. (witness on A, witness on B, normal) in world space.
fn epa_distance(
    s0: &Convex,
    t0: &Transform,
    s1: &Convex,
    t1: &Transform,
    guess: V,
) -> Option<(V, V, V)> {
    let mut gjk = Gjk::new(Mink::new(s0, t0, s1, t1, false));

    if gjk.evaluate(guess) != 0 {
        return None;
    }

    let s = gjk.simp[gjk.msimplex];
    let (mut n, mut o, mut p) = (0.0, 0.0, 0.0);
    let (mut i, mut l, mut m) = (0.0, 0.0, 0.0);

    for g in 0..s.rank {
        let u = s.p[g];
        let sv = gjk.store[s.c[g]];
        let v0 = gjk.m.support0(sv.d);
        n += u * v0[0];
        o += u * v0[1];
        p += u * v0[2];
        let (q, r, t) = (-sv.d[0], -sv.d[1], -sv.d[2]);
        let tt = &gjk.m.to1;
        let dir = fv([
            tt[0][0] * q + tt[0][1] * r + tt[0][2] * t,
            tt[1][0] * q + tt[1][1] * r + tt[1][2] * t,
            tt[2][0] * q + tt[2][1] * r + tt[2][2] * t,
        ]);
        let g1 = fv(s1.support(dir));
        let (rr, oo) = (&gjk.m.to0, &gjk.m.to0_origin);
        i += u * (g1[0] * rr[0][0] + g1[1] * rr[0][1] + g1[2] * rr[0][2] + oo[0]);
        l += u * (g1[0] * rr[1][0] + g1[1] * rr[1][1] + g1[2] * rr[1][2] + oo[1]);
        m += u * (g1[0] * rr[2][0] + g1[1] * rr[2][1] + g1[2] * rr[2][2] + oo[2]);
    }

    let wp = |p: V| fv(t0.apply(p));
    let (F, G, H) = (n - i, o - l, p - m);
    let dist = (F * F + G * G + H * H).sqrt();
    let k = if dist > F_1E4 { 1.0 / dist } else { 1.0 };
    Some((wp([n, o, p]), wp([i, l, m]), [f(F * k), f(G * k), f(H * k)]))
}

/// sg: btGjkEpaPenetrationDepthSolver.calcPenDepth: (penetration, v, witness on A, witness on B).
pub fn calc_pen_depth(sa: &Convex, ta: &Transform, sb: &Convex, tb: &Transform) -> (bool, V, V, V) {
    // The transforms are btTransforms: floats.
    let round = |t: &Transform| Transform {
        basis: t.basis.map(fv),
        origin: fv(t.origin),
    };
    let (ta, tb) = (round(ta), round(tb));
    let guess = fv(sub(tb.origin, ta.origin));

    if let Some((a, b, n)) = epa_penetration(sa, &ta, sb, &tb, guess) {
        return (true, n, a, b);
    }

    if let Some((a, b, n)) = epa_distance(sa, &ta, sb, &tb, guess) {
        return (false, n, a, b);
    }

    (false, [0.0; 3], [0.0; 3], [0.0; 3])
}

// ---------------------------------------------------------------------------------------------
// SphereTriangleDetector

/// SphereTriangleDetector.collide in the triangle's space: (point, normal, depth). In 2.82 the
/// last edge closer than the capsule radius gives the contact point, not the nearest one.
pub fn sphere_triangle(center: V, radius: f64, v: &[V; 3], threshold: f64) -> Option<(V, V, f64)> {
    let radius_with_threshold = radius + threshold;
    let mut normal = normalize(cross(sub(v[1], v[0]), sub(v[2], v[0])));
    let mut distance_from_plane = dot(sub(center, v[0]), normal);

    if distance_from_plane < 0.0 {
        distance_from_plane *= -1.0;
        normal = mul(normal, -1.0);
    }

    let mut contact = None;

    if distance_from_plane < radius_with_threshold {
        if point_in_triangle(v, normal, center) {
            contact = Some(sub(center, mul(normal, distance_from_plane)));
        } else {
            let capsule2 = radius_with_threshold * radius_with_threshold;

            for i in 0..3 {
                let (d2, nearest) = segment_sqr_distance(v[i], v[(i + 1) % 3], center);

                if d2 < capsule2 {
                    contact = Some(nearest);
                }
            }
        }
    }

    let point = contact?;
    let to_center = sub(center, point);
    let d2 = len2(to_center);

    if d2 < radius_with_threshold * radius_with_threshold {
        if d2 > SIMD_EPSILON {
            let distance = d2.sqrt();
            return Some((point, normalize(to_center), -(radius - distance)));
        }

        return Some((point, normal, -radius));
    }

    None
}

pub fn point_in_triangle(v: &[V; 3], normal: V, p: V) -> bool {
    let r1 = dot(cross(sub(v[1], v[0]), normal), sub(p, v[0]));
    let r2 = dot(cross(sub(v[2], v[1]), normal), sub(p, v[1]));
    let r3 = dot(cross(sub(v[0], v[2]), normal), sub(p, v[2]));
    (r1 > 0.0 && r2 > 0.0 && r3 > 0.0) || (r1 <= 0.0 && r2 <= 0.0 && r3 <= 0.0)
}

fn segment_sqr_distance(from: V, to: V, p: V) -> (f64, V) {
    let mut diff = sub(p, from);
    let v = sub(to, from);
    let mut t = dot(v, diff);

    if t > 0.0 {
        let vv = dot(v, v);
        if t < vv {
            t /= vv;
            diff = sub(diff, mul(v, t));
        } else {
            t = 1.0;
            diff = sub(diff, v);
        }
    } else {
        t = 0.0;
    }

    (dot(diff, diff), add(from, mul(v, t)))
}

// ---------------------------------------------------------------------------------------------
// btPersistentManifold

#[derive(Clone, Copy, Debug)]
struct ManifoldPoint {
    a: V,
    b: V,
    normal: V,
    distance: f64,
}

/// btPersistentManifold of a pair whose bodies are at the identity (VolumePicker's rigid bodies).
#[derive(Debug)]
pub struct Manifold {
    points: Vec<ManifoldPoint>,
    threshold: f64,
}

impl Manifold {
    pub fn new(threshold: f64) -> Manifold {
        Manifold {
            points: Vec::with_capacity(4),
            threshold: f(threshold),
        }
    }

    /// btManifoldResult.addContactPoint.
    pub fn add_contact_point(&mut self, normal: V, point_b: V, depth: f64) {
        if depth > self.threshold {
            return;
        }

        let point_a = add(point_b, mul(normal, depth));
        let p = ManifoldPoint {
            a: fv(point_a),
            b: fv(point_b),
            normal: fv(normal),
            distance: f(depth),
        };

        // getCacheEntry.
        let mut shortest = self.threshold * self.threshold;
        let mut nearest = None;

        for (i, q) in self.points.iter().enumerate() {
            let d = len2(sub(q.a, p.a));
            if d < shortest {
                shortest = d;
                nearest = Some(i);
            }
        }

        match nearest {
            Some(i) => self.points[i] = p,
            None if self.points.len() == 4 => {
                let i = self.sort_cached_points(&p);
                self.points[i] = p;
            }
            None => self.points.push(p),
        }
    }

    /// sortCachedPoints (KEEP_DEEPEST_POINT, gContactCalcArea3Points).
    fn sort_cached_points(&self, pt: &ManifoldPoint) -> usize {
        let c = &self.points;
        let mut deepest = None;
        let mut max_penetration = pt.distance;

        for (i, q) in c.iter().enumerate() {
            if q.distance < max_penetration {
                deepest = Some(i);
                max_penetration = q.distance;
            }
        }

        let area = |a: V, b: V| len2(cross(a, b));
        let res = [
            if deepest != Some(0) {
                area(sub(pt.a, c[1].a), sub(c[3].a, c[2].a))
            } else {
                0.0
            },
            if deepest != Some(1) {
                area(sub(pt.a, c[0].a), sub(c[3].a, c[2].a))
            } else {
                0.0
            },
            if deepest != Some(2) {
                area(sub(pt.a, c[0].a), sub(c[3].a, c[1].a))
            } else {
                0.0
            },
            if deepest != Some(3) {
                area(sub(pt.a, c[0].a), sub(c[2].a, c[1].a))
            } else {
                0.0
            },
        ];

        // closestAxis4: the first largest absolute value.
        let mut best = 0;
        let mut value = -BT_LARGE_FLOAT;
        for (i, r) in res.iter().enumerate() {
            if r.abs() > value {
                best = i;
                value = r.abs();
            }
        }
        best
    }

    /// refreshContactPoints with both bodies at the identity.
    pub fn refresh(&mut self) {
        for p in &mut self.points {
            p.distance = f(dot(sub(p.a, p.b), p.normal));
        }

        let th2 = self.threshold * self.threshold;

        for i in (0..self.points.len()).rev() {
            let p = self.points[i];

            if p.distance > self.threshold {
                self.points.swap_remove(i);
                continue;
            }

            let projected = sub(p.a, mul(p.normal, p.distance));
            if len2(sub(p.b, projected)) > th2 {
                self.points.swap_remove(i);
            }
        }
    }

    /// VolumePicker.contactTest: a point with distance <= 0.
    pub fn hit(&self) -> bool {
        self.points.iter().any(|p| p.distance <= 0.0)
    }

    pub fn distances(&self) -> Vec<f64> {
        self.points.iter().map(|p| p.distance).collect()
    }
}

// ---------------------------------------------------------------------------------------------
// Concave shapes

#[derive(Clone, Copy, Debug)]
struct BvhNode {
    lo: V,
    hi: V,
    /// -1 for a leaf.
    escape: i64,
    triangle: usize,
}

/// btBvhTriangleMeshShape (btOptimizedBvh without quantization) of scaled triangles.
#[derive(Clone, Debug)]
pub struct BvhMesh {
    pub triangles: Vec<[V; 3]>,
    nodes: Vec<BvhNode>,
    local_aabb: (V, V),
}

impl BvhMesh {
    pub fn new(triangles: Vec<[V; 3]>) -> BvhMesh {
        let mut leaves: Vec<BvhNode> = triangles
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let lo = std::array::from_fn(|k| t[0][k].min(t[1][k]).min(t[2][k]));
                let hi = std::array::from_fn(|k| t[0][k].max(t[1][k]).max(t[2][k]));
                BvhNode {
                    lo,
                    hi,
                    escape: -1,
                    triangle: i,
                }
            })
            .collect();
        let mut nodes = Vec::with_capacity(2 * leaves.len());

        if !leaves.is_empty() {
            let n = leaves.len();
            build_tree(&mut leaves, &mut nodes, 0, n);
        }

        // recalcLocalAabb: the supporting vertices along the axes (margin 0).
        let all = triangles.iter().flatten();
        let lo = std::array::from_fn(|k| all.clone().map(|p| p[k]).fold(BT_LARGE_FLOAT, f64::min));
        let hi = std::array::from_fn(|k| all.clone().map(|p| p[k]).fold(-BT_LARGE_FLOAT, f64::max));
        BvhMesh {
            triangles,
            nodes,
            local_aabb: (lo, hi),
        }
    }

    /// walkStacklessTree.
    pub fn process_all_triangles(&self, lo: V, hi: V, mut cb: impl FnMut(&[V; 3])) {
        let mut i = 0;

        while i < self.nodes.len() {
            let n = &self.nodes[i];
            let overlap = aabb_overlap(&(lo, hi), &(n.lo, n.hi));
            let leaf = n.escape == -1;

            if leaf && overlap {
                cb(&self.triangles[n.triangle]);
            }

            if overlap || leaf {
                i += 1;
            } else {
                i += n.escape as usize;
            }
        }
    }

    /// btTriangleMeshShape.getAabb (margin 0).
    pub fn aabb(&self, t: &Transform) -> (V, V) {
        t.transform_aabb(self.local_aabb.0, self.local_aabb.1, 0.0)
    }
}

fn center(n: &BvhNode) -> V {
    mul(add(n.hi, n.lo), 0.5)
}

fn build_tree(leaves: &mut [BvhNode], nodes: &mut Vec<BvhNode>, start: usize, end: usize) {
    let num = end - start;
    let cur = nodes.len();

    if num == 1 {
        nodes.push(leaves[start]);
        return;
    }

    // calcSplittingAxis.
    let mut means = [0.0; 3];
    for l in &leaves[start..end] {
        means = add(means, center(l));
    }
    means = mul(means, 1.0 / num as f64);
    let mut variance = [0.0; 3];
    for l in &leaves[start..end] {
        let d = sub(center(l), means);
        variance = add(variance, [d[0] * d[0], d[1] * d[1], d[2] * d[2]]);
    }
    variance = mul(variance, 1.0 / (num as f64 - 1.0));
    let axis = max_axis(variance);

    // sortAndCalcSplittingIndex.
    let split_value = means[axis];
    let mut split = start;
    for i in start..end {
        if center(&leaves[i])[axis] > split_value {
            leaves.swap(i, split);
            split += 1;
        }
    }
    let balanced = num / 3;
    if split <= start + balanced || split >= end - 1 - balanced {
        split = start + (num >> 1);
    }

    let internal = nodes.len();
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for l in &leaves[start..end] {
        for k in 0..3 {
            lo[k] = lo[k].min(l.lo[k]);
            hi[k] = hi[k].max(l.hi[k]);
        }
    }
    nodes.push(BvhNode {
        lo,
        hi,
        escape: 0,
        triangle: 0,
    });

    build_tree(leaves, nodes, start, split);
    build_tree(leaves, nodes, split, end);
    nodes[internal].escape = (nodes.len() - cur) as i64;
}

/// btHeightfieldTerrainShape (Y up, PHY_FLOAT, flipQuadEdges) after setLocalScaling.
#[derive(Clone, Debug)]
pub struct HeightField {
    pub width: usize,
    pub length: usize,
    /// Raw heights (float), row-major by z.
    pub heights: Vec<f64>,
    pub min: f64,
    pub max: f64,
    pub scaling: V,
}

impl HeightField {
    fn local_origin(&self) -> V {
        [
            f((self.width - 1) as f64 * 0.5),
            f((self.min + self.max) * 0.5),
            f((self.length - 1) as f64 * 0.5),
        ]
    }

    fn local_aabb(&self) -> (V, V) {
        (
            [0.0, self.min, 0.0],
            [(self.width - 1) as f64, self.max, (self.length - 1) as f64],
        )
    }

    fn vertex(&self, x: usize, y: usize) -> V {
        let h = self.heights.get(y * self.width + x).copied().unwrap_or(0.0);
        let o = self.local_origin();
        fv([
            (-((self.width - 1) as f64) / 2.0 + x as f64) * self.scaling[0],
            (h - o[1]) * self.scaling[1],
            (-((self.length - 1) as f64) / 2.0 + y as f64) * self.scaling[2],
        ])
    }

    /// processAllTriangles.
    pub fn process_all_triangles(&self, lo: V, hi: V, mut cb: impl FnMut(&[V; 3])) {
        let o = self.local_origin();
        let inv = self.scaling.map(|s| f(1.0 / s));
        let local_lo = add([lo[0] * inv[0], lo[1] * inv[1], lo[2] * inv[2]], o);
        let local_hi = add([hi[0] * inv[0], hi[1] * inv[1], hi[2] * inv[2]], o);
        let (amin, amax) = self.local_aabb();
        let quantize = |p: V| -> [i64; 3] {
            std::array::from_fn(|i| {
                let x = p[i].max(amin[i]).min(amax[i]);
                if x < 0.0 {
                    (x - 0.5) as i64
                } else {
                    (x + 0.5) as i64
                }
            })
        };
        let qmin = quantize(local_lo).map(|v| v - 1);
        let qmax = quantize(local_hi).map(|v| v + 1);
        let start_x = qmin[0].max(0);
        let end_x = qmax[0].min(self.width as i64 - 1);
        let start_j = qmin[2].max(0);
        let end_j = qmax[2].min(self.length as i64 - 1);

        for j in start_j..end_j {
            for x in start_x..end_x {
                let (x, j) = (x as usize, j as usize);
                // flipQuadEdges (2.82 order).
                let mut v = [
                    self.vertex(x, j),
                    self.vertex(x + 1, j),
                    self.vertex(x + 1, j + 1),
                ];
                cb(&v);
                v[1] = self.vertex(x + 1, j + 1);
                v[2] = self.vertex(x, j + 1);
                cb(&v);
            }
        }
    }

    /// getAabb (centered on the transform's origin).
    pub fn aabb(&self, t: &Transform) -> (V, V) {
        let (lo, hi) = self.local_aabb();
        let half = std::array::from_fn(|i| (hi[i] - lo[i]) * self.scaling[i] * 0.5);
        let e = t.abs_extent(half);
        (sub(t.origin, e), add(t.origin, e))
    }
}

/// A shape of one compound child.
#[derive(Clone, Copy)]
pub enum Body<'a> {
    Convex(&'a Convex, Transform),
    Mesh(&'a BvhMesh, Transform),
    Height(&'a HeightField, Transform),
}

impl Body<'_> {
    pub fn aabb(&self) -> (V, V) {
        match self {
            Body::Convex(s, t) => s.aabb(t),
            Body::Mesh(m, t) => m.aabb(t),
            Body::Height(h, t) => h.aabb(t),
        }
    }
}

/// btConvexConcaveCollisionAlgorithm: the convex shape against the triangles near it, in one
/// manifold, then refreshContactPoints.
fn convex_concave(convex: &Convex, tc: &Transform, concave: &Body, threshold: f64) -> Manifold {
    let mut manifold = Manifold::new(threshold);
    let (Body::Mesh(_, tt) | Body::Height(_, tt)) = concave else {
        unreachable!()
    };
    let (lo, hi) = convex.aabb(&tt.inverse_times(tc));
    let max_distance2 = (convex.margin() + manifold.threshold).powi(2);

    let mut triangle = |v: &[V; 3]| {
        if !triangle_overlaps_aabb(v, lo, hi) {
            return;
        }

        if let Convex::Sphere { radius } = convex {
            if let Some((point, normal, depth)) =
                sphere_triangle(tt.inv_xform(tc.origin), *radius, v, manifold.threshold)
            {
                manifold.add_contact_point(tt.rotate(normal), tt.apply(point), depth);
            }
        } else {
            let t = Convex::Triangle { v: *v, margin: 0.0 };
            if let Some(r) = gjk_pair(
                Placed {
                    shape: convex,
                    t: *tc,
                },
                Placed { shape: &t, t: *tt },
                max_distance2,
            ) {
                manifold.add_contact_point(r.normal, r.point_on_b, r.distance);
            }
        }
    };

    match concave {
        Body::Mesh(m, _) => m.process_all_triangles(lo, hi, &mut triangle),
        Body::Height(h, _) => h.process_all_triangles(lo, hi, &mut triangle),
        Body::Convex(..) => unreachable!(),
    }

    manifold.refresh();
    manifold
}

/// The leaf pair algorithm of btDefaultCollisionConfiguration for body0 `a` and body1 `b` with
/// the manifold's contact breaking threshold, after the children's AABBs overlap. Box–box uses
/// the caller's dBoxBox2 test (`box_box`).
pub fn pair_manifold(
    a: &Body,
    b: &Body,
    threshold: f64,
    box_box: impl Fn(&Transform, V, &Transform, V) -> bool,
) -> Manifold {
    match (a, b) {
        (Body::Convex(sa, ta), Body::Convex(sb, tb)) => {
            let mut m = Manifold::new(threshold);

            match (sa, sb) {
                (Convex::Sphere { radius: r0 }, Convex::Sphere { radius: r1 }) => {
                    // btSphereSphereCollisionAlgorithm.
                    let diff = sub(ta.origin, tb.origin);
                    let l = len(diff);
                    if l <= r0 + r1 {
                        let n = if l > SIMD_EPSILON {
                            mul(diff, 1.0 / l)
                        } else {
                            [1.0, 0.0, 0.0]
                        };
                        m.add_contact_point(n, add(tb.origin, mul(n, *r1)), l - (r0 + r1));
                    }
                }
                (
                    Convex::Box {
                        half: h0,
                        margin: m0,
                    },
                    Convex::Box {
                        half: h1,
                        margin: m1,
                    },
                ) => {
                    if box_box(ta, add(*h0, [*m0; 3]), tb, add(*h1, [*m1; 3])) {
                        m.points.push(ManifoldPoint {
                            a: ta.origin,
                            b: tb.origin,
                            normal: [0.0; 3],
                            distance: 0.0,
                        });
                        return m;
                    }
                }
                _ => {
                    let max_distance2 = (sa.margin() + sb.margin() + m.threshold).powi(2);
                    if let Some(r) = gjk_pair(
                        Placed { shape: sa, t: *ta },
                        Placed { shape: sb, t: *tb },
                        max_distance2,
                    ) {
                        m.add_contact_point(r.normal, r.point_on_b, r.distance);
                    }
                }
            }

            m.refresh();
            m
        }
        (Body::Convex(s, t), concave) | (concave, Body::Convex(s, t)) => {
            convex_concave(s, t, concave, threshold)
        }
        _ => Manifold::new(threshold),
    }
}
