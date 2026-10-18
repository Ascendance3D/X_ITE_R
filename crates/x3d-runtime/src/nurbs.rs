//! NURBS math for the X3D NURBS component (ISO/IEC 19775-1 clause 27), shared by the geometry
//! builders (x3d-render) and the NURBS interpolators (this crate).
//!
//! The evaluation is a port of X_ITE 16.5.0's `lib/nurbs` (the de Boor code that
//! `evaluator.js` generates, in the same operation order, so non-rational results are bit equal)
//! and `extras/sample.js` (grid sampling, seam merging, triangle order), plus
//! `Browser/NURBS/NURBS.js` (tessellation counts, knot vectors, control points).
//!
//! Owner decision 2026-10-17 (REA batch 04): where ISO and X_ITE disagree, this module follows
//! ISO. The differences are marked `ISO:` in the comments:
//! - rational evaluation: control points are multiplied by their weights (homogeneous
//!   coordinates), X_ITE does not multiply them;
//! - fewer weights than control points: weights ignored; more: the first ones are used (X_ITE
//!   ignores any count other than the exact one);
//! - `closed` is ignored when a weight within (order - 1) control points of the seam is not 1.

/// NURBS.getTessellation: the number of tessellation faces (points = faces + 1).
pub fn tessellation(tessellation: i32, dimension: i32) -> i32 {
    if tessellation > 0 {
        return tessellation;
    }

    if tessellation < 0 {
        return tessellation.wrapping_neg().wrapping_mul(dimension);
    }

    2i32.wrapping_mul(dimension)
}

/// ISO 27.2.3 weights: `None` (non-rational) when there are fewer weights than control points.
pub fn weights(dimension: usize, weight: &[f64]) -> Option<Vec<f64>> {
    // ISO: "If the number of weight values is less than the number of control points, all
    // weight values shall be ignored and a value of 1.0 shall be used."
    if dimension == 0 || weight.len() < dimension {
        return None;
    }

    Some(weight[..dimension].to_vec())
}

/// ISO 27.2.3: `closed` is ignored when a weight within (order − 1) control points of the seam
/// is not 1.
fn seam_weights_unitary(order: usize, dimension: usize, weight_at: impl Fn(usize) -> f64) -> bool {
    let n = order.saturating_sub(1).min(dimension);

    (0..n).all(|i| weight_at(i) == 1.0 && weight_at(dimension - 1 - i) == 1.0)
}

/// NURBS.getClosed (3D curve) with the ISO weight rule. `points` are x y z triples.
pub fn curve_closed(order: usize, weights: Option<&[f64]>, points: &[[f64; 3]]) -> bool {
    let dimension = points.len();

    if dimension == 0 {
        return false;
    }

    if let Some(w) = weights {
        if !seam_weights_unitary(order, dimension, |i| w[i]) {
            return false;
        }
    }

    points[0] == points[dimension - 1]
}

/// NURBS.getClosed2D with the ISO weight rule.
pub fn curve2d_closed(order: usize, weights: Option<&[f64]>, points: &[[f64; 2]]) -> bool {
    let dimension = points.len();

    if dimension == 0 {
        return false;
    }

    if let Some(w) = weights {
        if !seam_weights_unitary(order, dimension, |i| w[i]) {
            return false;
        }
    }

    points[0] == points[dimension - 1]
}

/// NURBS.getUClosed with the ISO weight rule: every v row must close.
pub fn u_closed(order: usize, u_dimension: usize, v_dimension: usize, weights: Option<&[f64]>, point: impl Fn(usize) -> [f64; 3]) -> bool {
    for v in 0..v_dimension {
        let first = v * u_dimension;
        let last = v * u_dimension + u_dimension - 1;

        if let Some(w) = weights {
            if !seam_weights_unitary(order, u_dimension, |i| w[first + i]) {
                return false;
            }
        }

        if point(first) != point(last) {
            return false;
        }
    }

    true
}

/// NURBS.getVClosed with the ISO weight rule: every u column must close.
pub fn v_closed(order: usize, u_dimension: usize, v_dimension: usize, weights: Option<&[f64]>, point: impl Fn(usize) -> [f64; 3]) -> bool {
    for u in 0..u_dimension {
        let first = u;
        let last = (v_dimension - 1) * u_dimension + u;

        if let Some(w) = weights {
            if !seam_weights_unitary(order, v_dimension, |i| w[u + i * u_dimension]) {
                return false;
            }
        }

        if point(first) != point(last) {
            return false;
        }
    }

    true
}

/// NURBS.getKnots: the given knots if valid, else uniform (periodic when closed, pinned
/// otherwise) scaled to [0, 1]; closed knot vectors are extended periodically by order − 2.
pub fn knots(closed: bool, order: usize, dimension: usize, knot: &[f64]) -> Vec<f64> {
    let length = dimension + order;
    let mut knots = knot.to_vec();
    let mut generate_uniform = true;

    if knots.len() == length {
        generate_uniform = false;

        let mut consecutive_knots = 0;

        for i in 1..length {
            if knots[i] == knots[i - 1] {
                consecutive_knots += 1;
            } else {
                consecutive_knots = 0;
            }

            if consecutive_knots > order as i64 - 1 {
                generate_uniform = true;
            }

            if knots[i - 1] > knots[i] {
                generate_uniform = true;
            }
        }
    }

    if generate_uniform {
        knots.clear();

        if closed {
            for i in 0..length {
                knots.push(i as f64);
            }
        } else {
            let mut k = 1.0;

            for _ in 0..order {
                knots.push(0.0);
            }

            for _ in order..length.saturating_sub(order) {
                knots.push(k);
                k += 1.0;
            }

            while knots.len() < length {
                knots.push(k);
            }

            knots.truncate(length);
        }

        let max = knots.last().copied().unwrap_or(f64::NAN);

        for k in knots.iter_mut() {
            *k /= max;
        }
    }

    if closed {
        for i in 1..order.saturating_sub(1) {
            let last = knots.last().copied().unwrap_or(f64::NAN);
            let step = at(&knots, i) - at(&knots, i as i64 - 1);
            knots.push(last + step);
        }
    }

    knots
}

/// `array [i]` with JS `undefined` (→ NaN in arithmetic) outside the array.
fn at(a: &[f64], i: impl TryInto<usize>) -> f64 {
    i.try_into().ok().and_then(|i: usize| a.get(i).copied()).unwrap_or(f64::NAN)
}

/// A B-spline curve (one parameter) or surface (two), evaluated like X_ITE's lib/nurbs with an
/// open boundary: `points` is the control net as `size [0] × size [1]` points (u major) of
/// `components` values each.
#[derive(Clone, Debug, Default)]
pub struct Spline {
    pub degree: Vec<usize>,
    pub knots: Vec<Vec<f64>>,
    pub size: Vec<usize>,
    pub components: usize,
    pub points: Vec<f64>,
}

impl Spline {
    pub fn curve(degree: usize, knots: Vec<f64>, components: usize, points: Vec<f64>) -> Self {
        let size = if components > 0 { points.len() / components } else { 0 };
        Spline { degree: vec![degree], knots: vec![knots], size: vec![size], components, points }
    }

    pub fn surface(degree: [usize; 2], knots: [Vec<f64>; 2], size: [usize; 2], components: usize, points: Vec<f64>) -> Self {
        Spline { degree: degree.to_vec(), knots: knots.to_vec(), size: size.to_vec(), components, points }
    }

    /// nurbs.domain: [k [degree], k [size]] per parameter.
    pub fn domain(&self, d: usize) -> (f64, f64) {
        let k = &self.knots[d];
        (at(k, self.degree[d]), at(k, self.size[d]))
    }

    /// The generated `evaluate` (derivative None) or `evaluator (derivative)` function. `out`
    /// receives `components` values.
    pub fn evaluate(&self, t: &[f64], derivative: Option<&[usize]>, out: &mut [f64]) {
        let dims = self.degree.len();
        let c = self.components;
        let mut j = [0usize; 2];
        let mut kl: [Vec<f64>; 2] = [vec![], vec![]];

        for d in 0..dims {
            // Bisect to locate the knot interval.
            let k = &self.knots[d];
            let mut jd: i64 = 0;
            let mut h: i64 = self.size[d] as i64;

            while h > jd + 1 {
                let m = (0.5 * (h + jd) as f64) as i64;

                if at(k, m) > t[d] {
                    h = m;
                } else {
                    jd = m;
                }
            }

            j[d] = jd as usize;

            let p = self.degree[d] as i64;
            kl[d] = (-p + 1..=p).map(|i| at(k, jd + i)).collect();
        }

        let p0 = self.degree[0];
        let p1 = if dims == 2 { self.degree[1] } else { 0 };
        let n1 = p1 + 1;
        let idx = |a: usize, b: usize, comp: usize| ((a * n1) + b) * c + comp;
        let mut x = vec![f64::NAN; (p0 + 1) * n1 * c];

        // Fetch points.
        for a in 0..=p0 {
            for b in 0..n1 {
                let pu = j[0] as i64 + a as i64 - p0 as i64;
                let pv = j[1] as i64 + b as i64 - p1 as i64;

                if pu < 0 || pv < 0 || pu as usize >= self.size[0] || (dims == 2 && pv as usize >= self.size[1]) {
                    continue;
                }

                let base = if dims == 2 { (pu as usize * self.size[1] + pv as usize) * c } else { pu as usize * c };

                for comp in 0..c {
                    x[idx(a, b, comp)] = self.points[base + comp];
                }
            }
        }

        // De Boor's algorithm, last parameter first (`for (let d = n.length - 1; d >= 0; -- d)`).
        for d in (0..dims).rev() {
            let p = self.degree[d];
            let der = derivative.map(|v| v.get(d).copied().unwrap_or(0)).unwrap_or(0);
            let kv = &kl[d];
            let kv = |i: usize| kv[i];

            for i in 0..p {
                for jj in (i + 1..=p).rev() {
                    let is_derivative = derivative.is_some() && (p as i64 - i as i64 - der as i64) <= 0;
                    let lo = kv(jj - 1);
                    let hi = kv(jj - i + p - 1);

                    // The other parameter runs over its whole range while it is not reduced yet
                    // (d = 1 first: every a), else it sits at its degree.
                    let others: Vec<(usize, usize)> = if dims == 2 {
                        if d == 1 { (0..=p0).map(|a| (a, jj)).collect() } else { vec![(jj, p1)] }
                    } else {
                        vec![(jj, 0)]
                    };

                    if is_derivative {
                        let m = 1.0 / (hi - lo);
                        let coeff = (i + 1) as f64;

                        for (a, b) in others {
                            let (a1, b1) = if dims == 2 && d == 1 { (a, b - 1) } else { (a - 1, b) };

                            for comp in 0..c {
                                let v = coeff * (x[idx(a, b, comp)] - x[idx(a1, b1, comp)]) * m;
                                x[idx(a, b, comp)] = v;
                            }
                        }
                    } else {
                        let alpha = (t[d] - lo) / (hi - lo);
                        let beta = 1.0 - alpha;

                        for (a, b) in others {
                            let (a1, b1) = if dims == 2 && d == 1 { (a, b - 1) } else { (a - 1, b) };

                            for comp in 0..c {
                                let v = beta * x[idx(a1, b1, comp)] + alpha * x[idx(a, b, comp)];
                                x[idx(a, b, comp)] = v;
                            }
                        }
                    }
                }
            }
        }

        for comp in 0..c {
            out[comp] = x[idx(p0, p1, comp)];
        }
    }
}

/// A sampled mesh (extras/sample.js): `points` (`dimension` values each) and `faces` (point
/// indices, three per triangle; surfaces only).
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub points: Vec<f64>,
    pub faces: Vec<usize>,
}

/// sample.js, one parameter: `resolution + 1` points; a closed curve repeats the first point.
pub fn sample_curve(spline: &Spline, resolution: usize, closed: bool, have_weights: bool) -> Mesh {
    let dimension = spline.components - have_weights as usize;
    let nu = resolution;
    let (u0, u1) = spline.domain(0);
    let u_distance = u1 - u0;
    let mut tmp = vec![0.0; spline.components];
    let mut points = Vec::with_capacity((nu + 1) * dimension);

    for iu in 0..=nu {
        let k = if closed { iu % nu.max(1) } else { iu };
        let u = u0 + u_distance * k as f64 / nu as f64;

        spline.evaluate(&[u], None, &mut tmp);

        if have_weights {
            let w = tmp[dimension];
            points.extend(tmp[..dimension].iter().map(|v| v / w));
        } else {
            points.extend_from_slice(&tmp[..dimension]);
        }
    }

    Mesh { points, faces: vec![] }
}

/// The `key` of sample.js uvSeam: the decimal strings of the values. Equal strings ⇔ equal
/// values, with -0 equal to 0 and NaN equal to NaN.
fn seam_key(values: &[f64]) -> Vec<u64> {
    values
        .iter()
        .map(|v| {
            if v.is_nan() {
                f64::NAN.to_bits()
            } else if *v == 0.0 {
                0
            } else {
                v.to_bits()
            }
        })
        .collect()
}

/// sample.js, two parameters: a (nu + 1) × (nv + 1) grid, seam points merged when equal, two
/// triangles per cell.
pub fn sample_surface(spline: &Spline, nu: usize, nv: usize, u_closed: bool, v_closed: bool, have_weights: bool) -> Mesh {
    use std::collections::HashMap;

    let dimension = spline.components - have_weights as usize;
    let (nu_bound, nv_bound) = (nu + 1, nv + 1);
    let (u0, u1) = spline.domain(0);
    let (v0d, v1d) = spline.domain(1);
    let (u_distance, v_distance) = (u1 - u0, v1d - v0d);
    let mut tmp = vec![0.0; spline.components];
    let mut points = vec![0.0; nu_bound * nv_bound * dimension];

    for iv in 0..nv_bound {
        let v = v0d + v_distance * iv as f64 / nv as f64;

        for iu in 0..nu_bound {
            let u = u0 + u_distance * iu as f64 / nu as f64;
            let ptr = (iu + nu_bound * iv) * dimension;

            spline.evaluate(&[u, v], None, &mut tmp);

            if have_weights {
                let w = tmp[dimension];
                for d in 0..dimension {
                    points[ptr + d] = tmp[d] / w;
                }
            } else {
                points[ptr..ptr + dimension].copy_from_slice(&tmp[..dimension]);
            }
        }
    }

    // Combine seam points if equal.
    let mut point_index: HashMap<Vec<u64>, usize> = HashMap::new();
    let mut seam_index: HashMap<usize, usize> = HashMap::new();
    let mut uv_seam = |i: usize, points: &[f64]| {
        let key = seam_key(&points[i * dimension..(i + 1) * dimension]);

        match point_index.get(&key) {
            Some(&p) => {
                seam_index.insert(i, p);
            }
            None => {
                point_index.insert(key, i);
            }
        }
    };

    let u_seam = |u: usize, f: &mut dyn FnMut(usize)| (0..nv_bound).for_each(|v| f(u + nu_bound * v));
    let v_seam = |v: usize, f: &mut dyn FnMut(usize)| (0..nu_bound).for_each(|u| f(u + nu_bound * v));

    u_seam(0, &mut |i| uv_seam(i, &points));
    v_seam(0, &mut |i| uv_seam(i, &points));

    if !u_closed {
        u_seam(nu, &mut |i| uv_seam(i, &points));
    }

    if !v_closed {
        v_seam(nv, &mut |i| uv_seam(i, &points));
    }

    let mut faces = Vec::with_capacity(nu * nv * 6);

    for v0 in 0..nv {
        let v1 = if v_closed { (v0 + 1) % nv } else { v0 + 1 };

        for u0 in 0..nu {
            let u1 = if u_closed { (u0 + 1) % nu } else { u0 + 1 };
            let s = |p: usize| seam_index.get(&p).copied().unwrap_or(p);
            let p0 = s(u0 + nu_bound * v0);
            let p1 = s(u1 + nu_bound * v0);
            let p2 = s(u1 + nu_bound * v1);
            let p3 = s(u0 + nu_bound * v1);

            faces.extend_from_slice(&[p0, p1, p2, p0, p2, p3]);
        }
    }

    Mesh { points, faces }
}

/// The control net of a curve: x y z (or x y), homogeneous when weighted (ISO: multiplied by
/// the weight, the weight appended), closed curves extended by order − 2 points.
pub fn curve_control_points<const N: usize>(closed: bool, order: usize, weights: Option<&[f64]>, points: &[[f64; N]]) -> Vec<f64> {
    let mut out = vec![];
    let push = |out: &mut Vec<f64>, i: usize| {
        let p = &points[i];
        match weights {
            Some(w) => {
                out.extend(p.iter().map(|v| v * w[i]));
                out.push(w[i]);
            }
            None => out.extend_from_slice(p),
        }
    };

    for i in 0..points.len() {
        push(&mut out, i);
    }

    if closed {
        for i in 1..order.saturating_sub(1) {
            push(&mut out, i);
        }
    }

    out
}

/// NURBS.getUVControlPoints: the net as [u][v] (u major), homogeneous when weighted (ISO),
/// closed directions extended by order − 2 points. Returns (points, size).
#[allow(clippy::too_many_arguments)]
pub fn surface_control_points(
    u_closed: bool,
    v_closed: bool,
    u_order: usize,
    v_order: usize,
    u_dimension: usize,
    v_dimension: usize,
    weights: Option<&[f64]>,
    point: impl Fn(usize) -> [f64; 3],
) -> (Vec<f64>, [usize; 2]) {
    let components = 3 + weights.is_some() as usize;
    let column = |u: usize| -> Vec<f64> {
        let mut out = vec![];
        let push = |out: &mut Vec<f64>, v: usize| {
            let index = v * u_dimension + u;
            let p = point(index);
            match weights {
                Some(w) => {
                    out.extend(p.iter().map(|x| x * w[index]));
                    out.push(w[index]);
                }
                None => out.extend_from_slice(&p),
            }
        };

        for v in 0..v_dimension {
            push(&mut out, v);
        }

        if v_closed {
            for v in 1..v_order.saturating_sub(1) {
                push(&mut out, v);
            }
        }

        out
    };

    let mut columns: Vec<Vec<f64>> = (0..u_dimension).map(column).collect();

    if u_closed {
        for u in 1..u_order.saturating_sub(1) {
            columns.push(columns[u].clone());
        }
    }

    let size = [columns.len(), columns.first().map(|c| c.len() / components).unwrap_or(0)];

    (columns.concat(), size)
}
