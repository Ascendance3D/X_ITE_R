//! R0.3-D: Script `shutdown ()` (ISO/IEC 14772-1 4.12.3) and the release of disposed scenes.
//! These tests assert the Rust behavior from the standard directly; X_ITE never calls `shutdown`,
//! so there is nothing to compare with.

mod inline_common;

use inline_common::*;
use serde_json::json;

fn url(frame: u64, name: &str, urls: &[&str]) -> serde_json::Value {
    action(frame, "url", name, json!({ "url": urls }))
}

fn count(run: &Run, line: &str) -> usize {
    run.rt.console.iter().filter(|l| l.as_str() == line).count()
}

/// A scene that is replaced shuts its Scripts down, once each, in document order; a nested scene
/// goes with it. A Script without `shutdown` is left alone.
#[test]
fn replaced_scene_shuts_its_scripts_down_once() {
    big_stack(|| {
        let (run, _) = trace("main-shutdown.wrl", 8, &[url(3, "Inl", &["content-c.wrl"])]);
        let order: Vec<&str> = run.rt.console.iter().filter(|l| l.starts_with("log: ") && l.contains("shutdown")).map(String::as_str).collect();

        assert_eq!(order, ["log: S shutdown", "log: T shutdown", "log: Bad shutdown", "log: L shutdown"], "{:?}", run.rt.console);
        assert_eq!(count(&run, "log: Outer shutdown"), 0, "the world itself is still there");
    });
}

/// After `shutdown ()` returns the Script runs nothing: its `ping`, which its own `shutdown ()` calls
/// before the end, ran; `eventsProcessed` does not run again; the event that `shutdown ()` sent to a
/// Script that the same action deletes is not delivered.
#[test]
fn nothing_runs_after_shutdown_and_events_to_deleted_nodes_are_dropped() {
    big_stack(|| {
        let (run, _) = trace("main-shutdown.wrl", 8, &[url(3, "Inl", &["content-c.wrl"])]);
        let console = &run.rt.console;
        let at = console.iter().position(|l| l == "log: S shutdown").expect("S shutdown");

        assert!(console[at..].iter().any(|l| l == "log: S ping 8"), "shutdown () may call the Script's own methods: {console:?}");
        assert!(!console.iter().any(|l| l.starts_with("log: T got")), "no event reaches a deleted Script: {console:?}");
        assert!(!console[at..].iter().any(|l| l == "log: S processed"), "eventsProcessed after shutdown: {console:?}");
    });
}

/// A `shutdown ()` that throws is reported like any Script error, and the Scripts after it still shut down.
#[test]
fn a_throwing_shutdown_is_reported_and_does_not_stop_the_rest() {
    big_stack(|| {
        let (run, _) = trace("main-shutdown.wrl", 8, &[url(3, "Inl", &["content-c.wrl"])]);
        let console = &run.rt.console;

        assert!(console.iter().any(|l| l.contains("in function 'shutdown'")), "{console:?}");
        assert_eq!(count(&run, "log: L shutdown"), 1);
    });
}

/// Unloading the world shuts down every Script in it, nested scenes included, and a second unload
/// shuts nothing down again.
#[test]
fn unloading_the_world_shuts_everything_down_once() {
    big_stack(|| {
        let (mut run, _) = trace("main-shutdown.wrl", 3, &[]);

        run.rt.unload_world();
        run.rt.settle();
        run.rt.unload_world();
        run.rt.settle();

        for l in ["log: Outer shutdown", "log: S shutdown", "log: T shutdown", "log: Bad shutdown", "log: L shutdown"] {
            assert_eq!(count(&run, l), 1, "{l}: {:?}", run.rt.console);
        }
    });
}

/// An Inline switched off shuts down the Scripts of the scene it drops (the scene is replaced by the
/// empty one).
#[test]
fn unloading_an_inline_shuts_its_scene_down() {
    big_stack(|| {
        let (run, _) = trace("main-shutdown.wrl", 6, &[action(3, "load", "Inl", json!({ "value": false }))]);

        assert_eq!(count(&run, "log: S shutdown"), 1, "{:?}", run.rt.console);
        assert_eq!(count(&run, "log: L shutdown"), 1);
    });
}

// --- the node arena ---------------------------------------------------------------------------

fn arena_live(run: &Run) -> usize {
    run.rt.scene.nodes.len() - run.rt.scene.free_nodes.len()
}

/// Repeated reloads do not grow the node arena: after the first rounds its size stays the same, and
/// what a disposed scene held is handed out again (UNBOUNDED_NODE_ARENA_RETENTION).
#[test]
fn repeated_reloads_keep_the_arena_bounded() {
    big_stack(|| {
        let mut run = Run::start("main-one.wrl", 946684800.0, 0.1).unwrap();
        let pick = ["content-b.wrl", "content-a.wrl", "gone.wrl", "content-c.wrl"];
        let mut arena = vec![];
        let mut live = vec![];

        for k in 1..=160u64 {
            let todo = if k % 4 == 1 { vec![url(k, "Inl", &[pick[((k / 4) % 4) as usize]])] } else { vec![] };

            run.step(k, &todo).unwrap();

            if k % 16 == 0 {
                arena.push(run.rt.scene.nodes.len());
                live.push(arena_live(&run));
            }
        }

        println!("arena {arena:?} live {live:?} released {}", run.rt.released_total);

        // The first rounds fill the arena; from then on it does not grow by a single node.
        assert!(arena[3..].windows(2).all(|w| w[0] == w[1]), "the arena keeps growing: {arena:?}");
        assert!(run.rt.released_total > 100, "nothing was released: {}", run.rt.released_total);
        assert!(run.rt.release_pending() < 64, "candidates pile up: {}", run.rt.release_pending());
    });
}

/// The arena stays bounded for repeated reloads of a scene with a nested Inline, Scripts and PROTO-free
/// content too, and the world keeps working: the right scene is shown after the last reload.
#[test]
fn reloaded_content_is_correct_after_slots_are_reused() {
    big_stack(|| {
        let mut run = Run::start("main-nested.wrl", 946684800.0, 0.1).unwrap();
        let pick = ["content-c.wrl", "content-a.wrl", "content-b.wrl"];
        let mut arena = vec![];

        for k in 1..=120u64 {
            let todo = if k % 6 == 1 { vec![url(k, "Mid", &[pick[((k / 6) % 3) as usize]])] } else { vec![] };

            run.step(k, &todo).unwrap();

            if k % 24 == 0 {
                arena.push(run.rt.scene.nodes.len());
            }
        }

        assert!(arena[1..].windows(2).all(|w| w[0] == w[1]), "the arena keeps growing: {arena:?}");

        // The last request (k = 115) shows the scene `pick[(115 / 6) % 3]`; it ticks.
        let last = pick[(115 / 6) % 3];
        let name = last.trim_start_matches("content-").trim_end_matches(".wrl").to_uppercase();
        let tail: Vec<&String> = run.rt.console.iter().rev().take(40).collect();

        assert!(tail.iter().any(|l| l.starts_with(&format!("log: {name} fraction"))), "{name} is not running: {tail:?}");
    });
}

/// A node that a live field holds stays when its scene goes (DEF/USE identity), with its fields; the
/// others of the scene are released. When the field lets it go, it is released too.
#[test]
fn a_node_that_a_live_field_holds_stays() {
    big_stack(|| {
        let mut run = Run::start("main-hold.wrl", 946684800.0, 0.1).unwrap();
        let scene = *run.rt.inline_scenes.values().next().expect("the Inline's scene");
        let mover = run.rt.scene.get_named_node(scene, "MoverA").expect("MoverA");
        let clock = run.rt.scene.get_named_node(scene, "ClockA").expect("ClockA");
        let keeper = run.rt.scene.get_named_node(x3d_scene::ROOT, "Keeper").expect("Keeper");
        let held = run.rt.get_field(keeper, "held").unwrap();

        run.rt.set_value(held, x3d_fields::FieldValue::SFNode(Some(mover)));
        run.rt.settle();

        // Replace the Inline's scene.
        run.step(1, &[url(1, "Inl", &["content-b.wrl"])]).unwrap();
        run.step(2, &[]).unwrap();
        run.step(3, &[]).unwrap();

        assert!(run.rt.scene.is_live(mover), "the held node was released");
        assert_eq!(run.rt.type_name(mover), "Transform");
        assert!(!run.rt.scene.is_live(clock), "a node nothing refers to stays");

        // Let go of it.
        run.rt.set_value(held, x3d_fields::FieldValue::SFNode(None));
        run.step(4, &[]).unwrap();
        run.step(5, &[]).unwrap();

        assert!(!run.rt.scene.is_live(mover), "the node is still there after nothing holds it");
    });
}

/// A wrapper that a Script keeps of a released node does not reach the node that took its slot.
#[test]
fn a_stale_wrapper_does_not_alias_a_reused_slot() {
    big_stack(|| {
        let mut run = Run::start("main-hold.wrl", 946684800.0, 0.1).unwrap();
        let scene = *run.rt.inline_scenes.values().next().expect("the Inline's scene");
        let mover = run.rt.scene.get_named_node(scene, "MoverA").expect("MoverA");
        let keeper = run.rt.scene.get_named_node(x3d_scene::ROOT, "Keeper").expect("Keeper");
        let held = run.rt.get_field(keeper, "held").unwrap();
        let poke = |run: &mut Run, name: &str| {
            let k = run.rt.get_field(keeper, name).unwrap();
            let t = run.rt.now;

            run.rt.set_value(k, x3d_fields::FieldValue::SFTime(t));
            run.rt.process_events();
            run.rt.settle();
        };

        run.rt.set_value(held, x3d_fields::FieldValue::SFNode(Some(mover)));
        poke(&mut run, "save");
        run.rt.set_value(held, x3d_fields::FieldValue::SFNode(None));

        // The scene goes, then two other scenes take its slots.
        run.step(1, &[url(1, "Inl", &["content-b.wrl"])]).unwrap();

        for k in 2..=14 {
            let todo = if k % 4 == 0 { vec![url(k, "Inl", &[["content-c.wrl", "content-b.wrl"][(k / 4 % 2) as usize]])] } else { vec![] };

            run.step(k, &todo).unwrap();
        }

        // The slot is free or belongs to another node now (the scenes shown since have no `MoverA`).
        assert!(!run.rt.scene.is_live(mover) || run.rt.scene.node(mover).name != "MoverA");
        poke(&mut run, "report");

        let report = run.rt.console.iter().rev().find(|l| l.starts_with("log: saved is")).unwrap_or_else(|| panic!("no report: {:?}", run.rt.console.iter().rev().take(8).collect::<Vec<_>>())).clone();

        assert!(report.contains("X3DReleased"), "the wrapper reaches another node: {report}");
    });
}
