//! ParticleSystem pointing (lane XITE_R_PARTICLE_PICKING_01) against pinned X_ITE 16.5.0.
//!
//! The worlds and scripts are oracle/synthetic-rea-b14-particle-pick (oracle/tools/gen_rea_b14_particle_pick.py). A case's
//! X_ITE baseline is oracle/rea-b14-particle-pick-baseline/NAME/ (`js.json`, `js.f32`), recorded by
//! `oracle/tools/rea_b14_pick.py record` with `x3d-oracle particles` (three identical runs). Its `pickProbes` are
//! `browser .touch (x, y)` at every probe point after frames 15, 30 and 45: id, the DEF name of the hit shape node, the
//! hit point, normal and texCoord.
//!
//! Each case runs `x3d-particles-drive` with the baseline's seeds and compares:
//! * the particle count of every step, exactly;
//! * `pickProbesReference`, the probes with X_ITE's particle records of that frame in place of the run's own: bit for bit,
//!   like the rea_batch12 pk-* cases. This is the pointing pass alone;
//! * `pickProbes`, the probes with the run's own records: the hit shape (id and name) of every probe. The values are not
//!   compared: the simulation's records differ from X_ITE's by float32 noise (REA batch 06R, within 1e-5 + 1e-5 *
//!   |reference|), and the hit point moves with them.
//!
//! PICK_ULPS lists the cases whose reference probes differ in the last float32 digits of a value (the hit shapes still have to
//! match), each with the largest distance in ulps that is allowed: the largest the recordings show. pp-two-systems has two systems; only PS is dumped, so its second system stands still,
//! which makes its records exact without a reference.
//!
//! The two equal-depth cases are the depth ties of the instanced pointing draw (lane
//! XITE_R_B25_2_GPU_INSTANCED_PICKING): pp-stack-equal-depth stacks coincident QUAD particles, and
//! pp-overlap-equal-depth overlaps coplanar GEOMETRY rectangles, all at z = -3, where the hit texCoord says which of
//! them the LEQUAL depth test kept. A system goes through the pass as one instanced draw, so these cases decide
//! whether that draw resolves a tie the way X_ITE's draw per particle does.
//!
//! The pointing pass needs a GPU: without an adapter the test is skipped. `REA_ONLY=<substring>` restricts the cases.

mod common;

use common::ulps::{software_tol, within_abs, within_ulps};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// `X3D_TEST_SOFTWARE_TOL` as the test validated it at its start (none on a GPU); only the comparison with the X_ITE
/// records uses it, the comparison with the port's own records stays strict.
static SOFT: OnceLock<Option<f64>> = OnceLock::new();

/// (case, largest allowed float32 ulp distance of a probe number, why Rust differs). The hit shapes (id, DEF name) must match,
/// and so must every number that is not within the limit. The limit is the largest distance the recordings show, not a round
/// number: a probe that moves further fails.
const PICK_ULPS: &[(&str, u64, &str)] = &[
    (
        "pp-row-line",
        3,
        "LINE particles: every probe hits the same particle system, but the hit point differs by up to 3 float32 ulps on 20 \
         of 68 hits: GL line rasterization vs. the quad emulation, the sub:pointer.picking line gap of rea_batch12 pk-lines",
    ),
    (
        "pp-row-point-props",
        1,
        "POINT particles with PointProperties: same hits, the hit texCoord (gl_PointCoord) differs by 1 float32 ulp on 16 \
         of 216 hits, the rea_batch12 pk-points-style gap",
    ),
    (
        "pp-slant-point-props",
        1,
        "POINT particles with PointProperties: same hits, the hit texCoord (gl_PointCoord) differs by 1 float32 ulp on 4 \
         of 52 hits, the rea_batch12 pk-points-style gap",
    ),
];

fn json(path: &Path) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    )
    .unwrap()
}

/// Numbers equal as f64 (X_ITE writes `0`, Rust `0.0`), everything else equal.
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

/// The hit shape of a probe: (id, DEF name).
fn selection(p: &Value) -> (Option<i64>, Option<&str>) {
    (p["id"].as_i64(), p["shape"].as_str())
}

/// Compares the probes of every frame; returns (value differences, selection differences, first difference).
fn compare_probes(
    want: &Value,
    got: &Value,
    max_ulps: u64,
    seen: &mut u64,
    soft: Option<f64>,
) -> (usize, usize, Option<String>) {
    let (mut values, mut selections, mut first) = (0, 0, None);
    let mut seen_abs = 0.0;

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

            // Equal selection and every number within the allowed ulps: the case's documented tolerance, not a difference.
            if selection(p) == selection(q)
                && (within_ulps(p, q, max_ulps, seen)
                    || soft.is_some_and(|t| within_abs(p, q, t, &mut seen_abs)))
            {
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

fn run_case(root: &Path, name: &str, failures: &mut Vec<String>) {
    let base = common::reference::pair_dir(root, "rea-b14-particle-pick-baseline", name);
    let world = root
        .join("oracle/synthetic-rea-b14-particle-pick")
        .join(format!("{name}.x3dv"));
    let script_path = root
        .join("oracle/synthetic-rea-b14-particle-pick/scripts")
        .join(format!("{name}.json"));
    let script = json(&script_path);
    let js = json(&base.join("js.json"));
    let out = std::env::temp_dir().join(format!("rea-b14-pick-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();

    // --png-dir sizes the canvas like the oracle page (512 x 512); the images are not compared here (rea_b13_particles is).
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_x3d-particles-drive"));
    cmd.arg(&world)
        .arg(&script_path)
        .arg("--records-from")
        .arg(base.join("js.json"))
        .arg("--out")
        .arg(out.join("rs.json"))
        .arg("--png-dir")
        .arg(out.join("png"));

    if script["seedFree"] != Value::Bool(true) {
        cmd.arg("--seeds-from").arg(base.join("js.json"));
    }

    let status = cmd.status().unwrap();
    let mut fail = |what: String| failures.push(format!("{name}: {what}"));

    if !status.success() {
        return fail(format!(
            "x3d-particles-drive exited with {status}: {}",
            std::fs::read_to_string(out.join("rs.json")).unwrap_or_default()
        ));
    }

    let rs = json(&out.join("rs.json"));

    if js["ok"] != Value::Bool(true)
        || rs["ok"] != Value::Bool(true)
        || rs["pointingGpu"] != Value::Bool(true)
    {
        return fail(format!(
            "ok js {} rs {}, pointing GPU {}",
            js["ok"], rs["ok"], rs["pointingGpu"]
        ));
    }

    let counts = |r: &Value| {
        r["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["numParticles"].as_u64())
            .collect::<Vec<_>>()
    };

    if counts(&js) != counts(&rs) {
        fail(format!(
            "numParticles per step: X_ITE {:?} / Rust {:?}",
            counts(&js),
            counts(&rs)
        ));
    }

    let want = &js["pickProbes"];
    let probes: usize = want
        .as_object()
        .unwrap()
        .values()
        .map(|v| v.as_array().unwrap().len())
        .sum();
    let hits: usize = want
        .as_object()
        .unwrap()
        .values()
        .flat_map(|v| v.as_array().unwrap())
        .filter(|p| p["id"] == 1)
        .count();
    let max_ulps = PICK_ULPS
        .iter()
        .find(|(case, ..)| *case == name)
        .map_or(0, |(_, ulps, _)| *ulps);
    let mut seen = 0;
    let (values, selections, first) = compare_probes(
        want,
        &rs["pickProbesReference"],
        max_ulps,
        &mut seen,
        *SOFT.get().unwrap_or(&None),
    );
    let (own_values, own_selections, own_first) =
        compare_probes(want, &rs["pickProbes"], 0, &mut 0, None);

    if max_ulps > 0 {
        eprintln!(
            "{name}: probe numbers differ by at most {seen} float32 ulps (allowed {max_ulps})"
        );
    }

    eprintln!(
        "{name}: {probes} probes, {hits} X_ITE hits; X_ITE records: {values} value / {selections} hit differences; own records: {own_selections} hit differences ({own_values} values moved with the records)"
    );

    if own_selections > 0 {
        fail(format!(
            "own records: {own_selections} probes hit another shape; first {}",
            own_first.unwrap_or_default()
        ));
    }

    if selections + values > 0 {
        fail(format!(
            "X_ITE records: {selections} hit and {values} value differences (allowed {max_ulps} float32 ulps); first {}",
            first.unwrap_or_default()
        ));
    }

    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn rea_b14_particle_pick_matches_x_ite() {
    // Validate the setting before anything else (also before an adapter skip): a bad value, or the variable on a hardware
    // adapter, fails the run. Read once; the comparisons below use this value.
    let soft = software_tol();
    SOFT.set(soft).ok();

    if x3d_render::pointing_gpu::PointingGpu::new_headless().is_err() {
        return x3d_evidence::no_gpu("no adapter: the pointing pass needs a GPU, skipped");
    }

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let only = std::env::var("REA_ONLY").ok();
    let mut names: Vec<String> =
        std::fs::read_dir(root.join("oracle/rea-b14-particle-pick-baseline"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
    names.sort();
    names.retain(|n| only.as_deref().is_none_or(|o| n.contains(o)));

    assert!(
        only.is_some() || names.len() >= 20,
        "baselines missing: {names:?}"
    );

    // Every tolerance names a case that exists.
    for (gap, ..) in PICK_ULPS {
        assert!(
            only.is_some() || names.iter().any(|n| n == gap),
            "PICK_ULPS names {gap}, which has no baseline"
        );
    }

    // Four drives at a time: more processes than that ran the GPU out of memory on the reference machine, and a drive
    // without its pointing GPU fails the case (pointingGpu false).
    let failures = std::sync::Mutex::new(vec![]);
    let next = std::sync::atomic::AtomicUsize::new(0);

    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(name) = names.get(i) else { break };
                    let mut own = vec![];

                    run_case(&root, name, &mut own);
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

/// A bad `X3D_TEST_SOFTWARE_TOL` fails even when the compared values match strictly, where the lazy read behind
/// `within_ulps(..) || ..` would never run.
#[test]
fn an_invalid_software_tolerance_fails_even_when_the_values_match() {
    let v = serde_json::json!({"point": [1.0, 2.0, 3.0]});
    let mut seen = 0;

    assert!(within_ulps(&v, &v, 0, &mut seen), "strictly equal values");

    for bad in ["abc", "", "0", "-1e-4", "nan", "inf"] {
        assert!(
            common::ulps::parse_software_tol(Some(bad)).is_err(),
            "{bad:?} must be refused"
        );
    }

    assert_eq!(common::ulps::parse_software_tol(None), Ok(None));
    assert_eq!(
        common::ulps::parse_software_tol(Some(" 1e-4 ")),
        Ok(Some(1e-4))
    );
}

/// The variable is for a software adapter only (decided without a GPU).
#[test]
fn the_software_tolerance_is_refused_on_a_hardware_adapter() {
    use common::ulps::{decide_software_tol, is_software_adapter};
    use wgpu::DeviceType::{Cpu, DiscreteGpu, IntegratedGpu, Other, VirtualGpu};

    assert!(is_software_adapter(Cpu, "anything"));
    assert!(is_software_adapter(
        DiscreteGpu,
        "llvmpipe (LLVM 20.1.2, 256 bits)"
    ));
    assert!(is_software_adapter(Other, "Google SwiftShader"));
    assert!(!is_software_adapter(
        DiscreteGpu,
        "NVIDIA GeForce RTX 3060 Ti"
    ));
    assert!(!is_software_adapter(VirtualGpu, "Apple Paravirtual device"));

    assert_eq!(
        decide_software_tol(Some(1e-4), Cpu, "llvmpipe"),
        Ok(Some(1e-4))
    );
    assert_eq!(decide_software_tol(None, DiscreteGpu, "NVIDIA"), Ok(None));

    for (ty, name) in [
        (DiscreteGpu, "NVIDIA GeForce RTX 3060 Ti"),
        (IntegratedGpu, "Intel"),
        (VirtualGpu, "Apple Paravirtual device"),
    ] {
        let e = decide_software_tol(Some(1e-4), ty, name).unwrap_err();

        assert!(
            e.contains("X3D_TEST_SOFTWARE_TOL") && e.contains(name),
            "{e}"
        );
    }
}

/// `within_abs` compares numbers, not types, and does not panic (a number that cannot convert to f64 fails the comparison).
#[test]
fn within_abs_compares_within_the_limit() {
    let one = serde_json::json!(1.0);
    let mut seen = 0.0;

    assert!(within_abs(
        &one,
        &serde_json::json!(1.00001),
        1e-4,
        &mut seen
    ));
    assert!(!within_abs(&one, &serde_json::json!(1.1), 1e-4, &mut seen));
    assert!(!within_abs(&one, &serde_json::json!("1"), 1e-4, &mut seen));
}
