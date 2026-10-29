//! The embedding API: a viewer a host application drives, on the host's wgpu device.
//!
//! [`EmbeddedViewer`] owns the world, the navigation and the renderer, and nothing about
//! windows. The host:
//!
//! 1. creates the viewer with its own `wgpu::Device` and `wgpu::Queue`
//!    ([`EmbeddedViewer::new`]);
//! 2. loads a world ([`EmbeddedViewer::load`], [`EmbeddedViewer::reload`]);
//! 3. forwards pointer, wheel and key events as [`UiEvent`]s (pixel coordinates, origin at the
//!    bottom left of the canvas; no toolkit types);
//! 4. calls [`EmbeddedViewer::frame`] once per redraw with a target texture view, its format
//!    and its size. The viewer steps the world to `now`, builds the frame and draws it into the
//!    view; no CPU readback.
//!
//! The target format must be a non-sRGB color format (`Bgra8Unorm`, `Rgba8Unorm`, ...): the
//! shaders write the display-encoded values X_ITE's WebGL framebuffer holds. When a surface
//! only offers an sRGB format, configure it with the non-sRGB sibling in `view_formats` and
//! pass a view of that format (see `x3d-view`'s window code).
//!
//! Threading: the viewer is `!Send` (Rc-based scene data) and runs scripts in QuickJS, which
//! needs a large stack. Own it on one thread, and run that thread with at least 256 MB of
//! stack ([`spawn_big_stack`]).

use crate::viewer::{UiEvent, Viewer3d};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use x3d_fields::NodeId;
use x3d_render::depth_gpu::DepthGpu;
pub use x3d_render::gpu::DepthMode;
use x3d_render::gpu::Renderer;
use x3d_render::pointing_gpu::PointingGpu;
use x3d_render::traverse::{BrowserGlobals, browser_globals, set_browser_globals};

/// X_ITE's default `Multisampling` rendering property (RenderingProperties.js).
pub const DEFAULT_MULTISAMPLING: u32 = 4;

/// Seconds since the Unix epoch: the browser clock (`getCurrentTime`) of a live viewer.
pub fn realtime_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Run `f` on a thread with the stack QuickJS needs and wait for its result.
pub fn spawn_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(f)
        .expect("spawn viewer thread")
        .join()
        .expect("viewer thread panicked")
}

/// Where a world's relative files (Inline, textures, Script urls) are served from.
#[derive(Clone, Debug, Default)]
pub struct LoadOptions {
    /// Directory that relative URLs resolve inside; the world's own directory tree if `None`
    /// (an ancestor named `oracle` is used when present, like the oracle tools).
    pub root: Option<PathBuf>,
    /// The browser clock at load (seconds); the real time if `None`. Scripted runs pass theirs.
    pub start_time: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct ViewpointInfo {
    pub index: usize,
    pub description: String,
    pub bound: bool,
}

/// What one [`EmbeddedViewer::frame`] cost.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameInfo {
    /// CPU milliseconds: stepping the world (events, scripts, navigation, sensors).
    pub step_ms: f64,
    /// CPU milliseconds: traversing the scene into a frame (geometry is rebuilt only when
    /// changed).
    pub build_ms: f64,
    /// CPU milliseconds: encoding and submitting the GPU work.
    pub render_ms: f64,
    /// GPU milliseconds of the most recent finished render pass, when the device supports
    /// timestamp queries (`Features::TIMESTAMP_QUERY`); not blocking, so it trails by a frame.
    pub gpu_ms: Option<f64>,
    pub draw_calls: usize,
    /// Geometries / textures uploaded to the GPU this frame (0 in steady state).
    pub geometry_uploads: usize,
    pub texture_uploads: usize,
}

pub struct EmbeddedViewer {
    core: Option<Viewer3d>,
    renderer: Renderer,
    source: Option<(PathBuf, LoadOptions)>,
    size: (u32, u32),
    /// Browser.print output of unloaded worlds that the host has not taken yet.
    closed_prints: Vec<String>,
    /// The thread-wide browser options (qualities, logarithmic depth) of this viewer's world: saved when a
    /// call returns and restored when the next one starts, so viewers on one thread do not see each other's.
    globals: BrowserGlobals,
    /// Picks and collision on the renderer's device ([`EmbeddedViewer::set_gpu_picking`]).
    gpu_picking: bool,
    /// The pointing and collision depth buffers on the renderer's device while no world has them: made once, lent
    /// to each world the viewer loads and taken back when it is unloaded. Every viewer has its own.
    gpu: Option<(PointingGpu, Rc<RefCell<DepthGpu>>)>,
    /// The loaded world has the GPU buffers.
    gpu_lent: bool,
    /// A world lost its GPU buffers to a lost device: later worlds pick and collide on the CPU.
    gpu_lost: bool,
}

/// The host drops the viewer: its world is unloaded like by [`EmbeddedViewer::unload`].
impl Drop for EmbeddedViewer {
    fn drop(&mut self) {
        self.close_world();
    }
}

impl EmbeddedViewer {
    /// A viewer drawing on the host's device. `size` is the initial canvas size in pixels.
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, size: (u32, u32)) -> Self {
        Self::with_renderer(Renderer::new(device, queue), size)
    }

    /// [`EmbeddedViewer::new`] for a host that draws into targets of `format`: the renderer starts
    /// creating its most common pipeline for them right away (see [`Renderer::new_preparing`]).
    pub fn new_preparing(
        device: wgpu::Device,
        queue: wgpu::Queue,
        size: (u32, u32),
        format: wgpu::TextureFormat,
    ) -> Self {
        Self::with_renderer(
            Renderer::new_preparing(device, queue, format, DEFAULT_MULTISAMPLING),
            size,
        )
    }

    fn with_renderer(mut renderer: Renderer, size: (u32, u32)) -> Self {
        // BrowserOptions / RenderingProperties: Multisampling defaults to 4 (clamped to the
        // device maximum, which is 4 here until `configure_samples` has seen the adapter).
        renderer.set_sample_count(DEFAULT_MULTISAMPLING);
        EmbeddedViewer {
            core: None,
            renderer,
            source: None,
            size: (size.0.max(1), size.1.max(1)),
            closed_prints: vec![],
            globals: BrowserGlobals::default(),
            gpu_picking: true,
            gpu: None,
            gpu_lent: false,
            gpu_lost: false,
        }
    }

    /// Picks (the pointing buffer) and avatar collision (the 16x16 depth buffer) on the renderer's device, like
    /// X_ITE's; the default. `false`: the CPU ray cast and depth stand-in. Takes effect with the next load. A lost
    /// device switches to the CPU on its own.
    pub fn set_gpu_picking(&mut self, on: bool) {
        self.gpu_picking = on;

        if !on {
            self.gpu = None;
        }
    }

    /// What [`EmbeddedViewer::set_gpu_picking`] was asked for, not what the viewer is doing: this stays `true` after a
    /// lost device or buffers that could not be made, while the picks and collisions have fallen back to the CPU. The
    /// buffers answer that — [`Viewer3d::pointing_gpu`] and [`Viewer3d::depth_gpu`] of [`EmbeddedViewer::core`].
    pub fn gpu_picking(&self) -> bool {
        self.gpu_picking
    }

    /// Lend the GPU buffers to the loaded world.
    fn attach_gpu(&mut self) {
        let Some(core) = self.core.as_mut() else {
            return;
        };

        if !self.gpu_picking || self.gpu_lost {
            return;
        }

        let (device, queue) = (self.renderer.device(), self.renderer.queue());
        let made = match self.gpu.take() {
            Some(g) => Some(g),
            // A device that cannot make their pipelines (float32 targets): the CPU paths.
            None => {
                let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
                let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
                let g = (
                    PointingGpu::new(device.clone(), queue.clone()),
                    Rc::new(RefCell::new(DepthGpu::new(device.clone(), queue.clone()))),
                );
                let internal = ready(internal.pop());
                let validation = ready(validation.pop());
                internal.or(validation).is_none().then_some(g)
            }
        };
        let Some((pointing, depth)) = made else {
            self.gpu_lost = true;
            return;
        };
        core.set_gpu(device, Some(pointing), Some(depth))
            .expect("the renderer's device");
        self.gpu_lent = true;
    }

    /// Take the GPU buffers back from the world that is unloaded; the depth buffer frees the world's vertex buffers. A
    /// world that dropped them (or holds them lost) found the device lost: the viewer makes no new ones.
    fn detach_gpu(&mut self, core: &mut Viewer3d) {
        if !std::mem::take(&mut self.gpu_lent) {
            return;
        }

        match core.take_gpu() {
            (Some(pointing), Some(depth)) if !pointing.lost() && !depth.borrow().lost() => {
                if self.gpu_picking {
                    depth.borrow_mut().release();
                    self.gpu = Some((pointing, depth));
                }
            }
            _ => self.gpu_lost = true,
        }
    }

    /// Start of a call that runs or builds the world: this viewer's thread-wide options.
    fn enter(&self) {
        set_browser_globals(self.globals);
    }

    /// End of such a call: keep what the world set.
    fn leave(&mut self) {
        self.globals = browser_globals();
    }

    /// Tell the viewer which multisample counts the adapter supports for the host's target
    /// `format` (2, 8 and 16 need the device to have been created with
    /// [`EmbeddedViewer::wanted_features`]); without this call only 1 and 4 are used.
    pub fn configure_samples(&mut self, adapter: &wgpu::Adapter, format: wgpu::TextureFormat) {
        let supported = Renderer::supported_sample_counts(adapter, self.renderer.device(), format);
        self.renderer.set_supported_samples(supported);
    }

    /// X_ITE's `Multisampling` (0 or 1 = antialiasing off; default 4). The viewer uses the highest
    /// supported count not above the request, like X_ITE clamps to `getMaxSamples ()`.
    pub fn set_multisampling(&mut self, samples: u32) {
        self.renderer.set_sample_count(samples);
    }

    /// The sample count in use (after clamping).
    pub fn multisampling(&self) -> u32 {
        self.renderer.sample_count()
    }

    /// Supported counts, ascending.
    pub fn supported_multisampling(&self) -> &[u32] {
        self.renderer.supported_samples()
    }

    /// Step to the next supported count (wraps to 1); returns the new count.
    pub fn cycle_multisampling(&mut self) -> u32 {
        self.renderer.cycle_sample_count()
    }

    /// Depth representation (default [`DepthMode::Standard`], X_ITE's). `Logarithmic` is X_ITE's
    /// `LogarithmicDepthBuffer` property (far plane 1e10); `Reversed` is a beyond-X_ITE
    /// reversed-Z 32-bit float depth buffer that removes most z-fighting of near-coplanar
    /// surfaces. Set it before `load` so a "view all" far plane is computed for the mode.
    pub fn set_depth_mode(&mut self, mode: DepthMode) {
        self.enter();
        self.renderer.set_depth_mode(mode);
        self.leave();
    }

    pub fn depth_mode(&self) -> DepthMode {
        self.renderer.depth_mode()
    }

    /// Standard -> Reversed -> Logarithmic -> Standard; returns the new mode.
    pub fn cycle_depth_mode(&mut self) -> DepthMode {
        let next = match self.renderer.depth_mode() {
            DepthMode::Standard => DepthMode::Reversed,
            DepthMode::Reversed => DepthMode::Logarithmic,
            DepthMode::Logarithmic => DepthMode::Standard,
        };
        self.enter();
        self.renderer.set_depth_mode(next);
        self.leave();
        next
    }

    /// The device features that make [`FrameInfo::gpu_ms`] available; add them to the device
    /// you create if the adapter has them.
    pub fn wanted_features(adapter: &wgpu::Adapter) -> wgpu::Features {
        Renderer::wanted_features(adapter)
    }

    // --- world ---------------------------------------------------------------------------

    /// Load a world (replacing the current one). The current world is unloaded first, also when
    /// the new one fails to load: then no world is loaded.
    pub fn load(&mut self, path: &Path, options: LoadOptions) -> Result<(), String> {
        self.close_world();
        self.source = Some((path.to_path_buf(), options.clone()));

        // A host page has no `<x3d-canvas>` attributes: the BrowserOptions keep X_ITE's defaults.
        self.enter();
        let v = Viewer3d::load_attributes(
            path,
            options.root.as_deref(),
            options.start_time.unwrap_or_else(realtime_now),
            self.size.0,
            self.size.1,
            None,
            false,
            &[],
        );
        self.leave();
        self.core = Some(v?);
        self.attach_gpu();
        Ok(())
    }

    /// Unload the current world, if any (ISO/IEC 14772-1 4.12.3: its Scripts get `shutdown ()`,
    /// [`x3d_runtime::Runtime::unload_world`]), then drop it. This comes before anything creates
    /// the next world's script engine, which takes over the thread's script state. What the
    /// Scripts print in `shutdown ()` is kept for [`EmbeddedViewer::take_prints`].
    fn close_world(&mut self) {
        if let Some(mut c) = self.core.take() {
            self.enter();
            c.rt.unload_world();
            self.leave();
            self.closed_prints.append(&mut c.rt.prints);
            self.detach_gpu(&mut c);
        }
    }

    /// Load the same file again (the file changed on disk), keeping the navigation type.
    pub fn reload(&mut self) -> Result<(), String> {
        let Some((path, options)) = self.source.clone() else {
            return Err("nothing loaded".into());
        };
        let keep = self.core.as_ref().map(|c| c.viewer_kind().to_string());
        self.load(&path, options)?;

        if let (Some(kind), Some(c)) = (keep, self.core.as_mut())
            && kind != "NONE"
        {
            c.set_viewer(&kind);
        }

        Ok(())
    }

    pub fn unload(&mut self) {
        self.close_world();
        self.source = None;
    }

    pub fn is_loaded(&self) -> bool {
        self.core.is_some()
    }

    pub fn path(&self) -> Option<&Path> {
        self.source.as_ref().map(|s| s.0.as_path())
    }

    // --- canvas and input ------------------------------------------------------------------

    /// The canvas changed size (pixels). `frame` also resizes to its target.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.size = (width.max(1), height.max(1));

        if let Some(c) = self.core.as_mut() {
            c.resize(self.size.0, self.size.1);
        }
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Forward an input event (pointer, wheel, key, modifiers).
    pub fn input(&mut self, event: UiEvent) {
        if let Some(c) = self.core.as_mut() {
            set_browser_globals(self.globals);
            c.input(event);
            self.globals = browser_globals();
        }
    }

    // --- navigation and viewpoints -------------------------------------------------------

    /// "EXAMINE", "WALK", "FLY", "LOOKAT", "PLANE" or "NONE".
    pub fn navigation_type(&self) -> &str {
        self.core
            .as_ref()
            .map(|c| c.viewer_kind())
            .unwrap_or("NONE")
    }

    /// The navigation types the world's NavigationInfo allows ("ANY" allows all).
    pub fn available_navigation_types(&self) -> Vec<String> {
        self.core
            .as_ref()
            .map(|c| c.available_viewers())
            .unwrap_or_default()
    }

    pub fn set_navigation_type(&mut self, kind: &str) {
        if let Some(c) = self.core.as_mut() {
            c.set_viewer(kind);
        }
    }

    /// The world's viewpoints that have a description (the ones a viewer menu lists).
    pub fn viewpoints(&self) -> Vec<ViewpointInfo> {
        let Some(c) = self.core.as_ref() else {
            return vec![];
        };
        c.user_viewpoints()
            .iter()
            .enumerate()
            .map(|(index, n)| ViewpointInfo {
                index,
                description: c.viewpoint_description(*n),
                bound: c.rt.get_bool(*n, "isBound"),
            })
            .collect()
    }

    pub fn bind_viewpoint(&mut self, index: usize) {
        if let Some(c) = self.core.as_mut()
            && let Some(n) = c.user_viewpoints().get(index).copied()
        {
            c.bind_viewpoint(n);
        }
    }

    /// `1` next, `-1` previous (wraps).
    pub fn step_viewpoint(&mut self, delta: i32) {
        if let Some(c) = self.core.as_mut() {
            c.next_viewpoint(delta);
        }
    }

    pub fn first_viewpoint(&mut self) {
        if let Some(c) = self.core.as_mut() {
            c.first_viewpoint();
        }
    }

    pub fn last_viewpoint(&mut self) {
        if let Some(c) = self.core.as_mut() {
            c.last_viewpoint();
        }
    }

    // --- status ----------------------------------------------------------------------------

    /// The world's WorldInfo title, or empty.
    pub fn title(&self) -> String {
        self.core.as_ref().map(|c| c.title()).unwrap_or_default()
    }

    /// Browser.description, sensor descriptions, Anchor reports, errors: one status line.
    pub fn status(&self) -> &str {
        self.core.as_ref().map(|c| c.status.as_str()).unwrap_or("")
    }

    /// Console log of the world (warnings and errors from loading and scripts).
    pub fn log(&self) -> &[String] {
        self.core.as_ref().map(|c| c.log()).unwrap_or(&[])
    }

    /// Browser.print output since the last call (a world that was unloaded since comes first).
    pub fn take_prints(&mut self) -> Vec<String> {
        let mut out = std::mem::take(&mut self.closed_prints);

        if let Some(c) = self.core.as_mut() {
            out.append(&mut c.rt.prints);
        }

        out
    }

    /// Anchor activations since the last call (urls and parameters; the viewer never opens them).
    pub fn take_anchor_activations(&mut self) -> Vec<crate::pointer::AnchorActivation> {
        self.core
            .as_mut()
            .map(|c| std::mem::take(&mut c.anchors))
            .unwrap_or_default()
    }

    /// The core viewer, for everything the API above does not cover.
    pub fn core(&mut self) -> Option<&mut Viewer3d> {
        self.core.as_mut()
    }

    pub fn viewpoint_node(&self, index: usize) -> Option<NodeId> {
        self.core.as_ref()?.user_viewpoints().get(index).copied()
    }

    pub fn renderer(&mut self) -> &mut Renderer {
        &mut self.renderer
    }

    // --- frames ----------------------------------------------------------------------------

    /// Step the world to `now` (seconds, see [`realtime_now`]).
    pub fn step(&mut self, now: f64) {
        if let Some(c) = self.core.as_mut() {
            // The MaximumFrameRate option (advanceTime): a frame that comes too soon is not stepped.
            if !c.frame_due(now) {
                return;
            }

            c.date_now = now;
            set_browser_globals(self.globals);
            c.step(now);
            self.globals = browser_globals();

            // Browser.dispose () from a Script: the world goes as on unload (its Scripts' shutdown runs).
            if c.disposed {
                self.unload();
            }
        }
    }

    /// The size a Script asked for (Browser.resize), once: the host resizes the canvas.
    pub fn take_resize_request(&mut self) -> Option<(u32, u32)> {
        self.core.as_mut().and_then(|c| c.resize_request.take())
    }

    /// Step the world to `now` and draw it into `target`. With no world loaded the target is
    /// cleared. Returns the frame's costs.
    pub fn frame(
        &mut self,
        now: f64,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Result<FrameInfo, String> {
        if (width, height) != self.size {
            self.resize(width, height);
        }

        let t0 = Instant::now();
        self.step(now);
        let t1 = Instant::now();
        let mut info = FrameInfo {
            step_ms: (t1 - t0).as_secs_f64() * 1e3,
            ..Default::default()
        };

        let Some(core) = self.core.as_mut() else {
            self.renderer
                .render(&empty_frame(), target, format, width, height)?;
            return Ok(info);
        };

        // A Script's setBrowserOption changed Multisampling, LogarithmicDepthBuffer or Shading.
        set_browser_globals(self.globals);
        core.take_rendering_changes().apply(&mut self.renderer);

        let frame = core.build_frame();
        self.globals = browser_globals();
        let t2 = Instant::now();
        info.build_ms = (t2 - t1).as_secs_f64() * 1e3;
        self.renderer
            .render(&frame, target, format, width, height)?;
        info.render_ms = t2.elapsed().as_secs_f64() * 1e3;
        info.gpu_ms = self.renderer.gpu_time_ms(false);
        info.draw_calls = self.renderer.stats.draw_calls;
        info.geometry_uploads = self.renderer.stats.geometry_uploads;
        info.texture_uploads = self.renderer.stats.texture_uploads;
        Ok(info)
    }

    /// Draw the current state (no stepping) into an internal texture and read it back as RGBA8,
    /// top row first: screenshots. Not for the interactive path.
    pub fn screenshot_rgba(&mut self, width: u32, height: u32) -> Option<Vec<u8>> {
        let core = self.core.as_mut()?;
        set_browser_globals(self.globals);
        let frame = core.build_frame();
        self.globals = browser_globals();
        Some(self.renderer.render_to_rgba(&frame, width, height))
    }
}

fn empty_frame() -> x3d_render::traverse::Frame {
    x3d_render::traverse::Frame {
        projection: x3d_fields::math::Matrix4::IDENTITY,
        view: x3d_fields::math::Matrix4::IDENTITY,
        items: vec![],
        lights: vec![],
        background: None,
        fog: None,
        unsupported: Default::default(),
        viewpoint: None,
        cube_passes: vec![],
        rendered_passes: vec![],
        environment: None,
        tone_mapping: 0,
        color_space: 2,
        exposure: 1.0,
        viewport: None,
        layers: vec![],
        shadow_maps: vec![],
        projectors: vec![],
    }
}

/// The output of `f`, which native wgpu has ready (an error scope's `pop`).
fn ready<F: std::future::Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());

    loop {
        if let std::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }

        std::thread::yield_now();
    }
}
