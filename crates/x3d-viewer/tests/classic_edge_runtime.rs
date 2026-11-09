//! REA ledger entry sub:parse.vrml-classic: the six classic-encoding edge cases whose `log` the parse-only test
//! (crates/x3d-parse/tests/classic_edge.rs) lists as known differences, because X_ITE prints the result of a node's resource
//! load after the parse and a parse-only run loads nothing (crates/x3d-parse/tests/classic-edge-known.tsv, rows with the
//! field `log`). Here the same inputs run in the runtime through `x3d-drive` and the loads happen.
//!
//! The pinned log is the `log` of the case in oracle/classic-edge-baseline/classic-edge.json (pinned X_ITE 16.5.0, three
//! identical recordings). The recorder loaded each input with the base URL `http://oracle/x_ite-pin/src/`, so the world is
//! written to `<temporary root>/x_ite-pin/src/<case>.wrl` and `x3d-drive --oracle <temporary root>` gives it the same URL; a
//! missing file there is `Error: Not Found`, as at the recorder's server. Nothing of oracle/ is written.
//!
//! Compared: the sorted warn / error lines, each cut at its first line, which is the form of every runtime log comparison
//! (`Runtime::log_error` and `log_warn` keep the first line). Not compared: the lines after the first of a multi-line message
//! (the list of the tried URLs of `Couldn't load any of the N URLs, tried in this order:`); they are console text only.

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// The cases of the known table whose field is `log`.
fn log_cases(root: &std::path::Path) -> Vec<String> {
    let table = std::fs::read_to_string(root.join("crates/x3d-parse/tests/classic-edge-known.tsv"))
        .unwrap();

    table
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| {
            let mut f = l.split('\t');
            let (name, field) = (f.next()?, f.next()?);
            (field == "log").then(|| name.to_string())
        })
        .collect()
}

fn first_lines(v: &Value) -> Vec<String> {
    let mut lines: Vec<String> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().split('\n').next().unwrap().to_string())
        .collect();

    lines.sort();
    lines
}

#[test]
fn classic_edge_load_logs_match_x_ite_in_the_runtime() {
    let root = root();
    let cases = log_cases(&root);

    assert_eq!(
        cases.len(),
        6,
        "the known table has {} log rows, not 6: {cases:?}",
        cases.len()
    );

    let baseline: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("oracle/classic-edge-baseline/classic-edge.json"))
            .unwrap(),
    )
    .unwrap();
    let results = baseline["results"].as_array().unwrap();
    let tmp = std::env::temp_dir().join(format!("classic-edge-runtime-{}", std::process::id()));
    let src = tmp.join("x_ite-pin/src");

    std::fs::create_dir_all(&src).unwrap();

    let script = tmp.join("script.json");

    std::fs::write(
        &script,
        r#"{ "size": [64, 64], "t0": 946684800, "dt": 0.016666666666666666, "frames": 30, "events": [], "capture": [], "dump": [30] }"#,
    )
    .unwrap();

    let mut failures = vec![];

    for name in &cases {
        let case = results
            .iter()
            .find(|r| r["name"] == name.as_str())
            .unwrap_or_else(|| panic!("{name}: not in the baseline"));
        let pinned = first_lines(&case["log"]);

        assert!(
            !pinned.is_empty(),
            "{name}: the pinned log is empty, the case proves nothing"
        );

        let world = src.join(format!("{name}.wrl"));
        let out = tmp.join(format!("{name}.json"));

        std::fs::write(&world, case["text"].as_str().unwrap()).unwrap();

        let status = Command::new(env!("CARGO_BIN_EXE_x3d-drive"))
            .arg(&world)
            .arg(&script)
            .arg("--oracle")
            .arg(&tmp)
            .arg("--out")
            .arg(&out)
            .status()
            .unwrap();

        if !status.success() {
            failures.push(format!("{name}: x3d-drive exited with {status}"));
            continue;
        }

        let rs: Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        let port = first_lines(&rs["log"]);

        if port != pinned {
            failures.push(format!("{name}:\n  pinned: {pinned:?}\n  port:   {port:?}"));
        }
    }

    let _ = std::fs::remove_dir_all(&tmp);

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
