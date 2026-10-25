//! Depth modes of the embedded viewer (see `x3d_render::gpu::DepthMode`): the default is X_ITE's
//! standard buffer, the modes cycle, and a scene without near-coplanar surfaces looks the same in
//! all of them. Skipped when no Vulkan adapter exists.

mod common;

use common::gpu::device;
use std::path::PathBuf;
use x3d_viewer::embed::{DepthMode, EmbeddedViewer, LoadOptions, spawn_big_stack};

const T0: f64 = 946684800.0;

fn world(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("x3d-embed-tests-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

fn target(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    w: u32,
    h: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let t = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("test target"),
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
    let v = t.create_view(&Default::default());
    (t, v)
}

fn read(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    w: u32,
    h: u32,
) -> Vec<u8> {
    let bpr = (w * 4).div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (bpr * h) as u64,
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
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
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

const BOX: &str = "#VRML V2.0 utf8\nViewpoint { position 0 0 8 }\nShape { appearance Appearance { material Material { diffuseColor 1 0 0 } } geometry Box {} }\n";

#[test]
fn depth_modes_cycle_and_agree_on_plain_scenes() {
    let Some((device, queue)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let (w, h) = (160, 120);
        let mut v = EmbeddedViewer::new(device.clone(), queue.clone(), (w, h));
        assert_eq!(v.depth_mode(), DepthMode::Standard);
        v.set_multisampling(1);
        let p = world("depth-box.wrl", BOX);
        v.load(
            &p,
            LoadOptions {
                root: None,
                start_time: Some(T0),
            },
        )
        .unwrap();
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut images = vec![];

        for expected in [
            DepthMode::Reversed,
            DepthMode::Logarithmic,
            DepthMode::Standard,
        ] {
            let (tex, view) = target(&device, format, w, h);
            v.frame(T0 + 1.0, &view, format, w, h).unwrap();
            images.push(read(&device, &queue, &tex, w, h));
            assert_eq!(v.cycle_depth_mode(), expected);
        }

        // The Box is the same in all three (silhouette pixels can differ by one level of
        // rounding at most; the Box here is axis aligned).
        for pair in images.windows(2) {
            let worst = pair[0]
                .iter()
                .zip(&pair[1])
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap();
            assert!(
                worst <= 8,
                "depth modes disagree on a plain scene (worst {worst})"
            );
        }
    });
}
