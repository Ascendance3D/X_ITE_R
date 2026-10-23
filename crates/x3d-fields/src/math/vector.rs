//! Vector2 / Vector3 / Vector4 (standard/Math/Numbers/Vector{2,3,4}.js).
//!
//! Methods mutate in place and return `&mut Self`, like the JS originals, so call chains
//! translate one-to-one.

use crate::jsmath::{self, hypot};

macro_rules! vector {
    ($name:ident, $len:expr, $($f:ident),+) => {
        #[derive(Clone, Copy, Debug, PartialEq, Default)]
        pub struct $name { $(pub $f: f64),+ }

        impl $name {
            pub const LENGTH: usize = $len;

            pub fn new($($f: f64),+) -> Self { Self { $($f),+ } }

            pub fn set(&mut self, $($f: f64),+) -> &mut Self { $(self.$f = $f;)+ self }

            pub fn assign(&mut self, other: &Self) -> &mut Self { *self = *other; self }

            /// `===` on every component (so NaN never equals, and 0 equals -0).
            pub fn equals(&self, other: &Self) -> bool { true $(&& self.$f == other.$f)+ }

            pub fn abs(&mut self) -> &mut Self { $(self.$f = self.$f.abs();)+ self }

            pub fn add(&mut self, other: &Self) -> &mut Self { $(self.$f += other.$f;)+ self }

            pub fn subtract(&mut self, other: &Self) -> &mut Self { $(self.$f -= other.$f;)+ self }

            pub fn multiply(&mut self, value: f64) -> &mut Self { $(self.$f *= value;)+ self }

            pub fn mult_vec(&mut self, other: &Self) -> &mut Self { $(self.$f *= other.$f;)+ self }

            pub fn divide(&mut self, value: f64) -> &mut Self { $(self.$f /= value;)+ self }

            pub fn div_vec(&mut self, other: &Self) -> &mut Self { $(self.$f /= other.$f;)+ self }

            pub fn negate(&mut self) -> &mut Self { $(self.$f = -self.$f;)+ self }

            pub fn inverse(&mut self) -> &mut Self { $(self.$f = 1.0 / self.$f;)+ self }

            pub fn max(&mut self, other: &Self) -> &mut Self { $(self.$f = jsmath::js_max(self.$f, other.$f);)+ self }

            pub fn min(&mut self, other: &Self) -> &mut Self { $(self.$f = jsmath::js_min(self.$f, other.$f);)+ self }

            pub fn clamp(&mut self, min: &Self, max: &Self) -> &mut Self {
                $(self.$f = jsmath::clamp(self.$f, min.$f, max.$f);)+ self
            }

            pub fn lerp(&mut self, other: &Self, t: f64) -> &mut Self {
                $(self.$f += t * (other.$f - self.$f);)+ self
            }

            pub fn dot(&self, other: &Self) -> f64 {
                // Left-to-right sum like `this .x * x + this .y * y + …`.
                let mut sum = None::<f64>;
                $(sum = Some(match sum { None => self.$f * other.$f, Some(s) => s + self.$f * other.$f });)+
                sum.unwrap()
            }

            pub fn squared_norm(&self) -> f64 { self.dot(self) }

            pub fn norm(&self) -> f64 { hypot(&[$(self.$f),+]) }

            pub fn distance(&self, other: &Self) -> f64 { hypot(&[$(self.$f - other.$f),+]) }

            pub fn normalize(&mut self) -> &mut Self {
                let length = self.norm();
                if length != 0.0 && !length.is_nan() { $(self.$f /= length;)+ }
                self
            }

            pub fn reflect(&mut self, normal: &Self) -> &mut Self {
                let d = 2.0 * normal.dot(self);
                $(self.$f -= normal.$f * d;)+
                self
            }

            pub fn to_array(&self) -> [f64; $len] { [$(self.$f),+] }

            pub fn from_slice(v: &[f64]) -> Self {
                let mut i = 0;
                $(let $f = v[i]; i += 1;)+
                let _ = i;
                Self { $($f),+ }
            }

            pub fn get(&self, index: usize) -> f64 { self.to_array()[index] }

            pub fn set_index(&mut self, index: usize, value: f64) {
                let mut a = self.to_array();
                a[index] = value;
                *self = Self::from_slice(&a);
            }
        }
    };
}

vector!(Vector2, 2, x, y);
vector!(Vector3, 3, x, y, z);
vector!(Vector4, 4, x, y, z, w);

impl Vector2 {
    pub const ZERO: Vector2 = Vector2 { x: 0.0, y: 0.0 };
    pub const ONE: Vector2 = Vector2 { x: 1.0, y: 1.0 };
}

impl Vector3 {
    pub const ZERO: Vector3 = Vector3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const ONE: Vector3 = Vector3 {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };
    pub const X_AXIS: Vector3 = Vector3 {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    };
    pub const Y_AXIS: Vector3 = Vector3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    pub const Z_AXIS: Vector3 = Vector3 {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };

    pub fn cross(&mut self, b: &Vector3) -> &mut Self {
        let (ax, ay, az) = (self.x, self.y, self.z);

        self.x = ay * b.z - az * b.y;
        self.y = az * b.x - ax * b.z;
        self.z = ax * b.y - ay * b.x;
        self
    }
}

impl Vector4 {
    pub const ZERO: Vector4 = Vector4 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 0.0,
    };
    pub const W_AXIS: Vector4 = Vector4 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };
    pub const Z_AXIS: Vector4 = Vector4 {
        x: 0.0,
        y: 0.0,
        z: 1.0,
        w: 0.0,
    };
}
