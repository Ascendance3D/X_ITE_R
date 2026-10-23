//! The native window: winit 0.30 + wgpu 30, a thin host over [`EmbeddedViewer`].
//!
//! One wgpu device does everything: the viewer renders straight into the window's surface
//! texture (no CPU readback, no second device). Pointer, wheel and keys become `UiEvent`s for
//! the viewer (y flipped: the viewer uses a bottom-left origin like the oracle).
//!
//! Keys: every key goes to X3D as a DOM keydown / keyup (repeats included, as in the browser);
//! only the application shortcuts in [`crate::keys`] (Ctrl on Linux, Command on macOS, + 1-4, Q,
//! M, Z, R, L, S) stay with the window. Escape is an X3D key; closing is the window manager's.
//!
//! Status (load errors, Browser.print, sensor descriptions, Anchor activations) is shown in the
//! title bar and logged to stderr; there is no in-window text.
//!
//! Environment: `X3D_VIEW_PRESENT=fifo|fiforelaxed|mailbox|immediate` picks the present mode
//! (default `fifo`: strict vertical sync, never tears); `X3D_VIEW_NOVSYNC=1` is `immediate`;
//! `X3D_VIEW_STATS=1` prints frame statistics on exit and when the window closes;
//! `X3D_VIEW_STARTUP=1` prints the startup phases up to the first presented frame. Backend and
//! DX12 shader compiler selection (`WGPU_BACKEND`, `WGPU_DX12_COMPILER`): see [`crate::backend`].

use crate::backend::{self, GpuErrors};
use crate::embed::{EmbeddedViewer, FrameInfo, LoadOptions, realtime_now};
use crate::keys::{KeyAction, KeyTracker, Platform, Shortcut};
use crate::script::apply;
use crate::viewer::UiEvent;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::ModifiersState;
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
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
    /// GPU errors outside an error scope; the first one ends the run.
    gpu_errors: GpuErrors,
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

/// Consecutive frames a scripted run waits for a surface before it gives up.
const MAX_SURFACE_MISSES: u32 = 120;

/// The pause before a scripted run asks again for a surface that was not ready.
const SURFACE_RETRY: std::time::Duration = std::time::Duration::from_millis(25);

/// What a scripted frame needs from the window. Tests stand in for it without a GPU.
trait ScriptedHost {
    type Frame;
    /// The texture for the next frame; why not, when the surface is not ready.
    fn acquire(&mut self) -> Result<Self::Frame, String>;
    /// Apply one scripted event at the browser time `date_now`.
    fn event(&mut self, date_now: f64, e: &Value);
    /// Step the world to `now`, draw it into `frame` and present it.
    fn step_and_present(&mut self, frame: Self::Frame, now: f64) -> Result<(), String>;
}

/// The outcome of one scripted redraw.
#[derive(Debug, PartialEq)]
enum Tick {
    /// A frame was stepped and presented; more follow.
    Next,
    /// The surface was not ready: nothing happened, try again.
    Retry,
    /// The last frame was stepped and presented.
    Finished,
    /// The run cannot complete.
    Failed(String),
}

/// A deterministic scripted run (`--script`): events by frame index, a virtual clock.
struct ScriptedRun {
    t0: f64,
    dt: f64,
    frames: u64,
    events: Vec<Value>,
    /// Frames stepped and presented.
    done: u64,
    /// Consecutive redraws without a surface.
    misses: u32,
}

impl ScriptedRun {
    fn new(script: &Value) -> ScriptedRun {
        ScriptedRun {
            t0: script["t0"].as_f64().unwrap_or(946684800.0),
            dt: script["dt"].as_f64().unwrap_or(1.0 / 60.0),
            frames: script["frames"].as_u64().unwrap_or(1),
            events: script["events"].as_array().cloned().unwrap_or_default(),
            done: 0,
            misses: 0,
        }
    }

    /// Every requested frame was stepped and presented (at least one, as with `frames: 0`).
    fn complete(&self) -> bool {
        self.done >= self.frames.max(1)
    }

    /// One redraw. Frame `k` counts only once its world step is drawn and presented. The surface
    /// is acquired first: when it is not ready nothing happens, and frame `k` (its events and
    /// its step) is tried again on the next redraw.
    fn tick<H: ScriptedHost>(&mut self, host: &mut H) -> Tick {
        let k = self.done + 1;

        let frame = match host.acquire() {
            Ok(f) => f,
            Err(why) => {
                self.misses += 1;

                if self.misses < MAX_SURFACE_MISSES {
                    return Tick::Retry;
                }

                return Tick::Failed(format!(
                    "scripted run: no surface for frame {k} of {} after {} attempts ({why}); {} frames completed",
                    self.frames, self.misses, self.done
                ));
            }
        };

        self.misses = 0;

        // The frame's events, then the step that sees them.
        for e in self
            .events
            .iter()
            .filter(|e| e["frame"].as_u64() == Some(k))
        {
            host.event(
                self.t0 + (k - 1) as f64 * self.dt + e["t"].as_f64().unwrap_or(0.0),
                e,
            );
        }

        if let Err(e) = host.step_and_present(frame, self.t0 + k as f64 * self.dt) {
            return Tick::Failed(format!(
                "scripted run: frame {k} of {}: render: {e}",
                self.frames
            ));
        }

        self.done = k;

        if self.complete() {
            Tick::Finished
        } else {
            Tick::Next
        }
    }
}

/// `X3D_VIEW_STARTUP=1`: a startup phase on stderr, in milliseconds since [`run`] began and since
/// the Unix epoch (to compare with the clock of whatever launched the viewer).
fn startup_mark(what: &str) {
    static T0: OnceLock<Option<Instant>> = OnceLock::new();
    let Some(t0) = T0.get_or_init(|| std::env::var("X3D_VIEW_STARTUP").is_ok().then(Instant::now))
    else {
        return;
    };
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    eprintln!(
        "startup {what}: {:.1} ms (epoch {epoch} ms)",
        t0.elapsed().as_secs_f64() * 1e3
    );
}

/// The window as the host of a scripted run.
struct WindowHost<'a> {
    gfx: Option<&'a mut Gfx>,
    stats: &'a mut Stats,
    presented: &'a mut u64,
}

impl ScriptedHost for WindowHost<'_> {
    type Frame = (wgpu::SurfaceTexture, f64);

    fn acquire(&mut self) -> Result<Self::Frame, String> {
        let g = self.gfx.as_deref_mut().ok_or("no window")?;
        let t0 = Instant::now();
        acquire(g).map(|f| (f, t0.elapsed().as_secs_f64()))
    }

    fn event(&mut self, date_now: f64, e: &Value) {
        if let Some(v) = self.gfx.as_deref_mut().and_then(|g| g.viewer.core()) {
            v.date_now = date_now;
            apply(v, e);
        }
    }

    fn step_and_present(&mut self, (frame, acquire): Self::Frame, now: f64) -> Result<(), String> {
        let g = self.gfx.as_deref_mut().ok_or("no window")?;
        render_present(g, self.stats, self.presented, frame, acquire, now)?;
        g.gpu_errors
            .take()
            .map_or(Ok(()), |e| Err(format!("GPU error: {e}")))
    }
}

/// The surface texture for the next frame. When the surface is not ready, reconfigure it and say why.
fn acquire(g: &mut Gfx) -> Result<wgpu::SurfaceTexture, String> {
    let why = match g.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
            return Ok(f);
        }
        wgpu::CurrentSurfaceTexture::Timeout => "timeout",
        wgpu::CurrentSurfaceTexture::Occluded => "occluded",
        wgpu::CurrentSurfaceTexture::Outdated => "outdated",
        wgpu::CurrentSurfaceTexture::Lost => "lost",
        wgpu::CurrentSurfaceTexture::Validation => "validation error",
    };

    g.surface.configure(&g.device, &g.config);
    Err(format!("surface {why}"))
}

/// Step and draw the viewer into `frame`, present it. The frame is presented even when drawing
/// fails; the error is returned after.
fn render_present(
    g: &mut Gfx,
    s: &mut Stats,
    presented: &mut u64,
    frame: wgpu::SurfaceTexture,
    acquire: f64,
    now: f64,
) -> Result<FrameInfo, String> {
    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
        format: Some(g.view_format),
        ..Default::default()
    });
    let (w, h) = (g.config.width, g.config.height);
    let result = g.viewer.frame(now, &view, g.view_format, w, h);
    let info = result.as_ref().copied().unwrap_or_default();

    let t2 = Instant::now();
    g.window.pre_present_notify();
    g.queue.present(frame);
    let t3 = Instant::now();

    s.frames += 1;
    s.acquire += acquire;
    s.step += info.step_ms;
    s.build += info.build_ms;
    s.render += info.render_ms;
    s.present += (t3 - t2).as_secs_f64();

    if let Some(ms) = info.gpu_ms {
        s.gpu += ms;
        s.gpu_frames += 1;
    }

    *presented += 1;
    result
}

pub struct ViewerWindow {
    opts: Options,
    gfx: Option<Gfx>,
    mods: ModifiersState,
    keys: KeyTracker,
    cursor: (f64, f64),
    /// Time and position of the previous left button release (the browser's click count for "dblclick").
    last_left_up: Option<(std::time::Instant, f64, f64)>,
    scripted: Option<ScriptedRun>,
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
            gfx: None,
            mods: ModifiersState::empty(),
            keys: KeyTracker::default(),
            cursor: (0.0, 0.0),
            last_left_up: None,
            scripted: opts.script.as_ref().map(ScriptedRun::new),
            shots: 0,
            last_title: String::new(),
            last_prints: 0,
            stats: Stats::default(),
            want_stats: std::env::var("X3D_VIEW_STATS").is_ok(),
            opts,
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
            if t.is_empty() {
                self.opts
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            } else {
                t
            }
        };

        let vp = g
            .viewer
            .viewpoints()
            .into_iter()
            .find(|v| v.bound)
            .map(|v| v.description)
            .unwrap_or_default();
        let mut title = format!("{world} - {}", g.viewer.navigation_type());

        if !vp.is_empty() {
            title.push_str(&format!(" - {vp}"));
        }

        let prints = g
            .viewer
            .core()
            .map(|c| c.rt.prints.clone())
            .unwrap_or_default();

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

    fn key_action(&mut self, event_loop: &ActiveEventLoop, action: KeyAction) {
        match action {
            KeyAction::Dom { down, key, code } => {
                self.input(UiEvent::NativeKey { down, key, code })
            }
            KeyAction::Shortcut(s) => self.shortcut(event_loop, s),
            KeyAction::None => {}
        }
    }

    fn shortcut(&mut self, event_loop: &ActiveEventLoop, s: Shortcut) {
        match s {
            Shortcut::Viewer(kind) => self.set_viewer(kind),
            Shortcut::Quit => event_loop.exit(),
            Shortcut::Multisampling => {
                if let Some(g) = &mut self.gfx {
                    let n = g.viewer.cycle_multisampling();
                    eprintln!("multisampling {n}x");
                }
                self.update_title();
            }
            Shortcut::DepthMode => {
                if let Some(g) = &mut self.gfx {
                    let m = g.viewer.cycle_depth_mode();
                    eprintln!("depth buffer {m:?}");
                }
            }
            Shortcut::Reload => {
                if let Some(g) = &mut self.gfx {
                    if let Err(e) = g.viewer.reload() {
                        eprintln!("reload: {e}");
                    }
                }
            }
            Shortcut::ListViewpoints => {
                if let Some(g) = &self.gfx {
                    for v in g.viewer.viewpoints() {
                        eprintln!("viewpoint {}: {}", v.index, v.description);
                    }
                }
            }
            Shortcut::Screenshot => {
                self.shots += 1;
                let p = PathBuf::from(format!("/tmp/x3d-view-{}.png", self.shots));
                if let Err(e) = self.screenshot(&p) {
                    eprintln!("screenshot: {e}");
                } else {
                    eprintln!("saved {}", p.display());
                }
            }
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
        let rgba: Vec<u8> = rgba
            .chunks_exact(4)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect();
        x3d_render::write_png(path, w, h, &rgba)
    }

    /// Acquire the surface texture, draw the viewer into it, present.
    fn draw(&mut self, now: f64) -> Option<FrameInfo> {
        let g = self.gfx.as_mut()?;
        let t0 = Instant::now();
        let frame = acquire(g).ok()?;
        let t1 = Instant::now();

        match render_present(
            g,
            &mut self.stats,
            &mut self.presented,
            frame,
            (t1 - t0).as_secs_f64(),
            now,
        ) {
            Ok(i) => Some(i),
            Err(e) => {
                eprintln!("render: {e}");
                Some(FrameInfo::default())
            }
        }
    }

    fn print_stats(&self) {
        let s = &self.stats;

        if !self.want_stats || s.frames == 0 {
            return;
        }

        let n = s.frames as f64;
        let adapter = self
            .gfx
            .as_ref()
            .map(|g| {
                format!(
                    "{} ({:?}, {:?})",
                    g.adapter_info.name, g.adapter_info.backend, g.adapter_info.device_type
                )
            })
            .unwrap_or_default();
        let gpu = if s.gpu_frames > 0 {
            format!("{:.2} ms", s.gpu / s.gpu_frames as f64)
        } else {
            "n/a".to_string()
        };
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
        if self.presented == 0 {
            startup_mark("redraw");
            self.redraw_frame(event_loop);

            if self.presented > 0 {
                let pipelines = self
                    .gfx
                    .as_mut()
                    .map_or(0, |g| g.viewer.renderer().stats.pipelines_created);
                startup_mark(&format!("first frame presented ({pipelines} pipelines)"));
            }

            return;
        }

        self.redraw_frame(event_loop);
    }

    fn redraw_frame(&mut self, event_loop: &ActiveEventLoop) {
        if self.scripted.is_some() {
            return self.scripted_redraw(event_loop);
        }

        self.draw(realtime_now());

        if let Some(e) = self.gfx.as_ref().and_then(|g| g.gpu_errors.take()) {
            return self.fail(event_loop, format!("GPU error: {e}"));
        }

        self.update_title();

        if let Some(g) = &self.gfx {
            g.window.request_redraw();
        }
    }

    /// One redraw of a scripted run, on the script's virtual clock.
    fn scripted_redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(run) = self.scripted.as_mut() else {
            return;
        };
        let tick = run.tick(&mut WindowHost {
            gfx: self.gfx.as_mut(),
            stats: &mut self.stats,
            presented: &mut self.presented,
        });
        self.update_title();

        match tick {
            Tick::Next => {}
            Tick::Retry => std::thread::sleep(SURFACE_RETRY),
            Tick::Failed(e) => return self.fail(event_loop, e),
            Tick::Finished => {
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
                        eprintln!(
                            "final pose: position [{:.6}, {:.6}, {:.6}] viewer {} status {:?} presented {presented} frames",
                            p.x,
                            p.y,
                            p.z,
                            v.viewer_kind(),
                            v.status
                        );
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
    let attrs = Window::default_attributes()
        .with_inner_size(LogicalSize::new(w as f64, h as f64))
        .with_resizable(opts.script.is_none())
        .with_title("x3d-view");
    // Windows: the window shows once its first frame can be drawn (see `resumed`). Until then
    // the event loop is not running, and a visible window would be marked as not responding
    // after a few seconds.
    let attrs = attrs.with_visible(!cfg!(windows));
    let window = Arc::new(
        event_loop
            .create_window(attrs)
            .map_err(|e| format!("create_window: {e}"))?,
    );
    startup_mark("window");

    let desc = backend::instance_descriptor()?;
    let backends = desc.backends;
    let instance =
        wgpu::Instance::new(desc.with_display_handle(Box::new(event_loop.owned_display_handle())));
    let surface = instance
        .create_surface(window.clone())
        .map_err(|e| format!("create_surface: {e}"))?;
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
        ..Default::default()
    }))
    .map_err(|e| backend::adapter_error(backends, &e))?;
    startup_mark("adapter");
    let adapter_info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("x3d-view"),
        required_features: EmbeddedViewer::wanted_features(&adapter),
        ..Default::default()
    }))
    .map_err(|e| format!("request_device: {e}"))?;
    startup_mark("device");
    let gpu_errors = GpuErrors::install(&device);

    let caps = surface.get_capabilities(&adapter);
    // The viewer writes display-encoded values: draw through a non-sRGB view. Prefer a non-sRGB
    // surface format; otherwise reinterpret an sRGB one (view_formats).
    let surface_format = caps
        .formats
        .iter()
        .copied()
        .find(|f| x3d_render::gpu::target_format_ok(*f))
        .or_else(|| {
            caps.formats
                .iter()
                .copied()
                .find(|f| x3d_render::gpu::target_format_ok(f.remove_srgb_suffix()))
        })
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
    let present_mode = if caps.present_modes.contains(&requested) {
        requested
    } else {
        wgpu::PresentMode::Fifo
    };
    // The picture is opaque; do not let the compositor blend it with what is behind the window.
    let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
        wgpu::CompositeAlphaMode::Opaque
    } else {
        caps.alpha_modes[0]
    };
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode,
        alpha_mode,
        view_formats: if view_format != surface_format {
            vec![view_format]
        } else {
            vec![]
        },
        desired_maximum_frame_latency: 2,
        color_space: wgpu::SurfaceColorSpace::Auto,
    };
    surface.configure(&device, &config);

    if std::env::var("X3D_VIEW_STATS").is_ok() {
        eprintln!(
            "adapter {} ({:?}, {:?}); surface {:?} -> view {:?}; present {:?} of {:?}; alpha {:?} of {:?}",
            adapter_info.name,
            adapter_info.backend,
            adapter_info.device_type,
            surface_format,
            view_format,
            present_mode,
            caps.present_modes,
            alpha_mode,
            caps.alpha_modes
        );
    }

    startup_mark("surface");
    let mut viewer = backend::create_viewer(
        &device,
        &queue,
        (config.width, config.height),
        &adapter,
        view_format,
    )?;
    startup_mark("renderer");
    viewer.renderer().clear_color = [0.0, 0.0, 0.0, 1.0];

    if let Some(n) = opts.msaa {
        viewer.set_multisampling(n);
    }

    if let Some(m) = opts.depth {
        viewer.set_depth_mode(m);
    }

    if std::env::var("X3D_VIEW_STATS").is_ok() {
        eprintln!(
            "multisampling {} (supported {:?})",
            viewer.multisampling(),
            viewer.supported_multisampling()
        );
    }

    Ok(Gfx {
        window,
        surface,
        config,
        view_format,
        device,
        queue,
        adapter_info,
        gpu_errors,
        viewer,
    })
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

        let t0 = self
            .opts
            .script
            .as_ref()
            .map(|s| s["t0"].as_f64().unwrap_or(946684800.0));

        if let Err(e) = gfx.viewer.load(
            &self.opts.path,
            LoadOptions {
                root: self.opts.oracle.clone(),
                start_time: t0,
            },
        ) {
            return self.fail(
                event_loop,
                format!("load {}: {e}", self.opts.path.display()),
            );
        }

        startup_mark("load");

        // The script's warm-up frames (frames the browser draws while loading), on its clock.
        if let (Some(s), Some(t0)) = (&self.opts.script, t0) {
            if let Some(v) = gfx.viewer.core() {
                v.frame_rate = 1.0 / s["dt"].as_f64().unwrap_or(1.0 / 60.0);

                for _ in 0..s["warmup"].as_u64().unwrap_or(8) {
                    v.step(t0);
                }
            }
        }

        startup_mark("warmup");
        let window = gfx.window.clone();
        self.gfx = Some(gfx);
        self.update_title();

        // Windows: show the window when the first frame can be drawn without waiting for the
        // main pipeline (DX12 compiles it while the renderer is set up and the world loads).
        if cfg!(windows) {
            if let Some(g) = &mut self.gfx {
                g.viewer.renderer().finish_prepared();
            }

            startup_mark("prepared");
            window.set_visible(true);
        }

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
                self.input(UiEvent::Mods {
                    shift: self.mods.shift_key(),
                    ctrl: self.mods.control_key(),
                    alt: self.mods.alt_key(),
                });
            }
            WindowEvent::CursorMoved { position, .. } => {
                let h = self
                    .gfx
                    .as_ref()
                    .map(|g| g.viewer.size().1 as f64)
                    .unwrap_or(0.0);
                self.cursor = (position.x, h - position.y);
                self.input(UiEvent::MouseMove {
                    x: self.cursor.0,
                    y: self.cursor.1,
                });
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let b = match button {
                    MouseButton::Left => 0,
                    MouseButton::Middle => 1,
                    MouseButton::Right => 2,
                    _ => return,
                };
                let (x, y) = self.cursor;
                self.input(if state == ElementState::Pressed {
                    UiEvent::MouseDown { button: b, x, y }
                } else {
                    UiEvent::MouseUp { button: b, x, y }
                });

                // The second release of a double click fires the DOM "dblclick" event (LOOKAT, PLANE).
                if b == 0 && state == ElementState::Released {
                    let now = std::time::Instant::now();

                    match self.last_left_up.take() {
                        Some((t, px, py))
                            if now.duration_since(t).as_millis() < 500
                                && (px - x).abs() <= 4.0
                                && (py - y).abs() <= 4.0 =>
                        {
                            self.input(UiEvent::DblClick { x, y })
                        }
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
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                let pressed = event.state == ElementState::Pressed;
                let action = self.keys.event(
                    Platform::current(),
                    pressed,
                    event.repeat,
                    is_synthetic,
                    &event.logical_key,
                    &event.key_without_modifiers(),
                    &event.physical_key,
                    self.mods,
                );
                self.key_action(event_loop, action);
            }
            WindowEvent::Focused(false) => {
                // The browser element would keep a held key down forever; the window releases it.
                for action in self.keys.release_all() {
                    self.key_action(event_loop, action);
                }
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }
}

/// Run the window on the calling thread (a large-stack thread: QuickJS needs it).
pub fn run(opts: Options) -> Result<u64, String> {
    startup_mark("run");
    let mut builder = EventLoop::builder();

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use winit::platform::wayland::EventLoopBuilderExtWayland;
        use winit::platform::x11::EventLoopBuilderExtX11;

        EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
        EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
    }

    #[cfg(windows)]
    winit::platform::windows::EventLoopBuilderExtWindows::with_any_thread(&mut builder, true);

    let event_loop = builder.build().map_err(|e| format!("event loop: {e}"))?;
    event_loop.set_control_flow(ControlFlow::Poll);
    startup_mark("event loop");
    let mut app = ViewerWindow::new(opts);
    event_loop
        .run_app(&mut app)
        .map_err(|e| format!("run: {e}"))?;
    app.print_stats();
    let pending = app.gfx.as_ref().and_then(|g| g.gpu_errors.take());

    match (first_error(app.exit_error, pending), &app.scripted) {
        (Some(e), _) => Err(e),
        // Closed (window manager, Quit) before the last frame: the run did not happen.
        (None, Some(s)) if !s.complete() => Err(format!(
            "scripted run: the event loop ended after {} of {} frames",
            s.done, s.frames
        )),
        (None, _) => Ok(app.presented),
    }
}

/// The error that ends the run: the one it stopped for, else a GPU error raised after the last
/// checked frame (the final screenshot, or just before the window closed).
fn first_error(exit_error: Option<String>, pending_gpu_error: Option<String>) -> Option<String> {
    exit_error.or_else(|| pending_gpu_error.map(|e| format!("GPU error: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_gpu_error_after_the_last_frame_fails_the_run() {
        assert_eq!(first_error(None, None), None);
        assert_eq!(
            first_error(None, Some("lost".into())).as_deref(),
            Some("GPU error: lost")
        );
        assert_eq!(
            first_error(Some("screenshot: x".into()), Some("lost".into())).as_deref(),
            Some("screenshot: x")
        );
    }

    /// A window whose surface is not ready on the listed acquire attempts (1-based), or never.
    #[derive(Default)]
    struct FakeHost {
        not_ready: Vec<u32>,
        never_ready: bool,
        render_error_at: Option<f64>,
        attempts: u32,
        /// Events and steps in the order the world saw them.
        log: Vec<String>,
        steps: Vec<f64>,
    }

    impl ScriptedHost for FakeHost {
        type Frame = ();

        fn acquire(&mut self) -> Result<(), String> {
            self.attempts += 1;

            if self.never_ready || self.not_ready.contains(&self.attempts) {
                Err("surface outdated".into())
            } else {
                Ok(())
            }
        }

        fn event(&mut self, date_now: f64, e: &Value) {
            self.log.push(format!("event {} at {date_now}", e["id"]));
        }

        fn step_and_present(&mut self, (): (), now: f64) -> Result<(), String> {
            self.log.push(format!("step {now}"));
            self.steps.push(now);

            if self.render_error_at == Some(now) {
                Err("x3d-render: empty target".into())
            } else {
                Ok(())
            }
        }
    }

    fn script() -> Value {
        json!({ "t0": 100.0, "dt": 0.5, "frames": 4, "events": [
            { "frame": 2, "t": 0.25, "id": "a" },
            { "frame": 2, "t": 0.5, "id": "b" },
            { "frame": 4, "t": 0.0, "id": "c" },
        ] })
    }

    /// Redraw until the run finishes or fails; bounded, whatever the run does.
    fn drive(run: &mut ScriptedRun, host: &mut FakeHost) -> Vec<Tick> {
        let mut ticks = Vec::new();

        for _ in 0..10_000 {
            let t = run.tick(host);
            let end = matches!(t, Tick::Finished | Tick::Failed(_));
            ticks.push(t);

            if end {
                return ticks;
            }
        }

        panic!("the scripted run neither finished nor failed");
    }

    const EXPECTED: [&str; 7] = [
        "step 100.5",
        "event \"a\" at 100.75",
        "event \"b\" at 101",
        "step 101",
        "step 101.5",
        "event \"c\" at 101.5",
        "step 102",
    ];

    #[test]
    fn every_frame_is_stepped_and_presented() {
        let mut run = ScriptedRun::new(&script());
        let mut host = FakeHost::default();
        assert_eq!(drive(&mut run, &mut host).last(), Some(&Tick::Finished));
        assert_eq!(host.log, EXPECTED);
        assert!(run.complete());
    }

    #[test]
    fn a_surface_that_is_not_ready_does_not_count_as_a_frame() {
        // Not ready on the 2nd and 3rd attempts (frame 2, which has events) and the 6th (frame 4).
        let mut run = ScriptedRun::new(&script());
        let mut host = FakeHost {
            not_ready: vec![2, 3, 6],
            ..Default::default()
        };
        let ticks = drive(&mut run, &mut host);

        assert_eq!(ticks.last(), Some(&Tick::Finished));
        assert_eq!(ticks.iter().filter(|t| **t == Tick::Retry).count(), 3);
        // No fewer steps than frames, each event once, at its frame, before that frame's step.
        assert_eq!(host.steps.len(), 4);
        assert_eq!(host.log, EXPECTED);
        assert_eq!(run.done, 4);
    }

    #[test]
    fn a_surface_that_never_becomes_ready_fails_the_run() {
        let mut run = ScriptedRun::new(&script());
        let mut host = FakeHost {
            never_ready: true,
            ..Default::default()
        };
        let ticks = drive(&mut run, &mut host);

        let Some(Tick::Failed(e)) = ticks.last() else {
            panic!("the run did not fail: {:?}", ticks.last())
        };
        assert!(
            e.contains("frame 1 of 4") && e.contains("surface outdated"),
            "{e}"
        );
        assert_eq!(host.attempts, MAX_SURFACE_MISSES);
        assert!(
            host.log.is_empty(),
            "the world moved without a surface: {:?}",
            host.log
        );
        assert!(!run.complete());
    }

    #[test]
    fn a_render_error_fails_the_run() {
        let mut run = ScriptedRun::new(&script());
        let mut host = FakeHost {
            render_error_at: Some(101.0),
            ..Default::default()
        };
        let ticks = drive(&mut run, &mut host);

        let Some(Tick::Failed(e)) = ticks.last() else {
            panic!("the run did not fail: {:?}", ticks.last())
        };
        assert!(
            e.contains("frame 2 of 4") && e.contains("empty target"),
            "{e}"
        );
        assert_eq!(run.done, 1);
        assert!(!run.complete());
    }

    #[test]
    fn misses_are_counted_consecutively() {
        // MAX - 1 misses before every frame: slow, but never stuck.
        let n = MAX_SURFACE_MISSES - 1;
        let not_ready = (0..4)
            .flat_map(|f| (1..=n).map(move |i| f * (n + 1) + i))
            .collect();
        let mut run = ScriptedRun::new(&script());
        let mut host = FakeHost {
            not_ready,
            ..Default::default()
        };
        assert_eq!(drive(&mut run, &mut host).last(), Some(&Tick::Finished));
        assert_eq!(host.log, EXPECTED);
    }

    #[test]
    fn zero_frames_still_draws_one() {
        let mut run = ScriptedRun::new(&json!({ "t0": 0.0, "dt": 1.0, "frames": 0 }));
        let mut host = FakeHost {
            not_ready: vec![1],
            ..Default::default()
        };
        assert_eq!(drive(&mut run, &mut host), [Tick::Retry, Tick::Finished]);
        assert_eq!(host.steps, [1.0]);
    }
}
