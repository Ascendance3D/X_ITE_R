//! REA batch 08 lane render2: VolumeScatterMaterialExtension through load, reload and unload on a real
//! device (skipped when no adapter exists). Each reload draws the same picture as the first load; the
//! scatter buffer (RGBA32F color, depth and the two storage buffers) exists while a frame draws a shape with the
//! extension and is released (KEEP_FRAMES retention) after the unload. A second test clears the extension
//! of live materials, removes a Shape from a live Group, and checks the buffer follows.

mod common;

use common::gpu::device;
use std::path::PathBuf;
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};
use x3d_viewer::viewer::UiEvent;

const T0: f64 = 946684800.0;
const W: u32 = 128;
const H: u32 = 128;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-b08-vs")
}

fn worlds() -> Vec<PathBuf> {
    [
        "vs-ibl.wrl",
        "vs-lights.wrl",
        "vs-ibl-lights.wrl",
        "vs-geometry-ibl.wrl",
        "vs-transparent-ibl.wrl",
        "vs-textures-a-ibl.wrl",
        "vs-fog.wrl",
        "vs-back-ibl.wrl",
    ]
    .iter()
    .map(|n| dir().join(n))
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

fn differ(a: &[u8], b: &[u8]) -> usize {
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 8))
        .count()
}

#[test]
fn rea_b08_vs_survive_reload_and_are_released() {
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
            assert_eq!(
                v.renderer().scatter_resources(),
                1,
                "{name}: no scatter buffer in use"
            );

            for round in 0..5 {
                v.reload().unwrap();
                for _ in 0..4 {
                    now += 1.0 / 30.0;
                    shot(&mut v, &device, &queue, now);
                }
                let again = shot(&mut v, &device, &queue, now);
                let d = differ(&first, &again);
                assert!(
                    d == 0,
                    "{name}: reload {round} draws a different picture ({d} pixels)"
                );
                assert_eq!(
                    v.renderer().scatter_resources(),
                    1,
                    "{name}: reload {round}"
                );
            }

            eprintln!("{name}: scatter buffer in use after 5 reloads");
        }

        // After the unload the buffer is released after KEEP_FRAMES.
        v.unload();
        for _ in 0..240 {
            now += 1.0 / 30.0;
            let _ = shot(&mut v, &device, &queue, now);
        }
        assert_eq!(
            v.renderer().scatter_resources(),
            0,
            "the scatter buffer after unload"
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
    settle(v, device, queue, now, 3)
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

/// vs-runtime.wrl: M1 and M3 have a VolumeScatterMaterialExtension; 'f' clears M1's extensions, 'j' M3's (no shape
/// of the extension is left, the buffer is released), 'm' assigns it to M1 again; 'k' removes M1's Shape from the live
/// Group G1 (the buffer follows M3's shape: cleared again, so it is released), 'l' adds it back.
#[test]
fn rea_b08_vs_extension_cleared_and_shape_removed_release_the_buffer() {
    let Some((device, queue)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let mut v = EmbeddedViewer::new(device.clone(), queue.clone(), (W, H));
        let mut now = T0;
        v.load(
            &dir().join("vs-runtime.wrl"),
            LoadOptions {
                root: None,
                start_time: Some(T0),
            },
        )
        .unwrap();
        let loaded = settle(&mut v, &device, &queue, &mut now, 8);
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
        assert_eq!(
            v.renderer().scatter_resources(),
            1,
            "no scatter buffer with two shapes of the extension"
        );

        // The extension cleared from M1 (one shape left), then from M3: none left.
        key(&mut v, &device, &queue, &mut now, "f");
        let one = settle(&mut v, &device, &queue, &mut now, 8);
        assert!(
            differ(&loaded, &one) > 0,
            "clearing the extension changed nothing"
        );
        assert_eq!(
            v.renderer().scatter_resources(),
            1,
            "M3's shape still draws with the extension"
        );
        key(&mut v, &device, &queue, &mut now, "j");
        settle(&mut v, &device, &queue, &mut now, 240);
        assert_eq!(
            v.renderer().scatter_resources(),
            0,
            "the scatter buffer was kept with no shape of the extension"
        );

        // Assigned again to a live material: the buffer comes back.
        key(&mut v, &device, &queue, &mut now, "m");
        let again = settle(&mut v, &device, &queue, &mut now, 8);
        assert_eq!(
            v.renderer().scatter_resources(),
            1,
            "the extension assigned to a live material drew nothing"
        );
        assert!(
            differ(&one, &again) > 0,
            "assigning the extension changed nothing"
        );

        // The Shape removed from the live Group: released after the retention frames.
        key(&mut v, &device, &queue, &mut now, "k");
        settle(&mut v, &device, &queue, &mut now, 240);
        assert_eq!(
            v.renderer().scatter_resources(),
            0,
            "the removed Shape kept the scatter buffer"
        );
        key(&mut v, &device, &queue, &mut now, "l");
        settle(&mut v, &device, &queue, &mut now, 8);
        assert_eq!(
            v.renderer().scatter_resources(),
            1,
            "the re-added Shape drew nothing"
        );

        v.unload();
        settle(&mut v, &device, &queue, &mut now, 240);
        assert_eq!(
            v.renderer().scatter_resources(),
            0,
            "the scatter buffer after unload"
        );
    });
}
