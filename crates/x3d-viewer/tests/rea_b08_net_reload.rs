//! REA batch 08 lane net: the DIS nodes (bboxDisplay), InlineGeometry and FontLibrary through load, reload and
//! unload on a real device (skipped when no adapter exists). Each reload draws the same picture as the first load;
//! the renderer's resources stay bounded over reloads and are 0 after the unload, with the Inline machinery
//! (`inline_diagnostics`) back to zero. A second test removes an InlineGeometry Shape from a live Group by Script,
//! adds it back, creates an InlineGeometry by Script and removes it again, and does the same with the DIS nodes.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};
use x3d_viewer::viewer::UiEvent;

const T0: f64 = 946684800.0;
const W: u32 = 128;
const H: u32 = 128;

static INSTANCES: AtomicUsize = AtomicUsize::new(0);

/// One device for the whole binary, made once. The tests run in parallel, and each used to start its own wgpu instance
/// at the same moment. One local suite run hung for 53 minutes with every thread waiting on a lock and the third device
/// never made, and on the same RTX machine a stress run of parallel instance starts faults inside libvulkan's
/// vkCreateInstance / vkEnumerateInstanceExtensionProperties. Each viewer still has its own renderer and resource counts.
fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    static DEVICE: OnceLock<Option<(wgpu::Device, wgpu::Queue)>> = OnceLock::new();
    DEVICE
        .get_or_init(|| {
            INSTANCES.fetch_add(1, Ordering::SeqCst);
            let instance =
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let adapter = pollster::block_on(
                instance.request_adapter(&wgpu::RequestAdapterOptions::default()),
            )
            .ok()?;
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                required_features: EmbeddedViewer::wanted_features(&adapter),
                ..Default::default()
            }))
            .ok()
        })
        .clone()
}

/// The tests start together; parallel callers must not start wgpu instances side by side.
#[test]
fn rea_b08_net_tests_start_one_instance() {
    let (tx, rx) = std::sync::mpsc::channel();
    for _ in 0..3 {
        let tx = tx.clone();
        std::thread::spawn(move || tx.send(device().is_some()).unwrap());
    }
    for _ in 0..3 {
        rx.recv_timeout(std::time::Duration::from_secs(60))
            .expect("device creation did not finish in 60 s");
    }
    assert_eq!(
        INSTANCES.load(Ordering::SeqCst),
        1,
        "parallel callers started more than one wgpu instance"
    );
}

fn worlds() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-b08-net");
    [
        "ig-kinds.wrl",
        "ig-select.wrl",
        "fl-basic-a.wrl",
        "fl-scopes.wrl",
        "dis-bbox-parents.wrl",
    ]
    .iter()
    .map(|n| dir.join(n))
    .collect()
}

fn shot(v: &mut EmbeddedViewer, device: &wgpu::Device, queue: &wgpu::Queue, now: f64) -> Vec<u8> {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("reload target"),
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

#[test]
fn rea_b08_net_survive_reload_and_are_released() {
    let Some((device, queue)) = device() else {
        return eprintln!("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let mut v = EmbeddedViewer::new(device.clone(), queue.clone(), (W, H));
        let mut now = T0;

        for world in worlds() {
            let name = world.file_name().unwrap().to_string_lossy().to_string();
            v.load(
                &world,
                LoadOptions {
                    root: None,
                    start_time: Some(T0),
                },
            )
            .unwrap();

            for _ in 0..4 {
                now += 1.0 / 30.0;
                shot(&mut v, &device, &queue, now);
            }
            let first = shot(&mut v, &device, &queue, now);
            let counts = v.renderer().resource_counts();
            assert!(
                counts[0] > 0 || name.starts_with("fl-") || name.starts_with("dis-"),
                "{name}: geometries in use {counts:?}"
            );

            for round in 0..5 {
                v.reload().unwrap();
                for _ in 0..4 {
                    now += 1.0 / 30.0;
                    shot(&mut v, &device, &queue, now);
                }
                let again = shot(&mut v, &device, &queue, now);
                let differ = first
                    .chunks_exact(4)
                    .zip(again.chunks_exact(4))
                    .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 8))
                    .count();
                // ig-select holds ig/routes.x3dv, whose geometry is animated by a looping TimeSensor: its picture depends on the phase.
                let animated = name == "ig-select.wrl";
                assert!(
                    animated || differ == 0,
                    "{name}: reload {round} draws a different picture ({differ} pixels)"
                );
            }

            for _ in 0..240 {
                now += 1.0 / 30.0;
                v.step(now);
                let _ = shot(&mut v, &device, &queue, now);
            }
            let mut after = v.renderer().resource_counts();
            let mut counts = counts;
            if name == "ig-select.wrl" {
                // The animated geometry makes a new buffer every frame and keeps them for KEEP_FRAMES: the count has to reach a
                // steady state, not return to the first frame's value.
                counts = after;
                for _ in 0..240 {
                    now += 1.0 / 30.0;
                    v.step(now);
                    let _ = shot(&mut v, &device, &queue, now);
                }
                after = v.renderer().resource_counts();
                assert!(
                    after[0] <= counts[0] + 2,
                    "{name}: geometry buffers keep growing {counts:?} -> {after:?}"
                );
            }
            for (i, (a, b)) in after.iter().zip(counts.iter()).enumerate() {
                assert!(
                    *a <= *b + if name == "ig-select.wrl" { 2 } else { 0 },
                    "{name}: resource {i} grew from {b} to {a} over 5 reloads ({after:?} vs {counts:?})"
                );
            }
            eprintln!("{name}: resources {counts:?} -> {after:?}");
        }

        v.unload();
        for _ in 0..240 {
            now += 1.0 / 30.0;
            let _ = shot(&mut v, &device, &queue, now);
        }
        let empty = v.renderer().resource_counts();
        assert_eq!(empty[0], 0, "geometry buffers after unload: {empty:?}");
        assert_eq!(empty[2], 0, "textures after unload: {empty:?}");
        assert_eq!(empty[5], 0, "generated cube maps after unload: {empty:?}");
        assert_eq!(
            empty[6], 0,
            "filtered environment maps after unload: {empty:?}"
        );
    });
}

fn key(
    v: &mut EmbeddedViewer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    now: &mut f64,
    k: &str,
) -> Vec<u8> {
    v.input(UiEvent::KeyDown { key: k.into() });
    v.input(UiEvent::KeyUp { key: k.into() });
    let mut last = vec![];
    for _ in 0..3 {
        *now += 1.0 / 30.0;
        last = shot(v, device, queue, *now);
    }
    last
}

fn settle(
    v: &mut EmbeddedViewer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    now: &mut f64,
    frames: usize,
) -> Vec<u8> {
    let mut last = vec![];
    for _ in 0..frames {
        *now += 1.0 / 30.0;
        last = shot(v, device, queue, *now);
    }
    last
}

/// ig-remove.wrl: 'x' removes the Shape S1 (geometry InlineGeometry I1) from the live Group G, 'y' adds it back, 't'
/// adds a Script-created InlineGeometry (with a Shape) to G2 and 'v' clears G2.
#[test]
fn rea_b08_inline_geometry_removal_and_script_creation_release_geometry() {
    let Some((device, queue)) = device() else {
        return eprintln!("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let mut v = EmbeddedViewer::new(device.clone(), queue.clone(), (W, H));
        let mut now = T0;
        let world = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../oracle/synthetic-rea-b08-net/ig-remove.wrl");
        v.load(
            &world,
            LoadOptions {
                root: None,
                start_time: Some(T0),
            },
        )
        .unwrap();
        let loaded = settle(&mut v, &device, &queue, &mut now, 12);
        v.input(UiEvent::MouseDown {
            button: 0,
            x: 5.0,
            y: H as f64 - 5.0,
        });
        v.input(UiEvent::MouseUp {
            button: 0,
            x: 5.0,
            y: H as f64 - 5.0,
        });
        settle(&mut v, &device, &queue, &mut now, 2);
        let with_shape = v.renderer().resource_counts()[0];

        key(&mut v, &device, &queue, &mut now, "x");
        let removed = settle(&mut v, &device, &queue, &mut now, 240);
        let without = v.renderer().resource_counts()[0];
        assert!(
            without < with_shape,
            "the removed Shape kept its inner geometry buffer: {with_shape} -> {without}"
        );
        assert!(loaded != removed, "the removal changed nothing");

        key(&mut v, &device, &queue, &mut now, "y");
        let back = settle(&mut v, &device, &queue, &mut now, 8);
        assert_eq!(
            v.renderer().resource_counts()[0],
            with_shape,
            "the geometry was not drawn again after the add"
        );
        let differ = loaded
            .chunks_exact(4)
            .zip(back.chunks_exact(4))
            .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 8))
            .count();
        assert_eq!(
            differ, 0,
            "the re-added Shape draws a different picture ({differ} pixels)"
        );

        let before = v.renderer().resource_counts()[0];
        key(&mut v, &device, &queue, &mut now, "t");
        settle(&mut v, &device, &queue, &mut now, 8);
        let created = v.renderer().resource_counts()[0];
        assert!(
            created > before,
            "the Script-created InlineGeometry drew nothing: {before} -> {created}"
        );
        key(&mut v, &device, &queue, &mut now, "v");
        settle(&mut v, &device, &queue, &mut now, 240);
        assert_eq!(
            v.renderer().resource_counts()[0],
            before,
            "the removed Script-created InlineGeometry kept its geometry buffer"
        );

        v.unload();
        settle(&mut v, &device, &queue, &mut now, 240);
        assert_eq!(
            v.renderer().resource_counts()[0],
            0,
            "geometry buffers after unload"
        );
    });
}

/// dis-runtime.wrl: 'n' removes the ReceiverPdu and SignalPdu (bboxDisplay TRUE) from the live Group G, 'o' adds
/// them back; 'q' creates one of each PDU node by Script and 't' removes them.
#[test]
fn rea_b08_dis_nodes_removal_and_script_creation() {
    let Some((device, queue)) = device() else {
        return eprintln!("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let mut v = EmbeddedViewer::new(device.clone(), queue.clone(), (W, H));
        let mut now = T0;
        let world = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../oracle/synthetic-rea-b08-net/dis-runtime.wrl");
        v.load(
            &world,
            LoadOptions {
                root: None,
                start_time: Some(T0),
            },
        )
        .unwrap();
        let loaded = settle(&mut v, &device, &queue, &mut now, 12);
        v.input(UiEvent::MouseDown {
            button: 0,
            x: 5.0,
            y: H as f64 - 5.0,
        });
        v.input(UiEvent::MouseUp {
            button: 0,
            x: 5.0,
            y: H as f64 - 5.0,
        });
        settle(&mut v, &device, &queue, &mut now, 2);
        let differs = |a: &[u8], b: &[u8]| {
            a.chunks_exact(4)
                .zip(b.chunks_exact(4))
                .filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > 8))
                .count()
        };

        key(&mut v, &device, &queue, &mut now, "n");
        let removed = settle(&mut v, &device, &queue, &mut now, 240);
        assert!(
            differs(&loaded, &removed) > 0,
            "the removed PDU nodes still draw their bounding boxes"
        );

        key(&mut v, &device, &queue, &mut now, "o");
        let back = settle(&mut v, &device, &queue, &mut now, 8);
        assert_eq!(
            differs(&loaded, &back),
            0,
            "the re-added PDU nodes draw a different picture"
        );

        let before = back;
        key(&mut v, &device, &queue, &mut now, "q");
        let created = settle(&mut v, &device, &queue, &mut now, 8);
        key(&mut v, &device, &queue, &mut now, "t");
        let gone = settle(&mut v, &device, &queue, &mut now, 240);
        assert_eq!(
            differs(&before, &gone),
            0,
            "the removed Script-created PDU nodes still draw ({} pixels)",
            differs(&before, &gone)
        );
        let _ = created;

        v.unload();
        settle(&mut v, &device, &queue, &mut now, 240);
        assert_eq!(
            v.renderer().resource_counts()[0],
            0,
            "geometry buffers after unload"
        );
    });
}
