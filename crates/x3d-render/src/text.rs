//! Text geometry: Components/Text/Text.js (`set_fontStyle__`, `build`) over the x3d-text port of
//! Browser/Text/{X3DTextGeometry,PolygonText}.js, and the font selection of
//! Components/Text/X3DFontStyleNode.js `loadData` (default families, font files named by
//! `family`, FontLibrary registrations of the FontStyle's execution context).

use crate::geometry::Geometry;
use crate::sg::Sg;
use crate::texture::TextureImage;
use std::rc::Rc;
use x3d_fields::FieldValue;
use x3d_fields::NodeId;
use x3d_fields::math::Vector3;
use x3d_text::{Font, FontStyleParams, PrimitiveQuality, TextCompression, TextParams};

/// X3DFontStyleNode.loadData: every family name with "SERIF" appended is tried in order.
/// Returns None while no font loaded (the Text then has no geometry).
pub fn resolve_font(sg: &Sg, font_style: Option<NodeId>) -> Option<Rc<Font>> {
    let (mut family, style) = match font_style {
        Some(f) => {
            let style = match sg.value(f, "style") {
                Some(FieldValue::SFString(s)) => s.clone(),
                _ => "PLAIN".to_string(),
            };

            (sg.strings(f, "family"), style)
        }
        // browser.getDefaultFontStyle (): a FontStyle with default fields.
        None => (vec!["SERIF".to_string()], "PLAIN".to_string()),
    };

    // X3DFontStyleNode.font: the font the last loadData took from a FontLibrary (see x3d-runtime `font_library.rs`).
    if let Some(font) = font_style.and_then(|fs| sg.rt.font_style_library_font(fs)) {
        return Some(font);
    }

    family.push("SERIF".into());

    let base = url::Url::parse(&sg.rt.base_url(font_style.unwrap_or(0))).ok();

    for f in &family {
        if x3d_text::fonts::default_font_file(f, &style).is_some() {
            return x3d_text::fonts::load_default_font(f, &style);
        }


        // DEPRECATED: Try to get font by URL.
        let Some(file_url) = base.as_ref().and_then(|b| b.join(f).ok()) else {
            return None;
        };
        let path = file_url.path().to_ascii_lowercase();

        if file_url.scheme() == "data" || [".woff2", ".woff", ".otf", ".ttf"].iter().any(|e| path.ends_with(e)) {
            match x3d_text::fonts::url_font(file_url.as_str()) {
                Some(Some(font)) => return Some(font),
                // Unparsable data: the loadFont promise never resolves.
                Some(None) => return None,
                None => continue,
            }
        }
    }

    None
}

fn font_style_params(sg: &Sg, font_style: Option<NodeId>) -> FontStyleParams {
    let Some(f) = font_style else { return FontStyleParams::default() };

    // ScreenFontStyle.getScale: pointSize × PixelsPerPoint × ContentScale, in pixels; getContentScale: ContentScale.
    let screen = sg.type_name(f) == Some("ScreenFontStyle");
    let rendering = |name: &str| sg.rt.browser_options.rendering.get(name).map_or(1.0, |v| v.as_f64());
    let content_scale = if screen { rendering("ContentScale") } else { 1.0 };

    FontStyleParams {
        size: if screen { sg.f64(f, "pointSize", 12.0) * rendering("PixelsPerPoint") * content_scale } else { sg.f64(f, "size", 1.0) },
        content_scale,
        screen,
        spacing: sg.f64(f, "spacing", 1.0),
        horizontal: sg.bool(f, "horizontal", true),
        left_to_right: sg.bool(f, "leftToRight", true),
        top_to_bottom: sg.bool(f, "topToBottom", true),
        justify: sg.strings(f, "justify"),
        cached_alignments: sg.rt.font_style_alignments.get(&f).copied(),
    }
}

/// Text.build → X3DGeometryNode arrays (a ScreenFontStyle: ScreenText's quad and canvas).
pub fn text_geometry(sg: &Sg, n: NodeId, unsupported: &mut std::collections::BTreeSet<String>) -> Option<Geometry> {
    let font_style = match sg.value(n, "fontStyle") {
        Some(FieldValue::SFNode(Some(c))) => sg.inner(*c),
        _ => None,
    };

    // X3DCast (X3DConstants .X3DFontStyleNode, ...) ?? browser .getDefaultFontStyle ()
    let font_style = match font_style {
        Some(f) => match sg.type_name(f) {
            Some("FontStyle") => Some(f),
            Some("ScreenFontStyle") => Some(f),
            _ => None,
        },
        None => None,
    };

    let font = resolve_font(sg, font_style);
    let length: Vec<f64> = sg.f32s(n, "length").iter().map(|v| *v as f64).collect();
    let text = TextParams::from_strings(&sg.strings(n, "string"), &length, sg.f64(n, "maxExtent", 0.0));
    let fs = font_style_params(sg, font_style);
    // The PrimitiveQuality (PolygonText bezier steps) and TextCompression options.
    let quality = match crate::traverse::primitive_quality() {
        crate::traverse::Quality::Low => PrimitiveQuality::Low,
        crate::traverse::Quality::Medium => PrimitiveQuality::Medium,
        crate::traverse::Quality::High => PrimitiveQuality::High,
    };
    let compression = if crate::traverse::text_compression_scaling() { TextCompression::Scaling } else { TextCompression::CharSpacing };
    let layout = x3d_text::layout(font.as_deref(), &text, &fs, quality, compression);
    // ScreenText: its canvas is the texture, filtered NEAREST_PIXEL and clamped (X3DLayoutContext.getScreenTextureProperties).
    let texture = layout.texture.as_ref().map(|(width, height, rgba)| Rc::new(TextureImage::screen_text(*width, *height, rgba.clone())));
    let f32s = |v: &[f64]| v.iter().map(|x| *x as f32).collect::<Vec<f32>>();
    let has_vertices = !layout.vertices.is_empty();

    Some(Geometry {
        geometry_type: 3,
        vertices: f32s(&layout.vertices),
        normals: f32s(&layout.normals),
        tex_coords: f32s(&layout.tex_coords),
        colors: vec![],
        solid: sg.bool(n, "solid", false),
        ccw: true,
        texture,
        min: if has_vertices { Vector3::new(layout.min[0], layout.min[1], layout.min[2]) } else { Vector3::ZERO },
        max: if has_vertices { Vector3::new(layout.max[0], layout.max[1], layout.max[2]) } else { Vector3::ZERO },
        ..Default::default()
    })
}
