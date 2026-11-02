//! Issue #54: the frame a fetched Script candidate (a file, a base64 data URL) completes in.
//!
//! Pinned X_ITE completes these fetches in Chrome's network time, so the frame (1 or 2 of the oracle) and the order of the
//! completions differ from run to run: `oracle/rea-b09-sai-baseline-variable/scr-url-fetch-frame/xite-runs.json` holds 14 runs
//! (four different frame maps, five different orders). No baseline can be recorded (five identical runs do not exist). X_ITE_R
//! completes the fetches while the world loads, in request order, so every candidate has initialized in the first frame:
//! ISO/IEC 19775-1 V4.0 29.2.3 requires `initialize ()` "before the X3D browser presents the world to the user and before any
//! events are processed by any nodes in the same X3D file". The test pins that behavior and that it is one of the outcomes X_ITE
//! recorded.

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// For every LOG entry (`"<index> <text>"` lines of the reporter Script's `out`): the first dumped frame it appears in, and the
/// order of the last dump.
fn frames_and_order(result: &Value) -> (Vec<(String, u64)>, Vec<String>) {
    let vrml = result["vrml"].as_object().expect("vrml dumps");
    let mut keys: Vec<u64> = vrml.keys().filter_map(|k| k.parse().ok()).collect();
    keys.sort_unstable();
    let mut first: Vec<(String, u64)> = vec![];
    let mut order = vec![];

    for k in keys {
        let text = vrml[&k.to_string()].as_str().unwrap();
        let reporter = text.split("DEF R Script").last().unwrap();
        let reporter = reporter.split("inputOnly").next().unwrap();
        order = reporter
            .lines()
            .filter_map(|l| {
                let l = l.trim().trim_end_matches(',');
                let l = l.strip_prefix('"')?.strip_suffix('"')?;
                let (n, rest) = l.split_once(' ')?;
                n.parse::<u32>().ok()?;
                Some(rest.to_string())
            })
            .collect();

        for e in &order {
            if !first.iter().any(|(n, _)| n == e) {
                first.push((e.clone(), k));
            }
        }
    }

    (first, order)
}

#[test]
fn fetched_script_candidates_complete_before_the_first_frame_in_request_order() {
    let root = root();
    let out = std::env::temp_dir().join(format!("rea-b54-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let rs_json = out.join("rs.json");
    let status = Command::new(env!("CARGO_BIN_EXE_x3d-drive"))
        .current_dir(&root)
        .args([
            "oracle/synthetic-rea-b09-sai/scr-url-fetch-frame.wrl",
            "oracle/synthetic-rea-b09-sai/sai.json",
            "--oracle",
            "oracle",
            "--warmup",
            "8",
            "--out",
        ])
        .arg(&rs_json)
        .status()
        .unwrap();
    assert!(status.success());

    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&rs_json).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(&out);
    assert_eq!(rs["ok"], Value::Bool(true));

    let (first, order) = frames_and_order(&rs);

    // 29.2.3: every candidate, fetched or not, has initialized in the first frame.
    for (entry, frame) in &first {
        assert_eq!(*frame, 1, "{entry} first appears in frame {frame}");
    }

    // The fetches complete in request order: U05 (base64), U06 (missing file, then its second candidate), U08, U09, U16.
    let want = [
        "U03 init",
        "U04 init",
        "U12 init",
        "U05 init",
        "U06 init",
        "external scr-ok.js top level",
        "external scr-ok.js initialize",
        "U09 init",
        "external scr-throw.js top level",
        "U16 init",
    ];
    let got: Vec<&str> = order
        .iter()
        .map(String::as_str)
        .filter(|e| want.contains(e))
        .collect();
    assert_eq!(got, want);

    // It is one of the outcomes pinned X_ITE recorded (frames and order), not an outcome X_ITE never produced.
    let runs: Value = serde_json::from_str(
        &std::fs::read_to_string(
            root.join("oracle/rea-b09-sai-baseline-variable/scr-url-fetch-frame/xite-runs.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let runs = runs["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 14);

    let same = runs.iter().any(|r| {
        let o: Vec<&str> = r["order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let mut frames_match = true;

        for (entry, frame) in &first {
            frames_match &= r["firstFrame"][entry.as_str()].as_u64() == Some(*frame);
        }

        frames_match && o == order.iter().map(String::as_str).collect::<Vec<_>>()
    });
    assert!(
        same,
        "no recorded X_ITE run has this frame map and order: {first:?}"
    );

    // X_ITE's other outcomes complete them in frame 2 (13 of 14 runs have at least one candidate there).
    let later = runs
        .iter()
        .filter(|r| {
            r["firstFrame"]
                .as_object()
                .unwrap()
                .values()
                .any(|f| f.as_u64() == Some(2))
        })
        .count();
    assert!(later >= 12, "{later}");
}
