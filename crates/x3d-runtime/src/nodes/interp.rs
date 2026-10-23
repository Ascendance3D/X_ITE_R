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
            "EaseInEaseOut" => self.interest(node, "easeInEaseOut", Cb::InterpSetKeyValue),
            "SplinePositionInterpolator"
            | "SplinePositionInterpolator2D"
            | "SplineScalarInterpolator" => {
                self.interest(node, "keyValue", Cb::InterpSetKeyValue);
                self.interest(node, "keyVelocity", Cb::SplineSetKeyVelocity);
                self.interest(node, "normalizeVelocity", Cb::SplineSetNormalizeVelocity);
            }
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
        self.resize_to_key(node, "keyValue");
    }

    /// `field .resize (key .length, last or default)` when the field is shorter than key.
    fn resize_to_key(&mut self, node: NodeId, field: &str) {
        let key_len = self.key(node).len();
        let Some(k) = self.pre(node, field) else {
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
                if len > 0 {
                    a[a.len() - comps..].iter().map(|x| *x as f64).collect()
                } else {
                    vec![0.0; comps]
                }
            }
            FieldValue::MFVec3d(a) => {
                if len > 0 {
                    a[a.len() - comps..].to_vec()
                } else {
                    vec![0.0; comps]
                }
            }
            // new SFRotation () is 0 0 1 0.
            FieldValue::MFRotation(a) => {
                if len > 0 {
                    a[a.len() - 4..].to_vec()
                } else {
                    vec![0.0, 0.0, 1.0, 0.0]
                }
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
            "EaseInEaseOut" => {
                self.resize_to_key(node, "easeInEaseOut");
                return self.refresh_fraction(node);
            }
            "SplinePositionInterpolator"
            | "SplinePositionInterpolator2D"
            | "SplineScalarInterpolator" => {
                self.resize_key_value(node);
                return self.spline_set_key_velocity(node);
            }
            "SquadOrientationInterpolator" => {
                self.resize_key_value(node);
                self.squad_generate(node);
                return self.refresh_fraction(node);
            }
            _ => {}
        }

        match name {
            "PositionInterpolator"
            | "OrientationInterpolator"
            | "ScalarInterpolator"
            | "PositionInterpolator2D"
            | "ColorInterpolator"
            | "GeoPositionInterpolator" => self.resize_key_value(node),
            _ => {}
        }

        if name == "ColorInterpolator" {
            let hsv: Vec<[f64; 3]> = match self.pre_value(node, "keyValue") {
                Some(FieldValue::MFColor(a)) => a
                    .chunks(3)
                    .map(|c| Color3::new(c[0] as f64, c[1] as f64, c[2] as f64).get_hsv())
                    .collect(),
                _ => vec![],
            };

            self.nrt_mut(node).state = State::ColorInterp { hsv };
        }

        // "If there was already an set_fraction event in this frame": the modification time is
        // wall-clock time, always >= the frozen browser time once set.
        if self
            .pre(node, "set_fraction")
            .is_some_and(|k| self.field(k).modified)
        {
            self.interp_set_fraction(node);
        }
    }

    /// "If there was already an set_fraction event in this frame, send a new value_changed".
    fn refresh_fraction(&mut self, node: NodeId) {
        if self
            .pre(node, "set_fraction")
            .is_some_and(|k| self.field(k).modified)
        {
            self.interp_set_fraction(node);
        }
    }

    /// The numbers of an MFFloat / MFVec2f / MFVec3f field as elements of `n` components.
    fn elements(&self, node: NodeId, field: &str) -> (usize, Vec<Vec<f64>>) {
        let (n, a): (usize, &[f32]) = match self.pre_value(node, field) {
            Some(FieldValue::MFFloat(a)) => (1, a),
            Some(FieldValue::MFVec2f(a)) => (2, a),
            Some(FieldValue::MFVec3f(a)) => (3, a),
            _ => return (1, vec![]),
        };
        (
            n,
            a.chunks(n)
                .map(|c| c.iter().map(|x| *x as f64).collect())
                .collect(),
        )
    }

    /// set_keyVelocity__ of the spline interpolators.
    pub(crate) fn spline_set_key_velocity(&mut self, node: NodeId) {
        let len = self
            .pre_value(node, "keyVelocity")
            .map(|v| v.len())
            .unwrap_or(0);

        if len > 0 && len < self.key(node).len() {
            // keyVelocity .resize (key .length, new SFVec3f ()): zero elements.
            if let Some(k) = self.pre(node, "keyVelocity") {
                let mut v = self.value(k).clone();
                let comps = v.field_type().components();
                for _ in len..self.key(node).len() {
                    v.push_numbers(&vec![0.0; comps]);
                }
                self.set_value(k, v);
            }
        }

        self.spline_set_normalize_velocity(node);
    }

    /// set_normalizeVelocity__: CatmullRomSplineInterpolator.generate, then refresh.
    pub(crate) fn spline_set_normalize_velocity(&mut self, node: NodeId) {
        let closed = self.get_bool(node, "closed");
        let normalize = self.get_bool(node, "normalizeVelocity");
        let key = self.key(node);
        let (n, key_value) = self.elements(node, "keyValue");
        let (_, key_velocity) = self.elements(node, "keyVelocity");
        let (t0, t1) = catmull_rom_generate(n, closed, &key, &key_value, &key_velocity, normalize);

        self.nrt_mut(node).state = State::Spline { t0, t1 };
        self.refresh_fraction(node);
    }

    /// SquadInterpolator.generate.
    fn squad_generate(&mut self, node: NodeId) {
        let closed = self.get_bool(node, "closed");
        let key_len = self.key(node).len();
        let kv: Vec<Rotation4> = match self.pre_value(node, "keyValue") {
            Some(FieldValue::MFRotation(a)) => a
                .chunks(4)
                .map(|c| Rotation4::new(c[0], c[1], c[2], c[3]))
                .collect(),
            _ => vec![],
        };
        let at = |i: usize| kv.get(i).copied().unwrap_or_default();
        let mut s = vec![];

        if key_len > 1 {
            s.push(if closed {
                Rotation4::spline(&at(key_len - 2), &at(0), &at(1))
            } else {
                at(0)
            });

            for i in 1..key_len - 1 {
                s.push(Rotation4::spline(&at(i - 1), &at(i), &at(i + 1)));
            }

            s.push(if closed {
                Rotation4::spline(&at(key_len - 2), &at(key_len - 1), &at(1))
            } else {
                at(key_len - 1)
            });
        } else if key_len > 0 {
            s.push(at(0));
        }

        self.nrt_mut(node).state = State::Squad { s };
    }

    fn interpolate(&mut self, node: NodeId, index0: usize, index1: usize, weight: f64) {
        let name = self.builtin_name(node).unwrap_or("");

        match name {
            "GeoPositionInterpolator" => {
                return self.geo_position_interpolate(node, index0, index1, weight);
            }
            "EaseInEaseOut" => {
                let (_, e) = self.elements(node, "easeInEaseOut");
                let (Some(e0), Some(e1), Some(out)) = (
                    e.get(index0),
                    e.get(index1),
                    self.pre(node, "modifiedFraction_changed"),
                ) else {
                    return;
                };
                let value = ease_in_ease_out(e0[1], e1[0], weight);
                return self.set_value(out, FieldValue::SFFloat(value));
            }
            "SplinePositionInterpolator"
            | "SplinePositionInterpolator2D"
            | "SplineScalarInterpolator" => {
                let (n, kv) = self.elements(node, "keyValue");
                let Some(State::Spline { t0, t1 }) = self.nrt(node).map(|n| &n.state) else {
                    return;
                };
                let nan = vec![f64::NAN; n];
                let pick =
                    |v: &Vec<Vec<f64>>, i: usize| v.get(i).cloned().unwrap_or_else(|| nan.clone());
                let v = catmull_rom_interpolate(
                    weight,
                    &pick(&kv, index0),
                    &pick(&kv, index1),
                    &pick(t0, index0),
                    &pick(t1, index1),
                );
                let Some(out) = self.pre(node, "value_changed") else {
                    return;
                };
                let value = match n {
                    1 => FieldValue::SFFloat(v[0]),
                    2 => FieldValue::SFVec2f(Vector2::new(v[0], v[1])),
                    _ => FieldValue::SFVec3f(Vector3::new(v[0], v[1], v[2])),
                };
                return self.set_value(out, value);
            }
            "SquadOrientationInterpolator" => {
                let kv: Vec<Rotation4> = match self.pre_value(node, "keyValue") {
                    Some(FieldValue::MFRotation(a)) => a
                        .chunks(4)
                        .map(|c| Rotation4::new(c[0], c[1], c[2], c[3]))
                        .collect(),
                    _ => return,
                };
                let Some(State::Squad { s }) = self.nrt(node).map(|n| &n.state) else {
                    return;
                };
                let (Some(a), Some(b), Some(r0), Some(r1)) =
                    (s.get(index0), s.get(index1), kv.get(index0), kv.get(index1))
                else {
                    return;
                };
                let mut r = *r0;
                r.squad(a, b, r1, weight);
                let Some(out) = self.pre(node, "value_changed") else {
                    return;
                };
                return self.set_value(out, FieldValue::SFRotation(r));
            }
            _ => {}
        }

        let Some(kv) = self.pre_value(node, "keyValue").cloned() else {
            return;
        };
        let Some(out) = self.pre(node, "value_changed") else {
            return;
        };

        match (name, &kv) {
            ("PositionInterpolator", FieldValue::MFVec3f(a)) => {
                let get = |i: usize| {
                    Vector3::new(a[i * 3] as f64, a[i * 3 + 1] as f64, a[i * 3 + 2] as f64)
                };
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
                let get =
                    |i: usize| Rotation4::new(a[i * 4], a[i * 4 + 1], a[i * 4 + 2], a[i * 4 + 3]);
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
            ("CoordinateInterpolator", FieldValue::MFVec3f(a))
            | ("CoordinateInterpolator2D", FieldValue::MFVec2f(a)) => {
                let comps = if name == "CoordinateInterpolator" {
                    3
                } else {
                    2
                };
                let key_len = self.key(node).len();
                let size = if key_len > 0 {
                    (a.len() / comps) / key_len
                } else {
                    0
                };
                let i0 = index0 * size;
                let i1 = i0 + if key_len > 1 { size } else { 0 };
                let mut out_v = vec![0f32; size * comps];

                for (j, o) in out_v.iter_mut().enumerate() {
                    let (x0, x1) = (
                        a.get(i0 * comps + j).copied().unwrap_or(0.0) as f64,
                        a.get(i1 * comps + j).copied().unwrap_or(0.0) as f64,
                    );
                    *o = (x0 + weight * (x1 - x0)) as f32;
                }

                let v = if comps == 3 {
                    FieldValue::MFVec3f(out_v)
                } else {
                    FieldValue::MFVec2f(out_v)
                };
                self.set_value(out, v);
            }
            ("NormalInterpolator", FieldValue::MFVec3f(a)) => {
                // NormalInterpolator.interpolate: Algorithm.simpleSlerp per normal.
                let key_len = self.key(node).len();
                let size = if key_len > 0 {
                    (a.len() / 3) / key_len
                } else {
                    0
                };
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
    let cosom =
        source[0] * destination[0] + source[1] * destination[1] + source[2] * destination[2];

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

/// EaseInEaseOut.interpolate: the modified fraction for `weight` between two keys.
pub fn ease_in_ease_out(ease_out: f64, ease_in: f64, weight: f64) -> f64 {
    let (mut ease_out, mut ease_in) = (ease_out, ease_in);
    let sum = ease_out + ease_in;

    if sum < 0.0 {
        return weight;
    }

    if sum > 1.0 {
        ease_in /= sum;
        ease_out /= sum;
    }

    let t = 1.0 / (2.0 - ease_out - ease_in);

    if weight < ease_out {
        (t / ease_out) * weight * weight
    } else if weight <= 1.0 - ease_in {
        t * (2.0 * weight - ease_out)
    } else {
        let w = 1.0 - weight;
        1.0 - ((t * w * w) / ease_in)
    }
}

fn sub(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(x, y)| x - y).collect()
}

fn scale(a: &[f64], f: f64) -> Vec<f64> {
    a.iter().map(|x| x * f).collect()
}

/// `magnitude`: Math.abs for scalars, Vector.norm (Math.hypot) otherwise.
fn magnitude(a: &[f64]) -> f64 {
    if a.len() == 1 {
        a[0].abs()
    } else {
        jsmath::hypot(a)
    }
}

/// CatmullRomSplineInterpolator.generate (T0, T1) for elements of `n` components.
pub fn catmull_rom_generate(
    n: usize,
    closed: bool,
    key: &[f64],
    key_value: &[Vec<f64>],
    key_velocity: &[Vec<f64>],
    normalize: bool,
) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    let zero = vec![0.0; n];
    let nan = vec![f64::NAN; n];
    let kv = |i: usize| key_value.get(i).cloned().unwrap_or_else(|| nan.clone());

    if key.len() <= 1 {
        return (vec![zero.clone()], vec![zero]);
    }

    let mut t: Vec<Vec<f64>> = vec![];

    if key_velocity.is_empty() {
        let len = key_value.len();
        t.push(if closed {
            sub(&kv(1), &kv(len.wrapping_sub(2)))
                .iter()
                .map(|x| x / 2.0)
                .collect()
        } else {
            zero.clone()
        });

        for i in 1..len.saturating_sub(1) {
            t.push(
                sub(&kv(i + 1), &kv(i - 1))
                    .iter()
                    .map(|x| x / 2.0)
                    .collect(),
            );
        }

        let first = t[0].clone();
        t.push(first);
    } else {
        t.extend(key_velocity.iter().cloned());

        if normalize {
            let mut dtot = 0.0;

            for i in 0..key_value.len().saturating_sub(1) {
                dtot += magnitude(&sub(&kv(i), &kv(i + 1)));
            }

            for i in 0..t.len() - 1 {
                let tia = magnitude(&t[i]);

                if tia != 0.0 && !tia.is_nan() {
                    t[i] = scale(&t[i], dtot / tia);
                }
            }
        }
    }

    let (mut fm, mut fp) = (vec![], vec![]);

    if closed {
        let (i_1, i_2) = (key.len() - 1, key.len() - 2);
        let d = key[1] - key[0] + key[i_1] - key[i_2];
        fm.push(2.0 * (key[1] - key[0]) / d);
        fp.push(2.0 * (key[i_1] - key[i_2]) / d);
    } else {
        fm.push(1.0);
        fp.push(1.0);
    }

    for i in 1..key.len() - 1 {
        let d = key[i + 1] - key[i - 1];
        fm.push(2.0 * (key[i + 1] - key[i]) / d);
        fp.push(2.0 * (key[i] - key[i - 1]) / d);
    }

    fm.push(fm[0]);
    fp.push(fp[0]);

    let t0 = t
        .iter()
        .enumerate()
        .map(|(i, v)| scale(v, fp.get(i).copied().unwrap_or(f64::NAN)))
        .collect();
    let t1 = t
        .iter()
        .enumerate()
        .map(|(i, v)| scale(v, fm.get(i).copied().unwrap_or(f64::NAN)))
        .collect();
    (t0, t1)
}

/// CatmullRomSplineInterpolator.interpolate: SH = H · (w³, w², w, 1), then the dot product.
pub fn catmull_rom_interpolate(
    weight: f64,
    c0: &[f64],
    c1: &[f64],
    c2: &[f64],
    c3: &[f64],
) -> Vec<f64> {
    const H: [f64; 16] = [
        2.0, -2.0, 1.0, 1.0, -3.0, 3.0, -2.0, -1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0,
    ];
    let (x, y, z, w) = (
        jsmath::pow(weight, 3.0),
        jsmath::pow(weight, 2.0),
        weight,
        1.0,
    );
    let sh = [
        x * H[0] + y * H[4] + z * H[8] + w * H[12],
        x * H[1] + y * H[5] + z * H[9] + w * H[13],
        x * H[2] + y * H[6] + z * H[10] + w * H[14],
        x * H[3] + y * H[7] + z * H[11] + w * H[15],
    ];

    (0..c0.len())
        .map(|i| c0[i] * sh[0] + c1[i] * sh[1] + c2[i] * sh[2] + c3[i] * sh[3])
        .collect()
}
