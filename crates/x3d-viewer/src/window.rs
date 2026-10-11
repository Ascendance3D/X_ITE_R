//! The native window: winit 0.30 + wgpu 30.
//!
//! The 3D frame is drawn by x3d-render's offscreen `Gpu` (Vulkan, no surface) and read back as
//! RGBA; this window owns a second, small wgpu device with the surface, uploads the readback
//! into a texture every frame and blits it. Pointer, wheel and keys become `UiEvent`s for the
//! `Viewer3d` core (y flipped: the core uses a bottom-left origin like the oracle).
//!
//! Status (load errors, Browser.print, sensor descriptions, Anchor activations) is shown in the
//! title bar and logged to stderr; there is no in-window text.

use crate::script::apply;
use crate::viewer::{Key as VKey, UiEvent, Viewer3d};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

const BLIT: &str = "
struct V { @builtin(position) p: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> V {
  var o: V;
  let x = f32((i << 1u) & 2u);
  let y = f32(i & 2u);
  o.p = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
  o.uv = vec2<f32>(x, y);
  return o;
}
@group(0) @binding(0) var t: texture_2d<f32>;
@group(0) @binding(1) var s: sampler;
@fragment fn fs(v: V) -> @location(0) vec4<f32> { return textureSample(t, s, v.uv); }
";

struct Blit {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    texture: Option<(wgpu::Texture, wgpu::BindGroup, u32, u32)>,
    window: Arc<Window>,
}

pub struct Options {
    pub path: PathBuf,
    pub oracle: Option<PathBuf>,
    /// Deterministic scripted run (the xvfb smoke test): events by frame index, a virtual clock.
    pub script: Option<Value>,
    pub png: Option<PathBuf>,
    pub size: (u32, u32),
}

pub struct ViewerWindow {
    opts: Options,
    window: Option<Arc<Window>>,
    blit: Option<Blit>,
    gpu: Option<x3d_render::gpu::Gpu>,
    viewer: Option<Viewer3d>,
    mods: ModifiersState,
    cursor: (f64, f64),
    frame: u64,
    shots: u32,
    last_title: String,
    last_prints: usize,
    pub exit_error: Option<String>,
    pub presented: u64,
}

fn epoch() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

impl ViewerWindow {
    pub fn new(opts: Options) -> ViewerWindow {
        ViewerWindow {
            opts,
            window: None,
            blit: None,
            gpu: None,
            viewer: None,
            mods: ModifiersState::empty(),
            cursor: (0.0, 0.0),
            frame: 0,
            shots: 0,
            last_title: String::new(),
            last_prints: 0,
            exit_error: None,
            presented: 0,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, msg: String) {
        self.exit_error = Some(msg);
        event_loop.exit();
    }

    fn input(&mut self, ev: UiEvent) {
        if let Some(v) = &mut self.viewer {
            v.input(ev);
        }
    }

    /// The window title doubles as the status line.
    fn update_title(&mut self) {
        let (Some(v), Some(w)) = (&mut self.viewer, &self.window) else { return };

        let world = {
            let t = v.title();
            if t.is_empty() { self.opts.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default() } else { t }
        };

        let bound = v.layer.viewpoint();
        let vp = if bound == crate::layer::DEFAULT { String::new() } else { v.viewpoint_description(bound) };
        let mut title = format!("{world} - {}", v.viewer_kind());

        if !vp.is_empty() {
            title.push_str(&format!(" - {vp}"));
        }

        if let Some(p) = v.rt.prints.last() {
            title.push_str(&format!(" | print: {p}"));
        }

        if !v.status.is_empty() {
            title.push_str(&format!(" | {}", v.status));
        }

        if let Some(e) = v.log().iter().rev().find(|l| l.starts_with("error")) {
            title.push_str(&format!(" | {e}"));
        }

        if title != self.last_title {
            w.set_title(&title);
            self.last_title = title;
        }

        for p in v.rt.prints.iter().skip(self.last_prints) {
            eprintln!("print: {p}");
        }

        self.last_prints = v.rt.prints.len();
    }

    fn keyboard(&mut self, event_loop: &ActiveEventLoop, key: &Key) {
        match key {
            Key::Named(NamedKey::Home) => self.input(UiEvent::Key(VKey::Home)),
            Key::Named(NamedKey::End) => self.input(UiEvent::Key(VKey::End)),
            Key::Named(NamedKey::PageUp) => self.input(UiEvent::Key(VKey::PageUp)),
            Key::Named(NamedKey::PageDown) => self.input(UiEvent::Key(VKey::PageDown)),
            Key::Named(NamedKey::Escape) => event_loop.exit(),
            Key::Character(c) => match c.as_str() {
                "1" => self.set_viewer("EXAMINE"),
                "2" => self.set_viewer("WALK"),
                "3" => self.set_viewer("FLY"),
                "4" => self.set_viewer("NONE"),
                "q" | "Q" => event_loop.exit(),
                "l" | "L" => {
                    if let Some(v) = &self.viewer {
                        for (i, n) in v.user_viewpoints().iter().enumerate() {
                            eprintln!("viewpoint {i}: {}", v.viewpoint_description(*n));
                        }
                    }
                }
                "s" | "S" => {
                    self.shots += 1;
                    let p = PathBuf::from(format!("/tmp/x3d-view-{}.png", self.shots));
                    if let Err(e) = self.screenshot(&p) {
                        eprintln!("screenshot: {e}");
                    } else {
                        eprintln!("saved {}", p.display());
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn set_viewer(&mut self, kind: &str) {
        if let Some(v) = &mut self.viewer {
            if v.available_viewers().iter().any(|a| a == kind || a == "ANY") || kind == "NONE" {
                v.set_viewer(kind);
            } else {
                v.status = format!("viewer {kind} not available (NavigationInfo type {:?})", v.available_viewers());
            }
        }
    }

    fn render_rgba(&mut self) -> Option<(Vec<u8>, u32, u32)> {
        let (v, g) = (self.viewer.as_mut()?, self.gpu.as_mut()?);
        let (w, h) = (v.width_px(), v.height_px());
        let frame = v.build_frame();
        Some((g.render(&frame, w, h), w, h))
    }

    fn screenshot(&mut self, path: &std::path::Path) -> Result<(), String> {
        let (rgba, w, h) = self.render_rgba().ok_or("nothing to render")?;
        let rgba: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
        x3d_render::write_png(path, w, h, &rgba)
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        // Advance the world: real clock, or the script's virtual clock.
        let scripted = self.opts.script.clone();

        if let Some(script) = &scripted {
            let t0 = script["t0"].as_f64().unwrap_or(946684800.0);
            let dt = script["dt"].as_f64().unwrap_or(1.0 / 60.0);
            let frames = script["frames"].as_u64().unwrap_or(1);
            self.frame += 1;
            let k = self.frame;

            if let Some(v) = &mut self.viewer {
                let events = script["events"].as_array().cloned().unwrap_or_default();

                for e in events.iter().filter(|e| e["frame"].as_u64() == Some(k)) {
                    v.date_now = t0 + (k - 1) as f64 * dt + e["t"].as_f64().unwrap_or(0.0);
                    apply(v, e);
                }

                v.date_now = t0 + k as f64 * dt;
                v.step(t0 + k as f64 * dt);
            }

            if k >= frames {
                if let Some(p) = self.opts.png.clone() {
                    if let Err(e) = self.screenshot(&p) {
                        self.exit_error = Some(format!("screenshot: {e}"));
                    }
                }

                if let Some(v) = &mut self.viewer {
                    let (p, _, _) = v.user_pose();
                    eprintln!("final pose: position [{:.6}, {:.6}, {:.6}] viewer {} status {:?} presented {} frames", p.x, p.y, p.z, v.viewer_kind(), v.status, self.presented);
                }
            }
        } else if let Some(v) = &mut self.viewer {
            let now = epoch();
            v.date_now = now;
            v.step(now);
        }

        self.update_title();

        if let Some((rgba, w, h)) = self.render_rgba() {
            if let Some(b) = &mut self.blit {
                b.present(&rgba, w, h);
                self.presented += 1;
            }
        }

        if let Some(script) = &scripted {
            if self.frame >= script["frames"].as_u64().unwrap_or(1) {
                event_loop.exit();
                return;
            }
        }

        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

impl Blit {
    fn new(window: Arc<Window>, event_loop: &ActiveEventLoop) -> Result<Blit, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(event_loop.owned_display_handle())));
        let surface = instance.create_surface(window.clone()).map_err(|e| format!("create_surface: {e}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| format!("no adapter for the surface: {e}"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).map_err(|e| format!("request_device: {e}"))?;
        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        // The readback is already sRGB-encoded: present it through a non-sRGB format.
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("blit"), source: wgpu::ShaderSource::Wgsl(BLIT.into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit"),
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
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("blit"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit"),
            layout: Some(&pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            cache: None,
            multiview_mask: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Nearest, min_filter: wgpu::FilterMode::Nearest, ..Default::default() });

        Ok(Blit { surface, config, device, queue, pipeline, layout, sampler, texture: None, window })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        self.config.width = size.width.max(1);
        self.config.height = size.height.max(1);
        self.surface.configure(&self.device, &self.config);
    }

    fn present(&mut self, rgba: &[u8], w: u32, h: u32) {
        if self.texture.as_ref().is_none_or(|t| t.2 != w || t.3 != h) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("frame"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("frame"),
                layout: &self.layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) }],
            });
            self.texture = Some((texture, bind, w, h));
        }

        let (texture, bind, _, _) = self.texture.as_ref().unwrap();

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            _ => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("blit") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bind, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        self.queue.present(frame);
    }
}

impl ApplicationHandler for ViewerWindow {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let (w, h) = self.opts.size;
        let attrs = Window::default_attributes().with_inner_size(LogicalSize::new(w as f64, h as f64)).with_resizable(self.opts.script.is_none()).with_title("x3d-view");
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => return self.fail(event_loop, format!("create_window: {e}")),
        };
        let blit = match Blit::new(window.clone(), event_loop) {
            Ok(b) => b,
            Err(e) => return self.fail(event_loop, e),
        };
        let gpu = match x3d_render::gpu::Gpu::new() {
            Ok(g) => g,
            Err(e) => return self.fail(event_loop, e),
        };

        let size = window.inner_size();
        let t0 = self.opts.script.as_ref().map(|s| s["t0"].as_f64().unwrap_or(946684800.0)).unwrap_or_else(epoch);

        match Viewer3d::load(&self.opts.path, self.opts.oracle.as_deref(), t0, size.width.max(1), size.height.max(1)) {
            Ok(mut v) => {
                // The script's warm-up frames (frames the browser draws while loading).
                if let Some(s) = &self.opts.script {
                    v.frame_rate = 1.0 / s["dt"].as_f64().unwrap_or(1.0 / 60.0);
                    for _ in 0..s["warmup"].as_u64().unwrap_or(8) {
                        v.step(t0);
                    }
                }
                self.viewer = Some(v);
            }
            Err(e) => return self.fail(event_loop, format!("load {}: {e}", self.opts.path.display())),
        }

        self.blit = Some(blit);
        self.gpu = Some(gpu);
        self.window = Some(window.clone());
        self.update_title();
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(b) = &mut self.blit {
                    b.resize(size);
                }
                if let Some(v) = &mut self.viewer {
                    v.resize(size.width, size.height);
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                self.mods = m.state();
                self.input(UiEvent::Mods { shift: self.mods.shift_key(), ctrl: self.mods.control_key(), alt: self.mods.alt_key() });
            }
            WindowEvent::CursorMoved { position, .. } => {
                let h = self.viewer.as_ref().map(|v| v.height_px() as f64).unwrap_or(0.0);
                self.cursor = (position.x, h - position.y);
                self.input(UiEvent::MouseMove { x: self.cursor.0, y: self.cursor.1 });
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let b = match button {
                    MouseButton::Left => 0,
                    MouseButton::Middle => 1,
                    MouseButton::Right => 2,
                    _ => return,
                };
                let (x, y) = self.cursor;
                self.input(if state == ElementState::Pressed { UiEvent::MouseDown { button: b, x, y } } else { UiEvent::MouseUp { button: b, x, y } });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // DOM wheel events: deltaY > 0 scrolls down (zoom out); a notch is 100 px.
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -(y as f64) * 100.0,
                    MouseScrollDelta::PixelDelta(p) => -p.y,
                };
                let (x, y) = self.cursor;
                self.input(UiEvent::Wheel { delta_y: dy, x, y });
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed && !event.repeat => {
                self.keyboard(event_loop, &event.logical_key);
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }
}

/// Run the window on the calling thread (a large-stack thread: QuickJS needs it).
pub fn run(opts: Options) -> Result<u64, String> {
    use winit::platform::wayland::EventLoopBuilderExtWayland;
    use winit::platform::x11::EventLoopBuilderExtX11;

    let mut builder = EventLoop::builder();
    EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
    EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
    let event_loop = builder.build().map_err(|e| format!("event loop: {e}"))?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = ViewerWindow::new(opts);
    event_loop.run_app(&mut app).map_err(|e| format!("run: {e}"))?;
    match app.exit_error {
        Some(e) => Err(e),
        None => Ok(app.presented),
    }
}
