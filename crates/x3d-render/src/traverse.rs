//! Scene traversal: the CAMERA pass that finds the bound bindables (X3DLayerNode.bindBindables,
//! BindableList.getBound) and the DISPLAY pass that collects shapes and lights
//! (X3DLayerNode.display, X3DGroupingNode.traverse, X3DTransformMatrix3DNode, Switch, LOD,
//! Billboard, Inline, X3DShapeNode, X3DRenderObject.addDisplayShape, X3DLightNode.push).

use crate::fog::{self, FogParams};
use crate::geometry::{self, Colors, ElevationInput, ExtrusionInput, Geometry, IfsInput};
use crate::layer::{self, LayerKey, LayoutCamera, LayoutNode, LayoutStack};
use crate::sg::Sg;
use crate::text;
use crate::texture::{self, TextureImage};
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;
use x3d_fields::math::{Matrix3, Matrix4, Rotation4, Vector2, Vector3, Vector4};
use x3d_fields::{FieldValue, NodeId};
use x3d_runtime::particles::GeometryType;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shading {
    /// A legacy Material (specification version <= 3.3) with normals: getMaterialKey follows the
    /// Shading option, X3D_GOURAUD_MATERIAL by default, X3D_PHONG_MATERIAL with PHONG.
    Gouraud,
    /// Material (X3D 4) and TwoSidedMaterial with normals: X3D_PHONG_MATERIAL (getMaterialKey 2).
    Phong,
    /// UnlitMaterial / no material, or a Material on points and lines (X3D_UNLIT_MATERIAL).
    Unlit,
}

/// AlphaMode.js: Appearance.alphaMode. A [`DrawItem`] holds the shape's effective mode, never `Auto`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlphaMode {
    #[default]
    Auto,
    /// X3D_ALPHA_MODE_OPAQUE: Fragment2.glsl sets finalColor .a to 1.
    Opaque,
    /// X3D_ALPHA_MODE_MASK: Fragment2.glsl discards a fragment with finalColor .a < x3d_AlphaCutoff, then sets
    /// finalColor .a to 1.
    Mask,
    /// X3D_ALPHA_MODE_BLEND: the alpha is kept, and the shape is drawn with the transparent shapes.
    Blend,
}

impl AlphaMode {
    /// Appearance.set_alphaMode__: `AlphaMode .get (alphaMode) ?? AlphaMode .AUTO`, an exact, case-sensitive name.
    pub fn from_name(name: &str) -> Self {
        match name {
            "OPAQUE" => Self::Opaque,
            "MASK" => Self::Mask,
            "BLEND" => Self::Blend,
            _ => Self::Auto,
        }
    }

    /// X3DShapeNode.set_transparent__ (and ParticleSystem.set_transparent__): `auto_transparent` is the
    /// appearance's or the geometry's transparency. AUTO is transparent with it and draws as BLEND, else as
    /// OPAQUE; any other mode is itself, and only BLEND is transparent. Returns (transparent, effective mode).
    pub fn resolve(self, auto_transparent: bool) -> (bool, Self) {
        match self {
            Self::Auto if auto_transparent => (true, Self::Blend),
            Self::Auto => (false, Self::Opaque),
            mode => (mode == Self::Blend, mode),
        }
    }

    /// Appearance.set_alphaCutoff__: the alphaCutoff field for MASK, else 0.
    pub fn cutoff(self, alpha_cutoff: f64) -> f64 {
        if self == Self::Mask {
            alpha_cutoff
        } else {
            0.0
        }
    }
}

#[derive(Clone, Debug)]
pub struct MaterialParams {
    pub shading: Shading,
    /// diffuseColor; PhysicalMaterial baseColor.
    pub diffuse: [f64; 3],
    pub ambient_intensity: f64,
    pub specular: [f64; 3],
    pub shininess: f64,
    pub emissive: [f64; 3],
    pub transparency: f64,
    /// The physical shaders: 0 none, 1 PhysicalMaterial (Physical2.fs, X3D_MATERIAL_METALLIC_ROUGHNESS),
    /// 2 SpecularGlossinessMaterial (SpecularGlossiness2.fs).
    pub pbr: u32,
    /// metallic (PhysicalMaterial) or glossiness (SpecularGlossinessMaterial).
    pub metallic: f64,
    pub roughness: f64,
    pub normal_scale: f64,
    pub occlusion_strength: f64,
    /// X3D_MATERIAL_TEXTURES of the material (see [`MaterialTexture`]).
    pub textures: Vec<MaterialTexture>,
    /// PhysicalMaterial.extensions (X_ITE, experimental), `None` without one.
    pub extensions: Option<Rc<MaterialExtensions>>,
}

/// The shader options of the X3DMaterialExtensionNodes (X3D_<NAME>_MATERIAL_EXT).
/// SpotLight's declared defaults, as X3D writes them and X_ITE ships them
/// (`new Fields .SFFloat (0.785398)`, `(1.5708)`). They are *truncated* decimals, not
/// `FRAC_PI_4` and `FRAC_PI_2`: a scene that omits the field has to light exactly as X_ITE
/// lights it, and the cone falloff is sensitive to the last bits. The clamps alongside them
/// do use `std::f64::consts::FRAC_PI_2`, which is the real bound and not a default.
#[allow(
    clippy::approx_constant,
    reason = "X3D's declared SpotLight defaults, truncated as the spec writes them"
)]
const SPOT_CUT_OFF_ANGLE_DEFAULT: f64 = 0.785398;
#[allow(
    clippy::approx_constant,
    reason = "X3D's declared SpotLight defaults, truncated as the spec writes them"
)]
const SPOT_BEAM_WIDTH_DEFAULT: f64 = 1.5708;

pub const EXT_EMISSIVE_STRENGTH: u32 = 1;
pub const EXT_IOR: u32 = 2;
pub const EXT_SPECULAR: u32 = 4;
pub const EXT_CLEARCOAT: u32 = 8;
pub const EXT_SHEEN: u32 = 16;
pub const EXT_IRIDESCENCE: u32 = 32;
pub const EXT_ANISOTROPY: u32 = 64;
pub const EXT_TRANSMISSION: u32 = 128;
pub const EXT_VOLUME: u32 = 256;
pub const EXT_DIFFUSE_TRANSMISSION: u32 = 512;
pub const EXT_DISPERSION: u32 = 1024;
pub const EXT_RETROREFLECTION: u32 = 2048;
pub const EXT_VOLUME_SCATTER: u32 = 4096;

/// The uniforms the X3DMaterialExtensionNodes of a PhysicalMaterial set (their set_*__ clamps).
/// A later node of the same type sets its uniforms after an earlier one (extensionNodes sorted by
/// extension key, stable), so it wins.
#[derive(Clone, Debug)]
pub struct MaterialExtensions {
    pub flags: u32,
    pub emissive_strength: f64,
    pub ior: f64,
    pub specular: f64,
    pub specular_color: [f64; 3],
    pub specular_strength: f64,
    pub clearcoat: f64,
    pub clearcoat_roughness: f64,
    pub sheen_color: [f64; 3],
    pub sheen_roughness: f64,
    pub iridescence: f64,
    pub iridescence_ior: f64,
    pub iridescence_thickness: [f64; 2],
    /// cos (anisotropyRotation), sin (anisotropyRotation), anisotropyStrength.
    pub anisotropy: [f64; 3],
    pub transmission: f64,
    pub thickness: f64,
    pub attenuation_distance: f64,
    pub attenuation_color: [f64; 3],
    pub diffuse_transmission: f64,
    pub diffuse_transmission_color: [f64; 3],
    /// DispersionMaterialExtension.set_dispersion__: max (dispersion, 0).
    pub dispersion: f64,
    /// RetroreflectionMaterialExtension.set_retroreflection__: clamp (retroreflection, 0, 1).
    pub retroreflection: f64,
    /// VolumeScatterMaterialExtension.set_multiscatterColor__ (scatterStrength and scatterAnisotropy
    /// are clamped by X_ITE but no shader reads them).
    pub multiscatter_color: [f64; 3],
}

impl Default for MaterialExtensions {
    fn default() -> Self {
        MaterialExtensions {
            flags: 0,
            emissive_strength: 1.0,
            ior: 1.5,
            specular: 1.0,
            specular_color: [1.0; 3],
            specular_strength: 1.0,
            clearcoat: 0.0,
            clearcoat_roughness: 0.0,
            sheen_color: [0.0; 3],
            sheen_roughness: 0.0,
            iridescence: 0.0,
            iridescence_ior: 1.3,
            iridescence_thickness: [100.0, 400.0],
            anisotropy: [1.0, 0.0, 0.0],
            transmission: 0.0,
            thickness: 0.0,
            attenuation_distance: 1_000_000.0,
            attenuation_color: [1.0; 3],
            diffuse_transmission: 0.0,
            diffuse_transmission_color: [1.0; 3],
            dispersion: 0.0,
            retroreflection: 0.0,
            multiscatter_color: [0.0; 3],
        }
    }
}

/// The material texture kinds of the extensions (after the five of MaterialTexture), with the
/// extension type and the texture field: specular (a), specularColor (rgb), clearcoat (r),
/// clearcoatRoughness (g), clearcoatNormal (rgb), sheenColor (rgb), sheenRoughness (a),
/// iridescence (r), iridescenceThickness (g), anisotropy (rgb), transmission (r), thickness (g),
/// diffuseTransmission (a), diffuseTransmissionColor (rgb), retroreflection (r),
/// multiscatterColor (rgb; scatterStrengthTexture is read by no shader).
pub const EXTENSION_TEXTURES: [(&str, &str); 16] = [
    ("SpecularMaterialExtension", "specularTexture"),
    ("SpecularMaterialExtension", "specularColorTexture"),
    ("ClearcoatMaterialExtension", "clearcoatTexture"),
    ("ClearcoatMaterialExtension", "clearcoatRoughnessTexture"),
    ("ClearcoatMaterialExtension", "clearcoatNormalTexture"),
    ("SheenMaterialExtension", "sheenColorTexture"),
    ("SheenMaterialExtension", "sheenRoughnessTexture"),
    ("IridescenceMaterialExtension", "iridescenceTexture"),
    (
        "IridescenceMaterialExtension",
        "iridescenceThicknessTexture",
    ),
    ("AnisotropyMaterialExtension", "anisotropyTexture"),
    ("TransmissionMaterialExtension", "transmissionTexture"),
    ("VolumeMaterialExtension", "thicknessTexture"),
    (
        "DiffuseTransmissionMaterialExtension",
        "diffuseTransmissionTexture",
    ),
    (
        "DiffuseTransmissionMaterialExtension",
        "diffuseTransmissionColorTexture",
    ),
    ("RetroreflectionMaterialExtension", "retroreflectionTexture"),
    ("VolumeScatterMaterialExtension", "multiscatterColorTexture"),
];

/// The X3DMaterialExtensionNodes this renderer draws.
const MATERIAL_EXTENSIONS: [&str; 13] = [
    "EmissiveStrengthMaterialExtension",
    "IORMaterialExtension",
    "SpecularMaterialExtension",
    "ClearcoatMaterialExtension",
    "SheenMaterialExtension",
    "IridescenceMaterialExtension",
    "AnisotropyMaterialExtension",
    "TransmissionMaterialExtension",
    "VolumeMaterialExtension",
    "DiffuseTransmissionMaterialExtension",
    "DispersionMaterialExtension",
    "RetroreflectionMaterialExtension",
    "VolumeScatterMaterialExtension",
];

/// PhysicalMaterial.set_extensions__: X3DCast (X3DMaterialExtensionNode) of each node.
fn extension_nodes(sg: &Sg, m: NodeId) -> Vec<NodeId> {
    if sg.type_name(m) != Some("PhysicalMaterial") {
        return vec![];
    }

    sg.nodes(m, "extensions")
        .into_iter()
        .filter(|e| {
            sg.type_name(*e)
                .is_some_and(|t| t.ends_with("MaterialExtension"))
        })
        .collect()
}

/// The setShaderUniforms values of the extensions of `m`.
fn material_extensions(sg: &Sg, m: NodeId) -> Option<Rc<MaterialExtensions>> {
    let nodes = extension_nodes(sg, m);

    if nodes.is_empty() {
        return None;
    }

    let mut x = MaterialExtensions::default();

    for e in nodes {
        match sg.type_name(e).unwrap_or("") {
            "EmissiveStrengthMaterialExtension" => {
                x.flags |= EXT_EMISSIVE_STRENGTH;
                x.emissive_strength = sg.f64(e, "emissiveStrength", 1.0).max(0.0);
            }
            "IORMaterialExtension" => {
                x.flags |= EXT_IOR;
                x.ior = sg.f64(e, "indexOfRefraction", 1.5).max(1.0);
            }
            "SpecularMaterialExtension" => {
                x.flags |= EXT_SPECULAR;
                x.specular = sg.f64(e, "specular", 1.0).clamp(0.0, 1.0);
                x.specular_color = sg.color(e, "specularColor", [1.0; 3]);
                x.specular_strength = sg.f64(e, "specularStrength", 1.0).max(0.0);
            }
            "ClearcoatMaterialExtension" => {
                x.flags |= EXT_CLEARCOAT;
                x.clearcoat = sg.f64(e, "clearcoat", 0.0).clamp(0.0, 1.0);
                x.clearcoat_roughness = sg.f64(e, "clearcoatRoughness", 0.0).clamp(0.0, 1.0);
            }
            "SheenMaterialExtension" => {
                x.flags |= EXT_SHEEN;
                x.sheen_color = sg.color(e, "sheenColor", [0.0; 3]);
                x.sheen_roughness = sg.f64(e, "sheenRoughness", 0.0).clamp(0.0, 1.0);
            }
            "IridescenceMaterialExtension" => {
                x.flags |= EXT_IRIDESCENCE;
                x.iridescence = sg.f64(e, "iridescence", 0.0).clamp(0.0, 1.0);
                x.iridescence_ior = sg.f64(e, "iridescenceIndexOfRefraction", 1.3).max(1.0);
                x.iridescence_thickness = [
                    sg.f64(e, "iridescenceThicknessMinimum", 100.0).max(0.0),
                    sg.f64(e, "iridescenceThicknessMaximum", 400.0).max(0.0),
                ];
            }
            "AnisotropyMaterialExtension" => {
                x.flags |= EXT_ANISOTROPY;
                let rotation = sg.f64(e, "anisotropyRotation", 0.0);
                x.anisotropy = [
                    rotation.cos(),
                    rotation.sin(),
                    sg.f64(e, "anisotropyStrength", 0.0).clamp(0.0, 1.0),
                ];
            }
            "TransmissionMaterialExtension" => {
                x.flags |= EXT_TRANSMISSION;
                x.transmission = sg.f64(e, "transmission", 0.0).clamp(0.0, 1.0);
            }
            "VolumeMaterialExtension" => {
                x.flags |= EXT_VOLUME;
                x.thickness = sg.f64(e, "thickness", 0.0).max(0.0);
                // set_attenuationDistance__: 0 means 1e6 (infinite), negative values 0.
                let d = sg.f64(e, "attenuationDistance", 1_000_000.0);
                x.attenuation_distance = if d != 0.0 { d.max(0.0) } else { 1_000_000.0 };
                x.attenuation_color = sg.color(e, "attenuationColor", [1.0; 3]);
            }
            "DiffuseTransmissionMaterialExtension" => {
                x.flags |= EXT_DIFFUSE_TRANSMISSION;
                x.diffuse_transmission = sg.f64(e, "diffuseTransmission", 0.0).clamp(0.0, 1.0);
                x.diffuse_transmission_color = sg.color(e, "diffuseTransmissionColor", [1.0; 3]);
            }
            "DispersionMaterialExtension" => {
                x.flags |= EXT_DISPERSION;
                x.dispersion = sg.f64(e, "dispersion", 0.0).max(0.0);
            }
            "RetroreflectionMaterialExtension" => {
                x.flags |= EXT_RETROREFLECTION;
                x.retroreflection = sg.f64(e, "retroreflection", 0.0).clamp(0.0, 1.0);
            }
            "VolumeScatterMaterialExtension" => {
                x.flags |= EXT_VOLUME_SCATTER;
                x.multiscatter_color = sg.color(e, "multiscatterColor", [0.0; 3]);
            }
            _ => {}
        }
    }

    Some(Rc::new(x))
}

/// One named material texture (MaterialTextures.js): `kind` 0 base (diffuse), 1 emissive,
/// 2 metallic-roughness (specular-glossiness), 3 occlusion, 4 normal; the texture transform and
/// coordinate channel its mapping names (setNamedShaderUniforms, 0 when the mapping is unknown).
#[derive(Clone, Debug)]
pub struct MaterialTexture {
    pub kind: u32,
    pub texture: Rc<TextureImage>,
    pub transform: u32,
    pub coord: u32,
}

/// The first EnvironmentLight (getMaxEnvironmentLights 1): X3D_USE_IBL.
#[derive(Clone, Debug)]
pub struct EnvironmentParams {
    pub color: [f64; 3],
    pub intensity: f64,
    /// EnvironmentLightContainer.setGlobalVariables rotationMatrix (mat3, GLSL column order).
    pub rotation: [f32; 9],
    /// The specularTexture the diffuse and specular maps are filtered from.
    pub specular: Rc<TextureImage>,
}

/// BlendMode factor names (BlendMode.initialize factorTypes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlendFactor {
    Zero,
    One,
    SrcColor,
    OneMinusSrcColor,
    DstColor,
    OneMinusDstColor,
    SrcAlpha,
    OneMinusSrcAlpha,
    DstAlpha,
    OneMinusDstAlpha,
    SrcAlphaSaturate,
    ConstantColor,
    OneMinusConstantColor,
    ConstantAlpha,
    OneMinusConstantAlpha,
}

impl BlendFactor {
    fn from_name(name: &str) -> Option<BlendFactor> {
        use BlendFactor::*;

        Some(match name {
            "ZERO" => Zero,
            "ONE" => One,
            "SRC_COLOR" => SrcColor,
            "ONE_MINUS_SRC_COLOR" => OneMinusSrcColor,
            "DST_COLOR" => DstColor,
            "ONE_MINUS_DST_COLOR" => OneMinusDstColor,
            "SRC_ALPHA" => SrcAlpha,
            "ONE_MINUS_SRC_ALPHA" => OneMinusSrcAlpha,
            "DST_ALPHA" => DstAlpha,
            "ONE_MINUS_DST_ALPHA" => OneMinusDstAlpha,
            "SRC_ALPHA_SATURATE" => SrcAlphaSaturate,
            "CONSTANT_COLOR" => ConstantColor,
            "ONE_MINUS_CONSTANT_COLOR" => OneMinusConstantColor,
            "CONSTANT_ALPHA" => ConstantAlpha,
            "ONE_MINUS_CONSTANT_ALPHA" => OneMinusConstantAlpha,
            _ => return None,
        })
    }
}

/// BlendMode equation names (BlendMode.initialize equationTypes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlendEquation {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
}

impl BlendEquation {
    fn from_name(name: &str) -> Option<BlendEquation> {
        use BlendEquation::*;

        Some(match name {
            "FUNC_ADD" => Add,
            "FUNC_SUBTRACT" => Subtract,
            "FUNC_REVERSE_SUBTRACT" => ReverseSubtract,
            "MIN" => Min,
            "MAX" => Max,
            _ => return None,
        })
    }
}

/// BlendMode.enable: gl.blendColor, gl.blendFuncSeparate (srcRGB, dstRGB, srcAlpha, dstAlpha),
/// gl.blendEquationSeparate (rgb, alpha).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlendParams {
    pub color: [f64; 4],
    pub source_color: BlendFactor,
    pub source_alpha: BlendFactor,
    pub destination_color: BlendFactor,
    pub destination_alpha: BlendFactor,
    pub color_equation: BlendEquation,
    pub alpha_equation: BlendEquation,
}

/// DepthMode depthFunction names (set_depthFunction__, default LEQUAL).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DepthFunc {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

/// DepthMode.enable: polygon offset fill, depth test and function, depth range, depth mask.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthParams {
    pub test: bool,
    pub func: DepthFunc,
    pub mask: bool,
    /// gl.polygonOffset (factor, units).
    pub polygon_offset: [f64; 2],
    /// gl.depthRange (near, far), clamped to [0, 1] like GL.
    pub range: [f64; 2],
}

/// PointProperties uniforms (set_pointSizeScaleFactor__ … set_attenuation__, ContentScale 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointParams {
    pub scale_factor: f64,
    pub min_value: f64,
    pub max_value: f64,
    pub attenuation: [f64; 3],
}

/// LineProperties with `applied` TRUE (Appearance.set_applied__).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineParams {
    /// 1 … 16 (set_linetype__).
    pub linetype: i32,
    /// max (1, linewidthScaleFactor) × ContentScale.
    pub width: f64,
}

/// FillProperties uniforms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FillParams {
    pub filled: bool,
    pub hatched: bool,
    /// 1 … 19 (set_hatchStyle__).
    pub hatch_style: i32,
    pub hatch_color: [f64; 3],
}

/// Appearance.getStyleProperties (geometryType): X3D_STYLE_PROPERTIES of the shader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StyleParams {
    Point(PointParams),
    Line(LineParams),
    Fill(FillParams),
}

/// ALIASED_POINT_SIZE_RANGE [1] of the oracle GPU (ANGLE on OpenGL, NVIDIA).
pub const MAX_POINT_SIZE: f64 = 2047.0;

#[derive(Clone, Debug)]
pub struct DrawItem {
    pub geometry_node: NodeId,
    pub geometry: Rc<Geometry>,
    pub model_view: Matrix4,
    pub material: MaterialParams,
    /// The X3D_TEXTURE units: the appearance's X3DSingleTextureNode, or a MultiTexture's textures
    /// (at most maxTextures).
    pub textures: Vec<Rc<TextureImage>>,
    /// The MultiTexture parameters (X3D_MULTI_TEXTURING), `None` for a single texture.
    pub multi_texture: Option<MultiTextureParams>,
    /// x3d_TextureMatrix [X3D_NUM_TEXTURE_TRANSFORMS] (column-major for GLSL); at least one.
    pub texture_matrices: Vec<[f32; 16]>,
    /// The shape's texture projectors, indices into [`Frame::projectors`]: global, then the
    /// local ones of the enclosing grouping nodes, at most [`MAX_TEXTURES`].
    pub projectors: Vec<u32>,
    /// The shape's lights, indices into [`Frame::lights`]: the layer's global lights, then the
    /// local lights (`global FALSE`) of the enclosing grouping nodes, at most [`MAX_LIGHTS`]
    /// (X3DMaterialNode objectsKeys: getGlobalLightsKeys () .concat (localObjectsKeys)).
    pub lights: Vec<u32>,
    pub transparent: bool,
    /// The shape's effective alpha mode ([`AlphaMode::resolve`]): `Opaque`, `Mask` or `Blend`.
    pub alpha_mode: AlphaMode,
    /// x3d_AlphaCutoff ([`AlphaMode::cutoff`]): 0 unless `alpha_mode` is `Mask`.
    pub alpha_cutoff: f32,
    /// The back face material (Appearance.backMaterial, or a TwoSidedMaterial's back side):
    /// X3DGeometryNode.display draws a non-solid geometry twice, back faces with this material
    /// first, then front faces with `material`.
    pub back_material: Option<MaterialParams>,
    pub blend: Option<BlendParams>,
    pub depth: Option<DepthParams>,
    /// The enabled ClipPlanes in scope (view-space normal and distance, X3D_NUM_CLIP_PLANES at
    /// most maxClipPlanes), outermost first.
    pub clip_planes: Vec<[f64; 4]>,
    pub style: Option<StyleParams>,
    /// The hatch style or linetype texture the style uses.
    pub style_texture: Option<Rc<TextureImage>>,
    /// Sort key for transparent shapes: bbox center z in view space.
    pub distance: f64,
    /// The rectangle of the Viewport nodes around the shape (X3DViewportNode.push: the shape's
    /// gl.viewport and scissor); `None` for the layer's.
    pub viewport: Option<[f64; 4]>,
    /// A VolumeData node (VolumeRendering): the renderer draws its planes instead of `geometry`.
    pub volume: Option<Rc<crate::volume::VolumeDraw>>,
    /// An InstancedShape: the instances drawn with the shape's model-view matrix (`None`: one draw).
    pub instances: Option<Rc<InstanceData>>,
    /// A GaussianSplats node (component GaussianSplats): the renderer draws its splats instead of `geometry`.
    pub splats: Option<Rc<crate::splats::SplatDraw>>,
    /// The Appearance selected a valid ComposedShader (Batch 08): the renderer draws with its program.
    pub custom: Option<Rc<crate::shader_gpu::CustomDraw>>,
    /// The innermost LocalFog in scope (renderContext .fogNode); `None` uses the layer's Fog.
    pub fog: Option<FogParams>,
}

/// InstancedShape.set_matrices__: the matrix and the normal matrix (`submatrix .transpose () .inverse ()`,
/// column-major for GLSL) of every instance; `numInstances` is the longest of the five fields.
#[derive(Debug)]
pub struct InstanceData {
    pub matrices: Vec<Matrix4>,
    pub normals: Vec<[f32; 9]>,
    /// A ParticleSystem with a color ramp: `x3d_Color` of every instance (empty: the geometry's colors).
    pub colors: Vec<[f32; 4]>,
    /// A ParticleSystem quad with a texCoord ramp (`X3D_TEX_COORD_RAMP`): the ramp texture coordinates of the
    /// four quad corners of every instance (empty: the geometry's).
    pub tex_coords: Vec<[[f32; 4]; 4]>,
    /// The instances are the particles of a ParticleSystem (an InstancedShape otherwise).
    pub particle: bool,
}

/// InstancedShape.set_matrices__ of `n`: the last value of a shorter field repeats, an empty field
/// leaves its part of `Matrix4 .setTransform` out.
pub fn instance_data(sg: &Sg, n: NodeId) -> InstanceData {
    let t = sg.f64s(n, "translations");
    let r = sg.f64s(n, "rotations");
    let s = sg.f64s(n, "scales");
    let so = sg.f64s(n, "scaleOrientations");
    let c = sg.f64s(n, "centers");
    let (nt, nr, ns, nso, nc) = (
        t.len() / 3,
        r.len() / 4,
        s.len() / 3,
        so.len() / 4,
        c.len() / 3,
    );
    let count = nt.max(nr).max(ns).max(nso).max(nc);
    let vec3 = |v: &[f64], len: usize, i: usize| {
        (len > 0).then(|| {
            let k = i.min(len - 1) * 3;
            Vector3::new(v[k], v[k + 1], v[k + 2])
        })
    };
    let rot = |v: &[f64], len: usize, i: usize| {
        (len > 0).then(|| {
            let k = i.min(len - 1) * 4;
            Rotation4::new(v[k], v[k + 1], v[k + 2], v[k + 3])
        })
    };
    let mut matrices = Vec::with_capacity(count);
    let mut normals = Vec::with_capacity(count);

    for i in 0..count {
        let mut m = Matrix4::IDENTITY;
        m.set_transform(
            vec3(&t, nt, i).as_ref(),
            rot(&r, nr, i).as_ref(),
            vec3(&s, ns, i).as_ref(),
            rot(&so, nso, i).as_ref(),
            vec3(&c, nc, i).as_ref(),
        );
        let mut normal = m.submatrix();
        normal.transpose();
        normal.inverse();
        normals.push(normal.0.map(|v| v as f32));
        matrices.push(m);
    }

    InstanceData {
        matrices,
        normals,
        colors: vec![],
        tex_coords: vec![],
        particle: false,
    }
}

impl DrawItem {
    /// Appearance.isVolumeScatter: the front or back material has a VolumeScatterMaterialExtension
    /// (X3DShapeNode.set_volumeScatter__: the only shapes of the volume scatter pass).
    pub fn volume_scatter(&self) -> bool {
        let t = |m: &MaterialParams| {
            m.extensions
                .as_ref()
                .is_some_and(|x| x.flags & EXT_VOLUME_SCATTER != 0)
        };
        t(&self.material) || self.back_material.as_ref().is_some_and(t)
    }

    /// Appearance.isTransmission: the front or back material has a TransmissionMaterialExtension
    /// (X3DShapeNode.set_transmission__: not drawn in the transmission pass).
    pub fn transmission(&self) -> bool {
        let t = |m: &MaterialParams| {
            m.extensions
                .as_ref()
                .is_some_and(|x| x.flags & EXT_TRANSMISSION != 0)
        };
        t(&self.material) || self.back_material.as_ref().is_some_and(t)
    }
}

/// MultiTexture.setShaderUniforms: x3d_MultiTextureColor and per unit mode, alphaMode, source and
/// function (ModeType, SourceType, FunctionType numbers).
#[derive(Clone, Debug)]
pub struct MultiTextureParams {
    pub color: [f64; 4],
    pub units: Vec<[u32; 4]>,
}

#[derive(Clone, Debug)]
pub struct LightParams {
    /// 1 directional, 2 point, 3 spot (x3d_DirectionalLight …).
    pub kind: u32,
    pub color: [f64; 3],
    pub intensity: f64,
    pub ambient_intensity: f64,
    pub direction: Vector3,
    pub location: Vector3,
    pub attenuation: [f64; 3],
    pub radius: f64,
    pub cut_off_angle: f64,
    pub beam_width: f64,
    /// Inverse of the light's model-view 3×3 (point and spot lights).
    pub matrix: [f32; 9],
    /// The light's shadow (X3DLightNode shadows with shadowIntensity > 0 and shadowMapSize > 0).
    pub shadow: Option<ShadowParams>,
}

/// A TextureProjector or TextureProjectorParallel container (X3DTextureProjectorNode, light key
/// 3): Texture2.glsl x3d_TextureProjector [i] and its texture.
#[derive(Clone, Debug)]
pub struct ProjectorParams {
    pub texture: Rc<TextureImage>,
    pub color: [f64; 3],
    pub intensity: f64,
    /// The location in view space (computed in the pass that rendered the container first).
    pub location: Vector3,
    /// nearParameter, farParameter: the clamp range of the texture coordinate p.
    pub params: [f64; 2],
    /// setGlobalVariables matrix: view space to texture coordinates (stpq).
    pub matrix: Matrix4,
    /// X3DSingleTextureNode.isLinear (X3D_TEXTURE_PROJECTOR_i_LINEAR).
    pub linear: bool,
}

/// The shadow uniforms of a light container (setShaderUniforms with a shadowBuffer).
#[derive(Clone, Debug)]
pub struct ShadowParams {
    pub color: [f64; 3],
    pub intensity: f64,
    pub bias: f64,
    pub map_size: u32,
    /// setGlobalVariables shadowMatrix: view space to the shadow map (directional and spot lights:
    /// texture coordinates and depth; point lights: the light's space).
    pub matrix: Matrix4,
    /// The light's map in [`Frame::shadow_maps`].
    pub map: usize,
}

/// A light container's shadowBuffer (TextureBuffer of shadowMapSize², DEPTH_COMPONENT24, NEAREST)
/// and the views renderShadowMap draws into it.
#[derive(Clone, Debug)]
pub struct ShadowMap {
    pub size: u32,
    pub views: Vec<ShadowView>,
}

/// InstancedShape.set_bbox__: bboxSize and bboxCenter, or the union of the geometry's box under every
/// instance matrix.
fn instanced_bbox(sg: &Sg, n: NodeId, geometry: &Geometry, instances: &InstanceData) -> Box3 {
    if !sg
        .vec3(n, "bboxSize", Vector3::new(-1.0, -1.0, -1.0))
        .equals(&Vector3::new(-1.0, -1.0, -1.0))
    {
        return Box3::new(
            &sg.vec3(n, "bboxSize", Vector3::ZERO),
            &sg.vec3(n, "bboxCenter", Vector3::ZERO),
        );
    }

    let mut bbox = Box3::EMPTY;

    if geometry.vertex_count() > 0 {
        for m in &instances.matrices {
            let mut sub = Box3::new(&geometry.bbox_size(), &geometry.bbox_center());
            sub.mult_right(m);
            bbox.add(&sub);
        }
    }

    bbox
}

/// X3DShapeNode.set_bbox__ with a non-default bboxSize: the box of bboxSize and bboxCenter; `None` at the default
/// -1 -1 -1 (the geometry's box).
fn declared_bbox(sg: &Sg, n: NodeId) -> Option<Box3> {
    let default = Vector3::new(-1.0, -1.0, -1.0);
    let size = sg.vec3(n, "bboxSize", default);

    (!size.equals(&default)).then(|| Box3::new(&size, &sg.vec3(n, "bboxCenter", Vector3::ZERO)))
}

/// The box of an x y z w vertex soup.
fn soup_box(vertices: &[f32]) -> Box3 {
    if vertices.len() < 4 {
        return Box3::EMPTY;
    }

    let mut min = Vector3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut max = Vector3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);

    for v in vertices.chunks_exact(4) {
        min = Vector3::new(
            min.x.min(v[0] as f64),
            min.y.min(v[1] as f64),
            min.z.min(v[2] as f64),
        );
        max = Vector3::new(
            max.x.max(v[0] as f64),
            max.y.max(v[1] as f64),
            max.z.max(v[2] as f64),
        );
    }

    Box3::from_extents(&min, &max)
}

/// ViewVolume.set (projection, viewport) then intersectsSphere (radius, center): the six planes through the corners of
/// the unprojected viewport (window z 0 and 1; the viewport rectangle itself cancels out), a sphere outside one of them
/// is outside the volume.
fn view_volume_intersects_sphere(projection: &Matrix4, radius: f64, center: &Vector3) -> bool {
    let mut inv = *projection;
    inv.inverse();

    let unproject = |x: f64, y: f64, z: f64| {
        let mut v = Vector4::new(x, y, z * 2.0 - 1.0, 1.0);
        inv.mult_vec_matrix4(&mut v);
        let d = 1.0 / v.w;
        Vector3::new(v.x * d, v.y * d, v.z * d)
    };
    let p = [
        unproject(-1.0, -1.0, 0.0),
        unproject(1.0, -1.0, 0.0),
        unproject(1.0, 1.0, 0.0),
        unproject(-1.0, 1.0, 0.0),
        unproject(-1.0, -1.0, 1.0),
        unproject(1.0, -1.0, 1.0),
        unproject(1.0, 1.0, 1.0),
        unproject(-1.0, 1.0, 1.0),
    ];
    // Triangle3.normal (v1, v2, v3): normalize ((v3 - v2) x (v1 - v2)).
    let normal = |a: &Vector3, b: &Vector3, c: &Vector3| {
        let mut x1 = *c;
        x1.subtract(b);
        let mut x2 = *a;
        x2.subtract(b);
        x1.cross(&x2);
        x1.normalize();
        x1
    };
    let planes = [
        (p[1], normal(&p[0], &p[1], &p[2])),
        (p[4], normal(&p[7], &p[4], &p[0])),
        (p[2], normal(&p[6], &p[2], &p[1])),
        (p[6], normal(&p[2], &p[6], &p[7])),
        (p[0], normal(&p[1], &p[0], &p[4])),
        (p[7], normal(&p[4], &p[7], &p[6])),
    ];

    // Plane3.getDistanceToPoint: dot (point, normal) - distanceFromOrigin.
    !planes
        .iter()
        .any(|(point, normal)| center.dot(normal) - point.dot(normal) > radius)
}

/// One renderObject.render (TraverseType.SHADOW) of the light's group.
#[derive(Clone, Debug)]
pub struct ShadowView {
    /// GL projection (clip z in -w..w).
    pub projection: Matrix4,
    /// gl.viewport in the shadow map: x, y (from the bottom), width, height.
    pub viewport: [u32; 4],
    pub items: Vec<DepthItem>,
}

/// X3DRenderObject.addDepthShape: a shadow-casting shape of the pass.
#[derive(Clone, Debug)]
pub struct DepthItem {
    pub geometry: Rc<Geometry>,
    pub model_view: Matrix4,
    pub clip_planes: Vec<[f64; 4]>,
    /// An InstancedShape's instances.
    pub instances: Option<Rc<InstanceData>>,
    /// The gl.lineWidth of LINE particles (LineProperties.setShaderUniforms in X3DRenderObject.depth with a width up to
    /// ALIASED_LINE_WIDTH_RANGE [1]; 1 otherwise): [`shadow_view`] draws the wide lines as GL rasterizes them.
    pub line_width: f64,
    /// displaySimpleInstancedThick: the transformed LINE triangles of a ParticleSystem, made on the GPU
    /// (thick_lines.wgsl) under their instance matrices, in the system's space. `geometry` is empty then.
    pub thick_lines: Option<Rc<x3d_runtime::particles::ThickLines>>,
}

/// One renderObject.render (TraverseType.SHADOW) view of `items`. LINE particles with a gl.lineWidth above 1 px are the
/// aliased wide lines GL rasterizes in this view's viewport (crate::gpu::aliased_line_positions), two triangles per
/// segment under their instance matrices, in the system's space.
fn shadow_view(projection: Matrix4, viewport: [u32; 4], items: Vec<DepthItem>) -> ShadowView {
    let window = viewport.map(|v| v as f64);
    let items = items
        .into_iter()
        .map(|item| {
            let Some(instances) = item
                .instances
                .as_ref()
                .filter(|_| item.line_width > 1.0 && item.geometry.geometry_type == 1)
            else {
                return item;
            };
            let positions: Vec<[f32; 4]> = instances
                .matrices
                .iter()
                .flat_map(|m| {
                    crate::gpu::aliased_line_positions(
                        &item.geometry,
                        &item.model_view,
                        &projection,
                        window,
                        item.line_width,
                        Some(m),
                    )
                    .into_iter()
                    .map(|p| {
                        let mut v = Vector4::new(p[0] as f64, p[1] as f64, p[2] as f64, 1.0);
                        m.mult_vec_matrix4(&mut v);
                        [v.x as f32, v.y as f32, v.z as f32, 1.0]
                    })
                })
                .collect();

            DepthItem {
                geometry: Rc::new(crate::particles::line_triangle_geometry(&positions)),
                instances: None,
                line_width: 1.0,
                ..item
            }
        })
        .collect();

    ShadowView {
        projection,
        viewport,
        items,
    }
}

/// The groupNode of a light container: a grouping node (its own traverse, so a Transform applies
/// its matrix again in the shadow pass) or the root nodes of a layer or Inline (their Group).
#[derive(Clone, Debug)]
enum ShadowGroup {
    Node(NodeId),
    Roots(Vec<NodeId>),
}

/// Box3: a matrix whose axes are the half sizes and whose origin is the center; all zero when
/// empty (Box3.set ()).
#[derive(Clone, Copy, Debug)]
struct Box3(Matrix4);

impl Box3 {
    const EMPTY: Box3 = Box3(Matrix4([0.0; 16]));

    fn new(size: &Vector3, center: &Vector3) -> Box3 {
        Box3(Matrix4([
            size.x / 2.0,
            0.0,
            0.0,
            0.0,
            0.0,
            size.y / 2.0,
            0.0,
            0.0,
            0.0,
            0.0,
            size.z / 2.0,
            0.0,
            center.x,
            center.y,
            center.z,
            1.0,
        ]))
    }

    fn from_extents(min: &Vector3, max: &Vector3) -> Box3 {
        let mut size = *max;
        size.subtract(min);
        let mut center = *max;
        center.add(min).divide(2.0);
        Box3::new(&size, &center)
    }

    fn is_empty(&self) -> bool {
        self.0.0[15] == 0.0
    }

    /// Box3.getExtents.
    fn extents(&self) -> (Vector3, Vector3) {
        let m = &self.0.0;
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
        for p in [&mut p1, &mut p2, &mut p3, &mut p4] {
            p.negate();
        }
        min.min(&p1).min(&p2).min(&p3).min(&p4);
        max.max(&p1).max(&p2).max(&p3).max(&p4);
        let center = Vector3::new(m[12], m[13], m[14]);
        min.add(&center);
        max.add(&center);
        (min, max)
    }

    /// Box3.center.
    fn center(&self) -> Vector3 {
        Vector3::new(self.0.0[12], self.0.0[13], self.0.0[14])
    }

    /// Box3.add.
    fn add(&mut self, other: &Box3) {
        if self.is_empty() {
            *self = *other;
        } else if !other.is_empty() {
            let (mut min, mut max) = self.extents();
            let (b_min, b_max) = other.extents();
            min.min(&b_min);
            max.max(&b_max);
            *self = Box3::from_extents(&min, &max);
        }
    }

    fn mult_right(&mut self, m: &Matrix4) -> &mut Self {
        self.0.mult_right(m);
        self
    }
}

/// Camera.frustum.
fn frustum(l: f64, r: f64, b: f64, t: f64, n: f64, f: f64) -> Matrix4 {
    let (r_l, t_b, f_n, n_2) = (r - l, t - b, f - n, 2.0 * n);
    Matrix4([
        n_2 / r_l,
        0.0,
        0.0,
        0.0,
        0.0,
        n_2 / t_b,
        0.0,
        0.0,
        (r + l) / r_l,
        (t + b) / t_b,
        -(f + n) / f_n,
        -1.0,
        0.0,
        0.0,
        -(n_2 * f) / f_n,
        0.0,
    ])
}

/// X3DLightNode.getBiasMatrix.
const SHADOW_BIAS_MATRIX: Matrix4 = Matrix4([
    0.5, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.5, 0.5, 0.5, 1.0,
]);

/// The largest shadowMapSize (getMaxTextureSize clamps it; wgpu's default texture limit).
const MAX_SHADOW_MAP_SIZE: u32 = 8192;

#[derive(Clone, Debug)]
pub struct BackgroundParams {
    /// Vertex colors (rgba) and positions (xyzw) of the sphere or cube, X3DBackgroundNode.build.
    pub colors: Vec<f32>,
    pub sphere: Vec<f32>,
    pub transparency: f64,
    pub model_view: Matrix4,
    /// Face textures in draw order (X3DBackgroundNode.drawCube); empty without textures.
    pub faces: Vec<BackgroundFace>,
}

/// One loaded Background / TextureBackground face (X3DBackgroundNode.textureBits).
#[derive(Clone, Debug)]
pub struct BackgroundFace {
    /// Six vertices, xyzw (X3DBackgroundNode.transferRectangle).
    pub vertices: [f32; 24],
    pub texture: Rc<TextureImage>,
}

#[derive(Clone)]
pub struct Frame {
    pub projection: Matrix4,
    pub view: Matrix4,
    pub items: Vec<DrawItem>,
    pub lights: Vec<LightParams>,
    pub background: Option<BackgroundParams>,
    /// The bound Fog, if it draws fog (fogType != 0).
    pub fog: Option<FogParams>,
    /// Node types encountered that the renderer does not draw.
    pub unsupported: BTreeSet<String>,
    pub viewpoint: Option<NodeId>,
    /// GeneratedCubeMapTexture passes to draw before this frame.
    pub cube_passes: Vec<CubePass>,
    /// RenderedTexture passes to draw before this frame (after the cube passes).
    pub rendered_passes: Vec<RenderedPass>,
    /// The first EnvironmentLight (image based lighting of the physical materials).
    pub environment: Option<EnvironmentParams>,
    /// Browser options ToneMapping (0 NONE, 1 ACES_NARKOWICZ, 2 ACES_HILL, 3
    /// ACES_HILL_EXPOSURE_BOOST, 4 KHR_PBR_NEUTRAL; NONE for a texture pass, which is not
    /// independent), ColorSpace (0 SRGB, 1 LINEAR, 2 LINEAR_WHEN_PHYSICAL_MATERIAL) and Exposure
    /// (Math .max (value, 0)).
    pub tone_mapping: u32,
    pub color_space: u32,
    pub exposure: f64,
    /// The layer's viewport rectangle (x, y, width, height in pixels, y up: X3DLayerNode
    /// viewportNode .getRectangle ()); `None` for the whole target.
    pub viewport: Option<[f64; 4]>,
    /// LayerSet: the layers drawn, in order, each a frame of its own; this frame then carries
    /// only the texture passes and the browser-wide values. Empty without a LayerSet.
    pub layers: Vec<Frame>,
    /// The shadow maps of the frame's lights (X3DRenderObject.draw renderShadowMap), drawn before
    /// the frame.
    pub shadow_maps: Vec<ShadowMap>,
    /// The texture projectors of the frame (DrawItem.projectors).
    pub projectors: Vec<ProjectorParams>,
}

pub const MAX_LIGHTS: usize = 8;

/// The texture projectors of each shape: the global ones, then the shape's local ones, at most
/// MAX_TEXTURES (X3D_NUM_TEXTURE_PROJECTORS min (numTextureProjectors, browser .getMaxTextures ())).
fn shape_projectors(items: &mut [DrawItem], globals: &[u32]) {
    for item in items {
        let local = std::mem::take(&mut item.projectors);
        item.projectors = globals
            .iter()
            .chain(&local)
            .copied()
            .take(MAX_TEXTURES)
            .collect();
    }
}

/// The lights of each shape: the global lights, then the shape's local lights, at most
/// MAX_LIGHTS (X3D_NUM_LIGHTS min (numLights, browser .getMaxLights ())).
fn shape_lights(items: &mut [DrawItem], globals: &[u32]) {
    for item in items {
        let local = std::mem::take(&mut item.lights);
        item.lights = globals
            .iter()
            .chain(&local)
            .copied()
            .take(MAX_LIGHTS)
            .collect();
    }
}

/// Camera and bindable state supplied by an interactive viewer (x3d-viewer) instead of the
/// "first node found" choice of the static render: the bound viewpoint's cameraSpaceMatrix
/// (user position, orientation and model matrix), its user field of view, near/far values, the
/// bound NavigationInfo headlight, and the bound Background and Fog.
#[derive(Clone, Debug)]
pub struct CameraOverride {
    pub camera_space: Matrix4,
    pub fov: f64,
    pub near: f64,
    pub far: f64,
    /// The viewpoint's projection matrix when it is not `perspective (fov, near, far)`
    /// (OrthoViewpoint.getProjectionMatrixWithLimits).
    pub projection: Option<Matrix4>,
    pub headlight: bool,
    pub viewpoint: Option<NodeId>,
    pub navigation_info: Option<NodeId>,
    pub background: Option<(NodeId, Matrix4)>,
    pub fog: Option<NodeId>,
    /// The viewpoint's getMaxFarValue, the radius of the background (X3DBackgroundNode).
    pub max_far: f64,
    /// OrthoViewpoint getUserSizeX and getUserSizeY (Layout: getViewportSize).
    pub ortho_size: Option<(f64, f64)>,
}

struct Bound {
    viewpoint: Option<(NodeId, Matrix4)>,
    navigation_info: Option<NodeId>,
    background: Option<(NodeId, Matrix4)>,
    fog: Option<NodeId>,
    /// Model matrix of every Fog met by the CAMERA pass (Fog.traverse: modelMatrix).
    fog_matrices: HashMap<NodeId, Matrix4>,
}

/// Runtime identity + node: Inline scenes live in their own runtimes with their own node ids.
type Key = (usize, NodeId);

fn key(sg: &Sg, n: NodeId) -> Key {
    (sg.rt as *const x3d_runtime::Runtime as usize, n)
}

/// Geometry and texture results kept between frames by an interactive viewer, so an unchanged
/// node is not rebuilt (X_ITE rebuilds a geometry only when one of its fields changes:
/// X3DGeometryNode.set_live / addInterest rebuild). A node is rebuilt when the write stamp
/// ([`x3d_runtime::Runtime::stamp`]) of the node or of any node its fields reference changed.
#[derive(Default)]
pub struct RenderCache {
    geometries: HashMap<(u64, NodeId), CacheEntry<Geometry>>,
    textures: HashMap<(u64, NodeId), CacheEntry<TextureImage>>,
    /// Background face images (the six `*Url` fields of a Background), by node and face.
    faces: HashMap<(u64, NodeId, usize), CacheEntry<TextureImage>>,
    /// X_ITE's 1x1 white default texture, shared so the GPU keeps one bind group for it.
    white: Option<Rc<TextureImage>>,
    /// getBBoxShape (): the unit cube of every bboxDisplay (one buffer on the GPU).
    bbox_box: Option<Rc<Geometry>>,
    /// HAnimJoint.modelViewMatrix: the model-view of each joint's last DISPLAY traversal.
    joint_model_views: HashMap<(u64, NodeId), Matrix4>,
    /// ScreenGroup.matrix: the screen matrix of each ScreenGroup's last DISPLAY traversal. The CAMERA pass (and picking) of the next
    /// frame uses it, as ScreenGroup.traverse leaves `this .matrix` alone for them.
    screen_matrices: HashMap<(u64, NodeId), Matrix4>,
    /// Billboard.matrix: the rotation of each Billboard's last DISPLAY traversal (Billboard.rotate), which getBBox and the SHADOW
    /// pass use.
    billboard_matrices: HashMap<(u64, NodeId), Matrix4>,
    /// HAnimHumanoid.jointMatricesArray and the unlock count it was made at (the update Lock).
    joint_matrices: HashMap<(u64, NodeId), (u64, Rc<Vec<f32>>)>,
    /// Skinned copies of geometries, reused while their source, tables and matrices are the same.
    skinned: HashMap<(u64, NodeId, usize), SkinnedEntry>,
    /// GaussianSplats data (see [`crate::splats`]), by node; `used` is the frame it was last drawn.
    splats: HashMap<(u64, NodeId), (u64, Rc<crate::splats::SplatData>, u64)>,
    /// The POINT, LINE, TRIANGLE, QUAD or SPRITE geometry of each ParticleSystem, the bits it was built from and the
    /// frame it was last used, so the GPU keeps one buffer per system (X_ITE rebuilds it only when one of these changes).
    particles: HashMap<(u64, NodeId), (Vec<u64>, Rc<Geometry>, u64)>,
    frame: u64,
    /// Counters for tests and tools: geometries (re)built / reused in the last frame.
    pub built: usize,
    pub reused: usize,
}

struct SkinnedEntry {
    source: Rc<Geometry>,
    tables: Rc<crate::hanim::SkinTables>,
    matrices: Rc<Vec<f32>>,
    value: Rc<Geometry>,
    used: u64,
}

/// One HAnimHumanoid or HAnimSegment traversal of the DISPLAY pass (renderObject.getHAnimNode ()).
struct SkinSource {
    node: NodeId,
    tables: Rc<crate::hanim::SkinTables>,
    matrices: Rc<Vec<f32>>,
}

struct CacheEntry<T> {
    fingerprint: u64,
    value: Option<Rc<T>>,
    used: u64,
}

/// How many frames an unused entry is kept (a Switch or LOD may bring the node back).
const CACHE_KEEP_FRAMES: u64 = 600;

impl RenderCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop entries that have not been used for a while; call once per frame after the build.
    fn end_frame(&mut self) {
        let frame = self.frame;
        self.geometries
            .retain(|_, e| e.used + CACHE_KEEP_FRAMES >= frame);
        self.textures
            .retain(|_, e| e.used + CACHE_KEEP_FRAMES >= frame);
        self.faces
            .retain(|_, e| e.used + CACHE_KEEP_FRAMES >= frame);
        self.skinned
            .retain(|_, e| e.used + CACHE_KEEP_FRAMES >= frame);
        self.splats
            .retain(|_, e| e.2 + crate::splats_gpu::KEEP_FRAMES >= frame);
        self.particles
            .retain(|_, e| e.2 + CACHE_KEEP_FRAMES >= frame);
    }
}

/// FNV-style mix of the write stamps of `n` and of the nodes its fields reference.
fn fingerprint(sg: &Sg, n: NodeId) -> u64 {
    fn mix(h: &mut u64, v: u64) {
        *h = (*h ^ v).wrapping_mul(0x100000001b3).rotate_left(23);
    }

    fn walk(sg: &Sg, n: NodeId, depth: usize, h: &mut u64) {
        mix(h, n as u64);
        mix(h, sg.rt.stamp(n));

        if depth >= 3 {
            return;
        }

        for f in &sg.rt.scene.node(n).fields {
            match &f.value {
                FieldValue::SFNode(Some(c)) => {
                    if let Some(c) = sg.inner(*c) {
                        walk(sg, c, depth + 1, h);
                    }
                }
                FieldValue::MFNode(v) => {
                    for c in v.iter().flatten() {
                        if let Some(c) = sg.inner(*c) {
                            walk(sg, c, depth + 1, h);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let mut h = 0xcbf29ce484222325u64;
    mix(&mut h, quality_epoch());
    // A Text finds its font in the FontLibrary table, which a FontLibrary load changes.
    if sg.type_name(n) == Some("Text") {
        mix(&mut h, sg.rt.font_epoch);
        // A ScreenFontStyle's size and glyph canvas are in pixels × ContentScale (ScreenFontStyle.getScale).
        mix(
            &mut h,
            sg.rt
                .browser_options
                .rendering
                .get("ContentScale")
                .map_or(1.0, |v| v.as_f64())
                .to_bits(),
        );
    }
    walk(sg, n, 0, &mut h);
    h
}

/// Owned storage behind an [`crate::extras::Extras`] view.
pub struct ExtrasOwned<'s> {
    tex: Option<Vec<crate::extras::TexChannel<'s>>>,
    tangent: Option<&'s [f32]>,
    fog: Option<&'s [f32]>,
    attribs: Vec<crate::extras::Attrib<'s>>,
}

impl ExtrasOwned<'_> {
    pub fn view(&self) -> crate::extras::Extras<'_> {
        crate::extras::Extras {
            tex: self.tex.as_deref(),
            tangent: self.tangent,
            fog: self.fog,
            attribs: &self.attribs,
        }
    }
}

pub struct Builder<'a> {
    /// The world's runtime: its fetcher serves every load (Inline runtimes are detached).
    root: &'a x3d_runtime::Runtime,
    geometries: HashMap<Key, Option<Rc<Geometry>>>,
    textures: HashMap<Key, Option<Rc<TextureImage>>>,
    pub unsupported: BTreeSet<String>,
    items: Vec<DrawItem>,
    lights: Vec<LightParams>,
    /// renderObject.getGlobalLights (): indices into `lights`.
    global_lights: Vec<u32>,
    projectors: Vec<ProjectorParams>,
    global_projectors: Vec<u32>,
    local_projectors: Vec<u32>,
    /// The projector containers of the main pass: node, model matrix, location in view space
    /// (a texture pass uses the main pass's location, see `projector`).
    projector_sources: Vec<(NodeId, Matrix4, Vector3)>,
    /// The grouping nodes around the current node (the groupNode of a local light).
    groups: Vec<ShadowGroup>,
    /// The root nodes of the layer being built (renderObject.getLayer () .getGroups ()).
    layer_roots: Vec<NodeId>,
    /// The shadow maps of the frame being built.
    shadow_maps: Vec<ShadowMap>,
    /// The light containers with a shadow map of the frame being built: the light node, its
    /// model matrix, its invLightSpaceProjectionMatrix and its uniforms. A texture pass (a
    /// DependentRenderer) finds the container of the same light and uses its map.
    shadow_sources: Vec<(NodeId, Matrix4, Matrix4, ShadowParams)>,
    /// The X3DLightNode part of renderObject.getLocalObjects (): the `global FALSE` lights of the
    /// enclosing grouping nodes (indices into `lights`).
    local_lights: Vec<u32>,
    /// renderObject.getLocalFogs () past the global fog: the enabled LocalFog nodes of the enclosing grouping
    /// nodes (the innermost, last, is the shape's fog).
    local_fogs: Vec<FogParams>,
    /// Interactive camera (see [`CameraOverride`]); `None` for the oracle renders.
    pub camera: Option<CameraOverride>,
    /// Results kept between frames (interactive viewer); `None` rebuilds everything per frame.
    cache: Option<&'a mut RenderCache>,
    /// A query between frames (with_cache_query): the frame's built / reused counts stay.
    cache_query: bool,
    /// renderObject.getLocalObjects (): the clip planes pushed by the enclosing grouping nodes.
    clip_planes: Vec<[f64; 4]>,
    /// GeneratedCubeMapTexture.traverse: the textures to render this frame (update not NONE, a
    /// framebuffer) with the model matrix of the last shape that used them, in first-use order.
    generated: Vec<(NodeId, Matrix4)>,
    /// RenderedTextures this frame draws (Runtime::rendered_now) reached by the current layer, and
    /// those an earlier layer drew (RenderedTexture.traverse: checkUpdate after the first).
    rendered: Vec<NodeId>,
    rendered_done: Vec<NodeId>,
    /// The camera space matrix of the pass being built (modelMatrix = modelView × cameraSpace).
    camera_space: Matrix4,
    /// A texture pass (DependentRenderer): not independent, so no texture is collected, and the
    /// shapes that use `hidden_texture` are not drawn (setShaderUniforms: scissor 0 0 0 0).
    dependent: bool,
    hidden_texture: Option<NodeId>,
    /// setTransparent (background .isTransparent ()) of the layer's bound background.
    background_transparent: bool,
    /// The first EnvironmentLight that is on (X3D_USE_IBL).
    environment: Option<EnvironmentParams>,
    /// Interactive cameras of the other layers of a LayerSet (`camera` is layer 0's).
    pub layer_cameras: Vec<(LayerKey, CameraOverride)>,
    /// The layer [`Builder::build_layer`] builds.
    layer: LayerKey,
    /// renderObject.getViewVolumes (): the viewport rectangles, the layer's first.
    viewports: Vec<[f64; 4]>,
    /// renderObject.getLayouts () and getLayoutRectangles ().
    layouts: LayoutStack,
    /// The layer's viewpoint and navigation info as Layout.push reads them.
    layout_camera: LayoutCamera,
    /// The layer's projection matrix (ScreenText snaps its origin to a whole pixel).
    projection: Matrix4,
    /// ScreenGroup.matrix: set by the DISPLAY pass, kept for the passes that do not set it (CAMERA, SHADOW).
    screen_matrices: HashMap<NodeId, Matrix4>,
    /// Billboard.matrix: set by the DISPLAY pass (Billboard.rotate), kept for getBBox and the passes that do not rotate (SHADOW).
    billboard_matrices: HashMap<NodeId, Matrix4>,
    /// renderObject.getInvHumanoidMatrix (): starts with the identity.
    inv_humanoid: Vec<Matrix4>,
    /// renderObject.getHAnimNode (): the skin source around the current shape, indices into `skins`.
    hanim: Vec<Option<usize>>,
    skins: Vec<SkinSource>,
    /// The draw items to skin once their source is complete: item index, geometry node, source.
    skin_items: Vec<(usize, NodeId, usize)>,
    /// HAnimJoint.modelViewMatrix of this frame (and, with a cache, of earlier frames).
    joint_model_views: HashMap<NodeId, Matrix4>,
}

#[path = "traverse_shaders.rs"]
mod traverse_shaders;
#[path = "traverse_splats.rs"]
mod traverse_splats;

/// One GeneratedCubeMapTexture.renderTexture: six frames drawn into the texture's faces before
/// the frame that samples it (X3DRenderObject.draw: rendered textures first).
#[derive(Clone)]
pub struct CubePass {
    /// The texture (runtime uid, node), as in [`TextureImage::generated`].
    pub key: (u64, NodeId),
    pub size: u32,
    /// Front, back, left, right, top, bottom (X3DEnvironmentTextureNode.targets order).
    pub faces: Vec<Frame>,
    /// update NEXT_FRAME_ONLY: the host sets update to NONE after the pass.
    pub next_frame_only: bool,
}

/// A RenderedTexture pass (RenderedTexture.renderTexture): `frame` drawn into the texture's
/// width × height target, then copied into the texture.
#[derive(Clone)]
pub struct RenderedPass {
    /// The texture (runtime uid, node), as in [`TextureImage::generated`].
    pub key: (u64, NodeId),
    pub width: u32,
    pub height: u32,
    /// depthMap: TraverseType DEPTH (the shapes' window depth as color, cleared to 1 1 1 1, no
    /// background) into a float texture.
    pub depth_map: bool,
    pub frame: Frame,
}

/// RenderedTexture.setChild: the children by kind (the last of each special kind wins).
struct RenderedChildren {
    fog: Option<NodeId>,
    navigation_info: Option<NodeId>,
    background: Option<NodeId>,
    viewpoint: Option<NodeId>,
    group: Vec<NodeId>,
}

fn rendered_children(sg: &Sg, n: NodeId) -> RenderedChildren {
    let mut c = RenderedChildren {
        fog: None,
        navigation_info: None,
        background: None,
        viewpoint: None,
        group: vec![],
    };

    for child in sg.nodes(n, "children") {
        match sg.type_name(child) {
            Some("Fog") => c.fog = Some(child),
            Some("NavigationInfo") => c.navigation_info = Some(child),
            Some("Background" | "TextureBackground") => c.background = Some(child),
            Some("Viewpoint" | "OrthoViewpoint" | "GeoViewpoint") => c.viewpoint = Some(child),
            _ => c.group.push(child),
        }
    }

    c
}

/// The layer's camera for the RenderedTexture passes (built in `build_layer`).
#[derive(Clone)]
struct PassCamera {
    camera_space: Matrix4,
    view: Matrix4,
    fov: f64,
    near: f64,
    far: f64,
    ortho: Option<[f64; 4]>,
    headlight: bool,
}

/// Children-like fields of the grouping nodes.
fn children_of(sg: &Sg, n: NodeId, ty: &str) -> Vec<NodeId> {
    match ty {
        "Switch" => {
            let choice = sg.i32(n, "whichChoice", -1);
            // VRML 2.0 Switch has `choice`, X3D `children`; X_ITE aliases them.
            let mut kids = sg.nodes(n, "children");
            if kids.is_empty() {
                kids = sg.nodes(n, "choice");
            }
            let raw = {
                let mut r = sg.raw_nodes(n, "children");
                if r.is_empty() {
                    r = sg.raw_nodes(n, "choice");
                }
                r
            };
            let _ = kids;
            if choice >= 0 && (choice as usize) < raw.len() {
                raw[choice as usize]
                    .and_then(|c| sg.inner(c))
                    .into_iter()
                    .collect()
            } else {
                vec![]
            }
        }
        "CADFace" => sg.cad_face_child(n).into_iter().collect(),
        "HAnimHumanoid" | "HAnimJoint" | "HAnimSegment" | "HAnimSite" => sg
            .hanim_children(n, ty)
            .map(|g| g.concat())
            .unwrap_or_default(),
        "GeoLOD" => sg.geo_lod_kids(n, true),
        _ => {
            let mut kids = sg.nodes(n, "children");
            if kids.is_empty() && ty == "LOD" {
                kids = sg.nodes(n, "level");
            }
            kids
        }
    }
}

const GROUPS: &[&str] = &[
    "Group",
    "Transform",
    "Anchor",
    "Collision",
    "Billboard",
    "Switch",
    "LOD",
    "GeoLocation",
    "GeoTransform",
    "GeoLOD",
    "CADAssembly",
    "CADLayer",
    "CADPart",
    "CADFace",
    "PickableGroup",
    "GroupAnnotation",
    "HAnimHumanoid",
    "HAnimJoint",
    "HAnimSegment",
    "HAnimSite",
    "ScreenGroup",
    // EspduTransform is an X3DGroupingNode (DIS/EspduTransform.js): its children are drawn like a Group's; its
    // translation, rotation, scale and center are DIS state, not a matrix (no getMatrix override).
    "EspduTransform",
];

impl<'a> Builder<'a> {
    pub fn new(root: &'a x3d_runtime::Runtime) -> Self {
        Builder {
            root,
            geometries: HashMap::new(),
            textures: HashMap::new(),
            unsupported: BTreeSet::new(),
            items: vec![],
            lights: vec![],
            global_lights: vec![],
            projectors: vec![],
            global_projectors: vec![],
            local_projectors: vec![],
            projector_sources: vec![],
            groups: vec![],
            layer_roots: vec![],
            shadow_maps: vec![],
            shadow_sources: vec![],
            local_lights: vec![],
            local_fogs: vec![],
            camera: None,
            cache: None,
            cache_query: false,
            clip_planes: vec![],
            generated: vec![],
            rendered: vec![],
            rendered_done: vec![],
            camera_space: Matrix4::IDENTITY,
            dependent: false,
            hidden_texture: None,
            background_transparent: false,
            environment: None,
            layer_cameras: vec![],
            layer: None,
            viewports: vec![],
            layouts: LayoutStack::default(),
            layout_camera: LayoutCamera {
                ortho: None,
                fov: std::f64::consts::FRAC_PI_4,
                near: 0.125,
                content_scale: 1.0,
            },
            projection: Matrix4::IDENTITY,
            screen_matrices: HashMap::new(),
            billboard_matrices: HashMap::new(),
            inv_humanoid: vec![Matrix4::IDENTITY],
            hanim: vec![None],
            skins: vec![],
            skin_items: vec![],
            joint_model_views: HashMap::new(),
        }
    }

    /// Keep geometry and texture results in `cache` between frames.
    pub fn with_cache(mut self, cache: &'a mut RenderCache) -> Self {
        cache.frame += 1;
        cache.built = 0;
        cache.reused = 0;
        self.cache = Some(cache);
        self
    }

    /// Like [`Builder::with_cache`] for a query between frames (X3DLayerNode.getBBox): the frame count stays.
    pub fn with_cache_query(mut self, cache: &'a mut RenderCache) -> Self {
        self.cache = Some(cache);
        self.cache_query = true;
        self
    }

    /// X3DLayerNode.getBBox: X3DGroupingNode.getBBox of the layer's group (its children, shadows FALSE) in the layer's space,
    /// with the Billboard and ScreenGroup matrices of the last DISPLAY traversal (the cache). The Box3 matrix (its axes are the
    /// half extents, its origin the center; a box of a single transformed child keeps its orientation, as X_ITE's Box3 .add
    /// of an empty box does), or None for an empty box.
    pub fn layer_bbox(&mut self, key: crate::layer::LayerKey) -> Option<Matrix4> {
        let sg = Sg::new(self.root);
        let roots = crate::layer::layer_children(&sg, key);
        let bbox = self.bbox_of(&sg, &roots, 0, false);

        (!bbox.is_empty()).then_some(bbox.0)
    }

    /// ClipPlane.push for the ClipPlane children of a grouping node; returns how many were pushed.
    fn push_clip_planes(&mut self, sg: &Sg, kids: &[NodeId], model_view: &Matrix4) -> usize {
        let before = self.clip_planes.len();

        for &c in kids {
            if sg.type_name(c) == Some("ClipPlane") {
                if let Some(p) = clip_plane(sg, c, model_view) {
                    self.clip_planes.push(p);
                }
            }
        }

        self.clip_planes.len() - before
    }

    /// Billboard.rotate.
    fn billboard_matrix(&self, sg: &Sg, n: NodeId, model_view: &Matrix4) -> Matrix4 {
        let mut inv = *model_view;
        inv.inverse();
        let mut to_viewer = Vector3::new(inv.0[12], inv.0[13], inv.0[14]);
        to_viewer.normalize();
        let axis = sg.vec3(n, "axisOfRotation", Vector3::new(0.0, 1.0, 0.0));

        if axis.equals(&Vector3::ZERO) {
            let mut viewer_y = Vector3::Y_AXIS;
            inv.mult_dir_matrix(&mut viewer_y);
            viewer_y.normalize();
            let z = to_viewer;
            let mut x = viewer_y;
            x.cross(&to_viewer);
            let mut y = to_viewer;
            y.cross(&x);
            x.normalize();
            y.normalize();
            Matrix4([
                x.x, x.y, x.z, 0.0, y.x, y.y, y.z, 0.0, z.x, z.y, z.z, 0.0, 0.0, 0.0, 0.0, 1.0,
            ])
        } else {
            let mut n1 = axis;
            n1.cross(&to_viewer);
            let mut n2 = axis;
            n2.cross(&Vector3::Z_AXIS);
            let mut r = Rotation4::identity();
            r.set_vectors(&n2, &n1);
            let m3 = r.get_matrix();
            let m = m3.0;
            Matrix4([
                m[0], m[1], m[2], 0.0, m[3], m[4], m[5], 0.0, m[6], m[7], m[8], 0.0, 0.0, 0.0, 0.0,
                1.0,
            ])
        }
    }

    /// LOD.getLevel with a range.
    /// The LOD child to draw: `children [min (level_changed, length - 1)]` once a DISPLAY traversal
    /// (the viewer's LOD.changeLevel) has set level_changed; before that (a still image of a scene
    /// that never ran a frame) the level the distance gives.
    fn lod_kids(&self, sg: &Sg, n: NodeId, model_view: &Matrix4, kids: Vec<NodeId>) -> Vec<NodeId> {
        let stored = sg.i32(n, "level_changed", -1);

        if stored >= 0 {
            let raw = sg.raw_nodes(n, "children");
            let level = stored.min(raw.len() as i32 - 1);

            return if level >= 0 {
                raw[level as usize]
                    .and_then(|c| sg.inner(c))
                    .into_iter()
                    .collect()
            } else {
                vec![]
            };
        }

        self.lod_level(sg, n, model_view, kids.len())
            .map(|l| vec![kids[l]])
            .unwrap_or_default()
    }

    fn lod_level(&self, sg: &Sg, n: NodeId, model_view: &Matrix4, count: usize) -> Option<usize> {
        if count == 0 {
            return None;
        }
        let range = sg.f64s(n, "range");
        if range.is_empty() {
            return Some(0);
        }
        let mut c = sg.vec3(n, "center", Vector3::ZERO);
        model_view.mult_vec_matrix3(&mut c);
        let distance = c.norm();
        let level = range
            .iter()
            .position(|r| distance < *r)
            .unwrap_or(range.len());
        Some(level.min(count - 1))
    }

    // --- CAMERA pass ------------------------------------------------------------------------

    fn camera(&self, sg: &Sg, n: NodeId, m: &Matrix4, bound: &mut Bound, depth: usize) {
        if depth > 256 {
            return;
        }
        let Some(ty) = sg.type_name(n) else { return };
        let inline = sg.in_inline(n);

        // X3DGroupingNode: a child whose `visible` is FALSE (X3DBoundedObject._display) is in none
        // of the group's object sets, not even cameraObjects.
        if !sg.bool(n, "visible", true) {
            return;
        }

        match ty {
            "Viewpoint" | "OrthoViewpoint" | "GeoViewpoint"
                if bound.viewpoint.is_none() && !inline =>
            {
                bound.viewpoint = Some((n, *m))
            }
            "NavigationInfo" if bound.navigation_info.is_none() && !inline => {
                bound.navigation_info = Some(n)
            }
            "Background" | "TextureBackground" if bound.background.is_none() && !inline => {
                bound.background = Some((n, *m))
            }
            "Fog" if !inline => {
                bound.fog_matrices.insert(n, *m);

                if bound.fog.is_none() {
                    bound.fog = Some(n);
                }
            }
            _ if GROUPS.contains(&ty) || matches!(ty, "Viewport" | "LayoutGroup") => {
                let mut mm = *m;
                if let Some(t) = sg.group_matrix(n, ty) {
                    mm.mult_left(&t);
                } else if let Some(t) = self.screen_matrix(sg, n) {
                    mm.mult_left(&t);
                }
                let kids = children_of(sg, n, ty);
                let kids = if ty == "LOD" {
                    self.lod_kids(sg, n, &mm, kids)
                } else {
                    kids
                };
                for c in kids {
                    self.camera(sg, c, &mm, bound, depth + 1);
                }
            }
            _ => {}
        }
    }

    // --- DISPLAY pass -----------------------------------------------------------------------

    fn display(&mut self, sg: &Sg, n: NodeId, mv: &Matrix4, depth: usize) {
        if depth > 256 {
            return;
        }
        let Some(ty) = sg.type_name(n) else { return };

        // X3DGroupingNode.set_objects__: the bbox node of a bounded object with bboxDisplay is a visible
        // object whatever its `visible` is.
        if crate::bbox_display::DISPLAYED_TYPES.contains(&ty) {
            return self.bbox_display(sg, n, mv);
        }

        // X3DGroupingNode: a child whose `visible` is FALSE is not a visible object (a GaussianSplats node's bbox node is).
        if !sg.bool(n, "visible", true) {
            if ty == "GaussianSplats" && sg.bool(n, "bboxDisplay", false) {
                let data = self.splat_data(sg, n);
                let bbox = Self::splats_bbox(sg, n, &data);
                self.bbox_shape(n, bbox, mv);
            }

            return;
        }

        match ty {
            "Shape" | "InstancedShape" => self.shape(sg, n, mv),
            "ParticleSystem" => self.particle_system(sg, n, mv),
            "VolumeData" | "IsoSurfaceVolumeData" | "SegmentedVolumeData" => {
                self.volume(sg, n, ty, mv)
            }
            "GaussianSplats" => self.splats(sg, n, mv),
            "DirectionalLight" | "PointLight" | "SpotLight" => self.light(sg, n, ty, mv),
            "TextureProjector" | "TextureProjectorParallel" => self.projector(sg, n, ty, mv),
            "EnvironmentLight" => self.environment_light(sg, n, mv),
            // StaticGroup: the optimized group holds only the shapes (StaticGroup.createGroups).
            "StaticGroup" => {
                let mut shapes = vec![];
                sg.static_shapes(n, &Matrix4::IDENTITY, &mut shapes, depth);
                for (s, mut m) in shapes {
                    m.mult_right(mv);
                    self.shape(sg, s, &m);
                }
            }
            "Inline" => {
                // Inline.getInternalScene: the loaded scene's root nodes.
                // Inline.traverse: its private Group holds the root nodes.
                if let Some(ctx) = sg.inline_scene(n) {
                    if sg.bool(n, "load", true) {
                        // A grouping node: lights first, local lights end with the Inline.
                        let roots = sg.roots_of(ctx);
                        self.groups.push(ShadowGroup::Roots(roots.clone()));
                        self.group_children(sg, roots, mv, depth);
                        self.groups.pop();
                    }
                }
            }
            "HAnimHumanoid" => self.hanim_humanoid(sg, n, mv, depth),
            "HAnimSegment" if crate::hanim::segment_tables(sg, n).is_some() => {
                self.hanim_segment(sg, n, mv, depth)
            }
            _ if GROUPS.contains(&ty) => {
                let mut m = *mv;
                if let Some(t) = sg.group_matrix(n, ty) {
                    m.mult_left(&t);

                    // HAnimJoint.traverse: DISPLAY keeps the joint's model-view matrix.
                    if ty == "HAnimJoint" {
                        self.joint_model_views.insert(n, m);

                        if let Some(c) = self.cache.as_deref_mut() {
                            c.joint_model_views.insert((sg.rt.uid, n), m);
                        }
                    }
                } else if ty == "Billboard" {
                    let b = self.billboard_matrix(sg, n, &m);
                    self.billboard_matrices.insert(n, b);

                    if let Some(c) = self.cache.as_deref_mut() {
                        c.billboard_matrices.insert((sg.rt.uid, n), b);
                    }

                    m.mult_left(&b);
                } else if ty == "ScreenGroup" {
                    let s = self.screen_scale_matrix(&m, self.layout_camera.content_scale, false);
                    self.screen_matrices.insert(n, s);

                    if let Some(c) = self.cache.as_deref_mut() {
                        c.screen_matrices.insert((sg.rt.uid, n), s);
                    }

                    m.mult_left(&s);
                }
                let kids = children_of(sg, n, ty);
                let kids = if ty == "LOD" {
                    self.lod_kids(sg, n, &m, kids)
                } else {
                    kids
                };
                self.groups.push(ShadowGroup::Node(n));
                self.group_children(sg, kids, &m, depth);
                self.groups.pop();
            }
            // Viewport.traverse: its rectangle in the enclosing one around the children.
            "Viewport" => {
                let rectangle = layer::viewport_rectangle(sg, n, self.viewport());
                self.viewports.push(rectangle);
                self.groups.push(ShadowGroup::Node(n));
                self.group_children(sg, children_of(sg, n, ty), mv, depth);
                self.groups.pop();
                self.viewports.pop();
            }
            "LayoutGroup" => self.layout_group(
                sg,
                n,
                sg.node(n, "layout"),
                sg.node(n, "viewport"),
                children_of(sg, n, ty),
                mv,
                depth,
            ),
            // Nodes with no visual output.
            "WorldInfo"
            | "ClipPlane"
            | "Viewpoint"
            | "OrthoViewpoint"
            | "GeoViewpoint"
            | "NavigationInfo"
            | "Background"
            | "TextureBackground"
            | "Fog"
            | "TimeSensor"
            | "TouchSensor"
            | "GeoTouchSensor"
            | "PlaneSensor"
            | "SphereSensor"
            | "CylinderSensor"
            | "ProximitySensor"
            | "VisibilitySensor"
            | "Script"
            | "PositionInterpolator"
            | "OrientationInterpolator"
            | "ColorInterpolator"
            | "ScalarInterpolator"
            | "CoordinateInterpolator"
            | "NormalInterpolator"
            | "Sound"
            | "AudioClip"
            | "BooleanFilter"
            | "BooleanToggle"
            | "BooleanTrigger"
            | "BooleanSequencer"
            | "IntegerSequencer"
            | "IntegerTrigger"
            | "TimeTrigger"
            | "AnnotationTarget"
            | "TextAnnotation"
            | "URLAnnotation"
            | "IconAnnotation"
            | "GeoMetadata"
            | "FontLibrary"
            | "DISEntityManager"
            | "DISEntityTypeMapping"
            | "NurbsSet"
            | "NurbsPositionInterpolator"
            | "NurbsOrientationInterpolator"
            | "NurbsSurfaceInterpolator"
            | "MetadataString"
            | "MetadataFloat"
            | "MetadataInteger"
            | "MetadataSet"
            | "MetadataDouble"
            | "MetadataBoolean" => {}
            // LayerSet is drawn by the world (X3DWorld), not as a child of layer 0; Layer and
            // LayoutLayer are not X3DChildNodes.
            "LayerSet" | "Layer" | "LayoutLayer" | "AnnotationLayer" | "Layout" => {}
            // Not X3DChildNode: X3DGroupingNode.add ignores these in children.
            "Appearance" | "Material" | "UnlitMaterial" | "TwoSidedMaterial"
            | "PointProperties" | "LineProperties" | "FillProperties" | "TextureProperties"
            | "BlendMode" | "DepthMode" | "ImageTexture" | "PixelTexture" | "MovieTexture"
            | "TextureTransform" | "Coordinate" | "Color" | "Normal" | "TextureCoordinate"
            | "FontStyle" | "Box" | "Sphere" | "Cone" | "Cylinder" | "IndexedFaceSet"
            | "IndexedLineSet" | "PointSet" | "ElevationGrid" | "GeoElevationGrid"
            | "Extrusion" | "Text" => {}
            other => {
                self.unsupported.insert(other.to_string());
            }
        }
    }

    /// X3DBBoxNode.traverse for a ReceiverPdu, SignalPdu or TransmitterPdu with bboxDisplay: the bbox shape
    /// (a Shape without Appearance: unlit white lines) in `setTransform (bbox .center, null, bbox .size)`.
    /// The bounding box is bboxSize / bboxCenter, or empty (size and center 0) at the default -1 -1 -1.
    fn bbox_display(&mut self, sg: &Sg, n: NodeId, mv: &Matrix4) {
        if !sg.bool(n, "bboxDisplay", false) {
            return;
        }

        let size = sg.vec3(n, "bboxSize", Vector3::new(-1.0, -1.0, -1.0));
        let bbox = if size.equals(&Vector3::new(-1.0, -1.0, -1.0)) {
            Box3::EMPTY
        } else {
            Box3::new(&size, &sg.vec3(n, "bboxCenter", Vector3::ZERO))
        };
        self.bbox_shape(n, bbox, mv);
    }

    /// LineProperties.set_linewidthScaleFactor__ / setShaderUniforms: the line width is `max (1, linewidthScaleFactor) * ContentScale`,
    /// and `ContentScale` alone for a line geometry without applied LineProperties (gl.lineWidth). Rust's solid linetype is 1.
    fn content_scale_lines(
        &self,
        style: Option<StyleParams>,
        geometry_type: u8,
    ) -> Option<StyleParams> {
        let scale = self.layout_camera.content_scale;

        if geometry_type != 1 || scale == 1.0 {
            return style;
        }

        match style {
            Some(StyleParams::Line(l)) => Some(StyleParams::Line(LineParams {
                width: l.width * scale,
                ..l
            })),
            None => Some(StyleParams::Line(LineParams {
                linetype: 1,
                width: scale,
            })),
            other => other,
        }
    }

    /// X3DGroupingNode.set_objects__: a child that is an X3DBoundedObject with bboxDisplay TRUE adds `childNode .getBBoxNode ()` to
    /// the visible objects right after the child itself, whatever the child's `visible`; the bbox node draws the shape in the
    /// parent's space with the child's getBBox (a Transform's includes its matrix, a ScreenGroup's its screen matrix).
    fn child_bbox_display(&mut self, sg: &Sg, c: NodeId, m: &Matrix4, depth: usize) {
        let Some(ty) = sg.type_name(c) else { return };

        // The DIS nodes and GaussianSplats draw their own box (bbox_display, splats).
        let bounded = ty == "Shape"
            || ty == "InstancedShape"
            || ty == "ParticleSystem"
            || ty == "Inline"
            || ty == "StaticGroup"
            || ty == "Viewport"
            || ty == "LayoutGroup"
            || GROUPS.contains(&ty);

        if !bounded || !sg.bool(c, "bboxDisplay", false) {
            return;
        }

        let bbox = self.node_bbox_unfiltered(sg, c, depth + 1);
        self.bbox_shape(c, bbox, m);
    }

    /// getBBox (bbox) of `n` with the `visible` test of the node itself skipped (the bbox node is a visible object even when the
    /// bounded object is not), shadows FALSE.
    fn node_bbox_unfiltered(&mut self, sg: &Sg, n: NodeId, depth: usize) -> Box3 {
        self.node_bbox_inner(sg, n, depth, false)
    }

    /// X3DBBoxNode.traverse: the shared bbox shape in `bbox`'s box.
    pub(super) fn bbox_shape(&mut self, n: NodeId, bbox: Box3, mv: &Matrix4) {
        let (min, max) = bbox.extents();
        let mut extent = max;
        extent.subtract(&min);
        // X3DBBoxNode.getMatrix: `bboxSize = bbox .size .max (ContentScale === 1 ? 0 : 1e-6)`.
        if self.layout_camera.content_scale != 1.0 {
            extent.max(&Vector3::new(1e-6, 1e-6, 1e-6));
        }
        let center = bbox.center();
        let mut local = Matrix4::IDENTITY;
        local.set_transform(Some(&center), None, Some(&extent), None, None);
        let mut m = *mv;
        m.mult_left(&local);

        let geometry = match self.cache.as_deref_mut() {
            Some(c) => c
                .bbox_box
                .get_or_insert_with(|| Rc::new(crate::bbox_display::unit_box_lines()))
                .clone(),
            None => Rc::new(crate::bbox_display::unit_box_lines()),
        };
        let mut distance = Vector3::ZERO;
        m.mult_vec_matrix3(&mut distance);
        let style = self.content_scale_lines(None, 1);
        let style_texture = style.map(|_| crate::style::linetype_texture());

        self.items.push(DrawItem {
            geometry_node: n,
            geometry,
            model_view: m,
            material: default_material(),
            textures: vec![],
            multi_texture: None,
            texture_matrices: vec![IDENTITY_F32],
            projectors: self.local_projectors.clone(),
            lights: self.local_lights.clone(),
            transparent: false,
            alpha_mode: AlphaMode::Opaque,
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
            style,
            style_texture,
            distance: distance.z,
            viewport: (self.viewports.len() > 1).then(|| self.viewport()),
            volume: None,
            instances: None,
            splats: None,
            custom: None,
            fog: self.local_fogs.last().cloned(),
        });
    }

    /// X3DGroupingNode.traverse: lights first (displayNodes push, then visibleObjects), the
    /// ClipPlane children pushed before the children and popped after.
    fn group_children(&mut self, sg: &Sg, kids: Vec<NodeId>, m: &Matrix4, depth: usize) {
        let pushed = self.push_clip_planes(sg, &kids, m);
        // LocalFog.push: an enabled fog with a fogType, in the group's model-view space.
        let fogs = self.local_fogs.len();
        for c in kids
            .iter()
            .filter(|c| sg.type_name(**c) == Some("LocalFog") && sg.bool(**c, "enabled", true))
        {
            if let Some(f) = fog_params(sg, *c, Some(m), &Matrix4::IDENTITY) {
                self.local_fogs.push(f);
            }
        }
        let local = self.local_lights.len();
        let local_projectors = self.local_projectors.len();
        let (lights, rest): (Vec<NodeId>, Vec<NodeId>) = kids.into_iter().partition(|c| {
            matches!(
                sg.type_name(*c),
                Some(
                    "DirectionalLight"
                        | "PointLight"
                        | "SpotLight"
                        | "EnvironmentLight"
                        | "TextureProjector"
                        | "TextureProjectorParallel"
                )
            )
        });
        for c in lights.into_iter().chain(rest) {
            self.display(sg, c, m, depth + 1);
            self.child_bbox_display(sg, c, m, depth);
        }
        // X3DLightNode.pop: the group's local lights end with the group.
        self.local_lights.truncate(local);
        self.local_fogs.truncate(fogs);
        self.local_projectors.truncate(local_projectors);
        self.clip_planes.truncate(self.clip_planes.len() - pushed);
    }

    /// Billboard.matrix: this build's DISPLAY rotation, else the last frame's (the cache), else the identity it starts with.
    fn stored_billboard_matrix(&self, sg: &Sg, n: NodeId) -> Matrix4 {
        self.billboard_matrices
            .get(&n)
            .copied()
            .or_else(|| {
                self.cache
                    .as_deref()
                    .and_then(|c| c.billboard_matrices.get(&(sg.rt.uid, n)).copied())
            })
            .unwrap_or(Matrix4::IDENTITY)
    }

    /// ScreenGroup.matrix: this build's DISPLAY value, else the last frame's (the cache), else the identity (none: the caller adds nothing).
    fn screen_matrix(&self, sg: &Sg, n: NodeId) -> Option<Matrix4> {
        self.screen_matrices.get(&n).copied().or_else(|| {
            self.cache
                .as_deref()
                .and_then(|c| c.screen_matrices.get(&(sg.rt.uid, n)).copied())
        })
    }

    /// The screen matrices of the last DISPLAY traversals of the runtime `uid` (ScreenGroup.matrix), for the viewer's CAMERA walk.
    pub fn screen_matrices_of(cache: &RenderCache, uid: u64) -> HashMap<NodeId, Matrix4> {
        cache
            .screen_matrices
            .iter()
            .filter(|((u, _), _)| *u == uid)
            .map(|((_, n), m)| (*n, *m))
            .collect()
    }

    /// X3DLayoutContext.getScreenScaleMatrix (renderObject, matrix, contentScale, snap): the model-view matrix's axes scaled to
    /// one pixel (viewpoint.getScreenScale, in meter / pixel) at its origin, as a matrix relative to the model-view matrix.
    /// With `snap` the origin is moved to a whole pixel of the viewport. ScreenGroup uses ContentScale and no snap, ScreenText
    /// 1 and snap.
    fn screen_scale_matrix(&self, model_view: &Matrix4, content_scale: f64, snap: bool) -> Matrix4 {
        let mv = &model_view.0;
        let viewport = self.viewport();
        let [_, _, width, height] = viewport;
        let scale = match self.layout_camera.ortho {
            // OrthoViewpoint.getScreenScale
            Some((size_x, size_y)) => {
                if width / height > size_x / size_y {
                    size_y / height
                } else {
                    size_x / width
                }
            }
            // Viewpoint.getScreenScale (also GeoViewpoint): meter / pixel at the depth of the origin.
            None => {
                let size = mv[14].abs() * (self.layout_camera.fov / 2.0).tan() * 2.0;
                if width > height {
                    size / height
                } else {
                    size / width
                }
            }
        } * content_scale;
        let axis = |i: usize| {
            let mut v = Vector3::new(mv[i], mv[i + 1], mv[i + 2]);
            v.normalize();
            v.multiply(scale);
            v
        };
        let (x, y, z) = (axis(0), axis(4), axis(8));
        let mut screen = Matrix4([
            x.x, x.y, x.z, 0.0, y.x, y.y, y.z, 0.0, z.x, z.y, z.z, 0.0, mv[12], mv[13], mv[14], 1.0,
        ]);

        // Snap to whole pixel: ViewVolume.projectPoint, round, ViewVolume.unProjectPoint.
        if snap {
            let mut p = Vector4::new(0.0, 0.0, 0.0, 1.0);
            screen.mult_vec_matrix4(&mut p);
            self.projection.mult_vec_matrix4(&mut p);
            let d = 1.0 / (2.0 * p.w);
            let win = [
                (p.x * d + 0.5) * viewport[2] + viewport[0],
                (p.y * d + 0.5) * viewport[3] + viewport[1],
                p.z * d + 0.5,
            ];
            let (win_x, win_y) = ((win[0] + 0.5).floor(), (win[1] + 0.5).floor());

            let mut inv = screen;
            inv.mult_right(&self.projection);
            inv.inverse();

            let mut v = Vector4::new(
                (win_x - viewport[0]) / viewport[2] * 2.0 - 1.0,
                (win_y - viewport[1]) / viewport[3] * 2.0 - 1.0,
                win[2] * 2.0 - 1.0,
                1.0,
            );
            inv.mult_vec_matrix4(&mut v);
            let d = 1.0 / v.w;

            screen.translate(&Vector3::new(v.x * d, v.y * d, 0.0));
        }

        let mut matrix = *model_view;
        matrix.inverse();
        matrix.mult_left(&screen);
        matrix
    }

    /// The current viewport rectangle (renderObject .getViewVolumes () .at (-1) .getViewport ()).
    fn viewport(&self) -> [f64; 4] {
        self.viewports
            .last()
            .copied()
            .unwrap_or([0.0, 0.0, 1.0, 1.0])
    }

    /// LayoutGroup.traverse (also the group of a LayoutLayer): the Viewport pushed, then the
    /// Layout's matrix around the children.
    #[allow(clippy::too_many_arguments)]
    fn layout_group(
        &mut self,
        sg: &Sg,
        n: NodeId,
        layout: Option<NodeId>,
        viewport: Option<NodeId>,
        kids: Vec<NodeId>,
        mv: &Matrix4,
        depth: usize,
    ) {
        let viewport = viewport.filter(|v| sg.type_name(*v) == Some("Viewport"));
        self.groups.push(ShadowGroup::Node(n));

        if let Some(v) = viewport {
            let rectangle = layer::viewport_rectangle(sg, v, self.viewport());
            self.viewports.push(rectangle);
        }

        match layout.filter(|l| sg.type_name(*l) == Some("Layout")) {
            Some(l) => {
                let node = LayoutNode::read(sg, l);
                let current = self.viewport();
                let m = self.layouts.push(&node, &self.layout_camera, &current, mv);
                self.group_children(sg, kids, &m, depth);
                self.layouts.pop();
            }
            None => self.group_children(sg, kids, mv, depth),
        }

        if viewport.is_some() {
            self.viewports.pop();
        }

        self.groups.pop();
    }

    /// EnvironmentLight: X3DLightNode.set_on__ (on and intensity > 0); only the first one counts
    /// (getMaxEnvironmentLights 1). EnvironmentLightContainer.setGlobalVariables: the rotation part
    /// of the model-view matrix, multLeft (rotation), inverted. Without a specularTexture X_ITE
    /// binds its default white cube maps. A GeneratedCubeMapTexture specularTexture is rendered
    /// at the light's origin (EnvironmentLight.traverse).
    fn environment_light(&mut self, sg: &Sg, n: NodeId, mv: &Matrix4) {
        let specular = sg.node(n, "specularTexture").filter(|t| {
            matches!(
                sg.type_name(*t),
                Some("ComposedCubeMapTexture" | "ImageCubeMapTexture" | "GeneratedCubeMapTexture")
            )
        });

        // EnvironmentLight.traverse (a visible object while its specularTexture is a rendered
        // texture, whether the light is on or not).
        if let Some(t) = specular.filter(|t| sg.type_name(*t) == Some("GeneratedCubeMapTexture")) {
            if !self.dependent
                && sg.string(t, "update", "NONE") != "NONE"
                && sg.i32(t, "size", 128) > 0
            {
                let mut model = *mv;
                model.translate(&sg.vec3(n, "origin", Vector3::ZERO));
                model.mult_right(&self.camera_space);

                match self.generated.iter_mut().find(|(g, _)| *g == t) {
                    Some(e) => e.1 = model,
                    None => self.generated.push((t, model)),
                }
            }
        }

        let intensity = sg.f64(n, "intensity", 1.0).max(0.0);

        if !sg.bool(n, "on", true) || intensity <= 0.0 || self.environment.is_some() {
            return;
        }

        let texture = specular
            .and_then(|t| self.texture(sg, t))
            .unwrap_or_else(|| Rc::new(texture::white_cube()));
        let mut rotation = Rotation4::identity();
        mv.get_transform(None, Some(&mut rotation), None, None, None);
        rotation.mult_left(&sg.rotation(n, "rotation"));
        rotation.inverse();

        self.environment = Some(EnvironmentParams {
            color: sg.color(n, "color", [1.0, 1.0, 1.0]),
            intensity,
            rotation: rotation.get_matrix().0.map(|v| v as f32),
            specular: texture,
        });
    }

    fn light(&mut self, sg: &Sg, n: NodeId, ty: &str, mv: &Matrix4) {
        // X3DLightNode.set_on__: no push when off or intensity <= 0.
        let intensity = sg.f64(n, "intensity", 1.0).max(0.0);
        if !sg.bool(n, "on", true) || intensity <= 0.0 {
            return;
        }

        let color = sg.color(n, "color", [1.0, 1.0, 1.0]);
        let ambient_intensity = sg.f64(n, "ambientIntensity", 0.0).clamp(0.0, 1.0);

        // X3DLightNode.push: a global light joins renderObject.getGlobalLights (every shape of the
        // layer), a local one the local objects of the group (its shapes until the group's pop).
        let index = self.lights.len() as u32;
        if sg.bool(n, "global", ty != "DirectionalLight") {
            self.global_lights.push(index);
        } else {
            self.local_lights.push(index);
        }

        let global = sg.bool(n, "global", ty != "DirectionalLight");
        let shadow = self.light_shadow(sg, n, ty, mv, global);

        match ty {
            "DirectionalLight" => {
                // DirectionalLightContainer.setGlobalVariables.
                let mut d = sg.vec3(n, "direction", Vector3::new(0.0, 0.0, -1.0));
                mv.mult_dir_matrix(&mut d);
                d.normalize();
                self.lights.push(LightParams {
                    kind: 1,
                    color,
                    intensity,
                    ambient_intensity,
                    direction: d,
                    location: Vector3::ZERO,
                    attenuation: [0.0; 3],
                    radius: -1.0,
                    cut_off_angle: 0.0,
                    beam_width: 0.0,
                    matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                    shadow: None,
                });
            }
            _ => {
                // PointLightContainer / SpotLightContainer.setGlobalVariables.
                let mut inv = mv.submatrix();
                inv.inverse();
                let matrix = inv.0.map(|v| v as f32);
                let mut location = sg.vec3(n, "location", Vector3::ZERO);
                mv.mult_vec_matrix3(&mut location);
                let a = sg.vec3(n, "attenuation", Vector3::new(1.0, 0.0, 0.0));
                let mut direction = sg.vec3(n, "direction", Vector3::new(0.0, 0.0, -1.0));
                mv.mult_dir_matrix(&mut direction);
                direction.normalize();
                let spot = ty == "SpotLight";
                self.lights.push(LightParams {
                    kind: if spot { 3 } else { 2 },
                    color,
                    intensity,
                    ambient_intensity,
                    direction,
                    location,
                    attenuation: [a.x.max(0.0), a.y.max(0.0), a.z.max(0.0)],
                    radius: sg.f64(n, "radius", 100.0),
                    cut_off_angle: if spot {
                        sg.f64(n, "cutOffAngle", SPOT_CUT_OFF_ANGLE_DEFAULT)
                            .clamp(0.0, std::f64::consts::FRAC_PI_2)
                    } else {
                        0.0
                    },
                    beam_width: if spot {
                        sg.f64(n, "beamWidth", SPOT_BEAM_WIDTH_DEFAULT)
                            .clamp(0.0, std::f64::consts::FRAC_PI_2)
                    } else {
                        0.0
                    },
                    matrix,
                    shadow: None,
                });
            }
        }

        if let Some(l) = self.lights.last_mut() {
            l.shadow = shadow;
        }
    }

    /// X3DTextureProjectorNode.push and its container's renderShadowMap / setGlobalVariables: the
    /// projector's texture space (location, direction straightened to upVector, perspective of
    /// fieldOfView or the TextureProjectorParallel's rectangle, biased, the texture's flipY
    /// matrix) as seen from the view.
    fn projector(&mut self, sg: &Sg, n: NodeId, ty: &str, mv: &Matrix4) {
        // set_on__: on, intensity > 0 and an X3DTexture2DNode texture (setEnabled).
        let intensity = sg.f64(n, "intensity", 1.0).max(0.0);
        let texture_node = sg.node(n, "texture").filter(|t| {
            matches!(
                sg.type_name(*t),
                Some("ImageTexture" | "PixelTexture" | "MovieTexture" | "RenderedTexture")
            )
        });

        if !sg.bool(n, "on", true) || intensity <= 0.0 {
            return;
        }

        let Some(texture_node) = texture_node else {
            return;
        };
        let Some(texture) = self.texture(sg, texture_node) else {
            return;
        };
        let global = sg.bool(n, "global", true);
        let mut model = *mv;
        model.mult_right(&self.camera_space);

        let mut inv = if global { model } else { Matrix4::IDENTITY };
        let mut direction = sg.vec3(n, "direction", Vector3::new(0.0, 0.0, 1.0));
        direction.negate();
        let mut rotation = Rotation4::identity();
        rotation.set_vectors(&Vector3::Z_AXIS, &direction);
        let rotation = straighten(
            &rotation,
            &sg.vec3(n, "upVector", Vector3::new(0.0, 1.0, 0.0)),
        );
        let location = sg.vec3(n, "location", Vector3::ZERO);
        inv.translate(&location);
        inv.rotate(&rotation);
        inv.inverse();

        // set_nearDistance__ / set_farDistance__.
        let near_field = sg.f64(n, "nearDistance", -1.0);
        let far_field = sg.f64(n, "farDistance", -1.0);
        let (near, near_parameter) = if near_field < 0.0 {
            (0.125, 0.0)
        } else {
            (near_field, -1.0)
        };
        let (far, far_parameter) = if far_field < 0.0 {
            (100_000.0, 1.0)
        } else {
            (far_field, 2.0)
        };
        let (width, height) = (texture.width.max(1) as f64, texture.height.max(1) as f64);

        let projection = if ty == "TextureProjector" {
            // getFieldOfView: in (0, π), else π/4; Camera.perspective with the texture's size.
            let fov = sg.f64(n, "fieldOfView", std::f64::consts::FRAC_PI_4);
            let fov = if fov > 0.0 && fov < std::f64::consts::PI {
                fov
            } else {
                std::f64::consts::FRAC_PI_4
            };
            let ratio = (fov / 2.0).tan() * near;
            if width > height {
                let aspect = width * ratio / height;
                frustum(-aspect, aspect, -ratio, ratio, near, far)
            } else {
                let aspect = height * ratio / width;
                frustum(-ratio, ratio, -aspect, aspect, near, far)
            }
        } else {
            let f = sg.vec4(
                n,
                "fieldOfView",
                x3d_fields::math::Vector4::new(-1.0, -1.0, 1.0, 1.0),
            );
            let (min_x, min_y, max_x, max_y) = (f.x, f.y, f.z, f.w);
            let (size_x, size_y) = (max_x - min_x, max_y - min_y);
            let ratio = width / height;
            if ratio > size_x / size_y {
                let center = (min_x + max_x) / 2.0;
                let size1_2 = (size_y * ratio) / 2.0;
                ortho(center - size1_2, center + size1_2, min_y, max_y, near, far)
            } else {
                let center = (min_y + max_y) / 2.0;
                let size1_2 = (size_x / ratio) / 2.0;
                ortho(min_x, max_x, center - size1_2, center + size1_2, near, far)
            }
        };

        if !global {
            let mut inverse_model = model;
            inverse_model.inverse();
            inv.mult_left(&inverse_model);
        }

        // X3DSingleTextureNode.getMatrix: identity or the flipY matrix (ImageTexture, MovieTexture).
        let texture_matrix = if texture.flip_y {
            Matrix4([
                1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0,
            ])
        } else {
            Matrix4::IDENTITY
        };
        inv.mult_right(&projection)
            .mult_right(&SHADOW_BIAS_MATRIX)
            .mult_right(&texture_matrix);

        // The location: computed once, in the pass that renders the container first (a texture
        // pass keeps the main pass's value).
        let mut view_location = location;
        mv.mult_vec_matrix3(&mut view_location);
        let close = |a: &Matrix4, b: &Matrix4| {
            a.0.iter()
                .zip(b.0.iter())
                .all(|(x, y)| (x - y).abs() <= 1e-9 * (1.0 + x.abs()))
        };
        if self.dependent {
            if let Some((_, _, l)) = self
                .projector_sources
                .iter()
                .find(|(node, m, _)| *node == n && close(m, &model))
            {
                view_location = *l;
            }
        } else {
            self.projector_sources.push((n, model, view_location));
        }

        let mut matrix = self.camera_space;
        matrix.mult_right(&inv);
        let index = self.projectors.len() as u32;
        if global {
            self.global_projectors.push(index);
        } else {
            self.local_projectors.push(index);
        }

        self.projectors.push(ProjectorParams {
            texture,
            color: sg.color(n, "color", [1.0, 1.0, 1.0]),
            intensity,
            location: view_location,
            params: [near_parameter, far_parameter],
            matrix,
            linear: false,
        });
    }

    /// X3DLightNode.getShadowIntensity / getShadowMapSize and the container's renderShadowMap:
    /// the light's shadow map (the shadow-casting shapes of its group seen from the light) and
    /// the shadow uniforms. Not in a texture pass (a dependent renderer).
    fn light_shadow(
        &mut self,
        sg: &Sg,
        n: NodeId,
        ty: &str,
        mv: &Matrix4,
        global: bool,
    ) -> Option<ShadowParams> {
        let intensity = if sg.bool(n, "shadows", false) {
            sg.f64(n, "shadowIntensity", 1.0).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let size = sg
            .i32(n, "shadowMapSize", 1024)
            .clamp(0, MAX_SHADOW_MAP_SIZE as i32) as u32;

        if intensity <= 0.0 || size == 0 {
            return None;
        }

        let mut model = *mv;
        model.mult_right(&self.camera_space);

        // DependentRenderer.getLightContainer: the main pass's container, its map rendered once
        // (shadowMapRendered); setGlobalVariables with the texture pass's cameraSpaceMatrix.
        if self.dependent {
            let close = |a: &Matrix4, b: &Matrix4| {
                a.0.iter()
                    .zip(b.0.iter())
                    .all(|(x, y)| (x - y).abs() <= 1e-9 * (1.0 + x.abs()))
            };
            let (_, _, map_matrix, params) = self
                .shadow_sources
                .iter()
                .find(|(node, m, _, _)| *node == n && close(m, &model))?;
            let mut matrix = self.camera_space;
            matrix.mult_right(map_matrix);
            return Some(ShadowParams {
                matrix,
                ..params.clone()
            });
        }

        let group = if global {
            ShadowGroup::Roots(self.layer_roots.clone())
        } else {
            self.groups
                .last()
                .cloned()
                .unwrap_or_else(|| ShadowGroup::Roots(self.layer_roots.clone()))
        };
        let mut inv_light_space = if global { model } else { Matrix4::IDENTITY };
        let direction = sg.vec3(n, "direction", Vector3::new(0.0, 0.0, -1.0));
        let mut negated = direction;
        negated.negate();
        let mut rotation = Rotation4::identity();
        rotation.set_vectors(&Vector3::Z_AXIS, &negated);

        let mut views = vec![];
        let mut map_matrix;

        match ty {
            "DirectionalLight" => {
                inv_light_space.rotate(&rotation);
                inv_light_space.inverse();
                let mut light_bbox = self.shadow_bbox(sg, &group);
                light_bbox.mult_right(&inv_light_space);
                let (min, max) = light_bbox.extents();
                let projection = ortho(min.x, max.x, min.y, max.y, -max.z, -min.z);
                views.push(shadow_view(
                    projection,
                    [0, 0, size, size],
                    self.shadow_items(sg, &group, &inv_light_space),
                ));
                map_matrix = inv_light_space;
                if !global {
                    let mut inv = model;
                    inv.inverse();
                    map_matrix.mult_left(&inv);
                }
                map_matrix
                    .mult_right(&projection)
                    .mult_right(&SHADOW_BIAS_MATRIX);
            }
            "SpotLight" => {
                inv_light_space.translate(&sg.vec3(n, "location", Vector3::ZERO));
                inv_light_space.rotate(&rotation);
                inv_light_space.inverse();
                let mut light_bbox = self.shadow_bbox(sg, &group);
                light_bbox.mult_right(&inv_light_space);
                let (min, _) = light_bbox.extents();
                let far = sg.f64(n, "radius", 100.0).min(-min.z);
                // SpotLightContainer.renderShadow: no shadow without anything in front of the light.
                if !(far > 0.0) {
                    return None;
                }
                let cut_off_angle = sg
                    .f64(n, "cutOffAngle", SPOT_CUT_OFF_ANGLE_DEFAULT)
                    .clamp(0.0, std::f64::consts::FRAC_PI_2);
                // Camera.perspective (fieldOfView, 0.125, max (10000, far), size, size).
                let ratio = (cut_off_angle).tan() * 0.125;
                let projection = frustum(-ratio, ratio, -ratio, ratio, 0.125, far.max(10000.0));
                views.push(shadow_view(
                    projection,
                    [0, 0, size, size],
                    self.shadow_items(sg, &group, &inv_light_space),
                ));
                map_matrix = inv_light_space;
                if !global {
                    let mut inv = model;
                    inv.inverse();
                    map_matrix.mult_left(&inv);
                }
                map_matrix
                    .mult_right(&projection)
                    .mult_right(&SHADOW_BIAS_MATRIX);
            }
            _ => {
                // PointLightContainer: six 90° views (perspective2) into the compact layout.
                inv_light_space.translate(&sg.vec3(n, "location", Vector3::ZERO));
                inv_light_space.inverse();
                const AXES: [(f64, f64, f64); 6] = [
                    (1.0, 0.0, 0.0),
                    (-1.0, 0.0, 0.0),
                    (0.0, 0.0, -1.0),
                    (0.0, 0.0, 1.0),
                    (0.0, 1.0, 0.0),
                    (0.0, -1.0, 0.0),
                ];
                const VIEWPORTS: [[f64; 4]; 6] = [
                    [0.0, 0.5, 0.25, 0.5],
                    [0.5, 0.5, 0.25, 0.5],
                    [0.75, 0.5, 0.25, 0.5],
                    [0.25, 0.5, 0.25, 0.5],
                    [0.0, 0.0, 0.5, 0.5],
                    [0.5, 0.0, 0.5, 0.5],
                ];
                let ratio = std::f64::consts::FRAC_PI_4.tan() * 0.125;
                let projection = frustum(-ratio, ratio, -ratio, ratio, 0.125, 10_000.0);

                for (axis, v) in AXES.iter().zip(VIEWPORTS) {
                    let mut r = Rotation4::identity();
                    r.set_vectors(&Vector3::new(axis.0, axis.1, axis.2), &Vector3::Z_AXIS);
                    let mut orientation = Matrix4::IDENTITY;
                    orientation.rotate(&r);
                    // push (orientationMatrices [i]), multLeft (invLightSpaceMatrix).
                    let mut m = orientation;
                    m.mult_left(&inv_light_space);
                    let s = size as f64;
                    let viewport = [
                        (v[0] * s) as u32,
                        (v[1] * s) as u32,
                        (v[2] * s) as u32,
                        (v[3] * s) as u32,
                    ];
                    views.push(shadow_view(
                        projection,
                        viewport,
                        self.shadow_items(sg, &group, &m),
                    ));
                }

                map_matrix = inv_light_space;
                if !global {
                    let mut inv = model;
                    inv.inverse();
                    map_matrix.mult_left(&inv);
                }
            }
        }

        // setGlobalVariables: shadowMatrix = cameraSpaceMatrix · invLightSpaceProjectionMatrix.
        let mut matrix = self.camera_space;
        matrix.mult_right(&map_matrix);
        self.shadow_maps.push(ShadowMap { size, views });

        let params = ShadowParams {
            color: sg.color(n, "shadowColor", [0.0, 0.0, 0.0]),
            intensity,
            bias: sg.f64(n, "shadowBias", 0.005).clamp(0.0, 1.0),
            map_size: size,
            matrix,
            map: self.shadow_maps.len() - 1,
        };
        self.shadow_sources
            .push((n, model, map_matrix, params.clone()));
        Some(params)
    }

    /// groupNode .getSubBBox (bbox, true): the shadow-casting shapes of the group in its own space.
    fn shadow_bbox(&mut self, sg: &Sg, group: &ShadowGroup) -> Box3 {
        let kids = match group {
            ShadowGroup::Node(n) => {
                let ty = sg.type_name(*n).unwrap_or("");
                let kids = children_of(sg, *n, ty);
                if ty == "LOD" {
                    self.lod_kids(sg, *n, &Matrix4::IDENTITY, kids)
                } else {
                    kids
                }
            }
            ShadowGroup::Roots(r) => r.clone(),
        };
        self.bbox_of(sg, &kids, 0, true)
    }

    /// X3DBoundedObject.getBBox (nodes, bbox, shadows): `shadows` TRUE for the shadow bounds (a Shape only when it casts
    /// shadows), FALSE for bboxDisplay (X3DBBoxNode.getMatrix calls getBBox (bbox)).
    fn bbox_of(&mut self, sg: &Sg, nodes: &[NodeId], depth: usize, shadows: bool) -> Box3 {
        let mut bbox = Box3::EMPTY;

        for &c in nodes {
            bbox.add(&self.node_bbox(sg, c, depth + 1, shadows));
        }

        bbox
    }

    /// getBBox (bbox, true) of one child: a Shape only when it casts shadows; a grouping node its
    /// children (in its own matrix) or its bboxSize.
    fn node_bbox(&mut self, sg: &Sg, n: NodeId, depth: usize, shadows: bool) -> Box3 {
        if depth > 256 || !sg.bool(n, "visible", true) {
            return Box3::EMPTY;
        }

        self.node_bbox_inner(sg, n, depth, shadows)
    }

    fn node_bbox_inner(&mut self, sg: &Sg, n: NodeId, depth: usize, shadows: bool) -> Box3 {
        if depth > 256 {
            return Box3::EMPTY;
        }

        let Some(ty) = sg.type_name(n) else {
            return Box3::EMPTY;
        };
        let default_size = |sg: &Sg| {
            sg.vec3(n, "bboxSize", Vector3::new(-1.0, -1.0, -1.0))
                .equals(&Vector3::new(-1.0, -1.0, -1.0))
        };
        let own = |sg: &Sg| {
            Box3::new(
                &sg.vec3(n, "bboxSize", Vector3::ZERO),
                &sg.vec3(n, "bboxCenter", Vector3::ZERO),
            )
        };

        match ty {
            "Shape" => {
                if shadows && !sg.bool(n, "castShadow", true) {
                    return Box3::EMPTY;
                }
                if !default_size(sg) {
                    return own(sg);
                }
                match sg.node(n, "geometry").and_then(|g| self.geometry(sg, g)) {
                    Some(g) if g.vertex_count() > 0 => Box3::new(&g.bbox_size(), &g.bbox_center()),
                    _ => Box3::EMPTY,
                }
            }
            // InstancedShape.set_bbox__ (an empty instance list is not enabled: no bounded object).
            "InstancedShape" => {
                if shadows && !sg.bool(n, "castShadow", true) {
                    return Box3::EMPTY;
                }
                let instances = instance_data(sg, n);
                if instances.matrices.is_empty() {
                    return if default_size(sg) {
                        Box3::EMPTY
                    } else {
                        own(sg)
                    };
                }
                match sg.node(n, "geometry").and_then(|g| self.geometry(sg, g)) {
                    Some(g) => instanced_bbox(sg, n, &g, &instances),
                    None => Box3::EMPTY,
                }
            }
            // X3DShapeNode.getBBox of a ParticleSystem (a bounded object only when enabled or with a bboxSize).
            "ParticleSystem" => {
                if shadows && !sg.bool(n, "castShadow", true) {
                    return Box3::EMPTY;
                }
                if !default_size(sg) {
                    return own(sg);
                }
                if !self.particle_system_enabled(sg, n) {
                    return Box3::EMPTY;
                }
                self.particle_bbox(sg, n)
            }
            "Inline" => {
                if !default_size(sg) {
                    return own(sg);
                }
                match sg.inline_scene(n).filter(|_| sg.bool(n, "load", true)) {
                    Some(ctx) => self.bbox_of(sg, &sg.roots_of(ctx), depth, shadows),
                    None => Box3::EMPTY,
                }
            }
            "StaticGroup" => self.bbox_of(sg, &sg.nodes(n, "children"), depth, shadows),
            _ if GROUPS.contains(&ty) || matches!(ty, "Viewport" | "LayoutGroup") => {
                if !default_size(sg) {
                    return own(sg);
                }
                let kids = children_of(sg, n, ty);
                let kids = if ty == "LOD" {
                    self.lod_kids(sg, n, &Matrix4::IDENTITY, kids)
                } else {
                    kids
                };
                let mut bbox = self.bbox_of(sg, &kids, depth, shadows);
                if let Some(m) = sg.group_matrix(n, ty) {
                    bbox.mult_right(&m);
                } else if ty == "Billboard" {
                    // Billboard.getBBox: `getSubBBox (bbox, shadows) .multRight (this .matrix)`.
                    bbox.mult_right(&self.stored_billboard_matrix(sg, n));
                } else if let Some(m) = self.screen_matrix(sg, n) {
                    bbox.mult_right(&m);
                }
                bbox
            }
            _ => Box3::EMPTY,
        }
    }

    /// renderObject.render (TraverseType.SHADOW, groupNode.traverse): the shadow objects of the
    /// group with `model_view` (the light space); its clip planes, not the ones around it.
    fn shadow_items(
        &mut self,
        sg: &Sg,
        group: &ShadowGroup,
        model_view: &Matrix4,
    ) -> Vec<DepthItem> {
        let saved = std::mem::take(&mut self.clip_planes);
        let mut items = vec![];

        match group {
            ShadowGroup::Node(n) => self.shadow_display(sg, *n, model_view, 0, &mut items),
            ShadowGroup::Roots(roots) => {
                let pushed = self.push_clip_planes(sg, roots, model_view);
                for r in roots {
                    self.shadow_display(sg, *r, model_view, 1, &mut items);
                }
                self.clip_planes.truncate(self.clip_planes.len() - pushed);
            }
        }

        self.clip_planes = saved;
        items
    }

    /// X3DGroupingNode.traverse SHADOW: the visible shadow objects (a Shape with castShadow).
    fn shadow_display(
        &mut self,
        sg: &Sg,
        n: NodeId,
        mv: &Matrix4,
        depth: usize,
        out: &mut Vec<DepthItem>,
    ) {
        if depth > 256 || !sg.bool(n, "visible", true) {
            return;
        }

        let Some(ty) = sg.type_name(n) else { return };

        match ty {
            "Shape" => {
                if !sg.bool(n, "castShadow", true) {
                    return;
                }
                if let Some(geometry) = sg.node(n, "geometry").and_then(|g| self.geometry(sg, g)) {
                    out.push(DepthItem {
                        geometry,
                        model_view: *mv,
                        clip_planes: self
                            .clip_planes
                            .iter()
                            .take(MAX_CLIP_PLANES)
                            .copied()
                            .collect(),
                        instances: None,
                        line_width: 1.0,
                        thick_lines: None,
                    });
                }
            }
            // ParticleSystem.displaySimple: the particles drawn instanced into the shadow map. LINE particles with
            // LineProperties (setShaderUniforms): gl.lineWidth, or above ALIASED_LINE_WIDTH_RANGE [1]
            // displaySimpleInstancedThick, the triangles of the last color pass before this layer's (X3DRenderObject.draw
            // renders the shadow maps first) while their line count is the current one, and nothing otherwise.
            "ParticleSystem" => {
                if !sg.bool(n, "castShadow", true) {
                    return;
                }
                let line_width = match sg
                    .node(n, "appearance")
                    .and_then(|a| style_params(sg, a, 1))
                {
                    Some(StyleParams::Line(l))
                        if sg.rt.particle_geometry_type(n) == GeometryType::Line =>
                    {
                        l.width
                    }
                    _ => 1.0,
                };
                let clip_planes: Vec<[f64; 4]> = self
                    .clip_planes
                    .iter()
                    .take(MAX_CLIP_PLANES)
                    .copied()
                    .collect();

                if line_width > crate::gpu::MAX_LINE_WIDTH {
                    if let Some(lines) = self.particle_thick_lines_before_layer(sg, n) {
                        out.push(DepthItem {
                            geometry: Rc::new(Geometry {
                                geometry_type: 2,
                                ..Default::default()
                            }),
                            model_view: *mv,
                            clip_planes,
                            instances: None,
                            line_width: 1.0,
                            thick_lines: Some(lines),
                        });
                    }
                    return;
                }
                if let Some((geometry, instances, _)) = self.particle_draw(sg, n, mv) {
                    out.push(DepthItem {
                        geometry,
                        model_view: *mv,
                        clip_planes,
                        instances: Some(instances),
                        line_width,
                        thick_lines: None,
                    });
                }
            }
            // InstancedShape.displaySimple: the depth shader with X3D_INSTANCING.
            "InstancedShape" => {
                if !sg.bool(n, "castShadow", true) {
                    return;
                }
                let instances = instance_data(sg, n);
                if let (Some(geometry), false) = (
                    sg.node(n, "geometry").and_then(|g| self.geometry(sg, g)),
                    instances.matrices.is_empty(),
                ) {
                    out.push(DepthItem {
                        geometry,
                        model_view: *mv,
                        clip_planes: self
                            .clip_planes
                            .iter()
                            .take(MAX_CLIP_PLANES)
                            .copied()
                            .collect(),
                        instances: Some(Rc::new(instances)),
                        line_width: 1.0,
                        thick_lines: None,
                    });
                }
            }
            "StaticGroup" => {
                let mut shapes = vec![];
                sg.static_shapes(n, &Matrix4::IDENTITY, &mut shapes, depth);
                for (s, mut m) in shapes {
                    m.mult_right(mv);
                    self.shadow_display(sg, s, &m, depth + 1, out);
                }
            }
            "Inline" => {
                if let Some(ctx) = sg.inline_scene(n).filter(|_| sg.bool(n, "load", true)) {
                    let roots = sg.roots_of(ctx);
                    let pushed = self.push_clip_planes(sg, &roots, mv);
                    for r in roots {
                        self.shadow_display(sg, r, mv, depth + 1, out);
                    }
                    self.clip_planes.truncate(self.clip_planes.len() - pushed);
                }
            }
            _ if GROUPS.contains(&ty) || matches!(ty, "Viewport" | "LayoutGroup") => {
                let mut m = *mv;
                if let Some(t) = sg.group_matrix(n, ty) {
                    m.mult_left(&t);
                } else if ty == "Billboard" {
                    // Billboard.traverse (SHADOW): `modelViewMatrix .multLeft (this .matrix)`, no rotation towards the light.
                    m.mult_left(&self.stored_billboard_matrix(sg, n));
                } else if let Some(t) = self.screen_matrix(sg, n) {
                    m.mult_left(&t);
                }
                let kids = children_of(sg, n, ty);
                let kids = if ty == "LOD" {
                    self.lod_kids(sg, n, &m, kids)
                } else {
                    kids
                };
                let pushed = self.push_clip_planes(sg, &kids, &m);
                for c in kids {
                    self.shadow_display(sg, c, &m, depth + 1, out);
                }
                self.clip_planes.truncate(self.clip_planes.len() - pushed);
            }
            _ => {}
        }
    }

    fn white_texture(&mut self) -> Rc<TextureImage> {
        match self.cache.as_deref_mut() {
            Some(c) => c
                .white
                .get_or_insert_with(|| Rc::new(TextureImage::white()))
                .clone(),
            None => Rc::new(TextureImage::white()),
        }
    }

    fn texture(&mut self, sg: &Sg, n: NodeId) -> Option<Rc<TextureImage>> {
        if let Some(t) = self.textures.get(&key(sg, n)) {
            return t.clone();
        }

        // RenderedTexture isTransparent: setTransparent (background .isTransparent ()) of a
        // renderTexture of an earlier frame (the one of this frame comes after the shapes are
        // sorted); part of the cache fingerprint.
        let rendered_transparent = sg.type_name(n) == Some("RenderedTexture") && {
            let before = sg
                .rt
                .rendered_texture_first(n)
                .is_some_and(|f| f < sg.rt.now * 1000.0);
            before
                && rendered_children(sg, n)
                    .background
                    .map_or(self.background_transparent, |b| {
                        background_transparent(sg, b)
                    })
        };

        let persistent = self.cache.is_some();
        let (ckey, fp) = if persistent {
            (
                (sg.rt.uid, n),
                fingerprint(sg, n) ^ rendered_transparent as u64,
            )
        } else {
            ((0, 0), 0)
        };

        if let Some(c) = self.cache.as_deref_mut() {
            let frame = c.frame;

            if let Some(e) = c.textures.get_mut(&ckey) {
                if e.fingerprint == fp {
                    e.used = frame;
                    let t = e.value.clone();
                    self.textures.insert(key(sg, n), t.clone());
                    return t;
                }
            }
        }
        let ty = sg.type_name(n).unwrap_or("");
        let t = match ty {
            "ImageTexture" => {
                // X3DExecutionContext.getBaseURL: the world URL of the node's own scene (an
                // Inline's scene has the Inline's resolved URL).
                let base = sg
                    .rt
                    .scene
                    .world_url_of(sg.rt.scene.node(n).ctx)
                    .to_string();
                texture::load_image_texture(sg, n, &base, self.root.fetcher.as_ref()).map(Rc::new)
            }
            "PixelTexture" => texture::pixel_texture(sg, n).map(Rc::new),
            "ComposedCubeMapTexture" => {
                // Slot textures that are COMPLETE: an ImageTexture by its load state, a PixelTexture
                // with a valid image.
                let slots = sg.rt.composed_cube_faces(n).unwrap_or([None; 6]);
                let mut faces: HashMap<NodeId, Option<Rc<TextureImage>>> = HashMap::new();
                for f in slots.into_iter().flatten() {
                    let complete = match sg.type_name(f) {
                        Some("ImageTexture") => {
                            sg.rt.url_load_state(f) == x3d_runtime::nodes::LoadState::Complete
                        }
                        Some("PixelTexture") => true,
                        _ => false,
                    };
                    let image = if complete { self.texture(sg, f) } else { None };
                    faces.insert(f, image);
                }
                Some(Rc::new(texture::composed_cube(sg, n, slots, |f| {
                    faces.get(&f).cloned().flatten()
                })))
            }
            "GeneratedCubeMapTexture" => {
                // set_size__: no framebuffer for size <= 0 (the cleared texture).
                let size = sg.i32(n, "size", 128);
                Some(Rc::new(if size > 0 {
                    texture::generated_cube(sg, n, size as u32, self.background_transparent)
                } else {
                    texture::white_cube()
                }))
            }
            "ImageCubeMapTexture" => {
                let base = sg
                    .rt
                    .scene
                    .world_url_of(sg.rt.scene.node(n).ctx)
                    .to_string();
                // Until a url loads, the cleared texture (X3DEnvironmentTextureNode.initialize).
                Some(Rc::new(
                    texture::load_image_cube(sg, n, &base, self.root.fetcher.as_ref())
                        .unwrap_or_else(texture::white_cube),
                ))
            }
            "RenderedTexture" => Some(Rc::new(texture::rendered_texture(
                sg,
                n,
                rendered_transparent,
            ))),
            "PixelTexture3D" => Some(Rc::new(texture::pixel_texture_3d(sg, n))),
            // ImageTexture3D: the volume the runtime's NRRD / DICOM parser produced; until one loads, the cleared texture.
            "ImageTexture3D" => Some(Rc::new(match sg.rt.image_texture_3d_volume(n) {
                Some(v) => texture::image_texture_3d(sg, n, &v),
                None => texture::white_3d(),
            })),
            "ComposedTexture3D" => {
                // The X3DTexture2DNodes of `texture` with their images once COMPLETE
                // (checkLoadState, see Runtime::texture_2d_complete).
                let mut slices = vec![];
                for c in sg.nodes(n, "texture") {
                    let Some(c) = sg.inner(c).filter(|c| {
                        matches!(
                            sg.type_name(*c),
                            Some(
                                "ImageTexture"
                                    | "PixelTexture"
                                    | "MovieTexture"
                                    | "RenderedTexture"
                            )
                        )
                    }) else {
                        continue;
                    };
                    let complete = sg.rt.texture_2d_complete(c);
                    // An empty PixelTexture is complete with the cleared 1 × 1 texture.
                    let image = if complete {
                        self.texture(sg, c)
                            .or_else(|| Some(Rc::new(TextureImage::white())))
                    } else {
                        None
                    };
                    slices.push((c, image));
                }
                // The size warning is console output of the event pass, not drawn.
                Some(Rc::new(texture::composed_texture_3d(
                    sg,
                    n,
                    &slices,
                    &mut |_| {},
                )))
            }
            "ImageTextureAtlas" => {
                let base = sg
                    .rt
                    .scene
                    .world_url_of(sg.rt.scene.node(n).ctx)
                    .to_string();
                Some(Rc::new(
                    texture::load_image_texture_atlas(sg, n, &base, self.root.fetcher.as_ref())
                        .unwrap_or_else(texture::white_3d),
                ))
            }
            other => {
                self.unsupported.insert(other.to_string());
                None
            }
        };
        self.textures.insert(key(sg, n), t.clone());

        if let Some(c) = self.cache.as_deref_mut() {
            let frame = c.frame;
            c.textures.insert(
                ckey,
                CacheEntry {
                    fingerprint: fp,
                    value: t.clone(),
                    used: frame,
                },
            );
        }

        t
    }

    fn geometry(&mut self, sg: &Sg, n: NodeId) -> Option<Rc<Geometry>> {
        if let Some(g) = self.geometries.get(&key(sg, n)) {
            return g.clone();
        }
        let persistent = self.cache.is_some();
        let (ckey, fp) = if persistent {
            ((sg.rt.uid, n), fingerprint(sg, n))
        } else {
            ((0, 0), 0)
        };

        if let Some(c) = self.cache.as_deref_mut() {
            let frame = c.frame;

            if let Some(e) = c.geometries.get_mut(&ckey) {
                if e.fingerprint == fp {
                    e.used = frame;
                    c.reused += usize::from(!self.cache_query);
                    let g = e.value.clone();
                    self.geometries.insert(key(sg, n), g.clone());
                    return g;
                }
            }
        }

        let g = self
            .build_geometry(sg, n)
            .filter(|g| g.vertex_count() > 0)
            .map(Rc::new);
        self.geometries.insert(key(sg, n), g.clone());

        // A query reads the cache; only a frame's build fills it.
        let query = self.cache_query;

        if let Some(c) = self.cache.as_deref_mut().filter(|_| !query) {
            let frame = c.frame;
            c.built += 1;
            c.geometries.insert(
                ckey,
                CacheEntry {
                    fingerprint: fp,
                    value: g.clone(),
                    used: frame,
                },
            );
        }

        g
    }

    fn colors(&self, sg: &Sg, n: NodeId) -> Option<(Vec<f32>, usize)> {
        let c = sg.node(n, "color")?;
        match sg.type_name(c)? {
            "Color" => Some((sg.f32s(c, "color").to_vec(), 3)),
            "ColorRGBA" => Some((sg.f32s(c, "color").to_vec(), 4)),
            _ => None,
        }
    }

    /// The texCoord, tangent, fogCoord and attrib children of a geometry node.
    fn extras<'s>(&self, sg: &'s Sg, n: NodeId) -> ExtrasOwned<'s> {
        use crate::extras::{Attrib, TexChannel, generator_mode};
        let string = |c: NodeId, f: &str| -> &'s str {
            match sg.value(c, f) {
                Some(FieldValue::SFString(s)) => s.as_str(),
                _ => "",
            }
        };
        let single = |c: NodeId| -> Option<TexChannel<'s>> {
            match sg.type_name(c)? {
                "TextureCoordinate" => Some(TexChannel::Point {
                    values: sg.f32s(c, "point"),
                    components: 2,
                    mapping: string(c, "mapping"),
                }),
                "TextureCoordinate3D" => Some(TexChannel::Point {
                    values: sg.f32s(c, "point"),
                    components: 3,
                    mapping: string(c, "mapping"),
                }),
                "TextureCoordinate4D" => Some(TexChannel::Point {
                    values: sg.f32s(c, "point"),
                    components: 4,
                    mapping: string(c, "mapping"),
                }),
                "TextureCoordinateGenerator" => {
                    let mut parameter = [0.0f32; 6];
                    for (p, v) in parameter.iter_mut().zip(sg.f32s(c, "parameter")) {
                        *p = *v;
                    }
                    Some(TexChannel::Generator {
                        mode: generator_mode(string(c, "mode")),
                        parameter,
                        mapping: string(c, "mapping"),
                    })
                }
                _ => None,
            }
        };
        let tex = sg.node(n, "texCoord").and_then(|t| match sg.type_name(t)? {
            "MultiTextureCoordinate" => Some(
                sg.nodes(t, "texCoord")
                    .into_iter()
                    .filter_map(single)
                    .collect(),
            ),
            _ => single(t).map(|c| vec![c]),
        });
        let tangent = sg
            .node(n, "tangent")
            .filter(|t| sg.type_name(*t) == Some("Tangent"))
            .map(|t| sg.f32s(t, "vector"));
        let fog = sg
            .node(n, "fogCoord")
            .filter(|t| sg.type_name(*t) == Some("FogCoordinate"))
            .map(|t| sg.f32s(t, "depth"));
        let attribs = sg
            .nodes(n, "attrib")
            .into_iter()
            .filter_map(|a| match sg.type_name(a)? {
                "FloatVertexAttribute" => Some(Attrib::Float {
                    name: string(a, "name"),
                    components: sg.i32(a, "numComponents", 4).clamp(1, 4) as usize,
                    values: sg.f32s(a, "value"),
                }),
                "Matrix3VertexAttribute" => Some(Attrib::Matrix3 {
                    name: string(a, "name"),
                    values: sg.f32s(a, "value"),
                }),
                "Matrix4VertexAttribute" => Some(Attrib::Matrix4 {
                    name: string(a, "name"),
                    values: sg.f32s(a, "value"),
                }),
                _ => None,
            })
            .collect();
        ExtrasOwned {
            tex,
            tangent,
            fog,
            attribs,
        }
    }

    fn coord(&self, sg: &Sg, n: NodeId) -> Option<Vec<f32>> {
        sg.coord_points(sg.node(n, "coord")?)
    }

    fn build_geometry(&mut self, sg: &Sg, n: NodeId) -> Option<Geometry> {
        let ty = sg.type_name(n)?;

        let g = match ty {
            "IndexedFaceSet" => {
                let ex = self.extras(sg, n);
                let coord = self.coord(sg, n);
                let colors = self.colors(sg, n);
                let tex = sg
                    .node(n, "texCoord")
                    .filter(|t| sg.type_name(*t) == Some("TextureCoordinate"))
                    .map(|t| sg.f32s(t, "point").to_vec());
                let normal = sg
                    .node(n, "normal")
                    .filter(|t| sg.type_name(*t) == Some("Normal"))
                    .map(|t| sg.f32s(t, "vector").to_vec());
                let input = IfsInput {
                    coord: coord.as_deref(),
                    coord_index: sg.i32s(n, "coordIndex"),
                    tex_coord: tex.as_deref(),
                    tex_coord_index: sg.i32s(n, "texCoordIndex"),
                    normal: normal.as_deref(),
                    normal_index: sg.i32s(n, "normalIndex"),
                    color: colors.as_ref().map(|(v, c)| Colors {
                        values: v,
                        components: *c,
                    }),
                    color_index: sg.i32s(n, "colorIndex"),
                    ccw: sg.bool(n, "ccw", true),
                    solid: sg.bool(n, "solid", true),
                    convex: sg.bool(n, "convex", true),
                    crease_angle: sg.f64(n, "creaseAngle", 0.0),
                    color_per_vertex: sg.bool(n, "colorPerVertex", true),
                    normal_per_vertex: sg.bool(n, "normalPerVertex", true),
                    extras: ex.view(),
                };
                geometry::indexed_face_set(&input)
            }
            "IndexedLineSet" => {
                let coord = self.coord(sg, n);
                let colors = self.colors(sg, n);
                let ex = self.extras(sg, n);
                geometry::indexed_line_set(
                    coord.as_deref(),
                    sg.i32s(n, "coordIndex"),
                    colors.as_ref().map(|(v, c)| Colors {
                        values: v,
                        components: *c,
                    }),
                    sg.i32s(n, "colorIndex"),
                    sg.bool(n, "colorPerVertex", true),
                    sg.node(n, "normal")
                        .filter(|t| sg.type_name(*t) == Some("Normal"))
                        .map(|t| sg.f32s(t, "vector")),
                    &ex.view(),
                )
            }
            "PointSet" => {
                let coord = self.coord(sg, n);
                let colors = self.colors(sg, n);
                let normal = sg
                    .node(n, "normal")
                    .filter(|t| sg.type_name(*t) == Some("Normal"))
                    .map(|t| sg.f32s(t, "vector"));
                geometry::point_set(
                    coord.as_deref(),
                    colors.as_ref().map(|(v, c)| Colors {
                        values: v,
                        components: *c,
                    }),
                    normal,
                )
            }
            "TriangleSet"
            | "TriangleFanSet"
            | "TriangleStripSet"
            | "QuadSet"
            | "IndexedTriangleSet"
            | "IndexedTriangleFanSet"
            | "IndexedTriangleStripSet"
            | "IndexedQuadSet" => {
                use crate::composed::{ComposedInput, Kind, composed_geometry};
                let ex = self.extras(sg, n);
                let coord = self.coord(sg, n);
                let colors = self.colors(sg, n);
                let tex = sg
                    .node(n, "texCoord")
                    .filter(|t| sg.type_name(*t) == Some("TextureCoordinate"))
                    .map(|t| sg.f32s(t, "point").to_vec());
                let normal = sg
                    .node(n, "normal")
                    .filter(|t| sg.type_name(*t) == Some("Normal"))
                    .map(|t| sg.f32s(t, "vector").to_vec());
                let kind = match ty {
                    "TriangleSet" => Kind::TriangleSet,
                    "QuadSet" => Kind::QuadSet,
                    "TriangleFanSet" => Kind::TriangleFanSet {
                        fan_count: sg.i32s(n, "fanCount"),
                    },
                    "TriangleStripSet" => Kind::TriangleStripSet {
                        strip_count: sg.i32s(n, "stripCount"),
                    },
                    "IndexedTriangleSet" => Kind::IndexedTriangleSet {
                        index: sg.i32s(n, "index"),
                    },
                    "IndexedTriangleFanSet" => Kind::IndexedTriangleFanSet {
                        index: sg.i32s(n, "index"),
                    },
                    "IndexedTriangleStripSet" => Kind::IndexedTriangleStripSet {
                        index: sg.i32s(n, "index"),
                    },
                    _ => Kind::IndexedQuadSet {
                        index: sg.i32s(n, "index"),
                    },
                };
                composed_geometry(&ComposedInput {
                    kind,
                    coord: coord.as_deref(),
                    color: colors.as_ref().map(|(v, c)| Colors {
                        values: v,
                        components: *c,
                    }),
                    tex_coord: tex.as_deref(),
                    normal: normal.as_deref(),
                    solid: sg.bool(n, "solid", true),
                    ccw: sg.bool(n, "ccw", true),
                    color_per_vertex: sg.bool(n, "colorPerVertex", true),
                    normal_per_vertex: sg.bool(n, "normalPerVertex", true),
                    extras: ex.view(),
                })
            }
            "GeoElevationGrid" => {
                let ex = self.extras(sg, n);
                let colors = self.colors(sg, n);
                let tex = sg
                    .node(n, "texCoord")
                    .filter(|t| sg.type_name(*t) == Some("TextureCoordinate"))
                    .map(|t| sg.f32s(t, "point").to_vec());
                let normal = sg
                    .node(n, "normal")
                    .filter(|t| sg.type_name(*t) == Some("Normal"))
                    .map(|t| sg.f32s(t, "vector").to_vec());
                let (xd, zd) = (sg.i32(n, "xDimension", 0), sg.i32(n, "zDimension", 0));
                let geo_points = (xd >= 2 && zd >= 2).then(|| {
                    // GeoElevationGrid.createPoints.
                    let geo = sg.geo_object(n);
                    let height = sg.f64s(n, "height");
                    let y_scale = sg.f64(n, "yScale", 1.0);
                    let (xs, zs) = (sg.f64(n, "xSpacing", 1.0), sg.f64(n, "zSpacing", 1.0));
                    let origin = sg.vec3(n, "geoGridOrigin", Vector3::ZERO);
                    let mut points = vec![];
                    for z in 0..zd as usize {
                        for x in 0..xd as usize {
                            let h =
                                height.get(x + z * xd as usize).copied().unwrap_or(0.0) * y_scale;
                            let mut p = if geo.standard_order {
                                Vector3::new(zs * z as f64, xs * x as f64, h)
                            } else {
                                Vector3::new(xs * x as f64, zs * z as f64, h)
                            };
                            p.add(&origin);
                            points.push(geo.get_coord(&p));
                        }
                    }
                    points
                });
                geometry::elevation_grid(&ElevationInput {
                    x_dimension: xd,
                    z_dimension: zd,
                    x_spacing: 1.0,
                    z_spacing: 1.0,
                    height: &[],
                    color: colors.as_ref().map(|(v, c)| Colors {
                        values: v,
                        components: *c,
                    }),
                    tex_coord: tex.as_deref(),
                    normal: normal.as_deref(),
                    ccw: sg.bool(n, "ccw", true),
                    solid: sg.bool(n, "solid", true),
                    crease_angle: sg.f64(n, "creaseAngle", 0.0),
                    color_per_vertex: sg.bool(n, "colorPerVertex", true),
                    normal_per_vertex: sg.bool(n, "normalPerVertex", true),
                    extras: ex.view(),
                    geo_points,
                })
            }
            "Arc2D" => crate::geometry2d::arc2d(
                sg.f64(n, "radius", 1.0),
                sg.f64(n, "startAngle", 0.0),
                sg.f64(n, "endAngle", std::f64::consts::FRAC_PI_2),
            ),
            "ArcClose2D" => crate::geometry2d::arc_close2d(
                sg.f64(n, "radius", 1.0),
                sg.f64(n, "startAngle", 0.0),
                sg.f64(n, "endAngle", std::f64::consts::FRAC_PI_2),
                matches!(sg.value(n, "closureType"), Some(x3d_fields::FieldValue::SFString(c)) if c == "CHORD"),
                sg.bool(n, "solid", false),
            ),
            "Circle2D" => crate::geometry2d::circle2d(sg.f64(n, "radius", 1.0)),
            "Disk2D" => crate::geometry2d::disk2d(
                sg.f64(n, "innerRadius", 0.0),
                sg.f64(n, "outerRadius", 1.0),
                sg.bool(n, "solid", false),
            ),
            "Polyline2D" => crate::geometry2d::polyline2d(sg.f32s(n, "lineSegments")),
            "Polypoint2D" => crate::geometry2d::polypoint2d(sg.f32s(n, "point")),
            "Rectangle2D" => {
                let size = sg.vec2(n, "size", Vector2::new(2.0, 2.0));
                crate::geometry2d::rectangle2d((size.x, size.y), sg.bool(n, "solid", false))
            }
            "TriangleSet2D" => crate::geometry2d::triangle_set2d(
                sg.f32s(n, "vertices"),
                sg.bool(n, "solid", false),
            ),
            "LineSet" => {
                let coord = self.coord(sg, n);
                let colors = self.colors(sg, n);
                let normal = sg
                    .node(n, "normal")
                    .filter(|t| sg.type_name(*t) == Some("Normal"))
                    .map(|t| sg.f32s(t, "vector").to_vec());
                let ex = self.extras(sg, n);
                crate::geometry2d::line_set(
                    sg.i32s(n, "vertexCount"),
                    coord.as_deref(),
                    colors.as_ref().map(|(v, c)| Colors {
                        values: v,
                        components: *c,
                    }),
                    normal.as_deref(),
                    &ex.view(),
                )
            }
            "Box" => geometry::make_box(
                sg.vec3(n, "size", Vector3::new(2.0, 2.0, 2.0)),
                sg.bool(n, "solid", true),
            ),
            "Sphere" => geometry::make_sphere(sg.f64(n, "radius", 1.0), sg.bool(n, "solid", true)),
            "Cone" => geometry::make_cone(
                sg.f64(n, "height", 2.0),
                sg.f64(n, "bottomRadius", 1.0),
                sg.bool(n, "side", true),
                sg.bool(n, "bottom", true),
                sg.bool(n, "solid", true),
            ),
            "Cylinder" => geometry::make_cylinder(
                sg.f64(n, "height", 2.0),
                sg.f64(n, "radius", 1.0),
                sg.bool(n, "side", true),
                sg.bool(n, "top", true),
                sg.bool(n, "bottom", true),
                sg.bool(n, "solid", true),
            ),
            "ElevationGrid" => {
                let ex = self.extras(sg, n);
                let colors = self.colors(sg, n);
                let tex = sg
                    .node(n, "texCoord")
                    .filter(|t| sg.type_name(*t) == Some("TextureCoordinate"))
                    .map(|t| sg.f32s(t, "point").to_vec());
                let normal = sg
                    .node(n, "normal")
                    .filter(|t| sg.type_name(*t) == Some("Normal"))
                    .map(|t| sg.f32s(t, "vector").to_vec());
                geometry::elevation_grid(&ElevationInput {
                    x_dimension: sg.i32(n, "xDimension", 0),
                    z_dimension: sg.i32(n, "zDimension", 0),
                    x_spacing: sg.f64(n, "xSpacing", 1.0),
                    z_spacing: sg.f64(n, "zSpacing", 1.0),
                    height: sg.f32s(n, "height"),
                    color: colors.as_ref().map(|(v, c)| Colors {
                        values: v,
                        components: *c,
                    }),
                    tex_coord: tex.as_deref(),
                    normal: normal.as_deref(),
                    ccw: sg.bool(n, "ccw", true),
                    solid: sg.bool(n, "solid", true),
                    crease_angle: sg.f64(n, "creaseAngle", 0.0),
                    color_per_vertex: sg.bool(n, "colorPerVertex", true),
                    normal_per_vertex: sg.bool(n, "normalPerVertex", true),
                    extras: ex.view(),
                    geo_points: None,
                })
            }
            "Text" => return text::text_geometry(sg, n, &mut self.unsupported),
            "NurbsPatchSurface" => {
                crate::nurbs::surface(sg, n, sg.rt.nurbs_tessellation_scale(n), false)
            }
            "NurbsTrimmedSurface" => {
                crate::nurbs::surface(sg, n, sg.rt.nurbs_tessellation_scale(n), true)
            }
            "NurbsCurve" => crate::nurbs::curve(sg, n),
            "NurbsSweptSurface" => crate::nurbs::swept(sg, n, false),
            "NurbsSwungSurface" => crate::nurbs::swept(sg, n, true),
            "Extrusion" => {
                let orientation = sg.f64s(n, "orientation");
                geometry::extrusion(&ExtrusionInput {
                    cross_section: sg.f32s(n, "crossSection"),
                    orientation: &orientation,
                    scale: sg.f32s(n, "scale"),
                    spine: sg.f32s(n, "spine"),
                    begin_cap: sg.bool(n, "beginCap", true),
                    end_cap: sg.bool(n, "endCap", true),
                    solid: sg.bool(n, "solid", true),
                    ccw: sg.bool(n, "ccw", true),
                    convex: sg.bool(n, "convex", true),
                    crease_angle: sg.f64(n, "creaseAngle", 0.0),
                })
            }
            other => {
                self.unsupported.insert(other.to_string());
                return None;
            }
        };

        Some(g)
    }

    /// HAnimHumanoid.traverse (DISPLAY): its private Transform's groups (skeleton, viewpoints, the
    /// Skin group that pushes the humanoid as hAnimNode) with the inverse humanoid model-view on
    /// renderObject.getInvHumanoidMatrix (), then skinning: the joint matrices when the update
    /// Lock is open (the runtime's unlock count changed) and the humanoid has a skinCoord.
    fn hanim_humanoid(&mut self, sg: &Sg, n: NodeId, mv: &Matrix4, depth: usize) {
        let mut m = *mv;
        m.mult_left(&sg.group_matrix(n, "HAnimHumanoid").unwrap());
        let mut inv = m;
        inv.inverse();

        let Some([skeleton, viewpoints, skin]) = sg.hanim_children(n, "HAnimHumanoid") else {
            return;
        };
        let (joints, bindings) = crate::hanim::humanoid_joints(sg, n);
        let tables = Rc::new(crate::hanim::humanoid_tables(sg, n, &joints));
        let source = self.skins.len();

        self.skins.push(SkinSource {
            node: n,
            tables,
            matrices: Rc::new(vec![]),
        });
        self.inv_humanoid.push(inv);
        self.groups.push(ShadowGroup::Node(n));
        self.group_children(sg, skeleton, &m, depth);
        self.group_children(sg, viewpoints, &m, depth);
        self.hanim.push(Some(source));
        self.group_children(sg, skin, &m, depth);
        self.hanim.pop();
        self.groups.pop();
        self.inv_humanoid.pop();

        // skinning: Function.prototype without a Coordinate skinCoord; the matrices stay zero.
        let skin_coord = sg
            .node(n, "skinCoord")
            .filter(|c| matches!(sg.type_name(*c), Some("Coordinate" | "CoordinateDouble")));
        let unlocks = sg.rt.hanim_unlocks(n);
        let key = (sg.rt.uid, n);
        let cached = self
            .cache
            .as_deref()
            .and_then(|c| c.joint_matrices.get(&key).cloned());

        let matrices = match (skin_coord, cached) {
            (None, _) => Rc::new(vec![0.0; joints.len() * 32]),
            (Some(_), Some((count, m))) if count == unlocks && m.len() == joints.len() * 32 => m,
            (Some(_), _) => {
                let model_views: Vec<Matrix4> = joints
                    .iter()
                    .map(|j| {
                        self.joint_model_views
                            .get(j)
                            .copied()
                            .or_else(|| {
                                self.cache.as_deref().and_then(|c| {
                                    c.joint_model_views.get(&(sg.rt.uid, *j)).copied()
                                })
                            })
                            .unwrap_or(Matrix4::IDENTITY)
                    })
                    .collect();
                let m = Rc::new(crate::hanim::joint_matrices(&bindings, &model_views, &inv));

                if let Some(c) = self.cache.as_deref_mut() {
                    c.joint_matrices.insert(key, (unlocks, m.clone()));
                }

                m
            }
        };

        self.skins[source].matrices = matrices;
        self.apply_skins(source);
    }

    /// HAnimSegment.traverse with a coord and displacements: the segment is the hAnimNode of its
    /// children; skinning sets joint 0 to its model-view × the inverse humanoid model-view.
    fn hanim_segment(&mut self, sg: &Sg, n: NodeId, mv: &Matrix4, depth: usize) {
        let Some(tables) = crate::hanim::segment_tables(sg, n) else {
            return;
        };
        let matrices = Rc::new(crate::hanim::segment_matrices(
            mv,
            self.inv_humanoid.last().unwrap(),
        ));
        let source = self.skins.len();

        self.skins.push(SkinSource {
            node: n,
            tables: Rc::new(tables),
            matrices,
        });
        self.hanim.push(Some(source));
        self.groups.push(ShadowGroup::Node(n));
        self.group_children(sg, children_of(sg, n, "HAnimSegment"), mv, depth);
        self.groups.pop();
        self.hanim.pop();
        self.apply_skins(source);
    }

    /// Skin2.glsl for the draw items of `source`.
    fn apply_skins(&mut self, source: usize) {
        let pending: Vec<(usize, NodeId)> = self
            .skin_items
            .iter()
            .filter(|(_, _, s)| *s == source)
            .map(|(i, g, _)| (*i, *g))
            .collect();
        self.skin_items.retain(|(_, _, s)| *s != source);

        let s = &self.skins[source];
        let (node, tables, matrices) = (s.node, s.tables.clone(), s.matrices.clone());

        for (i, gnode) in pending {
            let Some(item) = self.items.get(i).filter(|item| item.geometry_node == gnode) else {
                continue;
            };
            let src = item.geometry.clone();
            let key = (self.root.uid, node, Rc::as_ptr(&src) as usize);

            let value = match self.cache.as_deref_mut() {
                Some(c) => {
                    let frame = c.frame;

                    match c.skinned.get_mut(&key) {
                        Some(e)
                            if Rc::ptr_eq(&e.source, &src)
                                && e.tables == tables
                                && e.matrices == matrices =>
                        {
                            e.used = frame;
                            e.value.clone()
                        }
                        _ => {
                            let value = Rc::new(crate::hanim::skin(&src, &tables, &matrices));
                            c.skinned.insert(
                                key,
                                SkinnedEntry {
                                    source: src,
                                    tables: tables.clone(),
                                    matrices: matrices.clone(),
                                    value: value.clone(),
                                    used: frame,
                                },
                            );
                            value
                        }
                    }
                }
                None => Rc::new(crate::hanim::skin(&src, &tables, &matrices)),
            };

            self.items[i].geometry = value;
        }
    }

    /// The transformed LINE triangles of the ParticleSystem `n` as the shadow map draw of the layer being built sees
    /// them: those of the last color pass before this layer's
    /// ([`x3d_runtime::particles::Sim::thick_lines_before_layer`]), drawn under their instance matrices in the system's
    /// space; `None` while their line count (one segment per particle) is not the current one.
    fn particle_thick_lines_before_layer(
        &self,
        sg: &Sg,
        n: NodeId,
    ) -> Option<Rc<x3d_runtime::particles::ThickLines>> {
        if !self.particle_system_enabled(sg, n) {
            return None;
        }

        let sim = sg.rt.particle_sim(n)?;
        let lines = sim
            .thick_lines_before_layer
            .iter()
            .find(|(key, _)| *key == self.layer)
            .and_then(|(_, lines)| lines.as_ref())
            .filter(|t| t.num_lines == sim.num_particles && !t.instances.is_empty())?;

        Some(lines.clone())
    }

    /// X3DShapeNode.isEnabled of a ParticleSystem: `numParticles`, and a geometry for GEOMETRY.
    fn particle_system_enabled(&self, sg: &Sg, n: NodeId) -> bool {
        let num = sg.rt.particle_sim(n).map_or(0, |s| s.num_particles);

        num > 0
            && (sg.rt.particle_geometry_type(n) != GeometryType::Geometry
                || sg.node(n, "geometry").is_some())
    }

    /// ParticleSystem.set_bbox__ with the default bboxSize: the union of the BoundedPhysicsModels' geometry boxes,
    /// or else the emitter's getBBox (X_ITE's default emitter is a PointEmitter): a cube of
    /// `particleLifetime * (1 + lifetimeVariation) * speed * (1 + variation) * 2` around the emitter's position
    /// (Point, Cone, Explosion) or around the center of its geometry's box, joined with that box (Surface, Volume,
    /// Polyline; a SurfaceEmitter without a surface has an empty box).
    fn particle_bbox(&mut self, sg: &Sg, n: NodeId) -> Box3 {
        let bounded: Vec<NodeId> = sg
            .nodes(n, "physics")
            .into_iter()
            .filter(|m| sg.type_name(*m) == Some("BoundedPhysicsModel"))
            .collect();

        if !bounded.is_empty() {
            let mut bbox = Box3::EMPTY;

            for m in bounded {
                if let Some(g) = sg.node(m, "geometry").and_then(|g| self.geometry(sg, g)) {
                    bbox.add(&Box3::from_extents(&g.min, &g.max));
                }
            }

            return bbox;
        }

        const EMITTERS: [&str; 6] = [
            "PointEmitter",
            "ConeEmitter",
            "ExplosionEmitter",
            "SurfaceEmitter",
            "VolumeEmitter",
            "PolylineEmitter",
        ];

        let emitter = sg
            .node(n, "emitter")
            .filter(|e| sg.type_name(*e).is_some_and(|t| EMITTERS.contains(&t)));
        let lifetime = sg.f64(n, "particleLifetime", 5.0).max(0.0);
        let variation = sg.f64(n, "lifetimeVariation", 0.25).max(0.0);
        let (speed, speed_variation) = emitter.map_or((0.0, 0.25), |e| {
            (sg.f64(e, "speed", 0.0), sg.f64(e, "variation", 0.25))
        });
        let size = lifetime * (1.0 + variation) * speed * (1.0 + speed_variation) * 2.0;
        let cube = |center: &Vector3| Box3::new(&Vector3::new(size, size, size), center);

        let geometry_box = |builder: &mut Self, e: NodeId| -> Option<Box3> {
            match sg.type_name(e)? {
                "SurfaceEmitter" => sg.node(e, "surface").map(|s| {
                    builder
                        .geometry(sg, s)
                        .map_or(Box3::EMPTY, |g| Box3::from_extents(&g.min, &g.max))
                }),
                "VolumeEmitter" => {
                    let request = x3d_runtime::nodes::particle_systems::GeometryRequest::Volume {
                        coord: sg.node(e, "coord"),
                        coord_index: sg.i32s(e, "coordIndex").to_vec(),
                    };
                    let soup = particle_geometry(sg.rt, &request)?;

                    Some(soup_box(&soup.vertices))
                }
                "PolylineEmitter" => {
                    // The private IndexedLineSet's box: its polylines' vertices.
                    let points = sg
                        .node(e, "coord")
                        .and_then(|c| sg.coord_points(c))
                        .unwrap_or_default();
                    let index = sg.i32s(e, "coordIndex");
                    let mut vertices = vec![];

                    for line in index.split(|i| *i < 0) {
                        if line.len() >= 2 {
                            for i in line {
                                if let Some(p) = points.get(*i as usize * 3..*i as usize * 3 + 3) {
                                    vertices.extend_from_slice(&[p[0], p[1], p[2], 1.0]);
                                }
                            }
                        }
                    }

                    Some(soup_box(&vertices))
                }
                _ => None,
            }
        };

        match emitter {
            Some(e)
                if matches!(
                    sg.type_name(e),
                    Some("SurfaceEmitter" | "VolumeEmitter" | "PolylineEmitter")
                ) =>
            {
                match geometry_box(self, e) {
                    Some(g) if !g.is_empty() => {
                        let mut bbox = cube(&g.center());
                        bbox.add(&g);
                        bbox
                    }
                    Some(g) => {
                        let mut bbox = cube(&Vector3::ZERO);
                        bbox.add(&g);
                        bbox
                    }
                    None if sg.type_name(e) == Some("SurfaceEmitter") => Box3::EMPTY,
                    None => cube(&Vector3::ZERO),
                }
            }
            Some(e) => cube(&sg.vec3(e, "position", Vector3::ZERO)),
            None => cube(&Vector3::ZERO),
        }
    }

    /// The ParticleSystem's bbox as set_bbox__ leaves it: bboxSize / bboxCenter, or [`Self::particle_bbox`].
    fn particle_system_bbox(&mut self, sg: &Sg, n: NodeId) -> Box3 {
        let size = sg.vec3(n, "bboxSize", Vector3::new(-1.0, -1.0, -1.0));

        if size.equals(&Vector3::new(-1.0, -1.0, -1.0)) {
            self.particle_bbox(sg, n)
        } else {
            Box3::new(&size, &sg.vec3(n, "bboxCenter", Vector3::ZERO))
        }
    }

    /// renderObject.addDisplayShape: `viewVolume .intersectsSphere (radius, bboxCenter)` with the shape's bbox in view
    /// space (`radius = bboxSize .norm () / 2`). The view volume's planes are those of the layer's projection.
    fn bbox_in_view(&self, mv: &Matrix4, bbox: &Box3) -> bool {
        let (min, max) = bbox.extents();
        let mut size = max;
        size.subtract(&min);
        let mut center = bbox.center();

        mv.mult_dir_matrix(&mut size);
        mv.mult_vec_matrix3(&mut center);

        view_volume_intersects_sphere(&self.projection, size.norm() / 2.0, &center)
    }

    /// The geometry, the instances and the color-ramp flag of a ParticleSystem's draw with `mv` (SPRITE turns its quad
    /// to face the viewer of `mv`); `None` when it draws nothing.
    fn particle_draw(
        &mut self,
        sg: &Sg,
        n: NodeId,
        mv: &Matrix4,
    ) -> Option<(Rc<Geometry>, Rc<InstanceData>, bool)> {
        if !self.particle_system_enabled(sg, n) {
            return None;
        }

        let sim = sg.rt.particle_sim(n)?;
        let num = sim.num_particles.min(sim.particles.len());
        let gt = sg.rt.particle_geometry_type(n);
        let ramps = sg.rt.particle_ramps(n).cloned().unwrap_or_default();
        // set_color__: geometryContext .colorMaterial = numKeys && colorRampNode (GEOMETRY draws its own geometry context).
        let color_material = gt != GeometryType::Geometry
            && ramps.colors.is_some()
            && sg
                .node(n, "color")
                .is_some_and(|c| matches!(sg.type_name(c), Some("Color" | "ColorRGBA")));
        let particles =
            crate::particles::instances(&sim.particles[..num], &ramps, gt, color_material);
        let size = sg.vec2(n, "particleSize", Vector2::new(0.02, 0.02));

        let geometry = match gt {
            GeometryType::Geometry => self.geometry(sg, sg.node(n, "geometry")?)?,
            _ => {
                let rows = (gt == GeometryType::Sprite)
                    .then(|| crate::particles::screen_aligned_rotation(mv));
                let mut bits = vec![gt as u64, color_material as u64];

                match (gt, &rows) {
                    (GeometryType::Line, _) => bits.push(size.y.to_bits()),
                    (GeometryType::Sprite, Some(r)) => bits.extend(
                        [size.x, size.y]
                            .into_iter()
                            .chain(r.iter().flat_map(|v| [v.x, v.y, v.z]))
                            .map(f64::to_bits),
                    ),
                    _ => (),
                }

                let ckey = (sg.rt.uid, n);

                match self.cache.as_deref_mut().and_then(|c| {
                    let frame = c.frame;
                    c.particles.get_mut(&ckey).filter(|e| e.0 == bits).map(|e| {
                        e.2 = frame;
                        e.1.clone()
                    })
                }) {
                    Some(g) => g,
                    None => {
                        let g = Rc::new(match gt {
                            GeometryType::Point => crate::particles::point_geometry(color_material),
                            GeometryType::Line => {
                                crate::particles::line_geometry(size.y, color_material)
                            }
                            GeometryType::Sprite => crate::particles::quad_geometry(
                                rows.as_ref().map(|r| (r, [size.x, size.y])),
                                color_material,
                            ),
                            _ => crate::particles::quad_geometry(None, color_material),
                        });

                        if let Some(c) = self.cache.as_deref_mut() {
                            let frame = c.frame;
                            c.particles.insert(ckey, (bits, g.clone(), frame));
                        }

                        g
                    }
                }
            }
        };

        if particles.matrices.is_empty() {
            return None;
        }

        let identity = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        let instances = InstanceData {
            normals: vec![identity; particles.matrices.len()],
            matrices: particles.matrices,
            colors: particles.colors,
            tex_coords: particles.tex_coords,
            particle: true,
        };

        Some((geometry, Rc::new(instances), color_material))
    }

    /// ParticleSystem.traverse DISPLAY: X3DShapeNode.traverse → addDisplayShape (view volume test with the
    /// ParticleSystem's bbox; the bbox center is the sort distance) → ParticleSystem.display, an instanced draw of
    /// the particles with the appearance.
    fn particle_system(&mut self, sg: &Sg, n: NodeId, mv: &Matrix4) {
        let bbox = self.particle_system_bbox(sg, n);

        if !self.particle_system_enabled(sg, n) || !self.bbox_in_view(mv, &bbox) {
            return;
        }

        let Some((geometry, instances, _)) = self.particle_draw(sg, n, mv) else {
            return;
        };
        let mut center = bbox.center();

        mv.mult_vec_matrix3(&mut center);

        // GEOMETRY draws the geometry node's context, the other types the ParticleSystem's own (no texture
        // coordinate mapping). set_transparent__: the appearance, a transparent color ramp (ColorRGBA), or a
        // transparent geometry of GEOMETRY.
        let ramp_transparent = sg
            .node(n, "color")
            .is_some_and(|c| sg.type_name(c) == Some("ColorRGBA"));

        match sg.rt.particle_geometry_type(n) {
            GeometryType::Geometry => {
                let Some(gnode) = sg.node(n, "geometry") else {
                    return;
                };
                let coords = coordinate_mappings(sg, gnode);

                self.shape_item(
                    sg,
                    n,
                    gnode,
                    geometry,
                    mv,
                    Some(instances),
                    center.z,
                    coords,
                    ramp_transparent,
                );
            }
            _ => self.shape_item(
                sg,
                n,
                n,
                geometry,
                mv,
                Some(instances),
                center.z,
                vec![],
                false,
            ),
        }
    }

    fn shape(&mut self, sg: &Sg, n: NodeId, mv: &Matrix4) {
        let Some(gnode) = sg.node(n, "geometry") else {
            return;
        };
        let Some(geometry) = self.geometry(sg, gnode) else {
            return;
        };

        // ScreenText.traverseBefore: the model-view matrix with getScreenScaleMatrix (renderObject, matrix, 1, true) around the shape.
        let screen_text_mv = geometry.texture.as_ref().map(|_| {
            let mut m = *mv;
            let s = self.screen_scale_matrix(mv, 1.0, true);

            m.mult_left(&s);
            m
        });
        let mv = screen_text_mv.as_ref().unwrap_or(mv);

        // InstancedShape: isEnabled () needs instances (numInstances) and a geometry.
        let instances =
            (sg.type_name(n) == Some("InstancedShape")).then(|| Rc::new(instance_data(sg, n)));

        if instances.as_ref().is_some_and(|i| i.matrices.is_empty()) {
            return;
        }

        // addDisplayShape: distance = bbox center z in view space (an InstancedShape's bbox is the
        // union of the instances' boxes, or its bboxSize / bboxCenter).
        let mut center = geometry.bbox_center();

        if let Some(i) = &instances {
            center = instanced_bbox(sg, n, &geometry, i).center();
        } else if let Some(bbox) = declared_bbox(sg, n) {
            // X3DShapeNode.set_bbox__: a non-default bboxSize is the shape's bbox, so addDisplayShape's view volume
            // test uses the declared box (a box out of view culls the shape) and its center is the sort distance.
            // The default box is the geometry's, which the view volume test cannot cull visibly.
            if !self.bbox_in_view(mv, &bbox) {
                return;
            }

            center = bbox.center();
        }

        mv.mult_vec_matrix3(&mut center);

        if let Some(Some(s)) = self.hanim.last() {
            self.skin_items.push((self.items.len(), gnode, *s));
        }

        let coords = coordinate_mappings(sg, gnode);

        self.shape_item(
            sg, n, gnode, geometry, mv, instances, center.z, coords, false,
        );
    }

    /// The display item of an X3DShapeNode: its appearance with `geometry` drawn by `gnode` (the geometry node,
    /// or a ParticleSystem for its own geometry), `distance` the sort key, `coords` the geometry's texture
    /// coordinate mappings; `extra_transparent` adds to the appearance's transparency (a ParticleSystem's
    /// ColorRGBA ramp).
    #[allow(clippy::too_many_arguments)]
    fn shape_item(
        &mut self,
        sg: &Sg,
        n: NodeId,
        gnode: NodeId,
        geometry: Rc<Geometry>,
        mv: &Matrix4,
        instances: Option<Rc<InstanceData>>,
        distance: f64,
        coords: Vec<String>,
        extra_transparent: bool,
    ) {
        let appearance = sg
            .node(n, "appearance")
            .filter(|a| sg.type_name(*a) == Some("Appearance"));
        let material_node = appearance.and_then(|a| sg.node(a, "material"));
        let texture_node = appearance.and_then(|a| sg.node(a, "texture"));
        let transform_node = appearance.and_then(|a| sg.node(a, "textureTransform"));

        let has_normals = !geometry.normals.is_empty();

        // GeneratedCubeMapTexture: hidden in its own texture pass; collected by an independent pass.
        let generated = generated_textures(sg, texture_node);

        if self.hidden_texture.is_some_and(|h| generated.contains(&h)) {
            return;
        }

        if !self.dependent {
            let mut model = *mv;
            model.mult_right(&self.camera_space);

            for g in generated {
                if sg.string(g, "update", "NONE") == "NONE" || sg.i32(g, "size", 128) <= 0 {
                    continue;
                }

                match self.generated.iter_mut().find(|(n, _)| *n == g) {
                    Some(e) => e.1 = model,
                    None => self.generated.push((g, model)),
                }
            }

            // Appearance.traverse → RenderedTexture.traverse: the first independent pass to
            // reach a texture that renders this frame draws it.
            for r in rendered_textures(sg, appearance) {
                if sg.rt.rendered_now.contains(&r)
                    && !self.rendered.contains(&r)
                    && !self.rendered_done.contains(&r)
                {
                    self.rendered.push(r);
                }
            }
        }

        // Appearance.set_material__: X3DCast (X3DMaterialNode, material) ?? the browser default
        // material (UnlitMaterial, emissiveColor 1 1 1).
        let front = material_node.and_then(|m| {
            let params = material_params(sg, m, has_normals, true);
            if params.is_none() {
                self.unsupported
                    .insert(sg.type_name(m).unwrap_or("?").to_string());
            }
            params.map(|p| (m, p))
        });

        // Appearance.set_backMaterial__: X3DCast (X3DOneSidedMaterialNode, backMaterial), then the
        // deprecated TwoSidedMaterial handling `??= X3DCast (TwoSidedMaterial, materialNode)`.
        let back = appearance
            .and_then(|a| sg.node(a, "backMaterial"))
            .filter(|m| {
                matches!(
                    sg.type_name(*m),
                    Some(
                        "Material"
                            | "UnlitMaterial"
                            | "PhysicalMaterial"
                            | "SpecularGlossinessMaterial"
                    )
                )
            })
            .and_then(|m| material_params(sg, m, has_normals, true).map(|p| (m, p)))
            .or_else(|| {
                front
                    .as_ref()
                    .filter(|(m, _)| sg.type_name(*m) == Some("TwoSidedMaterial"))
                    .and_then(|(m, _)| material_params(sg, *m, has_normals, false).map(|p| (*m, p)))
            });

        // X3DMaterialExtensionNodes this renderer does not draw (VolumeScatter).
        for m in front.iter().map(|f| f.0).chain(back.iter().map(|b| b.0)) {
            for e in extension_nodes(sg, m) {
                let name = sg.type_name(e).unwrap_or("?");
                if !MATERIAL_EXTENSIONS.contains(&name) {
                    self.unsupported.insert(name.to_string());
                }
            }
        }

        let (material, front_transparent, emissive_texture) = match &front {
            Some((m, p)) => (
                p.clone(),
                material_transparent(sg, *m, p),
                sg.node(*m, "emissiveTexture")
                    .filter(|_| sg.type_name(*m) == Some("UnlitMaterial")),
            ),
            None => (default_material(), false, None),
        };

        if let Some(b) = appearance.and_then(|a| sg.node(a, "backMaterial")) {
            if !matches!(
                sg.type_name(b),
                Some(
                    "Material"
                        | "UnlitMaterial"
                        | "PhysicalMaterial"
                        | "SpecularGlossinessMaterial"
                )
            ) {
                self.unsupported
                    .insert(sg.type_name(b).unwrap_or("?").to_string());
            }
        }

        // X3D_MATERIAL_TEXTURES of the physical materials, with the texture transform and
        // coordinate channels their mappings name.
        let transforms = transform_mappings(sg, transform_node);
        let mut material = material;
        let mut front_transparent = front_transparent;

        if let Some((m, _)) = &front {
            material.textures = self.material_textures(sg, *m, &transforms, &coords);
            // X3DOneSidedMaterialNode.set_transparent__: transparency || baseTexture .isTransparent ().
            front_transparent |= material
                .textures
                .iter()
                .any(|t| t.kind == 0 && t.texture.transparent);
        }

        let back = back.map(|(m, mut p)| {
            p.textures = self.material_textures(sg, m, &transforms, &coords);
            (m, p)
        });
        let back_transparent = back.as_ref().is_some_and(|(m, p)| {
            material_transparent(sg, *m, p)
                || p.textures
                    .iter()
                    .any(|t| t.kind == 0 && t.texture.transparent)
        });
        let back_material = back.map(|(_, p)| p);

        // BlendMode, DepthMode: Appearance.set_renderModes__.
        let blend = appearance
            .and_then(|a| sg.node(a, "blendMode"))
            .filter(|b| sg.type_name(*b) == Some("BlendMode"))
            .map(|b| blend_params(sg, b));
        let depth = appearance
            .and_then(|a| sg.node(a, "depthMode"))
            .filter(|d| sg.type_name(*d) == Some("DepthMode"))
            .map(|d| depth_params(sg, d));

        // Appearance.set_texture__: X3DCast (X3DTextureNode, texture). A MultiTexture's units are its
        // X3DSingleTextureNode textures (at most maxTextures); MultiTexture is never transparent.
        let (textures, multi_texture, texture_transparent) = match texture_node {
            Some(t) if sg.type_name(t) == Some("MultiTexture") => {
                let units: Vec<Rc<TextureImage>> = sg
                    .nodes(t, "texture")
                    .into_iter()
                    .filter(|u| is_single_texture(sg, *u))
                    .take(MAX_TEXTURES)
                    .map(|u| self.texture(sg, u).unwrap_or_else(|| self.white_texture()))
                    .collect();
                let params = multi_texture_params(sg, t, units.len());
                (units, Some(params), false)
            }
            Some(t) => {
                let texture = self.texture(sg, t);
                // An X3DTextureNode that failed to load still binds X_ITE's 1×1 white default texture.
                let texture = match texture {
                    None if is_single_texture(sg, t) && can_load(sg, t) => {
                        Some(self.white_texture())
                    }
                    t => t,
                };
                // Appearance.set_transparent__: the appearance texture counts even when an
                // emissive texture replaces it in the shader.
                let transparent = texture.as_ref().is_some_and(|t| t.transparent);
                (texture.into_iter().collect(), None, transparent)
            }
            None => (vec![], None, false),
        };

        // Unlit2.glsl getEmissiveColor: X3D_EMISSIVE_TEXTURE wins over X3D_TEXTURE.
        let (textures, multi_texture) = match emissive_texture {
            Some(t) if can_load(sg, t) => (
                vec![self.texture(sg, t).unwrap_or_else(|| self.white_texture())],
                None,
            ),
            Some(t) => {
                self.unsupported
                    .insert(sg.type_name(t).unwrap_or("?").to_string());
                (textures, multi_texture)
            }
            None => (textures, multi_texture),
        };

        // Text.display: renderContext .textureNode, the ScreenText's canvas, replaces the appearance's texture; the text is
        // transparent (setTransparent (true)).
        let (textures, multi_texture, texture_transparent) = match &geometry.texture {
            Some(t) => (vec![t.clone()], None, true),
            None => (textures, multi_texture, texture_transparent),
        };

        // Appearance.set_textureTransform__: X3DCast (X3DTextureTransformNode) ?? the default
        // TextureTransform; MultiTextureTransform: its X3DSingleTextureTransformNodes (at most
        // maxTextureTransforms), the default one if there is none.
        let texture_matrices: Vec<[f32; 16]> = match transform_node {
            Some(t) if sg.type_name(t) == Some("MultiTextureTransform") => {
                let matrices: Vec<[f32; 16]> = sg
                    .nodes(t, "textureTransform")
                    .into_iter()
                    .filter_map(|u| single_texture_transform_matrix(sg, u))
                    .take(MAX_TEXTURES)
                    .collect();
                if matrices.is_empty() {
                    vec![IDENTITY_F32]
                } else {
                    matrices
                }
            }
            Some(t) => vec![single_texture_transform_matrix(sg, t).unwrap_or(IDENTITY_F32)],
            None => vec![IDENTITY_F32],
        };

        // Appearance.set_pointProperties__ / set_lineProperties__ / set_fillProperties__.
        let style = self.content_scale_lines(
            appearance.and_then(|a| style_params(sg, a, geometry.geometry_type)),
            geometry.geometry_type,
        );
        let style_texture = match style {
            Some(StyleParams::Fill(f)) if f.hatched => {
                Some(crate::style::hatch_texture(f.hatch_style))
            }
            Some(StyleParams::Line(_)) => Some(crate::style::linetype_texture()),
            _ => None,
        };
        // FillProperties.set_filled__: transparent when not filled.
        let fill_transparent = matches!(style, Some(StyleParams::Fill(f)) if !f.filled);

        // X3DGeometryNode.set_color__: setTransparent (colorNode .isTransparent ()), and ColorRGBA
        // is always transparent.
        let color_transparent = sg
            .node(gnode, "color")
            .is_some_and(|c| sg.type_name(c) == Some("ColorRGBA"));

        let auto_transparent = extra_transparent
            || color_transparent
            || fill_transparent
            || front_transparent
            || back_transparent
            || texture_transparent
            || (multi_texture.is_none() && textures.first().is_some_and(|t| t.transparent))
            || blend.is_some();

        // Appearance.set_alphaMode__ / set_alphaCutoff__ and X3DShapeNode.set_transparent__. Without an
        // Appearance the shape uses the default one (AUTO).
        let alpha_mode = AlphaMode::from_name(
            &appearance.map_or_else(String::new, |a| sg.string(a, "alphaMode", "AUTO")),
        );
        let alpha_cutoff = appearance.map_or(0.0, |a| {
            alpha_mode.cutoff(sg.f64(a, "alphaCutoff", 0.5)) as f32
        });
        let (transparent, alpha_mode) = alpha_mode.resolve(auto_transparent);

        let custom = self.custom_draw(sg, appearance);

        self.items.push(DrawItem {
            geometry_node: gnode,
            geometry,
            model_view: *mv,
            material,
            textures,
            multi_texture,
            texture_matrices,
            projectors: self.local_projectors.clone(),
            lights: self.local_lights.clone(),
            transparent,
            alpha_mode,
            alpha_cutoff,
            back_material,
            blend,
            depth,
            clip_planes: self
                .clip_planes
                .iter()
                .take(MAX_CLIP_PLANES)
                .copied()
                .collect(),
            style,
            style_texture,
            distance,
            viewport: (self.viewports.len() > 1).then(|| self.viewport()),
            volume: None,
            instances,
            splats: None,
            custom,
            fog: self.local_fogs.last().cloned(),
        });
    }

    /// X3DVolumeDataNode.traverse: the private Group's Shape (a BLEND alphaMode, so a transparent
    /// item sorted by the center of its planes). See [`crate::volume`].
    fn volume(&mut self, sg: &Sg, n: NodeId, ty: &str, mv: &Matrix4) {
        let d = sg.vec3(n, "dimensions", Vector3::ONE);
        let orientation = crate::volume::sensor_orientation(mv, [d.x, d.y, d.z])
            .unwrap_or_else(Rotation4::identity);
        let mut unsupported = vec![];
        let draw = {
            let mut tex = |t: NodeId| self.texture(sg, t);
            crate::volume::build(
                sg,
                n,
                ty,
                mv,
                texture_quality(),
                orientation,
                &mut tex,
                &mut unsupported,
            )
        };
        self.unsupported.extend(unsupported);
        // addDisplayShape: distance = the planes' bbox center (the origin) in view space.
        let mut center = Vector3::ZERO;
        mv.mult_vec_matrix3(&mut center);
        self.items.push(DrawItem {
            geometry_node: n,
            geometry: crate::volume::empty_geometry(),
            model_view: *mv,
            material: default_material(),
            textures: vec![],
            multi_texture: None,
            texture_matrices: vec![IDENTITY_F32],
            projectors: vec![],
            lights: self.local_lights.clone(),
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
            volume: Some(Rc::new(draw)),
            instances: None,
            splats: None,
            custom: None,
            fog: self.local_fogs.last().cloned(),
        });
    }

    /// The named textures of a PhysicalMaterial or SpecularGlossinessMaterial (X3DCast
    /// (X3DSingleTextureNode, field)); one that has not loaded binds the default white texture.
    fn material_textures(
        &mut self,
        sg: &Sg,
        m: NodeId,
        transforms: &[String],
        coords: &[String],
    ) -> Vec<MaterialTexture> {
        // The kinds of Material (Phong): diffuse, emissive, specular, occlusion, normal in the slots of
        // the physical kinds, ambient and shininess after them (Material.getTextureIndices).
        let names: &[&str] = match sg.type_name(m) {
            Some("PhysicalMaterial") => &[
                "baseTexture",
                "emissiveTexture",
                "metallicRoughnessTexture",
                "occlusionTexture",
                "normalTexture",
            ],
            Some("SpecularGlossinessMaterial") => &[
                "diffuseTexture",
                "emissiveTexture",
                "specularGlossinessTexture",
                "occlusionTexture",
                "normalTexture",
            ],
            Some("Material") => &[
                "diffuseTexture",
                "emissiveTexture",
                "specularTexture",
                "occlusionTexture",
                "normalTexture",
                "ambientTexture",
                "shininessTexture",
            ],
            _ => return vec![],
        };
        let mut out = vec![];

        for (kind, name) in names.iter().enumerate() {
            let Some(t) = sg.node(m, name).filter(|t| is_single_texture(sg, *t)) else {
                continue;
            };
            let texture = match self.texture(sg, t) {
                Some(t) => t,
                None => {
                    if !can_load(sg, t) {
                        self.unsupported
                            .insert(sg.type_name(t).unwrap_or("?").to_string());
                    }
                    self.white_texture()
                }
            };
            let mapping = sg.string(m, &format!("{name}Mapping"), "");
            out.push(MaterialTexture {
                kind: kind as u32,
                texture,
                transform: mapping_channel(transforms, &mapping).min(3),
                coord: mapping_channel(coords, &mapping).min(3),
            });
        }

        // X3DMaterialExtensionNode.addTexture: the extension textures, kinds 5.. (a later node of
        // the same type with the texture sets its uniform last).
        for e in extension_nodes(sg, m) {
            let ty = sg.type_name(e).unwrap_or("");

            for (i, (ext, name)) in EXTENSION_TEXTURES.iter().enumerate() {
                if *ext != ty {
                    continue;
                }

                let Some(t) = sg.node(e, name).filter(|t| is_single_texture(sg, *t)) else {
                    continue;
                };
                let texture = match self.texture(sg, t) {
                    Some(t) => t,
                    None => {
                        if !can_load(sg, t) {
                            self.unsupported
                                .insert(sg.type_name(t).unwrap_or("?").to_string());
                        }
                        self.white_texture()
                    }
                };
                let mapping = sg.string(e, &format!("{name}Mapping"), "");
                let kind = 5 + i as u32;
                out.retain(|o| o.kind != kind);
                out.push(MaterialTexture {
                    kind,
                    texture,
                    transform: mapping_channel(transforms, &mapping).min(3),
                    coord: mapping_channel(coords, &mapping).min(3),
                });
            }
        }

        out
    }

    /// X3DBackgroundNode.textureBits: the faces whose texture loaded. Background loads its six
    /// private ImageTextures (backgroundTextureProperties); TextureBackground uses its
    /// X3DTexture2DNode fields.
    fn background_faces(&mut self, sg: &Sg, b: NodeId) -> Vec<BackgroundFace> {
        let mut faces = vec![];
        let textured = sg.type_name(b) == Some("TextureBackground");
        let names = ["front", "back", "left", "right", "top", "bottom"];

        for (i, name) in names.iter().enumerate() {
            let texture = if textured {
                match sg.node(b, &format!("{name}Texture")) {
                    Some(t) => match sg.type_name(t) {
                        Some("ImageTexture") => self.texture(sg, t),
                        // X3DBackgroundNode.set_loadState__: a PixelTexture without an image is COMPLETE (texture bit set); its face
                        // shows X_ITE's 1 x 1 white default texture.
                        Some("PixelTexture") => {
                            self.texture(sg, t).or_else(|| Some(self.white_texture()))
                        }
                        Some(other) => {
                            self.unsupported.insert(other.to_string());
                            None
                        }
                        None => None,
                    },
                    None => None,
                }
            } else {
                let ckey = (sg.rt.uid, b, i);
                let fp = if self.cache.is_some() {
                    fingerprint(sg, b)
                } else {
                    0
                };
                let hit = self.cache.as_deref_mut().and_then(|c| {
                    let frame = c.frame;
                    c.faces
                        .get_mut(&ckey)
                        .filter(|e| e.fingerprint == fp)
                        .map(|e| {
                            e.used = frame;
                            e.value.clone()
                        })
                });

                match hit {
                    Some(t) => t,
                    None => {
                        // The url the face's private ImageTexture last requested (a change while
                        // LoadUrlObjects is FALSE does not load).
                        let field = format!("{name}Url");
                        let private = sg
                            .rt
                            .background_textures
                            .get(&b)
                            .and_then(|v| v.iter().find(|(f, _)| *f == field).map(|(_, t)| *t));
                        let urls = private
                            .and_then(|t| sg.rt.texture_url(t))
                            .unwrap_or_else(|| sg.strings(b, &field));
                        let t = texture::load_background_texture(
                            &urls,
                            sg.rt.scene.world_url_of(sg.rt.scene.node(b).ctx),
                            self.root.fetcher.as_ref(),
                        )
                        .map(Rc::new);

                        if let Some(c) = self.cache.as_deref_mut() {
                            let frame = c.frame;
                            c.faces.insert(
                                ckey,
                                CacheEntry {
                                    fingerprint: fp,
                                    value: t.clone(),
                                    used: frame,
                                },
                            );
                        }

                        t
                    }
                }
            };

            if let Some(texture) = texture {
                faces.push(BackgroundFace {
                    vertices: FACE_VERTICES[i],
                    texture,
                });
            }
        }

        faces
    }

    /// GeneratedCubeMapTexture.renderTexture, face `i`: the layer drawn by a DependentRenderer from
    /// the texture's model matrix (rotations to the negated face normals, scales), 90° square
    /// perspective with the viewpoint's near and far distance, the headlight placed by the main
    /// camera, the shapes that use the texture hidden.
    #[allow(clippy::too_many_arguments)]
    fn cube_face(
        &mut self,
        sg: &Sg,
        roots: &[NodeId],
        node: NodeId,
        model: &Matrix4,
        i: usize,
        near: f64,
        far: f64,
        headlight: Option<&Matrix4>,
        bound: &Bound,
        background_far: f64,
        background_faces: &[BackgroundFace],
    ) -> Frame {
        let z = Vector3::Z_AXIS;
        let to = [
            Vector3::new(0.0, 0.0, -1.0),
            z,
            Vector3::X_AXIS,
            Vector3::new(-1.0, 0.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            Vector3::Y_AXIS,
        ];
        let scales = [
            Vector3::new(-1.0, -1.0, 1.0),
            Vector3::new(-1.0, -1.0, 1.0),
            Vector3::new(-1.0, -1.0, 1.0),
            Vector3::new(-1.0, -1.0, 1.0),
            Vector3::ONE,
            Vector3::ONE,
        ];

        let mut rotation = Rotation4::identity();
        rotation.set_vectors(&z, &to[i]);
        let mut camera_space = *model;
        camera_space.rotate(&rotation);
        camera_space.scale(&scales[i]);
        let mut view = camera_space;
        view.inverse();

        // A DependentRenderer pass of its own.
        let saved = (
            std::mem::take(&mut self.items),
            std::mem::take(&mut self.lights),
            std::mem::take(&mut self.clip_planes),
            self.camera_space,
            self.dependent,
            self.hidden_texture,
            self.environment.take(),
        );
        let saved_lights = (
            std::mem::take(&mut self.global_lights),
            std::mem::take(&mut self.local_lights),
            std::mem::take(&mut self.local_fogs),
        );
        let saved_projectors = (
            std::mem::take(&mut self.projectors),
            std::mem::take(&mut self.global_projectors),
            std::mem::take(&mut self.local_projectors),
        );
        let size = sg.i32(node, "size", 128).max(1) as f64;
        let saved_viewports = std::mem::replace(&mut self.viewports, vec![[0.0, 0.0, size, size]]);
        self.camera_space = camera_space;
        self.dependent = true;
        self.hidden_texture = Some(node);

        // headlightContainer .modelViewMatrix = invCameraSpace, multLeft (viewpoint cameraSpace).
        if let Some(vp_camera_space) = headlight {
            self.global_lights.push(self.lights.len() as u32);
            let mut m = view;
            m.mult_left(vp_camera_space);
            let mut d = Vector3::new(0.0, 0.0, -1.0);
            m.mult_dir_matrix(&mut d);
            d.normalize();
            self.lights.push(LightParams {
                kind: 1,
                color: [1.0, 1.0, 1.0],
                intensity: 1.0,
                ambient_intensity: 0.0,
                direction: d,
                location: Vector3::ZERO,
                attenuation: [0.0; 3],
                radius: -1.0,
                cut_off_angle: 0.0,
                beam_width: 0.0,
                matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                shadow: None,
            });
        }

        // The layer's Group: lights first, its clip planes around the roots.
        self.group_children(sg, roots.to_vec(), &view, 0);

        let mut items = std::mem::take(&mut self.items);
        let lights = std::mem::take(&mut self.lights);
        shape_lights(&mut items, &std::mem::take(&mut self.global_lights));
        shape_projectors(&mut items, &std::mem::take(&mut self.global_projectors));
        let projectors = std::mem::take(&mut self.projectors);
        let environment = self.environment.take();
        (
            self.items,
            self.lights,
            self.clip_planes,
            self.camera_space,
            self.dependent,
            self.hidden_texture,
            self.environment,
        ) = saved;
        (self.global_lights, self.local_lights, self.local_fogs) = saved_lights;
        (
            self.projectors,
            self.global_projectors,
            self.local_projectors,
        ) = saved_projectors;
        self.viewports = saved_viewports;

        Frame {
            projection: perspective(std::f64::consts::FRAC_PI_2, near, far, 1.0, 1.0),
            view,
            items,
            lights,
            background: bound.background.and_then(|(b, m)| {
                background_params(sg, b, &m, &view, background_far, background_faces.to_vec())
            }),
            fog: bound
                .fog
                .and_then(|f| fog_params(sg, f, bound.fog_matrices.get(&f), &view)),
            unsupported: BTreeSet::new(),
            viewpoint: None,
            cube_passes: vec![],
            rendered_passes: vec![],
            environment,
            tone_mapping: 0,
            color_space: color_space(&self.root.browser_options),
            exposure: self
                .root
                .browser_options
                .get_option("Exposure")
                .map_or(1.0, |v| v.as_f64())
                .max(0.0),
            viewport: None,
            layers: vec![],
            // The main pass's maps (the texture pass draws none of its own).
            shadow_maps: self.shadow_maps.clone(),
            projectors,
        }
    }

    /// RenderedTexture.renderTexture: a DependentRenderer pass of width × height from the
    /// texture's Viewpoint child (identity model matrix) or the layer's viewpoint, with the layer's
    /// NavigationInfo near and far; the Group of its other children, or the whole layer when
    /// there are none; its Background and Fog children, or the layer's. The headlight
    /// (NavigationInfo child or the layer's, not for a depth map) uses the container matrix
    /// `view (layer) × cameraSpace (texture viewpoint)`: pushed as a light of its own for the
    /// children, the layer's NavigationInfo pushes it (if on) for the layer.
    fn rendered_pass(
        &mut self,
        sg: &Sg,
        node: NodeId,
        roots: &[NodeId],
        bound: &Bound,
        layer: &PassCamera,
        background_far: f64,
    ) -> RenderedPass {
        let (width, height) = (
            sg.i32(node, "width", 128).max(1) as u32,
            sg.i32(node, "height", 128).max(1) as u32,
        );
        let depth_map = sg.bool(node, "depthMap", false);
        let children = rendered_children(sg, node);

        // viewpointNode .getProjectionMatrix (dependentRenderer, viewport).
        let (camera_space, fov, near, far, ortho) = match children.viewpoint {
            Some(v) => {
                let ty = sg.type_name(v).unwrap_or("Viewpoint");
                let is_ortho = ty == "OrthoViewpoint";
                let mut camera_space = Matrix4::IDENTITY;
                camera_space.set_transform(
                    Some(&sg.vec3(v, "position", Vector3::new(0.0, 0.0, 10.0))),
                    Some(&sg.rotation(v, "orientation")),
                    Some(&Vector3::ONE),
                    Some(&Rotation4::identity()),
                    None,
                );
                let far_max_val = if is_ortho { 1e4 } else { far_max() };
                let (near, far) = match bound.navigation_info {
                    Some(ni) => {
                        let radius = sg
                            .f64s(ni, "avatarSize")
                            .first()
                            .copied()
                            .unwrap_or(0.25)
                            .max(0.0);
                        let limit = sg.f64(ni, "visibilityLimit", 0.0).max(0.0);
                        (
                            if radius == 0.0 { 0.125 } else { radius / 2.0 },
                            if limit != 0.0 { limit } else { far_max_val },
                        )
                    }
                    None => (0.125, far_max_val),
                };
                let ortho = is_ortho.then(|| sg.ortho_field_of_view(v));
                let fov = sg.f64(v, "fieldOfView", std::f64::consts::FRAC_PI_4);
                let fov = if fov > 0.0 && fov < std::f64::consts::PI {
                    fov
                } else {
                    std::f64::consts::FRAC_PI_4
                };
                (camera_space, fov, near, far, ortho)
            }
            None => (
                layer.camera_space,
                layer.fov,
                layer.near,
                layer.far,
                layer.ortho,
            ),
        };
        let projection = match ortho {
            Some(f) => ortho_projection(f, near, far, width as f64, height as f64),
            None => perspective(fov, near, far, width as f64, height as f64),
        };
        let mut view = camera_space;
        view.inverse();

        // A DependentRenderer pass of its own.
        let saved = (
            std::mem::take(&mut self.items),
            std::mem::take(&mut self.lights),
            std::mem::take(&mut self.clip_planes),
            self.camera_space,
            self.dependent,
            self.hidden_texture,
            self.environment.take(),
        );
        let saved_lights = (
            std::mem::take(&mut self.global_lights),
            std::mem::take(&mut self.local_lights),
            std::mem::take(&mut self.local_fogs),
        );
        let saved_projectors = (
            std::mem::take(&mut self.projectors),
            std::mem::take(&mut self.global_projectors),
            std::mem::take(&mut self.local_projectors),
        );
        let saved_viewports = std::mem::replace(
            &mut self.viewports,
            vec![[0.0, 0.0, width as f64, height as f64]],
        );
        self.camera_space = camera_space;
        self.dependent = true;
        self.hidden_texture = None;

        let navigation_info = children.navigation_info.or(bound.navigation_info);
        let headlight =
            !depth_map && navigation_info.is_none_or(|ni| sg.bool(ni, "headlight", true));
        let mut container = layer.view;
        container.mult_left(&camera_space);
        let headlight_matrix = if children.group.is_empty() {
            (layer.headlight && !depth_map).then(|| {
                if headlight {
                    container
                } else {
                    Matrix4::IDENTITY
                }
            })
        } else {
            headlight.then_some(container)
        };

        // Headlights left by earlier depth map passes of the layer (the same container, drawn with
        // the matrix it has now).
        let leaked = if depth_map || !layer.headlight {
            0
        } else {
            sg.rt.rendered_texture_leaked_headlights(node)
        };
        let headlights = headlight_matrix.map_or(0, |_| 1) + leaked;
        let headlight_matrix = headlight_matrix.or_else(|| {
            (leaked > 0).then(|| {
                if headlight {
                    container
                } else {
                    Matrix4::IDENTITY
                }
            })
        });

        for m in headlight_matrix.iter().cycle().take(headlights as usize) {
            self.global_lights.push(self.lights.len() as u32);
            let mut d = Vector3::new(0.0, 0.0, -1.0);
            m.mult_dir_matrix(&mut d);
            d.normalize();
            self.lights.push(LightParams {
                kind: 1,
                color: [1.0, 1.0, 1.0],
                intensity: 1.0,
                ambient_intensity: 0.0,
                direction: d,
                location: Vector3::ZERO,
                attenuation: [0.0; 3],
                radius: -1.0,
                cut_off_angle: 0.0,
                beam_width: 0.0,
                matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                shadow: None,
            });
        }

        let pass_roots = if children.group.is_empty() {
            roots.to_vec()
        } else {
            children.group.clone()
        };
        self.group_children(sg, pass_roots, &view, 0);

        let mut items = std::mem::take(&mut self.items);
        let lights = std::mem::take(&mut self.lights);
        shape_lights(&mut items, &std::mem::take(&mut self.global_lights));
        shape_projectors(&mut items, &std::mem::take(&mut self.global_projectors));
        let projectors = std::mem::take(&mut self.projectors);
        let environment = self.environment.take();
        (
            self.items,
            self.lights,
            self.clip_planes,
            self.camera_space,
            self.dependent,
            self.hidden_texture,
            self.environment,
        ) = saved;
        (self.global_lights, self.local_lights, self.local_fogs) = saved_lights;
        (
            self.projectors,
            self.global_projectors,
            self.local_projectors,
        ) = saved_projectors;
        self.viewports = saved_viewports;

        // The texture's Background child (not bound: identity model matrix) or the layer's; a
        // depth map draws none.
        let background = if depth_map {
            None
        } else {
            match children.background {
                Some(b) => {
                    let faces = self.background_faces(sg, b);
                    background_params(sg, b, &Matrix4::IDENTITY, &view, background_far, faces)
                }
                None => bound.background.and_then(|(b, m)| {
                    let faces = self.background_faces(sg, b);
                    background_params(sg, b, &m, &view, background_far, faces)
                }),
            }
        };
        let fog = match children.fog {
            Some(f) => fog_params(sg, f, None, &view),
            None => bound
                .fog
                .and_then(|f| fog_params(sg, f, bound.fog_matrices.get(&f), &view)),
        };

        let frame = Frame {
            projection,
            view,
            items,
            lights,
            background,
            fog,
            unsupported: BTreeSet::new(),
            viewpoint: None,
            cube_passes: vec![],
            rendered_passes: vec![],
            environment,
            tone_mapping: 0,
            color_space: color_space(&self.root.browser_options),
            exposure: self
                .root
                .browser_options
                .get_option("Exposure")
                .map_or(1.0, |v| v.as_f64())
                .max(0.0),
            viewport: None,
            layers: vec![],
            shadow_maps: self.shadow_maps.clone(),
            projectors,
        };

        RenderedPass {
            key: (sg.rt.uid, node),
            width,
            height,
            depth_map,
            frame,
        }
    }

    pub fn build(mut self, width: u32, height: u32) -> Frame {
        let root_sg = Sg::new(self.root);
        let sg = &root_sg;
        let browser = [0.0, 0.0, width as f64, height as f64];
        self.layout_camera.content_scale = self
            .root
            .browser_options
            .rendering
            .get("ContentScale")
            .map_or(1.0, |v| v.as_f64());

        // X3DWorld.traverse: the LayerSet's layers in order, or layer 0 alone.
        let mut frame = if layer::layer_set(sg).is_none() {
            self.build_layer(sg, None, browser, false)
        } else {
            let mut layers: Vec<Frame> = layer::drawn_layers(sg)
                .into_iter()
                .map(|key| self.build_layer(sg, key, browser, true))
                .collect();
            let cube_passes = layers
                .iter_mut()
                .flat_map(|l| std::mem::take(&mut l.cube_passes))
                .collect();
            let rendered_passes = layers
                .iter_mut()
                .flat_map(|l| std::mem::take(&mut l.rendered_passes))
                .collect();

            Frame {
                projection: Matrix4::IDENTITY,
                view: Matrix4::IDENTITY,
                items: vec![],
                lights: vec![],
                background: None,
                fog: None,
                unsupported: BTreeSet::new(),
                viewpoint: None,
                cube_passes,
                rendered_passes,
                environment: None,
                tone_mapping: tone_mapping(&self.root.browser_options),
                color_space: color_space(&self.root.browser_options),
                exposure: self
                    .root
                    .browser_options
                    .get_option("Exposure")
                    .map_or(1.0, |v| v.as_f64())
                    .max(0.0),
                viewport: None,
                layers,
                shadow_maps: vec![],
                projectors: vec![],
            }
        };

        frame.unsupported = std::mem::take(&mut self.unsupported);

        if let Some(c) = self.cache.as_deref_mut() {
            c.end_frame();
        }

        frame
    }

    /// X3DLayerNode.traverse of one layer: its CAMERA pass (bindables), its viewport, then its
    /// DISPLAY pass into a frame. `layered`: the world has a LayerSet (the frame gets its
    /// viewport, and layer 0 its default Background).
    fn build_layer(&mut self, sg: &Sg, key: LayerKey, browser: [f64; 4], layered: bool) -> Frame {
        self.layer = key;
        let roots = layer::layer_children(sg, key);
        let layout_layer = layer::is_layout_layer(sg, key);
        let viewport = layer::layer_viewport(sg, key, browser);
        let (width, height) = (viewport[2], viewport[3]);
        let camera = if key.is_none() {
            self.camera.clone()
        } else {
            self.layer_cameras
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, c)| c.clone())
        };

        // CAMERA pass.
        let mut bound = Bound {
            viewpoint: None,
            navigation_info: None,
            background: None,
            fog: None,
            fog_matrices: HashMap::new(),
        };
        for r in &roots {
            self.camera(sg, *r, &Matrix4::IDENTITY, &mut bound, 0);
        }

        if let Some(o) = &camera {
            bound.viewpoint = o.viewpoint.map(|v| (v, Matrix4::IDENTITY));
            bound.navigation_info = o.navigation_info;
            bound.background = o.background;
            bound.fog = o.fog;
        }

        // NavigationInfo.set_type__: the active viewer.
        let viewer = match bound.navigation_info {
            Some(ni) => navigation_viewer(&sg.strings(ni, "type")),
            None => "EXAMINE".to_string(),
        };

        // Viewpoint (default: position 0 0 10, fieldOfView π/4; a LayoutLayer's default viewpoint
        // is an OrthoViewpoint).
        let mut up_vector = Vector3::Y_AXIS;
        let (position, orientation, fov, vp_model, is_ortho, ortho_fov) = match bound.viewpoint {
            Some((v, m)) => {
                let ty = sg.type_name(v).unwrap_or("Viewpoint");
                if ty == "OrthoViewpoint" {
                    let fov = sg.ortho_field_of_view(v);
                    (
                        sg.vec3(v, "position", Vector3::new(0.0, 0.0, 10.0)),
                        sg.rotation(v, "orientation"),
                        std::f64::consts::FRAC_PI_4,
                        m,
                        true,
                        fov,
                    )
                } else {
                    let fov = sg.f64(v, "fieldOfView", std::f64::consts::FRAC_PI_4);
                    let fov = if fov > 0.0 && fov < std::f64::consts::PI {
                        fov
                    } else {
                        std::f64::consts::FRAC_PI_4
                    };
                    let mut position = sg.vec3(v, "position", Vector3::new(0.0, 0.0, 10.0));
                    let mut orientation = sg.rotation(v, "orientation");

                    if ty == "GeoViewpoint" {
                        // GeoViewpoint.getPosition / getOrientation.
                        let geo = sg.geo_object(v);
                        let mut r = Rotation4::identity();
                        r.set_matrix(&geo.get_location_matrix(&position).submatrix());
                        r.mult_left(&orientation);
                        orientation = r;
                        position = geo.get_coord(&position);

                        // GeoViewpoint.getUpVector (true), no position offset yet.
                        up_vector = if position.norm() < 6.5e6 {
                            geo.get_geo_up_vector(&position)
                        } else {
                            Vector3::Z_AXIS
                        };
                    }

                    (position, orientation, fov, m, false, [-1.0, -1.0, 1.0, 1.0])
                }
            }
            None => (
                Vector3::new(0.0, 0.0, 10.0),
                Rotation4::identity(),
                std::f64::consts::FRAC_PI_4,
                Matrix4::IDENTITY,
                layout_layer,
                [-1.0, -1.0, 1.0, 1.0],
            ),
        };

        // ExamineViewer.set_activeViewpoint__: with StraightenHorizon (default TRUE) the EXAMINE
        // viewer sets an orientation offset that straightens the bound viewpoint's horizon.
        let orientation = if viewer == "EXAMINE" {
            straighten(&orientation, &up_vector)
        } else {
            orientation
        };

        // X3DViewpointNode.update: cameraSpaceMatrix = T(position) R(orientation) × modelMatrix.
        let mut camera_space = Matrix4::IDENTITY;
        camera_space.set_transform(
            Some(&position),
            Some(&orientation),
            Some(&Vector3::ONE),
            Some(&Rotation4::identity()),
            None,
        );
        camera_space.mult_right(&vp_model);
        let mut view = camera_space;
        view.inverse();

        // NavigationInfo: near = avatarSize [0] / 2 (0.125 if 0), far = visibilityLimit or 1e5 (1e4 for OrthoViewpoint).
        // GeoViewpoint.getMaxFarValue: 1e9.
        let is_geo = bound
            .viewpoint
            .is_some_and(|(v, _)| sg.type_name(v) == Some("GeoViewpoint"));
        let far_max_val = if is_ortho {
            1e4
        } else if is_geo {
            1e9
        } else {
            far_max()
        };
        let (headlight, near, far) = match bound.navigation_info {
            Some(ni) => {
                let avatar = sg.f64s(ni, "avatarSize");
                let radius = avatar.first().copied().unwrap_or(0.25).max(0.0);
                let near = if radius == 0.0 { 0.125 } else { radius / 2.0 };
                let limit = sg.f64(ni, "visibilityLimit", 0.0).max(0.0);
                (
                    sg.bool(ni, "headlight", true),
                    near,
                    if limit != 0.0 { limit } else { far_max_val },
                )
            }
            None => (true, 0.125, far_max_val),
        };

        let projection = if is_ortho {
            ortho_projection(ortho_fov, near, far, width, height)
        } else {
            perspective(fov, near, far, width, height)
        };

        self.layout_camera.ortho =
            is_ortho.then(|| (ortho_fov[2] - ortho_fov[0], ortho_fov[3] - ortho_fov[1]));
        self.layout_camera.fov = fov;
        self.layout_camera.near = near;

        let (view, projection, headlight, near, far) = match &camera {
            Some(o) => {
                let mut view = o.camera_space;
                view.inverse();
                self.layout_camera.ortho = o.ortho_size;
                self.layout_camera.fov = o.fov;
                self.layout_camera.near = o.near;
                (
                    view,
                    o.projection
                        .unwrap_or_else(|| perspective(o.fov, o.near, o.far, width, height)),
                    o.headlight,
                    o.near,
                    o.far,
                )
            }
            None => (view, projection, headlight, near, far),
        };

        self.projection = projection;
        let mut camera_space = view;
        camera_space.inverse();
        self.camera_space = camera_space;
        self.background_transparent = bound
            .background
            .is_some_and(|(b, _)| background_transparent(sg, b));
        self.viewports = vec![viewport];
        self.layouts = LayoutStack::default();

        // DISPLAY pass. NavigationInfo.enable pushes the headlight first.
        if headlight {
            self.global_lights.push(self.lights.len() as u32);
            self.lights.push(LightParams {
                kind: 1,
                color: [1.0, 1.0, 1.0],
                intensity: 1.0,
                ambient_intensity: 0.0,
                direction: Vector3::new(0.0, 0.0, -1.0),
                location: Vector3::ZERO,
                attenuation: [0.0; 3],
                radius: -1.0,
                cut_off_angle: 0.0,
                beam_width: 0.0,
                matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                shadow: None,
            });
        }

        // X3DLayerNode: the root nodes are the children of the layer's Group (a LayoutLayer's
        // is a LayoutGroup with the layer's layout).
        self.layer_roots = roots.clone();
        self.shadow_maps.clear();
        self.shadow_sources.clear();
        self.projector_sources.clear();
        self.groups = vec![ShadowGroup::Roots(roots.clone())];

        if layout_layer {
            self.layout_group(
                sg,
                key.unwrap(),
                sg.node(key.unwrap(), "layout"),
                None,
                roots.clone(),
                &view,
                0,
            );
        } else {
            self.group_children(sg, roots.clone(), &view, 0);
        }

        self.clip_planes.clear();

        // X3DRenderObject.setGlobalFog: modelViewMatrix = fog modelMatrix × viewMatrix, inverted 3×3.
        let fog = bound
            .fog
            .and_then(|f| fog_params(sg, f, bound.fog_matrices.get(&f), &view));

        let faces = bound
            .background
            .map(|(b, _)| self.background_faces(sg, b))
            .unwrap_or_default();
        let background_far = camera
            .as_ref()
            .map_or(if is_geo { 1e9 } else { far_max() }, |c| c.max_far);
        let background = match bound.background {
            Some((b, m)) => background_params(sg, b, &m, &view, background_far, faces.clone()),
            // X3DLayerNode.setLayer0: only layer 0 shows its default Background (black; with a
            // single layer the cleared target already is).
            None if layered && key.is_none() => background_build(
                &[[0.0, 0.0, 0.0]],
                &[],
                &[],
                &[],
                0.0,
                vec![],
                &Matrix4::IDENTITY,
                &view,
                background_far,
            ),
            None => None,
        };

        // X3DRenderObject.draw: GeneratedCubeMapTexture.renderTexture for each texture collected.
        let mut cube_passes = vec![];

        for (node, model) in std::mem::take(&mut self.generated) {
            let size = sg.i32(node, "size", 128).max(1) as u32;
            let mut frames = vec![];

            for i in 0..6 {
                frames.push(self.cube_face(
                    sg,
                    &roots,
                    node,
                    &model,
                    i,
                    near,
                    far,
                    headlight.then_some(&camera_space),
                    &bound,
                    background_far,
                    &faces,
                ));
            }

            cube_passes.push(CubePass {
                key: (sg.rt.uid, node),
                size,
                faces: frames,
                next_frame_only: sg.string(node, "update", "NONE") == "NEXT_FRAME_ONLY",
            });
        }

        // RenderedTexture.renderTexture for each texture collected.
        let pass_camera = PassCamera {
            camera_space,
            view,
            fov: self.layout_camera.fov,
            near,
            far,
            ortho: is_ortho.then_some(ortho_fov),
            headlight,
        };
        let mut rendered_passes = vec![];

        for node in std::mem::take(&mut self.rendered) {
            self.rendered_done.push(node);
            rendered_passes.push(self.rendered_pass(
                sg,
                node,
                &roots,
                &bound,
                &pass_camera,
                background_far,
            ));
        }

        let mut items = std::mem::take(&mut self.items);
        shape_lights(&mut items, &std::mem::take(&mut self.global_lights));
        self.local_lights.clear();
        self.local_fogs.clear();
        shape_projectors(&mut items, &std::mem::take(&mut self.global_projectors));
        self.local_projectors.clear();

        Frame {
            projection,
            view,
            items,
            lights: std::mem::take(&mut self.lights),
            background,
            fog,
            unsupported: BTreeSet::new(),
            viewpoint: bound.viewpoint.map(|v| v.0),
            cube_passes,
            rendered_passes,
            environment: self.environment.take(),
            tone_mapping: tone_mapping(&self.root.browser_options),
            color_space: color_space(&self.root.browser_options),
            exposure: self
                .root
                .browser_options
                .get_option("Exposure")
                .map_or(1.0, |v| v.as_f64())
                .max(0.0),
            viewport: layered.then_some(viewport),
            layers: vec![],
            shadow_maps: std::mem::take(&mut self.shadow_maps),
            projectors: std::mem::take(&mut self.projectors),
        }
    }
}

/// A ParticleSystem in the pointing pass (X3DShapeNode.traverse POINTER → X3DRenderObject.pointing →
/// ParticleSystem.displaySimple): its instance matrices, the geometry each instance draws and the system's bbox.
pub struct ParticlePointing {
    /// The geometry the instances draw; `None` for GEOMETRY, which draws the `geometry` node's own.
    pub geometry: Option<Rc<Geometry>>,
    /// `x3d_InstanceMatrix` of the drawn particles, in buffer order (hidden particles left out, they cover no pixel).
    pub matrices: Vec<Matrix4>,
    /// The ParticleSystem's bbox (bboxSize / bboxCenter or set_bbox__), min and max: addPointingShape's view volume
    /// test and `hit .shapeNode .getBBox ()`.
    pub bbox: (Vector3, Vector3),
}

/// [`ParticlePointing`] of the ParticleSystem `n` with model-view `mv`, `None` unless it is a pointing object
/// (set_pointingObject__: isEnabled () and pointerEvents). displaySimple enables only the vertex, x3d_Particle and
/// x3d_InstanceMatrix attributes, so TRIANGLE, QUAD and SPRITE (geometryContext .hasNormals, X3D_NORMALS) read the
/// disabled x3d_Normal and x3d_TexCoord0 as the generic attribute value 0 0 0 1: their hits have normal 0 0 0 and
/// texCoord 0 0 0 1 in X_ITE. There is no X3D_INSTANCE_NORMAL, and no color or texCoord ramp in this pass.
pub fn particle_pointing(sg: &Sg, n: NodeId, mv: &Matrix4) -> Option<ParticlePointing> {
    if !sg.bool(n, "pointerEvents", true) {
        return None;
    }

    particle_simple(sg, n, mv)
}

/// The particles a ParticleSystem draws into the collision depth buffer (X3DShapeNode.traverse COLLISION →
/// addCollisionShape, then X3DRenderObject.depth → ParticleSystem.displaySimple), `None` unless it is a collision object
/// (set_collisionObject__: isEnabled (), whatever pointerEvents says). displaySimple is the pointing pass's draw, so the
/// geometry, instances and bbox are [`particle_pointing`]'s: TRIANGLE, QUAD and SPRITE read the disabled x3d_Normal as
/// 0 0 0 (their depth normal is 0 0 0), POINT and LINE have no normals (Depth2.fs: 0 0 1), and SPRITE turns to the
/// viewer of `mv`, the model-view of the COLLISION traversal (the bound viewpoint's view, not the collision camera).
pub fn particle_collision(sg: &Sg, n: NodeId, mv: &Matrix4) -> Option<ParticlePointing> {
    particle_simple(sg, n, mv)
}

/// ParticleSystem.displaySimple of the ParticleSystem `n` with model-view `mv`, `None` unless isEnabled ().
fn particle_simple(sg: &Sg, n: NodeId, mv: &Matrix4) -> Option<ParticlePointing> {
    let mut builder = Builder::new(sg.rt);

    if !builder.particle_system_enabled(sg, n) {
        return None;
    }

    let sim = sg.rt.particle_sim(n)?;
    let num = sim.num_particles.min(sim.particles.len());
    let gt = sg.rt.particle_geometry_type(n);
    let matrices =
        crate::particles::instances(&sim.particles[..num], &Default::default(), gt, false).matrices;
    let size = sg.vec2(n, "particleSize", Vector2::new(0.02, 0.02));
    let geometry = match gt {
        GeometryType::Geometry => None,
        GeometryType::Point => Some(crate::particles::point_geometry(false)),
        GeometryType::Line => Some(crate::particles::line_geometry(size.y, false)),
        _ => {
            let rows =
                (gt == GeometryType::Sprite).then(|| crate::particles::screen_aligned_rotation(mv));
            let mut g = crate::particles::quad_geometry(
                rows.as_ref().map(|r| (r, [size.x, size.y])),
                false,
            );

            g.normals = vec![0.0; g.normals.len()];
            g.tex_coords = [0.0, 0.0, 0.0, 1.0].repeat(g.vertex_count());
            Some(g)
        }
    };

    Some(ParticlePointing {
        geometry: geometry.map(Rc::new),
        matrices,
        bbox: builder.particle_system_bbox(sg, n).extents(),
    })
}

/// addPointingShape: `viewVolume .intersectsSphere (radius, bboxCenter)` of a shape's `bbox` (min, max) under `mv`.
pub fn bbox_in_view_volume(projection: &Matrix4, mv: &Matrix4, bbox: &(Vector3, Vector3)) -> bool {
    let mut size = bbox.1;
    size.subtract(&bbox.0);
    let mut center = Box3::from_extents(&bbox.0, &bbox.1).center();

    mv.mult_dir_matrix(&mut size);
    mv.mult_vec_matrix3(&mut center);

    view_volume_intersects_sphere(projection, size.norm() / 2.0, &center)
}

/// The arrays X_ITE would upload for a geometry node (X3DGeometryNode after rebuild), for tests
/// and tools. Returns None for unsupported node types.
pub fn geometry_of(rt: &x3d_runtime::Runtime, n: NodeId) -> Option<Geometry> {
    let sg = Sg::new(rt);
    let n = sg.inner(n)?;
    Builder::new(rt).build_geometry(&sg, n)
}

/// The geometry arrays the ParticleSystems emitters and bounded physics models sample
/// (`getVertices ()` and `getNormals ()`): a geometry node as X_ITE builds it, or the private
/// IndexedFaceSet of a VolumeEmitter (`creaseAngle` PI, `convex` FALSE).
pub fn particle_geometry(
    rt: &x3d_runtime::Runtime,
    request: &x3d_runtime::nodes::particle_systems::GeometryRequest,
) -> Option<x3d_runtime::nodes::particle_systems::GeometrySoup> {
    use x3d_runtime::nodes::particle_systems::{GeometryRequest, GeometrySoup};

    match request {
        GeometryRequest::Node(n) => {
            let g = geometry_of(rt, *n)?;

            Some(GeometrySoup {
                vertices: g.vertices,
                normals: g.normals,
                solid: g.solid,
            })
        }
        GeometryRequest::Volume { coord, coord_index } => {
            let sg = Sg::new(rt);
            let points = coord.and_then(|c| sg.coord_points(c));
            let mut input = IfsInput::new(&[], coord_index);

            input.coord = points.as_deref();
            input.crease_angle = std::f64::consts::PI;
            input.convex = false;

            let g = geometry::indexed_face_set(&input);

            Some(GeometrySoup {
                vertices: g.vertices,
                normals: g.normals,
                solid: g.solid,
            })
        }
    }
}

/// Hand the runtime the geometry builder of [`particle_geometry`]. Call once before a world with
/// SurfaceEmitters, VolumeEmitters or BoundedPhysicsModels loads; calling again is harmless.
pub fn install_particle_geometry() {
    x3d_runtime::nodes::particle_systems::set_geometry_provider(particle_geometry);
}

/// NavigationInfo.set_type__: first recognized type, EXAMINE by default.
fn navigation_viewer(types: &[String]) -> String {
    for t in types {
        match t.as_str() {
            "EXAMINE" | "WALK" | "FLY" | "LOOKAT" | "PLANE" | "NONE" => return t.clone(),
            "EXPLORE" => return "EXAMINE".into(),
            "PLANE_create3000.github.io" | "PLANE_create3000.de" => return "PLANE".into(),
            _ => continue,
        }
    }
    "EXAMINE".into()
}

/// Rotation4.straighten (upVector).
pub fn straighten(r: &Rotation4, up: &Vector3) -> Rotation4 {
    let mut r = *r;
    let mut up_normal = *up;
    up_normal.normalize();
    let mut local_x = Vector3::X_AXIS;
    local_x.negate();
    r.mult_vec_rot(&mut local_x);
    let mut local_z = Vector3::Z_AXIS;
    r.mult_vec_rot(&mut local_z);

    if local_z.dot(&up_normal).abs() >= 1.0 {
        return r;
    }

    let mut new_x = local_z;
    new_x.cross(&up_normal).normalize();

    if new_x.dot(&local_x) <= -1.0 {
        let rot = Rotation4::new(0.0, 0.0, 1.0, std::f64::consts::PI);
        r.mult_left(&rot);
    } else {
        let mut rot = Rotation4::identity();
        rot.set_vectors(&local_x, &new_x);
        r.mult_right(&rot);
    }

    r
}

static LOGARITHMIC_DEPTH_BUFFER: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

thread_local! {
    /// BrowserOptions PrimitiveQuality, TextureQuality and TextCompression (browser-wide in
    /// X_ITE): the values of the browser running on this thread (a viewer builds its frames on
    /// its own thread). The epoch counts changes so cached geometries and textures are rebuilt
    /// (X_ITE: the option interests rebuild the primitives, Text.requestRebuild).
    static BROWSER_QUALITIES: std::cell::Cell<(Quality, Quality, bool, u64)> = const { std::cell::Cell::new((Quality::Medium, Quality::Medium, false, 0)) };
}

/// Quality levels (Browser/Core/PrimitiveQuality.js, TextureQuality.js).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    Low,
    Medium,
    High,
}

impl Quality {
    /// `PrimitiveQuality .get (value) ?? MEDIUM` (the option string, case sensitive).
    pub fn from_option(value: &str) -> Quality {
        match value {
            "LOW" => Quality::Low,
            "HIGH" => Quality::High,
            _ => Quality::Medium,
        }
    }
}

/// set_PrimitiveQuality__, set_TextureQuality__ and the TextCompression option
/// (getTextCompression: SCALING, else CHAR_SPACING). Returns true when something changed.
pub fn set_browser_qualities(primitive: Quality, texture: Quality, text_scaling: bool) -> bool {
    BROWSER_QUALITIES.with(|q| {
        let (p, t, c, epoch) = q.get();
        let changed = (p, t, c) != (primitive, texture, text_scaling);

        q.set((primitive, texture, text_scaling, epoch + changed as u64));
        changed
    })
}

pub fn primitive_quality() -> Quality {
    BROWSER_QUALITIES.with(|q| q.get().0)
}

pub fn texture_quality() -> Quality {
    BROWSER_QUALITIES.with(|q| q.get().1)
}

pub fn text_compression_scaling() -> bool {
    BROWSER_QUALITIES.with(|q| q.get().2)
}

fn quality_epoch() -> u64 {
    BROWSER_QUALITIES.with(|q| q.get().3)
}

/// X3D `LogarithmicDepthBuffer` rendering property (a browser-wide property in X_ITE, so a
/// process-wide switch here). Set through `gpu::Renderer::set_depth_mode`.
pub fn set_logarithmic_depth_buffer(on: bool) {
    LOGARITHMIC_DEPTH_BUFFER.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn logarithmic_depth_buffer() -> bool {
    LOGARITHMIC_DEPTH_BUFFER.load(std::sync::atomic::Ordering::Relaxed)
}

/// The browser-wide options above as one value: X_ITE keeps them per browser, this port keeps them per
/// thread (qualities) and per process (logarithmic depth), so a host with several viewers on a thread saves
/// them after it ran one viewer and restores them before it runs the next (`x3d_viewer::embed`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrowserGlobals {
    qualities: (Quality, Quality, bool, u64),
    log_depth: bool,
}

impl Default for BrowserGlobals {
    /// A browser that has not seen any option: Medium qualities, no logarithmic depth buffer.
    fn default() -> Self {
        BrowserGlobals {
            qualities: (Quality::Medium, Quality::Medium, false, 0),
            log_depth: false,
        }
    }
}

/// The current values of this thread.
pub fn browser_globals() -> BrowserGlobals {
    BrowserGlobals {
        qualities: BROWSER_QUALITIES.with(|q| q.get()),
        log_depth: logarithmic_depth_buffer(),
    }
}

/// Make `g` the values of this thread.
pub fn set_browser_globals(g: BrowserGlobals) {
    BROWSER_QUALITIES.with(|q| q.set(g.qualities));
    set_logarithmic_depth_buffer(g.log_depth);
}

/// X3DViewpointNode.getMaxFarValue: 1e10 with the LogarithmicDepthBuffer rendering property, else 1e5.
pub fn far_max() -> f64 {
    if logarithmic_depth_buffer() {
        1e10
    } else {
        1e5
    }
}

/// Camera.perspective → Camera.frustum (column-major array, X_ITE layout).
pub fn perspective(fov: f64, near: f64, far: f64, width: f64, height: f64) -> Matrix4 {
    let ratio = x3d_fields::jsmath::tan(fov / 2.0) * near;

    let (l, r, b, t) = if width > height {
        let aspect = width * ratio / height;
        (-aspect, aspect, -ratio, ratio)
    } else {
        let aspect = height * ratio / width;
        (-ratio, ratio, -aspect, aspect)
    };

    let (r_l, t_b, f_n, n_2) = (r - l, t - b, far - near, 2.0 * near);
    let a = (r + l) / r_l;
    let bb = (t + b) / t_b;
    let c = -(far + near) / f_n;
    let d = -(n_2 * far) / f_n;
    let e = n_2 / r_l;
    let f = n_2 / t_b;

    Matrix4([
        e, 0.0, 0.0, 0.0, 0.0, f, 0.0, 0.0, a, bb, c, -1.0, 0.0, 0.0, d, 0.0,
    ])
}

/// Camera.ortho (column-major array, X_ITE layout).
pub fn ortho(l: f64, r: f64, b: f64, t: f64, near: f64, far: f64) -> Matrix4 {
    let r_l = r - l;
    let t_b = t - b;
    let f_n = far - near;

    let a = 2.0 / r_l;
    let bb = 2.0 / t_b;
    let c = -2.0 / f_n;
    let d = -(r + l) / r_l;
    let e = -(t + b) / t_b;
    let f = -(far + near) / f_n;

    Matrix4([
        a, 0.0, 0.0, 0.0, 0.0, bb, 0.0, 0.0, 0.0, 0.0, c, 0.0, d, e, f, 1.0,
    ])
}

/// OrthoViewpoint.getProjectionMatrixWithLimits (X_ITE layout).
pub fn ortho_projection(fov: [f64; 4], near: f64, far: f64, width: f64, height: f64) -> Matrix4 {
    let ratio = width / height;
    let (min_x, min_y, max_x, max_y) = (fov[0], fov[1], fov[2], fov[3]);
    let size_x = max_x - min_x;
    let size_y = max_y - min_y;

    if ratio > size_x / size_y {
        let center = (min_x + max_x) / 2.0;
        let size1_2 = (size_y * ratio) / 2.0;
        ortho(center - size1_2, center + size1_2, min_y, max_y, near, far)
    } else {
        let center = (min_y + max_y) / 2.0;
        let size1_2 = (size_x / ratio) / 2.0;
        ortho(min_x, max_x, center - size1_2, center + size1_2, near, far)
    }
}

/// TextureTransform.eventsProcessed.
pub fn texture_transform_matrix(sg: &Sg, n: NodeId) -> [f32; 16] {
    let translation = sg.vec2(n, "translation", Vector2::ZERO);
    let rotation = sg.f64(n, "rotation", 0.0);
    let scale = sg.vec2(n, "scale", Vector2::ONE);
    let center = sg.vec2(n, "center", Vector2::ZERO);
    let mut m3 = Matrix3([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);

    if !center.equals(&Vector2::ZERO) {
        let mut c = center;
        c.negate();
        m3.translate(&c);
    }
    if !scale.equals(&Vector2::ONE) {
        m3.scale(&scale);
    }
    if rotation != 0.0 {
        m3.rotate(rotation);
    }
    if !center.equals(&Vector2::ZERO) {
        m3.translate(&center);
    }
    if !translation.equals(&Vector2::ZERO) {
        m3.translate(&translation);
    }

    let m = m3.0;
    let mut m4 = Matrix4::IDENTITY.0;
    m4[0] = m[0];
    m4[1] = m[1];
    m4[4] = m[3];
    m4[5] = m[4];
    m4[12] = m[6];
    m4[13] = m[7];
    m4.map(|v| v as f32)
}

const IDENTITY_F32: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/// X3DTexturingContext maxTextures (and maxTextureTransforms, maxTexCoords).
pub const MAX_TEXTURES: usize = 4;

/// The texture node types this renderer loads (`Builder::texture`).
fn can_load(sg: &Sg, n: NodeId) -> bool {
    matches!(
        sg.type_name(n),
        Some(
            "ImageTexture"
                | "PixelTexture"
                | "ComposedCubeMapTexture"
                | "ImageCubeMapTexture"
                | "GeneratedCubeMapTexture"
        )
    )
}

/// The GeneratedCubeMapTexture nodes an appearance texture uses (Appearance.set_renderedTextures__:
/// the texture node's getRenderedTextures, a MultiTexture's of its textures).
/// Appearance.renderedTextures restricted to RenderedTexture nodes (getRenderedTextures of the
/// texture: itself, a MultiTexture's units, a ComposedTexture3D's slices; of the material and
/// back material: their texture fields), in that order.
pub fn rendered_textures(sg: &Sg, appearance: Option<NodeId>) -> Vec<NodeId> {
    let Some(a) = appearance else { return vec![] };
    let mut out: Vec<NodeId> = vec![];
    let mut add = |t: NodeId| {
        if sg.type_name(t) == Some("RenderedTexture") && !out.contains(&t) {
            out.push(t);
        }
    };

    if let Some(t) = sg.node(a, "texture") {
        match sg.type_name(t) {
            Some("MultiTexture") => sg.nodes(t, "texture").into_iter().for_each(&mut add),
            Some("ComposedTexture3D") => sg
                .nodes(t, "texture")
                .into_iter()
                .filter_map(|c| sg.inner(c))
                .for_each(&mut add),
            _ => add(t),
        }
    }

    for field in ["material", "backMaterial"] {
        let Some(m) = sg.node(a, field) else { continue };
        for f in &sg.rt.scene.node(m).fields {
            if let FieldValue::SFNode(Some(t)) = &f.value {
                if let Some(t) = sg.inner(*t) {
                    add(t);
                }
            }
        }
    }

    out
}

fn generated_textures(sg: &Sg, texture: Option<NodeId>) -> Vec<NodeId> {
    let Some(t) = texture else { return vec![] };

    match sg.type_name(t) {
        Some("GeneratedCubeMapTexture") => vec![t],
        Some("MultiTexture") => sg
            .nodes(t, "texture")
            .into_iter()
            .filter(|u| sg.type_name(*u) == Some("GeneratedCubeMapTexture"))
            .collect(),
        _ => vec![],
    }
}

/// BrowserOptions ToneMapping as the shader number (X3DMaterialNode.addRenderOptions).
fn tone_mapping(o: &x3d_runtime::browser_options::BrowserOptions) -> u32 {
    match o
        .get_option("ToneMapping")
        .map(|v| v.as_str().to_string())
        .as_deref()
    {
        Some("ACES_NARKOWICZ") => 1,
        Some("ACES_HILL") => 2,
        Some("ACES_HILL_EXPOSURE_BOOST") => 3,
        Some("KHR_PBR_NEUTRAL") => 4,
        _ => 0,
    }
}

/// BrowserOptions ColorSpace (X3DMaterialNode.getShaderOptions; anything else is
/// LINEAR_WHEN_PHYSICAL_MATERIAL).
fn color_space(o: &x3d_runtime::browser_options::BrowserOptions) -> u32 {
    match o
        .get_option("ColorSpace")
        .map(|v| v.as_str().to_string())
        .as_deref()
    {
        Some("SRGB") => 0,
        Some("LINEAR") => 1,
        _ => 2,
    }
}

/// Appearance.textureTransformMapping / X3DGeometryNode.textureCoordinateMapping lookup
/// (`.get (mapping) ?? 0`): a node with an empty mapping is keyed by its channel number, which no
/// mapping string matches; a later node with the same mapping wins.
fn mapping_channel(names: &[String], mapping: &str) -> u32 {
    if mapping.is_empty() {
        return 0;
    }

    names.iter().rposition(|n| n == mapping).unwrap_or(0) as u32
}

/// The mapping names of the appearance's texture transforms, by channel.
fn transform_mappings(sg: &Sg, t: Option<NodeId>) -> Vec<String> {
    match t {
        Some(t) if sg.type_name(t) == Some("MultiTextureTransform") => sg
            .nodes(t, "textureTransform")
            .into_iter()
            .map(|u| sg.string(u, "mapping", ""))
            .collect(),
        Some(t) => vec![sg.string(t, "mapping", "")],
        None => vec![],
    }
}

/// The mapping names of the geometry's texture coordinates, by channel.
fn coordinate_mappings(sg: &Sg, geometry: NodeId) -> Vec<String> {
    match sg.node(geometry, "texCoord") {
        Some(t) if sg.type_name(t) == Some("MultiTextureCoordinate") => sg
            .nodes(t, "texCoord")
            .into_iter()
            .map(|u| sg.string(u, "mapping", ""))
            .collect(),
        Some(t) => vec![sg.string(t, "mapping", "")],
        None => vec![],
    }
}

/// X3DBackgroundNode.isTransparent: transparency > 0, unless all six faces are loaded and opaque.
fn background_transparent(sg: &Sg, b: NodeId) -> bool {
    sg.f64(b, "transparency", 0.0) > 0.0
}

/// X3DCast (X3DSingleTextureNode, node).
fn is_single_texture(sg: &Sg, n: NodeId) -> bool {
    matches!(
        sg.type_name(n),
        Some(
            "ImageTexture"
                | "PixelTexture"
                | "MovieTexture"
                | "ComposedCubeMapTexture"
                | "ImageCubeMapTexture"
                | "GeneratedCubeMapTexture"
                | "ComposedTexture3D"
                | "ImageTexture3D"
                | "PixelTexture3D"
                | "ImageTextureAtlas"
                | "RenderedTexture"
        )
    )
}

/// The matrix of an X3DSingleTextureTransformNode (TextureTransform, TextureTransform3D,
/// TextureTransformMatrix3D); `None` for other nodes.
fn single_texture_transform_matrix(sg: &Sg, n: NodeId) -> Option<[f32; 16]> {
    match sg.type_name(n)? {
        "TextureTransform" => Some(texture_transform_matrix(sg, n)),
        "TextureTransform3D" => Some(texture_transform_3d_matrix(sg, n)),
        // TextureTransformMatrix3D.eventsProcessed: setMatrix (matrix).
        "TextureTransformMatrix3D" => Some(match sg.value(n, "matrix") {
            Some(FieldValue::SFMatrix4f(m)) => m.0.map(|v| v as f32),
            _ => IDENTITY_F32,
        }),
        _ => None,
    }
}

/// TextureTransform3D.eventsProcessed.
pub fn texture_transform_3d_matrix(sg: &Sg, n: NodeId) -> [f32; 16] {
    let translation = sg.vec3(n, "translation", Vector3::ZERO);
    let rotation = sg.rotation(n, "rotation");
    let scale = sg.vec3(n, "scale", Vector3::ONE);
    let center = sg.vec3(n, "center", Vector3::ZERO);
    let mut m = Matrix4::IDENTITY;

    if !center.equals(&Vector3::ZERO) {
        let mut c = center;
        c.negate();
        m.translate(&c);
    }
    if !scale.equals(&Vector3::ONE) {
        m.scale(&scale);
    }
    if !rotation.equals(&Rotation4::identity()) {
        m.rotate(&rotation);
    }
    if !center.equals(&Vector3::ZERO) {
        m.translate(&center);
    }
    if !translation.equals(&Vector3::ZERO) {
        m.translate(&translation);
    }

    m.0.map(|v| v as f32)
}

/// MultiTexture.set_color__, set_alpha__, set_mode__, set_source__, set_function__ for `count`
/// units (ModeType, SourceType and FunctionType numbers; missing entries are MODULATE / DEFAULT).
fn multi_texture_params(sg: &Sg, n: NodeId, count: usize) -> MultiTextureParams {
    let mode_type = |name: &str| -> u32 {
        match name {
            "REPLACE" => 0,
            "MODULATE" => 1,
            "MODULATE_2X" | "MODULATE2X" => 2,
            "MODULATE_4X" | "MODULATE4X" => 3,
            "ADD" => 4,
            "ADD_SIGNED" | "ADDSIGNED" => 5,
            "ADD_SIGNED_2X" | "ADDSIGNED2X" => 6,
            "ADD_SMOOTH" | "ADDSMOOTH" => 7,
            "SUBTRACT" => 8,
            "BLEND_DIFFUSE_ALPHA" | "BLENDDIFFUSEALPHA" => 9,
            "BLEND_TEXTURE_ALPHA" | "BLENDTEXTUREALPHA" => 10,
            "BLEND_FACTOR_ALPHA" | "BLENDFACTORALPHA" => 11,
            "BLEND_CURRENT_ALPHA" | "BLENDCURRENTALPHA" => 12,
            "MODULATE_ALPHA_ADD_COLOR" | "MODULATEALPHA_ADDCOLOR" => 13,
            "MODULATE_INV_ALPHA_ADD_COLOR" | "MODULATEINVALPHA_ADDCOLOR" => 14,
            "MODULATE_INV_COLOR_ADD_ALPHA" | "MODULATEINVCOLOR_ADDALPHA" => 15,
            "DOT_PRODUCT_3" | "DOTPRODUCT3" => 16,
            "SELECT_ARG1" | "SELECTARG1" => 17,
            "SELECT_ARG2" | "SELECTARG2" => 18,
            "OFF" => 19,
            _ => 1,
        }
    };
    // `mode` entries are "rgb" or "rgb,alpha".
    let modes: Vec<(u32, u32)> = sg
        .strings(n, "mode")
        .iter()
        .map(|m| {
            let parts: Vec<&str> = m.split(',').collect();
            let rgb = parts[0];
            let alpha = parts.get(1).copied().unwrap_or(rgb);
            (mode_type(rgb.trim()), mode_type(alpha.trim()))
        })
        .collect();
    let sources: Vec<u32> = sg
        .strings(n, "source")
        .iter()
        .map(|s| match s.as_str() {
            "DIFFUSE" => 1,
            "SPECULAR" => 2,
            "FACTOR" => 3,
            _ => 0,
        })
        .collect();
    let functions: Vec<u32> = sg
        .strings(n, "function")
        .iter()
        .map(|s| match s.as_str() {
            "COMPLEMENT" => 1,
            "ALPHA_REPLICATE" | "ALPHAREPLICATE" => 2,
            _ => 0,
        })
        .collect();
    let color = sg.color(n, "color", [1.0, 1.0, 1.0]);

    MultiTextureParams {
        color: [color[0], color[1], color[2], sg.f64(n, "alpha", 1.0)],
        units: (0..count)
            .map(|i| {
                let (mode, alpha_mode) = modes.get(i).copied().unwrap_or((1, 1));
                [
                    mode,
                    alpha_mode,
                    sources.get(i).copied().unwrap_or(0),
                    functions.get(i).copied().unwrap_or(0),
                ]
            })
            .collect(),
    }
}

/// Algorithm.upperBound (array, first, last, value): first index with array [i] > value.
fn upper_bound(array: &[f64], value: f64) -> usize {
    array.iter().position(|a| *a > value).unwrap_or(array.len())
}

/// X3DBackgroundNode.build + display matrices.
fn background_params(
    sg: &Sg,
    n: NodeId,
    model: &Matrix4,
    view: &Matrix4,
    far: f64,
    faces: Vec<BackgroundFace>,
) -> Option<BackgroundParams> {
    let transparency = sg.f64(n, "transparency", 0.0).clamp(0.0, 1.0);
    let sky_color = sg
        .f32s(n, "skyColor")
        .chunks_exact(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect::<Vec<_>>();
    let ground_color = sg
        .f32s(n, "groundColor")
        .chunks_exact(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect::<Vec<_>>();

    background_build(
        &sky_color,
        &ground_color,
        &sg.f64s(n, "skyAngle"),
        &sg.f64s(n, "groundAngle"),
        transparency,
        faces,
        model,
        view,
        far,
    )
}

/// X3DBackgroundNode.build and display from the field values.
#[allow(clippy::too_many_arguments)]
fn background_build(
    sky_color: &[[f32; 3]],
    ground_color: &[[f32; 3]],
    sky_angle: &[f64],
    ground_angle: &[f64],
    transparency: f64,
    faces: Vec<BackgroundFace>,
    model: &Matrix4,
    view: &Matrix4,
    far: f64,
) -> Option<BackgroundParams> {
    // drawSphere returns early for transparency 1; drawCube still draws the loaded textures.
    if transparency >= 1.0 && faces.is_empty() {
        return None;
    }

    let (sky_angle, ground_angle) = (sky_angle.to_vec(), ground_angle.to_vec());

    let mut colors: Vec<f32> = vec![];
    let mut sphere: Vec<f32> = vec![];
    let pi = std::f64::consts::PI;

    if ground_color.is_empty() && sky_color.len() == 1 {
        #[rustfmt::skip]
        let cube: [f32; 144] = [
             1.,  1., -1., 1., -1.,  1., -1., 1., -1., -1., -1., 1.,
             1.,  1., -1., 1., -1., -1., -1., 1.,  1., -1., -1., 1.,
            -1.,  1.,  1., 1.,  1.,  1.,  1., 1., -1., -1.,  1., 1.,
            -1., -1.,  1., 1.,  1.,  1.,  1., 1.,  1., -1.,  1., 1.,
            -1.,  1., -1., 1., -1.,  1.,  1., 1., -1., -1.,  1., 1.,
            -1.,  1., -1., 1., -1., -1.,  1., 1., -1., -1., -1., 1.,
             1.,  1.,  1., 1.,  1.,  1., -1., 1.,  1., -1.,  1., 1.,
             1., -1.,  1., 1.,  1.,  1., -1., 1.,  1., -1., -1., 1.,
             1.,  1.,  1., 1., -1.,  1.,  1., 1., -1.,  1., -1., 1.,
             1.,  1.,  1., 1., -1.,  1., -1., 1.,  1.,  1., -1., 1.,
            -1., -1.,  1., 1.,  1., -1.,  1., 1., -1., -1., -1., 1.,
            -1., -1., -1., 1.,  1., -1.,  1., 1.,  1., -1., -1., 1.,
        ];
        sphere.extend_from_slice(&cube);
        let c = sky_color[0];
        for _ in 0..36 {
            colors.extend_from_slice(&[c[0], c[1], c[2], 1.0]);
        }
    } else {
        let mut build = |v_angle: &[f64], angle: &[f64], color: &[[f32; 3]], bottom: bool| {
            const U_DIMENSION: usize = 20;
            let v_angle_max = if bottom { pi / 2.0 } else { pi };
            let get_color = |theta: f64| color[upper_bound(angle, theta).min(color.len() - 1)];

            for v in 0..v_angle.len() - 1 {
                let mut theta1 = v_angle[v].clamp(0.0, v_angle_max);
                let mut theta2 = v_angle[v + 1].clamp(0.0, v_angle_max);
                if bottom {
                    theta1 = pi - theta1;
                    theta2 = pi - theta2;
                }
                let z1 = (
                    x3d_fields::jsmath::cos(theta1),
                    x3d_fields::jsmath::sin(theta1),
                );
                let z2 = (
                    x3d_fields::jsmath::cos(theta2),
                    x3d_fields::jsmath::sin(theta2),
                );
                let c1 = get_color(v_angle[v]);
                let c2 = get_color(v_angle[v + 1]);

                for u in 0..U_DIMENSION {
                    let u1 = if u < U_DIMENSION - 1 { u + 1 } else { 0 };
                    let polar = |m: f64, a: f64| {
                        (
                            m * x3d_fields::jsmath::cos(a),
                            m * x3d_fields::jsmath::sin(a),
                        )
                    };
                    let phi = 2.0 * pi * (u as f64 / U_DIMENSION as f64);
                    let y1 = polar(-z1.1, phi);
                    let y2 = polar(-z2.1, phi);
                    let phi = 2.0 * pi * (u1 as f64 / U_DIMENSION as f64);
                    let y3 = polar(-z2.1, phi);
                    let y4 = polar(-z1.1, phi);

                    for c in [c1, c2, c2, c1, c1, c2] {
                        colors.extend_from_slice(&[c[0], c[1], c[2], 1.0]);
                    }
                    for p in [
                        (y1, z1.0),
                        (y3, z2.0),
                        (y2, z2.0),
                        (y1, z1.0),
                        (y4, z1.0),
                        (y3, z2.0),
                    ] {
                        sphere.extend_from_slice(&[
                            (p.0).1 as f32,
                            p.1 as f32,
                            (p.0).0 as f32,
                            1.0,
                        ]);
                    }
                }
            }
        };

        if sky_color.len() > sky_angle.len() {
            let mut v_angle = sky_angle.clone();
            if v_angle.is_empty() || v_angle[0] > 0.0 {
                v_angle.insert(0, 0.0);
            }
            if *v_angle.last().unwrap() < pi {
                v_angle.push(pi);
            }
            if v_angle.len() == 2 {
                let mid = (v_angle[0] + v_angle[1]) / 2.0;
                v_angle.insert(1, mid);
            }
            build(&v_angle, &sky_angle, &sky_color, false);
        }

        if ground_color.len() > ground_angle.len() {
            let mut v_angle: Vec<f64> = ground_angle.iter().rev().copied().collect();
            if v_angle.is_empty() || v_angle[0] < pi / 2.0 {
                v_angle.insert(0, pi / 2.0);
            }
            if *v_angle.last().unwrap() > 0.0 {
                v_angle.push(0.0);
            }
            build(&v_angle, &ground_angle, &ground_color, true);
        }
    }

    // display: rotation of (modelMatrix × viewMatrix), scaled by the max far value.
    let mut m = *model;
    m.mult_right(view);
    let rotation = rotation_of(&m);
    let mut mv = Matrix4::IDENTITY;
    mv.rotate(&rotation);
    mv.scale(&Vector3::new(far, far, far));

    if transparency >= 1.0 {
        colors.clear();
        sphere.clear();
    }

    Some(BackgroundParams {
        colors,
        sphere,
        transparency,
        model_view: mv,
        faces,
    })
}

/// X3DBackgroundNode.transferRectangle: the six face quads (front, back, left, right, top, bottom).
#[rustfmt::skip]
const FACE_VERTICES: [[f32; 24]; 6] = [
    [ 1.,  1., -1., 1., -1.,  1., -1., 1., -1., -1., -1., 1.,  1.,  1., -1., 1., -1., -1., -1., 1.,  1., -1., -1., 1.],
    [-1.,  1.,  1., 1.,  1.,  1.,  1., 1.,  1., -1.,  1., 1., -1.,  1.,  1., 1.,  1., -1.,  1., 1., -1., -1.,  1., 1.],
    [-1.,  1., -1., 1., -1.,  1.,  1., 1., -1., -1.,  1., 1., -1.,  1., -1., 1., -1., -1.,  1., 1., -1., -1., -1., 1.],
    [ 1.,  1.,  1., 1.,  1.,  1., -1., 1.,  1., -1., -1., 1.,  1.,  1.,  1., 1.,  1., -1., -1., 1.,  1., -1.,  1., 1.],
    [ 1.,  1.,  1., 1., -1.,  1.,  1., 1., -1.,  1., -1., 1.,  1.,  1.,  1., 1., -1.,  1., -1., 1.,  1.,  1., -1., 1.],
    [ 1., -1., -1., 1., -1., -1., -1., 1., -1., -1.,  1., 1.,  1., -1., -1., 1., -1., -1.,  1., 1.,  1., -1.,  1., 1.],
];

/// X3DBackgroundNode.drawCube texture coordinates for every face.
pub const FACE_TEX_COORDS: [f32; 24] = [
    1., 1., 0., 1., 0., 1., 0., 1., 0., 0., 0., 1., 1., 1., 0., 1., 0., 0., 0., 1., 1., 0., 0., 1.,
];

/// FogContainer.set / X3DFogObject: None when the fog does not draw (fogType 0).
fn fog_params(sg: &Sg, f: NodeId, model: Option<&Matrix4>, view: &Matrix4) -> Option<FogParams> {
    let type_field = match sg.value(f, "fogType") {
        Some(x3d_fields::FieldValue::SFString(s)) => s.clone(),
        _ => "LINEAR".to_string(),
    };
    let (fog_type, start, range) = fog::fog_type(
        &type_field,
        sg.f64(f, "visibilityStart", 0.0),
        sg.f64(f, "visibilityRange", 0.0),
        false,
    );

    if fog_type == 0 {
        return None;
    }

    let mut mv = model.copied().unwrap_or(Matrix4::IDENTITY);
    mv.mult_right(view);

    let mut inv = mv.submatrix();
    inv.inverse();

    let c = sg.color(f, "color", [1.0, 1.0, 1.0]);

    Some(FogParams {
        fog_type,
        color: [c[0] as f32, c[1] as f32, c[2] as f32],
        visibility_start: start as f32,
        visibility_range: range as f32,
        matrix: inv.0.map(|v| v as f32),
    })
}

/// Matrix4.getTransform (null, rotation): the rotation part of a matrix (polar decomposition via X_ITE's Matrix3 factor).
fn rotation_of(m: &Matrix4) -> Rotation4 {
    // modelViewMatrix .getTransform (null, rotation): the polar decomposition of Matrix4.factor, so a singular matrix (the
    // zero matrix X_ITE's ScreenGroup feedback can end in) gives a NaN rotation and a NaN background matrix, not the identity.
    let mut r = Rotation4::identity();
    m.get_transform(None, Some(&mut r), None, None, None);
    r
}

/// RenderingConfiguration.maxClipPlanes.
pub const MAX_CLIP_PLANES: usize = 6;

/// Plane3.multRight (matrix) of a plane given as (normal, distanceFromOrigin).
pub fn plane_mult_right(plane: [f64; 4], matrix: &Matrix4) -> [f64; 4] {
    // The point on the plane along the normal from the origin.
    let mut point = Vector3::new(
        plane[0] * plane[3],
        plane[1] * plane[3],
        plane[2] * plane[3],
    );
    // n' = ~m * n with the inverse of the upper 3x3.
    let mut normal = Vector3::new(plane[0], plane[1], plane[2]);
    let mut inv = matrix.submatrix();

    inv.inverse();
    inv.mult_matrix_vec3(&mut normal);
    normal.normalize();
    matrix.mult_vec_matrix3(&mut point);

    [normal.x, normal.y, normal.z, normal.dot(&point)]
}

/// ClipPlane.push: the plane of an enabled ClipPlane in the space of `model_view`
/// (ClipPlaneContainer.set): normal = plane.xyz, distanceFromOrigin = -plane.w, multRight.
pub fn clip_plane(sg: &Sg, n: NodeId, model_view: &Matrix4) -> Option<[f64; 4]> {
    let plane = sg.vec4(
        n,
        "plane",
        x3d_fields::math::Vector4::new(0.0, 1.0, 0.0, 0.0),
    );

    // set_enabled__: enabled && plane != 0 0 0 0.
    if !sg.bool(n, "enabled", true)
        || (plane.x == 0.0 && plane.y == 0.0 && plane.z == 0.0 && plane.w == 0.0)
    {
        return None;
    }

    Some(plane_mult_right(
        [plane.x, plane.y, plane.z, -plane.w],
        model_view,
    ))
}

/// ClipPlanes2.glsl clip (): a point (in the planes' space) is discarded when
/// `dot (vertex, normal) < distance` for any plane.
pub fn is_clipped(planes: &[[f64; 4]], point: &Vector3) -> bool {
    planes
        .iter()
        .take(MAX_CLIP_PLANES)
        .any(|p| point.x * p[0] + point.y * p[1] + point.z * p[2] < p[3])
}

/// The browser default material: UnlitMaterial, emissiveColor 1 1 1.
fn default_material() -> MaterialParams {
    MaterialParams {
        shading: Shading::Unlit,
        diffuse: [0.8; 3],
        ambient_intensity: 0.2,
        specular: [0.0; 3],
        shininess: 0.2,
        emissive: [1.0, 1.0, 1.0],
        transparency: 0.0,
        pbr: 0,
        metallic: 1.0,
        roughness: 1.0,
        normal_scale: 1.0,
        occlusion_strength: 1.0,
        textures: vec![],
        extensions: None,
    }
}

/// The uniforms an X3DMaterialNode sets (setShaderUniforms), `front` selecting a
/// TwoSidedMaterial's side. `None` for material types that are not drawn here.
fn material_params(sg: &Sg, m: NodeId, has_normals: bool, front: bool) -> Option<MaterialParams> {
    // Material.createShader / TwoSidedMaterial (Material's shader) with normals. Material's
    // constructor replaces getMaterialKey (2, Phong) with the Shading dependent one for a
    // specification version <= 3.3; TwoSidedMaterial uses Material .prototype .getMaterialKey.
    let lit = |s: Shading| if has_normals { s } else { Shading::Unlit };
    let legacy = sg
        .rt
        .scene
        .spec_version_of(sg.rt.scene.node(m).ctx)
        .trim()
        .parse::<f64>()
        .is_ok_and(|v| v <= 3.3);

    match sg.type_name(m)? {
        "Material" => Some(MaterialParams {
            shading: lit(if legacy {
                Shading::Gouraud
            } else {
                Shading::Phong
            }),
            diffuse: sg.color(m, "diffuseColor", [0.8, 0.8, 0.8]),
            ambient_intensity: sg.f64(m, "ambientIntensity", 0.2).clamp(0.0, 1.0),
            specular: sg.color(m, "specularColor", [0.0, 0.0, 0.0]),
            shininess: sg.f64(m, "shininess", 0.2).clamp(0.0, 1.0),
            emissive: sg.color(m, "emissiveColor", [0.0, 0.0, 0.0]),
            transparency: sg.f64(m, "transparency", 0.0).clamp(0.0, 1.0),
            normal_scale: sg.f64(m, "normalScale", 1.0).max(0.0),
            occlusion_strength: sg.f64(m, "occlusionStrength", 1.0).clamp(0.0, 1.0),
            ..default_material()
        }),
        // PhysicalMaterial: set_metallic__ / set_roughness__ / set_occlusionStrength__ clamp to
        // [0, 1], X3DOneSidedMaterialNode.set_normalScale__ Math .max (value, 0). The physical
        // shader is used with or without normals (no X3D_LIGHTING, no IBL without them).
        "PhysicalMaterial" => Some(MaterialParams {
            shading: Shading::Phong,
            diffuse: sg.color(m, "baseColor", [1.0, 1.0, 1.0]),
            emissive: sg.color(m, "emissiveColor", [0.0, 0.0, 0.0]),
            transparency: sg.f64(m, "transparency", 0.0).clamp(0.0, 1.0),
            pbr: 1,
            metallic: sg.f64(m, "metallic", 1.0).clamp(0.0, 1.0),
            roughness: sg.f64(m, "roughness", 1.0).clamp(0.0, 1.0),
            normal_scale: sg.f64(m, "normalScale", 1.0).max(0.0),
            occlusion_strength: sg.f64(m, "occlusionStrength", 1.0).clamp(0.0, 1.0),
            extensions: material_extensions(sg, m),
            ..default_material()
        }),
        // SpecularGlossinessMaterial: set_glossiness__ clamps to [0, 1].
        "SpecularGlossinessMaterial" => Some(MaterialParams {
            shading: Shading::Phong,
            diffuse: sg.color(m, "diffuseColor", [1.0, 1.0, 1.0]),
            specular: sg.color(m, "specularColor", [1.0, 1.0, 1.0]),
            emissive: sg.color(m, "emissiveColor", [0.0, 0.0, 0.0]),
            transparency: sg.f64(m, "transparency", 0.0).clamp(0.0, 1.0),
            pbr: 2,
            metallic: sg.f64(m, "glossiness", 1.0).clamp(0.0, 1.0),
            normal_scale: sg.f64(m, "normalScale", 1.0).max(0.0),
            occlusion_strength: sg.f64(m, "occlusionStrength", 1.0).clamp(0.0, 1.0),
            ..default_material()
        }),
        // UnlitMaterial (X3DOneSidedMaterialNode): emissiveColor and transparency.
        "UnlitMaterial" => Some(MaterialParams {
            shading: Shading::Unlit,
            emissive: sg.color(m, "emissiveColor", [1.0, 1.0, 1.0]),
            transparency: sg.f64(m, "transparency", 0.0).clamp(0.0, 1.0),
            ..default_material()
        }),
        "TwoSidedMaterial" => {
            let back = !front && sg.bool(m, "separateBackColor", false);
            let f = |name: &str| {
                if back {
                    format!("back{}{}", name[..1].to_uppercase(), &name[1..])
                } else {
                    name.to_string()
                }
            };

            // set_ambientIntensity__: Math .max (value, 0), no upper clamp.
            Some(MaterialParams {
                shading: lit(Shading::Phong),
                diffuse: sg.color(m, &f("diffuseColor"), [0.8, 0.8, 0.8]),
                ambient_intensity: sg.f64(m, &f("ambientIntensity"), 0.2).max(0.0),
                specular: sg.color(m, &f("specularColor"), [0.0, 0.0, 0.0]),
                shininess: sg.f64(m, &f("shininess"), 0.2).clamp(0.0, 1.0),
                emissive: sg.color(m, &f("emissiveColor"), [0.0, 0.0, 0.0]),
                transparency: sg.f64(m, &f("transparency"), 0.0).clamp(0.0, 1.0),
                ..default_material()
            })
        }
        _ => None,
    }
}

/// X3DMaterialNode.isTransparent.
fn material_transparent(sg: &Sg, m: NodeId, params: &MaterialParams) -> bool {
    match sg.type_name(m) {
        // TwoSidedMaterial.set_transparent__: the field values, not the clamped ones.
        Some("TwoSidedMaterial") => {
            sg.f64(m, "transparency", 0.0) != 0.0
                || (sg.bool(m, "separateBackColor", false)
                    && sg.f64(m, "backTransparency", 0.0) != 0.0)
        }
        // UnlitMaterial.set_transparent__: transparency || emissiveTexture .isTransparent (); the
        // texture's part is decided where the texture is loaded (`shape`).
        _ => params.transparency > 0.0,
    }
}

/// BlendMode.set_*__: unknown names fall back to the field defaults.
fn blend_params(sg: &Sg, b: NodeId) -> BlendParams {
    let factor = |name: &str, default: BlendFactor| {
        BlendFactor::from_name(&sg.string(b, name, "")).unwrap_or(default)
    };
    let equation = |name: &str| {
        BlendEquation::from_name(&sg.string(b, name, "")).unwrap_or(BlendEquation::Add)
    };

    BlendParams {
        color: sg.color4(b, "blendColor", [0.0; 4]),
        source_color: factor("sourceColorFactor", BlendFactor::SrcAlpha),
        source_alpha: factor("sourceAlphaFactor", BlendFactor::One),
        destination_color: factor("destinationColorFactor", BlendFactor::OneMinusSrcAlpha),
        destination_alpha: factor("destinationAlphaFactor", BlendFactor::OneMinusSrcAlpha),
        color_equation: equation("colorEquation"),
        alpha_equation: equation("alphaEquation"),
    }
}

/// DepthMode.enable state.
fn depth_params(sg: &Sg, d: NodeId) -> DepthParams {
    let func = match sg.string(d, "depthFunction", "LESS_EQUAL").as_str() {
        "NEVER" => DepthFunc::Never,
        "LESS" => DepthFunc::Less,
        "EQUAL" => DepthFunc::Equal,
        "GREATER" => DepthFunc::Greater,
        "NOT_EQUAL" => DepthFunc::NotEqual,
        "GREATER_EQUAL" => DepthFunc::GreaterEqual,
        "ALWAYS" => DepthFunc::Always,
        _ => DepthFunc::LessEqual,
    };
    let offset = sg.vec2(d, "polygonOffset", Vector2::ZERO);
    let range = sg.vec2(d, "depthRange", Vector2::new(0.0, 1.0));

    DepthParams {
        test: sg.bool(d, "depthTest", true),
        func,
        mask: sg.bool(d, "depthMask", true),
        polygon_offset: [offset.x, offset.y],
        range: [range.x.clamp(0.0, 1.0), range.y.clamp(0.0, 1.0)],
    }
}

/// Appearance.getStyleProperties (geometryType) with ContentScale 1: the PointProperties for
/// points, an applied LineProperties for lines, the FillProperties for 2D and 3D geometry.
pub fn style_params(sg: &Sg, appearance: NodeId, geometry_type: u8) -> Option<StyleParams> {
    match geometry_type {
        0 => {
            let p = sg
                .node(appearance, "pointProperties")
                .filter(|p| sg.type_name(*p) == Some("PointProperties"))?;
            let a = sg.vec3(p, "attenuation", Vector3::new(1.0, 0.0, 0.0));

            Some(StyleParams::Point(PointParams {
                scale_factor: sg.f64(p, "pointSizeScaleFactor", 1.0).max(0.0),
                min_value: sg
                    .f64(p, "pointSizeMinValue", 1.0)
                    .clamp(0.0, MAX_POINT_SIZE),
                max_value: sg
                    .f64(p, "pointSizeMaxValue", 1.0)
                    .clamp(0.0, MAX_POINT_SIZE),
                attenuation: [a.x.max(0.0), a.y.max(0.0), a.z.max(0.0)],
            }))
        }
        1 => {
            let l = sg
                .node(appearance, "lineProperties")
                .filter(|l| sg.type_name(*l) == Some("LineProperties"))?;

            if !sg.bool(l, "applied", true) {
                return None;
            }

            let linetype = sg.i32(l, "linetype", 1);

            Some(StyleParams::Line(LineParams {
                linetype: if (1..=16).contains(&linetype) {
                    linetype
                } else {
                    1
                },
                width: sg.f64(l, "linewidthScaleFactor", 0.0).max(1.0),
            }))
        }
        _ => {
            let f = sg
                .node(appearance, "fillProperties")
                .filter(|f| sg.type_name(*f) == Some("FillProperties"))?;
            let style = sg.i32(f, "hatchStyle", 1);

            Some(StyleParams::Fill(FillParams {
                filled: sg.bool(f, "filled", true),
                hatched: sg.bool(f, "hatched", true),
                hatch_style: if (1..=19).contains(&style) { style } else { 1 },
                hatch_color: sg.color(f, "hatchColor", [1.0, 1.0, 1.0]),
            }))
        }
    }
}

#[cfg(test)]
mod background_rotation_tests {
    use super::*;

    fn sphere_matrix(view: &Matrix4) -> [f64; 16] {
        let sky = [[0.1f32, 0.2, 0.3]];
        let p = background_build(
            &sky,
            &[],
            &[],
            &[],
            0.0,
            vec![],
            &Matrix4::IDENTITY,
            view,
            100.0,
        )
        .unwrap();
        p.model_view.0
    }

    /// X3DBackgroundNode.display with the camera of a ScreenGroup feedback that ended in the zero matrix (Matrix4.inverse of
    /// a singular matrix is ZERO): the rotation of Matrix4.factor is NaN, so the background matrix is NaN (not the identity) and
    /// nothing is drawn over the clear color.
    #[test]
    fn zero_view_matrix_gives_a_nan_background_matrix() {
        let m = sphere_matrix(&Matrix4::ZERO);
        assert!(m[..12].iter().all(|v| v.is_nan()), "{m:?}");
        assert_eq!(m[15], 1.0);
    }

    #[test]
    fn rigid_and_scaled_view_matrices_keep_their_rotation() {
        let mut view = Matrix4::IDENTITY;
        view.rotate(&Rotation4::new(0.0, 1.0, 0.0, 0.7));
        view.translate(&Vector3::new(3.0, -2.0, 5.0));
        let a = sphere_matrix(&view);
        let mut scaled = view;
        scaled.scale(&Vector3::new(2.0, 2.0, 2.0));
        let b = sphere_matrix(&scaled);

        for i in 0..16 {
            assert!(
                a[i].is_finite() && (a[i] - b[i]).abs() < 1e-9,
                "{i}: {a:?} {b:?}"
            );
        }

        assert!((a[0] - 100.0 * 0.7f64.cos()).abs() < 1e-9, "{a:?}");
    }
}
