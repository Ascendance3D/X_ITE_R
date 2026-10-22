//! REA batch 08 lane render2: GaussianSplats through load, reload and unload on a real device
//! (skipped when no adapter exists). Each reload draws the same picture as the first load; the
//! renderer's splat data buffers, sorted index buffers, pipelines and bind groups stay bounded over
//! reloads and are released (KEEP_FRAMES retention) after the unload. A second test removes a
//! GaussianSplats node from a live Group by Script, adds it (and a Script-created one) back, and
//! checks the buffers follow.

mod common;

use common::gpu::device;
use std::path::PathBuf;
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};
use x3d_viewer::viewer::UiEvent;

const T0: f64 = 946684800.0;
const W: u32 = 128;
const H: u32 = 128;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-b08-gs")
}

fn worlds() -> Vec<PathBuf> {
    ["gs-basic.wrl", "gs-overlap.wrl", "gs-sh3.wrl", "gs-random-3000.wrl", "gs-transparent-mix.wrl", "gs-ply.wrl", "gs-layers.wrl", "gs-transmission.wrl", "gs-fog-linear.wrl"].iter().map(|n| dir().join(n)).collect()
}

fn shot(v: &mut EmbeddedViewer, device: &wgpu::Device, queue: &wgpu::Queue, now: f64) -> Vec<u8> {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("reload target"),
        size: wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    v.frame(now, &view, wgpu::TextureFormat::Rgba8Unorm, W, H).unwrap();

    let bpr = (W * 4).div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (bpr * H) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bpr), rows_per_image: Some(H) } },
        wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
    );
    queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let data = buf.slice(..).get_mapped_range().expect("map");
    (0..H).flat_map(|y| data[(y * bpr) as usize..(y * bpr + W * 4) as usize].to_vec()).collect()
}

fn differ(a: &[u8], b: &[u8]) -> usize {
    a.chunks_exact(4).zip(b.chunks_exact(4)).filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 8)).count()
}

#[test]
fn rea_b08_gs_survive_reload_and_are_released() {
    let Some((device, queue)) = device() else { return eprintln!("no adapter, skipped") };
    spawn_big_stack(move || {
        let mut v = EmbeddedViewer::new(device.clone(), queue.clone(), (W, H));
        let mut now = T0;

        for world in worlds() {
            let name = world.file_name().unwrap().to_string_lossy().to_string();
            v.load(&world, LoadOptions { root: None, start_time: Some(T0) }).unwrap();

            for _ in 0..4 {
                now += 1.0 / 30.0;
                shot(&mut v, &device, &queue, now);
            }
            let first = shot(&mut v, &device, &queue, now);
            let counts = v.renderer().splat_resources();
            assert!(counts[0] > 0 && counts[1] > 0 && counts[2] > 0 && counts[3] > 0, "{name}: no splat resources in use {counts:?}");

            for round in 0..5 {
                v.reload().unwrap();
                for _ in 0..4 {
                    now += 1.0 / 30.0;
                    shot(&mut v, &device, &queue, now);
                }
                let again = shot(&mut v, &device, &queue, now);
                let d = differ(&first, &again);
                assert!(d == 0, "{name}: reload {round} draws a different picture ({d} pixels)");
            }

            // The buffers of the world that was replaced go after KEEP_FRAMES; the new world's stay.
            for _ in 0..240 {
                now += 1.0 / 30.0;
                v.step(now);
                let _ = shot(&mut v, &device, &queue, now);
            }
            let after = v.renderer().splat_resources();
            for (i, (a, b)) in after.iter().zip(counts.iter()).enumerate() {
                assert!(a <= b, "{name}: splat resource {i} grew from {b} to {a} over 5 reloads ({after:?} vs {counts:?})");
            }
            eprintln!("{name}: splat resources {counts:?} -> {after:?}");
        }

        v.unload();
        for _ in 0..240 {
            now += 1.0 / 30.0;
            let _ = shot(&mut v, &device, &queue, now);
        }
        assert_eq!(v.renderer().splat_resources(), [0, 0, 0, 0], "splat GPU objects after unload");
    });
}

fn key(v: &mut EmbeddedViewer, device: &wgpu::Device, queue: &wgpu::Queue, now: &mut f64, k: &str) -> Vec<u8> {
    v.input(UiEvent::KeyDown { key: k.into() });
    v.input(UiEvent::KeyUp { key: k.into() });
    settle(v, device, queue, now, 3)
}

fn settle(v: &mut EmbeddedViewer, device: &wgpu::Device, queue: &wgpu::Queue, now: &mut f64, frames: usize) -> Vec<u8> {
    let mut last = vec![];
    for _ in 0..frames {
        *now += 1.0 / 30.0;
        last = shot(v, device, queue, *now);
    }
    last
}

/// gs-runtime.wrl: 'k' removes the GaussianSplats S1 from the live Group G1, 'l' adds it back; 'm' adds a
/// Script-created GaussianSplats and 'o' removes it; 'p' empties the positions of S2 and 'q' restores them.
#[test]
fn rea_b08_gs_removal_from_live_group_releases_buffers() {
    let Some((device, queue)) = device() else { return eprintln!("no adapter, skipped") };
    spawn_big_stack(move || {
        let mut v = EmbeddedViewer::new(device.clone(), queue.clone(), (W, H));
        let mut now = T0;
        v.load(&dir().join("gs-runtime.wrl"), LoadOptions { root: None, start_time: Some(T0) }).unwrap();
        let loaded = settle(&mut v, &device, &queue, &mut now, 8);
        // Click once so the KeySensor has focus, as the stepped scripts do.
        v.input(UiEvent::MouseDown { button: 0, x: 5.0, y: H as f64 - 5.0 });
        v.input(UiEvent::MouseUp { button: 0, x: 5.0, y: H as f64 - 5.0 });
        settle(&mut v, &device, &queue, &mut now, 2);
        let both = v.renderer().splat_resources();
        assert_eq!(both[0], 2, "two splat data sets expected: {both:?}");

        key(&mut v, &device, &queue, &mut now, "k");
        let removed = settle(&mut v, &device, &queue, &mut now, 240);
        let one = v.renderer().splat_resources();
        assert_eq!(one[0], 1, "the removed GaussianSplats kept its data buffers: {both:?} -> {one:?}");
        assert!(one[1] < both[1], "the removed GaussianSplats kept its sorted indices: {both:?} -> {one:?}");
        assert!(differ(&loaded, &removed) > 0, "the removal changed nothing");

        key(&mut v, &device, &queue, &mut now, "l");
        let back = settle(&mut v, &device, &queue, &mut now, 8);
        assert_eq!(v.renderer().splat_resources()[0], 2, "the data was not drawn again after the add");
        let d = differ(&loaded, &back);
        assert_eq!(d, 0, "the re-added GaussianSplats draws a different picture ({d} pixels)");

        // A Script-created instance, then its removal.
        key(&mut v, &device, &queue, &mut now, "m");
        settle(&mut v, &device, &queue, &mut now, 4);
        assert_eq!(v.renderer().splat_resources()[0], 3, "the Script-created GaussianSplats drew nothing");
        key(&mut v, &device, &queue, &mut now, "o");
        settle(&mut v, &device, &queue, &mut now, 240);
        assert_eq!(v.renderer().splat_resources()[0], 2, "the removed Script-created GaussianSplats kept its data buffers");

        // Empty positions: not drawn (isEnabled), the data is released.
        key(&mut v, &device, &queue, &mut now, "p");
        settle(&mut v, &device, &queue, &mut now, 240);
        assert_eq!(v.renderer().splat_resources()[0], 1, "the emptied GaussianSplats kept its data buffers");
        key(&mut v, &device, &queue, &mut now, "q");
        settle(&mut v, &device, &queue, &mut now, 8);
        assert_eq!(v.renderer().splat_resources()[0], 2, "the restored GaussianSplats drew nothing");

        v.unload();
        settle(&mut v, &device, &queue, &mut now, 240);
        assert_eq!(v.renderer().splat_resources(), [0, 0, 0, 0], "splat GPU objects after unload");
    });
}
