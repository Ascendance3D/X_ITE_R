//! Issue: the plain Inline reload and abort paths (`set_internal_scene`, `inline_abort`) against the browser
//! shutdown rule, step by step with the pinned X_ITE (16.5.0, 34cdf9f97).
//!
//! The 38 scenarios of `oracle/synthetic-inline-reload/` (see `inline_reload.rs`) replace, switch off, unload,
//! abort and nest Inline scenes whose Scripts print in `initialize ()` and `shutdown ()`. `inline_reload.rs`
//! allows a difference by `(case, steps, field)`, so a table entry also hides any other console line of its
//! step. This file closes that: it pins the one owner-approved difference, the `shutdown ()` of a replaced or
//! unloaded scene (ISO/IEC 14772-1 4.12.3; X_ITE calls none), by its exact console, and compares everything
//! else exactly. See `inline_common::check_shutdown_rule`.
//!
//! * The browser build (`Runtime::replace_skips_shutdown`) equals X_ITE in every step, apart from the four
//!   scenarios of `synthetic-inline-reload-known.tsv` that differ for another reason (c07 in step 5 only), and prints no `shutdown`.
//! * The native build differs only by the pinned `shutdown` lines.

mod inline_common;

use inline_common::{Pin, check_shutdown_rule, oracle_root};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// Native console of every step with a `shutdown` line (printed by `X3D_PRINT_PINS=1 cargo test ... -- --nocapture`).
const PINS: &[Pin] = &[
    (
        "c02-url-replace",
        5,
        &[
            "log: B fraction 0",
            "log: parent tick 2",
            "log: A fraction 5",
            "log: A shutdown",
        ],
    ),
    (
        "c03b-fallback-runtime",
        5,
        &[
            "log: B fraction 0",
            "log: parent tick 2",
            "log: A fraction 5",
            "log: A shutdown",
        ],
    ),
    ("c05-load-toggle", 4, &["log: A shutdown"]),
    ("c06-url-while-load-off", 3, &["log: A shutdown"]),
    (
        "c07-overlap-frames",
        5,
        &[
            "log: B fraction 0",
            "log: parent tick 2",
            "log: A fraction 5",
            "log: A shutdown",
            "log: C initialize",
        ],
    ),
    (
        "c07-overlap-frames",
        6,
        &["log: C fraction 0", "log: B fraction 2", "log: B shutdown"],
    ),
    (
        "c08-overlap-same-frame",
        5,
        &[
            "log: C fraction 8",
            "log: parent tick 2",
            "log: A fraction 5",
            "log: A shutdown",
        ],
    ),
    (
        "c09-disable-while-loading",
        5,
        &["log: B shutdown", "log: A shutdown", "log: parent tick 2"],
    ),
    (
        "c10-nested",
        7,
        &["log: B fraction 4", "log: A fraction 7", "log: A shutdown"],
    ),
    (
        "c10-nested",
        11,
        &[
            "log: C fraction 7",
            "log: B fraction 2",
            "log: Mid shutdown",
            "log: B shutdown",
        ],
    ),
    (
        "c12-repeat-success",
        4,
        &["log: B fraction 7", "log: A fraction 3", "log: A shutdown"],
    ),
    (
        "c12-repeat-success",
        7,
        &["log: A fraction 7", "log: B fraction 4", "log: B shutdown"],
    ),
    (
        "c12-repeat-success",
        10,
        &[
            "log: B fraction 0",
            "log: A fraction 10",
            "log: parent tick 3",
            "log: A fraction 0",
            "log: A shutdown",
        ],
    ),
    (
        "c12-repeat-success",
        13,
        &["log: A fraction 2", "log: B fraction 5", "log: B shutdown"],
    ),
    (
        "c12-repeat-success",
        16,
        &["log: B fraction 2", "log: A fraction 6", "log: A shutdown"],
    ),
    (
        "c12-repeat-success",
        19,
        &["log: A fraction 8", "log: B fraction 7", "log: B shutdown"],
    ),
    (
        "c13-repeat-failure",
        3,
        &[
            "error: Inline: Couldn't load URL 'http://oracle/synthetic-inline-reload/gone1.wrl'. Error: Not Found",
            "log: A shutdown",
        ],
    ),
    (
        "c14-remove-while-loading",
        5,
        &[
            "log: B fraction 0",
            "log: parent tick 2",
            "log: A fraction 5",
            "log: A shutdown",
        ],
    ),
    (
        "c18-load-now",
        6,
        &["log: A fraction 6", "log: A fraction 6", "log: A shutdown"],
    ),
    (
        "c19-auto-refresh",
        6,
        &["log: B fraction 2", "log: L fraction 6", "log: L shutdown"],
    ),
    (
        "c19-auto-refresh",
        12,
        &["log: B fraction 4", "log: B fraction 4", "log: B shutdown"],
    ),
    (
        "c19-auto-refresh",
        18,
        &["log: B fraction 5", "log: B fraction 5", "log: B shutdown"],
    ),
    (
        "c20-unload-now",
        5,
        &["log: A shutdown", "log: parent tick 2"],
    ),
    ("c21-empty-url", 4, &["log: A shutdown"]),
    (
        "c24-shared",
        6,
        &[
            "log: B fraction 2",
            "log: A fraction 6",
            "log: A fraction 6",
            "log: A shutdown",
        ],
    ),
    (
        "c25-shared-nested",
        7,
        &[
            "log: C fraction 1",
            "log: B fraction 4",
            "log: B fraction 4",
            "log: B shutdown",
        ],
    ),
    (
        "c26-same-url-again",
        6,
        &["log: A fraction 6", "log: A fraction 6", "log: A shutdown"],
    ),
    ("c27-x3d", 9, &["log: B fraction 7", "log: X shutdown"]),
    ("c27-x3d", 11, &["log: B shutdown"]),
    (
        "c28-dispose-nested-while-loading",
        6,
        &[
            "log: B fraction 2",
            "log: Mid tick 3",
            "log: A fraction 6",
            "log: A shutdown",
            "log: C initialize",
        ],
    ),
    (
        "c28-dispose-nested-while-loading",
        7,
        &[
            "log: C fraction 1",
            "log: B fraction 4",
            "log: Mid shutdown",
            "log: B shutdown",
        ],
    ),
    (
        "c30-dispose-nested-loaded",
        7,
        &[
            "log: C fraction 1",
            "log: A fraction 7",
            "log: Mid shutdown",
            "log: A shutdown",
        ],
    ),
    (
        "c36-dispose-nested-queued",
        6,
        &[
            "log: C fraction 0",
            "log: Mid tick 3",
            "log: A fraction 6",
            "log: Mid shutdown",
            "log: A shutdown",
            "log: B initialize",
        ],
    ),
    ("c37-url-and-load-false-same-frame", 4, &["log: A shutdown"]),
];

fn baseline() -> HashMap<String, Value> {
    let text = std::fs::read_to_string(oracle_root().join("synthetic-inline-reload-baseline.json"))
        .expect("baseline");
    let json: Value = serde_json::from_str(&text).unwrap();

    assert_eq!(json["pinned"], "34cdf9f97");

    json["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["case"].as_str().unwrap().to_string(), r.clone()))
        .collect()
}

/// `(case, first step, last step)` of the table entries whose reason is not the `shutdown ()` call: a deliberate
/// difference of another kind (an older request, a late result of a disabled Inline, the Script interests of a
/// disposed scene, no refresh after a failure). They are tested directly by `inline_reload_safety.rs`.
fn other_differences() -> Vec<(String, u64, u64)> {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/synthetic-inline-reload-known.tsv"),
    )
    .unwrap();

    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .filter(|p| !p[4].contains("Script.shutdown ()"))
        .map(|p| {
            (
                p[0].to_string(),
                p[1].parse().unwrap(),
                p[2].parse().unwrap(),
            )
        })
        .collect()
}

#[test]
fn plain_inline_paths_differ_from_x_ite_only_by_the_shutdown_call() {
    let other = other_differences();
    let mut cases: Vec<&str> = other.iter().map(|o| o.0.as_str()).collect();

    cases.dedup();

    // A new reason for a difference is a new decision: it is not added to this list by accident.
    assert_eq!(
        cases,
        [
            "c07-overlap-frames",
            "c09-disable-while-loading",
            "c29-script-interests",
            "c35-auto-refresh-after-failure"
        ]
    );

    let fails = check_shutdown_rule(&baseline(), PINS, &|c, k| {
        other.iter().any(|(n, a, b)| n == c && k >= *a && k <= *b)
    });

    assert!(fails.is_empty(), "{}:\n{}", fails.len(), fails.join("\n"));
}
