//! The native UI's layer over the world: the software canvas of [`crate::ui`] as a texture, drawn as one
//! quad with premultiplied blending after the frame.

use crate::ui::Overlay;
use std::collections::HashMap;

const SHADER: &str = "
struct Rect { v: vec4<f32> };
@group(0) @binding(0) var<uniform> rect: Rect;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
struct Out { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs_main(@builtin(vertex_index) i: u32) -> Out {
    var c = array<vec2<f32>, 6>(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0), vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0));
    let p = c[i];
    var o: Out;
    o.pos = vec4(mix(rect.v.x, rect.v.z, p.x), mix(rect.v.y, rect.v.w, p.y), 0.0, 1.0);
    o.uv = p;
    return o;
}
@fragment fn fs_main(in: Out) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, in.uv);
}
";

struct Layer {
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    /// x, y, width, height in pixels.
    rect: [u32; 4],
    srgb: bool,
}

#[derive(Default)]
pub struct OverlayGpu {
    layer: Option<Layer>,
    pipelines: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
    layout: Option<wgpu::BindGroupLayout>,
    sampler: Option<wgpu::Sampler>,
}

impl OverlayGpu {
    fn layout(&mut self, device: &wgpu::Device) -> wgpu::BindGroupLayout {
        self.layout
            .get_or_insert_with(|| {
                let entry = |binding, visibility, ty| wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility,
                    ty,
                    count: None,
                };

                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("ui overlay"),
                    entries: &[
                        entry(
                            0,
                            wgpu::ShaderStages::VERTEX,
                            wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                        ),
                        entry(
                            1,
                            wgpu::ShaderStages::FRAGMENT,
                            wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                        ),
                        entry(
                            2,
                            wgpu::ShaderStages::FRAGMENT,
                            wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        ),
                    ],
                })
            })
            .clone()
    }

    /// A new overlay (an empty canvas removes the layer). `srgb`: the target is an sRGB format.
    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, o: &Overlay, srgb: bool) {
        if o.canvas.is_empty() {
            self.layer = None;
            return;
        }

        let (w, h) = (o.canvas.width, o.canvas.height);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ui overlay"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: if srgb {
                wgpu::TextureFormat::Rgba8UnormSrgb
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            },
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &o.canvas.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui overlay rect"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = self
            .sampler
            .get_or_insert_with(|| {
                device.create_sampler(&wgpu::SamplerDescriptor {
                    mag_filter: wgpu::FilterMode::Nearest,
                    min_filter: wgpu::FilterMode::Nearest,
                    ..Default::default()
                })
            })
            .clone();
        let layout = self.layout(device);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui overlay"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &texture.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        self.layer = Some(Layer {
            bind_group,
            uniform,
            rect: [o.x, o.y, w, h],
            srgb,
        });
    }

    pub fn is_visible(&self) -> bool {
        self.layer.is_some()
    }

    /// Draw the layer over `target` (`width` × `height` pixels).
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) {
        let Some(layer) = &self.layer else { return };

        // The layer's rectangle in clip space: x from the left, y from the top.
        let [x, y, w, h] = layer.rect;
        let ndc = |v: u32, size: u32| v as f32 / size as f32 * 2.0 - 1.0;
        let rect = [
            ndc(x, width),
            -ndc(y, height),
            ndc(x + w, width),
            -ndc(y + h, height),
        ];

        queue.write_buffer(&layer.uniform, 0, bytemuck_cast(&rect));

        if !self.pipelines.contains_key(&format) {
            let layout = self.layout(device);
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("ui overlay"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("ui overlay"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("ui overlay"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::One,
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

            self.pipelines.insert(format, pipeline);
        }

        let Some(layer) = &self.layer else { return };
        let mut encoder = device.create_command_encoder(&Default::default());

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui overlay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });

            pass.set_pipeline(&self.pipelines[&format]);
            pass.set_bind_group(0, &layer.bind_group, &[]);
            pass.draw(0..6, 0..1);
        }

        queue.submit(Some(encoder.finish()));
    }

    /// Whether the layer's texture was made for an sRGB target.
    pub fn srgb(&self) -> Option<bool> {
        self.layer.as_ref().map(|l| l.srgb)
    }
}

fn bytemuck_cast(v: &[f32; 4]) -> &[u8] {
    // SAFETY: four f32 are 16 initialized bytes.
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, 16) }
}
