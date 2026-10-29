//! Navigation viewers: Browser/Navigation/X3DViewer.js, ExamineViewer.js, X3DFlyViewer.js,
//! FlyViewer.js, WalkViewer.js, NoneViewer.js. Input arrives as [`Input`] values in viewport
//! pixels (origin bottom left, like X3DPointingDeviceSensorContext.getPointerFromEvent).
//!
//! Chasers are ticked from [`Viewer::prepare`] (the browser's prepareEvents); events the JS
//! sends to a chaser (`_set_value`, `_set_destination`) are queued and delivered at the start
//! of the next prepare phase, which is when X_ITE's routing context processes them.

use crate::chaser::Chaser;
use crate::geom::ViewVolume;
use crate::layer::{NavInfo, Vp, VpBase};
use x3d_fields::math::{Matrix4, Rotation4, Vector2, Vector3, Vector4};

mod lookat;
mod plane;

pub use lookat::LookAt;
pub use plane::Plane;

const SPIN_RELEASE_TIME: f64 = 20.0; // ms
const SPIN_ANGLE: f64 = 2.0 * std::f64::consts::PI / 180.0;
const SPIN_FACTOR: f64 = 0.3;
const SCROLL_FACTOR: f64 = 1.0 / 20.0;
const MOVE_TIME: f64 = 0.2;
const ROTATE_TIME_EXAMINE: f64 = 0.2;
const ROTATE_TIME_FLY: f64 = 0.3;
const DISK_ANGLE: f64 = 15.0 * std::f64::consts::PI / 180.0;
const CRITICAL_ANGLE_EXAMINE: f64 = 0.1 * std::f64::consts::PI / 180.0;

const SPEED_FACTOR: f64 = 0.007;
const SHIFT_SPEED_FACTOR: f64 = 4.0 * SPEED_FACTOR;
const ROTATION_SPEED_FACTOR: f64 = 1.4;
const ROTATION_LIMIT: f64 = 40.0;
const PAN_SPEED_FACTOR: f64 = SPEED_FACTOR;
const PAN_SHIFT_SPEED_FACTOR: f64 = 1.4 * PAN_SPEED_FACTOR;
const ROLL_ANGLE: f64 = std::f64::consts::PI / 32.0;
const CRITICAL_ANGLE_FLY: f64 = 0.99;

#[derive(Clone, Copy, Debug)]
pub enum Input {
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
    /// WheelEvent.deltaY (negative = zoom in / roll).
    Wheel {
        delta_y: f64,
        x: f64,
        y: f64,
    },
    /// The "dblclick" DOM event (after the second mouseup of a double click).
    DblClick {
        x: f64,
        y: f64,
    },
    /// touchstart / touchmove / touchend with `event .touches` (pageX, pageY in viewport pixels).
    TouchStart(Touches),
    TouchMove(Touches),
    TouchEnd,
}

/// `event .touches` of a touch event (only the first two touches matter to the viewers), at pointer coordinates
/// (`getPointerFromEvent`).
#[derive(Clone, Copy, Debug)]
pub struct Touches {
    pub n: usize,
    pub p: [(f64, f64); 2],
    /// `window .innerWidth` at pointer scale: ExamineViewer's pinch zoom factor is the distance change over it.
    pub zoom_width: f64,
}

/// X3DViewer.lookAtPoint / lookAtBBox, requested by a viewer's dblclick handler. The viewer
/// calls `this .disconnect ()` first, then the browser picks at (x, y) and the viewpoint starts
/// its lookAt transition (Viewer3d does that part, it owns the pointing device and the layer).
#[derive(Clone, Copy, Debug)]
pub struct LookAtRequest {
    pub x: f64,
    pub y: f64,
    /// `browser .getAltKey ()`: look at the hit point instead of the hit shape's bounding box.
    pub point: bool,
    /// `getStraightenHorizon ()`.
    pub straighten: bool,
}

/// DEVELOPMENT.js: `true` in the source tree the oracle loads (src/), replaced during the dist build.
/// PlaneViewer.dblclick looks at only when it is true.
pub const DEVELOPMENT: bool = true;

/// Per-call environment (the browser's time, keys and viewport).
#[derive(Clone, Copy, Debug)]
pub struct Env {
    /// browser .getCurrentTime ().
    pub now: f64,
    /// Date.now () / 1000.
    pub date_now: f64,
    /// browser .getCurrentFrameRate ().
    pub frame_rate: f64,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    /// x, y, width, height.
    pub viewport: [f64; 4],
    /// BrowserOptions StraightenHorizon (default TRUE).
    pub straighten: bool,
}

pub struct Cx<'a> {
    pub env: &'a Env,
    pub base: &'a VpBase,
    pub vp: &'a mut Vp,
    pub nav: &'a NavInfo,
    /// X3DLayerNode.constrainTranslation.
    pub constrain: &'a mut dyn FnMut(Vector3, &Vp) -> Vector3,
    /// lookAt requests of dblclick handlers, executed by the caller after the event.
    pub requests: Vec<LookAtRequest>,
}

fn mr(a: &Rotation4, b: &Rotation4) -> Rotation4 {
    let mut r = *a;
    r.mult_right(b);
    r
}

fn inv(a: &Rotation4) -> Rotation4 {
    let mut r = *a;
    r.inverse();
    r
}

fn rot_vec(r: &Rotation4, v: &Vector3) -> Vector3 {
    let mut v = *v;
    r.mult_vec_rot(&mut v);
    v
}

fn add(a: &Vector3, b: &Vector3) -> Vector3 {
    let mut r = *a;
    r.add(b);
    r
}

fn sub(a: &Vector3, b: &Vector3) -> Vector3 {
    let mut r = *a;
    r.subtract(b);
    r
}

fn set_vectors(from: &Vector3, to: &Vector3) -> Rotation4 {
    let mut r = Rotation4::identity();
    r.set_vectors(from, to);
    r
}

/// Rotation4.fromVectors.
fn from_vectors(from: &Vector3, to: &Vector3) -> Rotation4 {
    set_vectors(from, to)
}

fn angle_of(r: &Rotation4) -> f64 {
    let mut r = *r;
    r.angle()
}

fn in_rect(env: &Env, x: f64, y: f64) -> bool {
    let r = env.viewport;
    x >= r[0] && x <= r[0] + r[2] && y >= r[1] && y <= r[1] + r[3]
}

/// X3DViewer.getButton: Alt + button 0 emulates the middle button.
fn get_button(button: i32, alt: bool) -> i32 {
    if button == 0 && alt { 1 } else { button }
}

/// X3DViewer.trackballProjectToSphere.
fn trackball(env: &Env, x: f64, y: f64) -> Vector3 {
    let v = env.viewport;
    let x = (x - v[0]) / v[2] - 0.5;
    let y = (y - v[1]) / v[3] - 0.5;
    Vector3::new(x, y, tb_project_to_sphere(0.5, x, y))
}

fn tb_project_to_sphere(r: f64, x: f64, y: f64) -> f64 {
    let d = x.hypot(y);

    if d < r * std::f64::consts::FRAC_1_SQRT_2 {
        return (r * r - d * d).sqrt();
    }

    let t = r / std::f64::consts::SQRT_2;
    t * t / d
}

fn distance_to_center(cx: &Cx, position_offset: Option<&Vector3>) -> Vector3 {
    let mut d = cx.base.position;
    d.add(position_offset.unwrap_or(&cx.vp.position_offset));
    d.subtract(&cx.vp.user_center_of_rotation(cx.base));
    d
}

/// X3DViewer.getPointOnCenterPlane.
fn point_on_center_plane(cx: &Cx, x: f64, y: f64) -> Vector3 {
    let v = cx.env.viewport;
    let near = cx.vp.near_distance(cx.base, cx.nav);
    let far_v = cx.vp.far_distance(cx.base, cx.nav);
    let proj = cx.vp.projection(cx.base, near, far_v, v[2], v[3]);
    let mut far = Vector3::ZERO;
    ViewVolume::unproject_point(
        x,
        y,
        0.9,
        &Matrix4::IDENTITY,
        &proj,
        &Vector4::new(v[0], v[1], v[2], v[3]),
        &mut far,
    );

    if cx.base.is_ortho {
        return Vector3::new(far.x, far.y, -distance_to_center(cx, None).norm());
    }

    let mut direction = far;
    direction.normalize();
    let dist = distance_to_center(cx, None).norm();
    let mut r = direction;
    r.multiply(dist / direction.dot(&Vector3::new(0.0, 0.0, -1.0)));
    r
}

// --- the viewer enum -------------------------------------------------------------------------

pub enum Viewer {
    None,
    Examine(Box<Examine>),
    Fly(Box<Fly>),
    Plane(Box<Plane>),
    LookAt(Box<LookAt>),
}

impl Viewer {
    pub fn new(kind: &str, now: f64, cx: &mut Cx) -> Viewer {
        match kind {
            "NONE" => Viewer::None,
            "WALK" => Viewer::Fly(Box::new(Fly::new(true, now))),
            "FLY" => Viewer::Fly(Box::new(Fly::new(false, now))),
            "PLANE" => Viewer::Plane(Box::new(Plane::new())),
            "LOOKAT" => Viewer::LookAt(Box::new(LookAt::new(now))),
            // Unknown names: the default branch of X3DNavigationContext.set_viewer__.
            _ => {
                let mut e = Examine::new(now);
                e.set_active_viewpoint(cx);
                Viewer::Examine(Box::new(e))
            }
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Viewer::None => "NONE",
            Viewer::Examine(_) => "EXAMINE",
            Viewer::Fly(f) => {
                if f.walk {
                    "WALK"
                } else {
                    "FLY"
                }
            }
            Viewer::Plane(_) => "PLANE",
            Viewer::LookAt(_) => "LOOKAT",
        }
    }

    /// X3DViewer.isCollidable.
    pub fn is_collidable(&self) -> bool {
        match self {
            Viewer::Fly(f) => f.walk || f.is_active,
            _ => false,
        }
    }

    pub fn is_active(&self) -> bool {
        match self {
            Viewer::None => false,
            Viewer::Examine(e) => e.is_active,
            Viewer::Fly(f) => f.is_active,
            Viewer::Plane(p) => p.is_active,
            Viewer::LookAt(l) => l.is_active,
        }
    }

    pub fn input(&mut self, ev: &Input, cx: &mut Cx) {
        match self {
            Viewer::None => {}
            Viewer::Examine(e) => e.input(ev, cx),
            Viewer::Fly(f) => f.input(ev, cx),
            Viewer::Plane(p) => p.input(ev, cx),
            Viewer::LookAt(l) => l.input(ev, cx),
        }
    }

    /// The browser's prepareEvents.
    pub fn prepare(&mut self, cx: &mut Cx) {
        match self {
            Viewer::None => {}
            Viewer::Examine(e) => e.prepare(cx),
            Viewer::Fly(f) => f.prepare(cx),
            Viewer::Plane(_) => {}
            Viewer::LookAt(l) => l.prepare(cx),
        }
    }

    /// `browser ._activeViewpoint .addInterest ("set_activeViewpoint__")` (ExamineViewer).
    pub fn active_viewpoint_changed(&mut self, cx: &mut Cx) {
        if let Viewer::Examine(e) = self {
            e.set_active_viewpoint(cx);
        }
    }

    /// `navigationInfo ._transitionStart` / viewer disconnect.
    pub fn disconnect(&mut self, now: f64) {
        match self {
            Viewer::Examine(e) => e.disconnect(now),
            Viewer::Fly(f) => f.disconnect(),
            Viewer::LookAt(l) => l.disconnect(),
            Viewer::Plane(_) | Viewer::None => {}
        }
    }
}

// --- ExamineViewer ---------------------------------------------------------------------------

enum Pend {
    PosValue(Vector3),
    PosDest(Vector3),
    CorValue(Vector3),
    CorDest(Vector3),
    RotValue(Rotation4),
    RotDest(Rotation4),
}

pub struct Examine {
    button: i32,
    alt_key: bool,
    pub is_active: bool,
    motion_time: f64,
    orientation_offset: Rotation4,
    from_vector: Vector3,
    from_pointer: Vector2,
    from_point: Vector3,
    start_orientation: Rotation4,
    roll: Rotation4,
    rotation: Rotation4,
    delta_rotation: Rotation4,
    direction: Vector3,
    axis: Vector3,
    last_roll: f64,
    negate: f64,
    initial_position_offset: Vector3,
    initial_orientation_offset: Rotation4,
    position_chaser: Chaser<Vector3>,
    cor_chaser: Chaser<Vector3>,
    rotation_chaser: Chaser<Rotation4>,
    pending: Vec<Pend>,
    /// timeSensor (loop): Some((startTime, cycleInterval)) while spinning.
    spin_ts: Option<(f64, f64)>,
    /// Non-straighten spinning is a prepareEvents interest.
    spin_prepare: bool,
    touch_mode: i32,
    touch1: Vector2,
    touch2: Vector2,
    /// `tapedTwice`, valid while Date.now () is before `tap_until` (the 300 ms setTimeout).
    taped_twice: bool,
    tap_until: f64,
}

impl Examine {
    pub fn new(now: f64) -> Examine {
        Examine {
            button: -1,
            alt_key: false,
            is_active: false,
            motion_time: 0.0,
            orientation_offset: Rotation4::identity(),
            from_vector: Vector3::ZERO,
            from_pointer: Vector2::ZERO,
            from_point: Vector3::ZERO,
            start_orientation: Rotation4::identity(),
            roll: Rotation4::identity(),
            rotation: Rotation4::identity(),
            delta_rotation: Rotation4::identity(),
            direction: Vector3::ZERO,
            axis: Vector3::ZERO,
            last_roll: 0.0,
            negate: 0.0,
            initial_position_offset: Vector3::ZERO,
            initial_orientation_offset: Rotation4::identity(),
            position_chaser: Chaser::new(MOVE_TIME, now),
            cor_chaser: Chaser::new(MOVE_TIME, now),
            rotation_chaser: Chaser::new(ROTATE_TIME_EXAMINE, now),
            pending: vec![],
            spin_ts: None,
            spin_prepare: false,
            touch_mode: 0,
            touch1: Vector2::ZERO,
            touch2: Vector2::ZERO,
            taped_twice: false,
            tap_until: 0.0,
        }
    }

    /// ExamineViewer.set_activeViewpoint__.
    pub fn set_active_viewpoint(&mut self, cx: &mut Cx) {
        if cx.env.straighten {
            self.start_orientation = cx.vp.user_orientation(cx.base);
            let before = cx.vp.orientation_offset;
            cx.vp.orientation_offset = self.get_orientation_offset(
                cx,
                &Rotation4::identity(),
                &Rotation4::identity(),
                &before,
            );
        }

        self.disconnect(cx.env.date_now);
    }

    pub fn disconnect(&mut self, date_now: f64) {
        self.position_chaser.connected = false;
        self.cor_chaser.connected = false;
        self.rotation_chaser.connected = false;
        self.remove_spinning(date_now);
    }

    fn remove_spinning(&mut self, _date_now: f64) {
        self.spin_ts = None;
        self.spin_prepare = false;
    }

    fn get_orientation_offset(
        &self,
        cx: &Cx,
        roll: &Rotation4,
        rotation: &Rotation4,
        before: &Rotation4,
    ) -> Rotation4 {
        let user = if cx.env.straighten {
            let mut u = *roll;
            u.mult_right(&self.start_orientation).mult_right(rotation);
            cx.base.straighten_horizon(cx.vp, &u)
        } else {
            let mut u = *rotation;
            u.mult_right(&cx.base.orientation).mult_right(before);
            u
        };

        let mut after = inv(&cx.base.orientation);
        after.mult_right(&user);
        after
    }

    fn get_position_offset(
        &self,
        cx: &Cx,
        before: &Vector3,
        orientation_before: &Rotation4,
        orientation_after: &Rotation4,
    ) -> Vector3 {
        let distance = distance_to_center(cx, Some(before));
        let mut oob = inv(orientation_before);
        oob.mult_right(orientation_after);
        let mut d = rot_vec(&oob, &distance);
        d.subtract(&distance).add(before);
        d
    }

    pub fn input(&mut self, ev: &Input, cx: &mut Cx) {
        match *ev {
            Input::MouseDown { button, x, y } => self.mousedown(button as i32, x, y, cx),
            Input::MouseUp { button, .. } => self.mouseup(button as i32, true, cx),
            Input::MouseMove { x, y } => self.mousemove(x, y, cx),
            Input::Wheel { delta_y, x, y } => {
                if !in_rect(cx.env, x, y) {
                    return;
                }

                self.remove_spinning(cx.env.date_now);
                cx.vp.transition_stop();
                self.zoom(SCROLL_FACTOR, delta_y, cx);
            }
            Input::DblClick { x, y } => self.dblclick(x, y, cx),
            // The stepped harness sends no touch events; the web browser does (crates/x3d-web).
            Input::TouchStart(t) => self.touchstart(t, cx),
            Input::TouchMove(t) => self.touchmove(t, cx),
            Input::TouchEnd => self.touchend(true, cx),
        }
    }

    /// ExamineViewer.dblclick: disconnect, then lookAtPoint (Alt) or lookAtBBox, run by the caller.
    fn dblclick(&mut self, x: f64, y: f64, cx: &mut Cx) {
        self.disconnect(cx.env.date_now);
        cx.requests.push(LookAtRequest {
            x,
            y,
            point: cx.env.alt,
            straighten: cx.env.straighten,
        });
    }

    /// ExamineViewer.wheel after the pointer test (`event .zoomFactor || SCROLL_FACTOR`).
    fn wheel(&mut self, zoom_factor: f64, delta_y: f64, x: f64, y: f64, cx: &mut Cx) {
        if !in_rect(cx.env, x, y) {
            return;
        }

        self.remove_spinning(cx.env.date_now);
        cx.vp.transition_stop();
        let zf = if zoom_factor != 0.0 && !zoom_factor.is_nan() {
            zoom_factor
        } else {
            SCROLL_FACTOR
        };
        self.zoom(zf, delta_y, cx);
    }

    fn touchstart(&mut self, t: Touches, cx: &mut Cx) {
        match t.n {
            1 => {
                // Start rotate (button 0).
                self.mousedown(0, t.p[0].0, t.p[0].1, cx);

                // Remember tap.
                self.touch1 = Vector2::new(t.p[0].0, t.p[0].1);
            }
            2 => {
                // End rotate (button 0).
                self.touchend(false, cx);

                // Start move (button 1).
                let (x, y) = ((t.p[0].0 + t.p[1].0) / 2.0, (t.p[0].1 + t.p[1].1) / 2.0);
                self.mousedown(1, x, y, cx);

                // Start zoom (mouse wheel).
                self.touch1 = Vector2::new(t.p[0].0, t.p[0].1);
                self.touch2 = Vector2::new(t.p[1].0, t.p[1].1);
            }
            _ => {
                // End move (button 1).
                self.touchend(false, cx);
            }
        }
    }

    fn touchend(&mut self, spin: bool, cx: &mut Cx) {
        match self.button {
            0 => {
                // End rotate (button 0).
                self.mouseup(0, spin, cx);

                // Start dblclick (button 0).
                let now_ms = cx.env.date_now * 1000.0;

                if self.taped_twice && now_ms < self.tap_until {
                    let (x, y) = (self.touch1.x, self.touch1.y);
                    self.dblclick(x, y, cx);
                } else {
                    self.taped_twice = true;
                    self.tap_until = now_ms + 300.0;
                }
            }
            1 => {
                // End move (button 1).
                self.touch_mode = 0;
                self.mouseup(1, false, cx);
            }
            _ => {}
        }
    }

    fn touchmove(&mut self, t: Touches, cx: &mut Cx) {
        match t.n {
            1 => self.mousemove(t.p[0].0, t.p[0].1, cx),
            2 => {
                let mut c1 = Vector2::new(t.p[0].0, t.p[0].1);
                c1.subtract(&self.touch1).normalize();
                let mut c2 = Vector2::new(t.p[1].0, t.p[1].1);
                c2.subtract(&self.touch2).normalize();

                let dot = c1.dot(&c2);
                let (move_, zoom) = (dot > 0.7, dot < -0.7);
                let mode = if self.touch_mode != 0 {
                    self.touch_mode
                } else if move_ {
                    1
                } else if zoom {
                    2
                } else {
                    0
                };
                let (x, y) = ((t.p[0].0 + t.p[1].0) / 2.0, (t.p[0].1 + t.p[1].1) / 2.0);

                match mode {
                    1 => {
                        self.touch_mode = 1;
                        self.mousemove(x, y, cx);
                    }
                    2 => {
                        self.touch_mode = 2;
                        let distance1 = self.touch1.distance(&self.touch2);
                        self.touch1 = Vector2::new(t.p[0].0, t.p[0].1);
                        self.touch2 = Vector2::new(t.p[1].0, t.p[1].1);
                        let delta = distance1 - self.touch1.distance(&self.touch2);

                        // `window .innerWidth` (the canvas width when the host does not know it).
                        let width = if t.zoom_width > 0.0 {
                            t.zoom_width
                        } else {
                            cx.env.viewport[2]
                        };

                        self.wheel(delta.abs() / width, delta, x, y, cx);
                    }
                    _ => {}
                }

                self.touch1 = Vector2::new(t.p[0].0, t.p[0].1);
                self.touch2 = Vector2::new(t.p[1].0, t.p[1].1);
            }
            _ => {}
        }
    }

    fn mousedown(&mut self, button: i32, x: f64, y: f64, cx: &mut Cx) {
        if self.button >= 0 {
            return;
        }

        if !in_rect(cx.env, x, y) {
            return;
        }

        self.alt_key = cx.env.alt;
        let button = get_button(button, self.alt_key);

        match button {
            0 => {
                self.button = button;
                self.motion_time = cx.env.date_now * 1000.0;
                self.delta_rotation = Rotation4::identity();
                self.disconnect(cx.env.date_now);
                cx.vp.transition_stop();
                self.start_rotate(x, y, 0.0, cx);
                self.is_active = true;
            }
            1 => {
                self.button = button;
                self.disconnect(cx.env.date_now);
                cx.vp.transition_stop();
                self.from_point = point_on_center_plane(cx, x, y);
                self.is_active = true;
            }
            _ => {}
        }
    }

    fn mouseup(&mut self, button: i32, spin: bool, cx: &mut Cx) {
        let button = get_button(button, self.alt_key);

        if button != self.button {
            return;
        }

        self.button = -1;
        self.is_active = false;

        if button == 0 {
            if !spin {
                return;
            }

            if angle_of(&self.delta_rotation).abs() > SPIN_ANGLE
                && cx.env.date_now * 1000.0 - self.motion_time < SPIN_RELEASE_TIME
            {
                let d = self.delta_rotation;
                self.add_spinning(&d, cx);
            }
        }
    }

    fn mousemove(&mut self, x: f64, y: f64, cx: &mut Cx) {
        let button = get_button(self.button, self.alt_key);

        if button != self.button {
            return;
        }

        match button {
            0 => {
                self.rotate(x, y, cx);
                self.motion_time = cx.env.date_now * 1000.0;
            }
            1 => self.pan(x, y, cx),
            _ => {}
        }
    }

    fn start_rotate(&mut self, x: f64, y: f64, negate: f64, cx: &mut Cx) {
        if cx.env.straighten {
            self.from_pointer = Vector2::new(x, y);
            self.start_orientation = cx.vp.user_orientation(cx.base);
            self.last_roll = 0.0;
            self.negate = negate;
        } else {
            self.from_vector = trackball(cx.env, x, y);
        }
    }

    fn rotate(&mut self, x: f64, y: f64, cx: &mut Cx) {
        let v = cx.env.viewport;

        if cx.env.straighten {
            let up = cx.base.up_vector(cx.vp, true);
            let tx = x - self.from_pointer.x;
            let ty = y - self.from_pointer.y;
            let ppr_x = v[2] * 2.0;
            let ppr_y = v[3] * 2.0;
            let zaxis = rot_vec(&self.start_orientation, &Vector3::Z_AXIS);
            let start_roll = zaxis.dot(&up).clamp(-1.0, 1.0).acos();
            let roll = std::f64::consts::PI * 2.0 * ty / ppr_y;
            let clamped_roll = (start_roll + roll).clamp(
                CRITICAL_ANGLE_EXAMINE,
                std::f64::consts::PI - CRITICAL_ANGLE_EXAMINE,
            ) - start_roll;

            if self.last_roll - roll.abs() > 0.0 {
                self.from_pointer.y += (roll - clamped_roll) / (std::f64::consts::PI * 2.0) * ppr_y;
            }

            self.last_roll = roll.abs();

            if self.negate == 0.0 {
                if std::f64::consts::PI / 2.0 - (start_roll - std::f64::consts::PI / 2.0).abs()
                    < DISK_ANGLE
                {
                    if y - v[1] > v[3] / 2.0 {
                        self.negate = if start_roll < std::f64::consts::PI / 4.0 {
                            -1.0
                        } else {
                            1.0
                        };
                    } else {
                        self.negate = if start_roll > std::f64::consts::PI / 4.0 {
                            -1.0
                        } else {
                            1.0
                        };
                    }
                } else {
                    self.negate = 1.0;
                }
            }

            self.delta_rotation = self.rotation;
            self.roll = Rotation4::new(1.0, 0.0, 0.0, clamped_roll);
            let mut r = Rotation4::identity();
            r.set(
                up.x,
                up.y,
                up.z,
                std::f64::consts::PI * 2.0 * -tx / ppr_x * self.negate,
            );
            self.rotation = r;
            let mut d = inv(&self.delta_rotation);
            d.mult_right(&self.rotation);
            self.delta_rotation = d;
        } else {
            let to_vector = trackball(cx.env, x, y);
            self.delta_rotation = self.rotation;
            self.roll = Rotation4::identity();
            self.rotation = set_vectors(&to_vector, &self.from_vector);
            let mut d = inv(&self.delta_rotation);
            d.mult_right(&self.rotation);
            self.delta_rotation = d;
        }

        let (roll, rotation) = (self.roll, self.rotation);
        self.add_rotate(&roll, &rotation, cx);
    }

    fn add_rotate(&mut self, roll: &Rotation4, rotation: &Rotation4, cx: &mut Cx) {
        let _ = roll;

        if self.rotation_chaser.connected {
            self.pending.push(Pend::RotDest(*rotation));
        } else {
            self.rotation_chaser.connected = true;
            self.initial_orientation_offset = cx.vp.orientation_offset;
            self.initial_position_offset = cx.vp.position_offset;
            self.pending.push(Pend::RotValue(Rotation4::identity()));
            self.pending.push(Pend::RotDest(*rotation));
        }
    }

    fn add_spinning(&mut self, delta: &Rotation4, cx: &mut Cx) {
        self.disconnect(cx.env.date_now);

        if cx.env.straighten {
            let up = cx.base.up_vector(cx.vp, true);
            let user_position = cx.vp.user_position(cx.base);
            let user_center = cx.vp.user_center_of_rotation(cx.base);
            let direction = sub(&user_position, &user_center);
            let mut d = *delta;
            let angle = d.angle().abs().min(std::f64::consts::PI / 4.0);

            self.axis = up;

            let axis_dot = d.get_axis().dot(&up);
            let sign = |v: f64| {
                if v > 0.0 {
                    1.0
                } else if v < 0.0 {
                    -1.0
                } else {
                    v
                }
            };

            if sign(axis_dot) != sign(d.angle()) {
                self.axis.negate();
            }

            let interval = std::f64::consts::PI / (angle * SPIN_FACTOR * 25.0);
            self.spin_ts = Some((cx.env.date_now, interval));

            let look = cx
                .vp
                .look_at_rotation(cx.base, &user_position, &user_center);
            self.direction = direction;
            let mut oo = cx.vp.user_orientation(cx.base);
            oo.mult_right(&inv(&look));
            self.orientation_offset = oo;
        } else {
            self.spin_prepare = true;
            self.rotation = *delta;
        }
    }

    /// ExamineViewer.spin.
    fn spin(&mut self, cx: &mut Cx, fraction: f64) {
        if cx.env.straighten {
            let user_center = cx.vp.user_center_of_rotation(cx.base);
            let rotation = Rotation4::new(
                self.axis.x,
                self.axis.y,
                self.axis.z,
                2.0 * std::f64::consts::PI * fraction,
            );
            let mut user_position = rot_vec(&rotation, &self.direction);
            user_position.add(&user_center);
            let look = cx
                .vp
                .look_at_rotation(cx.base, &user_position, &user_center);
            cx.vp.position_offset = sub(&user_position, &cx.base.position);
            let mut oo = inv(&cx.base.orientation);
            oo.mult_right(&self.orientation_offset).mult_right(&look);
            cx.vp.orientation_offset = oo;
        } else {
            let mut rotation = Rotation4::identity();
            rotation.slerp(&self.rotation, SPIN_FACTOR * 60.0 / cx.env.frame_rate);
            self.orientation_offset = cx.vp.orientation_offset;
            let before = self.orientation_offset;
            let new = self.get_orientation_offset(cx, &Rotation4::identity(), &rotation, &before);
            cx.vp.orientation_offset = new;
            let po = self.get_position_offset(
                cx,
                &cx.vp.position_offset,
                &self.orientation_offset,
                &new,
            );
            cx.vp.position_offset = po;
        }
    }

    fn pan(&mut self, x: f64, y: f64, cx: &mut Cx) {
        let to_point = point_on_center_plane(cx, x, y);
        let uo = cx.vp.user_orientation(cx.base);
        let translation = rot_vec(&uo, &sub(&self.from_point, &to_point));
        self.add_move(&translation, &translation, cx);
        self.from_point = to_point;
    }

    fn zoom(&mut self, zoom_factor: f64, delta_y: f64, cx: &mut Cx) {
        let mut step = distance_to_center(cx, None);
        step.multiply(zoom_factor);
        let uo = cx.vp.user_orientation(cx.base);
        let translation = rot_vec(&uo, &Vector3::new(0.0, 0.0, step.norm()));

        if delta_y < 0.0 {
            let mut t = translation;
            t.negate();
            self.add_move(&t, &Vector3::ZERO, cx);
        } else if delta_y > 0.0 {
            self.add_move(&translation, &Vector3::ZERO, cx);
        }
    }

    fn add_move(&mut self, position_change: &Vector3, cor_change: &Vector3, cx: &mut Cx) {
        if self.position_chaser.connected {
            let p = add(&self.position_chaser.set_destination_value, position_change);
            // The set_destination *field* changes at once; only its interest (set_destination__)
            // waits for event processing (ExamineViewer.addMove reads `_set_destination .getValue ()`
            // for every mouse event of the frame).
            self.position_chaser.set_destination_value = p;
            self.pending.push(Pend::PosDest(p));
        } else {
            self.position_chaser.connected = true;
            let p = add(&cx.vp.position_offset, position_change);
            self.position_chaser.set_destination_value = p;
            self.pending.push(Pend::PosValue(cx.vp.position_offset));
            self.pending.push(Pend::PosDest(p));
        }

        if self.cor_chaser.connected {
            let p = add(&self.cor_chaser.set_destination_value, cor_change);
            self.cor_chaser.set_destination_value = p;
            self.pending.push(Pend::CorDest(p));
        } else {
            self.cor_chaser.connected = true;
            let p = add(&cx.vp.center_of_rotation_offset, cor_change);
            self.cor_chaser.set_destination_value = p;
            self.pending
                .push(Pend::CorValue(cx.vp.center_of_rotation_offset));
            self.pending.push(Pend::CorDest(p));
        }
    }

    pub fn prepare(&mut self, cx: &mut Cx) {
        let now = cx.env.now;
        let (pa, ca, ra) = (
            self.position_chaser.is_active,
            self.cor_chaser.is_active,
            self.rotation_chaser.is_active,
        );

        for p in std::mem::take(&mut self.pending) {
            match p {
                Pend::PosValue(v) => {
                    let o = self.position_chaser.set_value(v, now);
                    if self.position_chaser.connected {
                        cx.vp.position_offset = o;
                    }
                }
                Pend::PosDest(v) => self.position_chaser.set_destination(v, now),
                Pend::CorValue(v) => {
                    let o = self.cor_chaser.set_value(v, now);
                    if self.cor_chaser.connected {
                        cx.vp.center_of_rotation_offset = o;
                    }
                }
                Pend::CorDest(v) => self.cor_chaser.set_destination(v, now),
                Pend::RotValue(v) => {
                    let o = self.rotation_chaser.set_value(v, now);
                    if self.rotation_chaser.connected {
                        self.set_rotation(cx, &o);
                    }
                }
                Pend::RotDest(v) => self.rotation_chaser.set_destination(v, now),
            }
        }

        // prepareEvents runs the interests in the order the chasers became active. Chasers that
        // were not active when the frame began (activated by the events above) wait for the next one.
        let mut order: Vec<(u64, u8)> = Vec::new();
        if pa && self.position_chaser.is_active {
            order.push((self.position_chaser.activated_at, 0));
        }
        if ca && self.cor_chaser.is_active {
            order.push((self.cor_chaser.activated_at, 1));
        }
        if ra && self.rotation_chaser.is_active {
            order.push((self.rotation_chaser.activated_at, 2));
        }
        order.sort();

        for (_, which) in order {
            match which {
                0 => {
                    let o = self.position_chaser.prepare(now);
                    if self.position_chaser.connected {
                        cx.vp.position_offset = o;
                    }
                }
                1 => {
                    let o = self.cor_chaser.prepare(now);
                    if self.cor_chaser.connected {
                        cx.vp.center_of_rotation_offset = o;
                    }
                }
                _ => {
                    let o = self.rotation_chaser.prepare(now);
                    if self.rotation_chaser.connected {
                        self.set_rotation(cx, &o);
                    }
                }
            }
        }

        // The spin: a looping TimeSensor's fraction_changed, or a prepareEvents interest.
        if let Some((start, interval)) = self.spin_ts {
            let f = ((now - start) / interval) % 1.0;
            let f = if f < 0.0 { 0.0 } else { f };
            self.spin(cx, f);
        } else if self.spin_prepare {
            self.spin(cx, 0.0);
        }
    }

    /// ExamineViewer.set_rotation__.
    fn set_rotation(&mut self, cx: &mut Cx, value: &Rotation4) {
        let roll = self.roll;
        let init = self.initial_orientation_offset;
        let oo = self.get_orientation_offset(cx, &roll, value, &init);
        cx.vp.orientation_offset = oo;
        cx.vp.position_offset = self.get_position_offset(
            cx,
            &self.initial_position_offset,
            &self.initial_orientation_offset,
            &oo,
        );
    }
}

// --- FlyViewer / WalkViewer ------------------------------------------------------------------

enum FPend {
    Value(Rotation4),
    Dest(Rotation4),
}

pub struct Fly {
    pub walk: bool,
    button: i32,
    pub is_active: bool,
    control_key: bool,
    alt_key: bool,
    from_vector: Vector3,
    to_vector: Vector3,
    direction: Vector3,
    /// Date.now () of the last fly / pan step, 0 when not flying.
    start_time: f64,
    fly_active: bool,
    pan_active: bool,
    look_around: bool,
    chaser: Chaser<Rotation4>,
    pending: Vec<FPend>,
}

impl Fly {
    pub fn new(walk: bool, now: f64) -> Fly {
        Fly {
            walk,
            button: -1,
            is_active: false,
            control_key: false,
            alt_key: false,
            from_vector: Vector3::ZERO,
            to_vector: Vector3::ZERO,
            direction: Vector3::ZERO,
            start_time: 0.0,
            fly_active: false,
            pan_active: false,
            look_around: false,
            chaser: Chaser::new(ROTATE_TIME_FLY, now),
            pending: vec![],
        }
    }

    fn straighten_horizon(&self, cx: &Cx) -> bool {
        self.walk || cx.env.straighten
    }

    /// WalkViewer / FlyViewer.getTranslationOffset.
    fn translation_offset(&self, cx: &Cx, velocity: &Vector3) -> Vector3 {
        let uo = cx.vp.user_orientation(cx.base);

        if self.walk {
            let up = cx.base.up_vector(cx.vp, false);
            let local_y = rot_vec(&uo, &Vector3::Y_AXIS);
            let rotation = set_vectors(&local_y, &up);
            let orientation = mr(&uo, &rotation);
            rot_vec(&orientation, velocity)
        } else {
            rot_vec(&uo, velocity)
        }
    }

    fn constrain_pan_direction(&self, mut d: Vector3) -> Vector3 {
        if self.walk && d.y < 0.0 {
            d.y = 0.0;
        }
        d
    }

    pub fn input(&mut self, ev: &Input, cx: &mut Cx) {
        match *ev {
            Input::MouseDown { button, x, y } => self.mousedown(button as i32, x, y, cx),
            Input::MouseUp { button, .. } => self.mouseup(button as i32, cx),
            Input::MouseMove { x, y } => self.mousemove(x, y, cx),
            Input::Wheel { delta_y, x, y } => {
                if !in_rect(cx.env, x, y) {
                    return;
                }

                cx.vp.transition_stop();

                if delta_y < 0.0 {
                    self.add_roll(-ROLL_ANGLE, cx);
                } else if delta_y > 0.0 {
                    self.add_roll(ROLL_ANGLE, cx);
                }
            }
            // The Fly / Walk viewers' touch handlers are not ported (they have no dblclick).
            Input::DblClick { .. }
            | Input::TouchStart(_)
            | Input::TouchMove(_)
            | Input::TouchEnd => {}
        }
    }

    fn mousedown(&mut self, button: i32, x: f64, y: f64, cx: &mut Cx) {
        if self.button >= 0 {
            return;
        }

        if !in_rect(cx.env, x, y) {
            return;
        }

        self.control_key = cx.env.ctrl;
        self.alt_key = cx.env.alt;
        let button = get_button(button, self.alt_key);

        match button {
            0 => {
                self.button = button;
                self.disconnect();
                cx.vp.transition_stop();

                if self.control_key || self.look_around {
                    self.from_vector = trackball(cx.env, x, y);
                } else {
                    self.from_vector = Vector3::new(x, 0.0, -y);
                    self.to_vector = self.from_vector;
                    self.direction = sub(&self.to_vector, &self.from_vector);
                    self.add_fly(cx);
                }

                self.is_active = true;
            }
            1 => {
                self.button = button;
                self.disconnect();
                cx.vp.transition_stop();
                self.from_vector = Vector3::new(x, y, 0.0);
                self.to_vector = self.from_vector;
                self.direction = Vector3::ZERO;
                self.add_pan(cx);
                self.is_active = true;
            }
            _ => {}
        }
    }

    fn mouseup(&mut self, button: i32, _cx: &mut Cx) {
        let button = get_button(button, self.alt_key);

        if button != self.button {
            return;
        }

        self.direction = Vector3::ZERO;
        self.button = -1;
        self.disconnect();
        self.is_active = false;
    }

    fn mousemove(&mut self, x: f64, y: f64, cx: &mut Cx) {
        let button = get_button(self.button, self.alt_key);

        if button != self.button {
            return;
        }

        match button {
            0 => {
                if self.control_key || self.look_around {
                    let to = trackball(cx.env, x, y);
                    self.to_vector = to;
                    let from = self.from_vector;
                    self.add_rotation(&from, &to, cx);
                    self.from_vector = to;
                } else {
                    self.to_vector = Vector3::new(x, 0.0, -y);
                    self.direction = sub(&self.to_vector, &self.from_vector);
                }
            }
            1 => {
                self.to_vector = Vector3::new(x, y, 0.0);
                self.direction = sub(&self.to_vector, &self.from_vector);
            }
            _ => {}
        }
    }

    fn add_fly(&mut self, cx: &Cx) {
        if self.start_time != 0.0 {
            return;
        }

        self.fly_active = true;
        self.start_time = cx.env.date_now;
    }

    fn add_pan(&mut self, cx: &Cx) {
        if self.start_time != 0.0 {
            return;
        }

        self.disconnect();
        self.pan_active = true;
        self.start_time = cx.env.date_now;
    }

    pub fn disconnect(&mut self) {
        self.fly_active = false;
        self.pan_active = false;
        self.chaser.connected = false;
        self.start_time = 0.0;
    }

    /// X3DFlyViewer.fly.
    fn fly(&mut self, cx: &mut Cx) {
        let now = cx.env.date_now;
        let dt = now - self.start_time;

        let axis_back = Vector3::new(0.0, 0.0, 1.0);
        let mut rubber = if self.direction.z > 0.0 {
            set_vectors(&self.direction, &axis_back)
        } else {
            set_vectors(&Vector3::new(0.0, 0.0, -1.0), &self.direction)
        };
        let rubber_length = self.direction.norm();

        let mut speed_factor = 1.0 - rubber.angle() / (std::f64::consts::PI / 2.0);
        speed_factor *= cx.nav.speed;
        speed_factor *= cx.base.speed_factor(cx.vp);
        speed_factor *= if cx.env.shift {
            SHIFT_SPEED_FACTOR
        } else {
            SPEED_FACTOR
        };
        speed_factor *= dt;

        let mut d = self.direction;
        d.multiply(speed_factor);
        let translation = self.translation_offset(cx, &d);
        let mut constrained = (cx.constrain)(translation, &*cx.vp);
        constrained.add(&cx.vp.position_offset);
        cx.vp.position_offset = constrained;

        let weight =
            ROTATION_SPEED_FACTOR * dt * (rubber_length / (rubber_length + ROTATION_LIMIT)).powi(2);

        let mut user_orientation = Rotation4::identity();
        user_orientation.slerp(&rubber, weight);
        user_orientation.mult_right(&cx.vp.user_orientation(cx.base));

        if self.straighten_horizon(cx) {
            user_orientation = cx.base.straighten_horizon(cx.vp, &user_orientation);
        }

        let mut oo = inv(&cx.base.orientation);
        oo.mult_right(&user_orientation);
        cx.vp.orientation_offset = oo;

        self.start_time = now;
    }

    /// X3DFlyViewer.pan.
    fn pan(&mut self, cx: &mut Cx) {
        let now = cx.env.date_now;
        let dt = now - self.start_time;
        let up = cx.base.up_vector(cx.vp, false);

        let direction = self.constrain_pan_direction(self.direction);

        let mut speed_factor = 1.0;
        speed_factor *= cx.nav.speed;
        speed_factor *= cx.base.speed_factor(cx.vp);
        speed_factor *= if cx.env.shift {
            PAN_SHIFT_SPEED_FACTOR
        } else {
            PAN_SPEED_FACTOR
        };
        speed_factor *= dt;

        let uo = cx.vp.user_orientation(cx.base);
        let y_axis = rot_vec(&uo, &Vector3::Y_AXIS);
        let orientation = mr(&uo, &from_vectors(&y_axis, &up));
        let mut d = direction;
        d.multiply(speed_factor);
        let translation = rot_vec(&orientation, &d);
        let mut constrained = (cx.constrain)(translation, &*cx.vp);
        constrained.add(&cx.vp.position_offset);
        cx.vp.position_offset = constrained;

        self.start_time = now;
    }

    fn add_roll(&mut self, roll_angle: f64, cx: &mut Cx) {
        let roll = Rotation4::new(1.0, 0.0, 0.0, roll_angle);
        let mut oo = inv(&cx.base.orientation);
        oo.mult_right(&roll);

        if self.chaser.is_active && self.chaser.connected {
            // `.multRight (viewpoint .getOrientation ()) .multRight (orientationChaser ._set_destination)`
            oo.mult_right(&cx.base.orientation)
                .mult_right(&self.chaser.set_destination_value);
            self.chaser.set_destination_value = oo; // the field changes at once (FlyViewer addRotation/addRoll read it)
            self.pending.push(FPend::Dest(oo));
        } else {
            // `.multRight (viewpoint .getUserOrientation ())`
            oo.mult_right(&cx.vp.user_orientation(cx.base));
            self.pending.push(FPend::Value(cx.vp.orientation_offset));
            self.chaser.set_destination_value = oo; // the field changes at once (FlyViewer addRotation/addRoll read it)
            self.pending.push(FPend::Dest(oo));
        }

        self.disconnect();
        self.chaser.connected = true;
    }

    fn add_rotation(&mut self, from: &Vector3, to: &Vector3, cx: &mut Cx) {
        let up = cx.base.up_vector(cx.vp, false);

        if self.chaser.is_active && self.chaser.connected {
            let mut uo = set_vectors(to, from);
            uo.mult_right(&cx.base.orientation)
                .mult_right(&self.chaser.set_destination_value);

            if self.straighten_horizon(cx) {
                uo = cx.base.straighten_horizon(cx.vp, &uo);
            }

            if up.dot(&rot_vec(&uo, &Vector3::new(0.0, 0.0, 1.0))).abs() > CRITICAL_ANGLE_FLY {
                return;
            }

            let mut oo = inv(&cx.base.orientation);
            oo.mult_right(&uo);
            self.chaser.set_destination_value = oo; // the field changes at once (FlyViewer addRotation/addRoll read it)
            self.pending.push(FPend::Dest(oo));
        } else {
            let mut uo = set_vectors(to, from);
            uo.mult_right(&cx.vp.user_orientation(cx.base));

            if self.straighten_horizon(cx) {
                uo = cx.base.straighten_horizon(cx.vp, &uo);
            }

            if up.dot(&rot_vec(&uo, &Vector3::new(0.0, 0.0, 1.0))).abs() > CRITICAL_ANGLE_FLY {
                return;
            }

            let mut oo = inv(&cx.base.orientation);
            oo.mult_right(&uo);
            self.pending.push(FPend::Value(cx.vp.orientation_offset));
            self.chaser.set_destination_value = oo; // the field changes at once (FlyViewer addRotation/addRoll read it)
            self.pending.push(FPend::Dest(oo));
        }

        self.disconnect();
        self.chaser.connected = true;
    }

    pub fn prepare(&mut self, cx: &mut Cx) {
        let now = cx.env.now;
        let active = self.chaser.is_active;

        for p in std::mem::take(&mut self.pending) {
            match p {
                FPend::Value(v) => {
                    let o = self.chaser.set_value(v, now);
                    if self.chaser.connected {
                        cx.vp.orientation_offset = o;
                    }
                }
                FPend::Dest(v) => self.chaser.set_destination(v, now),
            }
        }

        if active && self.chaser.is_active {
            let o = self.chaser.prepare(now);
            if self.chaser.connected {
                cx.vp.orientation_offset = o;
            }
        }

        if self.fly_active {
            self.fly(cx);
        }

        if self.pan_active {
            self.pan(cx);
        }
    }
}
