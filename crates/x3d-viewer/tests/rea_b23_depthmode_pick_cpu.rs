//! DepthMode in the CPU ray cast's pick (issue #79) against pinned X_ITE 16.5.0.
//!
//! The same 54 cells and the same baseline as `rea_b23_depthmode_pick.rs`
//! (oracle/synthetic-rea-b23-depthmode-pick, oracle/rea-batch23-depthmode-pick-baseline/dm-pick/), but the viewer has no
//! pointing GPU, so every pick is the CPU ray cast through the emulated depth buffer
//! (`pointer::cpu_winner`: 24-bit depths, depth function, mask, test, range and polygon offset in drawing order).
//! This test needs no adapter, so it runs on hosted CI too.
//!
//! The test compares the hit shape of every cell. KNOWN_GAPS lists cells where Rust differs from the reference; the
//! test requires them to keep differing, so a fix shows up as a failure that asks for the entry to go.

use serde_json::Value;
use std::path::PathBuf;
use x3d_viewer::embed::spawn_big_stack;
use x3d_viewer::viewer::Viewer3d;

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
fn depthmode_cpu_pick_matches_pinned_x_ite() {
    let base = root().join("oracle/rea-batch23-depthmode-pick-baseline/dm-pick");
    let want = json(base.join("js.json"));
    let want = want["pickProbes"]["3"].as_array().unwrap().clone();
    let script = json(root().join("oracle/synthetic-rea-b23-depthmode-pick/scripts/dm-pick.json"));
    let points = script["pickProbes"]["points"].as_array().unwrap().clone();
    let cases = json(root().join("oracle/synthetic-rea-b23-depthmode-pick/cases.json"));
    let cases = cases.as_array().unwrap().clone();
    let world = root().join("oracle/synthetic-rea-b23-depthmode-pick/dm-pick.x3dv");

    assert_eq!(want.len(), points.len());

    let got: Vec<String> = spawn_big_stack(move || {
        let mut v = Viewer3d::load(&world, None, T0, 512, 512).unwrap();

        assert!(v.pointing_gpu().is_none(), "this test picks on the CPU");

        for s in 1..=3 {
            v.step(T0 + s as f64 / 30.0);
        }

        points
            .iter()
            .map(|p| {
                let h = v.probe_pick(p[0].as_f64().unwrap(), p[1].as_f64().unwrap());

                match (&h.shape, h.id) {
                    (Some(s), true) => x3d_viewer::walk::rt_at(&v.rt, &s.path)
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
