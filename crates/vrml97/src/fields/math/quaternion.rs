//! Quaternion (standard/Math/Numbers/Quaternion.js).

use super::matrix::{Matrix3, Matrix4};
use super::vector::{Vector3, Vector4};
use crate::fields::jsmath::{self, hypot};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quaternion {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

impl Default for Quaternion {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Quaternion {
    pub const IDENTITY: Quaternion = Quaternion { x: 0.0, y: 0.0, z: 0.0, w: 1.0 };
    pub const ZERO: Quaternion = Quaternion { x: 0.0, y: 0.0, z: 0.0, w: 0.0 };

    pub fn new(x: f64, y: f64, z: f64, w: f64) -> Self {
        Self { x, y, z, w }
    }

    fn v4(&self) -> Vector4 {
        Vector4::new(self.x, self.y, self.z, self.w)
    }

    fn from_v4(v: Vector4) -> Self {
        Self::new(v.x, v.y, v.z, v.w)
    }

    pub fn set(&mut self, x: f64, y: f64, z: f64, w: f64) -> &mut Self {
        *self = Self::new(x, y, z, w);
        self
    }

    pub fn get(&self, i: usize) -> f64 {
        [self.x, self.y, self.z, self.w][i]
    }

    pub fn set_index(&mut self, i: usize, v: f64) {
        match i {
            0 => self.x = v,
            1 => self.y = v,
            2 => self.z = v,
            _ => self.w = v,
        }
    }

    pub fn equals(&self, other: &Self) -> bool {
        self.v4().equals(&other.v4())
    }

    pub fn add(&mut self, o: &Self) -> &mut Self {
        *self = Self::from_v4(*self.v4().add(&o.v4()));
        self
    }

    pub fn subtract(&mut self, o: &Self) -> &mut Self {
        *self = Self::from_v4(*self.v4().subtract(&o.v4()));
        self
    }

    pub fn multiply(&mut self, value: f64) -> &mut Self {
        *self = Self::from_v4(*self.v4().multiply(value));
        self
    }

    pub fn divide(&mut self, value: f64) -> &mut Self {
        *self = Self::from_v4(*self.v4().divide(value));
        self
    }

    pub fn negate(&mut self) -> &mut Self {
        *self = Self::from_v4(*self.v4().negate());
        self
    }

    pub fn dot(&self, o: &Self) -> f64 {
        self.v4().dot(&o.v4())
    }

    pub fn squared_norm(&self) -> f64 {
        self.v4().squared_norm()
    }

    pub fn norm(&self) -> f64 {
        self.v4().norm()
    }

    pub fn normalize(&mut self) -> &mut Self {
        *self = Self::from_v4(*self.v4().normalize());
        self
    }

    pub fn conjugate(&mut self) -> &mut Self {
        self.x = -self.x;
        self.y = -self.y;
        self.z = -self.z;
        self
    }

    pub fn inverse(&mut self) -> &mut Self {
        self.x = -self.x;
        self.y = -self.y;
        self.z = -self.z;

        let squared_norm = self.squared_norm();

        if squared_norm != 0.0 && !squared_norm.is_nan() {
            self.x /= squared_norm;
            self.y /= squared_norm;
            self.z /= squared_norm;
            self.w /= squared_norm;
        }

        self
    }

    pub fn mult_left(&mut self, q: &Self) -> &mut Self {
        let (ax, ay, az, aw) = (self.x, self.y, self.z, self.w);
        let (bx, by, bz, bw) = (q.x, q.y, q.z, q.w);

        self.x = aw * bx + ax * bw + ay * bz - az * by;
        self.y = aw * by + ay * bw + az * bx - ax * bz;
        self.z = aw * bz + az * bw + ax * by - ay * bx;
        self.w = aw * bw - ax * bx - ay * by - az * bz;
        self
    }

    pub fn mult_right(&mut self, q: &Self) -> &mut Self {
        let (ax, ay, az, aw) = (self.x, self.y, self.z, self.w);
        let (bx, by, bz, bw) = (q.x, q.y, q.z, q.w);

        self.x = bw * ax + bx * aw + by * az - bz * ay;
        self.y = bw * ay + by * aw + bz * ax - bx * az;
        self.z = bw * az + bz * aw + bx * ay - by * ax;
        self.w = bw * aw - bx * ax - by * ay - bz * az;
        self
    }

    pub fn mult_quat_vec(&self, v: &mut Vector3) {
        let (qx, qy, qz, qw) = (self.x, self.y, self.z, self.w);
        let (vx, vy, vz) = (v.x, v.y, v.z);
        let tx = 2.0 * (qz * vy - qy * vz);
        let ty = 2.0 * (qx * vz - qz * vx);
        let tz = 2.0 * (qy * vx - qx * vy);

        v.x += qw * tx - qy * tz + qz * ty;
        v.y += qw * ty - qz * tx + qx * tz;
        v.z += qw * tz - qx * ty + qy * tx;
    }

    pub fn mult_vec_quat(&self, v: &mut Vector3) {
        let (qx, qy, qz, qw) = (self.x, self.y, self.z, self.w);
        let (vx, vy, vz) = (v.x, v.y, v.z);
        let tx = 2.0 * (qy * vz - qz * vy);
        let ty = 2.0 * (qz * vx - qx * vz);
        let tz = 2.0 * (qx * vy - qy * vx);

        v.x += qw * tx + qy * tz - qz * ty;
        v.y += qw * ty + qz * tx - qx * tz;
        v.z += qw * tz + qx * ty - qy * tx;
    }

    fn matrix_elements(&self) -> [f64; 9] {
        let (x, y, z, w) = (self.x, self.y, self.z, self.w);
        let xx = x * x;
        let xy = x * y;
        let xw = x * w;
        let xz = x * z;
        let yy = y * y;
        let yw = y * w;
        let yz = y * z;
        let zz = z * z;
        let zw = z * w;

        [
            1.0 - 2.0 * (yy + zz),
            2.0 * (xy + zw),
            2.0 * (xz - yw),
            2.0 * (xy - zw),
            1.0 - 2.0 * (xx + zz),
            2.0 * (xw + yz),
            2.0 * (xz + yw),
            2.0 * (yz - xw),
            1.0 - 2.0 * (xx + yy),
        ]
    }

    pub fn get_matrix3(&self) -> Matrix3 {
        Matrix3(self.matrix_elements())
    }

    pub fn get_matrix4(&self) -> Matrix4 {
        let e = self.matrix_elements();

        Matrix4([
            e[0], e[1], e[2], 0.0, e[3], e[4], e[5], 0.0, e[6], e[7], e[8], 0.0, 0.0, 0.0, 0.0, 1.0,
        ])
    }

    pub fn set_matrix(&mut self, m: &Matrix3) -> &mut Self {
        let m = &m.0;
        let i = if m[0] > m[4] {
            if m[0] > m[8] { 0 } else { 2 }
        } else if m[4] > m[8] {
            1
        } else {
            2
        };

        let scale_row = m[0] + m[4] + m[8];

        if scale_row > m[i * 3 + i] {
            self.w = jsmath::sqrt(scale_row + 1.0) / 2.0;

            let d = 4.0 * self.w;

            self.x = (m[5] - m[7]) / d;
            self.y = (m[6] - m[2]) / d;
            self.z = (m[1] - m[3]) / d;
        } else {
            let j = (i + 1) % 3;
            let k = (i + 2) % 3;
            let vi = jsmath::sqrt(m[i * 3 + i] - m[j * 3 + j] - m[k * 3 + k] + 1.0) / 2.0;

            self.set_index(i, vi);

            let d = 4.0 * vi;

            self.set_index(j, (m[i * 3 + j] + m[j * 3 + i]) / d);
            self.set_index(k, (m[i * 3 + k] + m[k * 3 + i]) / d);
            self.w = (m[j * 3 + k] - m[k * 3 + j]) / d;
        }

        self
    }

    pub fn set_euler(&mut self, x: f64, y: f64, z: f64) -> &mut Self {
        // Order "XYZ" only.
        let (x, y, z) = (x / 2.0, y / 2.0, z / 2.0);
        let (sx, sy, sz) = (jsmath::sin(x), jsmath::sin(y), jsmath::sin(z));
        let (cx, cy, cz) = (jsmath::cos(x), jsmath::cos(y), jsmath::cos(z));

        self.x = sx * cy * cz + cx * sy * sz;
        self.y = cx * sy * cz - sx * cy * sz;
        self.z = cx * cy * sz + sx * sy * cz;
        self.w = cx * cy * cz - sx * sy * sz;
        self
    }

    /// Algorithm.js: slerp (short = true).
    pub fn slerp(&mut self, destination: &Self, t: f64) -> &mut Self {
        slerp(self, *destination, t, true);
        self
    }

    pub fn log(&mut self) -> &mut Self {
        let (x, y, z, w) = (self.x, self.y, self.z, self.w);
        let r = hypot(&[x, y, z]);
        let t = if r > 0.0 { jsmath::atan2(r, w) / r } else { 0.0 };

        self.x = x * t;
        self.y = y * t;
        self.z = z * t;
        self.w = 0.5 * jsmath::log(x * x + y * y + z * z + w * w);
        self
    }

    pub fn exp(&mut self) -> &mut Self {
        let (x, y, z, w) = (self.x, self.y, self.z, self.w);
        let r = hypot(&[x, y, z]);
        let et = jsmath::exp(w);
        let s = if r > 0.0 { (et * jsmath::sin(r)) / r } else { 0.0 };

        self.x = x * s;
        self.y = y * s;
        self.z = z * s;
        self.w = et * jsmath::cos(r);
        self
    }
}

/// standard/Math/Algorithm.js: slerp on 4-component values.
pub(crate) fn slerp(source: &mut Quaternion, mut destination: Quaternion, t: f64, short: bool) {
    let mut cosom = source.dot(&destination);

    if short && cosom < 0.0 {
        cosom = -cosom;
        destination.negate();
    }

    if cosom >= 1.0 {
        return;
    }

    let omega = jsmath::acos(cosom);
    let sinom = jsmath::sin(omega);
    let scale0 = jsmath::sin((1.0 - t) * omega) / sinom;
    let scale1 = jsmath::sin(t * omega) / sinom;

    source.x = source.x * scale0 + destination.x * scale1;
    source.y = source.y * scale0 + destination.y * scale1;
    source.z = source.z * scale0 + destination.z * scale1;
    source.w = source.w * scale0 + destination.w * scale1;
}
