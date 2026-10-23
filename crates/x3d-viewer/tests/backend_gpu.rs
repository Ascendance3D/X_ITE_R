//! Backend startup on a real device (`x3d_viewer::backend`): a backend that cannot run the
//! renderer is an error, not a panic; on Windows, the default DX12 + DXC start draws frames
//! offscreen, the first with the main pipeline prepared while the renderer started. Skipped
//! without the adapter they need.

mod common;

use common::gpu::serialized;
use x3d_viewer::backend::create_viewer;
#[cfg(windows)]
use {
    std::time::Instant,
    x3d_viewer::backend::{GpuErrors, instance_descriptor},
    x3d_viewer::embed::{LoadOptions, spawn_big_stack},
};

#[cfg(windows)]
const T0: f64 = 946684800.0;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

/// An adapter and device of `desc` (with the viewer's wanted features, as `x3d-view` asks, when
/// `wanted`); why not, when there is none.
fn start(
    desc: wgpu::InstanceDescriptor,
    wanted: bool,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), String> {
    serialized(|| {
        let instance = wgpu::Instance::new(desc);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("backend test"),
            required_features: if wanted {
                x3d_viewer::embed::EmbeddedViewer::wanted_features(&adapter)
            } else {
                wgpu::Features::empty()
            },
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        Ok((adapter, device, queue))
    })
}

/// GL cannot translate the scatter-depth compute shader (`textureLoad` of a depth texture), so
/// the renderer cannot start there. That is an error naming the backend, not wgpu's panic.
/// On the NVIDIA Linux driver the GL device does not start; Mesa's runs it
/// (`__EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json`).
#[test]
fn gl_backend_cannot_run_the_renderer_and_says_so() {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::GL;
    let (adapter, device, queue) = match start(desc, false) {
        Ok(s) => s,
        Err(e) => return eprintln!("no GL device ({e}), skipped"),
    };

    match create_viewer(&device, &queue, (64, 64), &adapter, FORMAT) {
        Err(e) => {
            eprintln!("{e}");
            assert!(e.contains("Gl backend cannot run this renderer"), "{e}");
        }
        Ok(_) => panic!(
            "the renderer started on GL ({}); update this test",
            adapter.get_info().name
        ),
    }
}

/// The Windows default: DX12 with `dxcompiler.dll` beside the test executable. One box, 30
/// frames into a texture; prints the adapter, compiler and frame times.
#[cfg(windows)]
#[test]
#[ignore = "needs a DX12 GPU and dxcompiler.dll beside the test executable"]
fn windows_default_dx12_draws_offscreen_frames() {
    let t = Instant::now();
    let desc = instance_descriptor().expect("instance options");
    assert_eq!(
        desc.backends,
        wgpu::Backends::DX12,
        "the Windows default backend (WGPU_BACKEND unset)"
    );
    eprintln!(
        "DX12 compiler: {:?}",
        desc.backend_options.dx12.shader_compiler
    );
    let (adapter, device, queue) = start(desc, true).expect("a DX12 device");
    let info = adapter.get_info();
    eprintln!(
        "adapter: {} ({:?}, {:?}, driver {} {})",
        info.name, info.backend, info.device_type, info.driver, info.driver_info
    );
    assert_eq!(info.backend, wgpu::Backend::Dx12);
    eprintln!("device: {:.3} s", t.elapsed().as_secs_f64());

    spawn_big_stack(move || {
        let (w, h) = (320, 240);
        let errors = GpuErrors::install(&device);
        let t = Instant::now();
        let mut v = create_viewer(&device, &queue, (w, h), &adapter, FORMAT).expect("renderer");
        eprintln!("renderer: {:.3} s", t.elapsed().as_secs_f64());
        let dir = std::env::temp_dir().join(format!("x3d-backend-tests-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let world = dir.join("box.wrl");
        std::fs::write(&world, "#VRML V2.0 utf8\nShape { appearance Appearance { material Material { diffuseColor 1 0 0 } } geometry Box {} }\n").unwrap();
        v.load(
            &world,
            LoadOptions {
                root: None,
                start_time: Some(T0),
            },
        )
        .expect("load");
        let t = Instant::now();
        v.renderer().finish_prepared();
        eprintln!(
            "prepared pipeline: {:.3} s after load",
            t.elapsed().as_secs_f64()
        );

        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen target"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let mut times = vec![];

        for k in 1..=30 {
            let t = Instant::now();
            let f = v
                .frame(T0 + k as f64 / 60.0, &view, FORMAT, w, h)
                .expect("frame");
            device
                .poll(wgpu::PollType::wait_indefinitely())
                .expect("poll");
            times.push(t.elapsed().as_secs_f64());
            assert_eq!(f.draw_calls, 1, "frame {k}");
            assert_eq!(
                v.renderer().stats.pipelines_created,
                0,
                "frame {k}: the box draws with the prepared pipeline"
            );
            assert_eq!(errors.take(), None, "GPU error in frame {k}");
        }

        let rest = &times[1..];
        eprintln!(
            "frame 1: {:.3} s; frames 2-30: min {:.2} ms, max {:.2} ms",
            times[0],
            rest.iter().cloned().fold(f64::INFINITY, f64::min) * 1e3,
            rest.iter().cloned().fold(0.0, f64::max) * 1e3
        );
        let _ = std::fs::remove_dir_all(&dir);
    });
}
