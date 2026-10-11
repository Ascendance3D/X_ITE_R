//! The embedding API on a real device (skipped when no Vulkan adapter exists): render into a
//! host-owned texture, input, viewpoints, reload, error cases.

use std::path::PathBuf;
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};
use x3d_viewer::viewer::UiEvent;

const T0: f64 = 946684800.0;

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { required_features: EmbeddedViewer::wanted_features(&adapter), ..Default::default() })).ok()
}

fn world(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("x3d-embed-tests-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

fn target(device: &wgpu::Device, format: wgpu::TextureFormat, w: u32, h: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let t = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("test target"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let v = t.create_view(&Default::default());
    (t, v)
}

fn read(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture, w: u32, h: u32) -> Vec<u8> {
    let bpr = (w * 4).div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (bpr * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bpr), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let data = buf.slice(..).get_mapped_range().expect("map");
    let mut out = vec![];

    for y in 0..h {
        out.extend_from_slice(&data[(y * bpr) as usize..(y * bpr + w * 4) as usize]);
    }

    out
}

const TWO_VIEWPOINTS: &str = "#VRML V2.0 utf8\nViewpoint { description \"front\" position 0 0 8 }\nViewpoint { description \"side\" position 8 0 0 orientation 0 1 0 1.5708 }\nShape { appearance Appearance { material Material { diffuseColor 1 0 0 } } geometry Box {} }\n";

#[test]
fn renders_into_a_host_texture_and_reloads() {
    let Some((device, queue)) = device() else { return eprintln!("no adapter, skipped") };
    spawn_big_stack(move || {
        let (w, h) = (200, 150);
        let mut v = EmbeddedViewer::new(device.clone(), queue.clone(), (w, h));
        let p = world("box.wrl", TWO_VIEWPOINTS);
        v.load(&p, LoadOptions { root: None, start_time: Some(T0) }).unwrap();
        assert!(v.is_loaded());

        for format in [wgpu::TextureFormat::Rgba8Unorm, wgpu::TextureFormat::Bgra8Unorm] {
            let (tex, view) = target(&device, format, w, h);
            let info = v.frame(T0 + 1.0, &view, format, w, h).unwrap();
            assert_eq!(info.draw_calls, 1);
            let px = read(&device, &queue, &tex, w, h);
            // The red box is in the middle; its red channel is the second (BGRA) or first (RGBA)
            // byte of the center pixel.
            let c = ((h / 2 * w + w / 2) * 4) as usize;
            let red = if format == wgpu::TextureFormat::Bgra8Unorm { px[c + 2] } else { px[c] };
            assert!(red > 60, "{format:?}: red box at the center: {:?}", &px[c..c + 4]);
        }

        // Steady state uploads nothing.
        let (_, view) = target(&device, wgpu::TextureFormat::Rgba8Unorm, w, h);
        let again = v.frame(T0 + 2.0, &view, wgpu::TextureFormat::Rgba8Unorm, w, h).unwrap();
        assert_eq!((again.geometry_uploads, again.texture_uploads), (0, 0));

        // sRGB targets are refused, not silently double-encoded.
        let (_, srgb) = target(&device, wgpu::TextureFormat::Rgba8UnormSrgb, w, h);
        assert!(v.frame(T0 + 3.0, &srgb, wgpu::TextureFormat::Rgba8UnormSrgb, w, h).is_err());

        // Viewpoints and navigation.
        let list = v.viewpoints();
        assert_eq!(list.iter().map(|i| i.description.as_str()).collect::<Vec<_>>(), ["front", "side"]);
        assert!(list[0].bound && !list[1].bound);
        v.step_viewpoint(1);
        v.step(T0 + 4.0);
        assert!(v.viewpoints()[1].bound);
        v.set_navigation_type("WALK");
        assert_eq!(v.navigation_type(), "WALK");

        // Reload keeps the navigation type and rebinds the first viewpoint.
        std::fs::write(&p, TWO_VIEWPOINTS.replace("diffuseColor 1 0 0", "diffuseColor 0 1 0")).unwrap();
        v.reload().unwrap();
        assert_eq!(v.navigation_type(), "WALK");
        assert!(v.viewpoints()[0].bound);
        let (tex, view) = target(&device, wgpu::TextureFormat::Rgba8Unorm, w, h);
        v.frame(T0 + 5.0, &view, wgpu::TextureFormat::Rgba8Unorm, w, h).unwrap();
        let px = read(&device, &queue, &tex, w, h);
        let c = ((h / 2 * w + w / 2) * 4) as usize;
        assert!(px[c + 1] > 60 && px[c] < 20, "the reloaded box is green: {:?}", &px[c..c + 4]);

        // Input is toolkit neutral.
        v.set_navigation_type("EXAMINE");
        v.input(UiEvent::MouseDown { button: 0, x: 100.0, y: 75.0 });
        v.input(UiEvent::MouseMove { x: 140.0, y: 90.0 });
        v.input(UiEvent::MouseUp { button: 0, x: 140.0, y: 90.0 });

        // A resize is picked up from the target size.
        let (_, big) = target(&device, wgpu::TextureFormat::Rgba8Unorm, 320, 240);
        v.frame(T0 + 6.0, &big, wgpu::TextureFormat::Rgba8Unorm, 320, 240).unwrap();
        assert_eq!(v.size(), (320, 240));

        // No world: the target is just cleared.
        v.unload();
        v.frame(T0 + 7.0, &big, wgpu::TextureFormat::Rgba8Unorm, 320, 240).unwrap();
    });
}
