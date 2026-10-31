//! Viewpoint look at: X3DViewer.lookAtPoint / lookAtBBox (Browser/Navigation/X3DViewer.js),
//! X3DViewpointNode.lookAtPoint / lookAtBBox / lookAt and OrthoViewpoint.lookAtBBox / lookAt
//! (Components/Navigation), and the Box3 they use (standard/Math/Geometry/Box3.js).
//!
//! A look at is a viewpoint transition (the viewpoint's private TimeSensor, EaseInEaseOut and
//! interpolators, here a [`Transition`] ticked by `Layer::tick`). The viewers' dblclick handlers
//! (LookAtViewer, PlaneViewer, and later ExamineViewer) request it; `Viewer3d::look_at` picks.
//!
//! The interpolators' key values are `MFVec3f` / `MFRotation` / `MFFloat`: Float32Array in X_ITE,
//! so they are rounded to float32 here too.

use crate::layer::{FromState, Layer, Transition, Vp, VpBase};
use x3d_fields::NodeId;
use x3d_fields::math::{Matrix4, Rotation4, Vector3};
use x3d_runtime::Runtime;

/// Box3: a Matrix4 whose axes are the half extents and whose origin is the center.
#[derive(Clone, Debug)]
pub struct Box3 {
    pub matrix: Matrix4,
}

impl std::fmt::Display for Box3 {
    /// Box3.toString.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}, {}", self.size(), self.center())
    }
}

impl Box3 {
    /// Box3.setExtents.
    pub fn from_extents(min: &Vector3, max: &Vector3) -> Box3 {
        let (sx, sy, sz) = (
            (max.x - min.x) / 2.0,
            (max.y - min.y) / 2.0,
            (max.z - min.z) / 2.0,
        );
        let (cx, cy, cz) = (
            (max.x + min.x) / 2.0,
            (max.y + min.y) / 2.0,
            (max.z + min.z) / 2.0,
        );

        Box3 {
            matrix: Matrix4([
                sx, 0.0, 0.0, 0.0, 0.0, sy, 0.0, 0.0, 0.0, 0.0, sz, 0.0, cx, cy, cz, 1.0,
            ]),
        }
    }

    pub fn mult_right(&mut self, matrix: &Matrix4) -> &mut Self {
        self.matrix.mult_right(matrix);
        self
    }

    /// Box3.getAbsoluteExtents.
    pub fn absolute_extents(&self) -> (Vector3, Vector3) {
        let m = &self.matrix.0;
        let x = Vector3::new(m[0], m[1], m[2]);
        let y = Vector3::new(m[4], m[5], m[6]);
        let z = Vector3::new(m[8], m[9], m[10]);

        let mut r1 = y;
        r1.add(&z);

        let mut r2 = z;
        r2.subtract(&y);

        let mut p1 = x;
        p1.add(&r1);
        let mut p4 = x;
        p4.add(&r2);

        let mut p2 = r1;
        p2.subtract(&x);
        let mut p3 = r2;
        p3.subtract(&x);

        let mut min = p1;
        min.min(&p2).min(&p3).min(&p4);
        let mut max = p1;
        max.max(&p2).max(&p3).max(&p4);

        p1.negate();
        p2.negate();
        p3.negate();
        p4.negate();

        min.min(&p1).min(&p2).min(&p3).min(&p4);
        max.max(&p1).max(&p2).max(&p3).max(&p4);

        (min, max)
    }

    /// Box3.getExtents: the extents of the box in the space it is in (absolute extents + center).
    pub fn extents(&self) -> (Vector3, Vector3) {
        let (mut min, mut max) = self.absolute_extents();
        let c = self.center();
        min.add(&c);
        max.add(&c);
        (min, max)
    }

    /// Box3.size.
    pub fn size(&self) -> Vector3 {
        let (min, mut max) = self.absolute_extents();
        max.subtract(&min);
        max
    }

    /// Box3.center (the matrix origin).
    pub fn center(&self) -> Vector3 {
        Vector3::new(self.matrix.0[12], self.matrix.0[13], self.matrix.0[14])
    }
}

fn f32v(v: &Vector3) -> Vector3 {
    Vector3::new(v.x as f32 as f64, v.y as f32 as f64, v.z as f32 as f64)
}

fn f32r(r: &Rotation4) -> Rotation4 {
    let mut r = *r;
    let c = r.components();
    Rotation4::new(
        c[0] as f32 as f64,
        c[1] as f32 as f64,
        c[2] as f32 as f64,
        c[3] as f32 as f64,
    )
}

fn f32s(v: f64) -> f64 {
    v as f32 as f64
}

impl Layer {
    /// X3DViewer.lookAtPoint: `point` is the hit point in view space.
    pub fn look_at_point(
        &mut self,
        rt: &mut Runtime,
        n: NodeId,
        base: &VpBase,
        point: Vector3,
        straighten: bool,
        date_now: f64,
    ) {
        // X3DViewpointNode.lookAtPoint: camera space → world → viewpoint local space.
        let (camera_space, model) = {
            let vp = self.vp(n);
            (vp.camera_space, vp.model)
        };
        let mut p = point;
        camera_space.mult_vec_matrix3(&mut p);
        let mut inv_model = model;
        inv_model.inverse();
        inv_model.mult_vec_matrix3(&mut p);

        self.look_at(rt, n, base, &p, 0.5, 0.5, 1.0, straighten, date_now);
    }

    /// X3DViewpointNode.lookAtBBox (+ OrthoViewpoint.lookAtBBox); `bbox` is in world space: the hit shape's box of
    /// X3DViewer.lookAtBBox (`shape .getBBox () .multRight (modelView) .multRight (cameraSpace)`, factor 0.8, transitionTime 1)
    /// or the layer's of X3DLayerNode.viewAll (factor 1, the time X3DBrowser.viewAll got).
    #[allow(clippy::too_many_arguments)]
    pub fn look_at_bbox(
        &mut self,
        rt: &mut Runtime,
        n: NodeId,
        base: &VpBase,
        bbox: &Box3,
        factor: f64,
        transition_time: f64,
        straighten: bool,
        date_now: f64,
    ) {
        let (model, view, saved_offset, saved_scale) = {
            let vp = self.vp(n);
            (
                vp.model,
                vp.view,
                vp.field_of_view_offset,
                vp.field_of_view_scale,
            )
        };

        if bbox.size().equals(&Vector3::ZERO) {
            return;
        }

        let mut inv_model = model;
        inv_model.inverse();
        let mut b = bbox.clone();
        b.mult_right(&inv_model);
        let point = b.center();

        let mut local = bbox.clone();
        local.mult_right(&view);
        let vp = self.vp(n).clone();
        let distance = self.look_at_distance(base, &vp, self.viewport, &local.size());

        self.look_at(
            rt,
            n,
            base,
            &point,
            distance,
            factor,
            transition_time,
            straighten,
            date_now,
        );

        if base.is_ortho {
            // OrthoViewpoint.lookAtBBox: the interpolators go to the view-all scale of the local box.
            let scale = self.ortho_view_all_scale(base, &local.size());
            let target: [f64; 4] =
                std::array::from_fn(|i| base.ortho_fov[i] * scale - base.ortho_fov[i]);
            let t = self
                .vp(n)
                .transition
                .as_mut()
                .expect("lookAt installed its transition");
            t.fov_offset = Some((saved_offset.map(f32s), target.map(f32s)));
            t.fov_scale = (f32s(saved_scale), 1.0);
        }
    }

    /// X3DViewpointNode.straightenView: unless a straightenView transition runs, a one-second transition from the
    /// orientation offset to the one that straightens the user orientation (the other offsets stay).
    pub fn straighten_view(&mut self, rt: &mut Runtime, n: NodeId, base: &VpBase, date_now: f64) {
        let nav_node = self.navigation_info();
        let was_active = self.vp(n).transition.is_some();

        // checkTransition ("straightenView")
        if was_active && self.vp(n).transition_description == "straightenView" {
            return;
        }

        let vp = self.vp(n);

        vp.transition_description = "straightenView".into();

        let straight = base.straighten_horizon(vp, &vp.user_orientation(base));
        let mut rotation = base.orientation;
        rotation.inverse();
        rotation.mult_right(&straight);

        let mut t = Transition {
            start: date_now,
            interval: 1.0,
            ease_out: 1.0,
            ease_in: 1.0,
            position: (f32v(&vp.position_offset), f32v(&vp.position_offset)),
            orientation: (f32r(&vp.orientation_offset), f32r(&rotation)),
            scale: (f32v(&vp.scale_offset), f32v(&vp.scale_offset)),
            scale_orientation: (
                f32r(&vp.scale_orientation_offset),
                f32r(&vp.scale_orientation_offset),
            ),
            fov_scale: (1.0, 1.0),
            fov_offset: None,
            nav_info: nav_node,
            notify: !was_active,
            lazy_start: true,
        };

        // this .setInterpolators (this, this .getRelativeTransformation (this))
        let from = FromState {
            same: true,
            user_fov: vp.user_fov(base),
            user_extent: std::array::from_fn(|i| vp.user_extent(base, i)),
            fov_scale: vp.field_of_view_scale,
        };
        let (scale, offsets) = vp.set_interpolators(base, &from);
        t.fov_scale = (f32s(scale.0), f32s(scale.1));
        t.fov_offset = offsets.map(|(a, b)| (a.map(f32s), b.map(f32s)));

        vp.transition = Some(t);
        self.nav_start(rt, nav_node);
    }

    /// X3DViewpointNode.lookAt (OrthoViewpoint.lookAt calls it with `factor: 0`); `transition_time` is the TimeSensor's
    /// cycleInterval.
    #[allow(clippy::too_many_arguments)]
    pub fn look_at(
        &mut self,
        rt: &mut Runtime,
        n: NodeId,
        base: &VpBase,
        point: &Vector3,
        distance: f64,
        factor: f64,
        transition_time: f64,
        straighten_horizon: bool,
        date_now: f64,
    ) {
        let nav_node = self.navigation_info();

        // OrthoViewpoint.lookAt: remember the offsets before and compute where they go.
        let (saved_offset, saved_scale, ortho_target) = {
            let vp = self.vp(n);
            let scale = 1.0 - (0.9 * factor);
            let target: [f64; 4] =
                std::array::from_fn(|i| vp.user_extent(base, i) * scale - base.ortho_fov[i]);
            (vp.field_of_view_offset, vp.field_of_view_scale, target)
        };
        let factor = if base.is_ortho { 0.0 } else { factor };

        let was_active = self.vp(n).transition.is_some();
        let vp = self.vp(n);

        vp.transition_description = "lookAt".into();

        let user_orientation = vp.user_orientation(base);
        let mut z_distance = Vector3::new(0.0, 0.0, distance);
        user_orientation.mult_vec_rot(&mut z_distance);
        let mut offset = *point;
        offset.add(&z_distance).subtract(&base.position);

        let mut translation = vp.position_offset;
        translation.lerp(&offset, factor);

        let mut direction = base.position;
        direction.add(&translation).subtract(point);

        let mut z_axis = Vector3::new(0.0, 0.0, 1.0);
        user_orientation.mult_vec_rot(&mut z_axis);
        let mut from_vectors = Rotation4::identity();
        from_vectors.set_vectors(&z_axis, &direction);
        let mut rotation = vp.orientation_offset;
        rotation.mult_right(&from_vectors);

        if straighten_horizon {
            let mut o = base.orientation;
            o.mult_right(&rotation);
            let s = base.straighten_horizon(vp, &o);
            let mut inv = base.orientation;
            inv.inverse();
            inv.mult_right(&s);
            rotation = inv;
        }

        // layerNode .getNavigationInfo () ._transitionStart = true
        let mut t = Transition {
            start: date_now,
            interval: transition_time,
            ease_out: 1.0,
            ease_in: 1.0,
            position: (f32v(&vp.position_offset), f32v(&translation)),
            orientation: (f32r(&vp.orientation_offset), f32r(&rotation)),
            scale: (f32v(&vp.scale_offset), Vector3::ONE),
            scale_orientation: (
                f32r(&vp.scale_orientation_offset),
                f32r(&vp.scale_orientation_offset),
            ),
            fov_scale: (1.0, 1.0),
            fov_offset: None,
            nav_info: nav_node,
            // The TimeSensor's own stop event removes the set_active__ interest this lookAt just added.
            notify: !was_active,
            lazy_start: true,
        };

        let mut c = *point;
        c.subtract(&base.center_of_rotation);
        vp.center_of_rotation_offset = c;

        let near = distance * (0.125 / 10.0);
        vp.near_override = Some(Some(near));
        vp.far_override = Some(Some(near * Vp::max_far_value(base) / 0.125));

        // this .setInterpolators (this, this .getRelativeTransformation (this))
        let from = FromState {
            same: true,
            user_fov: vp.user_fov(base),
            user_extent: std::array::from_fn(|i| vp.user_extent(base, i)),
            fov_scale: vp.field_of_view_scale,
        };
        let (scale, offsets) = vp.set_interpolators(base, &from);
        t.fov_scale = (f32s(scale.0), f32s(scale.1));
        t.fov_offset = offsets.map(|(a, b)| (a.map(f32s), b.map(f32s)));

        if base.is_ortho {
            t.fov_offset = Some((saved_offset.map(f32s), ortho_target.map(f32s)));
            t.fov_scale = (f32s(saved_scale), 1.0);
        }

        vp.transition = Some(t);
        self.nav_start(rt, nav_node);
    }
}
