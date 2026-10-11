//! The viewer core: a live world, the per-frame order of X3DBrowserContext [_traverse], the
//! bound viewpoint, the navigation viewer and the pointing device. It runs without a window:
//! the caller supplies the clock (`step (now)`) and the input events, so a run is
//! deterministic.

use crate::collision::{CShape, World};
use crate::layer::{BBox, DEFAULT, Layer, NavInfo, VpBase, bbox_transform, factor_simple};
use crate::lookat::Box3;
use crate::nav::{Cx, Env, Input, LookAtRequest, Viewer};
use crate::pointer::{AnchorActivation, Hit, PickCamera, Pointer, pick};
use crate::walk::{NodeRef, Scan, ShapeInst, rt_at, rt_at_mut, scan};
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use x3d_fields::math::{Matrix4, Rotation4, Vector3, Vector4};
use x3d_fields::{FieldValue, NodeId};
use x3d_render::geometry::Geometry;
use x3d_render::traverse::{Builder, CameraOverride, Frame};
use x3d_runtime::{LocalFetcher, Runtime};
use x3d_script::compare::{ORACLE_PREFIX, encode_path};

const GRAVITY: f64 = 9.80665;
const WALL_FRICTION: f64 = 0.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Home,
    End,
    PageUp,
    PageDown,
}

/// A user event in viewport pixels (origin bottom left).
#[derive(Clone, Copy, Debug)]
pub enum UiEvent {
    MouseDown { button: u8, x: f64, y: f64 },
    MouseUp { button: u8, x: f64, y: f64 },
    MouseMove { x: f64, y: f64 },
    Wheel { delta_y: f64, x: f64, y: f64 },
    /// The "dblclick" DOM event (the harness sends the two mousedown / mouseup pairs before it).
    DblClick { x: f64, y: f64 },
    Key(Key),
    Mods { shift: bool, ctrl: bool, alt: bool },
}

#[derive(Default)]
struct ProxState {
    inside: bool,
    traversed: bool,
    registered: bool,
    seen_camera: bool,
}

pub struct Viewer3d {
    pub rt: Runtime,
    pub layer: Layer,
    pub nav: Viewer,
    nav_info_node: NodeId,
    pub pointer: Pointer,
    pub width: u32,
    pub height: u32,
    /// browser .getCurrentTime ().
    pub time: f64,
    /// Date.now () / 1000, settable between frames (input events use it).
    pub date_now: f64,
    pub frame_rate: f64,
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
    gravity_speed: f64,
    geom_cache: HashMap<(Vec<NodeId>, NodeId), Option<Rc<Geometry>>>,
    /// Geometry and textures kept between frames (rebuilt only when their nodes change).
    pub render_cache: x3d_render::traverse::RenderCache,
    pub scan_cam: Scan,
    pub scan_display: Scan,
    prox: HashMap<(Vec<NodeId>, NodeId), ProxState>,
    vis: HashMap<(Vec<NodeId>, NodeId), ProxState>,
    verify_motion: Option<(f64, f64)>,
    /// Status line text (Browser.description, sensor descriptions, anchor reports, errors).
    pub status: String,
    pub anchors: Vec<AnchorActivation>,
    pub frame_count: u64,
}

fn geom_of(root: &Runtime, cache: &mut HashMap<(Vec<NodeId>, NodeId), Option<Rc<Geometry>>>, s: &ShapeInst) -> Option<Rc<Geometry>> {
    let key = (s.shape.path.to_vec(), s.geometry);

    if let Some(g) = cache.get(&key) {
        return g.clone();
    }

    let rt = rt_at(root, &s.shape.path)?;
    let g = x3d_render::traverse::geometry_of(rt, s.geometry).filter(|g| g.vertex_count() > 0).map(Rc::new);
    cache.insert(key, g.clone());
    g
}

impl Viewer3d {
    /// Load a world like the oracle's FileLoader (parse, setup, loads) but live, then bind the
    /// first bindables (X3DBrowser.replaceWorld → X3DWorld.bindBindables).
    pub fn load(file: &Path, oracle_root: Option<&Path>, t0: f64, width: u32, height: u32) -> Result<Viewer3d, String> {
        Self::load_pinned(file, oracle_root, t0, width, height, None)
    }

    /// [`Viewer3d::load`] with `currentFrameRate` pinned (see [`Viewer3d::pinned_frame_rate`]) from
    /// the first frame on, including the frame that finishes loading.
    pub fn load_pinned(file: &Path, oracle_root: Option<&Path>, t0: f64, width: u32, height: u32, pinned_frame_rate: Option<f64>) -> Result<Viewer3d, String> {
        x3d_fields::jsmath::set_flavor(x3d_fields::jsmath::MathFlavor::Chrome);

        let file = std::fs::canonicalize(file).map_err(|e| format!("{}: {e}", file.display()))?;
        let found = file.ancestors().skip(1).find(|d| d.file_name().is_some_and(|n| n == "oracle")).map(|d| d.to_path_buf());
        let root_buf = match oracle_root {
            Some(r) => std::fs::canonicalize(r).map_err(|e| e.to_string())?,
            None => found.unwrap_or_else(|| Path::new("/").to_path_buf()),
        };
        let root = root_buf.as_path();
        let rel = file.strip_prefix(root).unwrap_or(&file).to_string_lossy().replace('\\', "/");
        let url = format!("{ORACLE_PREFIX}{}", encode_path(&rel));
        let data = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let scene = x3d_parse::parse_bytes(&data, &url).map_err(|e| format!("{e:?}"))?;
        let fetcher = Box::new(LocalFetcher { root: root.to_path_buf(), prefix: ORACLE_PREFIX.to_string() });
        let mut rt = Runtime::new(scene, fetcher);
        rt.script_host = Some(Box::new(x3d_script::QuickJsHost::new()));
        rt.live = true;
        rt.now = t0;

        if let Some(rate) = pinned_frame_rate {
            rt.frame_rate = rate;
        }

        rt.load_world();

        let mut v = Self::from_runtime(rt, t0, width, height);
        v.pinned_frame_rate = pinned_frame_rate;
        Ok(v)
    }

    pub fn from_runtime(rt: Runtime, t0: f64, width: u32, height: u32) -> Viewer3d {
        let mut v = Viewer3d {
            rt,
            layer: Layer::new(),
            nav: Viewer::None,
            nav_info_node: DEFAULT,
            pointer: Pointer::new(),
            width,
            height,
            time: t0,
            date_now: t0,
            frame_rate: 60.0,
            last_time: None,
            shift: false,
            ctrl: false,
            alt: false,
            straighten: true,
            advance_only_time: false,
            pinned_frame_rate: None,
            gravity_speed: 0.0,
            geom_cache: HashMap::new(),
            render_cache: Default::default(),
            scan_cam: Scan::default(),
            scan_display: Scan::default(),
            prox: HashMap::new(),
            vis: HashMap::new(),
            verify_motion: None,
            status: String::new(),
            anchors: vec![],
            frame_count: 0,
        };

        v.layer.viewport = (width as f64, height as f64);
        v.bind_world();
        v
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
        self.layer.viewport = (self.width as f64, self.height as f64);
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

    /// The scene bounding box in world space (X3DLayerNode.getBBox), from shape bounding boxes.
    fn scene_bbox(&mut self) -> Option<BBox> {
        let s = scan(&self.rt, &Matrix4::IDENTITY, true);
        let mut min = Vector3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut max = Vector3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        let mut any = false;

        for sh in &s.shapes {
            if let Some(g) = geom_of(&self.rt, &mut self.geom_cache, sh) {
                let b = bbox_transform(&BBox { min: g.min, max: g.max }, &sh.model_view);
                min.min(&b.min);
                max.max(&b.max);
                any = true;
            }
        }

        any.then_some(BBox { min, max })
    }

    fn bind_world(&mut self) {
        self.scan_cam = scan(&self.rt, &Matrix4::IDENTITY, true);
        let bbox = self.scene_bbox();
        let scan_cam = std::mem::take(&mut self.scan_cam);
        self.layer.bind_bindables(&mut self.rt, &scan_cam, None, &move || bbox);
        self.scan_cam = scan_cam;
        self.rt.process_events();
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

    /// X3DNavigationContext.set_activeNavigationInfo__ / set_viewer__.
    fn set_active_navigation_info(&mut self, force: bool) {
        let node = self.layer.navigation_info();

        if !force && node == self.nav_info_node {
            return;
        }

        self.nav_info_node = node;
        let kind = self.nav_info().viewer();
        self.set_viewer(&kind);
    }

    /// Switch the viewer (the context menu's viewer choice).
    pub fn set_viewer(&mut self, kind: &str) {
        let base = self.base();
        let nav = self.nav_info();
        let vp_node = self.layer.viewpoint();
        let env = self.env();
        let vp = self.layer.vp(vp_node);
        let mut constrain = |t: Vector3, _: &crate::layer::Vp| t;
        let mut cx = Cx { env: &env, base: &base, vp, nav: &nav, constrain: &mut constrain, requests: vec![] };
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
            viewport: self.viewport(),
            straighten: self.straighten,
        }
    }

    /// The pointing camera for the bound viewpoint.
    pub fn pick_camera(&mut self) -> PickCamera {
        let base = self.base();
        let nav = self.nav_info();
        let vp = self.layer.vp(self.layer.viewpoint()).clone();
        let (near, far) = (vp.near_distance(&base, &nav), vp.far_distance(&base, &nav));

        PickCamera {
            projection: vp.projection(&base, near, far, self.width as f64, self.height as f64),
            viewport: Vector4::new(0.0, 0.0, self.width as f64, self.height as f64),
            view: vp.view,
            near,
            far,
        }
    }

    // --- the frame -------------------------------------------------------------------------

    /// All runtimes (world and Inline scenes), parents first.
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
        self.for_each_rt(&mut |rt| {
            rt.now = now;
            rt.frame_rate = rate;
            rt.process_phase(name);
        });
    }

    /// One iteration of X3DBrowserContext [_traverse] at browser time `now`
    /// (advanceTime has set getCurrentTime to `now`).
    pub fn step(&mut self, now: f64) {
        if let Some(last) = self.last_time {
            let dt = now - last;

            if dt > 0.0 {
                self.frame_rate = 1.0 / dt;
            }
        }

        self.last_time = Some(now);
        self.time = now;
        self.date_now = self.date_now.max(now);
        self.geom_cache.clear();
        self.frame_count += 1;

        // Timeouts that fired between frames (setTimeout).
        self.for_each_rt(&mut |rt| rt.fire_timeouts(now));

        // Scripts' requests of the browser (Browser.nextViewpoint ...).
        for r in std::mem::take(&mut self.rt.browser_requests) {
            match r.as_str() {
                "nextViewpoint" => self.next_viewpoint(1),
                "previousViewpoint" => self.next_viewpoint(-1),
                "firstViewpoint" => self.first_viewpoint(),
                "lastViewpoint" => self.last_viewpoint(),
                _ => {}
            }
        }

        // Prepare and time events.
        self.phase("prepareEvents");
        self.prepare_viewer();
        self.phase("timePrepareEvents");
        self.phase("timeEvents");
        self.layer.tick(&mut self.rt);
        self.rt.process_events();

        // Camera.
        self.camera_pass();
        self.phase("cameraEvents");

        // Collision (gravity): the COLLISION traversal starts from the view of this frame.
        self.refresh_display_scan();
        self.collision_pass();

        // Sensor events.
        self.sensor_pass();
        self.phase("sensorEvents");

        // Display.
        self.phase("displayEvents");
        self.refresh_display_scan();
        self.display_pass();
        self.phase("finishEvents");

        if let Some((x, y)) = self.verify_motion.take() {
            self.pointer_motion(x, y);
        }

        for d in std::mem::take(&mut self.rt.prints) {
            self.status = d;
        }

        if !self.rt.description.is_empty() {
            self.status = self.rt.description.clone();
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
        let mut constrain = |t: Vector3, v: &crate::layer::Vp| match &world {
            Some(w) => w.constrain_translation(v, &bb, &nv, t, true, true, WALL_FRICTION),
            None => t,
        };
        let mut cx = Cx { env: &env, base: &base, vp, nav: &nav, constrain: &mut constrain, requests: vec![] };
        self.nav.prepare(&mut cx);
    }

    /// X3DLayerNode.camera and the NavigationContext reactions.
    fn camera_pass(&mut self) {
        self.scan_cam = scan(&self.rt, &Matrix4::IDENTITY, true);

        let before_vp = self.layer.viewpoint();
        let before_nav = self.layer.navigation_info();

        let bbox = if self.scan_cam.viewpoints.iter().any(|(n, _)| {
            let b = VpBase::read(&self.rt, n.node);
            b.view_all
        }) {
            self.scene_bbox()
        } else {
            None
        };

        let scan_cam = std::mem::take(&mut self.scan_cam);
        self.layer.camera(&mut self.rt, &scan_cam, &move || bbox);
        self.scan_cam = scan_cam;
        self.rt.process_events();

        if self.layer.navigation_info() != before_nav {
            self.set_active_navigation_info(false);
        } else if self.layer.navigation_info() != self.nav_info_node {
            self.set_active_navigation_info(false);
        }

        if self.layer.viewpoint() != before_vp {
            // browser ._activeViewpoint changed: ExamineViewer.set_activeViewpoint__.
            let base = self.base();
            let nav = self.nav_info();
            let env = self.env();
            let vp = self.layer.vp(self.layer.viewpoint());
            let mut constrain = |t: Vector3, _: &crate::layer::Vp| t;
            let mut cx = Cx { env: &env, base: &base, vp, nav: &nav, constrain: &mut constrain, requests: vec![] };
            self.nav.active_viewpoint_changed(&mut cx);
        }

        // A NavigationInfo transitionStart disconnects the ExamineViewer's spin.
        // (NavigationInfo ._transitionStart .addInterest ("disconnect"))
        let cur = self.layer.viewpoint();
        let base = self.base();
        self.layer.vp(cur).update(&base);
    }

    /// Whether the COLLISION traversal runs and the shapes it would collect.
    fn collision_world(&mut self) -> Option<World> {
        let collidable = self.nav.is_collidable();

        if self.scan_display.collisions == 0 && !collidable {
            return None;
        }

        let nav_node = self.layer.navigation_info();

        // X3DLayerNode.collision: no collision while a transition is active.
        if self.layer.transition_active.get(&nav_node).copied().unwrap_or(false) {
            return None;
        }

        let mut shapes = vec![];

        for s in &self.scan_display.shapes {
            if !s.collidable {
                continue;
            }

            if let Some(g) = geom_of(&self.rt, &mut self.geom_cache, s) {
                shapes.push(CShape { geometry: g, model_view: s.model_view });
            }
        }

        Some(World { shapes })
    }

    fn collision_pass(&mut self) {
        if self.nav.kind() != "WALK" {
            return;
        }

        let Some(world) = self.collision_world() else { return };
        let base = self.base();
        let nav = self.nav_info();
        let vp_node = self.layer.viewpoint();
        let mut speed = self.gravity_speed;
        let frame_rate = self.frame_rate;
        let vp = self.layer.vp(vp_node).clone();

        if let Some(d) = world.gravitate(&vp, &base, &nav, &mut speed, frame_rate, GRAVITY) {
            let v = self.layer.vp(vp_node);
            v.position_offset.add(&d);
        }

        self.gravity_speed = speed;
    }

    /// Recompute the display scan with the current view (X3DLayerNode.display traversal of the
    /// pointer, sensor and collision data).
    fn refresh_display_scan(&mut self) {
        let vp_node = self.layer.viewpoint();
        let view = self.layer.vp(vp_node).view;
        self.scan_display = scan(&self.rt, &view, false);
    }

    // ProximitySensor / VisibilitySensor (EnvironmentalSensor): CAMERA + DISPLAY traversals and
    // the sensorEvents update.
    fn display_pass(&mut self) {
        let vp_node = self.layer.viewpoint();
        let base = self.base();
        let nav = self.nav_info();
        let vpv = self.layer.vp(vp_node).clone();
        let (near, far) = (vpv.near_distance(&base, &nav), vpv.far_distance(&base, &nav));
        let proj = vpv.projection(&base, near, far, self.width as f64, self.height as f64);

        for e in self.scan_display.proximity.clone() {
            let key = (e.node.path.to_vec(), e.node.node);
            let Some(rt) = rt_at(&self.rt, &e.node.path) else { continue };
            let enabled = rt.get_bool(e.node.node, "enabled");
            let size = match rt.pre_value(e.node.node, "size") {
                Some(FieldValue::SFVec3f(v)) => *v,
                _ => Vector3::ZERO,
            };
            let center = match rt.pre_value(e.node.node, "center") {
                Some(FieldValue::SFVec3f(v)) => *v,
                _ => Vector3::ZERO,
            };

            let st = self.prox.entry(key).or_default();
            st.traversed = true;

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
                st.inside = center.x - hx <= p.x && center.x + hx >= p.x && center.y - hy <= p.y && center.y + hy >= p.y && center.z - hz <= p.z && center.z + hz >= p.z;
            }
        }

        for e in self.scan_display.visibility.clone() {
            let key = (e.node.path.to_vec(), e.node.node);
            let Some(rt) = rt_at(&self.rt, &e.node.path) else { continue };
            let enabled = rt.get_bool(e.node.node, "enabled");
            let size = match rt.pre_value(e.node.node, "size") {
                Some(FieldValue::SFVec3f(v)) => *v,
                _ => Vector3::ZERO,
            };
            let center = match rt.pre_value(e.node.node, "center") {
                Some(FieldValue::SFVec3f(v)) => *v,
                _ => Vector3::ZERO,
            };

            let st = self.vis.entry(key).or_default();
            st.traversed = true;

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
    fn sensor_pass(&mut self) {
        let now = self.time;
        let vp_node = self.layer.viewpoint();
        let base = self.base();
        let vpv = self.layer.vp(vp_node).clone();

        // CAMERA pass data: which sensors were reached, their model matrices.
        for st in self.prox.values_mut() {
            st.seen_camera = false;
        }

        let cam_prox: HashMap<(Vec<NodeId>, NodeId), Matrix4> = self.scan_cam.proximity.iter().map(|e| ((e.node.path.to_vec(), e.node.node), e.model_view)).collect();
        let cam_vis: Vec<(Vec<NodeId>, NodeId)> = self.scan_cam.visibility.iter().map(|e| (e.node.path.to_vec(), e.node.node)).collect();

        let keys: Vec<_> = self.prox.keys().cloned().collect();

        for key in keys {
            let st = self.prox.get_mut(&key).unwrap();

            if !st.registered {
                continue;
            }

            let seen = cam_prox.get(&key).copied();
            let (inside, traversed) = (st.inside, st.traversed);
            let Some(rt) = rt_at_mut(&mut self.rt, &key.0) else { continue };
            let n = key.1;

            if inside && traversed && seen.is_some() && rt.get_bool(n, "enabled") {
                let model = seen.unwrap();
                let mut inv_model = model;
                inv_model.inverse();

                let mut cor_m = vpv.model;
                cor_m.translate(&vpv.user_center_of_rotation(&base));
                cor_m.mult_right(&inv_model);
                let (cor, _, _, _) = factor_simple(&cor_m);

                let mut m = inv_model;
                m.mult_left(&vpv.camera_space);
                let (position, orientation, _, _) = factor_simple(&m);

                let set = |rt: &mut Runtime, name: &str, v: FieldValue| {
                    if let Some(k) = rt.pre(n, name) {
                        rt.set_value(k, v);
                    }
                };

                if rt.get_bool(n, "isActive") {
                    if !matches!(rt.pre_value(n, "position_changed"), Some(FieldValue::SFVec3f(p)) if p.equals(&position)) {
                        set(rt, "position_changed", FieldValue::SFVec3f(position));
                    }

                    if !matches!(rt.pre_value(n, "orientation_changed"), Some(FieldValue::SFRotation(o)) if o.equals(&orientation)) {
                        set(rt, "orientation_changed", FieldValue::SFRotation(orientation));
                    }

                    if !matches!(rt.pre_value(n, "centerOfRotation_changed"), Some(FieldValue::SFVec3f(p)) if p.equals(&cor)) {
                        set(rt, "centerOfRotation_changed", FieldValue::SFVec3f(cor));
                    }
                } else {
                    set(rt, "isActive", FieldValue::SFBool(true));
                    set(rt, "enterTime", FieldValue::SFTime(now));
                    set(rt, "position_changed", FieldValue::SFVec3f(position));
                    set(rt, "orientation_changed", FieldValue::SFRotation(orientation));
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

        let keys: Vec<_> = self.vis.keys().cloned().collect();

        for key in keys {
            let st = self.vis.get_mut(&key).unwrap();

            if !st.registered {
                continue;
            }

            let (visible, traversed) = (st.inside, st.traversed);
            let _ = &cam_vis;
            let Some(rt) = rt_at_mut(&mut self.rt, &key.0) else { continue };
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
    pub fn user_viewpoints(&self) -> Vec<NodeId> {
        self.layer
            .viewpoint_list()
            .iter()
            .copied()
            .filter(|n| *n != DEFAULT && !VpBase::read(&self.rt, *n).description.is_empty())
            .collect()
    }

    pub fn viewpoint_description(&self, n: NodeId) -> String {
        VpBase::read(&self.rt, n).description
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
        let base = self.base();
        let vp = self.layer.vp(self.layer.viewpoint());
        (vp.user_position(&base), vp.user_orientation(&base), vp.user_center_of_rotation(&base))
    }

    // --- input -------------------------------------------------------------------------------

    pub fn input(&mut self, ev: UiEvent) {
        match ev {
            UiEvent::Mods { shift, ctrl, alt } => {
                self.shift = shift;
                self.ctrl = ctrl;
                self.alt = alt;
            }
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
        let base = self.base();
        let nav = self.nav_info();
        let env = self.env();
        let vp_node = self.layer.viewpoint();
        let world = self.collision_world();
        let (bb, nv) = (base.clone(), nav.clone());
        let vp = self.layer.vp(vp_node);
        let mut constrain = |t: Vector3, v: &crate::layer::Vp| match &world {
            Some(w) => w.constrain_translation(v, &bb, &nv, t, true, true, WALL_FRICTION),
            None => t,
        };
        let mut cx = Cx { env: &env, base: &base, vp, nav: &nav, constrain: &mut constrain, requests: vec![] };
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
            self.layer.look_at_point(&mut self.rt, vp_node, &base, hit.point, r.straighten, date_now);
        } else if let Some((min, max)) = hit.bbox {
            // hit .modelViewMatrix is a Float32Array copy of the pointing context's matrix.
            let mut mv = hit.model_view;
            mv.0 = mv.0.map(|v| v as f32 as f64);
            let camera_space = self.layer.vp(vp_node).camera_space;
            let mut bbox = Box3::from_extents(&min, &max);
            bbox.mult_right(&mv).mult_right(&camera_space);
            self.layer.look_at_bbox(&mut self.rt, vp_node, &base, &bbox, r.straighten, date_now);
        }
    }

    /// The bound viewpoint's fieldOfViewScale and, for an OrthoViewpoint, its fieldOfViewOffset.
    pub fn field_of_view_state(&mut self) -> (f64, Option<[f64; 4]>) {
        let base = self.base();
        let vp = self.layer.vp(self.layer.viewpoint());
        (vp.field_of_view_scale, base.is_ortho.then_some(vp.field_of_view_offset))
    }

    fn has_sensors(&self) -> bool {
        self.scan_display.shapes.iter().any(|s| !s.sensors.is_empty())
    }

    fn do_pick(&mut self, x: f64, y: f64) -> Hit {
        let cam = self.pick_camera();
        let shapes = std::mem::take(&mut self.scan_display.shapes);
        let mut cache = std::mem::take(&mut self.geom_cache);
        let rt = &self.rt;
        let hit = pick(&shapes, &mut |s| geom_of(rt, &mut cache, s), &cam, x, y);
        self.geom_cache = cache;
        self.scan_display.shapes = shapes;
        hit
    }

    fn collect_pointer_output(&mut self) {
        let out = std::mem::take(&mut self.pointer.out);

        if let Some(d) = out.description {
            self.status = d;
        }

        for a in out.anchors {
            self.status = format!("Anchor: {} -> {}", if a.description.is_empty() { "(no description)" } else { &a.description }, a.urls.first().cloned().unwrap_or_default());
            self.anchors.push(a);
        }
    }

    fn mouse_down(&mut self, button: u8, x: f64, y: f64) {
        if button == 0 && self.has_sensors() {
            if self.advance_only_time {
                self.rt.now = self.date_now;
            }

            let hit = self.do_pick(x, y);
            let cam = self.pick_camera();
            let now = if self.advance_only_time { self.date_now } else { self.time };
            let pressed = self.pointer.button_press(&mut self.rt, hit, &cam, now);
            self.collect_pointer_output();

            if pressed {
                return;
            }
        }

        self.viewer_input(Input::MouseDown { button, x, y });
    }

    fn mouse_up(&mut self, button: u8, x: f64, y: f64) {
        if button == 0 && self.pointer.is_active() {
            let cam = self.pick_camera();
            let now = if self.advance_only_time { self.date_now } else { self.time };
            self.pointer.button_release(&mut self.rt, &cam, now);
            self.collect_pointer_output();
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

        let hit = if self.nav.is_active() { self.pointer.hit.clone() } else { self.do_pick(x, y) };
        let cam = self.pick_camera();
        let now = if self.advance_only_time { self.date_now } else { self.time };
        self.pointer.motion(&mut self.rt, hit, &cam, now);
        self.collect_pointer_output();
    }

    // --- rendering ---------------------------------------------------------------------------

    /// The frame for the renderer: the bound viewpoint's camera and bindables.
    pub fn build_frame(&mut self) -> Frame {
        let base = self.base();
        let nav = self.nav_info();
        let vp = self.layer.vp(self.layer.viewpoint()).clone();
        let bg = self.layer.background();
        let fog = self.layer.fog();
        let vp_node = self.layer.viewpoint();
        let nav_node = self.layer.navigation_info();

        let background = if bg == DEFAULT { None } else { self.scan_cam.backgrounds.iter().find(|(n, _)| n.node == bg).map(|(n, m)| (n.node, *m)) };

        let mut b = Builder::new(&self.rt).with_cache(&mut self.render_cache);
        b.camera = Some(CameraOverride {
            camera_space: vp.camera_space,
            fov: vp.user_fov(&base),
            near: vp.near_distance(&base, &nav),
            far: vp.far_distance(&base, &nav),
            projection: base.is_ortho.then(|| vp.projection(&base, vp.near_distance(&base, &nav), vp.far_distance(&base, &nav), self.width as f64, self.height as f64)),
            headlight: nav.headlight,
            viewpoint: (vp_node != DEFAULT).then_some(vp_node),
            navigation_info: (nav_node != DEFAULT).then_some(nav_node),
            background,
            fog: (fog != DEFAULT).then_some(fog),
        });
        b.build(self.width, self.height)
    }

    pub fn log(&self) -> &[String] {
        &self.rt.log
    }

    pub fn node_ref(&self, n: NodeId) -> NodeRef {
        NodeRef { path: Rc::new(vec![]), node: n }
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
        let planes = [c.x >= -c.w, c.x <= c.w, c.y >= -c.w, c.y <= c.w, c.z >= -c.w, c.z <= c.w];

        for (k, inside) in planes.iter().enumerate() {
            if *inside {
                outside[k] = false;
            }
        }
    }

    !outside.iter().any(|o| *o)
}
