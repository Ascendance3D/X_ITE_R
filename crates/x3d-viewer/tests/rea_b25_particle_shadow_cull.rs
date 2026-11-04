//! ParticleSystem shadow-pass culling (lane XITE_R_AGENT_X_PARTICLES) against pinned X_ITE 16.5.0.
//!
//! X3DRenderObject.addDepthShape keeps a shape in a light's shadow pass only when the bbox sphere of the shape
//! (`bboxSize .norm () / 2` at `bboxCenter`, in the light's view space) meets the light view's volume. A ParticleSystem's
//! default bbox is its emitter's (a point for a PointEmitter), so a system whose emitter is outside a SpotLight's cone, or
//! outside five of a PointLight's six faces, casts no shadow there although its particles pass through. The worlds and
//! scripts are oracle/synthetic-rea-b25-particle-shadow-cull (oracle/tools/gen_rea_b25_particle_shadow_cull.py); a case's
//! X_ITE baseline is oracle/rea-b25-particle-shadow-cull-baseline/NAME/ (`js.json`, `js.f32`, `png/<frame>.png`,
//! `msaa4/<frame>.png`), recorded by `oracle/tools/rea_b25_particle_shadow_cull.py record` (three identical runs per pass).
//!
//! Each case runs `x3d-particles-drive` with the baseline's seeds and compares:
//! * the particle count of every step exactly, and every dumped particle record with the rule of rea_b13_particles.rs;
//! * every image of both passes (MSAA 0 and 4): at most [`IMAGE_PIXELS`] pixels more than 8 off in any channel. A missing
//!   or an extra shadow is thousands of pixels, so the rule is far tighter than the 0.5% of rea_b13_particles.rs.
//!
//! The test is skipped without an adapter (the shadow pass needs a GPU). `REA_ONLY=<substring>` restricts the cases.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

const STRIDE: usize = 28;

/// Pixels more than 8 off allowed in one 512 x 512 image.
const IMAGE_PIXELS: usize = 8;

fn json(path: &Path) -> Result<Value, String> {
    serde_json::from_str(
        &std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?,
    )
    .map_err(|e| format!("{}: {e}", path.display()))
}

fn f32s(path: &Path) -> Result<Vec<f32>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;

    Ok(bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect())
}

/// The records of one dump frame.
fn records<'a>(meta: &Value, data: &'a [f32], frame: &str) -> Option<&'a [f32]> {
    let d = &meta["dumps"][frame];
    let (n, offset) = (
        d["numParticles"].as_u64()? as usize,
        d["offset"].as_u64()? as usize,
    );

    data.get(offset..offset + n * STRIDE)
}

/// rea_b18_particle_lines.rs compare_records.
fn compare_records(frame: &str, js: &[f32], rs: &[f32], fail: &mut dyn FnMut(String)) {
    if js.len() != rs.len() {
        return fail(format!(
            "frame {frame}: {} records in X_ITE, {} in Rust",
            js.len() / STRIDE,
            rs.len() / STRIDE
        ));
    }

    let mut worst = (0.0f64, 0usize, 0usize);

    for (p, (a, b)) in js
        .chunks_exact(STRIDE)
        .zip(rs.chunks_exact(STRIDE))
        .enumerate()
    {
        let hidden = (a[24].is_nan(), b[24].is_nan());

        if hidden.0 != hidden.1 {
            fail(format!(
                "frame {frame} particle {p}: hidden in {}",
                if hidden.0 { "X_ITE only" } else { "Rust only" }
            ));
            continue;
        }

        if a[0] != b[0] || a[3] != b[3] {
            fail(format!(
                "frame {frame} particle {p}: life / texCoordIndex0 {:?} vs {:?}",
                (a[0], a[3]),
                (b[0], b[3])
            ));
        }

        let fields: Vec<usize> = if hidden.0 {
            vec![1, 2]
        } else {
            (1..3).chain(4..11).chain(12..28).collect()
        };

        for k in fields {
            let (x, y) = (a[k] as f64, b[k] as f64);
            let error = if x.is_nan() || y.is_nan() {
                if x.is_nan() == y.is_nan() {
                    0.0
                } else {
                    f64::INFINITY
                }
            } else {
                (x - y).abs()
            };

            if error > 1e-5 + 1e-5 * x.abs() && error > worst.0 {
                worst = (error, p, k);
            }
        }
    }

    if worst.0 > 0.0 {
        let (_, p, k) = worst;
        fail(format!(
            "frame {frame} particle {p} float {k}: {} vs {}",
            js[p * STRIDE + k],
            rs[p * STRIDE + k]
        ));
    }
}

/// Every image of one pass: at most [`IMAGE_PIXELS`] pixels more than 8 off in any channel.
fn compare_images(
    pass: &str,
    js_dir: &Path,
    rs_dir: &Path,
    frames: &[String],
    fail: &mut dyn FnMut(String),
) {
    for frame in frames {
        let (js, rs) = (
            js_dir.join(format!("{frame}.png")),
            rs_dir.join(format!("{frame}.png")),
        );

        match (
            x3d_render::compare::load_png(&js),
            x3d_render::compare::load_png(&rs),
        ) {
            (Ok((w, h, a)), Ok((rw, rh, b))) if (w, h) == (rw, rh) => {
                let s = x3d_render::compare::compare(w, h, &a, &b, 8);

                if s.pixels_over > IMAGE_PIXELS {
                    fail(format!(
                        "{pass} frame {frame}: {} pixels over 8 ({:.3}%), max {}",
                        s.pixels_over, s.pct_over, s.max_delta
                    ));
                }
            }
            (a, b) => fail(format!(
                "{pass} frame {frame}: images {:?} / {:?}",
                a.map(|i| (i.0, i.1)),
                b.map(|i| (i.0, i.1))
            )),
        }
    }
}

/// Runs one case and returns its failures.
fn run_case(root: &Path, name: &str) -> Vec<String> {
    let mut found = vec![];
    let base = root
        .join("oracle/rea-b25-particle-shadow-cull-baseline")
        .join(name);
    let world = root
        .join("oracle/synthetic-rea-b25-particle-shadow-cull")
        .join(format!("{name}.x3dv"));
    let script = root
        .join("oracle/synthetic-rea-b25-particle-shadow-cull/scripts")
        .join(format!("{name}.json"));
    let out = std::env::temp_dir().join(format!(
        "rea-b25-particle-shadow-cull-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();

    let js = match json(&base.join("js.json")) {
        Ok(js) => js,
        Err(e) => return vec![e],
    };
    // The records do not depend on the image pass; the MSAA 4 pass compares images only.
    for (pass, msaa) in [("png", None), ("msaa4", Some("4"))] {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_x3d-particles-drive"));
        cmd.arg(&world)
            .arg(&script)
            .arg("--seeds-from")
            .arg(base.join("js.json"))
            .arg("--out")
            .arg(out.join(pass).join("rs.json"))
            .arg("--png-dir")
            .arg(out.join(pass));

        if let Some(n) = msaa {
            cmd.arg("--msaa").arg(n);
        }

        std::fs::create_dir_all(out.join(pass)).unwrap();

        let status = cmd.output().expect("x3d-particles-drive");

        if !status.status.success() {
            found.push(format!(
                "{pass}: x3d-particles-drive failed: {}",
                String::from_utf8_lossy(&status.stderr)
            ));
            continue;
        }

        let mut fail = |what: String| found.push(what);

        compare_pass(pass, &js, &base, &out.join(pass), &mut fail);
    }

    let _ = std::fs::remove_dir_all(&out);
    found
}

/// One pass of one case against the baseline.
fn compare_pass(pass: &str, js: &Value, base: &Path, out: &Path, fail: &mut dyn FnMut(String)) {
    let rs = match json(&out.join("rs.json")) {
        Ok(rs) => rs,
        Err(e) => return fail(e),
    };
    if js["ok"] != Value::Bool(true) || rs["ok"] != Value::Bool(true) {
        fail(format!("ok js {} rs {}", js["ok"], rs["ok"]));
    }

    let counts = |r: &Value| {
        r["steps"]
            .as_array()
            .map(|s| {
                s.iter()
                    .map(|s| s["numParticles"].as_u64())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };

    if counts(&js) != counts(&rs) {
        fail(format!(
            "numParticles per step: X_ITE {:?} / Rust {:?}",
            counts(&js),
            counts(&rs)
        ));
    }

    let frames: Vec<String> = js["dumps"]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();

    match (
        f32s(&base.join(js["fileF32"].as_str().unwrap_or("js.f32"))),
        f32s(&out.join("rs.f32")),
    ) {
        (Ok(a), Ok(b)) => {
            for frame in &frames {
                match (records(&js, &a, frame), records(&rs, &b, frame)) {
                    (Some(a), Some(b)) => compare_records(frame, a, b, fail),
                    _ => fail(format!("frame {frame}: dump missing")),
                }
            }
        }
        (a, b) => fail(format!("{:?} {:?}", a.err(), b.err())),
    }

    compare_images(pass, &base.join(pass), out, &frames, fail);
}

#[test]
fn rea_b25_particle_shadow_cull_matches_x_ite() {
    if x3d_render::pointing_gpu::PointingGpu::new_headless().is_err() {
        return x3d_evidence::no_gpu("no adapter: the shadow pass needs a GPU, skipped");
    }

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let only = std::env::var("REA_ONLY").ok();
    let mut names: Vec<String> =
        std::fs::read_dir(root.join("oracle/synthetic-rea-b25-particle-shadow-cull"))
            .unwrap()
            .filter_map(|e| {
                e.ok()?
                    .file_name()
                    .to_str()?
                    .strip_suffix(".x3dv")
                    .map(str::to_string)
            })
            .collect();
    names.sort();
    names.retain(|n| only.as_deref().is_none_or(|o| n.contains(o)));

    assert!(!names.is_empty(), "no case selected");

    let failures = std::sync::Mutex::new(vec![]);
    let next = std::sync::atomic::AtomicUsize::new(0);

    // Four drives at a time, like rea_b22_two_layer_shadows (each holds a GPU device).
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(name) = names.get(i) else { break };
                    let found = run_case(&root, name);

                    failures
                        .lock()
                        .unwrap()
                        .extend(found.into_iter().map(|w| format!("{name}: {w}")));
                }
            });
        }
    });

    let failures = failures.into_inner().unwrap();
    eprintln!("{} cases checked", names.len());
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
