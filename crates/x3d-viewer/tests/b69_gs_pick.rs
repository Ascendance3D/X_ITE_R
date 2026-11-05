//! GaussianSplats pointing (issue #69, pass 3) against pinned X_ITE 16.5.0.
//!
//! The worlds and probe scripts are oracle/synthetic-b69-gs-pick (oracle/tools/gen_b69_gs_pick.py). A case's baseline is
//! oracle/b69-gs-pick-baseline/msaa{0,4}/NAME/js.json, recorded by `oracle/tools/b69_gs_pick_record.py record NAME MSAA`
//! with `x3d-oracle particles` (five byte-identical runs each): `browser .touch (x, y)` on a 21 x 21 grid of pixel centers of the
//! 512 x 512 canvas, after frame 3: hit or miss, the hit point, normal and texCoord. The msaa4 baselines have a multisampled
//! display; the pointing buffer is not multisampled, so they must agree with the msaa0 ones.
//!
//! The test loads the same world in the embedded viewer (512 x 512), picks the same points through the wgpu pointing pass and
//! compares: hit or miss exactly, the values within VALUE_TOL (the two GPUs and their drivers round the splat quad differently).
//! A GaussianSplats shape has no CPU ray cast, so the pass is the only way to pick it. Skipped without an adapter (hosted CI),
//! through `x3d_evidence::no_gpu`.
//!
//! With `X3D_REFERENCE_PLATFORM=apple-m1` a case is compared with the pinned page recorded on the M1 (ANGLE Metal),
//! oracle/b69-gs-pick-apple-m1-baseline/NAME-msaaN.json and oracle/b69-gs-pick-log-apple-m1-baseline/ (`oracle/tools/b69_gs_pick_record.py
//! platform apple-m1`, three byte-identical runs), with VALUE_TOL on every component. The M1 page's hit distances of gs-pick-ortho
//! differ from the NVIDIA page's by up to 4.1e-4 (304 hits); every other value by at most 7.1e-5, every hit or miss equal
//! (docs/metal-reference-variation.md). gs-pick-ortho has no M1 file (owner decision 2026-11-04, issue #69): the page inverts
//! the projection with the driver's GLSL `inverse ()` (GaussianSplats.vs.js:344), which rounds differently on ANGLE Metal and on
//! NVIDIA; Rust keeps its CPU f64 inverse, equal to the NVIDIA page, so that case is compared with the NVIDIA recording and
//! DISTANCE_TOL_MACOS on macOS (measured on the M1: 1.68e-4 to 1.69e-4 from the M1 page, at most 2.43e-4 from the NVIDIA page).

mod common;

use common::gpu::{adapter_info, device};
use serde_json::Value;
use std::path::PathBuf;
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};

const T0: f64 = 946684800.0;

/// Largest difference of a hit value (point, normal, texCoord) that still counts as equal.
const VALUE_TOL: f64 = 1e-4;

/// The limit of the hit distance (the z of the hit point, index 2 of a probe) on macOS against the NVIDIA recording (issue #89),
/// fixed before the run that uses it. The hosted macOS 15 runner is a virtual machine with a paravirtual graphics device, not a
/// physical GPU; its first run (PR #342) differed from the pinned baselines by 1.7e-4 to 1.9e-4 in the hit distance of
/// gs-pick-ortho (about 10.7 units, depth buffer rounding), with every hit or miss and every other component equal. Only the
/// distance gets this limit; every other component (x, y, the normal, the texture coordinate) keeps VALUE_TOL on every platform.
/// It applies only where a case is compared with the NVIDIA recording: a case read from a platform recording
/// (`X3D_REFERENCE_PLATFORM`, see [`want_path`]) keeps VALUE_TOL on every component.
const DISTANCE_TOL_MACOS: f64 = 3e-4;

/// Index of the hit distance in a probe (`flat`: point x, y, z, normal, texCoord).
const DISTANCE: usize = 2;

/// The NVIDIA recordings; the platform recordings are `oracle/b69-gs-pick[-log]-<platform>-baseline/NAME-msaaN.json`.
const BASELINE: &str = "b69-gs-pick-baseline";
const LOG_BASELINE: &str = "b69-gs-pick-log-baseline";

/// The limit of component `i` of a probe; `platform` is true when the case is read from a platform recording.
fn tol_of(i: usize, platform: bool) -> f64 {
    if i == DISTANCE && cfg!(target_os = "macos") && !platform {
        DISTANCE_TOL_MACOS
    } else {
        VALUE_TOL
    }
}

/// The first component of two equal-hit probes that is outside its limit, and the largest difference of all.
fn compare_probe(w: &[f64; 10], g: &[f64; 10], platform: bool) -> (bool, f64) {
    let mut worst = 0f64;
    let mut ok = true;

    for (i, (a, b)) in w.iter().zip(g).enumerate() {
        let d = (a - b).abs();

        worst = worst.max(d);
        ok &= d <= tol_of(i, platform);
    }

    (ok, worst)
}

#[test]
fn only_the_distance_gets_the_macos_limit() {
    let base = [0.0f64; 10];

    for platform in [false, true] {
        for i in 0..10 {
            let mut off = base;

            off[i] = 2e-4;
            let (ok, _) = compare_probe(&base, &off, platform);

            assert_eq!(
                ok,
                i == DISTANCE && cfg!(target_os = "macos") && !platform,
                "component {i} off by 2e-4 (platform recording: {platform})"
            );

            off[i] = 4e-4;
            assert!(
                !compare_probe(&base, &off, platform).0,
                "component {i} off by 4e-4 is never accepted"
            );
        }
    }
}

/// Negative control of the M1 recording: a hit distance of gs-pick-default moved by 2e-4 is rejected under the limit that applies
/// to a platform recording, on every platform.
#[test]
fn a_moved_distance_fails_against_the_m1_recording() {
    let path = root().join(format!(
        "oracle/{}/gs-pick-default-msaa0.json",
        common::reference::platform_base(BASELINE, "apple-m1")
    ));
    let want: Vec<Probe> = json(path)["pickProbes"]["3"]
        .as_array()
        .unwrap()
        .iter()
        .map(want_of)
        .collect();
    let w = want.iter().flatten().next().expect("a hit");
    let mut g = *w;

    assert!(compare_probe(w, &g, true).0);
    g[DISTANCE] += 2e-4;
    assert!(!compare_probe(w, &g, true).0);
}

/// The baseline of a case: the platform recording `oracle/<stem>-<platform>-baseline/NAME-msaaN.json` when
/// `X3D_REFERENCE_PLATFORM` names a platform and the file exists (common/reference.rs), else the NVIDIA recording
/// `oracle/<base>/msaaN/NAME/js.json`. The flag is true for a platform recording.
fn want_path(base: &str, name: &str, msaa: u32) -> (PathBuf, bool) {
    if let Some(p) = common::reference::platform() {
        let over = root().join(format!(
            "oracle/{}/{name}-msaa{msaa}.json",
            common::reference::platform_base(base, &p)
        ));

        if over.is_file() {
            eprintln!("reference: {name} MSAA {msaa} uses the {p} recording");
            return (over, true);
        }

        // No platform file: allowed only for a case the recorder does not keep (NOT_KEPT in
        // oracle/tools/b69_gs_pick_record.py) or one its PROVENANCE.json lists as equal to the NVIDIA
        // recording. A missing or misspelled file is an error, not a quiet change of the reference.
        let case = format!("{name}-msaa{msaa}");
        let prov = over.with_file_name("PROVENANCE.json");
        let equal = prov.is_file()
            && json(prov)["equal_to_nvidia"]
                .as_array()
                .is_some_and(|a| a.iter().any(|c| c.as_str() == Some(case.as_str())));

        assert!(
            PLATFORM_NOT_KEPT.contains(&name) || equal,
            "{case}: no {p} recording at {} and the case is neither not kept nor equal to the NVIDIA recording",
            over.display()
        );
    }

    (
        root().join(format!("oracle/{base}/msaa{msaa}/{name}/js.json")),
        false,
    )
}

/// The cases that have no platform recording on purpose (`NOT_KEPT` of oracle/tools/b69_gs_pick_record.py).
const PLATFORM_NOT_KEPT: &[&str] = &["gs-pick-ortho"];

const CASES: &[&str] = &[
    "gs-pick-basic",
    "gs-pick-default",
    "gs-pick-xform",
    "gs-pick-clip",
    "gs-pick-mixed",
    "gs-pick-ortho",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn json(path: PathBuf) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    )
    .unwrap()
}

/// One probe's answer: `None` for a miss, else point, normal and texCoord.
type Probe = Option<[f64; 10]>;

fn flat(h: &x3d_viewer::pointer::Hit) -> Probe {
    h.id.then(|| {
        [
            h.point.x,
            h.point.y,
            h.point.z,
            h.normal.x,
            h.normal.y,
            h.normal.z,
            h.tex_coord.x,
            h.tex_coord.y,
            h.tex_coord.z,
            h.tex_coord.w,
        ]
    })
}

fn want_of(p: &Value) -> Probe {
    p["id"].as_i64().filter(|i| *i != 0)?;
    let n = |k: &str, i: usize| p[k][i].as_f64().unwrap();

    Some([
        n("point", 0),
        n("point", 1),
        n("point", 2),
        n("normal", 0),
        n("normal", 1),
        n("normal", 2),
        n("texCoord", 0),
        n("texCoord", 1),
        n("texCoord", 2),
        n("texCoord", 3),
    ])
}

#[test]
fn splat_picks_match_pinned_x_ite() {
    splat_picks(false);
}

/// Issue #69, the logarithmic depth pass: the same probes with the pinned page's `LogarithmicDepthBuffer` option on
/// (oracle/b69-gs-pick-log-baseline, `LOG=1 oracle/tools/b69_gs_pick_record.py record NAME 0` and `... NAME 4`, five byte-identical runs each; the MSAA 4 baselines have a multisampled display and must pick the same) and the
/// viewer in `DepthMode::Logarithmic`. Hit or miss is the same as without the option in all 2646 probes; X_ITE's own values move by at
/// most 4.7e-5 (528 probes) under the option. The wgpu pointing pass has no logarithmic mode, and stays within VALUE_TOL of the
/// pinned values (worst 7.1e-5, the standard-depth figure), so no mode is added to it.
#[test]
fn splat_picks_under_logarithmic_depth_match_pinned_x_ite() {
    splat_picks(true);
}

fn splat_picks(log: bool) {
    let Some((d, q)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    let info = adapter_info().expect("adapter");

    println!(
        "rust: adapter {:?}, backend {:?}, driver {:?} {:?}",
        info.name, info.backend, info.driver, info.driver_info
    );

    let only = std::env::var("B69_ONLY").ok();
    let mut failures = vec![];
    let (mut total, mut hits, mut exact, mut worst) = (0usize, 0usize, 0usize, 0f64);

    for name in CASES
        .iter()
        .filter(|n| only.as_deref().is_none_or(|o| n.contains(o)))
    {
        let dir = root().join("oracle/synthetic-b69-gs-pick");
        let world = dir.join(format!("{name}.x3dv"));
        let script = json(dir.join(format!("scripts/{name}.json")));
        let points = script["pickProbes"]["points"].as_array().unwrap().clone();
        let base = if log { LOG_BASELINE } else { BASELINE };
        // gs-pick-ortho has no platform file on purpose (module documentation).
        let (path0, platform) = want_path(base, name, 0);
        let (path4, platform4) = want_path(base, name, 4);

        assert_eq!(
            platform, platform4,
            "{name}: one platform recording for both MSAA levels"
        );

        let (base0, base4) = (json(path0), json(path4));
        let want: Vec<Probe> = base0["pickProbes"]["3"]
            .as_array()
            .unwrap()
            .iter()
            .map(want_of)
            .collect();
        let want4: Vec<Probe> = base4["pickProbes"]["3"]
            .as_array()
            .unwrap()
            .iter()
            .map(want_of)
            .collect();

        assert_eq!(want.len(), points.len());
        // The multisampled display changes no pick.
        assert_eq!(
            want, want4,
            "{name}: the MSAA 4 baseline differs from MSAA 0"
        );

        let (device, queue, probe_points) = (d.clone(), q.clone(), points.clone());
        let got: Vec<Probe> = spawn_big_stack(move || {
            let mut v = EmbeddedViewer::new(device, queue, (512, 512));

            if log {
                v.set_depth_mode(x3d_viewer::embed::DepthMode::Logarithmic);
            }

            v.load(
                &world,
                LoadOptions {
                    root: None,
                    start_time: Some(T0),
                },
            )
            .unwrap();

            for s in 1..=3 {
                v.step(T0 + s as f64 / 30.0);
            }

            let core = v.core().unwrap();

            probe_points
                .iter()
                .map(|p| flat(&core.probe_pick(p[0].as_f64().unwrap(), p[1].as_f64().unwrap())))
                .collect()
        });

        let (mut misses, mut case_hits, mut case_worst) = (0, 0, 0f64);

        for (i, (w, g)) in want.iter().zip(&got).enumerate() {
            total += 1;

            match (w, g) {
                (None, None) => {}
                (Some(w), Some(g)) => {
                    hits += 1;
                    case_hits += 1;
                    let (within, diff) = compare_probe(w, g, platform);

                    exact += (diff == 0.0) as usize;
                    case_worst = case_worst.max(diff);

                    if !within {
                        misses += 1;
                        failures.push(format!("{name} #{i}: X_ITE {w:?} / Rust {g:?}"));
                    }
                }
                _ => {
                    misses += 1;
                    failures.push(format!("{name} #{i}: X_ITE {w:?} / Rust {g:?}"));
                }
            }
        }

        worst = worst.max(case_worst);
        println!(
            "{name}: {} probes, {case_hits} hits, {misses} differ, worst value difference {case_worst:e}",
            points.len()
        );
    }

    println!(
        "{total} probes, {hits} hits, {exact} bit-identical, worst value difference {worst:e}"
    );
    failures.truncate(20);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
