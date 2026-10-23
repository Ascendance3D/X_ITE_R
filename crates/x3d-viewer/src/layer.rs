//! Bindable stacks and viewpoint state: Execution/BindableStack.js, Execution/BindableList.js,
//! Components/Layering/X3DLayerNode.js (bindBindables, camera), Components/Core/X3DBindableNode.js,
//! Components/Navigation/X3DViewpointNode.js, Viewpoint.js and NavigationInfo.js.
//!
//! Viewpoint private fields (positionOffset, orientationOffset, scaleOffset, ...) live in
//! [`Vp`] here, not in node fields. Transitions (the viewpoint's private TimeSensor,
//! EaseInEaseOut and interpolators) are a small [`Transition`] ticked on the frame clock.

use crate::geom::Camera;
use std::collections::HashMap;
use x3d_fields::math::{Matrix4, Rotation4, Vector3};
use x3d_fields::{FieldValue, NodeId};
use x3d_render::traverse::{perspective, straighten};
use x3d_runtime::Runtime;

/// The layer's default Viewpoint / NavigationInfo / Background / Fog (stack bottom).
pub const DEFAULT: NodeId = NodeId::MAX;

/// A LayoutLayer's default viewpoint: an OrthoViewpoint (LayoutLayer: `new OrthoViewpoint`).
pub const DEFAULT_ORTHO: NodeId = NodeId::MAX - 1;

/// One of the layers' default bindables.
pub fn is_default(n: NodeId) -> bool {
    n >= DEFAULT_ORTHO
}

const VIEW_ALL_SCALE_FACTOR: f64 = 1.1;

// --- NavigationInfo ----------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct NavInfo {
    pub types: Vec<String>,
    pub avatar_size: Vec<f64>,
    pub speed: f64,
    pub headlight: bool,
    pub visibility_limit: f64,
    pub transition_type: Vec<String>,
    pub transition_time: f64,
}

impl NavInfo {
    pub fn read(rt: &Runtime, n: NodeId) -> NavInfo {
        if n == DEFAULT {
            return NavInfo {
                types: vec!["EXAMINE".into(), "ANY".into()],
                avatar_size: vec![0.25, 1.6, 0.75],
                speed: 1.0,
                headlight: true,
                visibility_limit: 0.0,
                transition_type: vec!["LINEAR".into()],
                transition_time: 1.0,
            };
        }

        let strings = |name: &str| match rt.pre_value(n, name) {
            Some(FieldValue::MFString(v)) => v.clone(),
            _ => vec![],
        };

        NavInfo {
            types: strings("type"),
            avatar_size: match rt.pre_value(n, "avatarSize") {
                Some(FieldValue::MFFloat(v)) => v.iter().map(|x| *x as f64).collect(),
                _ => vec![],
            },
            speed: rt.get_f64(n, "speed"),
            headlight: rt.get_bool(n, "headlight"),
            visibility_limit: rt.get_f64(n, "visibilityLimit"),
            transition_type: strings("transitionType"),
            transition_time: rt.get_f64(n, "transitionTime"),
        }
    }

    /// NavigationInfo.set_type__: the active viewer.
    pub fn viewer(&self) -> String {
        for t in &self.types {
            match t.as_str() {
                "EXAMINE" | "WALK" | "FLY" | "LOOKAT" | "PLANE" | "NONE" => return t.clone(),
                "EXPLORE" => return "EXAMINE".into(),
                "PLANE_create3000.github.io" | "PLANE_create3000.de" => return "PLANE".into(),
                _ => continue,
            }
        }

        "EXAMINE".into()
    }

    /// NavigationInfo.set_type__: availableViewers.
    pub fn available_viewers(&self) -> Vec<String> {
        let (mut examine, mut walk, mut fly, mut plane, mut none, mut lookat) =
            (false, false, false, false, false, false);

        if self.types.is_empty() {
            examine = true;
            walk = true;
            fly = true;
            plane = true;
            none = true;
            lookat = true;
        } else {
            for s in &self.types {
                match s.as_str() {
                    "EXAMINE" => examine = true,
                    "WALK" => walk = true,
                    "FLY" => fly = true,
                    "LOOKAT" => lookat = true,
                    "PLANE" | "PLANE_create3000.github.io" | "PLANE_create3000.de" => plane = true,
                    "NONE" => none = true,
                    "ANY" => {
                        examine = true;
                        walk = true;
                        fly = true;
                        plane = true;
                        none = true;
                        lookat = true;
                        break;
                    }
                    _ => examine = true,
                }
            }
        }

        let mut v = vec![];
        for (on, name) in [
            (examine, "EXAMINE"),
            (walk, "WALK"),
            (fly, "FLY"),
            (plane, "PLANE"),
            (lookat, "LOOKAT"),
            (none, "NONE"),
        ] {
            if on {
                v.push(name.to_string());
            }
        }
        v
    }

    pub fn collision_radius(&self) -> f64 {
        self.avatar_size.first().copied().unwrap_or(0.25).max(0.0)
    }

    pub fn avatar_height(&self) -> f64 {
        self.avatar_size.get(1).copied().unwrap_or(1.6).max(0.0)
    }

    pub fn step_height(&self) -> f64 {
        self.avatar_size.get(2).copied().unwrap_or(0.75).max(0.0)
    }

    pub fn near_value(&self) -> f64 {
        let r = self.collision_radius();
        if r == 0.0 { 0.125 } else { r / 2.0 }
    }

    pub fn far_value(&self) -> f64 {
        self.visibility_limit.max(0.0)
    }

    /// NavigationInfo.getTransitionType.
    pub fn transition_kind(&self) -> &str {
        for v in &self.transition_type {
            if matches!(v.as_str(), "TELEPORT" | "LINEAR" | "ANIMATE") {
                return v;
            }
        }
        "LINEAR"
    }
}

// --- Viewpoint ---------------------------------------------------------------------------------

/// A viewpoint's own field values (position, orientation, ...).
#[derive(Clone, Debug)]
pub struct VpBase {
    pub position: Vector3,
    pub orientation: Rotation4,
    pub center_of_rotation: Vector3,
    pub field_of_view: f64,
    pub jump: bool,
    pub retain_user_offsets: bool,
    pub view_all: bool,
    pub near_field: Option<f64>,
    pub far_field: Option<f64>,
    pub description: String,
    /// OrthoViewpoint (Components/Navigation/OrthoViewpoint.js): `fieldOfView` is minimumX, minimumY,
    /// maximumX, maximumY (an SFVec4f, or an MFFloat for specification versions up to 4.0:
    /// `this ._fieldOfView [0] ?? -1` ...).
    pub is_ortho: bool,
    pub ortho_fov: [f64; 4],
    /// GeoViewpoint: `position`, `orientation` and `centerOfRotation` above are already the
    /// results of getPosition, getOrientation and getCenterOfRotation.
    pub geo: Option<GeoVp>,
}

/// GeoViewpoint (Components/Geospatial/GeoViewpoint.js): the X3DGeospatialObject frame and
/// `speedFactor`.
#[derive(Clone, Copy, Debug)]
pub struct GeoVp {
    pub object: x3d_runtime::geo::GeospatialObject,
    pub speed_factor: f64,
    /// The rotation of getLocationMatrix (position): `_orientation⁻¹ · getOrientation ()`.
    pub location_rotation: Rotation4,
}

impl VpBase {
    pub fn read(rt: &Runtime, n: NodeId) -> VpBase {
        if is_default(n) {
            return VpBase {
                position: Vector3::new(0.0, 0.0, 10.0),
                orientation: Rotation4::identity(),
                center_of_rotation: Vector3::ZERO,
                field_of_view: 0.785398,
                jump: true,
                retain_user_offsets: false,
                view_all: false,
                near_field: None,
                far_field: None,
                description: String::new(),
                is_ortho: n == DEFAULT_ORTHO,
                ortho_fov: [-1.0, -1.0, 1.0, 1.0],
                geo: None,
            };
        }

        let sg = x3d_render::sg::Sg::new(rt);
        let ge0 = |v: f64| if v >= 0.0 { Some(v) } else { None };
        let is_ortho = rt.builtin_name(n) == Some("OrthoViewpoint");
        let ortho_fov = match sg.value(n, "fieldOfView") {
            Some(FieldValue::SFVec4f(v)) if is_ortho => [v.x, v.y, v.z, v.w],
            Some(FieldValue::MFFloat(v)) if is_ortho => {
                let at = |i: usize, d: f64| v.get(i).map_or(d, |x| *x as f64);
                [at(0, -1.0), at(1, -1.0), at(2, 1.0), at(3, 1.0)]
            }
            _ => [-1.0, -1.0, 1.0, 1.0],
        };

        let mut position = sg.vec3(n, "position", Vector3::new(0.0, 0.0, 10.0));
        let mut orientation = sg.rotation(n, "orientation");
        let mut center_of_rotation = sg.vec3(n, "centerOfRotation", Vector3::ZERO);
        let mut geo = (rt.builtin_name(n) == Some("GeoViewpoint")).then(|| GeoVp {
            object: sg.geo_object(n),
            speed_factor: sg.f64(n, "speedFactor", 1.0),
            location_rotation: Rotation4::identity(),
        });

        if let Some(g) = &mut geo {
            // GeoViewpoint.getOrientation: the location frame's rotation at `position`, then
            // `multLeft (this ._orientation)`.
            g.location_rotation
                .set_matrix(&g.object.get_location_matrix(&position).submatrix());
            let mut r = g.location_rotation;
            r.mult_left(&orientation);
            orientation = r;
            position = g.object.get_coord(&position);
            center_of_rotation = g.object.get_coord(&center_of_rotation);
        }

        VpBase {
            position,
            orientation,
            center_of_rotation,
            field_of_view: sg.f64(n, "fieldOfView", 0.785398),
            jump: sg.bool(n, "jump", true),
            retain_user_offsets: sg.bool(n, "retainUserOffsets", false),
            view_all: sg.bool(n, "viewAll", false),
            near_field: ge0(sg.f64(n, "nearDistance", -1.0)),
            far_field: ge0(sg.f64(n, "farDistance", -1.0)),
            description: match sg.value(n, "description") {
                Some(FieldValue::SFString(s)) => s.clone(),
                _ => String::new(),
            },
            is_ortho,
            ortho_fov,
            geo,
        }
    }

    /// X3DViewpointNode.getUpVector (the Y axis) and GeoViewpoint.getUpVector: the geo up vector
    /// at the user position, or with `dynamic` the Z axis once the user is 6.5e6 m away.
    pub fn up_vector(&self, vp: &Vp, dynamic: bool) -> Vector3 {
        let Some(g) = &self.geo else {
            return Vector3::Y_AXIS;
        };
        let position = vp.user_position(self);

        if !dynamic || position.norm() < 6.5e6 {
            g.object.get_geo_up_vector(&position)
        } else {
            Vector3::Z_AXIS
        }
    }

    /// X3DViewpointNode.getSpeedFactor (1) and GeoViewpoint.getSpeedFactor: faster with the
    /// elevation (set_position__) of the user position.
    pub fn speed_factor(&self, vp: &Vp) -> f64 {
        let Some(g) = &self.geo else { return 1.0 };
        let elevation = g.object.get_geo_elevation(&vp.user_position(self));

        (x3d_fields::jsmath::js_max(elevation, 0.0) + 10.0) / 10.0 * g.speed_factor
    }

    /// `_orientation⁻¹ · getOrientation ()` (identity but for a GeoViewpoint).
    pub fn local_orientation(&self) -> Rotation4 {
        self.geo
            .map_or(Rotation4::identity(), |g| g.location_rotation)
    }

    /// X3DViewpointNode.straightenHorizon (orientation, getUpVector (true)).
    pub fn straighten_horizon(&self, vp: &Vp, orientation: &Rotation4) -> Rotation4 {
        straighten(orientation, &self.up_vector(vp, true))
    }

    /// The classes compared by `this .constructor !== fromViewpointNode .constructor`.
    pub fn same_class(&self, other: &VpBase) -> bool {
        self.is_ortho == other.is_ortho && self.geo.is_some() == other.geo.is_some()
    }

    /// Viewpoint.getFieldOfView.
    pub fn fov(&self) -> f64 {
        let f = self.field_of_view;
        if f > 0.0 && f < std::f64::consts::PI {
            f
        } else {
            std::f64::consts::FRAC_PI_4
        }
    }
}

/// X3DViewpointNode state that is not a field.
#[derive(Clone, Debug)]
pub struct Vp {
    pub position_offset: Vector3,
    pub orientation_offset: Rotation4,
    pub scale_offset: Vector3,
    pub scale_orientation_offset: Rotation4,
    pub center_of_rotation_offset: Vector3,
    pub field_of_view_scale: f64,
    /// OrthoViewpoint ._fieldOfViewOffset.
    pub field_of_view_offset: [f64; 4],
    /// X3DViewpointNode nearDistance / farDistance (set by lookAt / viewAll, else the field).
    pub near_override: Option<Option<f64>>,
    pub far_override: Option<Option<f64>>,
    pub model: Matrix4,
    pub camera_space: Matrix4,
    pub view: Matrix4,
    pub vrml_transition: bool,
    pub transition: Option<Transition>,
    /// timeSensor ._description (checkTransition).
    pub transition_description: String,
}

impl Default for Vp {
    fn default() -> Vp {
        Vp {
            position_offset: Vector3::ZERO,
            orientation_offset: Rotation4::identity(),
            scale_offset: Vector3::ONE,
            scale_orientation_offset: Rotation4::identity(),
            center_of_rotation_offset: Vector3::ZERO,
            field_of_view_scale: 1.0,
            field_of_view_offset: [0.0; 4],
            near_override: None,
            far_override: None,
            model: Matrix4::IDENTITY,
            camera_space: Matrix4([
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 10.0, 1.0,
            ]),
            view: Matrix4([
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, -10.0, 1.0,
            ]),
            vrml_transition: false,
            transition: None,
            transition_description: String::new(),
        }
    }
}

/// The viewpoint's private TimeSensor + EaseInEaseOut + interpolators.
#[derive(Clone, Debug)]
pub struct Transition {
    pub start: f64,
    pub interval: f64,
    /// easeInEaseOut [0].y (easeOut of key 0) and [1].x (easeIn of key 1).
    pub ease_out: f64,
    pub ease_in: f64,
    pub position: (Vector3, Vector3),
    pub orientation: (Rotation4, Rotation4),
    pub scale: (Vector3, Vector3),
    pub scale_orientation: (Rotation4, Rotation4),
    pub fov_scale: (f64, f64),
    /// OrthoViewpoint fieldOfViewOffsetInterpolator 0..3 (key values at fraction 0 and 1).
    pub fov_offset: Option<([f64; 4], [f64; 4])>,
    /// The NavigationInfo to notify when the transition completes (set_active__).
    pub nav_info: NodeId,
    /// Whether `set_active__` is still an interest of the timeSensor's isActive when it stops
    /// (a lookAt that interrupts a running transition loses it: its own stop event removes it).
    pub notify: bool,
    /// The TimeSensor was started from an input handler: `do_start` runs at the first event
    /// processing of the next frame, where `startTime` is Date.now () of the event and the current
    /// time is that frame's (set_start: offset = (time - startTime) / interval, cycle = time - fraction * interval).
    pub lazy_start: bool,
}

/// EaseInEaseOut.interpolate with two keys.
fn ease(weight: f64, ease_out_in: f64, ease_in_in: f64) -> f64 {
    let (mut ease_out, mut ease_in) = (ease_out_in, ease_in_in);
    let sum = ease_out + ease_in;

    if sum < 0.0 {
        return weight;
    }

    if sum > 1.0 {
        ease_in /= sum;
        ease_out /= sum;
    }

    let t = 1.0 / (2.0 - ease_out - ease_in);

    if weight < ease_out {
        (t / ease_out) * weight * weight
    } else if weight <= 1.0 - ease_in {
        t * (2.0 * weight - ease_out)
    } else {
        let w = 1.0 - weight;
        1.0 - ((t * w * w) / ease_in)
    }
}

fn lerp3(a: &Vector3, b: &Vector3, t: f64) -> Vector3 {
    let mut r = *a;
    r.lerp(b, t);
    r
}

fn slerp(a: &Rotation4, b: &Rotation4, t: f64) -> Rotation4 {
    let mut r = *a;
    r.slerp(b, t);
    r
}

impl Vp {
    pub fn user_position(&self, base: &VpBase) -> Vector3 {
        let mut p = base.position;
        p.add(&self.position_offset);
        p
    }

    pub fn user_orientation(&self, base: &VpBase) -> Rotation4 {
        let mut o = base.orientation;
        o.mult_right(&self.orientation_offset);
        o
    }

    pub fn user_center_of_rotation(&self, base: &VpBase) -> Vector3 {
        let mut c = base.center_of_rotation;
        c.add(&self.center_of_rotation_offset);
        c
    }

    /// Viewpoint.getUserFieldOfView.
    pub fn user_fov(&self, base: &VpBase) -> f64 {
        let f = base.field_of_view * self.field_of_view_scale;
        if f > 0.0 && f < std::f64::consts::PI {
            f
        } else {
            std::f64::consts::FRAC_PI_4
        }
    }

    /// OrthoViewpoint.getUserMinimumX / Y, getUserMaximumX / Y (index 0..3).
    pub fn user_extent(&self, base: &VpBase, i: usize) -> f64 {
        (base.ortho_fov[i] + self.field_of_view_offset[i]) * self.field_of_view_scale
    }

    pub fn user_size_x(&self, base: &VpBase) -> f64 {
        self.user_extent(base, 2) - self.user_extent(base, 0)
    }

    pub fn user_size_y(&self, base: &VpBase) -> f64 {
        self.user_extent(base, 3) - self.user_extent(base, 1)
    }

    /// X3DViewpointNode.getMaxFarValue (OrthoViewpoint: 1e4, GeoViewpoint: 1e9).
    pub fn max_far_value(base: &VpBase) -> f64 {
        if base.is_ortho {
            1e4
        } else if base.geo.is_some() {
            1e9
        } else {
            max_far()
        }
    }

    /// Viewpoint / OrthoViewpoint.getProjectionMatrixWithLimits.
    pub fn projection(
        &self,
        base: &VpBase,
        near: f64,
        far: f64,
        width: f64,
        height: f64,
    ) -> Matrix4 {
        if !base.is_ortho {
            return perspective(self.user_fov(base), near, far, width, height);
        }

        let ratio = width / height;
        let (size_x, size_y) = (self.user_size_x(base), self.user_size_y(base));
        let mut m = Matrix4::IDENTITY;

        if ratio > size_x / size_y {
            let center = (self.user_extent(base, 0) + self.user_extent(base, 2)) / 2.0;
            let size1_2 = (size_y * ratio) / 2.0;
            Camera::ortho(
                center - size1_2,
                center + size1_2,
                self.user_extent(base, 1),
                self.user_extent(base, 3),
                near,
                far,
                &mut m,
            );
        } else {
            let center = (self.user_extent(base, 1) + self.user_extent(base, 3)) / 2.0;
            let size1_2 = (size_x / ratio) / 2.0;
            Camera::ortho(
                self.user_extent(base, 0),
                self.user_extent(base, 2),
                center - size1_2,
                center + size1_2,
                near,
                far,
                &mut m,
            );
        }

        m
    }

    pub fn near_distance(&self, base: &VpBase, nav: &NavInfo) -> f64 {
        let v = match self.near_override {
            Some(o) => o,
            None => base.near_field,
        };
        v.unwrap_or_else(|| nav.near_value())
    }

    pub fn far_distance(&self, base: &VpBase, nav: &NavInfo) -> f64 {
        let v = match self.far_override {
            Some(o) => o,
            None => base.far_field,
        };
        v.unwrap_or_else(|| {
            if nav.far_value() != 0.0 {
                nav.far_value()
            } else {
                Self::max_far_value(base)
            }
        })
    }

    /// X3DViewpointNode.update.
    pub fn update(&mut self, base: &VpBase) {
        let mut m = Matrix4::IDENTITY;
        m.set_transform(
            Some(&self.user_position(base)),
            Some(&self.user_orientation(base)),
            Some(&self.scale_offset),
            Some(&self.scale_orientation_offset),
            None,
        );
        m.mult_right(&self.model);
        self.camera_space = m;
        let mut v = m;
        v.inverse();
        self.view = v;
    }

    /// X3DViewpointNode.resetUserOffsets.
    pub fn reset_user_offsets(&mut self, base: &VpBase) {
        self.position_offset = Vector3::ZERO;
        self.orientation_offset = Rotation4::identity();
        self.scale_offset = Vector3::ONE;
        self.scale_orientation_offset = Rotation4::identity();
        self.center_of_rotation_offset = Vector3::ZERO;
        self.field_of_view_scale = 1.0;
        self.field_of_view_offset = [0.0; 4];
        self.near_override = None;
        self.far_override = None;
        self.update(base);
    }

    /// X3DViewpointNode.getLookAtRotation.
    pub fn look_at_rotation(&self, base: &VpBase, from: &Vector3, to: &Vector3) -> Rotation4 {
        let up = base.up_vector(self, true);
        let mut z = *from;
        z.subtract(to).normalize();
        let mut x = up;
        x.cross(&z).normalize();
        let mut y = z;
        y.cross(&x).normalize();
        let m = x3d_fields::math::Matrix3([x.x, x.y, x.z, y.x, y.y, y.z, z.x, z.y, z.z]);
        let mut r = Rotation4::identity();
        r.set_matrix(&m);
        r
    }

    /// Relative transformation between this viewpoint and `from` (getRelativeTransformation).
    fn relative(&self, base: &VpBase, from_view: &Matrix4) -> Relative {
        let mut diff = self.model;
        diff.mult_right(from_view);
        diff.inverse();

        let (mut position, mut orientation, scale, scale_orientation);
        (position, orientation, scale, scale_orientation) = factor_simple(&diff);

        position.subtract(&base.position);
        let mut inv = base.orientation;
        inv.inverse();
        orientation.mult_left(&inv);

        Relative {
            position,
            orientation,
            scale,
            scale_orientation,
        }
    }

    /// Viewpoint.setInterpolators / OrthoViewpoint.setInterpolators (called by transitionStart and
    /// by lookAt with `from` = this viewpoint): sets the field of view scale (and the ortho offsets)
    /// at once and returns the interpolator key values: (scale from, to), (offset from, to).
    pub fn set_interpolators(
        &mut self,
        base: &VpBase,
        from: &FromState,
    ) -> ((f64, f64), Option<([f64; 4], [f64; 4])>) {
        if !from.same {
            self.field_of_view_scale = 1.0;
            let offset = self.field_of_view_offset;
            return ((1.0, 1.0), base.is_ortho.then_some((offset, offset)));
        }

        if base.is_ortho {
            let mut offset = [0.0; 4];

            for (i, o) in offset.iter_mut().enumerate() {
                *o = from.user_extent[i] - base.ortho_fov[i];
            }

            let keys = (offset, self.field_of_view_offset);
            self.field_of_view_offset = offset;
            self.field_of_view_scale = from.fov_scale;
            return ((from.fov_scale, 1.0), Some(keys));
        }

        let scale = from.user_fov / self.user_fov(base);
        self.field_of_view_scale = scale;
        ((scale, 1.0), None)
    }

    #[allow(clippy::too_many_arguments)]
    fn start_transition(
        &mut self,
        now: f64,
        interval: f64,
        easing: bool,
        nav_info: NodeId,
        rel: &Relative,
        from: &FromState,
        base: &VpBase,
    ) {
        // ANIMATE: easeInEaseOut = [(0, 1), (1, 0)] → easeOut = [0].y = 1, easeIn = [1].x = 1.
        let (ease_out, ease_in) = if easing { (1.0, 1.0) } else { (0.0, 0.0) };

        let mut t = Transition {
            start: now,
            interval,
            ease_out,
            ease_in,
            position: (rel.position, self.position_offset),
            orientation: (rel.orientation, self.orientation_offset),
            scale: (rel.scale, self.scale_offset),
            scale_orientation: (rel.scale_orientation, self.scale_orientation_offset),
            fov_scale: (1.0, 1.0),
            fov_offset: None,
            nav_info,
            notify: true,
            lazy_start: false,
        };

        self.position_offset = rel.position;
        self.orientation_offset = rel.orientation;
        self.scale_offset = rel.scale;
        self.scale_orientation_offset = rel.scale_orientation;

        let (fov_scale, fov_offset) = self.set_interpolators(base, from);
        t.fov_scale = fov_scale;
        t.fov_offset = fov_offset;
        self.transition = Some(t);
    }

    /// One frame of the transition. Returns `Some(nav_info)` when it just completed.
    pub fn tick_transition(&mut self, now: f64) -> Option<NodeId> {
        let t = self.transition.as_mut()?;

        if t.lazy_start {
            // TimeSensor.set_start (offset = true): the sensor starts in the first event processing of
            // this frame; cycle = currentTime - fract ((currentTime - startTime) / interval) * interval.
            let offset = if t.interval != 0.0 {
                (now - t.start) / t.interval
            } else {
                0.0
            };
            t.start = now - (offset % 1.0) * t.interval;
            t.lazy_start = false;
        }

        let t = t.clone();
        let elapsed = now - t.start;
        let done = elapsed >= t.interval;
        let f = if done {
            1.0
        } else if t.interval > 0.0 {
            ((elapsed / t.interval) % 1.0).max(0.0)
        } else {
            1.0
        };
        let w = ease(f, t.ease_out, t.ease_in);

        self.position_offset = lerp3(&t.position.0, &t.position.1, w);
        self.orientation_offset = slerp(&t.orientation.0, &t.orientation.1, w);
        self.scale_offset = lerp3(&t.scale.0, &t.scale.1, w);
        self.scale_orientation_offset = slerp(&t.scale_orientation.0, &t.scale_orientation.1, w);
        self.field_of_view_scale = t.fov_scale.0 + (t.fov_scale.1 - t.fov_scale.0) * w;

        if let Some((a, b)) = t.fov_offset {
            for i in 0..4 {
                self.field_of_view_offset[i] = a[i] + (b[i] - a[i]) * w;
            }
        }

        if done {
            self.transition = None;
            return t.notify.then_some(t.nav_info);
        }

        None
    }

    /// X3DViewpointNode.transitionStop (the timeSensor stops; set_active__ is removed, so no
    /// transitionComplete is ever sent for the interrupted transition).
    pub fn transition_stop(&mut self) {
        self.transition = None;
    }
}

/// What the viewpoint a transition starts from contributes (`getRelativeTransformation`: its user
/// field of view / user extents when it has the same constructor).
#[derive(Clone, Debug)]
pub struct FromState {
    pub same: bool,
    pub user_fov: f64,
    pub user_extent: [f64; 4],
    pub fov_scale: f64,
}

struct Relative {
    position: Vector3,
    orientation: Rotation4,
    scale: Vector3,
    scale_orientation: Rotation4,
}

/// X3DViewpointNode.getMaxFarValue (no logarithmic depth buffer).
/// X3DViewpointNode.getMaxFarValue (1e5, or 1e10 with the LogarithmicDepthBuffer property).
pub fn max_far() -> f64 {
    x3d_render::traverse::far_max()
}

/// The layer: four bindable stacks and lists, the viewpoint states.
pub struct Layer {
    pub nav_stack: Vec<NodeId>,
    pub vp_stack: Vec<NodeId>,
    pub bg_stack: Vec<NodeId>,
    pub fog_stack: Vec<NodeId>,
    nav_list: Vec<NodeId>,
    vp_list: Vec<NodeId>,
    bg_list: Vec<NodeId>,
    fog_list: Vec<NodeId>,
    nav_mod: u64,
    vp_mod: u64,
    bg_mod: u64,
    fog_mod: u64,
    pub vps: HashMap<NodeId, Vp>,
    /// NavigationInfo ._transitionActive.
    pub transition_active: HashMap<NodeId, bool>,
    /// X3DViewpointNode.descriptions, from the last CAMERA traversal that reached the viewpoint.
    vp_descriptions: HashMap<NodeId, Vec<String>>,
    pub bound_changed: bool,
    /// Viewport size in pixels (getLookAtDistance).
    pub viewport: (f64, f64),
    /// The ParticleSystems among the collision shapes of the last COLLISION traversal (renderObject
    /// .collisionShapes); `None` before the first. getClosestObject and constrainTranslation between traversals draw
    /// those systems' particles as they are now.
    pub collision_particle_systems: Option<Vec<crate::walk::NodeRef>>,
}

impl Layer {
    pub fn new() -> Layer {
        Self::with_viewpoint(DEFAULT)
    }

    /// A LayoutLayer's state: its default viewpoint is an OrthoViewpoint.
    pub fn new_layout() -> Layer {
        Self::with_viewpoint(DEFAULT_ORTHO)
    }

    fn with_viewpoint(default: NodeId) -> Layer {
        let mut vps = HashMap::new();
        vps.insert(default, Vp::default());

        Layer {
            nav_stack: vec![DEFAULT],
            vp_stack: vec![default],
            bg_stack: vec![DEFAULT],
            fog_stack: vec![DEFAULT],
            nav_list: vec![DEFAULT],
            vp_list: vec![default],
            bg_list: vec![DEFAULT],
            fog_list: vec![DEFAULT],
            nav_mod: 0,
            vp_mod: 0,
            bg_mod: 0,
            fog_mod: 0,
            vps,
            transition_active: HashMap::new(),
            vp_descriptions: HashMap::new(),
            bound_changed: false,
            viewport: (512.0, 512.0),
            collision_particle_systems: None,
        }
    }

    pub fn viewpoint(&self) -> NodeId {
        *self.vp_stack.last().unwrap()
    }

    pub fn navigation_info(&self) -> NodeId {
        *self.nav_stack.last().unwrap()
    }

    pub fn background(&self) -> NodeId {
        *self.bg_stack.last().unwrap()
    }

    pub fn fog(&self) -> NodeId {
        *self.fog_stack.last().unwrap()
    }

    /// Nodes the runtime released (their ids may belong to other nodes soon): their saved state goes.
    /// The stacks and lists drop them in the next scan, which no longer finds them.
    pub fn forget(&mut self, released: &[NodeId]) {
        self.vps
            .retain(|n, _| is_default(*n) || !released.contains(n));
        self.transition_active.retain(|n, _| !released.contains(n));
        self.vp_descriptions.retain(|n, _| !released.contains(n));
    }

    pub fn vp(&mut self, n: NodeId) -> &mut Vp {
        self.vps.entry(n).or_default()
    }

    pub fn viewpoint_list(&self) -> &[NodeId] {
        &self.vp_list
    }

    /// X3DViewpointNode.getDescriptions: the descriptions of the enclosing ViewpointGroup nodes
    /// and the viewpoint's own, empty when it has none or a group is not displayed.
    pub fn descriptions(&self, n: NodeId) -> &[String] {
        self.vp_descriptions.get(&n).map_or(&[], |d| d.as_slice())
    }

    /// BindableList.getBound: the bindables of Inline scenes are in the lists but are not
    /// bound here (`enableInlineBindables = false`).
    fn get_bound(rt: &Runtime, list: &[NodeId], name: Option<&str>) -> NodeId {
        if list.len() == 1 {
            return list[0];
        }

        let sg = x3d_render::sg::Sg::new(rt);
        let own: Vec<NodeId> = list[1..]
            .iter()
            .copied()
            .filter(|n| !sg.in_inline(*n))
            .collect();

        if let Some(name) = name {
            for &n in &own {
                if rt.scene.node(n).name == name {
                    return n;
                }
            }
        }

        for &n in &own {
            if rt.get_bool(n, "isBound") {
                return n;
            }
        }

        own.first().copied().unwrap_or(list[0])
    }

    fn set_bound(rt: &mut Runtime, n: NodeId, bound: bool, bind_time: Option<f64>) {
        if is_default(n) {
            return;
        }

        if let Some(k) = rt.pre(n, "isBound") {
            rt.set_value(k, FieldValue::SFBool(bound));
        }

        if let Some(t) = bind_time {
            if let Some(k) = rt.pre(n, "bindTime") {
                rt.set_value(k, FieldValue::SFTime(t));
            }
        }
    }

    /// BindableStack.pushOnTop.
    fn push_on_top(stack: &mut Vec<NodeId>, rt: &mut Runtime, node: NodeId) {
        if node != stack[0] {
            let top = *stack.last().unwrap();
            Self::set_bound(rt, top, false, None);
            stack.push(node);
        }

        let now = rt.now;
        Self::set_bound(rt, node, true, Some(now));
    }

    /// BindableStack.update. Returns (from, top, transition) when the viewpoint stack needs
    /// `top .transitionStart (layerNode, fromNode)`.
    fn stack_update(
        stack: &mut Vec<NodeId>,
        rt: &mut Runtime,
        removed: &[NodeId],
        changed: &[NodeId],
    ) -> Option<(NodeId, NodeId)> {
        if removed.is_empty() && changed.is_empty() {
            return None;
        }

        let from = *stack.last().unwrap();
        let bound = from;

        for r in removed {
            if let Some(i) = stack.iter().position(|n| n == r) {
                if i > 0 {
                    stack.remove(i);
                }
            }
        }

        let set_bind = |rt: &Runtime, n: NodeId| rt.get_bool(n, "set_bind");

        if bound != stack[0] && changed.iter().any(|n| !set_bind(rt, *n) && *n == bound) {
            stack.pop();
        }

        for b in changed {
            if !set_bind(rt, *b) {
                continue;
            }

            if let Some(i) = stack.iter().position(|n| n == b) {
                stack.remove(i);
            }

            stack.push(*b);
        }

        let top = *stack.last().unwrap();

        if top != bound {
            Self::set_bound(rt, bound, false, None);
            let now = rt.now;
            Self::set_bound(rt, top, true, Some(now));
            Some((from, top))
        } else if changed.contains(&top) {
            Some((from, top))
        } else {
            None
        }
    }

    fn lists_update(
        &mut self,
        rt: &Runtime,
        scan: &crate::walk::Scan,
    ) -> [(Vec<NodeId>, Vec<NodeId>); 4] {
        for (n, m) in &scan.viewpoints {
            self.vp(n.node).model = *m;
        }

        for (n, d) in &scan.viewpoint_descriptions {
            self.vp_descriptions.insert(*n, d.clone());
        }

        let mut out: [(Vec<NodeId>, Vec<NodeId>); 4] = Default::default();

        for which in 0..4usize {
            let collected = Self::collected(scan, which, self.vp_stack[0]);
            let (list, modc) = match which {
                0 => (&mut self.nav_list, &mut self.nav_mod),
                1 => (&mut self.vp_list, &mut self.vp_mod),
                2 => (&mut self.bg_list, &mut self.bg_mod),
                _ => (&mut self.fog_list, &mut self.fog_mod),
            };

            let changed: Vec<NodeId> = collected
                .iter()
                .copied()
                .filter(|n| {
                    !is_default(*n) && rt.bind_modifications.get(n).copied().unwrap_or(0) > *modc
                })
                .collect();
            let removed: Vec<NodeId> = if *list != collected {
                list.iter()
                    .copied()
                    .filter(|n| !collected.contains(n))
                    .collect()
            } else {
                vec![]
            };

            *list = collected;
            *modc = rt.bind_counter;
            out[which] = (removed, changed);
        }

        out
    }

    fn collected(scan: &crate::walk::Scan, which: usize, default_viewpoint: NodeId) -> Vec<NodeId> {
        let mut v = vec![if which == 1 {
            default_viewpoint
        } else {
            DEFAULT
        }];

        match which {
            0 => v.extend(scan.navigation_infos.iter().map(|n| n.node)),
            1 => v.extend(scan.viewpoints.iter().map(|(n, _)| n.node)),
            2 => v.extend(scan.backgrounds.iter().map(|(n, _)| n.node)),
            _ => v.extend(scan.fogs.iter().map(|n| n.node)),
        }

        v
    }

    /// X3DLayerNode.bindBindables: the first CAMERA pass binds the first bindables.
    pub fn bind_bindables(
        &mut self,
        rt: &mut Runtime,
        scan: &crate::walk::Scan,
        viewpoint_name: Option<&str>,
        bbox: &dyn Fn() -> Option<crate::lookat::Box3>,
    ) {
        let _ = self.lists_update(rt, scan);

        let nav = Self::get_bound(rt, &self.nav_list, None);
        let bg = Self::get_bound(rt, &self.bg_list, None);
        let fog = Self::get_bound(rt, &self.fog_list, None);
        let vp = Self::get_bound(rt, &self.vp_list, viewpoint_name);

        Self::push_on_top(&mut self.nav_stack, rt, nav);
        Self::push_on_top(&mut self.bg_stack, rt, bg);
        Self::push_on_top(&mut self.fog_stack, rt, fog);
        Self::push_on_top(&mut self.vp_stack, rt, vp);

        let base = VpBase::read(rt, vp);
        self.vp(vp).reset_user_offsets(&base);

        if base.view_all {
            self.view_all(rt, vp, &base, bbox());
        }

        self.vp(vp).update(&base);
    }

    /// X3DLayerNode.camera after the traversal: lists, stacks, viewpoint transitions,
    /// `getViewpoint () .update ()`.
    pub fn camera(
        &mut self,
        rt: &mut Runtime,
        scan: &crate::walk::Scan,
        bbox: &dyn Fn() -> Option<crate::lookat::Box3>,
    ) {
        let [nav, vp, bg, fog] = self.lists_update(rt, scan);

        Self::stack_update(&mut self.nav_stack, rt, &nav.0, &nav.1);
        let vp_change = Self::stack_update(&mut self.vp_stack, rt, &vp.0, &vp.1);
        Self::stack_update(&mut self.bg_stack, rt, &bg.0, &bg.1);
        Self::stack_update(&mut self.fog_stack, rt, &fog.0, &fog.1);

        // Bound NavigationInfo's own transitions are no-ops; a viewpoint starts a transition.
        // NavigationInfo.set_isBound__: unbinding ends an active transition.
        for n in self.nav_list.clone() {
            if n != DEFAULT
                && !rt.get_bool(n, "isBound")
                && self.transition_active.get(&n).copied().unwrap_or(false)
            {
                self.transition_active.insert(n, false);
            }
        }

        // X3DViewpointNode.set_bound__: an unbound viewpoint stops its transition TimeSensor
        // (`stopTime = now`), so binding it again starts a new transition (checkTransition).
        for (n, vp) in self.vps.iter_mut() {
            if !is_default(*n) && vp.transition.is_some() && !rt.get_bool(*n, "isBound") {
                vp.transition_stop();
            }
        }

        if let Some((from, top)) = vp_change {
            self.transition_start(rt, top, from, bbox);
        }

        let cur = self.viewpoint();
        let base = VpBase::read(rt, cur);
        self.vp(cur).update(&base);
    }

    fn nav_complete(&mut self, rt: &mut Runtime, nav: NodeId) {
        if nav != DEFAULT {
            if let Some(k) = rt.pre(nav, "transitionComplete") {
                rt.set_value(k, FieldValue::SFBool(true));
            }
        }

        self.transition_active.insert(nav, false);
    }

    pub(crate) fn nav_start(&mut self, rt: &mut Runtime, nav: NodeId) {
        if nav != DEFAULT {
            if let Some(k) = rt.private_field(nav, "transitionStart") {
                rt.set_value(k, FieldValue::SFBool(true));
            }
        }

        self.transition_active.insert(nav, true);
    }

    /// `getRelativeTransformation`'s field of view part for a transition from `from` to a viewpoint with `base`.
    fn from_state(&mut self, from: NodeId, from_base: &VpBase, base: &VpBase) -> FromState {
        let f = self.vp(from);

        FromState {
            same: from_base.same_class(base),
            user_fov: f.user_fov(from_base),
            user_extent: std::array::from_fn(|i| f.user_extent(from_base, i)),
            fov_scale: f.field_of_view_scale,
        }
    }

    /// X3DViewpointNode.transitionStart.
    fn transition_start(
        &mut self,
        rt: &mut Runtime,
        top: NodeId,
        from: NodeId,
        bbox: &dyn Fn() -> Option<crate::lookat::Box3>,
    ) {
        let base = VpBase::read(rt, top);
        let from_base = VpBase::read(rt, from);
        let from_view = self.vp(from).view;
        let nav_node = self.navigation_info();
        let nav = NavInfo::read(rt, nav_node);
        let now = rt.now;

        if base.jump {
            if self.vp(top).transition.is_some()
                && self.vp(top).transition_description == "transitionStart"
            {
                return;
            }

            self.vp(top).transition_description = "transitionStart".into();

            let from_state = self.from_state(from, &from_base, &base);
            let rel = self.vp(top).relative(&base, &from_view);

            if !base.retain_user_offsets {
                self.vp(top).reset_user_offsets(&base);
            }

            if base.view_all {
                self.view_all(rt, top, &base, bbox());
            }

            let mut kind = nav.transition_kind().to_string();

            // VRML behavior
            if rt.scene.spec_version_of(rt.scene.world).parse::<f64>().ok() == Some(2.0) {
                kind = if self.vp(top).vrml_transition {
                    "LINEAR".into()
                } else {
                    "TELEPORT".into()
                };
            }

            self.vp(top).vrml_transition = false;

            if nav.transition_time <= 0.0 {
                kind = "TELEPORT".into();
            }

            // `this .constructor !== fromViewpointNode .constructor`
            if !base.same_class(&from_base) {
                kind = "TELEPORT".into();
            }

            if kind == "TELEPORT" {
                self.nav_complete(rt, nav_node);
                return;
            }

            self.nav_start(rt, nav_node);
            let vp = self.vp(top);
            vp.start_transition(
                now,
                nav.transition_time,
                kind == "ANIMATE",
                nav_node,
                &rel,
                &from_state,
                &base,
            );
        } else {
            self.vp(top).transition_stop();
            self.nav_complete(rt, nav_node);

            let from_state = self.from_state(from, &from_base, &base);
            let rel = self.vp(top).relative(&base, &from_view);
            let vp = self.vp(top);
            vp.position_offset = rel.position;
            vp.orientation_offset = rel.orientation;
            vp.scale_offset = rel.scale;
            vp.scale_orientation_offset = rel.scale_orientation;
            vp.set_interpolators(&base, &from_state);
        }
    }

    /// Advance the bound viewpoint's transition (its private TimeSensor on timeEvents).
    pub fn tick(&mut self, rt: &mut Runtime) {
        let cur = self.viewpoint();
        let now = rt.now;

        if let Some(nav) = self.vp(cur).tick_transition(now) {
            // set_active__: when still bound and fraction is 1, the transition is complete.
            self.nav_complete(rt, nav);
        }
    }

    /// X3DViewpointNode.getLookAtDistance (Viewpoint).
    pub fn look_at_distance(
        &self,
        base: &VpBase,
        vp: &Vp,
        viewport: (f64, f64),
        bbox_size: &Vector3,
    ) -> f64 {
        if base.is_ortho {
            // OrthoViewpoint.getLookAtDistance.
            return bbox_size.norm() / 2.0 + 10.0;
        }

        let (w, h) = viewport;
        let size = if w < h {
            if w / h < bbox_size.x / bbox_size.y {
                bbox_size.x
            } else {
                bbox_size.y * w / h
            }
        } else if w / h < bbox_size.x / bbox_size.y {
            bbox_size.x * h / w
        } else {
            bbox_size.y
        };
        let size = size * VIEW_ALL_SCALE_FACTOR;
        (size / 2.0) / (vp.user_fov(base) / 2.0).tan() + bbox_size.z / 2.0
    }

    /// OrthoViewpoint.getViewAllScale (`bbox .size` of the viewpoint-local box).
    pub fn ortho_view_all_scale(&self, base: &VpBase, bbox_size: &Vector3) -> f64 {
        let (width, height) = self.viewport;
        let ratio = width / height;
        let size_x = base.ortho_fov[2] - base.ortho_fov[0];
        let size_y = base.ortho_fov[3] - base.ortho_fov[1];

        let (sx, sy) = if ratio > size_x / size_y {
            (size_y * ratio, size_y)
        } else {
            (size_x, size_x / ratio)
        };
        let scale_x = bbox_size.x / sx;
        let scale_y = bbox_size.y / sy;
        let scale = x3d_fields::jsmath::js_max(scale_x, scale_y);

        scale * VIEW_ALL_SCALE_FACTOR
    }

    /// X3DViewpointNode.viewAll with the layer's bounding box (X3DLayerNode.getBBox, a Box3) in world space.
    pub fn view_all(
        &mut self,
        rt: &Runtime,
        n: NodeId,
        base: &VpBase,
        bbox: Option<crate::lookat::Box3>,
    ) {
        let Some(bbox) = bbox else {
            let vp = self.vp(n);
            vp.near_override = None;
            vp.far_override = None;
            return;
        };
        let _ = rt;
        let size = bbox.size();

        if size.equals(&Vector3::ZERO) {
            let vp = self.vp(n);
            vp.near_override = None;
            vp.far_override = None;
            return;
        }

        let viewport = self.viewport;
        let vp = self.vp(n).clone();
        let mut inv_model = vp.model;
        inv_model.inverse();
        let center = bbox.clone().mult_right(&inv_model).center();
        let mut local = bbox.clone();
        local.mult_right(&vp.view);
        let mut direction = vp.user_position(base);
        direction.subtract(&center).normalize();
        let distance = self.look_at_distance(base, &vp, viewport, &local.size());
        let mut user_position = center;
        let mut d = direction;
        d.multiply(distance);
        user_position.add(&d);
        let user_orientation = vp.look_at_rotation(&base, &user_position, &center);

        let v = self.vp(n);
        let mut po = user_position;
        po.subtract(&base.position);
        v.position_offset = po;
        let mut inv = base.orientation;
        inv.inverse();
        inv.mult_right(&user_orientation);
        v.orientation_offset = inv;
        let mut c = center;
        c.subtract(&base.center_of_rotation);
        v.center_of_rotation_offset = c;
        v.field_of_view_scale = 1.0;
        let near = distance * (0.125 / 10.0);
        v.near_override = Some(Some(near));
        v.far_override = Some(Some(near * Vp::max_far_value(base) / 0.125));

        if base.is_ortho {
            // OrthoViewpoint.viewAll
            let scale = self.ortho_view_all_scale(base, &local.size());
            let v = self.vp(n);

            for i in 0..4 {
                let e = v.user_extent(base, i);
                v.field_of_view_offset[i] = e * scale - e;
            }
        }
    }
}

/// Matrix4.getTransform (translation, rotation, scale, scaleOrientation) without X_ITE's eigen
/// decomposition: exact for translation + rotation + non-negative axis-aligned scale (every
/// viewpoint model matrix without shear), scaleOrientation is the identity.
pub fn factor_simple(m: &Matrix4) -> (Vector3, Rotation4, Vector3, Rotation4) {
    let a = &m.0;
    let t = Vector3::new(a[12], a[13], a[14]);
    let sx = Vector3::new(a[0], a[1], a[2]).norm();
    let sy = Vector3::new(a[4], a[5], a[6]).norm();
    let sz = Vector3::new(a[8], a[9], a[10]).norm();
    let det = m.determinant();
    let sign = if det < 0.0 { -1.0 } else { 1.0 };
    let (sx, sy, sz) = (sign * sx, sign * sy, sign * sz);
    let r = x3d_fields::math::Matrix3([
        a[0] / sx,
        a[1] / sx,
        a[2] / sx,
        a[4] / sy,
        a[5] / sy,
        a[6] / sy,
        a[8] / sz,
        a[9] / sz,
        a[10] / sz,
    ]);
    let mut rot = Rotation4::identity();
    rot.set_matrix(&r);
    (t, rot, Vector3::new(sx, sy, sz), Rotation4::identity())
}
