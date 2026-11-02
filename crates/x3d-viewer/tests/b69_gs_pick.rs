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

mod common;

use common::gpu::{adapter_info, device};
use serde_json::Value;
use std::path::PathBuf;
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};

const T0: f64 = 946684800.0;

/// Largest difference of a hit value (point, normal, texCoord) that still counts as equal.
const VALUE_TOL: f64 = 1e-4;

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
        let base0 = json(root().join(format!(
            "oracle/{}/msaa0/{name}/js.json",
            if log {
                "b69-gs-pick-log-baseline"
            } else {
                "b69-gs-pick-baseline"
            }
        )));
        let base4 = json(root().join(format!(
            "oracle/{}/msaa4/{name}/js.json",
            if log {
                "b69-gs-pick-log-baseline"
            } else {
                "b69-gs-pick-baseline"
            }
        )));
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
                    let diff = w
                        .iter()
                        .zip(g)
                        .map(|(a, b)| (a - b).abs())
                        .fold(0.0, f64::max);

                    exact += (diff == 0.0) as usize;
                    case_worst = case_worst.max(diff);

                    if diff > VALUE_TOL {
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
