//! XR views against pinned X_ITE 16.5.0 (34cdf9f97), issue #83: the world is drawn once per view of an XR pose, as
//! X3DRenderObject.draw does with `browser .getPose ()`, and compared with the picture the pinned page draws through its
//! own WebXR context (X3DWebXRContext) on a mock session. The page and the recorder: oracle/xr-shot.html,
//! oracle/x_ite-xr-shot.mjs; the script with the pose and the views: oracle/synthetic-xr/NAME.json; the key:
//! oracle/xr-shot-baseline/NAME.png (three identical runs, see NAME.json beside it).
//!
//! The bound is the one of `x3d-shot compare`: at most 0.5% of the pixels more than 8 off. A missing key fails the test.
//! No headset and no XR runtime are involved; what a real session adds (the XRWebGLLayer or XRGPUBinding framebuffer,
//! the device's own poses, input) is not tested here.

use serde_json::Value;
use std::path::{Path, PathBuf};
use x3d_fields::math::Matrix4;
use x3d_render::gpu::Gpu;
use x3d_render::xr::{XrPose, XrView};

const CASES: &[&str] = &[
    "stereo-shapes",
    "stereo-toein",
    "stereo-offaxis",
    "stereo-pose-only",
    "stereo-pose",
    "stereo-lights",
    "stereo-fog-background",
];

/// `XR_CASES=a,b`: other cases than CASES (for trying a script before it is added).
fn cases() -> Vec<String> {
    match std::env::var("XR_CASES") {
        Ok(v) => v.split(',').map(str::to_string).collect(),
        Err(_) => CASES.iter().map(|s| s.to_string()).collect(),
    }
}

fn root() -> PathBuf {
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

fn script(root: &Path, name: &str) -> (Value, XrPose) {
    let text = std::fs::read_to_string(root.join(format!("synthetic-xr/{name}.json"))).unwrap();
    let json: Value = serde_json::from_str(&text).unwrap();
    let views = json["views"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            let vp = v["viewport"].as_array().unwrap();
            XrView {
                projection: matrix(&v["projectionMatrix"]),
                view_matrix: matrix(&v["inverse"]),
                viewport: std::array::from_fn(|i| vp[i].as_u64().unwrap() as u32),
            }
        })
        .collect();
    let pose = XrPose {
        camera_space: matrix(&json["pose"]["matrix"]),
        view_matrix: matrix(&json["pose"]["inverse"]),
        headlight: json["headlight"].as_bool().unwrap_or(true),
        views,
    };

    (json, pose)
}

fn size(json: &Value) -> (u32, u32) {
    let s = json["size"].as_array().unwrap();
    (s[0].as_u64().unwrap() as u32, s[1].as_u64().unwrap() as u32)
}

/// The pixels over 8 and the percentage of the picture, for the Rust views of `name` against the key.
fn measure(
    g: &mut Gpu,
    root: &Path,
    name: &str,
    pose_for_rust: impl Fn(XrPose) -> XrPose,
) -> (usize, f64, u8) {
    let (json, pose) = script(root, name);
    let (w, h) = size(&json);
    let (kw, kh, want) =
        x3d_render::compare::load_png(&root.join(format!("xr-shot-baseline/{name}.png")))
            .unwrap_or_else(|e| {
                panic!("{name}: no answer key oracle/xr-shot-baseline/{name}.png: {e}")
            });

    assert_eq!((kw, kh), (w, h), "{name}: the key's size");

    let world = root.join(format!(
        "synthetic-xr/{}.x3dv",
        json["world"].as_str().unwrap()
    ));
    let shot = x3d_render::render_file_xr(g, root, &world, w, h, &pose_for_rust(pose)).unwrap();
    let rgba: Vec<u8> = shot
        .rgba
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2], 255])
        .collect();
    let s = x3d_render::compare::compare(w, h, &want, &rgba, 8);

    // `XR_DUMP=<dir>`: the Rust picture of each measured case, for looking at a difference.
    if let Ok(dir) = std::env::var("XR_DUMP") {
        let tag = std::thread::current()
            .name()
            .unwrap_or("t")
            .replace("::", "-");
        x3d_render::write_png(
            &Path::new(&dir).join(format!("{name}-{tag}.png")),
            w,
            h,
            &rgba,
        )
        .unwrap();
    }

    eprintln!(
        "{name}: {} pixels over 8 ({:.3}%), max {}",
        s.pixels_over, s.pct_over, s.max_delta
    );

    (
        s.pixels_over,
        s.pixels_over as f64 * 100.0 / (w * h) as f64,
        s.max_delta,
    )
}

#[test]
fn xr_views_match_x_ite() {
    let Ok(mut g) = Gpu::new() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let root = root();
    let mut failures = vec![];

    for name in cases() {
        let name = name.as_str();
        let (_, pct, max) = measure(&mut g, &root, name, |p| p);

        if pct > 0.5 {
            failures.push(format!("{name}: {pct:.3}% of the pixels over 8, max {max}"));
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The comparison has teeth: with a part of the pose left out the pictures are over the bound.
#[test]
fn xr_views_with_a_part_of_the_pose_left_out_differ_from_x_ite() {
    let Ok(mut g) = Gpu::new() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let root = root();
    let identity = Matrix4([
        1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
    ]);
    let mut results = vec![];

    // Both views drawn from the viewpoint: no eye offsets.
    results.push((
        "no eye offsets",
        measure(&mut g, &root, "stereo-shapes", |mut p| {
            for v in &mut p.views {
                v.view_matrix = identity;
            }
            p
        })
        .1,
    ));
    // The views swapped.
    results.push((
        "swapped views",
        measure(&mut g, &root, "stereo-shapes", |mut p| {
            let a = p.views[0].view_matrix;
            p.views[0].view_matrix = p.views[1].view_matrix;
            p.views[1].view_matrix = a;
            p
        })
        .1,
    ));
    // The eyes' toe-in left out: their translations only.
    results.push((
        "no eye rotation",
        measure(&mut g, &root, "stereo-toein", |mut p| {
            for v in &mut p.views {
                let t = [
                    v.view_matrix.0[12],
                    v.view_matrix.0[13],
                    v.view_matrix.0[14],
                ];
                v.view_matrix = identity;
                v.view_matrix.0[12..15].copy_from_slice(&t);
            }
            p
        })
        .1,
    ));
    // The viewer pose not put in front of the model-view matrices.
    results.push((
        "no pose in the model view",
        measure(&mut g, &root, "stereo-pose-only", |mut p| {
            p.view_matrix = identity;
            p
        })
        .1,
    ));
    // The off-axis frusta replaced by the symmetric one of the first.
    results.push((
        "symmetric frusta",
        measure(&mut g, &root, "stereo-offaxis", |mut p| {
            p.views[1].projection = p.views[0].projection;
            p
        })
        .1,
    ));

    for (what, pct) in results {
        assert!(pct > 0.5, "{what}: {pct:.3}% over 8, not over the bound");
    }
}

/// An absent answer key fails; every case has its world, script and key.
#[test]
fn xr_keys_are_present() {
    let root = root();
    let missing: Vec<String> = CASES
        .iter()
        .flat_map(|n| {
            [
                format!("synthetic-xr/{n}.json"),
                format!("xr-shot-baseline/{n}.png"),
                format!("xr-shot-baseline/{n}.json"),
            ]
        })
        .filter(|p| !root.join(p).is_file())
        .collect();

    assert!(missing.is_empty(), "missing: {missing:?}");
}
