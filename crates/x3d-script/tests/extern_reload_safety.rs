//! R0.3-B: what the Rust runtime does where it differs from the pinned X_ITE on purpose
//! (IMPLEMENTATION_SAFETY), and the bounds of auto refresh. The differences are listed, with
//! their reasons, in `synthetic-externproto-reload-known.tsv`; `extern_reload.rs` compares everything else.

mod extern_common;

use extern_common::{big_stack, case_dir, case_names, run_scenario};
use serde_json::{Value, json};

fn trace(name: &str) -> Value {
    let n = name.to_string();

    big_stack(move || run_scenario(&n)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn steps(t: &Value) -> &Vec<Value> {
    t["steps"].as_array().unwrap()
}

fn step(t: &Value, k: usize) -> &Value {
    &steps(t)[k]
}

/// How many console messages of the whole run contain `needle`.
fn count(t: &Value, needle: &str) -> usize {
    steps(t)
        .iter()
        .flat_map(|s| s["console"].as_array().unwrap())
        .filter(|l| l.as_str().unwrap().contains(needle))
        .count()
}

fn state(t: &Value, k: usize) -> &str {
    step(t, k)["externs"][0]["loadState"].as_str().unwrap()
}

/// Is the `body` of the instance `name` at step `k` present?
fn has_body(t: &Value, k: usize, name: &str) -> bool {
    !step(t, k)["instances"][name]["body"].is_null()
}

fn final_vrml(t: &Value) -> String {
    steps(t)
        .iter()
        .rev()
        .find_map(|s| s["vrml"].as_str())
        .unwrap()
        .to_string()
}

/// IMPLEMENTATION_SAFETY: nothing of any load is left behind after any scenario, and an armed
/// auto refresh timer is exactly one (never more, and none unless the scenario turned it on).
#[test]
fn extern_reload_leaves_no_pending_state() {
    for name in case_names() {
        let t = trace(&name);
        let scenario: Value = serde_json::from_str(
            &std::fs::read_to_string(case_dir().join(format!("{name}.json"))).unwrap(),
        )
        .unwrap();
        let refreshes = scenario["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["type"] == "autoRefresh" && a["value"].as_f64().unwrap() > 0.0);

        for (field, value) in t["cleanup"].as_object().unwrap() {
            if field == "timers" || field == "disposedBodies" {
                continue;
            }

            assert!(
                value == &json!(0) || value == &json!(false),
                "{name}: {field} = {value}"
            );
        }

        let timers = t["cleanup"]["timers"].as_u64().unwrap();

        assert!(
            timers <= 1
                && (timers == 1)
                    == (refreshes && name != "c07-refresh" && name != "c07c-refresh-cancelled"),
            "{name}: {timers} auto refresh timers"
        );
    }
}

/// IMPLEMENTATION_SAFETY (X_ITE: the older request replaces the newer one for a frame): a slower
/// older request never realizes anything; the newer request is the result.
#[test]
fn extern_older_request_does_not_replace_newer() {
    let t = trace("c08-overlap-frames");

    assert_eq!(
        count(&t, "B initialize"),
        0,
        "the older request (lib-b) was realized"
    );
    assert_eq!(count(&t, "C initialize"), 2);
    assert_eq!(state(&t, 4), "IN_PROGRESS");
    assert_eq!(
        state(&t, 5),
        "IN_PROGRESS",
        "the newer request is still loading in step 5"
    );
    assert_eq!(state(&t, 6), "COMPLETE");
    assert_eq!(
        step(&t, 16)["externs"][0]["url"],
        json!(["lib-c.wrl#Gadget"])
    );

    // Two requests in one frame make one load (both implementations).
    let t = trace("c09-overlap-same-frame");

    assert_eq!(count(&t, "B initialize"), 0);
    assert_eq!(count(&t, "C initialize"), 2);
}

/// IMPLEMENTATION_SAFETY (X_ITE: a request that was in flight when `load` became FALSE activates the
/// declaration): the declaration stays NOT_STARTED with the PROTO and bodies it had.
#[test]
fn extern_disabled_declaration_ignores_old_request() {
    let t = trace("c10-disable-while-loading");

    assert_eq!(count(&t, "B initialize"), 0);

    for k in 5..=16 {
        assert_eq!(state(&t, k), "NOT_STARTED", "step {k}");
        assert_eq!(step(&t, k)["externs"][0]["proto"], json!("Gadget"));
        assert!(
            has_body(&t, k, "Plain") && has_body(&t, k, "Explicit"),
            "step {k}: the old bodies are gone"
        );
    }

    // The old bodies still answer.
    assert!(count(&t, "A poke 3") == 2 && count(&t, "A poke 4") == 2);
    assert_eq!(step(&t, 16)["loadCount"], json!(0));
}

/// IMPLEMENTATION_SAFETY (X_ITE: the next successful load builds the body of one instance twice).
#[test]
fn extern_reload_after_failure_builds_one_body_per_instance() {
    for name in ["c05-missing", "c16-recover-new-url"] {
        let t = trace(name);

        assert_eq!(state(&t, 4), "FAILED", "{name}");
        assert!(
            !has_body(&t, 4, "Plain") && !has_body(&t, 4, "Explicit"),
            "{name}: a failed load keeps a body"
        );
        assert_eq!(
            count(&t, "B initialize"),
            2,
            "{name}: one body per instance"
        );
        assert_eq!(
            count(&t, "B poke 4"),
            2,
            "{name}: one body per instance answers"
        );
        assert_eq!(count(&t, "mirror explicit 2004"), 1, "{name}");

        // The two bodies of A were disposed when the load failed; each instance built B once.
        assert_eq!(
            t["cleanup"]["disposedBodies"],
            json!(2),
            "{name}: a body was built and thrown away"
        );
        assert!(
            has_body(&t, 24, "Plain") && has_body(&t, 24, "Explicit"),
            "{name}"
        );
    }
}

/// IMPLEMENTATION_SAFETY (X_ITE caches a failed fetch, so the file is never fetched again): a
/// resource that appears later loads, from the same URL.
#[test]
fn extern_recovers_when_the_resource_appears() {
    let t = trace("c17-recover-same-url");

    assert_eq!(state(&t, 8), "FAILED");
    assert_eq!(state(&t, 9), "IN_PROGRESS");
    assert_eq!(state(&t, 10), "COMPLETE");
    assert_eq!(count(&t, "TypeError"), 0);
    assert_eq!(count(&t, "B initialize"), 2);
    assert!(has_body(&t, 24, "Plain") && has_body(&t, 24, "Explicit"));
}

/// IMPLEMENTATION_SAFETY (X_ITE: the load of a removed declaration finishes and replaces the bodies).
#[test]
fn extern_removed_declaration_keeps_no_load() {
    let t = trace("c20-remove-while-loading");

    assert_eq!(count(&t, "B initialize"), 0);
    assert_eq!(step(&t, 5)["externs"], json!([]));
    assert_eq!(step(&t, 16)["loadCount"], json!(0));
    assert!(
        has_body(&t, 16, "Plain") && has_body(&t, 16, "Explicit"),
        "the instances keep their bodies"
    );
    assert_eq!(count(&t, "A poke 4"), 2, "the old bodies keep working");
}

/// IMPLEMENTATION_SAFETY: a reload deletes and re-adds routes, it does not duplicate them.
#[test]
fn extern_reload_keeps_routes_unique() {
    for name in [
        "c02-url-change",
        "c06-load-toggle",
        "c07-refresh",
        "c11-route-interface-change",
        "c12-nested-reload",
    ] {
        let t = trace(name);
        let routes = |v: &str| {
            v.lines()
                .filter(|l| l.starts_with("ROUTE "))
                .map(String::from)
                .collect::<Vec<_>>()
        };
        let last = routes(&final_vrml(&t));
        let mut unique = last.clone();

        unique.sort();
        unique.dedup();
        assert_eq!(
            last.len(),
            unique.len(),
            "{name}: duplicate routes {last:?}"
        );
    }

    // The same set of routes before and after a reload to an implementation with the same interface.
    let t = trace("c02-url-change");
    let first = steps(&t)
        .iter()
        .find_map(|s| s["vrml"].as_str())
        .unwrap()
        .to_string();
    let sorted = |v: &str| {
        let mut r: Vec<String> = v
            .lines()
            .filter(|l| l.starts_with("ROUTE "))
            .map(String::from)
            .collect();

        r.sort();
        r
    };

    assert_eq!(sorted(&first), sorted(&final_vrml(&t)));
}

/// XITE_COMPATIBILITY: the number of reloads that `autoRefresh` and `autoRefreshTimeLimit` allow
/// (a controlled clock: no real time is involved).
#[test]
fn extern_auto_refresh_is_bounded() {
    // autoRefresh 1 s from step 2, limit 2.5 s: a reload at 1.0 s and at 2.0 s, none after.
    let t = trace("c07-refresh");

    assert_eq!(count(&t, "A initialize level=2"), 3);

    // Interval changed to 3 s at 0.6 s: next reload at 3.0 s (step 30), none before; one timer.
    let t = trace("c07b-refresh-interval-change");

    assert_eq!(count(&t, "A initialize level=2"), 2);
    assert!(
        step(&t, 30)["console"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l.as_str().unwrap().contains("A initialize level=2"))
    );

    // autoRefresh set back to 0: the timer is gone.
    let t = trace("c07c-refresh-cancelled");

    assert_eq!(count(&t, "A initialize level=2"), 1);
}
