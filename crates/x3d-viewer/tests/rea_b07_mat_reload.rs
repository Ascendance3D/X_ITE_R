//! REA batch 07 part C: DispersionMaterialExtension, RetroreflectionMaterialExtension, ImageTexture3D
//! and InstancedShape through load, reload and unload on a real device (skipped when no adapter
//! exists). Each reload draws the same picture as the first load; the renderer's geometry buffers,
//! textures (3D volumes included), bind groups and the transmission buffer stay bounded over reloads
//! and are released (KEEP_FRAMES retention) after the unload. A second test removes an
//! InstancedShape from a live Group by Script and adds it (and a Script-created one) back: its
//! geometry buffer is released after the removal and drawn again after the add.

mod common;

use common::gpu::device;
use std::path::PathBuf;
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};
use x3d_viewer::viewer::UiEvent;

const T0: f64 = 946684800.0;
const W: u32 = 128;
const H: u32 = 128;

fn worlds() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-b07-mat");
    [
        "disp-transmission-ibl.wrl",
        "disp-combined-ibl.wrl",
        "retro-combined-ibl.wrl",
        "tex3d-nrrd-types.wrl",
        "tex3d-dicom.wrl",
        "tex3d-inline.wrl",
        "tex3d-layers.wrl",
        "inst-transforms.wrl",
        "inst-geometries.wrl",
        "inst-transparent-textured.wrl",
        "inst-shadows.wrl",
        "inst-pbr-ibl.wrl",
        "inst-twosided-layers.wrl",
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
fn rea_b07_mat_survive_reload_and_are_released() {
    let Some((device, queue)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
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
            let shadows = v.renderer().shadow_resources();
            let transmission = v.renderer().transmission_resources();
            assert!(
                !name.starts_with("disp-") || transmission == 1,
                "{name}: no transmission buffer"
            );
            assert!(counts[0] > 0, "{name}: geometries in use {counts:?}");
            assert!(
                !name.contains("shadows") || shadows.0 > 0,
                "{name}: no shadow maps {shadows:?}"
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
                assert!(
                    differ == 0,
                    "{name}: reload {round} draws a different picture ({differ} pixels)"
                );
            }

            for _ in 0..240 {
                now += 1.0 / 30.0;
                v.step(now);
                let _ = shot(&mut v, &device, &queue, now);
            }
            let after = v.renderer().resource_counts();
            assert!(
                v.renderer().transmission_resources() <= transmission,
                "{name}: transmission buffers grew"
            );
            for (i, (a, b)) in after.iter().zip(counts.iter()).enumerate() {
                assert!(
                    a <= b,
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
        assert_eq!(
            v.renderer().shadow_resources(),
            (0, 0),
            "shadow maps after unload"
        );
        assert_eq!(
            v.renderer().transmission_resources(),
            0,
            "transmission buffer after unload"
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

/// inst-runtime.wrl: 'm' removes InstancedShape I1 from the live Group G1, 'n' adds it back; 'o' adds a
/// Script-created InstancedShape (own Sphere geometry) and 'q' removes it.
#[test]
fn rea_b07_instanced_shape_removal_from_live_group_releases_geometry() {
    let Some((device, queue)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let mut v = EmbeddedViewer::new(device.clone(), queue.clone(), (W, H));
        let mut now = T0;
        let world = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../oracle/synthetic-rea-b07-mat/inst-runtime.wrl");
        v.load(
            &world,
            LoadOptions {
                root: None,
                start_time: Some(T0),
            },
        )
        .unwrap();
        let loaded = settle(&mut v, &device, &queue, &mut now, 8);
        // Click once so the KeySensor has focus, as the stepped scripts do.
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
        let with_box = v.renderer().resource_counts()[0];

        key(&mut v, &device, &queue, &mut now, "m");
        let removed = settle(&mut v, &device, &queue, &mut now, 240);
        let without_box = v.renderer().resource_counts()[0];
        assert!(
            without_box < with_box,
            "the removed InstancedShape kept its geometry buffer: {with_box} -> {without_box}"
        );
        assert!(loaded != removed, "the removal changed nothing");

        key(&mut v, &device, &queue, &mut now, "n");
        let back = settle(&mut v, &device, &queue, &mut now, 8);
        assert_eq!(
            v.renderer().resource_counts()[0],
            with_box,
            "the geometry buffer was not drawn again after the add"
        );
        let differ = loaded
            .chunks_exact(4)
            .zip(back.chunks_exact(4))
            .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 8))
            .count();
        assert_eq!(
            differ, 0,
            "the re-added InstancedShape draws a different picture ({differ} pixels)"
        );

        // A Script-created instance, then its removal.
        let before = v.renderer().resource_counts()[0];
        key(&mut v, &device, &queue, &mut now, "o");
        settle(&mut v, &device, &queue, &mut now, 4);
        let created = v.renderer().resource_counts()[0];
        assert!(
            created > before,
            "the Script-created InstancedShape drew nothing: {before} -> {created}"
        );
        key(&mut v, &device, &queue, &mut now, "q");
        settle(&mut v, &device, &queue, &mut now, 240);
        assert_eq!(
            v.renderer().resource_counts()[0],
            before,
            "the removed Script-created InstancedShape kept its geometry buffer"
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
