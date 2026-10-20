//! sub:fields.math: the port's geometry (src/geom.rs, Box3 in src/lookat.rs) against the pinned X_ITE's standard/Math/Geometry
//! classes, bit for bit. oracle/geom-math-baseline.json is written by oracle/tools/geom_xite.mjs, which runs X_ITE's own modules
//! (three runs identical); every number is the hex of its IEEE 754 double, inputs included, so both sides start from the same
//! bits. No tolerance: an output equals X_ITE's when its bits do (any NaN equals any NaN).

use serde_json::Value;
use x3d_fields::math::{Matrix3, Matrix4, Vector2, Vector3, Vector4};
use x3d_viewer::geom::{Camera, Cylinder3, Line2, Line3, Plane3, Sphere3, Triangle3, Uvt, ViewVolume};
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

    assert!(failures.is_empty(), "{} of {} cases differ:\n{}", failures.len(), cases.len(), failures.iter().take(20).cloned().collect::<Vec<_>>().join("\n"));
}
