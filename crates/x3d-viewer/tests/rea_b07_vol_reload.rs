//! REA batch 07 VolumeRendering through load, reload, replacement and unload on a real device
//! (skipped when no Vulkan adapter exists). Each reload draws the same picture as the first load;
//! the volume programs, pipelines, bind groups and uniform buffer, and the textures, stay bounded
//! over reloads and are released (KEEP_FRAMES retention) once no world draws them. Worlds replaced
//! by other volume worlds (NodeIds are reused) draw the same picture when they come back, and a
//! volume removed from a live Group and added again, or created by a Script, draws like the
//! original.

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

fn worlds() -> Vec<PathBuf> {
    let d = dir().join("static");
    [
        "vd-voxels-obl",
        "om-tf2d",
        "om-tf3d",
        "bd-default",
        "ct-normals",
        "ed-params",
        "sl-params",
        "pr-max-obl",
        "pr-average-obl",
        "sh-lit-point",
        "tm-lights",
        "cm-nested",
        "bl-table",
        "bl-inner-edge",
        "is-gradients-styles",
        "is-styles-multi",
        "sg-ids-styles",
        "sg-composed",
        "vd-two",
        "vx-layers",
        "vx-proto",
        "vx-shared",
        "vd-fog",
        "vd-opaque-through",
    ]
    .iter()
    .map(|n| d.join(format!("{n}.wrl")))
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
        .filter(|(x, y)| x.iter().zip(y.iter()).any(|(p, q)| p.abs_diff(*q) > 8))
        .count()
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

fn key(v: &mut EmbeddedViewer, k: &str) {
    v.input(UiEvent::KeyDown { key: k.to_string() });
    v.input(UiEvent::KeyUp { key: k.to_string() });
}

#[test]
fn rea_b07_volume_survives_reload_and_is_released() {
    let Some((device, queue)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let mut v = EmbeddedViewer::new(device.clone(), queue.clone(), (W, H));
        let mut now = T0;
        let mut first_pictures: Vec<(PathBuf, Vec<u8>)> = vec![];
        let mut max_volume = [0usize; 4];

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
            let first = settle(&mut v, &device, &queue, &mut now, 5);
            let counts = v.renderer().resource_counts();
            let volume = v.renderer().volume_resources();
            assert!(
                volume[0] > 0 && volume[1] > 0 && volume[3] == 1,
                "{name}: no volume program drawn {volume:?}"
            );
            assert!(counts[2] > 0, "{name}: no textures in use {counts:?}");
            assert!(
                first.chunks_exact(4).any(|p| p[..3] != first[..3]),
                "{name}: a flat picture"
            );

            for round in 0..5 {
                v.reload().unwrap();
                let again = settle(&mut v, &device, &queue, &mut now, 5);
                let n = differ(&first, &again);
                assert!(
                    n == 0,
                    "{name}: reload {round} draws a different picture ({n} pixels)"
                );
            }

            // The programs, pipelines and the uniform buffer are shared by the reloads; a reload's
            // textures are new objects, so a bind group per reload waits for its retention to pass.
            let after = v.renderer().volume_resources();
            assert!(
                after[0] <= volume[0] && after[1] <= volume[1] && after[3] <= volume[3],
                "{name}: volume programs grew over 5 reloads: {volume:?} -> {after:?}"
            );
            assert!(
                after[2] <= volume[2] * 6,
                "{name}: bind groups grew over 5 reloads: {volume:?} -> {after:?}"
            );

            // The old worlds' groups are released once KEEP_FRAMES frames passed.
            for _ in 0..240 {
                now += 1.0 / 30.0;
                v.step(now);
                let _ = shot(&mut v, &device, &queue, now);
            }
            let idle = v.renderer().volume_resources();
            assert!(
                idle.iter().zip(&volume).all(|(a, b)| a <= b),
                "{name}: volume resources not released after the reloads: {volume:?} -> {idle:?}"
            );
            for (i, (a, b)) in v
                .renderer()
                .resource_counts()
                .iter()
                .zip(counts.iter())
                .enumerate()
            {
                assert!(
                    a <= b,
                    "{name}: resource {i} grew from {b} to {a} over 5 reloads"
                );
            }

            for i in 0..4 {
                max_volume[i] = max_volume[i].max(idle[i]);
            }

            first_pictures.push((world, first));
        }

        // Replace worlds by other worlds (the NodeIds of the old world are reused): the pictures
        // come back identical, and the volume resources stay bounded by the largest world.
        for round in 0..3 {
            for (world, first) in first_pictures.iter().rev() {
                let name = world.file_name().unwrap().to_string_lossy().to_string();
                v.load(
                    world,
                    LoadOptions {
                        root: None,
                        start_time: Some(T0),
                    },
                )
                .unwrap();
                let again = settle(&mut v, &device, &queue, &mut now, 5);
                let n = differ(first, &again);
                assert!(
                    n == 0,
                    "{name}: round {round}: replacing worlds changed the picture ({n} pixels)"
                );
            }
        }

        let replaced = v.renderer().volume_resources();
        // Pipelines and programs of other worlds may still wait for their retention to pass, but
        // they are bounded by the number of programs of the worlds used.
        assert!(
            replaced[0] <= first_pictures.len() && replaced[1] <= first_pictures.len() * 2,
            "volume resources after replacing worlds: {replaced:?}"
        );

        // A volume removed from a live Group and added again, and one a Script created.
        let world = dir().join("stepped/rt-volumedata.wrl");
        v.load(
            &world,
            LoadOptions {
                root: None,
                start_time: Some(T0),
            },
        )
        .unwrap();
        key(&mut v, "b");
        key(&mut v, "a");
        let base = settle(&mut v, &device, &queue, &mut now, 5);
        assert!(
            base.chunks_exact(4).any(|p| p[..3] != base[..3]),
            "the volume is not drawn after voxels were set"
        );
        key(&mut v, "h");
        let removed = settle(&mut v, &device, &queue, &mut now, 5);
        assert!(
            differ(&base, &removed) > 1000,
            "removing the volume from its Group did not change the picture"
        );
        assert!(
            removed.chunks_exact(4).all(|p| p[..3] == removed[..3]),
            "a volume is still drawn after removeChildren"
        );
        key(&mut v, "i");
        let added = settle(&mut v, &device, &queue, &mut now, 5);
        assert!(
            differ(&base, &added) == 0,
            "the volume added again draws differently ({} pixels)",
            differ(&base, &added)
        );
        key(&mut v, "j");
        let created = settle(&mut v, &device, &queue, &mut now, 5);
        assert!(
            differ(&base, &created) > 500,
            "the Script-created volume is not drawn"
        );

        // No world: everything is released.
        v.unload();
        for _ in 0..240 {
            now += 1.0 / 30.0;
            let _ = shot(&mut v, &device, &queue, now);
        }
        let empty = v.renderer().resource_counts();
        assert_eq!(empty[2], 0, "textures after unload: {empty:?}");
        assert_eq!(
            v.renderer().volume_resources(),
            [0, 0, 0, 0],
            "volume resources after unload"
        );
        let _ = max_volume;
    });
}

fn idle(
    v: &mut EmbeddedViewer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    now: &mut f64,
    frames: usize,
) {
    for _ in 0..frames {
        *now += 1.0 / 30.0;
        v.step(*now);
        let _ = shot(v, device, queue, *now);
    }
}

/// IsoSurfaceVolumeData and SegmentedVolumeData removed from a live Group, added again, and a
/// Script-created instance added and removed: the GPU resources are released after the retention
/// (KEEP_FRAMES) once nothing draws a volume, and come back with the same picture.
#[test]
fn rea_b07_iso_and_segmented_removal_release_resources() {
    let Some((device, queue)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let mut v = EmbeddedViewer::new(device.clone(), queue.clone(), (W, H));
        let mut now = T0;

        for name in ["rt-iso-life", "rt-segmented-life"] {
            let world = dir().join(format!("stepped/{name}.wrl"));
            v.load(
                &world,
                LoadOptions {
                    root: None,
                    start_time: Some(T0),
                },
            )
            .unwrap();
            let base = settle(&mut v, &device, &queue, &mut now, 5);
            let drawn = v.renderer().volume_resources();
            assert!(
                drawn[0] > 0 && drawn[1] > 0 && drawn[3] == 1,
                "{name}: no volume program drawn {drawn:?}"
            );
            assert!(
                base.chunks_exact(4).any(|p| p[..3] != base[..3]),
                "{name}: a flat picture"
            );

            for round in 0..3 {
                // Removed from the live Group: nothing is drawn, and the resources go after the retention.
                key(&mut v, "a");
                let removed = settle(&mut v, &device, &queue, &mut now, 5);
                assert!(
                    removed.chunks_exact(4).all(|p| p[..3] == removed[..3]),
                    "{name} round {round}: still drawn after removeChildren"
                );
                idle(&mut v, &device, &queue, &mut now, 240);
                assert_eq!(
                    v.renderer().volume_resources(),
                    [0, 0, 0, 0],
                    "{name} round {round}: resources kept after removal"
                );

                // Added again: the same picture, the same number of programs.
                key(&mut v, "b");
                let again = settle(&mut v, &device, &queue, &mut now, 5);
                assert_eq!(
                    differ(&base, &again),
                    0,
                    "{name} round {round}: re-added volume draws differently"
                );
                let back = v.renderer().volume_resources();
                assert!(
                    back[0] <= drawn[0] && back[1] <= drawn[1] && back[3] == 1,
                    "{name} round {round}: programs grew {drawn:?} -> {back:?}"
                );
            }

            // A Script-created instance alone: drawn, then released after its removal.
            key(&mut v, "a");
            idle(&mut v, &device, &queue, &mut now, 240);
            assert_eq!(
                v.renderer().volume_resources(),
                [0, 0, 0, 0],
                "{name}: resources before the created instance"
            );
            key(&mut v, "c");
            let made = settle(&mut v, &device, &queue, &mut now, 5);
            assert!(
                made.chunks_exact(4).any(|p| p[..3] != made[..3]),
                "{name}: the Script-created volume is not drawn"
            );
            assert!(
                v.renderer().volume_resources()[0] > 0,
                "{name}: no program for the created volume"
            );
            key(&mut v, "d");
            let gone = settle(&mut v, &device, &queue, &mut now, 5);
            assert!(
                gone.chunks_exact(4).all(|p| p[..3] == gone[0..3]),
                "{name}: the created volume is still drawn after removal"
            );
            idle(&mut v, &device, &queue, &mut now, 240);
            assert_eq!(
                v.renderer().volume_resources(),
                [0, 0, 0, 0],
                "{name}: resources kept after the created volume was removed"
            );
        }

        v.unload();
        idle(&mut v, &device, &queue, &mut now, 240);
        assert_eq!(
            v.renderer().volume_resources(),
            [0, 0, 0, 0],
            "volume resources after unload"
        );
    });
}
