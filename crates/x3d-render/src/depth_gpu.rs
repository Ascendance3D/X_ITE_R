//! The collision depth buffer of X_ITE (X3DRenderObject.depth / getCollisionShape, Depth2.vs and
//! Depth2.fs with X3D_NORMAL_BUFFER, TextureBuffer 16x16 with two RGBA32F color textures and a
//! 24 bit depth texture). The collision shapes are drawn through the same pipeline so the
//! rasterizer's clipping and vertex snapping decide the depth, as they do in X_ITE; the caller
//! reads the closest pixel back like TextureBuffer.readDepth. A geometry of `geometry_type` 0 is drawn as gl.POINTS
//! (1 px, Depth2.vs `gl_PointSize = 1.0`) through a point list pipeline. Lines are not drawn here: a WebGPU line list
//! does not cover GL's pixels (the diamond-exit rule), so the caller rasterizes them and passes them as `empty`.

use crate::geometry::Geometry;
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use std::rc::Rc;
use web_time::{Duration, Instant};
use x3d_fields::math::Matrix4;

pub const DEPTH_WGSL: &str = include_str!("depth.wgsl");

/// DEPTH_BUFFER_SIZE of X3DRenderObject.
pub const SIZE: u32 = 16;

const MAX_CLIP_PLANES: usize = 6;
/// Bytes between the uniform blocks of two shapes (dynamic offsets are 256 aligned).
const STRIDE: u64 = 512;
/// Bytes of one pixel row in the readback (copy_texture_to_buffer rows are 256 aligned).
const ROW: u64 = 256;
/// Bytes of one pass's readback region: its two color targets.
const REGION: u64 = 2 * ROW * SIZE as u64;
/// Bytes of one pass's two timestamps.
const TIMESTAMPS: u64 = 16;
/// depthClearColor: (1, -1, -1, -1), '1' for infinity, '-1' for no hit.
const CLEAR: [f32; 4] = [1.0, -1.0, -1.0, -1.0];
/// Frames (queries) an unused vertex buffer stays.
const KEEP: u64 = 600;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Draw {
    projection: [f32; 16],
    model_view: [f32; 16],
    instance: [f32; 16],
    instance_normal: [[f32; 4]; 3],
    clip_planes: [[f32; 4]; MAX_CLIP_PLANES],
    params: [f32; 4],
    /// x: 1 when the instance matrices come from `instances`, y: the first, z: vertices per instance (0: one
    /// instance per gl_InstanceID).
    instancing: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    vertex: [f32; 4],
    normal: [f32; 3],
}

/// One collision shape of the pass.
pub struct DepthShape<'a> {
    /// The geometry; its vertex buffer is kept under the pointer, and the buffer keeps the
    /// geometry alive so that the address is not reused by another one.
    pub geometry: &'a Rc<Geometry>,
    /// Draw nothing (the caller decides which shapes take part with their points and lines).
    pub empty: bool,
    /// x3d_ModelViewMatrix (Float32Array of the shape's model-view matrix).
    pub model_view: [f32; 16],
    /// x3d_InstanceMatrix of an InstancedShape (identity otherwise) and x3d_InstanceNormalMatrix
    /// (column-major 3x3).
    pub instance: [f32; 16],
    pub instance_normal: [f32; 9],
    /// renderContext .clipPlanes (view space of the model-view matrix).
    pub clip_planes: &'a [[f64; 4]],
    /// A particle of a ParticleSystem (crate::transfer).
    pub particle: bool,
    /// The particles of a ParticleSystem drawn at once (ParticleSystem.displaySimple, gl.drawArraysInstanced): their
    /// x3d_InstanceMatrix (`instance` is not used then), particle k with x3d_Id `id + k`. Each draws `geometry`, or with
    /// `per_instance_vertices` n its own vertices k·n .. (k+1)·n of it (the transformed LINE triangles).
    pub instances: Option<&'a Rc<Vec<Matrix4>>>,
    pub per_instance_vertices: u32,
    /// x3d_Id: the shape's index among the collision shapes (of its first particle).
    pub id: f32,
}

impl DepthShape<'_> {
    /// The vertices of its vertex buffer: `Geometry::vertices` holds four floats a vertex (x3d_Vertex).
    fn count(&self) -> u32 {
        if self.empty {
            0
        } else {
            (self.geometry.vertices.len() / 4) as u32
        }
    }

    /// Whether the pass draws anything of it: one vertex or more (a triangle is three), and particles if it has them.
    fn draws(&self) -> bool {
        self.count() > 0 && !self.instances.is_some_and(|m| m.is_empty())
    }
}

/// gl_FragData 0 and 1 for every pixel, row 0 first (GL's bottom row).
pub struct DepthPixels {
    pub data0: Vec<[f32; 4]>,
    pub data1: Vec<[f32; 4]>,
}

impl DepthPixels {
    /// The buffer as the pass clears it: [`CLEAR`] in both targets.
    fn cleared() -> DepthPixels {
        let clear = vec![CLEAR; (SIZE * SIZE) as usize];

        DepthPixels {
            data0: clear.clone(),
            data1: clear,
        }
    }
}

/// What the passes cost since the last [`DepthGpu::take_stats`] (for benchmarks).
#[derive(Clone, Copy, Debug, Default)]
pub struct DepthStats {
    /// Collision queries answered, one 16x16 pass each.
    pub queries: usize,
    pub submissions: usize,
    pub polls: usize,
    /// Mappings waited for.
    pub readbacks: usize,
    pub upload_bytes: u64,
    pub readback_bytes: u64,
    /// CPU time building, encoding and submitting the passes.
    pub encode: Duration,
    /// Time blocked in the poll for the mapping: the queue's earlier work, the passes and the copies.
    pub wait: Duration,
    /// Time reading the mapped bytes out and unmapping.
    pub transfer: Duration,
    /// GPU time of the passes from timestamp queries. Zero, and no measurement, when [`DepthGpu::set_timestamps`] is
    /// off or returned false (no `Features::TIMESTAMP_QUERY`): check [`DepthGpu::timestamps`] before reading it.
    pub gpu: Duration,
}

struct Buffer {
    _geometry: Rc<Geometry>,
    buffer: wgpu::Buffer,
    count: u32,
    used: u64,
}

pub struct DepthGpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// Triangle and point list (geometryType 2 and 3, 0).
    pipelines: [wgpu::RenderPipeline; 2],
    layout: wgpu::BindGroupLayout,
    targets: [wgpu::Texture; 2],
    views: [wgpu::TextureView; 2],
    depth_view: wgpu::TextureView,
    readback: wgpu::Buffer,
    /// The uniform blocks, the instance matrices (`instances` of the pass's shapes, capacity in matrices) and their bind
    /// group.
    uniforms: Option<(wgpu::Buffer, wgpu::BindGroup, u64)>,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    /// The instance matrices the instance buffer holds, in order (kept alive so the addresses identify them).
    instance_sources: Vec<Rc<Vec<Matrix4>>>,
    buffers: HashMap<usize, Buffer>,
    queries: u64,
    /// The device was lost under a readback: no more passes are drawn.
    lost: bool,
    /// Readbacks that did not arrive, lost device or not ([`DepthGpu::readback_failures`]).
    readback_failures: usize,
    /// Mappings that failed in a row under a clean poll; a readback that arrives clears it.
    map_failures: usize,
    /// Passes answered, the ones with nothing to draw included (for tests).
    pub passes: usize,
    stats: DepthStats,
    timestamps: bool,
    /// The timestamp queries (two per pass), their resolve buffer and their capacity in passes.
    query_set: Option<(wgpu::QuerySet, wgpu::Buffer, usize)>,
    /// Mappings still to fail ([`DepthGpu::fail_mappings`]).
    fail_mappings: usize,
}

impl DepthGpu {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> DepthGpu {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("depth"),
            source: wgpu::ShaderSource::Wgsl(DEPTH_WGSL.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("depth"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Draw>() as u64),
                    },
                    count: None,
                },
                // 1: the instance matrices (depth.wgsl `instances`).
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
            label: Some("depth"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
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
        ];
        let buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        })];
        let target = || {
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba32Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })
        };
        let pipeline = |topology| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("depth"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &buffers,
                },
                // gl .disable (gl .CULL_FACE)
                primitive: wgpu::PrimitiveState {
                    topology,
                    cull_mode: None,
                    ..Default::default()
                },
                // X3DRenderingContext: gl .enable (gl .DEPTH_TEST), gl .depthFunc (gl .LEQUAL)
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth24Plus,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[target(), target()],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipelines = [
            pipeline(wgpu::PrimitiveTopology::TriangleList),
            pipeline(wgpu::PrimitiveTopology::PointList),
        ];
        let make = |i: u32| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("depth buffer {i}")),
                size: wgpu::Extent3d {
                    width: SIZE,
                    height: SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };
        let targets = [make(0), make(1)];
        let views = [
            targets[0].create_view(&Default::default()),
            targets[1].create_view(&Default::default()),
        ];
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth buffer depth"),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&Default::default());
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("depth readback"),
            size: REGION,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let instance_buffer = Self::make_instances(&device, 64);

        DepthGpu {
            device,
            queue,
            pipelines,
            layout,
            targets,
            views,
            depth_view,
            readback,
            uniforms: None,
            instance_buffer,
            instance_capacity: 64,
            instance_sources: vec![],
            buffers: HashMap::new(),
            queries: 0,
            lost: false,
            readback_failures: 0,
            map_failures: 0,
            passes: 0,
            stats: DepthStats::default(),
            timestamps: false,
            query_set: None,
            fail_mappings: 0,
        }
    }

    fn make_instances(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("depth instances"),
            size: (capacity * 64) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Vertex buffers held (0 after `release`).
    pub fn resources(&self) -> usize {
        self.buffers.len()
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// The device was lost under a readback: `draw` returns `None` from now on, and the caller uses the CPU.
    pub fn lost(&self) -> bool {
        self.lost
    }

    /// How many readbacks did not arrive. A mapping that fails under a clean poll counts here and leaves
    /// [`DepthGpu::lost`] false: that pass falls back to the CPU stand-in and the next one is drawn on the GPU again.
    pub fn readback_failures(&self) -> usize {
        self.readback_failures
    }

    /// Record a readback that did not arrive, like [`crate::pointing_gpu::PointingGpu`]: a lost device at once, a
    /// failed mapping only once [`crate::pointing_gpu::MAX_MAP_FAILURES`] of them in a row say it is not coming back.
    fn fail_readback(&mut self, e: crate::pointing_gpu::ReadbackFailure) {
        self.readback_failures += 1;

        match e {
            crate::pointing_gpu::ReadbackFailure::DeviceLost => self.lost = true,
            crate::pointing_gpu::ReadbackFailure::Mapping => {
                self.map_failures += 1;
                self.lost |= self.map_failures >= crate::pointing_gpu::MAX_MAP_FAILURES;
            }
        }
    }

    pub fn release(&mut self) {
        self.buffers.clear();
        self.instance_sources.clear();
    }

    /// Measure the GPU time of the passes with timestamp queries ([`DepthStats::gpu`]). Returns whether the device
    /// can: `Features::TIMESTAMP_QUERY` must be requested when the `wgpu::Device` is created, this does not enable it.
    /// Off by default.
    pub fn set_timestamps(&mut self, on: bool) -> bool {
        let can = self
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY);
        self.timestamps = on && can;
        self.query_set = None;
        self.timestamps
    }

    /// Whether [`DepthStats::gpu`] is measured.
    pub fn timestamps(&self) -> bool {
        self.timestamps
    }

    /// Fail the mappings of the next `n` batches that reach the GPU, like a mapping that fails under a clean poll
    /// (for tests of the CPU fallback: the batch is drawn and submitted, then its readback is dropped).
    #[doc(hidden)]
    pub fn fail_mappings(&mut self, n: usize) {
        self.fail_mappings = n;
    }

    /// The counters since the last call, cleared.
    pub fn take_stats(&mut self) -> DepthStats {
        std::mem::take(&mut self.stats)
    }

    /// X3DRenderObject.depth (shapes, numShapes, true) into the cleared buffer, then the readback.
    /// `projection` is the view-projection matrix of the pass (Float32Array of the double matrix). `None` when the
    /// readback does not arrive: once the device is lost, or for the one pass whose mapping failed.
    pub fn draw(&mut self, projection: &[f32; 16], shapes: &[DepthShape]) -> Option<DepthPixels> {
        self.draw_batch(std::slice::from_ref(projection), shapes)
            .map(|mut p| p.pop().expect("one pass"))
    }

    /// [`DepthGpu::draw`] once for each projection, over the same shapes: independent passes, each with its own uniform
    /// blocks and its own region of the readback, recorded in order into one submission and read back through one
    /// mapping. The pixels come back in the order of `projections`. `None` when the readback does not arrive; then no
    /// pass of the batch has an answer.
    pub fn draw_batch(
        &mut self,
        projections: &[[f32; 16]],
        shapes: &[DepthShape],
    ) -> Option<Vec<DepthPixels>> {
        if self.lost {
            return None;
        }

        if projections.is_empty() {
            return Some(vec![]);
        }

        let encode = Instant::now();
        let passes = projections.len();
        let draws = shapes.iter().any(DepthShape::draws);
        self.passes += passes;
        self.stats.queries += passes;

        // A batch ages the vertex buffers by its passes, like the passes one by one.
        self.queries += passes as u64;
        let stamp = self.queries;

        for s in shapes {
            let key = Rc::as_ptr(s.geometry) as usize;
            let count = s.count();

            match self.buffers.get_mut(&key) {
                Some(b) if b.count == count => b.used = stamp,
                // Nothing to draw: the buffers it holds age with the others, and no new one is uploaded.
                _ if !draws => {}
                _ => {
                    let g = &s.geometry;
                    let data: Vec<Vertex> = (0..count as usize)
                        .map(|i| Vertex {
                            vertex: [
                                g.vertices[i * 4],
                                g.vertices[i * 4 + 1],
                                g.vertices[i * 4 + 2],
                                g.vertices[i * 4 + 3],
                            ],
                            // No normals: the fragment shader's constant (0, 0, 1).
                            normal: if g.normals.len() >= (i + 1) * 3 {
                                [g.normals[i * 3], g.normals[i * 3 + 1], g.normals[i * 3 + 2]]
                            } else {
                                [0.0, 0.0, 1.0]
                            },
                        })
                        .collect();
                    let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("depth vertices"),
                        size: (data.len() * std::mem::size_of::<Vertex>()).max(4) as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    self.queue
                        .write_buffer(&buffer, 0, bytemuck::cast_slice(&data));
                    let bytes = std::mem::size_of_val(data.as_slice());
                    crate::transfer::upload(crate::transfer::kind(s.particle), bytes);
                    self.stats.upload_bytes += bytes as u64;
                    self.buffers.insert(
                        key,
                        Buffer {
                            _geometry: s.geometry.clone(),
                            buffer,
                            count,
                            used: stamp,
                        },
                    );
                }
            }
        }

        self.buffers.retain(|_, b| stamp - b.used < KEEP);

        // Nothing to draw: every pass reads the cleared buffer, which needs no GPU.
        if !draws {
            self.stats.encode += encode.elapsed();
            return Some((0..passes).map(|_| DepthPixels::cleared()).collect());
        }

        // The instance matrices of the particle shapes, one after the other: written when they are not those of the
        // last pass (the passes of one collision query share them).
        let sources: Vec<&Rc<Vec<Matrix4>>> = shapes.iter().filter_map(|s| s.instances).collect();
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
                self.stats.upload_bytes += bytes.len() as u64;
            }

            self.instance_sources = sources.into_iter().cloned().collect();
        }

        // One uniform block per shape and pass: pass q's blocks follow pass q - 1's.
        let blocks = shapes.len() * passes;
        let needed = (blocks.max(1) as u64) * STRIDE;

        if regroup || self.uniforms.as_ref().is_none_or(|u| u.2 < needed) {
            let size = self
                .uniforms
                .as_ref()
                .map_or(0, |u| u.2)
                .max(needed.next_power_of_two());
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("depth uniforms"),
                size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("depth"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &buffer,
                            offset: 0,
                            size: wgpu::BufferSize::new(std::mem::size_of::<Draw>() as u64),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.instance_buffer.as_entire_binding(),
                    },
                ],
            });
            self.uniforms = Some((buffer, group, size));
        }

        // The readback: pass q's two color targets at q · REGION, then the timestamps.
        let timestamps = self.timestamps.then_some(passes as u64 * REGION);
        let used = passes as u64 * REGION
            + if self.timestamps {
                passes as u64 * TIMESTAMPS
            } else {
                0
            };

        if self.readback.size() < used {
            self.readback = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("depth readback"),
                size: used.next_power_of_two(),
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        if self.timestamps && self.query_set.as_ref().is_none_or(|q| q.2 < passes) {
            let capacity = passes.next_power_of_two();
            let set = self.device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("depth timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 2 * capacity as u32,
            });
            let resolve = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("depth timestamps"),
                size: TIMESTAMPS * capacity as u64,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            self.query_set = Some((set, resolve, capacity));
        }

        let (uniforms, group, _) = self.uniforms.as_ref().unwrap();
        let mut blob = vec![0u8; blocks * STRIDE as usize];

        for (q, projection) in projections.iter().enumerate() {
            let mut first = 0u32;

            for (i, s) in shapes.iter().enumerate() {
                let mut planes = [[0.0f32; 4]; MAX_CLIP_PLANES];
                let instancing = match s.instances {
                    Some(m) => {
                        first += m.len() as u32;
                        [1, first - m.len() as u32, s.per_instance_vertices, 0]
                    }
                    None => [0; 4],
                };

                for (k, p) in s.clip_planes.iter().take(MAX_CLIP_PLANES).enumerate() {
                    planes[k] = p.map(|v| v as f32);
                }

                let n = s.instance_normal;
                let draw = Draw {
                    projection: *projection,
                    model_view: s.model_view,
                    instance: s.instance,
                    instance_normal: [
                        [n[0], n[1], n[2], 0.0],
                        [n[3], n[4], n[5], 0.0],
                        [n[6], n[7], n[8], 0.0],
                    ],
                    clip_planes: planes,
                    params: [
                        s.id,
                        s.clip_planes.len().min(MAX_CLIP_PLANES) as f32,
                        0.0,
                        0.0,
                    ],
                    instancing,
                };
                let o = (q * shapes.len() + i) * STRIDE as usize;
                blob[o..o + std::mem::size_of::<Draw>()].copy_from_slice(bytemuck::bytes_of(&draw));
            }
        }

        if !blob.is_empty() {
            self.queue.write_buffer(uniforms, 0, &blob);
            crate::transfer::split_upload(
                blob.len(),
                shapes.iter().filter(|s| s.particle).count() * passes * STRIDE as usize,
            );
            self.stats.upload_bytes += blob.len() as u64;
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("depth"),
            });

        for q in 0..passes {
            {
                let clear = |view| {
                    Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: CLEAR[0] as f64,
                                g: CLEAR[1] as f64,
                                b: CLEAR[2] as f64,
                                a: CLEAR[3] as f64,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })
                };
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("depth"),
                    color_attachments: &[clear(&self.views[0]), clear(&self.views[1])],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: self.query_set.as_ref().filter(|_| self.timestamps).map(
                        |(set, _, _)| wgpu::RenderPassTimestampWrites {
                            query_set: set,
                            beginning_of_pass_write_index: Some(2 * q as u32),
                            end_of_pass_write_index: Some(2 * q as u32 + 1),
                        },
                    ),
                    ..Default::default()
                });
                pass.set_viewport(0.0, 0.0, SIZE as f32, SIZE as f32, 0.0, 1.0);
                let mut bound = usize::MAX;

                for (i, s) in shapes.iter().enumerate() {
                    let b = &self.buffers[&(Rc::as_ptr(s.geometry) as usize)];

                    if b.count == 0 {
                        continue;
                    }

                    debug_assert!(
                        s.geometry.geometry_type != 1,
                        "lines are drawn by the caller"
                    );
                    let pipeline = usize::from(s.geometry.geometry_type == 0);

                    if pipeline != bound {
                        pass.set_pipeline(&self.pipelines[pipeline]);
                        bound = pipeline;
                    }

                    // One draw of every particle: gl_InstanceID, or the vertices of each in turn.
                    let instances = match s.instances {
                        Some(m) if s.per_instance_vertices == 0 => m.len() as u32,
                        Some(m) if m.is_empty() => 0,
                        _ => 1,
                    };

                    if instances == 0 {
                        continue;
                    }

                    let block = (q * shapes.len() + i) as u64 * STRIDE;
                    pass.set_bind_group(0, group, &[block as u32]);
                    pass.set_vertex_buffer(0, b.buffer.slice(..));
                    pass.draw(0..b.count, 0..instances);
                }
            }

            // The pass's pixels to its own region before the next pass clears the targets.
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
                            offset: q as u64 * REGION + i as u64 * ROW * SIZE as u64,
                            bytes_per_row: Some(ROW as u32),
                            rows_per_image: Some(SIZE),
                        },
                    },
                    wgpu::Extent3d {
                        width: SIZE,
                        height: SIZE,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }

        if let (Some(offset), Some((set, resolve, _))) = (timestamps, &self.query_set) {
            encoder.resolve_query_set(set, 0..2 * passes as u32, resolve, 0);
            encoder.copy_buffer_to_buffer(
                resolve,
                0,
                &self.readback,
                offset,
                passes as u64 * TIMESTAMPS,
            );
        }

        self.queue.submit([encoder.finish()]);
        self.stats.submissions += 1;
        self.stats.encode += encode.elapsed();

        let slice = self.readback.slice(..used);
        crate::transfer::readback(crate::transfer::Kind::Other, used);
        self.stats.readback_bytes += used;
        self.stats.readbacks += 1;
        self.stats.polls += 1;
        let wait = Instant::now();
        let mut mapped = crate::pointing_gpu::map_read(&self.device, &slice);
        self.stats.wait += wait.elapsed();

        if mapped.is_ok() && self.fail_mappings > 0 {
            self.fail_mappings -= 1;
            self.readback.unmap();
            mapped = Err(crate::pointing_gpu::ReadbackFailure::Mapping);
        }

        if let Err(e) = mapped {
            self.fail_readback(e);
            return None;
        }

        self.map_failures = 0;

        let transfer = Instant::now();
        let mapped = slice.get_mapped_range().expect("map depth readback");
        let read = |q: usize, t: usize| -> Vec<[f32; 4]> {
            (0..(SIZE * SIZE) as usize)
                .map(|i| {
                    let o =
                        (q as u64 * REGION) as usize + t * (ROW * SIZE as u64) as usize + i * 16;
                    std::array::from_fn(|k| {
                        f32::from_le_bytes(mapped[o + k * 4..o + k * 4 + 4].try_into().unwrap())
                    })
                })
                .collect()
        };
        let pixels = (0..passes)
            .map(|q| DepthPixels {
                data0: read(q, 0),
                data1: read(q, 1),
            })
            .collect();

        if let Some(offset) = timestamps {
            let tick = |k: usize| {
                let o = offset as usize + k * 8;
                u64::from_le_bytes(mapped[o..o + 8].try_into().unwrap())
            };
            let period = self.queue.get_timestamp_period() as f64;
            let ns: f64 = (0..passes)
                .map(|q| tick(2 * q + 1).saturating_sub(tick(2 * q)) as f64 * period)
                .sum();
            self.stats.gpu += Duration::from_nanos(ns as u64);
        }

        drop(mapped);
        self.readback.unmap();
        self.stats.transfer += transfer.elapsed();
        Some(pixels)
    }
}
