//! Browser/Navigation/PlaneViewer.js.
//!
//! The PlaneViewer has no chasers: every handler writes the viewpoint's user offsets at once.
//! Pan is the MIDDLE button (or Alt + left); a plain left drag does nothing in the JS (case 0 of
//! `mousedown` registers no `mousemove`). The wheel changes `fieldOfViewScale` (for a perspective
//! Viewpoint and an OrthoViewpoint alike) and moves the viewpoint so that the point under the
//! pointer stays where it is.

use super::*;

pub struct Plane {
    button: i32,
    alt_key: bool,
    pub is_active: bool,
    from_point: Vector3,
    to_point: Vector3,
    touch_mode: i32,
    touch1: Vector2,
    touch2: Vector2,
    /// `tapedTwice`, valid while Date.now () is before `tap_until` (the 300 ms setTimeout).
    taped_twice: bool,
    tap_until: f64,
}

impl Default for Plane {
    fn default() -> Self {
        Self::new()
    }
}

impl Plane {
    pub fn new() -> Plane {
        Plane {
            button: -1,
            alt_key: false,
            is_active: false,
            from_point: Vector3::ZERO,
            to_point: Vector3::ZERO,
            touch_mode: 0,
            touch1: Vector2::ZERO,
            touch2: Vector2::ZERO,
            taped_twice: false,
            tap_until: 0.0,
        }
    }

    pub fn input(&mut self, ev: &Input, cx: &mut Cx) {
        match *ev {
            Input::MouseDown { button, x, y } => self.mousedown(button as i32, x, y, cx),
            Input::MouseUp { button, .. } => self.mouseup(button as i32, cx),
            Input::MouseMove { x, y } => self.mousemove(x, y, cx),
            Input::Wheel { delta_y, x, y } => self.wheel(delta_y, None, x, y, cx),
            Input::DblClick { x, y } => self.dblclick(x, y, cx),
            Input::TouchStart(t) => self.touchstart(t, cx),
            Input::TouchMove(t) => self.touchmove(t, cx),
            Input::TouchEnd => self.touchend(cx),
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
                // Only mouseup is bound: a left drag does nothing.
                self.button = button;
            }
            1 => {
                self.button = button;
                cx.vp.transition_stop();
                self.from_point = point_on_center_plane(cx, x, y);
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

        self.button = -1;
        self.is_active = false;
    }

    /// PlaneViewer.dblclick: only with DEVELOPMENT.
    fn dblclick(&mut self, x: f64, y: f64, cx: &mut Cx) {
        if !DEVELOPMENT {
            return;
        }

        cx.requests.push(LookAtRequest {
            x,
            y,
            point: cx.env.alt,
            straighten: cx.env.straighten,
        });
    }

    fn mousemove(&mut self, x: f64, y: f64, cx: &mut Cx) {
        let button = get_button(self.button, self.alt_key);

        if button != self.button {
            return;
        }

        if button == 1 {
            let to_point = point_on_center_plane(cx, x, y);
            self.to_point = to_point;
            let uo = cx.vp.user_orientation(cx.base);
            let translation = rot_vec(&uo, &sub(&self.from_point, &to_point));

            cx.vp.position_offset = add(&cx.vp.position_offset, &translation);
            cx.vp.center_of_rotation_offset = add(&cx.vp.center_of_rotation_offset, &translation);

            self.from_point = to_point;
        }
    }

    fn wheel(&mut self, delta_y: f64, zoom_factor: Option<f64>, x: f64, y: f64, cx: &mut Cx) {
        if !in_rect(cx.env, x, y) {
            return;
        }

        let from_point = point_on_center_plane(cx, x, y);
        self.from_point = from_point;

        cx.vp.transition_stop();

        let field_of_view_scale = cx.vp.field_of_view_scale;
        // `event .zoomFactor || SCROLL_FACTOR`
        let zoom_factor = match zoom_factor {
            Some(z) if z != 0.0 && !z.is_nan() => z,
            _ => SCROLL_FACTOR,
        };

        if delta_y < 0.0 {
            // Move backwards.
            cx.vp.field_of_view_scale =
                x3d_fields::jsmath::js_max(0.00001, field_of_view_scale * (1.0 - zoom_factor));
        } else if delta_y > 0.0 {
            // Move forwards.
            cx.vp.field_of_view_scale = field_of_view_scale * (1.0 + zoom_factor);
            self.constrain_field_of_view_scale(cx);
        }

        let to_point = point_on_center_plane(cx, x, y);
        self.to_point = to_point;
        let uo = cx.vp.user_orientation(cx.base);
        let translation = rot_vec(&uo, &sub(&from_point, &to_point));

        cx.vp.position_offset = add(&cx.vp.position_offset, &translation);
        cx.vp.center_of_rotation_offset = add(&cx.vp.center_of_rotation_offset, &translation);
    }

    /// PlaneViewer.constrainFieldOfViewScale: `Viewpoint` and `GeoViewpoint` only.
    fn constrain_field_of_view_scale(&self, cx: &mut Cx) {
        if !cx.base.is_ortho
            && cx.base.field_of_view * cx.vp.field_of_view_scale >= std::f64::consts::PI
        {
            cx.vp.field_of_view_scale = (std::f64::consts::PI - 0.001) / cx.base.field_of_view;
        }
    }

    fn touchstart(&mut self, t: Touches, cx: &mut Cx) {
        match t.n {
            1 => {
                // Start tab (button 0).
                self.mousedown(0, t.p[0].0, t.p[0].1, cx);

                // Remember tap.
                self.touch1 = Vector2::new(t.p[0].0, t.p[0].1);
            }
            2 => {
                // End tab (button 0).
                self.touchend(cx);

                // Start move (button 1).
                let (x, y) = ((t.p[0].0 + t.p[1].0) / 2.0, (t.p[0].1 + t.p[1].1) / 2.0);
                self.mousedown(1, x, y, cx);

                // Start zoom (mouse wheel).
                self.touch1 = Vector2::new(t.p[0].0, t.p[0].1);
                self.touch2 = Vector2::new(t.p[1].0, t.p[1].1);
            }
            _ => {}
        }
    }

    fn touchend(&mut self, cx: &mut Cx) {
        match self.button {
            0 => {
                // End tab (button 0).
                self.mouseup(0, cx);

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
                // End move (button 0).
                self.touch_mode = 0;
                self.mouseup(1, cx);
            }
            _ => {}
        }
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
        let (move_, zoom) = (dot > MOVE_ANGLE, dot < ZOOM_ANGLE);
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
                // Move (button 1).
                self.touch_mode = 1;
                self.mousemove(x, y, cx);
            }
            2 => {
                // Zoom (mouse wheel).
                self.touch_mode = 2;

                let distance1 = self.touch1.distance(&self.touch2);

                self.touch1 = Vector2::new(t.p[0].0, t.p[0].1);
                self.touch2 = Vector2::new(t.p[1].0, t.p[1].1);

                let distance2 = self.touch1.distance(&self.touch2);
                let delta = distance1 - distance2;
                // `window .innerWidth`: the canvas width here.
                let zoom_factor = delta.abs() / cx.env.viewport[2];

                self.wheel(delta, Some(zoom_factor), x, y, cx);
            }
            _ => {}
        }

        self.touch1 = Vector2::new(t.p[0].0, t.p[0].1);
        self.touch2 = Vector2::new(t.p[1].0, t.p[1].1);
    }
}
