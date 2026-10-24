//! Wide LINE particles in the pointing, collision and shadow passes (lane XITE_R_PARTICLE_WIDE_LINE_PASSES_01) against
//! pinned X_ITE 16.5.0.
//!
//! The worlds and scripts are oracle/synthetic-rea-b21-wide-lines (oracle/tools/gen_rea_b21_wide_lines.py). A case's
//! X_ITE baseline is oracle/rea-batch21-wide-lines-baseline/NAME/ (`js.json`, `js.f32`, `png/<frame>.png`), recorded by
//! `oracle/tools/rea_b21_wide_lines.py record` (three identical runs, `antialiased="false"`, 512 x 512).
//!
//! Each case runs `x3d-particles-drive` with the baseline's seeds at the oracle's canvas size and compares:
//! * the particle count of every step exactly, and every dumped particle record with the rule of rea_b13_particles.rs
//!   (life and texCoordIndex0 exact, the other floats within `1e-5 + 1e-5 * |reference|`);
//! * pw-pick-*: the pick probes with the rules of rea_b14_particle_pick.rs (with X_ITE's records in place: bit for bit;
//!   with the run's own records: the hit shape of every probe);
//! * pw-col-*: the collision probes of every frame with the rules of rea_b15_particle_collision.rs (hit states and
//!   shapes exactly, distances, normals and translations within [`PROBE_TOL`], pose within [`POSE_TOL`]);
//! * every case's images with the rule of rea_b18_particle_lines.rs (at most 0.5% of the pixels more than 8 off in any
//!   channel). The pw-shadow-* cases test the shadow pass this way.
//!
//! KNOWN_GAPS lists (case, part) pairs whose values may differ ("pick values", "collision values"); the hit shapes and
//! hit states must still match there, and the test requires the gap to keep differing. The collision cases also run
//! through the CPU depth stand-in (`--cpu-pointing`, no adapter needed) against CPU_KNOWN_GAPS.
//!
//! The GPU test is skipped without an adapter. `REA_ONLY=<substring>` restricts the cases.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

const STRIDE: usize = 28;

/// Allowed absolute difference of a probe's distance, normal and constrained translation (rea_b15_particle_collision).
const PROBE_TOL: f64 = 1e-6;

/// Allowed difference of the pose and positionOffset (rea_b15_particle_collision).
const POSE_TOL: f64 = 1e-4;

/// The oracle page's canvas.
const SIZE: &str = "512x512";

/// 1 px LINE particles in the collision depth buffer: rea_b15_particle_collision's model (GL's diamond-exit rule, merged
/// after the pass) probed in every frame and along two more directions than pc-line.
const ONE_PX_COLLISION: &str = "1 px LINE particles: rea_b15's diamond-exit model probed every frame; in 34 of 45 \
                                frames the closest pixel or a line's end pixel differs (distances up to 2.3e-2 along \
                                directions 6, 7, 9 and 10, translations 3 and 4 up to 9.2e-3), the hit states and \
                                shapes match";

/// gl.lineWidth particle lines in the pointing pass: the quad emulation of GL's wide-line rasterization.
const ALIASED_PICK: &str = "gl.lineWidth particle lines: the rea_b14 pp-row-line gap, the same hits and hit points up \
                            to 1.2e-7 off (GL line rasterization vs. the quad emulation)";

/// Transformed particle lines in the pointing pass.
const TRANSFORMED_PICK: &str = "transformed particle lines: the same hits; X_ITE runs LineTransform2.vs on the GPU in \
                                float32, the port emulates it in float32 but not bit for bit: hit points up to 1.4e-6 \
                                off";

/// (case, part: "pick values" or "collision values", why Rust differs).
const KNOWN_GAPS: &[(&str, &str, &str)] = &[
    ("pw-col-w1", "collision values", ONE_PX_COLLISION),
    (
        "pw-col-w6",
        "collision values",
        "the collision depth buffer draws a 6 px gl.lineWidth line as 1 px (measured: X_ITE's pw-col-w6 probes are \
         its pw-col-w1 probes), so the 1 px gap of pw-col-w1",
    ),
    ("pw-pick-row-w1", "pick values", ALIASED_PICK),
    ("pw-pick-row-w6", "pick values", ALIASED_PICK),
    ("pw-pick-row-w10", "pick values", ALIASED_PICK),
    ("pw-pick-row-w11", "pick values", TRANSFORMED_PICK),
    ("pw-pick-row-w24", "pick values", TRANSFORMED_PICK),
    ("pw-pick-row-w24-steady", "pick values", TRANSFORMED_PICK),
    (
        "pw-pick-slant-w24",
        "pick values",
        "transformed slanted particle lines: the same hits, hit points up to 1.6e-5 off: the float32 emulation of \
         the row cases, and overlapping lines at one depth (every particle lies in z = 0), where the triangle that \
         wins the tie gives the value; a double inverse of the instance matrix does not change it",
    ),
];

/// The CPU stand-in samples triangles at the pixel centers (rea_b15 CPU_TRIANGLES) instead of rasterizing them.
const CPU_TRIANGLES: &str = "transformed lines through the CPU stand-in: their triangles, thinner than a pixel of the \
                             16 x 16 buffer, are sampled at the pixel centers (rea_b15 CPU_TRIANGLES); the hit states \
                             match, distances and translations differ (up to 8.3e-2); the GPU run matches X_ITE";

/// (case, part, why the CPU depth stand-in differs) of the `--cpu-pointing` runs.
const CPU_KNOWN_GAPS: &[(&str, &str, &str)] = &[
    ("pw-col-w1", "collision values", ONE_PX_COLLISION),
    ("pw-col-w6", "collision values", ONE_PX_COLLISION),
    ("pw-col-w11", "collision values", CPU_TRIANGLES),
    ("pw-col-w24", "collision values", CPU_TRIANGLES),
    ("pw-col-w24-steady", "collision values", CPU_TRIANGLES),
];

type Gaps = &'static [(&'static str, &'static str, &'static str)];

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

/// Numbers equal as f64 (X_ITE writes `0`, Rust `0.0`), everything else equal (rea_b14_particle_pick).
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

/// The pick probes of every frame (rea_b14_particle_pick): (value differences, hit shape differences, first difference).
fn compare_picks(want: &Value, got: &Value) -> (usize, usize, Option<String>) {
    let (mut values, mut selections, mut first) = (0, 0, None);
    let selection = |p: &Value| (p["id"].as_i64(), p["shape"].as_str().map(str::to_string));

    for (frame, w) in want.as_object().unwrap() {
        let w = w.as_array().unwrap();
        let g = got[frame].as_array().cloned().unwrap_or_default();

        if g.len() != w.len() {
            selections += w.len().max(g.len());
            first.get_or_insert(format!(
                "frame {frame}: {} probes, X_ITE {}",
                g.len(),
                w.len()
            ));
            continue;
        }

        for (i, (p, q)) in w.iter().zip(&g).enumerate() {
            if same(p, q) {
                continue;
            }

            if selection(p) != selection(q) {
                selections += 1;
            } else {
                values += 1;
            }

            first.get_or_insert(format!("frame {frame} #{i}: X_ITE {p} / Rust {q}"));
        }
    }

    (values, selections, first)
}

fn nums(v: &Value) -> Vec<f64> {
    v.as_array()
        .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect())
        .unwrap_or_default()
}

/// The largest absolute difference of two number arrays (infinite when their lengths differ).
fn diff(a: &Value, b: &Value) -> f64 {
    let (a, b) = (nums(a), nums(b));

    if a.len() != b.len() {
        return f64::INFINITY;
    }

    a.iter()
        .zip(&b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
}

/// The collision probes of one frame (rea_b15_particle_collision): (hit or shape differences, value differences).
fn compare_collision(want: &Value, got: &Value) -> (Vec<String>, Vec<String>) {
    let mut hits = vec![];
    let mut values = vec![];
    let names = |v: &Value| {
        let mut n: Vec<String> = v
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap_or("").to_string())
            .collect();
        n.sort();
        n
    };

    if names(&want["shapes"]) != names(&got["shapes"]) {
        hits.push(format!(
            "shapes X_ITE {} / Rust {}",
            want["shapes"], got["shapes"]
        ));
    }

    let (wc, gc) = (
        want["closest"].as_array().unwrap(),
        got["closest"].as_array().unwrap(),
    );

    if wc.len() != gc.len() {
        hits.push(format!("{} / {} directions", wc.len(), gc.len()));
    }

    for (i, (p, q)) in wc.iter().zip(gc).enumerate() {
        let hit = |v: &Value| v["id"].as_i64().unwrap_or(-1) >= 0;

        if hit(p) != hit(q) || (hit(p) && p["shape"] != q["shape"]) {
            hits.push(format!("direction {i}: X_ITE {p} / Rust {q}"));
            continue;
        }

        if !hit(p) {
            continue;
        }

        let dd = (p["distance"].as_f64().unwrap_or(f64::NAN)
            - q["distance"].as_f64().unwrap_or(f64::NAN))
        .abs();
        let dn = diff(&p["normal"], &q["normal"]);

        if !(dd <= PROBE_TOL && dn <= PROBE_TOL) {
            values.push(format!(
                "direction {i}: X_ITE {p} / Rust {q} (distance {dd:.2e}, normal {dn:.2e})"
            ));
        }
    }

    for (i, (p, q)) in want["constrained"]
        .as_array()
        .unwrap()
        .iter()
        .zip(got["constrained"].as_array().unwrap())
        .enumerate()
    {
        let d = diff(p, q);

        if !(d <= PROBE_TOL) {
            values.push(format!("translation {i}: X_ITE {p} / Rust {q} ({d:.2e})"));
        }
    }

    for key in ["position", "orientation"] {
        if !(diff(&want["pose"][key], &got["pose"][key]) <= POSE_TOL) {
            hits.push(format!(
                "pose {key}: X_ITE {} / Rust {}",
                want["pose"][key], got["pose"][key]
            ));
        }
    }

    if !(diff(&want["positionOffset"], &got["positionOffset"]) <= POSE_TOL) {
        hits.push(format!(
            "positionOffset: X_ITE {} / Rust {}",
            want["positionOffset"], got["positionOffset"]
        ));
    }

    (hits, values)
}

/// rea_b18_particle_lines.rs compare_images: at most 0.5% of the pixels more than 8 off.
fn compare_images(js_dir: &Path, rs_dir: &Path, frames: &[String], fail: &mut dyn FnMut(String)) {
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

                if s.pixels_over * 1000 > (w * h) as usize * 5 {
                    fail(format!(
                        "frame {frame}: {} pixels over 8 ({:.3}%), max {}",
                        s.pixels_over, s.pct_over, s.max_delta
                    ));
                }
            }
            (a, b) => fail(format!(
                "frame {frame}: images {:?} / {:?}",
                a.map(|i| (i.0, i.1)),
                b.map(|i| (i.0, i.1))
            )),
        }
    }
}

/// Runs one case and returns its failures by part ("run", "records", "pick hits", "pick values", "collision hits",
/// "collision values", "png").
fn run_case(root: &Path, name: &str, cpu: bool) -> Vec<(&'static str, String)> {
    let found: std::cell::RefCell<Vec<(&'static str, String)>> = Default::default();
    let base = root
        .join("oracle/rea-batch21-wide-lines-baseline")
        .join(name);
    let world = root
        .join("oracle/synthetic-rea-b21-wide-lines")
        .join(format!("{name}.x3dv"));
    let script = root
        .join("oracle/synthetic-rea-b21-wide-lines/scripts")
        .join(format!("{name}.json"));
    let out = std::env::temp_dir().join(format!(
        "rea-b21-wide-lines-{}-{name}{}",
        std::process::id(),
        if cpu { "-cpu" } else { "" }
    ));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();

    let js = match json(&base.join("js.json")) {
        Ok(js) => js,
        Err(e) => return vec![("run", e)],
    };
    let pick = name.starts_with("pw-pick-");
    let collision = name.starts_with("pw-col-");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_x3d-particles-drive"));
    cmd.arg(&world)
        .arg(&script)
        .arg("--seeds-from")
        .arg(base.join("js.json"))
        .arg("--out")
        .arg(out.join("rs.json"));

    if cpu {
        cmd.arg("--cpu-pointing").arg("--png-size").arg(SIZE);
    } else {
        cmd.arg("--png-dir").arg(out.join("png"));
    }

    if pick {
        cmd.arg("--records-from").arg(base.join("js.json"));
    }

    let status = cmd.output().expect("x3d-particles-drive");

    if !status.status.success() {
        return vec![(
            "run",
            format!(
                "x3d-particles-drive failed: {}",
                String::from_utf8_lossy(&status.stderr)
            ),
        )];
    }

    let rs = match json(&out.join("rs.json")) {
        Ok(rs) => rs,
        Err(e) => return vec![("run", e)],
    };

    {
        let fail = |part: &'static str, what: String| found.borrow_mut().push((part, what));

        if js["ok"] != Value::Bool(true) || rs["ok"] != Value::Bool(true) {
            fail("run", format!("ok js {} rs {}", js["ok"], rs["ok"]));
        }

        if pick && rs["pointingGpu"] != Value::Bool(!cpu) {
            fail("run", format!("pointing GPU {}", rs["pointingGpu"]));
        }

        if collision && rs["depthGpu"] != Value::Bool(!cpu) {
            fail("run", format!("depth GPU {}", rs["depthGpu"]));
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
            fail(
                "records",
                format!(
                    "numParticles per step: X_ITE {:?} / Rust {:?}",
                    counts(&js),
                    counts(&rs)
                ),
            );
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
                        (Some(a), Some(b)) => {
                            compare_records(frame, a, b, &mut |w| fail("records", w))
                        }
                        _ => fail("records", format!("frame {frame}: dump missing")),
                    }
                }
            }
            (a, b) => fail("records", format!("{:?} {:?}", a.err(), b.err())),
        }

        if pick && !cpu {
            let want = &js["pickProbes"];
            let (values, selections, first) = compare_picks(want, &rs["pickProbesReference"]);
            let (_, own_selections, own_first) = compare_picks(want, &rs["pickProbes"]);

            if own_selections > 0 {
                fail(
                    "pick hits",
                    format!(
                        "own records: {own_selections} probes hit another shape; first {}",
                        own_first.unwrap_or_default()
                    ),
                );
            }

            if selections > 0 {
                fail(
                    "pick hits",
                    format!(
                        "X_ITE records: {selections} probes hit another shape; first {}",
                        first.clone().unwrap_or_default()
                    ),
                );
            }

            if values > 0 {
                fail(
                    "pick values",
                    format!(
                        "X_ITE records: {values} probes hit the same shape with other values; first {}",
                        first.unwrap_or_default()
                    ),
                );
            }
        }

        if collision {
            for (frame, w) in js["collisionProbes"].as_object().unwrap() {
                let g = &rs["collisionProbes"][frame];

                if g.is_null() {
                    fail("collision hits", format!("frame {frame}: no Rust probe"));
                    continue;
                }

                let (hits, values) = compare_collision(w, g);

                for h in hits {
                    fail("collision hits", format!("frame {frame}: {h}"));
                }

                for v in values {
                    fail("collision values", format!("frame {frame}: {v}"));
                }
            }
        }

        if !cpu {
            compare_images(&base.join("png"), &out.join("png"), &frames, &mut |w| {
                fail("png", w)
            });
        }
    }

    let _ = std::fs::remove_dir_all(&out);
    found.into_inner()
}

/// Every baseline case (or those `REA_ONLY` names) that `select` keeps, `threads` drives at a time.
fn check_all(cpu: bool, gaps: Gaps, threads: usize, select: fn(&str) -> bool) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let only = std::env::var("REA_ONLY").ok();
    let mut names: Vec<String> =
        std::fs::read_dir(root.join("oracle/synthetic-rea-b21-wide-lines"))
            .unwrap()
            .filter_map(|e| {
                e.ok()?
                    .file_name()
                    .to_str()?
                    .strip_suffix(".x3dv")
                    .map(str::to_string)
            })
            .filter(|n| select(n))
            .collect();
    names.sort();
    names.retain(|n| only.as_deref().is_none_or(|o| n.contains(o)));

    assert!(!names.is_empty(), "no case selected");

    for (gap, part, _) in gaps {
        assert!(
            only.is_some() || names.iter().any(|n| n == gap),
            "KNOWN_GAPS names {gap}, which is not a case"
        );
        assert!(
            part.ends_with(" values"),
            "{gap}: only values may be a known gap, not {part}"
        );
    }

    let failures = std::sync::Mutex::new(vec![]);
    let next = std::sync::atomic::AtomicUsize::new(0);

    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(name) = names.get(i) else { break };
                    let found = run_case(&root, name, cpu);
                    let mut own = vec![];

                    for (part, what) in &found {
                        if !gaps.iter().any(|(g, p, _)| g == name && p == part) {
                            own.push(format!("{name} {part}: {what}"));
                        }
                    }

                    for (_, part, why) in gaps.iter().filter(|(g, _, _)| g == name) {
                        if found.iter().any(|(p, _)| p == part) {
                            eprintln!("{name} {part}: known gap ({why})");
                        } else {
                            own.push(format!(
                                "{name} {part}: matches X_ITE now; remove it from the known gaps ({why})"
                            ));
                        }
                    }

                    failures.lock().unwrap().extend(own);
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

#[test]
fn rea_b21_wide_lines_match_x_ite() {
    if x3d_render::pointing_gpu::PointingGpu::new_headless().is_err() {
        return eprintln!(
            "no adapter: the pointing, collision and shadow passes need a GPU, skipped"
        );
    }

    // Four drives at a time, like rea_b14_particle_pick (each holds a GPU device).
    check_all(false, KNOWN_GAPS, 4, |_| true);
}

/// The collision cases through the CPU depth stand-in: runs without an adapter.
#[test]
fn rea_b21_wide_lines_cpu_collision_match_x_ite() {
    check_all(true, CPU_KNOWN_GAPS, 8, |n| n.starts_with("pw-col-"));
}
