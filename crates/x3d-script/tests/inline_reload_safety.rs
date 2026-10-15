//! R0.3-C: what this port does where the pinned X_ITE is unsafe, and the cleanup every Inline
//! scenario owes (see the header of `src/inline.rs` and `synthetic-inline-reload-known.tsv`).
//! These tests assert the Rust behavior directly; they do not compare with X_ITE.

mod inline_common;

use inline_common::*;
use serde_json::{Value, json};

fn url(frame: u64, name: &str, urls: &[&str]) -> Value {
    action(frame, "url", name, json!({ "url": urls }))
}

fn load(frame: u64, name: &str, value: bool) -> Value {
    action(frame, "load", name, json!({ "value": value }))
}

fn on_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    big_stack(f)
}

/// Nothing of any request or scene stays behind when a scenario ends: no load in progress, no scene
/// that waits for a frame, no live scene that no Inline shows, no queued task.
#[test]
fn every_scenario_leaves_nothing_behind() {
    on_big_stack(|| {
        for name in case_names() {
            let result = run_scenario(&name).unwrap_or_else(|e| panic!("{name}: {e}"));
            let c = &result["cleanup"];
            let last = result["steps"].as_array().unwrap().last().unwrap();
            let shown = last["inlines"].as_array().unwrap().iter().filter(|i| i["loaded"] == true).count();

            assert_eq!(c["unfinishedLoads"], false, "{name}");
            assert_eq!(c["pendingScenes"], 0, "{name}");
            assert_eq!(c["frameWaiters"], 0, "{name}");
            assert_eq!(c["tasks"], 0, "{name}");
            assert_eq!(c["orphanLiveScenes"], 0, "{name}: a live scene that no Inline shows");
            assert_eq!(c["liveScenes"].as_u64().unwrap(), shown as u64, "{name}: live scenes and shown scenes");

            // Only the scenarios that set autoRefresh keep a timer.
            let timers = if name == "c19-auto-refresh" { 1 } else { 0 };
            assert_eq!(c["timers"], timers, "{name}");
        }
    });
}

/// Overlapping requests: the older request finishes after the newer one was made. X_ITE shows the
/// older scene for a frame; this port never shows it, and keeps the old content until the newest
/// scene is ready.
#[test]
fn overlapping_request_never_shows_the_older_scene() {
    on_big_stack(|| {
        let (run, steps) = trace("main-one.wrl", 12, &[url(4, "Inl", &["content-b.wrl"]), url(5, "Inl", &["content-c.wrl"])]);
        let all = lines(&steps, 0, 12);

        assert!(all.iter().any(|l| l == "log: C fraction 0"), "C is shown");
        assert!(!all.iter().any(|l| l.starts_with("log: B fraction")), "B was never shown: {all:?}");

        // A stays while the newest request is in progress, and the Inline is IN_PROGRESS the whole time.
        for k in 4..=5 {
            let i = inline_of(&steps[k], "Inl");

            assert_eq!(i["loadState"], "IN_PROGRESS", "step {k}");
            assert_eq!(i["loaded"], true, "step {k}: the old scene stays");
        }

        assert_eq!(inline_of(&steps[12], "Inl")["loadState"], "COMPLETE");
        assert_eq!(run.rt.inline_diagnostics().orphan_live_scenes, 0);
    });
}

/// `load` FALSE while a request is in flight: the Inline is empty and NOT_STARTED, the late result
/// is dropped (X_ITE shows it), and `load` TRUE loads again.
#[test]
fn disabled_inline_drops_the_late_result_and_recovers() {
    on_big_stack(|| {
        let (run, steps) = trace("main-one.wrl", 14, &[url(4, "Inl", &["content-b.wrl"]), load(5, "Inl", false), load(9, "Inl", true)]);

        for k in 5..=8 {
            let i = inline_of(&steps[k], "Inl");

            assert_eq!(i["loadState"], "NOT_STARTED", "step {k}");
            assert_eq!(i["loaded"], false, "step {k}: nothing is shown");
            assert_eq!(i["children"], 0, "step {k}");
        }

        assert!(!lines(&steps, 5, 8).iter().any(|l| l.starts_with("log: B fraction")), "the late B never ran");
        assert_eq!(inline_of(&steps[9], "Inl")["loadState"], "IN_PROGRESS");
        assert_eq!(inline_of(&steps[14], "Inl")["loadState"], "COMPLETE");
        assert!(lines(&steps, 9, 14).iter().any(|l| l == "log: B fraction 2"), "B runs after load TRUE");
        assert_eq!(run.rt.inline_diagnostics().orphan_live_scenes, 0);
    });
}

/// Request cancellation: `loadNow` and `unloadNow` in the same frame leave nothing loading. The
/// `loadData` event of the cancelled request is processed and does nothing (X_ITE loads the scene).
#[test]
fn unload_now_cancels_a_request_in_the_same_frame() {
    on_big_stack(|| {
        let (run, steps) = trace("main-off.wrl", 8, &[load(2, "Inl", true), action(4, "loadNow", "Inl", json!({})), action(4, "unloadNow", "Inl", json!({}))]);

        // main-off.wrl starts with load FALSE; load TRUE at frame 2 loads A; at frame 4 the load is restarted and cancelled.
        assert_eq!(inline_of(&steps[3], "Inl")["loadState"], "COMPLETE");

        let i = inline_of(&steps[4], "Inl");

        assert_eq!(i["loadState"], "NOT_STARTED");
        assert_eq!(i["loaded"], false);
        assert_eq!(i["children"], 0);

        // Step 5 onwards: nothing starts by itself.
        for k in 5..=8 {
            assert_eq!(inline_of(&steps[k], "Inl")["loadState"], "NOT_STARTED", "step {k}");
        }

        assert_eq!(run.rt.inline_diagnostics().live_scenes, 0);
        assert!(!run.rt.has_unfinished_loads());
    });
}

/// Cyclic scenes: a scene that loads itself, and two scenes that load each other, fail with one
/// error each instead of loading without end.
#[test]
fn cyclic_inline_fails_instead_of_loading_without_end() {
    on_big_stack(|| {
        for (name, expect_state_of) in [("c22-cycle-self", "Inl/Again"), ("c23-cycle-pair", "Inl/ToB/ToA")] {
            let result = run_scenario(name).unwrap();
            let steps = result["steps"].as_array().unwrap();
            let last = steps.last().unwrap();
            let errors: Vec<&str> = steps.iter().flat_map(|s| s["console"].as_array().unwrap()).filter_map(|l| l.as_str()).filter(|l| l.contains("Cyclic Inline")).collect();

            assert_eq!(errors.len(), 1, "{name}: {errors:?}");
            assert!(errors[0].starts_with("error: Inline: Couldn't load URL '"), "{name}: {}", errors[0]);
            assert_eq!(inline_of(last, expect_state_of)["loadState"], "FAILED", "{name}");
            assert_eq!(result["cleanup"]["unfinishedLoads"], false, "{name}");

            // The scenes above the cycle are loaded and live.
            assert_eq!(inline_of(last, "Inl")["loadState"], "COMPLETE", "{name}");
        }
    });
}

/// A file used in two places is shared use, not a cycle: both Inlines of `main-shared.wrl` load the
/// same file, and a scene holds the same leaf file twice.
#[test]
fn shared_resources_are_not_cycles() {
    on_big_stack(|| {
        let (_, steps) = trace("main-shared.wrl", 6, &[]);
        let last = steps.last().unwrap();

        assert_eq!(inline_of(last, "First")["loadState"], "COMPLETE");
        assert_eq!(inline_of(last, "Second")["loadState"], "COMPLETE");

        let (_, steps) = trace("main-shared-nested.wrl", 8, &[]);
        let last = steps.last().unwrap();

        for p in ["Inl", "Inl/Left", "Inl/Right"] {
            assert_eq!(inline_of(last, p)["loadState"], "COMPLETE", "{p}");
        }

        assert!(!lines(&steps, 0, 8).iter().any(|l| l.contains("Cyclic")));
    });
}

/// A scene that is disposed takes the loads below it along: the nested Inline's request is cancelled
/// and its scene never goes live, and a Script of the scene gets no more `prepareEvents` or
/// `eventsProcessed` (X_ITE keeps calling both).
#[test]
fn disposed_scene_cancels_nested_loads_and_script_interests() {
    on_big_stack(|| {
        // Mid/Leaf starts loading B at frame 5; Mid is replaced at frame 6, then no Inline is nested any more.
        let (run, steps) = trace("main-nested.wrl", 14, &[url(5, "Mid/Leaf", &["content-b.wrl"]), url(6, "Mid", &["content-c.wrl"])]);
        let d = run.rt.inline_diagnostics();

        assert_eq!((d.pending_scenes, d.frame_waiters, d.in_progress, d.orphan_live_scenes), (0, 0, 0, 0));
        assert!(!lines(&steps, 9, 14).iter().any(|l| l.starts_with("log: B ")), "B stopped: {:?}", lines(&steps, 9, 14));

        // A Script with prepareEvents: one interest while its scene is shown, none after it is replaced.
        let prepare_interests = |run: &Run| {
            let k = run.rt.private_field(run.rt.browser_node, "prepareEvents").unwrap();
            run.rt.frt.get(&k).map_or(0, |f| f.interests.iter().filter(|i| i.cb == x3d_runtime::Cb::ScriptPrepareEvents).count())
        };
        let (mut run, mut steps) = (Run::start("main-prep.wrl", 946684800.0, 0.1).unwrap(), vec![]);

        steps.push(run.record(0));
        assert_eq!(prepare_interests(&run), 1);

        for k in 1..=8 {
            let todo = if k == 3 { vec![url(3, "Inl", &["content-b.wrl"])] } else { vec![] };

            run.step(k, &todo).unwrap();
            steps.push(run.record(k));
        }

        assert_eq!(prepare_interests(&run), 0, "the replaced scene's Script takes no more prepareEvents");
        assert!(!lines(&steps, 6, 8).iter().any(|l| l.starts_with("log: P ")), "{:?}", lines(&steps, 6, 8));
    });
}

/// The auto refresh timer of an Inline in a disposed scene is cleared with the scene, and the Inline
/// is not left IN_PROGRESS.
#[test]
fn auto_refresh_timer_is_cleared_with_its_scene() {
    on_big_stack(|| {
        let set = |frame: u64, name: &str, value: f64| action(frame, "autoRefresh", name, json!({ "value": value }));
        let actions = [set(2, "Mid/Leaf", 2.0), url(6, "Mid", &["content-c.wrl"])];

        // Mid/Leaf refreshes every 2 s (due at frame 20); Mid is replaced at frame 6, the timer is armed at that moment.
        let mut run = Run::start("main-nested.wrl", 946684800.0, 0.1).unwrap();
        let mut steps = vec![run.record(0)];

        for k in 1..=24u64 {
            let todo: Vec<Value> = actions.iter().filter(|a| a["frame"].as_u64() == Some(k)).cloned().collect();

            if k == 6 {
                assert_eq!(run.rt.inline_diagnostics().timers, 1, "the Leaf timer is armed before its scene is disposed");
            }

            run.step(k, &todo).unwrap();
            steps.push(run.record(k));

            if k == 7 {
                assert_eq!(run.rt.inline_diagnostics().timers, 0, "the timer went with the scene");
            }
        }

        assert!(!run.rt.has_unfinished_loads());
        assert_eq!(run.rt.inline_diagnostics().in_progress, 0);
        assert!(!lines(&steps, 8, 24).iter().any(|l| l.contains("A initialize")), "the disposed Leaf does not refresh");
    });
}

/// A request that is queued (its fetch has not run) when its scene is disposed is cancelled: the
/// nested Inline is not left IN_PROGRESS and never loads.
#[test]
fn nested_request_queued_in_a_disposed_scene_is_cancelled() {
    on_big_stack(|| {
        let (run, steps) = trace("main-nested.wrl", 14, &[url(5, "Mid", &["content-c.wrl"]), url(6, "Mid/Leaf", &["content-b.wrl"])]);
        let d = run.rt.inline_diagnostics();

        assert_eq!((d.in_progress, d.pending_scenes, d.frame_waiters, d.orphan_live_scenes), (0, 0, 0, 0));
        assert!(!run.rt.has_unfinished_loads());
        assert!(!lines(&steps, 0, 14).iter().any(|l| l.contains("B initialize")), "B was never fetched");
    });
}

/// `url` and `load` FALSE in the same frame: the request that `url` queued is cancelled by `load`
/// before its fetch runs, so nothing is loaded (the queued fetch must not bring a scene to a disabled Inline).
#[test]
fn request_cancelled_before_its_fetch_runs_loads_nothing() {
    on_big_stack(|| {
        let (run, steps) = trace("main-one.wrl", 12, &[url(4, "Inl", &["content-b.wrl"]), load(4, "Inl", false)]);

        for k in 4..=12 {
            let i = inline_of(&steps[k], "Inl");

            assert_eq!(i["loadState"], "NOT_STARTED", "step {k}");
            assert_eq!(i["loaded"], false, "step {k}");
        }

        assert!(!lines(&steps, 0, 12).iter().any(|l| l.contains("B initialize")), "the fetch of the cancelled request did nothing");
        assert_eq!(run.rt.inline_diagnostics().live_scenes, 0);
    });
}

/// An Inline taken out of its Group keeps its load, as in X_ITE: it is still a node that routes and
/// names reach, so its scene is not dropped (see the report).
#[test]
fn inline_taken_out_of_its_group_finishes_its_load() {
    on_big_stack(|| {
        let (_, steps) = trace("main-one.wrl", 8, &[url(4, "Inl", &["content-b.wrl"]), action(5, "remove", "Inl", json!({ "group": "Holder" }))]);

        assert_eq!(inline_of(&steps[8], "Inl")["loadState"], "COMPLETE");
        assert_eq!(inline_of(&steps[8], "Inl")["roots"], json!(["TimeSensor", "Script", "Transform"]));
    });
}

/// The parent scene's routes and clock go on across every reload, successful or not.
#[test]
fn parent_routes_survive_every_reload() {
    on_big_stack(|| {
        let actions: Vec<Value> = (0..6)
            .map(|i| url(3 + 3 * i, "Inl", &[["content-b.wrl", "gone.wrl", "content-a.wrl", "content-bad.wrl", "content-c.wrl", "gone.wrl"][i as usize]]))
            .collect();
        let (_, steps) = trace("main-one.wrl", 24, &actions);
        let ticks: Vec<u32> = lines(&steps, 0, 24).iter().filter_map(|l| l.strip_prefix("log: parent tick ")).map(|n| n.parse().unwrap()).collect();

        assert!(ticks.len() >= 4, "the parent clock ticks: {ticks:?}");
        assert!(ticks.iter().enumerate().all(|(i, t)| *t as usize == i + 1), "no tick lost or doubled: {ticks:?}");
    });
}

/// Repeated reloads, successful and failed, add no live state: after the first cycles the number
/// of watchers, followers, interests, timers and tasks stays the same. (The node arena is append-only
/// and grows with every scene; the report names it as a limit.)
#[test]
fn repeated_reloads_do_not_accumulate_live_state() {
    on_big_stack(|| {
        let mut run = Run::start("main-one.wrl", 946684800.0, 0.1).unwrap();
        let mut samples = vec![];
        let pick = ["content-b.wrl", "content-a.wrl", "gone.wrl", "content-c.wrl"];
        let mut arena = vec![];

        run.record(0);

        for k in 1..=80u64 {
            let todo = if k % 4 == 1 { vec![url(k, "Inl", &[pick[((k / 4) % 4) as usize]])] } else { vec![] };

            run.step(k, &todo).unwrap();
            run.record(k);

            if k % 16 == 0 {
                arena.push(run.rt.scene.nodes.len());
            }

            if k % 4 == 0 && k >= 16 {
                let rt = &run.rt;
                let browser_prepare = rt.private_field(rt.browser_node, "prepareEvents").unwrap();

                samples.push((
                    rt.live_watchers.len(),
                    rt.followers.len(),
                    rt.frt.get(&browser_prepare).map_or(0, |f| f.interests.len()),
                    rt.tasks.len() + rt.fetches.len(),
                    rt.pending_scenes.len() + rt.frame_waiters.len(),
                    rt.inline_diagnostics().live_scenes,
                ));
            }
        }

        // The four requests repeat (a scene, another scene, a missing file, a third scene): the
        // counts at the same point of the cycle must not grow from one round to the next.
        // The node arena is append-only: every loaded scene stays in it. The growth is linear, the same
        // for every round of four reloads (the report names it as a limit).
        let growth: Vec<usize> = arena.windows(2).map(|w| w[1] - w[0]).collect();

        println!("node arena growth per 4 reloads: {growth:?} (total {})", arena.last().unwrap());
        assert!(growth.windows(2).all(|g| g[0] == g[1]), "arena growth is not linear: {growth:?}");
        assert!(samples.len() > 8);
        assert!(samples.iter().zip(&samples[4..]).all(|(a, b)| a == b), "live state grows with reloads: {samples:?}");
    });
}

/// X3D 4.0, 9.3.2: "If the preceding file loading fails ... no automatic refresh is performed."
/// X_ITE refreshes a failed Inline; this port does not, and an explicit request still recovers it.
#[test]
fn no_auto_refresh_after_a_failed_load() {
    on_big_stack(|| {
        let publish = action(4, "publish", "", json!({ "path": "late.wrl", "from": "content-b.wrl" }));
        let (run, steps) = trace("main-late-refresh.wrl", 10, &[publish, action(7, "loadNow", "Inl", json!({}))]);

        for k in 0..=6 {
            assert_eq!(inline_of(&steps[k], "Inl")["loadState"], "FAILED", "step {k}");
        }

        assert_eq!(run.rt.inline_diagnostics().in_progress, 0);
        assert_eq!(inline_of(&steps[9], "Inl")["loadState"], "COMPLETE", "loadNow recovers after the file exists");
        assert!(lines(&steps, 0, 6).iter().all(|l| !l.contains("B initialize")));
    });
}
