//! Layering and Layout (Components/Layering, Components/Layout, Execution/X3DWorld.js): which
//! layers a world draws and in what order, their viewport rectangles, and the Layout matrices.
//! The renderer's frame builder and the viewer's traversals both use these.

use x3d_fields::NodeId;
use x3d_fields::math::{Matrix4, Rotation4, Vector3};

use crate::sg::Sg;

/// A layer of the world: `None` is layer 0 (X3DWorld.layer0, whose children are the root nodes),
/// `Some (node)` a Layer or LayoutLayer of the LayerSet.
pub type LayerKey = Option<NodeId>;

/// X3DWorld.set_rootNodes__: the last LayerSet among the root nodes.
pub fn layer_set(sg: &Sg) -> Option<NodeId> {
    sg.roots()
        .into_iter()
        .filter(|n| sg.type_name(*n) == Some("LayerSet"))
        .last()
}

/// X3DCast (X3DLayerNode, node).
pub fn is_layer(sg: &Sg, n: NodeId) -> bool {
    matches!(
        sg.type_name(n),
        Some("Layer" | "LayoutLayer" | "AnnotationLayer")
    )
}

/// X3DLayerNode.set_visible_and_hidden__: `_display` (the private `hidden` field is never set).
pub fn displayed(sg: &Sg, key: LayerKey) -> bool {
    key.is_none_or(|n| sg.bool(n, "visible", true))
}

/// The entry `index` of LayerSet `layers` (`X3DCast (X3DConstants .X3DLayerNode, layers [index])`).
fn layer_at(sg: &Sg, set: NodeId, index: i32) -> Option<NodeId> {
    let raw = sg.raw_nodes(set, "layers");

    if index < 0 || index as usize >= raw.len() {
        return None;
    }

    raw[index as usize]
        .and_then(|n| sg.inner(n))
        .filter(|n| is_layer(sg, *n))
}

/// LayerSet.set_layers__: the layers in `order` (0 is layer 0, which is always added; the others
/// only while displayed). Without a LayerSet only layer 0.
pub fn drawn_layers(sg: &Sg) -> Vec<LayerKey> {
    let Some(set) = layer_set(sg) else {
        return vec![None];
    };
    let mut out = vec![];

    for &index in sg.i32s(set, "order") {
        if index == 0 {
            out.push(None);
        } else if let Some(n) = layer_at(sg, set, index - 1) {
            if displayed(sg, Some(n)) {
                out.push(Some(n));
            }
        }
    }

    out
}

/// LayerSet.set_activeLayer__: the active layer (`None` when activeLayer names no displayed layer).
pub fn active_layer(sg: &Sg) -> Option<LayerKey> {
    let Some(set) = layer_set(sg) else {
        return Some(None);
    };
    let index = sg.i32(set, "activeLayer", 0);
    let key = if index == 0 {
        Some(None)
    } else {
        layer_at(sg, set, index - 1).map(Some)
    };

    key.filter(|k| displayed(sg, *k))
}

/// LayerSet.bindBindables: layer 0 and every layer of `layers`.
pub fn all_layers(sg: &Sg) -> Vec<LayerKey> {
    let mut out = vec![None];

    if let Some(set) = layer_set(sg) {
        for n in sg.nodes(set, "layers") {
            if is_layer(sg, n) && !out.contains(&Some(n)) {
                out.push(Some(n));
            }
        }
    }

    out
}

/// The children of a layer's group: the root nodes for layer 0.
pub fn layer_children(sg: &Sg, key: LayerKey) -> Vec<NodeId> {
    match key {
        None => sg.roots(),
        Some(n) => sg.nodes(n, "children"),
    }
}

pub fn is_layout_layer(sg: &Sg, key: LayerKey) -> bool {
    key.is_some_and(|n| sg.type_name(n) == Some("LayoutLayer"))
}

/// Viewport.getRectangle (viewport): the clipBoundary fractions (clamped to 0 … 1) of the
/// enclosing rectangle `parent` (x, y, width, height in pixels, y up), floored.
pub fn viewport_rectangle(sg: &Sg, n: NodeId, parent: [f64; 4]) -> [f64; 4] {
    let b = sg.f64s(n, "clipBoundary");
    let at = |i: usize, d: f64| b.get(i).copied().unwrap_or(d).clamp(0.0, 1.0);
    let (left, right, bottom, top) = (at(0, 0.0), at(1, 1.0), at(2, 0.0), at(3, 1.0));
    let l = (parent[0] + parent[2] * left).floor();
    let r = (parent[0] + parent[2] * right).floor();
    let bo = (parent[1] + parent[3] * bottom).floor();
    let t = (parent[1] + parent[3] * top).floor();

    [l, bo, (r - l).max(0.0), (t - bo).max(0.0)]
}

/// X3DLayerNode.set_viewport__: the layer's Viewport (X3DCast X3DViewportNode), else the browser's
/// default viewport (the whole canvas).
pub fn layer_viewport(sg: &Sg, key: LayerKey, browser: [f64; 4]) -> [f64; 4] {
    match key
        .and_then(|n| sg.node(n, "viewport"))
        .filter(|v| sg.type_name(*v) == Some("Viewport"))
    {
        Some(v) => viewport_rectangle(sg, v, browser),
        None => browser,
    }
}

/// X3DViewportNode.push for the pointer: the point lies in the rectangle
/// (X3DPointingDeviceSensorContext.isPointerInRectangle).
pub fn pointer_in_rectangle(r: &[f64; 4], x: f64, y: f64) -> bool {
    x >= r[0] && x <= r[0] + r[2] && y >= r[1] && y <= r[1] + r[3]
}

// --- Layout ------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
    Left,
    Center,
    Right,
    Bottom,
    Top,
    World,
    Fraction,
    Pixel,
    None,
    Stretch,
}

/// Layout's field values after set_align__, set_offsetUnits__ … set_scaleMode__.
#[derive(Clone, Copy, Debug)]
pub struct LayoutNode {
    align: [Unit; 2],
    offset_units: [Unit; 2],
    offset: [f64; 2],
    size_units: [Unit; 2],
    size: [f64; 2],
    scale_mode: [Unit; 2],
}

/// The bound viewpoint and navigation info of the layer, as Layout.push reads them:
/// getViewportSize (OrthoViewpoint: its user size, else nearValue × tan (userFieldOfView / 2) × 2)
/// and the ContentScale rendering property.
#[derive(Clone, Copy, Debug)]
pub struct LayoutCamera {
    /// OrthoViewpoint getUserSizeX, getUserSizeY.
    pub ortho: Option<(f64, f64)>,
    pub fov: f64,
    pub near: f64,
    pub content_scale: f64,
}

impl LayoutCamera {
    /// getViewportSize (viewport, nearValue), in meters.
    pub fn viewport_size(&self, viewport: &[f64; 4]) -> (f64, f64) {
        let (width, height) = (viewport[2], viewport[3]);
        let ratio = width / height;

        match self.ortho {
            Some((size_x, size_y)) => {
                if ratio > size_x / size_y {
                    (size_y * ratio, size_y)
                } else {
                    (size_x, size_x / ratio)
                }
            }
            None => {
                let size = self.near * (self.fov / 2.0).tan() * 2.0;

                if ratio > 1.0 {
                    (size * ratio, size)
                } else {
                    (size, size / ratio)
                }
            }
        }
    }
}

impl LayoutNode {
    pub fn read(sg: &Sg, n: NodeId) -> LayoutNode {
        let strings = |name: &str| sg.strings(n, name);
        let unit = |v: Option<&String>, scale: bool| match v.map(|s| s.as_str()) {
            Some("FRACTION") => Unit::Fraction,
            Some("PIXEL") => Unit::Pixel,
            Some("STRETCH") if scale => Unit::Stretch,
            _ if scale => Unit::None,
            _ => Unit::World,
        };
        // If a field has a length of one, that value applies to both directions.
        let pair = |name: &str, scale: bool| {
            let v = strings(name);
            let x = unit(v.first(), scale);
            [
                x,
                if v.len() > 1 {
                    unit(v.get(1), scale)
                } else {
                    x
                },
            ]
        };
        let align = strings("align");
        let ax = match align.first().map(|s| s.as_str()) {
            Some("LEFT") => Unit::Left,
            Some("RIGHT") => Unit::Right,
            _ => Unit::Center,
        };
        let ay = match align.get(1).map(|s| s.as_str()) {
            Some("BOTTOM") => Unit::Bottom,
            Some("TOP") => Unit::Top,
            _ => Unit::Center,
        };
        let numbers = |name: &str| {
            let v = sg.f64s(n, name);
            let x = v.first().copied().unwrap_or(0.0);
            [x, v.get(1).copied().unwrap_or(x)]
        };

        LayoutNode {
            align: [ax, ay],
            offset_units: pair("offsetUnits", false),
            offset: numbers("offset"),
            size_units: pair("sizeUnits", false),
            size: numbers("size"),
            scale_mode: pair("scaleMode", true),
        }
    }
}

/// renderObject.getLayouts () and getLayoutRectangles ().
#[derive(Clone, Debug, Default)]
pub struct LayoutStack {
    layouts: Vec<LayoutNode>,
    rectangles: Vec<[f64; 4]>,
}

impl LayoutStack {
    /// getOffsetUnitX / getSizeUnitX …: WORLD takes the parent layout's unit, FRACTION at the top.
    fn unit(&self, which: fn(&LayoutNode) -> Unit, own: Unit) -> Unit {
        let mut unit = own;

        for parent in self.layouts.iter().rev() {
            if unit != Unit::World {
                return unit;
            }

            unit = which(parent);
        }

        if unit == Unit::World {
            Unit::Fraction
        } else {
            unit
        }
    }

    /// Layout.push: the new model-view matrix (translation and rotation of `model_view`, then the
    /// layout translation and scale) for the rectangle of `layout` in `viewport` (pixels).
    pub fn push(
        &mut self,
        layout: &LayoutNode,
        camera: &LayoutCamera,
        viewport: &[f64; 4],
        model_view: &Matrix4,
    ) -> Matrix4 {
        let viewport_meter = camera.viewport_size(viewport);
        let parent_rectangle = self.rectangles.last().copied().unwrap_or([
            0.0,
            0.0,
            viewport_meter.0,
            viewport_meter.1,
        ]);
        let viewport_pixel = (
            viewport[2] / camera.content_scale,
            viewport[3] / camera.content_scale,
        );
        let pixel_size = (
            viewport_meter.0 / viewport_pixel.0,
            viewport_meter.1 / viewport_pixel.1,
        );
        let mut rectangle = [0.0; 4];

        let size_unit_x = self.unit(|l| l.size_units[0], layout.size_units[0]);
        let size_unit_y = self.unit(|l| l.size_units[1], layout.size_units[1]);

        match size_unit_x {
            Unit::Fraction => rectangle[2] = layout.size[0] * parent_rectangle[2],
            Unit::Pixel => rectangle[2] = layout.size[0] * pixel_size.0,
            _ => {}
        }

        match size_unit_y {
            Unit::Fraction => rectangle[3] = layout.size[1] * parent_rectangle[3],
            Unit::Pixel => rectangle[3] = layout.size[1] * pixel_size.1,
            _ => {}
        }

        // Translation (the CENTER y case tests sizeUnitX, as X_ITE does).
        let odd = |v: f64| (v as i64) & 1 == 1;
        let mut translation = Vector3::ZERO;

        match layout.align[0] {
            Unit::Left => translation.x = -(parent_rectangle[2] - rectangle[2]) / 2.0,
            Unit::Right => translation.x = (parent_rectangle[2] - rectangle[2]) / 2.0,
            _ if size_unit_x == Unit::Pixel && odd(viewport_pixel.0) => {
                translation.x = -pixel_size.0 / 2.0
            }
            _ => {}
        }

        match layout.align[1] {
            Unit::Bottom => translation.y = -(parent_rectangle[3] - rectangle[3]) / 2.0,
            Unit::Top => translation.y = (parent_rectangle[3] - rectangle[3]) / 2.0,
            _ if size_unit_x == Unit::Pixel && odd(viewport_pixel.1) => {
                translation.y = -pixel_size.1 / 2.0
            }
            _ => {}
        }

        // Offset.
        let mut offset = Vector3::ZERO;

        match self.unit(|l| l.offset_units[0], layout.offset_units[0]) {
            Unit::Fraction => offset.x = layout.offset[0] * parent_rectangle[2],
            Unit::Pixel => offset.x = layout.offset[0] * viewport_meter.0 / viewport_pixel.0,
            _ => {}
        }

        match self.unit(|l| l.offset_units[1], layout.offset_units[1]) {
            Unit::Fraction => offset.y = layout.offset[1] * parent_rectangle[3],
            Unit::Pixel => offset.y = layout.offset[1] * viewport_meter.1 / viewport_pixel.1,
            _ => {}
        }

        // Scale: getScaleModeX (parent): NONE is FRACTION without a parent layout.
        let has_parent = !self.layouts.is_empty();
        let mode = |m: Unit| {
            if !has_parent && m == Unit::None {
                Unit::Fraction
            } else {
                m
            }
        };
        let (scale_mode_x, scale_mode_y) = (mode(layout.scale_mode[0]), mode(layout.scale_mode[1]));
        let mut current_translation = Vector3::ZERO;
        let mut current_rotation = Rotation4::identity();
        let mut current_scale = Vector3::ONE;
        model_view.get_transform(
            Some(&mut current_translation),
            Some(&mut current_rotation),
            Some(&mut current_scale),
            None,
            None,
        );
        let mut scale = Vector3::ONE;

        match scale_mode_x {
            Unit::None => scale.x = current_scale.x,
            Unit::Fraction => scale.x = rectangle[2],
            Unit::Pixel => scale.x = viewport_meter.0 / viewport_pixel.0,
            _ => {}
        }

        match scale_mode_y {
            Unit::None => scale.y = current_scale.y,
            Unit::Fraction => scale.y = rectangle[3],
            Unit::Pixel => scale.y = viewport_meter.1 / viewport_pixel.1,
            _ => {}
        }

        if scale_mode_x == Unit::Stretch {
            if scale_mode_y == Unit::Stretch {
                if rectangle[2] > rectangle[3] {
                    scale.x = rectangle[2];
                    scale.y = scale.x;
                } else {
                    scale.y = rectangle[3];
                    scale.x = scale.y;
                }
            } else {
                scale.x = scale.y;
            }
        } else if scale_mode_y == Unit::Stretch {
            scale.y = scale.x;
        }

        translation.add(&offset);
        rectangle[0] = translation.x;
        rectangle[1] = translation.y;

        let mut matrix = Matrix4::IDENTITY;
        matrix.set_transform(
            Some(&current_translation),
            Some(&current_rotation),
            None,
            None,
            None,
        );
        matrix.translate(&translation);
        matrix.scale(&scale);

        self.layouts.push(*layout);
        self.rectangles.push(rectangle);
        matrix
    }

    /// Layout.pop.
    pub fn pop(&mut self) {
        self.layouts.pop();
        self.rectangles.pop();
    }
}
