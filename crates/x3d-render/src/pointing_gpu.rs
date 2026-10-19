//! The pointing buffer of X_ITE (Rendering/PointingBuffer.js, Pointing2.vs / Pointing2.fs): the
//! pixel under the pointer is drawn into a 1x1 RGBA32F buffer with the viewport shifted so that it
//! covers that pixel, and the hit point, normal and texCoord are the interpolated varyings. The
//! values are the GPU's own float32 results, so the CPU ray cast of the viewer cannot reproduce
//! them. This draws the one triangle the ray cast found through the same pipeline and reads the
//! three outputs back; the caller keeps the ray cast's double values when no triangle covers the
//! pixel centre.

use bytemuck::{Pod, Zeroable};

/// The pointing WGSL.
pub const POINTING_WGSL: &str = include_str!("pointing.wgsl");

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    projection: [f32; 16],
    model_view: [f32; 16],
    instance: [f32; 16],
    instance_normal: [[f32; 4]; 3],
    id: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct VertexData {
    vertex: [f32; 4],
    normal: [f32; 3],
    tex_coord: [f32; 4],
}

/// One shape of the pointing pass: its candidate triangles and the matrices it is drawn with.
pub struct PointingTriangle {
    /// x3d_Vertex of the corners (object space, w included), three per triangle.
    pub vertices: Vec<[f32; 4]>,
    /// x3d_Normal; `(0, 0, 1)` without normals (the shader's constant).
    pub normals: Vec<[f32; 3]>,
    /// x3d_TexCoord0.
    pub tex_coords: Vec<[f32; 4]>,
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
    /// x3d_Id.
    pub id: f32,
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

pub const IDENTITY: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

pub struct PointingGpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline_a: wgpu::RenderPipeline,
    pipeline_b: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    group: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    targets: [wgpu::Texture; 3],
    views: [wgpu::TextureView; 3],
    readback: wgpu::Buffer,
    /// Samples taken (for tests).
    pub samples: usize,
}

const ROW: u64 = 256;

impl PointingGpu {
    /// A headless Vulkan device of its own (the discrete GPU), like [`crate::gpu::Gpu::new`].
    pub fn new_headless() -> Result<PointingGpu, String> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = if std::env::var("X3D_POINTING_BACKEND").as_deref() == Ok("gl") { wgpu::Backends::GL } else { wgpu::Backends::VULKAN };
        let instance = wgpu::Instance::new(desc);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        }))
        .map_err(|e| format!("no Vulkan adapter: {e}"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("x3d-pointing"), ..Default::default() })).map_err(|e| e.to_string())?;
        Ok(PointingGpu::new(device, queue))
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> PointingGpu {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("pointing"), source: wgpu::ShaderSource::Wgsl(POINTING_WGSL.into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pointing"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("pointing"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let attributes = [
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0, shader_location: 0 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 16, shader_location: 1 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 28, shader_location: 2 },
        ];
        let buffers = [Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<VertexData>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attributes })];
        let rgba32 = |_: ()| Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba32Float, blend: None, write_mask: wgpu::ColorWrites::ALL });
        let make = |entry: &'static str, targets: &[Option<wgpu::ColorTargetState>]| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &buffers },
                // gl.disable (gl .CULL_FACE)
                primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some(entry), compilation_options: Default::default(), targets }),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipeline_a = make("fs_a", &[rgba32(()), rgba32(())]);
        let pipeline_b = make("fs_b", &[rgba32(())]);
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor { label: Some("pointing uniforms"), size: std::mem::size_of::<Uniforms>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("pointing"), layout: &layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() }] });
        let vertices = device.create_buffer(&wgpu::BufferDescriptor { label: Some("pointing vertices"), size: 3 * std::mem::size_of::<VertexData>() as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let target = |i: u32| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("pointing buffer {i}")),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };
        let targets = [target(0), target(1), target(2)];
        let view = |i: usize| targets[i].create_view(&wgpu::TextureViewDescriptor::default());
        let views = [view(0), view(1), view(2)];
        let readback = device.create_buffer(&wgpu::BufferDescriptor { label: Some("pointing readback"), size: 3 * ROW, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });

        PointingGpu { device, queue, pipeline_a, pipeline_b, uniforms, group, vertices, targets, views, readback, samples: 0 }
    }

    /// Draw `tri` into the cleared 1x1 buffer and read it back. `None` when no fragment was
    /// written (the triangle does not cover the pixel centre).
    pub fn sample(&mut self, tri: &PointingTriangle) -> Option<PointingSample> {
        self.samples += 1;
        let data: Vec<VertexData> = (0..tri.vertices.len()).map(|i| VertexData { vertex: tri.vertices[i], normal: tri.normals[i], tex_coord: tri.tex_coords[i] }).collect();
        let bytes = (data.len() * std::mem::size_of::<VertexData>()) as u64;

        if self.vertices.size() < bytes {
            self.vertices = self.device.create_buffer(&wgpu::BufferDescriptor { label: Some("pointing vertices"), size: bytes.next_power_of_two(), usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        }

        self.queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(&data));
        self.queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&Uniforms { projection: tri.projection, model_view: tri.model_view, instance: tri.instance, instance_normal: [[tri.instance_normal[0], tri.instance_normal[1], tri.instance_normal[2], 0.0], [tri.instance_normal[3], tri.instance_normal[4], tri.instance_normal[5], 0.0], [tri.instance_normal[6], tri.instance_normal[7], tri.instance_normal[8], 0.0]], id: [tri.id, 0.0, 0.0, 0.0] }));

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("pointing") });
        let [x, y, w, h] = tri.viewport.map(|v| v as f32);
        let clear = |view| Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } });

        for (pipeline, views) in [(&self.pipeline_a, &self.views[0..2]), (&self.pipeline_b, &self.views[2..3])] {
            let attachments: Vec<_> = views.iter().map(clear).collect();
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor { label: Some("pointing"), color_attachments: &attachments, ..Default::default() });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.group, &[]);
            pass.set_vertex_buffer(0, self.vertices.slice(..bytes));
            pass.set_viewport(x, y, w, h, 0.0, 1.0);
            pass.draw(0..data.len() as u32, 0..1);
        }

        for (i, texture) in self.targets.iter().enumerate() {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                wgpu::TexelCopyBufferInfo { buffer: &self.readback, layout: wgpu::TexelCopyBufferLayout { offset: i as u64 * ROW, bytes_per_row: Some(ROW as u32), rows_per_image: Some(1) } },
                wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            );
        }

        self.queue.submit([encoder.finish()]);
        let slice = self.readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let mapped = slice.get_mapped_range().expect("map pointing readback");
        let f = |i: usize| -> [f32; 4] { std::array::from_fn(|k| f32::from_le_bytes(mapped[i * ROW as usize + k * 4..i * ROW as usize + k * 4 + 4].try_into().unwrap())) };
        let (a, b, c) = (f(0), f(1), f(2));
        drop(mapped);
        self.readback.unmap();

        (a[3] != 0.0).then_some(PointingSample { id: a[3], point: [a[0], a[1], a[2]], normal: [b[0], b[1], b[2]], tex_coord: c })
    }
}
