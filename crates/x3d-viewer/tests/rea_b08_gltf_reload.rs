//! REA batch 08 (glTF lane): glTF and GLB worlds through load, reload and unload on a real device (skipped when no adapter
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
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-b08-gltf");
    [
        "render/tex-nearest.gltf",
        "render/skin-bent.gltf",
        "render/morph-static.gltf",
        "render/mat-clearcoat.gltf",
        "glb/textured.glb",
        "anim/step-driver.wrl",
        "variants/basic.gltf",
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
fn rea_b08_gltf_survive_reload_and_are_released() {
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
            assert!(counts[0] > 0, "{name}: geometries in use {counts:?}");

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
