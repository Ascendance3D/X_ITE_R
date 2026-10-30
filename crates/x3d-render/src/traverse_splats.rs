//! GaussianSplats in the DISPLAY pass (a child module of `traverse`): GaussianSplats.traverse hands
//! its private GaussianSplatsShape to renderObject.addDisplayShape. The shape is transparent
//! (`set_transparent__`: setTransparent (true), alphaMode BLEND), so it is a transparent item sorted
//! by the view-space z of its bounding box center. See [`crate::splats`].

use super::*;
use crate::splats::{self, SplatDraw};

impl Builder<'_> {
    /// The GaussianSplatsShape.rebuild () data of `n`, kept in the cache between frames while the
    /// node's write stamp does not change.
    pub(super) fn splat_data(&mut self, sg: &Sg, n: NodeId) -> Rc<splats::SplatData> {
        let stamp = sg.rt.stamp(n);
        let key = (sg.rt.uid, n);
        let mut previous: Option<Rc<splats::SplatData>> = None;

        if let Some(c) = self.cache.as_deref_mut() {
            let frame = c.frame;

            if let Some(e) = c.splats.get_mut(&key) {
                if e.0 == stamp {
                    e.2 = frame;
                    return e.1.clone();
                }

                previous = Some(e.1.clone());
            }
        }

        let data = Rc::new(splats::build(
            &splats::fields_of(sg, n),
            previous.as_deref(),
        ));

        if let Some(c) = self.cache.as_deref_mut() {
            let frame = c.frame;
            c.built += 1;
            c.splats.insert(key, (stamp, data.clone(), frame));
        }

        data
    }

    /// X3DShapeNode.getBBox of the shape: bboxSize and bboxCenter, or the box of the positions.
    pub(super) fn splats_bbox(sg: &Sg, n: NodeId, data: &splats::SplatData) -> Box3 {
        let default = Vector3::new(-1.0, -1.0, -1.0);

        if !sg.vec3(n, "bboxSize", default).equals(&default) {
            return Box3::new(
                &sg.vec3(n, "bboxSize", Vector3::ZERO),
                &sg.vec3(n, "bboxCenter", Vector3::ZERO),
            );
        }

        match data.bbox {
            Some((size, center)) => Box3::new(
                &Vector3::new(size[0], size[1], size[2]),
                &Vector3::new(center[0], center[1], center[2]),
            ),
            None => Box3::EMPTY,
        }
    }

    pub(super) fn splats(&mut self, sg: &Sg, n: NodeId, mv: &Matrix4) {
        let data = self.splat_data(sg, n);

        // X3DShapeNode.isEnabled (): numSplats.
        if data.count == 0 {
            return;
        }

        let bbox = Self::splats_bbox(sg, n, &data);

        // X3DGroupingNode.set_objects__: the node's bbox node (bboxDisplay) is a visible object of the group.
        if sg.bool(n, "bboxDisplay", false) {
            self.bbox_shape(n, bbox, mv);
        }

        // addDisplayShape: distance = the bbox center z in view space.
        let mut center = bbox.center();
        mv.mult_vec_matrix3(&mut center);

        let linear = sg.string(n, "colorSpace", "SRGB_REC709_DISPLAY") == "LIN_REC709_DISPLAY";

        self.items.push(DrawItem {
            geometry_node: n,
            geometry: crate::volume::empty_geometry(),
            model_view: *mv,
            material: default_material(),
            textures: vec![],
            multi_texture: None,
            texture_matrices: vec![IDENTITY_F32],
            projectors: vec![],
            lights: vec![],
            transparent: true,
            alpha_mode: AlphaMode::Blend,
            alpha_cutoff: 0.0,
            back_material: None,
            blend: None,
            depth: None,
            clip_planes: self
                .clip_planes
                .iter()
                .take(MAX_CLIP_PLANES)
                .copied()
                .collect(),
            style: None,
            style_texture: None,
            distance: center.z,
            viewport: (self.viewports.len() > 1).then(|| self.viewport()),
            volume: None,
            instances: None,
            splats: Some(Rc::new(SplatDraw {
                node: n,
                data,
                model_view: *mv,
                linear,
            })),
            custom: None,
            fog: self.local_fogs.last().cloned(),
        });
    }
}
