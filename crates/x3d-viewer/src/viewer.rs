//! The viewer core: a live world, the per-frame order of X3DBrowserContext [_traverse], the
//! bound viewpoint, the navigation viewer and the pointing device. It runs without a window:
//! the caller supplies the clock (`step (now)`) and the input events, so a run is
//! deterministic.

use crate::collision::{CShape, World};
use crate::layer::{DEFAULT, Layer, NavInfo, VpBase, is_default};
use crate::lookat::Box3;
use crate::nav::{Cx, Env, Input, LookAtRequest, Viewer};
use crate::pointer::{AnchorActivation, Hit, PickCamera, Pointer, pick};
use crate::walk::{LayerWalk, NodeRef, Scan, ShapeInst, rt_at, rt_at_mut, scan, scan_screens};
use std::collections::HashMap;
#[cfg(feature = "script")]
use std::path::Path;
use std::rc::Rc;
use x3d_fields::math::{Matrix4, Rotation4, Vector3, Vector4};
use x3d_fields::{FieldValue, NodeId};
use x3d_render::geometry::Geometry;
use x3d_render::layer::{self as layers, LayerKey, LayoutCamera};
use x3d_render::traverse::{Builder, CameraOverride, Frame};
#[cfg(feature = "script")]
use x3d_runtime::LocalFetcher;
use x3d_runtime::Runtime;
#[cfg(feature = "script")]
use x3d_script::compare::{ORACLE_PREFIX, encode_path};

/// RenderingProperties changes for the renderer ([`Viewer3d::take_rendering_changes`]).
#[derive(Clone, Debug, Default)]
pub struct RenderingChanges {
    pub samples: Option<u32>,
    /// X3DRenderObject.set_renderKey__: the LogarithmicDepthBuffer rendering property or the
    /// bound viewpoint's getLogarithmicDepthBuffer (GeoViewpoint).
    pub log_depth: Option<bool>,
    pub shading: Option<x3d_render::gpu::ShadingMode>,
    /// The LogarithmicDepthBuffer rendering property alone (getMaxFarValue reads it).
    pub property_log_depth: bool,
}

impl RenderingChanges {
    /// Apply the changes to `renderer`.
    pub fn apply(&self, renderer: &mut x3d_render::gpu::Renderer) {
        if let Some(n) = self.samples {
            renderer.set_sample_count(n);
        }

        if let Some(l) = self.log_depth {
            renderer.set_depth_mode(if l {
                x3d_render::gpu::DepthMode::Logarithmic
            } else {
                x3d_render::gpu::DepthMode::Standard
            });
            x3d_render::traverse::set_logarithmic_depth_buffer(self.property_log_depth);
        }

        if let Some(s) = self.shading {
            renderer.set_shading(s);
        }
    }
}

/// Samples, logarithmic depth (`geo`: a GeoViewpoint is bound) and Shading.
fn rendering_state(rt: &Runtime, geo: bool) -> (u32, bool, String) {
    let b = &rt.browser_options;
    (
        b.multisampling(),
        b.logarithmic_depth_buffer() || geo,
        b.rendering
            .get("Shading")
            .map(|v| v.as_str().to_string())
            .unwrap_or_default(),
    )
}

/// The `<x3d-canvas>` attributes of the oracle pages (anim.html, anim-sized.html).
pub const ORACLE_PAGE_ATTRIBUTES: &[(&str, &str)] = &[
    ("antialiased", "false"),
    ("preserveDrawingBuffer", "true"),
    ("splashScreen", "false"),
    ("contextMenu", "false"),
    ("notifications", "false"),
    ("timings", "false"),
    ("contentScale", "1"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Home,
    End,
    PageUp,
    PageDown,
}

/// A user event in viewport pixels (origin bottom left).
#[derive(Clone, Debug)]
pub enum UiEvent {
    MouseDown {
        button: u8,
        x: f64,
        y: f64,
    },
    MouseUp {
        button: u8,
        x: f64,
        y: f64,
    },
    MouseMove {
        x: f64,
        y: f64,
    },
    Wheel {
        delta_y: f64,
        x: f64,
        y: f64,
    },
    /// The "dblclick" DOM event (the harness sends the two mousedown / mouseup pairs before it).
    DblClick {
        x: f64,
        y: f64,
    },
    Key(Key),
    Mods {
        shift: bool,
        ctrl: bool,
        alt: bool,
    },
    /// A DOM keydown / keyup of the browser element: `key` is `event .key`.
    KeyDown {
        key: String,
    },
    KeyUp {
        key: String,
    },
    /// A native keydown / keyup: as `KeyDown` / `KeyUp`, with the host's `event .keyCode` (the
    /// window derives it from the physical key when the key name has none, e.g. other layouts).
    NativeKey {
        down: bool,
        key: String,
        code: u32,
    },
}

#[derive(Default)]
struct ProxState {
    inside: bool,
    traversed: bool,
    registered: bool,
    seen_camera: bool,
    /// The layer whose DISPLAY traversal reached the sensor last (its viewpoint updates it).
    layer: LayerKey,
}

/// What `browser .getActiveViewpoint ()` and `getCurrentViewer ()` answer right after `replaceWorld`: the bindables of
/// the new world are bound, but the browser's `activeViewpoint` and `viewer` fields follow their stacks by events.
struct HeldPose {
    pose: (Vector3, Rotation4, Vector3),
    /// The name of the viewpoint (`<default>` for the default one): its node may be released by now.
    viewpoint: String,
    viewer: String,
}

/// The state of a layer (X3DLayerNode) other than the one in [`Viewer3d::layer`].
pub struct LayerSlot {
    pub key: LayerKey,
    pub layer: Layer,
    pub scan_cam: Scan,
    pub scan_display: Scan,
}

pub struct Viewer3d {
    pub rt: Runtime,
    pub layer: Layer,
    pub nav: Viewer,
    nav_info_node: NodeId,
    /// The bound NavigationInfo's `_viewer` (set_type__) the browser's viewer follows.
    nav_viewer: String,
    /// The browser's `viewer` field (`browser .getCurrentViewer ()`): whatever was set last, the
    /// NavigationInfo's choice or a name given to `set_viewer`, even one that falls back to EXAMINE.
    viewer_field: String,
    pub pointer: Pointer,
    pub width: u32,
    pub height: u32,
    /// browser .getCurrentTime ().
    pub time: f64,
    /// Date.now () / 1000, settable between frames (input events use it).
    pub date_now: f64,
    pub frame_rate: f64,
    /// Seconds spent in the scene walks of [`Self::refresh_display_scan`] and [`Self::display_scan`] since the last
    /// [`Self::take_step_seconds`].
    traversal_seconds: f64,
    last_time: Option<f64>,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub straighten: bool,
    /// X3DTimeContext.advanceOnlyTime on pointer events (a real clock); off for scripted runs.
    pub advance_only_time: bool,
    /// `browser .currentFrameRate` as the scene's followers (X3DDamperNode tau) read it. `None`: the
    /// measured frame interval (X3DTimeContext.advanceTime). Scripted oracle runs pin it to 1 / dt:
    /// the oracle pages stub advanceTime and getCurrentFrameRate, but the SAI property
    /// `currentFrameRate` still returns the wall-clock rate of the page's own early frames, so
    /// oracle/x_ite-anim*.mjs pin that property for scripts with `"pinFrameRate": true`.
    pub pinned_frame_rate: Option<f64>,
    /// The RenderingProperties Multisampling, LogarithmicDepthBuffer and Shading the renderer
    /// was last told about ([`Viewer3d::take_rendering_changes`]).
    synced_rendering: (u32, bool, String),
    /// The LogarithmicDepthBuffer rendering property the camera last used.
    camera_log_depth: bool,
    gravity_speed: f64,
    /// X3DRenderObject.activeCollisions: the Collision nodes whose isActive the last COLLISION traversal set.
    active_collisions: Vec<x3d_fields::NodeId>,
    geom_cache: HashMap<(Vec<NodeId>, NodeId), Option<Rc<Geometry>>>,
    /// The geometry each ParticleSystem last drew into the collision depth buffer, kept between frames while it does not
    /// change, so that the depth buffer keeps its vertex buffer (a SPRITE's changes when the view turns).
    particle_depth_geometry: HashMap<(Vec<NodeId>, NodeId), Rc<Geometry>>,
    /// The ParticleSystems whose transformed LINE triangles a DISPLAY pass made (they keep them,
    /// [`x3d_runtime::particles::Sim::thick_lines`]).
    thick_line_systems: Vec<(Vec<NodeId>, NodeId)>,
    /// The collision geometry (one per particle) of each system's transformed LINE triangles, kept while they are the
    /// same so that the depth buffer keeps their vertex buffers.
    thick_line_depth: HashMap<(Vec<NodeId>, NodeId), ThickLineDepth>,
    /// The pointing buffer on the GPU (float32 hit values like X_ITE's); without it picks use the
    /// ray cast's double values. Private so that the only way in is [`Viewer3d::set_gpu`], which holds both buffers to
    /// the device that draws the frame; read it with [`Viewer3d::pointing_gpu`].
    pointing_gpu: Option<x3d_render::pointing_gpu::PointingGpu>,
    /// The collision depth buffer on the GPU (X_ITE's rasterization of the 16x16 buffer). Private for the same reason;
    /// read it with [`Viewer3d::depth_gpu`].
    depth_gpu: Option<std::rc::Rc<std::cell::RefCell<x3d_render::depth_gpu::DepthGpu>>>,
    /// Wall time of the collision work of [`Viewer3d::step`] (the viewer's constrained moves, collide and gravitate),
    /// added up for benchmarks; the caller resets it. The constrained moves are timed with the whole navigation prepare
    /// around them (the collision world's traversal and the navigation's own small step).
    pub collision_time: std::time::Duration,
    /// The Ammo state of the pick shapes (X3DPickSensorNode.getPickShape).
    pub pick_shapes: crate::pick_shapes::PickShapes,
    /// Geometry and textures kept between frames (rebuilt only when their nodes change).
    pub render_cache: x3d_render::traverse::RenderCache,
    pub scan_cam: Scan,
    pub scan_display: Scan,
    /// LOD frame rate averages and the levels the DISPLAY traversal changed (LOD.changeLevel).
    pub lod_pass: crate::walk::LodPass,
    /// GeneratedCubeMapTextures drawn with update NEXT_FRAME_ONLY in the last frame: renderTexture
    /// sets update to NONE while drawing, the event is processed in the next frame.
    generated_none: Vec<NodeRef>,
    prox: HashMap<(Vec<NodeId>, NodeId), ProxState>,
    vis: HashMap<(Vec<NodeId>, NodeId), ProxState>,
    verify_motion: Option<(f64, f64)>,
    /// Status line text (Browser.description, sensor descriptions, anchor reports, errors).
    pub status: String,
    pub anchors: Vec<AnchorActivation>,
    pub frame_count: u64,
    /// The world the viewer's state is for ([`x3d_runtime::browser_load::WorldState::generation`]).
    world_gen: u64,
    /// The browser description that the status line shows.
    status_description: String,
    /// Anchors clicked and not yet loading: (Anchor, url, parameter).
    pending_anchors: Vec<(NodeId, Vec<String>, Vec<String>)>,
    /// The active viewpoint and viewer of the world that was replaced, until the first events of the new one.
    held_pose: Option<HeldPose>,
    /// A Script called Browser.dispose (): the browser stops (EmbeddedViewer unloads the world).
    pub disposed: bool,
    /// A Script called Browser.resize (width, height): applied here; a host that owns the canvas size takes it
    /// (`EmbeddedViewer::take_resize_request`).
    pub resize_request: Option<(u32, u32)>,
    /// The layer whose state `layer`, `scan_cam` and `scan_display` hold: the active layer
    /// (navigation, viewpoint lists), except while a pass visits the layers one by one.
    current: LayerKey,
    /// The other layers' states (LayerSet).
    others: Vec<LayerSlot>,
    /// LayerSet.getLayers: the layers drawn this frame, in order.
    drawn: Vec<LayerKey>,
    /// X3DWorld.getActiveLayer: the layer navigation acts on (`None`: no displayed layer).
    active: Option<LayerKey>,
    /// X3DPointingDeviceSensorContext.getPointingLayer: the layer of the pressed hit while the
    /// button is down.
    pointing_layer: Option<LayerKey>,
}

/// A system's transformed LINE triangles and their collision shapes: every particle's triangles one after the other,
/// the instance matrices, the vertices of one particle.
type ThickLineDepth = (
    Rc<x3d_runtime::particles::ThickLines>,
    Rc<Geometry>,
    Rc<Vec<Matrix4>>,
    usize,
);

fn geom_of(
    root: &Runtime,
    cache: &mut HashMap<(Vec<NodeId>, NodeId), Option<Rc<Geometry>>>,
    s: &ShapeInst,
) -> Option<Rc<Geometry>> {
    if let Some(g) = &s.particle_geometry {
        return Some(g.clone());
    }

    let key = (s.shape.path.to_vec(), s.geometry);

    if let Some(g) = cache.get(&key) {
        return g.clone();
    }

    let rt = rt_at(root, &s.shape.path)?;
    let g = x3d_render::traverse::geometry_of(rt, s.geometry)
        .filter(|g| g.vertex_count() > 0)
        .map(Rc::new);
    cache.insert(key, g.clone());
    g
}

impl Viewer3d {
    /// Load a world like the oracle's FileLoader (parse, setup, loads) but live, then bind the
    /// first bindables (X3DBrowser.replaceWorld → X3DWorld.bindBindables).
    #[cfg(feature = "script")]
    pub fn load(
        file: &Path,
        oracle_root: Option<&Path>,
        t0: f64,
        width: u32,
        height: u32,
    ) -> Result<Viewer3d, String> {
        Self::load_pinned(file, oracle_root, t0, width, height, None)
    }

    /// [`Viewer3d::load`] with `currentFrameRate` pinned (see [`Viewer3d::pinned_frame_rate`]) from
    /// the first frame on, including the frame that finishes loading.
    #[cfg(feature = "script")]
    pub fn load_pinned(
        file: &Path,
        oracle_root: Option<&Path>,
        t0: f64,
        width: u32,
        height: u32,
        pinned_frame_rate: Option<f64>,
    ) -> Result<Viewer3d, String> {
        Self::load_opts(
            file,
            oracle_root,
            t0,
            width,
            height,
            pinned_frame_rate,
            false,
        )
    }

    /// [`Viewer3d::load_pinned`] that can also record the console output in order
    /// ([`Runtime::console`]) from the first message on.
    #[cfg(feature = "script")]
    pub fn load_opts(
        file: &Path,
        oracle_root: Option<&Path>,
        t0: f64,
        width: u32,
        height: u32,
        pinned_frame_rate: Option<f64>,
        record_console: bool,
    ) -> Result<Viewer3d, String> {
        Self::load_attributes(
            file,
            oracle_root,
            t0,
            width,
            height,
            pinned_frame_rate,
            record_console,
            ORACLE_PAGE_ATTRIBUTES,
        )
    }

    /// [`Viewer3d::load_opts`] with the `<x3d-canvas>` attributes of the page (BrowserOptions.reset
    /// applies them before the world loads), like [`ORACLE_PAGE_ATTRIBUTES`].
    #[cfg(feature = "script")]
    #[allow(clippy::too_many_arguments)]
    pub fn load_attributes(
        file: &Path,
        oracle_root: Option<&Path>,
        t0: f64,
        width: u32,
        height: u32,
        pinned_frame_rate: Option<f64>,
        record_console: bool,
        attributes: &[(&str, &str)],
    ) -> Result<Viewer3d, String> {
        x3d_fields::jsmath::set_flavor(x3d_fields::jsmath::MathFlavor::Chrome);
        // The geometry the ParticleSystems emitters and bounded models sample.
        x3d_render::traverse::install_particle_geometry();

        let file = std::fs::canonicalize(file).map_err(|e| format!("{}: {e}", file.display()))?;
        let found = file
            .ancestors()
            .skip(1)
            .find(|d| d.file_name().is_some_and(|n| n == "oracle"))
            .map(|d| d.to_path_buf());
        let root_buf = match oracle_root {
            Some(r) => std::fs::canonicalize(r).map_err(|e| e.to_string())?,
            None => found.unwrap_or_else(|| Path::new("/").to_path_buf()),
        };
        let root = root_buf.as_path();
        let rel = file
            .strip_prefix(root)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        let url = format!("{ORACLE_PREFIX}{}", encode_path(&rel));
        let data = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let fetcher = Box::new(LocalFetcher {
            root: root.to_path_buf(),
            prefix: ORACLE_PREFIX.to_string(),
        });
        // glTF buffers and images of a top-level world come through the runtime fetcher, as in x3d-render's load.rs.
        let scene = x3d_runtime::parse_bytes_with_fetcher(&*fetcher, &data, &url)
            .scene
            .map_err(|e| format!("{e:?}"))?;
        let mut rt = Runtime::new(scene, fetcher);
        // The oracle page (oracle/x_ite-pin/src/anim.html) is the document the browser lives in.
        rt.page_url = format!("{ORACLE_PREFIX}x_ite-pin/src/anim.html");
        rt.script_host = Some(Box::new(x3d_script::QuickJsHost::new()));
        x3d_render::shader_gpu::install(&mut rt);
        rt.live = true;
        rt.now = t0;
        rt.record_console = record_console;
        rt.browser_options = x3d_runtime::browser_options::BrowserOptions::new(width, height);
        rt.browser_options.apply_attributes(attributes);

        if let Some(rate) = pinned_frame_rate {
            rt.frame_rate = rate;
        }

        rt.load_scene();
        rt.replace_world();

        let mut v = Self::from_runtime(rt, t0, width, height);
        v.pinned_frame_rate = pinned_frame_rate;
        Ok(v)
    }

    pub fn from_runtime(rt: Runtime, t0: f64, width: u32, height: u32) -> Viewer3d {
        let synced_rendering = rendering_state(&rt, false);
        let mut v = Viewer3d {
            camera_log_depth: synced_rendering.1,
            synced_rendering,
            rt,
            layer: Layer::new(),
            nav: Viewer::None,
            nav_info_node: DEFAULT,
            nav_viewer: String::new(),
            viewer_field: String::new(),
            pointer: Pointer::new(),
            width,
            height,
            time: t0,
            date_now: t0,
            frame_rate: 60.0,
            traversal_seconds: 0.0,
            last_time: None,
            shift: false,
            ctrl: false,
            alt: false,
            straighten: true,
            advance_only_time: false,
            pinned_frame_rate: None,
            gravity_speed: 0.0,
            active_collisions: vec![],
            geom_cache: HashMap::new(),
            particle_depth_geometry: HashMap::new(),
            thick_line_systems: vec![],
            thick_line_depth: HashMap::new(),
            pointing_gpu: None,
            depth_gpu: None,
            collision_time: std::time::Duration::ZERO,
            pick_shapes: Default::default(),
            render_cache: Default::default(),
            scan_cam: Scan::default(),
            scan_display: Scan::default(),
            lod_pass: Default::default(),
            generated_none: vec![],
            prox: HashMap::new(),
            vis: HashMap::new(),
            verify_motion: None,
            status: String::new(),
            anchors: vec![],
            frame_count: 0,
            world_gen: 0,
            pending_anchors: vec![],
            status_description: String::new(),
            held_pose: None,
            disposed: false,
            resize_request: None,
            current: None,
            others: vec![],
            drawn: vec![None],
            active: Some(None),
            pointing_layer: None,
        };

        v.layer.viewport = (width as f64, height as f64);
        v.bind_world();
        v
    }

    /// X3DBrowser.setExecutionContext: `new X3DWorld (scene)` has layers of its own and `bindBindables ()` binds the first
    /// viewpoint, navigation info, background and fog of the new world. Everything the viewer remembers of the old world
    /// (stacks, saved viewpoints, sensors, hits, caches keyed by node) goes.
    fn hold_pose(&mut self) -> HeldPose {
        match &self.held_pose {
            Some(h) => HeldPose {
                pose: h.pose,
                viewpoint: h.viewpoint.clone(),
                viewer: h.viewer.clone(),
            },
            None => HeldPose {
                pose: self.user_pose(),
                viewpoint: self.bound_viewpoint_name(),
                viewer: self.viewer_field().to_string(),
            },
        }
    }

    fn reset_world(&mut self, before: Option<HeldPose>) {
        self.world_gen = self.rt.world_state.generation;

        // `browser .getActiveViewpoint ()` and the viewer still name the old world's until the events of the new bindables run.
        self.held_pose = Some(before.unwrap_or_else(|| self.hold_pose()));
        self.nav.disconnect(self.date_now);
        self.nav = Viewer::None;
        self.nav_info_node = DEFAULT;
        self.nav_viewer = String::new();
        self.viewer_field = String::new();
        self.layer = Layer::new();
        self.layer.viewport = (self.width as f64, self.height as f64);
        self.others.clear();
        self.current = None;
        self.drawn = vec![None];
        self.active = Some(None);
        self.pointing_layer = None;
        self.pointer = Pointer::new();
        self.geom_cache.clear();
        self.particle_depth_geometry.clear();
        self.thick_line_systems.clear();
        self.thick_line_depth.clear();
        self.render_cache = Default::default();
        self.pick_shapes = Default::default();
        self.scan_cam = Scan::default();
        self.scan_display = Scan::default();
        self.lod_pass = Default::default();
        self.generated_none.clear();
        self.prox.clear();
        self.vis.clear();
        self.active_collisions.clear();
        self.gravity_speed = 0.0;
        self.verify_motion = None;
        self.bind_world_events(false);
    }

    fn bound_viewpoint_name(&self) -> String {
        let bound = self.layer.viewpoint();

        if is_default(bound) {
            "<default>".to_string()
        } else {
            self.rt.scene.node(bound).name.clone()
        }
    }

    /// The name of the viewpoint `browser .getActiveViewpoint ()` names (`<default>` for the default one; see [`HeldPose`]).
    pub fn active_viewpoint_name(&self) -> String {
        self.held_pose
            .as_ref()
            .map_or_else(|| self.bound_viewpoint_name(), |h| h.viewpoint.clone())
    }

    fn viewport(&self) -> [f64; 4] {
        [0.0, 0.0, self.width as f64, self.height as f64]
    }

    pub fn width_px(&self) -> u32 {
        self.width
    }

    pub fn height_px(&self) -> u32 {
        self.height
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.layer_sizes();
        self.rt
            .browser_options
            .set_canvas_size(self.width, self.height);
    }

    // --- layers ------------------------------------------------------------------------------

    /// The viewport rectangle of layer `key` (X3DLayerNode viewportNode .getRectangle ()).
    pub fn layer_rect(&self, key: LayerKey) -> [f64; 4] {
        layers::layer_viewport(&x3d_render::sg::Sg::new(&self.rt), key, self.viewport())
    }

    /// The layers' viewport sizes (getLookAtDistance, viewAll).
    fn layer_sizes(&mut self) {
        let r = self.layer_rect(self.current);
        self.layer.viewport = (r[2], r[3]);

        for i in 0..self.others.len() {
            let r = self.layer_rect(self.others[i].key);
            self.others[i].layer.viewport = (r[2], r[3]);
        }
    }

    /// Swap the state of layer `key` into `layer`, `scan_cam` and `scan_display` (a new layer
    /// starts with its default bindables).
    fn swap_to(&mut self, key: LayerKey) {
        if key == self.current {
            return;
        }

        let i = match self.others.iter().position(|s| s.key == key) {
            Some(i) => i,
            None => {
                let layout = key.is_some_and(|n| self.rt.builtin_name(n) == Some("LayoutLayer"));
                let mut layer = if layout {
                    Layer::new_layout()
                } else {
                    Layer::new()
                };
                let r = self.layer_rect(key);
                layer.viewport = (r[2], r[3]);
                self.others.push(LayerSlot {
                    key,
                    layer,
                    scan_cam: Scan::default(),
                    scan_display: Scan::default(),
                });
                self.others.len() - 1
            }
        };

        let slot = &mut self.others[i];
        std::mem::swap(&mut slot.layer, &mut self.layer);
        std::mem::swap(&mut slot.scan_cam, &mut self.scan_cam);
        std::mem::swap(&mut slot.scan_display, &mut self.scan_display);
        slot.key = self.current;
        self.current = key;
    }

    /// How many layer states the viewer keeps (layer 0 and every layer it met).
    pub fn layer_slots(&self) -> usize {
        1 + self.others.len()
    }

    /// LayerSet.getLayers: the layers drawn in the last frame, in order.
    pub fn drawn_layers(&self) -> &[LayerKey] {
        &self.drawn
    }

    /// The layer navigation and the viewpoint lists use: the active layer, else layer 0.
    fn home_layer(&self) -> LayerKey {
        self.active.unwrap_or(None)
    }

    /// X3DWorld and LayerSet: the layers drawn this frame and the active layer, whose state is
    /// swapped in.
    fn update_layers(&mut self) {
        let sg = x3d_render::sg::Sg::new(&self.rt);
        let drawn = layers::drawn_layers(&sg);
        let active = layers::active_layer(&sg);
        self.drawn = drawn;
        self.active = active;
        self.swap_to(self.home_layer());
        self.layer_sizes();
    }

    /// The walk of the current layer: its children, viewport rectangle, and the bound viewpoint
    /// its Layout nodes read.
    fn layer_walk(&self) -> LayerWalk {
        let base = self.base();
        let nav = self.nav_info();
        let vp = self
            .layer
            .vps
            .get(&self.layer.viewpoint())
            .cloned()
            .unwrap_or_default();
        let content_scale = self
            .rt
            .browser_options
            .rendering
            .get("ContentScale")
            .map_or(1.0, |v| v.as_f64());

        LayerWalk {
            key: self.current,
            viewport: self.layer_rect(self.current),
            layout: LayoutCamera {
                ortho: base
                    .is_ortho
                    .then(|| (vp.user_size_x(&base), vp.user_size_y(&base))),
                fov: vp.user_fov(&base),
                near: vp.near_distance(&base, &nav),
                content_scale,
            },
        }
    }

    /// X3DLayerNode `pointerEvents` (layer 0: TRUE).
    fn pointer_events(&self, key: LayerKey) -> bool {
        key.is_none_or(|n| self.rt.get_bool(n, "pointerEvents"))
    }

    /// The pointing camera of layer `key`.
    fn pick_camera_of(&mut self, key: LayerKey) -> PickCamera {
        let home = self.current;
        self.swap_to(key);
        let cam = self.pick_camera();
        self.swap_to(home);
        cam
    }

    /// The frame buffer size X3DRenderingContext.reshape gives the canvas: its size × the
    /// ContentScale option, limited to MAX_VIEWPORT_DIMS.
    pub fn framebuffer_size(&self) -> (u32, u32) {
        self.rt.browser_options.framebuffer
    }

    /// The RenderingProperties the renderer follows, when they changed since the last call
    /// (a Script's `setBrowserOption`): Multisampling (the sample count),
    /// LogarithmicDepthBuffer and Shading. [`Viewer3d::step`] already switched the process-wide
    /// LogarithmicDepthBuffer property (`getMaxFarValue`) for the camera.
    pub fn take_rendering_changes(&mut self) -> RenderingChanges {
        let now = rendering_state(&self.rt, self.base().geo.is_some());
        let old = std::mem::replace(&mut self.synced_rendering, now.clone());

        RenderingChanges {
            samples: (now.0 != old.0).then_some(now.0),
            log_depth: (now.1 != old.1).then_some(now.1),
            shading: (now.2 != old.2)
                .then(|| x3d_render::gpu::ShadingMode::from_rendering_property(&now.2)),
            property_log_depth: self.rt.browser_options.logarithmic_depth_buffer(),
        }
    }

    /// The RenderingProperties the renderer starts with (see [`Viewer3d::take_rendering_changes`]).
    pub fn rendering_properties(&self) -> RenderingChanges {
        let now = rendering_state(&self.rt, self.base().geo.is_some());

        RenderingChanges {
            samples: Some(now.0),
            log_depth: Some(now.1),
            shading: Some(x3d_render::gpu::ShadingMode::from_rendering_property(
                &now.2,
            )),
            property_log_depth: self.rt.browser_options.logarithmic_depth_buffer(),
        }
    }

    pub fn title(&self) -> String {
        let rt = &self.rt;

        for n in 0..rt.scene.nodes.len() as NodeId {
            if rt.builtin_name(n) == Some("WorldInfo") {
                if let Some(FieldValue::SFString(t)) = rt.pre_value(n, "title") {
                    if !t.is_empty() {
                        return t.clone();
                    }
                }
            }
        }

        String::new()
    }

    /// The current layer's bounding box in its space (X3DLayerNode.getBBox: the grouping nodes' boxes nested as X_ITE builds
    /// them), None when it is empty.
    fn scene_bbox(&mut self) -> Option<Box3> {
        let matrix = Builder::new(&self.rt)
            .with_cache_query(&mut self.render_cache)
            .layer_bbox(self.current)?;

        Some(Box3 { matrix })
    }

    /// X3DWorld.bindBindables: layer 0 and every layer of the LayerSet bind their first bindables.
    fn bind_world(&mut self) {
        self.bind_world_events(true);
    }

    /// X3DWorld.bindBindables; `process` runs the events it queued at once (the first world) or leaves them for the
    /// next events of the browser (a world that replaced another: `replaceWorld` runs in a continuation).
    fn bind_world_events(&mut self, process: bool) {
        self.update_layers();
        let all = layers::all_layers(&x3d_render::sg::Sg::new(&self.rt));

        // X3DWorld.bindBindables: `decodeURIComponent (new URL (worldURL) .hash .substring (1))` names the first viewpoint.
        let fragment = self
            .rt
            .scene
            .world_url_of(self.rt.scene.world)
            .split_once('#')
            .map(|(_, f)| percent_decode(f));
        let fragment = fragment.filter(|f| !f.is_empty());

        for key in all {
            self.swap_to(key);
            self.scan_cam = scan_screens(
                &self.rt,
                &self.layer_walk(),
                &Matrix4::IDENTITY,
                true,
                &self.screen_matrices(),
            );
            let bbox = self.scene_bbox();
            let scan_cam = std::mem::take(&mut self.scan_cam);
            self.layer
                .bind_bindables(&mut self.rt, &scan_cam, fragment.as_deref(), &move || {
                    bbox.clone()
                });
            self.scan_cam = scan_cam;
        }

        self.swap_to(self.home_layer());

        if process {
            self.rt.process_events();
        }

        self.set_active_navigation_info(true);
        self.refresh_display_scan();
    }

    pub fn base(&self) -> VpBase {
        VpBase::read(&self.rt, self.layer.viewpoint())
    }

    pub fn nav_info(&self) -> NavInfo {
        NavInfo::read(&self.rt, self.layer.navigation_info())
    }

    pub fn viewer_kind(&self) -> &'static str {
        self.nav.kind()
    }

    /// `browser .getCurrentViewer ()`: the browser's `viewer` field.
    pub fn viewer_field(&self) -> &str {
        if let Some(h) = &self.held_pose {
            return &h.viewer;
        }

        if self.viewer_field.is_empty() {
            self.nav.kind()
        } else {
            &self.viewer_field
        }
    }

    /// X3DNavigationContext.set_activeNavigationInfo__ / set_viewer__.
    fn set_active_navigation_info(&mut self, force: bool) {
        let node = self.layer.navigation_info();

        if !force && node == self.nav_info_node {
            return;
        }

        self.nav_info_node = node;
        let kind = self.nav_info().viewer();
        self.nav_viewer = kind.clone();
        self.set_viewer(&kind);
    }

    /// Switch the viewer (the context menu's viewer choice).
    pub fn set_viewer(&mut self, kind: &str) {
        self.viewer_field = kind.to_string();
        let base = self.base();
        let nav = self.nav_info();
        let vp_node = self.layer.viewpoint();
        let env = self.env();
        let vp = self.layer.vp(vp_node);
        let mut constrain = |t: Vector3, _: &crate::layer::Vp| t;
        let mut cx = Cx {
            env: &env,
            base: &base,
            vp,
            nav: &nav,
            constrain: &mut constrain,
            requests: vec![],
        };
        self.nav.disconnect(env.date_now);
        self.nav = Viewer::new(kind, env.now, &mut cx);
    }

    pub fn available_viewers(&self) -> Vec<String> {
        self.nav_info().available_viewers()
    }

    fn env(&self) -> Env {
        Env {
            now: self.time,
            date_now: self.date_now,
            frame_rate: self.frame_rate,
            shift: self.shift,
            ctrl: self.ctrl,
            alt: self.alt,
            // X3DViewer.getViewport: the active layer's rectangle.
            viewport: self.layer_rect(self.home_layer()),
            // getStraightenHorizon (): the StraightenHorizon option, unless the host overrides it.
            straighten: self.straighten && self.rt.browser_options.straighten_horizon(),
        }
    }

    /// The pointing camera for the bound viewpoint.
    pub fn pick_camera(&mut self) -> PickCamera {
        let base = self.base();
        let nav = self.nav_info();
        let vp = self.layer.vp(self.layer.viewpoint()).clone();
        let (near, far) = (vp.near_distance(&base, &nav), vp.far_distance(&base, &nav));
        let r = self.layer_rect(self.current);

        PickCamera {
            projection: vp.projection(&base, near, far, r[2], r[3]),
            viewport: Vector4::new(r[0], r[1], r[2], r[3]),
            view: vp.view,
            near,
            far,
        }
    }

    // --- the frame -------------------------------------------------------------------------

    /// All runtimes (world and Inline scenes), parents first.
    /// For benchmarks: the seconds of the scene walks (POINTER / COLLISION and DISPLAY, the old scan dropped included)
    /// and of the particle simulation (`run_pass` of every runtime) since the last call, and zero them.
    pub fn take_step_seconds(&mut self) -> (f64, f64) {
        let mut sim = 0.0;

        self.for_each_rt(&mut |rt| sim += std::mem::take(&mut rt.particle_sim_seconds));

        (std::mem::take(&mut self.traversal_seconds), sim)
    }

    fn for_each_rt(&mut self, f: &mut dyn FnMut(&mut Runtime)) {
        for p in crate::walk::all_paths(&self.rt) {
            if let Some(rt) = rt_at_mut(&mut self.rt, &p) {
                f(rt);
            }
        }
    }

    fn phase(&mut self, name: &str) {
        let now = self.time;
        let rate = self.pinned_frame_rate.unwrap_or(self.frame_rate);

        // The Anchors that were clicked since the last frame request their urls when their TouchSensors' events run.
        if name == "prepareEvents" {
            for (anchor, urls, parameters) in std::mem::take(&mut self.pending_anchors) {
                self.rt.anchor_load(anchor, urls, parameters);
            }
        }

        self.for_each_rt(&mut |rt| {
            rt.now = now;
            rt.frame_rate = rate;
            rt.process_phase(name);
        });

        if name == "prepareEvents" {
            self.held_pose = None;
        }

        // X3DBrowser.bindViewpoint sets `set_bind` while the Script runs, so the viewpoint is
        // bound in the same frame (at its time), not in the next one.
        self.browser_requests();
    }

    /// Scripts' requests of the browser: X3DBrowser.nextViewpoint, previousViewpoint,
    /// firstViewpoint, lastViewpoint.
    fn browser_requests(&mut self) {
        self.sync_browser_active();

        for r in std::mem::take(&mut self.rt.browser_requests) {
            match r.as_str() {
                "nextViewpoint" => self.next_viewpoint(1),
                "previousViewpoint" => self.next_viewpoint(-1),
                "firstViewpoint" => self.first_viewpoint(),
                "lastViewpoint" => self.last_viewpoint(),
                // Browser.changeViewpoint (name), and loadURL (url) with a fragment of the world's own URL.
                r if r.starts_with("changeViewpoint:") => {
                    self.change_viewpoint(&r["changeViewpoint:".len()..])
                }
                // BrowserOptions.set_StraightenHorizon__: TRUE straightens the active layer's view; the ExamineViewer's interest
                // disconnects it on every set.
                r if r.starts_with("straightenHorizon:") => {
                    if r.ends_with("true") {
                        let home = self.current;
                        let date_now = self.date_now;

                        self.swap_to(self.home_layer());

                        let vp_node = self.layer.viewpoint();
                        let base = self.base();

                        self.layer
                            .straighten_view(&mut self.rt, vp_node, &base, date_now);
                        self.swap_to(home);
                    }

                    if let Viewer::Examine(e) = &mut self.nav {
                        e.disconnect(self.date_now);
                    }
                }
                // Browser.viewAll (layerNode, transitionTime): the layer's node id (-1: the active layer) and the time.
                r if r.starts_with("viewAll:") => {
                    let (layer, time) =
                        r["viewAll:".len()..].split_once(',').unwrap_or(("-1", "1"));
                    let layer = layer
                        .parse::<NodeId>()
                        .ok()
                        .filter(|&n| self.rt.scene.is_live(n));
                    self.browser_view_all(layer, time.parse().unwrap_or(f64::NAN));
                }
                // Browser.bindViewpoint (layerNode, viewpointNode): the Script checked both types.
                r if r.starts_with("bindViewpoint:") => {
                    if let Ok(n) = r["bindViewpoint:".len()..].parse::<NodeId>() {
                        if self.rt.scene.is_live(n) {
                            self.bind_viewpoint(n);
                        }
                    }
                }
                // Browser.resize (width, height) and Browser.dispose ().
                r if r.starts_with("resize:") => {
                    let mut it = r["resize:".len()..]
                        .split(',')
                        .map(|v| v.parse::<f64>().unwrap_or(f64::NAN));
                    let (w, h) = (it.next().unwrap_or(f64::NAN), it.next().unwrap_or(f64::NAN));

                    if w.is_finite() && h.is_finite() {
                        let size = (w.max(1.0) as u32, h.max(1.0) as u32);

                        self.resize(size.0, size.1);
                        self.resize_request = Some(size);
                    }
                }
                "dispose" => self.disposed = true,
                // loadURL with a foreign target (window.open) or a page URL: another window or document, outside the native scope.
                _ => {}
            }
        }
    }

    /// X3DBrowserContext.getActiveLayer / getActiveNavigationInfo / getActiveViewpoint / getCurrentViewer for Scripts: the
    /// active layer and its bound nodes (`None`: layer 0, or the layer's default node).
    fn sync_browser_active(&mut self) {
        let real = |n: NodeId| (!is_default(n)).then_some(n);

        self.rt.browser_active = x3d_runtime::BrowserActive {
            layer: self.current,
            navigation_info: real(self.layer.navigation_info()),
            viewpoint: real(self.layer.viewpoint()),
            viewer: self.viewer_field().to_string(),
        };
    }

    /// Nodes the runtime released since the last call: nothing the viewer remembers of them may
    /// meet a node that takes their id (saved viewpoints, sensor state, the pointer's drags,
    /// hovers and Anchor presses).
    fn forget_released(&mut self) {
        let released = self.rt.take_released();

        if released.is_empty() {
            return;
        }

        let dead = |key: &(Vec<NodeId>, NodeId)| {
            released.contains(&key.1) || key.0.iter().any(|n| released.contains(n))
        };

        // A released layer node goes with its state (layer 0 takes its place while it is current).
        if self.current.is_some_and(|n| released.contains(&n)) {
            self.swap_to(None);
        }

        self.others
            .retain(|s| s.key.is_none_or(|n| !released.contains(&n)));
        self.drawn
            .retain(|k| k.is_none_or(|n| !released.contains(&n)));

        if self
            .pointing_layer
            .is_some_and(|k| k.is_some_and(|n| released.contains(&n)))
        {
            self.pointing_layer = None;
        }

        self.layer.forget(&released);

        for s in &mut self.others {
            s.layer.forget(&released);
        }

        self.prox.retain(|k, _| !dead(k));
        self.vis.retain(|k, _| !dead(k));
        self.lod_pass.static_done.retain(|k| !dead(k));
        self.geom_cache.retain(|k, _| !dead(k));
        self.particle_depth_geometry.retain(|k, _| !dead(k));
        self.thick_line_systems.retain(|k| !dead(k));
        self.thick_line_depth.retain(|k, _| !dead(k));

        for layer in
            std::iter::once(&mut self.layer).chain(self.others.iter_mut().map(|s| &mut s.layer))
        {
            if let Some(systems) = layer.collision_particle_systems.as_mut() {
                systems.retain(|n| {
                    !released.contains(&n.node) && !n.path.iter().any(|p| released.contains(p))
                });
            }
        }
        self.pick_shapes.retain(|n| !released.contains(&n));
        self.pointer.forget(&released);
    }

    /// One iteration of X3DBrowserContext [_traverse] at browser time `now`
    /// (advanceTime has set getCurrentTime to `now`).
    /// X3DTimeContext.advanceTime's frame rate limit: a live browser skips a frame that comes
    /// sooner than 1 / MaximumFrameRate after the last one (the oracle pages replace
    /// advanceTime, so the harnesses step every frame).
    pub fn frame_due(&self, now: f64) -> bool {
        let max = self
            .rt
            .browser_options
            .get_option("MaximumFrameRate")
            .map_or(80.0, |v| v.as_f64());

        // `interval <= 1 / Math .max (MaximumFrameRate, 1)` skips; a NaN limit never does.
        match self.last_time {
            Some(last) => !(now - last <= 1.0 / x3d_fields::jsmath::js_max(max, 1.0)),
            None => true,
        }
    }

    pub fn step(&mut self, now: f64) {
        if self.disposed {
            return;
        }

        if let Some(last) = self.last_time {
            let dt = now - last;

            if dt > 0.0 {
                self.frame_rate = 1.0 / dt;
            }
        }

        self.forget_released();

        let prev_time = self.last_time.unwrap_or(now);

        self.last_time = Some(now);
        self.time = now;
        self.date_now = self.date_now.max(now);
        self.geom_cache.clear();
        self.frame_count += 1;

        // Timeouts that fired between frames (setTimeout).
        let date_now = self.date_now;

        self.for_each_rt(&mut |rt| {
            rt.timer_prev_time = prev_time;
            rt.timer_date = date_now;
            rt.fire_timeouts(now);
        });

        // Scripts' requests of the browser made between frames (Browser.nextViewpoint ...).
        self.browser_requests();

        // The update fields the last frame's GeneratedCubeMapTexture passes set to NONE.
        for n in std::mem::take(&mut self.generated_none) {
            if let Some(rt) = rt_at_mut(&mut self.rt, &n.path) {
                if let Some(k) = rt.pre(n.node, "update") {
                    rt.set_value(k, FieldValue::SFString("NONE".into()));
                }
            }
        }

        // Prepare and time events.
        self.sync_active_layer();
        self.phase("prepareEvents");
        let collision = web_time::Instant::now();
        self.prepare_viewer();
        self.collision_time += collision.elapsed();
        self.phase("timePrepareEvents");
        self.phase("timeEvents");
        self.layer.tick(&mut self.rt);

        for s in &mut self.others {
            s.layer.tick(&mut self.rt);
        }

        self.rt.process_events();

        // X3DViewpointNode.getMaxFarValue reads the LogarithmicDepthBuffer rendering property,
        // which a Script's setBrowserOption changes in the events above.
        let log_depth = self.rt.browser_options.logarithmic_depth_buffer();

        if log_depth != self.camera_log_depth {
            self.camera_log_depth = log_depth;
            x3d_render::traverse::set_logarithmic_depth_buffer(log_depth);
        }

        // set_PrimitiveQuality__, set_TextureQuality__ and getTextCompression for the geometries
        // and textures built on this thread.
        let options = &self.rt.browser_options;
        let quality = |name: &str| {
            x3d_render::traverse::Quality::from_option(
                options.get_option(name).map_or("", |v| v.as_str()),
            )
        };
        let scaling = options
            .get_option("TextCompression")
            .is_some_and(|v| v.as_str() == "SCALING");
        x3d_render::traverse::set_browser_qualities(
            quality("PrimitiveQuality"),
            quality("TextureQuality"),
            scaling,
        );

        // Camera.
        self.camera_pass();
        self.phase("cameraEvents");

        // Collision (gravity): the COLLISION traversal starts from the view of this frame.
        self.refresh_display_scan();
        let collision = web_time::Instant::now();
        self.collision_pass();
        self.collision_time += collision.elapsed();

        // Sensor events.
        self.sensor_pass();
        self.picking_pass();
        self.phase("sensorEvents");

        // Display: the DISPLAY traversal (LOD.changeLevel), then displayFinishEvents.
        self.phase("displayEvents");
        self.display_scan();
        self.display_pass();
        self.phase("displayFinishEvents");

        // finishEvents runs the continuation of a pending replaceWorld, and the end of the frame releases the old world's
        // nodes: what `browser .getActiveViewpoint ()` names is read before.
        let replacing = !self.rt.world_state.waiters.is_empty();
        let before = replacing.then(|| self.hold_pose());

        self.phase("finishEvents");

        // X3DBrowser.replaceWorld ran in the continuation of this frame: a new X3DWorld binds its bindables.
        if self.rt.world_state.generation != self.world_gen {
            self.reset_world(before);
        }

        if let Some((x, y)) = self.verify_motion.take() {
            self.pointer_motion(x, y);
        }

        for d in std::mem::take(&mut self.rt.prints) {
            self.status = d;
        }

        if !self.rt.description.is_empty() && self.rt.description != self.status_description {
            self.status = self.rt.description.clone();
            self.status_description = self.status.clone();
        }
    }

    fn prepare_viewer(&mut self) {
        let base = self.base();
        let nav = self.nav_info();
        let env = self.env();
        let vp_node = self.layer.viewpoint();
        let world = self.collision_world();
        let (bb, nv) = (base.clone(), nav.clone());
        let vp = self.layer.vp(vp_node);
        let wall_friction = self.rt.browser_options.wall_friction();
        let mut constrain = |t: Vector3, v: &crate::layer::Vp| match &world {
            Some(w) => w.constrain_translation(v, &bb, &nv, t, true, true, wall_friction),
            None => t,
        };
        let mut cx = Cx {
            env: &env,
            base: &base,
            vp,
            nav: &nav,
            constrain: &mut constrain,
            requests: vec![],
        };
        self.nav.prepare(&mut cx);
    }

    /// X3DLayerNode.camera and the NavigationContext reactions.
    fn camera_pass(&mut self) {
        // A new active layer changes the browser's active viewpoint and navigation info too.
        let before_vp = self.layer.viewpoint();
        let before_nav = self.layer.navigation_info();
        self.update_layers();
        let home = self.current;

        // LayerSet.traverse (CAMERA): each layer drawn, in order.
        for key in self.drawn.clone() {
            self.swap_to(key);
            self.scan_cam = scan_screens(
                &self.rt,
                &self.layer_walk(),
                &Matrix4::IDENTITY,
                true,
                &self.screen_matrices(),
            );

            let bbox = if self.scan_cam.viewpoints.iter().any(|(n, _)| {
                let b = VpBase::read(&self.rt, n.node);
                b.view_all
            }) {
                self.scene_bbox()
            } else {
                None
            };

            let scan_cam = std::mem::take(&mut self.scan_cam);
            self.layer
                .camera(&mut self.rt, &scan_cam, &move || bbox.clone());
            self.scan_cam = scan_cam;
        }

        self.swap_to(home);
        self.rt.process_events();

        self.bindables_changed(before_vp, before_nav);

        // A NavigationInfo transitionStart disconnects the ExamineViewer's spin.
        // (NavigationInfo ._transitionStart .addInterest ("disconnect"))
        let cur = self.layer.viewpoint();
        let base = self.base();
        self.layer.vp(cur).update(&base);
    }

    /// X3DNavigationContext: the browser's active navigation info and viewpoint follow the
    /// active layer's stacks (set_activeNavigationInfo__, set_activeViewpoint__).
    fn bindables_changed(&mut self, before_vp: NodeId, before_nav: NodeId) {
        if self.layer.navigation_info() != before_nav {
            self.set_active_navigation_info(false);
        } else if self.layer.navigation_info() != self.nav_info_node {
            self.set_active_navigation_info(false);
        } else {
            // NavigationInfo.set_type__ of the bound node: `_viewer .addFieldInterest (browser
            // ._viewer)`, so a new type switches the viewer.
            let kind = self.nav_info().viewer();

            if kind != self.nav_viewer {
                self.nav_viewer = kind.clone();
                self.set_viewer(&kind);
            }
        }

        if self.layer.viewpoint() != before_vp {
            // browser ._activeViewpoint changed: ExamineViewer.set_activeViewpoint__.
            let base = self.base();
            let nav = self.nav_info();
            let env = self.env();
            let vp = self.layer.vp(self.layer.viewpoint());
            let mut constrain = |t: Vector3, _: &crate::layer::Vp| t;
            let mut cx = Cx {
                env: &env,
                base: &base,
                vp,
                nav: &nav,
                constrain: &mut constrain,
                requests: vec![],
            };
            self.nav.active_viewpoint_changed(&mut cx);
        }
    }

    /// X3DWorld.set_activeLayer__: a Script's or a key handler's new activeLayer takes effect while
    /// its events are processed, before the next frame's navigation (the viewer stops acting on
    /// the old layer's viewpoint at once).
    fn sync_active_layer(&mut self) {
        let (before_vp, before_nav, before) = (
            self.layer.viewpoint(),
            self.layer.navigation_info(),
            self.current,
        );
        let sg = x3d_render::sg::Sg::new(&self.rt);
        let active = layers::active_layer(&sg);

        if active == self.active {
            return;
        }

        self.update_layers();

        if self.current != before {
            self.bindables_changed(before_vp, before_nav);
        }
    }

    /// Whether the COLLISION traversal runs and the shapes it would collect.
    fn collision_world(&mut self) -> Option<World> {
        let collidable = self.nav.is_collidable();

        if self.scan_display.collisions == 0 && !collidable {
            return None;
        }

        let nav_node = self.layer.navigation_info();

        // X3DLayerNode.collision: no collision while a transition is active.
        if self
            .layer
            .transition_active
            .get(&nav_node)
            .copied()
            .unwrap_or(false)
        {
            return None;
        }

        self.collision_world_unconditional()
    }

    /// The shapes the COLLISION traversal collected, whatever the viewer and the transitions say.
    fn collision_world_unconditional(&mut self) -> Option<World> {
        let mut shapes = vec![];

        for s in &self.scan_display.shapes {
            if !s.collidable {
                continue;
            }

            if let Some(g) = geom_of(&self.rt, &mut self.geom_cache, s) {
                shapes.push(CShape {
                    node: s.shape.clone(),
                    geometry: g,
                    particle: false,
                    model_view: s.model_view,
                    base_model_view: s.shape_model_view,
                    instance: s.instance,
                    clip_planes: s.clip_planes.as_slice().into(),
                    collisions: s.collisions.as_slice().into(),
                    instances: None,
                });
            }
        }

        // Collision.proxy: the proxy nodes' shapes are collision objects too.
        for s in &self.scan_display.proxy_shapes {
            if let Some(g) = geom_of(&self.rt, &mut self.geom_cache, s) {
                shapes.push(CShape {
                    node: s.shape.clone(),
                    geometry: g,
                    particle: false,
                    model_view: s.model_view,
                    base_model_view: s.shape_model_view,
                    instance: s.instance,
                    clip_planes: s.clip_planes.as_slice().into(),
                    collisions: s.collisions.as_slice().into(),
                    instances: None,
                });
            }
        }

        // The drawn particles (ParticleSystem.displaySimple) of the systems the last COLLISION traversal collected.
        // A system's particles come together, with one geometry, clip-plane and Collision stack: a run of them is
        // resolved once and shares those, while every particle keeps its own entry (its instance matrix) in traversal
        // order. A system reached twice (USE) gives two runs; adjacent kept runs of the same system, matrix, clip planes
        // and Collision stack are one.
        let mut seen: Vec<(Vec<NodeId>, NodeId)> = vec![];
        let members = self.layer.collision_particle_systems.clone();
        let kept = self.collision_particles();
        let mut i = 0;

        while i < kept.len() {
            let first = &self.scan_display.particle_collisions[kept[i]].template;
            let run = kept[i..]
                .iter()
                .take_while(|&&k| {
                    let s = &self.scan_display.particle_collisions[k].template;

                    s.shape == first.shape
                        && s.shape_model_view == first.shape_model_view
                        && s.clip_planes == first.clip_planes
                        && s.collisions == first.collisions
                })
                .count();
            let runs = &kept[i..i + run];
            i += run;

            if members.as_ref().is_some_and(|m| !m.contains(&first.shape)) {
                continue;
            }

            let Some(g) = geom_of(&self.rt, &mut self.geom_cache, first) else {
                continue;
            };
            // All particles of a system draw the same geometry: it is compared once per world.
            let key = (first.shape.path.to_vec(), first.shape.node);
            let g = match self.particle_depth_geometry.get(&key) {
                Some(kept)
                    if seen.contains(&key)
                        || Rc::ptr_eq(kept, &g)
                        || (kept.geometry_type == g.geometry_type
                            && kept.vertices == g.vertices
                            && kept.normals == g.normals) =>
                {
                    kept.clone()
                }
                _ => {
                    self.particle_depth_geometry.insert(key.clone(), g.clone());
                    g
                }
            };

            if !seen.contains(&key) {
                seen.push(key.clone());
            }

            let clip_planes: Rc<[[f64; 4]]> = first.clip_planes.as_slice().into();
            let collisions: Rc<[NodeId]> = first.collisions.as_slice().into();
            let line_width = match first.style {
                Some(x3d_render::traverse::StyleParams::Line(l)) if g.geometry_type == 1 => l.width,
                _ => 1.0,
            };

            // displaySimpleInstancedThick: the triangles the last DISPLAY pass made, while the line count is the
            // current one, under the instance matrices of that pass; nothing otherwise. Narrower lines are 1 px here:
            // measured, the collision depth buffer does not widen gl.lineWidth lines (rea_b21_wide_lines pw-col-w6
            // gives X_ITE's pw-col-w1 probes in every frame, while a 6 px line would reach pixels they miss).
            if line_width > x3d_render::gpu::MAX_LINE_WIDTH {
                let sim = rt_at(&self.rt, &first.shape.path)
                    .and_then(|rt| rt.particle_sim(first.shape.node));
                let lines = sim
                    .and_then(|sim| {
                        let num_lines = g.vertex_count() / 2 * sim.num_particles;

                        sim.thick_lines.clone().filter(|t| t.num_lines == num_lines)
                    })
                    .filter(|t| t.num_lines > 0);
                let Some(lines) = lines else {
                    continue;
                };
                let (geometry, matrices, per) = match self.thick_line_depth.get(&key) {
                    Some((kept, geometry, matrices, per)) if Rc::ptr_eq(kept, &lines) => {
                        (geometry.clone(), matrices.clone(), *per)
                    }
                    _ => {
                        let positions = x3d_render::gpu::thick_line_positions(&lines);
                        let per = positions.len() / lines.instances.len();
                        let geometry =
                            Rc::new(x3d_render::particles::line_triangle_geometry(&positions));
                        let matrices: Rc<Vec<Matrix4>> = Rc::new(
                            lines
                                .instances
                                .iter()
                                .map(|m| Matrix4(m.map(f64::from)))
                                .collect(),
                        );

                        self.thick_line_depth.insert(
                            key.clone(),
                            (lines.clone(), geometry.clone(), matrices.clone(), per),
                        );
                        (geometry, matrices, per)
                    }
                };

                // Every particle's triangles in one draw, particle k its own vertices under its instance matrix.
                shapes.push(CShape {
                    node: first.shape.clone(),
                    geometry,
                    particle: true,
                    model_view: first.shape_model_view,
                    base_model_view: first.shape_model_view,
                    instance: None,
                    clip_planes: clip_planes.clone(),
                    collisions: collisions.clone(),
                    instances: Some(crate::collision::CInstances {
                        matrices,
                        per_vertices: per,
                    }),
                });

                continue;
            }

            // Each run's particles in one draw (ParticleSystem.displaySimple, gl.drawArraysInstanced).
            for &k in runs {
                let r = &self.scan_display.particle_collisions[k];

                shapes.push(CShape {
                    node: r.template.shape.clone(),
                    geometry: g.clone(),
                    particle: true,
                    model_view: r.template.shape_model_view,
                    base_model_view: r.template.shape_model_view,
                    instance: None,
                    clip_planes: clip_planes.clone(),
                    collisions: collisions.clone(),
                    instances: Some(crate::collision::CInstances {
                        matrices: r.matrices.clone(),
                        per_vertices: 0,
                    }),
                });
            }
        }

        self.particle_depth_geometry.retain(|k, _| seen.contains(k));
        self.thick_line_depth.retain(|k, _| seen.contains(k));

        // Shading WIREFRAME is global GL state (the polygon mode) for X_ITE's depth renders too.
        let fill = match self
            .rt
            .browser_options
            .rendering
            .get("Shading")
            .map(|v| v.as_str())
        {
            Some("WIREFRAME") => crate::collision::Fill::Edges,
            _ => crate::collision::Fill::Triangles,
        };

        // A lost device: the CPU stand-in from now on.
        if self.depth_gpu.as_ref().is_some_and(|g| g.borrow().lost()) {
            self.depth_gpu = None;
        }

        Some(World {
            shapes,
            fill,
            gpu: self.depth_gpu.clone(),
        })
    }

    /// The runs of [`Scan::particle_collisions`] with particles that X3DRenderObject.addCollisionShape keeps: it rejects a
    /// ParticleSystem whose bbox sphere is outside the COLLISION traversal's view volume, X3DLayerNode.collision's
    /// `ortho (-size, size, -size, size, -size, size)` with `size = max (collisionRadius * 2, avatarHeight * 2)` around
    /// the viewer.
    fn collision_particles(&self) -> Vec<usize> {
        let nav = self.nav_info();
        let size = (nav.collision_radius() * 2.0).max(nav.avatar_height() * 2.0);
        let mut volume = Matrix4::IDENTITY;
        crate::geom::Camera::ortho(-size, size, -size, size, -size, size, &mut volume);

        (0..self.scan_display.particle_collisions.len())
            .filter(|&i| {
                let r = &self.scan_display.particle_collisions[i];
                let s = &r.template;

                // A system with no particle drawn has no entry.
                r.len() > 0
                    && s.explicit_bbox.is_some_and(|b| {
                        x3d_render::traverse::bbox_in_view_volume(&volume, &s.shape_model_view, &b)
                    })
            })
            .collect()
    }

    /// X3DRenderObject.collide: the closest object along each of the six axes within the collision radius
    /// (times sqrt 2) makes the Collision nodes around it active (isActive, collideTime). It runs for every
    /// viewer while an enabled Collision exists. `closest` holds the six closest objects, empty without collision
    /// shapes.
    fn collide(&mut self, world: &World, closest: &[crate::collision::Closest]) {
        let nav = self.nav_info();
        let mut active: Vec<x3d_fields::NodeId> = vec![];
        let radius = nav.collision_radius() * std::f64::consts::SQRT_2;
        let mut closest_shapes: Vec<usize> = vec![];

        for c in closest {
            if let Some(id) = c.shape.filter(|_| c.distance <= radius)
                && !closest_shapes.contains(&id)
            {
                closest_shapes.push(id);
            }
        }

        for id in closest_shapes {
            if let Some((shape, _)) = world.shape_of(id) {
                active.extend(shape.collisions.iter().copied());
            }
        }

        // Set isActive to FALSE for affected nodes, then TRUE.
        for c in std::mem::take(&mut self.active_collisions) {
            if !active.contains(&c) {
                self.set_collision_active(c, false);
            }
        }

        for c in &active {
            self.set_collision_active(*c, true);
        }

        self.active_collisions = active;
    }

    /// Collision.set_active__.
    fn set_collision_active(&mut self, node: x3d_fields::NodeId, value: bool) {
        if self.rt.get_bool(node, "isActive") == value {
            return;
        }

        if let Some(k) = self.rt.pre(node, "isActive") {
            self.rt.set_value(k, FieldValue::SFBool(value));
        }

        if value {
            let now = self.rt.now;

            if let Some(k) = self.rt.pre(node, "collideTime") {
                self.rt.set_value(k, FieldValue::SFTime(now));
            }
        }
    }

    fn collision_pass(&mut self) {
        // The COLLISION traversal (X3DBrowserContext: `getCollisionCount () || getViewer () .isCollidable ()`, not
        // during a transition) collects the collision shapes.
        if (self.scan_display.collisions > 0 || self.nav.is_collidable())
            && !self
                .layer
                .transition_active
                .get(&self.layer.navigation_info())
                .copied()
                .unwrap_or(false)
        {
            let mut systems: Vec<NodeRef> = vec![];

            for i in self.collision_particles() {
                let s = &self.scan_display.particle_collisions[i].template.shape;

                if !systems.contains(s) {
                    systems.push(s.clone());
                }
            }

            self.layer.collision_particle_systems = Some(systems);
        }

        // collide runs while an enabled Collision exists, gravitate in WALK. Both read the same collision shapes from
        // the same viewpoint (collide's isActive events wait for the event loop), so their depth passes are one batch.
        let collide = self.scan_display.collisions > 0;
        let walk = self.nav.kind() == "WALK";

        if !collide && !walk {
            return;
        }

        let Some(world) = self.collision_world() else {
            return;
        };
        let base = self.base();
        let nav = self.nav_info();
        let vp_node = self.layer.viewpoint();
        let vp = self.layer.vp(vp_node).clone();
        let axes: &[Vector3] = if collide && !world.shapes.is_empty() {
            &[
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(-1.0, 0.0, 0.0),
                Vector3::new(0.0, -1.0, 0.0),
                Vector3::new(0.0, 0.0, -1.0),
            ]
        } else {
            &[]
        };
        let (closest, ground) = world.collision_queries(&vp, &base, &nav, axes, walk);

        if collide {
            self.collide(&world, &closest);
        }

        let Some(ground) = ground else {
            return;
        };
        let mut speed = self.gravity_speed;
        let frame_rate = self.frame_rate;

        // X3DRenderObject.gravitate: `getBrowserOptions () ._Gravity .getValue ()`.
        let gravity = self.rt.browser_options.gravity();

        if let Some(d) =
            world.gravitate_on(&ground, &vp, &base, &nav, &mut speed, frame_rate, gravity)
        {
            let v = self.layer.vp(vp_node);
            v.position_offset.add(&d);
        }

        self.gravity_speed = speed;
    }

    /// Recompute the display scan with the current view (X3DLayerNode.display traversal of the
    /// pointer, sensor and collision data).
    /// The DISPLAY traversal: like [`Self::refresh_display_scan`], and each LOD visited changes its
    /// level (LOD.changeLevel: `this ._level_changed = level`).
    fn display_scan(&mut self) {
        let mut lod = std::mem::take(&mut self.lod_pass);
        lod.levels.clear();
        lod.current_frame_rate = self.pinned_frame_rate.unwrap_or(self.frame_rate);
        let home = self.current;

        for (path, node) in &self.thick_line_systems {
            if let Some(sim) =
                rt_at_mut(&mut self.rt, path).and_then(|rt| rt.particle_sim_mut(*node))
            {
                sim.thick_lines_before_layer.clear();
            }
        }

        for key in self.drawn.clone() {
            // X3DRenderObject.draw of this layer renders its shadow maps before its shapes: they draw the transformed
            // LINE triangles of the last color pass before them (the previous frame's last layer, or a layer drawn
            // before this one).
            for (path, node) in &self.thick_line_systems {
                if let Some(sim) =
                    rt_at_mut(&mut self.rt, path).and_then(|rt| rt.particle_sim_mut(*node))
                {
                    let lines = sim.thick_lines.clone();
                    sim.thick_lines_before_layer.push((key, lines));
                }
            }

            self.swap_to(key);
            let vp_node = self.layer.viewpoint();
            let view = self.layer.vp(vp_node).view;
            let start = web_time::Instant::now();
            self.scan_display =
                crate::walk::scan_display(&self.rt, &self.layer_walk(), &view, &mut lod);
            self.traversal_seconds += start.elapsed().as_secs_f64();
            self.display_thick_lines();
        }

        self.clear_undrawn_scans();
        self.swap_to(home);

        for key in std::mem::take(&mut lod.static_reached) {
            lod.static_done.insert(key);
        }

        for (n, level) in std::mem::take(&mut lod.changes) {
            if let Some(rt) = rt_at_mut(&mut self.rt, &n.path) {
                if let Some(k) = rt.pre(n.node, "level_changed") {
                    rt.set_value(k, FieldValue::SFInt32(level));
                }
            }
        }

        for (n, level) in std::mem::take(&mut lod.geo_changes) {
            if let Some(rt) = rt_at_mut(&mut self.rt, &n.path) {
                rt.geo_lod_change_level(n.node, level);
            }
        }

        // GeneratedCubeMapTexture.renderTexture (this frame's draw): `if (this ._update .equals
        // ("NEXT_FRAME_ONLY")) this ._update = "NONE"`, processed in the next frame.
        self.generated_none = std::mem::take(&mut lod.generated_once);

        // RenderedTexture.traverse: the textures the renderer draws this frame.
        self.for_each_rt(&mut |rt| rt.rendered_now.clear());
        let now_ms = self.date_now * 1000.0;

        for r in std::mem::take(&mut lod.rendered_reached) {
            if let Some(rt) = rt_at_mut(&mut self.rt, &r.path) {
                if rt.rendered_texture_traverse(r.node, now_ms) {
                    rt.rendered_now.push(r.node);
                }
            }
        }

        // GeoLOD.traverse draws what changeLevel left, before the frame's remaining events.
        for n in std::mem::take(&mut lod.geo_visited) {
            if let Some(rt) = rt_at_mut(&mut self.rt, &n.path) {
                rt.geo_lod_display(n.node);
            }
        }

        self.lod_pass = lod;
    }

    /// X3DLineGeometryNode.displayInstancedThick in the DISPLAY pass of the current layer: every LINE system with
    /// transformed lines that addDisplayShape keeps (its bbox in the view volume) gets the triangles of this view, which
    /// the pointing, collision and shadow passes draw until the next DISPLAY pass draws it.
    fn display_thick_lines(&mut self) {
        if self.scan_display.thick_lines.is_empty() {
            return;
        }

        let cam = self.pick_camera();

        for d in std::mem::take(&mut self.scan_display.thick_lines) {
            if !x3d_render::traverse::bbox_in_view_volume(&cam.projection, &d.model_view, &d.bbox) {
                continue;
            }

            // The inputs only: the shadow passes make the triangles on the GPU, pointing and collision on demand.
            let lines = x3d_render::gpu::thick_lines(
                &d.geometry,
                &d.model_view,
                &cam.projection,
                d.viewport,
                d.width,
                d.instances.iter().map(|m| m.0.map(|v| v as f32)).collect(),
            );
            let key = (d.system.path.to_vec(), d.system.node);

            if let Some(sim) = rt_at_mut(&mut self.rt, &d.system.path)
                .and_then(|rt| rt.particle_sim_mut(d.system.node))
            {
                sim.thick_lines = Some(Rc::new(lines));

                if !self.thick_line_systems.contains(&key) {
                    self.thick_line_systems.push(key);
                }
            }
        }
    }

    fn refresh_display_scan(&mut self) {
        let home = self.current;

        for key in self.drawn.clone() {
            self.swap_to(key);
            let vp_node = self.layer.viewpoint();
            let view = self.layer.vp(vp_node).view;
            let start = web_time::Instant::now();
            self.scan_display = scan_screens(
                &self.rt,
                &self.layer_walk(),
                &view,
                false,
                &self.screen_matrices(),
            );
            self.traversal_seconds += start.elapsed().as_secs_f64();
        }

        self.clear_undrawn_scans();
        self.swap_to(home);
    }

    /// A layer that is not drawn is not traversed: nothing of it can be hit or collided with.
    fn clear_undrawn_scans(&mut self) {
        if !self.drawn.contains(&self.current) {
            self.scan_display = Scan::default();
        }

        for s in &mut self.others {
            if !self.drawn.contains(&s.key) {
                s.scan_display = Scan::default();
            }
        }
    }

    // ProximitySensor / VisibilitySensor (EnvironmentalSensor): CAMERA + DISPLAY traversals and
    // the sensorEvents update.
    fn display_pass(&mut self) {
        let home = self.current;

        for key in self.drawn.clone() {
            self.swap_to(key);
            self.display_pass_layer();
        }

        self.swap_to(home);
    }

    fn display_pass_layer(&mut self) {
        let key = self.current;
        let vp_node = self.layer.viewpoint();
        let base = self.base();
        let nav = self.nav_info();
        let vpv = self.layer.vp(vp_node).clone();
        let (near, far) = (
            vpv.near_distance(&base, &nav),
            vpv.far_distance(&base, &nav),
        );
        let r = self.layer_rect(key);
        let proj = vpv.projection(&base, near, far, r[2], r[3]);

        // SpatialSound.traverse and ListenerPointSource.traverse (visible objects, DISPLAY).
        for e in self.scan_display.sounds.clone() {
            if let Some(rt) = rt_at_mut(&mut self.rt, &e.node.path) {
                rt.sound_traverse(e.node.node, &e.model_view);
            }
        }

        for e in self.scan_display.proximity.clone() {
            let key = (e.node.path.to_vec(), e.node.node);
            let Some(rt) = rt_at(&self.rt, &e.node.path) else {
                continue;
            };
            let enabled = rt.get_bool(e.node.node, "enabled");
            let size = match rt.pre_value(e.node.node, "size") {
                Some(FieldValue::SFVec3f(v)) => *v,
                _ => Vector3::ZERO,
            };
            let center = match rt.pre_value(e.node.node, "center") {
                Some(FieldValue::SFVec3f(v) | FieldValue::SFVec3d(v)) => *v,
                _ => Vector3::ZERO,
            };

            let st = self.prox.entry(key).or_default();
            st.traversed = true;
            st.layer = self.current;

            if enabled {
                st.registered = true;
            }

            if st.inside {
                continue;
            }

            if size.equals(&Vector3::new(-1.0, -1.0, -1.0)) {
                st.inside = true;
            } else {
                let mut inv = e.model_view;
                inv.inverse();
                let p = Vector3::new(inv.0[12], inv.0[13], inv.0[14]);
                let (hx, hy, hz) = (size.x / 2.0, size.y / 2.0, size.z / 2.0);
                st.inside = center.x - hx <= p.x
                    && center.x + hx >= p.x
                    && center.y - hy <= p.y
                    && center.y + hy >= p.y
                    && center.z - hz <= p.z
                    && center.z + hz >= p.z;
            }
        }

        for e in self.scan_display.visibility.clone() {
            let key = (e.node.path.to_vec(), e.node.node);
            let Some(rt) = rt_at(&self.rt, &e.node.path) else {
                continue;
            };
            let enabled = rt.get_bool(e.node.node, "enabled");
            let size = match rt.pre_value(e.node.node, "size") {
                Some(FieldValue::SFVec3f(v)) => *v,
                _ => Vector3::ZERO,
            };
            let center = match rt.pre_value(e.node.node, "center") {
                Some(FieldValue::SFVec3f(v) | FieldValue::SFVec3d(v)) => *v,
                _ => Vector3::ZERO,
            };

            let st = self.vis.entry(key).or_default();
            st.traversed = true;
            st.layer = self.current;

            if enabled {
                st.registered = true;
            }

            if st.inside {
                continue;
            }

            if size.equals(&Vector3::new(-1.0, -1.0, -1.0)) {
                st.inside = true;
            } else {
                st.inside = box_in_frustum(&size, &center, &e.model_view, &proj);
            }
        }
    }

    /// The sensorEvents interests of ProximitySensor.update and VisibilitySensor.update.
    /// X3DPickingContext.picking (a sensorEvents interest while TransformSensors or pick sensors are
    /// enabled): the PICKING traversal of the layers, then TransformSensor.process and
    /// X3DPickSensorNode.process in the browser's set orders.
    fn picking_pass(&mut self) {
        if self.rt.transform_sensors.is_empty() && self.rt.pick_sensors.is_empty() {
            return;
        }

        let sensors = self.rt.transform_sensors.clone();
        let targets: std::collections::HashSet<NodeId> = sensors
            .iter()
            .filter_map(|&s| self.rt.transform_sensor_target(s))
            .collect();
        let pick = crate::picking::PickInput::new(&self.rt);
        // X3DLayerNode.picking: the layers drawn whose `pickable` is TRUE.
        let mut scan = Scan::default();
        let home = self.current;

        for key in self.drawn.clone() {
            if key.is_some_and(|n| !self.rt.get_bool(n, "pickable")) {
                continue;
            }

            // X3DLayerNode constructor: the layer's group is a PickableGroup when its scene has
            // the Picking component.
            let ctx = key.map_or(self.rt.scene.world, |n| self.rt.scene.node(n).ctx);
            let pickable_layer = self.rt.scene.has_component_in(ctx, "Picking");

            self.swap_to(key);
            let object_type = key.and_then(|n| self.rt.layer_object_types.get(&n).cloned());
            let s = crate::walk::scan_picking(
                &self.rt,
                &self.layer_walk(),
                &targets,
                &pick,
                pickable_layer,
                object_type.as_deref(),
            );
            scan.transform_sensors.extend(s.transform_sensors);
            scan.pick_targets.extend(s.pick_targets);
            scan.pick_sensor_matrices.extend(s.pick_sensor_matrices);
            scan.pick_collected.extend(s.pick_collected);
        }

        self.swap_to(home);
        self.rt.now = self.time;

        for s in sensors {
            let models = scan
                .transform_sensors
                .iter()
                .filter(|e| e.node.node == s)
                .map(|e| e.model_view)
                .collect();
            let target = self.rt.transform_sensor_target(s);
            let target_matrices = scan
                .pick_targets
                .iter()
                .filter(|(n, _)| Some(*n) == target)
                .map(|(_, m)| *m)
                .collect();
            self.rt.transform_sensor_process(s, models, target_matrices);
        }

        if pick.sensors.is_empty() {
            return;
        }

        let mut geometry = HashMap::new();

        for g in crate::picking::geometry_nodes(&self.rt, &pick.sensors, &scan.pick_collected) {
            let built = self
                .geom_cache
                .entry((vec![], g))
                .or_insert_with(|| {
                    x3d_render::traverse::geometry_of(&self.rt, g)
                        .filter(|g| g.vertex_count() > 0)
                        .map(Rc::new)
                })
                .clone();
            geometry.insert(g, built);
        }

        for s in &pick.sensors {
            let models: Vec<Matrix4> = scan
                .pick_sensor_matrices
                .iter()
                .filter(|(n, _)| *n == s.node)
                .map(|(_, m)| *m)
                .collect();
            let collected = scan
                .pick_collected
                .iter()
                .filter(|(n, _, _)| *n == s.node)
                .map(|(_, g, m)| (*g, *m))
                .collect();

            match self.rt.builtin_name(s.node) {
                Some("LinePickSensor") => crate::picking::line_pick_process(
                    &mut self.rt,
                    &geometry,
                    s.node,
                    &models,
                    collected,
                ),
                Some("PointPickSensor" | "PrimitivePickSensor" | "VolumePickSensor") => {
                    crate::picking::volume_pick_process(
                        &mut self.rt,
                        &mut self.pick_shapes,
                        &geometry,
                        s.node,
                        &models,
                        collected,
                    )
                }
                _ => {}
            }
        }
    }

    fn sensor_pass(&mut self) {
        let home = self.current;

        for st in self.prox.values_mut() {
            st.seen_camera = false;
        }

        // ProximitySensor.update uses the viewpoint of the layer that traversed it.
        for key in self.drawn.clone() {
            self.swap_to(key);
            self.sensor_pass_layer(Some(key));
        }

        self.swap_to(home);
        self.sensor_pass_layer(None);
    }

    /// The sensors of layer `only`; with `None` the ProximitySensors of no layer drawn, then
    /// the VisibilitySensors.
    fn sensor_pass_layer(&mut self, only: Option<LayerKey>) {
        let now = self.time;
        let vp_node = self.layer.viewpoint();
        let base = self.base();
        let vpv = self.layer.vp(vp_node).clone();
        let drawn = self.drawn.clone();

        let cam_prox: HashMap<(Vec<NodeId>, NodeId), Matrix4> = self
            .scan_cam
            .proximity
            .iter()
            .map(|e| ((e.node.path.to_vec(), e.node.node), e.model_view))
            .collect();
        let cam_vis: Vec<(Vec<NodeId>, NodeId)> = self
            .scan_cam
            .visibility
            .iter()
            .map(|e| (e.node.path.to_vec(), e.node.node))
            .collect();

        let keys: Vec<_> = self.prox.keys().cloned().collect();

        for key in keys {
            let st = self.prox.get_mut(&key).unwrap();

            if !st.registered {
                continue;
            }

            match only {
                Some(layer) if st.layer != layer => continue,
                None if drawn.contains(&st.layer) => continue,
                _ => {}
            }

            let seen = cam_prox.get(&key).copied();
            let (inside, traversed) = (st.inside, st.traversed);
            let Some(rt) = rt_at_mut(&mut self.rt, &key.0) else {
                continue;
            };
            let n = key.1;

            if inside && traversed && seen.is_some() && rt.get_bool(n, "enabled") {
                let model = seen.unwrap();
                let mut inv_model = model;
                inv_model.inverse();

                let mut cor_m = vpv.model;
                cor_m.translate(&vpv.user_center_of_rotation(&base));
                cor_m.mult_right(&inv_model);
                // ProximitySensor.update: Matrix4.getTransform (the full decomposition).
                let mut cor = Vector3::ZERO;
                cor_m.get_transform(Some(&mut cor), None, None, None, None);

                let mut m = inv_model;
                m.mult_left(&vpv.camera_space);
                let (mut position, mut orientation) = (Vector3::ZERO, Rotation4::identity());
                m.get_transform(
                    Some(&mut position),
                    Some(&mut orientation),
                    None,
                    None,
                    None,
                );
                let (position, orientation) = (position, orientation);

                let set = |rt: &mut Runtime, name: &str, v: FieldValue| {
                    if let Some(k) = rt.pre(n, name) {
                        rt.set_value(k, v);
                    }
                };

                // GeoProximitySensor.set_position__: geoCoord_changed follows every position_changed.
                let geo = rt.builtin_name(n) == Some("GeoProximitySensor");
                let set_position = |rt: &mut Runtime| {
                    set(rt, "position_changed", FieldValue::SFVec3f(position));

                    if geo {
                        let g = rt.geo_object(n).get_geo_coord(&position);
                        set(rt, "geoCoord_changed", FieldValue::SFVec3d(g));
                    }
                };

                if rt.get_bool(n, "isActive") {
                    if !matches!(rt.pre_value(n, "position_changed"), Some(FieldValue::SFVec3f(p)) if p.equals(&position))
                    {
                        set_position(rt);
                    }

                    if !matches!(rt.pre_value(n, "orientation_changed"), Some(FieldValue::SFRotation(o)) if o.equals(&orientation))
                    {
                        set(
                            rt,
                            "orientation_changed",
                            FieldValue::SFRotation(orientation),
                        );
                    }

                    if !matches!(rt.pre_value(n, "centerOfRotation_changed"), Some(FieldValue::SFVec3f(p)) if p.equals(&cor))
                    {
                        set(rt, "centerOfRotation_changed", FieldValue::SFVec3f(cor));
                    }
                } else {
                    set(rt, "isActive", FieldValue::SFBool(true));
                    set(rt, "enterTime", FieldValue::SFTime(now));
                    set_position(rt);
                    set(
                        rt,
                        "orientation_changed",
                        FieldValue::SFRotation(orientation),
                    );
                    set(rt, "centerOfRotation_changed", FieldValue::SFVec3f(cor));
                }
            } else if rt.get_bool(n, "isActive") {
                if let Some(k) = rt.pre(n, "isActive") {
                    rt.set_value(k, FieldValue::SFBool(false));
                }
                if let Some(k) = rt.pre(n, "exitTime") {
                    rt.set_value(k, FieldValue::SFTime(now));
                }
            }

            let st = self.prox.get_mut(&key).unwrap();
            st.inside = false;

            // setTraversed (false): an untraversed sensor stops updating.
            if !st.traversed {
                st.registered = false;
            }

            st.traversed = false;
        }

        if only.is_some() {
            return;
        }

        let keys: Vec<_> = self.vis.keys().cloned().collect();

        for key in keys {
            let st = self.vis.get_mut(&key).unwrap();

            if !st.registered {
                continue;
            }

            let (visible, traversed) = (st.inside, st.traversed);
            let _ = &cam_vis;
            let Some(rt) = rt_at_mut(&mut self.rt, &key.0) else {
                continue;
            };
            let n = key.1;

            if visible && traversed && rt.get_bool(n, "enabled") {
                if !rt.get_bool(n, "isActive") {
                    if let Some(k) = rt.pre(n, "isActive") {
                        rt.set_value(k, FieldValue::SFBool(true));
                    }
                    if let Some(k) = rt.pre(n, "enterTime") {
                        rt.set_value(k, FieldValue::SFTime(now));
                    }
                }
            } else if rt.get_bool(n, "isActive") {
                if let Some(k) = rt.pre(n, "isActive") {
                    rt.set_value(k, FieldValue::SFBool(false));
                }
                if let Some(k) = rt.pre(n, "exitTime") {
                    rt.set_value(k, FieldValue::SFTime(now));
                }
            }

            let st = self.vis.get_mut(&key).unwrap();
            st.inside = false;

            if !st.traversed {
                st.registered = false;
            }

            st.traversed = false;
        }
    }

    // --- viewpoints --------------------------------------------------------------------------

    /// X3DLayerNode.getUserViewpoints: viewpoints of the world with a description.
    /// X3DLayerNode.getUserViewpoints: the viewpoints with a description, without the ones of
    /// Inline scenes when the EnableInlineViewpoints option is FALSE.
    pub fn user_viewpoints(&self) -> Vec<NodeId> {
        let inline_viewpoints = self
            .rt
            .browser_options
            .get_option("EnableInlineViewpoints")
            .is_some_and(|v| v.as_bool());

        self.layer
            .viewpoint_list()
            .iter()
            .copied()
            .filter(|n| !is_default(*n) && !self.layer.descriptions(*n).is_empty())
            .filter(|n| {
                inline_viewpoints
                    || self.rt.scene.scene_root(self.rt.scene.node(*n).ctx) == self.rt.scene.world
            })
            .collect()
    }

    /// ContextMenu: the viewpoint's descriptions joined by " » " (with the enclosing
    /// ViewpointGroup descriptions first); its own description before the first CAMERA pass.
    pub fn viewpoint_description(&self, n: NodeId) -> String {
        match self.layer.descriptions(n) {
            [] => VpBase::read(&self.rt, n).description,
            d => d.join(" » "),
        }
    }

    /// X3DBrowser.changeViewpoint: the first viewpoint of the active layer with the name gets `set_bind`
    /// (with the VRML transition), the field event is processed by the next frame.
    pub fn change_viewpoint(&mut self, name: &str) {
        let found = self
            .layer
            .viewpoint_list()
            .iter()
            .copied()
            .find(|n| !is_default(*n) && self.rt.scene.node(*n).name == name);

        if let Some(n) = found {
            self.layer.vp(n).vrml_transition = true;

            if let Some(k) = self.rt.pre(n, "set_bind") {
                self.rt.set_value(k, FieldValue::SFBool(true));
            }
        }
    }

    /// X3DBrowser.bindViewpoint.
    pub fn bind_viewpoint(&mut self, n: NodeId) {
        self.layer.vp(n).vrml_transition = true;

        if let Some(k) = self.rt.pre(n, "set_bind") {
            self.rt.set_value(k, FieldValue::SFBool(true));
            self.rt.process_events();
        }
    }

    fn bound_index(&self, list: &[NodeId]) -> Option<usize> {
        list.iter().position(|n| self.rt.get_bool(*n, "isBound"))
    }

    pub fn next_viewpoint(&mut self, delta: i32) {
        let list = self.user_viewpoints();

        if list.is_empty() {
            return;
        }

        let i = self.bound_index(&list);
        let len = list.len() as i32;
        let target = if delta >= 0 {
            // (i + 1) % length, i = -1 when none is bound.
            (i.map(|i| i as i32).unwrap_or(-1) + 1) % len
        } else {
            // previous: at (i > -1 ? i - 1 : -1)
            match i {
                Some(i) => (i as i32 - 1).rem_euclid(len),
                None => len - 1,
            }
        };
        self.bind_viewpoint(list[target as usize]);
    }

    /// X3DBrowser.viewAll → X3DLayerNode.viewAll ({ transitionTime, straighten: StraightenHorizon }): the layer's viewpoint
    /// looks at the layer's bounding box (lookAtBBox, factor 1). `layer` None: the active layer.
    pub fn browser_view_all(&mut self, layer: Option<NodeId>, transition_time: f64) {
        let home = self.current;

        self.swap_to(layer.map_or(self.home_layer(), Some));

        if let Some(bbox) = self.scene_bbox() {
            let vp_node = self.layer.viewpoint();
            let base = self.base();
            let straighten = self.rt.browser_options.straighten_horizon();
            let date_now = self.date_now;

            self.layer.look_at_bbox(
                &mut self.rt,
                vp_node,
                &base,
                &bbox,
                1.0,
                transition_time,
                straighten,
                date_now,
            );
        }

        self.swap_to(home);
    }

    pub fn first_viewpoint(&mut self) {
        if let Some(n) = self.user_viewpoints().first().copied() {
            self.bind_viewpoint(n);
        }
    }

    pub fn last_viewpoint(&mut self) {
        if let Some(n) = self.user_viewpoints().last().copied() {
            self.bind_viewpoint(n);
        }
    }

    /// The bound viewpoint's user position and orientation (what the navigation oracle compares).
    pub fn user_pose(&mut self) -> (Vector3, Rotation4, Vector3) {
        if let Some(h) = &self.held_pose {
            return h.pose;
        }

        let base = self.base();
        let vp = self.layer.vp(self.layer.viewpoint());
        (
            vp.user_position(&base),
            vp.user_orientation(&base),
            vp.user_center_of_rotation(&base),
        )
    }

    // --- input -------------------------------------------------------------------------------

    pub fn input(&mut self, ev: UiEvent) {
        // A slot released since the last frame may have been handed out again (a load that
        // completed, a Script that created nodes): the pointer must not reach the new node.
        self.forget_released();
        self.input_event(ev);

        // The event handler returned: pending microtasks run and the loads they wait for complete.
        self.rt.settle();
        self.sync_active_layer();
    }

    fn input_event(&mut self, ev: UiEvent) {
        match ev {
            UiEvent::Mods { shift, ctrl, alt } => {
                self.shift = shift;
                self.ctrl = ctrl;
                self.alt = alt;
            }
            UiEvent::KeyDown { key } => {
                self.key_event(true, &key, x3d_runtime::nodes::key_sensor::key_code(&key))
            }
            UiEvent::KeyUp { key } => {
                self.key_event(false, &key, x3d_runtime::nodes::key_sensor::key_code(&key))
            }
            UiEvent::NativeKey { down, key, code } => self.key_event(down, &key, code),
            UiEvent::Key(k) => match k {
                Key::Home => self.first_viewpoint(),
                Key::End => self.last_viewpoint(),
                Key::PageUp => self.next_viewpoint(-1),
                Key::PageDown => self.next_viewpoint(1),
            },
            UiEvent::MouseDown { button, x, y } => self.mouse_down(button, x, y),
            UiEvent::MouseUp { button, x, y } => self.mouse_up(button, x, y),
            UiEvent::MouseMove { x, y } => self.mouse_move(x, y),
            UiEvent::Wheel { delta_y, x, y } => self.viewer_input(Input::Wheel { delta_y, x, y }),
            UiEvent::DblClick { x, y } => self.viewer_input(Input::DblClick { x, y }),
        }
    }

    fn viewer_input(&mut self, ev: Input) {
        // No displayed active layer: nothing to navigate.
        if self.active.is_none() {
            return;
        }

        // The viewers' mousedown and wheel: only inside the active layer's viewport
        // (X3DViewer.isPointerInRectangle).
        if let Input::MouseDown { x, y, .. } | Input::Wheel { x, y, .. } = ev {
            if !layers::pointer_in_rectangle(&self.layer_rect(self.current), x, y) {
                return;
            }
        }

        let base = self.base();
        let nav = self.nav_info();
        let env = self.env();
        let vp_node = self.layer.viewpoint();
        let world = self.collision_world();
        let (bb, nv) = (base.clone(), nav.clone());
        let vp = self.layer.vp(vp_node);
        let wall_friction = self.rt.browser_options.wall_friction();
        let mut constrain = |t: Vector3, v: &crate::layer::Vp| match &world {
            Some(w) => w.constrain_translation(v, &bb, &nv, t, true, true, wall_friction),
            None => t,
        };
        let mut cx = Cx {
            env: &env,
            base: &base,
            vp,
            nav: &nav,
            constrain: &mut constrain,
            requests: vec![],
        };
        self.nav.input(&ev, &mut cx);
        let requests = cx.requests;

        for r in requests {
            self.look_at_request(r);
        }
    }

    /// X3DViewer.lookAtPoint / lookAtBBox: `browser .touch (x, y)` (false while the viewer is active or
    /// nothing is hit), then the viewpoint's lookAt.
    fn look_at_request(&mut self, r: LookAtRequest) {
        if self.nav.is_active() {
            return;
        }

        let hit = self.do_pick(r.x, r.y);

        if !hit.id {
            return;
        }

        let vp_node = self.layer.viewpoint();
        let base = self.base();
        let date_now = self.date_now;

        if r.point {
            self.layer.look_at_point(
                &mut self.rt,
                vp_node,
                &base,
                hit.point,
                r.straighten,
                date_now,
            );
        } else if let Some((min, max)) = hit.bbox {
            // hit .modelViewMatrix is a Float32Array copy of the pointing context's matrix.
            let mut mv = hit.model_view;
            mv.0 = mv.0.map(|v| v as f32 as f64);
            let camera_space = self.layer.vp(vp_node).camera_space;
            let mut bbox = Box3::from_extents(&min, &max);
            bbox.mult_right(&mv).mult_right(&camera_space);
            self.layer.look_at_bbox(
                &mut self.rt,
                vp_node,
                &base,
                &bbox,
                0.8,
                1.0,
                r.straighten,
                date_now,
            );
        }
    }

    /// The bound viewpoint's fieldOfViewScale and, for an OrthoViewpoint, its fieldOfViewOffset.
    pub fn field_of_view_state(&mut self) -> (f64, Option<[f64; 4]>) {
        let base = self.base();
        let vp = self.layer.vp(self.layer.viewpoint());
        (
            vp.field_of_view_scale,
            base.is_ortho.then_some(vp.field_of_view_offset),
        )
    }

    fn has_sensors(&self) -> bool {
        self.scan_display.has_sensors() || self.others.iter().any(|l| l.scan_display.has_sensors())
    }

    /// X3DPointingDeviceSensorContext.touch: the POINTER traversal of every layer drawn, in
    /// order, into one pointing buffer: each layer clears the depth, so the last layer with a
    /// shape under the pointer wins. A layer takes part with `pointerEvents` TRUE and the pointer
    /// in its viewport, or while it holds the pointer (pointingLayer: only that one).
    /// `layerNode .getClosestObject (direction)` for the oracle's `collisionProbes`: the collision
    /// shapes of the last traversal drawn into the depth buffer toward `direction`.
    pub fn probe_closest(&mut self, direction: Vector3) -> crate::collision::Closest {
        let base = self.base();
        let nav = self.nav_info();
        let vp = self.layer.vp(self.layer.viewpoint()).clone();

        match self.collision_world_unconditional() {
            Some(w) => w.closest_object(&vp, &base, &nav, &direction),
            None => crate::collision::Closest {
                shape: None,
                distance: 0.0,
                normal: Vector3::ZERO,
                hit: false,
            },
        }
    }

    /// `layerNode .constrainTranslation (translation)` for the oracle's `collisionProbes`: the move the collision shapes
    /// of the last traversal let the avatar make.
    pub fn probe_constrain(&mut self, translation: Vector3) -> Vector3 {
        let base = self.base();
        let nav = self.nav_info();
        let vp = self.layer.vp(self.layer.viewpoint()).clone();
        let wall_friction = self.rt.browser_options.wall_friction();

        match self.collision_world_unconditional() {
            Some(w) => {
                w.constrain_translation(&vp, &base, &nav, translation, true, true, wall_friction)
            }
            None => translation,
        }
    }

    /// `renderObject .collisionShapes` for the oracle's `collisionProbes`: the Shape or ParticleSystem of every collision
    /// shape the depth pass draws, once per shape (the particles of a system are one X_ITE collision shape), and the
    /// node of each [`crate::collision::Closest::shape`] index.
    pub fn probe_collision_shapes(
        &mut self,
    ) -> (Vec<crate::walk::NodeRef>, Vec<crate::walk::NodeRef>) {
        let shapes: Vec<crate::walk::NodeRef> = self
            .collision_world_unconditional()
            .map(|w| w.nodes())
            .unwrap_or_default();
        let mut once: Vec<crate::walk::NodeRef> = vec![];

        for s in &shapes {
            if once.last() != Some(s) {
                once.push(s.clone());
            }
        }

        (once, shapes)
    }

    /// The bound viewpoint's positionOffset (WALK gravity moves it).
    pub fn position_offset(&mut self) -> Vector3 {
        let vp_node = self.layer.viewpoint();

        self.layer.vp(vp_node).position_offset
    }

    /// The pointer's walk again (the one each frame takes before its sensor events), for tools that change the scene
    /// state between frames and then pick: x3d-particles-drive puts reference particle records in place.
    pub fn refresh_pointer_scan(&mut self) {
        self.refresh_display_scan();
    }

    /// The pointing buffer and the collision depth buffer on the GPU (`None`: the CPU ray cast and depth stand-in).
    /// Both have to be on `device`, the one that draws the frame, so that no resource of one device meets another
    /// device. A caller that draws no frames of its own (the headless drivers) passes the device it made them on.
    pub fn set_gpu(
        &mut self,
        device: &x3d_render::wgpu::Device,
        pointing: Option<x3d_render::pointing_gpu::PointingGpu>,
        depth: Option<std::rc::Rc<std::cell::RefCell<x3d_render::depth_gpu::DepthGpu>>>,
    ) -> Result<(), String> {
        // Each against the frame's device, not only against each other: two buffers that agree on a device the frame
        // does not draw on are the very case this has to refuse.
        if pointing.as_ref().is_some_and(|p| p.device() != device) {
            return Err("the pointing buffer is on another device than the frame".into());
        }

        if depth
            .as_ref()
            .is_some_and(|d| d.borrow().device() != device)
        {
            return Err("the collision depth buffer is on another device than the frame".into());
        }

        self.pointing_gpu = pointing;
        self.depth_gpu = depth;
        Ok(())
    }

    /// The pointing buffer this viewer picks with, while it is on the GPU.
    pub fn pointing_gpu(&self) -> Option<&x3d_render::pointing_gpu::PointingGpu> {
        self.pointing_gpu.as_ref()
    }

    /// The collision depth buffer this viewer collides with, while it is on the GPU.
    pub fn depth_gpu(
        &self,
    ) -> Option<&std::rc::Rc<std::cell::RefCell<x3d_render::depth_gpu::DepthGpu>>> {
        self.depth_gpu.as_ref()
    }

    /// Take both buffers back, for an owner that keeps them across the worlds it loads: picks and collisions are on the
    /// CPU until the next [`Viewer3d::set_gpu`].
    pub fn take_gpu(
        &mut self,
    ) -> (
        Option<x3d_render::pointing_gpu::PointingGpu>,
        Option<std::rc::Rc<std::cell::RefCell<x3d_render::depth_gpu::DepthGpu>>>,
    ) {
        (self.pointing_gpu.take(), self.depth_gpu.take())
    }

    /// `browser .touch (x, y)` for the oracle's `pickProbes`: the hit, without any sensor event.
    pub fn probe_pick(&mut self, x: f64, y: f64) -> Hit {
        self.do_pick(x, y)
    }

    fn do_pick(&mut self, x: f64, y: f64) -> Hit {
        // A lost device: the ray cast from now on.
        if self.pointing_gpu.as_ref().is_some_and(|g| g.lost()) {
            self.pointing_gpu = None;
        }

        let home = self.current;
        let mut hit: Option<Hit> = None;

        for key in self.drawn.clone() {
            if !self.pointer_events(key) {
                continue;
            }

            match self.pointing_layer {
                Some(p) if p != key => continue,
                Some(_) => {}
                None if !layers::pointer_in_rectangle(&self.layer_rect(key), x, y) => continue,
                None => {}
            }

            self.swap_to(key);
            let cam = self.pick_camera();
            let scan = std::mem::take(&mut self.scan_display);
            // The ParticleSystems go in as runs: the pointing pass draws each at once, and only the CPU ray cast
            // expands them into a shape per particle (crate::pointer::PointingShapes).
            let shapes = crate::pointer::PointingShapes {
                shapes: &scan.shapes,
                particles: &scan.particle_pointing,
            };
            let mut cache = std::mem::take(&mut self.geom_cache);
            let rt = &self.rt;
            let mut h = pick(
                shapes,
                &mut |s| geom_of(rt, &mut cache, s),
                &cam,
                x,
                y,
                self.pointing_layer.is_some(),
                self.pointing_gpu.as_mut(),
            );
            self.geom_cache = cache;
            self.scan_display = scan;
            h.layer = key;

            // No hit: the ray of the pointing layer (hit .pointingLayerNode .getHitRay ()).
            if h.id || hit.as_ref().is_none_or(|b| !b.id) {
                hit = Some(h);
            }
        }

        self.swap_to(home);

        match hit {
            Some(h) => h,
            // The pointing layer is not drawn any more: no hit, and its last hit ray (hit .ray
            // .assign (hit .pointingLayerNode .getHitRay ()), not traversed since).
            None => match self.pointing_layer {
                Some(layer) => Hit {
                    pointer: x3d_fields::math::Vector2::new(x, y),
                    ray: self.pointer.hit.ray.copy(),
                    view_matrix: self.pointer.hit.view_matrix,
                    layer,
                    ..Hit::default()
                },
                None => Hit {
                    pointer: x3d_fields::math::Vector2::new(x, y),
                    ..Hit::default()
                },
            },
        }
    }

    fn collect_pointer_output(&mut self) {
        let out = std::mem::take(&mut self.pointer.out);

        if let Some(d) = out.description {
            // X3DPointingDeviceSensorNode.set_over__: `getBrowser () .setDescription (this ._description)`.
            self.rt.description = d.clone();
            self.status_description = d.clone();
            self.status = d;
        }

        for a in out.anchors {
            self.status = format!(
                "Anchor: {} -> {}",
                if a.description.is_empty() {
                    "(no description)"
                } else {
                    &a.description
                },
                a.urls.first().cloned().unwrap_or_default()
            );

            // Anchor.requestImmediateLoad: FileLoader.createX3DFromURL (url, parameter, replaceWorld, changeViewpoint,
            // window.open): a url that only names a viewpoint of this world changes the viewpoint, a scene replaces the
            // world once it is loaded (`Runtime::anchor_load`), the urls are tried in turn. A foreign target (another
            // window, a page) is only reported: the viewer opens nothing.
            // The sensor's events are processed with the next events of the browser, which is where the interest of the
            // Anchor's TouchSensor (`set_active__`) runs (see [`Viewer3d::phase`]).
            self.pending_anchors
                .push((a.node.node, a.urls.clone(), a.parameters.clone()));

            self.anchors.push(a);
        }
    }

    /// The element's keydown / keyup listeners: X3DCoreContext (modifier state, viewpoint keys) and
    /// X3DKeyDeviceSensorContext (the key device sensors, then processEvents).
    fn key_event(&mut self, down: bool, key: &str, code: u32) {
        let e = x3d_runtime::nodes::key_sensor::KeyEvent {
            key: key.to_string(),
            code,
        };

        match (e.code, down) {
            (16, d) => self.shift = d,
            (17, d) => self.ctrl = d,
            (18, d) => self.alt = d,
            (36, true) => self.first_viewpoint(),
            (35, true) => self.last_viewpoint(),
            (33, true) => self.next_viewpoint(-1),
            (34, true) => self.next_viewpoint(1),
            _ => {}
        }

        // Must advance time, because the events are processed now (advanceOnlyTime).
        if self.advance_only_time {
            self.rt.now = self.date_now;
        }

        self.rt.key_event(down, &e);
        // The Scripts ran in that processEvents: their requests of the browser (Browser.viewAll ...) run now, at the time of
        // the event, as X_ITE calls the browser from inside the Script.
        self.browser_requests();
    }

    fn mouse_down(&mut self, button: u8, x: f64, y: f64) {
        if button == 0 && self.has_sensors() {
            if self.advance_only_time {
                self.rt.now = self.date_now;
            }

            let hit = self.do_pick(x, y);
            let cam = self.pick_camera_of(hit.layer);
            let now = if self.advance_only_time {
                self.date_now
            } else {
                self.time
            };
            let id = hit.id;
            let layer = hit.layer;
            let pressed = self.pointer.button_press(&mut self.rt, hit, &cam, now);

            // hit .pointingLayerNode = hit .layerNode.
            if id {
                self.pointing_layer = Some(layer);
            }

            self.collect_pointer_output();
            self.browser_requests();

            if pressed {
                // PointingDevice.mousedown → onverifymotion: motion after the next frame.
                self.verify_motion = Some((x, y));
                return;
            }
        }

        self.viewer_input(Input::MouseDown { button, x, y });
    }

    fn mouse_up(&mut self, button: u8, x: f64, y: f64) {
        if button == 0 && self.pointer.is_active() {
            let cam = self.pick_camera_of(self.pointing_layer.unwrap_or(self.pointer.hit.layer));
            let now = if self.advance_only_time {
                self.date_now
            } else {
                self.time
            };
            self.pointing_layer = None;
            self.pointer.button_release(&mut self.rt, &cam, now);
            self.collect_pointer_output();
            self.browser_requests();
            // PointingDevice.onverifymotion: motion after the next frame.
            self.verify_motion = Some((x, y));
        }

        self.viewer_input(Input::MouseUp { button, x, y });
    }

    fn mouse_move(&mut self, x: f64, y: f64) {
        self.pointer_motion(x, y);
        self.viewer_input(Input::MouseMove { x, y });
    }

    fn pointer_motion(&mut self, x: f64, y: f64) {
        if !self.has_sensors() && !self.pointer.is_active() {
            return;
        }

        let hit = if self.nav.is_active() {
            self.pointer.hit.clone()
        } else {
            self.do_pick(x, y)
        };
        let cam = self.pick_camera_of(self.pointing_layer.unwrap_or(hit.layer));
        let now = if self.advance_only_time {
            self.date_now
        } else {
            self.time
        };
        self.pointer.motion(&mut self.rt, hit, &cam, now);
        self.collect_pointer_output();
        self.browser_requests();
    }

    // --- rendering ---------------------------------------------------------------------------

    /// The frame for the renderer: the bound viewpoint's camera and bindables.
    /// ScreenGroup.matrix of each ScreenGroup's last DISPLAY traversal (the last `build_frame`).
    fn screen_matrices(&self) -> std::collections::HashMap<NodeId, Matrix4> {
        Builder::screen_matrices_of(&self.render_cache, self.rt.uid)
    }

    pub fn build_frame(&mut self) -> Frame {
        let mut cameras = vec![(
            self.current,
            self.camera_override(&self.layer, &self.scan_cam, self.current),
        )];

        for s in &self.others {
            cameras.push((s.key, self.camera_override(&s.layer, &s.scan_cam, s.key)));
        }

        let (width, height) = self.framebuffer_size();
        let mut b = Builder::new(&self.rt).with_cache(&mut self.render_cache);
        b.camera = cameras
            .iter()
            .find(|(k, _)| k.is_none())
            .map(|(_, c)| c.clone());
        b.layer_cameras = cameras.into_iter().filter(|(k, _)| k.is_some()).collect();
        // X3DRenderingContext.reshape: the viewport, projection and ScreenGroup / ScreenText scale are in drawing-buffer pixels
        // (the canvas size × ContentScale).
        b.build(width, height)
    }

    /// The bound viewpoint's camera and the bindables of a layer for the renderer.
    fn camera_override(&self, layer: &Layer, scan_cam: &Scan, key: LayerKey) -> CameraOverride {
        let vp_node = layer.viewpoint();
        let nav_node = layer.navigation_info();
        let base = VpBase::read(&self.rt, vp_node);
        let nav = NavInfo::read(&self.rt, nav_node);
        let vp = layer.vps.get(&vp_node).cloned().unwrap_or_default();
        let bg = layer.background();
        let fog = layer.fog();
        let r = self.layer_rect(key);

        let background = if bg == DEFAULT {
            None
        } else {
            scan_cam
                .backgrounds
                .iter()
                .find(|(n, _)| n.node == bg)
                .map(|(n, m)| (n.node, *m))
        };

        CameraOverride {
            camera_space: vp.camera_space,
            fov: vp.user_fov(&base),
            near: vp.near_distance(&base, &nav),
            far: vp.far_distance(&base, &nav),
            projection: base.is_ortho.then(|| {
                vp.projection(
                    &base,
                    vp.near_distance(&base, &nav),
                    vp.far_distance(&base, &nav),
                    r[2],
                    r[3],
                )
            }),
            headlight: nav.headlight,
            viewpoint: (!is_default(vp_node)).then_some(vp_node),
            navigation_info: (nav_node != DEFAULT).then_some(nav_node),
            background,
            fog: (fog != DEFAULT).then_some(fog),
            // X3DBackgroundNode scales the background by getMaxFarValue (the OrthoViewpoint's
            // 1e4 is not used here: the background radius stays 1e5 for it).
            max_far: if base.geo.is_some() {
                1e9
            } else {
                crate::layer::max_far()
            },
            ortho_size: base
                .is_ortho
                .then(|| (vp.user_size_x(&base), vp.user_size_y(&base))),
        }
    }

    pub fn log(&self) -> &[String] {
        &self.rt.log
    }

    pub fn node_ref(&self, n: NodeId) -> NodeRef {
        NodeRef {
            path: Rc::new(vec![]),
            node: n,
        }
    }
}

/// Conservative view frustum test of a box (VisibilitySensor.traverse → ViewVolume.intersectsBox
/// uses a SAT test; this rejects only boxes fully outside one frustum plane).
fn box_in_frustum(size: &Vector3, center: &Vector3, model_view: &Matrix4, proj: &Matrix4) -> bool {
    let mut outside = [true; 6];

    for i in 0..8 {
        let mut p = Vector3::new(
            center.x + if i & 1 == 0 { -size.x } else { size.x } / 2.0,
            center.y + if i & 2 == 0 { -size.y } else { size.y } / 2.0,
            center.z + if i & 4 == 0 { -size.z } else { size.z } / 2.0,
        );
        model_view.mult_vec_matrix3(&mut p);
        let mut c = Vector4::new(p.x, p.y, p.z, 1.0);
        proj.mult_vec_matrix4(&mut c);
        let planes = [
            c.x >= -c.w,
            c.x <= c.w,
            c.y >= -c.w,
            c.y <= c.w,
            c.z >= -c.w,
            c.z <= c.w,
        ];

        for (k, inside) in planes.iter().enumerate() {
            if *inside {
                outside[k] = false;
            }
        }
    }

    !outside.iter().any(|o| *o)
}

/// decodeURIComponent: %XX sequences as UTF-8 bytes (an invalid sequence is kept as it is).
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let (mut out, mut i) = (Vec::with_capacity(b.len()), 0);

    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && b.get(i + 1)
                .zip(b.get(i + 2))
                .is_some_and(|(x, y)| x.is_ascii_hexdigit() && y.is_ascii_hexdigit())
        {
            out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }

    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}
