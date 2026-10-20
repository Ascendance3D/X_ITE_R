//! Rotation4 (standard/Math/Numbers/Rotation4.js).
//!
//! Quirk kept on purpose: `set` stores the raw axis/angle *and* a quaternion. Getters return
//! the raw values until an operation "taints" the rotation; then they are recomputed from the
//! quaternion. `equals` compares quaternions, so `0 1 0 0` equals the identity `0 0 1 0`.

use super::matrix::Matrix3;
use super::quaternion::Quaternion;
use super::vector::{Vector3, Vector4};
use crate::jsmath::{self, hypot};

#[derive(Clone, Copy, Debug)]
pub struct Rotation4 {
    x: f64,
    y: f64,
    z: f64,
    angle: f64,
    tainted: bool,
    quaternion: Quaternion,
}

impl Default for Rotation4 {
    fn default() -> Self {
        Self::new(0.0, 0.0, 1.0, 0.0)
    }
}

impl PartialEq for Rotation4 {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

impl Rotation4 {
    pub const IDENTITY_TUPLE: [f64; 4] = [0.0, 0.0, 1.0, 0.0];

    pub fn new(x: f64, y: f64, z: f64, angle: f64) -> Self {
        let mut r = Self { x: 0.0, y: 0.0, z: 1.0, angle: 0.0, tainted: false, quaternion: Quaternion::IDENTITY };
        r.set(x, y, z, angle);
        r
    }

    pub fn identity() -> Self {
        Self::default()
    }

    pub fn set(&mut self, x: f64, y: f64, z: f64, angle: f64) -> &mut Self {
        self.x = x;
        self.y = y;
        self.z = z;
        self.angle = angle;
        self.tainted = false;

        let scale = hypot(&[x, y, z]);

        if scale == 0.0 {
            self.quaternion.set(0.0, 0.0, 0.0, 1.0);
            return self;
        }

        let half_theta = jsmath::interval(angle / 2.0, 0.0, std::f64::consts::PI);
        let a_scale = jsmath::sin(half_theta) / scale;

        self.quaternion.set(x * a_scale, y * a_scale, z * a_scale, jsmath::cos(half_theta));
        self
    }

    /// The `get ()` method: axis/angle derived from the quaternion.
    pub fn get_from_quaternion(&self) -> Vector4 {
        let q = &self.quaternion;

        if q.w.abs() >= 1.0 {
            return Vector4::Z_AXIS;
        }

        let angle = jsmath::acos(q.w) * 2.0;
        let scale = jsmath::sin(angle / 2.0);

        if scale == 0.0 {
            Vector4::Z_AXIS
        } else {
            Vector4::new(q.x / scale, q.y / scale, q.z / scale, angle)
        }
    }

    pub fn normalize(&mut self) -> &mut Self {
        let r = self.get_from_quaternion();

        self.x = r.x;
        self.y = r.y;
        self.z = r.z;
        self.angle = r.w;
        self.tainted = false;
        self
    }

    fn untaint(&mut self) {
        if self.tainted {
            self.normalize();
        }
    }

    pub fn x(&mut self) -> f64 {
        self.untaint();
        self.x
    }

    pub fn y(&mut self) -> f64 {
        self.untaint();
        self.y
    }

    pub fn z(&mut self) -> f64 {
        self.untaint();
        self.z
    }

    pub fn angle(&mut self) -> f64 {
        self.untaint();
        self.angle
    }

    /// Iteration order of the JS object: x, y, z, angle (normalizing first if tainted).
    pub fn components(&mut self) -> [f64; 4] {
        self.untaint();
        [self.x, self.y, self.z, self.angle]
    }

    /// Raw stored components without normalizing (for a non-tainted rotation these are what
    /// the getters return).
    pub fn raw(&self) -> [f64; 4] {
        if self.tainted {
            let mut c = *self;
            c.normalize();
            return [c.x, c.y, c.z, c.angle];
        }
        [self.x, self.y, self.z, self.angle]
    }

    /// Index setter `rotation [i] = value` (x, y, z, angle setters).
    pub fn set_index(&mut self, index: usize, value: f64) {
        self.untaint();

        let mut a = [self.x, self.y, self.z, self.angle];
        a[index] = value;
        self.set(a[0], a[1], a[2], a[3]);
    }

    pub fn set_x(&mut self, v: f64) {
        self.set_index(0, v);
    }

    pub fn set_y(&mut self, v: f64) {
        self.set_index(1, v);
    }

    pub fn set_z(&mut self, v: f64) {
        self.set_index(2, v);
    }

    pub fn set_angle(&mut self, v: f64) {
        self.set_index(3, v);
    }

    pub fn quaternion(&self) -> Quaternion {
        self.quaternion
    }

    /// The whole state of the JS object (`[_x]`, `[_y]`, `[_z]`, `[_angle]`, `[_tainted]`, `[_quaternion]`), for a Script's
    /// Rotation4 that must hold exactly what the field holds.
    pub fn state(&self) -> ([f64; 4], bool, Quaternion) {
        ([self.x, self.y, self.z, self.angle], self.tainted, self.quaternion)
    }

    /// A rotation with exactly this state (see [`Rotation4::state`]).
    pub fn from_state(axis_angle: [f64; 4], tainted: bool, quaternion: Quaternion) -> Self {
        let [x, y, z, angle] = axis_angle;
        Self { x, y, z, angle, tainted, quaternion }
    }

    pub fn equals(&self, other: &Self) -> bool {
        self.quaternion.equals(&other.quaternion)
    }

    pub fn is_tainted(&self) -> bool {
        self.tainted
    }

    pub fn get_axis(&mut self) -> Vector3 {
        self.untaint();
        Vector3::new(self.x, self.y, self.z)
    }

    pub fn get_matrix(&self) -> Matrix3 {
        self.quaternion.get_matrix3()
    }

    pub fn inverse(&mut self) -> &mut Self {
        self.quaternion.conjugate();
        self.tainted = true;
        self
    }

    pub fn mult_left(&mut self, rotation: &Rotation4) -> &mut Self {
        self.quaternion.mult_left(&rotation.quaternion).normalize();
        self.tainted = true;
        self
    }

    pub fn mult_right(&mut self, rotation: &Rotation4) -> &mut Self {
        self.quaternion.mult_right(&rotation.quaternion).normalize();
        self.tainted = true;
        self
    }

    pub fn mult_rot_vec(&self, v: &mut Vector3) {
        self.quaternion.mult_quat_vec(v);
    }

    pub fn mult_vec_rot(&self, v: &mut Vector3) {
        self.quaternion.mult_vec_quat(v);
    }

    pub fn set_matrix(&mut self, m: &Matrix3) -> &mut Self {
        self.quaternion.set_matrix(m).normalize();
        self.tainted = true;
        self
    }

    pub fn set_quaternion(&mut self, q: &Quaternion) -> &mut Self {
        self.quaternion = *q;
        self.quaternion.normalize();
        self.tainted = true;
        self
    }

    pub fn set_euler(&mut self, x: f64, y: f64, z: f64) -> &mut Self {
        self.quaternion.set_euler(x, y, z);
        self.tainted = true;
        self
    }

    /// Rotation4.setEuler (x, y, z, order): an unknown order keeps the quaternion (still tainted).
    pub fn set_euler_order(&mut self, x: f64, y: f64, z: f64, order: &str) -> &mut Self {
        self.quaternion.set_euler_order(x, y, z, order);
        self.tainted = true;
        self
    }

    pub fn slerp(&mut self, dest: &Rotation4, t: f64) -> &mut Self {
        self.quaternion.slerp(&dest.quaternion, t);
        self.tainted = true;
        self
    }

    /// Rotation4.squad (a, b, dest, t).
    pub fn squad(&mut self, a: &Rotation4, b: &Rotation4, dest: &Rotation4, t: f64) -> &mut Self {
        self.quaternion.squad(&a.quaternion, &b.quaternion, &dest.quaternion, t);
        self.tainted = true;
        self
    }

    /// Rotation4.spline (r0, r1, r2): a rotation made from Quaternion.spline, then normalized.
    pub fn spline(r0: &Rotation4, r1: &Rotation4, r2: &Rotation4) -> Rotation4 {
        let mut r = Rotation4 { x: 0.0, y: 0.0, z: 1.0, angle: 0.0, tainted: true, quaternion: Quaternion::spline(&r0.quaternion, &r1.quaternion, &r2.quaternion) };
        r.normalize();
        r
    }

    /// Rotation4.setVectors (Coin3D SbRotation algorithm).
    pub fn set_vectors(&mut self, from_vec: &Vector3, to_vec: &Vector3) -> &mut Self {
        let mut from = *from_vec;
        from.normalize();
        let mut to = *to_vec;
        to.normalize();

        let cos_angle = jsmath::clamp(from.dot(&to), -1.0, 1.0);
        let mut crossvec = from;
        crossvec.cross(&to).normalize();
        let crosslen = crossvec.norm();

        if crosslen == 0.0 {
            if cos_angle > 0.0 {
                self.quaternion.set(0.0, 0.0, 0.0, 1.0);
            } else {
                let mut t = from;
                t.cross(&Vector3::X_AXIS);

                if t.squared_norm() == 0.0 {
                    t = from;
                    t.cross(&Vector3::Y_AXIS);
                }

                t.normalize();
                self.quaternion.set(t.x, t.y, t.z, 0.0);
            }
        } else {
            crossvec.multiply(jsmath::sqrt((1.0 - cos_angle).abs() / 2.0));
            self.quaternion.set(crossvec.x, crossvec.y, crossvec.z, jsmath::sqrt((1.0 + cos_angle).abs() / 2.0));
        }

        self.tainted = true;
        self
    }
}
