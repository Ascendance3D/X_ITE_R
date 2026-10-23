//! REA batch 02: LayerSet, Layer, LayoutLayer, Viewport and Layout through load, reload, unload and
//! node slot reuse. The pictures and events are compared with X_ITE by the stepped cases
//! (rea_batch02.rs) and the still images (x3d-render render_rea_b02.rs); these tests check what
//! the oracle cannot see: each reload draws the same picture, the renderer and the viewer keep a
//! bounded number of objects, and a press held in a layer never drives the node that takes a
//! released sensor's slot (R0.3-E generation-safe ids).

mod common;

use common::gpu::device;
use std::path::{Path, PathBuf};
use x3d_fields::{FieldValue, NodeId};
use x3d_scene::{CtxId, ROOT};
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};
use x3d_viewer::viewer::{UiEvent, Viewer3d};

const T0: f64 = 946684800.0;
const W: u32 = 128;
const H: u32 = 128;

fn shot(v: &mut EmbeddedViewer, device: &wgpu::Device, queue: &wgpu::Queue, now: f64) -> Vec<u8> {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("layers target"),
        size: wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    v.frame(now, &view, wgpu::TextureFormat::Rgba8Unorm, W, H)
        .unwrap();

    let bpr = (W * 4).div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (bpr * H) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bpr),
                rows_per_image: Some(H),
            },
        },
        wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let data = buf.slice(..).get_mapped_range().expect("map");
    (0..H)
        .flat_map(|y| data[(y * bpr) as usize..(y * bpr + W * 4) as usize].to_vec())
        .collect()
}

/// (world, layers drawn after load).
fn worlds() -> Vec<(PathBuf, usize)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-b02");
    [
        ("layers-basic.wrl", 3),
        ("layers-order.wrl", 3),
        ("layers-no-layer0.wrl", 1),
        ("layers-styles.wrl", 2),
        ("layout-units.wrl", 5),
        ("layout-nested.wrl", 3),
        ("viewport-in-layer.wrl", 2),
        ("layers-runtime.wrl", 3),
    ]
    .iter()
    .map(|(n, c)| (dir.join(n), *c))
    .collect()
}

#[test]
fn rea_b02_layers_survive_reload_and_are_released() {
    let Some((device, queue)) = device() else {
        return eprintln!("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let mut v = EmbeddedViewer::new(device.clone(), queue.clone(), (W, H));
        let mut now = T0;

        for (world, layers) in worlds() {
            let name = world.file_name().unwrap().to_string_lossy().to_string();
            v.load(
                &world,
                LoadOptions {
                    root: None,
                    start_time: Some(T0),
                },
            )
            .unwrap();

            for _ in 0..3 {
                now += 1.0 / 30.0;
                shot(&mut v, &device, &queue, now);
            }
            let first = shot(&mut v, &device, &queue, now);
            let counts = v.renderer().resource_counts();
            let core = v.core().unwrap();
            assert_eq!(
                core.drawn_layers().len(),
                layers,
                "{name}: layers drawn {:?}",
                core.drawn_layers()
            );
            let slots = core.layer_slots();
            assert!(counts[0] > 0, "{name}: geometries in use {counts:?}");

            for round in 0..5 {
                v.reload().unwrap();
                for _ in 0..3 {
                    now += 1.0 / 30.0;
                    shot(&mut v, &device, &queue, now);
                }
                let again = shot(&mut v, &device, &queue, now);
                let differ = first
                    .chunks_exact(4)
                    .zip(again.chunks_exact(4))
                    .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 8))
                    .count();
                assert!(
                    differ == 0,
                    "{name}: reload {round} draws a different picture ({differ} pixels)"
                );
                assert_eq!(
                    v.core().unwrap().layer_slots(),
                    slots,
                    "{name}: layer states after reload {round}"
                );
            }

            for _ in 0..240 {
                now += 1.0 / 30.0;
                let _ = shot(&mut v, &device, &queue, now);
            }
            let after = v.renderer().resource_counts();
            for (i, (a, b)) in after.iter().zip(counts.iter()).enumerate() {
                assert!(
                    a <= b,
                    "{name}: resource {i} grew from {b} to {a} over 5 reloads ({after:?} vs {counts:?})"
                );
            }
        }

        v.unload();
        for _ in 0..240 {
            now += 1.0 / 30.0;
            let _ = shot(&mut v, &device, &queue, now);
        }
        let empty = v.renderer().resource_counts();
        assert_eq!(empty[0], 0, "geometries after unload: {empty:?}");
    });
}

// --- slot reuse inside a layer -------------------------------------------------------------------

fn big(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

fn write(d: &Path, name: &str, text: &str) {
    std::fs::write(d.join(name), text).unwrap();
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

    fn value(&self, node: NodeId, name: &str) -> String {
        format!("{:?}", self.v.rt.pre_value(node, name).unwrap())
    }
}

/// A PlaneSensor of an Inline inside a Layer is pressed; the Inline unloads and loads a scene whose
/// sensor takes the same slot. The held press (its pointing layer is still drawn) does not drive
/// the new sensor; a fresh press in the layer does. A press outside the layer's viewport reaches
/// nothing of the layer.
#[test]
fn a_press_held_in_a_layer_does_not_drive_the_node_that_takes_its_slot() {
    big(|| {
        let d = std::env::temp_dir().join(format!("x3d-rea-b02-layers-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let sub = |n: u32| {
            format!(
                "#VRML V2.0 utf8\nGroup {{ children [ DEF S{n} PlaneSensor {{ }} Shape {{ geometry Box {{ size 4 4 4 }} }} ] }}\n"
            )
        };
        write(&d, "sub_1.wrl", &sub(1));
        write(&d, "sub_2.wrl", &sub(2));
        write(
            &d,
            "main.wrl",
            "#X3D V3.2 utf8\nPROFILE Interchange\nCOMPONENT Layering:1\nViewpoint { position 0 0 10 }\nLayerSet { order [ 0 1 ] layers [ Layer { viewport Viewport { clipBoundary [ 0.5 1 0 1 ] } children [ Viewpoint { position 0 0 10 } DEF Inl Inline { url \"sub_1.wrl\" } ] } ] }\n",
        );

        let mut w = World {
            v: Viewer3d::load_opts(
                &d.join("main.wrl"),
                Some(&d),
                100.0,
                256,
                256,
                Some(60.0),
                true,
            )
            .expect("load"),
            t: 100.0,
        };
        w.steps(4);
        assert_eq!(w.v.drawn_layers().len(), 2);
        let s1 = w.named(w.inline_ctx(), "S1");

        // Outside the layer's viewport (the left half): nothing of the layer is hit.
        w.v.input(UiEvent::MouseMove { x: 60.0, y: 128.0 });
        w.v.input(UiEvent::MouseDown {
            button: 0,
            x: 60.0,
            y: 128.0,
        });
        assert!(
            !w.v.rt.get_bool(s1, "isActive"),
            "a press outside the layer's viewport"
        );
        w.v.input(UiEvent::MouseUp {
            button: 0,
            x: 60.0,
            y: 128.0,
        });

        // The box in the middle of the layer's viewport (the right half).
        w.v.input(UiEvent::MouseMove { x: 192.0, y: 128.0 });
        w.v.input(UiEvent::MouseDown {
            button: 0,
            x: 192.0,
            y: 128.0,
        });
        assert!(w.v.rt.get_bool(s1, "isActive"), "the press activates S1");

        let inl = w.named(ROOT, "Inl");
        w.field(inl, "load", FieldValue::SFBool(false));
        w.steps(2);
        w.field(inl, "url", FieldValue::MFString(vec!["sub_2.wrl".into()]));
        w.field(inl, "load", FieldValue::SFBool(true));
        w.steps(3);

        let s2 = w.named(w.inline_ctx(), "S2");
        assert_eq!(s2, s1, "precondition: S2 took S1's slot");
        let before = w.value(s2, "translation_changed");

        w.v.input(UiEvent::MouseMove { x: 220.0, y: 150.0 });
        assert!(!w.v.rt.get_bool(s2, "isActive"), "S2 was never pressed");
        assert_eq!(
            w.value(s2, "translation_changed"),
            before,
            "the old press drove S2"
        );
        w.v.input(UiEvent::MouseUp {
            button: 0,
            x: 220.0,
            y: 150.0,
        });
        w.steps(1);
        assert_eq!(
            w.value(s2, "translation_changed"),
            before,
            "the release drove S2"
        );

        w.v.input(UiEvent::MouseMove { x: 192.0, y: 128.0 });
        w.v.input(UiEvent::MouseDown {
            button: 0,
            x: 192.0,
            y: 128.0,
        });
        assert!(
            w.v.rt.get_bool(s2, "isActive"),
            "a new press in the layer activates S2"
        );
        w.v.input(UiEvent::MouseMove { x: 210.0, y: 110.0 });
        assert_ne!(
            w.value(s2, "translation_changed"),
            before,
            "the new drag drives S2"
        );
        w.v.input(UiEvent::MouseUp {
            button: 0,
            x: 210.0,
            y: 110.0,
        });
        assert_eq!(w.v.layer_slots(), 2, "one state per layer");

        let _ = std::fs::remove_dir_all(&d);
    });
}

/// Picking (REA batch 02): the Inline that holds a LinePickSensor (IMPORTed by the world) is
/// unloaded and loaded 20 times while the sensor picks. The browser's pick sensor set and the node
/// arena stay bounded (the arena takes one more run of slots early on, then stays), and the
/// reloaded sensor picks again.
#[test]
fn pick_sensors_survive_inline_reloads() {
    big(|| {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-b02");
        let mut w = World {
            v: Viewer3d::load_opts(
                &dir.join("picking-layers.wrl"),
                Some(&dir),
                T0,
                256,
                256,
                Some(30.0),
                true,
            )
            .expect("load"),
            t: T0,
        };
        w.steps(8);

        let isens = w.named(ROOT, "ISENS");
        let first = w.v.rt.pick_sensors.len();
        assert_eq!(first, 4, "S0, S1, S2 and the Inline's IN are live");
        let mut arena = 0;

        for round in 0..20 {
            w.field(isens, "load", FieldValue::SFBool(false));
            w.steps(6);
            assert_eq!(
                w.v.rt.pick_sensors.len(),
                first - 1,
                "round {round}: the unloaded sensor left the set"
            );

            w.field(isens, "load", FieldValue::SFBool(true));
            w.steps(6);
            assert_eq!(
                w.v.rt.pick_sensors.len(),
                first,
                "round {round}: the reloaded sensor is live"
            );

            let live = w.v.rt.scene.nodes.len() - w.v.rt.scene.free_nodes.len();

            if round == 10 {
                arena = w.v.rt.scene.nodes.len();
            } else if round > 10 {
                assert!(
                    w.v.rt.scene.nodes.len() <= arena,
                    "round {round}: arena grew from {arena} to {} ({live} live)",
                    w.v.rt.scene.nodes.len()
                );
            }
        }

        // The reloaded sensor picks during one sweep (4 s) of the world's interpolator.
        let ctx = w.v.rt.inline_scenes[&isens];
        let sensor = w.named(ctx, "IN");
        assert!(w.v.rt.pick_sensors.contains(&sensor));
        let mut active = 0;

        for _ in 0..260 {
            w.steps(1);
            active += usize::from(w.v.rt.get_bool(sensor, "isActive"));
        }

        assert!(active > 0, "the reloaded sensor never picked");
    });
}
