//! The native window: winit 0.30 + wgpu 30, a thin host over [`EmbeddedViewer`].
//!
//! One wgpu device does everything: the viewer renders straight into the window's surface
//! texture (no CPU readback, no second device). Pointer, wheel and keys become `UiEvent`s for
//! the viewer (y flipped: the viewer uses a bottom-left origin like the oracle).
//!
//! Status (load errors, Browser.print, sensor descriptions, Anchor activations) is shown in the
//! title bar and logged to stderr; there is no in-window text.
//!
//! Environment: `X3D_VIEW_PRESENT=fifo|fiforelaxed|mailbox|immediate` picks the present mode
//! (default `fifo`: strict vertical sync, never tears); `X3D_VIEW_NOVSYNC=1` is `immediate`;
//! `X3D_VIEW_STATS=1` prints frame statistics on exit and when the window closes.

use crate::embed::{EmbeddedViewer, FrameInfo, LoadOptions, realtime_now};
use crate::script::apply;
use crate::viewer::{Key as VKey, UiEvent};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

pub struct Options {
    pub path: PathBuf,
    pub oracle: Option<PathBuf>,
    /// Deterministic scripted run (the xvfb smoke test): events by frame index, a virtual clock.
    pub script: Option<Value>,
    pub png: Option<PathBuf>,
    pub size: (u32, u32),
    /// Requested multisampling (`--msaa N`); X_ITE's default 4 when `None`.
    pub msaa: Option<u32>,
    /// `--depth standard|log|reversed`; X_ITE's standard depth buffer when `None`.
    pub depth: Option<crate::embed::DepthMode>,
}

/// Everything that exists once the window does.
struct Gfx {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    /// The format views of the surface texture use (the non-sRGB sibling of the surface format).
    view_format: wgpu::TextureFormat,
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_info: wgpu::AdapterInfo,
    viewer: EmbeddedViewer,
}

#[derive(Default)]
struct Stats {
    frames: u64,
    step: f64,
    build: f64,
    render: f64,
    acquire: f64,
    present: f64,
    gpu: f64,
    gpu_frames: u64,
}

pub struct ViewerWindow {
    opts: Options,
    gfx: Option<Gfx>,
    mods: ModifiersState,
    cursor: (f64, f64),
    /// Time and position of the previous left button release (the browser's click count for "dblclick").
    last_left_up: Option<(std::time::Instant, f64, f64)>,
    frame: u64,
    shots: u32,
    last_title: String,
    last_prints: usize,
    stats: Stats,
    want_stats: bool,
    pub exit_error: Option<String>,
    pub presented: u64,
}

impl ViewerWindow {
    pub fn new(opts: Options) -> ViewerWindow {
        ViewerWindow {
            opts,
            gfx: None,
            mods: ModifiersState::empty(),
            cursor: (0.0, 0.0),
            last_left_up: None,
            frame: 0,
            shots: 0,
            last_title: String::new(),
            last_prints: 0,
            stats: Stats::default(),
            want_stats: std::env::var("X3D_VIEW_STATS").is_ok(),
            exit_error: None,
            presented: 0,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, msg: String) {
        self.exit_error = Some(msg);
        event_loop.exit();
    }

    fn input(&mut self, ev: UiEvent) {
        if let Some(g) = &mut self.gfx {
            g.viewer.input(ev);
        }
    }

    /// The window title doubles as the status line.
    fn update_title(&mut self) {
        let Some(g) = &mut self.gfx else { return };

        let world = {
            let t = g.viewer.title();
            if t.is_empty() { self.opts.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default() } else { t }
        };

        let vp = g.viewer.viewpoints().into_iter().find(|v| v.bound).map(|v| v.description).unwrap_or_default();
        let mut title = format!("{world} - {}", g.viewer.navigation_type());

        if !vp.is_empty() {
            title.push_str(&format!(" - {vp}"));
        }

        let prints = g.viewer.core().map(|c| c.rt.prints.clone()).unwrap_or_default();

        if let Some(p) = prints.last() {
            title.push_str(&format!(" | print: {p}"));
        }

        if !g.viewer.status().is_empty() {
            title.push_str(&format!(" | {}", g.viewer.status()));
        }

        if let Some(e) = g.viewer.log().iter().rev().find(|l| l.starts_with("error")) {
            title.push_str(&format!(" | {e}"));
        }

        if title != self.last_title {
            g.window.set_title(&title);
            self.last_title = title;
        }

        for p in prints.iter().skip(self.last_prints) {
            eprintln!("print: {p}");
        }

        self.last_prints = prints.len();
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
                "m" | "M" => {
                    if let Some(g) = &mut self.gfx {
                        let n = g.viewer.cycle_multisampling();
                        eprintln!("multisampling {n}x");
                    }
                    self.update_title();
                }
                "z" | "Z" => {
                    if let Some(g) = &mut self.gfx {
                        let m = g.viewer.cycle_depth_mode();
                        eprintln!("depth buffer {m:?}");
                    }
                }
                "r" | "R" => {
                    if let Some(g) = &mut self.gfx {
                        if let Err(e) = g.viewer.reload() {
                            eprintln!("reload: {e}");
                        }
                    }
                }
                "l" | "L" => {
                    if let Some(g) = &self.gfx {
                        for v in g.viewer.viewpoints() {
                            eprintln!("viewpoint {}: {}", v.index, v.description);
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
        if let Some(g) = &mut self.gfx {
            let allowed = g.viewer.available_navigation_types();

            if allowed.iter().any(|a| a == kind || a == "ANY") || kind == "NONE" {
                g.viewer.set_navigation_type(kind);
            } else if let Some(c) = g.viewer.core() {
                c.status = format!("viewer {kind} not available (NavigationInfo type {allowed:?})");
            }
        }
    }

    fn screenshot(&mut self, path: &std::path::Path) -> Result<(), String> {
        let g = self.gfx.as_mut().ok_or("no window")?;
        let (w, h) = g.viewer.size();
        let rgba = g.viewer.screenshot_rgba(w, h).ok_or("nothing to render")?;
        let rgba: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
        x3d_render::write_png(path, w, h, &rgba)
    }

    /// Acquire the surface texture, draw the viewer into it, present.
    fn draw(&mut self, now: f64) -> Option<FrameInfo> {
        let g = self.gfx.as_mut()?;
        let t0 = Instant::now();

        let frame = match g.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            _ => {
                g.surface.configure(&g.device, &g.config);
                return None;
            }
        };

        let t1 = Instant::now();
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor { format: Some(g.view_format), ..Default::default() });
        let (w, h) = (g.config.width, g.config.height);

        let info = match g.viewer.frame(now, &view, g.view_format, w, h) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("render: {e}");
                FrameInfo::default()
            }
        };

        let t2 = Instant::now();
        g.window.pre_present_notify();
        g.queue.present(frame);
        let t3 = Instant::now();

        let s = &mut self.stats;
        s.frames += 1;
        s.acquire += (t1 - t0).as_secs_f64();
        s.step += info.step_ms;
        s.build += info.build_ms;
        s.render += info.render_ms;
        s.present += (t3 - t2).as_secs_f64();

        if let Some(ms) = info.gpu_ms {
            s.gpu += ms;
            s.gpu_frames += 1;
        }

        self.presented += 1;
        Some(info)
    }

    fn print_stats(&self) {
        let s = &self.stats;

        if !self.want_stats || s.frames == 0 {
            return;
        }

        let n = s.frames as f64;
        let adapter = self.gfx.as_ref().map(|g| format!("{} ({:?}, {:?})", g.adapter_info.name, g.adapter_info.backend, g.adapter_info.device_type)).unwrap_or_default();
        let gpu = if s.gpu_frames > 0 { format!("{:.2} ms", s.gpu / s.gpu_frames as f64) } else { "n/a".to_string() };
        eprintln!(
            "stats: {} frames on {adapter}: step {:.2} ms, build {:.2} ms, render submit {:.2} ms, GPU {gpu}, acquire {:.2} ms, present {:.2} ms (CPU, per frame)",
            s.frames,
            s.step / n,
            s.build / n,
            s.render / n,
            s.acquire / n * 1e3,
            s.present / n * 1e3
        );
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        // Advance the world: real clock, or the script's virtual clock.
        let scripted = self.opts.script.clone();
        let mut now = realtime_now();

        if let Some(script) = &scripted {
            let t0 = script["t0"].as_f64().unwrap_or(946684800.0);
            let dt = script["dt"].as_f64().unwrap_or(1.0 / 60.0);
            self.frame += 1;
            let k = self.frame;
            now = t0 + k as f64 * dt;

            if let Some(g) = &mut self.gfx {
                if let Some(v) = g.viewer.core() {
                    let events = script["events"].as_array().cloned().unwrap_or_default();

                    for e in events.iter().filter(|e| e["frame"].as_u64() == Some(k)) {
                        v.date_now = t0 + (k - 1) as f64 * dt + e["t"].as_f64().unwrap_or(0.0);
                        apply(v, e);
                    }
                }
            }

        }

        self.draw(now);
        self.update_title();

        if let Some(script) = &scripted {
            if self.frame >= script["frames"].as_u64().unwrap_or(1) {
                // The last frame of a scripted run: screenshot and final pose, then quit.
                if let Some(p) = self.opts.png.clone() {
                    if let Err(e) = self.screenshot(&p) {
                        self.exit_error = Some(format!("screenshot: {e}"));
                    }
                }

                let presented = self.presented;

                if let Some(g) = &mut self.gfx {
                    if let Some(v) = g.viewer.core() {
                        let (p, _, _) = v.user_pose();
                        eprintln!("final pose: position [{:.6}, {:.6}, {:.6}] viewer {} status {:?} presented {presented} frames", p.x, p.y, p.z, v.viewer_kind(), v.status);
                    }
                }

                event_loop.exit();
                return;
            }
        }

        if let Some(g) = &self.gfx {
            g.window.request_redraw();
        }
    }
}

/// The window, surface, device and viewer.
fn create_gfx(event_loop: &ActiveEventLoop, opts: &Options) -> Result<Gfx, String> {
    let (w, h) = opts.size;
    let attrs = Window::default_attributes().with_inner_size(LogicalSize::new(w as f64, h as f64)).with_resizable(opts.script.is_none()).with_title("x3d-view");
    let window = Arc::new(event_loop.create_window(attrs).map_err(|e| format!("create_window: {e}"))?);

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(event_loop.owned_display_handle())));
    let surface = instance.create_surface(window.clone()).map_err(|e| format!("create_surface: {e}"))?;
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
        ..Default::default()
    }))
    .map_err(|e| format!("no adapter for the surface: {e}"))?;
    let adapter_info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("x3d-view"),
        required_features: EmbeddedViewer::wanted_features(&adapter),
        ..Default::default()
    }))
    .map_err(|e| format!("request_device: {e}"))?;

    let caps = surface.get_capabilities(&adapter);
    // The viewer writes display-encoded values: draw through a non-sRGB view. Prefer a non-sRGB
    // surface format; otherwise reinterpret an sRGB one (view_formats).
    let surface_format = caps
        .formats
        .iter()
        .copied()
        .find(|f| x3d_render::gpu::target_format_ok(*f))
        .or_else(|| caps.formats.iter().copied().find(|f| x3d_render::gpu::target_format_ok(f.remove_srgb_suffix())))
        .ok_or_else(|| format!("no usable surface format in {:?}", caps.formats))?;
    let view_format = surface_format.remove_srgb_suffix();
    let size = window.inner_size();
    // Strict FIFO by default. AutoVsync would pick FifoRelaxed where supported, which presents a
    // late frame immediately and tears mid-scan — visible as a split picture while dragging.
    let requested = match std::env::var("X3D_VIEW_PRESENT").as_deref() {
        Ok("fiforelaxed") => wgpu::PresentMode::FifoRelaxed,
        Ok("mailbox") => wgpu::PresentMode::Mailbox,
        Ok("immediate") => wgpu::PresentMode::Immediate,
        _ if std::env::var("X3D_VIEW_NOVSYNC").is_ok() => wgpu::PresentMode::Immediate,
        _ => wgpu::PresentMode::Fifo,
    };
    // Fifo is always supported; fall back to it when the requested mode is not.
    let present_mode = if caps.present_modes.contains(&requested) { requested } else { wgpu::PresentMode::Fifo };
    // The picture is opaque; do not let the compositor blend it with what is behind the window.
    let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) { wgpu::CompositeAlphaMode::Opaque } else { caps.alpha_modes[0] };
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode,
        alpha_mode,
        view_formats: if view_format != surface_format { vec![view_format] } else { vec![] },
        desired_maximum_frame_latency: 2,
        color_space: wgpu::SurfaceColorSpace::Auto,
    };
    surface.configure(&device, &config);

    if std::env::var("X3D_VIEW_STATS").is_ok() {
        eprintln!("adapter {} ({:?}, {:?}); surface {:?} -> view {:?}; present {:?} of {:?}; alpha {:?} of {:?}", adapter_info.name, adapter_info.backend, adapter_info.device_type, surface_format, view_format, present_mode, caps.present_modes, alpha_mode, caps.alpha_modes);
    }

    let mut viewer = EmbeddedViewer::new(device.clone(), queue.clone(), (config.width, config.height));
    viewer.renderer().clear_color = [0.0, 0.0, 0.0, 1.0];
    viewer.configure_samples(&adapter, view_format);

    if let Some(n) = opts.msaa {
        viewer.set_multisampling(n);
    }

    if let Some(m) = opts.depth {
        viewer.set_depth_mode(m);
    }

    if std::env::var("X3D_VIEW_STATS").is_ok() {
        eprintln!("multisampling {} (supported {:?})", viewer.multisampling(), viewer.supported_multisampling());
    }

    Ok(Gfx { window, surface, config, view_format, device, queue, adapter_info, viewer })
}

impl ApplicationHandler for ViewerWindow {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }

        let mut gfx = match create_gfx(event_loop, &self.opts) {
            Ok(g) => g,
            Err(e) => return self.fail(event_loop, e),
        };

        let t0 = self.opts.script.as_ref().map(|s| s["t0"].as_f64().unwrap_or(946684800.0));

        if let Err(e) = gfx.viewer.load(&self.opts.path, LoadOptions { root: self.opts.oracle.clone(), start_time: t0 }) {
            return self.fail(event_loop, format!("load {}: {e}", self.opts.path.display()));
        }

        // The script's warm-up frames (frames the browser draws while loading), on its clock.
        if let (Some(s), Some(t0)) = (&self.opts.script, t0) {
            if let Some(v) = gfx.viewer.core() {
                v.frame_rate = 1.0 / s["dt"].as_f64().unwrap_or(1.0 / 60.0);

                for _ in 0..s["warmup"].as_u64().unwrap_or(8) {
                    v.step(t0);
                }
            }
        }

        let window = gfx.window.clone();
        self.gfx = Some(gfx);
        self.update_title();
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(g) = &mut self.gfx {
                    g.config.width = size.width.max(1);
                    g.config.height = size.height.max(1);
                    g.surface.configure(&g.device, &g.config);
                    g.viewer.resize(g.config.width, g.config.height);
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                self.mods = m.state();
                self.input(UiEvent::Mods { shift: self.mods.shift_key(), ctrl: self.mods.control_key(), alt: self.mods.alt_key() });
            }
            WindowEvent::CursorMoved { position, .. } => {
                let h = self.gfx.as_ref().map(|g| g.viewer.size().1 as f64).unwrap_or(0.0);
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

                // The second release of a double click fires the DOM "dblclick" event (LOOKAT, PLANE).
                if b == 0 && state == ElementState::Released {
                    let now = std::time::Instant::now();

                    match self.last_left_up.take() {
                        Some((t, px, py)) if now.duration_since(t).as_millis() < 500 && (px - x).abs() <= 4.0 && (py - y).abs() <= 4.0 => self.input(UiEvent::DblClick { x, y }),
                        _ => self.last_left_up = Some((now, x, y)),
                    }
                }
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
    app.print_stats();

    match app.exit_error {
        Some(e) => Err(e),
        None => Ok(app.presented),
    }
}
