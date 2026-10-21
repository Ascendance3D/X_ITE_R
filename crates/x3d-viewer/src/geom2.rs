//! The rest of X_ITE's `standard/Math` geometry and algorithms, ported line for line (same algorithms, same operation
//! order, f64): Geometry/Box2.js, Triangle2.js, Spheroid3.js, Box3.js beyond what lookat.rs needs, Camera.js frustum /
//! perspective / perspective2 / orthoBox, the ViewVolume instance (set, getViewport, getEdges, intersectsSphere,
//! intersectsBox), Algorithms/SAT.js, Bezier.js, eigen_decomposition.js, MergeSort.js, QuickSort.js.
//!
//! Compared bit for bit with the pinned X_ITE modules: tests/geom_math.rs, oracle/geom-math-baseline.json written by
//! oracle/tools/geom_xite.mjs. Conventions as in geom.rs.

use crate::geom::{Camera, Plane3, Triangle3, ViewVolume};
use crate::lookat::Box3;
use x3d_fields::jsmath;
use x3d_fields::math::{Matrix3, Matrix4, Vector2, Vector3, Vector4};

// ---------------------------------------------------------------------------------------------
// Box2
// ---------------------------------------------------------------------------------------------

/// Box2.js: a Matrix3 whose axes are the half extents and whose origin is the center.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Box2 {
    pub matrix: Matrix3,
}

impl Default for Box2 {
    /// `new Box2 ()`: `set ()` without arguments, the zero matrix (an empty box).
    fn default() -> Self {
        Box2 { matrix: Matrix3([0.0; 9]) }
    }
}

impl Box2 {
    /// `new Box2 (size, center)`.
    pub fn new(size: &Vector2, center: &Vector2) -> Self {
        let mut b = Box2::default();
        b.set(Some((size, center)));
        b
    }

    pub fn copy(&self) -> Self {
        *self
    }

    pub fn assign(&mut self, b: &Box2) -> &mut Self {
        self.matrix = b.matrix;
        self
    }

    pub fn equals(&self, b: &Box2) -> bool {
        self.matrix.equals(&b.matrix)
    }

    /// Box2.set: with size and center, or the zero matrix.
    pub fn set(&mut self, size_center: Option<(&Vector2, &Vector2)>) -> &mut Self {
        match size_center {
            Some((size, center)) => self.matrix.0 = [size.x / 2.0, 0.0, 0.0, 0.0, size.y / 2.0, 0.0, center.x, center.y, 1.0],
            None => self.matrix.0 = [0.0; 9],
        }

        self
    }

    pub fn set_extents(&mut self, min: &Vector2, max: &Vector2) -> &mut Self {
        let sx = (max.x - min.x) / 2.0;
        let sy = (max.y - min.y) / 2.0;
        let cx = (max.x + min.x) / 2.0;
        let cy = (max.y + min.y) / 2.0;

        self.matrix.0 = [sx, 0.0, 0.0, 0.0, sy, 0.0, cx, cy, 1.0];
        self
    }

    pub fn is_empty(&self) -> bool {
        self.matrix.0[8] == 0.0
    }

    pub fn add(&mut self, b: &Box2) -> &mut Self {
        if self.is_empty() {
            return self.assign(b);
        }

        if b.is_empty() {
            return self;
        }

        let (mut lhs_min, mut lhs_max) = self.extents();
        let (rhs_min, rhs_max) = b.extents();

        lhs_min.min(&rhs_min);
        lhs_max.max(&rhs_max);
        self.set_extents(&lhs_min, &lhs_max)
    }

    pub fn mult_left(&mut self, matrix: &Matrix3) -> &mut Self {
        self.matrix.mult_left(matrix);
        self
    }

    pub fn mult_right(&mut self, matrix: &Matrix3) -> &mut Self {
        self.matrix.mult_right(matrix);
        self
    }

    /// Box2.getExtents.
    pub fn extents(&self) -> (Vector2, Vector2) {
        let (mut min, mut max) = self.absolute_extents();
        let c = self.center();

        min.add(&c);
        max.add(&c);
        (min, max)
    }

    /// Box2.getAbsoluteExtents.
    pub fn absolute_extents(&self) -> (Vector2, Vector2) {
        let m = &self.matrix.0;
        let x = Vector2::new(m[0], m[1]);
        let y = Vector2::new(m[3], m[4]);

        let mut p1 = x;
        p1.add(&y);

        let mut p2 = y;
        p2.subtract(&x);

        let mut min = p1;
        min.min(&p2);
        let mut max = p1;
        max.max(&p2);

        p1.negate();
        p2.negate();

        min.min(&p1).min(&p2);
        max.max(&p1).max(&p2);
        (min, max)
    }

    pub fn contains_point(&self, point: &Vector2) -> bool {
        let (min, max) = self.extents();

        min.x <= point.x && max.x >= point.x && min.y <= point.y && max.y >= point.y
    }

    pub fn size(&self) -> Vector2 {
        let (min, mut max) = self.absolute_extents();
        max.subtract(&min);
        max
    }

    pub fn center(&self) -> Vector2 {
        Vector2::new(self.matrix.0[6], self.matrix.0[7])
    }

    pub fn from_extents(min: &Vector2, max: &Vector2) -> Self {
        let mut b = Box2::default();
        b.set_extents(min, max);
        b
    }

    pub fn from_points(points: &[Vector2]) -> Self {
        if points.is_empty() {
            return Box2::default();
        }

        let mut min = Vector2::new(f64::INFINITY, f64::INFINITY);
        let mut max = Vector2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);

        for p in points {
            min.min(p);
            max.max(p);
        }

        Box2::from_extents(&min, &max)
    }
}

// ---------------------------------------------------------------------------------------------
// Triangle2, Spheroid3
// ---------------------------------------------------------------------------------------------

/// Triangle2.js
pub struct Triangle2;

/// Triangle2.toBarycentric's result.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Barycentric {
    pub u: f64,
    pub v: f64,
    pub t: f64,
}

impl Triangle2 {
    pub fn area(a: &Vector2, b: &Vector2, c: &Vector2) -> f64 {
        (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y)).abs() / 2.0
    }

    pub fn is_point_in_triangle(p: &Vector2, a: &Vector2, b: &Vector2, c: &Vector2) -> bool {
        let det = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);

        if det == 0.0 {
            return false;
        }

        let u = ((b.y - c.y) * (p.x - c.x) + (c.x - b.x) * (p.y - c.y)) / det;

        if u < 0.0 || u > 1.0 {
            return false;
        }

        let v = ((c.y - a.y) * (p.x - c.x) + (a.x - c.x) * (p.y - c.y)) / det;

        if v < 0.0 || v > 1.0 {
            return false;
        }

        let t = 1.0 - u - v;

        !(t < 0.0 || t > 1.0)
    }

    pub fn to_barycentric(point: &Vector2, a: &Vector2, b: &Vector2, c: &Vector2) -> Barycentric {
        let mut v0 = *b;
        v0.subtract(a);
        let mut v1 = *c;
        v1.subtract(a);
        let mut v2 = *point;
        v2.subtract(a);

        let d00 = v0.dot(&v0);
        let d01 = v0.dot(&v1);
        let d11 = v1.dot(&v1);
        let d20 = v2.dot(&v0);
        let d21 = v2.dot(&v1);
        let denom = d00 * d11 - d01 * d01;

        let v = (d11 * d20 - d01 * d21) / denom;
        let t = (d00 * d21 - d01 * d20) / denom;

        Barycentric { u: 1.0 - v - t, v, t }
    }
}

/// Spheroid3.js
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spheroid3 {
    pub semi_major_axis: f64,
    pub semi_minor_axis: f64,
}

impl Spheroid3 {
    pub fn new(semi_major_axis: f64, semi_minor_axis: f64, use_f_1: bool) -> Self {
        let mut s = Spheroid3 { semi_major_axis: 1.0, semi_minor_axis: 1.0 };
        s.set(semi_major_axis, semi_minor_axis, use_f_1);
        s
    }

    pub fn set(&mut self, semi_major_axis: f64, semi_minor_axis: f64, use_f_1: bool) -> &mut Self {
        if use_f_1 {
            let f_1 = semi_minor_axis;

            self.semi_major_axis = semi_major_axis;
            self.semi_minor_axis = semi_major_axis * (1.0 - (1.0 / f_1));
        } else {
            self.semi_major_axis = semi_major_axis;
            self.semi_minor_axis = semi_minor_axis;
        }

        self
    }

    pub fn equals(&self, s: &Spheroid3) -> bool {
        self.semi_major_axis == s.semi_major_axis && self.semi_minor_axis == s.semi_minor_axis
    }
}

// ---------------------------------------------------------------------------------------------
// Box3 (the part lookat.rs does not need)
// ---------------------------------------------------------------------------------------------

fn axis(m: &Matrix4, i: usize) -> Vector3 {
    Vector3::new(m.0[i * 4], m.0[i * 4 + 1], m.0[i * 4 + 2])
}

impl Default for Box3 {
    /// `new Box3 ()`: the zero matrix (an empty box).
    fn default() -> Self {
        Box3 { matrix: Matrix4([0.0; 16]) }
    }
}

impl Box3 {
    /// Box3.set (size, center), or `set ()`: the zero matrix.
    pub fn set(&mut self, size_center: Option<(&Vector3, &Vector3)>) -> &mut Self {
        match size_center {
            Some((s, c)) => {
                self.matrix.0 = [s.x / 2.0, 0.0, 0.0, 0.0, 0.0, s.y / 2.0, 0.0, 0.0, 0.0, 0.0, s.z / 2.0, 0.0, c.x, c.y, c.z, 1.0];
            }
            None => self.matrix.0 = [0.0; 16],
        }

        self
    }

    pub fn equals(&self, b: &Box3) -> bool {
        self.matrix.equals(&b.matrix)
    }

    pub fn set_extents(&mut self, min: &Vector3, max: &Vector3) -> &mut Self {
        *self = Box3::from_extents(min, max);
        self
    }

    /// Box3.setPoints.
    pub fn set_points(&mut self, points: &[Vector3]) -> &mut Self {
        if points.is_empty() {
            return self.set(None);
        }

        let mut min = Vector3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut max = Vector3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);

        for p in points {
            min.min(p);
            max.max(p);
        }

        self.set_extents(&min, &max)
    }

    /// Box3.setArray (array, stride).
    pub fn set_array(&mut self, array: &[f64], stride: usize) -> &mut Self {
        let length = array.len();

        if length == 0 {
            return self.set(None);
        }

        let (mut min_x, mut min_y, mut min_z) = (f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let (mut max_x, mut max_y, mut max_z) = (f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        // array [i + n] beyond the end is undefined: every comparison with it is false.
        let at = |i: usize| array.get(i).copied().unwrap_or(f64::NAN);
        let mut i = 0;

        while i < length {
            let (x, y, z) = (at(i), at(i + 1), at(i + 2));

            if x < min_x {
                min_x = x;
            }
            if y < min_y {
                min_y = y;
            }
            if z < min_z {
                min_z = z;
            }

            if x > max_x {
                max_x = x;
            }
            if y > max_y {
                max_y = y;
            }
            if z > max_z {
                max_z = z;
            }

            i += stride;
        }

        self.set_extents(&Vector3::new(min_x, min_y, min_z), &Vector3::new(max_x, max_y, max_z))
    }

    pub fn from_points(points: &[Vector3]) -> Self {
        let mut b = Box3::default();
        b.set_points(points);
        b
    }

    pub fn from_array(array: &[f64], stride: usize) -> Self {
        let mut b = Box3::default();
        b.set_array(array, stride);
        b
    }

    /// Box3.getPoints: the eight corners.
    pub fn points(&self) -> [Vector3; 8] {
        let m = &self.matrix;
        let x = axis(m, 0);
        let y = axis(m, 1);
        let mut z = axis(m, 2);

        let mut r1 = y;
        r1.add(&z);

        let r2 = *z.subtract(&y);

        let mut p = [Vector3::default(); 8];

        p[0] = *Vector3::default().assign(&x).add(&r1);
        p[1] = *Vector3::default().assign(&r1).subtract(&x);
        p[2] = *Vector3::default().assign(&r2).subtract(&x);
        p[3] = *Vector3::default().assign(&x).add(&r2);

        p[4] = *Vector3::default().assign(&p[2]).negate();
        p[5] = *Vector3::default().assign(&p[3]).negate();
        p[6] = *Vector3::default().assign(&p[0]).negate();
        p[7] = *Vector3::default().assign(&p[1]).negate();

        let center = self.center();

        for q in &mut p {
            q.add(&center);
        }

        p
    }

    pub fn axes(&self) -> [Vector3; 3] {
        [axis(&self.matrix, 0), axis(&self.matrix, 1), axis(&self.matrix, 2)]
    }

    /// Box3.getNormals: the face normals, also of a flat or degenerate box.
    pub fn normals(&self) -> [Vector3; 3] {
        let m = &self.matrix;
        let mut x = axis(m, 0);
        let mut y = axis(m, 1);
        let mut z = axis(m, 2);
        let axes = [Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 1.0, 0.0), Vector3::new(0.0, 0.0, 1.0)];

        if x.squared_norm() == 0.0 {
            x.assign(&y).cross(&z);

            if x.squared_norm() == 0.0 {
                for a in &axes {
                    x.assign(a).cross(&y);

                    if x.squared_norm() != 0.0 {
                        break;
                    }
                }

                if x.squared_norm() == 0.0 {
                    for a in &axes {
                        x.assign(a).cross(&z);

                        if x.squared_norm() != 0.0 {
                            break;
                        }
                    }

                    if x.squared_norm() == 0.0 {
                        x = axes[0];
                    }
                }
            }
        }

        if y.squared_norm() == 0.0 {
            y.assign(&z).cross(&x);

            if y.squared_norm() == 0.0 {
                for a in &axes {
                    y.assign(a).cross(&z);

                    if y.squared_norm() != 0.0 {
                        break;
                    }
                }

                if y.squared_norm() == 0.0 {
                    for a in &axes {
                        y.assign(a).cross(&x);

                        if y.squared_norm() != 0.0 {
                            break;
                        }
                    }

                    if y.squared_norm() == 0.0 {
                        y = axes[1];
                    }
                }
            }
        }

        if z.squared_norm() == 0.0 {
            z.assign(&x).cross(&y);

            if z.squared_norm() == 0.0 {
                for a in &axes {
                    z.assign(a).cross(&x);

                    if z.squared_norm() != 0.0 {
                        break;
                    }
                }

                if z.squared_norm() == 0.0 {
                    for a in &axes {
                        z.assign(a).cross(&y);

                        if z.squared_norm() != 0.0 {
                            break;
                        }
                    }

                    if z.squared_norm() == 0.0 {
                        z = axes[2];
                    }
                }
            }
        }

        [*Vector3::default().assign(&y).cross(&z).normalize(), *Vector3::default().assign(&z).cross(&x).normalize(), *Vector3::default().assign(&x).cross(&y).normalize()]
    }

    pub fn is_empty(&self) -> bool {
        self.matrix.0[15] == 0.0
    }

    pub fn add(&mut self, b: &Box3) -> &mut Self {
        if self.is_empty() {
            self.matrix = b.matrix;
            return self;
        }

        if b.is_empty() {
            return self;
        }

        let (mut lhs_min, mut lhs_max) = self.extents();
        let (rhs_min, rhs_max) = b.extents();

        lhs_min.min(&rhs_min);
        lhs_max.max(&rhs_max);
        self.set_extents(&lhs_min, &lhs_max)
    }

    pub fn mult_left(&mut self, matrix: &Matrix4) -> &mut Self {
        self.matrix.mult_left(matrix);
        self
    }

    pub fn contains_point(&self, point: &Vector3) -> bool {
        let (min, max) = self.extents();

        min.x <= point.x && max.x >= point.x && min.y <= point.y && max.y >= point.y && min.z <= point.z && max.z >= point.z
    }

    /// Box3.intersectsBox (SAT over the face normals of both boxes and the nine edge crosses).
    pub fn intersects_box(&self, other: &Box3) -> bool {
        if self.is_empty() || other.is_empty() {
            return false;
        }

        let points1 = self.points();
        let points2 = other.points();

        if Sat::is_separated(&self.normals(), &points1, &points2) {
            return false;
        }

        if Sat::is_separated(&other.normals(), &points1, &points2) {
            return false;
        }

        let axes1 = self.axes();
        let axes2 = other.axes();
        let mut axes9 = [Vector3::default(); 9];

        for i1 in 0..3 {
            for i2 in 0..3 {
                axes9[i1 * 3 + i2] = *Vector3::default().assign(&axes1[i1]).cross(&axes2[i2]);
            }
        }

        !Sat::is_separated(&axes9, &points1, &points2)
    }
}

// ---------------------------------------------------------------------------------------------
// SAT
// ---------------------------------------------------------------------------------------------

/// Algorithms/SAT.js
pub struct Sat;

impl Sat {
    pub fn is_separated(axes: &[Vector3], points1: &[Vector3], points2: &[Vector3]) -> bool {
        for axis in axes {
            let (min1, max1) = Self::project(points1, axis);
            let (min2, max2) = Self::project(points2, axis);

            if Self::overlaps(min1, max1, min2, max2) {
                continue;
            }

            return true;
        }

        false
    }

    fn project(points: &[Vector3], axis: &Vector3) -> (f64, f64) {
        let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);

        for p in points {
            let d = p.dot(axis);

            if d < min {
                min = d;
            }

            if d > max {
                max = d;
            }
        }

        (min, max)
    }

    fn overlaps(min1: f64, max1: f64, min2: f64, max2: f64) -> bool {
        Self::is_between(min2, min1, max1) || Self::is_between(min1, min2, max2)
    }

    fn is_between(value: f64, lower: f64, upper: f64) -> bool {
        lower <= value && value <= upper
    }
}

// ---------------------------------------------------------------------------------------------
// Camera
// ---------------------------------------------------------------------------------------------

impl Camera {
    /// Camera.js:frustum.
    pub fn frustum(l: f64, r: f64, b: f64, t: f64, n: f64, f: f64, matrix: &mut Matrix4) -> &mut Matrix4 {
        let r_l = r - l;
        let t_b = t - b;
        let f_n = f - n;
        let n_2 = 2.0 * n;

        let a = (r + l) / r_l;
        let bb = (t + b) / t_b;
        let c = -(f + n) / f_n;
        let d = -(n_2 * f) / f_n;
        let e = n_2 / r_l;
        let ff = n_2 / t_b;

        matrix.0 = [e, 0.0, 0.0, 0.0, 0.0, ff, 0.0, 0.0, a, bb, c, -1.0, 0.0, 0.0, d, 0.0];
        matrix
    }

    /// Camera.js:perspective.
    pub fn perspective(field_of_view: f64, z_near: f64, z_far: f64, width: f64, height: f64, matrix: &mut Matrix4) -> &mut Matrix4 {
        let ratio = jsmath::tan(field_of_view / 2.0) * z_near;

        if width > height {
            let aspect = width * ratio / height;
            Self::frustum(-aspect, aspect, -ratio, ratio, z_near, z_far, matrix)
        } else {
            let aspect = height * ratio / width;
            Self::frustum(-ratio, ratio, -aspect, aspect, z_near, z_far, matrix)
        }
    }

    /// Camera.js:perspective2.
    pub fn perspective2(field_of_view: f64, z_near: f64, z_far: f64, matrix: &mut Matrix4) -> &mut Matrix4 {
        let ratio = jsmath::tan(field_of_view / 2.0) * z_near;

        Self::frustum(-ratio, ratio, -ratio, ratio, z_near, z_far, matrix)
    }

    /// Camera.js:orthoBox.
    pub fn ortho_box<'a>(b: &Box3, matrix: &'a mut Matrix4) -> &'a mut Matrix4 {
        let (min, max) = b.extents();

        Self::ortho(min.x, max.x, min.y, max.y, -max.z, -min.z, matrix)
    }
}

// ---------------------------------------------------------------------------------------------
// ViewVolume (instance)
// ---------------------------------------------------------------------------------------------

/// A ViewVolume object (`new ViewVolume (projectionMatrix, viewport)`): its corners, face normals, planes and edges.
#[derive(Clone, Debug)]
pub struct ViewVolumeObject {
    pub viewport: Vector4,
    pub points: [Vector3; 8],
    /// front, left, right, top, bottom, back
    pub normals: [Vector3; 6],
    pub planes: [Plane3; 6],
    edges: [Vector3; 8],
    edges_tainted: bool,
}

impl Default for ViewVolumeObject {
    fn default() -> Self {
        let plane = Plane3::new(Vector3::default(), Vector3::new(0.0, 0.0, 1.0));

        ViewVolumeObject {
            viewport: Vector4::default(),
            points: [Vector3::default(); 8],
            normals: [Vector3::default(); 6],
            planes: [plane; 6],
            edges: [Vector3::default(); 8],
            edges_tainted: false,
        }
    }
}

impl ViewVolumeObject {
    pub fn new(projection_matrix: &Matrix4, viewport: &Vector4) -> Self {
        let mut v = ViewVolumeObject::default();
        v.set(projection_matrix, viewport);
        v
    }

    /// ViewVolume.set.
    pub fn set(&mut self, projection_matrix: &Matrix4, viewport: &Vector4) -> &mut Self {
        self.viewport = *viewport;

        let x1 = viewport.x;
        let x2 = x1 + viewport.z;
        let y1 = viewport.y;
        let y2 = y1 + viewport.w;

        let mut matrix = *projection_matrix;
        matrix.inverse();

        let p = &mut self.points;
        let corners = [(x1, y1, 0.0), (x2, y1, 0.0), (x2, y2, 0.0), (x1, y2, 0.0), (x1, y1, 1.0), (x2, y1, 1.0), (x2, y2, 1.0), (x1, y2, 1.0)];

        for (i, (x, y, z)) in corners.into_iter().enumerate() {
            ViewVolume::unproject_point_matrix(x, y, z, &matrix, viewport, &mut p[i]);
        }

        let p = self.points;
        let n = &mut self.normals;

        Triangle3::normal(&p[0], &p[1], &p[2], &mut n[0]);
        Triangle3::normal(&p[7], &p[4], &p[0], &mut n[1]);
        Triangle3::normal(&p[6], &p[2], &p[1], &mut n[2]);
        Triangle3::normal(&p[2], &p[6], &p[7], &mut n[3]);
        Triangle3::normal(&p[1], &p[0], &p[4], &mut n[4]);
        Triangle3::normal(&p[4], &p[7], &p[6], &mut n[5]);

        let n = self.normals;

        self.planes[0].set(&p[1], &n[0]);
        self.planes[1].set(&p[4], &n[1]);
        self.planes[2].set(&p[2], &n[2]);
        self.planes[3].set(&p[6], &n[3]);
        self.planes[4].set(&p[0], &n[4]);
        self.planes[5].set(&p[7], &n[5]);

        self.edges_tainted = true;
        self
    }

    pub fn viewport(&self) -> &Vector4 {
        &self.viewport
    }

    /// ViewVolume.getEdges.
    pub fn edges(&mut self) -> [Vector3; 8] {
        if self.edges_tainted {
            let p = self.points;
            let e = &mut self.edges;

            e[0] = *Vector3::default().assign(&p[0]).subtract(&p[1]);
            e[1] = *Vector3::default().assign(&p[1]).subtract(&p[2]);
            e[2] = *Vector3::default().assign(&p[2]).subtract(&p[3]);
            e[3] = *Vector3::default().assign(&p[3]).subtract(&p[0]);

            e[4] = *Vector3::default().assign(&p[0]).subtract(&p[4]);
            e[5] = *Vector3::default().assign(&p[1]).subtract(&p[5]);
            e[6] = *Vector3::default().assign(&p[2]).subtract(&p[6]);
            e[7] = *Vector3::default().assign(&p[3]).subtract(&p[7]);

            self.edges_tainted = false;
        }

        self.edges
    }

    /// ViewVolume.intersectsSphere.
    pub fn intersects_sphere(&self, radius: f64, center: &Vector3) -> bool {
        !self.planes.iter().any(|plane| plane.get_distance_to_point(center) > radius)
    }

    /// ViewVolume.intersectsBox. X_ITE writes the 24 edge crosses at `i1 * 3 + i2` (i2 < 8) into an array that keeps its
    /// entries between calls: entries 0 … 13 are written (later writes win), 14 … 23 stay zero vectors.
    pub fn intersects_box(&mut self, b: &Box3) -> bool {
        let points1 = b.points();
        let points2 = self.points;

        if Sat::is_separated(&b.normals(), &points1, &points2) {
            return false;
        }

        if Sat::is_separated(&self.normals, &points1, &points2) {
            return false;
        }

        let axes1 = b.axes();
        let edges = self.edges();
        let mut axes = [Vector3::default(); 24];

        for i1 in 0..3 {
            for i2 in 0..8 {
                axes[i1 * 3 + i2] = *Vector3::default().assign(&axes1[i1]).cross(&edges[i2]);
            }
        }

        !Sat::is_separated(&axes, &points1, &points2)
    }
}

// ---------------------------------------------------------------------------------------------
// Bezier
// ---------------------------------------------------------------------------------------------

/// Algorithms/Bezier.js
pub struct Bezier;

impl Bezier {
    /// Bezier.quadric: `steps` points (t = i / (steps - 1)).
    #[allow(clippy::too_many_arguments)]
    pub fn quadric(x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64, steps: i64, points: &mut Vec<Vector3>) {
        let c = Matrix3([1.0, 0.0, 0.0, -2.0, 2.0, 0.0, 1.0, -2.0, 1.0]);
        let mut p = Matrix3([x0, y0, z0, x1, y1, z1, x2, y2, z2]);

        p.mult_left(&c);

        let d = (steps - 1) as f64;
        let mut i = 0;

        while i < steps {
            let t = i as f64 / d;
            let mut v = Vector3::new(1.0, t, t * t);

            p.mult_vec_matrix3(&mut v);
            points.push(v);
            i += 1;
        }
    }

    /// Bezier.cubic.
    #[allow(clippy::too_many_arguments)]
    pub fn cubic(
        x0: f64,
        y0: f64,
        z0: f64,
        x1: f64,
        y1: f64,
        z1: f64,
        x2: f64,
        y2: f64,
        z2: f64,
        x3: f64,
        y3: f64,
        z3: f64,
        steps: i64,
        points: &mut Vec<Vector3>,
    ) {
        let c = Matrix4([1.0, 0.0, 0.0, 0.0, -3.0, 3.0, 0.0, 0.0, 3.0, -6.0, 3.0, 0.0, -1.0, 3.0, -3.0, 1.0]);
        let mut p = Matrix4([x0, y0, z0, 0.0, x1, y1, z1, 0.0, x2, y2, z2, 0.0, x3, y3, z3, 0.0]);

        p.mult_left(&c);

        let d = (steps - 1) as f64;
        let mut i = 0;

        while i < steps {
            let t = i as f64 / d;
            let mut v = Vector4::new(1.0, t, t * t, t * t * t);

            p.mult_vec_matrix4(&mut v);
            points.push(Vector3::new(v.x, v.y, v.z));
            i += 1;
        }
    }

    /// Bezier.arc (the SVG elliptical arc).
    #[allow(clippy::too_many_arguments)]
    pub fn arc(ax: f64, ay: f64, rx: f64, ry: f64, x_axis_rotation: f64, large_arc_flag: bool, sweep_flag: bool, x: f64, y: f64, steps: f64, points: &mut Vec<Vector3>) {
        use std::f64::consts::PI;

        if ax == x && ay == y {
            points.push(Vector3::new(x, y, 0.0));
            return;
        }

        let mut rx = rx.abs();
        let mut ry = ry.abs();

        if rx == 0.0 || ry == 0.0 {
            points.push(Vector3::new(ax, ay, 0.0));
            points.push(Vector3::new(x, y, 0.0));
            return;
        }

        let rx2 = rx * rx;
        let ry2 = ry * ry;

        let x_axis_rotation = jsmath::interval(x_axis_rotation, 0.0, 2.0 * PI);

        let sin_rotation = jsmath::sin(x_axis_rotation);
        let cos_rotation = jsmath::cos(x_axis_rotation);

        let mut d = Vector2::new(ax - x, ay - y);
        d.divide(2.0);

        let transformed_point = Vector2::new(cos_rotation * d.x + sin_rotation * d.y, -sin_rotation * d.x + cos_rotation * d.y);
        let mut transformed_point2 = transformed_point;
        transformed_point2.mult_vec(&transformed_point);

        let radii_check = transformed_point2.x / rx2 + transformed_point2.y / ry2;

        if radii_check > 1.0 {
            rx = jsmath::sqrt(radii_check) * rx;
            ry = jsmath::sqrt(radii_check) * ry;
        }

        let c_square_numerator = rx2 * ry2 - rx2 * transformed_point2.y - ry2 * transformed_point2.x;
        let c_square_root_denom = rx2 * transformed_point2.y + ry2 * transformed_point2.x;
        let mut c_radicand = c_square_numerator / c_square_root_denom;

        c_radicand = jsmath::js_max(0.0, c_radicand);

        let c_coef = (if large_arc_flag != sweep_flag { 1.0 } else { -1.0 }) * jsmath::sqrt(c_radicand);

        let transformed_center = Vector2::new(c_coef * rx * transformed_point.y / ry, -c_coef * ry * transformed_point.x / rx);

        let center = Vector2::new(
            cos_rotation * transformed_center.x - sin_rotation * transformed_center.y + ((ax + x) / 2.0),
            sin_rotation * transformed_center.x + cos_rotation * transformed_center.y + ((ay + y) / 2.0),
        );

        let start_vector = Vector2::new((transformed_point.x - transformed_center.x) / rx, (transformed_point.y - transformed_center.y) / ry);
        let end_vector = Vector2::new((-transformed_point.x - transformed_center.x) / rx, (-transformed_point.y - transformed_center.y) / ry);

        let get_angle = |x: f64| if x > 0.0 { x } else { 2.0 * PI + x };
        let start_angle = get_angle(jsmath::atan2(start_vector.y, start_vector.x));
        let end_angle = get_angle(jsmath::atan2(end_vector.y, end_vector.x));

        let mut sweep_angle = end_angle - start_angle;

        if large_arc_flag {
            if sweep_angle < 0.0 {
                sweep_angle += 2.0 * PI;
            }
        } else if sweep_angle > 0.0 {
            sweep_angle -= 2.0 * PI;
        }

        if sweep_flag && sweep_angle < 0.0 {
            sweep_angle += 2.0 * PI;
        } else if !sweep_flag && sweep_angle > 0.0 {
            sweep_angle -= 2.0 * PI;
        }

        let bezier_steps = jsmath::js_max(4.0, sweep_angle.abs() * steps / (2.0 * PI));
        let bezier_steps_1 = bezier_steps - 1.0;

        points.push(Vector3::new(ax, ay, 0.0));

        let mut i = 1.0;

        while i < bezier_steps_1 {
            let t = i / bezier_steps_1;
            let angle = start_angle + (sweep_angle * t);
            let x = rx * jsmath::cos(angle);
            let y = ry * jsmath::sin(angle);

            points.push(Vector3::new(cos_rotation * x - sin_rotation * y + center.x, sin_rotation * x + cos_rotation * y + center.y, 0.0));
            i += 1.0;
        }

        points.push(Vector3::new(x, y, 0.0));
    }
}

// ---------------------------------------------------------------------------------------------
// eigen_decomposition
// ---------------------------------------------------------------------------------------------

/// Algorithms/eigen_decomposition.js (SGI Inventor's Jacobi method) of a matrix of `order` 3 or 4 given as its
/// `order * order` elements: the eigenvalues and the eigenvector matrix (`vectors [i] [j]`).
pub fn eigen_decomposition(m: &[f64], order: usize) -> (Vec<f64>, Vec<Vec<f64>>) {
    let size = m.len() as f64;
    let get1 = |r: usize, c: usize| m[r * order + c];
    let mut values = vec![0.0; order];
    let mut vectors = vec![vec![0.0; order]; order];
    let mut a = vec![vec![0.0; order]; order];
    let mut b = vec![0.0; order];
    let mut z = vec![0.0; order];

    for i in 0..order {
        b[i] = get1(i, i);
        values[i] = b[i];
        z[i] = 0.0;

        for j in 0..order {
            vectors[i][j] = if i == j { 1.0 } else { 0.0 };
            a[i][j] = get1(j, i);
        }
    }

    for i in 0..50 {
        let mut sm = 0.0;

        for p in 0..order - 1 {
            for q in p + 1..order {
                sm += a[p][q].abs();
            }
        }

        if sm == 0.0 {
            break;
        }

        let thresh = if i < 3 { 0.2 * sm / size } else { 0.0 };

        for p in 0..order - 1 {
            for q in p + 1..order {
                let g = 100.0 * a[p][q].abs();

                if i > 3 && (values[p].abs() + g == values[p].abs()) && (values[q].abs() + g == values[q].abs()) {
                    a[p][q] = 0.0;
                } else if a[p][q].abs() > thresh {
                    let mut h = values[q] - values[p];
                    let mut t;

                    if h.abs() + g == h.abs() {
                        t = a[p][q] / h;
                    } else {
                        let theta = 0.5 * h / a[p][q];
                        t = 1.0 / (theta.abs() + jsmath::sqrt(1.0 + theta * theta));

                        if theta < 0.0 {
                            t = -t;
                        }
                    }

                    let c = 1.0 / jsmath::sqrt(1.0 + t * t);
                    let s = t * c;
                    let tau = s / (1.0 + c);
                    h = t * a[p][q];
                    z[p] -= h;
                    z[q] += h;
                    values[p] -= h;
                    values[q] += h;
                    a[p][q] = 0.0;

                    for j in 0..p {
                        let g = a[j][p];
                        let h = a[j][q];
                        a[j][p] = g - s * (h + g * tau);
                        a[j][q] = h + s * (g - h * tau);
                    }

                    for j in p + 1..q {
                        let g = a[p][j];
                        let h = a[j][q];
                        a[p][j] = g - s * (h + g * tau);
                        a[j][q] = h + s * (g - h * tau);
                    }

                    for j in q + 1..order {
                        let g = a[p][j];
                        let h = a[q][j];
                        a[p][j] = g - s * (h + g * tau);
                        a[q][j] = h + s * (g - h * tau);
                    }

                    for j in 0..order {
                        let g = vectors[j][p];
                        let h = vectors[j][q];
                        vectors[j][p] = g - s * (h + g * tau);
                        vectors[j][q] = h + s * (g - h * tau);
                    }
                }
            }
        }

        for p in 0..order {
            b[p] += z[p];
            values[p] = b[p];
            z[p] = 0.0;
        }
    }

    (values, vectors)
}

// ---------------------------------------------------------------------------------------------
// MergeSort, QuickSort
// ---------------------------------------------------------------------------------------------

/// Algorithms/MergeSort.js: `new MergeSort (array, compare) .sort (first, last)`.
pub fn merge_sort<T: Clone>(array: &mut [T], first: usize, last: usize, compare: &dyn Fn(&T, &T) -> bool) {
    fn merge_sort_r<T: Clone>(array: &mut [T], aux: &mut Vec<T>, lo: isize, hi: isize, compare: &dyn Fn(&T, &T) -> bool) {
        if lo < hi {
            let m = ((lo + hi) as usize >> 1) as isize;
            merge_sort_r(array, aux, lo, m, compare);
            merge_sort_r(array, aux, m + 1, hi, compare);
            merge(array, aux, lo as usize, m as usize, hi as usize, compare);
        }
    }

    fn merge<T: Clone>(array: &mut [T], aux: &mut Vec<T>, lo: usize, m: usize, hi: usize, compare: &dyn Fn(&T, &T) -> bool) {
        let mut i = 0;
        let mut j = lo;

        while j <= m {
            if i < aux.len() {
                aux[i] = array[j].clone();
            } else {
                aux.push(array[j].clone());
            }
            i += 1;
            j += 1;
        }

        i = 0;
        let mut k = lo;

        while k < j && j <= hi {
            if compare(&array[j], &aux[i]) {
                array[k] = array[j].clone();
                j += 1;
            } else {
                array[k] = aux[i].clone();
                i += 1;
            }
            k += 1;
        }

        while k < j {
            array[k] = aux[i].clone();
            k += 1;
            i += 1;
        }
    }

    let mut aux = vec![];
    merge_sort_r(array, &mut aux, first as isize, last as isize - 1, compare);
}

/// Algorithms/QuickSort.js: `new QuickSort (array, compare) .sort (first, last)`.
pub fn quick_sort<T: Clone>(array: &mut [T], first: usize, last: usize, compare: &dyn Fn(&T, &T) -> bool) {
    fn quicksort<T: Clone>(array: &mut [T], lo: isize, hi: isize, compare: &dyn Fn(&T, &T) -> bool) {
        let mut i = lo;
        let mut j = hi;
        let x = array[((lo + hi) as usize) >> 1].clone();

        loop {
            while compare(&array[i as usize], &x) {
                i += 1;
            }
            while compare(&x, &array[j as usize]) {
                j -= 1;
            }

            if i < j {
                array.swap(i as usize, j as usize);
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
            quicksort(array, lo, j, compare);
        }
        if i < hi {
            quicksort(array, i, hi, compare);
        }
    }

    if last as isize - first as isize > 1 {
        quicksort(array, first as isize, last as isize - 1, compare);
    }
}
