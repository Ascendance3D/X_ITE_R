//! REA batch 08 shaders lane: ComposedShader and ShaderPart (user shaders, textures, attributes, invalid parts)
//! through load, reload and unload on a real device (skipped when no adapter
//! exists). Each reload draws the same picture as the first load; the renderer's shader modules, pipelines,
//! vertex buffers and bind groups (Renderer::shader_resources) stay bounded over reloads and are 0
//! after the unload. A second test removes a shaded Transform from a live Group by Script and adds
//! it back, and a third has a Script create the ComposedShader and its parts.

use std::path::PathBuf;
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};
use x3d_viewer::viewer::UiEvent;

const T0: f64 = 946684800.0;
const W: u32 = 128;
const H: u32 = 128;

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { required_features: EmbeddedViewer::wanted_features(&adapter), ..Default::default() })).ok()
}

fn worlds() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-b08-shaders");
    ["basic.wrl", "es300.wrl", "lights.wrl", "texture-field.wrl", "texture-appearance.wrl", "attrib-float.wrl", "geometries.wrl", "fog.wrl", "bad-syntax.wrl", "select-first-valid.wrl", "file-fallback.wrl", "stub-nodes.wrl", "ctx-phong-light.wrl", "ctx-pbr.wrl", "ctx-fog.wrl", "ctx-texture.wrl"].iter().map(|n| dir.join(n)).collect()
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

#[test]
fn rea_b08_shaders_survive_reload_and_are_released() {
    let Some((device, queue)) = device() else { return eprintln!("no adapter, skipped") };
    spawn_big_stack(move || {
        let mut v = EmbeddedViewer::new(device.clone(), queue.clone(), (W, H));
        let mut now = T0;

        for world in worlds() {
            let name = world.file_name().unwrap().to_string_lossy().to_string();
            v.load(&world, LoadOptions { root: None, start_time: Some(T0) }).unwrap();

            for _ in 0..6 {
                now += 1.0 / 30.0;
                shot(&mut v, &device, &queue, now);
            }
            let first = shot(&mut v, &device, &queue, now);
            let counts = v.renderer().shader_resources();
            // The previous world's objects stay up to KEEP_FRAMES frames, so whether this world draws
            // with its shader is read from the draws of the last frame.
            let draws = v.renderer().shader_draws();
            let invalid = name.starts_with("bad-");
            assert!(invalid || (counts[1] > 0 && draws > 0), "{name}: no shader draw {counts:?}, {draws} draws");
            assert!(!invalid || draws == 0, "{name}: an invalid shader drew ({draws} draws)");

            for round in 0..5 {
                v.reload().unwrap();
                for _ in 0..6 {
                    now += 1.0 / 30.0;
                    shot(&mut v, &device, &queue, now);
                }
                let again = shot(&mut v, &device, &queue, now);
                let differ = first.chunks_exact(4).zip(again.chunks_exact(4)).filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 8)).count();
                assert!(differ == 0, "{name}: reload {round} draws a different picture ({differ} pixels)");
            }

            for _ in 0..240 {
                now += 1.0 / 30.0;
                v.step(now);
                let _ = shot(&mut v, &device, &queue, now);
            }
            let after = v.renderer().shader_resources();
            for (i, (a, b)) in after.iter().zip(counts.iter()).enumerate() {
                assert!(a <= b, "{name}: shader resource {i} grew from {b} to {a} over 5 reloads ({after:?} vs {counts:?})");
            }
            eprintln!("{name}: shader resources {counts:?} -> {after:?}");
        }

        v.unload();
        for _ in 0..240 {
            now += 1.0 / 30.0;
            let _ = shot(&mut v, &device, &queue, now);
        }
        assert_eq!(v.renderer().shader_resources(), [0; 5], "shader resources after unload");
    });
}

fn key(v: &mut EmbeddedViewer, device: &wgpu::Device, queue: &wgpu::Queue, now: &mut f64, k: &str) -> Vec<u8> {
    v.input(UiEvent::KeyDown { key: k.into() });
    v.input(UiEvent::KeyUp { key: k.into() });
    let mut last = vec![];
    for _ in 0..3 {
        *now += 1.0 / 30.0;
        last = shot(v, device, queue, *now);
    }
    last
}

fn settle(v: &mut EmbeddedViewer, device: &wgpu::Device, queue: &wgpu::Queue, now: &mut f64, frames: usize) -> Vec<u8> {
    let mut last = vec![];
    for _ in 0..frames {
        *now += 1.0 / 30.0;
        last = shot(v, device, queue, *now);
    }
    last
}

/// parts-runtime.wrl: 'k' removes the Transform T1 (whose Appearance has the ComposedShader CSA) from the live
/// Group G, 'l' adds it back; the shader program of CSA is released after the removal and made again by the add.
#[test]
fn rea_b08_shader_removal_from_live_group_releases_program() {
    let Some((device, queue)) = device() else { return eprintln!("no adapter, skipped") };
    spawn_big_stack(move || {
        let mut v = EmbeddedViewer::new(device.clone(), queue.clone(), (W, H));
        let mut now = T0;
        let world = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-b08-shaders/parts-runtime.wrl");
        v.load(&world, LoadOptions { root: None, start_time: Some(T0) }).unwrap();
        v.input(UiEvent::MouseDown { button: 0, x: 5.0, y: H as f64 - 5.0 });
        v.input(UiEvent::MouseUp { button: 0, x: 5.0, y: H as f64 - 5.0 });
        let loaded = settle(&mut v, &device, &queue, &mut now, 8);
        let with = v.renderer().shader_resources();
        assert_eq!(with[0], 2, "two programs (CSA, CSB) expected: {with:?}");

        key(&mut v, &device, &queue, &mut now, "k");
        let removed = settle(&mut v, &device, &queue, &mut now, 240);
        let without = v.renderer().shader_resources();
        assert_eq!(without[0], 1, "the removed shape kept its program: {with:?} -> {without:?}");
        assert!(without[1] < with[1] && without[2] < with[2], "pipelines and vertex buffers were kept: {with:?} -> {without:?}");
        assert!(loaded != removed, "the removal changed nothing");

        key(&mut v, &device, &queue, &mut now, "l");
        let back = settle(&mut v, &device, &queue, &mut now, 8);
        assert_eq!(v.renderer().shader_resources()[..3], with[..3], "the program was not drawn again after the add");
        let differ = loaded.chunks_exact(4).zip(back.chunks_exact(4)).filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 8)).count();
        assert_eq!(differ, 0, "the re-added shape draws a different picture ({differ} pixels)");

        v.unload();
        settle(&mut v, &device, &queue, &mut now, 240);
        assert_eq!(v.renderer().shader_resources(), [0; 5], "shader resources after unload");
    });
}

/// created-runtime.wrl: 'a' creates ShaderParts and a ComposedShader in a Script (nothing drawn), 'b' sets it
/// on the Appearance, 'c' changes a part's url (a new program), 'd' removes it, 'e' sets it again, 'f' drops all.
#[test]
fn rea_b08_script_created_shader_draws_and_is_released() {
    let Some((device, queue)) = device() else { return eprintln!("no adapter, skipped") };
    spawn_big_stack(move || {
        let mut v = EmbeddedViewer::new(device.clone(), queue.clone(), (W, H));
        let mut now = T0;
        let world = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-b08-shaders/created-runtime.wrl");
        v.load(&world, LoadOptions { root: None, start_time: Some(T0) }).unwrap();
        v.input(UiEvent::MouseDown { button: 0, x: 5.0, y: H as f64 - 5.0 });
        v.input(UiEvent::MouseUp { button: 0, x: 5.0, y: H as f64 - 5.0 });
        let plain = settle(&mut v, &device, &queue, &mut now, 8);
        assert_eq!(v.renderer().shader_resources()[0], 0);

        key(&mut v, &device, &queue, &mut now, "a");
        settle(&mut v, &device, &queue, &mut now, 4);
        assert_eq!(v.renderer().shader_resources()[0], 0, "a shader not yet in an Appearance drew");
        key(&mut v, &device, &queue, &mut now, "b");
        let drawn = settle(&mut v, &device, &queue, &mut now, 6);
        assert_eq!(v.renderer().shader_resources()[0], 1, "the Script-created ComposedShader did not draw");
        assert!(plain != drawn);

        key(&mut v, &device, &queue, &mut now, "c");
        let blue = settle(&mut v, &device, &queue, &mut now, 6);
        assert!(drawn != blue, "a part's url change did not re-link the program");

        key(&mut v, &device, &queue, &mut now, "d");
        let none = settle(&mut v, &device, &queue, &mut now, 240);
        assert_eq!(v.renderer().shader_resources(), [0; 5], "the removed Script-created shader kept resources");
        assert!(plain == none || plain.chunks_exact(4).zip(none.chunks_exact(4)).all(|(a, b)| a.iter().zip(b.iter()).all(|(x, y)| x.abs_diff(*y) <= 8)));

        key(&mut v, &device, &queue, &mut now, "e");
        settle(&mut v, &device, &queue, &mut now, 6);
        assert_eq!(v.renderer().shader_resources()[0], 1);
        key(&mut v, &device, &queue, &mut now, "f");
        settle(&mut v, &device, &queue, &mut now, 240);
        assert_eq!(v.renderer().shader_resources(), [0; 5]);
    });
}
