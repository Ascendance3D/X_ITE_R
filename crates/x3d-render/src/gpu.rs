//! Offscreen wgpu backend: X3DRenderObject.draw / drawShapes, X3DGeometryNode.display /
//! displayGeometry, X3DPointGeometryNode / X3DLineGeometryNode.display and
//! X3DBackgroundNode.display, against an RGBA8 colour + 24-bit depth target like
//! MultiSampleFramebuffer with Multisampling 0.

use crate::texture::{TextureImage, mip_chain};
use crate::traverse::{DrawItem, Frame, MAX_LIGHTS, Shading};
use std::collections::HashMap;
use std::rc::Rc;
use x3d_fields::math::{Matrix3, Matrix4};

const FLOATS_PER_VERTEX: usize = 14;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Topology {
    Triangles,
    Lines,
    Points,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Cull {
    None,
    Front,
    Back,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct PipelineKey {
    topology: Topology,
    cull: Cull,
    ccw: bool,
    blend: bool,
    depth_write: bool,
    depth_test: bool,
}

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_info: wgpu::AdapterInfo,
    shader: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    frame_layout: wgpu::BindGroupLayout,
    draw_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LightU {
    color_type: [f32; 4],
    direction_intensity: [f32; 4],
    location_ambient: [f32; 4],
    attenuation_radius: [f32; 4],
    spot: [f32; 4],
    m0: [f32; 4],
    m1: [f32; 4],
    m2: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameU {
    projection: [f32; 16],
    num_lights: [u32; 4],
    lights: [LightU; MAX_LIGHTS],
    /// x3d_Fog: color rgb + type, visibilityStart / visibilityRange, matrix columns.
    fog_color_type: [f32; 4],
    fog_range: [f32; 4],
    fog_m0: [f32; 4],
    fog_m1: [f32; 4],
    fog_m2: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawU {
    model_view: [f32; 16],
    texture_matrix: [f32; 16],
    n0: [f32; 4],
    n1: [f32; 4],
    n2: [f32; 4],
    diffuse_ambient: [f32; 4],
    specular_shininess: [f32; 4],
    emissive_transparency: [f32; 4],
    flags: [u32; 4],
    flags2: [u32; 4],
}

const DRAW_STRIDE: u64 = 256;

impl Gpu {
    /// Vulkan adapter, high performance (the discrete GPU), no surface.
    pub fn new() -> Result<Gpu, String> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::VULKAN;
        let instance = wgpu::Instance::new(desc);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        }))
        .map_err(|e| format!("no Vulkan adapter: {e}"))?;
        let adapter_info = adapter.get_info();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("x3d-render"), ..Default::default() })).map_err(|e| e.to_string())?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("x3d.wgsl"), source: wgpu::ShaderSource::Wgsl(include_str!("x3d.wgsl").into()) });

        let uniform_entry = |dynamic: bool, size: u64| wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: dynamic, min_binding_size: wgpu::BufferSize::new(size) },
            count: None,
        };

        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame"),
            entries: &[uniform_entry(false, std::mem::size_of::<FrameU>() as u64)],
        });
        let draw_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("draw"),
            entries: &[uniform_entry(true, std::mem::size_of::<DrawU>() as u64)],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("x3d"),
            bind_group_layouts: &[Some(&frame_layout), Some(&draw_layout), Some(&texture_layout)],
            ..Default::default()
        });

        Ok(Gpu { device, queue, adapter_info, shader, layout, frame_layout, draw_layout, texture_layout, pipelines: HashMap::new() })
    }

    fn pipeline(&mut self, key: PipelineKey) -> &wgpu::RenderPipeline {
        if !self.pipelines.contains_key(&key) {
            let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4, 3 => Float32x4];
            let blend = key.blend.then_some(wgpu::BlendState {
                // gl.blendFuncSeparate (SRC_ALPHA, ONE_MINUS_SRC_ALPHA, ONE, ONE_MINUS_SRC_ALPHA)
                color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
                alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
            });
            let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("x3d"),
                layout: Some(&self.layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout { array_stride: (FLOATS_PER_VERTEX * 4) as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attributes })],
                },
                primitive: wgpu::PrimitiveState {
                    topology: match key.topology {
                        Topology::Triangles => wgpu::PrimitiveTopology::TriangleList,
                        Topology::Lines => wgpu::PrimitiveTopology::LineList,
                        Topology::Points => wgpu::PrimitiveTopology::PointList,
                    },
                    front_face: if key.ccw { wgpu::FrontFace::Ccw } else { wgpu::FrontFace::Cw },
                    cull_mode: match key.cull {
                        Cull::None => None,
                        Cull::Front => Some(wgpu::Face::Front),
                        Cull::Back => Some(wgpu::Face::Back),
                    },
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth24Plus,
                    depth_write_enabled: Some(key.depth_write),
                    depth_compare: Some(if key.depth_test { wgpu::CompareFunction::LessEqual } else { wgpu::CompareFunction::Always }),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba8Unorm, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            });
            self.pipelines.insert(key, pipeline);
        }

        &self.pipelines[&key]
    }

    fn upload_texture(&self, t: &TextureImage) -> wgpu::BindGroup {
        let levels = if t.no_mipmaps { vec![(t.width, t.height, t.rgba.clone())] } else { mip_chain(t) };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("image"),
            size: wgpu::Extent3d { width: t.width, height: t.height, depth_or_array_layers: 1 },
            mip_level_count: levels.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        for (level, (w, h, data)) in levels.iter().enumerate() {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: level as u32, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                data,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(*h) },
                wgpu::Extent3d { width: *w, height: *h, depth_or_array_layers: 1 },
            );
        }

        let address = |repeat: bool| if repeat { wgpu::AddressMode::Repeat } else { wgpu::AddressMode::ClampToEdge };
        // TextureQuality MEDIUM default TextureProperties: mag LINEAR, min LINEAR_MIPMAP_NEAREST
        // (AVG_PIXEL_NEAREST_MIPMAP); below 16 px: NEAREST without mipmaps.
        let (mag, min) = if t.nearest { (wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest) } else { (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear) };
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("image"),
            address_mode_u: address(t.repeat_s),
            address_mode_v: address(t.repeat_t),
            mag_filter: mag,
            min_filter: min,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let view = texture.create_view(&Default::default());
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture"),
            layout: &self.texture_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) }],
        })
    }

    /// Render a frame; returns tightly packed RGBA8 rows, top row first.
    pub fn render(&mut self, frame: &Frame, width: u32, height: u32) -> Vec<u8> {
        // --- frame uniforms
        let mut fu: FrameU = bytemuck::Zeroable::zeroed();
        fu.projection = f32x16(&frame.projection);
        let n = frame.lights.len().min(MAX_LIGHTS);
        fu.num_lights = [n as u32, 0, 0, 0];
        for (i, l) in frame.lights.iter().take(MAX_LIGHTS).enumerate() {
            let m = l.matrix;
            fu.lights[i] = LightU {
                color_type: [l.color[0] as f32, l.color[1] as f32, l.color[2] as f32, l.kind as f32],
                direction_intensity: [l.direction.x as f32, l.direction.y as f32, l.direction.z as f32, l.intensity as f32],
                location_ambient: [l.location.x as f32, l.location.y as f32, l.location.z as f32, l.ambient_intensity as f32],
                attenuation_radius: [l.attenuation[0] as f32, l.attenuation[1] as f32, l.attenuation[2] as f32, l.radius as f32],
                spot: [l.cut_off_angle as f32, l.beam_width as f32, 0.0, 0.0],
                m0: [m[0], m[1], m[2], 0.0],
                m1: [m[3], m[4], m[5], 0.0],
                m2: [m[6], m[7], m[8], 0.0],
            };
        }

        if let Some(f) = &frame.fog {
            fu.fog_color_type = [f.color[0], f.color[1], f.color[2], f.fog_type as f32];
            fu.fog_range = [f.visibility_start, f.visibility_range, 0.0, 0.0];
            fu.fog_m0 = [f.matrix[0], f.matrix[1], f.matrix[2], 0.0];
            fu.fog_m1 = [f.matrix[3], f.matrix[4], f.matrix[5], 0.0];
            fu.fog_m2 = [f.matrix[6], f.matrix[7], f.matrix[8], 0.0];
        }

        let device = &self.device;
        use wgpu::util::DeviceExt;
        let frame_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("frame"), contents: bytemuck::bytes_of(&fu), usage: wgpu::BufferUsages::UNIFORM });
        let frame_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame"),
            layout: &self.frame_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: frame_buffer.as_entire_binding() }],
        });

        // --- draw list in X3DRenderObject.drawShapes order
        struct Call {
            key: PipelineKey,
            vertex_range: (u64, u64),
            count: u32,
            draw_index: usize,
            texture: Option<*const TextureImage>,
        }

        let mut vertex_data: Vec<f32> = vec![];
        let mut draws: Vec<DrawU> = vec![];
        let mut calls: Vec<Call> = vec![];
        let mut geometry_ranges: HashMap<*const crate::geometry::Geometry, (u64, u64, u32)> = HashMap::new();
        let mut textures: HashMap<*const TextureImage, Rc<TextureImage>> = HashMap::new();

        if let Some(bg) = &frame.background {
            let start = vertex_data.len() as u64 * 4;
            let count = bg.sphere.len() / 4;
            for i in 0..count {
                let p = &bg.sphere[i * 4..i * 4 + 3];
                let c = &bg.colors[i * 4..i * 4 + 4];
                vertex_data.extend_from_slice(&[p[0], p[1], p[2], 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, c[0], c[1], c[2], c[3]]);
            }
            let end = vertex_data.len() as u64 * 4;
            let mut d: DrawU = bytemuck::Zeroable::zeroed();
            d.model_view = f32x16(&bg.model_view);
            d.texture_matrix = f32x16(&Matrix4::IDENTITY);
            d.emissive_transparency = [1.0, 1.0, 1.0, bg.transparency as f32];
            d.flags = [0, 0, 1, 0];
            d.flags2 = [0, (bg.transparency == 0.0) as u32, 3, 1];
            draws.push(d);
            if count > 0 {
                calls.push(Call {
                    key: PipelineKey { topology: Topology::Triangles, cull: Cull::Back, ccw: true, blend: bg.transparency > 0.0, depth_write: false, depth_test: false },
                    vertex_range: (start, end),
                    count: count as u32,
                    draw_index: draws.len() - 1,
                    texture: None,
                });
            }

            // X3DBackgroundNode.drawCube: every loaded face, unlit white × texture, blended
            // when the image has alpha, same cull/depth state as the sphere.
            for face in &bg.faces {
                let start = vertex_data.len() as u64 * 4;

                for i in 0..6 {
                    let p = &face.vertices[i * 4..i * 4 + 3];
                    let t = &crate::traverse::FACE_TEX_COORDS[i * 4..i * 4 + 4];
                    vertex_data.extend_from_slice(&[p[0], p[1], p[2], 0.0, 0.0, 0.0, t[0], t[1], t[2], t[3], 1.0, 1.0, 1.0, 1.0]);
                }

                let end = vertex_data.len() as u64 * 4;
                let tp = Rc::as_ptr(&face.texture);

                textures.entry(tp).or_insert_with(|| face.texture.clone());

                let mut d: DrawU = bytemuck::Zeroable::zeroed();
                d.model_view = f32x16(&bg.model_view);
                d.texture_matrix = f32x16(&Matrix4::IDENTITY);
                d.emissive_transparency = [1.0, 1.0, 1.0, 0.0];
                d.flags = [0, 0, 0, 1];
                d.flags2 = [face.texture.flip_y as u32, (!face.texture.transparent) as u32, 3, 1];
                draws.push(d);
                calls.push(Call {
                    key: PipelineKey { topology: Topology::Triangles, cull: Cull::Back, ccw: true, blend: face.texture.transparent, depth_write: false, depth_test: false },
                    vertex_range: (start, end),
                    count: 6,
                    draw_index: draws.len() - 1,
                    texture: Some(tp),
                });
            }
        }

        let mut order: Vec<&DrawItem> = frame.items.iter().filter(|i| !i.transparent).collect();
        let mut transparent: Vec<&DrawItem> = frame.items.iter().filter(|i| i.transparent).collect();
        // MergeSort (a .distance < b .distance): stable, ascending view-space z.
        transparent.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
        let n_opaque = order.len();
        order.extend(transparent);

        let num_lights = frame.lights.len();

        for (k, item) in order.iter().enumerate() {
            let is_transparent = k >= n_opaque;
            let g = &item.geometry;
            let gp = Rc::as_ptr(g);
            let (start, end, count) = *geometry_ranges.entry(gp).or_insert_with(|| {
                let start = vertex_data.len() as u64 * 4;
                let count = g.vertex_count();
                for i in 0..count {
                    let v = &g.vertices[i * 4..i * 4 + 3];
                    let nrm = if g.normals.len() >= (i + 1) * 3 { [g.normals[i * 3], g.normals[i * 3 + 1], g.normals[i * 3 + 2]] } else { [0.0; 3] };
                    let t = if g.tex_coords.len() >= (i + 1) * 4 { [g.tex_coords[i * 4], g.tex_coords[i * 4 + 1], g.tex_coords[i * 4 + 2], g.tex_coords[i * 4 + 3]] } else { [0.0, 0.0, 0.0, 1.0] };
                    let c = if g.colors.len() >= (i + 1) * 4 { [g.colors[i * 4], g.colors[i * 4 + 1], g.colors[i * 4 + 2], g.colors[i * 4 + 3]] } else { [1.0; 4] };
                    vertex_data.extend_from_slice(&[v[0], v[1], v[2], nrm[0], nrm[1], nrm[2], t[0], t[1], t[2], t[3], c[0], c[1], c[2], c[3]]);
                }
                (start, vertex_data.len() as u64 * 4, count as u32)
            });

            let mv = f32x16(&item.model_view);
            let normal = normal_matrix(&mv);
            let m = &item.material;
            let mut d: DrawU = bytemuck::Zeroable::zeroed();
            d.model_view = mv;
            d.texture_matrix = item.texture_matrix;
            d.n0 = [normal[0], normal[1], normal[2], 0.0];
            d.n1 = [normal[3], normal[4], normal[5], 0.0];
            d.n2 = [normal[6], normal[7], normal[8], 0.0];
            d.diffuse_ambient = [m.diffuse[0] as f32, m.diffuse[1] as f32, m.diffuse[2] as f32, m.ambient_intensity as f32];
            d.specular_shininess = [m.specular[0] as f32, m.specular[1] as f32, m.specular[2] as f32, m.shininess as f32];
            d.emissive_transparency = [m.emissive[0] as f32, m.emissive[1] as f32, m.emissive[2] as f32, m.transparency as f32];
            let gouraud = m.shading == Shading::Gouraud;
            d.flags = [gouraud as u32, (gouraud && num_lights > 0) as u32, (!g.colors.is_empty()) as u32, item.texture.is_some() as u32];
            d.flags2 = [item.texture.as_ref().is_some_and(|t| t.flip_y) as u32, (!is_transparent) as u32, g.geometry_type as u32, 0];
            draws.push(d);
            let draw_index = draws.len() - 1;

            let tex = item.texture.as_ref().map(|t| {
                let p = Rc::as_ptr(t);
                textures.entry(p).or_insert_with(|| t.clone());
                p
            });

            let topology = match g.geometry_type {
                0 => Topology::Points,
                1 => Topology::Lines,
                _ => Topology::Triangles,
            };

            // X3DGeometryNode.display: frontFace from ccw, swapped for a negative determinant.
            let positive_scale = determinant3(&mv) >= 0.0;
            let ccw = if positive_scale { g.ccw } else { !g.ccw };
            let base = PipelineKey { topology, cull: Cull::None, ccw, blend: is_transparent, depth_write: !is_transparent, depth_test: true };

            if topology != Topology::Triangles {
                calls.push(Call { key: base, vertex_range: (start, end), count, draw_index, texture: tex });
            } else if is_transparent {
                if !g.solid {
                    calls.push(Call { key: PipelineKey { cull: Cull::Front, ..base }, vertex_range: (start, end), count, draw_index, texture: tex });
                }
                calls.push(Call { key: PipelineKey { cull: Cull::Back, ..base }, vertex_range: (start, end), count, draw_index, texture: tex });
            } else {
                let cull = if g.solid { Cull::Back } else { Cull::None };
                calls.push(Call { key: PipelineKey { cull, ..base }, vertex_range: (start, end), count, draw_index, texture: tex });
            }
        }

        // --- GPU resources
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertices"),
            contents: if vertex_data.is_empty() { &[0u8; 64] } else { bytemuck::cast_slice(&vertex_data) },
            usage: wgpu::BufferUsages::VERTEX,
        });

        let mut draw_bytes = vec![0u8; draws.len().max(1) * DRAW_STRIDE as usize];
        for (i, d) in draws.iter().enumerate() {
            let b = bytemuck::bytes_of(d);
            draw_bytes[i * DRAW_STRIDE as usize..i * DRAW_STRIDE as usize + b.len()].copy_from_slice(b);
        }
        let draw_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("draws"), contents: &draw_bytes, usage: wgpu::BufferUsages::UNIFORM });
        let draw_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw"),
            layout: &self.draw_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &draw_buffer, offset: 0, size: wgpu::BufferSize::new(std::mem::size_of::<DrawU>() as u64) }),
            }],
        });

        let white = Rc::new(TextureImage::white());
        let white_group = self.upload_texture(&white);
        let texture_groups: HashMap<*const TextureImage, wgpu::BindGroup> = textures.iter().map(|(p, t)| (*p, self.upload_texture(t))).collect();

        for c in &calls {
            self.pipeline(c.key);
        }

        let device = &self.device;
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("color"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let target_view = target.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());

        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("x3d"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    depth_slice: None,
                    resolve_target: None,
                    // drawShapes: gl.clearColor (0, 0, 0, 0)
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            pass.set_bind_group(0, &frame_group, &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));

            for c in &calls {
                pass.set_pipeline(&self.pipelines[&c.key]);
                pass.set_bind_group(1, &draw_group, &[(c.draw_index as u64 * DRAW_STRIDE) as u32]);
                let tg = c.texture.and_then(|p| texture_groups.get(&p)).unwrap_or(&white_group);
                pass.set_bind_group(2, tg, &[]);
                pass.set_vertex_buffer(0, vertex_buffer.slice(c.vertex_range.0..c.vertex_range.1));
                pass.draw(0..c.count, 0..1);
            }
        }

        let bytes_per_row = (width * 4).div_ceil(256) * 256;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &target, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bytes_per_row), rows_per_image: Some(height) } },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);

        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let data = slice.get_mapped_range().expect("map readback");
        let mut out = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            let row = &data[(y * bytes_per_row) as usize..(y * bytes_per_row + width * 4) as usize];
            out.extend_from_slice(row);
        }
        drop(data);
        readback.unmap();
        out
    }
}

fn f32x16(m: &Matrix4) -> [f32; 16] {
    m.0.map(|v| v as f32)
}

/// Matrix4.determinant3 of the (Float32) model-view matrix.
fn determinant3(m: &[f32; 16]) -> f64 {
    let m: Vec<f64> = m.iter().map(|v| *v as f64).collect();
    m[0] * (m[5] * m[10] - m[6] * m[9]) - m[1] * (m[4] * m[10] - m[6] * m[8]) + m[2] * (m[4] * m[9] - m[5] * m[8])
}

/// X3DProgrammableShaderObject.setUniforms: transpose the 3×3 of the Float32 model-view matrix,
/// invert it in doubles, upload as Float32 (column-major).
fn normal_matrix(mv: &[f32; 16]) -> [f32; 9] {
    let a: Vec<f64> = mv.iter().map(|v| *v as f64).collect();
    let mut n = Matrix3([0.0; 9]);
    n.0[0] = a[0];
    n.0[3] = a[1];
    n.0[6] = a[2];
    n.0[1] = a[4];
    n.0[4] = a[5];
    n.0[7] = a[6];
    n.0[2] = a[8];
    n.0[5] = a[9];
    n.0[8] = a[10];
    n.inverse();
    n.0.map(|v| v as f32)
}
