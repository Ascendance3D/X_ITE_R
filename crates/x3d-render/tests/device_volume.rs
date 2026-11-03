//! Volume rendering diagnostics (issue #89), information only: these tests never fail on what they find. On the hosted
//! macOS device the VolumeRendering comparisons (`rea_b07_vol`, `rea_batch07_vol`) differ from the pinned images by 0.6%
//! to 1.1% of the pixels, only along the contours of the styles. The volume shader reads the voxels with a trilinear
//! `textureSampleLevel` and takes central differences of those samples for the gradient normal (volume.rs `normal_fn`),
//! which the styles then threshold. Two hypotheses, separated here by small compute shaders on the device:
//!
//! * H1, the trilinear filter: how many weight levels does the device use between two texels (8 bits give 256), and how
//!   far is the filtered value from the exact one? (`diag: filter ...`)
//! * H2, the gradient: does the normal that comes out of those samples differ from the exact one by more than the
//!   filter alone explains? (`diag: gradient ...`) The same positions are also evaluated on the CPU with the weight
//!   quantized to 8 bits (rounded and truncated), so a device that matches one of them shows that the quantization is the
//!   cause.
//!
//! A result from a virtual machine is never a physical-device result. Lines start with `diag:`.

use x3d_render::gpu::headless_backends;

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = headless_backends(std::env::var("X3D_BACKEND").ok().as_deref()).ok()?;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
}

const SHADER: &str = "
@group(0) @binding(0) var t3: texture_3d<f32>;
@group(0) @binding(1) var t2: texture_2d<f32>;
@group(0) @binding(2) var s: sampler;
@group(0) @binding(3) var<storage, read> pos: array<vec4<f32>>;
@group(0) @binding(4) var<storage, read_write> out: array<vec4<f32>>;

fn i3(p: vec3<f32>) -> f32 { return textureSampleLevel(t3, s, p, 0.0).r; }

// 0: the filter in 3D (the sample itself), 1: the filter in 2D, 2: the gradient of the volume.rs normal_fn.
@compute @workgroup_size(64) fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if (i >= arrayLength(&pos)) { return; }
    let p = pos[i].xyz;
    let mode = u32(pos[i].w);
    if (mode == 0u) {
        out[i] = textureSampleLevel(t3, s, p, 0.0);
    } else if (mode == 1u) {
        out[i] = textureSampleLevel(t2, s, p.xy, 0.0);
    } else {
        let offset = vec4<f32>(1.0 / vec3<f32>(textureDimensions(t3, 0)), 0.0);
        let i0 = i3(p + offset.xww);
        let i1 = i3(p - offset.xww);
        let i2 = i3(p + offset.wyw);
        let i3_ = i3(p - offset.wyw);
        let i4 = i3(p + offset.wwz);
        let i5 = i3(p - offset.wwz);
        let n = vec3<f32>(i1 - i0, i3_ - i2, i5 - i4);
        out[i] = vec4<f32>(n, length(n));
    }
}
";

struct Probe {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    sampler: wgpu::Sampler,
}

impl Probe {
    fn new() -> Option<Probe> {
        let (device, queue) = device()?;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Some(Probe {
            device,
            queue,
            pipeline,
            sampler,
        })
    }

    fn texture(
        &self,
        dim: wgpu::TextureDimension,
        n: [u32; 3],
        texels: &[u8],
    ) -> wgpu::TextureView {
        let t = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: n[0],
                height: n[1],
                depth_or_array_layers: n[2],
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: dim,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            t.as_image_copy(),
            texels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * n[0]),
                rows_per_image: Some(n[1]),
            },
            wgpu::Extent3d {
                width: n[0],
                height: n[1],
                depth_or_array_layers: n[2],
            },
        );
        t.create_view(&Default::default())
    }

    /// Run `items` (x, y, z, mode) against the two textures; the results.
    fn run(
        &self,
        v3: &wgpu::TextureView,
        v2: &wgpu::TextureView,
        items: &[[f32; 4]],
    ) -> Vec<[f32; 4]> {
        let bytes = (items.len() * 16) as u64;
        let make = |usage| {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: bytes,
                usage,
                mapped_at_creation: false,
            })
        };
        let input = make(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        let output = make(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC);
        let read = make(wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST);
        self.queue
            .write_buffer(&input, 0, bytemuck::cast_slice(items));
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(v3),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(v2),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: input.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: output.as_entire_binding(),
                },
            ],
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups((items.len() as u32).div_ceil(64), 1, 1);
        }
        enc.copy_buffer_to_buffer(&output, 0, &read, 0, bytes);
        self.queue.submit([enc.finish()]);
        let slice = read.slice(..bytes);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        let data = slice.get_mapped_range().unwrap();
        bytemuck::cast_slice::<u8, [f32; 4]>(&data).to_vec()
    }
}

/// How the fraction of a trilinear weight is made on the CPU.
#[derive(Clone, Copy)]
enum Weight {
    Exact,
    /// Rounded to 8 fractional bits.
    Round8,
    /// Truncated to 8 fractional bits.
    Floor8,
}

impl Weight {
    fn apply(self, w: f64) -> f64 {
        match self {
            Weight::Exact => w,
            Weight::Round8 => (w * 256.0).round() / 256.0,
            Weight::Floor8 => (w * 256.0).floor() / 256.0,
        }
    }
}

/// Trilinear sample of the red channel of `texels` (n^3, clamp to edge) at `p` (texture coordinates).
fn trilinear(texels: &[u8], n: usize, p: [f64; 3], weight: Weight) -> f64 {
    let mut i = [0isize; 3];
    let mut f = [0.0f64; 3];

    for a in 0..3 {
        let x = p[a] * n as f64 - 0.5;
        let fl = x.floor();
        i[a] = fl as isize;
        f[a] = weight.apply(x - fl);
    }

    let at = |x: isize, y: isize, z: isize| {
        let c = |v: isize| v.clamp(0, n as isize - 1) as usize;
        texels[((c(z) * n + c(y)) * n + c(x)) * 4] as f64 / 255.0
    };
    let (x0, y0, z0) = (i[0], i[1], i[2]);
    let mut sum = 0.0;

    for dz in 0..2 {
        for dy in 0..2 {
            for dx in 0..2 {
                let w = [f[0], f[1], f[2]];
                let k = |d: isize, f: f64| if d == 1 { f } else { 1.0 - f };
                sum += k(dx, w[0]) * k(dy, w[1]) * k(dz, w[2]) * at(x0 + dx, y0 + dy, z0 + dz);
            }
        }
    }

    sum
}

fn lcg(s: &mut u32) -> f64 {
    *s = s.wrapping_mul(1664525).wrapping_add(1013904223);
    (*s >> 8) as f64 / (1u32 << 24) as f64
}

#[test]
fn filter_weights() {
    let Some(probe) = Probe::new() else {
        return eprintln!("diag: no device for the filter probe");
    };
    // 2 x 2 x 2: the red channel is 255 along x, the green along y, the blue along z, so the filtered value of each channel
    // between the texel centers (0.25 and 0.75) is that axis' weight itself.
    let mut t3 = vec![];
    for z in 0..2u8 {
        for y in 0..2u8 {
            for x in 0..2u8 {
                t3.extend([x * 255, y * 255, z * 255, 255]);
            }
        }
    }
    let v3 = probe.texture(wgpu::TextureDimension::D3, [2, 2, 2], &t3);
    let t2: Vec<u8> = (0..4u8)
        .flat_map(|i| [(i & 1) * 255, (i >> 1) * 255, 0, 255])
        .collect();
    let v2 = probe.texture(wgpu::TextureDimension::D2, [2, 2, 1], &t2);
    const N: usize = 8192;
    let c = |i: usize| 0.25 + 0.5 * (i % N) as f32 / (N - 1) as f32;
    let at = |i: usize| [c(i), c(i * 7 + 3), c(i * 13 + 5)];

    for (mode, name) in [(0.0f32, "3d"), (1.0, "2d")] {
        let items: Vec<[f32; 4]> = (0..N)
            .map(|i| {
                let p = at(i);
                [p[0], p[1], p[2], mode]
            })
            .collect();
        let out = probe.run(&v3, &v2, &items);
        let axes = if mode == 0.0 { 3 } else { 2 };

        for a in 0..axes {
            let mut levels: Vec<u32> = out.iter().map(|o| o[a].to_bits()).collect();
            levels.sort_unstable();
            levels.dedup();
            let (mut worst, mut sum) = (0.0f64, 0.0f64);

            for (i, o) in out.iter().enumerate() {
                let exact = (at(i)[a] as f64 - 0.25) * 2.0;
                let e = (o[a] as f64 - exact).abs();
                worst = worst.max(e);
                sum += e;
            }

            eprintln!(
                "diag: filter {name} axis {a}: {} distinct weights in {N} steps, worst error {:.5} (1/{:.0}), mean {:.6}",
                levels.len(),
                worst,
                1.0 / worst.max(1e-12),
                sum / N as f64
            );
        }
    }
}

#[test]
fn gradient_normals() {
    let Some(probe) = Probe::new() else {
        return eprintln!("diag: no device for the gradient probe");
    };
    // A radial blob of 8-bit voxels, 16^3 like the smallest volumes of the batch.
    const V: usize = 16;
    let mut texels = vec![];
    for z in 0..V {
        for y in 0..V {
            for x in 0..V {
                let d = |a: usize| (a as f64 + 0.5) / V as f64 - 0.5;
                let r = (d(x) * d(x) + d(y) * d(y) + d(z) * d(z)).sqrt();
                let v = (1.0 - 1.6 * r).clamp(0.0, 1.0);
                texels.extend([(v * 255.0).round() as u8, 0, 0, 255]);
            }
        }
    }
    let v3 = probe.texture(wgpu::TextureDimension::D3, [V as u32; 3], &texels);
    let v2 = probe.texture(wgpu::TextureDimension::D2, [2, 2, 1], &[0u8; 16]);
    let mut s = 12345u32;
    const N: usize = 4096;
    let ps: Vec<[f32; 3]> = (0..N)
        .map(|_| {
            [
                (0.2 + 0.6 * lcg(&mut s)) as f32,
                (0.2 + 0.6 * lcg(&mut s)) as f32,
                (0.2 + 0.6 * lcg(&mut s)) as f32,
            ]
        })
        .collect();
    let items: Vec<[f32; 4]> = ps.iter().map(|p| [p[0], p[1], p[2], 2.0]).collect();
    let out = probe.run(&v3, &v2, &items);
    let h = 1.0 / V as f64;

    let cpu = |p: &[f32; 3], w: Weight| {
        let q = [p[0] as f64, p[1] as f64, p[2] as f64];
        let s = |d: [f64; 3], sign: f64| {
            trilinear(
                &texels,
                V,
                [q[0] + sign * d[0], q[1] + sign * d[1], q[2] + sign * d[2]],
                w,
            )
        };
        [
            s([h, 0.0, 0.0], -1.0) - s([h, 0.0, 0.0], 1.0),
            s([0.0, h, 0.0], -1.0) - s([0.0, h, 0.0], 1.0),
            s([0.0, 0.0, h], -1.0) - s([0.0, 0.0, h], 1.0),
        ]
    };
    let angle = |a: [f64; 3], b: [f64; 3]| {
        let (la, lb) = (
            (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt(),
            (b[0] * b[0] + b[1] * b[1] + b[2] * b[2]).sqrt(),
        );
        let d = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]) / (la * lb);
        d.clamp(-1.0, 1.0).acos()
    };
    let exact: Vec<[f64; 3]> = ps.iter().map(|p| cpu(p, Weight::Exact)).collect();
    let device: Vec<[f64; 3]> = out
        .iter()
        .map(|o| [o[0] as f64, o[1] as f64, o[2] as f64])
        .collect();
    let len = |v: &[f64; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    // Positions where the gradient is not tiny: below that the direction is noise for every device.
    let kept: Vec<usize> = (0..N).filter(|i| len(&exact[*i]) > 0.02).collect();

    for (name, other) in [
        ("device", device.clone()),
        (
            "cpu-round8",
            ps.iter().map(|p| cpu(p, Weight::Round8)).collect(),
        ),
        (
            "cpu-floor8",
            ps.iter().map(|p| cpu(p, Weight::Floor8)).collect(),
        ),
    ] {
        let a: Vec<f64> = kept.iter().map(|i| angle(exact[*i], other[*i])).collect();
        let over = |t: f64| a.iter().filter(|x| **x > t).count();
        let worst = a.iter().cloned().fold(0.0, f64::max);
        let worst_len = kept
            .iter()
            .map(|i| (len(&exact[*i]) - len(&other[*i])).abs())
            .fold(0.0, f64::max);
        eprintln!(
            "diag: gradient {name} against exact weights: {} positions, angle over 0.01 rad {}, over 0.1 rad {}, worst {:.4} rad, worst length difference {:.5} ({:.2} of 1/255)",
            kept.len(),
            over(0.01),
            over(0.1),
            worst,
            worst_len,
            worst_len * 255.0
        );
    }
}

/// H3, the blend of many planes: a volume is drawn as hundreds of view-aligned planes that blend into an 8-bit target
/// (SRC_ALPHA, ONE_MINUS_SRC_ALPHA, as volume_gpu.rs), so the rounding of each blend step accumulates. Draw `planes` full
/// screen triangles of one color and alpha over a clear color and compare the pixel with the CPU evaluated in floats, and
/// with every step rounded to nearest or truncated to 8 bits.
#[test]
fn blend_accumulation() {
    let Some((device, queue)) = device() else {
        return eprintln!("diag: no device for the blend probe");
    };
    let clear = [0.12f64, 0.13, 0.28];
    let color = [1.0f64, 0.5, 0.25];

    for (alpha, planes) in [(0.03f32, 150u32), (0.011, 300), (0.2, 12)] {
        let source = format!(
            "@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {{
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}}
@fragment fn fs() -> @location(0) vec4<f32> {{ return vec4<f32>(1.0, 0.5, 0.25, {alpha:?}); }}"
        );
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 64,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256 * 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear[0],
                            g: clear[1],
                            b: clear[2],
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline);

            for _ in 0..planes {
                pass.draw(0..3, 0..1);
            }
        }
        enc.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &read,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(4),
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 4,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([enc.finish()]);
        let slice = read.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let data = slice.get_mapped_range().unwrap();
        let got = [data[0], data[1], data[2]];
        drop(data);
        let run = |step: fn(f64) -> f64, quantized_source: bool| {
            let q = |v: f64| {
                if quantized_source {
                    (v * 255.0).round() / 255.0
                } else {
                    v
                }
            };
            let a = q(alpha as f64);
            let mut d = clear.map(|c| step(c * 255.0) / 255.0);

            for _ in 0..planes {
                for c in 0..3 {
                    d[c] = step((q(color[c]) * a + d[c] * (1.0 - a)) * 255.0) / 255.0;
                }
            }

            d.map(|v| (v * 255.0).round() as i32)
        };
        eprintln!(
            "diag: blend alpha {alpha} x {planes} planes: device {got:?}, float {:?}; per step rounded {:?} truncated {:?}; source color and alpha as 8 bits, rounded {:?} truncated {:?}",
            run(|v| v, false),
            run(f64::round, false),
            run(f64::floor, false),
            run(f64::round, true),
            run(f64::floor, true)
        );
    }
}
