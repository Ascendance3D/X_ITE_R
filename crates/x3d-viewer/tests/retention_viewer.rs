//! R0.3-E: the viewer workloads of the retention cleanup, 1,000 cycles each. The runtime's tables
//! and the script host's field handles stay flat after the warm-up while a viewer swaps an Inline
//! under the pointer, and an EmbeddedViewer loads and unloads scripted worlds. Newly authored worlds
//! in a temp directory; time is virtual. `MEM` lines at cycles 10, 100, 500 and 1,000.

#[cfg(feature = "gpu")]
mod common;

use std::path::{Path, PathBuf};
use x3d_fields::{FieldValue, NodeId};
use x3d_runtime::Runtime;
use x3d_scene::{CtxId, ROOT};
use x3d_script::engine::slot_stats;
use x3d_viewer::viewer::{UiEvent, Viewer3d};

fn big(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("x3d-retention-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(d: &Path, name: &str, text: &str) -> PathBuf {
    let p = d.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

fn rss_kib() -> usize {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines().find_map(|l| {
                l.strip_prefix("VmRSS:")
                    .and_then(|r| r.split_whitespace().next())
                    .and_then(|n| n.parse().ok())
            })
        })
        .unwrap_or(0)
}

/// Lengths and capacities of every retained table.
fn tables(rt: &Runtime) -> Vec<(&'static str, usize)> {
    let s = &rt.scene;
    let js = slot_stats();

    vec![
        ("arena", s.nodes.len()),
        ("arena_cap", s.nodes.capacity()),
        ("live", s.nodes.len() - s.free_nodes.len()),
        ("contexts", s.contexts.len()),
        ("contexts_cap", s.contexts.capacity()),
        ("protos", s.protos.len()),
        ("externprotos", s.externprotos.len()),
        ("imported", s.imported.len()),
        ("exported", s.exported.len()),
        ("emptied_ctxs", rt.emptied_contexts()),
        ("dead_ctxs", rt.dead_ctxs.len()),
        ("disposed_scenes", rt.disposed_scenes.len()),
        ("js_slots", js.len),
        ("js_slots_cap", js.capacity),
        ("js_attached", js.attached),
    ]
}

fn line(workload: &str, cycle: usize, t: &[(&str, usize)], extra: &str) -> String {
    let cols: Vec<String> = t.iter().map(|(k, v)| format!("{k}={v}")).collect();
    format!(
        "MEM\t{workload}\t{cycle}\t{}\t{extra}\trss_kib={}",
        cols.join("\t"),
        rss_kib()
    )
}

struct World {
    v: Viewer3d,
    t: f64,
}

impl World {
    fn steps(&mut self, n: usize) {
        for _ in 0..n {
            self.t += 1.0 / 60.0;
            self.v.step(self.t);
        }
    }

    fn named(&self, ctx: CtxId, name: &str) -> NodeId {
        self.v.rt.scene.contexts[ctx]
            .named_nodes
            .get(name)
            .unwrap_or_else(|| panic!("no {name}"))
    }

    fn inline_ctx(&self) -> CtxId {
        self.v.rt.inline_scenes[&self.named(ROOT, "Inl")]
    }

    fn field(&mut self, node: NodeId, name: &str, value: FieldValue) {
        let k = self.v.rt.get_field(node, name).unwrap();
        self.v.rt.set_value(k, value);
    }
}

/// 6. Viewer pointer slot reuse: the pointer rests over, and presses, a TouchSensor of an Inline whose
/// scene is replaced by one whose TouchSensor takes the same node slot.
#[test]
fn w6_viewer_pointer_slot_reuse() {
    big(|| {
        let d = dir("pointer");
        let scene = |n: u32| {
            format!(
                "#VRML V2.0 utf8\nGroup {{ children [ DEF T TouchSensor {{ description \"Touch{n}\" }} Shape {{ geometry Box {{ size 4 4 4 }} }} ] }}\nDEF S Script {{ eventIn SFBool over url \"javascript: function over (v) {{ }}\" }}\nROUTE T.isOver TO S.over\n"
            )
        };

        write(&d, "a.wrl", &scene(1));
        write(&d, "b.wrl", &scene(2));

        let p = write(
            &d,
            "main.wrl",
            "#VRML V2.0 utf8\nViewpoint { position 0 0 10 }\nDEF Inl Inline { url \"a.wrl\" }\n",
        );
        let mut w = World {
            v: Viewer3d::load_opts(&p, Some(&d), 100.0, 256, 256, Some(60.0), true).expect("load"),
            t: 100.0,
        };
        let mut base = None;
        let mut stale = 0;

        w.steps(4);

        for c in 1..=1000 {
            let inl = w.named(ROOT, "Inl");

            w.v.input(UiEvent::MouseMove { x: 128.0, y: 128.0 });
            w.v.input(UiEvent::MouseDown {
                button: 0,
                x: 128.0,
                y: 128.0,
            });
            w.field(inl, "load", FieldValue::SFBool(false));
            w.steps(2);
            w.field(
                inl,
                "url",
                FieldValue::MFString(vec![if c % 2 == 0 { "a.wrl" } else { "b.wrl" }.into()]),
            );
            w.field(inl, "load", FieldValue::SFBool(true));
            w.steps(3);

            // The new TouchSensor did not see the old press or hover.
            let t = w.named(w.inline_ctx(), "T");

            if w.v.rt.get_bool(t, "isOver") || w.v.rt.get_bool(t, "isActive") {
                stale += 1;
            }

            w.v.input(UiEvent::MouseUp {
                button: 0,
                x: 128.0,
                y: 128.0,
            });
            w.v.input(UiEvent::MouseMove { x: 129.0, y: 128.0 });
            assert!(
                w.v.rt.get_bool(t, "isOver"),
                "cycle {c}: the pointer is over the new TouchSensor"
            );
            w.v.input(UiEvent::MouseMove { x: 5.0, y: 5.0 });
            w.steps(1);
            w.v.rt.console.clear();

            let now = tables(&w.v.rt);

            if [10, 100, 500, 1000].contains(&c) {
                println!(
                    "{}",
                    line(
                        "viewer-pointer",
                        c,
                        &now,
                        &format!("stale_pointer_events={stale}")
                    )
                );
            }

            if c == 10 {
                base = Some(now.clone());
            }

            if let Some(b) = &base {
                assert_eq!(&now, b, "cycle {c}: a table grew after the warm-up");
            }
        }

        assert_eq!(
            stale, 0,
            "pointer state of a released node reached the node that took its slot"
        );
        let _ = std::fs::remove_dir_all(&d);
    });
}

#[cfg(feature = "gpu")]
mod embedded {
    use super::*;
    use crate::common::gpu::device;
    use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};

    const T0: f64 = 946684800.0;

    /// 7. EmbeddedViewer load and unload: each world's Scripts shut down once; the script host's
    /// handle table and the process stay flat.
    #[test]
    fn w7_embedded_viewer_load_and_unload() {
        let Some((device, queue)) = device() else {
            return eprintln!("no adapter, skipped");
        };
        spawn_big_stack(move || {
            let d = dir("embedded");
            let sub = "#VRML V2.0 utf8\nDEF T Transform { }\nDEF S Script { field SFNode t USE T url \"javascript: function initialize () { t .translation = new SFVec3f (1, 2, 3); } function shutdown () { print ('shutdown sub'); }\" }\n";
            let main = "#VRML V2.0 utf8\nDEF Inl Inline { url \"sub.wrl\" }\nDEF M Script { field MFVec3f v [ 1 2 3 ] url \"javascript: function initialize () { print ('init'); v [0] = new SFVec3f (4, 5, 6); } function shutdown () { print ('shutdown'); }\" }\n";

            write(&d, "sub.wrl", sub);

            let p = write(&d, "main.wrl", main);
            let mut v = EmbeddedViewer::new(device, queue, (64, 64));
            let (mut inits, mut downs) = (0, 0);
            let mut base = None;

            for c in 1..=1000 {
                v.load(
                    &p,
                    LoadOptions {
                        root: Some(d.clone()),
                        start_time: Some(T0),
                    },
                )
                .unwrap();
                inits += v.take_prints().iter().filter(|l| *l == "init").count();

                for k in 1..=4 {
                    v.step(T0 + k as f64 * 0.02);
                }

                v.unload();

                let prints = v.take_prints();

                downs += prints.iter().filter(|l| *l == "shutdown").count();

                let js = slot_stats();
                let now = (js.len, js.capacity);

                if [10, 100, 500, 1000].contains(&c) {
                    println!(
                        "MEM\tembedded-load-unload\t{c}\tinit={inits}\tshutdown={downs}\tjs_slots={}/{}\trss_kib={}",
                        now.0,
                        now.1,
                        rss_kib()
                    );
                }

                if c == 10 {
                    base = Some(now);
                }

                if let Some(b) = base {
                    assert_eq!(now, b, "cycle {c}: the script host's handle table grew");
                }
            }

            assert_eq!(
                (inits, downs),
                (1000, 1000),
                "every world initializes and shuts down once"
            );
            let _ = std::fs::remove_dir_all(&d);
        });
    }
}
