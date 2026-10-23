//! ParticleSystem rendering (lane XITE_R_PARTICLE_RENDER_01) against pinned X_ITE.
//!
//! The worlds and scripts are oracle/synthetic-rea-b13-particles (oracle/tools/gen_rea_b13_particles.py). A case's
//! X_ITE baseline is oracle/rea-batch13-particles-baseline/NAME/: `js.json` and `js.f32` in the format of
//! oracle/tools/x_ite_particles_b07.mjs (the per-frame emitter seeds and the 28-float particle records after each dump
//! frame) as written by `x3d-oracle particles`, `png/<frame>.png` the 512 x 512 canvas after that frame (anim.html, `antialiased="false"`) and
//! `msaa4/<frame>.png` the same with `antialiased="true" multisampling="4"` (anim-msaa.html).
//!
//! Each case runs `x3d-particles-drive` with the baseline's seeds and compares:
//! * every particle record: life, texCoordIndex0 exact; lifetime, elapsedTime, color, velocity, scale / rotation
//!   matrix and position within `1e-5 + 1e-5 * |reference|` (the GPU's float32 transcendental noise, measured up to
//!   4.2e-6 in REA batch 06R); a hidden record must be hidden on both sides;
//! * the images with the render oracle rule: at most 0.5% of the pixels more than 8 off in any channel.
//!
//! KNOWN_GAPS lists image passes the renderer cannot match yet (the particle records of those cases are still compared);
//! the test requires them to keep failing.
//!
//! Without a GPU adapter the image part is skipped. A case without a baseline is listed and skipped; the test fails
//! when no case has one, so a missing baseline directory is not a silent pass. `REA_ONLY=<substring>` restricts the cases.

mod common;

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

const STRIDE: usize = 28;

/// (case, image pass, why Rust differs).
const KNOWN_GAPS: &[(&str, &str, &str)] = &[(
    "pr-line",
    "msaa4",
    "MSAA line edges, not particles: the same 45 lines of frame 45 as a static IndexedLineSet differ from X_ITE by 1524 \
     pixels (0.581%, max 115) with multisampling 4 and 93 (0.035%) without, as the particles do (1523 and 93); X_ITE draws GL \
     lines, Rust its line emulation",
)];

fn f32s(path: &Path) -> Result<Vec<f32>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;

    Ok(bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect())
}

fn json(path: &Path) -> Result<Value, String> {
    serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?).map_err(|e| format!("{}: {e}", path.display()))
}

/// The records of one dump frame.
fn records<'a>(meta: &Value, data: &'a [f32], frame: &str) -> Option<&'a [f32]> {
    let d = &meta["dumps"][frame];
    let (n, offset) = (d["numParticles"].as_u64()? as usize, d["offset"].as_u64()? as usize);

    data.get(offset..offset + n * STRIDE)
}

fn compare_records(name: &str, frame: &str, js: &[f32], rs: &[f32], failures: &mut Vec<String>) {
    if js.len() != rs.len() {
        failures.push(format!("{name} frame {frame}: {} records in X_ITE, {} in Rust", js.len() / STRIDE, rs.len() / STRIDE));
        return;
    }

    let mut worst = (0.0f64, 0usize, 0usize);

    for (p, (a, b)) in js.chunks_exact(STRIDE).zip(rs.chunks_exact(STRIDE)).enumerate() {
        let hidden = (a[24].is_nan(), b[24].is_nan());

        if hidden.0 != hidden.1 {
            failures.push(format!("{name} frame {frame} particle {p}: hidden in {}", if hidden.0 { "X_ITE only" } else { "Rust only" }));
            continue;
        }

        if a[0] != b[0] || a[3] != b[3] {
            failures.push(format!("{name} frame {frame} particle {p}: life / texCoordIndex0 {:?} vs {:?}", (a[0], a[3]), (b[0], b[3])));
        }

        // Hidden records: only life, lifetime and elapsed time carry values.
        let fields: Vec<usize> = if hidden.0 { vec![1, 2] } else { (1..3).chain(4..11).chain(12..28).collect() };

        for k in fields {
            let (x, y) = (a[k] as f64, b[k] as f64);
            let error = if x.is_nan() || y.is_nan() { if x.is_nan() == y.is_nan() { 0.0 } else { f64::INFINITY } } else { (x - y).abs() };

            if error > 1e-5 + 1e-5 * x.abs() && error > worst.0 {
                worst = (error, p, k);
            }
        }
    }

    if worst.0 > 0.0 {
        let (_, p, k) = worst;
        failures.push(format!("{name} frame {frame} particle {p} float {k}: {} vs {}", js[p * STRIDE + k], rs[p * STRIDE + k]));
    }
}

fn compare_images(name: &str, js_dir: &Path, rs_dir: &Path, frames: &[String], failures: &mut Vec<String>) {
    for frame in frames {
        let (js, rs) = (js_dir.join(format!("{frame}.png")), rs_dir.join(format!("{frame}.png")));

        if !js.exists() {
            continue;
        }

        match (x3d_render::compare::load_png(&js), x3d_render::compare::load_png(&rs)) {
            (Ok((w, h, a)), Ok((rw, rh, b))) if (w, h) == (rw, rh) => {
                let s = x3d_render::compare::compare(w, h, &a, &b, 8);

                eprintln!("{name} {} frame {frame}: {} px over 8 ({:.3}%), max {}", js_dir.file_name().unwrap().to_string_lossy(), s.pixels_over, s.pct_over, s.max_delta);

                if s.pixels_over * 1000 > (w * h) as usize * 5 {
                    failures.push(format!("{name} {}: frame {frame}: {} pixels over 8 ({:.3}%), max {}", js_dir.display(), s.pixels_over, s.pct_over, s.max_delta));
                }
            }
            (a, b) => failures.push(format!("{name} frame {frame}: images {:?} / {:?}", a.map(|i| (i.0, i.1)), b.map(|i| (i.0, i.1)))),
        }
    }
}

fn run_case(root: &Path, name: &str, gpu: bool, failures: &mut Vec<String>) -> bool {
    let base = root.join("oracle/rea-batch13-particles-baseline").join(name);
    let world = root.join("oracle/synthetic-rea-b13-particles").join(format!("{name}.x3dv"));
    let script = root.join("oracle/synthetic-rea-b13-particles/scripts").join(format!("{name}.json"));

    if !base.join("js.json").exists() {
        return false;
    }

    let out = std::env::temp_dir().join(format!("rea-b13-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);

    for (pass, msaa) in [("png", None), ("msaa4", Some("4"))] {
        if pass == "msaa4" && !(gpu && base.join("msaa4").is_dir()) {
            continue;
        }

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_x3d-particles-drive"));
        cmd.arg(&world).arg(&script).arg("--seeds-from").arg(base.join("js.json")).arg("--out").arg(out.join(pass).join("rs.json"));

        if gpu {
            cmd.arg("--png-dir").arg(out.join(pass));
        }

        if let Some(n) = msaa {
            cmd.arg("--msaa").arg(n);
        }

        std::fs::create_dir_all(out.join(pass)).unwrap();

        let status = cmd.output().expect("x3d-particles-drive");

        if !status.status.success() {
            failures.push(format!("{name} {pass}: x3d-particles-drive failed: {}", String::from_utf8_lossy(&status.stderr)));
            continue;
        }

        let (js_meta, rs_meta) = match (json(&base.join("js.json")), json(&out.join(pass).join("rs.json"))) {
            (Ok(a), Ok(b)) => (a, b),
            (a, b) => {
                failures.push(format!("{name}: {:?} {:?}", a.err(), b.err()));
                continue;
            }
        };
        let frames: Vec<String> = js_meta["dumps"].as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default();

        if pass == "png" {
            let (js_data, rs_data) = match (f32s(&base.join(js_meta["fileF32"].as_str().unwrap_or("js.f32"))), f32s(&out.join(pass).join("rs.f32"))) {
                (Ok(a), Ok(b)) => (a, b),
                (a, b) => {
                    failures.push(format!("{name}: {:?} {:?}", a.err(), b.err()));
                    continue;
                }
            };

            for frame in &frames {
                match (records(&js_meta, &js_data, frame), records(&rs_meta, &rs_data, frame)) {
                    (Some(a), Some(b)) => compare_records(name, frame, a, b, failures),
                    _ => failures.push(format!("{name} frame {frame}: dump missing")),
                }
            }
        }

        if !gpu {
            continue;
        }

        if let Some((.., why)) = KNOWN_GAPS.iter().find(|(gap, gap_pass, _)| *gap == name && *gap_pass == pass) {
            let mut own = vec![];
            compare_images(name, &base.join(pass), &out.join(pass), &frames, &mut own);

            if own.is_empty() {
                failures.push(format!("{name} {pass}: matches X_ITE now; remove it from KNOWN_GAPS ({why})"));
            } else {
                eprintln!("{name} {pass}: known gap, {} differences ({why})", own.len());
            }
        } else {
            compare_images(name, &base.join(pass), &out.join(pass), &frames, failures);
        }
    }

    let _ = std::fs::remove_dir_all(&out);
    true
}

#[test]
fn rea_b13_particle_cases_match_x_ite() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let only = std::env::var("REA_ONLY").ok();
    let gpu = common::gpu::renderer_available();
    let mut names: Vec<String> = std::fs::read_dir(root.join("oracle/synthetic-rea-b13-particles"))
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".x3dv").map(str::to_string))
        .filter(|n| only.as_deref().is_none_or(|o| n.contains(o)))
        .collect();

    names.sort();

    let mut failures = vec![];
    let mut checked = vec![];
    let mut without = vec![];

    for name in &names {
        if run_case(&root, name, gpu, &mut failures) {
            checked.push(name.as_str());
        } else {
            without.push(name.as_str());
        }
    }

    eprintln!("{} cases checked (images: {gpu}); without a baseline: {}", checked.len(), without.join(", "));
    assert!(!checked.is_empty(), "no case has a baseline in oracle/rea-batch13-particles-baseline");
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
}
