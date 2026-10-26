//! Components/TextureProjection/X3DTextureProjectorNode.js (set_texture__, set_textureNode__) and
//! the X3DTexture2DNode width and height they read (clearTexture, setTextureData, setTexture):
//! the `aspectRatio` output. Rendering is in x3d-render's traverse (projector).

use crate::*;

/// An image whose decode ends after the pending events (see [`Runtime::apply_decoded_images`]).
#[derive(Clone, Copy, Debug)]
pub enum Decoded {
    /// ImageTexture.setImage: width, height.
    Image(u32, u32),
    /// ImageTextureAtlas.setImage: the slicing (slicesOverX, slicesOverY, numberOfSlices), then
    /// updateOutputs (width, height, depth, colorDepth) and COMPLETE_STATE.
    Atlas([i32; 3], [i32; 4]),
}

/// ImageTextureAtlas.setImage "Slice me nice": `rgba` (w × h, row 0 = top) cut into slicesOverX ×
/// slicesOverY tiles of floor (w / slicesOverX) × floor (h / slicesOverY), read from the top left
/// in rows, min (numberOfSlices, slicesOverX · slicesOverY) of them. Returns width, height, depth
/// (0 for an incomplete texture: `Math.floor (w / 0)` is Infinity, GL's 0), the tiles' texels and
/// the transparency.
///
/// The transparency is X_ITE's: each tile is read into the start of a buffer of all `depth` tiles
/// that is otherwise 0, and isImageTransparent tests the whole buffer, so more than one slice is
/// always transparent.
pub fn atlas_tiles(
    rgba: &[u8],
    w: u32,
    h: u32,
    over_x: i32,
    over_y: i32,
    number_of_slices: i32,
) -> (u32, u32, u32, Vec<u8>, bool) {
    let width = if over_x > 0 { w / over_x as u32 } else { 0 };
    let height = if over_y > 0 { h / over_y as u32 } else { 0 };
    let depth = number_of_slices
        .min(over_x.max(0).saturating_mul(over_y.max(0)))
        .max(0) as u32;
    let mut tiles = Vec::with_capacity((width * height * depth * 4) as usize);

    if width == 0 || height == 0 || depth == 0 {
        return (width, height, depth, tiles, false);
    }

    'tiles: for y in 0..over_y as u32 {
        for x in 0..over_x as u32 {
            if tiles.len() == (width * height * depth * 4) as usize {
                break 'tiles;
            }
            let (sx, sy) = (
                (x as u64 * w as u64 / over_x as u64) as u32,
                (y as u64 * h as u64 / over_y as u64) as u32,
            );
            for row in 0..height {
                let start = (((sy + row) * w + sx) * 4) as usize;
                tiles.extend_from_slice(&rgba[start..start + (width * 4) as usize]);
            }
        }
    }

    let transparent = depth > 1 || tiles.chunks_exact(4).any(|p| p[3] != 255);

    (width, height, depth, tiles, transparent)
}

/// X3DCast (X3DConstants .X3DTexture2DNode, node): the concrete node types.
pub fn is_texture_2d(name: &str) -> bool {
    matches!(
        name,
        "ImageTexture" | "MovieTexture" | "PixelTexture" | "RenderedTexture"
    )
}

impl Runtime {
    /// X3DTexture2DNode.getWidth / getHeight: (1, 1) until texture data is set (clearTexture).
    pub fn texture_2d_size(&self, node: NodeId) -> (u32, u32) {
        self.nrt(node)
            .and_then(|n| n.texture_size)
            .unwrap_or((1, 1))
    }

    /// X3DTexture2DNode.setTextureData / setTexture with setWidth and setHeight: the new size,
    /// then addNodeEvent. clearTexture is `set_texture_2d_size (node, 1, 1)`.
    pub(crate) fn set_texture_2d_size(&mut self, node: NodeId, width: u32, height: u32) {
        self.nrt_mut(node).texture_size = Some((width, height));
        self.add_node_event(node);
    }

    /// PixelTexture.set_image__: the image size, or clearTexture for an empty or invalid image.
    pub(crate) fn pixel_texture_set_image(&mut self, node: NodeId) {
        let (width, height) = match self.pre_value(node, "image") {
            Some(FieldValue::SFImage(i))
                if i.width > 0 && i.height > 0 && i.comp > 0 && i.comp <= 4 =>
            {
                (i.width as u32, i.height as u32)
            }
            _ => (1, 1),
        };

        self.set_texture_2d_size(node, width, height);
    }

    /// ImageTexture.loadData → setImage: the size of the fetched image (its header; the pixels are
    /// decoded by the renderer). Data that is not a readable image keeps the current size.
    ///
    /// The image decode (`await this .loadImage (objectURL)`) ends after the events that were
    /// pending when the fetch ended, so the size waits for [`Runtime::apply_decoded_images`].
    /// The load state is still set when the fetch ends, before the size (X_ITE sets the size first).
    ///
    /// Data in no image format fails the candidate, like the `<img>` of `loadImage` (`onerror`). X_ITE names the
    /// blob URL of the data; the engine has none and names the file URL. SVG is no format of the image crate but
    /// the browser's `<img>` decodes it, so it does not fail.
    pub(crate) fn image_texture_set_image(
        &mut self,
        node: NodeId,
        data: &[u8],
        url: &url::Url,
    ) -> Result<(), String> {
        let reader = image::ImageReader::new(std::io::Cursor::new(data)).with_guessed_format();

        if let Ok(r) = &reader
            && r.format().is_none()
            && !looks_like_svg(data)
        {
            return Err(format!("Error: Couldn't load image '{url}': error."));
        }

        if let Ok((width, height)) = reader
            .map_err(image::ImageError::IoError)
            .and_then(|r| r.into_dimensions())
        {
            self.decoded_images.retain(|(n, ..)| *n != node);
            self.decoded_images
                .push((node, Decoded::Image(width, height)));
        }

        Ok(())
    }

    /// ImageTextureAtlas.loadData: an image that decodes is cut into its tiles (for the outputs
    /// and the transparency); data that does not decode fails the candidate (`loadImage` rejects).
    pub(crate) fn image_texture_atlas_set_image(
        &mut self,
        node: NodeId,
        data: &[u8],
    ) -> Result<(), String> {
        let image = image::load_from_memory(data)
            .map_err(|e| format!("Error: {e}"))?
            .to_rgba8();
        let (w, h) = image.dimensions();
        let (over_x, over_y, slices) = (
            self.get_f64(node, "slicesOverX") as i32,
            self.get_f64(node, "slicesOverY") as i32,
            self.get_f64(node, "numberOfSlices") as i32,
        );
        let (width, height, depth, _, transparent) =
            atlas_tiles(image.as_raw(), w, h, over_x, over_y, slices);

        self.decoded_images.retain(|(n, ..)| *n != node);
        self.decoded_images.push((
            node,
            Decoded::Atlas(
                [over_x, over_y, slices],
                [
                    width as i32,
                    height as i32,
                    depth as i32,
                    if transparent { 4 } else { 3 },
                ],
            ),
        ));
        Ok(())
    }

    /// The slicing of the image an ImageTextureAtlas shows (`None`: the cleared texture).
    pub fn atlas_slicing(&self, node: NodeId) -> Option<[i32; 3]> {
        self.nrt(node).and_then(|n| n.atlas)
    }

    /// ImageTextureAtlas clearTexture with updateOutputs (0, 0, 0, 0): unloadData, a failed load.
    pub(crate) fn image_texture_atlas_clear(&mut self, node: NodeId) {
        self.decoded_images.retain(|(n, ..)| *n != node);
        self.nrt_mut(node).atlas = None;
        self.image_texture_atlas_outputs(node, [0; 4]);
    }

    /// ImageTextureAtlas.updateOutputs.
    pub(crate) fn image_texture_atlas_outputs(&mut self, node: NodeId, outputs: [i32; 4]) {
        for (name, value) in ["width", "height", "depth", "colorDepth"]
            .into_iter()
            .zip(outputs)
        {
            if let Some(k) = self.pre(node, name) {
                self.set_value(k, FieldValue::SFInt32(value));
            }
        }
    }

    /// The decoded images (after the frame's prepareEvents): an ImageTexture's setTextureData with
    /// its size; an ImageTextureAtlas's outputs and COMPLETE_STATE.
    pub(crate) fn apply_decoded_images(&mut self) {
        for (node, decoded) in std::mem::take(&mut self.decoded_images) {
            match decoded {
                Decoded::Image(width, height) => self.set_texture_2d_size(node, width, height),
                Decoded::Atlas(slicing, outputs) => {
                    self.nrt_mut(node).atlas = Some(slicing);
                    self.image_texture_atlas_outputs(node, outputs);
                    self.set_load_state(node, nodes::LoadState::Complete);
                }
            }
        }
    }

    /// X3DTextureProjectorNode.initialize: `_texture .addInterest ("set_texture__")`, set_texture__.
    pub(crate) fn texture_projector_initialize(&mut self, node: NodeId) {
        self.interest(node, "texture", Cb::ProjectorSetTexture);
        self.texture_projector_set_texture(node);
    }

    /// set_texture__: move the node interest to the new X3DTexture2DNode, then set_textureNode__.
    pub(crate) fn texture_projector_set_texture(&mut self, node: NodeId) {
        let old = match &self.nrt(node).map(|n| &n.state) {
            Some(nodes::State::Projector { texture }) => *texture,
            _ => None,
        };

        if let Some(old) = old {
            self.remove_node_interest(old, node, Cb::ProjectorSetTextureNode);
        }

        let texture = match self.pre_value(node, "texture") {
            Some(FieldValue::SFNode(Some(t))) => self
                .inner_node(*t)
                .filter(|&t| self.builtin_name(t).is_some_and(is_texture_2d)),
            _ => None,
        };

        self.nrt_mut(node).state = nodes::State::Projector { texture };

        if let Some(texture) = texture {
            self.add_node_interest(texture, node, Cb::ProjectorSetTextureNode);
        }

        self.texture_projector_set_texture_node(node);
    }

    /// set_textureNode__: `this ._aspectRatio = width / height` (an event on every call).
    pub(crate) fn texture_projector_set_texture_node(&mut self, node: NodeId) {
        let texture = match &self.nrt(node).map(|n| &n.state) {
            Some(nodes::State::Projector { texture }) => *texture,
            _ => None,
        };
        let aspect_ratio = texture.map_or(1.0, |t| {
            let (width, height) = self.texture_2d_size(t);
            width as f64 / height as f64
        });

        if let Some(k) = self.pre(node, "aspectRatio") {
            self.set_value(k, FieldValue::SFFloat(aspect_ratio));
        }
    }

    /// X3DTexture2DNode.checkLoadState () == COMPLETE_STATE: an ImageTexture or MovieTexture by its
    /// load state (an ImageTexture whose image is still decoding is not yet complete: X_ITE sets
    /// the size and COMPLETE_STATE together), a PixelTexture once set_image__ accepted its image,
    /// a RenderedTexture always.
    pub fn texture_2d_complete(&self, node: NodeId) -> bool {
        match self.builtin_name(node) {
            Some("ImageTexture") => {
                self.url_load_state(node) == nodes::LoadState::Complete
                    && !self.decoded_images.iter().any(|(n, _)| *n == node)
            }
            Some("MovieTexture") => self.url_load_state(node) == nodes::LoadState::Complete,
            Some("PixelTexture") => {
                self.nrt(node).is_some_and(|n| n.initialized)
                    && match self.pre_value(node, "image") {
                        Some(FieldValue::SFImage(i)) => {
                            i.width >= 0 && i.height >= 0 && (0..=4).contains(&i.comp)
                        }
                        _ => true,
                    }
            }
            Some("RenderedTexture") => true,
            _ => false,
        }
    }

    /// ComposedTexture3D.initialize.
    pub(crate) fn composed_3d_initialize(&mut self, node: NodeId) {
        self.interest(node, "texture", Cb::Composed3DSetTexture);

        if let Some(k) = self.private_field(node, "update") {
            self.add_interest(k, node, Cb::Composed3DUpdate);
        }

        self.composed_3d_set_texture(node);
    }

    /// ComposedTexture3D.textureNodes.
    pub fn composed_3d_slices(&self, node: NodeId) -> Vec<NodeId> {
        match self.nrt(node).map(|n| &n.state) {
            Some(nodes::State::Composed3D(v)) => v.clone(),
            _ => vec![],
        }
    }

    /// set_texture__: the X3DTexture2DNodes of `texture`, each with `addInterest ("addEvent",
    /// this ._update)`, then `_update .addEvent ()`.
    pub(crate) fn composed_3d_set_texture(&mut self, node: NodeId) {
        for old in self.composed_3d_slices(node) {
            self.remove_node_interest(old, node, Cb::Composed3DSliceEvent);
        }

        let slices: Vec<NodeId> = match self.pre_value(node, "texture") {
            Some(FieldValue::MFNode(v)) => v
                .clone()
                .into_iter()
                .flatten()
                .filter_map(|t| self.inner_node(t))
                .filter(|&t| self.builtin_name(t).is_some_and(is_texture_2d))
                .collect(),
            _ => vec![],
        };

        for &t in &slices {
            self.add_node_interest(t, node, Cb::Composed3DSliceEvent);
        }

        self.nrt_mut(node).state = nodes::State::Composed3D(slices);

        if let Some(k) = self.private_field(node, "update") {
            self.add_event(k);
        }
    }

    /// update: with every slice COMPLETE, a slice whose size is not the first's warns (the
    /// texture itself is built by the renderer).
    pub(crate) fn composed_3d_update(&mut self, node: NodeId) {
        let slices = self.composed_3d_slices(node);

        if slices.is_empty() || !slices.iter().all(|&t| self.texture_2d_complete(t)) {
            return;
        }

        let size = self.texture_2d_size(slices[0]);

        for &t in &slices {
            if self.texture_2d_size(t) != size {
                self.log_warn("ComposedTexture3D: all textures must have same size.");
            }
        }
    }

    /// RenderedTexture.initialize: X3DTexture2DNode's clearTexture, the interests, set_dimensions__.
    pub(crate) fn rendered_texture_initialize(&mut self, node: NodeId) {
        self.set_texture_2d_size(node, 1, 1);
        self.interest(node, "singleFrame", Cb::RenderedResetUpdate);
        self.interest(node, "width", Cb::RenderedSetDimensions);
        self.interest(node, "height", Cb::RenderedSetDimensions);
        self.interest(node, "depthMap", Cb::RenderedSetDimensions);
        self.interest(node, "children", Cb::RenderedResetUpdate);
        self.rendered_texture_set_dimensions(node);
    }

    pub(crate) fn rendered_texture_reset_update(&mut self, node: NodeId) {
        if let nodes::State::Rendered { check_update, .. } = &mut self.nrt_mut(node).state {
            *check_update = false;
        }
    }

    /// set_dimensions__: a framebuffer of width × height (setTextureData) or none (clearTexture).
    pub(crate) fn rendered_texture_set_dimensions(&mut self, node: NodeId) {
        let (width, height) = (
            self.get_f64(node, "width") as i32,
            self.get_f64(node, "height") as i32,
        );

        if width > 0 && height > 0 {
            self.set_texture_2d_size(node, width as u32, height as u32);
        } else {
            self.set_texture_2d_size(node, 1, 1);
        }

        self.rendered_texture_reset_update(node);
    }

    /// RenderedTexture.traverse (DISPLAY, independent render object): whether renderTexture runs
    /// this frame. A framebuffer (width and height > 0) is needed; after a render, singleFrame
    /// TRUE waits for a field change and FALSE for updateInterval seconds (`Date .now ()`).
    pub fn rendered_texture_traverse(&mut self, node: NodeId, now_ms: f64) -> bool {
        let (width, height) = (self.get_f64(node, "width"), self.get_f64(node, "height"));
        let single_frame = self.get_bool(node, "singleFrame");
        let interval_ms = self.get_f64(node, "updateInterval") * 1000.0;
        let depth_map = self.get_bool(node, "depthMap");
        let layer_pass = self.rendered_texture_group_children(node) == 0;

        if width <= 0.0 || height <= 0.0 {
            return false;
        }

        let nodes::State::Rendered {
            check_update,
            last_update_ms,
            first_ms,
            leaks,
            display_leaks,
        } = &mut self.nrt_mut(node).state
        else {
            return false;
        };

        if *check_update && (single_frame || now_ms - *last_update_ms < interval_ms) {
            return false;
        }

        *last_update_ms = now_ms;
        *check_update = true;
        first_ms.get_or_insert(now_ms);

        // A depth map pass of the layer: X3DLayerNode.display (DEPTH) → NavigationInfo.enable
        // pushes the headlight, and render (DEPTH) never reaches draw (), which resets the global
        // lights. The next DISPLAY pass draws them as well.
        if depth_map {
            *display_leaks = 0;
            if layer_pass {
                *leaks += 1;
            }
        } else {
            *display_leaks = std::mem::take(leaks);
        }

        true
    }

    /// RenderedTexture.setChild: the number of children that go into its Group (X3DChildNodes
    /// other than Fog, NavigationInfo, X3DBackgroundNode and X3DViewpointNode).
    pub fn rendered_texture_group_children(&self, node: NodeId) -> usize {
        let children = match self.pre_value(node, "children") {
            Some(FieldValue::MFNode(v)) => v.clone(),
            _ => vec![],
        };

        children
            .into_iter()
            .flatten()
            .filter_map(|c| self.inner_node(c))
            .filter(|&c| {
                !matches!(
                    self.builtin_name(c),
                    Some(
                        "Fog"
                            | "NavigationInfo"
                            | "Background"
                            | "TextureBackground"
                            | "Viewpoint"
                            | "OrthoViewpoint"
                            | "GeoViewpoint"
                    )
                )
            })
            .count()
    }

    /// The headlights left by earlier depth map passes that this frame's pass also draws.
    pub fn rendered_texture_leaked_headlights(&self, node: NodeId) -> u32 {
        match self.nrt(node).map(|n| &n.state) {
            Some(nodes::State::Rendered { display_leaks, .. }) => *display_leaks,
            _ => 0,
        }
    }

    /// The time of a RenderedTexture's first renderTexture (`None` before).
    pub fn rendered_texture_first(&self, node: NodeId) -> Option<f64> {
        match self.nrt(node).map(|n| &n.state) {
            Some(nodes::State::Rendered { first_ms, .. }) => *first_ms,
            _ => None,
        }
    }

    /// For a still drawn after several frames without a display walk: every RenderedTexture with
    /// a framebuffer renders now and rendered before (its setTransparent applies).
    pub fn rendered_textures_settled(&mut self) {
        let now_ms = self.now * 1000.0;
        self.rendered_now.clear();

        for node in 0..self.scene.nodes.len() as NodeId {
            if self.builtin_name(node) != Some("RenderedTexture")
                || self.get_f64(node, "width") <= 0.0
                || self.get_f64(node, "height") <= 0.0
            {
                continue;
            }

            if let Some(nodes::State::Rendered {
                check_update,
                last_update_ms,
                first_ms,
                ..
            }) = self.nrt.get_mut(node as usize).map(|n| &mut n.state)
            {
                *check_update = true;
                *last_update_ms = now_ms;
                *first_ms = Some(now_ms - 1.0);
                self.rendered_now.push(node);
            }
        }
    }
}

/// An SVG document: `<svg` near the start, after an optional XML declaration, comments or a doctype.
fn looks_like_svg(data: &[u8]) -> bool {
    let head = &data[..data.len().min(4096)];

    head.windows(4).any(|w| w.eq_ignore_ascii_case(b"<svg"))
}
