//! VolumeRendering component: VolumeData, IsoSurfaceVolumeData, SegmentedVolumeData and the
//! volume render styles (Components/VolumeRendering/*.js, Browser/VolumeRendering/*.js of the
//! pinned X_ITE).
//!
//! X_ITE draws a volume as a stack of view aligned planes: X3DVolumeDataNode builds a private
//! Group → ProximitySensor + Transform → Shape (a QuadSet of 200 / 400 / 600 planes per
//! TextureQuality, a TextureCoordinate3D with the same points, an Appearance with a
//! TextureTransform3D and the voxels as texture, alphaMode BLEND, an unlit VolumeMaterial). The
//! ProximitySensor's orientation_changed rotates both the Transform and the texture transform, so
//! the planes face the viewer. Each render style contributes GLSL snippets to one fragment shader
//! (VolumeStyle.fs.js); this module composes the same snippets as WGSL ([`generate`]) and
//! [`gpu`] draws the planes procedurally (no vertex buffer) in the ordinary transparent pass.

use crate::sg::Sg;
use crate::texture::{self, Filter, Sampler, TextureImage, Wrap};
use std::collections::HashMap;
use std::fmt::Write;
use std::rc::Rc;
use x3d_fields::math::{Matrix4, Rotation4, Vector3};
use x3d_fields::NodeId;

pub const VOLUME_DATA_TYPES: [&str; 3] = ["VolumeData", "IsoSurfaceVolumeData", "SegmentedVolumeData"];

/// The textures a program binds: at most this many (one texture and sampler binding each; the
/// fragment stage has 16 of each).
pub const MAX_SLOTS: usize = 15;

/// X3DVolumeDataNode.getNumPlanes.
pub fn num_planes(quality: crate::traverse::Quality) -> u32 {
    match quality {
        crate::traverse::Quality::Low => 200,
        crate::traverse::Quality::Medium => 400,
        crate::traverse::Quality::High => 600,
    }
}

/// One texture binding of a program: 2D transfer functions and weight tables, 3D voxels.
#[derive(Clone)]
pub struct Slot {
    pub texture: Rc<TextureImage>,
    pub three_d: bool,
}

/// The generated fragment program of a volume (the same for equal text and slot layout).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Program {
    /// WGSL declarations and `get_texture_color`.
    pub source: String,
    /// The dimension of each slot (true: 3D).
    pub dims: Vec<bool>,
    /// vec4 count of the value pool (`u.v`).
    pub vec4s: usize,
}

/// Everything the renderer needs to draw one volume.
#[derive(Clone, Debug)]
pub struct VolumeDraw {
    /// The model-view matrix of the planes: the VolumeData's, times the Transform rotation the
    /// ProximitySensor sets (column-major as GLSL).
    pub model_view: [f32; 16],
    /// x3d_TextureMatrix [0]: the TextureTransform3D of the private Appearance.
    pub tex_matrix: [f32; 16],
    /// x3d_TextureNormalMatrix as X_ITE stores it (a row-major Matrix3, uploaded transposed).
    pub normal_matrix: [f32; 9],
    pub planes: u32,
    /// The planes' edge length: the norm of `dimensions`.
    pub size: f32,
    pub program: Rc<Program>,
    pub values: Vec<f32>,
    pub textures: Vec<Rc<TextureImage>>,
}

thread_local! {
    static DEFAULTS: (Rc<TextureImage>, Rc<TextureImage>) = (Rc::new(default_voxels()), Rc::new(default_transfer_function()));
}

/// An empty geometry for the DrawItem of a volume (the renderer draws `DrawItem::volume`).
pub fn empty_geometry() -> Rc<crate::geometry::Geometry> {
    thread_local! {
        static EMPTY: Rc<crate::geometry::Geometry> = Rc::new(crate::geometry::Geometry::default());
    }
    EMPTY.with(|e| e.clone())
}

/// X3DVolumeRenderingContext.getDefaultVoxels: a 1 × 1 × 1 PixelTexture3D, [1, 1, 1, 1, 255].
fn default_voxels() -> TextureImage {
    texture::white_3d()
}

/// X3DVolumeRenderingContext.getDefaultTransferFunction: a 256 × 1 two component PixelTexture,
/// luminance = alpha = i, with boundaryModeS CLAMP_TO_EDGE, boundaryModeT REPEAT, NICEST filters.
fn default_transfer_function() -> TextureImage {
    let mut rgba = Vec::with_capacity(256 * 4);

    for i in 0..=255u8 {
        rgba.extend_from_slice(&[i, i, i, i]);
    }

    TextureImage {
        width: 256,
        height: 1,
        rgba,
        transparent: true,
        flip_y: false,
        repeat_s: false,
        repeat_t: true,
        nearest: false,
        no_mipmaps: true,
        url: String::new(),
        sampler: Sampler { mag: Filter::Linear, min: Filter::Linear, mipmap: None, wrap_s: Wrap::ClampToEdge, wrap_t: Wrap::Repeat, wrap_r: Wrap::ClampToEdge, anisotropy: 1.0 },
        cube: false,
        generated: None,
        volume: false,
        depth: 1,
        float: false,
    }
}

fn default_voxels_rc() -> Rc<TextureImage> {
    DEFAULTS.with(|d| d.0.clone())
}

fn default_tf_rc() -> Rc<TextureImage> {
    DEFAULTS.with(|d| d.1.clone())
}

/// true: an X3DTexture3DNode, false: an X3DTexture2DNode, None: neither.
fn texture_dimension(ty: &str) -> Option<bool> {
    match ty {
        "ImageTexture" | "PixelTexture" | "MovieTexture" | "RenderedTexture" => Some(false),
        "ImageTexture3D" | "PixelTexture3D" | "ComposedTexture3D" | "ImageTextureAtlas" => Some(true),
        _ => None,
    }
}

/// X3DComposableVolumeRenderStyleNode types.
fn composable(ty: &str) -> bool {
    matches!(
        ty,
        "OpacityMapVolumeStyle" | "BlendedVolumeStyle" | "BoundaryEnhancementVolumeStyle" | "CartoonVolumeStyle" | "ComposedVolumeStyle" | "EdgeEnhancementVolumeStyle" | "ShadedVolumeStyle" | "SilhouetteEnhancementVolumeStyle" | "ToneMappedVolumeStyle"
    )
}

/// X3DVolumeRenderStyleNode types (the composable ones and ProjectionVolumeStyle).
fn render_style(ty: &str) -> bool {
    composable(ty) || ty == "ProjectionVolumeStyle"
}

/// The program generator: the WGSL counterpart of the getUniformsText / getFunctionsText /
/// addShaderFields of the render styles.
struct Gen<'a, 'b> {
    sg: &'a Sg<'b>,
    tex: &'a mut dyn FnMut(NodeId) -> Option<Rc<TextureImage>>,
    unsupported: &'a mut Vec<String>,
    decls: String,
    slots: Vec<Slot>,
    values: Vec<f32>,
    next_id: u32,
    hsv: bool,
    plane: bool,
}

impl<'a, 'b> Gen<'a, 'b> {
    fn id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    /// A texture slot; the same texture and dimension share a slot. `None` when the program is
    /// out of bindings.
    fn slot(&mut self, texture: Rc<TextureImage>, three_d: bool) -> Option<usize> {
        if let Some(i) = self.slots.iter().position(|s| Rc::ptr_eq(&s.texture, &texture) && s.three_d == three_d) {
            return Some(i);
        }

        if self.slots.len() >= MAX_SLOTS {
            return None;
        }

        self.slots.push(Slot { texture, three_d });
        Some(self.slots.len() - 1)
    }

    /// A scalar uniform: `u.v[i].c`.
    fn scalar(&mut self, v: f64) -> String {
        let i = self.values.len();
        self.values.push(v as f32);
        format!("u.v[{}].{}", i / 4, ['x', 'y', 'z', 'w'][i % 4])
    }

    /// A vec4 uniform (aligned).
    fn vec4(&mut self, v: [f64; 4]) -> String {
        while self.values.len() % 4 != 0 {
            self.values.push(0.0);
        }

        let i = self.values.len() / 4;
        self.values.extend(v.iter().map(|v| *v as f32));
        format!("u.v[{i}]")
    }

    fn sample3(&self, slot: usize, coord: &str) -> String {
        format!("textureSampleLevel(t{slot}, s{slot}, {coord}, 0.0)")
    }

    /// X3DVolumeRenderStyleNode.getNormalText: `get_normal_<id> (tc)`; the normals texture of the
    /// style, else central differences of the voxels (slot `vox`).
    fn normal_fn(&mut self, id: u32, normals: Option<usize>, vox: usize) {
        let mut s = String::new();
        let _ = writeln!(s, "\nfn get_normal_{id}(tc: vec3<f32>) -> vec4<f32> {{");

        match normals {
            Some(n) => {
                let _ = writeln!(s, "    let n = {}.xyz * 2.0 - vec3<f32>(1.0);", self.sample3(n, "tc"));
            }
            None => {
                let _ = writeln!(s, "    let offset = vec4<f32>(1.0 / vec3<f32>(textureDimensions(t{vox}, 0)), 0.0);");
                let _ = writeln!(s, "    let i0 = {}.r;", self.sample3(vox, "tc + offset.xww"));
                let _ = writeln!(s, "    let i1 = {}.r;", self.sample3(vox, "tc - offset.xww"));
                let _ = writeln!(s, "    let i2 = {}.r;", self.sample3(vox, "tc + offset.wyw"));
                let _ = writeln!(s, "    let i3 = {}.r;", self.sample3(vox, "tc - offset.wyw"));
                let _ = writeln!(s, "    let i4 = {}.r;", self.sample3(vox, "tc + offset.wwz"));
                let _ = writeln!(s, "    let i5 = {}.r;", self.sample3(vox, "tc - offset.wwz"));
                let _ = writeln!(s, "    let n = vec3<f32>(i1 - i0, i3 - i2, i5 - i4);");
            }
        }

        let _ = writeln!(s, "    return vec4<f32>(normalize(normal_matrix() * n), length(n));");
        let _ = writeln!(s, "}}");
        self.decls.push_str(&s);
    }

    /// The surfaceNormals texture of a style (X3DCast (X3DTexture3DNode, ...)).
    fn surface_normals(&mut self, n: NodeId) -> Option<usize> {
        let t = self.sg.node(n, "surfaceNormals").filter(|t| texture_dimension(self.sg.type_name(*t).unwrap_or("")) == Some(true))?;
        let image = (self.tex)(t)?;
        self.slot(image, true)
    }

    fn enabled(&self, n: NodeId) -> bool {
        self.sg.bool(n, "enabled", true)
    }

    /// The transfer function of an OpacityMapVolumeStyle: X3DTexture2DNode ?? X3DTexture3DNode ??
    /// the default; returns (slot, three_d).
    fn transfer_function(&mut self, n: NodeId) -> (usize, bool) {
        if let Some(t) = self.sg.node(n, "transferFunction") {
            if let Some(three_d) = texture_dimension(self.sg.type_name(t).unwrap_or("")) {
                let image = (self.tex)(t).unwrap_or_else(|| Rc::new(TextureImage::white()));

                if let Some(i) = self.slot(image, three_d) {
                    return (i, three_d);
                }
            }
        }

        (self.slot(default_tf_rc(), false).unwrap_or(0), false)
    }

    /// The default OpacityMapVolumeStyle statement (getDefaultVolumeStyle / the default
    /// BlendedVolumeStyle's), declared with its own id.
    fn default_opacity(&mut self, target: &str) -> String {
        let id = self.id();
        let slot = self.slot(default_tf_rc(), false).unwrap_or(0);
        let _ = write!(self.decls, "\nfn get_opacity_map_{id}(c: vec4<f32>) -> vec4<f32> {{\n    return {};\n}}\n", format_args!("textureSampleLevel(t{slot}, s{slot}, c.rg, 0.0)"));
        format!("    {target} = get_opacity_map_{id}({target});\n")
    }

    /// The style's `getFunctionsText`, after adding its declarations. `target` is the color
    /// variable ("texture_color", or the blend color of a BlendedVolumeStyle), `vox` the slot of
    /// the voxels the style samples.
    fn style(&mut self, n: NodeId, target: &str, vox: usize) -> String {
        let Some(ty) = self.sg.type_name(n) else { return String::new() };

        if !self.enabled(n) {
            return String::new();
        }

        let id = self.id();
        let call = |name: &str| format!("    {target} = {name}_{id}({target}, tc, vertex);\n");

        match ty {
            "OpacityMapVolumeStyle" => {
                let (slot, three_d) = self.transfer_function(n);
                let coord = if three_d { "c.rgb" } else { "c.rg" };
                let _ = write!(self.decls, "\nfn get_opacity_map_{id}(c: vec4<f32>) -> vec4<f32> {{\n    return textureSampleLevel(t{slot}, s{slot}, {coord}, 0.0);\n}}\n");
                format!("    {target} = get_opacity_map_{id}({target});\n")
            }
            "BoundaryEnhancementVolumeStyle" => {
                let retained = self.scalar(self.sg.f64(n, "retainedOpacity", 0.2));
                let boundary = self.scalar(self.sg.f64(n, "boundaryOpacity", 0.9));
                let factor = self.scalar(self.sg.f64(n, "opacityFactor", 2.0));
                let f0 = self.sample3(vox, "tc");
                let f1 = self.sample3(vox, &format!("tc + vec3<f32>(0.0, 0.0, 1.0 / f32(textureDimensions(t{vox}, 0).z))"));
                let _ = write!(
                    self.decls,
                    "\nfn get_boundary_{id}(original: vec4<f32>, tc: vec3<f32>, vertex: vec3<f32>) -> vec4<f32> {{\n    let f0 = {f0}.r;\n    let f1 = {f1}.r;\n    let f = abs(f0 - f1);\n    return vec4<f32>(original.rgb, original.a * ({retained} + {boundary} * pow(f, {factor})));\n}}\n"
                );
                call("get_boundary")
            }
            "CartoonVolumeStyle" => {
                self.hsv = true;
                let steps = self.scalar(self.sg.i32(n, "colorSteps", 4) as f64);
                let orthogonal = self.vec4(self.sg.color4(n, "orthogonalColor", [1.0, 1.0, 1.0, 1.0]));
                let parallel = self.vec4(self.sg.color4(n, "parallelColor", [0.0, 0.0, 0.0, 1.0]));
                let normals = self.surface_normals(n);
                self.normal_fn(id, normals, vox);
                let _ = write!(
                    self.decls,
                    "\nfn get_cartoon_{id}(original: vec4<f32>, tc: vec3<f32>, vertex: vec3<f32>) -> vec4<f32> {{
    let surface_normal = get_normal_{id}(tc);
    if (surface_normal.w == 0.0) {{
        return vec4<f32>(0.0);
    }}
    let steps = clamp(f32(i32({steps})), 1.0, 64.0);
    let step_ = M_PI / 2.0 / steps;
    let cos_theta = min(dot(surface_normal.xyz, normalize(vertex)), 1.0);
    if (cos_theta < 0.0) {{
        return vec4<f32>(0.0);
    }}
    let t = cos(min(floor(acos(cos_theta) / step_) * select(1.0, steps / (steps - 1.0), steps > 1.0), steps) * step_);
    let orthogonal_hsv = rgb2hsv({orthogonal}.rgb);
    let parallel_hsv = rgb2hsv({parallel}.rgb);
    return vec4<f32>(hsv2rgb(mix_hsv(orthogonal_hsv, parallel_hsv, t)), original.a);
}}\n"
                );
                call("get_cartoon")
            }
            "EdgeEnhancementVolumeStyle" => {
                let edge = self.vec4(self.sg.color4(n, "edgeColor", [0.0, 0.0, 0.0, 1.0]));
                let threshold = self.scalar(self.sg.f64(n, "gradientThreshold", 0.4));
                let normals = self.surface_normals(n);
                self.normal_fn(id, normals, vox);
                let _ = write!(
                    self.decls,
                    "\nfn get_edge_{id}(original: vec4<f32>, tc: vec3<f32>, vertex: vec3<f32>) -> vec4<f32> {{
    let surface_normal = get_normal_{id}(tc);
    if (surface_normal.w == 0.0) {{
        return vec4<f32>(0.0);
    }}
    let angle = abs(dot(surface_normal.xyz, normalize(vertex)));
    if (angle >= cos({threshold})) {{
        return original;
    }} else {{
        return vec4<f32>(mix({edge}.rgb, original.rgb, angle), original.a);
    }}
}}\n"
                );
                call("get_edge")
            }
            "SilhouetteEnhancementVolumeStyle" => {
                let retained = self.scalar(self.sg.f64(n, "silhouetteRetainedOpacity", 1.0));
                let boundary = self.scalar(self.sg.f64(n, "silhouetteBoundaryOpacity", 0.0));
                let sharpness = self.scalar(self.sg.f64(n, "silhouetteSharpness", 0.5));
                let normals = self.surface_normals(n);
                self.normal_fn(id, normals, vox);
                let _ = write!(
                    self.decls,
                    "\nfn get_silhouette_{id}(original: vec4<f32>, tc: vec3<f32>, vertex: vec3<f32>) -> vec4<f32> {{
    let surface_normal = get_normal_{id}(tc);
    if (surface_normal.w == 0.0) {{
        return vec4<f32>(0.0);
    }}
    return vec4<f32>(original.rgb, original.a * ({retained} + {boundary} * pow(1.0 - abs(dot(surface_normal.xyz, normalize(vertex))), {sharpness})));
}}\n"
                );
                call("get_silhouette")
            }
            "ToneMappedVolumeStyle" => {
                let cool = self.vec4(self.sg.color4(n, "coolColor", [0.0, 0.0, 1.0, 0.0]));
                let warm = self.vec4(self.sg.color4(n, "warmColor", [1.0, 1.0, 0.0, 0.0]));
                let normals = self.surface_normals(n);
                self.normal_fn(id, normals, vox);
                let _ = write!(
                    self.decls,
                    "\nfn get_tone_mapped_{id}(original: vec4<f32>, tc: vec3<f32>, vertex: vec3<f32>) -> vec4<f32> {{
    let surface_normal = get_normal_{id}(tc);
    if (surface_normal.w == 0.0) {{
        return vec4<f32>(0.0);
    }}
    var tone_color = vec3<f32>(0.0);
    let cool_color = {cool}.rgb;
    let warm_color = {warm}.rgb;
    for (var i = 0u; i < u.counts.y; i = i + 1u) {{
        let light = frame.lights[u.light_index[i / 4u][i % 4u]];
        var L: vec3<f32>;
        if (u32(light.color_type.w) == 1u) {{
            L = -light.direction_intensity.xyz;
        }} else {{
            L = normalize(light.location_ambient.xyz - vertex);
        }}
        let color_factor = dot(L, surface_normal.xyz) * 0.5 + 0.5;
        tone_color = tone_color + mix(warm_color, cool_color, color_factor);
    }}
    return vec4<f32>(tone_color, original.a);
}}\n"
                );
                call("get_tone_mapped")
            }
            "ShadedVolumeStyle" => self.shaded(n, id, target, vox),
            "ComposedVolumeStyle" => {
                let mut body = String::new();

                for c in self.sg.nodes(n, "renderStyle") {
                    if composable(self.sg.type_name(c).unwrap_or("")) {
                        body.push_str(&self.style(c, "texture_color", vox));
                    }
                }

                let _ = write!(self.decls, "\nfn get_composed_{id}(original: vec4<f32>, tc: vec3<f32>, vertex: vec3<f32>) -> vec4<f32> {{\n    var texture_color = original;\n{body}\n    return texture_color;\n}}\n");
                call("get_composed")
            }
            "BlendedVolumeStyle" => self.blended(n, id, target),
            "ProjectionVolumeStyle" => self.projection(n, id, target),
            _ => String::new(),
        }
    }

    fn shaded(&mut self, n: NodeId, id: u32, target: &str, vox: usize) -> String {
        let material = self.sg.node(n, "material").filter(|m| matches!(self.sg.type_name(*m), Some("Material" | "TwoSidedMaterial")));

        if let Some(m) = self.sg.node(n, "material") {
            if material.is_none() {
                // X3DMaterialNode without ambientIntensity, diffuseColor, ...: X_ITE's
                // addShaderFields throws on `.copy ()` of the missing field.
                self.unsupported.push(self.sg.type_name(m).unwrap_or("?").to_string());
            }
        }

        let (ambient, diffuse, specular, emissive, shininess, transparency) = match material {
            Some(m) => {
                let d = self.sg.color(m, "diffuseColor", [0.8, 0.8, 0.8]);
                let s = self.sg.color(m, "specularColor", [0.0, 0.0, 0.0]);
                let e = self.sg.color(m, "emissiveColor", [0.0, 0.0, 0.0]);
                (self.sg.f64(m, "ambientIntensity", 0.2), [d[0], d[1], d[2], 0.0], [s[0], s[1], s[2], 0.0], [e[0], e[1], e[2], 0.0], self.sg.f64(m, "shininess", 0.2), self.sg.f64(m, "transparency", 0.0))
            }
            None => (0.0, [0.0; 4], [0.0; 4], [0.0; 4], 0.0, 0.0),
        };
        let ambient = self.scalar(ambient);
        let diffuse = self.vec4(diffuse);
        let specular = self.vec4(specular);
        let emissive = self.vec4(emissive);
        let shininess = self.scalar(shininess);
        let transparency = self.scalar(transparency);
        let normals = self.surface_normals(n);
        self.normal_fn(id, normals, vox);
        let lighting = self.sg.bool(n, "lighting", false);
        let mut body = String::new();

        if lighting {
            if material.is_some() {
                let _ = write!(body, "    let diffuse_factor = {diffuse}.rgb;\n    let ambient_term = diffuse_factor * {ambient};\n    shaded.a = original.a * (1.0 - {transparency});\n");
            } else {
                body.push_str("    let diffuse_factor = original.rgb;\n    let ambient_term = vec3<f32>(0.0);\n    shaded.a = original.a;\n");
            }

            let _ = write!(
                body,
                "    let N = surface_normal.xyz;
    let V = normalize(-vertex);
    for (var i = 0u; i < u.counts.y; i = i + 1u) {{
        let light = frame.lights[u.light_index[i / 4u][i % 4u]];
        let light_matrix = mat3x3<f32>(light.m0.xyz, light.m1.xyz, light.m2.xyz);
        let vL = light.location_ambient.xyz - vertex;
        let dL = length(light_matrix * vL);
        let light_type = u32(light.color_type.w);
        let di = light_type == 1u;
        if (di || dL <= light.attenuation_radius.w) {{
            let d = light.direction_intensity.xyz;
            let c = light.attenuation_radius.xyz;
            var L: vec3<f32>;
            if (di) {{ L = -d; }} else {{ L = normalize(vL); }}
            let H = normalize(L + V);
            let light_angle = max(dot(N, L), 0.0);
            let diffuse_term = diffuse_factor * light_angle;
            var specular_factor = 1.0;
            if ({shininess} > 0.0) {{ specular_factor = pow(max(dot(N, H), 0.0), {shininess} * 128.0); }}
            let specular_term = light.direction_intensity.w * {specular}.rgb * specular_factor;
            var attenuation_factor = 1.0;
            if (!di) {{ attenuation_factor = 1.0 / max(dot(c, vec3<f32>(1.0, dL, dL * dL)), 1.0); }}
            var spot_factor = 1.0;
            if (light_type == 3u) {{ spot_factor = spot_factor_of(light.spot.x, light.spot.y, L, d); }}
            let attenuation_spot_factor = attenuation_factor * spot_factor;
            let ambient_color = light.location_ambient.w * ambient_term;
            let diffuse_specular_color = light.direction_intensity.w * (diffuse_term + specular_term);
            shaded = vec4<f32>(shaded.rgb + attenuation_spot_factor * light.color_type.xyz * (ambient_color + diffuse_specular_color), shaded.a);
        }}
        shaded = vec4<f32>(shaded.rgb + {emissive}.rgb, shaded.a);
    }}
"
            );
        } else if material.is_some() {
            let _ = write!(body, "    shaded = vec4<f32>({diffuse}.rgb, original.a * (1.0 - {transparency}));\n");
        } else {
            body.push_str("    shaded = original;\n");
        }

        let _ = write!(
            self.decls,
            "\nfn get_shaded_{id}(original: vec4<f32>, tc: vec3<f32>, vertex: vec3<f32>) -> vec4<f32> {{
    let surface_normal = get_normal_{id}(tc);
    if (surface_normal.w == 0.0) {{
        return vec4<f32>(0.0);
    }}
    var shaded = vec4<f32>(0.0);
{body}
    return shaded;
}}\n"
        );
        format!("    {target} = get_shaded_{id}({target}, tc, vertex);\n")
    }

    fn projection(&mut self, n: NodeId, id: u32, target: &str) -> String {
        self.plane = true;
        let threshold = self.scalar(self.sg.f64(n, "intensityThreshold", 0.0));
        let ty = self.sg.string(n, "type", "MAX");
        let (init, loop_body) = match ty.as_str() {
            "MIN" => ("1.0", format!("        if (intensity < {threshold}) {{ continue; }}\n        if (first && {threshold} > 0.0) {{ break; }}\n        if (intensity >= projection_color) {{ first = true; continue; }}\n        projection_color = intensity;\n")),
            "AVERAGE" => ("0.0", "        projection_color = projection_color + intensity;\n".to_string()),
            _ => ("0.0", format!("        if (intensity < {threshold}) {{ continue; }}\n        if (first && {threshold} > 0.0) {{ break; }}\n        if (intensity <= projection_color) {{ first = true; continue; }}\n        projection_color = intensity;\n")),
        };
        let average = if ty == "AVERAGE" { "    projection_color = projection_color / f32(samples);\n" } else { "" };
        let vox = self.sample3(0, "ray");
        let _ = write!(
            self.decls,
            "\nfn get_projection_{id}(original: vec4<f32>, tc: vec3<f32>, vertex: vec3<f32>) -> vec4<f32> {{
    var projection_color = {init};
    let samples = 16;
    let normal = normalize(normal_matrix() * vec3<f32>(0.0, 0.0, 1.0));
    let plane_normal = normal;
    let plane_distance = dot(normal, vec3<f32>(0.5));
    let point = tc + plane_normal * (dot(tc, plane_normal) - plane_distance);
    var ray = point - normal * M_SQRT1_2;
    let step_ = normal * (M_SQRT2 / f32(samples));
    var first = false;
    var i = 0;
    loop {{
        if (i >= samples) {{ break; }}
        if (any(abs(ray - vec3<f32>(0.5)) > vec3<f32>(0.5))) {{ continue; }}
        let intensity = {vox}.r;
{loop_body}        continuing {{
            i = i + 1;
            ray = ray + step_;
        }}
    }}
{average}    return vec4<f32>(vec3<f32>(projection_color), original.a);
}}\n"
        );
        format!("    {target} = get_projection_{id}({target}, tc, vertex);\n")
    }

    fn blended(&mut self, n: NodeId, id: u32, target: &str) -> String {
        // X3DCast (X3DTexture3DNode, voxels): without voxels the style does nothing.
        let Some(voxels) = self.sg.node(n, "voxels").filter(|t| texture_dimension(self.sg.type_name(*t).unwrap_or("")) == Some(true)) else { return String::new() };
        let image = (self.tex)(voxels).unwrap_or_else(|| Rc::new(texture::white_3d()));
        let Some(vox) = self.slot(image, true) else { return String::new() };
        let constant1 = self.scalar(self.sg.f64(n, "weightConstant1", 0.5));
        let constant2 = self.scalar(self.sg.f64(n, "weightConstant2", 0.5));
        let table = |g: &mut Self, name: &str| -> Option<usize> {
            let t = g.sg.node(n, name).filter(|t| texture_dimension(g.sg.type_name(*t).unwrap_or("")) == Some(false))?;
            let image = (g.tex)(t).unwrap_or_else(|| Rc::new(TextureImage::white()));
            g.slot(image, false)
        };
        let table1 = table(self, "weightTransferFunction1");
        let table2 = table(self, "weightTransferFunction2");
        let bc = format!("blend_color_{id}");
        let mut body = format!("    var {bc} = {};\n", self.sample3(vox, "tc"));
        body.push_str(&self.default_opacity(&bc));

        if let Some(inner) = self.sg.node(n, "renderStyle").filter(|c| composable(self.sg.type_name(*c).unwrap_or(""))) {
            let s = self.style(inner, &bc, vox);
            body.push_str(&s);
        }

        let weight = |which: usize, function: String, constant: &str, table: Option<usize>| -> String {
            let w = format!("w{which}_{id}");
            match function.as_str() {
                "ALPHA0" => format!("    let {w} = original.a;\n"),
                // X_ITE writes `blendColor_ <id> .a` (a GLSL syntax error); these take the alpha
                // of the blend color as the specification and the intent of the code say.
                "ALPHA1" => format!("    let {w} = {bc}.a;\n"),
                "ONE_MINUS_ALPHA0" => format!("    let {w} = 1.0 - original.a;\n"),
                "ONE_MINUS_ALPHA1" => format!("    let {w} = 1.0 - {bc}.a;\n"),
                "TABLE" => match table {
                    Some(t) => format!("    let {w} = textureSampleLevel(t{t}, s{t}, vec2<f32>(original.a, {bc}.a), 0.0).r;\n"),
                    None => format!("    let {w} = {constant};\n"),
                },
                _ => format!("    let {w} = {constant};\n"),
            }
        };
        body.push_str(&weight(1, self.sg.string(n, "weightFunction1", "CONSTANT"), &constant1, table1));
        body.push_str(&weight(2, self.sg.string(n, "weightFunction2", "CONSTANT"), &constant2, table2));
        let _ = write!(self.decls, "\nfn get_blended_{id}(original: vec4<f32>, tc: vec3<f32>, vertex: vec3<f32>) -> vec4<f32> {{\n{body}\n    return clamp(original * w1_{id} + {bc} * w2_{id}, vec4<f32>(0.0), vec4<f32>(1.0));\n}}\n");
        format!("    {target} = get_blended_{id}({target}, tc, vertex);\n")
    }
}

/// Number of lights a draw item uses and clip planes: filled in by the renderer.
pub fn texture_matrix(dimensions: [f64; 3], orientation: &Rotation4) -> Matrix4 {
    // TextureTransform3D.eventsProcessed with translation 0.5, center -0.5, scale 1 / dimensions
    // and the ProximitySensor's rotation.
    let translation = Vector3::new(0.5, 0.5, 0.5);
    let center = Vector3::new(-0.5, -0.5, -0.5);
    let scale = Vector3::new(1.0 / dimensions[0], 1.0 / dimensions[1], 1.0 / dimensions[2]);
    let mut m = Matrix4::IDENTITY;
    let mut c = center;
    c.negate();
    m.translate(&c);

    if !scale.equals(&Vector3::ONE) {
        m.scale(&scale);
    }

    if !orientation.equals(&Rotation4::identity()) {
        m.rotate(orientation);
    }

    m.translate(&center);
    m.translate(&translation);
    m
}

/// ProximitySensor.update for the volume's sensor (size 200 · |dimensions|, center 0): the
/// viewer's orientation in the volume's space while the viewer is inside the box, else none.
pub fn sensor_orientation(model_view: &Matrix4, dimensions: [f64; 3]) -> Option<Rotation4> {
    let mut inverse = *model_view;
    inverse.inverse();
    let half = 100.0 * (dimensions[0] * dimensions[0] + dimensions[1] * dimensions[1] + dimensions[2] * dimensions[2]).sqrt();
    let (x, y, z) = (inverse.0[12], inverse.0[13], inverse.0[14]);

    // ProximitySensor.containsPoint (min <= p <= max).
    if !(-half <= x && half >= x && -half <= y && half >= y && -half <= z && half >= z) {
        return None;
    }

    let mut orientation = Rotation4::identity();
    inverse.get_transform(None, Some(&mut orientation), None, None, None);
    Some(orientation)
}

fn f32x16(m: &Matrix4) -> [f32; 16] {
    m.0.map(|v| v as f32)
}

/// The volume node as a draw: the planes, their texture matrix and the fragment program of its
/// render styles. `tex` loads a texture node; `quality` is the browser's TextureQuality;
/// `orientation` the sensor's orientation (identity until the viewer is inside the sensor).
pub fn build(sg: &Sg, n: NodeId, ty: &str, model_view: &Matrix4, quality: crate::traverse::Quality, orientation: Rotation4, tex: &mut dyn FnMut(NodeId) -> Option<Rc<TextureImage>>, unsupported: &mut Vec<String>) -> VolumeDraw {
    let d = sg.vec3(n, "dimensions", Vector3::ONE);
    let dimensions = [d.x, d.y, d.z];
    let size = (d.x * d.x + d.y * d.y + d.z * d.z).sqrt();
    let mut rotation_matrix = Matrix4::IDENTITY;

    if !orientation.equals(&Rotation4::identity()) {
        rotation_matrix.rotate(&orientation);
    }

    let mut mv = rotation_matrix;
    mv.mult_right(model_view);
    let texture_matrix = texture_matrix(dimensions, &orientation);
    // set_textureTransform__: Matrix4 (...matrix) .submatrix .inverse.
    let mut normal_matrix = texture_matrix.submatrix();
    normal_matrix.inverse();

    let mut g = Gen { sg, tex, unsupported, decls: String::new(), slots: vec![], values: vec![], next_id: 0, hsv: false, plane: false };

    // Slot 0: the voxels (X3DCast (X3DTexture3DNode, voxels) ?? the default voxels).
    let voxels = volume_voxels(sg, n, ty).filter(|t| texture_dimension(sg.type_name(*t).unwrap_or("")) == Some(true)).and_then(|t| (g.tex)(t)).unwrap_or_else(default_voxels_rc);
    g.slots.push(Slot { texture: voxels, three_d: true });

    // The default OpacityMapVolumeStyle comes first in every program.
    let mut functions = g.default_opacity("texture_color");

    let styles: Vec<NodeId> = match ty {
        "VolumeData" => sg.node(n, "renderStyle").filter(|c| render_style(sg.type_name(*c).unwrap_or(""))).into_iter().collect(),
        _ => sg.nodes(n, "renderStyle").into_iter().filter(|c| composable(sg.type_name(*c).unwrap_or(""))).collect(),
    };

    match ty {
        "VolumeData" => {
            for s in &styles {
                functions.push_str(&g.style(*s, "texture_color", 0));
            }
        }
        "IsoSurfaceVolumeData" => functions.push_str(&iso_surface(&mut g, n, &styles)),
        _ => functions.push_str(&segmented(&mut g, n, &styles)),
    }

    let mut source = String::new();

    if g.hsv {
        source.push_str(HSV_WGSL);
    }

    source.push_str(&g.decls);
    let _ = write!(
        source,
        "\nfn get_texture_color(tc: vec3<f32>, vertex: vec3<f32>) -> vec4<f32> {{
    if (any(abs(tc - vec3<f32>(0.5)) > vec3<f32>(0.5))) {{
        discard;
    }}
    var texture_color = textureSampleLevel(t0, s0, tc, 0.0);
{functions}
    return texture_color;
}}\n"
    );

    let mut values = g.values;

    while values.len() % 4 != 0 || values.is_empty() {
        values.push(0.0);
    }

    let program = Program { source, dims: g.slots.iter().map(|s| s.three_d).collect(), vec4s: values.len() / 4 };
    let textures = g.slots.iter().map(|s| s.texture.clone()).collect();
    let nm = normal_matrix.0;

    VolumeDraw { model_view: f32x16(&mv), tex_matrix: f32x16(&texture_matrix), normal_matrix: nm.map(|v| v as f32), planes: num_planes(quality), size: size as f32, program: Rc::new(program), values, textures }
}

thread_local! {
    static VOXELS: std::cell::RefCell<HashMap<(u64, NodeId), (String, Option<NodeId>)>> = std::cell::RefCell::new(HashMap::new());
}

/// The `voxels` node of a volume. X_ITE's IsoSurfaceVolumeData and SegmentedVolumeData read `voxels`
/// once, in initialize () (only VolumeData listens to the field): a later assignment is ignored. The
/// first sight stands for initialize () of a parsed node; a node a Script created with `createNode`
/// (in `dynamic_nodes` and already written to) was initialized with the default voxels.
fn volume_voxels(sg: &Sg, n: NodeId, ty: &str) -> Option<NodeId> {
    if ty == "VolumeData" {
        return sg.node(n, "voxels");
    }

    VOXELS.with(|m| {
        let mut m = m.borrow_mut();

        if m.len() > 512 {
            m.clear();
        }

        let key = (sg.rt.uid, n);

        match m.get(&key) {
            Some((t, v)) if t == ty => *v,
            _ => {
                let v = if sg.rt.dynamic_nodes.contains(&n) && sg.rt.stamp(n) > 0 { None } else { sg.node(n, "voxels") };
                m.insert(key, (ty.to_string(), v));
                v
            }
        }
    })
}

/// The write stamps of what makes a style rebuild its volume's shader (X_ITE: `renderStyleNode
/// .addInterest ("updateShader", ...)`, forwarded through "addNodeEvent" by a ComposedVolumeStyle,
/// a BlendedVolumeStyle and a ShadedVolumeStyle's material).
fn style_stamp(sg: &Sg, n: NodeId, depth: usize) -> u64 {
    let mut h = sg.rt.stamp(n).wrapping_mul(0x9E3779B97F4A7C15) ^ n as u64;

    if depth > 8 {
        return h;
    }

    let children: Vec<NodeId> = match sg.type_name(n) {
        Some("ComposedVolumeStyle") => sg.nodes(n, "renderStyle"),
        Some("BlendedVolumeStyle") => sg.node(n, "renderStyle").into_iter().collect(),
        Some("ShadedVolumeStyle") => sg.node(n, "material").into_iter().collect(),
        _ => vec![],
    };

    for c in children {
        h = h.rotate_left(17) ^ style_stamp(sg, c, depth + 1);
    }

    h
}

struct IsoSnapshot {
    step: f64,
    surface_values: Vec<f32>,
    tolerance: f64,
    styles: Vec<(NodeId, u64)>,
    gradients: Option<NodeId>,
}

thread_local! {
    static ISO_SNAPSHOTS: std::cell::RefCell<HashMap<(u64, NodeId), IsoSnapshot>> = std::cell::RefCell::new(HashMap::new());
}

/// The `gradients` node the IsoSurfaceVolumeData's shader was last built with.
fn iso_gradients(sg: &Sg, n: NodeId, styles: &[NodeId], surface_values: &[f64]) -> Option<NodeId> {
    let current = IsoSnapshot {
        step: sg.f64(n, "contourStepSize", 0.0),
        surface_values: surface_values.iter().map(|v| *v as f32).collect(),
        tolerance: sg.f64(n, "surfaceTolerance", 0.0),
        styles: styles.iter().map(|s| (*s, style_stamp(sg, *s, 0))).collect(),
        gradients: sg.node(n, "gradients"),
    };

    ISO_SNAPSHOTS.with(|m| {
        let mut m = m.borrow_mut();

        if m.len() > 512 {
            m.clear();
        }

        let key = (sg.rt.uid, n);
        let rebuild = m.get(&key).is_none_or(|s| s.step != current.step || s.surface_values != current.surface_values || s.tolerance != current.tolerance || s.styles != current.styles);

        if rebuild {
            let gradients = current.gradients;
            m.insert(key, current);
            gradients
        } else {
            m[&key].gradients
        }
    })
}

/// IsoSurfaceVolumeData.createShader: the contour tests after the default opacity map.
fn iso_surface(g: &mut Gen, n: NodeId, styles: &[NodeId]) -> String {
    let sg = g.sg;
    let surface_values: Vec<f64> = sg.f32s(n, "surfaceValues").iter().map(|v| *v as f64).collect();
    let tolerance = g.scalar(sg.f64(n, "surfaceTolerance", 0.0));
    // The styles' declarations (each style's getUniformsText), then the statements of each branch.
    let mut s = String::new();
    s.push_str("\n    // IsoSurfaceVolumeData\n\n");

    // IsoSurfaceVolumeData.set_gradients__ does not rebuild the shader (only contourStepSize,
    // surfaceValues, surfaceTolerance and renderStyle do): a gradients texture changed at run time is
    // used from the next rebuild on.
    let gradients = iso_gradients(sg, n, styles, &surface_values).filter(|t| texture_dimension(sg.type_name(*t).unwrap_or("")) == Some(true)).and_then(|t| (g.tex)(t));

    match gradients.and_then(|t| g.slot(t, true)) {
        Some(slot) => {
            let _ = writeln!(s, "    if (length({}.xyz * 2.0 - vec3<f32>(1.0)) < {tolerance}) {{\n        discard;\n    }}", g.sample3(slot, "tc"));
        }
        None => {
            // getNormal (texCoord) of the voxels.
            let id = g.id();
            g.normal_fn(id, None, 0);
            let _ = writeln!(s, "    if (get_normal_{id}(tc).w < {tolerance}) {{\n        discard;\n    }}");
        }
    }

    s.push_str("\n    let intensity = texture_color.r;\n\n");

    // The functions text of a style at a branch: declared once (its uniforms), called per branch.
    // Each style's code is generated for the first use only, as X_ITE emits one function per node.
    let mut generated: HashMap<usize, String> = HashMap::new();
    let mut branch = |g: &mut Gen, i: usize| -> String {
        generated.entry(i).or_insert_with(|| g.style(styles[i], "texture_color", 0)).clone()
    };
    // getUniformsText of every style is part of the program, used or not.
    for i in 0..styles.len() {
        branch(g, i);
    }

    if surface_values.len() == 1 {
        let step = (sg.f64(n, "contourStepSize", 0.0)).abs();

        if step == 0.0 {
            let sv = g.scalar(surface_values[0]);
            let _ = writeln!(s, "    if (intensity > {sv}) {{\n        texture_color = vec4<f32>(vec3<f32>({sv}), 1.0);");

            if !styles.is_empty() {
                s.push_str(&branch(g, 0));
            }

            s.push_str("    } else {\n        discard;\n    }\n\n");
        } else {
            // The contour values: only the lowest one is tested (`for (i = surfaceValues .length - 1; ...)`
            // runs once for a single surfaceValues entry).
            let mut values: Vec<f64> = vec![];
            let mut v = surface_values[0] - step;

            while v > 0.0 {
                values.insert(0, v);
                v -= step;
            }

            values.push(surface_values[0]);
            let mut v = surface_values[0] + step;

            while v < 1.0 {
                values.push(v);
                v += step;
            }

            let sv = g.scalar(values[0]);
            let _ = writeln!(s, "    if (false) {{\n    }} else if (intensity > {sv}) {{\n        texture_color = vec4<f32>(vec3<f32>({sv}), 1.0);");

            if !styles.is_empty() {
                s.push_str(&branch(g, 0));
            }

            s.push_str("    } else {\n        discard;\n    }\n\n");
        }
    } else {
        let svs: Vec<String> = surface_values.iter().map(|v| g.scalar(*v)).collect();
        s.push_str("    if (false) {\n    }");

        for i in (0..surface_values.len()).rev() {
            let _ = writeln!(s, " else if (intensity > {}) {{\n        texture_color = vec4<f32>(vec3<f32>({}), 1.0);", svs[i], svs[i]);

            if !styles.is_empty() {
                let r = i.min(styles.len() - 1);
                s.push_str(&branch(g, r));
            }

            s.push_str("    }");
        }

        s.push_str(" else {\n        discard;\n    }\n\n");
    }

    s
}

/// SegmentedVolumeData.createShader: a switch over the segment of the voxel.
fn segmented(g: &mut Gen, n: NodeId, styles: &[NodeId]) -> String {
    let sg = g.sg;
    let mut s = String::new();
    let ids = sg.node(n, "segmentIdentifiers").filter(|t| texture_dimension(sg.type_name(*t).unwrap_or("")) == Some(true)).and_then(|t| (g.tex)(t));

    match ids.and_then(|t| g.slot(t, true)) {
        Some(slot) => {
            let _ = writeln!(s, "\n    let segment = i32({}.r * 255.0);", g.sample3(slot, "tc"));
        }
        None => s.push_str("    let segment = 0;\n"),
    }

    if !styles.is_empty() {
        let enabled: Vec<bool> = match sg.value(n, "segmentEnabled") {
            Some(x3d_fields::FieldValue::MFBool(v)) => v.clone(),
            _ => vec![],
        };
        s.push_str("\n    switch (segment) {\n");

        for (i, style) in styles.iter().enumerate() {
            let _ = writeln!(s, "        case {i}: {{");

            if enabled.get(i).copied().unwrap_or(true) {
                s.push_str(&g.style(*style, "texture_color", 0));
            } else {
                s.push_str("            discard;\n");
            }

            s.push_str("        }\n");
        }

        s.push_str("        default: {}\n    }\n");
    }

    s
}

const HSV_WGSL: &str = "
fn rgb2hsv(color: vec3<f32>) -> vec3<f32> {
    var h = 0.0;
    var s = 0.0;
    let lo = min(min(color.r, color.g), color.b);
    let hi = max(max(color.r, color.g), color.b);
    let v = hi;
    let delta = hi - lo;

    if (hi != 0.0 && delta != 0.0) {
        s = delta / hi;

        if (color.r == hi) {
            h = (color.g - color.b) / delta;
        } else if (color.g == hi) {
            h = 2.0 + (color.b - color.r) / delta;
        } else {
            h = 4.0 + (color.r - color.g) / delta;
        }

        h = h * (M_PI / 3.0);

        if (h < 0.0) {
            h = h + M_PI * 2.0;
        }
    } else {
        s = 0.0;
        h = 0.0;
    }

    return vec3<f32>(h, s, v);
}

fn hsv2rgb(hsv: vec3<f32>) -> vec3<f32> {
    let h = hsv[0];
    let s = clamp(hsv[1], 0.0, 1.0);
    let v = clamp(hsv[2], 0.0, 1.0);

    if (s == 0.0) {
        return vec3<f32>(v, v, v);
    }

    let w = (h * (180.0 / M_PI)) / 60.0;
    let i = floor(w);
    let f = w - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    switch (i32(i) % 6) {
        case 0: { return vec3<f32>(v, t, p); }
        case 1: { return vec3<f32>(q, v, p); }
        case 2: { return vec3<f32>(p, v, t); }
        case 3: { return vec3<f32>(p, q, v); }
        case 4: { return vec3<f32>(t, p, v); }
        default: { return vec3<f32>(v, p, q); }
    }
}

fn mix_hsv(a: vec3<f32>, b: vec3<f32>, t: f32) -> vec3<f32> {
    var ha = a[0];
    let sa = a[1];
    let va = a[2];
    var hb = b[0];
    let sb = b[1];
    let vb = b[2];

    if (sa == 0.0) {
        ha = hb;
    }

    if (sb == 0.0) {
        hb = ha;
    }

    let range = abs(hb - ha);

    if (range <= M_PI) {
        let h = ha + t * (hb - ha);
        let s = sa + t * (sb - sa);
        let v = va + t * (vb - va);
        return vec3<f32>(h, s, v);
    }

    let PI2 = M_PI * 2.0;
    let step_ = (PI2 - range) * t;
    var h = ha + step_;

    if (ha < hb) {
        h = ha - step_;
    }

    if (h < 0.0) {
        h = h + PI2;
    } else if (h > PI2) {
        h = h - PI2;
    }

    let s = sa + t * (sb - sa);
    let v = va + t * (vb - va);
    return vec3<f32>(h, s, v);
}
";

pub const VOLUME_WGSL: &str = include_str!("volume.wgsl");
