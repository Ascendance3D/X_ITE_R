//! sub:fields.math: the port's geometry (src/geom.rs, Box3 in src/lookat.rs) against the pinned X_ITE's standard/Math/Geometry
//! classes, bit for bit. oracle/geom-math-baseline.json is written by oracle/tools/geom_xite.mjs, which runs X_ITE's own modules
//! (three runs identical); every number is the hex of its IEEE 754 double, inputs included, so both sides start from the same
//! bits. No tolerance: an output equals X_ITE's when its bits do (any NaN equals any NaN).

use serde_json::Value;
use x3d_fields::math::{Matrix3, Matrix4, Vector2, Vector3, Vector4};
use x3d_viewer::geom::{Camera, Cylinder3, Line2, Line3, Plane3, Sphere3, Triangle3, Uvt, ViewVolume};
use x3d_viewer::geom2::{Bezier, Box2, Sat, Spheroid3, Triangle2, ViewVolumeObject, eigen_decomposition, merge_sort, quick_sort};
use x3d_viewer::lookat::Box3;

/// The inputs of one case, read in order.
struct In(Vec<f64>, usize);

impl In {
    fn f(&mut self) -> f64 {
        self.1 += 1;
        self.0[self.1 - 1]
    }

    fn v2(&mut self) -> Vector2 {
        Vector2::new(self.f(), self.f())
    }

    fn v3(&mut self) -> Vector3 {
        Vector3::new(self.f(), self.f(), self.f())
    }

    fn v4(&mut self) -> Vector4 {
        Vector4::new(self.f(), self.f(), self.f(), self.f())
    }

    fn m3(&mut self) -> Matrix3 {
        let mut m = [0.0; 9];
        m.iter_mut().for_each(|x| *x = self.f());
        Matrix3(m)
    }

    fn b(&mut self) -> bool {
        self.f() != 0.0
    }

    fn rest(&mut self) -> Vec<f64> {
        let r = self.0[self.1..].to_vec();
        self.1 = self.0.len();
        r
    }

    fn m4(&mut self) -> Matrix4 {
        let mut m = [0.0; 16];
        m.iter_mut().for_each(|x| *x = self.f());
        Matrix4(m)
    }
}

#[derive(Default)]
struct Out(Vec<f64>);

impl Out {
    fn f(&mut self, x: f64) -> &mut Self {
        self.0.push(x);
        self
    }

    fn b(&mut self, x: bool) -> &mut Self {
        self.f(if x { 1.0 } else { 0.0 })
    }

    fn v2(&mut self, v: &Vector2) -> &mut Self {
        self.f(v.x).f(v.y)
    }

    fn v3(&mut self, v: &Vector3) -> &mut Self {
        self.f(v.x).f(v.y).f(v.z)
    }

    fn m4(&mut self, m: &Matrix4) -> &mut Self {
        m.0.iter().for_each(|x| self.0.push(*x));
        self
    }

    fn m3(&mut self, m: &Matrix3) -> &mut Self {
        m.0.iter().for_each(|x| self.0.push(*x));
        self
    }

    fn v4(&mut self, v: &Vector4) -> &mut Self {
        self.f(v.x).f(v.y).f(v.z).f(v.w)
    }

    fn v3s(&mut self, vs: &[Vector3]) -> &mut Self {
        vs.iter().for_each(|v| {
            self.v3(v);
        });
        self
    }

    fn fs(&mut self, vs: &[f64]) -> &mut Self {
        self.0.extend_from_slice(vs);
        self
    }
}

fn bits(s: &Value) -> f64 {
    f64::from_bits(u64::from_str_radix(s.as_str().unwrap(), 16).unwrap())
}

fn run(op: &str, i: &mut In) -> Vec<f64> {
    let mut o = Out::default();

    match op {
        "line3.points" => {
            let (a, b, q) = (i.v3(), i.v3(), i.v3());
            let l = Line3::from_points(&a, &b);
            o.v3(&l.point).v3(&l.direction).v3(&l.get_closest_point_to_point(&q)).v3(&l.get_perpendicular_vector_to_point(&q));
        }
        "line3.line" => {
            let (a, b, c, d) = (i.v3(), i.v3(), i.v3(), i.v3());
            let (l, k) = (Line3::from_points(&a, &b), Line3::from_points(&c, &d));
            let mut p = Vector3::default();
            let hit = l.get_closest_point_to_line(&k, &mut p);
            o.b(hit).v3(&p).v3(&l.get_perpendicular_vector_to_line(&k));
        }
        "line3.triangle" => {
            let (a, b, ta, mut tb, mut tc) = (i.v3(), i.v3(), i.v3(), i.v3(), i.v3());
            let l = Line3::from_points(&a, &b);
            let mut uvt = Uvt { u: 0.0, v: 0.0, t: 0.0 };
            let hit = l.intersects_triangle(&ta, &mut tb, &mut tc, &mut uvt);
            o.b(hit).f(uvt.u).f(uvt.v).f(uvt.t).v3(&tb).v3(&tc);
        }
        "line3.matrix" => {
            let (a, b, m) = (i.v3(), i.v3(), i.m4());
            let l = Line3::from_points(&a, &b);
            let (mut l1, mut l2) = (l.copy(), l.copy());
            l1.mult_matrix_line(&m);
            l2.mult_line_matrix(&m);
            o.v3(&l1.point).v3(&l1.direction).v3(&l2.point).v3(&l2.direction);
        }
        "line2" => {
            let (a, b, q, c, d, m) = (i.v2(), i.v2(), i.v2(), i.v2(), i.v2(), i.m3());
            let (l, k) = (Line2::from_points(&a, &b), Line2::from_points(&c, &d));
            let mut p = Vector2::default();
            let hit = l.intersects_line(&k, &mut p);
            let (mut l1, mut l2) = (l.copy(), l.copy());
            l1.mult_matrix_line(&m);
            l2.mult_line_matrix(&m);
            o.v2(&l.point).v2(&l.direction).v2(&l.get_closest_point_to_point(&q)).v2(&l.get_perpendicular_vector_to_point(&q));
            o.b(hit).v2(&p).v2(&l1.point).v2(&l1.direction).v2(&l2.point).v2(&l2.direction);
        }
        "plane3" => {
            let (p, n, q, a, b, m) = (i.v3(), i.v3(), i.v3(), i.v3(), i.v3(), i.m4());
            let s = Plane3::new(p, n);
            let mut x = Vector3::default();
            let hit = s.intersects_line(&Line3::from_points(&a, &b), &mut x);
            let (mut s1, mut s2) = (s.copy(), s.copy());
            s1.mult_right(&m);
            s2.mult_left(&m);
            o.v3(&s.normal).f(s.distance_from_origin).f(s.get_distance_to_point(&q)).v3(&s.get_closest_point_to_point(&q));
            o.v3(&s.get_perpendicular_vector_to_point(&q)).b(hit).v3(&x).v3(&s1.normal).f(s1.distance_from_origin);
            o.v3(&s2.normal).f(s2.distance_from_origin);
        }
        "sphere3" => {
            let (radius, c, a, b) = (i.f(), i.v3(), i.v3(), i.v3());
            let (mut enter, mut exit) = (Vector3::default(), Vector3::default());
            let hit = Sphere3::new(radius, c).intersects_line(&Line3::from_points(&a, &b), &mut enter, &mut exit);
            o.b(hit).v3(&enter).v3(&exit);
        }
        "cylinder3" => {
            let (a, b, radius, c, d) = (i.v3(), i.v3(), i.f(), i.v3(), i.v3());
            let (mut enter, mut exit) = (Vector3::default(), Vector3::default());
            let hit = Cylinder3::new(Line3::from_points(&a, &b), radius).intersects_line(&Line3::from_points(&c, &d), &mut enter, &mut exit);
            o.b(hit).v3(&enter).v3(&exit);
        }
        "triangle3" => {
            let (a, b, c, d) = (i.v3(), i.v3(), i.v3(), i.v3());
            let (mut n, mut q) = (Vector3::default(), Vector3::default());
            Triangle3::normal(&a, &b, &c, &mut n);
            Triangle3::quad_normal(&a, &b, &c, &d, &mut q);
            o.f(Triangle3::area(&a, &b, &c)).v3(&n).v3(&q);
        }
        "viewvolume" => {
            let (mv, pm, vp, win, pt, a, b) = (i.m4(), i.m4(), i.v4(), i.v3(), i.v3(), i.v3(), i.v3());
            let (mut u, mut out) = (Vector3::default(), Vector3::default());
            let mut ray = Line3::new(Vector3::default(), Vector3::new(0.0, 0.0, 1.0));
            let mut l2 = Line2::new(Vector2::default(), Vector2::new(0.0, 1.0));
            ViewVolume::unproject_point(win.x, win.y, win.z, &mv, &pm, &vp, &mut u);
            ViewVolume::unproject_ray(win.x, win.y, &mv, &pm, &vp, &mut ray);
            ViewVolume::project_point(&pt, &mv, &pm, &vp, &mut out);
            ViewVolume::project_line(&Line3::from_points(&a, &b), &mv, &pm, &vp, &mut l2);
            o.v3(&u).v3(&ray.point).v3(&ray.direction).v3(&out).v2(&l2.point).v2(&l2.direction);
        }
        "camera.ortho" => {
            let (l, r, b, t, n, f) = (i.f(), i.f(), i.f(), i.f(), i.f(), i.f());
            let mut m = Matrix4([0.0; 16]);
            Camera::ortho(l, r, b, t, n, f, &mut m);
            o.m4(&m);
        }
        "box3" => {
            let (min, max, m) = (i.v3(), i.v3(), i.m4());
            let mut b = Box3::from_extents(&min, &max);
            b.mult_right(&m);
            let (lo, hi) = b.extents();
            o.v3(&lo).v3(&hi);
        }
        "box2" => {
            let (min, max, m, mb, p, size, center) = (i.v2(), i.v2(), i.m3(), i.m3(), i.v2(), i.v2(), i.v2());
            let (q1, q2, q3, omin, omax) = (i.v2(), i.v2(), i.v2(), i.v2(), i.v2());
            let mut b = Box2::from_extents(&min, &max);
            let c = Box2::new(&size, &center);
            o.m3(&c.matrix).b(b.is_empty()).b(Box2::default().is_empty()).b(b.contains_point(&p)).b(b.equals(&b.copy()));
            o.v2(&b.size()).v2(&b.center());
            b.mult_right(&m);
            let (lo, hi) = b.extents();
            o.v2(&lo).v2(&hi).b(b.contains_point(&p));
            b.mult_left(&mb);
            let (lo, hi) = b.extents();
            o.v2(&lo).v2(&hi).v2(&b.size());
            let ob = Box2::from_extents(&omin, &omax);
            b.add(&ob);
            let (lo, hi) = b.extents();
            o.v2(&lo).v2(&hi);
            let mut e = Box2::default();
            e.add(&ob);
            let (lo, hi) = e.extents();
            o.v2(&lo).v2(&hi);
            let (lo, hi) = Box2::from_points(&[q1, q2, q3]).extents();
            o.v2(&lo).v2(&hi).m3(&Box2::from_points(&[]).matrix);
        }
        "triangle2" => {
            let (a, b, c, p) = (i.v2(), i.v2(), i.v2(), i.v2());
            let bc = Triangle2::to_barycentric(&p, &a, &b, &c);
            o.f(Triangle2::area(&a, &b, &c)).b(Triangle2::is_point_in_triangle(&p, &a, &b, &c)).f(bc.u).f(bc.v).f(bc.t);
        }
        "spheroid3" => {
            let (a, c, f) = (i.f(), i.f(), i.b());
            let s = Spheroid3::new(a, c, f);
            o.f(s.semi_major_axis).f(s.semi_minor_axis).b(s.equals(&s));
        }
        "box3.more" => {
            let (size, center, m, min, max, mo, p) = (i.v3(), i.v3(), i.m4(), i.v3(), i.v3(), i.m4(), i.v3());
            let arr = i.rest();
            let mut a = Box3::default();
            a.set(Some((&size, &center)));
            let mut b = Box3::from_extents(&min, &max);
            b.mult_left(&mo);
            o.m4(&a.matrix).b(a.is_empty()).b(Box3::default().is_empty()).v3s(&a.points()).v3s(&a.axes()).v3s(&a.normals());
            o.b(a.contains_point(&p)).b(a.intersects_box(&b)).b(b.intersects_box(&a)).b(a.intersects_box(&Box3::default()));
            o.v3s(&b.points()).v3s(&b.normals());
            let mut c = a.clone();
            c.mult_left(&m);
            let (lo, hi) = c.extents();
            o.v3(&lo).v3(&hi);
            let mut d = a.clone();
            d.add(&b);
            let (lo, hi) = d.extents();
            o.v3(&lo).v3(&hi);
            let mut e = Box3::default();
            e.add(&b);
            o.m4(&e.matrix);
            let (lo, hi) = Box3::from_points(&[size, center, min, max, p]).extents();
            o.v3(&lo).v3(&hi);
            let (lo, hi) = Box3::from_array(&arr, 3).extents();
            o.v3(&lo).v3(&hi);
            let (lo, hi) = Box3::from_array(&arr, 4).extents();
            o.v3(&lo).v3(&hi);
            o.m4(&Box3::from_points(&[]).matrix).m4(&Box3::from_array(&[], 3).matrix).v3(&a.size());
            let mut om = Matrix4([0.0; 16]);
            Camera::ortho_box(&b, &mut om);
            o.m4(&om);
        }
        "camera" => {
            let (l, r, b, t, n, f, fov, w, h) = (i.f(), i.f(), i.f(), i.f(), i.f(), i.f(), i.f(), i.f(), i.f());
            let (mut m1, mut m2, mut m3) = (Matrix4([0.0; 16]), Matrix4([0.0; 16]), Matrix4([0.0; 16]));
            Camera::frustum(l, r, b, t, n, f, &mut m1);
            Camera::perspective(fov, n, f, w, h, &mut m2);
            Camera::perspective2(fov, n, f, &mut m3);
            o.m4(&m1).m4(&m2).m4(&m3);
        }
        "viewvolume.object" => {
            let (pm, vp, radius, center, size, bc, m) = (i.m4(), i.v4(), i.f(), i.v3(), i.v3(), i.v3(), i.m4());
            let mut v = ViewVolumeObject::new(&pm, &vp);
            let mut b = Box3::default();
            b.set(Some((&size, &bc)));
            b.mult_right(&m);
            o.v3s(&v.points).v3s(&v.normals);
            for p in &v.planes {
                o.v3(&p.normal).f(p.distance_from_origin);
            }
            let edges = v.edges();
            o.v3s(&edges).v4(&v.viewport().clone()).b(v.intersects_sphere(radius, &center)).b(v.intersects_box(&b));
            let mut c = Box3::default();
            c.set(Some((&size, &center)));
            o.b(v.intersects_box(&c));
        }
        "sat" => {
            let axes = [i.v3(), i.v3(), i.v3()];
            let p1 = [i.v3(), i.v3(), i.v3(), i.v3()];
            let p2 = [i.v3(), i.v3(), i.v3(), i.v3()];
            o.b(Sat::is_separated(&axes, &p1, &p2));
        }
        "bezier" => {
            let c: Vec<f64> = (0..12).map(|_| i.f()).collect();
            let steps = i.f() as i64;
            let (ax, ay, rx, ry, rot, large, sweep, x, y, st) = (i.f(), i.f(), i.f(), i.f(), i.f(), i.b(), i.b(), i.f(), i.f(), i.f());
            let (mut q, mut k, mut a) = (vec![], vec![], vec![]);
            Bezier::quadric(c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7], c[8], steps, &mut q);
            Bezier::cubic(c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7], c[8], c[9], c[10], c[11], steps, &mut k);
            Bezier::arc(ax, ay, rx, ry, rot, large, sweep, x, y, st, &mut a);
            o.f(q.len() as f64).v3s(&q).f(k.len() as f64).v3s(&k).f(a.len() as f64).v3s(&a);
        }
        "eigen" => {
            let order = i.f() as usize;
            let m = i.rest();
            let (values, vectors) = eigen_decomposition(&m, order);
            o.fs(&values);
            for row in &vectors {
                o.fs(row);
            }
        }
        "sort" => {
            let n = i.f() as usize;
            let keys: Vec<f64> = (0..n).map(|_| i.f()).collect();
            let (first, last) = (i.f() as usize, i.f() as usize);
            let items: Vec<(f64, usize)> = keys.iter().copied().zip(0..).collect();
            let cmp = |a: &(f64, usize), b: &(f64, usize)| a.0 < b.0;
            let num = |a: &f64, b: &f64| a < b;
            let mut m = items.clone();
            merge_sort(&mut m, first, last, &cmp);
            let mut q = items.clone();
            quick_sort(&mut q, first, last, &cmp);
            let mut mn = keys.clone();
            merge_sort(&mut mn, first, last, &num);
            let mut qn = keys.clone();
            quick_sort(&mut qn, first, last, &num);
            o.fs(&m.iter().map(|x| x.1 as f64).collect::<Vec<_>>()).fs(&q.iter().map(|x| x.1 as f64).collect::<Vec<_>>()).fs(&mn).fs(&qn);
        }
        _ => panic!("unknown op {op}"),
    }

    o.0
}

#[test]
fn geometry_matches_x_ite_bit_for_bit() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let baseline: Value = serde_json::from_str(&std::fs::read_to_string(root.join("oracle/geom-math-baseline.json")).unwrap()).unwrap();
    let cases = baseline["cases"].as_array().unwrap();
    let mut failures = vec![];

    for (n, case) in cases.iter().enumerate() {
        let op = case["op"].as_str().unwrap();
        let input: Vec<f64> = case["in"].as_array().unwrap().iter().map(bits).collect();
        let expected: Vec<f64> = case["out"].as_array().unwrap().iter().map(bits).collect();
        let mut i = In(input, 0);
        let got = run(op, &mut i);

        assert_eq!(i.1, i.0.len(), "case {n} {op}: inputs left over");

        let same = got.len() == expected.len() && got.iter().zip(&expected).all(|(a, b)| a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()));

        if !same {
            failures.push(format!("case {n} {op}:\n  x_ite {expected:?}\n  port  {got:?}"));
        }
    }

    // x3d-render's own Camera.perspective (traverse.rs, used for drawing) against X_ITE's, on the same cases.
    for case in cases.iter().filter(|c| c["op"] == "camera") {
        let input: Vec<f64> = case["in"].as_array().unwrap().iter().map(bits).collect();
        let expected: Vec<f64> = case["out"].as_array().unwrap().iter().map(bits).collect();
        let (n, f, fov, w, h) = (input[4], input[5], input[6], input[7], input[8]);
        let got = x3d_render::traverse::perspective(fov, n, f, w, h);
        let want = &expected[16..32];

        if !got.0.iter().zip(want).all(|(a, b)| a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())) {
            failures.push(format!("x3d-render perspective {input:?}:\n  x_ite {want:?}\n  port  {:?}", got.0));
        }
    }

    assert!(failures.is_empty(), "{} of {} cases differ:\n{}", failures.len(), cases.len(), failures.iter().take(20).cloned().collect::<Vec<_>>().join("\n"));
}
