//! REA batch 08 lane net: the worlds of oracle/synthetic-rea-b08-net (DIS nodes, InlineGeometry, FontLibrary) loaded and
//! unloaded with no renderer. After `unload_world` nothing of any load or scene is left: no pending scene, no frame
//! waiter, no load in progress, no live scene, no timer, no queued task, and no InlineGeometry bookkeeping. A reload
//! (a second runtime on the same world) ends the same way.

use std::path::Path;
use x3d_runtime::{LocalFetcher, Runtime};
use x3d_script::compare::{ORACLE_PREFIX, encode_path};

fn big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}

fn start(world: &str) -> Runtime {
    x3d_fields::jsmath::set_flavor(x3d_fields::jsmath::MathFlavor::Chrome);

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle");
    let url = format!(
        "{ORACLE_PREFIX}{}",
        encode_path(&format!("synthetic-rea-b08-net/{world}"))
    );
    let data = std::fs::read(root.join("synthetic-rea-b08-net").join(world)).unwrap();
    let scene = x3d_parse::parse_bytes(&data, &url).unwrap();
    let mut rt = Runtime::new(
        scene,
        Box::new(LocalFetcher {
            root,
            prefix: ORACLE_PREFIX.to_string(),
        }),
    );

    rt.script_host = Some(Box::new(x3d_script::QuickJsHost::new()));
    rt.live = true;
    rt.now = 946684800.0;
    rt.record_console = true;
    rt.load_scene();
    rt.replace_world();
    rt.settle();
    rt.frame();
    rt.settle();
    rt
}

fn assert_idle(rt: &Runtime, what: &str) {
    let d = rt.inline_diagnostics();

    assert!(
        !rt.has_unfinished_loads(),
        "{what}: loads still in progress"
    );
    assert_eq!(
        (
            d.pending_scenes,
            d.frame_waiters,
            d.in_progress,
            d.live_scenes,
            d.orphan_live_scenes,
            d.timers
        ),
        (0, 0, 0, 0, 0, 0),
        "{what}: {d:?}"
    );
    assert_eq!(rt.tasks.len() + rt.fetches.len(), 0, "{what}: queued tasks");
    assert!(
        rt.inline_geometries.is_empty(),
        "{what}: InlineGeometry bookkeeping"
    );
    assert!(
        rt.pending_font_styles.is_empty(),
        "{what}: FontStyle loops that still wait"
    );
}

#[test]
fn net_worlds_leave_nothing_behind_after_unload_and_reload() {
    big_stack(|| {
        for world in [
            "ig-kinds.wrl",
            "ig-select.wrl",
            "ig-loads.wrl",
            "ig-runtime.wrl",
            "ig-remove.wrl",
            "fl-basic-a.wrl",
            "fl-basic-b.wrl",
            "fl-scopes.wrl",
            "fl-runtime.wrl",
            "dis-fields.wrl",
            "dis-bbox-parents.wrl",
            "dis-runtime.wrl",
        ] {
            for round in 0..3 {
                let mut rt = start(world);
                let mut now = rt.now;

                for _ in 0..60 {
                    now += 1.0 / 30.0;
                    rt.now = now;
                    rt.fire_timeouts(now);
                    rt.frame();
                    rt.settle();
                }

                rt.unload_world();
                rt.settle();
                assert_idle(&rt, &format!("{world} round {round}"));
            }
        }
    });
}
