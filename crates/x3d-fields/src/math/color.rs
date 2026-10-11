//! Color3 / Color4 (standard/Math/Numbers/Color{3,4}.js). Every setter clamps to [0, 1].

use crate::jsmath::{self, clamp};
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Color3 {
    r: f64,
    g: f64,
    b: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Color4 {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

impl Color3 {
    pub const BLACK: Color3 = Color3 { r: 0.0, g: 0.0, b: 0.0 };
    pub const WHITE: Color3 = Color3 { r: 1.0, g: 1.0, b: 1.0 };

    pub fn new(r: f64, g: f64, b: f64) -> Self {
        Self { r: clamp(r, 0.0, 1.0), g: clamp(g, 0.0, 1.0), b: clamp(b, 0.0, 1.0) }
    }

    pub fn set(&mut self, r: f64, g: f64, b: f64) -> &mut Self {
        *self = Self::new(r, g, b);
        self
    }

    pub fn r(&self) -> f64 {
        self.r
    }

    pub fn g(&self) -> f64 {
        self.g
    }

    pub fn b(&self) -> f64 {
        self.b
    }

    pub fn set_r(&mut self, v: f64) {
        self.r = clamp(v, 0.0, 1.0);
    }

    pub fn set_g(&mut self, v: f64) {
        self.g = clamp(v, 0.0, 1.0);
    }

    pub fn set_b(&mut self, v: f64) {
        self.b = clamp(v, 0.0, 1.0);
    }

    pub fn set_index(&mut self, i: usize, v: f64) {
        match i {
            0 => self.set_r(v),
            1 => self.set_g(v),
            _ => self.set_b(v),
        }
    }

    pub fn components(&self) -> [f64; 3] {
        [self.r, self.g, self.b]
    }

    pub fn equals(&self, o: &Self) -> bool {
        self.r == o.r && self.g == o.g && self.b == o.b
    }

    pub fn get_hsv(&self) -> [f64; 3] {
        hsv_from_rgb(self.r, self.g, self.b)
    }

    /// Note: setHSV assigns the private components directly, without clamping.
    pub fn set_hsv(&mut self, h: f64, s: f64, v: f64) -> &mut Self {
        let [r, g, b] = rgb_from_hsv(h, s, v);
        self.r = r;
        self.g = g;
        self.b = b;
        self
    }
}

impl Color4 {
    pub fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r: clamp(r, 0.0, 1.0), g: clamp(g, 0.0, 1.0), b: clamp(b, 0.0, 1.0), a: clamp(a, 0.0, 1.0) }
    }

    pub fn set(&mut self, r: f64, g: f64, b: f64, a: f64) -> &mut Self {
        *self = Self::new(r, g, b, a);
        self
    }

    pub fn set_index(&mut self, i: usize, v: f64) {
        let v = clamp(v, 0.0, 1.0);
        match i {
            0 => self.r = v,
            1 => self.g = v,
            2 => self.b = v,
            _ => self.a = v,
        }
    }

    pub fn components(&self) -> [f64; 4] {
        [self.r, self.g, self.b, self.a]
    }

    pub fn equals(&self, o: &Self) -> bool {
        self.r == o.r && self.g == o.g && self.b == o.b && self.a == o.a
    }

    pub fn get_hsva(&self) -> [f64; 4] {
        let [h, s, v] = hsv_from_rgb(self.r, self.g, self.b);
        [h, s, v, self.a]
    }

    pub fn set_hsva(&mut self, h: f64, s: f64, v: f64, a: f64) -> &mut Self {
        let [r, g, b] = rgb_from_hsv(h, s, v);
        self.r = r;
        self.g = g;
        self.b = b;
        self.a = a;
        self
    }
}

/// Color3.getHSV.
fn hsv_from_rgb(r: f64, g: f64, b: f64) -> [f64; 3] {
    let min = jsmath::js_min(jsmath::js_min(r, g), b);
    let max = jsmath::js_max(jsmath::js_max(r, g), b);
    let v = max;
    let delta = max - min;
    let (mut h, s);

    if max != 0.0 && delta != 0.0 {
        s = delta / max;

        h = if r == max {
            (g - b) / delta
        } else if g == max {
            2.0 + (b - r) / delta
        } else {
            4.0 + (r - g) / delta
        };

        h *= PI / 3.0;

        if h < 0.0 {
            h += PI * 2.0;
        }
    } else {
        s = 0.0;
        h = 0.0;
    }

    [h, s, v]
}

/// Color3.setHSV.
fn rgb_from_hsv(h: f64, s: f64, v: f64) -> [f64; 3] {
    let s = jsmath::js_max(s, 0.0);
    let v = jsmath::js_max(v, 0.0);

    if s == 0.0 {
        return [v, v, v];
    }

    // degrees (interval (h, 0, 2π)) / 60
    let w = (jsmath::interval(h, 0.0, PI * 2.0) * (180.0 / PI)) / 60.0;
    let i = w.floor();
    let f = w - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    // JS `i % 6` on a float; i is in [0, 6].
    match (i % 6.0) as i64 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}
