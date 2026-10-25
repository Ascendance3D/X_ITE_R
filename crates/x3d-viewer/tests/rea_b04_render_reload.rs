//! REA batch 04: the NURBS geometry, HAnim skinning (a skinned copy of each skin geometry per
//! change of the joint matrices), ViewpointGroup, Annotation, GeoMetadata and AcousticProperties
//! worlds through load, reload and unload on a real device (skipped when no Vulkan adapter
//! exists). Each reload draws the same picture as the first load; the renderer resources stay
//! bounded over reloads and are released (KEEP_FRAMES retention) once no world draws them.

mod common;

use common::gpu::device;
use std::path::PathBuf;
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};

const T0: f64 = 946684800.0;
const W: u32 = 128;
const H: u32 = 128;

fn worlds() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-b04");
    [
        "nurbs-patch.wrl",
        "nurbs-patch-tex.wrl",
        "nurbs-trimmed.wrl",
        "nurbs-curve.wrl",
        "nurbs-swept.wrl",
        "nurbs-set.wrl",
        "hanim-humanoid.wrl",
        "hanim-sites-41.wrl",
        "viewpoint-group.wrl",
        "annotation-runtime.wrl",
        "annotation-layer.wrl",
        "icon-annotation.wrl",
        "geo-metadata.wrl",
        "acoustic-properties.wrl",
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
fn rea_b04_render_features_survive_reload_and_are_released() {
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

            // Settle (generated cube maps converge over a few frames).
            for _ in 0..4 {
                now += 1.0 / 30.0;
                shot(&mut v, &device, &queue, now);
            }
            let first = shot(&mut v, &device, &queue, now);
            let counts = v.renderer().resource_counts();
            let shadows = v.renderer().shadow_resources();
            let rendered = v.renderer().rendered_resources();
            assert!(
                !name.starts_with("rt-") || rendered > 0,
                "{name}: no rendered textures"
            );
            let transmission = v.renderer().transmission_resources();
            assert!(
                !name.contains("transmission") || transmission == 1,
                "{name}: no transmission buffer"
            );
            assert!(counts[0] > 0, "{name}: geometries in use {counts:?}");
            assert!(
                !name.contains("shadow") || shadows.0 > 0,
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

            // Old worlds' resources are released once they were not used for KEEP_FRAMES frames.
            for _ in 0..240 {
                now += 1.0 / 30.0;
                v.step(now);
                let _ = shot(&mut v, &device, &queue, now);
            }
            let after = v.renderer().resource_counts();
            let shadows_after = v.renderer().shadow_resources();
            assert!(
                v.renderer().rendered_resources() <= rendered,
                "{name}: rendered textures grew"
            );
            assert!(
                shadows_after.0 <= shadows.0 && shadows_after.1 <= shadows.1,
                "{name}: shadow maps grew from {shadows:?} to {shadows_after:?}"
            );
            for (i, (a, b)) in after.iter().zip(counts.iter()).enumerate() {
                assert!(
                    a <= b,
                    "{name}: resource {i} grew from {b} to {a} over 5 reloads ({after:?} vs {counts:?})"
                );
            }
        }

        // No world: everything is released.
        v.unload();
        for _ in 0..240 {
            now += 1.0 / 30.0;
            let _ = shot(&mut v, &device, &queue, now);
        }
        let empty = v.renderer().resource_counts();
        assert_eq!(empty[5], 0, "generated cube maps after unload: {empty:?}");
        assert_eq!(empty[2], 0, "textures after unload: {empty:?}");
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
            v.renderer().rendered_resources(),
            0,
            "rendered textures after unload"
        );
        assert_eq!(
            v.renderer().transmission_resources(),
            0,
            "transmission buffer after unload"
        );
    });
}
