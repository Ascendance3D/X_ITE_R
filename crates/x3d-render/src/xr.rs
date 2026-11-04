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
//! The pose is given by its matrices, as XRRigidTransform and XRView hand them out (16 numbers, column-major). Not done
//! (UNVERIFIED, no reference recorded): backgrounds of a layer other than the active one and projective textures.

use crate::traverse::{Frame, plane_mult_right};
use x3d_fields::math::{Matrix3, Matrix4};

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
}

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
