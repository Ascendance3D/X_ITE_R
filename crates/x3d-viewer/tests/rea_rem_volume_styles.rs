//! The volume style lifecycle on a real device (skipped when no Vulkan adapter exists), for the eight
//! styles of `oracle/synthetic-rea-b07-vol/styles` under the attached-parent contract: a style held by a
//! live volume is updated, cleared from it and assigned again; the volume is removed from its live Group
//! (nothing is drawn, every volume resource is released after the retention) and added back (the picture
//! returns); three rounds keep the resources bounded; an unload releases everything.

mod common;

use common::gpu::device;
use std::path::PathBuf;
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};
use x3d_viewer::viewer::UiEvent;

const T0: f64 = 946684800.0;
const W: u32 = 128;
const H: u32 = 128;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-b07-vol")
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
    a.chunks_exact(4).zip(b.chunks_exact(4)).filter(|(x, y)| x.iter().zip(y.iter()).any(|(p, q)| p.abs_diff(*q) > 8)).count()
}

fn settle(v: &mut EmbeddedViewer, device: &wgpu::Device, queue: &wgpu::Queue, now: &mut f64, frames: usize) -> Vec<u8> {
    let mut last = vec![];

    for _ in 0..frames {
        *now += 1.0 / 30.0;
        last = shot(v, device, queue, *now);
    }

    last
}

fn key(v: &mut EmbeddedViewer, k: &str) {
    v.input(UiEvent::KeyDown { key: k.to_string() });
    v.input(UiEvent::KeyUp { key: k.to_string() });
}

fn idle(v: &mut EmbeddedViewer, device: &wgpu::Device, queue: &wgpu::Queue, now: &mut f64, frames: usize) {
    for _ in 0..frames {
        *now += 1.0 / 30.0;
        v.step(*now);
        let _ = shot(v, device, queue, *now);
    }
}


const STYLES: [&str; 8] = ["boundary", "cartoon", "composed", "edge", "opacitymap", "projection", "silhouette", "tonemapped"];

#[test]
fn rea_rem_volume_styles_assign_update_clear_and_release() {
    let Some((device, queue)) = device() else { return eprintln!("no adapter, skipped") };
    spawn_big_stack(move || {
        let mut v = EmbeddedViewer::new(device.clone(), queue.clone(), (W, H));
        let mut now = T0;

        for style in STYLES {
            let world = dir().join(format!("styles/style-{style}.wrl"));
            v.load(&world, LoadOptions { root: None, start_time: Some(T0) }).unwrap();
            let first = settle(&mut v, &device, &queue, &mut now, 5);
            let drawn = v.renderer().volume_resources();
            assert!(drawn[0] > 0 && drawn[1] > 0 && drawn[3] == 1, "{style}: no volume program drawn {drawn:?}");
            assert!(first.chunks_exact(4).any(|p| p[..3] != first[..3]), "{style}: a flat picture");

            for round in 0..3 {
                // Update the style, clear it from the live parent, assign it again.
                key(&mut v, "b");
                let _ = settle(&mut v, &device, &queue, &mut now, 3);
                key(&mut v, "c");
                let _ = settle(&mut v, &device, &queue, &mut now, 3);
                key(&mut v, "a");
                let attached = settle(&mut v, &device, &queue, &mut now, 5);
                let now_drawn = v.renderer().volume_resources();
                assert!(now_drawn[0] <= drawn[0] * 3 && now_drawn[1] <= drawn[1] * 3 && now_drawn[3] == 1, "{style} round {round}: programs grew {drawn:?} -> {now_drawn:?}");

                // Remove the parent from its live Group: nothing is drawn, then everything is released.
                key(&mut v, "d");
                let removed = settle(&mut v, &device, &queue, &mut now, 5);
                assert!(removed.chunks_exact(4).all(|p| p[..3] == removed[..3]), "{style} round {round}: still drawn after removeChildren");
                idle(&mut v, &device, &queue, &mut now, 240);
                assert_eq!(v.renderer().volume_resources(), [0, 0, 0, 0], "{style} round {round}: resources kept after the parent was removed");

                // Add it back: the same picture as before the removal.
                key(&mut v, "e");
                let back = settle(&mut v, &device, &queue, &mut now, 5);
                assert_eq!(differ(&attached, &back), 0, "{style} round {round}: the re-added volume draws differently");
            }

            v.unload();
            idle(&mut v, &device, &queue, &mut now, 240);
            assert_eq!(v.renderer().volume_resources(), [0, 0, 0, 0], "{style}: volume resources after unload");
            assert_eq!(v.renderer().resource_counts()[2], 0, "{style}: textures after unload");
        }
    });
}
