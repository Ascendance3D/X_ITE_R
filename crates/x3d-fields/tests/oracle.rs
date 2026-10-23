//! Number formatting and math checked against values computed by the pinned X_ITE modules
//! (oracle/x_ite-fields.mjs → oracle/x_ite-fields.json). Read by path; nothing is copied.

use serde_json::Value;
use x3d_fields::format::{double_format, float_format};
use x3d_fields::math::*;

fn oracle() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../oracle/x_ite-fields.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("run oracle/x_ite-fields.mjs first"))
        .unwrap()
}

fn num(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap(),
        Value::String(s) => match s.as_str() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            "-0" => -0.0,
            _ => panic!("bad number {s}"),
        },
        _ => panic!("not a number: {v}"),
    }
}

fn nums(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(num).collect()
}

/// Bit-exact equality (NaN == NaN, 0 != -0).
fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

/// For transcendental functions (sin/cos/acos/log/exp) allow 2 ulp: V8 uses fdlibm ports, the
/// libm crate uses musl's; they agree almost always but are not guaranteed bit-identical.
fn close(a: f64, b: f64) -> bool {
    if same(a, b) {
        return true;
    }
    let (ia, ib) = (a.to_bits() as i64, b.to_bits() as i64);
    (ia - ib).abs() <= 2 || (a - b).abs() < 1e-15
}

fn check(group: &str, name: &str, expected: &Value, actual: &[f64], exact: bool) {
    let e = match expected {
        Value::Array(_) => nums(expected),
        _ => vec![num(expected)],
    };

    assert_eq!(e.len(), actual.len(), "{group}.{name}: length");

    for (i, (x, y)) in e.iter().zip(actual).enumerate() {
        let ok = if exact { same(*x, *y) } else { close(*x, *y) };
        assert!(
            ok,
            "{group}.{name}[{i}]: x_ite {x:?} ({:#x}) rust {y:?} ({:#x})",
            x.to_bits(),
            y.to_bits()
        );
    }
}

#[test]
fn float_and_double_format_match_x_ite() {
    let o = oracle();
    let mut failures = vec![];
    let cases = o["format"].as_array().unwrap();

    for case in cases {
        let v = num(&case[0]);
        let (f, d) = (float_format(v), double_format(v));

        if f != case[1].as_str().unwrap() || d != case[2].as_str().unwrap() {
            failures.push(format!(
                "{v:?}: float {f} vs {}, double {d} vs {}",
                case[1], case[2]
            ));
        }
    }

    assert!(cases.len() > 3000);
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

#[test]
fn vector_math_matches_x_ite() {
    let o = oracle();
    let m = &o["math"]["vector3"];
    let a = Vector3::new(1.5, -2.25, 3.125);
    let b = Vector3::new(-0.5, 4.0, 0.75);
    let mut nb = b;
    nb.normalize();

    let cases: Vec<(&str, Vec<f64>)> = vec![
        ("add", a.clone().add(&b).to_array().to_vec()),
        ("subtract", a.clone().subtract(&b).to_array().to_vec()),
        ("multiply", a.clone().multiply(1.7).to_array().to_vec()),
        ("divide", a.clone().divide(3.0).to_array().to_vec()),
        ("multVec", a.clone().mult_vec(&b).to_array().to_vec()),
        ("divVec", a.clone().div_vec(&b).to_array().to_vec()),
        ("cross", a.clone().cross(&b).to_array().to_vec()),
        ("dot", vec![a.dot(&b)]),
        ("norm", vec![a.norm()]),
        ("squaredNorm", vec![a.squared_norm()]),
        ("normalize", a.clone().normalize().to_array().to_vec()),
        ("distance", vec![a.distance(&b)]),
        ("lerp", a.clone().lerp(&b, 0.3).to_array().to_vec()),
        ("negate", a.clone().negate().to_array().to_vec()),
        ("inverse", a.clone().inverse().to_array().to_vec()),
        ("reflect", a.clone().reflect(&nb).to_array().to_vec()),
        ("min", a.clone().min(&b).to_array().to_vec()),
        ("max", a.clone().max(&b).to_array().to_vec()),
        ("abs", a.clone().abs().to_array().to_vec()),
        (
            "hypot3",
            vec![x3d_fields::jsmath::hypot(&[1e200, 1e200, 3.0])],
        ),
        (
            "hypotTiny",
            vec![x3d_fields::jsmath::hypot(&[3e-200, 4e-200, 0.0])],
        ),
    ];

    for (name, actual) in cases {
        check("vector3", name, &m[name], &actual, true);
    }

    let m = &o["math"]["vector2"];
    let a = Vector2::new(0.3, -0.7);
    let b = Vector2::new(2.0, 5.0);
    let mut nb = b;
    nb.normalize();

    check(
        "vector2",
        "add",
        &m["add"],
        &a.clone().add(&b).to_array(),
        true,
    );
    check("vector2", "dot", &m["dot"], &[a.dot(&b)], true);
    check("vector2", "norm", &m["norm"], &[a.norm()], true);
    check(
        "vector2",
        "normalize",
        &m["normalize"],
        &a.clone().normalize().to_array(),
        true,
    );
    check(
        "vector2",
        "lerp",
        &m["lerp"],
        &a.clone().lerp(&b, 0.25).to_array(),
        true,
    );
    check(
        "vector2",
        "distance",
        &m["distance"],
        &[a.distance(&b)],
        true,
    );
    check(
        "vector2",
        "reflect",
        &m["reflect"],
        &a.clone().reflect(&nb).to_array(),
        true,
    );

    let m = &o["math"]["vector4"];
    let a = Vector4::new(1.0, 2.0, 3.0, 4.0);
    let b = Vector4::new(-1.0, 0.5, 0.25, 2.0);

    check(
        "vector4",
        "add",
        &m["add"],
        &a.clone().add(&b).to_array(),
        true,
    );
    check("vector4", "dot", &m["dot"], &[a.dot(&b)], true);
    check("vector4", "norm", &m["norm"], &[a.norm()], true);
    check(
        "vector4",
        "normalize",
        &m["normalize"],
        &a.clone().normalize().to_array(),
        true,
    );
    check(
        "vector4",
        "lerp",
        &m["lerp"],
        &a.clone().lerp(&b, 0.6).to_array(),
        true,
    );
    check(
        "vector4",
        "distance",
        &m["distance"],
        &[a.distance(&b)],
        true,
    );
}

fn q(q: &Quaternion) -> Vec<f64> {
    vec![q.x, q.y, q.z, q.w]
}

#[test]
fn rotation_and_quaternion_math_match_x_ite() {
    let o = oracle();
    let m = &o["math"]["rotation4"];
    let r1 = Rotation4::new(0.0, 1.0, 0.0, 1.2);
    let r2 = Rotation4::new(1.0, 1.0, 0.0, 0.7);
    let v = Vector3::new(1.0, 2.0, 3.0);

    check(
        "rotation4",
        "quaternion1",
        &m["quaternion1"],
        &q(&r1.quaternion()),
        false,
    );
    check(
        "rotation4",
        "quaternion2",
        &m["quaternion2"],
        &q(&r2.quaternion()),
        false,
    );
    check("rotation4", "raw2", &m["raw2"], &r2.raw(), true);
    check(
        "rotation4",
        "multLeft",
        &m["multLeft"],
        &r1.clone().mult_left(&r2).components(),
        false,
    );
    check(
        "rotation4",
        "multRight",
        &m["multRight"],
        &r1.clone().mult_right(&r2).components(),
        false,
    );
    check(
        "rotation4",
        "inverse",
        &m["inverse"],
        &r2.clone().inverse().components(),
        false,
    );
    check(
        "rotation4",
        "matrix",
        &m["matrix"],
        &r2.get_matrix().0,
        false,
    );

    let mut w = v;
    r2.mult_vec_rot(&mut w);
    check(
        "rotation4",
        "multVecRot",
        &m["multVecRot"],
        &w.to_array(),
        false,
    );

    let mut w = v;
    r2.mult_rot_vec(&mut w);
    check(
        "rotation4",
        "multRotVec",
        &m["multRotVec"],
        &w.to_array(),
        false,
    );

    check(
        "rotation4",
        "slerp",
        &m["slerp"],
        &r1.clone().slerp(&r2, 0.35).components(),
        false,
    );
    check(
        "rotation4",
        "setVectors",
        &m["setVectors"],
        &Rotation4::identity()
            .set_vectors(&Vector3::new(1.0, 0.0, 0.0), &Vector3::new(0.0, 1.0, 1.0))
            .components(),
        false,
    );
    check(
        "rotation4",
        "setVectorsOpposite",
        &m["setVectorsOpposite"],
        &Rotation4::identity()
            .set_vectors(&Vector3::new(1.0, 0.0, 0.0), &Vector3::new(-1.0, 0.0, 0.0))
            .components(),
        false,
    );
    check(
        "rotation4",
        "fromMatrix",
        &m["fromMatrix"],
        &Rotation4::identity()
            .set_matrix(&r2.get_matrix())
            .components(),
        false,
    );
    check(
        "rotation4",
        "bigAngle",
        &m["bigAngle"],
        &q(&Rotation4::new(0.0, 0.0, 1.0, 7.0).quaternion()),
        false,
    );
    check(
        "rotation4",
        "euler",
        &m["euler"],
        &Rotation4::identity().set_euler(0.1, 0.2, 0.3).components(),
        false,
    );
    assert_eq!(
        m["identityEquals"].as_bool().unwrap(),
        Rotation4::new(0.0, 1.0, 0.0, 0.0).equals(&Rotation4::identity())
    );
    assert_eq!(
        m["twoPiEquals"].as_bool().unwrap(),
        Rotation4::new(0.0, 1.0, 0.0, 2.0 * std::f64::consts::PI).equals(&Rotation4::identity())
    );

    let m = &o["math"]["quaternion"];
    let q1 = Quaternion::new(0.1, 0.2, 0.3, 0.9);
    let q2 = Quaternion::new(-0.3, 0.1, 0.5, 0.8);
    let mut n1 = q1;
    n1.normalize();
    let mut n2 = q2;
    n2.normalize();

    check(
        "quaternion",
        "multLeft",
        &m["multLeft"],
        &q(q1.clone().mult_left(&q2)),
        true,
    );
    check(
        "quaternion",
        "multRight",
        &m["multRight"],
        &q(q1.clone().mult_right(&q2)),
        true,
    );
    check(
        "quaternion",
        "inverse",
        &m["inverse"],
        &q(q1.clone().inverse()),
        true,
    );
    check("quaternion", "normalize", &m["normalize"], &q(&n1), true);
    check("quaternion", "norm", &m["norm"], &[q1.norm()], true);
    check(
        "quaternion",
        "matrix",
        &m["matrix"],
        &n1.get_matrix3().0,
        true,
    );
    check(
        "quaternion",
        "slerp",
        &m["slerp"],
        &q(n1.clone().slerp(&n2, 0.4)),
        false,
    );
    check("quaternion", "log", &m["log"], &q(n1.clone().log()), false);
    check("quaternion", "exp", &m["exp"], &q(q1.clone().exp()), false);
    check(
        "quaternion",
        "setMatrix",
        &m["setMatrix"],
        &q(Quaternion::IDENTITY
            .clone()
            .set_matrix(&Matrix3([0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0]))),
        true,
    );
}

#[test]
fn matrix_math_matches_x_ite() {
    let o = oracle();
    let m = &o["math"]["matrix3"];
    let a = Matrix3([2.0, 0.5, 0.0, -1.0, 3.0, 0.25, 4.0, 5.0, 1.0]);
    let b = Matrix3([1.0, 2.0, 3.0, 0.0, 1.0, 4.0, 5.0, 6.0, 0.0]);

    check(
        "matrix3",
        "determinant",
        &m["determinant"],
        &[a.determinant()],
        true,
    );
    check(
        "matrix3",
        "inverse",
        &m["inverse"],
        &a.clone().inverse().0,
        true,
    );
    check(
        "matrix3",
        "transpose",
        &m["transpose"],
        &a.clone().transpose().0,
        true,
    );
    check(
        "matrix3",
        "multLeft",
        &m["multLeft"],
        &a.clone().mult_left(&b).0,
        true,
    );
    check(
        "matrix3",
        "multRight",
        &m["multRight"],
        &a.clone().mult_right(&b).0,
        true,
    );

    let mut v2 = Vector2::new(1.0, 2.0);
    a.mult_vec_matrix2(&mut v2);
    check(
        "matrix3",
        "multVecMatrix2",
        &m["multVecMatrix2"],
        &v2.to_array(),
        true,
    );

    let mut v3 = Vector3::new(1.0, 2.0, 3.0);
    a.mult_vec_matrix3(&mut v3);
    check(
        "matrix3",
        "multVecMatrix3",
        &m["multVecMatrix3"],
        &v3.to_array(),
        true,
    );

    let mut v3 = Vector3::new(1.0, 2.0, 3.0);
    a.mult_matrix_vec3(&mut v3);
    check(
        "matrix3",
        "multMatrixVec3",
        &m["multMatrixVec3"],
        &v3.to_array(),
        true,
    );

    check(
        "matrix3",
        "rotate",
        &m["rotate"],
        &a.clone().rotate(0.4).0,
        false,
    );
    check(
        "matrix3",
        "scale",
        &m["scale"],
        &a.clone().scale(&Vector2::new(2.0, 3.0)).0,
        true,
    );
    check(
        "matrix3",
        "translate",
        &m["translate"],
        &a.clone().translate(&Vector2::new(5.0, -1.0)).0,
        true,
    );
    check(
        "matrix3",
        "singularInverse",
        &m["singularInverse"],
        &Matrix3([1.0, 2.0, 3.0, 2.0, 4.0, 6.0, 0.0, 0.0, 0.0])
            .inverse()
            .0,
        true,
    );

    let m = &o["math"]["matrix4"];
    let a = Matrix4([
        2.0, 0.5, 0.0, 0.1, -1.0, 3.0, 0.25, 0.0, 4.0, 5.0, 1.0, 0.2, 1.0, 2.0, 3.0, 1.0,
    ]);
    let b = Matrix4([
        1.0, 0.0, 2.0, 0.0, 0.0, 1.0, 0.0, 3.0, 4.0, 0.0, 1.0, 0.0, 0.0, 5.0, 0.0, 1.0,
    ]);

    check(
        "matrix4",
        "determinant",
        &m["determinant"],
        &[a.determinant()],
        true,
    );
    check(
        "matrix4",
        "inverse",
        &m["inverse"],
        &a.clone().inverse().0,
        true,
    );
    check(
        "matrix4",
        "transpose",
        &m["transpose"],
        &a.clone().transpose().0,
        true,
    );
    check(
        "matrix4",
        "multLeft",
        &m["multLeft"],
        &a.clone().mult_left(&b).0,
        true,
    );
    check(
        "matrix4",
        "multRight",
        &m["multRight"],
        &a.clone().mult_right(&b).0,
        true,
    );

    let mut v = Vector3::new(1.0, 2.0, 3.0);
    a.mult_vec_matrix3(&mut v);
    check(
        "matrix4",
        "multVecMatrix",
        &m["multVecMatrix"],
        &v.to_array(),
        true,
    );

    let mut v = Vector3::new(1.0, 2.0, 3.0);
    a.mult_matrix_vec3(&mut v);
    check(
        "matrix4",
        "multMatrixVec",
        &m["multMatrixVec"],
        &v.to_array(),
        true,
    );

    let mut v = Vector3::new(1.0, 2.0, 3.0);
    a.mult_dir_matrix(&mut v);
    check(
        "matrix4",
        "multDirMatrix",
        &m["multDirMatrix"],
        &v.to_array(),
        true,
    );

    let mut v = Vector3::new(1.0, 2.0, 3.0);
    a.mult_matrix_dir(&mut v);
    check(
        "matrix4",
        "multMatrixDir",
        &m["multMatrixDir"],
        &v.to_array(),
        true,
    );

    let mut v = Vector4::new(1.0, 2.0, 3.0, 4.0);
    a.mult_vec_matrix4(&mut v);
    check(
        "matrix4",
        "multVecMatrix4",
        &m["multVecMatrix4"],
        &v.to_array(),
        true,
    );

    let t = Matrix4::IDENTITY
        .clone()
        .set_transform(
            Some(&Vector3::new(1.0, 2.0, 3.0)),
            Some(&Rotation4::new(0.0, 1.0, 0.0, 0.5)),
            Some(&Vector3::new(2.0, 2.0, 3.0)),
            Some(&Rotation4::new(1.0, 0.0, 0.0, 0.3)),
            Some(&Vector3::new(0.5, 0.0, -1.0)),
        )
        .0;
    check("matrix4", "setTransform", &m["setTransform"], &t, false);

    let t = Matrix4::IDENTITY
        .clone()
        .translate(&Vector3::new(1.0, 0.0, 0.0))
        .rotate(&Rotation4::new(0.0, 0.0, 1.0, 1.0))
        .scale(&Vector3::new(2.0, 3.0, 4.0))
        .0;
    check(
        "matrix4",
        "translateRotateScale",
        &m["translateRotateScale"],
        &t,
        false,
    );
}

#[test]
fn color_math_matches_x_ite() {
    let o = oracle();
    let m = &o["math"]["color"];
    let c = Color3::new(0.2, 0.6, 0.4);
    let d = Color4::new(0.9, 0.1, 0.3, 0.5);

    check(
        "color",
        "clamp3",
        &m["clamp3"],
        &Color3::new(1.5, -0.5, 0.5).components(),
        true,
    );
    check(
        "color",
        "clamp4",
        &m["clamp4"],
        &Color4::new(2.0, -1.0, 0.5, 3.0).components(),
        true,
    );
    check("color", "hsv", &m["hsv"], &c.get_hsv(), true);

    let [h, s, v] = c.get_hsv();
    check(
        "color",
        "fromHsv",
        &m["fromHsv"],
        &Color3::BLACK.clone().set_hsv(h, s, v).components(),
        true,
    );
    check("color", "hsva", &m["hsva"], &d.get_hsva(), true);

    let [h, s, v, a] = d.get_hsva();
    check(
        "color",
        "fromHsva",
        &m["fromHsva"],
        &Color4::default().clone().set_hsva(h, s, v, a).components(),
        true,
    );
    check(
        "color",
        "hsvGrey",
        &m["hsvGrey"],
        &Color3::new(0.5, 0.5, 0.5).get_hsv(),
        true,
    );
    check(
        "color",
        "fromHsvWrap",
        &m["fromHsvWrap"],
        &Color3::BLACK.clone().set_hsv(7.5, 0.5, 0.8).components(),
        true,
    );
}

#[test]
fn js_math_functions_match_v8() {
    use x3d_fields::jsmath;

    let o = oracle();
    let m = &o["jsmath"];
    let inputs = nums(&m["inputs"]);
    let unit = nums(&m["unit"]);
    let mut report = vec![];
    let mut exact_failures = 0;

    let mut run = |name: &str, f: &dyn Fn(usize) -> f64, must_be_exact: bool| {
        let expected = nums(&m[name]);
        let differ = expected
            .iter()
            .enumerate()
            .filter(|(i, e)| !same(**e, f(*i)))
            .count();
        let near = expected
            .iter()
            .enumerate()
            .filter(|(i, e)| !close(**e, f(*i)))
            .count();

        report.push(format!(
            "{name}: {differ}/{} not bit-exact, {near} beyond 2 ulp",
            expected.len()
        ));

        if must_be_exact && differ > 0 {
            exact_failures += 1;
        }

        assert_eq!(near, 0, "{name}: {near} results more than 2 ulp from V8");
    };

    // fdlibm ports: must be bit-exact with V8.
    run("sin", &|i| jsmath::sin(inputs[i]), true);
    run("cos", &|i| jsmath::cos(inputs[i]), true);
    run("exp", &|i| jsmath::exp(inputs[i] / 100.0), true);
    run("log", &|i| jsmath::log(inputs[i].abs()), true);
    run("log10", &|i| jsmath::log10(inputs[i].abs()), true);
    run("acos", &|i| jsmath::acos(unit[i]), true);
    run(
        "hypot",
        &|i| jsmath::hypot(&[inputs[i], unit[i], 0.5]),
        true,
    );
    // libm crate: reported, only required to be within 2 ulp.
    run("tan", &|i| jsmath::tan(inputs[i]), false);
    run("asin", &|i| jsmath::asin(unit[i]), false);
    run("atan2", &|i| jsmath::atan2(inputs[i], unit[i]), false);

    println!("{}", report.join("\n"));
    assert_eq!(exact_failures, 0, "{}", report.join("\n"));
}
