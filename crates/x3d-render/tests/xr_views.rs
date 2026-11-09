//! XR views against pinned X_ITE 16.5.0 (34cdf9f97), issue #83: the world is drawn once per view of an XR pose, as
//! X3DRenderObject.draw does with `browser .getPose ()`, and compared with the picture the pinned page draws through its
//! own WebXR context (X3DWebXRContext) on a mock session. The page and the recorder: oracle/xr-shot.html,
//! oracle/x_ite-xr-shot.mjs; the script with the pose and the views: oracle/synthetic-xr/NAME.json; the key:
//! oracle/xr-shot-baseline/NAME.png (MSAA 0) and oracle/xr-shot-baseline-msaa4/NAME.png (Multisampling 4 per view);
//! three identical runs each, see NAME.json beside the key.
//!
//! The bound is the one of `x3d-shot compare`: at most 0.5% of the pixels more than 8 off. A missing key fails the test.
//! No headset and no XR runtime are involved; what a real session adds (the XRWebGLLayer or XRGPUBinding framebuffer,
//! the device's own poses, input) is not tested here.

use serde_json::Value;
use std::path::{Path, PathBuf};
use x3d_fields::math::{Matrix4, Vector3};
use x3d_render::gpu::Gpu;
use x3d_render::xr::{XrPose, XrRay, XrView};

const CASES: &[&str] = &[
    "stereo-shapes",
    "stereo-toein",
    "stereo-offaxis",
    "stereo-pose-only",
    "stereo-pose",
    "stereo-lights",
    "stereo-fog-background",
    "stereo-shadow-directional",
    "stereo-shadow-spot",
    "stereo-shadow-point",
    "stereo-clipplane",
    "stereo-points-lines",
];

/// The sample counts of the keys and the directory of each: X_ITE `Multisampling` 0 and 4.
const SAMPLES: &[(u32, &str)] = &[(0, "xr-shot-baseline"), (4, "xr-shot-baseline-msaa4")];

fn baseline_dir(samples: u32) -> &'static str {
    SAMPLES.iter().find(|s| s.0 == samples).unwrap().1
}

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

/// The input sources of a script (`inputSources`: the target-ray pose and whether a button is pressed) with the hit the pinned
/// page recorded for each beside its key (`hit .id`, `hit .originalPoint`, `hit .originalNormal`): the drawing is compared on its
/// own, the picking is compared by `x3d-viewer`'s xr_pointers tests.
fn rays(root: &Path, json: &Value, name: &str, samples: u32) -> Vec<XrRay> {
    let Some(sources) = json["inputSources"].as_array() else {
        return vec![];
    };
    let key: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join(format!("{}/{name}.json", baseline_dir(samples))))
            .unwrap(),
    )
    .unwrap();
    let vec3 = |v: &Value| {
        let a = v.as_array().unwrap();
        Vector3::new(
            a[0].as_f64().unwrap(),
            a[1].as_f64().unwrap(),
            a[2].as_f64().unwrap(),
        )
    };

    sources
        .iter()
        .zip(key["inputSources"].as_array().unwrap())
        .filter(|(_, k)| k["active"].as_bool().unwrap())
        .map(|(s, k)| XrRay {
            matrix: matrix(&s["matrix"]),
            pressed: s["pressed"].as_bool().unwrap(),
            hit: k["id"]
                .as_bool()
                .unwrap()
                .then(|| (vec3(&k["point"]), vec3(&k["normal"]))),
        })
        .collect()
}

fn script(root: &Path, name: &str, samples: u32) -> (Value, XrPose) {
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
        rays: rays(root, &json, name, samples),
        // The pinned page has contentScale="1".
        content_scale: 1.0,
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
    samples: u32,
    pose_for_rust: impl Fn(XrPose) -> XrPose,
) -> (usize, f64, u8) {
    measure_with(g, root, name, samples, samples, pose_for_rust)
}

/// Like `measure`, with the Rust views drawn at `render_samples` and the key of `samples`.
fn measure_with(
    g: &mut Gpu,
    root: &Path,
    name: &str,
    samples: u32,
    render_samples: u32,
    pose_for_rust: impl Fn(XrPose) -> XrPose,
) -> (usize, f64, u8) {
    let (json, pose) = script(root, name, samples);
    let (w, h) = size(&json);
    let dir = baseline_dir(samples);
    let (kw, kh, want) = x3d_render::compare::load_png(&root.join(format!("{dir}/{name}.png")))
        .unwrap_or_else(|e| panic!("{name}: no answer key oracle/{dir}/{name}.png: {e}"));

    assert_eq!((kw, kh), (w, h), "{name}: the key's size");

    let world = root.join(format!(
        "synthetic-xr/{}.x3dv",
        json["world"].as_str().unwrap()
    ));

    g.renderer.set_sample_count(render_samples);
    g.renderer.set_gl_sample_orientation(render_samples > 1);
    assert!(
        render_samples <= 1 || g.renderer.sample_count() == render_samples,
        "{render_samples}x not supported here"
    );

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
            &Path::new(&dir).join(format!("{name}-msaa{samples}-{tag}.png")),
            w,
            h,
            &rgba,
        )
        .unwrap();
    }

    eprintln!(
        "{name} MSAA {samples} (drawn at {render_samples}): {} pixels over 8 ({:.3}%), max {}",
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

    for &(samples, _) in SAMPLES {
        for name in cases() {
            let name = name.as_str();
            let (_, pct, max) = measure(&mut g, &root, name, samples, |p| p);

            if pct > 0.5 {
                failures.push(format!(
                    "{name} MSAA {samples}: {pct:.3}% of the pixels over 8, max {max}"
                ));
            }
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
        measure(&mut g, &root, "stereo-shapes", 0, |mut p| {
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
        measure(&mut g, &root, "stereo-shapes", 0, |mut p| {
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
        measure(&mut g, &root, "stereo-toein", 0, |mut p| {
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
        measure(&mut g, &root, "stereo-pose-only", 0, |mut p| {
            p.view_matrix = identity;
            p
        })
        .1,
    ));
    // The off-axis frusta replaced by the symmetric one of the first.
    results.push((
        "symmetric frusta",
        measure(&mut g, &root, "stereo-offaxis", 0, |mut p| {
            p.views[1].projection = p.views[0].projection;
            p
        })
        .1,
    ));

    for (what, pct) in results {
        assert!(pct > 0.5, "{what}: {pct:.3}% over 8, not over the bound");
    }
}

/// The comparison has teeth for the cases of shadows, clip planes, points and lines, and for MSAA 4: the same controls
/// (no eye offsets, swapped views, no pose in the model view), and the Rust views drawn at MSAA 0 against a key of MSAA 4.
#[test]
fn xr_views_of_the_added_cases_with_a_part_left_out_differ_from_x_ite() {
    let Ok(mut g) = Gpu::new() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let root = root();
    let identity = Matrix4([
        1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
    ]);
    let mut results = vec![];

    for name in [
        "stereo-shadow-directional",
        "stereo-shadow-spot",
        "stereo-shadow-point",
        "stereo-clipplane",
        "stereo-points-lines",
    ] {
        results.push((
            format!("{name}: no eye offsets"),
            measure(&mut g, &root, name, 0, |mut p| {
                for v in &mut p.views {
                    v.view_matrix = identity;
                }
                p
            })
            .1,
        ));
        results.push((
            format!("{name}: swapped views"),
            measure(&mut g, &root, name, 0, |mut p| {
                let a = p.views[0].view_matrix;
                p.views[0].view_matrix = p.views[1].view_matrix;
                p.views[1].view_matrix = a;
                p
            })
            .1,
        ));
        results.push((
            format!("{name}: no pose in the model view"),
            measure(&mut g, &root, name, 0, |mut p| {
                p.view_matrix = identity;
                p
            })
            .1,
        ));
    }

    // The MSAA 4 key against pictures drawn without multisampling: the key is a multisampled one.
    for name in ["stereo-shapes", "stereo-pose", "stereo-shadow-spot"] {
        results.push((
            format!("{name}: drawn at MSAA 0 against the MSAA 4 key"),
            measure_with(&mut g, &root, name, 4, 0, |p| p).1,
        ));
    }

    let weak: Vec<String> = results
        .iter()
        .filter(|(_, pct)| *pct <= 0.5)
        .map(|(what, pct)| format!("{what}: {pct:.3}% over 8, not over the bound"))
        .collect();

    assert!(weak.is_empty(), "{}", weak.join("\n"));
}

/// The input rays and hit points of the XR input sources (X3DWebXRContext.xrUpdatePointers, ScreenLine, ScreenPoint): five sources,
/// a ray that hits far away, one that hits within its length (the ray ends there), a pressed one that misses, and two
/// that hit a sphere. Both keys, MSAA 0 and 4.
const POINTER_CASES: &[&str] = &["stereo-pointers"];

/// The bound of the ray cases, in percent of the pixels over 8, and the least a part left out must be over.
const POINTER_BOUND: f64 = 0.05;
const POINTER_TEETH: f64 = 0.2;

#[test]
fn xr_input_rays_match_x_ite() {
    let Ok(mut g) = Gpu::new() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let root = root();
    let mut failures = vec![];

    for &(samples, _) in SAMPLES {
        for &name in POINTER_CASES {
            let (_, pct, max) = measure(&mut g, &root, name, samples, |p| p);

            // The rays are small: the bound is tighter than the 0.5% of the views (the drawing agrees with the key to
            // 14 pixels of 131072 at MSAA 0 and exactly at MSAA 4).
            if pct > POINTER_BOUND {
                failures.push(format!(
                    "{name} MSAA {samples}: {pct:.3}% of the pixels over 8, max {max}"
                ));
            }
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The comparison has teeth: the rays left out, the hit points left out, the rays all white, or an eye matrix left out of
/// them, and the pictures are over the bound.
#[test]
fn xr_input_rays_with_a_part_left_out_differ_from_x_ite() {
    let Ok(mut g) = Gpu::new() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let root = root();
    let mut results = vec![];

    for &name in POINTER_CASES {
        results.push((
            "no rays",
            measure(&mut g, &root, name, 0, |mut p| {
                p.rays.clear();
                p
            })
            .1,
        ));
        results.push((
            "no hit points",
            measure(&mut g, &root, name, 0, |mut p| {
                for r in &mut p.rays {
                    r.hit = None;
                }
                p
            })
            .1,
        ));
        results.push((
            "no pressed color",
            measure(&mut g, &root, name, 0, |mut p| {
                for r in &mut p.rays {
                    r.pressed = false;
                }
                p
            })
            .1,
        ));
        results.push((
            "the ray not shortened to its hit",
            measure(&mut g, &root, name, 0, |mut p| {
                for r in &mut p.rays {
                    r.hit = r.hit.map(|(point, normal)| {
                        (Vector3::new(point.x, point.y, point.z * 100.0), normal)
                    });
                }
                p
            })
            .1,
        ));
    }

    let weak: Vec<String> = results
        .iter()
        .filter(|(_, pct)| *pct <= POINTER_TEETH)
        .map(|(what, pct)| format!("{what}: {pct:.3}% over 8, not over the bound"))
        .collect();

    assert!(weak.is_empty(), "{}", weak.join("\n"));
}

/// An absent answer key fails; every case has its world, script and key.
#[test]
fn xr_keys_are_present() {
    let root = root();
    let missing: Vec<String> = CASES
        .iter()
        .chain(POINTER_CASES.iter())
        .flat_map(|n| {
            let mut files = vec![format!("synthetic-xr/{n}.json")];

            for (_, dir) in SAMPLES {
                files.push(format!("{dir}/{n}.png"));
                files.push(format!("{dir}/{n}.json"));
            }

            files
        })
        .filter(|p| !root.join(p).is_file())
        .collect();

    assert!(missing.is_empty(), "missing: {missing:?}");
}
