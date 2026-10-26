//! REA batch 08 (glTF lane), issue #56: an Inline's glTF whose buffers fail to load, step by step
//! against the pinned X_ITE. `oracle/synthetic-rea-b08-gltf-inline-buffers/` holds the scenarios;
//! `oracle/x_ite-inline.mjs` recorded the pinned X_ITE (five byte-identical recordings) in
//! `oracle/rea-batch08-gltf-baseline/inline-buffer-errors-baseline.json`.
//!
//! `s-*`: one failed buffer whose authored URI and resolved URL differ (`DATA:`, a leading space,
//! percent-encoding, dot segments), or whose URI is no URL (`s-invalid-url`). The console subject
//! comes from the resolved URL (FileLoader.js:221, :390-394). `m-*`: two buffers, one or both
//! failing. GLTF2Parser.js:600 loads every buffer at once, each through its own FileLoader
//! (:613-614), so each failed buffer logs its own error when its fetch fails (a data URL before a
//! requested file: `m-missing-then-data`), and the Inline logs none (FileLoader.js:382-383). An
//! invalid buffer URL fails the Inline at once with its own error, before the other buffer's error
//! (`m-invalid-then-missing`, `m-missing-then-invalid`).
//!
//! For every step the console messages, the load count, the state and root nodes of the Inline and
//! the scene text must be equal.

mod gltf_inline_common;

use gltf_inline_common::{big_stack, case_names, oracle_root, run_scenario, set_case_dir};
use serde_json::Value;

fn scenarios_match_x_ite(prefix: &str) {
    set_case_dir("synthetic-rea-b08-gltf-inline-buffers");

    let text = std::fs::read_to_string(
        oracle_root().join("rea-batch08-gltf-baseline/inline-buffer-errors-baseline.json"),
    )
    .expect("baseline");
    let json: Value = serde_json::from_str(&text).unwrap();
    let base: Vec<&Value> = json["results"].as_array().unwrap().iter().collect();
    let mut failures = vec![];
    let mut compared = 0;

    for name in case_names().into_iter().filter(|n| n.starts_with(prefix)) {
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
    assert!(compared > 0, "no {prefix}* scenarios");
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

/// One failed buffer: `data URL` or `URL '<resolved>'`, from the resolved URL.
#[test]
fn gltf_inline_buffer_subject_matches_x_ite() {
    scenarios_match_x_ite("s-");
}

/// Two buffers: every failed buffer logs its own error, in the order the fetches fail; the Inline
/// logs none, or its own error first for an invalid buffer URL.
#[test]
fn gltf_inline_multi_buffer_errors_match_x_ite() {
    scenarios_match_x_ite("m-");
}
