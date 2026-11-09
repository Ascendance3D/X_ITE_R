//! The picks behind the XR input rays (issue #83) against pinned X_ITE 16.5.0: the same world and the same five input
//! sources as `x3d-render`'s xr_views case `stereo-pointers` (oracle/synthetic-xr/stereo-pointers.{x3dv,json}). The pinned
//! page recorded for each source what X3DWebXRContext.xrUpdatePointers found (`hit .id`, `hit .originalPoint`,
//! `hit .originalNormal`, oracle/xr-shot-baseline/stereo-pointers.json); `Viewer3d::xr_rays` must give the same, which
//! `x3d_render::xr::pointer_draws` then draws (xr_views.rs compares the pixels).

use serde_json::Value;
use std::path::{Path, PathBuf};
use x3d_fields::math::Matrix4;
use x3d_viewer::viewer::{ORACLE_PAGE_ATTRIBUTES, Viewer3d, XrSource};

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 60.0;

fn oracle() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap()
}

fn matrix(v: &Value) -> Matrix4 {
    let a: Vec<f64> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_f64().unwrap())
        .collect();
    Matrix4(a.try_into().unwrap())
}

fn json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The pinned page's per-view viewport is the size of the pick (`touch (viewport [2] / 2, viewport [3] / 2)`): 256 x 256.
fn world(root: &Path) -> Viewer3d {
    let mut v = Viewer3d::load_attributes(
        &root.join("synthetic-xr/stereo-pointers.x3dv"),
        Some(root),
        T0,
        256,
        256,
        Some(60.0),
        false,
        ORACLE_PAGE_ATTRIBUTES,
    )
    .expect("load");
    let mut t = T0;

    for _ in 0..3 {
        t += DT;
        v.date_now = t;
        v.step(t);
    }

    v
}

fn sources(script: &Value) -> Vec<XrSource> {
    script["inputSources"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let matrix = matrix(&s["matrix"]);
            let mut inverse = matrix;

            inverse.inverse();

            XrSource {
                id: i as u32,
                active: true,
                button0: s["pressed"].as_bool().unwrap(),
                inverse,
                matrix,
                pressed: s["pressed"].as_bool().unwrap(),
            }
        })
        .collect()
}

#[test]
fn the_hits_of_the_input_rays_equal_the_ones_pinned_x_ite_found() {
    let root = oracle();
    let script = json(&root.join("synthetic-xr/stereo-pointers.json"));
    let key = json(&root.join("xr-shot-baseline/stereo-pointers.json"));
    let mut v = world(&root);
    let sources = sources(&script);

    v.xr_pointers(&sources);

    let rays = v.xr_rays(&sources);
    let want = key["inputSources"].as_array().unwrap();

    assert_eq!(rays.len(), want.len());

    for (i, (ray, k)) in rays.iter().zip(want).enumerate() {
        assert_eq!(
            ray.hit.is_some(),
            k["id"].as_bool().unwrap(),
            "source {i}: a hit"
        );
        assert_eq!(
            ray.pressed,
            script["inputSources"][i]["pressed"].as_bool().unwrap()
        );

        if let Some((point, normal)) = ray.hit {
            let close = |got: f64, want: &Value, what: &str| {
                let want = want.as_f64().unwrap();

                assert!(
                    (got - want).abs() < 1e-4,
                    "source {i}: {what} {got} against pinned {want}"
                );
            };

            for (n, (got, name)) in [(point.x, "x"), (point.y, "y"), (point.z, "z")]
                .into_iter()
                .enumerate()
            {
                close(got, &k["point"][n], &format!("point {name}"));
            }

            for (n, (got, name)) in [(normal.x, "x"), (normal.y, "y"), (normal.z, "z")]
                .into_iter()
                .enumerate()
            {
                close(got, &k["normal"][n], &format!("normal {name}"));
            }
        }
    }

    // The five sources show both cases: three hits and a pressed miss at least.
    assert!(rays.iter().any(|r| r.hit.is_none() && r.pressed));
    assert!(rays.iter().filter(|r| r.hit.is_some()).count() >= 3);
}

/// An inactive source has no ray.
#[test]
fn an_inactive_source_has_no_ray() {
    let root = oracle();
    let script = json(&root.join("synthetic-xr/stereo-pointers.json"));
    let mut v = world(&root);
    let mut sources = sources(&script);

    sources[1].active = false;
    v.xr_pointers(&sources);
    assert_eq!(v.xr_rays(&sources).len(), sources.len() - 1);
}
