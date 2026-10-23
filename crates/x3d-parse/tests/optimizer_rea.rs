//! REA batch 01: X3DOptimizer.optimizeSceneGraph against pinned X_ITE, every option set.
//! Baseline `oracle/synthetic-rea-optimizer/optimizer-baseline.json`, written by
//! `node oracle/x_ite-optimizer.mjs synthetic-rea-optimizer synthetic-rea-optimizer/optimizer-baseline.json`,
//! which runs X_ITE's own X3DOptimizer on each parsed world and prints the scene.

use std::path::Path;
use x3d_parse::optimizer::Optimizer;

#[test]
fn optimizer_matches_x_ite() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-optimizer");
    let baseline: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("optimizer-baseline.json")).unwrap(),
    )
    .unwrap();
    let mut failures = vec![];
    let mut checks = 0;

    for entry in baseline["results"].as_array().unwrap() {
        let file = entry["file"].as_str().unwrap();
        let data = std::fs::read(root.join(file)).unwrap();

        for (key, expected) in entry["options"].as_object().unwrap() {
            let optimizer = match key.as_str() {
                "none" => Optimizer::default(),
                "remove" => Optimizer {
                    remove_empty_groups: true,
                    ..Default::default()
                },
                "combine" => Optimizer {
                    combine_grouping_nodes: true,
                    ..Default::default()
                },
                "all" => Optimizer {
                    remove_empty_groups: true,
                    combine_grouping_nodes: true,
                    optimize_interpolators: true,
                },
                _ => panic!("unknown option set {key}"),
            };

            let mut scene = x3d_parse::parse_bytes(
                &data,
                &format!("http://oracle/synthetic-rea-optimizer/{file}"),
            )
            .unwrap();

            optimizer.optimize_scene_graph(&mut scene, x3d_scene::ROOT);
            scene.clone_counts = Some(scene.parse_clone_counts());

            let actual = scene.to_vrml_string();
            let expected = expected["value"].as_str().unwrap();

            checks += 1;

            if actual != expected {
                let (e, a): (Vec<&str>, Vec<&str>) =
                    (expected.lines().collect(), actual.lines().collect());
                let i = (0..e.len().max(a.len()))
                    .find(|i| e.get(*i) != a.get(*i))
                    .unwrap_or(0);
                failures.push(format!(
                    "{file} [{key}] line {}:\n  X_ITE: {}\n  Rust:  {}",
                    i + 1,
                    e.get(i).unwrap_or(&"<EOF>"),
                    a.get(i).unwrap_or(&"<EOF>")
                ));

                if let Ok(dir) = std::env::var("OPT_OUT") {
                    std::fs::create_dir_all(&dir).unwrap();
                    std::fs::write(Path::new(&dir).join(format!("{file}.{key}.rust")), &actual)
                        .unwrap();
                    std::fs::write(Path::new(&dir).join(format!("{file}.{key}.xite")), expected)
                        .unwrap();
                }
            }
        }
    }

    println!("{checks} checks, {} failed", failures.len());
    assert!(
        failures.is_empty(),
        "{} of {checks} checks failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
