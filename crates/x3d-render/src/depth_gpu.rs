//! The collision depth buffer of X_ITE (X3DRenderObject.depth / getCollisionShape, Depth2.vs and
//! Depth2.fs with X3D_NORMAL_BUFFER, TextureBuffer 16x16 with two RGBA32F color textures and a
//! 24 bit depth texture). The collision shapes are drawn through the same pipeline so the
//! rasterizer's clipping and vertex snapping decide the depth, as they do in X_ITE; the caller
//! reads the closest pixel back like TextureBuffer.readDepth.

use bytemuck::{Pod, Zeroable};
use crate::geometry::Geometry;
use std::collections::HashMap;
use std::rc::Rc;

pub const DEPTH_WGSL: &str = include_str!("depth.wgsl");

/// DEPTH_BUFFER_SIZE of X3DRenderObject.
pub const SIZE: u32 = 16;

const MAX_CLIP_PLANES: usize = 6;
/// Bytes between the uniform blocks of two shapes (dynamic offsets are 256 aligned).
const STRIDE: u64 = 512;
/// Bytes of one pixel row in the readback (copy_texture_to_buffer rows are 256 aligned).
const ROW: u64 = 256;
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
    /// Draw nothing (points and lines are not drawn here).
    pub empty: bool,
    /// x3d_ModelViewMatrix (Float32Array of the shape's model-view matrix).
    pub model_view: [f32; 16],
    /// x3d_InstanceMatrix of an InstancedShape (identity otherwise) and x3d_InstanceNormalMatrix
    /// (column-major 3x3).
    pub instance: [f32; 16],
    pub instance_normal: [f32; 9],
    /// renderContext .clipPlanes (view space of the model-view matrix).
    pub clip_planes: &'a [[f64; 4]],
}

/// gl_FragData 0 and 1 for every pixel, row 0 first (GL's bottom row).
pub struct DepthPixels {
    pub data0: Vec<[f32; 4]>,
    pub data1: Vec<[f32; 4]>,
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
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    targets: [wgpu::Texture; 2],
    views: [wgpu::TextureView; 2],
    depth_view: wgpu::TextureView,
    readback: wgpu::Buffer,
    uniforms: Option<(wgpu::Buffer, wgpu::BindGroup, u64)>,
    buffers: HashMap<usize, Buffer>,
    queries: u64,
    /// Passes drawn (for tests).
    pub passes: usize,
}

impl DepthGpu {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> DepthGpu {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("depth"), source: wgpu::ShaderSource::Wgsl(DEPTH_WGSL.into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("depth"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: true, min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Draw>() as u64) },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("depth"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let attributes = [
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0, shader_location: 0 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 16, shader_location: 1 },
        ];
        let buffers = [Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attributes })];
        let target = || Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba32Float, blend: None, write_mask: wgpu::ColorWrites::ALL });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("depth"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &buffers },
            // gl .disable (gl .CULL_FACE)
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            // X3DRenderingContext: gl .enable (gl .DEPTH_TEST), gl .depthFunc (gl .LEQUAL)
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs_main"), compilation_options: Default::default(), targets: &[target(), target()] }),
            multiview_mask: None,
            cache: None,
        });
        let make = |i: u32| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("depth buffer {i}")),
                size: wgpu::Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };
        let targets = [make(0), make(1)];
        let views = [targets[0].create_view(&Default::default()), targets[1].create_view(&Default::default())];
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth buffer depth"),
            size: wgpu::Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&Default::default());
        let readback = device.create_buffer(&wgpu::BufferDescriptor { label: Some("depth readback"), size: 2 * ROW * SIZE as u64, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });

        DepthGpu { device, queue, pipeline, layout, targets, views, depth_view, readback, uniforms: None, buffers: HashMap::new(), queries: 0, passes: 0 }
    }

    /// Vertex buffers held (0 after `release`).
    pub fn resources(&self) -> usize {
        self.buffers.len()
    }

    pub fn release(&mut self) {
        self.buffers.clear();
    }

    /// X3DRenderObject.depth (shapes, numShapes, true) into the cleared buffer, then the readback.
    /// `projection` is the view-projection matrix of the pass (Float32Array of the double matrix).
    pub fn draw(&mut self, projection: &[f32; 16], shapes: &[DepthShape]) -> DepthPixels {
        self.passes += 1;
        self.queries += 1;
        let stamp = self.queries;

        for s in shapes {
            let key = Rc::as_ptr(s.geometry) as usize;
            let count = if s.empty { 0 } else { (s.geometry.vertices.len() / 4) as u32 };

            match self.buffers.get_mut(&key) {
                Some(b) if b.count == count => b.used = stamp,
                _ => {
                    let g = &s.geometry;
                    let data: Vec<Vertex> = (0..count as usize)
                        .map(|i| Vertex {
                            vertex: [g.vertices[i * 4], g.vertices[i * 4 + 1], g.vertices[i * 4 + 2], g.vertices[i * 4 + 3]],
                            // No normals: the fragment shader's constant (0, 0, 1).
                            normal: if g.normals.len() >= (i + 1) * 3 { [g.normals[i * 3], g.normals[i * 3 + 1], g.normals[i * 3 + 2]] } else { [0.0, 0.0, 1.0] },
                        })
                        .collect();
                    let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("depth vertices"),
                        size: (data.len() * std::mem::size_of::<Vertex>()).max(4) as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    self.queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&data));
                    self.buffers.insert(key, Buffer { _geometry: s.geometry.clone(), buffer, count, used: stamp });
                }
            }
        }

        self.buffers.retain(|_, b| stamp - b.used < KEEP);

        // One uniform block per shape.
        let needed = (shapes.len().max(1) as u64) * STRIDE;

        if self.uniforms.as_ref().is_none_or(|u| u.2 < needed) {
            let size = needed.next_power_of_two();
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor { label: Some("depth uniforms"), size, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("depth"),
                layout: &self.layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &buffer, offset: 0, size: wgpu::BufferSize::new(std::mem::size_of::<Draw>() as u64) }) }],
            });
            self.uniforms = Some((buffer, group, size));
        }

        let (uniforms, group, _) = self.uniforms.as_ref().unwrap();
        let mut blob = vec![0u8; shapes.len() * STRIDE as usize];

        for (i, s) in shapes.iter().enumerate() {
            let mut planes = [[0.0f32; 4]; MAX_CLIP_PLANES];

            for (k, p) in s.clip_planes.iter().take(MAX_CLIP_PLANES).enumerate() {
                planes[k] = p.map(|v| v as f32);
            }

            let n = s.instance_normal;
            let draw = Draw { projection: *projection, model_view: s.model_view, instance: s.instance, instance_normal: [[n[0], n[1], n[2], 0.0], [n[3], n[4], n[5], 0.0], [n[6], n[7], n[8], 0.0]], clip_planes: planes, params: [i as f32, s.clip_planes.len().min(MAX_CLIP_PLANES) as f32, 0.0, 0.0] };
            blob[i * STRIDE as usize..i * STRIDE as usize + std::mem::size_of::<Draw>()].copy_from_slice(bytemuck::bytes_of(&draw));
        }

        if !blob.is_empty() {
            self.queue.write_buffer(uniforms, 0, &blob);
        }

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("depth") });
        {
            // depthClearColor: (1, -1, -1, -1), '1' for infinity, '-1' for no hit.
            let clear = |view| Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 1.0, g: -1.0, b: -1.0, a: -1.0 }), store: wgpu::StoreOp::Store } });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("depth"),
                color_attachments: &[clear(&self.views[0]), clear(&self.views[1])],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_viewport(0.0, 0.0, SIZE as f32, SIZE as f32, 0.0, 1.0);

            for (i, s) in shapes.iter().enumerate() {
                let b = &self.buffers[&(Rc::as_ptr(s.geometry) as usize)];

                if b.count == 0 {
                    continue;
                }

                pass.set_bind_group(0, group, &[(i as u64 * STRIDE) as u32]);
                pass.set_vertex_buffer(0, b.buffer.slice(..));
                pass.draw(0..b.count, 0..1);
            }
        }

        for (i, texture) in self.targets.iter().enumerate() {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                wgpu::TexelCopyBufferInfo { buffer: &self.readback, layout: wgpu::TexelCopyBufferLayout { offset: i as u64 * ROW * SIZE as u64, bytes_per_row: Some(ROW as u32), rows_per_image: Some(SIZE) } },
                wgpu::Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
            );
        }

        self.queue.submit([encoder.finish()]);
        let slice = self.readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let mapped = slice.get_mapped_range().expect("map depth readback");
        let read = |t: usize| -> Vec<[f32; 4]> {
            (0..(SIZE * SIZE) as usize)
                .map(|i| {
                    let o = t * (ROW * SIZE as u64) as usize + i * 16;
                    std::array::from_fn(|k| f32::from_le_bytes(mapped[o + k * 4..o + k * 4 + 4].try_into().unwrap()))
                })
                .collect()
        };
        let pixels = DepthPixels { data0: read(0), data1: read(1) };
        drop(mapped);
        self.readback.unmap();
        pixels
    }
}
