//! REA batch 24, issue #58: the live state of EXTERNPROTO declarations that share a cached scene, and a
//! declaration whose context is disposed while its load is in flight, against the pinned X_ITE
//! (16.5.0, 34cdf9f97).
//!
//! `oracle/synthetic-rea-b24-externproto-live/` holds clean-room worlds and one scenario file per case;
//! `oracle/x_ite-extern.mjs` recorded each step of each case five times with byte-identical output
//! (`oracle/synthetic-rea-b24-externproto-live-baseline.json`). The scenario runner is the one of
//! `extern_reload.rs` (`extern_common`), pointed at the new case directory.
//!
//! * `l01`, `l02`: two declarations with one URL share one cached scene. Each reloads, stops loading,
//!   changes URL (to a cached and to a new scene, with a changed interface) on its own; explicit field
//!   values, routes and the other declaration's instances stay live. Rust equals X_ITE in every step.
//! * `d1` to `d4`: an Inline scene that holds an EXTERNPROTO is disposed (`load` FALSE, or a new `url`)
//!   1 to 4 ticks after it was asked for. When the disposal comes while the declaration's scene is
//!   still in flight, X_ITE lets the load finish into the disposed scene (the PROTO body is built and its
//!   Script's `initialize` runs); Rust cancels the declaration (`Runtime::extern_close`). That is the one
//!   known difference, listed below and asserted directly in `disposed_context_cancels_the_load`.

mod extern_common;

use extern_common::{big_stack, case_names_in, oracle_root, run_scenario_in};
use serde_json::Value;
use std::collections::HashMap;

const DIR: &str = "synthetic-rea-b24-externproto-live";

/// `case, first step, last step, fields`: differences in these fields in these steps are the deliberate
/// difference of Rust. The disposal comes one tick after the Inline scene was asked for, while the
/// declaration inside it is still loading: X_ITE finishes that load into the disposed scene and builds the
/// instance body (`A initialize level=9` is logged although the scene is gone); Rust cancels the load
/// (`Runtime::extern_close`) and logs nothing. Awaiting the owner's decision (register, issue #63).
const KNOWN: &[(&str, u64, u64, &[&str])] = &[
    ("d1-dispose-load", 4, 4, &["console"]),
    ("d1-dispose-url", 4, 4, &["console"]),
];

fn baseline() -> HashMap<String, Value> {
    let text = std::fs::read_to_string(oracle_root().join(format!("{DIR}-baseline.json")))
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

fn run(name: &str) -> Value {
    let n = name.to_string();
    big_stack(move || run_scenario_in(DIR, &n)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// Differences of one case as `(step, field, x_ite, rust)`.
fn compare(
    name: &str,
    expected: &Value,
    actual: &Value,
) -> Vec<(u64, &'static str, String, String)> {
    let (es, as_) = (
        expected["steps"].as_array().unwrap(),
        actual["steps"].as_array().unwrap(),
    );

    assert_eq!(es.len(), as_.len(), "{name}: step count");

    let (mut ev, mut av) = (String::new(), String::new());
    let mut out = vec![];

    for (e, a) in es.iter().zip(as_) {
        let k = e["k"].as_u64().unwrap();

        if let Some(v) = e["vrml"].as_str() {
            ev = v.to_string();
        }

        if let Some(v) = a["vrml"].as_str() {
            av = v.to_string();
        }

        for field in ["loadCount", "console", "externs", "instances"] {
            if e[field] != a[field] {
                out.push((k, field, e[field].to_string(), a[field].to_string()));
            }
        }

        if ev != av {
            out.push((k, "vrml", "(scene text)".into(), "(scene text)".into()));
        }
    }

    out
}

fn known(name: &str, k: u64, field: &str) -> bool {
    KNOWN.iter().any(|(c, first, last, fields)| {
        *c == name && k >= *first && k <= *last && fields.contains(&field)
    })
}

#[test]
fn rea_b24_externproto_live_matches_x_ite() {
    let base = baseline();
    let names = case_names_in(DIR);
    let mut unexplained = vec![];
    let mut used = vec![false; KNOWN.len()];

    assert_eq!(names.len(), base.len(), "every case has a baseline");

    for name in &names {
        let actual = run(name);

        for (k, field, x, r) in compare(name, &base[name], &actual) {
            if known(name, k, field) {
                for (i, (c, first, last, fields)) in KNOWN.iter().enumerate() {
                    if c == name && k >= *first && k <= *last && fields.contains(&field) {
                        used[i] = true;
                    }
                }
            } else {
                unexplained.push(format!(
                    "{name} step {k} {field}:\n  x_ite {x}\n  rust  {r}"
                ));
            }
        }
    }

    let stale: Vec<_> = KNOWN
        .iter()
        .zip(&used)
        .filter(|(_, u)| !**u)
        .map(|(k, _)| k.0)
        .collect();

    assert!(
        unexplained.is_empty() && stale.is_empty(),
        "unexplained differences ({}):\n{}\nstale known entries: {stale:?}",
        unexplained.len(),
        unexplained
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

fn lines(trace: &Value, needle: &str) -> Vec<u64> {
    trace["steps"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|s| {
            let k = s["k"].as_u64().unwrap();

            s["console"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|l| l.as_str().unwrap().contains(needle))
                .map(move |_| k)
        })
        .collect()
}

/// The disposed-context cancel, pinned: what X_ITE does and what Rust does instead, side by side.
#[test]
fn disposed_context_cancels_the_load() {
    let base = baseline();
    let body = "A initialize level=9";

    for mode in ["load", "url"] {
        let name = format!("d1-dispose-{mode}");
        let rust = run(&name);

        // X_ITE: the load that was in flight finishes into the disposed scene (step 4, after the
        // disposal at step 3) and the instance body is built.
        assert_eq!(
            lines(&base[&name], body),
            vec![4],
            "{name}: X_ITE builds the body once"
        );

        // Rust: the declaration was closed with its context; the body is never built.
        assert!(lines(&rust, body).is_empty(), "{name}: Rust builds no body");

        // Nothing is left behind.
        let c = &rust["cleanup"];

        assert_eq!(c["unfinishedLoads"], false, "{name}");

        for key in [
            "pendingScenes",
            "loadEvents",
            "inProgress",
            "pendingCache",
            "timers",
            "tasks",
        ] {
            assert_eq!(c[key], 0, "{name}: {key}");
        }
    }

    // The disposal one tick later, or later, finds the declaration loaded: X_ITE and Rust agree
    // (covered step by step by `rea_b24_externproto_live_matches_x_ite`), and the body is built once.
    for mode in ["load", "url"] {
        for k in 2..=4 {
            let name = format!("d{k}-dispose-{mode}");

            assert_eq!(lines(&run(&name), body), vec![4], "{name}");
        }
    }
}

/// Declarations that share a cached scene stay independent and live.
#[test]
fn shared_scene_declarations_stay_independent() {
    let t = run("l01-shared-declaration");
    let state = |k: usize, i: usize| -> (String, String) {
        let e = &t["steps"][k]["externs"][i];

        (
            e["url"][0].as_str().unwrap().to_string(),
            e["loadState"].as_str().unwrap().to_string(),
        )
    };

    // Step 4: First reloaded to lib-b, Second still lib-a.
    assert_eq!(state(4, 0).0, "lib-b.wrl#Gadget");
    assert_eq!(state(4, 1).0, "lib-a.wrl#Gadget");
    assert_eq!(state(4, 1).1, "COMPLETE");

    // The instance of Second keeps its explicit value across every reload (B initialize level=5 ...).
    assert!(lines(&t, "B initialize level=5").len() >= 2);

    // First stopped loading at 15: the url change at 16 starts nothing until `load` is TRUE again at 18.
    assert_eq!(state(16, 0).1, "NOT_STARTED");
    assert_eq!(state(17, 0).1, "NOT_STARTED");
    assert_eq!(state(18, 0).1, "COMPLETE");
}
