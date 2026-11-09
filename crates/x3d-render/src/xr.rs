//! Drawing a frame once per view of an XR pose: the draw loop of X3DRenderObject.draw with a WebXR pose
//! (`browser .getPose ()`), without a session or a headset.
//!
//! For each view of the pose X_ITE draws the same display list with the view's projection matrix, into the view's
//! viewport of the layer (X3DWebXRContext.xrUpdatePose: `reshapeFramebuffer (i, x, y, width, height)`; a view of width
//! zero is left out), and with two matrices more in the vertex shaders:
//!
//! * the layer's view matrix of the active layer starts with the pose (X3DLayerNode.traverse:
//!   `getViewMatrix () .push (pose .viewMatrix)`, then `.multLeft (viewpointNode .getViewMatrix ())`), so every
//!   model-view matrix, and the normal matrix with it, is `pose .viewMatrix` after the viewpoint's;
//! * `x3d_EyeMatrix` (`view .matrix`, `pose .cameraSpaceMatrix .multRight (view .viewMatrix)`) is applied to the
//!   positions only: `tVertex = x3d_EyeMatrix * x3d_ModelViewMatrix * vertex` (X3D_XR_SESSION).
//!
//! * the clip planes of each shape are in the space of its model-view matrix (after the pose) and get the eye matrix
//!   (ClipPlaneContainer.setShaderUniforms with a view);
//! * the shadow matrix of each light with a shadow map is `view .inverse · cameraSpaceMatrix · invLightSpaceProjectionMatrix`
//!   (X3DLight setGlobalVariables), where `view .inverse` is the view's own transform and then the pose's inverse.
//!
//! * the input rays and their hit points (X3DWebXRContext.xrUpdatePointers, ScreenLine and ScreenPoint) are drawn over each
//!   view after it, see [`XrRay`] and [`pointer_draws`].
//!
//! The pose is given by its matrices, as XRRigidTransform and XRView hand them out (16 numbers, column-major). Not done
//! (UNVERIFIED, no reference recorded): backgrounds of a layer other than the active one and projective textures.

use crate::traverse::{Frame, plane_mult_right};
use x3d_fields::math::{Matrix3, Matrix4, Rotation4, Vector3, Vector4};

/// One XRView: the projection matrix, `transform .inverse .matrix` (X3DWebXRContext `view .viewMatrix`) and the
/// viewport of the layer (x, y, width, height; y up, in pixels).
#[derive(Clone, Debug)]
pub struct XrView {
    pub projection: Matrix4,
    pub view_matrix: Matrix4,
    pub viewport: [u32; 4],
}

/// An XRViewerPose: its `transform .matrix` (X3DWebXRContext `pose .cameraSpaceMatrix`), its
/// `transform .inverse .matrix` (`pose .viewMatrix`) and its views.
#[derive(Clone, Debug)]
pub struct XrPose {
    pub camera_space: Matrix4,
    pub view_matrix: Matrix4,
    pub views: Vec<XrView>,
    /// The bound NavigationInfo's headlight. The frame's first light is then the headlight, which the pose does not
    /// move (it is placed by the viewpoint, X3DRenderObject.draw `browser .getHeadlight () .setGlobalVariables`).
    pub headlight: bool,
    /// The active input sources (`inputSource .active`): their rays are drawn over each view. ContentScale is the rendering
    /// property of that name.
    pub rays: Vec<XrRay>,
    pub content_scale: f64,
}

/// One input source for the ray drawing, as X3DWebXRContext keeps it: the target-ray pose and the pick of its last
/// `touch`.
#[derive(Clone, Debug)]
pub struct XrRay {
    /// `inputSource .matrix`, the target-ray pose's `transform .matrix`.
    pub matrix: Matrix4,
    /// `gamepad ?.buttons .some (button => button .pressed)`: the ray and the hit point are blue and the point smaller.
    pub pressed: bool,
    /// `hit .id`: the ray hit a shape, with `hit .originalPoint` and `hit .originalNormal` (in the space of the ray).
    pub hit: Option<(Vector3, Vector3)>,
}

/// What X3DWebXRContext.xrUpdatePointers draws for one input source in one view: the ray (`ScreenLine`, in pixels of the
/// view) and, with a hit, the hit point (`ScreenPoint`).
#[derive(Clone, Debug)]
pub struct XrPointerDraw {
    pub line: crate::rubber_band::RubberBand,
    /// The color of the white strip of the line.
    pub color: [f32; 3],
    pub point: Option<XrPointDraw>,
}

/// `ScreenPoint .display (radius, color, 0.3, 0.8, modelViewMatrix, projectionMatrix)` after its screen scale: the model-view
/// matrix with its axes scaled to `radius` times the distance, and the view's projection matrix.
#[derive(Clone, Debug)]
pub struct XrPointDraw {
    pub model_view: Matrix4,
    pub projection: Matrix4,
    pub color: [f32; 3],
}

/// The ring is drawn black at transparency 0.8 and the disk in the ray's color at transparency 0.3
/// (`display (radius, color, 0.3, 0.8, ...)`).
pub const POINT_DISK_TRANSPARENCY: f64 = 0.3;
pub const POINT_RING_TRANSPARENCY: f64 = 0.8;

impl XrPose {
    /// The views that X_ITE draws: a view with a viewport of width zero is left out (WebXR Emulator, non-stereo).
    pub fn drawn_views(&self) -> impl Iterator<Item = &XrView> {
        self.views.iter().filter(|v| v.viewport[2] != 0)
    }

    /// `view .matrix` of X3DWebXRContext.xrUpdatePose, the value of `x3d_EyeMatrix`.
    pub fn eye_matrix(&self, view: &XrView) -> Matrix4 {
        let mut eye = self.camera_space;
        eye.mult_right(&view.view_matrix);
        eye
    }
}

/// The frame of one view and its eye matrix: the projection of the view, its viewport, and the pose in front of
/// every model-view matrix (GL: `pose .viewMatrix * modelView`, so `modelView .multRight (pose .viewMatrix)` in
/// X_ITE's row-vector matrices). The eye matrix goes to `Renderer::set_xr_eye`.
pub fn view_frame(frame: &Frame, pose: &XrPose, view: &XrView) -> (Frame, Matrix4) {
    let mut f = frame.clone();

    f.projection = view.projection;
    f.viewport = Some(view.viewport.map(f64::from));
    f.xr_pointers = pointer_draws(pose, view);

    for item in &mut f.items {
        item.model_view.mult_right(&pose.view_matrix);
    }

    // The clip planes are in the space of the model-view matrix, so after the pose, and the eye matrix is then put on
    // each plane (ClipPlaneContainer.setShaderUniforms with a view: `planeView .assign (plane) .multRight (view .matrix)`),
    // since the shader tests the position after `x3d_EyeMatrix`.
    let eye = pose.eye_matrix(view);

    for item in &mut f.items {
        for plane in &mut item.clip_planes {
            *plane = plane_mult_right(plane_mult_right(*plane, &pose.view_matrix), &eye);
        }
    }

    // The lights' model-view matrices start with the pose as well (PointLight.setGlobalVariables:
    // `modelViewMatrix .multVecMatrix (location)`, and the inverse of its 3 x 3 for the cone and the attenuation).
    let mut pose_inverse = pose.view_matrix.submatrix();
    pose_inverse.inverse();

    for light in f.lights.iter_mut().skip(usize::from(pose.headlight)) {
        pose.view_matrix.mult_dir_matrix(&mut light.direction);
        light.direction.normalize();

        if light.kind != 1 {
            pose.view_matrix.mult_vec_matrix3(&mut light.location);

            let mut m = Matrix3(light.matrix.map(f64::from));

            m.mult_left(&pose_inverse);
            light.matrix = m.0.map(|v| v as f32);
        }
    }

    // The shadow matrix of every light with a shadow map (X3DLight setGlobalVariables, in all three light nodes):
    // `view .inverse .multRight (cameraSpaceMatrixArray) .multRight (invLightSpaceProjectionMatrix)`. The frame's
    // matrix is `cameraSpaceMatrix · invLightSpaceProjectionMatrix` of the viewpoint; under a pose the layer's camera
    // space matrix starts with `pose .cameraSpaceMatrix` and `view .inverse` is
    // `view .cameraSpaceMatrix .multRight (pose .viewMatrix)`. The map itself is drawn once, from the light.
    let mut view_inverse = view.view_matrix;

    view_inverse.inverse();
    view_inverse.mult_right(&pose.view_matrix);

    for light in &mut f.lights {
        if let Some(shadow) = &mut light.shadow {
            let mut m = view_inverse;

            m.mult_right(&pose.camera_space);
            m.mult_right(&shadow.matrix);

            shadow.matrix = m;
        }
    }

    (f, eye)
}

/// The input rays of one view, in the order of the input sources (X3DWebXRContext.xrUpdatePointers, "Draw input source
/// rays"). The ray runs from the origin of its space to 0.5 m ahead (-z), or to the hit if that is nearer; a ray with an
/// end behind the view is left out with its hit point. The line is white, blue when a button is pressed (`blue = 0.5,
/// 0.75, 1`), 4 px wide at the start and 2 at the end; the hit point is a ring and a disk of radius 0.007 m, 0.005 m when
/// pressed, in the plane of the hit, scaled by the distance so that its size on the screen does not depend on it. There is
/// no depth test, and nothing is drawn for a source that has no pose (`active`). Both are drawn into the framebuffer of the
/// view, whose own viewport `[0, 0, width, height]` projects them.
pub fn pointer_draws(pose: &XrPose, view: &XrView) -> Vec<XrPointerDraw> {
    let [vx, vy, vw, vh] = view.viewport;
    let viewport = [0.0, 0.0, f64::from(vw), f64::from(vh)];
    let mut draws = vec![];

    for ray in &pose.rays {
        let color = if ray.pressed {
            [0.5, 0.75, 1.0]
        } else {
            [1.0, 1.0, 1.0]
        };

        let mut m = ray.matrix;
        m.mult_right(&view.view_matrix);

        let mut from = Vector3::ZERO;
        let mut to = Vector3::new(0.0, 0.0, -0.5);
        let mut hit_point = ray.hit.map_or(Vector3::ZERO, |h| h.0);

        m.mult_vec_matrix3(&mut from);
        m.mult_vec_matrix3(&mut to);
        m.mult_vec_matrix3(&mut hit_point);

        // Make ray shorter if track point is very close.
        if ray.hit.is_some() && hit_point.distance(&from) < to.distance(&from) {
            to = hit_point;
        }

        if from.z > 0.0 || to.z > 0.0 {
            continue;
        }

        let project = |p: &Vector3| {
            // ViewVolume.projectPointMatrix, then `z = 0`.
            let mut v = Vector4::new(p.x, p.y, p.z, 1.0);

            view.projection.mult_vec_matrix4(&mut v);

            let d = 1.0 / (2.0 * v.w);

            [
                (v.x * d + 0.5) * viewport[2] + viewport[0],
                (v.y * d + 0.5) * viewport[3] + viewport[1],
            ]
        };

        let line = crate::rubber_band::RubberBand {
            from: project(&from),
            to: project(&to),
            content_scale: pose.content_scale,
            viewport: [f64::from(vx), f64::from(vy), f64::from(vw), f64::from(vh)],
        };

        let point = ray.hit.map(|(point, normal)| {
            let radius = if ray.pressed { 0.005 } else { 0.007 };
            let mut m = ray.matrix;

            m.mult_right(&view.view_matrix);
            m.translate(&point);

            let mut rotation = Rotation4::identity();

            rotation.set_vectors(&Vector3::Z_AXIS, &normal);
            m.rotate(&rotation);

            // ScreenPoint.display: "Apply screen scale to matrix."
            let scale = m.0[14].abs() * radius;
            let axis = |i: usize| {
                let mut a = Vector3::new(m.0[i], m.0[i + 1], m.0[i + 2]);

                a.normalize();
                [a.x * scale, a.y * scale, a.z * scale]
            };
            let (x, y, z) = (axis(0), axis(4), axis(8));
            let model_view = Matrix4([
                x[0], x[1], x[2], 0.0, y[0], y[1], y[2], 0.0, z[0], z[1], z[2], 0.0, m.0[12],
                m.0[13], m.0[14], 1.0,
            ]);

            XrPointDraw {
                model_view,
                projection: view.projection,
                color,
            }
        });

        draws.push(XrPointerDraw { line, color, point });
    }

    draws
}
