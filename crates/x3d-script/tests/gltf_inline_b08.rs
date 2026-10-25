//! REA batch 08 (glTF lane): glTF and GLB scenes loaded through an Inline, step by step against the
//! pinned X_ITE. `oracle/synthetic-rea-b08-gltf-inline/` holds one scenario per case (set `url`,
//! `load`, `loadNow`, `unloadNow`, remove the Inline from its Group, valid and invalid glTF/GLB files);
//! `oracle/x_ite-inline.mjs` recorded the pinned X_ITE in
//! `oracle/rea-batch08-gltf-baseline/inline-baseline.json`. For every step the console messages,
//! the load count, the state and root nodes of the Inline and the scene text must be equal.

mod gltf_inline_common;

use gltf_inline_common::{big_stack, case_names, oracle_root, run_scenario};
use serde_json::Value;

#[test]
fn gltf_inline_scenarios_match_x_ite() {
    let text = std::fs::read_to_string(
        oracle_root().join("rea-batch08-gltf-baseline/inline-baseline.json"),
    )
    .expect("baseline");
    let json: Value = serde_json::from_str(&text).unwrap();
    let base: Vec<&Value> = json["results"].as_array().unwrap().iter().collect();
    let mut failures = vec![];
    let mut compared = 0;

    for name in case_names() {
        let expected = base
            .iter()
            .find(|r| r["case"] == name.as_str())
            .unwrap_or_else(|| panic!("{name}: no baseline"));
        let n = name.clone();
        let actual = big_stack(move || run_scenario(&n)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let (es, as_) = (
            expected["steps"].as_array().unwrap(),
            actual["steps"].as_array().unwrap(),
        );

        assert_eq!(es.len(), as_.len(), "{name}: step count");
        compared += 1;

        let (mut ev, mut av) = (String::new(), String::new());

        for (e, a) in es.iter().zip(as_) {
            if let Some(v) = e["vrml"].as_str() {
                ev = v.to_string();
            }

            if let Some(v) = a["vrml"].as_str() {
                av = v.to_string();
            }

            let k = e["k"].as_u64().unwrap();

            for field in ["loadCount", "console", "inlines"] {
                if e[field] != a[field] {
                    failures.push(format!(
                        "{name} step {k} {field}:\n  x_ite {}\n  rust  {}",
                        e[field], a[field]
                    ));
                }
            }

            if ev != av {
                failures.push(format!("{name} step {k} scene text differs"));
            }
        }
    }

    println!("{compared} scenarios compared");
    assert!(
        failures.is_empty(),
        "{} differences:\n{}",
        failures.len(),
        failures
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
