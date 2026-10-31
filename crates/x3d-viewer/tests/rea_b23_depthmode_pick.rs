//! DepthMode in the pointing pass (issue #79) against pinned X_ITE 16.5.0.
//!
//! The world and script are oracle/synthetic-rea-b23-depthmode-pick (oracle/tools/gen_rea_b23_depthmode_pick.py): 54 cells,
//! each with two or three coincident Rectangle2D shapes drawn in scene order under the pointer. The baseline is
//! oracle/rea-batch23-depthmode-pick-baseline/dm-pick/, recorded by `oracle/tools/rea_b23_record.py record` with
//! `x3d-oracle particles` (five byte-identical runs): `browser .touch (x, y)` at every cell center, the DEF name of the hit
//! shape. `renderer.json` beside it names the GPU the reference ran on: WebGL 2 on ANGLE over OpenGL, 24 depth bits.
//!
//! The test loads the same world in the embedded viewer (512 x 512, the oracle canvas), picks the same points and compares
//! the hit shape of every cell. Hit points are not compared: this test is about which shape the depth state keeps.
//! The pointing pass runs on wgpu; the adapter name and backend are printed so the evidence says what ran.
//!
//! KNOWN_GAPS lists cells where Rust differs from the reference. The test requires them to keep differing, so a fix shows
//! up as a failure that asks for the entry to go. Skipped without an adapter (hosted CI), through `x3d_evidence::no_gpu`.

mod common;

use common::gpu::{adapter_info, device};
use serde_json::Value;
use std::path::PathBuf;
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};

const T0: f64 = 946684800.0;

/// (case, why Rust differs).
const KNOWN_GAPS: &[(&str, &str)] = &[];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn json(path: PathBuf) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    )
    .unwrap()
}

#[test]
fn depthmode_pick_matches_pinned_x_ite() {
    let Some((d, q)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    let info = adapter_info().expect("adapter");
    let base = root().join("oracle/rea-batch23-depthmode-pick-baseline/dm-pick");
    let renderer = json(base.join("renderer.json"));

    println!(
        "reference: {} ({}, {} depth bits, Chrome {})",
        renderer["renderer"].as_str().unwrap(),
        renderer["version"].as_str().unwrap(),
        renderer["depthBits"],
        renderer["chrome"].as_str().unwrap()
    );
    println!(
        "rust: adapter {:?}, backend {:?}, driver {:?} {:?}",
        info.name, info.backend, info.driver, info.driver_info
    );

    let want = json(base.join("js.json"));
    let want = want["pickProbes"]["3"].as_array().unwrap().clone();
    let script = json(root().join("oracle/synthetic-rea-b23-depthmode-pick/scripts/dm-pick.json"));
    let points = script["pickProbes"]["points"].as_array().unwrap().clone();
    let cases = json(root().join("oracle/synthetic-rea-b23-depthmode-pick/cases.json"));
    let cases = cases.as_array().unwrap().clone();
    let world = root().join("oracle/synthetic-rea-b23-depthmode-pick/dm-pick.x3dv");

    assert_eq!(want.len(), points.len());

    let got: Vec<String> = spawn_big_stack(move || {
        let mut v = EmbeddedViewer::new(d, q, (512, 512));

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

        points
            .iter()
            .map(|p| {
                let h = core.probe_pick(p[0].as_f64().unwrap(), p[1].as_f64().unwrap());

                match (&h.shape, h.id) {
                    (Some(s), true) => x3d_viewer::walk::rt_at(&core.rt, &s.path)
                        .map(|rt| rt.scene.nodes[s.node as usize].name.clone())
                        .unwrap_or_default(),
                    _ => String::new(),
                }
            })
            .collect()
    });

    let mut failures = vec![];
    let mut differing = 0;

    for (i, case) in cases.iter().enumerate() {
        let name = case["case"].as_str().unwrap();
        let reference = want[i]["shape"].as_str().unwrap_or("");
        let gap = KNOWN_GAPS.iter().find(|(n, _)| *n == name);

        if got[i] == reference {
            if let Some((_, why)) = gap {
                failures.push(format!(
                    "{name}: matches X_ITE now, remove the KNOWN_GAPS entry ({why})"
                ));
            }
        } else {
            differing += 1;
            println!(
                "DIFFERS {name}: X_ITE {reference:?}, Rust {:?} [{}]",
                got[i], case["requirement"]
            );

            if gap.is_none() {
                failures.push(format!("{name}: X_ITE {reference:?}, Rust {:?}", got[i]));
            }
        }
    }

    println!(
        "{} cells, {} match, {} differ",
        cases.len(),
        cases.len() - differing,
        differing
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
