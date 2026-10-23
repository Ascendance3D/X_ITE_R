//! The smallest host of the embedding API, without a window: own wgpu device, a target texture
//! the host owns, scripted input, and a PNG written from the host's own readback.
//!
//!   cargo run --release -p x3d-viewer --example embed_minimal -- world.wrl out.png
//!
//! A real host (a Tauri or winit app) does the same per redraw, with the surface's texture view
//! instead of its own texture.

use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, realtime_now, spawn_big_stack};
use x3d_viewer::viewer::UiEvent;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (world, out) = (
        args.get(1).expect("world.wrl").clone(),
        args.get(2).expect("out.png").clone(),
    );
    // Scripts run in QuickJS, which needs a big stack: own the viewer on such a thread.
    spawn_big_stack(move || host(&world, &out));
}

fn host(world: &str, out: &str) {
    // 1. The host's device and queue (a Tauri app already has these from its own wgpu setup).
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: EmbeddedViewer::wanted_features(&adapter), // optional: GPU timing
        ..Default::default()
    }))
    .expect("device");

    // 2. The viewer on that device; load a world.
    let (w, h) = (800u32, 600u32);
    let mut viewer = EmbeddedViewer::new(device.clone(), queue.clone(), (w, h));
    viewer
        .load(std::path::Path::new(world), LoadOptions::default())
        .expect("load");

    // 3. The host's target: a non-sRGB texture it renders into and can show or copy anywhere.
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("host target"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());

    // 4. Per redraw: forward input (pixels, origin bottom left), then frame().
    let now = realtime_now();
    viewer.input(UiEvent::MouseDown {
        button: 0,
        x: 400.0,
        y: 300.0,
    });

    for i in 1..=10 {
        viewer.input(UiEvent::MouseMove {
            x: 400.0 + 8.0 * i as f64,
            y: 300.0 + 3.0 * i as f64,
        });
        let info = viewer
            .frame(now + i as f64 / 60.0, &view, format, w, h)
            .expect("frame");
        eprintln!(
            "frame {i}: step {:.2} ms, build {:.2} ms, submit {:.2} ms, gpu {:?}, {} draws",
            info.step_ms, info.build_ms, info.render_ms, info.gpu_ms, info.draw_calls
        );
    }

    viewer.input(UiEvent::MouseUp {
        button: 0,
        x: 480.0,
        y: 330.0,
    });
    println!(
        "navigation {} | viewpoints {:?} | status {:?}",
        viewer.navigation_type(),
        viewer.viewpoints(),
        viewer.status()
    );

    // 5. The host reads its own texture back (a real host just samples or presents it).
    let bytes_per_row = (w * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (bytes_per_row * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let data = buffer.slice(..).get_mapped_range().expect("map");
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);

    for y in 0..h {
        for px in
            data[(y * bytes_per_row) as usize..(y * bytes_per_row + w * 4) as usize].chunks_exact(4)
        {
            rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
        }
    }

    x3d_render::write_png(std::path::Path::new(out), w, h, &rgba).expect("png");
    eprintln!("wrote {out}");
}
