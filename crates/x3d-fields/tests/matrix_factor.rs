//! Matrix4.factor / get_transform round-trip tests.
//!
//! Ports the contract of X_ITE Matrix4.js `factor` / `getTransform`:
//! `setTransform` then `factor` then `setTransform` must rebuild the same
//! matrix. Fixed cases only, no randomness.

use x3d_fields::math::{Matrix4, Rotation4, Vector3};

const TOL: f64 = 1e-9;

fn assert_close(a: &Matrix4, b: &Matrix4) {
    for i in 0..16 {
        let (x, y) = (a.0[i], b.0[i]);
        let tol = TOL * (1.0 + x.abs().max(y.abs()));
        assert!((x - y).abs() <= tol, "element {i}: {x} != {y}",);
    }
}

fn round_trip(
    translation: Vector3,
    rotation: Rotation4,
    scale: Vector3,
    scale_orientation: Rotation4,
    center: Option<Vector3>,
) {
    let mut m = Matrix4::IDENTITY;
    m.set_transform(
        Some(&translation),
        Some(&rotation),
        Some(&scale),
        Some(&scale_orientation),
        center.as_ref(),
    );

    let (mut t, mut r, mut s, mut so) = (
        Vector3::ZERO,
        Rotation4::identity(),
        Vector3::ZERO,
        Rotation4::identity(),
    );
    m.get_transform(
        Some(&mut t),
        Some(&mut r),
        Some(&mut s),
        Some(&mut so),
        center.as_ref(),
    );

    let mut m2 = Matrix4::IDENTITY;
    m2.set_transform(Some(&t), Some(&r), Some(&s), Some(&so), center.as_ref());
    assert_close(&m, &m2);
}

#[test]
fn factor_translation_only() {
    round_trip(
        Vector3::new(3.0, -4.0, 5.0),
        Rotation4::identity(),
        Vector3::ONE,
        Rotation4::identity(),
        None,
    );
}

#[test]
fn factor_rotation_uniform_scale() {
    let mut r = Rotation4::identity();
    r.set_euler(0.3, -0.7, 1.1);
    round_trip(
        Vector3::new(1.0, 2.0, 3.0),
        r,
        Vector3::new(2.0, 2.0, 2.0),
        Rotation4::identity(),
        None,
    );
}

#[test]
fn factor_non_uniform_scale_with_orientation() {
    let mut r = Rotation4::identity();
    r.set_euler(0.0, 0.0, 0.9);
    let mut sor = Rotation4::identity();
    sor.set_euler(0.4, 0.2, -0.3);
    round_trip(
        Vector3::new(-1.5, 0.25, 7.0),
        r,
        Vector3::new(1.0, 2.5, 0.5),
        sor,
        None,
    );
}

#[test]
fn factor_negative_determinant() {
    let mut r = Rotation4::identity();
    r.set_euler(0.1, 0.5, -0.2);
    round_trip(
        Vector3::new(0.0, 0.0, 0.0),
        r,
        Vector3::new(1.0, 1.0, -1.0),
        Rotation4::identity(),
        None,
    );
}

#[test]
fn factor_with_center() {
    let mut r = Rotation4::identity();
    r.set_euler(-0.6, 0.0, 0.8);
    let mut sor = Rotation4::identity();
    sor.set_euler(0.0, 1.2, 0.0);
    round_trip(
        Vector3::new(4.0, -2.0, 1.0),
        r,
        Vector3::new(0.75, 1.5, 3.0),
        sor,
        Some(Vector3::new(10.0, 0.5, -3.0)),
    );
}

#[test]
fn factor_reports_translation_exactly() {
    let mut m = Matrix4::IDENTITY;
    let t = Vector3::new(8.0, -9.0, 10.0);
    m.set_transform(Some(&t), None, None, None, None);

    let mut out = Vector3::ZERO;
    m.factor(Some(&mut out), None, None, None);
    assert!(out.equals(&t));
}

/// Direct comparison against X_ITE Matrix4.js `getTransform`, run under node
/// 24 against `x_ite/src/standard/Math/Numbers/`. Factor outputs are only
/// unique up to eigenvector sign and axis order, so each case also rebuilds
/// the matrix and checks the rebuild against the oracle element list.
#[test]
fn factor_matches_x_ite_oracle() {
    // (translation, euler, scale, scale-euler, center, oracle factor output)
    let cases: [(
        [f64; 3],
        [f64; 3],
        [f64; 3],
        [f64; 3],
        Option<[f64; 3]>,
        ([f64; 3], [f64; 4], [f64; 3], [f64; 4], [f64; 16]),
    ); 3] = [
        (
            [4.0, -2.0, 1.0],
            [-0.6, 0.0, 0.8],
            [0.75, 1.5, 3.0],
            [0.0, 1.2, 0.0],
            Some([10.0, 0.5, -3.0]),
            (
                [4.0, -2.0, 1.0],
                [
                    -0.5728961792300403,
                    0.24221661961946356,
                    0.7830204831316621,
                    0.9901916904402646,
                ],
                [3.0, 1.5, 0.75],
                [
                    8.877714927779175e-17,
                    -0.9999999999999982,
                    -1.478687790204638e-16,
                    0.3707963267948972,
                ],
                [
                    1.8842906227049587,
                    2.030334819611774,
                    -0.46831517901709496,
                    0.0,
                    -1.0760341363492842,
                    0.8625252905561119,
                    -0.5900852993950492,
                    0.0,
                    0.5294246960328329,
                    1.040199064938676,
                    0.5550366286035694,
                    0.0,
                    -2.7166150707764487,
                    -19.114013646579767,
                    4.643304325679182,
                    1.0,
                ],
            ),
        ),
        (
            [1.0, 2.0, 3.0],
            [0.3, -0.7, 1.1],
            [2.0, 2.0, 2.0],
            [0.0, 0.0, 0.0],
            None,
            (
                [1.0, 2.0, 3.0],
                [
                    -0.10018762345965629,
                    -0.6310395447714674,
                    0.769253880744274,
                    1.2235521652695105,
                ],
                [1.9999999999999998, 1.9999999999999998, 2.0],
                [
                    0.4563433694439408,
                    0.6724795986940313,
                    0.5826850937726683,
                    0.9516844307850278,
                ],
                [
                    0.6938588993097978,
                    1.5300951567509715,
                    1.085066191131129,
                    0.0,
                    -1.3632659731868455,
                    1.206008797520428,
                    -0.8288839886583971,
                    0.0,
                    -1.288435374475382,
                    -0.45205264249924604,
                    1.4613632998710249,
                    0.0,
                    1.0,
                    2.0,
                    3.0,
                    1.0,
                ],
            ),
        ),
        (
            [0.0, 0.0, 0.0],
            [0.1, 0.5, -0.2],
            [1.0, 1.0, -1.0],
            [0.0, 0.0, 0.0],
            None,
            (
                [0.0, 0.0, 0.0],
                [
                    0.25159047843317467,
                    -0.023599212081860624,
                    0.9675460238928594,
                    2.9727809198395367,
                ],
                [-0.9999999999999999, -1.0, -0.9999999999999999],
                [
                    0.09826064165165267,
                    0.9943994945626525,
                    0.038916468438780556,
                    2.34583387388012,
                ],
                [
                    0.8600893382050472,
                    -0.15076818930386648,
                    -0.4873553974257773,
                    0.0,
                    0.1743487402881757,
                    0.9846791757040914,
                    0.003072083110434942,
                    0.0,
                    -0.47942553860420295,
                    0.08761206554319241,
                    -0.8731983044562817,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                ],
            ),
        ),
    ];

    for (tt, ee, ss, see, cc, (et, er, es, eso, em)) in cases {
        let mut r = Rotation4::identity();
        r.set_euler(ee[0], ee[1], ee[2]);
        let mut sor = Rotation4::identity();
        sor.set_euler(see[0], see[1], see[2]);
        let (t, s) = (
            Vector3::new(tt[0], tt[1], tt[2]),
            Vector3::new(ss[0], ss[1], ss[2]),
        );
        let center = cc.map(|c| Vector3::new(c[0], c[1], c[2]));

        let mut m = Matrix4::IDENTITY;
        m.set_transform(Some(&t), Some(&r), Some(&s), Some(&sor), center.as_ref());
        for i in 0..16 {
            assert!(
                (m.0[i] - em[i]).abs() <= 1e-12,
                "set {i}: {} != {}",
                m.0[i],
                em[i]
            );
        }

        let (mut ot, mut orr, mut os, mut oso) = (
            Vector3::ZERO,
            Rotation4::identity(),
            Vector3::ZERO,
            Rotation4::identity(),
        );
        m.get_transform(
            Some(&mut ot),
            Some(&mut orr),
            Some(&mut os),
            Some(&mut oso),
            center.as_ref(),
        );

        for (got, want) in [ot.x, ot.y, ot.z].iter().zip(et.iter()) {
            assert!((got - want).abs() <= 1e-9, "t: {got} != {want}");
        }
        for (got, want) in [os.x, os.y, os.z].iter().zip(es.iter()) {
            assert!((got - want).abs() <= 1e-9, "s: {got} != {want}");
        }
        for (got, want) in [orr.x(), orr.y(), orr.z(), orr.angle()]
            .iter()
            .zip(er.iter())
        {
            assert!((got - want).abs() <= 1e-9, "r: {got} != {want}");
        }
        for (got, want) in [oso.x(), oso.y(), oso.z(), oso.angle()]
            .iter()
            .zip(eso.iter())
        {
            assert!((got - want).abs() <= 1e-9, "so: {got} != {want}");
        }
    }
}
