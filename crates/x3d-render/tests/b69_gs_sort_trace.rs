//! Issue #69, the GaussianSplats sort timing: the recording of the pinned X_ITE 16.5.0's sort worker for larger splat counts
//! (oracle/web-browser-splat-sort-01-baseline/splatsort01-trace-counts.json, `oracle/x_ite-splat-sort-trace.mjs`, three
//! identical runs per count). X_ITE posts the sort, draws the frame of a view change with the old order, and draws again when
//! the worker answers: exactly one transient draw at 2, 441, 3969 and 30000 splats. X_ITE_R has no worker and sorts in the
//! frame that draws (a documented difference, web/API.md, decided in #309), so there is nothing to compare it with here;
//! this test pins the recording and fails when it is absent or changed, because the decision rests on it.

use serde_json::Value;

#[test]
fn the_pinned_sort_worker_answers_after_one_transient_draw_at_every_recorded_count() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/web-browser-splat-sort-01-baseline/splatsort01-trace-counts.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("no sort trace recording {}: {e}", path.display()));
    let json: Value = serde_json::from_str(&text).unwrap();

    assert_eq!(json["pinned"], "34cdf9f97");
    assert!(json["runs"].as_u64().unwrap() >= 3);

    let traces = json["traces"].as_object().unwrap();
    let counts: Vec<&str> = traces.keys().map(String::as_str).collect();

    for want in ["2", "441", "3969", "30000"] {
        assert!(counts.contains(&want), "no trace for {want} splats");
    }

    for (n, trace) in traces {
        let got: Vec<&str> = trace
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let draw = format!("draw {n}");

        assert_eq!(
            got,
            ["post sort", draw.as_str(), "recv sorted", draw.as_str()],
            "{n} splats"
        );
    }
}
