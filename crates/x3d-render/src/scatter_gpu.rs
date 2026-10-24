//! The GPU side of the VolumeScatterMaterialExtension: X3DX_ITEContext.getVolumeScatterBuffer, a
//! TextureBuffer the size of the target with an RGBA32F color texture and a depth texture. The scatter
//! pass (x3d.wgsl get_scatter_pass_color) draws the shapes with the extension into it; the main
//! pass reads both back per fragment (get_subsurface_scattering). The main pass's fragment stage has no
//! sampled texture binding left, so the color is copied into a storage buffer and the depth is
//! converted into another one by a small compute pass. Rows are padded to `stride` pixels.

/// The buffers and textures of one target size, kept while frames use them (see `Renderer`).
pub struct ScatterGpu {
    pub width: u32,
    pub height: u32,
    /// Pixels per row in the buffers (COPY_BYTES_PER_ROW_ALIGNMENT of 16 byte pixels).
    pub stride: u32,
    pub color: wgpu::Texture,
    pub color_view: wgpu::TextureView,
    pub color_buffer: wgpu::Buffer,
    pub depth_buffer: wgpu::Buffer,
    params: wgpu::Buffer,
    pub used: u64,
}

/// The compute pipeline that copies the window depth of a depth texture into a storage buffer.
pub struct ScatterDepthCopy {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
}

const DEPTH_WGSL: &str = r#"
struct Params {
    size: vec4<u32>, // width, height, stride
};

@group(0) @binding(0) var depth: texture_depth_2d;
@group(0) @binding(1) var<storage, read_write> out: array<f32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.size.x || id.y >= params.size.y) {
        return;
    }

    out[id.y * params.size.z + id.x] = textureLoad(depth, vec2<i32>(id.xy), 0);
}
"#;

impl ScatterDepthCopy {
    pub fn new(device: &wgpu::Device) -> ScatterDepthCopy {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scatter depth"),
            source: wgpu::ShaderSource::Wgsl(DEPTH_WGSL.into()),
        });
        let entry = |binding: u32, ty: wgpu::BindingType| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty,
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scatter depth"),
            entries: &[
                entry(
                    0,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(
                    1,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                entry(
                    2,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scatter depth"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("scatter depth"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        ScatterDepthCopy { layout, pipeline }
    }
}

impl ScatterGpu {
    pub fn new(device: &wgpu::Device, width: u32, height: u32, frame_no: u64) -> ScatterGpu {
        let stride = (width * 16).div_ceil(256) * 256 / 16;
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("volume scatter buffer"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let color_view = color.create_view(&Default::default());
        let color_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("volume scatter colors"),
            size: stride as u64 * height as u64 * 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let depth_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("volume scatter depth"),
            size: stride as u64 * height as u64 * 4,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("volume scatter params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ScatterGpu {
            width,
            height,
            stride,
            color,
            color_view,
            color_buffer,
            depth_buffer,
            params,
            used: frame_no,
        }
    }

    /// Copies the color texture and the depth of `depth` (the pass's depth texture) into the buffers.
    pub fn read_back(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        copy: &ScatterDepthCopy,
        depth: &wgpu::Texture,
    ) {
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.color_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.stride * 16),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        queue.write_buffer(
            &self.params,
            0,
            bytemuck::cast_slice(&[self.width, self.height, self.stride, 0u32]),
        );
        crate::transfer::upload(crate::transfer::Kind::Other, 16);
        let view = depth.create_view(&wgpu::TextureViewDescriptor {
            aspect: wgpu::TextureAspect::DepthOnly,
            ..Default::default()
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scatter depth"),
            layout: &copy.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.depth_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("scatter depth"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&copy.pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
    }
}
