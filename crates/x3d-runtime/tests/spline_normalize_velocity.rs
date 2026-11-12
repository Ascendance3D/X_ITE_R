//! Ledger entries node:SplinePositionInterpolator and node:SplinePositionInterpolator2D (DIFFERENT, owner decision
//! 2026-10-16 "keep the math"). Pinned X_ITE 16.5.0 throws `value.norm is not a function` when `normalizeVelocity`
//! is TRUE and `keyVelocity` is not empty, and the scene load fails. The port computes the normalized velocity.
//! These tests pin that path, which had no test before (oracle/synthetic-rea/interp-ext.wrl normalizes only a
//! SplineScalarInterpolator).
//!
//! ISO/IEC 19775-1:2023 19.2.4: T(i) = keyVelocity[i] × (Dtot / |keyVelocity[i]|), with Dtot the sum of
//! |v(i) - v(i+1)|. The port uses the loop of CatmullRomSplineInterpolator.generate (`i < T.length - 1`), which leaves
//! the last velocity as given. The SplineScalarInterpolator path of X_ITE does the same and is compared exactly.
//! The last-key assertions record that loop. They change only by an owner decision.

use x3d_fields::FieldValue;
use x3d_fields::NodeId;
use x3d_runtime::nodes::interp::catmull_rom_generate;
use x3d_runtime::{FROZEN_TIME, Fetcher, Runtime};

struct NoFetch;

impl Fetcher for NoFetch {
    fn fetch(&self, _: &url::Url) -> Result<Vec<u8>, String> {
        Err("TypeError: Failed to fetch".into())
    }
}

fn run(body: &str) -> Runtime {
    let text = format!("#X3D V3.3 utf8\nPROFILE Full\n{body}");
    let scene = x3d_parse::parse_bytes(text.as_bytes(), "http://oracle/t.x3dv").unwrap();
    let mut rt = Runtime::new(scene, Box::new(NoFetch));

    rt.load_world();
    rt
}

fn node(rt: &Runtime, name: &str) -> NodeId {
    rt.scene.get_named_node(x3d_scene::ROOT, name).unwrap()
}

fn field(rt: &Runtime, name: &str, field: &str) -> FieldValue {
    let n = node(rt, name);
    let loc = rt.scene.get_field(n, field).unwrap();
    rt.value((n, loc)).clone()
}

fn close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?} != {expected:?}");
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < 1e-4, "{actual:?} != {expected:?}");
    }
}

/// Hermite basis of 19.2.4 on the segment that holds `fraction` (open spline, key [0 0.5 1], all F± equal to 1).
fn hermite(fraction: f64, v: &[Vec<f64>], t: &[Vec<f64>]) -> Vec<f64> {
    let i = if fraction < 0.5 { 0 } else { 1 };
    let s = (fraction - 0.5 * i as f64) / 0.5;
    let (s2, s3) = (s * s, s * s * s);
    let h = [
        2.0 * s3 - 3.0 * s2 + 1.0,
        -2.0 * s3 + 3.0 * s2,
        s3 - 2.0 * s2 + s,
        s3 - s2,
    ];

    (0..v[0].len())
        .map(|c| h[0] * v[i][c] + h[1] * v[i + 1][c] + h[2] * t[i][c] + h[3] * t[i + 1][c])
        .collect()
}

#[test]
fn generate_normalizes_every_velocity_but_the_last() {
    // Dtot = |(3,0,0)| + |(0,4,0)| = 7.
    let key = [0.0, 0.5, 1.0];
    let value = vec![
        vec![0.0, 0.0, 0.0],
        vec![3.0, 0.0, 0.0],
        vec![3.0, 4.0, 0.0],
    ];
    let velocity = vec![
        vec![1.0, 0.0, 0.0],
        vec![0.0, 2.0, 0.0],
        vec![0.0, 0.0, 5.0],
    ];
    let (t0, t1) = catmull_rom_generate(3, false, &key, &value, &velocity, true);

    close(&t0[0], &[7.0, 0.0, 0.0]);
    close(&t0[1], &[0.0, 7.0, 0.0]);
    close(&t1[1], &[0.0, 7.0, 0.0]);
    // X_ITE's loop: the last velocity keeps its magnitude (19.2.4 would give 0 0 7).
    close(&t1[2], &[0.0, 0.0, 5.0]);

    // A zero velocity stays zero; FALSE uses keyVelocity as given (19.2.4).
    let zero = vec![
        vec![0.0, 0.0, 0.0],
        vec![0.0, 2.0, 0.0],
        vec![0.0, 0.0, 5.0],
    ];
    let (t0, _) = catmull_rom_generate(3, false, &key, &value, &zero, true);
    close(&t0[0], &[0.0, 0.0, 0.0]);
    let (t0, _) = catmull_rom_generate(3, false, &key, &value, &velocity, false);
    close(&t0[1], &[0.0, 2.0, 0.0]);

    // 2D, and a zero Dtot (all key values equal) scales each velocity to zero.
    let value2 = vec![vec![0.0, 0.0], vec![3.0, 0.0], vec![3.0, 4.0]];
    let velocity2 = vec![vec![1.0, 0.0], vec![0.0, 2.0], vec![1.0, 1.0]];
    let (t0, t1) = catmull_rom_generate(2, false, &key, &value2, &velocity2, true);
    close(&t0[0], &[7.0, 0.0]);
    close(&t0[1], &[0.0, 7.0]);
    close(&t1[2], &[1.0, 1.0]);
    let same = vec![vec![1.0, 1.0]; 3];
    let (t0, _) = catmull_rom_generate(2, false, &key, &same, &velocity2, true);
    close(&t0[1], &[0.0, 0.0]);
}

#[test]
fn normalized_velocity_scenes_load_and_interpolate() {
    // cycleInterval 7: FROZEN_TIME / 7 has fraction 5/7, in the second segment.
    let rt = run("DEF T TimeSensor { cycleInterval 7 loop TRUE }
         DEF P SplinePositionInterpolator {
           key [ 0 0.5 1 ] keyValue [ 0 0 0, 3 0 0, 3 4 0 ]
           keyVelocity [ 1 0 0, 0 2 0, 0 0 5 ] normalizeVelocity TRUE }
         DEF P2 SplinePositionInterpolator2D {
           key [ 0 0.5 1 ] keyValue [ 0 0, 3 0, 3 4 ]
           keyVelocity [ 1 0, 0 2, 1 1 ] normalizeVelocity TRUE }
         DEF X Transform { }
         DEF TT TextureTransform { }
         ROUTE T.fraction_changed TO P.set_fraction
         ROUTE T.fraction_changed TO P2.set_fraction
         ROUTE P.value_changed TO X.set_translation
         ROUTE P2.value_changed TO TT.set_translation");

    let f = (FROZEN_TIME / 7.0) % 1.0;
    assert!(f > 0.5, "fraction {f}");

    let v3 = [
        vec![0.0, 0.0, 0.0],
        vec![3.0, 0.0, 0.0],
        vec![3.0, 4.0, 0.0],
    ];
    let t3 = [
        vec![7.0, 0.0, 0.0],
        vec![0.0, 7.0, 0.0],
        vec![0.0, 0.0, 5.0],
    ];
    let FieldValue::SFVec3f(p) = field(&rt, "X", "translation") else {
        panic!("translation")
    };
    close(&[p.x, p.y, p.z], &hermite(f, &v3, &t3));

    let v2 = [vec![0.0, 0.0], vec![3.0, 0.0], vec![3.0, 4.0]];
    let t2 = [vec![7.0, 0.0], vec![0.0, 7.0], vec![1.0, 1.0]];
    let FieldValue::SFVec2f(p) = field(&rt, "TT", "translation") else {
        panic!("translation")
    };
    close(&[p.x, p.y], &hermite(f, &v2, &t2));
}
