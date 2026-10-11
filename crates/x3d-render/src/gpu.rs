//! wgpu backend: X3DRenderObject.draw / drawShapes, X3DGeometryNode.display /
//! displayGeometry, X3DPointGeometryNode / X3DLineGeometryNode.display and
//! X3DBackgroundNode.display, against a colour target + 24-bit depth like
//! MultiSampleFramebuffer with Multisampling 0.
//!
//! Two entry points share one implementation:
//!
//! * [`Renderer`] draws into a texture view the host owns (a window surface texture, an
//!   offscreen texture) on the host's `wgpu::Device` / `wgpu::Queue`. It keeps every GPU
//!   resource between frames: pipelines, uniform buffers and bind groups, one vertex buffer per
//!   geometry (uploaded when the geometry is first seen, replaced when the viewer hands out a
//!   rebuilt one), one bind group per texture, the depth texture.
//! * [`Gpu`] is the headless wrapper used by `x3d-shot` and the oracle tools: its own Vulkan
//!   device, an RGBA8 target and a CPU readback.
//!
//! The shaders write display-encoded values (X_ITE's WebGL framebuffer is not sRGB), so the
//! target view must have a non-sRGB format (`Bgra8Unorm`, `Rgba8Unorm`).

use crate::texture::{TextureImage, mip_chain};
use crate::traverse::{DrawItem, Frame, MAX_LIGHTS, Shading};
use std::collections::HashMap;
use std::rc::Rc;

use wgpu::util::DeviceExt;
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

/// Whether `format` can be the colour target: the shaders write display-encoded values, so it
/// must be a non-sRGB format with 8, 10 or 16/32 bit float channels.
pub fn target_format_ok(format: wgpu::TextureFormat) -> bool {
    matches!(
        format,
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgb10a2Unorm | wgpu::TextureFormat::Rgba16Float | wgpu::TextureFormat::Rgba32Float
    )
}

struct GeoGpu {
    /// Keeps the allocation (and so the address used as the key) alive while cached.
    _geometry: Rc<crate::geometry::Geometry>,
    buffer: wgpu::Buffer,
    count: u32,
    used: u64,
}

struct TexGpu {
    _image: Rc<TextureImage>,
    group: wgpu::BindGroup,
    used: u64,
}

/// Where a draw call's vertices live.
#[derive(Clone, Copy)]
enum Source {
    Scratch,
    Geometry(*const crate::geometry::Geometry),
}

struct Call {
    key: PipelineKey,
    source: Source,
    /// Byte range inside the scratch buffer (Source::Scratch only).
    range: (u64, u64),
    count: u32,
    draw_index: usize,
    texture: Option<*const TextureImage>,
}

/// GPU time of the render pass, from timestamp queries (needs `Features::TIMESTAMP_QUERY` on
/// the host's device).
struct GpuTimer {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    period_ns: f32,
    /// The last frame's readback is mapped or being mapped.
    pending: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    last_ms: Option<f64>,
    recorded: bool,
}

/// Cache entries unused for this many frames are dropped.
const KEEP_FRAMES: u64 = 120;

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    shader: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    draw_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    pipelines: HashMap<(PipelineKey, wgpu::TextureFormat), wgpu::RenderPipeline>,
    frame_buffer: wgpu::Buffer,
    frame_group: wgpu::BindGroup,
    draw_buffer: wgpu::Buffer,
    draw_capacity: usize,
    draw_group: wgpu::BindGroup,
    scratch: wgpu::Buffer,
    scratch_capacity: u64,
    /// The scratch vertex data of the last frame (background); unchanged data is not re-uploaded.
    scratch_data: Vec<f32>,
    geometries: HashMap<*const crate::geometry::Geometry, GeoGpu>,
    textures: HashMap<*const TextureImage, TexGpu>,
    white_group: wgpu::BindGroup,
    depth: Option<(wgpu::Texture, wgpu::TextureView, u32, u32)>,
    /// RGBA8 target and readback buffer of the offscreen paths (`render_to_rgba`).
    offscreen: Option<(wgpu::Texture, wgpu::TextureView, u32, u32)>,
    readback: Option<(wgpu::Buffer, u32, u32)>,
    frame_no: u64,
    timer: Option<GpuTimer>,
    /// Colour the target is cleared to; X_ITE clears to (0, 0, 0, 0) (drawShapes: gl.clearColor).
    pub clear_color: [f64; 4],
    /// Reused per-frame scratch memory.
    draw_bytes: Vec<u8>,
    calls: Vec<Call>,
    /// Frame statistics for the last `encode`: draw calls, geometry uploads, texture uploads.
    pub stats: RenderStats,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct RenderStats {
    pub draw_calls: usize,
    pub geometry_uploads: usize,
    pub texture_uploads: usize,
    pub pipelines_created: usize,
}

impl Renderer {
    /// The device features this renderer can use if the adapter offers them (GPU timing).
    pub fn wanted_features(adapter: &wgpu::Adapter) -> wgpu::Features {
        adapter.features() & wgpu::Features::TIMESTAMP_QUERY
    }

    /// A renderer on the host's device and queue. Cheap resources only; nothing is drawn.
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Renderer {
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

        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame"),
            size: std::mem::size_of::<FrameU>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: frame_buffer.as_entire_binding() }],
        });

        let draw_capacity = 64;
        let (draw_buffer, draw_group) = Self::make_draw_buffer(&device, &draw_layout, draw_capacity);
        let scratch_capacity = 64 * 1024;
        let scratch = device.create_buffer(&wgpu::BufferDescriptor { label: Some("scratch"), size: scratch_capacity, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });

        let mut r = Renderer {
            white_group: Self::make_texture_group(&device, &queue, &texture_layout, &TextureImage::white()),
            device,
            queue,
            shader,
            layout,
            draw_layout,
            texture_layout,
            pipelines: HashMap::new(),
            frame_buffer,
            frame_group,
            draw_buffer,
            draw_capacity,
            draw_group,
            scratch,
            scratch_capacity,
            scratch_data: vec![],
            geometries: HashMap::new(),
            textures: HashMap::new(),
            depth: None,
            offscreen: None,
            readback: None,
            frame_no: 0,
            timer: None,
            clear_color: [0.0; 4],
            draw_bytes: vec![],
            calls: vec![],
            stats: RenderStats::default(),
        };
        r.timer = GpuTimer::new(&r.device, &r.queue);
        r
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Whether GPU timing is available (the device was created with `TIMESTAMP_QUERY`).
    pub fn gpu_timing_available(&self) -> bool {
        self.timer.is_some()
    }

    /// GPU milliseconds of the most recent finished render pass. With `block`, waits for the
    /// frame just submitted; otherwise returns the latest value that is already available.
    pub fn gpu_time_ms(&mut self, block: bool) -> Option<f64> {
        let timer = self.timer.as_mut()?;

        if timer.pending.is_none() && !timer.recorded {
            return timer.last_ms;
        }

        if block {
            let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        } else {
            let _ = self.device.poll(wgpu::PollType::Poll);
        }

        if let Some(flag) = &timer.pending {
            if flag.load(std::sync::atomic::Ordering::Acquire) {
                {
                    let data = timer.read.slice(..).get_mapped_range().expect("timestamps");
                    let t: &[u64] = bytemuck::cast_slice(&data);
                    timer.last_ms = Some(t[1].saturating_sub(t[0]) as f64 * timer.period_ns as f64 / 1e6);
                }
                timer.read.unmap();
                timer.pending = None;
            }
        }

        timer.last_ms
    }

    fn make_draw_buffer(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, capacity: usize) -> (wgpu::Buffer, wgpu::BindGroup) {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("draws"),
            size: capacity as u64 * DRAW_STRIDE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &buffer, offset: 0, size: wgpu::BufferSize::new(std::mem::size_of::<DrawU>() as u64) }),
            }],
        });
        (buffer, group)
    }

    fn make_pipeline(&self, key: PipelineKey, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4, 3 => Float32x4];
        let blend = key.blend.then_some(wgpu::BlendState {
            // gl.blendFuncSeparate (SRC_ALPHA, ONE_MINUS_SRC_ALPHA, ONE, ONE_MINUS_SRC_ALPHA)
            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
        });
        self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        })
    }

    fn make_texture_group(device: &wgpu::Device, queue: &wgpu::Queue, layout: &wgpu::BindGroupLayout, t: &TextureImage) -> wgpu::BindGroup {
        let levels = if t.no_mipmaps { vec![(t.width, t.height, t.rgba.clone())] } else { mip_chain(t) };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
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
            queue.write_texture(
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
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("image"),
            address_mode_u: address(t.repeat_s),
            address_mode_v: address(t.repeat_t),
            mag_filter: mag,
            min_filter: min,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let view = texture.create_view(&Default::default());
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture"),
            layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) }],
        })
    }

    /// Draw `frame` into `target` (a `width` x `height` view with `format`, which must be a
    /// non-sRGB colour format) and submit. The target keeps what it had outside the draw; the
    /// renderer clears it to [`Renderer::clear_color`] first.
    pub fn render(&mut self, frame: &Frame, target: &wgpu::TextureView, format: wgpu::TextureFormat, width: u32, height: u32) -> Result<(), String> {
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("x3d") });
        self.encode(&mut encoder, frame, target, format, width, height)?;
        self.queue.submit([encoder.finish()]);
        self.after_submit();
        Ok(())
    }


    fn ensure_offscreen(&mut self, width: u32, height: u32) {
        if self.offscreen.as_ref().is_none_or(|t| t.2 != width || t.3 != height) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("color"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            self.offscreen = Some((texture, view, width, height));
        }
    }

    /// Render a frame into an internal RGBA8 texture and read it back: tightly packed rows, top
    /// row first. The path of the screenshot tools and the oracle comparisons (no window).
    pub fn render_to_rgba(&mut self, frame: &Frame, width: u32, height: u32) -> Vec<u8> {
        self.ensure_offscreen(width, height);
        let bytes_per_row = (width * 4).div_ceil(256) * 256;

        if self.readback.as_ref().is_none_or(|r| r.1 != width || r.2 != height) {
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("readback"),
                size: (bytes_per_row * height) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            self.readback = Some((buffer, width, height));
        }

        let view = self.offscreen.as_ref().unwrap().1.clone();
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.encode(&mut encoder, frame, &view, wgpu::TextureFormat::Rgba8Unorm, width, height).expect("render");
        let (texture, readback) = (&self.offscreen.as_ref().unwrap().0, &self.readback.as_ref().unwrap().0);
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bytes_per_row), rows_per_image: Some(height) } },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);
        self.after_submit();

        let readback = &self.readback.as_ref().unwrap().0;
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let data = slice.get_mapped_range().expect("map readback");
        let mut out = Vec::with_capacity((width * height * 4) as usize);

        for y in 0..height {
            out.extend_from_slice(&data[(y * bytes_per_row) as usize..(y * bytes_per_row + width * 4) as usize]);
        }

        drop(data);
        readback.unmap();
        out
    }

    /// Render into the internal texture without a readback; waits for the GPU and returns the
    /// pass's GPU milliseconds (None without timestamp support). For benchmarks.
    pub fn render_offscreen_timed(&mut self, frame: &Frame, width: u32, height: u32) -> Option<f64> {
        self.ensure_offscreen(width, height);
        let view = self.offscreen.as_ref().unwrap().1.clone();
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.encode(&mut encoder, frame, &view, wgpu::TextureFormat::Rgba8Unorm, width, height).expect("render");
        self.queue.submit([encoder.finish()]);
        self.after_submit();
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        self.gpu_time_ms(true)
    }

    /// Call after submitting an encoder filled by [`Renderer::encode`]: starts the timestamp
    /// readback.
    pub fn after_submit(&mut self) {
        if let Some(t) = self.timer.as_mut() {
            if t.recorded {
                t.recorded = false;
                let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let f2 = flag.clone();
                t.read.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                    if r.is_ok() {
                        f2.store(true, std::sync::atomic::Ordering::Release);
                    }
                });
                t.pending = Some(flag);
            }
        }
    }

    /// Record the frame's commands into `encoder` (the caller submits, then calls
    /// [`Renderer::after_submit`]); lets a host append its own passes or a texture copy.
    pub fn encode(&mut self, encoder: &mut wgpu::CommandEncoder, frame: &Frame, target: &wgpu::TextureView, format: wgpu::TextureFormat, width: u32, height: u32) -> Result<(), String> {
        if !target_format_ok(format) {
            return Err(format!("x3d-render: the target format {format:?} is not supported; use a non-sRGB color format such as Bgra8Unorm or Rgba8Unorm"));
        }

        if width == 0 || height == 0 {
            return Err("x3d-render: empty target".into());
        }

        self.frame_no += 1;
        self.stats = RenderStats::default();
        let frame_no = self.frame_no;

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

        self.queue.write_buffer(&self.frame_buffer, 0, bytemuck::bytes_of(&fu));

        // --- draw list in X3DRenderObject.drawShapes order
        let mut scratch_data: Vec<f32> = std::mem::take(&mut self.scratch_data);
        scratch_data.clear();
        let mut draws: Vec<DrawU> = Vec::with_capacity(frame.items.len() + 8);
        let mut calls = std::mem::take(&mut self.calls);
        calls.clear();
        let mut used_textures: Vec<Rc<TextureImage>> = vec![];

        if let Some(bg) = &frame.background {
            let start = scratch_data.len() as u64 * 4;
            let count = bg.sphere.len() / 4;
            for i in 0..count {
                let p = &bg.sphere[i * 4..i * 4 + 3];
                let c = &bg.colors[i * 4..i * 4 + 4];
                scratch_data.extend_from_slice(&[p[0], p[1], p[2], 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, c[0], c[1], c[2], c[3]]);
            }
            let end = scratch_data.len() as u64 * 4;
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
                    source: Source::Scratch,
                    range: (start, end),
                    count: count as u32,
                    draw_index: draws.len() - 1,
                    texture: None,
                });
            }

            // X3DBackgroundNode.drawCube: every loaded face, unlit white x texture, blended
            // when the image has alpha, same cull/depth state as the sphere.
            for face in &bg.faces {
                let start = scratch_data.len() as u64 * 4;

                for i in 0..6 {
                    let p = &face.vertices[i * 4..i * 4 + 3];
                    let t = &crate::traverse::FACE_TEX_COORDS[i * 4..i * 4 + 4];
                    scratch_data.extend_from_slice(&[p[0], p[1], p[2], 0.0, 0.0, 0.0, t[0], t[1], t[2], t[3], 1.0, 1.0, 1.0, 1.0]);
                }

                let end = scratch_data.len() as u64 * 4;
                let tp = Rc::as_ptr(&face.texture);
                used_textures.push(face.texture.clone());

                let mut d: DrawU = bytemuck::Zeroable::zeroed();
                d.model_view = f32x16(&bg.model_view);
                d.texture_matrix = f32x16(&Matrix4::IDENTITY);
                d.emissive_transparency = [1.0, 1.0, 1.0, 0.0];
                d.flags = [0, 0, 0, 1];
                d.flags2 = [face.texture.flip_y as u32, (!face.texture.transparent) as u32, 3, 1];
                draws.push(d);
                calls.push(Call {
                    key: PipelineKey { topology: Topology::Triangles, cull: Cull::Back, ccw: true, blend: face.texture.transparent, depth_write: false, depth_test: false },
                    source: Source::Scratch,
                    range: (start, end),
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

            // One vertex buffer per geometry, kept while the viewer keeps handing out the same
            // allocation (it hands out a new one when the geometry is rebuilt).
            let count = match self.geometries.get_mut(&gp) {
                Some(e) => {
                    e.used = frame_no;
                    e.count
                }
                None => {
                    let verts = geometry_vertices(g);
                    let count = (verts.len() / FLOATS_PER_VERTEX) as u32;
                    let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("vertices"),
                        contents: if verts.is_empty() { &[0u8; 64] } else { bytemuck::cast_slice(&verts) },
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    self.geometries.insert(gp, GeoGpu { _geometry: g.clone(), buffer, count, used: frame_no });
                    self.stats.geometry_uploads += 1;
                    count
                }
            };

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
                used_textures.push(t.clone());
                Rc::as_ptr(t)
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
            let source = Source::Geometry(gp);

            if topology != Topology::Triangles {
                calls.push(Call { key: base, source, range: (0, 0), count, draw_index, texture: tex });
            } else if is_transparent {
                if !g.solid {
                    calls.push(Call { key: PipelineKey { cull: Cull::Front, ..base }, source, range: (0, 0), count, draw_index, texture: tex });
                }
                calls.push(Call { key: PipelineKey { cull: Cull::Back, ..base }, source, range: (0, 0), count, draw_index, texture: tex });
            } else {
                let cull = if g.solid { Cull::Back } else { Cull::None };
                calls.push(Call { key: PipelineKey { cull, ..base }, source, range: (0, 0), count, draw_index, texture: tex });
            }
        }

        // --- GPU resources that depend on the frame
        // Textures: one bind group each, kept while used.
        for t in used_textures {
            let p = Rc::as_ptr(&t);

            match self.textures.get_mut(&p) {
                Some(e) => e.used = frame_no,
                None => {
                    let group = Self::make_texture_group(&self.device, &self.queue, &self.texture_layout, &t);
                    self.textures.insert(p, TexGpu { _image: t, group, used: frame_no });
                    self.stats.texture_uploads += 1;
                }
            }
        }

        // Draw uniforms: one buffer, grown when a frame needs more.
        if draws.len() > self.draw_capacity {
            self.draw_capacity = draws.len().next_power_of_two();
            let (b, g) = Self::make_draw_buffer(&self.device, &self.draw_layout, self.draw_capacity);
            self.draw_buffer = b;
            self.draw_group = g;
        }

        self.draw_bytes.clear();
        self.draw_bytes.resize(draws.len().max(1) * DRAW_STRIDE as usize, 0);
        for (i, d) in draws.iter().enumerate() {
            let b = bytemuck::bytes_of(d);
            self.draw_bytes[i * DRAW_STRIDE as usize..i * DRAW_STRIDE as usize + b.len()].copy_from_slice(b);
        }
        self.queue.write_buffer(&self.draw_buffer, 0, &self.draw_bytes);

        // Scratch vertices (background): re-uploaded only when they changed.
        let scratch_bytes = scratch_data.len() as u64 * 4;

        if scratch_bytes > self.scratch_capacity {
            self.scratch_capacity = scratch_bytes.next_power_of_two();
            self.scratch = self.device.create_buffer(&wgpu::BufferDescriptor { label: Some("scratch"), size: self.scratch_capacity, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
            self.scratch_data.clear();
        }

        if scratch_bytes > 0 && scratch_data != self.scratch_data {
            self.queue.write_buffer(&self.scratch, 0, bytemuck::cast_slice(&scratch_data));
        }

        self.scratch_data = scratch_data;

        // Pipelines for the formats and states of this frame.
        for c in &calls {
            if !self.pipelines.contains_key(&(c.key, format)) {
                let p = self.make_pipeline(c.key, format);
                self.pipelines.insert((c.key, format), p);
                self.stats.pipelines_created += 1;
            }
        }

        // Depth buffer sized to the target.
        if self.depth.as_ref().is_none_or(|d| d.2 != width || d.3 != height) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("depth"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth24Plus,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            self.depth = Some((texture, view, width, height));
        }

        // --- the pass
        {
            let depth_view = &self.depth.as_ref().unwrap().1;
            let timestamp_writes = self.timer.as_ref().map(|t| wgpu::RenderPassTimestampWrites { query_set: &t.queries, beginning_of_pass_write_index: Some(0), end_of_pass_write_index: Some(1) });
            let [r, g, b, a] = self.clear_color;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("x3d"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    // drawShapes: gl.clearColor (0, 0, 0, 0)
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes,
                ..Default::default()
            });

            pass.set_bind_group(0, &self.frame_group, &[]);
            let white_group = &self.white_group;

            for c in &calls {
                pass.set_pipeline(&self.pipelines[&(c.key, format)]);
                pass.set_bind_group(1, &self.draw_group, &[(c.draw_index as u64 * DRAW_STRIDE) as u32]);
                let tg = c.texture.and_then(|p| self.textures.get(&p)).map(|t| &t.group).unwrap_or(white_group);
                pass.set_bind_group(2, tg, &[]);

                match c.source {
                    Source::Scratch => pass.set_vertex_buffer(0, self.scratch.slice(c.range.0..c.range.1)),
                    Source::Geometry(p) => pass.set_vertex_buffer(0, self.geometries[&p].buffer.slice(..)),
                }

                pass.draw(0..c.count, 0..1);
            }
        }

        self.stats.draw_calls = calls.len();
        self.calls = calls;

        if let Some(t) = self.timer.as_mut() {
            // The previous readback must be finished (or skipped) before the buffer is reused.
            if t.pending.is_none() {
                encoder.resolve_query_set(&t.queries, 0..2, &t.resolve, 0);
                encoder.copy_buffer_to_buffer(&t.resolve, 0, &t.read, 0, 16);
                t.recorded = true;
            }
        }

        // Forget GPU resources the viewer stopped using.
        if frame_no % 30 == 0 {
            self.geometries.retain(|_, e| e.used + KEEP_FRAMES >= frame_no);
            self.textures.retain(|_, e| e.used + KEEP_FRAMES >= frame_no);
        }

        Ok(())
    }
}

impl GpuTimer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<GpuTimer> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }

        Some(GpuTimer {
            queries: device.create_query_set(&wgpu::QuerySetDescriptor { label: Some("x3d timing"), ty: wgpu::QueryType::Timestamp, count: 2 }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor { label: Some("x3d timing"), size: 16, usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false }),
            read: device.create_buffer(&wgpu::BufferDescriptor { label: Some("x3d timing"), size: 16, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false }),
            period_ns: queue.get_timestamp_period(),
            pending: None,
            last_ms: None,
            recorded: false,
        })
    }
}

fn geometry_vertices(g: &crate::geometry::Geometry) -> Vec<f32> {
    let count = g.vertex_count();
    let mut out = Vec::with_capacity(count * FLOATS_PER_VERTEX);

    for i in 0..count {
        let v = &g.vertices[i * 4..i * 4 + 3];
        let nrm = if g.normals.len() >= (i + 1) * 3 { [g.normals[i * 3], g.normals[i * 3 + 1], g.normals[i * 3 + 2]] } else { [0.0; 3] };
        let t = if g.tex_coords.len() >= (i + 1) * 4 { [g.tex_coords[i * 4], g.tex_coords[i * 4 + 1], g.tex_coords[i * 4 + 2], g.tex_coords[i * 4 + 3]] } else { [0.0, 0.0, 0.0, 1.0] };
        let c = if g.colors.len() >= (i + 1) * 4 { [g.colors[i * 4], g.colors[i * 4 + 1], g.colors[i * 4 + 2], g.colors[i * 4 + 3]] } else { [1.0; 4] };
        out.extend_from_slice(&[v[0], v[1], v[2], nrm[0], nrm[1], nrm[2], t[0], t[1], t[2], t[3], c[0], c[1], c[2], c[3]]);
    }

    out
}

/// Headless renderer: its own Vulkan device (the discrete GPU), an RGBA8 target and a CPU
/// readback. Used by `x3d-shot`, the oracle tools and the tests.
pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_info: wgpu::AdapterInfo,
    pub renderer: Renderer,
}

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
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("x3d-render"),
            required_features: Renderer::wanted_features(&adapter),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let renderer = Renderer::new(device.clone(), queue.clone());
        Ok(Gpu { device, queue, adapter_info, renderer })
    }

    /// Render a frame; returns tightly packed RGBA8 rows, top row first.
    pub fn render(&mut self, frame: &Frame, width: u32, height: u32) -> Vec<u8> {
        self.renderer.render_to_rgba(frame, width, height)
    }

    /// Render without the readback and return the GPU time of the pass in milliseconds (None
    /// without timestamp support). For benchmarks.
    pub fn render_timed(&mut self, frame: &Frame, width: u32, height: u32) -> Option<f64> {
        self.renderer.render_offscreen_timed(frame, width, height)
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
