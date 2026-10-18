//! btGjkPairDetector.getClosestPointsNonVirtual with btVoronoiSimplexSolver, rounded the way the
//! Ammo asm.js build rounds it: doubles in locals, float rounding where Bullet stores into a
//! struct member (vertices, closest points, barycentric weights, the separating axis).
//! The EPA fallback is `crate::bullet::calc_pen_depth`, a transcription of the same build's btGjkEpaSolver2.

use crate::bullet::{self, add, closest_point_triangle, cross, dot, len2, mul, sub, ClosestPoints, Convex, Placed, SubResult, Transform, V};

fn f(x: f64) -> f64 {
    x as f32 as f64
}

fn fv(v: V) -> V {
    v.map(f)
}

fn neg(a: V) -> V {
    [-a[0], -a[1], -a[2]]
}

/// SIMD_EPSILON.
const EPS: f64 = f32::EPSILON as f64;
/// BT_LARGE_FLOAT as a float: 999999984306749440.0.
const LARGE: f64 = 1e18_f32 as f64;
/// REL_ERROR2 as a float.
const REL_ERROR2: f64 = 1e-6_f32 as f64;
/// m_equalVertexThreshold as a float.
const EQUAL_VERTEX_THRESHOLD: f64 = 1e-4_f32 as f64;

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
    /// m_cachedBC.m_degenerate
    degenerate_bc: bool,
}

impl Voronoi {
    fn new() -> Voronoi {
        Voronoi { n: 0, w: [[0.0; 3]; 5], p: [[0.0; 3]; 5], q: [[0.0; 3]; 5], cp1: [0.0; 3], cp2: [0.0; 3], cv: [0.0; 3], last_w: [f(1e30); 3], valid: false, bc: SubResult::default(), needs_update: true, degenerate_bc: false }
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

    /// `w` is the unrounded difference; the stored copies are floats.
    fn add_vertex(&mut self, w: V, p: V, q: V) {
        self.last_w = fv(w);
        self.needs_update = true;
        self.w[self.n] = fv(w);
        self.p[self.n] = fv(p);
        self.q[self.n] = fv(q);
        self.n += 1;
    }

    fn update(&mut self) -> bool {
        if !self.needs_update {
            return self.valid;
        }

        self.bc = SubResult::default();
        self.degenerate_bc = false;
        self.needs_update = false;

        match self.n {
            0 => self.valid = false,
            1 => {
                self.cp1 = self.p[0];
                self.cp2 = self.q[0];
                self.cv = sub(self.cp1, self.cp2).map(f);
                self.bc.bary = [1.0, 0.0, 0.0, 0.0];
                self.valid = true;
            }
            2 => {
                let from = self.w[0];
                let v = sub(self.w[1], from);
                let tn = dot(neg(from), v);
                let mut used = [false; 4];
                let t;

                if tn > 0.0 {
                    let vv = dot(v, v);
                    if tn < vv {
                        t = tn / vv;
                        used[0] = true;
                        used[1] = true;
                    } else {
                        t = 1.0;
                        used[1] = true;
                    }
                } else {
                    t = 0.0;
                    used[0] = true;
                }

                let e = 1.0 - t;
                self.bc.bary = [f(e), f(t), 0.0, 0.0];
                let a = add(self.p[0], mul(sub(self.p[1], self.p[0]), t));
                let b = add(self.q[0], mul(sub(self.q[1], self.q[0]), t));
                self.cp1 = fv(a);
                self.cp2 = fv(b);
                self.cv = fv(sub(a, b));
                self.reduce(used);
                self.valid = t >= 0.0 && e >= 0.0;
            }
            3 => {
                let r = closest_point_triangle([0.0; 3], self.w[0], self.w[1], self.w[2]);
                let b = [f(r.bary[0]), f(r.bary[1]), f(r.bary[2])];
                self.bc.bary = [b[0], b[1], b[2], 0.0];
                let a = add(add(mul(self.p[0], b[0]), mul(self.p[1], b[1])), mul(self.p[2], b[2]));
                let c = add(add(mul(self.q[0], b[0]), mul(self.q[1], b[1])), mul(self.q[2], b[2]));
                self.cp1 = fv(a);
                self.cp2 = fv(c);
                self.cv = fv(sub(a, c));
                self.reduce(r.used);
                self.valid = b[0] >= 0.0 && b[1] >= 0.0 && b[2] >= 0.0;
            }
            4 => {
                let (separated, r) = closest_point_tetrahedron([0.0; 3], self.w[0], self.w[1], self.w[2], self.w[3]);
                self.bc = r;
                self.bc.bary = self.bc.bary.map(f);

                if separated {
                    let b = self.bc.bary;
                    let a = (0..4).fold([0.0; 3], |s, i| add(s, mul(self.p[i], b[i])));
                    let c = (0..4).fold([0.0; 3], |s, i| add(s, mul(self.q[i], b[i])));
                    self.cp1 = fv(a);
                    self.cp2 = fv(c);
                    self.cv = fv(sub(a, c));
                    self.reduce(self.bc.used);
                    self.valid = b.iter().all(|&x| x >= 0.0);
                } else if self.bc.degenerate {
                    self.valid = false;
                } else {
                    self.valid = true;
                    self.cv = [0.0; 3];
                }
            }
            _ => self.valid = false,
        }

        self.valid
    }

    /// The vertex w (unrounded) is already in the simplex or equal to the last one added.
    fn in_simplex(&self, w: V) -> bool {
        let found = (0..self.n).any(|i| len2(sub(w, self.w[i])) <= EQUAL_VERTEX_THRESHOLD);
        w == self.last_w || found
    }
}

/// localGetSupportingVertexWithoutMargin, stored to a float vector.
fn support(s: &Convex, d: V) -> V {
    fv(s.support(d))
}

/// m_maximumDistanceSquared is a float member of the input.
pub fn gjk_pair(a: Placed, b: Placed, max_distance2: f64) -> Option<ClosestPoints> {
    let max_distance2 = f(max_distance2);
    let (oa, ob) = (a.t.origin, b.t.origin);
    let offset = mul(add(oa, ob), 0.5);
    let ta = Transform { basis: a.t.basis, origin: fv(sub(oa, offset)) };
    let tb = Transform { basis: b.t.basis, origin: fv(sub(ob, offset)) };
    let (sa, sb) = (a.shape, b.shape);
    let margin_a = sa.margin();
    let margin_b = sb.margin();
    let margin = margin_a + margin_b;

    let mut axis: V = [0.0, 1.0, 0.0];
    let mut degenerate = 0;
    let mut distance = 0.0;
    let mut check_simplex = false;
    let mut solver = Voronoi::new();
    let mut l = LARGE;

    for iteration in 0.. {
        let pa = ta.inv_rotate(neg(axis)).map(f);
        let pb = tb.inv_rotate(axis).map(f);
        let p_world = ta.apply(support(sa, pa));
        let q_world = tb.apply(support(sb, pb));
        let w = sub(p_world, q_world);
        let delta = dot(w, axis);

        if delta > 0.0 && delta * delta > l * max_distance2 {
            degenerate = 10;
            check_simplex = true;
            break;
        }

        if solver.in_simplex(w) {
            degenerate = 1;
            check_simplex = true;
            break;
        }

        let f0 = l - delta;

        if f0 <= l * REL_ERROR2 {
            degenerate = if f0 <= 0.0 { 2 } else { 11 };
            check_simplex = true;
            break;
        }

        solver.add_vertex(w, p_world, q_world);
        let ok = solver.update();
        let new_axis = solver.cv;

        if !ok {
            degenerate = 3;
            check_simplex = true;
            break;
        }

        let nl = len2(new_axis);

        if nl < REL_ERROR2 {
            axis = new_axis;
            degenerate = 6;
            check_simplex = true;
            break;
        }

        if l - nl <= EPS * l {
            degenerate = 12;
            l = nl;
            check_simplex = true;
            break;
        }

        axis = new_axis;

        if iteration > 1000 {
            l = nl;
            break;
        }

        if solver.n == 4 {
            degenerate = 13;
            l = nl;
            break;
        }

        l = nl;
    }

    let mut normal = [0.0; 3];
    let mut point_b = [0.0; 3];
    let mut valid = false;

    if check_simplex {
        solver.update();
        let cp2 = solver.cp2;
        normal = axis;
        let a2 = len2(axis);

        if a2 < 0.0001 {
            degenerate = 5;
        }

        if a2 > EPS * EPS {
            let y = 1.0 / a2.sqrt();
            normal = fv(mul(axis, y));
            let z = margin_b / l.sqrt();
            point_b = add(mul(axis, z), cp2);
            distance = 1.0 / y - margin;
            valid = true;
        }
    }

    let catch = degenerate != 0 && margin + distance < 0.01;

    if !valid || catch {
        let (valid2, pen_axis, tmp_a, tmp_b) = bullet::calc_pen_depth(sa, &ta, sb, &tb);
        let (tmp_a, tmp_b, pen_axis) = (fv(tmp_a), fv(tmp_b), fv(pen_axis));

        if valid2 {
            let mut tn = sub(tmp_b, tmp_a);
            let mut l2 = len2(tn);

            if l2 <= EPS * EPS {
                tn = pen_axis;
                l2 = len2(pen_axis);
            }

            if l2 > EPS * EPS {
                let v = 1.0 / l2.sqrt();
                let d2 = -len2(sub(tmp_a, tmp_b)).sqrt();

                if !valid || d2 < distance {
                    distance = d2;
                    point_b = tmp_b;
                    normal = fv(mul(tn, v));
                    valid = true;
                }
            }
        } else if len2(pen_axis) > 0.0 {
            let d2 = len2(sub(tmp_a, tmp_b)).sqrt() - margin;

            if !valid || d2 < distance {
                distance = d2;
                point_b = add(mul(pen_axis, margin_b), tmp_b);
                let v = 1.0 / len2(pen_axis).sqrt();
                normal = fv(mul(pen_axis, v));
                valid = true;
            }
        }
    }

    if !valid {
        return None;
    }

    if !(distance < 0.0 || distance * distance < max_distance2) {
        return None;
    }

    let center = |s: &Convex, t: &Transform| {
        let (lo, hi) = s.aabb(t);
        mul(add(hi, lo), 0.5)
    };

    if dot(sub(center(sa, &ta), center(sb, &tb)), normal) < 0.0 {
        normal = neg(normal);
    }

    let _ = cross;
    Some(ClosestPoints { normal, point_on_b: add(point_b, offset), distance })
}

/// btVoronoiSimplexSolver.pointOutsideOfPlane. The degenerate limit is the float product
/// 1e-4f * 1e-4f.
fn point_outside_of_plane(p: V, a: V, b: V, c: V, d: V) -> i32 {
    let normal = cross(sub(b, a), sub(c, a));
    let signp = dot(sub(p, a), normal);
    let signd = dot(sub(d, a), normal);

    if signd * signd < 9.99999905104687e-09 {
        return -1;
    }

    (signp * signd < 0.0) as i32
}

/// btVoronoiSimplexSolver.closestPtPointTetrahedron: (separated, result). The closest points of
/// the faces are stored as floats before they are compared.
fn closest_point_tetrahedron(p: V, a: V, b: V, c: V, d: V) -> (bool, SubResult) {
    let mut fin = SubResult { closest: p, used: [true; 4], ..SubResult::default() };
    let outside = [point_outside_of_plane(p, a, b, c, d), point_outside_of_plane(p, a, c, d, b), point_outside_of_plane(p, a, d, b, c), point_outside_of_plane(p, b, d, c, a)];

    if outside.iter().any(|&o| o < 0) {
        fin.degenerate = true;
        return (false, fin);
    }

    if outside.iter().all(|&o| o == 0) {
        return (false, fin);
    }

    let mut best = f32::MAX as f64;
    let faces = [([a, b, c], [0, 1, 2]), ([a, c, d], [0, 2, 3]), ([a, d, b], [0, 3, 1]), ([b, d, c], [1, 3, 2])];

    for (k, (t, map)) in faces.iter().enumerate() {
        if outside[k] == 0 {
            continue;
        }

        let r = closest_point_triangle(p, t[0], t[1], t[2]);
        let q = fv(r.closest);
        let diff = sub(q, p);
        let dist = dot(diff, diff);

        if dist < best {
            best = dist;
            fin.closest = q;
            fin.used = [false; 4];
            fin.bary = [0.0; 4];
            for j in 0..3 {
                fin.used[map[j]] = r.used[j];
                fin.bary[map[j]] = f(r.bary[j]);
            }
        }
    }

    (true, fin)
}
