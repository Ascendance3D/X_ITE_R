//! The pointing buffer of X_ITE (Rendering/PointingBuffer.js, Pointing2.vs / Pointing2.fs, X3DRenderObject.pointing): the
//! pixel under the pointer is drawn into a 1x1 RGBA32F buffer with a DEPTH_COMPONENT24 depth buffer (depth test LEQUAL,
//! X3DRenderingContext), the viewport shifted so that it covers that pixel; every pointing shape is drawn in traversal order
//! with its own x3d_Id, so the nearest fragment wins and a depth tie goes to the primitive drawn last. The hit point, normal and texCoord are the
//! interpolated varyings: the GPU's own float32 results, which the CPU ray cast of the viewer cannot reproduce. The viewer
//! sends only the primitives that can cover the pixel (the ray cast's candidate triangles, the points and line segments
//! near it), in the shapes' drawing order. A ParticleSystem is one instanced draw instead
//! ([`PointingTriangle::instances`], ParticleSystem.displaySimple): the pointer does not test its particles one by one,
//! and particle k of a draw with x3d_Id `id` carries `id + k`.

use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use std::rc::Rc;
use x3d_fields::math::Matrix4;

/// The pointing WGSL.
pub const POINTING_WGSL: &str = include_str!("pointing.wgsl");

/// ClipPlanes2.glsl: at most this many x3d_ClipPlane entries (X3DRenderingContext maxClipPlanes).
pub const MAX_POINTING_CLIP_PLANES: usize = 6;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    projection: [f32; 16],
    model_view: [f32; 16],
    instance: [f32; 16],
    instance_normal: [[f32; 4]; 3],
    id: [f32; 4],
    viewport: [f32; 4],
    point: [f32; 4],
    attenuation: [f32; 4],
    clip_planes: [[f32; 4]; MAX_POINTING_CLIP_PLANES],
    flags: [f32; 4],
    /// x: 1 when the instance matrices come from `instances`, y: the first, z: vertices per instance (0: one instance
    /// per instance_index), w: the draw's first vertex, which `vertex_index` counts from.
    instancing: [u32; 4],
}

/// One draw's uniforms sit at a multiple of this (minUniformBufferOffsetAlignment).
const UNIFORM_STRIDE: u64 = 512;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct VertexData {
    vertex: [f32; 4],
    normal: [f32; 3],
    tex_coord: [f32; 4],
    offset: [f32; 2],
}

/// How a draw's vertices are rasterized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointingPrimitive {
    /// gl.TRIANGLES, three vertices per triangle.
    Triangles,
    /// Drawn as a WebGPU line list, two vertices per segment (not used for X3D lines: see `WideLines`).
    Lines,
    /// gl.POINTS: six vertices per point (two triangles), `offsets` the corner (±1, ±1) of each.
    Points,
    /// gl.LINES with an aliased width w (1 included): six vertices per segment (two triangles), `offsets` the pixel offset
    /// (±w / 2) of each corner along the minor axis.
    WideLines,
}

/// X3D_STYLE_PROPERTIES of a point draw (PointProperties.setShaderUniforms) and the texture defines that change the shader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointingPointStyle {
    pub scale_factor: f32,
    pub min_value: f32,
    pub max_value: f32,
    pub attenuation: [f32; 3],
    /// X3D_TEXTURE (PointSize2.glsl tests X3D_MATERIAL_TEXTURE, a define that never exists, so only this counts there).
    pub texture: bool,
    /// X3D_TEXTURE || X3D_MATERIAL_TEXTURES (Point2.glsl: no round point mask).
    pub any_texture: bool,
}

/// One shape of the pointing pass: the primitives that can cover the pixel and the matrices it is drawn with.
pub struct PointingTriangle {
    pub primitive: PointingPrimitive,
    /// x3d_Vertex of the vertices (object space, w included).
    pub vertices: Vec<[f32; 4]>,
    /// x3d_Normal; `(0, 0, 1)` without normals (the shader's constant).
    pub normals: Vec<[f32; 3]>,
    /// x3d_TexCoord0; `(0, 0, 0, 1)` for points and lines.
    pub tex_coords: Vec<[f32; 4]>,
    /// Corner offsets of `Points` and `WideLines`; empty otherwise.
    pub offsets: Vec<[f32; 2]>,
    /// x3d_ProjectionMatrix, column-major (Float32Array of the double matrix).
    pub projection: [f32; 16],
    /// x3d_ModelViewMatrix, column-major.
    pub model_view: [f32; 16],
    /// x3d_InstanceMatrix of an InstancedShape; the identity otherwise.
    pub instance: [f32; 16],
    /// x3d_InstanceNormalMatrix (`matrix .submatrix .transpose () .inverse ()`), column-major; the identity without instancing.
    pub instance_normal: [f32; 9],
    /// gl.viewport: x, y, width, height (the layer's viewport shifted by the pointer position).
    pub viewport: [i32; 4],
    /// x3d_ClipPlane: view-space planes (normal, distance); a fragment with `dot (vertex, normal) < distance` is discarded.
    pub clip_planes: Vec<[f32; 4]>,
    /// PointProperties of a point draw.
    pub point_style: Option<PointingPointStyle>,
    /// X3D_GEOMETRY_0D or X3D_GEOMETRY_1D: texCoord0 is the shader's constant (0, 0, 0, 1).
    pub constant_tex_coord: bool,
    /// x3d_Id; with `instances`, particle k carries `id + k`.
    pub id: f32,
    /// A particle of a ParticleSystem (crate::transfer).
    pub particle: bool,
    /// The particles of a ParticleSystem drawn at once (ParticleSystem.displaySimple, gl.drawArraysInstanced): their
    /// x3d_InstanceMatrix (`instance` is not used then). Each draws every vertex of the draw, or with
    /// `per_instance_vertices` n its own vertices k·n .. (k+1)·n of them (the transformed thick LINE triangles).
    ///
    /// The cache reads the matrices by the `Rc`'s address, which identifies them only because of two further facts,
    /// not because pointer equality is a validity proof on its own: the buffer *keeps a clone* of every source it
    /// uploaded, so the allocation cannot be freed and reused under the address, and that clone also makes the `Rc`
    /// non-unique, so `Rc::get_mut` is `None` and `Rc::make_mut` has to copy to a new address — which the identity
    /// test then sees as a different source. `Rc<Vec<Matrix4>>` hands out no interior mutability, so there is no third
    /// way to a `&mut Vec<Matrix4>`. A caller that ever did mutate one in place behind a live `Rc` would have to clear
    /// the cache ([`PointingGpu::forget_instances`]).
    pub instances: Option<Rc<Vec<Matrix4>>>,
    /// 0: every particle draws the whole vertex range; n: particle k draws vertices k·n .. (k+1)·n of it.
    pub per_instance_vertices: u32,
    /// The appearance's DepthMode (X3DRenderObject.pointing: `depthModeNode .enable (gl)` around the draw): depth test and
    /// function, depth mask, depth range and polygon offset. `None` keeps the pass defaults (LEQUAL, writes on).
    pub depth: Option<crate::traverse::DepthParams>,
}

/// What the pointing buffer holds after the draw: gl_FragData 0 to 2.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointingSample {
    pub id: f32,
    pub point: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord: [f32; 4],
}

pub const IDENTITY3: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

pub const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/// What a pointing pipeline varies with: the primitive topology, the gl_FragData pass and the DepthMode state.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct PipelineKey {
    lines: bool,
    second_pass: bool,
    depth_test: bool,
    depth_write: bool,
    depth_func: wgpu::CompareFunction,
    /// DepthMode polygonOffset (units, factor bits): POLYGON_OFFSET_FILL, polygons only.
    bias: (i32, u32),
}

impl PipelineKey {
    fn new(tri: &PointingTriangle, second_pass: bool) -> PipelineKey {
        use crate::traverse::DepthFunc;

        let mut key = PipelineKey {
            lines: tri.primitive == PointingPrimitive::Lines,
            second_pass,
            depth_test: true,
            depth_write: true,
            depth_func: wgpu::CompareFunction::LessEqual,
            bias: (0, 0),
        };

        if let Some(d) = &tri.depth {
            key.depth_test = d.test;
            key.depth_write = d.mask;
            key.depth_func = match d.func {
                DepthFunc::Never => wgpu::CompareFunction::Never,
                DepthFunc::Less => wgpu::CompareFunction::Less,
                DepthFunc::Equal => wgpu::CompareFunction::Equal,
                DepthFunc::LessEqual => wgpu::CompareFunction::LessEqual,
                DepthFunc::Greater => wgpu::CompareFunction::Greater,
                DepthFunc::NotEqual => wgpu::CompareFunction::NotEqual,
                DepthFunc::GreaterEqual => wgpu::CompareFunction::GreaterEqual,
                DepthFunc::Always => wgpu::CompareFunction::Always,
            };

            // POLYGON_OFFSET_FILL: polygons only (not GL points or lines).
            if tri.primitive == PointingPrimitive::Triangles {
                key.bias = (
                    d.polygon_offset[1].round() as i32,
                    (d.polygon_offset[0] as f32).to_bits(),
                );
            }
        }

        key
    }
}

/// MultiSampleFramebuffer / PointingBuffer: DEPTH_COMPONENT24.
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

pub struct PointingGpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// The pipelines made so far, by [`PipelineKey`]: the pass defaults and each DepthMode state a draw asked for.
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    module: wgpu::ShaderModule,
    pipeline_layout: wgpu::PipelineLayout,
    uniforms: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    group: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    /// The instance matrices of the pass's instanced draws, one draw's after the other (capacity in matrices).
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    /// The instance matrices the instance buffer holds, in order (kept alive so the addresses identify them).
    instance_sources: Vec<Rc<Vec<Matrix4>>>,
    targets: [wgpu::Texture; 3],
    views: [wgpu::TextureView; 3],
    depth: wgpu::TextureView,
    readback: wgpu::Buffer,
    /// The device was lost under a readback: the buffer gives no more samples.
    lost: bool,
    /// Readbacks that did not arrive, lost device or not ([`PointingGpu::readback_failures`]).
    readback_failures: usize,
    /// Mappings that failed in a row under a clean poll; a readback that arrives clears it.
    map_failures: usize,
    /// Samples taken (for tests).
    pub samples: usize,
    /// Draws the last sample submitted per pass (for tests): a ParticleSystem counts once, however many particles it
    /// has.
    pub draws: usize,
}

const ROW: u64 = 256;

impl PointingGpu {
    /// A headless Vulkan device of its own (the discrete GPU), like [`crate::gpu::Gpu::new`].
    pub fn new_headless() -> Result<PointingGpu, String> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = if std::env::var("X3D_POINTING_BACKEND").as_deref() == Ok("gl") {
            wgpu::Backends::GL
        } else {
            wgpu::Backends::VULKAN
        };
        let instance = wgpu::Instance::new(desc);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        }))
        .map_err(|e| format!("no Vulkan adapter: {e}"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("x3d-pointing"),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        Ok(PointingGpu::new(device, queue))
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// The device was lost under a readback: every later sample is `None`, and the caller picks on the CPU.
    pub fn lost(&self) -> bool {
        self.lost
    }

    /// How many readbacks did not arrive. A mapping that fails under a clean poll counts here and leaves
    /// [`PointingGpu::lost`] false: that sample falls back to the CPU and the next one is taken on the GPU again.
    pub fn readback_failures(&self) -> usize {
        self.readback_failures
    }

    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> PointingGpu {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pointing"),
            source: wgpu::ShaderSource::Wgsl(POINTING_WGSL.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pointing"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<Uniforms>() as u64
                        ),
                    },
                    count: None,
                },
                // 1: the instance matrices (pointing.wgsl `instances`).
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pointing"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipelines = HashMap::new();
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pointing uniforms"),
            size: UNIFORM_STRIDE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // One matrix to start with: a storage binding of zero bytes is not allowed.
        let instance_capacity = 1;
        let instance_buffer = Self::make_instances(&device, instance_capacity);
        let group = Self::bind_group(&device, &layout, &uniforms, &instance_buffer);
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pointing vertices"),
            size: 6 * std::mem::size_of::<VertexData>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let texture = |label: &str, format: wgpu::TextureFormat, usage: wgpu::TextureUsages| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
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
        let color = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC;
        let targets = [0, 1, 2].map(|i| {
            texture(
                &format!("pointing buffer {i}"),
                wgpu::TextureFormat::Rgba32Float,
                color,
            )
        });
        let views =
            [0, 1, 2].map(|i| targets[i].create_view(&wgpu::TextureViewDescriptor::default()));
        let depth = texture(
            "pointing depth",
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
        .create_view(&wgpu::TextureViewDescriptor::default());
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pointing readback"),
            size: 3 * ROW,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        PointingGpu {
            device,
            queue,
            pipelines,
            module,
            pipeline_layout,
            uniforms,
            layout,
            group,
            vertices,
            instance_buffer,
            instance_capacity,
            instance_sources: vec![],
            targets,
            views,
            depth,
            readback,
            lost: false,
            readback_failures: 0,
            map_failures: 0,
            samples: 0,
            draws: 0,
        }
    }

    /// The pipeline of `key`, made on first use. gl_FragData 2 is drawn in a second, identical pass (WebGPU allows 32 bytes
    /// of color attachments per sample): same primitives, same order, same depth state, its own buffer.
    fn make_pipeline(&self, key: PipelineKey) -> wgpu::RenderPipeline {
        let attributes = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 16,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 28,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 44,
                shader_location: 3,
            },
        ];
        let buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<VertexData>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        })];
        let rgba32 = || {
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba32Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })
        };
        let (entry, targets) = if key.second_pass {
            ("fs_b", vec![rgba32()])
        } else {
            ("fs_a", vec![rgba32(), rgba32()])
        };
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&self.pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.module,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &buffers,
                },
                // gl.disable (gl .CULL_FACE)
                primitive: wgpu::PrimitiveState {
                    topology: if key.lines {
                        wgpu::PrimitiveTopology::LineList
                    } else {
                        wgpu::PrimitiveTopology::TriangleList
                    },
                    cull_mode: None,
                    ..Default::default()
                },
                // X3DRenderingContext: gl.depthFunc (gl .LEQUAL), depth writes on; X3DRenderObject.pointing then enables
                // the appearance's DepthMode (DepthMode.enable): test and function, mask, polygon offset. gl.disable
                // (DEPTH_TEST) also stops depth writes. The depth range is the vertex shader's (flags .y).
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(key.depth_test && key.depth_write),
                    depth_compare: Some(if key.depth_test {
                        key.depth_func
                    } else {
                        wgpu::CompareFunction::Always
                    }),
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState {
                        constant: key.bias.0,
                        slope_scale: f32::from_bits(key.bias.1),
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &self.module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &targets,
                }),
                multiview_mask: None,
                cache: None,
            })
    }

    fn bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniforms: &wgpu::Buffer,
        instances: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pointing"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: uniforms,
                        offset: 0,
                        size: wgpu::BufferSize::new(std::mem::size_of::<Uniforms>() as u64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: instances.as_entire_binding(),
                },
            ],
        })
    }

    fn make_instances(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pointing instances"),
            size: (capacity * std::mem::size_of::<[f32; 16]>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Drop the instance-matrix cache, so the next pass uploads its sources again. Only a caller that mutates a
    /// `Vec<Matrix4>` in place behind a live `Rc` needs this; the viewer builds a fresh `Rc` per walk.
    pub fn forget_instances(&mut self) {
        self.instance_sources.clear();
    }

    /// The instance matrices held (for tests).
    pub fn instance_sources(&self) -> usize {
        self.instance_sources.len()
    }

    /// Draw `tri` into the cleared 1x1 buffer and read it back. `Ok(None)` when no fragment was written.
    pub fn sample(
        &mut self,
        tri: &PointingTriangle,
    ) -> Result<Option<PointingSample>, ReadbackFailure> {
        self.sample_all(std::slice::from_ref(tri))
    }

    /// X3DRenderObject.pointing: clear, draw every shape in order with its x3d_Id (depth test LEQUAL), read the buffer
    /// back (PointingBuffer.getHit). `Ok(None)` when no fragment was written; `Err` when no answer came back at all,
    /// which is not the same as a miss and sends the caller to the CPU ray cast for this pick.
    pub fn sample_all(
        &mut self,
        draws: &[PointingTriangle],
    ) -> Result<Option<PointingSample>, ReadbackFailure> {
        self.samples += 1;
        self.draws = draws.len();

        if self.lost {
            return Err(ReadbackFailure::DeviceLost);
        }

        if draws.is_empty() {
            return Ok(None);
        }

        // The instance matrices of the instanced draws, one draw's after the other: written only when they are not
        // those of the last pass (a pick that repeats on the same walk draws the same systems).
        let sources: Vec<&Rc<Vec<Matrix4>>> =
            draws.iter().filter_map(|d| d.instances.as_ref()).collect();
        let total: usize = sources.iter().map(|m| m.len()).sum();
        let same = sources.len() == self.instance_sources.len()
            && sources
                .iter()
                .zip(&self.instance_sources)
                .all(|(a, b)| Rc::ptr_eq(a, b));
        let mut regroup = false;

        if total > self.instance_capacity {
            self.instance_capacity = total.next_power_of_two();
            self.instance_buffer = Self::make_instances(&self.device, self.instance_capacity);
            self.instance_sources.clear();
            regroup = true;
        }

        if !same || regroup {
            let data: Vec<[f32; 16]> = sources
                .iter()
                .flat_map(|m| m.iter().map(|m| m.0.map(|v| v as f32)))
                .collect();

            if !data.is_empty() {
                let bytes: &[u8] = bytemuck::cast_slice(&data);

                self.queue.write_buffer(&self.instance_buffer, 0, bytes);
                crate::transfer::upload(crate::transfer::Kind::Particle, bytes.len());
            }

            self.instance_sources = sources.into_iter().cloned().collect();
        }

        let mut data: Vec<VertexData> = vec![];
        let mut ranges = vec![];
        let mut uniforms: Vec<u8> = vec![];
        let (mut particle_vertices, mut particle_draws) = (0usize, 0usize);
        let mut first = 0u32;

        for (k, tri) in draws.iter().enumerate() {
            let start = data.len() as u32;
            data.extend((0..tri.vertices.len()).map(|i| VertexData {
                vertex: tri.vertices[i],
                normal: tri.normals[i],
                tex_coord: tri.tex_coords[i],
                offset: tri.offsets.get(i).copied().unwrap_or([0.0; 2]),
            }));
            ranges.push(start..data.len() as u32);

            if tri.particle {
                particle_vertices += tri.vertices.len();
                particle_draws += 1;
            }

            let mode = match tri.primitive {
                PointingPrimitive::Triangles | PointingPrimitive::Lines => 0.0,
                PointingPrimitive::Points => 1.0,
                PointingPrimitive::WideLines => 2.0,
            };
            let style = tri.point_style;
            let mut clip_planes = [[0.0; 4]; MAX_POINTING_CLIP_PLANES];
            let planes = tri.clip_planes.len().min(MAX_POINTING_CLIP_PLANES);
            clip_planes[..planes].copy_from_slice(&tri.clip_planes[..planes]);
            let n = tri.instance_normal;
            let u = Uniforms {
                projection: tri.projection,
                model_view: tri.model_view,
                instance: tri.instance,
                instance_normal: [
                    [n[0], n[1], n[2], 0.0],
                    [n[3], n[4], n[5], 0.0],
                    [n[6], n[7], n[8], 0.0],
                ],
                id: [
                    tri.id,
                    mode,
                    style.map_or(0.0, |_| 1.0),
                    style.map_or(0.0, |s| s.texture as u8 as f32),
                ],
                viewport: tri.viewport.map(|v| v as f32),
                point: style.map_or([0.0; 4], |s| {
                    [
                        s.scale_factor,
                        s.min_value,
                        s.max_value,
                        s.any_texture as u8 as f32,
                    ]
                }),
                attenuation: style.map_or([1.0, 0.0, 0.0, planes as f32], |s| {
                    [
                        s.attenuation[0],
                        s.attenuation[1],
                        s.attenuation[2],
                        planes as f32,
                    ]
                }),
                clip_planes,
                flags: match &tri.depth {
                    // gl.depthRange (near, far). WebGL rejects near > far (INVALID_OPERATION) and keeps the range in force,
                    // which is the default (0, 1): DepthMode.disable restores it after every shape. The pinned reference
                    // (oracle rea-batch23 renderer.json) ignores a reversed range, so a reversed range draws unchanged.
                    Some(d) => {
                        let [n, f] = if d.range[0] > d.range[1] {
                            [0.0, 1.0]
                        } else {
                            d.range
                        };

                        [tri.constant_tex_coord as u8 as f32, 1.0, n as f32, f as f32]
                    }
                    None => [tri.constant_tex_coord as u8 as f32, 0.0, 0.0, 0.0],
                },
                instancing: match &tri.instances {
                    Some(m) => {
                        first += m.len() as u32;
                        [1, first - m.len() as u32, tri.per_instance_vertices, start]
                    }
                    None => [0; 4],
                },
            };
            uniforms.resize(k * UNIFORM_STRIDE as usize, 0);
            uniforms.extend_from_slice(bytemuck::bytes_of(&u));
        }

        let bytes = (data.len() * std::mem::size_of::<VertexData>()) as u64;

        if self.vertices.size() < bytes {
            self.vertices = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("pointing vertices"),
                size: bytes.next_power_of_two(),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        if self.uniforms.size() < uniforms.len() as u64 {
            self.uniforms = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("pointing uniforms"),
                size: (uniforms.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            regroup = true;
        }

        // A new uniform or instance buffer needs the bind group again.
        if regroup {
            self.group = Self::bind_group(
                &self.device,
                &self.layout,
                &self.uniforms,
                &self.instance_buffer,
            );
        }

        if bytes > 0 {
            self.queue
                .write_buffer(&self.vertices, 0, bytemuck::cast_slice(&data));
            crate::transfer::split_upload(
                bytes as usize,
                particle_vertices * std::mem::size_of::<VertexData>(),
            );
        }

        self.queue.write_buffer(&self.uniforms, 0, &uniforms);
        crate::transfer::split_upload(uniforms.len(), particle_draws * UNIFORM_STRIDE as usize);

        // The pipelines of the draws, made before the passes borrow them.
        let keys: Vec<[PipelineKey; 2]> = draws
            .iter()
            .map(|d| [PipelineKey::new(d, false), PipelineKey::new(d, true)])
            .collect();

        for key in keys.iter().flatten() {
            if !self.pipelines.contains_key(key) {
                let pipeline = self.make_pipeline(*key);

                self.pipelines.insert(*key, pipeline);
            }
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("pointing"),
            });
        let clear = |view| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })
        };

        for (pass_index, views) in [&self.views[0..2], &self.views[2..3]]
            .into_iter()
            .enumerate()
        {
            let attachments: Vec<_> = views.iter().map(clear).collect();
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pointing"),
                color_attachments: &attachments,
                // gl .clear (gl .DEPTH_BUFFER_BIT)
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            if bytes > 0 {
                pass.set_vertex_buffer(0, self.vertices.slice(..bytes));
            }

            for (k, (tri, range)) in draws.iter().zip(&ranges).enumerate() {
                if range.is_empty() {
                    continue;
                }

                // One draw of every particle: instance_index, or the vertices of each in turn.
                let instances = match &tri.instances {
                    Some(m) if tri.per_instance_vertices == 0 => m.len() as u32,
                    Some(m) if m.is_empty() => 0,
                    _ => 1,
                };

                if instances == 0 {
                    continue;
                }

                let [x, y, w, h] = tri.viewport.map(|v| v as f32);
                pass.set_pipeline(&self.pipelines[&keys[k][pass_index]]);
                pass.set_bind_group(0, &self.group, &[(k as u64 * UNIFORM_STRIDE) as u32]);
                pass.set_viewport(x, y, w, h, 0.0, 1.0);
                pass.draw(range.clone(), 0..instances);
            }
        }

        for (i, texture) in self.targets.iter().enumerate() {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &self.readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: i as u64 * ROW,
                        bytes_per_row: Some(ROW as u32),
                        rows_per_image: Some(1),
                    },
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }

        self.queue.submit([encoder.finish()]);
        let slice = self.readback.slice(..);
        crate::transfer::readback(crate::transfer::Kind::Other, self.readback.size());

        if let Err(e) = map_read(&self.device, &slice) {
            self.fail_readback(e);
            return Err(e);
        }

        self.map_failures = 0;

        let mapped = slice.get_mapped_range().expect("map pointing readback");
        let f = |i: usize| -> [f32; 4] {
            std::array::from_fn(|k| {
                f32::from_le_bytes(
                    mapped[i * ROW as usize + k * 4..i * ROW as usize + k * 4 + 4]
                        .try_into()
                        .unwrap(),
                )
            })
        };
        let (a, b, c) = (f(0), f(1), f(2));
        drop(mapped);
        self.readback.unmap();

        Ok((a[3] != 0.0).then_some(PointingSample {
            id: a[3],
            point: [a[0], a[1], a[2]],
            normal: [b[0], b[1], b[2]],
            tex_coord: c,
        }))
    }

    /// Record a readback that did not arrive. A lost device is lost at once; a mapping that failed under a clean poll
    /// may be the one blip, so the buffer keeps working and only gives up once [`MAX_MAP_FAILURES`] in a row say the
    /// device is not coming back (`wgpu::Device::destroy` is this case: the poll stays clean, every mapping fails).
    fn fail_readback(&mut self, e: ReadbackFailure) {
        self.readback_failures += 1;

        match e {
            ReadbackFailure::DeviceLost => self.lost = true,
            ReadbackFailure::Mapping => {
                self.map_failures += 1;
                self.lost |= self.map_failures >= MAX_MAP_FAILURES;
            }
        }
    }
}

/// Mappings that may fail in a row under a clean poll before the buffer gives the device up. One is a blip and costs
/// that pick the CPU ray cast; a run of them is a device that answers polls and nothing else.
pub const MAX_MAP_FAILURES: usize = 3;

/// Why a readback did not arrive. The two are not the same failure: a lost device never works again, a mapping that
/// failed on a live device says nothing about the next one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadbackFailure {
    /// The poll failed: the device is gone, and every buffer on it with it.
    DeviceLost,
    /// `map_async` reported an error (or never ran) while the device polled cleanly. This readback is lost; the device
    /// is still there, so the caller falls back to the CPU for this one and may use the GPU again.
    Mapping,
}

/// Map `slice` for reading and wait for it.
pub(crate) fn map_read(
    device: &wgpu::Device,
    slice: &wgpu::BufferSlice,
) -> Result<(), ReadbackFailure> {
    let mapped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let done = mapped.clone();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        done.store(r.is_ok(), std::sync::atomic::Ordering::Release)
    });

    // Only the poll tells the device apart from the mapping: `BufferAsyncError` carries no cause, so a mapping error
    // after a clean poll is read as the mapping's own, not as the device's.
    if device.poll(wgpu::PollType::wait_indefinitely()).is_err() {
        return Err(ReadbackFailure::DeviceLost);
    }

    if !mapped.load(std::sync::atomic::Ordering::Acquire) {
        return Err(ReadbackFailure::Mapping);
    }

    Ok(())
}
