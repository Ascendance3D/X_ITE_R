//! REA batch 24, issue #52: what happens to the Inlines of a PROTO instance's body (and to the scenes they
//! loaded) when the body is replaced, against the pinned X_ITE (16.5.0, 34cdf9f97).
//!
//! `oracle/synthetic-rea-b24-inline-proto/` holds clean-room worlds. Each PROTO body holds an Inline of a
//! small scene (a TimeSensor that drives a Script, `leaf.wrl`; `nest.wrl` holds an Inline of it), a
//! LoadSensor on that Inline and an observer Script whose `initialize`, `shutdown` and events print.
//! `oracle/x_ite-extern.mjs` recorded every case three times with byte-identical output
//! (`oracle/synthetic-rea-b24-inline-proto-baseline.json`). The scenario runner is the one of
//! `extern_reload.rs` (`extern_common`).
//!
//! * `c01`, `c02`: `updateInstances` of a PROTO declaration replaces the body of a live instance whose
//!   (nested) Inline has loaded. `c03`: two updates in one tick, the second disposes a body whose Inline load
//!   has only just been asked for. `c04`: the instance leaves its parent.
//! * `c05`, `c06`: an EXTERNPROTO url swap; `c07`: `load` FALSE, then TRUE; `c08`: `loadNow`.
//!
//! What X_ITE does: the old body's nodes are never disposed one by one. Its scenes stop (its TimeSensors no
//! longer tick) but no Script gets `shutdown ()`; the new body builds its Inline and loads it again. Rust
//! equals that in every step but the ones in the two tables below. It is checked twice: the browser build
//! (`replace_skips_shutdown`), which follows X_ITE, and the native build, which keeps the `shutdown ()` of
//! ISO/IEC 14772-1 4.12.3 (the owner-approved native difference, `Runtime::replace_skips_shutdown`).

mod extern_common;

use extern_common::{big_stack, case_names_in, oracle_root, run_scenario_with};
use serde_json::Value;
use std::collections::HashMap;

const DIR: &str = "synthetic-rea-b24-inline-proto";

/// The build modes of the runtime: the browser build (follows X_ITE in `shutdown ()`), and the native one.
const MODES: [(&str, bool); 2] = [("browser", true), ("native", false)];

/// `mode ("*" = both), case ("*" = every case), step, reason`: a step whose console differs on purpose.
const KNOWN_CONSOLE: &[(&str, &str, u64, &str)] = &[
    // The page of the X_ITE recorder runs two extra frames while the world loads (more `leaf time` lines), and a
    // LoadSensor reports `isActive` before the Inline's scene has been built there, after it here. Only the world's
    // first load is affected (the same LoadSensor lines agree in every later step). Not under test in this lane.
    (
        "*",
        "*",
        0,
        "initial load: the page's extra frames and LoadSensor order",
    ),
    // The native build gives the Scripts of a replaced body `shutdown ()`.
    // c03: the second update disposes the first update's body before its Scripts initialized; the native build
    // shuts that body down first, so its observer Script never initializes (one `obs initialize` less).
    (
        "native",
        "c03",
        2,
        "native shutdown () of a body replaced before it initialized",
    ),
    ("native", "c05", 5, "native shutdown () of a replaced body"),
    ("native", "c06", 5, "native shutdown () of a replaced body"),
    ("native", "c07", 7, "native shutdown () of a replaced body"),
    ("native", "c08", 4, "native shutdown () of a replaced body"),
];

/// `case, step`: the root scene's `loadCount` differs. X_ITE's counter stays at 1 for the step in which a body
/// that an EXTERNPROTO built loaded its Inline (and for the 2nd step of a nested one); Rust's is 0 there. The
/// counter is internal (a LoadSensor on the Inline, the Inline's Scripts and the scene text agree in all steps),
/// and the first case is the in-flight load of `c03`.
const KNOWN_LOAD_COUNT: &[(&str, u64)] = &[
    ("c05", 5),
    ("c05", 6),
    ("c06", 5),
    ("c06", 6),
    ("c07", 7),
    ("c08", 4),
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

fn run(name: &str, browser: bool) -> Value {
    let n = name.to_string();

    big_stack(move || run_scenario_with(DIR, &n, move |rt| rt.replace_skips_shutdown = browser))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn lines(step: &Value) -> Vec<String> {
    step["console"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn rea_b24_inline_proto_matches_x_ite() {
    let base = baseline();
    let names = case_names_in(DIR);
    let mut unexplained = vec![];
    let mut used_console = vec![false; KNOWN_CONSOLE.len()];
    let mut used_count = vec![false; KNOWN_LOAD_COUNT.len()];

    assert_eq!(names.len(), base.len(), "every case has a baseline");

    for (mode, browser) in MODES {
        for name in &names {
            let actual = run(name, browser);
            let (es, as_) = (
                base[name]["steps"].as_array().unwrap(),
                actual["steps"].as_array().unwrap(),
            );

            assert_eq!(es.len(), as_.len(), "{name}: step count");

            let (mut ev, mut av) = (String::new(), String::new());

            for (e, a) in es.iter().zip(as_) {
                let k = e["k"].as_u64().unwrap();

                if lines(e) != lines(a) {
                    let hit = KNOWN_CONSOLE.iter().position(|(m, c, s, _)| {
                        (*m == "*" || *m == mode) && (*c == "*" || c == name) && *s == k
                    });

                    match hit {
                        Some(i) => used_console[i] = true,
                        None => unexplained.push(format!(
                            "{mode} {name} step {k} console:\n  x_ite {:?}\n  rust  {:?}",
                            lines(e),
                            lines(a)
                        )),
                    }
                }

                if e["loadCount"] != a["loadCount"] {
                    match KNOWN_LOAD_COUNT
                        .iter()
                        .position(|(c, s)| c == name && *s == k)
                    {
                        Some(i) => used_count[i] = true,
                        None => unexplained.push(format!(
                            "{mode} {name} step {k} loadCount: x_ite {} rust {}",
                            e["loadCount"], a["loadCount"]
                        )),
                    }
                }

                if let Some(v) = e["vrml"].as_str() {
                    ev = v.to_string();
                }

                if let Some(v) = a["vrml"].as_str() {
                    av = v.to_string();
                }

                for field in ["externs", "instances"] {
                    if e[field] != a[field] {
                        unexplained.push(format!(
                            "{mode} {name} step {k} {field}:\n  x_ite {}\n  rust  {}",
                            e[field], a[field]
                        ));
                    }
                }

                if ev != av {
                    unexplained.push(format!(
                        "{mode} {name} step {k} vrml:\n  x_ite {ev}\n  rust  {av}"
                    ));
                }
            }
        }
    }

    // A known entry that nothing needed any more is stale; "native" entries are only hit by the native run.
    let stale: Vec<_> = KNOWN_CONSOLE
        .iter()
        .zip(&used_console)
        .filter(|(_, u)| !**u)
        .map(|(k, _)| format!("{} {} {}", k.0, k.1, k.2))
        .chain(
            KNOWN_LOAD_COUNT
                .iter()
                .zip(&used_count)
                .filter(|(_, u)| !**u)
                .map(|(k, _)| format!("loadCount {} {}", k.0, k.1)),
        )
        .collect();

    assert!(
        unexplained.is_empty() && stale.is_empty(),
        "unexplained differences ({}):\n{}\nstale known entries: {stale:?}",
        unexplained.len(),
        unexplained.join("\n")
    );
}

/// The browser build does not call `shutdown ()` for a body that an EXTERNPROTO replaces (X_ITE logs none), the
/// native build does, for every Script of the old body and of the Inline scenes inside it.
#[test]
fn replaced_body_shutdown_follows_the_build() {
    let (browser, native) = (run("c06", true), run("c06", false));
    let step5 = |v: &Value| lines(&v["steps"][5]);
    let shut = |l: &[String]| {
        l.iter()
            .filter(|s| s.contains("shutdown"))
            .cloned()
            .collect::<Vec<_>>()
    };

    assert!(shut(&step5(&browser)).is_empty(), "{:?}", step5(&browser));
    assert_eq!(
        shut(&step5(&native)),
        [
            "log: obs shutdown load=true",
            "log: nest shutdown",
            "log: leaf shutdown"
        ]
    );
    // X_ITE's body is never shut down: the baseline has none either.
    let base = baseline();
    let all: Vec<String> = base["c06"]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(lines)
        .collect();

    assert!(all.iter().all(|l| !l.contains("shutdown")));
}

/// X_ITE finishes the load of an Inline whose body is already disposed (its Scripts initialize); so does Rust
/// (issue #58, owner decision option B: the cancel was dropped).
#[test]
fn disposed_body_inline_load_in_flight() {
    let base = baseline();
    let rust = run("c03", true);
    let count = |steps: &Value, s: usize, what: &str| {
        lines(&steps[s]).iter().filter(|l| l.contains(what)).count()
    };
    let (x, r) = (&base["c03"]["steps"], &rust["steps"]);

    assert_eq!(count(x, 2, "leaf initialize"), 2, "X_ITE");
    assert_eq!(count(r, 2, "leaf initialize"), 2, "Rust");
}
