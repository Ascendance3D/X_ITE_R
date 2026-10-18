//! Browser/Navigation/LookAtViewer.js.
//!
//! Left drag looks around (trackball vectors, an OrientationChaser); the wheel moves the viewpoint
//! toward the center of rotation (PositionChasers); a double click looks at the clicked shape
//! (X3DViewer.lookAtBBox) or, with Alt, at the clicked point (X3DViewer.lookAtPoint). The look at
//! itself is Viewpoint.lookAt, a viewpoint transition: see [`LookAtRequest`] and
//! `Viewer3d::look_at`.
//!
//! Like [`Examine`], the chasers' `_set_value` / `_set_destination` events are queued and delivered
//! at the start of the next prepare phase, and they tick in the order they last became active.
//! Unlike Examine, `addMove` and `addRotation` call `disconnect ()` for all three chasers and then
//! connect only their own, and a chaser that is connected but no longer active starts afresh from
//! the viewpoint's offsets.

use super::*;

const LOOKAT_SCROLL_FACTOR: f64 = 1.0 / 20.0; // 1 / 120 on Mac OS X and OculusBrowser
const LOOKAT_MOVE_TIME: f64 = 0.3;
const LOOKAT_ROTATE_TIME: f64 = 0.3;
const DBL_TAP_TIME: f64 = 0.4;

enum LPend {
    PosValue(Vector3),
    PosDest(Vector3),
    CorValue(Vector3),
    CorDest(Vector3),
    OriValue(Rotation4),
    OriDest(Rotation4),
}

pub struct LookAt {
    button: i32,
    pub is_active: bool,
    from_vector: Vector3,
    touch1: Vector2,
    touch2: Vector2,
    tap_start: f64,
    position_chaser: Chaser<Vector3>,
    cor_chaser: Chaser<Vector3>,
    orientation_chaser: Chaser<Rotation4>,
    pending: Vec<LPend>,
}

impl LookAt {
    pub fn new(now: f64) -> LookAt {
        LookAt {
            button: -1,
            is_active: false,
            from_vector: Vector3::ZERO,
            touch1: Vector2::ZERO,
            touch2: Vector2::ZERO,
            tap_start: 0.0,
            position_chaser: Chaser::new(LOOKAT_MOVE_TIME, now),
            cor_chaser: Chaser::new(LOOKAT_MOVE_TIME, now),
            orientation_chaser: Chaser::new(LOOKAT_ROTATE_TIME, now),
            pending: vec![],
        }
    }

    pub fn input(&mut self, ev: &Input, cx: &mut Cx) {
        match *ev {
            Input::MouseDown { button, x, y } => self.mousedown(button as i32, x, y, cx),
            Input::MouseUp { button, .. } => self.mouseup(button as i32),
            Input::MouseMove { x, y } => self.mousemove(x, y, cx),
            Input::Wheel { delta_y, x, y } => self.wheel(delta_y, None, x, y, cx),
            Input::DblClick { x, y } => self.dblclick(x, y, cx),
            Input::TouchStart(t) => self.touchstart(t, cx),
            Input::TouchMove(t) => self.touchmove(t, cx),
            Input::TouchEnd => self.touchend(cx),
        }
    }

    pub fn disconnect(&mut self) {
        self.orientation_chaser.connected = false;
        self.position_chaser.connected = false;
        self.cor_chaser.connected = false;
    }

    fn mousedown(&mut self, button: i32, x: f64, y: f64, cx: &mut Cx) {
        if self.button >= 0 {
            return;
        }

        if !in_rect(cx.env, x, y) {
            return;
        }

        if button == 0 {
            // Look around.
            self.button = button;

            cx.vp.transition_stop();
            self.from_vector = trackball(cx.env, x, y);
            self.is_active = true;
        }
    }

    fn mouseup(&mut self, button: i32) {
        if button != self.button {
            return;
        }

        self.button = -1;
        self.is_active = false;
    }

    fn dblclick(&mut self, x: f64, y: f64, cx: &mut Cx) {
        self.disconnect();
        cx.requests.push(LookAtRequest { x, y, point: cx.env.alt, straighten: cx.env.straighten });
    }

    fn mousemove(&mut self, x: f64, y: f64, cx: &mut Cx) {
        if self.button == 0 {
            // Look around.
            let to_vector = trackball(cx.env, x, y);

            self.add_rotation(&self.from_vector.clone(), &to_vector, cx);
            self.from_vector = to_vector;
        }
    }

    fn wheel(&mut self, delta_y: f64, zoom_factor: Option<f64>, x: f64, y: f64, cx: &mut Cx) {
        if !in_rect(cx.env, x, y) {
            return;
        }

        // Change viewpoint position.
        cx.vp.transition_stop();

        let zoom_factor = match zoom_factor {
            Some(z) if z != 0.0 && !z.is_nan() => z,
            _ => LOOKAT_SCROLL_FACTOR,
        };

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

    fn touchstart(&mut self, t: Touches, cx: &mut Cx) {
        match t.n {
            1 => {
                // Start move (button 0).
                self.touch1 = Vector2::new(t.p[0].0, t.p[0].1);
            }
            2 => {
                // End move (button 0).
                self.touchend(cx);

                // Start look around (button 0).
                let (x, y) = ((t.p[0].0 + t.p[1].0) / 2.0, (t.p[0].1 + t.p[1].1) / 2.0);
                self.mousedown(0, x, y, cx);

                // Start zoom (mouse wheel).
                self.touch1 = Vector2::new(t.p[0].0, t.p[0].1);
                self.touch2 = Vector2::new(t.p[1].0, t.p[1].1);
            }
            _ => {
                // End move (button 1).
                self.touchend(cx);
            }
        }
    }

    fn touchend(&mut self, cx: &mut Cx) {
        // End look around (button 0).
        self.mouseup(0);

        // Start dblclick (button 0).
        if cx.env.date_now - self.tap_start < DBL_TAP_TIME {
            let (x, y) = (self.touch1.x, self.touch1.y);
            self.dblclick(x, y, cx);
        }

        self.tap_start = cx.env.date_now;
    }

    fn touchmove(&mut self, t: Touches, cx: &mut Cx) {
        const MOVE_ANGLE: f64 = 0.7;
        const ZOOM_ANGLE: f64 = -0.7;

        if t.n != 2 {
            return;
        }

        let mut touch1_change = Vector2::new(t.p[0].0, t.p[0].1);
        touch1_change.subtract(&self.touch1).normalize();
        let mut touch2_change = Vector2::new(t.p[1].0, t.p[1].1);
        touch2_change.subtract(&self.touch2).normalize();

        let dot = touch1_change.dot(&touch2_change);
        let (x, y) = ((t.p[0].0 + t.p[1].0) / 2.0, (t.p[0].1 + t.p[1].1) / 2.0);

        if dot > MOVE_ANGLE {
            // Look around (button 0).
            self.mousemove(x, y, cx);
        } else if dot < ZOOM_ANGLE {
            // Zoom (mouse wheel).
            let distance1 = self.touch1.distance(&self.touch2);

            self.touch1 = Vector2::new(t.p[0].0, t.p[0].1);
            self.touch2 = Vector2::new(t.p[1].0, t.p[1].1);

            let distance2 = self.touch1.distance(&self.touch2);
            let delta = distance1 - distance2;
            let zoom_factor = delta.abs() / cx.env.viewport[2];

            self.wheel(delta, Some(zoom_factor), x, y, cx);
        }

        self.touch1 = Vector2::new(t.p[0].0, t.p[0].1);
        self.touch2 = Vector2::new(t.p[1].0, t.p[1].1);
    }

    fn add_move(&mut self, position_change: &Vector3, cor_change: &Vector3, cx: &mut Cx) {
        if self.position_chaser.is_active && self.position_chaser.connected {
            let p = add(&self.position_chaser.set_destination_value, position_change);
            self.position_chaser.set_destination_value = p;
            self.pending.push(LPend::PosDest(p));
        } else {
            let p = add(&cx.vp.position_offset, position_change);
            self.pending.push(LPend::PosValue(cx.vp.position_offset));
            self.position_chaser.set_destination_value = p;
            self.pending.push(LPend::PosDest(p));
        }

        if self.cor_chaser.is_active && self.cor_chaser.connected {
            let p = add(&self.cor_chaser.set_destination_value, cor_change);
            self.cor_chaser.set_destination_value = p;
            self.pending.push(LPend::CorDest(p));
        } else {
            let p = add(&cx.vp.center_of_rotation_offset, cor_change);
            self.pending.push(LPend::CorValue(cx.vp.center_of_rotation_offset));
            self.cor_chaser.set_destination_value = p;
            self.pending.push(LPend::CorDest(p));
        }

        self.disconnect();
        self.position_chaser.connected = true;
        self.cor_chaser.connected = true;
    }

    fn add_rotation(&mut self, from_vector: &Vector3, to_vector: &Vector3, cx: &mut Cx) {
        if self.orientation_chaser.is_active && self.orientation_chaser.connected {
            let mut user_orientation = set_vectors(to_vector, from_vector);
            user_orientation.mult_right(&cx.base.orientation).mult_right(&self.orientation_chaser.set_destination_value);
            let user_orientation = cx.base.straighten_horizon(cx.vp, &user_orientation);

            let mut offset = inv(&cx.base.orientation);
            offset.mult_right(&user_orientation);

            self.orientation_chaser.set_destination_value = offset;
            self.pending.push(LPend::OriDest(offset));
        } else {
            let mut user_orientation = set_vectors(to_vector, from_vector);
            user_orientation.mult_right(&cx.vp.user_orientation(cx.base));
            let user_orientation = cx.base.straighten_horizon(cx.vp, &user_orientation);

            let mut offset = inv(&cx.base.orientation);
            offset.mult_right(&user_orientation);

            self.pending.push(LPend::OriValue(cx.vp.orientation_offset));
            self.orientation_chaser.set_destination_value = offset;
            self.pending.push(LPend::OriDest(offset));
        }

        self.disconnect();
        self.orientation_chaser.connected = true;
    }

    /// The browser's prepareEvents.
    pub fn prepare(&mut self, cx: &mut Cx) {
        let now = cx.env.now;
        let (pa, ca, oa) = (self.position_chaser.is_active, self.cor_chaser.is_active, self.orientation_chaser.is_active);

        for p in std::mem::take(&mut self.pending) {
            match p {
                LPend::PosValue(v) => {
                    let o = self.position_chaser.set_value(v, now);
                    if self.position_chaser.connected {
                        cx.vp.position_offset = o;
                    }
                }
                LPend::PosDest(v) => self.position_chaser.set_destination(v, now),
                LPend::CorValue(v) => {
                    let o = self.cor_chaser.set_value(v, now);
                    if self.cor_chaser.connected {
                        cx.vp.center_of_rotation_offset = o;
                    }
                }
                LPend::CorDest(v) => self.cor_chaser.set_destination(v, now),
                LPend::OriValue(v) => {
                    let o = self.orientation_chaser.set_value(v, now);
                    if self.orientation_chaser.connected {
                        cx.vp.orientation_offset = o;
                    }
                }
                LPend::OriDest(v) => self.orientation_chaser.set_destination(v, now),
            }
        }

        // prepareEvents runs the interests in the order the chasers became active (see Examine::prepare).
        let mut order: Vec<(u64, u8)> = Vec::new();
        if pa && self.position_chaser.is_active { order.push((self.position_chaser.activated_at, 0)); }
        if ca && self.cor_chaser.is_active { order.push((self.cor_chaser.activated_at, 1)); }
        if oa && self.orientation_chaser.is_active { order.push((self.orientation_chaser.activated_at, 2)); }
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
                    let o = self.orientation_chaser.prepare(now);
                    if self.orientation_chaser.connected {
                        cx.vp.orientation_offset = o;
                    }
                }
            }
        }
    }
}
