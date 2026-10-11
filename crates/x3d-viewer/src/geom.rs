//! Geometry primitives ported from X_ITE `standard/Math/Geometry/*.js` (Line2, Line3, Plane3,
//! Sphere3, Cylinder3, Triangle3, ViewVolume, Camera.ortho) and `standard/Math/Algorithm.js`
//! (`interval`).
//!
//! This is a line-for-line port: same algorithms, same operation order, same edge-case behavior,
//! f64 everywhere.
//!
//! Conventions used throughout:
//! * JS methods that mutate `this` and return `this` take `&mut self` and return `&mut Self`.
//! * JS methods that write an out-parameter and return a boolean take `&mut T` out parameters
//!   and return `bool`. The out parameter is left untouched when `false` is returned, as in JS.
//! * JS methods that only write an out-parameter take `&mut T` and return `()`.
//! * JS methods with a default `result = new X()` construct a new value and return it owned.
//! * JS constructors call `set`, which does NOT normalize the direction. `new` here does not
//!   normalize either.
//! * `copy` is provided as an explicit method. `toString` is not ported.

use x3d_fields::math::{Matrix3, Matrix4, Rotation4, Vector2, Vector3, Vector4};

/// Algorithm.js:interval
pub fn interval(value: f64, low: f64, high: f64) -> f64 {
    if value >= high {
        return ((value - low) % (high - low)) + low;
    }

    if value < low {
        return ((value - high) % (high - low)) + high;
    }

    value
}

// Matrix3.js:multMatrixVec, 2D branch. x3d-fields has only the 3D variant of this on Matrix3.
fn mat3_mult_matrix_vec2(m: &Matrix3, v: &mut Vector2) {
    let m = &m.0;
    let (x, y) = (v.x, v.y);
    let w = x * m[6] + y * m[7] + m[8];

    v.x = (x * m[0] + y * m[1] + m[2]) / w;
    v.y = (x * m[3] + y * m[4] + m[5]) / w;
}

// Matrix3.js:multDirMatrix, 2D branch.
fn mat3_mult_dir2(m: &Matrix3, v: &mut Vector2) {
    let m = &m.0;
    let (x, y) = (v.x, v.y);

    v.x = x * m[0] + y * m[3];
    v.y = x * m[1] + y * m[4];
}

// Matrix3.js:multMatrixDir, 2D branch.
fn mat3_mult_matrix_dir2(m: &Matrix3, v: &mut Vector2) {
    let m = &m.0;
    let (x, y) = (v.x, v.y);

    v.x = x * m[0] + y * m[1];
    v.y = x * m[3] + y * m[4];
}

// ---------------------------------------------------------------------------------------------
// Line2
// ---------------------------------------------------------------------------------------------

/// Line2.js
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line2 {
    pub point: Vector2,
    pub direction: Vector2,
}

impl Line2 {
    /// Line2.js: constructor, which calls `set`. The direction is not normalized.
    pub fn new(point: Vector2, direction: Vector2) -> Self {
        Self { point, direction }
    }

    /// Line2.js:copy
    pub fn copy(&self) -> Self {
        *self
    }

    /// Line2.js:assign
    pub fn assign(&mut self, line: &Line2) -> &mut Self {
        self.point.assign(&line.point);
        self.direction.assign(&line.direction);
        self
    }

    /// Line2.js:equals
    pub fn equals(&self, line: &Line2) -> bool {
        self.point.equals(&line.point) && self.direction.equals(&line.direction)
    }

    /// Line2.js:set
    pub fn set(&mut self, point: &Vector2, direction: &Vector2) -> &mut Self {
        self.point.assign(point);
        self.direction.assign(direction);
        self
    }

    /// Line2.js:setPoints. The direction is the normalized difference `point2 - point1`.
    pub fn set_points(&mut self, point1: &Vector2, point2: &Vector2) -> &mut Self {
        self.point.assign(point1);
        self.direction.assign(point2).subtract(point1).normalize();
        self
    }

    /// Line2.js:multMatrixLine. Takes the 2D (Matrix3) matrix.
    pub fn mult_matrix_line(&mut self, matrix: &Matrix3) -> &mut Self {
        mat3_mult_matrix_vec2(matrix, &mut self.point);
        mat3_mult_matrix_dir2(matrix, &mut self.direction);
        self.direction.normalize();
        self
    }

    /// Line2.js:multLineMatrix. Takes the 2D (Matrix3) matrix.
    pub fn mult_line_matrix(&mut self, matrix: &Matrix3) -> &mut Self {
        matrix.mult_vec_matrix2(&mut self.point);
        mat3_mult_dir2(matrix, &mut self.direction);
        self.direction.normalize();
        self
    }

    /// Line2.js:getClosestPointToPoint. Returns a new point.
    pub fn get_closest_point_to_point(&self, point: &Vector2) -> Vector2 {
        let mut r = *point;
        r.subtract(&self.point);
        let d = r.dot(&self.direction);
        let mut result = self.direction;
        result.multiply(d).add(&self.point);
        result
    }

    /// Line2.js:getPerpendicularVectorToPoint. Returns a new vector.
    pub fn get_perpendicular_vector_to_point(&self, point: &Vector2) -> Vector2 {
        let mut result = self.point;
        result.subtract(point);
        let mut t = self.direction;
        t.multiply(result.dot(&self.direction));
        result.subtract(&t);
        result
    }

    /// Line2.js:intersectsLine. Returns `false` for parallel lines (`|theta| >= 1`), leaving
    /// `point` untouched.
    pub fn intersects_line(&self, line: &Line2, point: &mut Vector2) -> bool {
        let (p1, d1) = (&self.point, &self.direction);
        let (p2, d2) = (&line.point, &line.direction);

        let theta = d1.dot(d2); // angle between both lines

        if theta.abs() >= 1.0 {
            return false; // lines are parallel
        }

        let mut u = *p2;
        u.subtract(p1);

        let t = (u.dot(d1) - theta * u.dot(d2)) / (1.0 - theta * theta);

        let mut result = *d1;
        result.multiply(t).add(p1);
        *point = result;

        true
    }

    /// Line2.js:fromPoints (static). The direction is the normalized difference `point2 - point1`.
    pub fn from_points(point1: &Vector2, point2: &Vector2) -> Self {
        let mut direction = *point2;
        direction.subtract(point1).normalize();

        Self {
            point: *point1,
            direction,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Line3
// ---------------------------------------------------------------------------------------------

/// Out parameter of `Line3::intersects_triangle`, the JS `uvt` object.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Uvt {
    pub u: f64,
    pub v: f64,
    pub t: f64,
}

/// Line3.js
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line3 {
    pub point: Vector3,
    pub direction: Vector3,
}

impl Line3 {
    /// Line3.js: constructor, which calls `set`. The direction is NOT normalized.
    pub fn new(point: Vector3, direction: Vector3) -> Self {
        Self { point, direction }
    }

    /// Line3.js:copy
    pub fn copy(&self) -> Self {
        *self
    }

    /// Line3.js:assign
    pub fn assign(&mut self, line: &Line3) -> &mut Self {
        self.point.assign(&line.point);
        self.direction.assign(&line.direction);
        self
    }

    /// Line3.js:equals
    pub fn equals(&self, line: &Line3) -> bool {
        self.point.equals(&line.point) && self.direction.equals(&line.direction)
    }

    /// Line3.js:set
    pub fn set(&mut self, point: &Vector3, direction: &Vector3) -> &mut Self {
        self.point.assign(point);
        self.direction.assign(direction);
        self
    }

    /// Line3.js:setPoints. The direction is the normalized difference `point2 - point1`.
    pub fn set_points(&mut self, point1: &Vector3, point2: &Vector3) -> &mut Self {
        self.point.assign(point1);
        self.direction.assign(point2).subtract(point1).normalize();
        self
    }

    /// Line3.js:multMatrixLine
    pub fn mult_matrix_line(&mut self, matrix: &Matrix4) -> &mut Self {
        matrix.mult_matrix_vec3(&mut self.point);
        matrix.mult_matrix_dir(&mut self.direction);
        self.direction.normalize();
        self
    }

    /// Line3.js:multLineMatrix
    pub fn mult_line_matrix(&mut self, matrix: &Matrix4) -> &mut Self {
        matrix.mult_vec_matrix3(&mut self.point);
        matrix.mult_dir_matrix(&mut self.direction);
        self.direction.normalize();
        self
    }

    /// Line3.js:getClosestPointToPoint. Returns a new point.
    pub fn get_closest_point_to_point(&self, point: &Vector3) -> Vector3 {
        let mut r = *point;
        r.subtract(&self.point);
        let d = r.dot(&self.direction);
        let mut result = self.direction;
        result.multiply(d).add(&self.point);
        result
    }

    /// Line3.js:getClosestPointToLine. Returns `false` for parallel lines (`|t| >= 1`), leaving
    /// `point` untouched.
    pub fn get_closest_point_to_line(&self, line: &Line3, point: &mut Vector3) -> bool {
        let (p1, d1) = (&self.point, &self.direction);
        let (p2, d2) = (&line.point, &line.direction);

        let mut t = d1.dot(d2);

        if t.abs() >= 1.0 {
            return false; // lines are parallel
        }

        let mut u = *p2;
        u.subtract(p1);

        t = (u.dot(d1) - t * u.dot(d2)) / (1.0 - t * t);

        let mut result = *d1;
        result.multiply(t).add(p1);
        *point = result;

        true
    }

    /// Line3.js:getPerpendicularVectorToPoint. Returns a new vector.
    pub fn get_perpendicular_vector_to_point(&self, point: &Vector3) -> Vector3 {
        let mut result = self.point;
        result.subtract(point);
        let mut t = self.direction;
        t.multiply(result.dot(&self.direction));
        result.subtract(&t);
        result
    }

    /// Line3.js:getPerpendicularVectorToLine. Returns a new vector.
    ///
    /// For parallel lines (`|t| >= 1`) the JS returns the projection scratch vector minus the
    /// offset, which is what this returns. Note that `d1.dot(d1)` is divided by without a guard.
    pub fn get_perpendicular_vector_to_line(&self, line: &Line3) -> Vector3 {
        let (p1, d1) = (&self.point, &self.direction);
        let (p2, d2) = (&line.point, &line.direction);

        let t = d1.dot(d2);

        if t.abs() >= 1.0 {
            // Lines are parallel
            let mut diff = *p2;
            diff.subtract(p1);

            let mut proj = *d1;
            proj.multiply(diff.dot(d1) / d1.dot(d1));

            proj.subtract(&diff);
            return proj;
        }

        let mut d = *p1;
        d.subtract(p2);

        let re1 = d.dot(d1);
        let re2 = d.dot(d2);
        let e12 = d1.dot(d2);
        let big_e12 = e12 * e12;

        let a = (re1 - re2 * e12) / (1.0 - big_e12);
        let b = -(re2 - re1 * e12) / (1.0 - big_e12);

        let mut point1 = *d1;
        point1.multiply(a);

        let mut result = *d2;
        result.multiply(b);

        result.subtract(&point1).add(&d);
        result
    }

    /// Line3.js:intersectsTriangle (Möller-Trumbore).
    ///
    /// Quirks preserved from the JS:
    /// * `B` and `C` are mutated in place. JS computes `edge1 = B.subtract(A)` and
    ///   `edge2 = C.subtract(A)` on the caller's objects, so after the call `b == B - A` and
    ///   `c == C - A`.
    /// * `uvt.u = 1 - u - v`, `uvt.v = u`, `uvt.t = v`. `t` is the weight of `C`, not the ray
    ///   parameter. The ray parameter (`edge2 . qvec * inv_det`) is commented out in the JS.
    ///
    /// Returns `false` (and leaves `uvt` untouched) on a near-zero determinant or outside the
    /// triangle.
    pub fn intersects_triangle(
        &self,
        a: &Vector3,
        b: &mut Vector3,
        c: &mut Vector3,
        uvt: &mut Uvt,
    ) -> bool {
        // Find vectors for two edges sharing vert0.
        b.subtract(a); // edge1 is the mutated B
        c.subtract(a); // edge2 is the mutated C
        let edge1 = *b;
        let edge2 = *c;

        // Begin calculating determinant - also used to calculate U parameter.
        let mut pvec = self.direction;
        pvec.cross(&edge2);

        // If determinant is near zero, ray lies in plane of triangle.
        let det = edge1.dot(&pvec);

        // Non culling intersection.
        if det == 0.0 {
            return false;
        }

        let inv_det = 1.0 / det;

        // Calculate distance from vert0 to ray point.
        let mut tvec = self.point;
        tvec.subtract(a);

        // Calculate U parameter and test bounds.
        let u = tvec.dot(&pvec) * inv_det;

        if u < 0.0 || u > 1.0 {
            return false;
        }

        // Prepare to test V parameter.
        let mut qvec = tvec;
        qvec.cross(&edge1);

        // Calculate V parameter and test bounds.
        let v = self.direction.dot(&qvec) * inv_det;

        if v < 0.0 || u + v > 1.0 {
            return false;
        }

        uvt.u = 1.0 - u - v;
        uvt.v = u;
        uvt.t = v;

        true
    }

    /// Line3.js:fromPoints (static). The direction is the normalized difference `point2 - point1`.
    pub fn from_points(point1: &Vector3, point2: &Vector3) -> Self {
        let mut direction = *point2;
        direction.subtract(point1).normalize();

        Self {
            point: *point1,
            direction,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Plane3
// ---------------------------------------------------------------------------------------------

/// Plane3.js
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane3 {
    pub normal: Vector3,
    pub distance_from_origin: f64,
}

impl Plane3 {
    /// Plane3.js: constructor, which calls `set`. The normal is NOT normalized.
    pub fn new(point: Vector3, normal: Vector3) -> Self {
        let mut plane = Self {
            normal: Vector3::default(),
            distance_from_origin: 0.0,
        };
        plane.set(&point, &normal);
        plane
    }

    /// Plane3.js:copy
    pub fn copy(&self) -> Self {
        *self
    }

    /// Plane3.js:assign
    pub fn assign(&mut self, plane: &Plane3) -> &mut Self {
        self.normal.assign(&plane.normal);
        self.distance_from_origin = plane.distance_from_origin;
        self
    }

    /// Plane3.js:equals
    pub fn equals(&self, plane: &Plane3) -> bool {
        self.distance_from_origin == plane.distance_from_origin && self.normal.equals(&plane.normal)
    }

    /// Plane3.js:set
    pub fn set(&mut self, point: &Vector3, normal: &Vector3) -> &mut Self {
        self.normal.assign(normal);
        self.distance_from_origin = normal.dot(point);
        self
    }

    /// Plane3.js:multLeft. Transforms the plane by `matrix` (Inventor's method, as in the JS).
    pub fn mult_left(&mut self, matrix: &Matrix4) -> &mut Self {
        // Find the point on the plane along the normal from the origin
        let mut point = self.normal;
        point.multiply(self.distance_from_origin);

        // Transform the plane normal by the matrix to get the new normal. Use the inverse
        // transpose of the matrix so that normals are not scaled incorrectly.
        // n' = !~m * n = n * ~m
        let mut inv_matrix = matrix.submatrix();
        inv_matrix.inverse();
        inv_matrix.mult_vec_matrix3(&mut self.normal);
        self.normal.normalize();

        // Transform the point by the matrix
        matrix.mult_matrix_vec3(&mut point);

        // The new distance is the projected distance of the vector to the transformed point
        // onto the (unit) transformed normal. This is just a dot product.
        self.distance_from_origin = self.normal.dot(&point);
        self
    }

    /// Plane3.js:multRight. Transforms the plane by `matrix` (Inventor's method, as in the JS).
    pub fn mult_right(&mut self, matrix: &Matrix4) -> &mut Self {
        // Find the point on the plane along the normal from the origin
        let mut point = self.normal;
        point.multiply(self.distance_from_origin);

        // Transform the plane normal by the matrix to get the new normal. Use the inverse
        // transpose of the matrix so that normals are not scaled incorrectly.
        // n' = n * !~m = ~m * n
        let mut inv_matrix = matrix.submatrix();
        inv_matrix.inverse();
        inv_matrix.mult_matrix_vec3(&mut self.normal);
        self.normal.normalize();

        // Transform the point by the matrix
        matrix.mult_vec_matrix3(&mut point);

        // The new distance is the projected distance of the vector to the transformed point
        // onto the (unit) transformed normal. This is just a dot product.
        self.distance_from_origin = self.normal.dot(&point);
        self
    }

    /// Plane3.js:getDistanceToPoint
    pub fn get_distance_to_point(&self, point: &Vector3) -> f64 {
        point.dot(&self.normal) - self.distance_from_origin
    }

    /// Plane3.js:getPerpendicularVectorToPoint. Returns a new vector.
    pub fn get_perpendicular_vector_to_point(&self, point: &Vector3) -> Vector3 {
        let mut result = self.normal;
        result.multiply(-self.get_distance_to_point(point));
        result
    }

    /// Plane3.js:getClosestPointToPoint. Returns a new point.
    pub fn get_closest_point_to_point(&self, point: &Vector3) -> Vector3 {
        let mut result = self.get_perpendicular_vector_to_point(point);
        result.add(point);
        result
    }

    /// Plane3.js:intersectsLine. Returns `false` if the line is parallel to the plane
    /// (`theta == 0`), leaving `intersection` untouched.
    pub fn intersects_line(&self, line: &Line3, intersection: &mut Vector3) -> bool {
        let (point, direction) = (&line.point, &line.direction);

        // Check if the line is parallel to the plane.
        let theta = direction.dot(&self.normal);

        // Plane and line are parallel.
        if theta == 0.0 {
            return false;
        }

        // Plane and line are not parallel. The intersection point can be calculated now.
        let t = (self.distance_from_origin - self.normal.dot(point)) / theta;

        let mut result = *direction;
        result.multiply(t).add(point);
        *intersection = result;

        true
    }
}

// ---------------------------------------------------------------------------------------------
// Sphere3
// ---------------------------------------------------------------------------------------------

/// Sphere3.js
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere3 {
    pub radius: f64,
    pub center: Vector3,
}

impl Sphere3 {
    /// Sphere3.js: constructor, which calls `set`.
    pub fn new(radius: f64, center: Vector3) -> Self {
        Self { radius, center }
    }

    /// Sphere3.js:copy
    pub fn copy(&self) -> Self {
        *self
    }

    /// Sphere3.js:assign
    pub fn assign(&mut self, sphere: &Sphere3) -> &mut Self {
        self.radius = sphere.radius;
        self.center.assign(&sphere.center);
        self
    }

    /// Sphere3.js:equals
    pub fn equals(&self, sphere: &Sphere3) -> bool {
        self.radius == sphere.radius && self.center.equals(&sphere.center)
    }

    /// Sphere3.js:set
    pub fn set(&mut self, radius: f64, center: &Vector3) -> &mut Self {
        self.radius = radius;
        self.center.assign(center);
        self
    }

    /// Sphere3.js:intersectsLine. Returns `false` (leaving the out parameters untouched) when the
    /// discriminant is negative. The enter and exit points are ordered so that `t1 <= t2`.
    pub fn intersects_line(
        &self,
        line: &Line3,
        enter_point: &mut Vector3,
        exit_point: &mut Vector3,
    ) -> bool {
        // https://github.com/Alexpux/Coin3D/blob/master/src/base/SbSphere.cpp
        let (point, direction) = (&line.point, &line.direction);
        let (center, radius) = (&self.center, self.radius);

        let b = 2.0 * (point.dot(direction) - center.dot(direction));
        let c = (point.x * point.x + point.y * point.y + point.z * point.z)
            + (center.x * center.x + center.y * center.y + center.z * center.z)
            - 2.0 * point.dot(center)
            - radius * radius;

        let core = b * b - 4.0 * c;

        if core >= 0.0 {
            let mut t1 = (-b + core.sqrt()) / 2.0;
            let mut t2 = (-b - core.sqrt()) / 2.0;

            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }

            let mut enter = *direction;
            enter.multiply(t1).add(point);
            *enter_point = enter;

            let mut exit = *direction;
            exit.multiply(t2).add(point);
            *exit_point = exit;

            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Cylinder3
// ---------------------------------------------------------------------------------------------

/// Cylinder3.js
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cylinder3 {
    pub axis: Line3,
    pub radius: f64,
}

impl Cylinder3 {
    /// Cylinder3.js: constructor, which calls `set`.
    pub fn new(axis: Line3, radius: f64) -> Self {
        Self { axis, radius }
    }

    /// Cylinder3.js:copy
    pub fn copy(&self) -> Self {
        *self
    }

    /// Cylinder3.js:assign
    pub fn assign(&mut self, cylinder: &Cylinder3) -> &mut Self {
        self.radius = cylinder.radius;
        self.axis.assign(&cylinder.axis);
        self
    }

    /// Cylinder3.js:equals
    pub fn equals(&self, cylinder: &Cylinder3) -> bool {
        self.radius == cylinder.radius && self.axis.equals(&cylinder.axis)
    }

    /// Cylinder3.js:set
    pub fn set(&mut self, axis: &Line3, radius: f64) {
        self.axis.assign(axis);
        self.radius = radius;
    }

    /// Cylinder3.js:intersectsLine. Returns `true` and writes `enter` and `exit` on intersection.
    ///
    /// Taken from Inventor SbCylinder.c++. The intersection runs on a radius 1 cylinder aligned
    /// with the y axis: the line is moved into that space, intersected, and the results are moved
    /// back.
    pub fn intersects_line(&self, line: &Line3, enter: &mut Vector3, exit: &mut Vector3) -> bool {
        // rotation to y axis
        let mut rot_to_y_axis = Rotation4::default();
        rot_to_y_axis.set_vectors(&self.axis.direction, &Vector3::new(0.0, 1.0, 0.0));
        let mtx_to_y_axis = rot_to_y_axis.quaternion().get_matrix4(); // Matrix4.fromRotation

        // scale to unit space
        let scale_factor = 1.0 / self.radius;
        let mut to_unit_cyl_space = Matrix4::default();
        to_unit_cyl_space.scale(&Vector3::new(scale_factor, scale_factor, scale_factor));
        to_unit_cyl_space.mult_left(&mtx_to_y_axis);

        // find the given line un-translated
        let mut point = line.point;
        point.subtract(&self.axis.point);
        let mut no_translation_line = Line3::new(point, line.direction);

        // find the un-translated line in unit cylinder's space
        no_translation_line.mult_line_matrix(&to_unit_cyl_space);

        // find the intersection on the unit cylinder
        if !Self::unit_cylinder_intersects_line(&no_translation_line, enter, exit) {
            return false;
        }

        // transform back to original space
        let mut from_unit_cyl_space = to_unit_cyl_space;
        from_unit_cyl_space.inverse();

        from_unit_cyl_space.mult_vec_matrix3(enter);
        enter.add(&self.axis.point);

        from_unit_cyl_space.mult_vec_matrix3(exit);
        exit.add(&self.axis.point);

        true
    }

    /// Cylinder3.js:unitCylinderIntersectsLine. Intersects a line with the radius 1 cylinder
    /// aligned with the y axis. Returns `false` (out parameters untouched) for a negative
    /// discriminant.
    pub fn unit_cylinder_intersects_line(line: &Line3, enter: &mut Vector3, exit: &mut Vector3) -> bool {
        let (pos, dir) = (&line.point, &line.direction);

        let a = dir.x * dir.x + dir.z * dir.z;
        let b = 2.0 * (pos.x * dir.x + pos.z * dir.z);
        let c = pos.x * pos.x + pos.z * pos.z - 1.0;

        // discriminant = B^2 - 4AC
        let discr = b * b - 4.0 * a * c;

        // if discriminant is negative, no intersection
        if discr < 0.0 {
            return false;
        }

        let sqroot = discr.sqrt();

        // magic to stabilize the answer
        let (t0, t1) = if b > 0.0 {
            (-(2.0 * c) / (sqroot + b), -(sqroot + b) / (2.0 * a))
        } else {
            ((2.0 * c) / (sqroot - b), (sqroot - b) / (2.0 * a))
        };

        let mut e = *dir;
        e.multiply(t0).add(pos);
        *enter = e;

        let mut x = *dir;
        x.multiply(t1).add(pos);
        *exit = x;

        true
    }
}

// ---------------------------------------------------------------------------------------------
// Triangle3
// ---------------------------------------------------------------------------------------------

/// Triangle3.js. The JS object of static functions maps to an associated-function namespace.
pub struct Triangle3;

impl Triangle3 {
    /// Triangle3.js:area
    pub fn area(a: &Vector3, b: &Vector3, c: &Vector3) -> f64 {
        let mut bb = *b;
        bb.subtract(a);

        let mut cc = *c;
        cc.subtract(a);

        bb.cross(&cc).norm() / 2.0
    }

    /// Triangle3.js:normal. Writes the normalized normal into `normal`.
    pub fn normal(a: &Vector3, b: &Vector3, c: &Vector3, normal: &mut Vector3) {
        let x1 = c.x - b.x;
        let y1 = c.y - b.y;
        let z1 = c.z - b.z;
        let x2 = a.x - b.x;
        let y2 = a.y - b.y;
        let z2 = a.z - b.z;

        normal.set(
            y1 * z2 - z1 * y2,
            z1 * x2 - x1 * z2,
            x1 * y2 - y1 * x2,
        );
        normal.normalize();
    }

    /// Triangle3.js:quadNormal. Writes the normalized normal into `normal`.
    pub fn quad_normal(a: &Vector3, b: &Vector3, c: &Vector3, d: &Vector3, normal: &mut Vector3) {
        let x1 = c.x - a.x;
        let y1 = c.y - a.y;
        let z1 = c.z - a.z;
        let x2 = d.x - b.x;
        let y2 = d.y - b.y;
        let z2 = d.z - b.z;

        normal.set(
            y1 * z2 - z1 * y2,
            z1 * x2 - x1 * z2,
            x1 * y2 - y1 * x2,
        );
        normal.normalize();
    }

    /// Triangle3.js:triangulateConvexPolygon. Appends a fan triangulation of `vertices` to
    /// `triangles`. Fewer than three vertices append nothing.
    ///
    /// The JS `triangulatePolygon` (libtess GLU tessellator) is not ported.
    pub fn triangulate_convex_polygon<T: Copy>(vertices: &[T], triangles: &mut Vec<T>) {
        let num_vertices = vertices.len();

        for i in 1..num_vertices.saturating_sub(1) {
            triangles.push(vertices[0]);
            triangles.push(vertices[i]);
            triangles.push(vertices[i + 1]);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// ViewVolume
// ---------------------------------------------------------------------------------------------

/// A point argument accepted by `ViewVolume::project_point`. JS branches on `point.length === 4`.
/// A Vector3 becomes `(x, y, z, 1)`, and a Vector4 is used as-is.
pub trait ProjectablePoint {
    /// The homogeneous `vin` used by the JS `projectPoint`.
    fn to_homogeneous(&self) -> Vector4;
}

impl ProjectablePoint for Vector3 {
    fn to_homogeneous(&self) -> Vector4 {
        Vector4::new(self.x, self.y, self.z, 1.0)
    }
}

impl ProjectablePoint for Vector4 {
    fn to_homogeneous(&self) -> Vector4 {
        *self
    }
}

/// ViewVolume.js. Only the static functions are ported. The viewport is a `Vector4` laid out as
/// the JS array: `viewport[0] = x`, `viewport[1] = y`, `viewport[2] = width` (`.z`),
/// `viewport[3] = height` (`.w`).
pub struct ViewVolume;

impl ViewVolume {
    /// ViewVolume.js:unProjectPoint
    pub fn unproject_point(
        win_x: f64,
        win_y: f64,
        win_z: f64,
        model_view_matrix: &Matrix4,
        projection_matrix: &Matrix4,
        viewport: &Vector4,
        point: &mut Vector3,
    ) {
        let mut inv_model_view_projection_matrix = *model_view_matrix;
        inv_model_view_projection_matrix
            .mult_right(projection_matrix)
            .inverse();

        Self::unproject_point_matrix(
            win_x,
            win_y,
            win_z,
            &inv_model_view_projection_matrix,
            viewport,
            point,
        );
    }

    /// ViewVolume.js:unProjectPointMatrix
    pub fn unproject_point_matrix(
        win_x: f64,
        win_y: f64,
        win_z: f64,
        inv_model_view_projection_matrix: &Matrix4,
        viewport: &Vector4,
        point: &mut Vector3,
    ) {
        // Transformation of normalized coordinates between -1 and 1
        let mut vin = Vector4::new(
            (win_x - viewport.x) / viewport.z * 2.0 - 1.0,
            (win_y - viewport.y) / viewport.w * 2.0 - 1.0,
            win_z * 2.0 - 1.0,
            1.0,
        );

        // Objects coordinates
        inv_model_view_projection_matrix.mult_vec_matrix4(&mut vin);

        let d = 1.0 / vin.w;

        point.set(vin.x * d, vin.y * d, vin.z * d);
    }

    /// ViewVolume.js:unProjectRay. Writes the ray from the near to the far plane into `result`.
    pub fn unproject_ray(
        win_x: f64,
        win_y: f64,
        model_view_matrix: &Matrix4,
        projection_matrix: &Matrix4,
        viewport: &Vector4,
        result: &mut Line3,
    ) {
        let mut inv_model_view_projection_matrix = *model_view_matrix;
        inv_model_view_projection_matrix
            .mult_right(projection_matrix)
            .inverse();

        Self::unproject_ray_matrix(
            win_x,
            win_y,
            &inv_model_view_projection_matrix,
            viewport,
            result,
        );
    }

    /// ViewVolume.js:unProjectRayMatrix. The far depth is 0.9, not 1.0, as in the JS.
    pub fn unproject_ray_matrix(
        win_x: f64,
        win_y: f64,
        inv_model_view_projection_matrix: &Matrix4,
        viewport: &Vector4,
        result: &mut Line3,
    ) {
        let mut near = Vector3::default();
        let mut far = Vector3::default();

        Self::unproject_point_matrix(
            win_x,
            win_y,
            0.0,
            inv_model_view_projection_matrix,
            viewport,
            &mut near,
        );
        Self::unproject_point_matrix(
            win_x,
            win_y,
            0.9,
            inv_model_view_projection_matrix,
            viewport,
            &mut far,
        );

        result.set_points(&near, &far);
    }

    /// ViewVolume.js:projectPoint. Writes the window coordinates into `v_out`.
    pub fn project_point<P: ProjectablePoint>(
        point: &P,
        model_view_matrix: &Matrix4,
        projection_matrix: &Matrix4,
        viewport: &Vector4,
        v_out: &mut Vector3,
    ) {
        let mut vin = point.to_homogeneous();

        // projectionMatrix.multVecMatrix (modelViewMatrix.multVecMatrix (vin))
        model_view_matrix.mult_vec_matrix4(&mut vin);
        projection_matrix.mult_vec_matrix4(&mut vin);

        let d = 1.0 / (2.0 * vin.w);

        v_out.set(
            (vin.x * d + 0.5) * viewport.z + viewport.x,
            (vin.y * d + 0.5) * viewport.w + viewport.y,
            vin.z * d + 0.5,
        );
    }

    /// ViewVolume.js:projectPointMatrix. Writes the window coordinates into `v_out`.
    pub fn project_point_matrix<P: ProjectablePoint>(
        point: &P,
        model_view_projection_matrix: &Matrix4,
        viewport: &Vector4,
        v_out: &mut Vector3,
    ) {
        let mut vin = point.to_homogeneous();

        model_view_projection_matrix.mult_vec_matrix4(&mut vin);

        let d = 1.0 / (2.0 * vin.w);

        v_out.set(
            (vin.x * d + 0.5) * viewport.z + viewport.x,
            (vin.y * d + 0.5) * viewport.w + viewport.y,
            vin.z * d + 0.5,
        );
    }

    /// ViewVolume.js:projectLine. Writes the projected 2D line into `result`.
    pub fn project_line(
        line: &Line3,
        model_view_matrix: &Matrix4,
        projection_matrix: &Matrix4,
        viewport: &Vector4,
        result: &mut Line2,
    ) {
        let mut model_view_projection_matrix = *model_view_matrix;
        model_view_projection_matrix.mult_right(projection_matrix);

        Self::project_line_matrix(line, &model_view_projection_matrix, viewport, result);
    }

    /// ViewVolume.js:projectLineMatrix. The near and far points are projected to window
    /// coordinates. The far point is `line.point + 1e9 * line.direction`. Only the x and y
    /// components are kept, as in the JS `Vector2`.
    pub fn project_line_matrix(
        line: &Line3,
        model_view_projection_matrix: &Matrix4,
        viewport: &Vector4,
        result: &mut Line2,
    ) {
        let mut near3 = Vector3::default();
        Self::project_point_matrix(&line.point, model_view_projection_matrix, viewport, &mut near3);

        let mut direction = line.direction;
        direction.multiply(1e9).add(&line.point);

        let mut far3 = Vector3::default();
        Self::project_point_matrix(&direction, model_view_projection_matrix, viewport, &mut far3);

        let near = Vector2::new(near3.x, near3.y);
        let far = Vector2::new(far3.x, far3.y);

        result.set_points(&near, &far);
    }
}

// ---------------------------------------------------------------------------------------------
// Camera
// ---------------------------------------------------------------------------------------------

/// Camera.js. Only `ortho` is ported. `perspective` already exists elsewhere in the crate.
pub struct Camera;

impl Camera {
    /// Camera.js:ortho. Writes the orthographic projection into `matrix`, column-major, as the
    /// JS `Matrix4.set` stores its sixteen arguments in order.
    pub fn ortho(
        l: f64,
        r: f64,
        b: f64,
        t: f64,
        n: f64,
        f: f64,
        matrix: &mut Matrix4,
    ) -> &mut Matrix4 {
        let r_l = r - l;
        let t_b = t - b;
        let f_n = f - n;

        let a = 2.0 / r_l;
        let bb = 2.0 / t_b;
        let c = -2.0 / f_n;
        let d = -(r + l) / r_l;
        let e = -(t + b) / t_b;
        let ff = -(f + n) / f_n;

        matrix.0 = [
            a, 0.0, 0.0, 0.0, //
            0.0, bb, 0.0, 0.0, //
            0.0, 0.0, c, 0.0, //
            d, e, ff, 1.0,
        ];

        matrix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-9;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < EPSILON,
            "expected {expected}, got {actual}"
        );
    }

    fn assert_vec3_close(actual: &Vector3, expected: &Vector3) {
        assert_close(actual.x, expected.x);
        assert_close(actual.y, expected.y);
        assert_close(actual.z, expected.z);
    }

    #[test]
    fn interval_wraps_values_like_js() {
        // value >= high: ((7 - 0) % 5) + 0
        assert_close(interval(7.0, 0.0, 5.0), 2.0);
        // value < low: ((-1 - 5) % 5) + 5
        assert_close(interval(-1.0, 0.0, 5.0), 4.0);
        // in range: unchanged
        assert_close(interval(3.0, 0.0, 5.0), 3.0);
    }

    #[test]
    fn line_plane_intersection_by_hand() {
        // Plane z = 0, normal (0, 0, 1).
        let plane = Plane3::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 1.0));

        // Line through (1, 2, 5) with direction (0, 0, -1) hits z = 0 at (1, 2, 0).
        // theta = -1, t = (0 - 5) / -1 = 5, point = (0, 0, -5) * 5 + ... = (1, 2, 0).
        let line = Line3::new(Vector3::new(1.0, 2.0, 5.0), Vector3::new(0.0, 0.0, -1.0));
        let mut hit = Vector3::default();

        assert!(plane.intersects_line(&line, &mut hit));
        assert_vec3_close(&hit, &Vector3::new(1.0, 2.0, 0.0));

        // A line parallel to the plane returns false and leaves the output untouched.
        let parallel = Line3::new(Vector3::new(0.0, 0.0, 3.0), Vector3::new(1.0, 0.0, 0.0));
        let mut untouched = Vector3::new(9.0, 9.0, 9.0);

        assert!(!plane.intersects_line(&parallel, &mut untouched));
        assert_vec3_close(&untouched, &Vector3::new(9.0, 9.0, 9.0));
    }

    #[test]
    fn plane_distance_and_closest_point() {
        // Plane x = 2, normal (1, 0, 0), distance 2.
        let plane = Plane3::new(Vector3::new(2.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0));

        assert_close(plane.distance_from_origin, 2.0);
        assert_close(plane.get_distance_to_point(&Vector3::new(5.0, 0.0, 0.0)), 3.0);
        assert_vec3_close(
            &plane.get_closest_point_to_point(&Vector3::new(5.0, 7.0, -1.0)),
            &Vector3::new(2.0, 7.0, -1.0),
        );
    }

    #[test]
    fn sphere_line_intersection_by_hand() {
        // Unit sphere at the origin, line from (-5, 0, 0) along +x.
        // b = 2 * (-5) = -10, c = 25 - 1 = 24, core = 100 - 96 = 4.
        // t1 = (10 - 2) / 2 = 4 and t2 = (10 + 2) / 2 = 6 after the swap.
        let sphere = Sphere3::new(1.0, Vector3::new(0.0, 0.0, 0.0));
        let line = Line3::new(Vector3::new(-5.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0));
        let mut enter = Vector3::default();
        let mut exit = Vector3::default();

        assert!(sphere.intersects_line(&line, &mut enter, &mut exit));
        assert_vec3_close(&enter, &Vector3::new(-1.0, 0.0, 0.0));
        assert_vec3_close(&exit, &Vector3::new(1.0, 0.0, 0.0));

        // A line that misses the sphere returns false.
        let miss = Line3::new(Vector3::new(-5.0, 3.0, 0.0), Vector3::new(1.0, 0.0, 0.0));
        assert!(!sphere.intersects_line(&miss, &mut enter, &mut exit));
    }

    #[test]
    fn triangle_intersection_keeps_js_quirks() {
        // Triangle A = (0, 0, 0), B = (1, 0, 0), C = (0, 1, 0) in the plane z = 0.
        let a = Vector3::new(0.0, 0.0, 0.0);
        let mut b = Vector3::new(1.0, 0.0, 0.0);
        let mut c = Vector3::new(0.0, 1.0, 0.0);
        let line = Line3::new(Vector3::new(0.25, 0.25, 5.0), Vector3::new(0.0, 0.0, -1.0));
        let mut uvt = Uvt::default();

        assert!(line.intersects_triangle(&a, &mut b, &mut c, &mut uvt));

        // B and C are mutated to edge vectors (B - A, C - A), as in the JS.
        assert_vec3_close(&b, &Vector3::new(1.0, 0.0, 0.0));
        assert_vec3_close(&c, &Vector3::new(0.0, 1.0, 0.0));

        // u = 0.25, v = 0.25, so uvt.u = 1 - u - v = 0.5, uvt.v = u, uvt.t = v.
        assert_close(uvt.u, 0.5);
        assert_close(uvt.v, 0.25);
        assert_close(uvt.t, 0.25);
    }

    #[test]
    fn closest_point_to_parallel_line_returns_false() {
        let l1 = Line3::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 1.0));
        let l2 = Line3::new(Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 1.0));
        let mut point = Vector3::new(7.0, 7.0, 7.0);

        assert!(!l1.get_closest_point_to_line(&l2, &mut point));
        assert_vec3_close(&point, &Vector3::new(7.0, 7.0, 7.0));
    }

    #[test]
    fn unproject_ray_then_project_point_round_trip() {
        let model_view = Matrix4::default();
        let mut projection = Matrix4::default();
        Camera::ortho(-1.0, 1.0, -1.0, 1.0, 1.0, 100.0, &mut projection);

        let viewport = Vector4::new(0.0, 0.0, 800.0, 600.0);

        // Window point (600, 225). The near point is (0.5, -0.25, -1).
        let mut ray = Line3::new(Vector3::default(), Vector3::new(0.0, 0.0, 1.0));
        ViewVolume::unproject_ray(600.0, 225.0, &model_view, &projection, &viewport, &mut ray);
        assert_vec3_close(&ray.point, &Vector3::new(0.5, -0.25, -1.0));

        // Projecting the near point back gives the window point at depth 0.
        let mut win = Vector3::default();
        ViewVolume::project_point(&ray.point, &model_view, &projection, &viewport, &mut win);
        assert_vec3_close(&win, &Vector3::new(600.0, 225.0, 0.0));
    }

    #[test]
    fn project_point_hand_values_and_unproject_inverse() {
        let model_view = Matrix4::default();
        let mut projection = Matrix4::default();
        Camera::ortho(-1.0, 1.0, -1.0, 1.0, 1.0, 100.0, &mut projection);

        let viewport = Vector4::new(0.0, 0.0, 800.0, 600.0);
        let p = Vector3::new(0.5, -0.25, -10.0);

        // x_ndc = 0.5, y_ndc = -0.25, z_ndc = -81/99.
        // win = (600, 225, 1/11).
        let mut win = Vector3::default();
        ViewVolume::project_point(&p, &model_view, &projection, &viewport, &mut win);
        assert_vec3_close(&win, &Vector3::new(600.0, 225.0, 1.0 / 11.0));

        let mut back = Vector3::default();
        ViewVolume::unproject_point(
            win.x,
            win.y,
            win.z,
            &model_view,
            &projection,
            &viewport,
            &mut back,
        );
        assert_vec3_close(&back, &p);
    }

    #[test]
    fn ortho_matrix_layout_is_column_major() {
        let mut m = Matrix4::default();
        Camera::ortho(-1.0, 1.0, -1.0, 1.0, 1.0, 100.0, &mut m);

        assert_close(m.0[0], 1.0);
        assert_close(m.0[5], 1.0);
        assert_close(m.0[10], -2.0 / 99.0);
        assert_close(m.0[14], -101.0 / 99.0);
        assert_close(m.0[15], 1.0);
    }
}
