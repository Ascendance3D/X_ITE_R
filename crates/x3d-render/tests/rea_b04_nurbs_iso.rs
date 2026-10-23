//! REA batch 04: the NURBS rules where Rust follows ISO/IEC 19775-1 clause 27 instead of X_ITE
//! 16.5.0 (owner decision 2026-10-17), checked against an independent reference written from the
//! ISO text: rational B-splines C(u) = Σ Nᵢ wᵢ Pᵢ / Σ Nᵢ wᵢ with the recursive Cox–de Boor basis
//! (not the de Boor evaluation the engine uses).
//!
//! Bound: |rust − reference| ≤ 2e-6 · max(1, |reference|) for positions and texture coordinates
//! (the engine stores Float32, whose rounding is at most 6e-8 relative, and the two algorithms
//! differ by a few double rounding steps). Areas in the trimming tests: 1e-4.
//!
//! The geometry cases are the `iso_*` entries of oracle/synthetic-rea-b04/geometry-cases.json; the
//! geometry oracle test (rea_b04_geometry.rs) shows that X_ITE gives other values for each.

use std::path::PathBuf;
use x3d_fields::math::{Rotation4, Vector3};
use x3d_fields::{FieldValue, NodeId};
use x3d_runtime::{LocalFetcher, Runtime};

fn oracle() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

const HEADER: &str = "#X3D V4.0 utf8\nPROFILE Interchange\nCOMPONENT NURBS:4\n";

fn runtime(body: &str) -> Runtime {
    x3d_fields::jsmath::set_flavor(x3d_fields::jsmath::MathFlavor::Chrome);
    let source = format!("{HEADER}{body}");
    let scene = x3d_parse::Parser::new(&source, "http://oracle/iso.x3dv")
        .parse()
        .expect("parse");
    let mut rt = Runtime::new(
        scene,
        Box::new(LocalFetcher {
            root: oracle(),
            prefix: "http://oracle/".into(),
        }),
    );
    rt.load_world();
    rt
}

fn case(name: &str) -> String {
    let cases: serde_json::Value = serde_json::from_slice(
        &std::fs::read(oracle().join("synthetic-rea-b04/geometry-cases.json")).unwrap(),
    )
    .unwrap();
    cases[name].as_str().unwrap().to_string()
}

fn geometry(text: &str) -> x3d_render::geometry::Geometry {
    let rt = runtime(&format!("DEF G {text}"));
    let n = rt.scene.get_named_node(x3d_scene::ROOT, "G").unwrap();
    x3d_render::traverse::geometry_of(&rt, n).unwrap()
}

// --- Reference (ISO 27.2.3 definitions) ------------------------------------------------------

/// Cox–de Boor N_{i,p}(u); the last non-empty span is closed on the right (u = domain end).
fn basis(i: usize, p: usize, u: f64, k: &[f64], last_span: usize) -> f64 {
    if p == 0 {
        return if (k[i] <= u && u < k[i + 1]) || (i == last_span && u == k[i + 1]) {
            1.0
        } else {
            0.0
        };
    }

    let mut n = 0.0;
    let d1 = k[i + p] - k[i];
    let d2 = k[i + p + 1] - k[i + 1];

    if d1 != 0.0 {
        n += (u - k[i]) / d1 * basis(i, p - 1, u, k, last_span);
    }

    if d2 != 0.0 {
        n += (k[i + p + 1] - u) / d2 * basis(i + 1, p - 1, u, k, last_span);
    }

    n
}

fn last_span(k: &[f64], n: usize) -> usize {
    (0..n).rev().find(|&i| k[i] < k[i + 1]).unwrap()
}

/// Pinned uniform knots scaled to [0, 1] (the default ISO leaves to the browser; X_ITE's choice).
fn default_knots(n: usize, order: usize) -> Vec<f64> {
    let mut k = vec![0.0; order];
    let inner = n - order;
    for i in 1..=inner {
        k.push(i as f64);
    }
    k.extend(std::iter::repeat((inner + 1) as f64).take(order));
    let max = *k.last().unwrap();
    k.iter().map(|x| x / max).collect()
}

/// ISO rational curve point.
fn curve_point(p: &[[f64; 3]], w: &[f64], k: &[f64], order: usize, u: f64) -> [f64; 3] {
    let ls = last_span(k, p.len());
    let (mut num, mut den) = ([0.0; 3], 0.0);

    for i in 0..p.len() {
        let b = basis(i, order - 1, u, k, ls) * w[i];
        for c in 0..3 {
            num[c] += b * p[i][c];
        }
        den += b;
    }

    num.map(|x| x / den)
}

/// ISO rational surface point; `p [i + j × uDimension]`.
#[allow(clippy::too_many_arguments)]
fn surface_point(
    p: &[[f64; 3]],
    w: &[f64],
    ud: usize,
    vd: usize,
    uk: &[f64],
    vk: &[f64],
    order: (usize, usize),
    u: f64,
    v: f64,
) -> [f64; 3] {
    let (lu, lv) = (last_span(uk, ud), last_span(vk, vd));
    let (mut num, mut den) = ([0.0; 3], 0.0);

    for j in 0..vd {
        let bv = basis(j, order.1 - 1, v, vk, lv);
        for i in 0..ud {
            let b = basis(i, order.0 - 1, u, uk, lu) * bv * w[i + j * ud];
            for c in 0..3 {
                num[c] += b * p[i + j * ud][c];
            }
            den += b;
        }
    }

    num.map(|x| x / den)
}

fn close(what: &str, got: f64, want: f64) {
    assert!(
        (got - want).abs() <= 2e-6 * want.abs().max(1.0),
        "{what}: {got} != {want} (ISO reference)"
    );
}

/// The control points the generator writes (oracle/tools/gen_rea_b04_nurbs.py), rounded to 6
/// digits and stored as Float32 by Coordinate.
fn wave() -> Vec<[f64; 3]> {
    let r = |x: f64| ((x * 1e6).round() / 1e6) as f32 as f64;
    let mut pts = vec![];
    for j in 0..4 {
        for i in 0..5 {
            let (fi, fj) = (i as f64, j as f64);
            pts.push([
                r(fi - 2.0),
                r(0.4 * (fi * 1.3).sin() * (fj * 0.9).cos()),
                r(fj - 1.5),
            ]);
        }
    }
    pts
}

/// The (u, v) of every vertex of an open grid surface (sample.js face order).
fn grid_params(nu: usize, nv: usize) -> Vec<(f64, f64)> {
    let mut out = vec![];
    for v0 in 0..nv {
        for u0 in 0..nu {
            let (a, b) = (u0 as f64 / nu as f64, (u0 + 1) as f64 / nu as f64);
            let (c, d) = (v0 as f64 / nv as f64, (v0 + 1) as f64 / nv as f64);
            out.extend_from_slice(&[(a, c), (b, c), (b, d), (a, c), (b, d), (a, d)]);
        }
    }
    out
}

fn check_surface(name: &str, g: &x3d_render::geometry::Geometry, p: &[[f64; 3]], w: &[f64]) {
    let (uk, vk) = (default_knots(5, 3), default_knots(4, 3));
    let params = grid_params(10, 8);
    assert_eq!(g.vertices.len(), params.len() * 4, "{name}: vertex count");

    for (i, (u, v)) in params.iter().enumerate() {
        let want = surface_point(p, w, 5, 4, &uk, &vk, (3, 3), *u, *v);
        for c in 0..3 {
            close(
                &format!("{name} vertex {i}.{c}"),
                g.vertices[i * 4 + c] as f64,
                want[c],
            );
        }
    }
}

#[test]
fn iso_rational_patch_surface() {
    // Weights 1 + (k % 3) · 0.5 (I1).
    let w: Vec<f64> = (0..20).map(|k| 1.0 + (k % 3) as f64 * 0.5).collect();
    check_surface(
        "iso_p_weighted",
        &geometry(&case("iso_p_weighted")),
        &wave(),
        &w,
    );

    // 22 weights for 20 points: ISO uses them (I2); X_ITE ignores them.
    let mut w = vec![1.0; 20];
    w[0] = 2.0;
    check_surface(
        "iso_p_weights_long",
        &geometry(&case("iso_p_weights_long")),
        &wave(),
        &w,
    );
}

#[test]
fn iso_closed_ignored_for_seam_weights() {
    // I3: a weight ≠ 1 within order − 1 control points of the seam: uClosed is ignored, the
    // surface is the open rational surface over the given 7 × 3 points.
    let mut p = vec![];
    for j in 0..3 {
        for i in 0..7 {
            let a = 2.0 * std::f64::consts::PI * (i % 6) as f64 / 6.0;
            let r = |x: f64| ((x * 1e6).round() / 1e6) as f32 as f64;
            p.push([r(a.cos()), r(j as f64 * 0.7 - 1.0), r(-a.sin())]);
        }
    }
    let mut w = vec![1.0; 21];
    w[0] = 2.0;
    w[6] = 2.0;
    let g = geometry(&case("iso_p_uclosed_seam_weight"));
    let (uk, vk) = (default_knots(7, 3), default_knots(3, 3));
    let params = grid_params(14, 6); // uTessellation 0: 2 × 7
    assert_eq!(g.vertices.len(), params.len() * 4);
    for (i, (u, v)) in params.iter().enumerate() {
        let want = surface_point(&p, &w, 7, 3, &uk, &vk, (3, 3), *u, *v);
        for c in 0..3 {
            close(
                &format!("iso_p_uclosed_seam_weight vertex {i}.{c}"),
                g.vertices[i * 4 + c] as f64,
                want[c],
            );
        }
    }

    // The same rule for a curve: closed TRUE with weight [1 2 1 1 1 1 1] stays open.
    let pts: Vec<[f64; 3]> = (0..7)
        .map(|k| {
            let a = 2.0 * std::f64::consts::PI * (k % 6) as f64 / 6.0;
            let r = |x: f64| ((x * 1e6).round() / 1e6) as f32 as f64;
            [r(a.cos()), r(a.sin()), 0.0]
        })
        .collect();
    let w = [1.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0];
    check_curve(
        "iso_c_closed_seam_weight",
        &geometry(&case("iso_c_closed_seam_weight")),
        &pts,
        &w,
        14,
    );
}

fn check_curve(
    name: &str,
    g: &x3d_render::geometry::Geometry,
    p: &[[f64; 3]],
    w: &[f64],
    res: usize,
) {
    let k = default_knots(p.len(), 3);
    assert_eq!(g.vertices.len(), res * 8, "{name}: vertex count");

    for s in 0..res {
        for (e, step) in [(0, s), (1, s + 1)] {
            let want = curve_point(p, w, &k, 3, step as f64 / res as f64);
            for c in 0..3 {
                close(
                    &format!("{name} segment {s}.{e}.{c}"),
                    g.vertices[s * 8 + e * 4 + c] as f64,
                    want[c],
                );
            }
        }
    }
}

#[test]
fn iso_rational_curve() {
    let p = [
        [0.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [2.0, -1.0, 0.5],
        [3.0, 0.5, 1.0],
        [4.0, 0.0, 0.0],
    ];
    check_curve(
        "iso_c_weighted",
        &geometry(&case("iso_c_weighted")),
        &p,
        &[1.0, 2.0, 0.5, 3.0, 1.0],
        10,
    );
}

#[test]
fn iso_rational_texture_coordinate() {
    // I8: NurbsTextureCoordinate 3 × 3, weights [1 2 1 2 4 2 1 2 1], sampled on the unit grid of
    // the surface (10 × 8): 2D coordinates of the rational surface, q = 1.
    let g = geometry(&case("iso_p_nurbstex_weighted"));
    let cp: Vec<[f64; 3]> = (0..9)
        .map(|k| [(k % 3) as f64 * 0.5, (k / 3) as f64 * 0.5, 0.0])
        .collect();
    let w = [1.0, 2.0, 1.0, 2.0, 4.0, 2.0, 1.0, 2.0, 1.0];
    let k = default_knots(3, 3);
    let params = grid_params(10, 8);

    for (i, (u, v)) in params.iter().enumerate() {
        let want = surface_point(&cp, &w, 3, 3, &k, &k, (3, 3), *u, *v);
        close(
            &format!("texCoord {i}.s"),
            g.tex_coords[i * 4] as f64,
            want[0],
        );
        close(
            &format!("texCoord {i}.t"),
            g.tex_coords[i * 4 + 1] as f64,
            want[1],
        );
        assert_eq!(
            (g.tex_coords[i * 4 + 2], g.tex_coords[i * 4 + 3]),
            (0.0, 1.0)
        );
    }
}

/// Area and triangle centroids of a trimmed surface in its default texture space (= the unit
/// (u, v) square of the domain).
fn tex_triangles(g: &x3d_render::geometry::Geometry) -> Vec<[[f64; 2]; 3]> {
    g.tex_coords
        .chunks_exact(12)
        .map(|t| {
            [
                [t[0] as f64, t[1] as f64],
                [t[4] as f64, t[5] as f64],
                [t[8] as f64, t[9] as f64],
            ]
        })
        .collect()
}

fn area(t: &[[f64; 2]; 3]) -> f64 {
    ((t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1])).abs()
        / 2.0
}

#[test]
fn iso_trimming_orientation() {
    // I4: one CW loop around [0.2, 0.8]²: its inside is discarded and the rest of the domain kept.
    let tris = tex_triangles(&geometry(&case("iso_t_cw_square")));
    let total: f64 = tris.iter().map(area).sum();
    assert!(
        (total - (1.0 - 0.36)).abs() < 1e-4,
        "kept area {total}, ISO 0.64"
    );

    for t in &tris {
        let c = [
            (t[0][0] + t[1][0] + t[2][0]) / 3.0,
            (t[0][1] + t[1][1] + t[2][1]) / 3.0,
        ];
        assert!(
            !(c[0] > 0.2 && c[0] < 0.8 && c[1] > 0.2 && c[1] < 0.8),
            "triangle inside the CW loop: {c:?}"
        );
    }

    // The CCW loop of the same square keeps its inside (ISO and X_ITE agree; exact in the oracle).
    let total: f64 = tex_triangles(&geometry(&case("t_ccw_square")))
        .iter()
        .map(area)
        .sum();
    assert!((total - 0.36).abs() < 1e-4, "kept area {total}, ISO 0.36");
}

#[test]
fn iso_rational_trimming_curve() {
    // The trimming loop is the rational NurbsCurve2D (tessellation 16, closed): the kept area is
    // the area of the polygon through its 17 ISO points.
    let p: Vec<[f64; 3]> = [
        [0.5, 0.1],
        [0.9, 0.1],
        [0.9, 0.5],
        [0.9, 0.9],
        [0.5, 0.9],
        [0.1, 0.9],
        [0.1, 0.5],
        [0.1, 0.1],
        [0.5, 0.1],
    ]
    .iter()
    .map(|q| [q[0], q[1], 0.0])
    .collect();
    let w = [1.0, 0.7, 1.0, 0.7, 1.0, 0.7, 1.0, 0.7, 1.0];
    // closed TRUE, but w [1] = 0.7 lies within order − 1 points of the seam, so ISO ignores
    // `closed` (I3): an open curve with pinned knots, sampled at 17 points (first = last).
    let k = default_knots(9, 3);
    let poly: Vec<[f64; 3]> = (0..16)
        .map(|s| curve_point(&p, &w, &k, 3, s as f64 / 16.0))
        .collect();
    let mut a = 0.0;
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        a += p[0] * q[1] - q[0] * p[1];
    }
    let want = a.abs() / 2.0;
    let total: f64 = tex_triangles(&geometry(&case("iso_t_weighted_curve")))
        .iter()
        .map(area)
        .sum();
    assert!(
        (total - want).abs() < 1e-4,
        "kept area {total}, ISO polygon {want}"
    );
}

#[test]
fn iso_weighted_swept_and_swung_curves() {
    // I1 through the child curves: the same surface built from an unweighted polyline through the
    // ISO points of the weighted curve.
    let traj = [
        [0.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [2.0, 1.0, 1.0],
        [3.0, 0.0, 1.0],
    ];
    let k = default_knots(4, 3);
    let pts: Vec<String> = (0..=10)
        .map(|s| curve_point(&traj, &[1.0, 3.0, 1.0, 1.0], &k, 3, s as f64 / 10.0))
        .map(|p| format!("{} {} {}", p[0], p[1], p[2]))
        .collect();
    let reference = format!(
        "NurbsSweptSurface {{ crossSectionCurve ContourPolyline2D {{ controlPoint [0.3 0, 0 0.3, -0.3 0, 0 -0.3, 0.3 0] }} trajectoryCurve NurbsCurve {{ order 2 tessellation 10 controlPoint CoordinateDouble {{ point [ {} ] }} }} }}",
        pts.join(", ")
    );
    compare_geometry(
        "iso_s_weighted_trajectory",
        &geometry(&case("iso_s_weighted_trajectory")),
        &geometry(&reference),
    );

    let profile = [
        [1.0, 0.0, 0.0],
        [1.2, 0.5, 0.0],
        [0.8, 1.0, 0.0],
        [1.0, 1.5, 0.0],
    ];
    let pts: Vec<String> = (0..=6)
        .map(|s| curve_point(&profile, &[1.0, 2.0, 1.0, 1.0], &k, 3, s as f64 / 6.0))
        .map(|p| format!("{} {}", p[0], p[1]))
        .collect();
    let reference = format!(
        "NurbsSwungSurface {{ profileCurve ContourPolyline2D {{ controlPoint [ {} ] }} trajectoryCurve NurbsCurve2D {{ tessellation 12 controlPoint [2 0, 2 2, -2 2, -2 -2, 2 -2, 2 0] }} }}",
        pts.join(", ")
    );
    compare_geometry(
        "iso_w_weighted_profile",
        &geometry(&case("iso_w_weighted_profile")),
        &geometry(&reference),
    );
}

fn compare_geometry(
    name: &str,
    got: &x3d_render::geometry::Geometry,
    want: &x3d_render::geometry::Geometry,
) {
    assert_eq!(
        got.vertices.len(),
        want.vertices.len(),
        "{name}: vertex count"
    );
    for (i, (a, b)) in got.vertices.iter().zip(&want.vertices).enumerate() {
        close(&format!("{name} vertex {i}"), *a as f64, *b as f64);
    }
}

// --- Interpolators (I5, I6, I7) --------------------------------------------------------------

fn set_and_read(
    rt: &mut Runtime,
    node: NodeId,
    input: &str,
    value: FieldValue,
    output: &str,
) -> FieldValue {
    let k = (node, rt.scene.get_field(node, input).unwrap());
    rt.set_value(k, value);
    rt.process_events();
    rt.value((node, rt.scene.get_field(node, output).unwrap()))
        .clone()
}

#[test]
fn iso_nurbs_interpolators() {
    let p = [
        [-3.0, 3.0, 0.0],
        [-1.0, 5.0, 1.0],
        [1.0, 2.0, 0.0],
        [3.0, 4.0, -1.0],
    ];
    let w = [1.0, 2.0, 0.5, 1.0];
    let knots = [0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0];
    let mut rt = runtime(
        "DEF PI NurbsPositionInterpolator { knot [0 0 0 1 2 2 2] weight [1 2 0.5 1] controlPoint DEF C Coordinate { point [-3 3 0, -1 5 1, 1 2 0, 3 4 -1] } }
         DEF OI NurbsOrientationInterpolator { knot [0 0 0 1 2 2 2] weight [1 2 0.5 1] controlPoint USE C }",
    );
    let pi = rt.scene.get_named_node(x3d_scene::ROOT, "PI").unwrap();
    let oi = rt.scene.get_named_node(x3d_scene::ROOT, "OI").unwrap();

    // I5: the fraction is the parameter: 0.5 and 1.5 lie in the knot domain [0, 2].
    for f in [0.0, 0.5, 1.0, 1.5, 2.0] {
        let FieldValue::SFVec3f(v) = set_and_read(
            &mut rt,
            pi,
            "set_fraction",
            FieldValue::SFFloat(f),
            "value_changed",
        ) else {
            panic!()
        };
        let want = curve_point(&p, &w, &knots, 3, f);
        close(&format!("position at {f}.x"), v.x, want[0]);
        close(&format!("position at {f}.y"), v.y, want[1]);
        close(&format!("position at {f}.z"), v.z, want[2]);

        // I6: the orientation turns +Z onto the tangent of the rational curve (central difference).
        let FieldValue::SFRotation(r) = set_and_read(
            &mut rt,
            oi,
            "set_fraction",
            FieldValue::SFFloat(f),
            "value_changed",
        ) else {
            panic!()
        };
        let h = 1e-6;
        let (a, b) = (
            curve_point(&p, &w, &knots, 3, (f - h).max(0.0)),
            curve_point(&p, &w, &knots, 3, (f + h).min(2.0)),
        );
        let mut t = Vector3::new(b[0] - a[0], b[1] - a[1], b[2] - a[2]);
        t.normalize();
        let mut z = Vector3::Z_AXIS;
        let r: Rotation4 = r;
        r.mult_vec_rot(&mut z);
        assert!(
            (z.x - t.x).abs() < 1e-5 && (z.y - t.y).abs() < 1e-5 && (z.z - t.z).abs() < 1e-5,
            "tangent at {f}: {z:?} != {t:?}"
        );
    }

    // Outside the domain the parameter is clamped to it.
    let FieldValue::SFVec3f(v) = set_and_read(
        &mut rt,
        pi,
        "set_fraction",
        FieldValue::SFFloat(3.0),
        "value_changed",
    ) else {
        panic!()
    };
    close("position clamped", v.x, 3.0);

    // I7: NurbsSurfaceInterpolator at (u, v) of the rational surface; the normal is Su × Sv of the
    // rational surface (central differences).
    let wv: Vec<f64> = (0..20).map(|k| 1.0 + (k % 3) as f64 * 0.5).collect();
    let pts = wave();
    let text: Vec<String> = pts
        .iter()
        .map(|p| format!("{} {} {}", p[0], p[1], p[2]))
        .collect();
    let mut rt = runtime(&format!(
        "DEF SI NurbsSurfaceInterpolator {{ uDimension 5 vDimension 4 weight [{}] controlPoint Coordinate {{ point [ {} ] }} }}",
        wv.iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(" "),
        text.join(", ")
    ));
    let si = rt.scene.get_named_node(x3d_scene::ROOT, "SI").unwrap();
    let (uk, vk) = (default_knots(5, 3), default_knots(4, 3));

    for (u, v) in [(0.3, 0.7), (0.5, 0.5), (0.1, 0.9)] {
        let fraction = FieldValue::SFVec2f(x3d_fields::math::Vector2::new(u, v));
        let FieldValue::SFVec3f(pos) =
            set_and_read(&mut rt, si, "set_fraction", fraction, "position_changed")
        else {
            panic!()
        };
        let FieldValue::SFVec3f(normal) = rt
            .value((si, rt.scene.get_field(si, "normal_changed").unwrap()))
            .clone()
        else {
            panic!()
        };
        let s = |u: f64, v: f64| surface_point(&pts, &wv, 5, 4, &uk, &vk, (3, 3), u, v);
        let want = s(u, v);
        close("surface x", pos.x, want[0]);
        close("surface y", pos.y, want[1]);
        close("surface z", pos.z, want[2]);

        let h = 1e-6;
        let d = |a: [f64; 3], b: [f64; 3]| {
            Vector3::new(
                (b[0] - a[0]) / (2.0 * h),
                (b[1] - a[1]) / (2.0 * h),
                (b[2] - a[2]) / (2.0 * h),
            )
        };
        let mut n = d(s(u - h, v), s(u + h, v));
        n.cross(&d(s(u, v - h), s(u, v + h)));
        let scale = n.dot(&n).sqrt().max(1.0);
        assert!(
            (normal.x - n.x).abs() < 1e-4 * scale
                && (normal.y - n.y).abs() < 1e-4 * scale
                && (normal.z - n.z).abs() < 1e-4 * scale,
            "normal at {u} {v}: {normal:?} != {n:?}"
        );
    }
}

#[test]
fn iso_nurbs_set_bbox_display_default() {
    // I9: ISO 27.4.8 gives NurbsSet bboxDisplay TRUE (X_ITE: FALSE).
    let rt = runtime("DEF S NurbsSet { }");
    let s = rt.scene.get_named_node(x3d_scene::ROOT, "S").unwrap();
    assert!(matches!(
        rt.value((s, rt.scene.get_field(s, "bboxDisplay").unwrap())),
        FieldValue::SFBool(true)
    ));
}
