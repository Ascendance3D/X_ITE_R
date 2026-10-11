//! X3DInterpolatorNode.js and the concrete interpolators (Interpolation component).

use super::State;
use crate::*;
use x3d_fields::jsmath;
use x3d_fields::math::{Color3, Rotation4, Vector2, Vector3};

/// Algorithm.upperBound (array, 0, length, value) with `less`.
pub fn upper_bound(array: &[f64], value: f64) -> usize {
    let (mut first, mut count) = (0usize, array.len());

    while count > 0 {
        let step = count >> 1;
        let index = first + step;

        if value < array[index] {
            count = step;
        } else {
            first = index + 1;
            count -= step + 1;
        }
    }

    first
}

/// Color3.lerp (a, b, t, r) in HSV space.
pub fn hsv_lerp(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    let [mut ha, sa, va] = a;
    let [mut hb, sb, vb] = b;

    if sa == 0.0 {
        ha = hb;
    }

    if sb == 0.0 {
        hb = ha;
    }

    let range = (hb - ha).abs();

    if range <= std::f64::consts::PI {
        return [ha + t * (hb - ha), sa + t * (sb - sa), va + t * (vb - va)];
    }

    let pi2 = std::f64::consts::PI * 2.0;
    let step = (pi2 - range) * t;
    let mut h = if ha < hb { ha - step } else { ha + step };

    if h < 0.0 {
        h += pi2;
    } else if h > pi2 {
        h -= pi2;
    }

    [h, sa + t * (sb - sa), va + t * (vb - va)]
}

impl Runtime {
    pub(crate) fn interp_initialize(&mut self, node: NodeId, name: &str) {
        self.interest(node, "set_fraction", Cb::InterpSetFraction);
        self.interest(node, "key", Cb::InterpSetKey);

        match name {
            // CoordinateInterpolator(2D) do not watch keyValue.
            "CoordinateInterpolator" | "CoordinateInterpolator2D" => {}
            _ => self.interest(node, "keyValue", Cb::InterpSetKeyValue),
        }
    }

    fn key(&self, node: NodeId) -> Vec<f64> {
        match self.pre_value(node, "key") {
            Some(FieldValue::MFFloat(v)) => v.iter().map(|x| *x as f64).collect(),
            _ => vec![],
        }
    }

    /// X3DInterpolatorNode.set_fraction__.
    pub(crate) fn interp_set_fraction(&mut self, node: NodeId) {
        let key = self.key(node);
        let fraction = self.get_f64(node, "set_fraction");
        let length = key.len();

        match length {
            0 => {}
            1 => self.interpolate(node, 0, 0, 0.0),
            _ => {
                if fraction <= key[0] {
                    return self.interpolate(node, 0, 1, 0.0);
                }

                let index1 = upper_bound(&key, fraction);

                if index1 != length {
                    let index0 = index1 - 1;
                    let weight = (fraction - key[index0]) / (key[index1] - key[index0]);

                    self.interpolate(node, index0, index1, super::time::clamp(weight, 0.0, 1.0));
                } else {
                    self.interpolate(node, length - 2, length - 1, 1.0);
                }
            }
        }
    }

    pub(crate) fn interp_set_key(&mut self, node: NodeId) {
        self.interp_set_key_value(node);
    }

    /// `keyValue .resize (key .length, last or default)`; MFField.resize adds an event.
    fn resize_key_value(&mut self, node: NodeId) {
        let key_len = self.key(node).len();
        let Some(k) = self.pre(node, "keyValue") else {
            return;
        };

        let mut v = self.value(k).clone();
        let len = v.len();

        if len >= key_len {
            return;
        }

        let comps = v.field_type().components();
        let fill: Vec<f64> = match &v {
            FieldValue::MFFloat(a) => vec![a.last().copied().unwrap_or(0.0) as f64],
            FieldValue::MFVec3f(a) | FieldValue::MFColor(a) | FieldValue::MFVec2f(a) => {
                if len > 0 { a[a.len() - comps..].iter().map(|x| *x as f64).collect() } else { vec![0.0; comps] }
            }
            // new SFRotation () is 0 0 1 0.
            FieldValue::MFRotation(a) => {
                if len > 0 { a[a.len() - 4..].to_vec() } else { vec![0.0, 0.0, 1.0, 0.0] }
            }
            _ => return,
        };

        for _ in len..key_len {
            v.push_numbers(&fill);
        }

        self.set_value(k, v);
    }

    /// set_keyValue__ of the concrete interpolators.
    pub(crate) fn interp_set_key_value(&mut self, node: NodeId) {
        let name = self.builtin_name(node).unwrap_or("");

        match name {
            "PositionInterpolator" | "OrientationInterpolator" | "ScalarInterpolator" | "PositionInterpolator2D" | "ColorInterpolator" => {
                self.resize_key_value(node)
            }
            _ => {}
        }

        if name == "ColorInterpolator" {
            let hsv: Vec<[f64; 3]> = match self.pre_value(node, "keyValue") {
                Some(FieldValue::MFColor(a)) => {
                    a.chunks(3).map(|c| Color3::new(c[0] as f64, c[1] as f64, c[2] as f64).get_hsv()).collect()
                }
                _ => vec![],
            };

            self.nrt_mut(node).state = State::ColorInterp { hsv };
        }

        // "If there was already an set_fraction event in this frame": the modification time is
        // wall-clock time, always >= the frozen browser time once set.
        if self.pre(node, "set_fraction").is_some_and(|k| self.field(k).modified) {
            self.interp_set_fraction(node);
        }
    }

    fn interpolate(&mut self, node: NodeId, index0: usize, index1: usize, weight: f64) {
        let name = self.builtin_name(node).unwrap_or("");
        let Some(kv) = self.pre_value(node, "keyValue").cloned() else {
            return;
        };
        let Some(out) = self.pre(node, "value_changed") else {
            return;
        };

        match (name, &kv) {
            ("PositionInterpolator", FieldValue::MFVec3f(a)) => {
                let get = |i: usize| Vector3::new(a[i * 3] as f64, a[i * 3 + 1] as f64, a[i * 3 + 2] as f64);
                let mut v = get(index0);
                v.lerp(&get(index1), weight);
                self.set_value(out, FieldValue::SFVec3f(v));
            }
            ("PositionInterpolator2D", FieldValue::MFVec2f(a)) => {
                let get = |i: usize| Vector2::new(a[i * 2] as f64, a[i * 2 + 1] as f64);
                let mut v = get(index0);
                v.lerp(&get(index1), weight);
                self.set_value(out, FieldValue::SFVec2f(v));
            }
            ("OrientationInterpolator", FieldValue::MFRotation(a)) => {
                let get = |i: usize| Rotation4::new(a[i * 4], a[i * 4 + 1], a[i * 4 + 2], a[i * 4 + 3]);
                let mut r = get(index0);
                r.slerp(&get(index1), weight);
                self.set_value(out, FieldValue::SFRotation(r));
            }
            ("ScalarInterpolator", FieldValue::MFFloat(a)) => {
                let (a0, a1) = (a[index0] as f64, a[index1] as f64);
                self.set_value(out, FieldValue::SFFloat(a0 + weight * (a1 - a0)));
            }
            ("ColorInterpolator", _) => {
                let hsv = match &self.nrt(node).map(|n| &n.state) {
                    Some(State::ColorInterp { hsv }) => hsv.clone(),
                    _ => return,
                };

                if index0 >= hsv.len() || index1 >= hsv.len() {
                    return;
                }

                let [h, s, v] = hsv_lerp(hsv[index0], hsv[index1], weight);
                let mut c = match self.value(out) {
                    FieldValue::SFColor(c) => *c,
                    _ => Color3::default(),
                };
                c.set_hsv(h, s, v);
                self.set_value(out, FieldValue::SFColor(c));
            }
            ("CoordinateInterpolator", FieldValue::MFVec3f(a)) | ("CoordinateInterpolator2D", FieldValue::MFVec2f(a)) => {
                let comps = if name == "CoordinateInterpolator" { 3 } else { 2 };
                let key_len = self.key(node).len();
                let size = if key_len > 0 { (a.len() / comps) / key_len } else { 0 };
                let i0 = index0 * size;
                let i1 = i0 + if key_len > 1 { size } else { 0 };
                let mut out_v = vec![0f32; size * comps];

                for (j, o) in out_v.iter_mut().enumerate() {
                    let (x0, x1) = (a.get(i0 * comps + j).copied().unwrap_or(0.0) as f64, a.get(i1 * comps + j).copied().unwrap_or(0.0) as f64);
                    *o = (x0 + weight * (x1 - x0)) as f32;
                }

                let v = if comps == 3 { FieldValue::MFVec3f(out_v) } else { FieldValue::MFVec2f(out_v) };
                self.set_value(out, v);
            }
            ("NormalInterpolator", FieldValue::MFVec3f(a)) => {
                // NormalInterpolator.interpolate: Algorithm.simpleSlerp per normal.
                let key_len = self.key(node).len();
                let size = if key_len > 0 { (a.len() / 3) / key_len } else { 0 };
                let i0 = index0 * size * 3;
                let i1 = (index0 * size + if key_len > 1 { size } else { 0 }) * 3;
                let mut out_v = vec![0f32; size * 3];
                let get = |i: usize| a.get(i).copied().unwrap_or(f32::NAN) as f64;

                for j in (0..size * 3).step_by(3) {
                    let v0 = [get(i0 + j), get(i0 + j + 1), get(i0 + j + 2)];
                    let v1 = [get(i1 + j), get(i1 + j + 1), get(i1 + j + 2)];
                    let v = simple_slerp3(v0, v1, weight);

                    out_v[j] = v[0] as f32;
                    out_v[j + 1] = v[1] as f32;
                    out_v[j + 2] = v[2] as f32;
                }

                self.set_value(out, FieldValue::MFVec3f(out_v));
            }
            _ => {}
        }
    }
}

/// Algorithm.slerp (source, destination, t, short = false) on Vector3 (the `w` it also
/// writes is an ignored expando on a Vector3).
pub fn simple_slerp3(source: [f64; 3], destination: [f64; 3], t: f64) -> [f64; 3] {
    let cosom = source[0] * destination[0] + source[1] * destination[1] + source[2] * destination[2];

    if cosom >= 1.0 {
        return source;
    }

    let omega = jsmath::acos(cosom);
    let sinom = jsmath::sin(omega);
    let scale0 = jsmath::sin((1.0 - t) * omega) / sinom;
    let scale1 = jsmath::sin(t * omega) / sinom;

    [
        source[0] * scale0 + destination[0] * scale1,
        source[1] * scale0 + destination[1] * scale1,
        source[2] * scale0 + destination[2] * scale1,
    ]
}
