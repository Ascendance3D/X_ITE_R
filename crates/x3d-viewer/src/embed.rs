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
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use x3d_fields::NodeId;
use x3d_render::gpu::Renderer;

/// Seconds since the Unix epoch: the browser clock (`getCurrentTime`) of a live viewer.
pub fn realtime_now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// Run `f` on a thread with the stack QuickJS needs and wait for its result.
pub fn spawn_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new().stack_size(256 << 20).spawn(f).expect("spawn viewer thread").join().expect("viewer thread panicked")
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
}

impl EmbeddedViewer {
    /// A viewer drawing on the host's device. `size` is the initial canvas size in pixels.
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, size: (u32, u32)) -> Self {
        EmbeddedViewer { core: None, renderer: Renderer::new(device, queue), source: None, size: (size.0.max(1), size.1.max(1)) }
    }

    /// The device features that make [`FrameInfo::gpu_ms`] available; add them to the device
    /// you create if the adapter has them.
    pub fn wanted_features(adapter: &wgpu::Adapter) -> wgpu::Features {
        Renderer::wanted_features(adapter)
    }

    // --- world ---------------------------------------------------------------------------

    /// Load a world (replacing the current one).
    pub fn load(&mut self, path: &Path, options: LoadOptions) -> Result<(), String> {
        let v = Viewer3d::load(path, options.root.as_deref(), options.start_time.unwrap_or_else(realtime_now), self.size.0, self.size.1)?;
        self.core = Some(v);
        self.source = Some((path.to_path_buf(), options));
        Ok(())
    }

    /// Load the same file again (the file changed on disk), keeping the navigation type.
    pub fn reload(&mut self) -> Result<(), String> {
        let Some((path, options)) = self.source.clone() else { return Err("nothing loaded".into()) };
        let keep = self.core.as_ref().map(|c| c.viewer_kind().to_string());
        self.load(&path, options)?;

        if let (Some(kind), Some(c)) = (keep, self.core.as_mut()) {
            if kind != "NONE" {
                c.set_viewer(&kind);
            }
        }

        Ok(())
    }

    pub fn unload(&mut self) {
        self.core = None;
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
            c.input(event);
        }
    }

    // --- navigation and viewpoints -------------------------------------------------------

    /// "EXAMINE", "WALK", "FLY", "LOOKAT", "PLANE" or "NONE".
    pub fn navigation_type(&self) -> &str {
        self.core.as_ref().map(|c| c.viewer_kind()).unwrap_or("NONE")
    }

    /// The navigation types the world's NavigationInfo allows ("ANY" allows all).
    pub fn available_navigation_types(&self) -> Vec<String> {
        self.core.as_ref().map(|c| c.available_viewers()).unwrap_or_default()
    }

    pub fn set_navigation_type(&mut self, kind: &str) {
        if let Some(c) = self.core.as_mut() {
            c.set_viewer(kind);
        }
    }

    /// The world's viewpoints that have a description (the ones a viewer menu lists).
    pub fn viewpoints(&self) -> Vec<ViewpointInfo> {
        let Some(c) = self.core.as_ref() else { return vec![] };
        c.user_viewpoints().iter().enumerate().map(|(index, n)| ViewpointInfo { index, description: c.viewpoint_description(*n), bound: c.rt.get_bool(*n, "isBound") }).collect()
    }

    pub fn bind_viewpoint(&mut self, index: usize) {
        if let Some(c) = self.core.as_mut() {
            if let Some(n) = c.user_viewpoints().get(index).copied() {
                c.bind_viewpoint(n);
            }
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

    /// Browser.print output since the last call.
    pub fn take_prints(&mut self) -> Vec<String> {
        self.core.as_mut().map(|c| std::mem::take(&mut c.rt.prints)).unwrap_or_default()
    }

    /// Anchor activations since the last call (urls and parameters; the viewer never opens them).
    pub fn take_anchor_activations(&mut self) -> Vec<crate::pointer::AnchorActivation> {
        self.core.as_mut().map(|c| std::mem::take(&mut c.anchors)).unwrap_or_default()
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
            c.date_now = now;
            c.step(now);
        }
    }

    /// Step the world to `now` and draw it into `target`. With no world loaded the target is
    /// cleared. Returns the frame's costs.
    pub fn frame(&mut self, now: f64, target: &wgpu::TextureView, format: wgpu::TextureFormat, width: u32, height: u32) -> Result<FrameInfo, String> {
        if (width, height) != self.size {
            self.resize(width, height);
        }

        let t0 = Instant::now();
        self.step(now);
        let t1 = Instant::now();
        let mut info = FrameInfo { step_ms: (t1 - t0).as_secs_f64() * 1e3, ..Default::default() };

        let Some(core) = self.core.as_mut() else {
            self.renderer.render(&empty_frame(), target, format, width, height)?;
            return Ok(info);
        };

        let frame = core.build_frame();
        let t2 = Instant::now();
        info.build_ms = (t2 - t1).as_secs_f64() * 1e3;
        self.renderer.render(&frame, target, format, width, height)?;
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
        let frame = core.build_frame();
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
    }
}
