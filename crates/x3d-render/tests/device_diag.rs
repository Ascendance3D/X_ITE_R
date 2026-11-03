//! Device diagnostics (issue #89), information only: these tests never fail on what they find. They print what the
//! graphics device is and offers, how a 24-bit-or-better depth format behaves on it, and how a few of the comparisons that
//! fail on the hosted macOS virtual machine differ from the pinned images. The hosted-macOS job runs them with
//! `--no-capture`, with `X3D_DIAG_DIR` set, and keeps the output and the images as an artifact. A result from a virtual
//! machine is never a physical-device result.
//!
//! Lines start with `diag:` so that a job can pick them out.

use std::path::PathBuf;
use x3d_render::gpu::{DepthMode, Gpu, headless_backends};

fn instance_and_adapter() -> Option<(wgpu::Instance, wgpu::Adapter)> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = headless_backends(std::env::var("X3D_BACKEND").ok().as_deref()).ok()?;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .ok()?;
    Some((instance, adapter))
}

#[test]
fn adapter_features_limits_and_depth_formats() {
    let Some((_instance, adapter)) = instance_and_adapter() else {
        println!("diag: no adapter");
        return;
    };

    println!("diag: adapter {:?}", adapter.get_info());
    println!("diag: features {:?}", adapter.features());
    println!("diag: limits {:#?}", adapter.limits());
    println!("diag: downlevel {:?}", adapter.get_downlevel_capabilities());

    for f in [
        wgpu::TextureFormat::Depth16Unorm,
        wgpu::TextureFormat::Depth24Plus,
        wgpu::TextureFormat::Depth24PlusStencil8,
        wgpu::TextureFormat::Depth32Float,
        wgpu::TextureFormat::Depth32FloatStencil8,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba16Float,
        wgpu::TextureFormat::Rgba32Float,
    ] {
        let ff = adapter.get_texture_format_features(f);

        println!(
            "diag: format {f:?} usages {:?} msaa4 {} msaa8 {} resolve {}",
            ff.allowed_usages,
            ff.flags.sample_count_supported(4),
            ff.flags.sample_count_supported(8),
            ff.flags
                .contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE),
        );
    }
}

/// Two full-screen triangles at depths 0.1 and 0.1 + 3e-8, drawn nearer first with LessEqual. A 24-bit unorm depth buffer
/// (step 6e-8) ties them and the second wins (green); a float32 buffer (step 7.5e-9 near 0.1) keeps the first (red).
#[test]
fn depth_format_precision() {
    let Some((_instance, adapter)) = instance_and_adapter() else {
        println!("diag: no adapter");
        return;
    };
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(
            "
@vertex fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((v << 1u) & 2u), f32(v & 2u)) * 2.0 - 1.0;
    var z = 0.1;
    if (i == 1u) { z = 0.1 + 3.0e-8; }
    return vec4<f32>(p, z, 1.0);
}
@fragment fn fs0() -> @location(0) vec4<f32> { return vec4<f32>(1.0, 0.0, 0.0, 1.0); }
@fragment fn fs1() -> @location(0) vec4<f32> { return vec4<f32>(0.0, 1.0, 0.0, 1.0); }
"
            .into(),
        ),
    });

    for depth_format in [
        wgpu::TextureFormat::Depth24Plus,
        wgpu::TextureFormat::Depth32Float,
    ] {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let make = |entry: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
                }),
                primitive: Default::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: depth_format,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let (red, green) = (make("fs0"), make("fs1"));
        let tex = |format, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: 64,
                    height: 64,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let color = tex(
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let depth = tex(depth_format, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let out = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256 * 64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&Default::default());

        {
            let cv = color.create_view(&Default::default());
            let dv = depth.create_view(&Default::default());
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &cv,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &dv,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_pipeline(&red);
            pass.draw(0..3, 0..1);
            pass.set_pipeline(&green);
            pass.draw(0..3, 1..2);
        }

        enc.copy_texture_to_buffer(
            color.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &out,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(64),
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([enc.finish()]);
        out.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let data = out.slice(..).get_mapped_range().unwrap();
        let px = &data[(32 * 256 + 32 * 4)..(32 * 256 + 32 * 4 + 4)];

        println!(
            "diag: depth {depth_format:?}: centre pixel {px:?} ({})",
            if px[1] > 128 {
                "the 3e-8 difference at depth 0.1 is not resolved (24-bit unorm behavior)"
            } else {
                "the 3e-8 difference at depth 0.1 is resolved (float32 behavior)"
            }
        );
    }
}

/// Where the images go, if the job asked for them.
fn diag_dir() -> Option<PathBuf> {
    let d = PathBuf::from(std::env::var_os("X3D_DIAG_DIR")?);
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

fn save(dir: &std::path::Path, name: &str, w: u32, h: u32, rgba: &[u8]) {
    if let Some(img) = image::RgbaImage::from_raw(w, h, rgba.to_vec()) {
        let _ = img.save(dir.join(name));
    }
}

/// The bounding box of the pixels more than 8 off, and the mean signed channel difference there.
fn differing(w: u32, a: &[u8], b: &[u8]) -> String {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    let (mut n, mut sum) = (0u64, [0i64; 3]);

    for (i, (p, q)) in a.chunks_exact(4).zip(b.chunks_exact(4)).enumerate() {
        if (0..3).any(|c| p[c].abs_diff(q[c]) > 8) {
            let (x, y) = (i as u32 % w, i as u32 / w);

            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            n += 1;

            for c in 0..3 {
                sum[c] += p[c] as i64 - q[c] as i64;
            }
        }
    }

    if n == 0 {
        return "no pixel over 8".into();
    }

    format!(
        "{n} pixels over 8 within x {x0}..{x1}, y {y0}..{y1}; mean (Rust - X_ITE) per channel {:.1} {:.1} {:.1}",
        sum[0] as f64 / n as f64,
        sum[1] as f64 / n as f64,
        sum[2] as f64 / n as f64
    )
}

/// Pixels more than 8 off per tile of an 8 x 8 grid, one line per tile row (where in the image the difference is).
fn tiles(w: u32, h: u32, a: &[u8], b: &[u8]) -> String {
    let mut grid = [[0u32; 8]; 8];

    for (i, (p, q)) in a.chunks_exact(4).zip(b.chunks_exact(4)).enumerate() {
        if (0..3).any(|c| p[c].abs_diff(q[c]) > 8) {
            let (x, y) = (i as u32 % w, i as u32 / w);
            grid[(y * 8 / h) as usize][(x * 8 / w) as usize] += 1;
        }
    }

    grid.iter()
        .map(|r| {
            r.iter()
                .map(|n| format!("{n:5}"))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join(" / ")
}

/// The failing worlds, rendered, compared and saved (Rust image, pinned image) when `X3D_DIAG_DIR` is set.
#[test]
fn failing_worlds_against_the_pinned_images() {
    let Ok(mut g) = Gpu::new() else {
        println!("diag: no adapter");
        return;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap();
    let dir = diag_dir();

    // (depth mode, baseline directory, world directory, file name of the world, name of the baseline image)
    let cases = [
        (
            DepthMode::Standard,
            "render-zfight-baseline",
            "synthetic-zfight",
            "tilt-pair.wrl",
            "tilt-pair.wrl.png",
        ),
        (
            DepthMode::Standard,
            "render-zfight-baseline",
            "synthetic-zfight",
            "far-range.wrl",
            "far-range.wrl.png",
        ),
        (
            DepthMode::Logarithmic,
            "render-zfight-log-baseline",
            "synthetic-zfight",
            "tilt-pair.wrl",
            "tilt-pair.wrl.png",
        ),
        (
            DepthMode::Logarithmic,
            "render-zfight-log-baseline",
            "synthetic-zfight",
            "far-range.wrl",
            "far-range.wrl.png",
        ),
        (
            DepthMode::Standard,
            "render-b69-gs-log-baseline",
            "synthetic-rea-b08-gs",
            "gs-linear.wrl",
            "gs-linear.wrl.png",
        ),
        (
            DepthMode::Logarithmic,
            "render-b69-gs-log-baseline",
            "synthetic-rea-b08-gs",
            "gs-linear.wrl",
            "gs-linear.wrl.png",
        ),
        (
            DepthMode::Logarithmic,
            "render-b69-gs-log-baseline",
            "synthetic-rea-b08-gs",
            "gs-ortho.wrl",
            "gs-ortho.wrl.png",
        ),
        (
            DepthMode::Standard,
            "render-b69-gs-log-baseline",
            "synthetic-rea-b08-gs",
            "gs-ortho.wrl",
            "gs-ortho.wrl.png",
        ),
        (
            DepthMode::Standard,
            "render-rea-b09-inputs-baseline",
            "synthetic-rea-b09-inputs",
            "texgen-modes.wrl",
            "texgen-modes.wrl.png",
        ),
        (
            DepthMode::Standard,
            "render-rea-b09-inputs-baseline",
            "synthetic-rea-b09-inputs",
            "texgen-params.wrl",
            "texgen-params.wrl.png",
        ),
        (
            DepthMode::Standard,
            "render-rea-b08-vs-baseline",
            "synthetic-rea-b08-vs",
            "vs-transparent-ibl.wrl",
            "vs-transparent-ibl.wrl.png",
        ),
        (
            DepthMode::Standard,
            "render-rea-b07-vol-baseline",
            "synthetic-rea-b07-vol/static",
            "sg-enabled.wrl",
            "sg-enabled.wrl.png",
        ),
        (
            DepthMode::Standard,
            "render-rea-b07-vol-baseline",
            "synthetic-rea-b07-vol/static",
            "vx-opaque-all-styles.wrl",
            "vx-opaque-all-styles.wrl.png",
        ),
    ];

    for (mode, baseline, world, file, png) in cases {
        let want_path = root.join(format!("{baseline}/{png}"));
        let Ok((w, h, want)) = x3d_render::compare::load_png(&want_path) else {
            println!("diag: {mode:?} {world}/{file}: no baseline image {baseline}/{png}");
            continue;
        };

        g.renderer.set_depth_mode(mode);
        let shot =
            x3d_render::render_file(&mut g, &root, &root.join(format!("{world}/{file}")), w, h);
        g.renderer.set_depth_mode(DepthMode::Standard);
        let Ok(shot) = shot else {
            println!("diag: {mode:?} {world}/{file}: the render failed");
            continue;
        };
        let rgba: Vec<u8> = shot
            .rgba
            .chunks_exact(4)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect();
        let s = x3d_render::compare::compare(w, h, &want, &rgba, 8);

        println!(
            "diag: tiles {mode:?} {world}/{file}: {}",
            tiles(w, h, &rgba, &want)
        );
        println!(
            "diag: {mode:?} {world}/{file}: over {} ({:.3}%), max delta {}; {}",
            s.pixels_over,
            s.pct_over,
            s.max_delta,
            differing(w, &rgba, &want)
        );

        if let Some(dir) = &dir {
            let stem = format!("{mode:?}-{}", file.trim_end_matches(".wrl"));

            save(dir, &format!("{stem}-rust.png"), w, h, &rgba);
            save(dir, &format!("{stem}-pinned.png"), w, h, &want);
        }
    }
}

/// Tiny synthetic worlds: is a Background drawn under an OrthoViewpoint (the gs-ortho case shows a black image where the
/// pinned image has the sky color), with and without logarithmic depth, with the cube and the sphere form of the Background?
#[test]
fn background_probes() {
    let Ok(mut g) = Gpu::new() else {
        println!("diag: no adapter");
        return;
    };
    let dir = std::env::temp_dir().join("x3d-device-diag-worlds");
    std::fs::create_dir_all(&dir).unwrap();
    let box_ = "Shape { geometry Box { size 1 1 1 } }";
    let worlds = [
        (
            "ortho-cube-bg",
            format!(
                "#X3D V4.1 utf8\nPROFILE Interchange\nOrthoViewpoint {{ position 0 0 7 }}\nBackground {{ skyColor 0.2 0.2 0.28 }}\n{box_}\n"
            ),
        ),
        (
            "persp-cube-bg",
            format!(
                "#X3D V4.1 utf8\nPROFILE Interchange\nViewpoint {{ position 0 0 7 }}\nBackground {{ skyColor 0.2 0.2 0.28 }}\n{box_}\n"
            ),
        ),
        (
            "ortho-sphere-bg",
            format!(
                "#X3D V4.1 utf8\nPROFILE Interchange\nOrthoViewpoint {{ position 0 0 7 }}\nBackground {{ skyColor [0.2 0.2 0.28, 0.8 0.2 0.2] skyAngle 1.5 }}\n{box_}\n"
            ),
        ),
        (
            "ortho-small-far",
            format!(
                "#X3D V4.1 utf8\nPROFILE Interchange\nOrthoViewpoint {{ position 0 0 7 }}\nNavigationInfo {{ visibilityLimit 50 }}\nBackground {{ skyColor 0.2 0.2 0.28 }}\n{box_}\n"
            ),
        ),
    ];

    for (name, text) in worlds {
        let path = dir.join(format!("{name}.x3dv"));
        std::fs::write(&path, text).unwrap();

        for mode in [DepthMode::Standard, DepthMode::Logarithmic] {
            g.renderer.set_depth_mode(mode);
            let shot = x3d_render::render_file(&mut g, &dir, &path, 128, 128);
            g.renderer.set_depth_mode(DepthMode::Standard);

            match shot {
                Ok(s) => {
                    let px = |x: usize, y: usize| {
                        let i = (y * 128 + x) * 4;
                        [s.rgba[i], s.rgba[i + 1], s.rgba[i + 2]]
                    };
                    println!(
                        "diag: background {name} {mode:?}: corner {:?} edge {:?} center {:?}",
                        px(2, 2),
                        px(120, 64),
                        px(64, 64)
                    );
                }
                Err(e) => println!("diag: background {name} {mode:?}: render failed {e:?}"),
            }
        }
    }
}
