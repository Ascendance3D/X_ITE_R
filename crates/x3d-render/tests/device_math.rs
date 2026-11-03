//! Device arithmetic diagnostics (issue #89), information only: these tests never fail on what they find. They run
//! small compute shaders on the graphics device and compare every result with the same expression evaluated on the CPU
//! (IEEE binary32): the transcendental builtins, the hash chain that the texture-coordinate NOISE modes use (Perlin2.glsl
//! rand1/rand3), and the infinity and NaN behavior that ray-box intersections and covariance code rely on. A second group
//! draws full-screen triangles at and just beyond the far clip plane. The hosted-macOS job runs them with `--nocapture`.
//! A result from a virtual machine is never a physical-device result.
//!
//! Lines start with `diag:` so that a job can pick them out.

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

/// Evaluate the WGSL f32 expression `body` (in `x` and `y`, a pair per item) for every item on the device; the result bits.
fn run(device: &wgpu::Device, queue: &wgpu::Queue, body: &str, items: &[[u32; 2]]) -> Vec<u32> {
    let source = format!(
        "
@group(0) @binding(0) var<storage, read> inp: array<vec2<u32>>;
@group(0) @binding(1) var<storage, read_write> out: array<u32>;
fn rand1(co: vec2<f32>) -> f32 {{
    return fract(sin(fma(co.y, 78.233, co.x * 12.9898)) * 43758.5453);
}}
@compute @workgroup_size(64) fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let i = id.x;
    if (i >= arrayLength(&inp)) {{ return; }}
    let x = bitcast<f32>(inp[i].x);
    let y = bitcast<f32>(inp[i].y);
    out[i] = bitcast<u32>({body});
}}
"
    );
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bytes = (items.len() * 4) as u64;
    let make = |usage| {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: bytes.max(8) * 2,
            usage,
            mapped_at_creation: false,
        })
    };
    let input = make(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
    let output = make(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC);
    let read = make(wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST);
    queue.write_buffer(&input, 0, bytemuck::cast_slice(items));
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output.as_entire_binding(),
            },
        ],
    });
    let mut enc = device.create_command_encoder(&Default::default());
    {
        let mut pass = enc.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups((items.len() as u32).div_ceil(64), 1, 1);
    }
    enc.copy_buffer_to_buffer(&output, 0, &read, 0, bytes);
    queue.submit([enc.finish()]);
    let slice = read.slice(..bytes);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let data = slice.get_mapped_range().unwrap();
    bytemuck::cast_slice::<u8, u32>(&data)[..items.len()].to_vec()
}

fn bits(items: &[(f32, f32)]) -> Vec<[u32; 2]> {
    items
        .iter()
        .map(|(x, y)| [x.to_bits(), y.to_bits()])
        .collect()
}

/// Distance in units of the last place between two finite floats; `u32::MAX` when one is not finite or they differ in sign class.
fn ulps(a: f32, b: f32) -> u32 {
    if !a.is_finite() || !b.is_finite() {
        return if a.to_bits() == b.to_bits() {
            0
        } else {
            u32::MAX
        };
    }
    let key = |f: f32| {
        let b = f.to_bits() as i32;
        if b < 0 { i32::MIN - b } else { b }
    };
    (key(a) as i64 - key(b) as i64)
        .unsigned_abs()
        .min(u32::MAX as u64 - 1) as u32
}

/// Compare the device with the CPU for `body` over `items`; print the mismatch count and the worst distance.
fn compare(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    name: &str,
    body: &str,
    items: &[(f32, f32)],
    cpu: impl Fn(f32, f32) -> f32,
) {
    let got = run(device, queue, body, &bits(items));
    let (mut differ, mut worst, mut at) = (0usize, 0u32, 0usize);

    for (i, (&(x, y), &g)) in items.iter().zip(&got).enumerate() {
        let want = cpu(x, y);
        let d = ulps(f32::from_bits(g), want);

        if d > 0 {
            differ += 1;
        }
        if d > worst {
            worst = d;
            at = i;
        }
    }

    let (x, y) = items[at];
    println!(
        "diag: math {name}: {differ} of {} differ, worst {} ulps at x={x:e} y={y:e} (device {:e}, cpu {:e})",
        items.len(),
        if worst == u32::MAX - 1 || worst == u32::MAX {
            "n/a (not finite)".to_string()
        } else {
            worst.to_string()
        },
        f32::from_bits(got[at]),
        cpu(x, y)
    );
}

fn rand1(x: f32, y: f32) -> f32 {
    let s = y.mul_add(78.233, x * 12.9898).sin();
    let v = s * 43758.5453;
    v - v.floor()
}

/// A deterministic pseudo-random sequence (xorshift), no `rand` dependency.
fn sequence(n: usize, lo: f32, hi: f32, mut s: u32) -> Vec<f32> {
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            lo + (hi - lo) * (s as f32 / u32::MAX as f32)
        })
        .collect()
}

fn pairs(a: &[f32], b: &[f32]) -> Vec<(f32, f32)> {
    a.iter().copied().zip(b.iter().copied()).collect()
}

#[test]
fn transcendental_builtins() {
    let Some((d, q)) = device() else {
        println!("diag: no adapter");
        return;
    };
    let n = 4096;

    for (lo, hi, tag) in [(-4.0, 4.0, "small"), (-100.0, 100.0, "large")] {
        let xs = sequence(n, lo, hi, 12345);
        let it = pairs(&xs, &xs);
        compare(&d, &q, &format!("sin {tag}"), "sin(x)", &it, |x, _| x.sin());
        compare(&d, &q, &format!("cos {tag}"), "cos(x)", &it, |x, _| x.cos());
    }

    let xs = sequence(n, 0.001, 20.0, 777);
    let ys = sequence(n, -3.0, 5.0, 4242);
    let it = pairs(&xs, &ys);
    compare(&d, &q, "exp(-x)", "exp(-x)", &it, |x, _| (-x).exp());
    compare(&d, &q, "log(x)", "log(x)", &it, |x, _| x.ln());
    compare(&d, &q, "sqrt(x)", "sqrt(x)", &it, |x, _| x.sqrt());
    compare(&d, &q, "pow(x, y)", "pow(x, y)", &it, |x, y| x.powf(y));
    compare(&d, &q, "atan2(y, x)", "atan2(y, x)", &it, |x, y| y.atan2(x));
    compare(&d, &q, "x / y", "x / y", &it, |x, y| x / y);
    compare(
        &d,
        &q,
        "fract(x * 43758.5453)",
        "fract(x * 43758.5453)",
        &it,
        |x, _| {
            let v = x * 43758.5453;
            v - v.floor()
        },
    );
}

#[test]
fn noise_hash_chain_by_stage() {
    let Some((d, q)) = device() else {
        println!("diag: no adapter");
        return;
    };

    // The inputs of TextureCoordinateGenerator NOISE: pos = floor (p * dim) in a small integer range, dim 1, time 0.
    let mut stage1 = vec![];
    for px in -4..=4 {
        for py in -4..=4 {
            stage1.push((px as f32, py as f32));
        }
    }
    compare(
        &d,
        &q,
        "rand1(co) on the lattice",
        "rand1(vec2<f32>(x, y))",
        &stage1,
        rand1,
    );

    // Stage 2 and 3 fed with the CPU's previous stage, so that a mismatch is the stage's own.
    let s2: Vec<(f32, f32)> = stage1.iter().map(|&(x, y)| (rand1(x, y), 1.0)).collect();
    compare(
        &d,
        &q,
        "rand1(rand1(co), 1) given CPU stage 1",
        "rand1(vec2<f32>(x, y))",
        &s2,
        rand1,
    );
    let s3: Vec<(f32, f32)> = s2.iter().map(|&(x, y)| (rand1(x, y), 0.0)).collect();
    compare(
        &d,
        &q,
        "rand1(rand1(..), 0) given CPU stage 2",
        "rand1(vec2<f32>(x, y))",
        &s3,
        rand1,
    );

    // The whole chain, device against CPU.
    compare(
        &d,
        &q,
        "rand3 chain, whole",
        "rand1(vec2<f32>(rand1(vec2<f32>(rand1(vec2<f32>(x, y)), 1.0)), 0.0))",
        &stage1,
        |x, y| rand1(rand1(rand1(x, y), 1.0), 0.0),
    );

    // The device's own chain output on the lattice, in hex, so that two devices can be compared with each other (the
    // pinned images come from a GPU whose sine is not the CPU's either).
    for (tag, body) in [
        (
            "chain",
            "rand1(vec2<f32>(rand1(vec2<f32>(rand1(vec2<f32>(x, y)), 1.0)), 0.0))",
        ),
        ("stage1", "rand1(vec2<f32>(x, y))"),
        ("sin", "sin(fma(y, 78.233, x * 12.9898))"),
    ] {
        let got = run(&d, &q, body, &bits(&stage1));
        let hex: Vec<String> = got.iter().map(|g| format!("{g:08x}")).collect();
        println!("diag: lattice-{tag} {}", hex.join(" "));
    }

    // The sine alone at the arguments of stage 1, and the product that follows it.
    compare(
        &d,
        &q,
        "sin(fma(y, 78.233, x * 12.9898)) on the lattice",
        "sin(fma(y, 78.233, x * 12.9898))",
        &stage1,
        |x, y| y.mul_add(78.233, x * 12.9898).sin(),
    );
    let arg: Vec<(f32, f32)> = s2
        .iter()
        .map(|&(x, y)| (y.mul_add(78.233, x * 12.9898), 0.0))
        .collect();
    compare(
        &d,
        &q,
        "sin of a stage 2 argument (0..80)",
        "sin(x)",
        &arg,
        |x, _| x.sin(),
    );
}

#[test]
fn infinity_and_nan_behavior() {
    let Some((d, q)) = device() else {
        println!("diag: no adapter");
        return;
    };
    let inf = f32::INFINITY;
    let cases = [
        (1.0f32, 0.0f32),
        (-1.0, 0.0),
        (0.0, 0.0),
        (inf, 1.0),
        (-inf, 1.0),
        (inf, 0.0),
        (2.0, inf),
    ];
    let expr = [
        ("x / y", "x / y"),
        ("min(x / y, 5.0)", "min(x / y, 5.0)"),
        ("max(x / y, -5.0)", "max(x / y, -5.0)"),
        ("clamp(x / y, 0.0, 1.0)", "clamp(x / y, 0.0, 1.0)"),
        (
            "select(1.0, 2.0, x / y > 100.0)",
            "select(1.0, 2.0, x / y > 100.0)",
        ),
        (
            "select(1.0, 2.0, (x / y) != (x / y))",
            "select(1.0, 2.0, (x / y) != (x / y))",
        ),
        ("x * y", "x * y"),
        ("min(x, y)", "min(x, y)"),
        ("max(x, y)", "max(x, y)"),
        ("abs(x)", "abs(x)"),
        ("step(1.0, x)", "step(1.0, x)"),
        ("sqrt(x)", "sqrt(x)"),
    ];
    let cpu_one = |name: &str, x: f32, y: f32| -> f32 {
        match name {
            "x / y" => x / y,
            "min(x / y, 5.0)" => (x / y).min(5.0),
            "max(x / y, -5.0)" => (x / y).max(-5.0),
            "clamp(x / y, 0.0, 1.0)" => (x / y).clamp(0.0, 1.0),
            "select(1.0, 2.0, x / y > 100.0)" => {
                if x / y > 100.0 {
                    2.0
                } else {
                    1.0
                }
            }
            "select(1.0, 2.0, (x / y) != (x / y))" => {
                if (x / y) != (x / y) {
                    2.0
                } else {
                    1.0
                }
            }
            "x * y" => x * y,
            "min(x, y)" => x.min(y),
            "max(x, y)" => x.max(y),
            "abs(x)" => x.abs(),
            "step(1.0, x)" => {
                if x >= 1.0 {
                    1.0
                } else {
                    0.0
                }
            }
            "sqrt(x)" => x.sqrt(),
            _ => f32::NAN,
        }
    };

    for (name, body) in expr {
        let got = run(&d, &q, body, &bits(&cases));
        let line: Vec<String> = cases
            .iter()
            .zip(&got)
            .map(|(&(x, y), &g)| {
                let want = cpu_one(name, x, y);
                let ok = g == want.to_bits() || (f32::from_bits(g).is_nan() && want.is_nan());
                format!(
                    "({x},{y}):{}{}",
                    f32::from_bits(g),
                    if ok {
                        ""
                    } else {
                        format!("(cpu {want})").leak()
                    }
                )
            })
            .collect();
        println!("diag: special {name}: {}", line.join(" "));
    }
}

/// Draw a full-screen triangle at clip z = k * w and report whether any pixel was written (the triangle is not clipped).
fn covered(device: &wgpu::Device, queue: &wgpu::Queue, z: &str, w: &str, s: &str) -> usize {
    let source = format!(
        "
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {{
    let p = vec2<f32>(f32((v << 1u) & 2u), f32(v & 2u)) * 2.0 - 1.0;
    let w = {w};
    return vec4<f32>(p * {s} * w, {z}, w);
}}
@fragment fn fs() -> @location(0) vec4<f32> {{ return vec4<f32>(1.0); }}
"
    );
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
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
        size: 64 * 64 * 4,
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
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.draw(0..3, 0..1);
    }
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &read,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(64 * 4),
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
    let slice = read.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let data = slice.get_mapped_range().unwrap();
    data.chunks_exact(4).filter(|p| p[0] > 0).count()
}

#[test]
fn far_plane_clipping() {
    let Some((d, q)) = device() else {
        println!("diag: no adapter");
        return;
    };

    for (z, w) in [
        ("1.0", "1.0"),
        ("1.0000001", "1.0"),
        ("1.0000002", "1.0"),
        ("1.000001", "1.0"),
        ("0.9999999", "1.0"),
        ("0.0", "1.0"),
        ("-0.0000001", "1.0"),
        ("-0.000001", "1.0"),
        ("1000.0", "1000.0"),
        ("1000.0001", "1000.0"),
        ("1000.0001", "1000.0"),
        ("1000.001", "1000.0"),
        ("min(1000.001, 1000.0)", "1000.0"),
    ] {
        println!(
            "diag: clip z={z} w={w}: {}",
            format!("{} of 4096 pixels", covered(&d, &q, z, w, "1.0"))
        );
    }
    for s in [
        "10.0",
        "100.0",
        "1000.0",
        "10000.0",
        "100000.0",
        "1000000.0",
        "10000000.0",
    ] {
        println!(
            "diag: clip huge triangle scale {s} (z=0.5, w=1): {}",
            format!("{} of 4096 pixels", covered(&d, &q, "0.5", "1.0", s))
        );
    }
}
