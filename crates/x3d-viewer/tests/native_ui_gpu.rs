//! The native UI on a real device (skipped when no Vulkan adapter exists): the context menu and the Notification
//! drawn over the world by `EmbeddedViewer::frame`, and gone again when they close.

mod common;

use common::gpu::device;
use std::path::PathBuf;
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};
use x3d_viewer::viewer::UiEvent;

const T0: f64 = 946684800.0;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn world(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("x3d-native-ui-gpu-{}", std::process::id()));
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

fn px(data: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    [data[i], data[i + 1], data[i + 2], data[i + 3]]
}

#[test]
fn menu_and_notification_are_drawn_over_the_world() {
    let Some((device, queue)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let (w, h) = (640, 480);
        let mut v = EmbeddedViewer::new(device.clone(), queue.clone(), (w, h));
        let p = world(
            "box.wrl",
            "#VRML V2.0 utf8\nBackground { skyColor 0 0 0 }\nShape { appearance Appearance { material Material { diffuseColor 1 0 0 } } geometry Box {} }\n",
        );

        v.load(
            &p,
            LoadOptions {
                root: None,
                start_time: Some(T0),
            },
        )
        .unwrap();

        let (tex, view) = target(&device, FORMAT, w, h);
        let mut now = T0 + 1.0;
        let mut shot = |v: &mut EmbeddedViewer, now: &mut f64| {
            *now += 1.0 / 30.0;
            v.frame(*now, &view, FORMAT, w, h).unwrap();
            read(&device, &queue, &tex, w, h)
        };
        let plain = shot(&mut v, &mut now);

        // The world's own pixels: a black background, the red box in the middle.
        assert_eq!(px(&plain, w, 10, 10)[..3], [0, 0, 0]);
        assert!(px(&plain, w, w / 2, h / 2)[0] > 60);

        // A right press opens the menu at the pointer (events have a bottom-left origin); its body is light.
        v.input(UiEvent::MouseMove {
            x: 40.0,
            y: h as f64 - 40.0,
        });
        v.input(UiEvent::MouseDown {
            button: 2,
            x: 40.0,
            y: h as f64 - 40.0,
        });
        v.input(UiEvent::MouseUp {
            button: 2,
            x: 40.0,
            y: h as f64 - 40.0,
        });
        assert!(v.core().expect("world").context_menu().is_some());

        let open = shot(&mut v, &mut now);
        let body = px(&open, w, 60, 60);

        assert!(
            body[0] > 180 && body[1] > 180 && body[2] > 180,
            "menu body over the black background: {body:?}"
        );
        assert_eq!(
            px(&open, w, 400, 400),
            px(&plain, w, 400, 400),
            "the rest of the world is as before"
        );

        // The press on the layer closes it and the world is as it was.
        v.input(UiEvent::MouseDown {
            button: 0,
            x: 500.0,
            y: 100.0,
        });
        v.input(UiEvent::MouseUp {
            button: 0,
            x: 500.0,
            y: 100.0,
        });
        assert!(v.core().expect("world").context_menu().is_none());

        let closed = shot(&mut v, &mut now);

        assert_eq!(px(&closed, w, 60, 60), px(&plain, w, 60, 60));

        // The notification: top right, 20 px down.
        v.core().expect("world").set_description("Notice");
        for _ in 0..30 {
            now += 1.0 / 30.0;
            v.step(now);
        }

        let noted = shot(&mut v, &mut now);
        let pill = px(&noted, w, w - 10, 28);

        assert!(pill[0] > 180, "notification pill: {pill:?}");

        // Five seconds later it is gone.
        for _ in 0..200 {
            now += 1.0 / 30.0;
            v.step(now);
        }

        let gone = shot(&mut v, &mut now);

        assert_eq!(px(&gone, w, w - 10, 28), px(&plain, w, w - 10, 28));
    });
}

#[test]
fn timings_panel_is_drawn_at_the_bottom_left_and_gone_again() {
    let Some((device, queue)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let (w, h) = (640, 480);
        let mut v = EmbeddedViewer::new(device.clone(), queue.clone(), (w, h));
        let p = world(
            "timings.wrl",
            "#VRML V2.0 utf8\nBackground { skyColor 0 0 0 }\nShape { geometry Box {} }\n",
        );

        v.load(
            &p,
            LoadOptions {
                root: None,
                start_time: Some(T0),
            },
        )
        .unwrap();

        let (tex, view) = target(&device, FORMAT, w, h);
        let mut now = T0 + 1.0;
        let shot = |v: &mut EmbeddedViewer, now: &mut f64, n: usize| {
            for _ in 0..n {
                *now += 1.0 / 30.0;
                v.frame(*now, &view, FORMAT, w, h).unwrap();
            }

            read(&device, &queue, &tex, w, h)
        };
        // The corner inside the panel (padding), and a pixel above it.
        let (cx, cy) = (6, h - 6);
        let plain = shot(&mut v, &mut now, 3);

        assert_eq!(px(&plain, w, cx, cy)[..3], [0, 0, 0]);

        // Ctrl+Plus through the key events of the window.
        for (down, key, code) in [(true, "Control", 17), (true, "=", 187), (false, "=", 187)] {
            v.input(UiEvent::NativeKey {
                down,
                key: key.into(),
                code,
            });
        }

        let on = shot(&mut v, &mut now, 3);
        let light = px(&on, w, cx, cy);

        assert!(light[0] > 200 && light[1] > 200, "panel body {light:?}");
        // The panel is small: the middle of the world is not covered.
        assert_eq!(px(&on, w, w - 6, 6)[..3], [0, 0, 0]);

        // The same keys again hide it.
        for (down, key, code) in [(true, "=", 187), (false, "=", 187)] {
            v.input(UiEvent::NativeKey {
                down,
                key: key.into(),
                code,
            });
        }

        let off = shot(&mut v, &mut now, 3);

        assert_eq!(px(&off, w, cx, cy)[..3], [0, 0, 0]);
    });
}
