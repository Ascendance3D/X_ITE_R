//! R0.3-C: Inline reload and load state, step by step against the pinned X_ITE.
//!
//! `oracle/synthetic-inline-reload/` holds clean-room worlds and one scenario file per case
//! (`<case>.json`: the world, the frame count, and the actions an embedding application performs on
//! an Inline: set `url`, `load`, `autoRefresh`, `autoRefreshTimeLimit`, `loadNow`, `unloadNow`,
//! remove it from its Group, publish a file). `oracle/x_ite-inline.mjs` runs each scenario in the
//! pinned X_ITE and records, for every step, the console messages, the state and scene of each named
//! Inline and the scene text (`oracle/synthetic-inline-reload-baseline.json`).
//! `inline_common::run_scenario` does the same with the Rust runtime and the two traces are compared
//! step by step.
//!
//! Each scenario carries its authority: `VRML97_REQUIRED`, `X3D_REQUIRED`, `XITE_COMPATIBILITY` or
//! `IMPLEMENTATION_SAFETY` (see the report). A difference from X_ITE is allowed only if
//! `synthetic-inline-reload-known.tsv` names it, with the reason, and `inline_reload_safety.rs`
//! asserts what Rust does instead.

mod inline_common;

use inline_common::{big_stack, case_names, oracle_root, run_scenario_with};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

type Diff = (&'static str, String, String);

fn diff_step(expected: &Value, actual: &Value, exp_vrml: &str, act_vrml: &str) -> Vec<Diff> {
    let mut out = vec![];

    for field in ["loadCount", "console", "inlines"] {
        if expected[field] != actual[field] {
            out.push((
                field,
                expected[field].to_string(),
                actual[field].to_string(),
            ));
        }
    }

    if exp_vrml != act_vrml {
        let (e, a): (Vec<&str>, Vec<&str>) = (
            exp_vrml.split('\n').collect(),
            act_vrml.split('\n').collect(),
        );
        let i = (0..e.len().max(a.len()))
            .find(|&i| e.get(i) != a.get(i))
            .unwrap_or(0);

        out.push((
            "vrml",
            format!("line {}: {:?}", i + 1, e.get(i)),
            format!("{:?}", a.get(i)),
        ));
    }

    out
}

/// `case<TAB>first<TAB>last<TAB>fields<TAB>reason`: differences in these fields in these steps are expected.
fn known() -> Vec<(String, u64, u64, Vec<String>, String)> {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/synthetic-inline-reload-known.tsv"),
    )
    .unwrap_or_default();

    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let p: Vec<&str> = l.split('\t').collect();
            (
                p[0].to_string(),
                p[1].parse().unwrap(),
                p[2].parse().unwrap(),
                p[3].split(',').map(String::from).collect(),
                p.get(4).unwrap_or(&"").to_string(),
            )
        })
        .collect()
}

fn baseline() -> HashMap<String, Value> {
    let text = std::fs::read_to_string(oracle_root().join("synthetic-inline-reload-baseline.json"))
        .expect("baseline");
    let json: Value = serde_json::from_str(&text).unwrap();

    json["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["case"].as_str().unwrap().to_string(), r.clone()))
        .collect()
}

/// Compare one case; returns the differences as `(step, field, x_ite, rust)`.
fn compare_case(
    name: &str,
    expected: &Value,
    browser: bool,
) -> Vec<(u64, &'static str, String, String)> {
    let actual = {
        let n = name.to_string();
        big_stack(move || run_scenario_with(&n, move |rt| rt.replace_skips_shutdown = browser))
            .unwrap_or_else(|e| panic!("{name}: {e}"))
    };

    let (es, as_) = (
        expected["steps"].as_array().unwrap(),
        actual["steps"].as_array().unwrap(),
    );
    let mut diffs = vec![];

    assert_eq!(es.len(), as_.len(), "{name}: step count");

    let (mut ev, mut av) = (String::new(), String::new());

    for (e, a) in es.iter().zip(as_) {
        if let Some(v) = e["vrml"].as_str() {
            ev = v.to_string();
        }

        if let Some(v) = a["vrml"].as_str() {
            av = v.to_string();
        }

        for (f, x, r) in diff_step(e, a, &ev, &av) {
            diffs.push((e["k"].as_u64().unwrap(), f, x, r));
        }
    }

    diffs
}

/// Every step of every case equals the pinned X_ITE, except for the differences listed (with the
/// reason) in `synthetic-inline-reload-known.tsv`. Cases the oracle did not run (`skipped`) are
/// checked by `inline_reload_safety.rs`.
#[test]
fn synthetic_inline_reload_matches_x_ite() {
    check_all(false);
}

/// The browser build (`Runtime::replace_skips_shutdown`, issues #52 and #202) follows X_ITE when an Inline
/// unloads or reloads: no Script of the old scene gets `shutdown ()`. The entries of the table that exist only for
/// that call are left out, so every step of every case must equal the pinned X_ITE without them.
#[test]
fn synthetic_inline_reload_browser_build_matches_x_ite_shutdown() {
    check_all(true);
}

fn check_all(browser: bool) {
    let (base, mut known) = (baseline(), known());

    if browser {
        let before = known.len();

        known.retain(|k| !k.4.contains("Script.shutdown ()"));
        assert!(before - known.len() >= 30, "the shutdown entries are there");
    }

    let mut unexplained = vec![];
    let mut used = vec![false; known.len()];
    let mut exact = 0;
    let mut compared = 0;

    for name in case_names() {
        let expected = base
            .get(&name)
            .unwrap_or_else(|| panic!("{name}: no baseline (run oracle/x_ite-inline.mjs)"));

        if expected["skipped"].is_string() {
            continue;
        }

        compared += 1;

        let diffs = compare_case(&name, expected, browser);

        if diffs.is_empty() {
            exact += 1;
        }

        for (k, field, x, r) in diffs {
            match known.iter().position(|(c, first, last, fields, _)| {
                *c == name && k >= *first && k <= *last && fields.iter().any(|f| f == field)
            }) {
                Some(i) => used[i] = true,
                None => unexplained.push(format!(
                    "{name} step {k} {field}:\n  x_ite {x}\n  rust  {r}"
                )),
            }
        }
    }

    let stale: Vec<String> = known
        .iter()
        .zip(&used)
        .filter(|(_, u)| !**u)
        .map(|(k, _)| format!("{} steps {}-{}", k.0, k.1, k.2))
        .collect();

    if std::env::var("X3D_SUMMARY").is_ok() {
        for u in &unexplained {
            println!(
                "DIFF {}",
                if std::env::var("X3D_SUMMARY").is_ok_and(|v| v == "full") {
                    u.as_str()
                } else {
                    u.lines().next().unwrap_or("")
                }
            );
        }
    }

    assert!(
        unexplained.len() + stale.len() == 0,
        "unexplained differences ({}):\n{}\nstale known entries: {:?}",
        unexplained.len(),
        unexplained
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n"),
        stale
    );
    println!("{exact} of {compared} cases match exactly");
}
