//! Plain Inline reload and abort started from inside the completion event of the load, against the pinned
//! X_ITE (16.5.0, 34cdf9f97) and the browser shutdown rule.
//!
//! `oracle/synthetic-inline-abort/` holds four clean-room worlds. A LoadSensor watches an Inline, and a Script
//! that its `isLoaded` drives changes the Inline in the handler, so `set_internal_scene` and `inline_abort` run
//! while the scene they replace is still being set up. The Scripts of the scenes print in `initialize ()` and
//! `shutdown ()`.
//!
//! * `loaded-url`: the url becomes another scene. `loaded-nested-url`: the same, for a scene that holds an Inline of
//!   its own. `loaded-off`: `load` becomes FALSE. `loaded-cycle`: `load` becomes FALSE and TRUE in one handler.
//!
//! `oracle/x_ite-inline.mjs` recorded the cases three times with byte-identical output
//! (`oracle/synthetic-inline-abort-baseline/inline-abort.json`). Rust equals X_ITE in every step in the browser
//! build; the native build differs by the pinned `shutdown ()` lines only (see `inline_reload_shutdown.rs`).
//! `loaded-cycle` pins that X_ITE does not reload at all in the last case, and Rust does not either.

mod inline_common;

use inline_common::{Pin, check_shutdown_rule, oracle_root, set_case_dir};
use serde_json::Value;
use std::collections::HashMap;

const DIR: &str = "synthetic-inline-abort";

/// Native console of every step with a `shutdown` line.
const PINS: &[Pin] = &[
    (
        "loaded-nested-url",
        0,
        &[
            "log: M initialize",
            "log: A initialize",
            "log: A tick 1",
            "log: loaded true #1",
            "log: B initialize",
            "log: B tick 1",
            "log: M shutdown",
            "log: A shutdown",
            "log: loaded true #2",
        ],
    ),
    (
        "loaded-off",
        0,
        &[
            "log: A initialize",
            "log: A tick 1",
            "log: loaded true #1",
            "log: A shutdown",
        ],
    ),
    (
        "loaded-url",
        0,
        &[
            "log: A initialize",
            "log: A tick 1",
            "log: loaded true #1",
            "log: B initialize",
            "log: B tick 1",
            "log: A shutdown",
            "log: loaded true #2",
        ],
    ),
];

fn baseline() -> HashMap<String, Value> {
    let text =
        std::fs::read_to_string(oracle_root().join(format!("{DIR}-baseline/inline-abort.json")))
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

#[test]
fn inline_changed_in_its_completion_event_matches_x_ite() {
    set_case_dir(DIR);

    let base = baseline();

    assert_eq!(base.len(), 4, "four cases");
    assert!(
        base.values().all(|r| r["ok"] == true),
        "every recording finished"
    );

    let fails = check_shutdown_rule(&base, PINS, &|_, _| false);

    assert!(fails.is_empty(), "{}:\n{}", fails.len(), fails.join("\n"));
}

/// The cases do what they claim: X_ITE loaded the first scene, then the new one (or none) in the same step.
#[test]
fn x_ite_runs_the_handler_in_the_load_step() {
    let base = baseline();
    let console = |case: &str| -> Vec<String> {
        base[case]["steps"][0]["console"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap().to_string())
            .collect()
    };

    assert_eq!(
        console("loaded-url"),
        [
            "log: A initialize",
            "log: A tick 1",
            "log: loaded true #1",
            "log: B initialize",
            "log: B tick 1",
            "log: loaded true #2"
        ]
    );
    assert_eq!(
        console("loaded-off"),
        ["log: A initialize", "log: A tick 1", "log: loaded true #1"]
    );
    assert!(
        console("loaded-nested-url")
            .iter()
            .any(|l| l == "log: M initialize")
    );
    // X_ITE: FALSE then TRUE in one handler leaves the loaded scene alone (no unload, no second load).
    assert_eq!(
        console("loaded-cycle"),
        ["log: A initialize", "log: A tick 1", "log: loaded true #1"]
    );
}
