//! Ledger entries node:SplinePositionInterpolator and node:SplinePositionInterpolator2D (DIFFERENT).
//!
//! Pinned X_ITE 16.5.0 throws `value.norm is not a function` when `normalizeVelocity` is TRUE and `keyVelocity` is not
//! empty, and the scene load fails. The port computes the normalized velocity (owner decision 2026-10-16).
//!
//! Owner decision 2026-11-12: the two position nodes follow ISO/IEC 19775-1:2023 19.2.3 and 19.2.4
//! (`hermite_generate_iso`). The rules, read in the adopted text (components/interpolators.md):
//! - keyVelocity of the size of keyValue gives every velocity; size 2 gives the first and the last; any other size is ignored.
//! - normalizeVelocity TRUE: T(i) = keyVelocity[i] × (Dtot / |keyVelocity[i]|), for each given velocity.
//! - closed is ignored when the first and last velocities are given, or when the first and last keyValue differ.
//!
//! SplineScalarInterpolator keeps X_ITE's CatmullRomSplineInterpolator.generate (`catmull_rom_generate`), which is compared
//! exactly in oracle/synthetic-rea/interp-ext.wrl. The last test guards that it did not change.

use x3d_fields::FieldValue;
use x3d_fields::NodeId;
use x3d_runtime::nodes::interp::{catmull_rom_generate, hermite_generate_iso};
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
        assert!(a.is_finite(), "{actual:?} is not finite");
        assert!((a - e).abs() < 1e-4, "{actual:?} != {expected:?}");
    }
}

fn close_all(actual: &[Vec<f64>], expected: &[Vec<f64>]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?} != {expected:?}");
    for (a, e) in actual.iter().zip(expected) {
        close(a, e);
    }
}

fn v3(a: &[[f64; 3]]) -> Vec<Vec<f64>> {
    a.iter().map(|v| v.to_vec()).collect()
}

/// Hermite basis of 19.2.4 on the segment that holds `fraction`, for key [0 0.5 1] (open, so F± = 1 at the ends and
/// F± = 1 at the middle key too: the intervals are equal).
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

const KEY: [f64; 3] = [0.0, 0.5, 1.0];

/// Dtot = |(3,0,0)| + |(0,4,0)| = 7.
fn value() -> Vec<Vec<f64>> {
    v3(&[[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [3.0, 4.0, 0.0]])
}

#[test]
fn iso_normalizes_every_given_velocity_the_last_included() {
    let velocity = v3(&[[1.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 5.0]]);
    let (t0, t1) = hermite_generate_iso(3, false, &KEY, &value(), &velocity, true);
    let t = v3(&[[7.0, 0.0, 0.0], [0.0, 7.0, 0.0], [0.0, 0.0, 7.0]]);

    close_all(&t0, &t);
    close_all(&t1, &t);

    // X_ITE's loop (`i < T.length - 1`) leaves the last one as given; the scalar node still uses it.
    let (_, t1) = catmull_rom_generate(3, false, &KEY, &value(), &velocity, true);
    close(&t1[2], &[0.0, 0.0, 5.0]);

    // FALSE: the velocities as given. A zero vector stays zero when normalized.
    let (t0, _) = hermite_generate_iso(3, false, &KEY, &value(), &velocity, false);
    close_all(&t0, &velocity);
    let zero = v3(&[[0.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 5.0]]);
    let (t0, _) = hermite_generate_iso(3, false, &KEY, &value(), &zero, true);
    close_all(
        &t0,
        &v3(&[[0.0, 0.0, 0.0], [0.0, 7.0, 0.0], [0.0, 0.0, 7.0]]),
    );

    // Equal key values: Dtot is 0, so each given velocity is scaled to zero, and every value is finite.
    let same = v3(&[[1.0, 1.0, 1.0]; 3]);
    let (t0, t1) = hermite_generate_iso(3, false, &KEY, &same, &velocity, true);
    close_all(&t0, &v3(&[[0.0; 3]; 3]));
    close_all(&t1, &v3(&[[0.0; 3]; 3]));
}

#[test]
fn iso_two_velocities_give_the_first_and_last_key() {
    // 19.2.4: "If the size of the keyVelocity field is 2, the first value is used as the velocity vector for the first key,
    // and the second value is used as the velocity for the last key." The middle key is calculated: (v2 - v0) / 2.
    let velocity = v3(&[[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
    let (t0, t1) = hermite_generate_iso(3, false, &KEY, &value(), &velocity, false);
    let t = v3(&[[1.0, 0.0, 0.0], [1.5, 2.0, 0.0], [0.0, 0.0, 1.0]]);

    close_all(&t0, &t);
    close_all(&t1, &t);

    let (t0, _) = hermite_generate_iso(3, false, &KEY, &value(), &velocity, true);
    close_all(
        &t0,
        &v3(&[[7.0, 0.0, 0.0], [1.5, 2.0, 0.0], [0.0, 0.0, 7.0]]),
    );
}

#[test]
fn iso_ignores_other_keyvelocity_sizes() {
    // "If the size of the keyVelocity field is anything other than those two values, the keyVelocity field is ignored."
    let (none0, none1) = hermite_generate_iso(3, false, &KEY, &value(), &[], true);
    let calculated = v3(&[[0.0, 0.0, 0.0], [1.5, 2.0, 0.0], [0.0, 0.0, 0.0]]);

    close_all(&none0, &calculated);
    close_all(&none1, &calculated);

    for velocity in [
        v3(&[[1.0, 0.0, 0.0]]),
        v3(&[
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
        ]),
    ] {
        let (t0, t1) = hermite_generate_iso(3, false, &KEY, &value(), &velocity, true);
        close_all(&t0, &none0);
        close_all(&t1, &none1);
    }
}

#[test]
fn iso_closed_is_ignored_for_unequal_ends_or_given_ends() {
    let key = [0.0, 0.25, 1.0];

    // Unequal first and last keyValue: closed is ignored, so the result is the open one.
    let (open0, open1) = hermite_generate_iso(3, false, &key, &value(), &[], false);
    let (t0, t1) = hermite_generate_iso(3, true, &key, &value(), &[], false);
    close_all(&t0, &open0);
    close_all(&t1, &open1);

    // Equal ends and no velocity: closed wraps, as X_ITE does (SP2 of interp-ext.wrl is exact).
    let ring = v3(&[[0.0, 0.0, 0.0], [2.0, 2.0, 0.0], [0.0, 0.0, 0.0]]);
    let iso = hermite_generate_iso(3, true, &key, &ring, &[], false);
    let xite = catmull_rom_generate(3, true, &key, &ring, &[], false);
    close_all(&iso.0, &xite.0);
    close_all(&iso.1, &xite.1);
    // Here N - 2 = 1, so T(0) = (v1 - v1) / 2 = 0, and the middle T(1) = (v2 - v0) / 2 = 0.
    close(&iso.0[0], &[0.0, 0.0, 0.0]);
    close(&iso.0[1], &[0.0, 0.0, 0.0]);

    // Equal ends with given first and last velocities: closed is ignored, F± is 1 at the ends.
    let velocity = v3(&[[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
    let (t0, t1) = hermite_generate_iso(3, true, &key, &ring, &velocity, false);
    close(&t0[0], &[1.0, 0.0, 0.0]);
    close(&t1[2], &[0.0, 0.0, 1.0]);
}

#[test]
fn iso_closed_seam_factors_on_unequal_intervals() {
    // 19.2.4 with the wrap t(-1) = t(N-2), t(N) = t(1): at both seam keys F- = 2a / (a + b) and F+ = 2b / (a + b), with
    // a = t1 - t0 = 0.2 and b = t(N-1) - t(N-2) = 0.5.
    let key = [0.0, 0.2, 0.5, 1.0];
    let ring = v3(&[
        [0.0, 0.0, 0.0],
        [1.0, 2.0, 0.0],
        [-2.0, 1.0, 1.0],
        [0.0, 0.0, 0.0],
    ]);
    let (t0, t1) = hermite_generate_iso(3, true, &key, &ring, &[], false);
    let (fm, fp) = (2.0 * 0.2 / 0.7, 2.0 * 0.5 / 0.7);
    // T at both seam keys = (v1 - v(N-2)) / 2 = (3, 1, -1) / 2.
    let t = [1.5, 0.5, -0.5];

    for i in [0, 3] {
        close(&t0[i], &t.map(|x| x * fp));
        close(&t1[i], &t.map(|x| x * fm));
    }

    let xite = catmull_rom_generate(3, true, &key, &ring, &[], false);
    close_all(&t0, &xite.0);
    close_all(&t1, &xite.1);
}

#[test]
fn iso_equals_xite_where_the_rules_agree() {
    // interp-ext.wrl SP1, S2 and S2C: no keyVelocity, or one of the size of keyValue without normalization.
    let key = [0.0, 0.25, 0.6, 1.0];
    let kv = v3(&[
        [0.0, 0.0, 0.0],
        [1.0, 2.0, 0.0],
        [3.0, -1.0, 1.0],
        [4.0, 0.0, 2.0],
    ]);
    let vel = v3(&[
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 1.0, 0.0],
    ]);

    for (velocity, closed) in [(vec![], false), (vel.clone(), false)] {
        let iso = hermite_generate_iso(3, closed, &key, &kv, &velocity, false);
        let xite = catmull_rom_generate(3, closed, &key, &kv, &velocity, false);
        close_all(&iso.0, &xite.0);
        close_all(&iso.1, &xite.1);
    }

    // S2C: closed TRUE, equal ends, three given velocities. X_ITE wraps F±; ISO ignores closed. For key [0 0.5 1] both
    // give F± = 1.
    let kv2: Vec<Vec<f64>> = [[0.0, 0.0], [1.0, 3.0], [0.0, 0.0]]
        .iter()
        .map(|v| v.to_vec())
        .collect();
    let vel2: Vec<Vec<f64>> = [[0.5, 0.5], [1.0, 0.0], [0.0, 1.0]]
        .iter()
        .map(|v| v.to_vec())
        .collect();
    let iso = hermite_generate_iso(2, true, &KEY, &kv2, &vel2, false);
    let xite = catmull_rom_generate(2, true, &KEY, &kv2, &vel2, false);
    close_all(&iso.0, &xite.0);
    close_all(&iso.1, &xite.1);
}

#[test]
fn position_scenes_use_the_iso_rules() {
    // cycleInterval 7: FROZEN_TIME / 7 has fraction 5/7, in the second segment.
    let rt = run("DEF T TimeSensor { cycleInterval 7 loop TRUE }
         DEF P SplinePositionInterpolator {
           key [ 0 0.5 1 ] keyValue [ 0 0 0, 3 0 0, 3 4 0 ]
           keyVelocity [ 1 0 0, 0 2 0, 0 0 5 ] normalizeVelocity TRUE }
         DEF P2 SplinePositionInterpolator2D {
           key [ 0 0.5 1 ] keyValue [ 0 0, 3 0, 3 4 ]
           keyVelocity [ 1 0, 0 2, 1 1 ] normalizeVelocity TRUE }
         DEF E SplinePositionInterpolator {
           key [ 0 0.5 1 ] keyValue [ 0 0 0, 3 0 0, 3 4 0 ] keyVelocity [ 1 0 0, 0 0 1 ] }
         DEF X Transform { }
         DEF XE Transform { }
         DEF TT TextureTransform { }
         ROUTE T.fraction_changed TO P.set_fraction
         ROUTE T.fraction_changed TO P2.set_fraction
         ROUTE T.fraction_changed TO E.set_fraction
         ROUTE P.value_changed TO X.set_translation
         ROUTE E.value_changed TO XE.set_translation
         ROUTE P2.value_changed TO TT.set_translation");

    let f = (FROZEN_TIME / 7.0) % 1.0;
    assert!(f > 0.5, "fraction {f}");
    // load_world sends the first frame: the routed fraction is that of FROZEN_TIME.
    let FieldValue::SFFloat(routed) = field(&rt, "T", "fraction_changed") else {
        panic!("fraction_changed")
    };
    assert!((routed - f).abs() < 1e-6, "{routed} != {f}");

    let t3 = v3(&[[7.0, 0.0, 0.0], [0.0, 7.0, 0.0], [0.0, 0.0, 7.0]]);
    let FieldValue::SFVec3f(p) = field(&rt, "X", "translation") else {
        panic!("translation")
    };
    close(&[p.x, p.y, p.z], &hermite(f, &value(), &t3));

    let v2: Vec<Vec<f64>> = [[0.0, 0.0], [3.0, 0.0], [3.0, 4.0]]
        .iter()
        .map(|v| v.to_vec())
        .collect();
    let s = 7.0 / 2f64.sqrt();
    let t2: Vec<Vec<f64>> = [[7.0, 0.0], [0.0, 7.0], [s, s]]
        .iter()
        .map(|v| v.to_vec())
        .collect();
    let FieldValue::SFVec2f(p) = field(&rt, "TT", "translation") else {
        panic!("translation")
    };
    close(&[p.x, p.y], &hermite(f, &v2, &t2));

    // Two velocities: the authored field keeps its two values (X_ITE resizes it to the key count with zeros), and the
    // middle key is calculated.
    let FieldValue::MFVec3f(kvel) = field(&rt, "E", "keyVelocity") else {
        panic!("keyVelocity")
    };
    assert_eq!(kvel.len(), 6, "{kvel:?}");
    let te = v3(&[[1.0, 0.0, 0.0], [1.5, 2.0, 0.0], [0.0, 0.0, 1.0]]);
    let FieldValue::SFVec3f(p) = field(&rt, "XE", "translation") else {
        panic!("translation")
    };
    close(&[p.x, p.y, p.z], &hermite(f, &value(), &te));
}

#[test]
fn scalar_node_keeps_the_xite_rules() {
    // interp-ext.wrl SS2 shape: a short keyVelocity is resized with zeros, and the last velocity is not normalized.
    let rt = run("DEF T TimeSensor { cycleInterval 7 loop TRUE }
         DEF S SplineScalarInterpolator { key [ 0 0.5 1 ] keyValue [ 1 5 9 ] keyVelocity [ 2 3 ] normalizeVelocity TRUE }
         ROUTE T.fraction_changed TO S.set_fraction");

    let FieldValue::MFFloat(kvel) = field(&rt, "S", "keyVelocity") else {
        panic!("keyVelocity")
    };
    assert_eq!(kvel.len(), 3, "{kvel:?}");

    // T = [2, 3, 0] → normalized [8, 8, 0] by X_ITE's loop (Dtot = 8); 19.2.4 would give 8 at both ends and a calculated middle.
    let f = (FROZEN_TIME / 7.0) % 1.0;
    let v = vec![vec![1.0], vec![5.0], vec![9.0]];
    let t = vec![vec![8.0], vec![8.0], vec![0.0]];
    let FieldValue::SFFloat(out) = field(&rt, "S", "value_changed") else {
        panic!("value_changed")
    };
    close(&[out], &hermite(f, &v, &t));
}
