//! The GPU side of the VolumeRendering component (see [`crate::volume`]): one pipeline per
//! generated program, a dynamic-offset uniform buffer, a bind group per program and texture set.
//! The planes are drawn without a vertex buffer. Everything is created when a frame first needs
//! it and dropped when no frame has used it for [`KEEP_FRAMES`] frames.

use crate::texture::TextureImage;
use crate::volume::{Program, VOLUME_WGSL, VolumeDraw};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Frames a program, pipeline or bind group stays after its last use.
pub const KEEP_FRAMES: u64 = 120;

/// The fixed part of `VolumeUniforms` (volume.wgsl) in bytes; the value pool follows.
const FIXED_BYTES: usize = 416;

/// A volume of the frame being drawn.
pub struct VolumeInput<'a> {
    pub draw: &'a VolumeDraw,
    /// Indices into `Frame::lights` (the draw item's lights).
    pub lights: Vec<u32>,
    pub clip_planes: Vec<[f64; 4]>,
    /// The fog of the draw: the innermost LocalFog, else the layer's Fog (renderContext .fogNode).
    pub fog: Option<&'a crate::fog::FogParams>,
}

struct Module {
    module: wgpu::ShaderModule,
    layout: wgpu::BindGroupLayout,
    struct_bytes: u64,
    used: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PipelineKey {
    module: u64,
    format: wgpu::TextureFormat,
    samples: u32,
    flip: bool,
    ccw: bool,
    reversed: bool,
}

struct Prepared {
    pipeline: PipelineKey,
    group: (u64, Vec<usize>),
    offset: u32,
    vertices: u32,
}

#[derive(Default)]
pub struct VolumeGpu {
    modules: HashMap<u64, Module>,
    pipelines: HashMap<PipelineKey, (wgpu::RenderPipeline, u64)>,
    groups: HashMap<(u64, Vec<usize>), (wgpu::BindGroup, u64)>,
    buffer: Option<(wgpu::Buffer, u64)>,
    prepared: Vec<Prepared>,
    /// Pipelines created so far (for tests).
    pub pipelines_created: usize,
}

/// The slice of `source` from `from` up to (not including) `to`.
fn between<'a>(source: &'a str, from: &str, to: &str) -> &'a str {
    let a = source.find(from).unwrap_or_else(|| {
        panic!("x3d.wgsl: marker {from:?} not found (volume.wgsl copies from it)")
    });
    let b = a + source[a..].find(to).unwrap_or_else(|| {
        panic!("x3d.wgsl: marker {to:?} not found (volume.wgsl copies from it)")
    });
    &source[a..b]
}

fn program_key(p: &Program) -> u64 {
    let mut h = DefaultHasher::new();
    p.hash(&mut h);
    h.finish()
}

/// volume.wgsl with the frame structs, the tone map and fog functions of x3d.wgsl, the texture
/// slots and the program filled in.
pub fn assemble(p: &Program) -> String {
    let x3d = include_str!("x3d.wgsl");
    let structs = between(x3d, "struct Light {", "struct DrawUniforms {");
    let tone = between(x3d, "// ToneMapping2.glsl linearToSRGB", "fn f_schlick(");
    let fog = between(
        x3d,
        "// Fog2.glsl getFogInterpolant",
        "@fragment\nfn fs_main(",
    );
    let mut slots = String::new();

    for (i, three_d) in p.dims.iter().enumerate() {
        let ty = if *three_d {
            "texture_3d<f32>"
        } else {
            "texture_2d<f32>"
        };
        slots.push_str(&format!(
            "@group(1) @binding({}) var t{i}: {ty};\n@group(1) @binding({}) var s{i}: sampler;\n",
            1 + 2 * i,
            2 + 2 * i
        ));
    }

    VOLUME_WGSL
        .replace("//@FRAME_STRUCTS@", structs)
        .replace("//@VEC4S@", &p.vec4s.to_string())
        .replace("//@SLOTS@", &slots)
        .replace("//@TONE_AND_FOG@", &format!("{tone}\n{fog}"))
        .replace("//@PROGRAM@", &p.source)
}

fn determinant3(m: &[f32; 16]) -> f32 {
    m[0] * (m[5] * m[10] - m[6] * m[9]) - m[1] * (m[4] * m[10] - m[6] * m[8])
        + m[2] * (m[4] * m[9] - m[5] * m[8])
}

impl VolumeGpu {
    /// GPU objects held (modules, pipelines, bind groups, uniform buffers).
    pub fn resources(&self) -> [usize; 4] {
        [
            self.modules.len(),
            self.pipelines.len(),
            self.groups.len(),
            self.buffer.is_some() as usize,
        ]
    }

    fn module(&mut self, device: &wgpu::Device, key: u64, p: &Program, frame_no: u64) {
        if let Some(m) = self.modules.get_mut(&key) {
            m.used = frame_no;
            return;
        }

        let source = assemble(p);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("volume"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let struct_bytes = (FIXED_BYTES + p.vec4s * 16) as u64;
        let mut entries = vec![wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(struct_bytes),
            },
            count: None,
        }];

        for (i, three_d) in p.dims.iter().enumerate() {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: 1 + 2 * i as u32,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: if *three_d {
                        wgpu::TextureViewDimension::D3
                    } else {
                        wgpu::TextureViewDimension::D2
                    },
                    multisampled: false,
                },
                count: None,
            });
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: 2 + 2 * i as u32,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            });
        }

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("volume"),
            entries: &entries,
        });
        self.modules.insert(
            key,
            Module {
                module,
                layout,
                struct_bytes,
                used: frame_no,
            },
        );
    }

    /// Record the volumes of a frame: programs, pipelines, uniforms and bind groups. Returns
    /// nothing; [`VolumeGpu::draw`] replays call `i`.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare<'a>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        samples: u32,
        flip: bool,
        depth_format: wgpu::TextureFormat,
        reversed: bool,
        linear_output: bool,
        frame_no: u64,
        inputs: &[VolumeInput],
        texture: &'a dyn Fn(
            *const TextureImage,
            bool,
        ) -> (&'a wgpu::TextureView, &'a wgpu::Sampler),
    ) {
        self.prepared.clear();

        if inputs.is_empty() {
            return;
        }

        let mut blobs: Vec<(usize, Vec<u8>)> = vec![];
        let mut total = 0u64;
        let mut offsets = vec![];

        for input in inputs {
            let d = input.draw;
            let key = program_key(&d.program);
            self.module(device, key, &d.program, frame_no);
            let m = &self.modules[&key];
            let stride = m.struct_bytes.div_ceil(256) * 256;
            offsets.push(total);
            total += stride;

            let mut bytes: Vec<u8> = Vec::with_capacity(stride as usize);
            let f =
                |v: &[f32], bytes: &mut Vec<u8>| bytes.extend_from_slice(bytemuck::cast_slice(v));
            f(&d.model_view, &mut bytes);
            f(&d.tex_matrix, &mut bytes);
            let n = &d.normal_matrix;

            // GLSL uniformMatrix3fv (transpose) of the row-major array: column c is (n[c], n[3 + c], n[6 + c]).
            for c in 0..3 {
                f(&[n[c], n[3 + c], n[6 + c], 0.0], &mut bytes);
            }

            f(&[d.planes as f32, d.size, 0.0, 0.0], &mut bytes);
            let clips = input.clip_planes.len().min(6);
            let lights: Vec<u32> = input.lights.iter().copied().take(8).collect();
            bytes.extend_from_slice(bytemuck::cast_slice(&[
                clips as u32,
                lights.len() as u32,
                linear_output as u32,
                0u32,
            ]));
            let mut index = [0u32; 8];
            index[..lights.len()].copy_from_slice(&lights);
            bytes.extend_from_slice(bytemuck::cast_slice(&index));
            let mut planes = [[0.0f32; 4]; 6];

            for (i, p) in input.clip_planes.iter().take(6).enumerate() {
                planes[i] = p.map(|v| v as f32);
            }

            bytes.extend_from_slice(bytemuck::cast_slice(&planes));
            match input.fog {
                Some(fog) => {
                    f(
                        &[
                            fog.color[0],
                            fog.color[1],
                            fog.color[2],
                            fog.fog_type as f32,
                        ],
                        &mut bytes,
                    );
                    f(
                        &[fog.visibility_start, fog.visibility_range, 0.0, 0.0],
                        &mut bytes,
                    );
                    f(
                        &[fog.matrix[0], fog.matrix[1], fog.matrix[2], 0.0],
                        &mut bytes,
                    );
                    f(
                        &[fog.matrix[3], fog.matrix[4], fog.matrix[5], 0.0],
                        &mut bytes,
                    );
                    f(
                        &[fog.matrix[6], fog.matrix[7], fog.matrix[8], 0.0],
                        &mut bytes,
                    );
                }
                None => bytes.extend_from_slice(&[0u8; 80]),
            }

            debug_assert_eq!(bytes.len(), FIXED_BYTES);
            f(&d.values, &mut bytes);
            bytes.resize(stride as usize, 0);
            blobs.push((offsets.len() - 1, bytes));
        }

        // The uniform buffer, grown when needed (the bind groups refer to it).
        if self.buffer.as_ref().is_none_or(|b| b.1 < total) {
            let capacity = total.next_power_of_two().max(4096);
            self.buffer = Some((
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("volume uniforms"),
                    size: capacity,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                capacity,
            ));
            self.groups.clear();
        }

        let mut all = vec![0u8; total as usize];

        for (i, bytes) in &blobs {
            all[offsets[*i] as usize..offsets[*i] as usize + bytes.len()].copy_from_slice(bytes);
        }

        queue.write_buffer(&self.buffer.as_ref().unwrap().0, 0, &all);
        crate::transfer::upload(crate::transfer::Kind::Other, all.len());

        for (i, input) in inputs.iter().enumerate() {
            let d = input.draw;
            let key = program_key(&d.program);
            let ccw = determinant3(&d.model_view) >= 0.0;
            let pipeline = PipelineKey {
                module: key,
                format,
                samples,
                flip,
                ccw,
                reversed,
            };

            if !self.pipelines.contains_key(&pipeline) {
                let m = &self.modules[&key];
                let blend = wgpu::BlendState {
                    // gl.blendFuncSeparate (SRC_ALPHA, ONE_MINUS_SRC_ALPHA, ONE, ONE_MINUS_SRC_ALPHA)
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
                };
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("volume"),
                    bind_group_layouts: &[Some(frame_layout), Some(&m.layout)],
                    immediate_size: 0,
                });
                let p = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("volume"),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &m.module,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        front_face: if ccw != flip {
                            wgpu::FrontFace::Ccw
                        } else {
                            wgpu::FrontFace::Cw
                        },
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: depth_format,
                        depth_write_enabled: Some(false),
                        depth_compare: Some(if reversed {
                            wgpu::CompareFunction::GreaterEqual
                        } else {
                            wgpu::CompareFunction::LessEqual
                        }),
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: wgpu::MultisampleState {
                        count: samples,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &m.module,
                        entry_point: Some("fs_main"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: Some(blend),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview_mask: None,
                    cache: None,
                });
                self.pipelines.insert(pipeline, (p, frame_no));
                self.pipelines_created += 1;
            }

            self.pipelines.get_mut(&pipeline).unwrap().1 = frame_no;

            let ptrs: Vec<usize> = d
                .textures
                .iter()
                .map(|t| std::rc::Rc::as_ptr(t) as usize)
                .collect();
            let group_key = (key, ptrs);

            match self.groups.get_mut(&group_key) {
                Some(g) => g.1 = frame_no,
                None => {
                    let m = &self.modules[&key];
                    let buffer = &self.buffer.as_ref().unwrap().0;
                    let resources: Vec<(&wgpu::TextureView, &wgpu::Sampler)> = d
                        .textures
                        .iter()
                        .zip(&d.program.dims)
                        .map(|(t, three_d)| texture(std::rc::Rc::as_ptr(t), *three_d))
                        .collect();
                    let mut entries = vec![wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer,
                            offset: 0,
                            size: wgpu::BufferSize::new(m.struct_bytes),
                        }),
                    }];

                    for (s, (view, sampler)) in resources.iter().enumerate() {
                        entries.push(wgpu::BindGroupEntry {
                            binding: 1 + 2 * s as u32,
                            resource: wgpu::BindingResource::TextureView(view),
                        });
                        entries.push(wgpu::BindGroupEntry {
                            binding: 2 + 2 * s as u32,
                            resource: wgpu::BindingResource::Sampler(sampler),
                        });
                    }

                    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("volume"),
                        layout: &m.layout,
                        entries: &entries,
                    });
                    self.groups.insert(group_key.clone(), (group, frame_no));
                }
            }

            self.prepared.push(Prepared {
                pipeline,
                group: group_key,
                offset: offsets[i] as u32,
                vertices: d.planes * 6,
            });
        }
    }

    /// Draw prepared volume `i`.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, i: usize) {
        let p = &self.prepared[i];
        pass.set_pipeline(&self.pipelines[&p.pipeline].0);
        pass.set_bind_group(1, &self.groups[&p.group].0, &[p.offset]);
        pass.draw(0..p.vertices, 0..1);
    }

    /// Drop what no frame has used for [`KEEP_FRAMES`] frames.
    pub fn prune(&mut self, frame_no: u64) {
        self.pipelines.retain(|_, e| e.1 + KEEP_FRAMES >= frame_no);
        self.groups.retain(|_, e| e.1 + KEEP_FRAMES >= frame_no);
        self.modules.retain(|_, m| m.used + KEEP_FRAMES >= frame_no);

        if self.modules.is_empty() && self.pipelines.is_empty() {
            self.buffer = None;
            self.groups.clear();
        }
    }
}
