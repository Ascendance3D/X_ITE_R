//! Noise hash forms on the device (issue #469), information only: this test never fails on what it finds.
//!
//! X_ITE's `Perlin2.glsl.js` hashes with `fract (sin (dot (co, vec2 (12.9898, 78.233))) * 43758.5453)`. The value of `dot`
//! depends on how the shader compiler evaluates it: two products and an add, or a fused multiply-add in one order or the
//! other. This test evaluates the argument, `rand1` and the `rand3 (co, 1, 0)` chain of `x3d.wgsl` (`rand3` is
//! `rand1 (rand1 (rand1 (co), l), t)`, and `perlin3` calls it with dim 1 and time 0) on the device for four WGSL forms,
//! and prints which CPU binary32 form each device argument equals. The CPU forms are references for the comparison;
//! none of them is a required device result. The regression test of the production shader is
//! `render_rea_b09_inputs.rs` (`texgen-params` against pinned X_ITE).
//! The inputs are the lattice points of the TextureCoordinateGenerator NOISE modes and the inputs of the chain's later
//! stages. Lines start with `diag: noise-dot` so that a job can pick them out.

use x3d_render::gpu::headless_backends;

const A: f32 = 12.9898;
const B: f32 = 78.233;

fn device() -> Option<(wgpu::Device, wgpu::Queue, wgpu::AdapterInfo)> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = headless_backends(std::env::var("X3D_BACKEND").ok().as_deref()).ok()?;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .ok()?;
    let info = adapter.get_info();
    let (d, q) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
    Some((d, q, info))
}

/// The WGSL forms of the hash argument, in `co`.
const FORMS: &[(&str, &str)] = &[
    ("wgsl-fma-y", "fma(co.y, 78.233, co.x * 12.9898)"),
    ("wgsl-fma-x", "fma(co.x, 12.9898, co.y * 78.233)"),
    ("wgsl-dot", "dot(co, vec2<f32>(12.9898, 78.233))"),
    ("wgsl-mul-add", "co.x * 12.9898 + co.y * 78.233"),
];

/// The device bits of `arg(co)`, `rand1(co)` and `rand3(co, 1, 0)` with `form` for each item.
struct Run {
    arg: Vec<u32>,
    rand1: Vec<u32>,
    chain: Vec<u32>,
}

/// Evaluates `arg(co)`, `rand1(co)` and the chain `rand1(rand1(rand1(co), 1), 0)` with `form` for each item on the device.
fn run(device: &wgpu::Device, queue: &wgpu::Queue, form: &str, items: &[[u32; 2]]) -> Run {
    let source = format!(
        "
@group(0) @binding(0) var<storage, read> inp: array<vec2<u32>>;
@group(0) @binding(1) var<storage, read_write> out: array<vec4<u32>>;
fn arg(co: vec2<f32>) -> f32 {{ return {form}; }}
fn rand1(co: vec2<f32>) -> f32 {{ return fract(sin(arg(co)) * 43758.5453); }}
@compute @workgroup_size(64) fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let i = id.x;
    if (i >= arrayLength(&inp)) {{ return; }}
    let co = vec2<f32>(bitcast<f32>(inp[i].x), bitcast<f32>(inp[i].y));
    out[i] = vec4<u32>(bitcast<u32>(arg(co)), bitcast<u32>(rand1(co)), bitcast<u32>(rand1(vec2<f32>(rand1(vec2<f32>(rand1(co), 1.0)), 0.0))), 0u);
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
    let bytes = (items.len() * 16) as u64;
    let make = |usage| {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: bytes.max(16),
            usage,
            mapped_at_creation: false,
        })
    };
    let input = make(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
    let output = make(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC);
    let read = make(wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST);
    let input_bytes: &[u8] = bytemuck::cast_slice(items);
    queue.write_buffer(&input, 0, input_bytes);
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
    let v = &bytemuck::cast_slice::<u8, u32>(&data)[..items.len() * 4];
    let lane = |k: usize| v.iter().skip(k).step_by(4).copied().collect();
    Run {
        arg: lane(0),
        rand1: lane(1),
        chain: lane(2),
    }
}

/// The CPU forms of the argument.
fn cpu_forms() -> [(&'static str, fn(f32, f32) -> f32); 4] {
    [
        ("cpu-unfused", |x, y| x * A + y * B),
        ("cpu-fma-y", |x, y| y.mul_add(B, x * A)),
        ("cpu-fma-x", |x, y| x.mul_add(A, y * B)),
        ("cpu-exact", |x, y| {
            (x as f64 * A as f64 + y as f64 * B as f64) as f32
        }),
    ]
}

#[test]
fn noise_dot_forms() {
    let Some((d, q, info)) = device() else {
        println!("diag: noise-dot no adapter");
        return;
    };
    println!(
        "diag: noise-dot adapter {:?} {:?} vendor {:#x} device {:#x} driver {:?} {:?}",
        info.name, info.backend, info.vendor, info.device, info.driver, info.driver_info
    );

    // Stage 1: the lattice points floor (p * dim) of the NOISE modes. Stages 2 and 3: (rand1 (..), 1) and (rand1 (..), 0),
    // fed with the device's own previous stage of the same form, so that the inputs are the ones the form gives.
    let mut lattice = vec![];
    for px in -6..=6 {
        for py in -6..=6 {
            lattice.push([(px as f32).to_bits(), (py as f32).to_bits()]);
        }
    }

    // Non-integer pairs in [-8, 8] (xorshift), where the forms round differently more often.
    let mut seed = 0x2545_f491u32;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        -8.0 + 16.0 * (seed as f32 / u32::MAX as f32)
    };
    let free: Vec<[u32; 2]> = (0..4096)
        .map(|_| [next().to_bits(), next().to_bits()])
        .collect();

    for &(name, form) in FORMS {
        let args = run(&d, &q, form, &free).arg;
        let cells: Vec<String> = cpu_forms()
            .iter()
            .map(|(n, f)| {
                let c = free
                    .iter()
                    .zip(&args)
                    .filter(|(it, g)| {
                        f(f32::from_bits(it[0]), f32::from_bits(it[1])).to_bits() == **g
                    })
                    .count();
                format!("{n}={c}/{}", free.len())
            })
            .collect();
        println!("diag: noise-dot {name} free {}", cells.join(" "));
    }

    for &(name, form) in FORMS {
        let mut stage = lattice.clone();
        let mut equal = [[0usize; 4]; 3];
        let mut total = 0;

        for (s, next_y) in [(0usize, 1.0f32), (1, 0.0), (2, 0.0)] {
            let r = run(&d, &q, form, &stage);
            for (it, &g) in stage.iter().zip(&r.arg) {
                let (x, y) = (f32::from_bits(it[0]), f32::from_bits(it[1]));
                for (k, (_, f)) in cpu_forms().iter().enumerate() {
                    if f(x, y).to_bits() == g {
                        equal[s][k] += 1;
                    }
                }
            }
            total = stage.len();
            // The next stage's input: (rand1 (co), next_y) with the device's rand1 of this form.
            stage = r.rand1.iter().map(|&v| [v, next_y.to_bits()]).collect();
        }

        for (s, row) in equal.iter().enumerate() {
            let cells: Vec<String> = cpu_forms()
                .iter()
                .zip(row)
                .map(|((n, _), c)| format!("{n}={c}/{total}"))
                .collect();
            println!("diag: noise-dot {name} stage{} {}", s + 1, cells.join(" "));
        }

        let chain = run(&d, &q, form, &lattice).chain;
        let hex: Vec<String> = chain.iter().map(|g| format!("{g:08x}")).collect();
        println!("diag: noise-dot {name} chain {}", hex.join(" "));
    }
}
