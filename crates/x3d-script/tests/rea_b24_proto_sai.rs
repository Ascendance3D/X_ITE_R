//! REA batch 24, issue #78: the SAI proto API (`createProto`, the declaration objects and their field
//! updates) against the pinned X_ITE (16.5.0, 34cdf9f97).
//!
//! `oracle/synthetic-rea-b24-proto-sai/` holds clean-room worlds with a Script that exercises the API
//! and prints each result (a thrown error prints its name and message); `oracle/x_ite-extern.mjs`
//! recorded every case three times with byte-identical output
//! (`oracle/synthetic-rea-b24-proto-sai-baseline.json`). The scenario runner is the one of
//! `extern_reload.rs` (`extern_common`).

mod extern_common;

use extern_common::{big_stack, case_names_in, oracle_root, run_scenario_with};
use serde_json::Value;
use std::collections::HashMap;

const DIR: &str = "synthetic-rea-b24-proto-sai";

/// The build modes of the runtime: the browser build follows X_ITE in `shutdown ()` (a replaced body's Scripts are
/// not shut down), the native one gives them `shutdown ()` (the owner-approved native difference).
const MODES: [(&str, bool); 2] = [("browser", true), ("native", false)];

/// `mode, case, X_ITE console line, reason`: a deliberate difference of Rust.
const KNOWN: &[(&str, &str, &str, &str)] = &[(
    "native",
    "p08",
    "log: Body initialize level=2",
    // X_ITE: a Script requests its load when it initializes and a disposal does not cancel it, so the body that
    // the update disposed before its Script's load finished still runs `initialize ()`. The native build calls
    // `shutdown ()` on the disposed body first, which cancels that (the kept shutdown difference); the browser
    // build matches X_ITE.
    "native shutdown () of a body disposed before its Script initialized",
)];

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

fn run(name: &str, browser: bool) -> Value {
    let n = name.to_string();

    big_stack(move || run_scenario_with(DIR, &n, move |rt| rt.replace_skips_shutdown = browser))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn rea_b24_proto_sai_matches_x_ite() {
    let base = baseline();
    let names = case_names_in(DIR);
    let mut unexplained = vec![];
    let mut used = vec![false; KNOWN.len()];

    assert_eq!(names.len(), base.len(), "every case has a baseline");

    for (mode, browser) in MODES {
        for name in &names {
            let actual = run(name, browser);

            if std::env::var_os("X3D_DUMP_CONSOLE").is_some() {
                for s in actual["steps"].as_array().unwrap() {
                    eprintln!("DUMP {mode} {name} {} {}", s["k"], s["console"]);
                }
            }
            let (es, as_) = (
                base[name]["steps"].as_array().unwrap(),
                actual["steps"].as_array().unwrap(),
            );

            assert_eq!(es.len(), as_.len(), "{name}: step count");

            // The console of the whole case, line by line, so one differing line names itself. A Script that
            // prints from a clock event is not aligned by step: X_ITE's page runs two frames while it loads.
            let flat = |steps: &[Value]| -> Vec<String> {
                steps
                    .iter()
                    .flat_map(|s| s["console"].as_array().unwrap().iter())
                    .map(|l| l.as_str().unwrap().to_string())
                    .collect()
            };
            let (ec, ac) = (flat(es), flat(as_));

            for i in 0..ec.len().max(ac.len()) {
                let x = ec.get(i).map(String::as_str).unwrap_or("(none)");
                let r = ac.get(i).map(String::as_str).unwrap_or("(none)");

                if x != r {
                    let line = (x.to_string(), r.to_string());

                    if let Some(i) = KNOWN
                        .iter()
                        .position(|(m, c, l, _)| *m == mode && c == name && *l == line.0)
                    {
                        used[i] = true;
                    } else {
                        unexplained.push(format!(
                            "{mode} {name} console line {i}:\n  x_ite {x}\n  rust  {r}"
                        ));
                    }
                }
            }

            let (mut ev, mut av) = (String::new(), String::new());

            for (e, a) in es.iter().zip(as_) {
                let k = e["k"].as_u64().unwrap();

                if let Some(v) = e["vrml"].as_str() {
                    ev = v.to_string();
                }

                if let Some(v) = a["vrml"].as_str() {
                    av = v.to_string();
                }

                let mut diffs: Vec<(&str, String, String)> = vec![];

                for field in ["loadCount", "externs", "instances"] {
                    if e[field] != a[field] {
                        diffs.push((field, e[field].to_string(), a[field].to_string()));
                    }
                }

                if ev != av {
                    diffs.push(("vrml", ev.clone(), av.clone()));
                }

                for (field, x, r) in diffs {
                    unexplained.push(format!(
                        "{mode} {name} step {k} {field}:\n  x_ite {x}\n  rust  {r}"
                    ));
                }
            }
        }
    }

    let stale: Vec<_> = KNOWN
        .iter()
        .zip(&used)
        .filter(|(_, u)| !**u)
        .map(|(k, _)| (k.0, k.1))
        .collect();

    assert!(
        unexplained.is_empty() && stale.is_empty(),
        "unexplained differences ({}):\n{}\nstale known entries: {stale:?}",
        unexplained.len(),
        unexplained.join("\n")
    );
}
