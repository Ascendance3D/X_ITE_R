//! wgpu backend: X3DRenderObject.draw / drawShapes, X3DGeometryNode.display /
//! displayGeometry, X3DPointGeometryNode / X3DLineGeometryNode.display and
//! X3DBackgroundNode.display, against a colour target + 24-bit depth like
//! MultiSampleFramebuffer with Multisampling 0.
//!
//! Two entry points share one implementation:
//!
//! * [`Renderer`] draws into a texture view the host owns (a window surface texture, an
//!   offscreen texture) on the host's `wgpu::Device` / `wgpu::Queue`. It keeps every GPU
//!   resource between frames: pipelines, uniform buffers and bind groups, one vertex buffer per
//!   geometry (uploaded when the geometry is first seen, replaced when the viewer hands out a
//!   rebuilt one), one bind group per texture, the depth texture.
//! * [`Gpu`] is the headless wrapper used by `x3d-shot` and the oracle tools: its own Vulkan
//!   device, an RGBA8 target and a CPU readback.
//!
//! The shaders write display-encoded values (X_ITE's WebGL framebuffer is not sRGB), so the
//! target view must have a non-sRGB format (`Bgra8Unorm`, `Rgba8Unorm`).

use crate::texture::{Filter, TextureImage, Wrap, mip_chain};
use crate::traverse::{
    AlphaMode, BlendEquation, BlendFactor, BlendParams, DepthFunc, DrawItem, Frame, LineParams,
    MAX_CLIP_PLANES, MAX_LIGHTS, MaterialParams, Shading, StyleParams,
};
use std::collections::HashMap;
use std::rc::Rc;

use wgpu::util::DeviceExt;
use x3d_fields::NodeId;
use x3d_fields::math::{Matrix3, Matrix4, Vector4};

/// position 3, normal 3, texCoord0 4, color 4, texCoord1..3 12, tangent 4.
const FLOATS_PER_VERTEX: usize = 31;

/// texCoord1..3 (0 0 0 1), the tangent (1 0 0 1) and the fog depth (0) of vertices without them.
const VERTEX_EXTRAS: [f32; 17] = [
    0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0,
];

/// X3DTexturingContext maxTextures: MultiTexture units.
const MAX_TEXTURES: usize = 4;

/// The texture slots of a draw (bind group 2): 2D slots, then cube slots.
const SLOTS_2D: usize = 8;
const SLOTS_CUBE: usize = 4;
const SLOTS: usize = SLOTS_2D + SLOTS_CUBE;
/// The volume layout (a draw with an X3DTexture3DNode unit): the last cube slots hold 3D
/// textures, as the fragment stage has no sampled texture binding left (16).
const SLOTS_3D: usize = 2;

/// MultiSampleFramebuffer: DEPTH_COMPONENT24.
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

/// The Shading browser option (X3DShadersContext.setShading, Material getMaterialKey): `Point`
/// draws every primitive as POINTS (gl_PointSize 1), `Wireframe` sets the polygon mode LINE for
/// every triangle draw except the background's, `Phong` makes a legacy Material (specification
/// version <= 3.3) X3D_PHONG_MATERIAL (the texture changes the diffuse color before lighting;
/// X3D 4 Material and TwoSidedMaterial always are), `Flat` leaves out X3D_NORMALS
/// (X3DMaterialNode: generateFlatNormals from the screen derivatives of the vertex).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ShadingMode {
    Point,
    Wireframe,
    Flat,
    #[default]
    Gouraud,
    Phong,
}

impl ShadingMode {
    /// The RenderingProperties Shading string (POINT, WIREFRAME, FLAT, GOURAUD, PHONG).
    pub fn from_rendering_property(s: &str) -> ShadingMode {
        match s {
            "POINT" => ShadingMode::Point,
            "WIREFRAME" => ShadingMode::Wireframe,
            "FLAT" => ShadingMode::Flat,
            "PHONG" => ShadingMode::Phong,
            _ => ShadingMode::Gouraud,
        }
    }
}

/// How depth is stored and compared.
///
/// `Standard` is X_ITE's default (DEPTH_COMPONENT24, `LESS_EQUAL`, perspective z/w).
/// `Logarithmic` is X_ITE's `LogarithmicDepthBuffer` rendering property ported exactly
/// (Logarithmic2.js: vertex `depth = 1 + gl_Position.w`, fragment
/// `gl_FragDepth = log2 (depth) / log2 (far + 1)`; `getMaxFarValue` becomes 1e10), so it can be
/// compared 1:1 with X_ITE run with `logarithmicDepthBuffer="true"`.
/// `Reversed` is NOT in X_ITE: reversed-Z with a 32-bit float depth buffer, same near and far
/// planes (so the visible volume is unchanged); beyond-X_ITE, viewer-only.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DepthMode {
    #[default]
    Standard,
    Logarithmic,
    Reversed,
}

impl DepthMode {
    pub fn parse(s: &str) -> Option<DepthMode> {
        match s.to_ascii_lowercase().as_str() {
            "standard" | "default" | "off" | "0" => Some(DepthMode::Standard),
            "log" | "logarithmic" => Some(DepthMode::Logarithmic),
            "reversed" | "reverse" | "reversed-z" | "reversez" => Some(DepthMode::Reversed),
            _ => None,
        }
    }

    fn depth_format(self) -> wgpu::TextureFormat {
        if self == DepthMode::Reversed {
            wgpu::TextureFormat::Depth32Float
        } else {
            DEPTH_FORMAT
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Topology {
    Triangles,
    Lines,
    Points,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Cull {
    None,
    Front,
    Back,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct PipelineKey {
    topology: Topology,
    cull: Cull,
    ccw: bool,
    blend: bool,
    depth_write: bool,
    depth_test: bool,
    /// BlendMode: the blend state instead of the transparent pass default.
    blend_mode: Option<wgpu::BlendState>,
    /// DepthMode depthFunction instead of LEQUAL (with the depth test on).
    depth_func: Option<wgpu::CompareFunction>,
    /// DepthMode polygonOffset (units, factor bits): polygon offset fill.
    bias: (i32, u32),
    /// X3DPointGeometryNode.display: SAMPLE_ALPHA_TO_COVERAGE for points that are not transparent.
    alpha_to_coverage: bool,
    /// Vertices with the x3d_LineStipple attribute (STIPPLE_FLOATS_PER_VERTEX, `vs_stipple`).
    stipple: bool,
    /// Shading WIREFRAME: polygonModeWEBGL (FRONT_AND_BACK, LINE_WEBGL).
    polygon_line: bool,
    /// The volume slot layout (`SlotKey::1`).
    volume: bool,
}

impl PipelineKey {
    fn new(
        topology: Topology,
        cull: Cull,
        ccw: bool,
        blend: bool,
        depth_write: bool,
        depth_test: bool,
    ) -> Self {
        PipelineKey {
            topology,
            cull,
            ccw,
            blend,
            depth_write,
            depth_test,
            blend_mode: None,
            depth_func: None,
            bias: (0, 0),
            alpha_to_coverage: false,
            stipple: false,
            polygon_line: false,
            volume: false,
        }
    }
}

/// The main pipeline's shaders, layouts and options: handles that can go to another thread.
struct PipelineSource {
    device: wgpu::Device,
    shader: wgpu::ShaderModule,
    shader_3d: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    layout_3d: wgpu::PipelineLayout,
    depth_mode: DepthMode,
    debug_ids: bool,
}

impl PipelineSource {
    fn make(
        &self,
        key: PipelineKey,
        format: wgpu::TextureFormat,
        samples: u32,
        flip: bool,
    ) -> wgpu::RenderPipeline {
        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4, 3 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32];
        let stipple_attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4, 3 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32, 4 => Float32x4];
        let (entry, stride, attributes): (&str, usize, &[wgpu::VertexAttribute]) = if key.stipple {
            ("vs_stipple", STIPPLE_FLOATS_PER_VERTEX, &stipple_attributes)
        } else {
            ("vs_main", FLOATS_PER_VERTEX, &attributes)
        };
        let float_blend = format != wgpu::TextureFormat::Rgba32Float
            || self
                .device
                .features()
                .contains(wgpu::Features::FLOAT32_BLENDABLE);
        let blend = (key.blend && !self.debug_ids && float_blend).then_some(
            key.blend_mode.unwrap_or(wgpu::BlendState {
                // gl.blendFuncSeparate (SRC_ALPHA, ONE_MINUS_SRC_ALPHA, ONE, ONE_MINUS_SRC_ALPHA)
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
        );
        let shader = if key.volume {
            &self.shader_3d
        } else {
            &self.shader
        };
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("x3d"),
                layout: Some(if key.volume {
                    &self.layout_3d
                } else {
                    &self.layout
                }),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: (stride * 4) as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes,
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology: match key.topology {
                        Topology::Triangles => wgpu::PrimitiveTopology::TriangleList,
                        Topology::Lines => wgpu::PrimitiveTopology::LineList,
                        Topology::Points => wgpu::PrimitiveTopology::PointList,
                    },
                    front_face: if key.ccw != flip {
                        wgpu::FrontFace::Ccw
                    } else {
                        wgpu::FrontFace::Cw
                    },
                    cull_mode: match key.cull {
                        _ if key.topology != Topology::Triangles => None,
                        Cull::None => None,
                        Cull::Front => Some(wgpu::Face::Front),
                        Cull::Back => Some(wgpu::Face::Back),
                    },
                    polygon_mode: if key.polygon_line
                        && self
                            .device
                            .features()
                            .contains(wgpu::Features::POLYGON_MODE_LINE)
                    {
                        wgpu::PolygonMode::Line
                    } else {
                        wgpu::PolygonMode::Fill
                    },
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: self.depth_mode.depth_format(),
                    depth_write_enabled: Some(key.depth_write),
                    depth_compare: Some(if !key.depth_test {
                        wgpu::CompareFunction::Always
                    } else if let Some(f) = key.depth_func {
                        f
                    } else if self.depth_mode == DepthMode::Reversed {
                        wgpu::CompareFunction::GreaterEqual
                    } else {
                        wgpu::CompareFunction::LessEqual
                    }),
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState {
                        constant: key.bias.0,
                        slope_scale: f32::from_bits(key.bias.1),
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    mask: !0,
                    alpha_to_coverage_enabled: key.alpha_to_coverage
                        && samples > 1
                        && !self.debug_ids,
                },
                fragment: Some(wgpu::FragmentState {
                    module: shader,
                    entry_point: Some(if self.debug_ids { "fs_id" } else { "fs_main" }),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
    }
}

/// A main pipeline created on another thread, for the key (pipeline key, format, samples, flip).
struct PreparedPipeline {
    key: (PipelineKey, wgpu::TextureFormat, u32, bool),
    #[cfg(not(target_arch = "wasm32"))]
    job: std::thread::JoinHandle<wgpu::RenderPipeline>,
}

/// wasm32 has no threads, and its wgpu objects are not `Send`: the first frame creates the pipeline.
#[cfg(target_arch = "wasm32")]
impl PreparedPipeline {
    fn start(
        _source: PipelineSource,
        _key: (PipelineKey, wgpu::TextureFormat, u32, bool),
    ) -> Option<PreparedPipeline> {
        None
    }

    fn finish(self) -> Option<wgpu::RenderPipeline> {
        unreachable!("no pipeline is prepared on wasm32")
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl PreparedPipeline {
    /// Start creating it; `None` when no thread can be started (the frame creates it instead).
    fn start(
        source: PipelineSource,
        key: (PipelineKey, wgpu::TextureFormat, u32, bool),
    ) -> Option<PreparedPipeline> {
        let job = std::thread::Builder::new()
            .name("x3d pipeline".to_string())
            .spawn(move || source.make(key.0, key.1, key.2, key.3))
            .ok()?;
        Some(PreparedPipeline { key, job })
    }

    /// Wait for it; `None` when its thread panicked (the frame creates it instead).
    fn finish(self) -> Option<wgpu::RenderPipeline> {
        self.job.join().ok()
    }
}

/// BlendMode.enable as a WebGPU blend state and constant. WebGPU's constant factor uses the
/// constant's own channel, so CONSTANT_ALPHA in the color factors is the constant (a, a, a, a);
/// a color equation using both CONSTANT_COLOR and CONSTANT_ALPHA cannot be expressed and uses the
/// color constant for both. MIN and MAX ignore the factors (WebGPU requires ONE).
fn blend_state(b: &BlendParams) -> (wgpu::BlendState, [f64; 4]) {
    use wgpu::BlendFactor as F;

    let uses = |f: &[BlendFactor]| {
        [b.source_color, b.destination_color]
            .iter()
            .any(|x| f.contains(x))
    };
    let alpha_constant = uses(&[
        BlendFactor::ConstantAlpha,
        BlendFactor::OneMinusConstantAlpha,
    ]) && !uses(&[
        BlendFactor::ConstantColor,
        BlendFactor::OneMinusConstantColor,
    ]);
    let constant = if alpha_constant {
        [b.color[3]; 4]
    } else {
        b.color
    };

    let factor = |f: BlendFactor| match f {
        BlendFactor::Zero => F::Zero,
        BlendFactor::One => F::One,
        BlendFactor::SrcColor => F::Src,
        BlendFactor::OneMinusSrcColor => F::OneMinusSrc,
        BlendFactor::DstColor => F::Dst,
        BlendFactor::OneMinusDstColor => F::OneMinusDst,
        BlendFactor::SrcAlpha => F::SrcAlpha,
        BlendFactor::OneMinusSrcAlpha => F::OneMinusSrcAlpha,
        BlendFactor::DstAlpha => F::DstAlpha,
        BlendFactor::OneMinusDstAlpha => F::OneMinusDstAlpha,
        BlendFactor::SrcAlphaSaturate => F::SrcAlphaSaturated,
        BlendFactor::ConstantColor | BlendFactor::ConstantAlpha => F::Constant,
        BlendFactor::OneMinusConstantColor | BlendFactor::OneMinusConstantAlpha => {
            F::OneMinusConstant
        }
    };
    let operation = |e: BlendEquation| match e {
        BlendEquation::Add => wgpu::BlendOperation::Add,
        BlendEquation::Subtract => wgpu::BlendOperation::Subtract,
        BlendEquation::ReverseSubtract => wgpu::BlendOperation::ReverseSubtract,
        BlendEquation::Min => wgpu::BlendOperation::Min,
        BlendEquation::Max => wgpu::BlendOperation::Max,
    };
    let component = |src: BlendFactor, dst: BlendFactor, e: BlendEquation| {
        let op = operation(e);

        if matches!(e, BlendEquation::Min | BlendEquation::Max) {
            wgpu::BlendComponent {
                src_factor: F::One,
                dst_factor: F::One,
                operation: op,
            }
        } else {
            wgpu::BlendComponent {
                src_factor: factor(src),
                dst_factor: factor(dst),
                operation: op,
            }
        }
    };

    (
        wgpu::BlendState {
            color: component(b.source_color, b.destination_color, b.color_equation),
            alpha: component(b.source_alpha, b.destination_alpha, b.alpha_equation),
        },
        constant,
    )
}

fn compare_function(f: DepthFunc) -> wgpu::CompareFunction {
    match f {
        DepthFunc::Never => wgpu::CompareFunction::Never,
        DepthFunc::Less => wgpu::CompareFunction::Less,
        DepthFunc::Equal => wgpu::CompareFunction::Equal,
        DepthFunc::LessEqual => wgpu::CompareFunction::LessEqual,
        DepthFunc::Greater => wgpu::CompareFunction::Greater,
        DepthFunc::NotEqual => wgpu::CompareFunction::NotEqual,
        DepthFunc::GreaterEqual => wgpu::CompareFunction::GreaterEqual,
        DepthFunc::Always => wgpu::CompareFunction::Always,
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LightU {
    color_type: [f32; 4],
    direction_intensity: [f32; 4],
    location_ambient: [f32; 4],
    attenuation_radius: [f32; 4],
    spot: [f32; 4],
    m0: [f32; 4],
    m1: [f32; 4],
    m2: [f32; 4],
    /// shadowColor, shadowIntensity (0 without a shadow map).
    shadow_color: [f32; 4],
    /// shadowBias, shadowMapSize, the map's first value in the shadow buffer and its row
    /// stride (u32 bits).
    shadow_params: [f32; 4],
    shadow_matrix: [f32; 16],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameU {
    projection: [f32; 16],
    num_lights: [u32; 4],
    /// Every light of the frame; each draw selects its own (DrawU.lights).
    lights: [LightU; MAX_FRAME_LIGHTS],
    /// x3d_Fog: color rgb + type, visibilityStart / visibilityRange, matrix columns.
    fog_color_type: [f32; 4],
    fog_range: [f32; 4],
    fog_m0: [f32; 4],
    fog_m1: [f32; 4],
    fog_m2: [f32; 4],
    /// Viewport width, height, flip (render target y flipped), 0.
    viewport: [f32; 4],
    /// ToneMapping, Exposure, 1 with an EnvironmentLight, specularTextureLevels.
    tone: [f32; 4],
    /// x3d_EnvironmentLightSource color, intensity; rotation columns.
    env_color: [f32; 4],
    env_m0: [f32; 4],
    env_m1: [f32; 4],
    env_m2: [f32; 4],
    /// The width and height of the layer's viewport (gl.viewport; point sprite sizes).
    layer: [f32; 4],
    /// x3d_TextureProjector: every projector of the frame (DrawU.projectors selects).
    projectors: [ProjectorU; MAX_FRAME_PROJECTORS],
    /// x: 1 in the transmission pass (X3D_LINEAR_OUTPUT), y: 1 in the volume scatter pass, z: the
    /// pixels per row of the volume scatter buffers.
    pass_flags: [f32; 4],
    /// The inverse of the GL projection matrix (getSubsurfaceScattering).
    inv_projection: [f32; 16],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ProjectorU {
    /// color, intensity.
    color: [f32; 4],
    /// location (view space), the texture is linear (1) or sRGB (0).
    location: [f32; 4],
    /// nearParameter, farParameter.
    params: [f32; 4],
    matrix: [f32; 16],
}

const MAX_FRAME_PROJECTORS: usize = 32;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawU {
    model_view: [f32; 16],
    /// x3d_TextureMatrix [X3D_NUM_TEXTURE_TRANSFORMS].
    texture_matrices: [[f32; 16]; 4],
    n0: [f32; 4],
    n1: [f32; 4],
    n2: [f32; 4],
    diffuse_ambient: [f32; 4],
    specular_shininess: [f32; 4],
    emissive_transparency: [f32; 4],
    flags: [u32; 4],
    flags2: [u32; 4],
    // x: number of clip planes, y: depth range on, z: X3D_PHONG_MATERIAL
    flags3: [u32; 4],
    /// DepthMode depthRange (near, far); z: x3d_AlphaCutoff.
    depth_range: [f32; 4],
    /// x3d_ClipPlane: normal, distanceFromOrigin.
    clip_planes: [[f32; 4]; MAX_CLIP_PLANES],
    /// x: style (0 none, 1 PointProperties, 2 LineProperties, 3 FillProperties), y: filled /
    /// linetype, z: hatched, w: 0.
    style: [u32; 4],
    /// Point: scale factor, min, max size; line: stipple scale, width; fill: hatch color.
    style_a: [f32; 4],
    /// Point: attenuation.
    style_b: [f32; 4],
    /// Texture units: binding (0 none, 1..=8 2D slot + 1, 9..=12 cube slot + 1), flip y, mode,
    /// alphaMode.
    tex_units: [[u32; 4]; 4],
    /// Texture units: source, function.
    tex_units2: [[u32; 4]; 4],
    /// x3d_TextureCoordinateGenerator per coordinate channel: mode, parameter.
    tex_gen: [[f32; 4]; 8],
    multi_color: [f32; 4],
    /// Textures, multi-texturing, coordinate channels, texture transforms.
    tex_counts: [u32; 4],
    /// MaterialTextures: binding, FLIP_Y, transform, coordinate (base, emissive,
    /// metallicRoughness, occlusion, normal, then the extension textures).
    mat_tex: [[u32; 4]; MAT_KINDS],
    /// metallic (glossiness), roughness, normalScale, occlusionStrength.
    pbr: [f32; 4],
    /// Physical kind, X3D_COLORSPACE_LINEAR, X3D_TANGENTS, X3D_NORMALS.
    pbr_flags: [u32; 4],
    /// x: the number of the shape's lights (X3D_NUM_LIGHTS); then their indices into
    /// FrameU.lights, four per vector.
    light_count: [u32; 4],
    light_index: [[u32; 4]; 2],
    /// x: X3D_NUM_TEXTURE_PROJECTORS; the projectors (FrameU.projectors) and their texture slots.
    projector_count: [u32; 4],
    projector_index: [u32; 4],
    projector_slot: [u32; 4],
    /// x: the PhysicalMaterial extensions (traverse EXT_* bits).
    ext_flags: [u32; 4],
    /// The extension uniforms (x3d.wgsl DrawUniforms.ext).
    ext: [[f32; 4]; 10],
    /// X3D_INSTANCING: x 1 for an instanced draw, y its instances have colors, z texCoord ramp corners, w the first
    /// of them in the instance buffer.
    inst_flags: [u32; 4],
    /// x: vertices per instance when every instance has its own vertices (one draw of all of them), else 0.
    inst_params: [u32; 4],
    /// x3d_Fog of the draw: color rgb + type, visibilityStart / visibilityRange, matrix columns.
    fog_color_type: [f32; 4],
    fog_range: [f32; 4],
    fog_m0: [f32; 4],
    fog_m1: [f32; 4],
    fog_m2: [f32; 4],
    /// x: X3D_FOG_COORDS.
    fog_flags: [u32; 4],
}

/// One instance of an instanced draw (x3d.wgsl Instance): its matrix, normal matrix columns, and of a
/// ParticleSystem its x3d_Color and the texCoord ramp's quad corners.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceU {
    matrix: [f32; 16],
    n0: [f32; 4],
    n1: [f32; 4],
    n2: [f32; 4],
    color: [f32; 4],
    tex: [[f32; 4]; 4],
}

/// The material texture kinds: five of the materials, sixteen of the extensions.
const MAT_KINDS: usize = 21;

/// The lights one frame uniform holds (global and local lights of all shapes).
const MAX_FRAME_LIGHTS: usize = 192;

const DRAW_STRIDE: u64 = 2048;
const _: () = assert!(std::mem::size_of::<DrawU>() as u64 <= DRAW_STRIDE);

/// X3DLightingContext.filterEnvironmentTexture uniforms (filter.wgsl).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FilterU {
    ints: [i32; 4],
    floats: [f32; 4],
    flip: [f32; 4],
}

/// The EnvironmentLight textures of the frame bind group: the specular texture they were filtered
/// from, the GeneratedDiffuseTexture, GeneratedSpecularTexture and GeneratedSheenTexture (linear
/// RGBA8 cube maps, the layers 0, 1 and 2 of one cube array).
struct EnvGpu {
    source: *const TextureImage,
    _source: Rc<TextureImage>,
    maps: (wgpu::TextureView, wgpu::Sampler),
    levels: u32,
    group: wgpu::BindGroup,
    used: u64,
}

/// Whether `format` can be the colour target: the shaders write display-encoded values, so it
/// must be a non-sRGB format with 8, 10 or 16/32 bit float channels.
pub fn target_format_ok(format: wgpu::TextureFormat) -> bool {
    matches!(
        format,
        wgpu::TextureFormat::Rgba8Unorm
            | wgpu::TextureFormat::Bgra8Unorm
            | wgpu::TextureFormat::Rgb10a2Unorm
            | wgpu::TextureFormat::Rgba16Float
            | wgpu::TextureFormat::Rgba32Float
    )
}

struct GeoGpu {
    /// Keeps the allocation (and so the address used as the key) alive while cached. Once the cache holds the only
    /// reference to a `per_frame` geometry, its buffer goes to [`Renderer::free_vertices`].
    geometry: Rc<crate::geometry::Geometry>,
    buffer: wgpu::Buffer,
    count: u32,
    used: u64,
}

/// A vertex buffer of a geometry no frame can hand out again, rewritten for the next geometry that fits (the wide
/// LINE particle lines of every frame are new geometries of about the same size).
struct FreeBuffer {
    buffer: wgpu::Buffer,
    /// The frame it was freed in.
    freed: u64,
}

/// Free vertex buffers not reused within this many frames are dropped.
const FREE_KEEP_FRAMES: u64 = 30;

/// A vertex buffer holding `verts`: the smallest free buffer that holds them and is at most twice their size,
/// rewritten, or a new one with up to 1/8 room to spare.
fn vertex_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    free: &mut Vec<FreeBuffer>,
    stats: &mut RenderStats,
    verts: &[f32],
    kind: crate::transfer::Kind,
) -> wgpu::Buffer {
    let contents: &[u8] = if verts.is_empty() {
        &[0u8; 64]
    } else {
        bytemuck::cast_slice(verts)
    };
    let buffer = pooled_buffer(
        device,
        free,
        stats,
        contents.len() as u64,
        wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    );

    queue.write_buffer(&buffer, 0, contents);
    crate::transfer::upload(kind, contents.len());
    buffer
}

/// The transfer kind of a geometry upload: the geometry of a ParticleSystem's instances, or the wide LINE triangles
/// made for one frame (particles::line_triangle_geometry).
fn particle_kind(
    instances: Option<&crate::traverse::InstanceData>,
    g: &crate::geometry::Geometry,
) -> crate::transfer::Kind {
    crate::transfer::kind(instances.is_some_and(|i| i.particle) || g.per_frame)
}

/// A buffer of at least `size` bytes with `usage`: the smallest free one that is at most twice that size, or a new one
/// with up to 1/8 room to spare.
fn pooled_buffer(
    device: &wgpu::Device,
    free: &mut Vec<FreeBuffer>,
    stats: &mut RenderStats,
    size: u64,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    let fit = free
        .iter()
        .enumerate()
        .filter(|(_, f)| f.buffer.usage() == usage && (size..=2 * size).contains(&f.buffer.size()))
        .min_by_key(|(_, f)| f.buffer.size())
        .map(|(i, _)| i);

    match fit {
        Some(i) => free.swap_remove(i).buffer,
        None => {
            let step = (size.next_power_of_two() / 8).max(wgpu::COPY_BUFFER_ALIGNMENT);
            stats.vertex_buffers_created += 1;
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("vertices"),
                size: size.div_ceil(step) * step,
                usage,
                mapped_at_creation: false,
            })
        }
    }
}

/// The triangles thick_lines.wgsl made of a [`x3d_runtime::particles::ThickLines`] (system space, 16 byte vertices).
struct ThickGpu {
    /// Keeps the address used as the key alive; once the entry holds the only reference, the buffers are free.
    lines: Rc<x3d_runtime::particles::ThickLines>,
    input: wgpu::Buffer,
    triangles: wgpu::Buffer,
    count: u32,
}

/// thick_lines.wgsl Input: the parameters, the line's vertices, the instance matrices.
fn thick_input(lines: &x3d_runtime::particles::ThickLines, stippled: bool) -> Vec<f32> {
    let (segments, instances) = (lines.vertices.len() / 8, lines.instances.len());
    let mut input: Vec<f32> = Vec::with_capacity(40 + lines.vertices.len() + 16 * instances);
    input.extend_from_slice(&lines.mvp);
    input.extend_from_slice(&lines.inverse);
    input.extend_from_slice(&lines.viewport);
    input.push(lines.half_width);
    input.extend([segments as u32, instances as u32, stippled as u32].map(f32::from_bits));
    input.extend_from_slice(&lines.vertices);
    input.extend(lines.instances.iter().flatten());
    input
}

/// The usage of thick_lines.wgsl's input and output buffers (COPY_SRC: read back by the tests).
const THICK_USAGE: wgpu::BufferUsages = wgpu::BufferUsages::STORAGE
    .union(wgpu::BufferUsages::VERTEX)
    .union(wgpu::BufferUsages::COPY_DST)
    .union(wgpu::BufferUsages::COPY_SRC);

struct TexGpu {
    _image: Rc<TextureImage>,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    used: u64,
}

/// The texture slots of a draw: the appearance texture units (at most maxTextures), then the
/// material textures, 2D and cube textures in their own slots. Returns the slot key, each unit's
/// binding (1..8 2D, 9..12 cube) and the material texture parameters (binding, FLIP_Y,
/// transform, coordinate) by kind; a texture beyond the slots binds nothing (white).
fn assign_slots(
    textures: &[Rc<TextureImage>],
    material: &[crate::traverse::MaterialTexture],
    projectors: &[Rc<TextureImage>],
) -> (
    SlotKey,
    [u32; MAX_TEXTURES],
    [[u32; 4]; MAT_KINDS],
    [u32; MAX_TEXTURES],
) {
    // Only units can be 3D textures (material and projector textures are 2D).
    let volume = textures.iter().take(MAX_TEXTURES).any(|t| t.volume);
    let mut key: SlotKey = ([None; SLOTS], volume);
    let mut bindings = [0; MAX_TEXTURES];
    let mut mat_tex = [[0; 4]; MAT_KINDS];
    let (mut n2d, mut ncube, mut n3d) = (0, 0, 0);
    let cube_slots = if volume {
        SLOTS_CUBE - SLOTS_3D
    } else {
        SLOTS_CUBE
    };
    let mut assign = |t: &Rc<TextureImage>| -> u32 {
        // A texture already in a slot binds that slot again.
        if let Some(i) = key.0.iter().position(|k| *k == Some(Rc::as_ptr(t))) {
            return i as u32 + 1;
        }

        if t.volume {
            if n3d == SLOTS_3D {
                return 0;
            }
            key.0[SLOTS_2D + cube_slots + n3d] = Some(Rc::as_ptr(t));
            n3d += 1;
            (SLOTS_2D + cube_slots + n3d) as u32
        } else if t.cube {
            if ncube == cube_slots {
                return 0;
            }
            key.0[SLOTS_2D + ncube] = Some(Rc::as_ptr(t));
            ncube += 1;
            (SLOTS_2D + ncube) as u32
        } else {
            if n2d == SLOTS_2D {
                return 0;
            }
            key.0[n2d] = Some(Rc::as_ptr(t));
            n2d += 1;
            n2d as u32
        }
    };

    for (i, t) in textures.iter().take(MAX_TEXTURES).enumerate() {
        bindings[i] = assign(t);
    }

    for m in material {
        let binding = assign(&m.texture);
        // An unbound material texture still counts (X_ITE's default texture is white).
        mat_tex[m.kind as usize] = [
            if binding == 0 { 13 } else { binding },
            m.texture.flip_y as u32,
            m.transform,
            m.coord,
        ];
    }

    // x3d_TextureProjectorTexture: the units after the shape's own (0: none left, white).
    let mut projector_bindings = [0; MAX_TEXTURES];
    for (i, t) in projectors.iter().take(MAX_TEXTURES).enumerate() {
        projector_bindings[i] = assign(t);
    }

    (key, bindings, mat_tex, projector_bindings)
}

/// The view dimension of slot `i` in the default or the volume layout.
fn slot_dimension(i: usize, volume: bool) -> wgpu::TextureViewDimension {
    if i < SLOTS_2D {
        wgpu::TextureViewDimension::D2
    } else if volume && i >= SLOTS - SLOTS_3D {
        wgpu::TextureViewDimension::D3
    } else {
        wgpu::TextureViewDimension::Cube
    }
}

/// x3d.wgsl for the volume layout: the last cube slots declared as 3D textures (sample_slot
/// samples them with the same stp coordinate).
fn volume_shader(src: &str) -> String {
    let mut src = src.to_string();
    for slot in ["tc2", "tc3"] {
        let a = format!("var {slot}: texture_cube<f32>;");
        assert_eq!(src.matches(&a).count(), 1, "volume shader anchor {a}");
        src = src.replace(&a, &format!("var {slot}: texture_3d<f32>;"));
    }
    src
}

/// The frame's EnvironmentLight specularTexture is a GeneratedCubeMapTexture drawn this frame.
fn env_source_rendered(frame: &Frame) -> bool {
    let rendered = |env: &Option<crate::traverse::EnvironmentParams>| {
        env.as_ref()
            .and_then(|e| e.specular.generated)
            .is_some_and(|k| frame.cube_passes.iter().any(|p| p.key == k))
    };

    rendered(&frame.environment) || frame.layers.iter().any(|l| rendered(&l.environment))
}

/// A rectangle (x, y, width, height, y up) as the integer wgpu viewport and scissor (y down) of a
/// `width` × `height` target, clipped to it; `None` when nothing of it is inside.
fn target_rect(r: &[f64; 4], width: u32, height: u32) -> Option<[u32; 4]> {
    let x0 = r[0].max(0.0);
    let x1 = (r[0] + r[2]).min(width as f64);
    let top = (height as f64 - (r[1] + r[3])).max(0.0);
    let bottom = (height as f64 - r[1]).min(height as f64);

    (x1 > x0 && bottom > top).then(|| {
        [
            x0 as u32,
            top as u32,
            (x1 - x0) as u32,
            (bottom - top) as u32,
        ]
    })
}

/// The transmission buffer: the texture, its view and sampler (LINEAR_MIPMAP_LINEAR,
/// CLAMP_TO_EDGE), its size.
struct TransmissionGpu {
    texture: wgpu::Texture,
    sampled: (wgpu::TextureView, wgpu::Sampler),
    width: u32,
    height: u32,
    used: u64,
}

/// gl.generateMipmap for the transmission buffer: one level from the one above.
const MIP_WGSL: &str = "
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_s: sampler;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    return vec4<f32>(p[i], 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(source)) / 2.0;
    return textureSampleLevel(source, source_s, position.xy / max(floor(size), vec2<f32>(1.0)), 0.0);
}
";

/// A RenderedTexture's texture: sampled through `view`; a pass draws into `scratch` (and the
/// depth map's `depth`) and copies it in (copyTexSubImage2D), so the pass may sample the texture.
struct RenGpu {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    scratch: wgpu::Texture,
    scratch_view: wgpu::TextureView,
    depth: Option<wgpu::TextureView>,
    width: u32,
    height: u32,
    float: bool,
    used: u64,
}

/// The texture format of a RenderedTexture (RGBA8, or a 16-bit float depth map).
/// A LUT PNG as the browser decodes it (8 bits per channel; 16-bit channels keep their high
/// byte, as Chromium's PNG decoder does).
fn lut_rgba8(png: &[u8]) -> image::RgbaImage {
    match image::load_from_memory(png).expect("LUT png") {
        image::DynamicImage::ImageLuma16(i) => {
            image::RgbaImage::from_fn(i.width(), i.height(), |x, y| {
                let v = (i.get_pixel(x, y)[0] >> 8) as u8;
                image::Rgba([v, v, v, 255])
            })
        }
        other => other.to_rgba8(),
    }
}

fn rendered_format(float: bool) -> wgpu::TextureFormat {
    if float {
        wgpu::TextureFormat::Rgba16Float
    } else {
        wgpu::TextureFormat::Rgba8Unorm
    }
}

/// A GeneratedCubeMapTexture's cube map: sampled through `view`, drawn face by face.
struct GenGpu {
    view: wgpu::TextureView,
    faces: Vec<wgpu::TextureView>,
    size: u32,
    used: u64,
}

/// The textures of one draw call by slot (2D slots, then cube slots, the last SLOTS_3D of them 3D
/// slots in the volume layout), and whether it is the volume layout; `None` is X_ITE's default
/// texture (1×1 white).
type SlotKey = ([Option<*const TextureImage>; SLOTS], bool);

/// Where a draw call's vertices live.
#[derive(Clone, Copy)]
enum Source {
    Scratch,
    Geometry(*const crate::geometry::Geometry),
    /// The points of a geometry as quads (PointProperties point sprites).
    Sprites(*const crate::geometry::Geometry),
    /// A volume of the frame (crate::volume_gpu::VolumeGpu), drawn without vertex buffer.
    Volume(usize),
    /// GaussianSplats of the frame (crate::splats_gpu::SplatGpu), drawn without vertex buffer.
    Splats(usize),
    /// A shape drawn with a ComposedShader's program (crate::shader_gpu::ShaderGpu): the input, and whether it is the back pass.
    Custom(usize, bool),
    /// Vertices a compute pass of the frame made (`Renderer::frame_buffers`).
    Buffer(usize),
}

struct Call {
    key: PipelineKey,
    /// gl.blendColor of a BlendMode.
    blend_constant: Option<[f64; 4]>,
    /// The hatch style or linetype texture (bind group 3).
    style_texture: Option<*const TextureImage>,
    source: Source,
    /// Byte range inside the scratch buffer (Source::Scratch only).
    range: (u64, u64),
    count: u32,
    /// gl.drawArraysInstanced: the instances of the draw.
    instances: u32,
    draw_index: usize,
    textures: SlotKey,
    /// A Viewport node's rectangle (DrawItem.viewport), `None` for the layer's.
    viewport: Option<[f64; 4]>,
}

/// How one pass of [`Renderer::encode_frame`] uses its target.
#[derive(Clone, Copy)]
struct PassTarget {
    /// A GeneratedCubeMapTexture face: its own depth buffer, no timestamps.
    face: bool,
    /// Keep what the previous layer drew (LayerSet: every layer after the first).
    load: bool,
    /// More layers follow: keep the multisampled colors, no timestamps.
    more: bool,
}

/// GPU time of the render pass, from timestamp queries (needs `Features::TIMESTAMP_QUERY` on
/// the host's device).
struct GpuTimer {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    period_ns: f32,
    /// The last frame's readback is mapped or being mapped.
    pending: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    last_ms: Option<f64>,
    recorded: bool,
}

/// Cache entries unused for this many frames are dropped.
const KEEP_FRAMES: u64 = 120;

/// The shadow map texture format (GL DEPTH_COMPONENT24; read back rounded to 24 bits).
const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// The shadow maps of a frame: one depth layer per light container, each map in the upper left
/// corner of its layer.
struct ShadowGpu {
    /// One depth texture per map, of the map's size (a depth copy takes the whole texture).
    maps: Vec<(u32, wgpu::Texture, wgpu::TextureView)>,
    /// The depth values the frame reads: each map's rows, `align_256 (size × 4)` bytes apart.
    buffer: wgpu::Buffer,
    used: u64,
}

/// One shadow map draw (shadow.wgsl ShadowDraw).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ShadowDrawU {
    projection: [f32; 16],
    model_view: [f32; 16],
    clip_planes: [[f32; 4]; MAX_CLIP_PLANES],
    /// x: the number of clip planes, y: 1 for an instanced draw, z: its first matrix in the shadow instances.
    counts: [u32; 4],
}

const SHADOW_STRIDE: u64 = 512;
const _: () = assert!(std::mem::size_of::<ShadowDrawU>() as u64 <= SHADOW_STRIDE);

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    shader: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    /// The volume slot layout: shader, slot bind group layout, pipeline layout.
    shader_3d: wgpu::ShaderModule,
    slots_layout_3d: wgpu::BindGroupLayout,
    layout_3d: wgpu::PipelineLayout,
    alt_shader_3d: Option<wgpu::ShaderModule>,
    draw_layout: wgpu::BindGroupLayout,
    /// Bind group 2: the texture slots.
    slots_layout: wgpu::BindGroupLayout,
    /// Bind group 3: the style texture.
    texture_layout: wgpu::BindGroupLayout,
    pipelines: HashMap<(PipelineKey, wgpu::TextureFormat, u32, bool), wgpu::RenderPipeline>,
    /// A main pipeline being created on another thread ([`Renderer::new_preparing`]).
    prepared: Option<PreparedPipeline>,
    frame_buffer: wgpu::Buffer,
    frame_group: wgpu::BindGroup,
    /// The frame group without an EnvironmentLight (default white cube maps).
    frame_group_default: wgpu::BindGroup,
    frame_layout: wgpu::BindGroupLayout,
    /// The default (white) environment cube maps.
    white_env: (wgpu::TextureView, wgpu::Sampler),
    /// The shadow maps of the last frame that had any, and a 1×1 stand-in.
    shadow: Option<ShadowGpu>,
    shadow_dummy: wgpu::Buffer,
    shadow_layout: wgpu::BindGroupLayout,
    /// Points, lines, triangles.
    shadow_pipelines: [wgpu::RenderPipeline; 3],
    /// The frame group's transmission buffer when a frame has no TransmissionMaterialExtension.
    no_transmission: (wgpu::TextureView, wgpu::Sampler),
    /// X3DX_ITEContext.getTransmissionBuffer: RGBA8 with mip levels, the size of the target.
    transmission: Option<TransmissionGpu>,
    /// encode_frame draws the transmission pass (RenderPass.TRANSMISSION_KEY).
    transmission_pass: bool,
    /// X3DX_ITEContext.getVolumeScatterBuffer: RGBA32F and depth, the size of the target.
    scatter: Option<crate::scatter_gpu::ScatterGpu>,
    /// encode_frame draws the volume scatter pass (RenderPass.VOLUME_SCATTER_KEY).
    scatter_pass: bool,
    /// The frame group's scatter buffers when no frame has a VolumeScatterMaterialExtension.
    scatter_dummy: (wgpu::Buffer, wgpu::Buffer),
    scatter_depth_copy: crate::scatter_gpu::ScatterDepthCopy,
    mip_layout: wgpu::BindGroupLayout,
    mip_pipeline: wgpu::RenderPipeline,
    /// RenderedTexture depthMap pipelines (shadow.wgsl fs_depth_color), points, lines, triangles.
    depth_color_pipelines: [wgpu::RenderPipeline; 3],
    rendered: HashMap<(u64, NodeId), RenGpu>,
    shadow_buffer: wgpu::Buffer,
    shadow_capacity: usize,
    /// The instance matrices of the shadow and depth color passes' instanced draws (shadow.wgsl `instances`), grown
    /// when a pass needs more.
    shadow_instances: wgpu::Buffer,
    shadow_instance_capacity: usize,
    shadow_group: wgpu::BindGroup,
    env: Option<EnvGpu>,
    /// The main frame's EnvironmentLight source was drawn by a texture pass this frame.
    env_rendered: bool,
    /// lut_ggx.png (AVG_PIXEL, CLAMP_TO_BOUNDARY).
    lut: (wgpu::TextureView, wgpu::Sampler),
    filter_pipeline: wgpu::RenderPipeline,
    filter_layout: wgpu::BindGroupLayout,
    filter_buffer: wgpu::Buffer,
    draw_buffer: wgpu::Buffer,
    /// The instances of the frame's instanced draws (draw group binding 1), grown when a frame needs more.
    instance_capacity: usize,
    instance_buffer: wgpu::Buffer,
    draw_capacity: usize,
    draw_group: wgpu::BindGroup,
    scratch: wgpu::Buffer,
    scratch_capacity: u64,
    /// The scratch vertex data of the last frame (background); unchanged data is not re-uploaded.
    scratch_data: Vec<f32>,
    geometries: HashMap<*const crate::geometry::Geometry, GeoGpu>,
    /// Point sprite quads per geometry (6 vertices per point).
    sprites: HashMap<*const crate::geometry::Geometry, GeoGpu>,
    free_vertices: Vec<FreeBuffer>,
    /// thick_lines.wgsl and the shadow and depth map pipelines that draw its triangles.
    thick_layout: wgpu::BindGroupLayout,
    thick_compute: wgpu::ComputePipeline,
    thick_pipeline: wgpu::RenderPipeline,
    thick: HashMap<*const x3d_runtime::particles::ThickLines, ThickGpu>,
    /// thick_lines.wgsl `display`.
    thick_display_layout: wgpu::BindGroupLayout,
    thick_display: wgpu::ComputePipeline,
    /// The buffers of this frame's compute passes ([`Source::Buffer`] and their inputs), free at the next frame.
    frame_buffers: Vec<wgpu::Buffer>,
    /// Uploaded textures (views and samplers), kept while used.
    textures: HashMap<*const TextureImage, TexGpu>,
    /// Bind groups of the slot combinations, kept while used.
    slot_groups: HashMap<SlotKey, (wgpu::BindGroup, u64)>,
    /// Style texture bind groups (group 3).
    style_groups: HashMap<*const TextureImage, (Rc<TextureImage>, wgpu::BindGroup, u64)>,
    /// VolumeRendering: programs, pipelines and bind groups of the volumes.
    volume: crate::volume_gpu::VolumeGpu,
    /// GaussianSplats: data buffers, sorted indices, pipelines and bind groups.
    splats: crate::splats_gpu::SplatGpu,
    custom: crate::shader_gpu::ShaderGpu,
    /// X_ITE's default textures: 1×1 white 2D and cube (X3DTexturingContext defaultTexture2D,
    /// defaultTextureCube).
    white_2d: (wgpu::TextureView, wgpu::Sampler),
    white_cube: (wgpu::TextureView, wgpu::Sampler),
    white_3d: (wgpu::TextureView, wgpu::Sampler),
    white_group: wgpu::BindGroup,
    depth: Option<(wgpu::Texture, wgpu::TextureView, u32, u32, u32)>,
    /// Depth buffer of the GeneratedCubeMapTexture face passes (TextureBuffer: size × size, no
    /// multisampling).
    face_depth: Option<(wgpu::Texture, wgpu::TextureView, u32)>,
    /// GeneratedCubeMapTexture cube maps by (runtime uid, node), kept between frames.
    generated: HashMap<(u64, NodeId), GenGpu>,
    /// Multisampled color target (MultiSampleFramebuffer colorBuffer): texture, view, format, size, samples.
    msaa_color: Option<(
        wgpu::Texture,
        wgpu::TextureView,
        wgpu::TextureFormat,
        u32,
        u32,
        u32,
    )>,
    /// Draw the scene mirrored vertically (and un-mirror `render_to_rgba`): GL framebuffers are
    /// bottom-up, so the hardware's MSAA sample pattern lands vertically mirrored relative to a
    /// top-down Vulkan one. Only the offscreen readback path can undo that; see
    /// [`Renderer::set_gl_sample_orientation`].
    gl_sample_orientation: bool,
    /// True while `render_to_rgba` draws mirrored.
    flip_now: bool,
    depth_mode: DepthMode,
    /// The Shading browser option ([`Renderer::set_shading`]).
    shading: ShadingMode,
    /// Debug: draw (draw index + 1, triangle index) into the RGBA target instead of shading; see
    /// [`Renderer::set_debug_ids`]. Never on in the oracle paths.
    debug_ids: bool,
    /// Shader for the non-default modes (`Logarithmic`, debug IDs), patched from `x3d.wgsl`.
    alt_shader: Option<wgpu::ShaderModule>,
    /// Debug IDs: per draw index, the geometry node and model-view matrix of the last `encode`.
    debug_draws: Vec<Option<(NodeId, Matrix4)>>,
    /// Requested sample count (X_ITE's `Multisampling` rendering property); 0 and 1 mean none.
    requested_samples: u32,
    /// Sample counts the adapter supports for the color target and Depth24Plus, ascending, 1 first.
    supported_samples: Vec<u32>,
    /// RGBA8 target and readback buffer of the offscreen paths (`render_to_rgba`).
    offscreen: Option<(wgpu::Texture, wgpu::TextureView, u32, u32)>,
    readback: Option<(wgpu::Buffer, u32, u32)>,
    frame_no: u64,
    timer: Option<GpuTimer>,
    /// Colour the target is cleared to; X_ITE clears to (0, 0, 0, 0) (drawShapes: gl.clearColor).
    pub clear_color: [f64; 4],
    /// Reused per-frame scratch memory.
    draw_bytes: Vec<u8>,
    calls: Vec<Call>,
    /// Frame statistics for the last `encode`: draw calls, geometry uploads, texture uploads.
    pub stats: RenderStats,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct RenderStats {
    /// The draw calls of the color passes.
    pub draw_calls: usize,
    /// The draw calls of the shadow map passes (every light, view and instance).
    pub shadow_draw_calls: usize,
    /// The draw calls of the depth color pass ([`Renderer::encode_depth_color`]).
    pub depth_draw_calls: usize,
    pub geometry_uploads: usize,
    /// Vertex buffers created (the other geometry uploads rewrote a free one).
    pub vertex_buffers_created: usize,
    pub texture_uploads: usize,
    pub pipelines_created: usize,
}

impl Renderer {
    /// The device features this renderer can use if the adapter offers them (GPU timing).
    pub fn wanted_features(adapter: &wgpu::Adapter) -> wgpu::Features {
        adapter.features()
            & (wgpu::Features::TIMESTAMP_QUERY
                | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
                | wgpu::Features::POLYGON_MODE_LINE
                | wgpu::Features::FLOAT32_BLENDABLE)
    }

    /// The sample counts (ascending, starting with 1) usable with `color_format` and the depth
    /// format on this device. WebGPU guarantees 1 and 4; 2, 8 and 16 need the device to have been
    /// created with `TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES` (see [`Renderer::wanted_features`])
    /// and the adapter to report them for both formats.
    pub fn supported_sample_counts(
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
    ) -> Vec<u32> {
        let adapter_specific = device
            .features()
            .contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);
        let color = adapter.get_texture_format_features(color_format).flags;
        let depth = adapter.get_texture_format_features(DEPTH_FORMAT).flags;
        [1u32, 2, 4, 8, 16]
            .into_iter()
            .filter(|&n| {
                n == 1
                    || (n == 4
                        && color.sample_count_supported(4)
                        && depth.sample_count_supported(4))
                    || (adapter_specific
                        && n != 4
                        && color.sample_count_supported(n)
                        && depth.sample_count_supported(n))
            })
            .collect()
    }

    /// Declare which sample counts the device supports (see [`Renderer::supported_sample_counts`]).
    /// The default is `[1, 4]`, the WebGPU baseline.
    pub fn set_supported_samples(&mut self, supported: Vec<u32>) {
        self.supported_samples = if supported.is_empty() {
            vec![1]
        } else {
            supported
        };
    }

    /// Match the sample positions of the GL oracle in `render_to_rgba` (oracle comparisons only;
    /// the picture is identical up to which samples cover an edge pixel). The windowed path
    /// ignores it.
    pub fn set_gl_sample_orientation(&mut self, on: bool) {
        if self.gl_sample_orientation != on {
            self.gl_sample_orientation = on;
        }
    }

    pub fn depth_mode(&self) -> DepthMode {
        self.depth_mode
    }

    /// Choose the depth representation (see [`DepthMode`]). Also switches the process-wide
    /// `LogarithmicDepthBuffer` property (far plane 1e10), so call it before the next frame is
    /// built.
    /// The Shading browser option: the RenderingProperties Shading.
    pub fn set_shading(&mut self, shading: ShadingMode) {
        self.shading = shading;
    }

    pub fn shading(&self) -> ShadingMode {
        self.shading
    }

    pub fn set_depth_mode(&mut self, mode: DepthMode) {
        if self.depth_mode != mode {
            self.depth_mode = mode;
            crate::traverse::set_logarithmic_depth_buffer(mode == DepthMode::Logarithmic);
            self.rebuild_shader_variant();
        }
    }

    /// Debug: render object identity instead of shading. Each pixel of `render_to_rgba` is
    /// `(draw index + 1) & 255, (draw index + 1) >> 8, triangle & 255, triangle >> 8` (RGBA);
    /// background faces count as draws; no antialiasing or blending. Off by default and unused by
    /// every oracle path. The mapping from draw index to node is [`Renderer::debug_draws`].
    pub fn set_debug_ids(&mut self, on: bool) {
        if self.debug_ids != on {
            self.debug_ids = on;
            self.rebuild_shader_variant();
        }
    }

    /// Debug IDs: for each draw index of the last `encode`, the geometry node and model-view matrix
    /// (`None` for background faces).
    pub fn debug_draws(&self) -> &[Option<(NodeId, Matrix4)>] {
        &self.debug_draws
    }

    fn rebuild_shader_variant(&mut self) {
        self.pipelines.clear();
        // Made with the previous variant; its thread finishes on its own.
        self.prepared = None;
        self.alt_shader = None;
        self.alt_shader_3d = None;

        if self.depth_mode == DepthMode::Standard && !self.debug_ids {
            return;
        }

        let mut src = include_str!("x3d.wgsl").to_string();
        fn patch(src: &mut String, a: &str, b: &str) {
            assert_eq!(src.matches(a).count(), 1, "shader patch anchor {a:?}");
            *src = src.replacen(a, b, 1);
        }

        if self.depth_mode == DepthMode::Logarithmic {
            // Logarithmic2.js, called last in Vertex2.glsl.js main () and Fragment2.glsl.js main ().
            patch(
                &mut src,
                "struct VOut {\n",
                "struct VOut {\n    @location(4) log_depth: f32,\n",
            );
            patch(
                &mut src,
                "    p.z = 0.5 * (p.z + p.w);\n    out.position = p;",
                "    p.z = 0.5 * (p.z + p.w);\n    out.position = p;\n    out.log_depth = 1.0 + p.w;",
            );
            patch(
                &mut src,
                "@fragment\nfn fs_main(in: VOut, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {",
                "struct FOut {\n    @location(0) color: vec4<f32>,\n    @builtin(frag_depth) depth: f32,\n};\n\n@fragment\nfn fs_main(in: VOut, @builtin(front_facing) front_facing: bool) -> FOut {",
            );
            let i = src
                .rfind("    return final_color;\n}")
                .expect("fs_main return");
            src.replace_range(i..i + "    return final_color;\n}".len(), "    // x3d_LogarithmicFarFactor1_2 travels in fog_range.z.\n    return FOut(final_color, log2(in.log_depth) * frame.fog_range.z);\n}");
        }

        if self.depth_mode == DepthMode::Reversed {
            // The projection already yields WebGPU clip z (see `reverse_depth`).
            patch(&mut src, "    p.z = 0.5 * (p.z + p.w);\n", "");
        }

        if self.debug_ids {
            patch(
                &mut src,
                "struct VOut {\n",
                "struct VOut {\n    @location(5) @interpolate(flat) tri: u32,\n",
            );
            patch(
                &mut src,
                "fn vs_main(in: VIn) -> VOut {\n    return vertex_common(in);\n}",
                "fn vs_main(in: VIn, @builtin(vertex_index) vi: u32) -> VOut {\n    var out = vertex_common(in);\n    out.tri = vi / 3u;\n    return out;\n}",
            );
            let color = "vec4<f32>(f32(id & 255u), f32((id >> 8u) & 255u), f32(in.tri & 255u), f32((in.tri >> 8u) & 255u)) / 255.0";

            if self.depth_mode == DepthMode::Logarithmic {
                src.push_str(&format!("\n@fragment\nfn fs_id(in: VOut) -> FOut {{\n    let id = u32(draw.diffuse_ambient.x);\n    return FOut({color}, log2(in.log_depth) * frame.fog_range.z);\n}}\n"));
            } else {
                src.push_str(&format!("\n@fragment\nfn fs_id(in: VOut) -> @location(0) vec4<f32> {{\n    let id = u32(draw.diffuse_ambient.x);\n    return {color};\n}}\n"));
            }
        }

        self.alt_shader_3d = Some(
            self.device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("x3d.wgsl variant (3D)"),
                    source: wgpu::ShaderSource::Wgsl(volume_shader(&src).into()),
                }),
        );
        self.alt_shader = Some(
            self.device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("x3d.wgsl variant"),
                    source: wgpu::ShaderSource::Wgsl(src.into()),
                }),
        );
    }

    /// Request a sample count: X_ITE's `Multisampling` rendering property (BrowserOptions
    /// set_Multisampling__). 0 or 1 draws without multisampling.
    pub fn set_sample_count(&mut self, samples: u32) {
        self.requested_samples = samples;
    }

    pub fn requested_sample_count(&self) -> u32 {
        self.requested_samples
    }

    /// The count actually used: the highest supported count not above the request, like X_ITE
    /// clamps Multisampling to `getMaxSamples ()`.
    pub fn sample_count(&self) -> u32 {
        self.supported_samples
            .iter()
            .copied()
            .filter(|&n| n <= self.requested_samples.max(1))
            .max()
            .unwrap_or(1)
    }

    pub fn supported_samples(&self) -> &[u32] {
        &self.supported_samples
    }

    /// Step the request to the next supported count (1, 2, 4, 8, 16 as supported), wrapping.
    pub fn cycle_sample_count(&mut self) -> u32 {
        let cur = self.sample_count();
        let next = self
            .supported_samples
            .iter()
            .copied()
            .find(|&n| n > cur)
            .unwrap_or(self.supported_samples[0]);
        self.requested_samples = next;
        next
    }

    /// A renderer on the host's device and queue. Cheap resources only; nothing is drawn.
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Renderer {
        Self::with_prepared(device, queue, None)
    }

    /// [`Renderer::new`], and start creating the main pipeline of the most common draw (opaque
    /// solid triangles) for a target of `format` and `samples` (the window's surface) on another
    /// thread. The first main pipeline compiles the whole x3d.wgsl; on DX12 that is seconds (DXC,
    /// then the driver), which this overlaps with the rest of the setup and the world's loading.
    /// The first frame that draws with it waits for it; other draws do not.
    pub fn new_preparing(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        samples: u32,
    ) -> Renderer {
        Self::with_prepared(device, queue, Some((format, samples)))
    }

    fn with_prepared(
        device: wgpu::Device,
        queue: wgpu::Queue,
        target: Option<(wgpu::TextureFormat, u32)>,
    ) -> Renderer {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("x3d.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("x3d.wgsl").into()),
        });
        let shader_3d = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("x3d.wgsl (3D)"),
            source: wgpu::ShaderSource::Wgsl(volume_shader(include_str!("x3d.wgsl")).into()),
        });

        let uniform_entry = |dynamic: bool, size: u64| wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: dynamic,
                min_binding_size: wgpu::BufferSize::new(size),
            },
            count: None,
        };

        let texture_entry =
            |binding: u32, view_dimension: wgpu::TextureViewDimension| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension,
                    multisampled: false,
                },
                count: None,
            };
        let sampler_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        // 1..6: IBL2.glsl diffuse, specular and sheen cube maps (one cube array), the
        // transmission buffer, lut_ggx.png, lut_charlie.png and lut_sheen_E.png (one array).
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame"),
            entries: &[
                uniform_entry(false, std::mem::size_of::<FrameU>() as u64),
                texture_entry(1, wgpu::TextureViewDimension::CubeArray),
                sampler_entry(2),
                texture_entry(3, wgpu::TextureViewDimension::D2),
                sampler_entry(4),
                texture_entry(5, wgpu::TextureViewDimension::D2Array),
                sampler_entry(6),
                // 7: the shadow maps (Shadow2.glsl x3d_ShadowMap): their depth values, copied from
                // the depth textures (a buffer, as the fragment stage has no texture binding left).
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 8, 9: the volume scatter buffer (VolumeScatterMaterialExtension): RGBA32F values and window depth.
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let draw_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("draw"),
            entries: &[
                uniform_entry(true, std::mem::size_of::<DrawU>() as u64),
                // 1: the instances of the frame's instanced draws (x3d.wgsl Instance).
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let slot_entries = |volume: bool| {
            let mut slot_entries = vec![];
            for i in 0..SLOTS as u32 {
                let view_dimension = slot_dimension(i as usize, volume);
                slot_entries.push(wgpu::BindGroupLayoutEntry {
                    binding: 2 * i,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension,
                        multisampled: false,
                    },
                    count: None,
                });
                slot_entries.push(wgpu::BindGroupLayoutEntry {
                    binding: 2 * i + 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                });
            }
            slot_entries
        };
        let slots_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture slots"),
            entries: &slot_entries(false),
        });
        let slots_layout_3d = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture slots (3D)"),
            entries: &slot_entries(true),
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("x3d"),
            // 3: the style texture (Hatch2.glsl, Stipple2.glsl).
            bind_group_layouts: &[
                Some(&frame_layout),
                Some(&draw_layout),
                Some(&slots_layout),
                Some(&texture_layout),
            ],
            ..Default::default()
        });
        let layout_3d = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("x3d (3D)"),
            bind_group_layouts: &[
                Some(&frame_layout),
                Some(&draw_layout),
                Some(&slots_layout_3d),
                Some(&texture_layout),
            ],
            ..Default::default()
        });
        let prepared = target.and_then(|(format, samples)| {
            let source = PipelineSource {
                device: device.clone(),
                shader: shader.clone(),
                shader_3d: shader_3d.clone(),
                layout: layout.clone(),
                layout_3d: layout_3d.clone(),
                depth_mode: DepthMode::Standard,
                debug_ids: false,
            };
            PreparedPipeline::start(
                source,
                (
                    PipelineKey::new(Topology::Triangles, Cull::Back, true, false, true, true),
                    format,
                    samples,
                    false,
                ),
            )
        });

        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame"),
            size: std::mem::size_of::<FrameU>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let white_cube_image = {
            let mut c = TextureImage::white();
            c.cube = true;
            c.rgba = vec![255; 24];
            c
        };
        let _ = white_cube_image;
        let white_env = Self::white_env_maps(&device, &queue);
        let lut = Self::upload_lut(&device, &queue);
        let no_transmission = Self::white_transmission(&device, &queue);
        let shadow_dummy = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("no shadow maps"),
            size: 16,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let scatter_dummy = (
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("no volume scatter colors"),
                size: 16,
                usage: wgpu::BufferUsages::STORAGE,
                mapped_at_creation: false,
            }),
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("no volume scatter depth"),
                size: 16,
                usage: wgpu::BufferUsages::STORAGE,
                mapped_at_creation: false,
            }),
        );
        let scatter_depth_copy = crate::scatter_gpu::ScatterDepthCopy::new(&device);
        let frame_group = Self::make_frame_group(
            &device,
            &frame_layout,
            &frame_buffer,
            &white_env,
            &no_transmission,
            &lut,
            &shadow_dummy,
            &scatter_dummy,
        );

        // TextureBuffer mipMaps: gl.generateMipmap of the transmission buffer (each level the
        // linear filtered middle of four texels of the one above).
        let mip_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mipmap"),
            source: wgpu::ShaderSource::Wgsl(MIP_WGSL.into()),
        });
        let mip_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mipmap"),
            entries: &[
                texture_entry(0, wgpu::TextureViewDimension::D2),
                sampler_entry(1),
            ],
        });
        let mip_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mipmap"),
            bind_group_layouts: &[Some(&mip_layout)],
            ..Default::default()
        });
        let mip_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mipmap"),
            layout: Some(&mip_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &mip_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &mip_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let frame_group_default = frame_group.clone();

        // X3DRenderObject.depth for the shadow maps (shadow.wgsl): depth only, no culling.
        let shadow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shadow.wgsl").into()),
        });
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow draw"),
            entries: &[
                uniform_entry(true, std::mem::size_of::<ShadowDrawU>() as u64),
                // 1: the instance matrices (shadow.wgsl `instances`).
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow"),
                bind_group_layouts: &[Some(&shadow_layout)],
                ..Default::default()
            });
        let shadow_pipelines = [
            wgpu::PrimitiveTopology::PointList,
            wgpu::PrimitiveTopology::LineList,
            wgpu::PrimitiveTopology::TriangleList,
        ]
        .map(|topology| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("shadow"),
                layout: Some(&shadow_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shadow_shader,
                    entry_point: Some("vs_shadow"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: (FLOATS_PER_VERTEX * 4) as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: SHADOW_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shadow_shader,
                    entry_point: Some("fs_shadow"),
                    compilation_options: Default::default(),
                    targets: &[],
                }),
                multiview_mask: None,
                cache: None,
            })
        });
        let depth_color_pipelines = [
            wgpu::PrimitiveTopology::PointList,
            wgpu::PrimitiveTopology::LineList,
            wgpu::PrimitiveTopology::TriangleList,
        ]
        .map(|topology| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("depth map"),
                layout: Some(&shadow_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shadow_shader,
                    entry_point: Some("vs_shadow"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: (FLOATS_PER_VERTEX * 4) as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: SHADOW_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shadow_shader,
                    entry_point: Some("fs_depth_color"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: rendered_format(true),
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        });
        // The LINE particle triangles of thick_lines.wgsl, drawn like the shadow shapes (16 byte vertices).
        let thick_vertex = [Some(wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3],
        })];
        let thick_pipeline = {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("thick line shadow"),
                layout: Some(&shadow_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shadow_shader,
                    entry_point: Some("vs_shadow"),
                    compilation_options: Default::default(),
                    buffers: &thick_vertex,
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: SHADOW_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shadow_shader,
                    entry_point: Some("fs_shadow"),
                    compilation_options: Default::default(),
                    targets: &[],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let storage_entry = |binding: u32, read_only: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let thick_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("thick lines"),
            entries: &[storage_entry(0, true), storage_entry(1, false)],
        });
        let thick_display_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("thick line display"),
                entries: &[
                    storage_entry(0, true),
                    storage_entry(2, false),
                    storage_entry(3, true),
                ],
            });
        let thick_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("thick_lines.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("thick_lines.wgsl").into()),
        });
        let [thick_compute, thick_display] =
            [("main", &thick_layout), ("display", &thick_display_layout)].map(|(entry, layout)| {
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("thick lines"),
                    layout: Some(
                        &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                            label: Some("thick lines"),
                            bind_group_layouts: &[Some(layout)],
                            ..Default::default()
                        }),
                    ),
                    module: &thick_module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    cache: None,
                })
            });
        let shadow_instances = Self::make_shadow_instances(&device, 64);
        let (shadow_buffer, shadow_group) =
            Self::make_shadow_buffer(&device, &shadow_layout, 64, &shadow_instances);

        // The environment filter (filter.wgsl): RGBA8 targets, no depth, no blending.
        let filter_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("filter.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("filter.wgsl").into()),
        });
        let filter_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("filter"),
            entries: &[
                uniform_entry(false, std::mem::size_of::<FilterU>() as u64),
                texture_entry(1, wgpu::TextureViewDimension::Cube),
                sampler_entry(2),
            ],
        });
        let filter_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("filter"),
                bind_group_layouts: &[Some(&filter_layout)],
                ..Default::default()
            });
        let filter_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("filter"),
            layout: Some(&filter_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &filter_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &filter_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let filter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("filter"),
            size: std::mem::size_of::<FilterU>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let draw_capacity = 64;
        let instance_capacity = 64;
        let instance_buffer = Self::make_instance_buffer(&device, instance_capacity);
        let (draw_buffer, draw_group) =
            Self::make_draw_buffer(&device, &draw_layout, draw_capacity, &instance_buffer);
        let scratch_capacity = 64 * 1024;
        let scratch = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scratch"),
            size: scratch_capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let white = TextureImage::white();
        let mut white_cube = TextureImage::white();
        white_cube.cube = true;
        white_cube.rgba = vec![255; 24];

        let mut r = Renderer {
            white_group: Self::make_texture_group(&device, &queue, &texture_layout, &white),
            white_2d: Self::upload_texture(&device, &queue, &white),
            white_cube: Self::upload_texture(&device, &queue, &white_cube),
            white_3d: Self::upload_texture(&device, &queue, &crate::texture::white_3d()),
            slot_groups: HashMap::new(),
            style_groups: HashMap::new(),
            volume: Default::default(),
            splats: Default::default(),
            custom: Default::default(),
            device,
            queue,
            shader,
            layout,
            shader_3d,
            slots_layout_3d,
            layout_3d,
            alt_shader_3d: None,
            draw_layout,
            slots_layout,
            texture_layout,
            pipelines: HashMap::new(),
            prepared,
            frame_buffer,
            frame_group,
            frame_group_default,
            frame_layout,
            white_env,
            shadow: None,
            shadow_dummy,
            shadow_layout,
            shadow_pipelines,
            depth_color_pipelines,
            no_transmission,
            transmission: None,
            transmission_pass: false,
            scatter: None,
            scatter_pass: false,
            scatter_dummy,
            scatter_depth_copy,
            mip_layout,
            mip_pipeline,
            rendered: HashMap::new(),
            shadow_buffer,
            shadow_capacity: 64,
            shadow_instances,
            shadow_instance_capacity: 64,
            shadow_group,
            env: None,
            env_rendered: false,
            lut,
            filter_pipeline,
            filter_layout,
            filter_buffer,
            draw_buffer,
            instance_capacity,
            instance_buffer,
            draw_capacity,
            draw_group,
            scratch,
            scratch_capacity,
            scratch_data: vec![],
            geometries: HashMap::new(),
            sprites: HashMap::new(),
            free_vertices: vec![],
            thick_layout,
            thick_compute,
            thick_pipeline,
            thick: HashMap::new(),
            thick_display_layout,
            thick_display,
            frame_buffers: vec![],
            textures: HashMap::new(),
            depth: None,
            face_depth: None,
            generated: HashMap::new(),
            msaa_color: None,
            gl_sample_orientation: false,
            flip_now: false,
            depth_mode: DepthMode::Standard,
            shading: ShadingMode::Gouraud,
            debug_ids: false,
            alt_shader: None,
            debug_draws: vec![],
            requested_samples: 1,
            supported_samples: vec![1, 4],
            offscreen: None,
            readback: None,
            frame_no: 0,
            timer: None,
            clear_color: [0.0; 4],
            draw_bytes: vec![],
            calls: vec![],
            stats: RenderStats::default(),
        };
        r.timer = GpuTimer::new(&r.device, &r.queue);
        r
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Whether GPU timing is available (the device was created with `TIMESTAMP_QUERY`).
    pub fn gpu_timing_available(&self) -> bool {
        self.timer.is_some()
    }

    /// GPU milliseconds of the most recent finished render pass. With `block`, waits for the
    /// frame just submitted; otherwise returns the latest value that is already available.
    pub fn gpu_time_ms(&mut self, block: bool) -> Option<f64> {
        let timer = self.timer.as_mut()?;

        if timer.pending.is_none() && !timer.recorded {
            return timer.last_ms;
        }

        if block {
            let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        } else {
            let _ = self.device.poll(wgpu::PollType::Poll);
        }

        if let Some(flag) = &timer.pending {
            if flag.load(std::sync::atomic::Ordering::Acquire) {
                {
                    let data = timer.read.slice(..).get_mapped_range().expect("timestamps");
                    let t: &[u64] = bytemuck::cast_slice(&data);
                    timer.last_ms =
                        Some(t[1].saturating_sub(t[0]) as f64 * timer.period_ns as f64 / 1e6);
                }
                timer.read.unmap();
                timer.pending = None;
            }
        }

        timer.last_ms
    }

    fn make_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instances"),
            size: (capacity * std::mem::size_of::<InstanceU>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn make_draw_buffer(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        capacity: usize,
        instances: &wgpu::Buffer,
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("draws"),
            size: capacity as u64 * DRAW_STRIDE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(std::mem::size_of::<DrawU>() as u64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: instances.as_entire_binding(),
                },
            ],
        });
        (buffer, group)
    }

    fn make_pipeline(
        &self,
        key: PipelineKey,
        format: wgpu::TextureFormat,
        samples: u32,
        flip: bool,
    ) -> wgpu::RenderPipeline {
        self.pipeline_source().make(key, format, samples, flip)
    }

    /// Wait for the pipeline [`Renderer::new_preparing`] started and keep it. When the sample
    /// count has changed since, it does not wait: frames at this count cannot use it. A host that
    /// shows its window only when the first frame can be drawn at once calls this before.
    pub fn finish_prepared(&mut self) {
        let samples = self.sample_count();
        let Some(p) = self.prepared.take_if(|p| p.key.2 == samples) else {
            return;
        };
        let key = p.key;

        if let Some(pipeline) = p.finish() {
            self.pipelines.entry(key).or_insert(pipeline);
        }
    }

    /// The main pipeline of `key` for this frame's target: the prepared one when it is that, else a new one.
    fn take_or_make_pipeline(
        &mut self,
        key: (PipelineKey, wgpu::TextureFormat, u32, bool),
    ) -> wgpu::RenderPipeline {
        if self.prepared.as_ref().is_some_and(|p| p.key == key) {
            if let Some(p) = self.prepared.take().and_then(PreparedPipeline::finish) {
                return p;
            }
        }

        self.make_pipeline(key.0, key.1, key.2, key.3)
    }

    /// What [`PipelineSource::make`] needs, with the current shader variant and options.
    fn pipeline_source(&self) -> PipelineSource {
        PipelineSource {
            device: self.device.clone(),
            shader: self
                .alt_shader
                .clone()
                .unwrap_or_else(|| self.shader.clone()),
            shader_3d: self
                .alt_shader_3d
                .clone()
                .unwrap_or_else(|| self.shader_3d.clone()),
            layout: self.layout.clone(),
            layout_3d: self.layout_3d.clone(),
            depth_mode: self.depth_mode,
            debug_ids: self.debug_ids,
        }
    }

    /// A shadow map depth texture of `size`² and its view.
    fn shadow_texture(device: &wgpu::Device, size: u32) -> (u32, wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow map"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_FORMAT,
            usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        (size, texture, view)
    }

    fn make_shadow_instances(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow instances"),
            size: (capacity * 64) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn make_shadow_buffer(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        capacity: usize,
        instances: &wgpu::Buffer,
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow draws"),
            size: capacity as u64 * SHADOW_STRIDE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow draws"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(std::mem::size_of::<ShadowDrawU>() as u64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: instances.as_entire_binding(),
                },
            ],
        });
        (buffer, group)
    }

    /// The uniforms of a shadow or depth color pass, one per draw, and the instance matrices of its instanced draws
    /// into their buffers, grown when the pass needs more. `particle_draws` and `particle_instances` of them are
    /// ParticleSystem data (crate::transfer).
    fn upload_shadow_draws(
        &mut self,
        draws: &[ShadowDrawU],
        instances: &[[f32; 16]],
        particle_draws: usize,
        particle_instances: usize,
    ) {
        let more_instances = instances.len() > self.shadow_instance_capacity;

        if more_instances {
            self.shadow_instance_capacity = instances.len().next_power_of_two();
            self.shadow_instances =
                Self::make_shadow_instances(&self.device, self.shadow_instance_capacity);
        }

        if draws.len() > self.shadow_capacity || more_instances {
            self.shadow_capacity = self.shadow_capacity.max(draws.len().next_power_of_two());
            (self.shadow_buffer, self.shadow_group) = Self::make_shadow_buffer(
                &self.device,
                &self.shadow_layout,
                self.shadow_capacity,
                &self.shadow_instances,
            );
        }

        let mut bytes = vec![0u8; draws.len().max(1) * SHADOW_STRIDE as usize];
        for (i, d) in draws.iter().enumerate() {
            let b = bytemuck::bytes_of(d);
            bytes[i * SHADOW_STRIDE as usize..i * SHADOW_STRIDE as usize + b.len()]
                .copy_from_slice(b);
        }
        self.queue.write_buffer(&self.shadow_buffer, 0, &bytes);
        crate::transfer::split_upload(bytes.len(), particle_draws * SHADOW_STRIDE as usize);

        if !instances.is_empty() {
            let bytes: &[u8] = bytemuck::cast_slice(instances);

            self.queue.write_buffer(&self.shadow_instances, 0, bytes);
            crate::transfer::split_upload(bytes.len(), particle_instances * 64);
        }
    }

    /// The light containers' renderShadowMap: each shadow map cleared to depth 1, then the
    /// shadow-casting shapes of each view (X3DRenderObject.depth, no culling) in its viewport.
    /// Submitted on its own; returns the buffer the frame reads and each map's (first value, row
    /// stride) in it.
    /// thick_lines.wgsl: the triangles of every [`crate::traverse::DepthItem::thick_lines`] of `items` this frame has not
    /// made yet, submitted before the passes that draw them.
    fn make_thick_lines<'a>(
        &mut self,
        items: impl Iterator<Item = &'a crate::traverse::DepthItem>,
    ) {
        let mut encoder: Option<wgpu::CommandEncoder> = None;

        for lines in items.filter_map(|i| i.thick_lines.as_ref()) {
            let key = Rc::as_ptr(lines);

            if self.thick.contains_key(&key) {
                continue;
            }

            let (segments, instances) = (lines.vertices.len() / 8, lines.instances.len());
            let count = segments * instances * 6;
            let input = thick_input(lines, false);
            let bytes: &[u8] = bytemuck::cast_slice(&input);
            let input_buffer = pooled_buffer(
                &self.device,
                &mut self.free_vertices,
                &mut self.stats,
                bytes.len() as u64,
                THICK_USAGE,
            );
            let triangles = pooled_buffer(
                &self.device,
                &mut self.free_vertices,
                &mut self.stats,
                (count as u64 * 16).max(16),
                THICK_USAGE,
            );
            self.queue.write_buffer(&input_buffer, 0, bytes);
            crate::transfer::upload(crate::transfer::Kind::Particle, bytes.len());

            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("thick lines"),
                layout: &self.thick_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: input_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: triangles.as_entire_binding(),
                    },
                ],
            });
            let encoder = encoder.get_or_insert_with(|| {
                self.device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("thick lines"),
                    })
            });
            let mut pass = encoder.begin_compute_pass(&Default::default());
            let groups = (segments * instances).div_ceil(64) as u32;
            pass.set_pipeline(&self.thick_compute);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(groups.min(65535), groups.div_ceil(65535), 1);
            drop(pass);

            self.thick.insert(
                key,
                ThickGpu {
                    lines: lines.clone(),
                    input: input_buffer,
                    triangles,
                    count: count as u32,
                },
            );
        }

        if let Some(encoder) = encoder {
            self.queue.submit([encoder.finish()]);
        }
    }

    /// displayInstancedThick on the GPU (thick_lines.wgsl `display`, recorded into `encoder`): the vertices of every
    /// instance of `g`, one after the other, in a buffer of [`Self::frame_buffers`]. Returns its index and the vertices
    /// per instance.
    #[allow(clippy::too_many_arguments)]
    fn thick_display_vertices(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        g: &crate::geometry::Geometry,
        model_view: &Matrix4,
        projection: &Matrix4,
        viewport: [f64; 4],
        l: &LineParams,
        matrices: &[Matrix4],
    ) -> (usize, u32) {
        let lines = thick_lines(
            g,
            model_view,
            projection,
            viewport,
            l.width,
            matrices.iter().map(f32x16).collect(),
        );
        let segments = g.vertex_count() / 2;
        let input = thick_input(&lines, l.linetype != 1);
        let mut line = geometry_vertices(g);
        line.truncate(segments * 2 * FLOATS_PER_VERTEX);
        line.resize(line.len().max(4), 0.0);

        let mut buffer = |size: u64| {
            pooled_buffer(
                &self.device,
                &mut self.free_vertices,
                &mut self.stats,
                size.max(16),
                THICK_USAGE,
            )
        };
        let input_buffer = buffer(input.len() as u64 * 4);
        let line_buffer = buffer(line.len() as u64 * 4);
        let vertices =
            buffer((segments * 6 * matrices.len() * STIPPLE_FLOATS_PER_VERTEX) as u64 * 4);
        self.queue
            .write_buffer(&input_buffer, 0, bytemuck::cast_slice(&input));
        self.queue
            .write_buffer(&line_buffer, 0, bytemuck::cast_slice(&line));
        crate::transfer::upload(
            crate::transfer::Kind::Particle,
            (input.len() + line.len()) * 4,
        );

        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("thick line display"),
            layout: &self.thick_display_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: vertices.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: line_buffer.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_compute_pass(&Default::default());
        let groups = (segments * matrices.len()).div_ceil(64) as u32;
        pass.set_pipeline(&self.thick_display);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(groups.min(65535), groups.div_ceil(65535), 1);
        drop(pass);

        self.frame_buffers.extend([input_buffer, line_buffer]);
        self.frame_buffers.push(vertices);
        (self.frame_buffers.len() - 1, (segments * 6) as u32)
    }

    fn render_shadow_maps(&mut self, frame: &Frame) -> (wgpu::Buffer, Vec<(u32, u32)>) {
        let frame_no = self.frame_no;
        // Each map's rows in the buffer (copy_texture_to_buffer: rows 256 byte aligned).
        let mut places = vec![];
        let mut bytes_total = 0u64;
        for m in &frame.shadow_maps {
            let row = (m.size * 4).div_ceil(256) * 256;
            places.push(((bytes_total / 4) as u32, row / 4));
            bytes_total += row as u64 * m.size as u64;
        }

        if self
            .shadow
            .as_ref()
            .is_none_or(|s| s.buffer.size() < bytes_total)
        {
            let bytes = self
                .shadow
                .as_ref()
                .map_or(bytes_total, |s| s.buffer.size().max(bytes_total))
                .max(16);
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("shadow map values"),
                size: bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let maps = self.shadow.take().map(|s| s.maps).unwrap_or_default();
            self.shadow = Some(ShadowGpu {
                maps,
                buffer,
                used: frame_no,
            });
        }

        // The textures: kept while the maps keep their sizes, at most one per map.
        {
            let device = &self.device;
            let shadow = self.shadow.as_mut().unwrap();
            shadow.maps.truncate(frame.shadow_maps.len());
            for (i, m) in frame.shadow_maps.iter().enumerate() {
                match shadow.maps.get(i) {
                    Some(t) if t.0 == m.size => {}
                    Some(_) => shadow.maps[i] = Self::shadow_texture(device, m.size),
                    None => shadow.maps.push(Self::shadow_texture(device, m.size)),
                }
            }
        }

        // Uniforms, one per depth draw; an instanced draw's matrices in the shadow instances.
        let mut draws: Vec<ShadowDrawU> = vec![];
        let mut instances: Vec<[f32; 16]> = vec![];
        let (mut particle_draws, mut particle_instances) = (0usize, 0usize);

        for map in &frame.shadow_maps {
            for view in &map.views {
                for item in &view.items {
                    let mut d: ShadowDrawU = bytemuck::Zeroable::zeroed();
                    d.projection = f32x16(&view.projection);
                    d.model_view = f32x16(&item.model_view);
                    for (k, p) in item.clip_planes.iter().take(MAX_CLIP_PLANES).enumerate() {
                        d.clip_planes[k] = p.map(|v| v as f32);
                    }
                    d.counts[0] = item.clip_planes.len().min(MAX_CLIP_PLANES) as u32;

                    // InstancedShape, ParticleSystem: one instanced draw (Depth2.vs.js getInstanceVertex).
                    let particle = item
                        .instances
                        .as_ref()
                        .map_or(item.thick_lines.is_some(), |i| i.particle);
                    particle_draws += particle as usize;

                    if let Some(i) = &item.instances {
                        d.counts[1] = 1;
                        d.counts[2] = instances.len() as u32;
                        instances.extend(i.matrices.iter().map(f32x16));
                        particle_instances += if particle { i.matrices.len() } else { 0 };
                    }

                    draws.push(d);
                }
            }
        }

        self.upload_shadow_draws(&draws, &instances, particle_draws, particle_instances);

        self.make_thick_lines(
            frame
                .shadow_maps
                .iter()
                .flat_map(|map| &map.views)
                .flat_map(|view| &view.items),
        );

        // Vertex buffers (the shared geometry cache).
        for map in &frame.shadow_maps {
            for view in &map.views {
                for item in view.items.iter().filter(|i| i.thick_lines.is_none()) {
                    let g = &item.geometry;
                    let gp = Rc::as_ptr(g);
                    match self.geometries.get_mut(&gp) {
                        Some(e) => e.used = frame_no,
                        None => {
                            let verts = geometry_vertices(g);
                            let count = (verts.len() / FLOATS_PER_VERTEX) as u32;
                            let buffer = vertex_buffer(
                                &self.device,
                                &self.queue,
                                &mut self.free_vertices,
                                &mut self.stats,
                                &verts,
                                particle_kind(item.instances.as_deref(), g),
                            );
                            self.geometries.insert(
                                gp,
                                GeoGpu {
                                    geometry: g.clone(),
                                    buffer,
                                    count,
                                    used: frame_no,
                                },
                            );
                            self.stats.geometry_uploads += 1;
                        }
                    }
                }
            }
        }

        let shadow = self.shadow.as_mut().unwrap();
        shadow.used = frame_no;
        let shadow_maps = shadow.maps.clone();
        let shadow_buffer = shadow.buffer.clone();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("shadow maps"),
            });
        let mut k = 0usize;

        for (i, map) in frame.shadow_maps.iter().enumerate() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow map"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &shadow_maps[i].2,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            for view in &map.views {
                // GL viewport (from the bottom of the map) to the texture's rows (from the top).
                let [x, y, w, h] = view.viewport;
                let top = map.size.saturating_sub(y + h);
                if w == 0 || h == 0 {
                    k += view.items.len();
                    continue;
                }
                pass.set_viewport(x as f32, top as f32, w as f32, h as f32, 0.0, 1.0);
                pass.set_scissor_rect(x, top, w, h);

                for item in &view.items {
                    if let Some(lines) = &item.thick_lines {
                        let e = &self.thick[&Rc::as_ptr(lines)];
                        pass.set_pipeline(&self.thick_pipeline);
                        pass.set_bind_group(
                            0,
                            &self.shadow_group,
                            &[(k as u64 * SHADOW_STRIDE) as u32],
                        );
                        pass.set_vertex_buffer(0, e.triangles.slice(..));
                        pass.draw(0..e.count, 0..1);
                        self.stats.shadow_draw_calls += 1;
                        k += 1;
                        continue;
                    }
                    let g = &item.geometry;
                    let e = &self.geometries[&Rc::as_ptr(g)];
                    let pipeline = match g.geometry_type {
                        0 => &self.shadow_pipelines[0],
                        1 => &self.shadow_pipelines[1],
                        _ => &self.shadow_pipelines[2],
                    };
                    let instances = item.instances.as_ref().map_or(1, |n| n.matrices.len()) as u32;

                    if e.count > 0 && instances > 0 {
                        pass.set_pipeline(pipeline);
                        pass.set_bind_group(
                            0,
                            &self.shadow_group,
                            &[(k as u64 * SHADOW_STRIDE) as u32],
                        );
                        pass.set_vertex_buffer(0, e.buffer.slice(..));
                        pass.draw(0..e.count, 0..instances);
                        self.stats.shadow_draw_calls += 1;
                    }
                    k += 1;
                }
            }
        }

        for (i, map) in frame.shadow_maps.iter().enumerate() {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &shadow_maps[i].1,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::DepthOnly,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &shadow_buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: places[i].0 as u64 * 4,
                        bytes_per_row: Some(places[i].1 * 4),
                        rows_per_image: Some(map.size),
                    },
                },
                wgpu::Extent3d {
                    width: map.size,
                    height: map.size,
                    depth_or_array_layers: 1,
                },
            );
        }

        self.queue.submit([encoder.finish()]);
        (shadow_buffer, places)
    }

    fn make_texture_group(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        t: &TextureImage,
    ) -> wgpu::BindGroup {
        let (view, sampler) = Self::upload_texture(device, queue, t);
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        })
    }

    /// X3DLightingContext.createLibraryTexture ("lut_ggx.png", "lut_charlie.png",
    /// "lut_sheen_E.png", all 1024 × 1024): AVG_PIXEL filters, no mipmaps, CLAMP_TO_BOUNDARY
    /// (clamped to the edge in WebGL), rows as stored (no FLIP_Y in IBL2.glsl); layers 0, 1, 2.
    fn upload_lut(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (wgpu::TextureView, wgpu::Sampler) {
        let pngs: [&[u8]; 3] = [
            include_bytes!("../../../oracle/x_ite-pin/src/assets/lib/lut_ggx.png"),
            include_bytes!("../../../oracle/x_ite-pin/src/assets/lib/lut_charlie.png"),
            include_bytes!("../../../oracle/x_ite-pin/src/assets/lib/lut_sheen_E.png"),
        ];
        let images: Vec<image::RgbaImage> = pngs.iter().map(|p| lut_rgba8(p)).collect();
        let (w, h) = images[0].dimensions();
        assert!(images.iter().all(|i| i.dimensions() == (w, h)), "LUT sizes");
        let data: Vec<u8> = images
            .iter()
            .flat_map(|i| i.as_raw().iter().copied())
            .collect();
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("lut_ggx.png"),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 3,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &data,
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("lut_ggx.png"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        (
            texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            }),
            sampler,
        )
    }

    /// The env maps without an EnvironmentLight: white 1 × 1 cube maps.
    fn white_env_maps(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (wgpu::TextureView, wgpu::Sampler) {
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("no environment"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 18,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &[255; 18 * 4],
        );
        let sampler = device.create_sampler(&Default::default());
        (
            texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::CubeArray),
                ..Default::default()
            }),
            sampler,
        )
    }

    /// The transmission buffer bound when the frame has no transmission pass (never sampled).
    fn white_transmission(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (wgpu::TextureView, wgpu::Sampler) {
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("no transmission"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &[0; 4],
        );
        (
            texture.create_view(&Default::default()),
            device.create_sampler(&Default::default()),
        )
    }

    /// The transmission buffer for a `width` × `height` target (recreated on a size change).
    fn transmission_texture(&mut self, width: u32, height: u32) -> wgpu::Texture {
        let frame_no = self.frame_no;

        if self
            .transmission
            .as_ref()
            .is_none_or(|t| t.width != width || t.height != height)
        {
            let levels = 32 - width.max(height).leading_zeros();
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("transmission buffer"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: levels,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("transmission buffer"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                lod_max_clamp: 32.0,
                ..Default::default()
            });
            let view = texture.create_view(&Default::default());
            self.transmission = Some(TransmissionGpu {
                texture,
                sampled: (view, sampler),
                width,
                height,
                used: frame_no,
            });
        }

        let t = self.transmission.as_mut().unwrap();
        t.used = frame_no;
        t.texture.clone()
    }

    /// X3DRenderObject.draw RenderPass.TRANSMISSION_KEY: every shape but the transmission ones
    /// and the background into the transmission buffer (cleared to 0 0 0 0, one sample, linear
    /// output), then its mip levels.
    #[allow(clippy::too_many_arguments)]
    fn render_transmission_pass(
        &mut self,
        frame: &Frame,
        width: u32,
        height: u32,
        flip: bool,
    ) -> Result<(), String> {
        let texture = self.transmission_texture(width, height);
        let level = |i: u32| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: i,
                mip_level_count: Some(1),
                ..Default::default()
            })
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("transmission"),
            });
        let clear = std::mem::replace(&mut self.clear_color, [0.0; 4]);
        self.transmission_pass = true;
        let result = self.encode_frame(
            &mut encoder,
            frame,
            &level(0),
            wgpu::TextureFormat::Rgba8Unorm,
            width,
            height,
            1,
            flip,
            PassTarget {
                face: true,
                load: false,
                more: false,
            },
        );
        self.transmission_pass = false;
        self.clear_color = clear;
        result?;

        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        for i in 1..texture.mip_level_count() {
            let source = level(i - 1);
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("mipmap"),
                layout: &self.mip_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&source),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });
            let target = level(i);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mipmap"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.mip_pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue.submit([encoder.finish()]);
        Ok(())
    }

    /// X3DRenderObject.draw RenderPass.VOLUME_SCATTER_KEY: the shapes with a
    /// VolumeScatterMaterialExtension (no background) into the scatter buffer (cleared to 0 0 0 0, one
    /// sample, linear output; the alpha is the shape id), then the color and the depth into the buffers
    /// the main pass reads.
    fn render_scatter_pass(
        &mut self,
        frame: &Frame,
        width: u32,
        height: u32,
        flip: bool,
    ) -> Result<(), String> {
        let frame_no = self.frame_no;

        if self
            .scatter
            .as_ref()
            .is_none_or(|s| s.width != width || s.height != height)
        {
            self.scatter = Some(crate::scatter_gpu::ScatterGpu::new(
                &self.device,
                width,
                height,
                frame_no,
            ));
        }

        self.scatter.as_mut().unwrap().used = frame_no;
        let target = self.scatter.as_ref().unwrap().color_view.clone();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("volume scatter"),
            });
        let clear = std::mem::replace(&mut self.clear_color, [0.0; 4]);
        self.scatter_pass = true;
        let result = self.encode_frame(
            &mut encoder,
            frame,
            &target,
            wgpu::TextureFormat::Rgba32Float,
            width,
            height,
            1,
            flip,
            PassTarget {
                face: true,
                load: false,
                more: false,
            },
        );
        self.scatter_pass = false;
        self.clear_color = clear;
        result?;

        let depth = self
            .face_depth
            .as_ref()
            .map(|d| d.0.clone())
            .ok_or("no scatter depth")?;
        self.scatter.as_ref().unwrap().read_back(
            &self.device,
            &self.queue,
            &mut encoder,
            &self.scatter_depth_copy,
            &depth,
        );
        self.queue.submit([encoder.finish()]);
        Ok(())
    }

    /// The scatter buffers (or the dummies of a frame without one).
    fn scatter_buffers(&self) -> (wgpu::Buffer, wgpu::Buffer) {
        match &self.scatter {
            Some(s) => (s.color_buffer.clone(), s.depth_buffer.clone()),
            None => self.scatter_dummy.clone(),
        }
    }

    /// The volume scatter buffer kept between frames (for retention tests): 1 or 0.
    pub fn scatter_resources(&self) -> usize {
        self.scatter.is_some() as usize
    }

    /// The transmission buffer kept between frames (for retention tests).
    pub fn transmission_resources(&self) -> usize {
        self.transmission.is_some() as usize
    }

    #[allow(clippy::too_many_arguments)]
    fn make_frame_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        buffer: &wgpu::Buffer,
        env: &(wgpu::TextureView, wgpu::Sampler),
        transmission: &(wgpu::TextureView, wgpu::Sampler),
        lut: &(wgpu::TextureView, wgpu::Sampler),
        shadow: &wgpu::Buffer,
        scatter: &(wgpu::Buffer, wgpu::Buffer),
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&env.0),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&env.1),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&transmission.0),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&transmission.1),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&lut.0),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&lut.1),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: shadow.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: scatter.0.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: scatter.1.as_entire_binding(),
                },
            ],
        })
    }

    /// EnvironmentLight.generateTextures: filter the specular texture into the diffuse
    /// (LAMBERTIAN, 2048 samples, one level) and specular (GGX, 1024 samples, roughness level /
    /// levels per mip level) cube maps when it changes, and for a GeneratedCubeMapTexture after
    /// each of its passes (its update callback). Returns specularTextureLevels.
    fn ensure_environment(
        &mut self,
        env: &crate::traverse::EnvironmentParams,
        rendered: bool,
    ) -> u32 {
        let source = Rc::as_ptr(&env.specular);

        let frame_no = self.frame_no;

        if let Some(e) = self
            .env
            .as_mut()
            .filter(|e| e.source == source && !rendered)
        {
            e.used = frame_no;
            self.frame_group = e.group.clone();
            return e.levels;
        }

        // The source as the draws sample it.
        let (source_view, source_sampler) = match env.specular.generated {
            Some(key) => {
                let view = self.generated_texture(key, env.specular.width).view.clone();
                (view, Self::make_sampler(&self.device, &env.specular, 1))
            }
            None => Self::upload_texture(&self.device, &self.queue, &env.specular),
        };
        // X3DEnvironmentTextureNode.getSize / setSize: levels = floor (log2 (max (size, 1))).
        let size = env.specular.width.max(1);
        let levels = (size as f64).log2().floor() as u32;
        let flip_x = env.specular.generated.is_none();

        let make = |device: &wgpu::Device, mips: u32| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("filtered environment"),
                size: wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 18,
                },
                mip_level_count: mips,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        };
        // Diffuse (one level, sampled at level 0), specular and sheen maps: the faces of the
        // layers 0..6, 6..12 and 12..18.
        let maps = make(&self.device, levels + 1);

        // Distribution LAMBERTIAN 0, GGX 1, CHARLIE 2.
        self.filter(
            &maps,
            0,
            &source_view,
            &source_sampler,
            size,
            0,
            2048,
            &[0.0],
            flip_x,
        );
        let roughnesses: Vec<f32> = (0..=levels)
            .map(|i| i as f32 / levels.max(1) as f32)
            .collect();
        self.filter(
            &maps,
            6,
            &source_view,
            &source_sampler,
            size,
            1,
            1024,
            &roughnesses,
            flip_x,
        );
        // generatedSheenTexture: Math .max (i / levels, 2, 0.000001) is 2 for every level.
        let sheen_roughnesses: Vec<f32> = (0..=levels)
            .map(|i| (i as f32 / levels.max(1) as f32).max(2.0).max(0.000001))
            .collect();
        self.filter(
            &maps,
            12,
            &source_view,
            &source_sampler,
            size,
            2,
            64,
            &sheen_roughnesses,
            flip_x,
        );

        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("filtered environment"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            lod_max_clamp: 32.0,
            ..Default::default()
        });
        let m = (
            maps.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::CubeArray),
                ..Default::default()
            }),
            sampler,
        );
        let group = Self::make_frame_group(
            &self.device,
            &self.frame_layout,
            &self.frame_buffer,
            &m,
            &self.no_transmission,
            &self.lut,
            &self.shadow_dummy,
            &self.scatter_buffers(),
        );
        self.frame_group = group.clone();
        self.env = Some(EnvGpu {
            source,
            _source: env.specular.clone(),
            maps: m,
            levels,
            group,
            used: frame_no,
        });
        levels
    }

    /// The draws of filterEnvironmentTexture into `target`: each mip level (size >> level) with
    /// its roughness, each face (X3DEnvironmentTextureNode.targets order), one submit each.
    #[allow(clippy::too_many_arguments)]
    fn filter(
        &mut self,
        target: &wgpu::Texture,
        first_layer: u32,
        source: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        size: u32,
        distribution: i32,
        samples: i32,
        roughnesses: &[f32],
        flip_x: bool,
    ) {
        // Front +Z, back -Z, left -X, right +X, top +Y, bottom -Y as wgpu cube layers.
        const LAYER: [u32; 6] = [4, 5, 1, 0, 2, 3];
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("filter"),
            layout: &self.filter_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.filter_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        for (level, roughness) in roughnesses.iter().enumerate() {
            let mip_size = (size >> level).max(1);

            for face in 0..6 {
                let u = FilterU {
                    ints: [face, distribution, samples, size as i32],
                    floats: [*roughness, 0.0, 1.0, 0.0],
                    flip: [if flip_x { -1.0 } else { 1.0 }, 1.0, 1.0, mip_size as f32],
                };
                self.queue
                    .write_buffer(&self.filter_buffer, 0, bytemuck::bytes_of(&u));
                crate::transfer::upload(crate::transfer::Kind::Other, std::mem::size_of_val(&u));
                let view = target.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_mip_level: level as u32,
                    mip_level_count: Some(1),
                    base_array_layer: first_layer + LAYER[face as usize],
                    array_layer_count: Some(1),
                    ..Default::default()
                });
                let mut encoder =
                    self.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("filter"),
                        });
                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("filter"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        ..Default::default()
                    });
                    pass.set_pipeline(&self.filter_pipeline);
                    pass.set_bind_group(0, &group, &[]);
                    pass.draw(0..6, 0..1);
                }
                self.queue.submit([encoder.finish()]);
            }
        }
    }

    /// X3DSingleTextureNode.updateTextureParameters as a sampler for a texture with `level_count`
    /// mip levels.
    fn make_sampler(device: &wgpu::Device, t: &TextureImage, level_count: usize) -> wgpu::Sampler {
        let address = |w: Wrap| match w {
            Wrap::Repeat => wgpu::AddressMode::Repeat,
            Wrap::ClampToEdge => wgpu::AddressMode::ClampToEdge,
            Wrap::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
        };
        let filter = |f: Filter| {
            if f == Filter::Nearest {
                wgpu::FilterMode::Nearest
            } else {
                wgpu::FilterMode::Linear
            }
        };
        let sp = &t.sampler;
        // A minification filter without mipmaps reads level 0 only.
        let mipmap = sp.mipmap.filter(|_| level_count > 1);
        // WebGPU allows anisotropy only with linear filters; GL also needs mipmaps to make it count.
        let anisotropy = if sp.anisotropy > 1.0
            && sp.mag == Filter::Linear
            && sp.min == Filter::Linear
            && mipmap == Some(Filter::Linear)
        {
            sp.anisotropy as u16
        } else {
            1
        };
        device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("image"),
            address_mode_u: address(sp.wrap_s),
            address_mode_v: address(sp.wrap_t),
            address_mode_w: address(sp.wrap_r),
            mag_filter: filter(sp.mag),
            min_filter: filter(sp.min),
            mipmap_filter: if mipmap == Some(Filter::Linear) {
                wgpu::MipmapFilterMode::Linear
            } else {
                wgpu::MipmapFilterMode::Nearest
            },
            lod_max_clamp: if mipmap.is_some() { 32.0 } else { 0.0 },
            anisotropy_clamp: anisotropy,
            ..Default::default()
        })
    }

    /// Upload an image (2D, or the six faces of a cube map) with its mip chain, and make its
    /// sampler (X3DSingleTextureNode.updateTextureParameters).
    fn upload_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        t: &TextureImage,
    ) -> (wgpu::TextureView, wgpu::Sampler) {
        if t.volume {
            return Self::upload_texture_3d(device, queue, t);
        }

        let layers = if t.cube { 6 } else { 1 };
        let face_bytes = (t.width * t.height * 4) as usize;
        let faces: Vec<Vec<(u32, u32, Vec<u8>)>> = (0..layers)
            .map(|f| {
                let face = TextureImage {
                    rgba: t.rgba[f * face_bytes..(f + 1) * face_bytes].to_vec(),
                    cube: false,
                    ..t.clone()
                };
                if t.no_mipmaps {
                    vec![(t.width, t.height, face.rgba)]
                } else {
                    mip_chain(&face)
                }
            })
            .collect();
        let level_count = faces[0].len();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("image"),
            size: wgpu::Extent3d {
                width: t.width,
                height: t.height,
                depth_or_array_layers: layers as u32,
            },
            mip_level_count: level_count as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        for (layer, levels) in faces.iter().enumerate() {
            for (level, (w, h, data)) in levels.iter().enumerate() {
                crate::transfer::upload(crate::transfer::Kind::Other, data.len());
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: level as u32,
                        origin: wgpu::Origin3d {
                            x: 0,
                            y: 0,
                            z: layer as u32,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * w),
                        rows_per_image: Some(*h),
                    },
                    wgpu::Extent3d {
                        width: *w,
                        height: *h,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }

        let sampler = Self::make_sampler(device, t, level_count);
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(if t.cube {
                wgpu::TextureViewDimension::Cube
            } else {
                wgpu::TextureViewDimension::D2
            }),
            ..Default::default()
        });
        (view, sampler)
    }

    /// Upload an X3DTexture3DNode texture (TEXTURE_3D) with its mip chain (gl.generateMipmap).
    fn upload_texture_3d(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        t: &TextureImage,
    ) -> (wgpu::TextureView, wgpu::Sampler) {
        let levels = crate::texture::mip_chain_3d(t);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("image 3D"),
            size: wgpu::Extent3d {
                width: t.width,
                height: t.height,
                depth_or_array_layers: t.depth,
            },
            mip_level_count: levels.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        for (level, (w, h, d, data)) in levels.iter().enumerate() {
            crate::transfer::upload(crate::transfer::Kind::Other, data.len());
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * w),
                    rows_per_image: Some(*h),
                },
                wgpu::Extent3d {
                    width: *w,
                    height: *h,
                    depth_or_array_layers: *d,
                },
            );
        }

        let sampler = Self::make_sampler(device, t, levels.len());
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D3),
            ..Default::default()
        });
        (view, sampler)
    }

    /// Draw `frame` into `target` (a `width` x `height` view with `format`, which must be a
    /// non-sRGB colour format) and submit. The target keeps what it had outside the draw; the
    /// renderer clears it to [`Renderer::clear_color`] first.
    pub fn render(
        &mut self,
        frame: &Frame,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("x3d") });
        self.encode(&mut encoder, frame, target, format, width, height)?;
        self.queue.submit([encoder.finish()]);
        self.after_submit();
        Ok(())
    }

    fn ensure_offscreen(&mut self, width: u32, height: u32) {
        if self
            .offscreen
            .as_ref()
            .is_none_or(|t| t.2 != width || t.3 != height)
        {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("color"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            self.offscreen = Some((texture, view, width, height));
        }
    }

    /// Render a frame into an internal RGBA8 texture and read it back: tightly packed rows, top
    /// row first. The path of the screenshot tools and the oracle comparisons (no window).
    pub fn render_to_rgba(&mut self, frame: &Frame, width: u32, height: u32) -> Vec<u8> {
        self.ensure_offscreen(width, height);
        let bytes_per_row = (width * 4).div_ceil(256) * 256;

        if self
            .readback
            .as_ref()
            .is_none_or(|r| r.1 != width || r.2 != height)
        {
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("readback"),
                size: (bytes_per_row * height) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            self.readback = Some((buffer, width, height));
        }

        let view = self.offscreen.as_ref().unwrap().1.clone();
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let flip = self.gl_sample_orientation && self.sample_count() > 1;
        self.flip_now = flip;
        let encoded = self.encode(
            &mut encoder,
            frame,
            &view,
            wgpu::TextureFormat::Rgba8Unorm,
            width,
            height,
        );
        self.flip_now = false;
        encoded.expect("render");
        let (texture, readback) = (
            &self.offscreen.as_ref().unwrap().0,
            &self.readback.as_ref().unwrap().0,
        );
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        self.after_submit();

        let readback = &self.readback.as_ref().unwrap().0;
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        crate::transfer::readback(crate::transfer::Kind::Other, readback.size());
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let data = slice.get_mapped_range().expect("map readback");
        let mut out = Vec::with_capacity((width * height * 4) as usize);

        for y in 0..height {
            let row = if flip { height - 1 - y } else { y };
            out.extend_from_slice(
                &data[(row * bytes_per_row) as usize..(row * bytes_per_row + width * 4) as usize],
            );
        }

        drop(data);
        readback.unmap();
        out
    }

    /// Render into the internal texture without a readback; waits for the GPU and returns the
    /// pass's GPU milliseconds (None without timestamp support). For benchmarks.
    pub fn render_offscreen_timed(
        &mut self,
        frame: &Frame,
        width: u32,
        height: u32,
    ) -> Option<f64> {
        self.ensure_offscreen(width, height);
        let view = self.offscreen.as_ref().unwrap().1.clone();
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.encode(
            &mut encoder,
            frame,
            &view,
            wgpu::TextureFormat::Rgba8Unorm,
            width,
            height,
        )
        .expect("render");
        self.queue.submit([encoder.finish()]);
        self.after_submit();
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        self.gpu_time_ms(true)
    }

    /// Call after submitting an encoder filled by [`Renderer::encode`]: starts the timestamp
    /// readback.
    pub fn after_submit(&mut self) {
        if let Some(t) = self.timer.as_mut() {
            if t.recorded {
                t.recorded = false;
                let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let f2 = flag.clone();
                crate::transfer::readback(crate::transfer::Kind::Other, t.read.size());
                t.read.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                    if r.is_ok() {
                        f2.store(true, std::sync::atomic::Ordering::Release);
                    }
                });
                t.pending = Some(flag);
            }
        }
    }

    /// Record the frame's commands into `encoder` (the caller submits, then calls
    /// [`Renderer::after_submit`]); lets a host append its own passes or a texture copy.
    pub fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &Frame,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        if !target_format_ok(format) {
            return Err(format!(
                "x3d-render: the target format {format:?} is not supported; use a non-sRGB color format such as Bgra8Unorm or Rgba8Unorm"
            ));
        }

        if width == 0 || height == 0 {
            return Err("x3d-render: empty target".into());
        }

        self.frame_no += 1;
        self.stats = RenderStats::default();

        // A per-frame geometry only its cache entry still holds is never handed out again: its buffer is free for
        // reuse. Other geometries age out after KEEP_FRAMES as before.
        let (frame_no, free) = (self.frame_no, &mut self.free_vertices);
        self.geometries.retain(|_, e| {
            if !e.geometry.per_frame || Rc::strong_count(&e.geometry) > 1 {
                return true;
            }
            free.push(FreeBuffer {
                buffer: e.buffer.clone(),
                freed: frame_no,
            });
            false
        });
        for buffer in self.frame_buffers.drain(..) {
            free.push(FreeBuffer {
                buffer,
                freed: frame_no,
            });
        }
        self.thick.retain(|_, e| {
            if Rc::strong_count(&e.lines) > 1 {
                return true;
            }
            for buffer in [&e.input, &e.triangles] {
                free.push(FreeBuffer {
                    buffer: buffer.clone(),
                    freed: frame_no,
                });
            }
            false
        });

        // X3DRenderObject.draw: the rendered textures first, each pass submitted on its own so
        // its uniform writes land before the next one.
        for pass in &frame.cube_passes {
            self.render_cube_pass(pass)?;
        }

        for pass in &frame.rendered_passes {
            self.render_rendered_pass(pass)?;
        }

        // EnvironmentLight: a rendered specularTexture's update callback filters it again.
        self.env_rendered = env_source_rendered(frame);

        let samples = if self.debug_ids {
            1
        } else {
            self.sample_count()
        };
        let flip = self.flip_now;

        if frame.layers.is_empty() {
            return self.encode_frame(
                encoder,
                frame,
                target,
                format,
                width,
                height,
                samples,
                flip,
                PassTarget {
                    face: false,
                    load: false,
                    more: false,
                },
            );
        }

        // LayerSet.traverse: one pass per layer, in order, each keeping what the layers before it
        // drew (drawShapes clears only the depth). The passes share the uniform buffers, so each
        // but the last is submitted before the next one writes them; the last goes into `encoder`.
        let count = frame.layers.len();

        for (i, layer) in frame.layers.iter().enumerate() {
            let pass = PassTarget {
                face: false,
                load: i > 0,
                more: i + 1 < count,
            };

            if pass.more {
                let mut own = self
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("x3d layer"),
                    });
                self.encode_frame(
                    &mut own, layer, target, format, width, height, samples, flip, pass,
                )?;
                self.queue.submit([own.finish()]);
            } else {
                self.encode_frame(
                    encoder, layer, target, format, width, height, samples, flip, pass,
                )?;
            }
        }

        Ok(())
    }

    /// GPU objects kept between frames: geometries, point sprites, textures, texture bind groups,
    /// style bind groups, GeneratedCubeMapTexture cube maps, filtered EnvironmentLight maps (for
    /// retention tests).
    /// The shadow map textures and the bytes of the shadow value buffer kept between frames.
    pub fn shadow_resources(&self) -> (usize, u64) {
        self.shadow
            .as_ref()
            .map_or((0, 0), |s| (s.maps.len(), s.buffer.size()))
    }

    /// VolumeRendering GPU objects: shader modules, pipelines, bind groups, uniform buffers.
    pub fn volume_resources(&self) -> [usize; 4] {
        self.volume.resources()
    }

    /// GaussianSplats GPU objects: data sets, sorted index buffers, pipelines, bind groups.
    /// ComposedShader GPU objects held: modules, pipelines, vertex buffers, bind groups, uniform buffer.
    pub fn shader_resources(&self) -> [usize; 5] {
        self.custom.resources()
    }

    /// ComposedShader draws of the last frame.
    pub fn shader_draws(&self) -> usize {
        self.custom.draws()
    }

    pub fn splat_resources(&self) -> [usize; 4] {
        self.splats.resources()
    }

    /// Sorts the GaussianSplats code ran so far (the sort runs when a draw's model-view matrix changes).
    pub fn splat_sorts(&self) -> usize {
        self.splats.sorts
    }

    pub fn resource_counts(&self) -> [usize; 7] {
        [
            self.geometries.len(),
            self.sprites.len(),
            self.textures.len(),
            self.slot_groups.len(),
            self.style_groups.len(),
            self.generated.len(),
            self.env.is_some() as usize,
        ]
    }

    /// Only the GeneratedCubeMapTexture passes of `frame`: a host that steps frames it does not
    /// show (x3d-drive) keeps the textures as current as a browser that draws every frame.
    pub fn render_cube_passes(&mut self, frame: &Frame) -> Result<(), String> {
        self.frame_no += 1;

        for pass in &frame.cube_passes {
            self.render_cube_pass(pass)?;
        }

        for pass in &frame.rendered_passes {
            self.render_rendered_pass(pass)?;
        }

        if let Some(env) = frame
            .environment
            .as_ref()
            .or_else(|| frame.layers.iter().find_map(|l| l.environment.as_ref()))
            .filter(|_| env_source_rendered(frame))
        {
            self.ensure_environment(env, true);
        }

        Ok(())
    }

    /// The GL texture of a GeneratedCubeMapTexture (zeros until a pass draws it).
    fn generated_texture(&mut self, key: (u64, NodeId), size: u32) -> &mut GenGpu {
        let frame_no = self.frame_no;

        if self.generated.get(&key).is_none_or(|g| g.size != size) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("generated cube map"),
                size: wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 6,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::Cube),
                ..Default::default()
            });
            let faces = (0..6)
                .map(|layer| {
                    texture.create_view(&wgpu::TextureViewDescriptor {
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        base_array_layer: layer,
                        array_layer_count: Some(1),
                        ..Default::default()
                    })
                })
                .collect();
            self.generated.insert(
                key,
                GenGpu {
                    view,
                    faces,
                    size,
                    used: frame_no,
                },
            );
            // Bind groups and texture entries that still refer to an old cube map.
            self.slot_groups.clear();
            self.textures.retain(|_, t| t._image.generated != Some(key));
        }

        let g = self.generated.get_mut(&key).unwrap();
        g.used = frame_no;
        g
    }

    /// The texture of a RenderedTexture (transparent black until a pass draws it).
    fn rendered_gpu(
        &mut self,
        key: (u64, NodeId),
        width: u32,
        height: u32,
        float: bool,
    ) -> &mut RenGpu {
        let frame_no = self.frame_no;

        if self
            .rendered
            .get(&key)
            .is_none_or(|r| r.width != width || r.height != height || r.float != float)
        {
            let format = rendered_format(float);
            let make = |label, usage| {
                self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
            };
            let texture = make(
                "rendered texture",
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            );
            let scratch = make(
                "rendered texture pass",
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            );
            let depth = float.then(|| {
                self.device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("rendered texture depth"),
                        size: wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: SHADOW_FORMAT,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            });
            let view = texture.create_view(&Default::default());
            let scratch_view = scratch.create_view(&Default::default());
            self.rendered.insert(
                key,
                RenGpu {
                    texture,
                    view,
                    scratch,
                    scratch_view,
                    depth,
                    width,
                    height,
                    float,
                    used: frame_no,
                },
            );
            // Bind groups and texture entries that still refer to an old texture.
            self.slot_groups.clear();
            self.textures.retain(|_, t| t._image.generated != Some(key));
        }

        let r = self.rendered.get_mut(&key).unwrap();
        r.used = frame_no;
        r
    }

    /// The RenderedTexture textures kept between frames (for retention tests).
    pub fn rendered_resources(&self) -> usize {
        self.rendered.len()
    }

    /// RenderedTexture.renderTexture: the pass frame into the scratch target (cleared to 0 0 0 0,
    /// or for a depth map to the depth clear color 1 1 1 1; no multisampling; rows bottom-up like
    /// copyTexSubImage2D from a GL framebuffer), then copied into the texture.
    fn render_rendered_pass(&mut self, pass: &crate::traverse::RenderedPass) -> Result<(), String> {
        let (width, height) = (pass.width, pass.height);
        let (scratch_view, scratch, texture, depth) = {
            let r = self.rendered_gpu(pass.key, width, height, pass.depth_map);
            (
                r.scratch_view.clone(),
                r.scratch.clone(),
                r.texture.clone(),
                r.depth.clone(),
            )
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rendered texture"),
            });

        if let Some(depth) = depth {
            self.encode_depth_color(
                &mut encoder,
                &pass.frame,
                &scratch_view,
                &depth,
                width,
                height,
            );
        } else {
            let clear = std::mem::replace(&mut self.clear_color, [0.0; 4]);
            let result = self.encode_frame(
                &mut encoder,
                &pass.frame,
                &scratch_view,
                rendered_format(false),
                width,
                height,
                1,
                true,
                PassTarget {
                    face: true,
                    load: false,
                    more: false,
                },
            );
            self.clear_color = clear;
            result?;
        }

        encoder.copy_texture_to_texture(
            scratch.as_image_copy(),
            texture.as_image_copy(),
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        Ok(())
    }

    /// X3DRenderObject.depth for a RenderedTexture depthMap: every shape of the pass frame with the
    /// depth shader (no culling, LEQUAL, clip planes), the window depth as color.
    fn encode_depth_color(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &Frame,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        let frame_no = self.frame_no;
        // Rows bottom-up (GL framebuffer order): the projection's y negated.
        let mut projection = frame.projection;
        for i in [1, 5, 9, 13] {
            projection.0[i] = -projection.0[i];
        }

        let mut draws: Vec<ShadowDrawU> = vec![];
        let mut instances: Vec<[f32; 16]> = vec![];
        let (mut particle_draws, mut particle_instances) = (0usize, 0usize);
        for item in &frame.items {
            let mut d: ShadowDrawU = bytemuck::Zeroable::zeroed();
            d.projection = f32x16(&projection);
            d.model_view = f32x16(&item.model_view);
            for (k, p) in item.clip_planes.iter().take(MAX_CLIP_PLANES).enumerate() {
                d.clip_planes[k] = p.map(|v| v as f32);
            }
            d.counts[0] = item.clip_planes.len().min(MAX_CLIP_PLANES) as u32;

            // InstancedShape, ParticleSystem: one instanced draw.
            if let Some(i) = &item.instances {
                d.counts[1] = 1;
                d.counts[2] = instances.len() as u32;
                instances.extend(i.matrices.iter().map(f32x16));

                if i.particle {
                    particle_draws += 1;
                    particle_instances += i.matrices.len();
                }
            }

            draws.push(d);

            let gp = Rc::as_ptr(&item.geometry);
            match self.geometries.get_mut(&gp) {
                Some(e) => e.used = frame_no,
                None => {
                    let verts = geometry_vertices(&item.geometry);
                    let count = (verts.len() / FLOATS_PER_VERTEX) as u32;
                    let buffer = vertex_buffer(
                        &self.device,
                        &self.queue,
                        &mut self.free_vertices,
                        &mut self.stats,
                        &verts,
                        particle_kind(item.instances.as_deref(), &item.geometry),
                    );
                    self.geometries.insert(
                        gp,
                        GeoGpu {
                            geometry: item.geometry.clone(),
                            buffer,
                            count,
                            used: frame_no,
                        },
                    );
                    self.stats.geometry_uploads += 1;
                }
            }
        }

        self.upload_shadow_draws(&draws, &instances, particle_draws, particle_instances);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("rendered texture depth map"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_viewport(0.0, 0.0, width as f32, height as f32, 0.0, 1.0);

        let mut k = 0usize;

        for item in frame.items.iter() {
            let e = &self.geometries[&Rc::as_ptr(&item.geometry)];
            let pipeline = match item.geometry.geometry_type {
                0 => &self.depth_color_pipelines[0],
                1 => &self.depth_color_pipelines[1],
                _ => &self.depth_color_pipelines[2],
            };
            let instances = item.instances.as_ref().map_or(1, |n| n.matrices.len()) as u32;

            if e.count > 0 && instances > 0 {
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &self.shadow_group, &[(k as u64 * SHADOW_STRIDE) as u32]);
                pass.set_vertex_buffer(0, e.buffer.slice(..));
                pass.draw(0..e.count, 0..instances);
                self.stats.depth_draw_calls += 1;
            }
            k += 1;
        }
    }

    /// GeneratedCubeMapTexture.renderTexture: the six face frames into the faces of the texture's
    /// cube map (cleared to 0 0 0 0, no multisampling, rows bottom-up like copyTexSubImage2D from a
    /// GL framebuffer).
    fn render_cube_pass(&mut self, pass: &crate::traverse::CubePass) -> Result<(), String> {
        let size = pass.size;
        let faces = self.generated_texture(pass.key, size).faces.clone();
        // X3DEnvironmentTextureNode.targets: front +Z, back -Z, left -X, right +X, top +Y, bottom -Y.
        const LAYER: [usize; 6] = [4, 5, 1, 0, 2, 3];
        let clear = std::mem::replace(&mut self.clear_color, [0.0; 4]);

        for (i, frame) in pass.faces.iter().enumerate() {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("generated cube map face"),
                });
            let result = self.encode_frame(
                &mut encoder,
                frame,
                &faces[LAYER[i]],
                wgpu::TextureFormat::Rgba8Unorm,
                size,
                size,
                1,
                true,
                PassTarget {
                    face: true,
                    load: false,
                    more: false,
                },
            );
            if let Err(e) = result {
                self.clear_color = clear;
                return Err(e);
            }
            self.queue.submit([encoder.finish()]);
        }

        self.clear_color = clear;
        Ok(())
    }

    /// Record one pass of `frame` into `target` (see [`Renderer::encode`]); `face` uses the cube
    /// face depth buffer and no timestamps.
    #[allow(clippy::too_many_arguments)]
    fn encode_frame(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &Frame,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        samples: u32,
        flip: bool,
        pass_target: PassTarget,
    ) -> Result<(), String> {
        let frame_no = self.frame_no;
        let face = pass_target.face;
        // The layer's gl.viewport (the whole target without a LayerSet).
        let layer_rect = frame
            .viewport
            .unwrap_or([0.0, 0.0, width as f64, height as f64]);

        // --- frame uniforms
        let mut fu: FrameU = bytemuck::Zeroable::zeroed();
        let mut projection = frame.projection;

        if self.depth_mode == DepthMode::Reversed {
            reverse_depth(&mut projection);
        }

        fu.projection = f32x16(&projection);

        if flip {
            for i in [1, 5, 9, 13] {
                fu.projection[i] = -fu.projection[i];
            }
        }
        let n = frame.lights.len().min(MAX_FRAME_LIGHTS);
        fu.num_lights = [n as u32, 0, 0, 0];
        for (i, l) in frame.lights.iter().take(MAX_FRAME_LIGHTS).enumerate() {
            let m = l.matrix;
            fu.lights[i] = LightU {
                color_type: [
                    l.color[0] as f32,
                    l.color[1] as f32,
                    l.color[2] as f32,
                    l.kind as f32,
                ],
                direction_intensity: [
                    l.direction.x as f32,
                    l.direction.y as f32,
                    l.direction.z as f32,
                    l.intensity as f32,
                ],
                location_ambient: [
                    l.location.x as f32,
                    l.location.y as f32,
                    l.location.z as f32,
                    l.ambient_intensity as f32,
                ],
                attenuation_radius: [
                    l.attenuation[0] as f32,
                    l.attenuation[1] as f32,
                    l.attenuation[2] as f32,
                    l.radius as f32,
                ],
                spot: [l.cut_off_angle as f32, l.beam_width as f32, 0.0, 0.0],
                m0: [m[0], m[1], m[2], 0.0],
                m1: [m[3], m[4], m[5], 0.0],
                m2: [m[6], m[7], m[8], 0.0],
                shadow_color: [0.0; 4],
                shadow_params: [0.0; 4],
                shadow_matrix: [0.0; 16],
            };

            if let Some(sh) = &l.shadow {
                fu.lights[i].shadow_color = [
                    sh.color[0] as f32,
                    sh.color[1] as f32,
                    sh.color[2] as f32,
                    sh.intensity as f32,
                ];
                fu.lights[i].shadow_params = [sh.bias as f32, sh.map_size as f32, 0.0, 0.0];
                fu.lights[i].shadow_matrix = f32x16(&sh.matrix);
            }
        }

        for (i, p) in frame
            .projectors
            .iter()
            .take(MAX_FRAME_PROJECTORS)
            .enumerate()
        {
            fu.projectors[i] = ProjectorU {
                color: [
                    p.color[0] as f32,
                    p.color[1] as f32,
                    p.color[2] as f32,
                    p.intensity as f32,
                ],
                location: [
                    p.location.x as f32,
                    p.location.y as f32,
                    p.location.z as f32,
                    p.linear as u32 as f32,
                ],
                params: [p.params[0] as f32, p.params[1] as f32, 0.0, 0.0],
                matrix: f32x16(&p.matrix),
            };
        }

        if let Some(f) = &frame.fog {
            fu.fog_color_type = [f.color[0], f.color[1], f.color[2], f.fog_type as f32];
            fu.fog_range = [f.visibility_start, f.visibility_range, 0.0, 0.0];
            fu.fog_m0 = [f.matrix[0], f.matrix[1], f.matrix[2], 0.0];
            fu.fog_m1 = [f.matrix[3], f.matrix[4], f.matrix[5], 0.0];
            fu.fog_m2 = [f.matrix[6], f.matrix[7], f.matrix[8], 0.0];
        }

        if self.depth_mode == DepthMode::Logarithmic {
            // x3d_LogarithmicFarFactor1_2 = 1 / log2 (far + 1); far recovered from the perspective
            // (or orthographic) projection of the frame.
            let (_, far) = near_far(&frame.projection);
            fu.fog_range[2] = (1.0 / (far + 1.0).log2()) as f32;
        }

        fu.viewport = [
            width as f32,
            height as f32,
            flip as u32 as f32,
            (self.shading == ShadingMode::Flat) as u32 as f32,
        ];
        fu.pass_flags = [
            self.transmission_pass as u32 as f32,
            self.scatter_pass as u32 as f32,
            self.scatter.as_ref().map_or(0, |s| s.stride) as f32,
            0.0,
        ];
        fu.inv_projection = crate::splats_gpu::inverse_f32(&f32x16(&frame.projection));

        // X3DRenderObject.draw: renderPasses & VOLUME_SCATTER_BIT renders the scatter buffer before the
        // transmission buffer.
        let scatter = !self.transmission_pass
            && !self.scatter_pass
            && frame.items.iter().any(|i| i.volume_scatter());

        if scatter {
            self.render_scatter_pass(frame, width, height, flip)?;
        }

        // The scatter buffer of this frame (also read by the transmission pass's shapes).
        let scatter_used =
            !self.scatter_pass && self.scatter.as_ref().is_some_and(|s| s.used == frame_no);

        if scatter_used {
            fu.pass_flags[2] = self.scatter.as_ref().map_or(0, |s| s.stride) as f32;
        }

        // X3DRenderObject.draw: renderPasses & TRANSMISSION_BIT renders the transmission buffer
        // before the shapes (its own submit, before this pass writes the uniforms).
        let transmission = !self.transmission_pass && frame.items.iter().any(|i| i.transmission());

        if transmission {
            self.render_transmission_pass(frame, width, height, flip)?;
        }
        fu.layer = [layer_rect[2] as f32, layer_rect[3] as f32, 0.0, 0.0];

        // ToneMapping2.glsl x3d_Exposure and tone map; IBL2.glsl x3d_EnvironmentLightSource.
        fu.tone = [frame.tone_mapping as f32, frame.exposure as f32, 0.0, 0.0];

        match &frame.environment {
            Some(env) => {
                let rendered = !face && std::mem::take(&mut self.env_rendered);
                let levels = self.ensure_environment(env, rendered);
                let r = &env.rotation;
                fu.tone[2] = 1.0;
                fu.tone[3] = levels as f32;
                fu.env_color = [
                    env.color[0] as f32,
                    env.color[1] as f32,
                    env.color[2] as f32,
                    env.intensity as f32,
                ];
                fu.env_m0 = [r[0], r[1], r[2], 0.0];
                fu.env_m1 = [r[3], r[4], r[5], 0.0];
                fu.env_m2 = [r[6], r[7], r[8], 0.0];
            }
            None => self.frame_group = self.frame_group_default.clone(),
        }

        // X3DRenderObject.draw: renderShadowMap of each light before the shapes.
        let mut shadow_buffer = self.shadow_dummy.clone();

        if !frame.shadow_maps.is_empty() {
            let (shadow, places) = self.render_shadow_maps(frame);

            for (i, l) in frame.lights.iter().take(MAX_FRAME_LIGHTS).enumerate() {
                if let Some(&(first, stride)) = l.shadow.as_ref().and_then(|sh| places.get(sh.map))
                {
                    fu.lights[i].shadow_params[2] = f32::from_bits(first);
                    fu.lights[i].shadow_params[3] = f32::from_bits(stride);
                }
            }

            shadow_buffer = shadow;
        }

        if !frame.shadow_maps.is_empty() || transmission || scatter_used {
            let maps = match (&frame.environment, &self.env) {
                (Some(_), Some(e)) => e.maps.clone(),
                _ => self.white_env.clone(),
            };
            let t = if transmission {
                self.transmission.as_ref().unwrap().sampled.clone()
            } else {
                self.no_transmission.clone()
            };
            self.frame_group = Self::make_frame_group(
                &self.device,
                &self.frame_layout,
                &self.frame_buffer,
                &maps,
                &t,
                &self.lut,
                &shadow_buffer,
                &self.scatter_buffers(),
            );
        }

        self.queue
            .write_buffer(&self.frame_buffer, 0, bytemuck::bytes_of(&fu));
        crate::transfer::upload(crate::transfer::Kind::Other, std::mem::size_of_val(&fu));

        // --- draw list in X3DRenderObject.drawShapes order
        let mut scratch_data: Vec<f32> = std::mem::take(&mut self.scratch_data);
        scratch_data.clear();
        let mut draws: Vec<DrawU> = Vec::with_capacity(frame.items.len() + 8);
        // The draws, instances and scratch floats of ParticleSystems among them (crate::transfer).
        let (mut particle_draws, mut particle_instances, mut particle_scratch) =
            (0usize, 0usize, 0usize);
        let mut instance_data: Vec<InstanceU> = vec![];
        let mut calls = std::mem::take(&mut self.calls);
        calls.clear();
        self.debug_draws.clear();
        let mut used_textures: Vec<Rc<TextureImage>> = vec![];
        let mut used_styles: Vec<Rc<TextureImage>> = vec![];

        // Shading POINT and WIREFRAME (X3DShadersContext.setShading): the primitive modes of the
        // geometry nodes, and the polygon mode, which X3DBackgroundNode.display sets back to FILL
        // for the background.
        let points = self.shading == ShadingMode::Point;
        let shading_phong = self.shading == ShadingMode::Phong;
        let wireframe = self.shading == ShadingMode::Wireframe;

        if let Some(bg) = &frame.background {
            let start = scratch_data.len() as u64 * 4;
            let count = bg.sphere.len() / 4;
            for i in 0..count {
                let p = &bg.sphere[i * 4..i * 4 + 3];
                let c = &bg.colors[i * 4..i * 4 + 4];
                scratch_data.extend_from_slice(&[
                    p[0], p[1], p[2], 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, c[0], c[1], c[2], c[3],
                ]);
                scratch_data.extend_from_slice(&VERTEX_EXTRAS);
            }
            let end = scratch_data.len() as u64 * 4;
            let mut d: DrawU = bytemuck::Zeroable::zeroed();
            d.model_view = f32x16(&bg.model_view);
            d.texture_matrices[0] = f32x16(&Matrix4::IDENTITY);
            d.emissive_transparency = [1.0, 1.0, 1.0, bg.transparency as f32];
            d.flags = [0, 0, 1, 0];
            d.flags2 = [0, (bg.transparency == 0.0) as u32, 3, 1];
            // The default material (UnlitMaterial) is not physical: linear only for ColorSpace LINEAR.
            d.pbr_flags[1] = (frame.color_space == 1) as u32;
            d.diffuse_ambient[0] = (draws.len() + 1) as f32;
            draws.push(d);
            if count > 0 {
                calls.push(Call {
                    key: PipelineKey::new(
                        Topology::Triangles,
                        Cull::Back,
                        true,
                        bg.transparency > 0.0,
                        false,
                        false,
                    ),
                    blend_constant: None,
                    style_texture: None,
                    source: Source::Scratch,
                    range: (start, end),
                    count: count as u32,
                    instances: 1,
                    draw_index: draws.len() - 1,
                    textures: ([None; SLOTS], false),
                    viewport: None,
                });
            }

            // X3DBackgroundNode.drawCube: every loaded face, unlit white x texture, blended
            // when the image has alpha, same cull/depth state as the sphere.
            for face in &bg.faces {
                let start = scratch_data.len() as u64 * 4;

                for i in 0..6 {
                    let p = &face.vertices[i * 4..i * 4 + 3];
                    let t = &crate::traverse::FACE_TEX_COORDS[i * 4..i * 4 + 4];
                    scratch_data.extend_from_slice(&[
                        p[0], p[1], p[2], 0.0, 0.0, 0.0, t[0], t[1], t[2], t[3], 1.0, 1.0, 1.0, 1.0,
                    ]);
                    scratch_data.extend_from_slice(&VERTEX_EXTRAS);
                }

                let end = scratch_data.len() as u64 * 4;
                let tp = Rc::as_ptr(&face.texture);
                used_textures.push(face.texture.clone());

                let mut d: DrawU = bytemuck::Zeroable::zeroed();
                d.model_view = f32x16(&bg.model_view);
                d.texture_matrices[0] = f32x16(&Matrix4::IDENTITY);
                d.emissive_transparency = [1.0, 1.0, 1.0, 0.0];
                d.flags = [0, 0, 0, 1];
                d.tex_units[0] = [1, face.texture.flip_y as u32, 0, 0];
                d.tex_counts = [1, 0, 1, 1];
                d.flags2 = [
                    face.texture.flip_y as u32,
                    (!face.texture.transparent) as u32,
                    3,
                    1,
                ];
                d.pbr_flags[1] = (frame.color_space == 1) as u32;
                d.diffuse_ambient[0] = (draws.len() + 1) as f32;
                draws.push(d);
                calls.push(Call {
                    key: PipelineKey::new(
                        Topology::Triangles,
                        Cull::Back,
                        true,
                        face.texture.transparent,
                        false,
                        false,
                    ),
                    blend_constant: None,
                    style_texture: None,
                    source: Source::Scratch,
                    range: (start, end),
                    count: 6,
                    instances: 1,
                    draw_index: draws.len() - 1,
                    textures: {
                        let mut k = [None; SLOTS];
                        k[0] = Some(tp);
                        (k, false)
                    },
                    viewport: None,
                });
            }
        }

        // X3DShapeNode [TRANSMISSION_KEY]: null for the transmission shapes.
        // X3DShapeNode [VOLUME_SCATTER_KEY]: only the shapes with a VolumeScatterMaterialExtension.
        let drawn = |i: &&DrawItem| {
            !(self.transmission_pass && i.transmission())
                && (!self.scatter_pass || i.volume_scatter())
        };
        let mut order: Vec<&DrawItem> = frame
            .items
            .iter()
            .filter(drawn)
            .filter(|i| !i.transparent)
            .collect();
        let mut transparent: Vec<&DrawItem> = frame
            .items
            .iter()
            .filter(drawn)
            .filter(|i| i.transparent)
            .collect();
        // MergeSort (a .distance < b .distance): stable, ascending view-space z.
        transparent.sort_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let n_opaque = order.len();
        order.extend(transparent);

        // VolumeRendering: the volumes of the frame, in draw order (crate::volume_gpu).
        let mut volume_inputs: Vec<crate::volume_gpu::VolumeInput> = vec![];
        // ComposedShader: the custom draws of the frame, in draw order (crate::shader_gpu).
        let mut custom_inputs: Vec<crate::shader_gpu::CustomInput> = vec![];
        let mut builtin_draws: Vec<(usize, Vec<Rc<TextureImage>>)> = vec![];

        // GaussianSplats: the splat draws of the frame, in draw order (crate::splats_gpu).
        let mut splat_inputs: Vec<crate::splats_gpu::SplatInput> = vec![];

        // X3DMaterialExtensionNode.setShaderUniforms: browser.getShapeId () per shape of the scatter pass.
        let mut scatter_id = 0u32;

        for (k, item) in order.iter().enumerate() {
            let is_transparent = k >= n_opaque;

            if let Some(s) = &item.splats {
                splat_inputs.push(crate::splats_gpu::SplatInput {
                    draw: s,
                    clip_planes: item
                        .clip_planes
                        .iter()
                        .take(MAX_CLIP_PLANES)
                        .copied()
                        .collect(),
                    viewport: item.viewport.unwrap_or(layer_rect),
                    fog: item.fog.as_ref().or(frame.fog.as_ref()),
                });
                calls.push(Call {
                    key: PipelineKey::new(Topology::Triangles, Cull::Back, true, true, false, true),
                    blend_constant: None,
                    style_texture: None,
                    source: Source::Splats(splat_inputs.len() - 1),
                    range: (0, 0),
                    count: 0,
                    instances: 1,
                    draw_index: 0,
                    textures: ([None; SLOTS], false),
                    viewport: item.viewport,
                });
                continue;
            }

            if let Some(v) = &item.volume {
                used_textures.extend(v.textures.iter().cloned());
                let lights = item
                    .lights
                    .iter()
                    .copied()
                    .filter(|l| (*l as usize) < MAX_FRAME_LIGHTS)
                    .take(MAX_LIGHTS)
                    .collect();
                volume_inputs.push(crate::volume_gpu::VolumeInput {
                    draw: v,
                    lights,
                    clip_planes: item
                        .clip_planes
                        .iter()
                        .take(MAX_CLIP_PLANES)
                        .copied()
                        .collect(),
                    fog: item.fog.as_ref().or(frame.fog.as_ref()),
                });
                calls.push(Call {
                    key: PipelineKey::new(Topology::Triangles, Cull::Back, true, true, false, true),
                    blend_constant: None,
                    style_texture: None,
                    source: Source::Volume(volume_inputs.len() - 1),
                    range: (0, 0),
                    count: 0,
                    instances: 1,
                    draw_index: 0,
                    textures: ([None; SLOTS], false),
                    viewport: item.viewport,
                });
                continue;
            }

            let g = &item.geometry;
            let gp = Rc::as_ptr(g);

            // One vertex buffer per geometry, kept while the viewer keeps handing out the same
            // allocation (it hands out a new one when the geometry is rebuilt).
            let sprites = matches!(item.style, Some(StyleParams::Point(_))) && g.geometry_type == 0;
            let cache = if sprites {
                &mut self.sprites
            } else {
                &mut self.geometries
            };
            let count = match cache.get_mut(&gp) {
                Some(e) => {
                    e.used = frame_no;
                    e.count
                }
                None => {
                    let verts = if sprites {
                        sprite_vertices(g)
                    } else {
                        geometry_vertices(g)
                    };
                    let count = (verts.len() / FLOATS_PER_VERTEX) as u32;
                    let buffer = vertex_buffer(
                        &self.device,
                        &self.queue,
                        &mut self.free_vertices,
                        &mut self.stats,
                        &verts,
                        particle_kind(item.instances.as_deref(), g),
                    );
                    cache.insert(
                        gp,
                        GeoGpu {
                            geometry: g.clone(),
                            buffer,
                            count,
                            used: frame_no,
                        },
                    );
                    self.stats.geometry_uploads += 1;
                    count
                }
            };

            let mv = f32x16(&item.model_view);
            let normal = normal_matrix(&mv);
            let mut d: DrawU = bytemuck::Zeroable::zeroed();
            d.model_view = mv;

            if self.scatter_pass {
                scatter_id += 1;
            }

            // The shape's global and local lights.
            let lights: Vec<u32> = item
                .lights
                .iter()
                .copied()
                .filter(|l| (*l as usize) < MAX_FRAME_LIGHTS)
                .take(MAX_LIGHTS)
                .collect();
            d.light_count[0] = lights.len() as u32;
            for (i, l) in lights.iter().enumerate() {
                d.light_index[i / 4][i % 4] = *l;
            }

            // Texture2.glsl: the units in slots, their matrices, coordinate channels and
            // generators, and the MultiTexture parameters.
            let projector_textures: Vec<Rc<TextureImage>> = item
                .projectors
                .iter()
                .filter_map(|p| {
                    frame
                        .projectors
                        .get(*p as usize)
                        .filter(|_| (*p as usize) < MAX_FRAME_PROJECTORS)
                })
                .map(|p| p.texture.clone())
                .collect();
            let (slot_key, bindings, _, projector_bindings) =
                assign_slots(&item.textures, &item.material.textures, &projector_textures);

            for t in item
                .textures
                .iter()
                .take(MAX_TEXTURES)
                .chain(item.material.textures.iter().map(|m| &m.texture))
                .chain(projector_textures.iter())
            {
                used_textures.push(t.clone());
            }

            // X3D_TEXTURE_PROJECTION.
            let projectors: Vec<u32> = item
                .projectors
                .iter()
                .copied()
                .filter(|p| {
                    (*p as usize) < MAX_FRAME_PROJECTORS && (*p as usize) < frame.projectors.len()
                })
                .take(MAX_TEXTURES)
                .collect();
            d.projector_count[0] = projectors.len() as u32;
            for (i, p) in projectors.iter().enumerate() {
                d.projector_index[i] = *p;
                d.projector_slot[i] = projector_bindings[i];
            }

            if let Some(b) = &item.back_material {
                used_textures.extend(b.textures.iter().map(|m| m.texture.clone()));
            }

            for (i, t) in item.textures.iter().take(MAX_TEXTURES).enumerate() {
                let binding = bindings[i];
                let (mode, alpha_mode, source, function) = match &item.multi_texture {
                    Some(m) => m
                        .units
                        .get(i)
                        .map(|u| (u[0], u[1], u[2], u[3]))
                        .unwrap_or((1, 1, 0, 0)),
                    None => (1, 1, 0, 0),
                };
                d.tex_units[i] = [binding, t.flip_y as u32, mode, alpha_mode];
                d.tex_units2[i] = [source, function, 0, 0];
            }

            for (i, m) in item.texture_matrices.iter().take(4).enumerate() {
                d.texture_matrices[i] = *m;
            }

            for (i, c) in g.tex_channels.iter().take(4).enumerate() {
                let p = &c.parameter;
                d.tex_gen[2 * i] = [c.mode as f32, p[0], p[1], p[2]];
                d.tex_gen[2 * i + 1] = [p[3], p[4], p[5], 0.0];
            }

            d.multi_color = item
                .multi_texture
                .as_ref()
                .map(|m| m.color.map(|v| v as f32))
                .unwrap_or([1.0; 4]);
            // X3D_NUM_TEXTURE_COORDINATES: geometryContext .textureCoordinateMapping .size, a Map keyed by the channel's
            // mapping name or else its number (X3DSingleTextureCoordinateNode.getTextureCoordinateMapping).
            let mut coordinate_keys: Vec<(bool, String)> = vec![];
            for (i, c) in g.tex_channels.iter().take(4).enumerate() {
                let key = if c.mapping.is_empty() {
                    (false, i.to_string())
                } else {
                    (true, c.mapping.clone())
                };

                if !coordinate_keys.contains(&key) {
                    coordinate_keys.push(key);
                }
            }
            d.tex_counts = [
                item.textures.len().min(MAX_TEXTURES) as u32,
                item.multi_texture.is_some() as u32,
                coordinate_keys.len().clamp(1, 4) as u32,
                item.texture_matrices.len().clamp(1, 4) as u32,
            ];
            d.n0 = [normal[0], normal[1], normal[2], 0.0];
            d.n1 = [normal[3], normal[4], normal[5], 0.0];
            d.n2 = [normal[6], normal[7], normal[8], 0.0];
            // X3DMaterialNode.addRenderOptions: X3D_ALPHA_MODE_BLEND (0), _OPAQUE (1) or _MASK (2), and
            // x3d_AlphaCutoff (Appearance.getAlphaCutoff) for MASK. The item's mode is opaque or masked exactly
            // when it is drawn with the opaque shapes.
            let alpha_mode = match item.alpha_mode {
                AlphaMode::Opaque | AlphaMode::Auto => 1,
                AlphaMode::Mask => 2,
                AlphaMode::Blend => 0,
            };
            debug_assert_eq!(is_transparent, alpha_mode == 0);
            d.flags2 = [0, alpha_mode, g.geometry_type as u32, 0];
            d.depth_range[2] = item.alpha_cutoff;

            let n_clip = item.clip_planes.len().min(MAX_CLIP_PLANES);
            d.flags3[0] = n_clip as u32;
            for (i, p) in item.clip_planes.iter().take(n_clip).enumerate() {
                d.clip_planes[i] = [p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32];
            }

            match item.style {
                Some(StyleParams::Point(p)) => {
                    d.style = [1, 0, 0, 0];
                    d.style_a = [
                        p.scale_factor as f32,
                        p.min_value as f32,
                        p.max_value as f32,
                        0.0,
                    ];
                    d.style_b = [
                        p.attenuation[0] as f32,
                        p.attenuation[1] as f32,
                        p.attenuation[2] as f32,
                        0.0,
                    ];
                }
                Some(StyleParams::Line(l)) => {
                    // LineProperties.setShaderUniforms: lineStippleScale = 1 / (PixelsPerPoint × ContentScale × 32).
                    d.style = [2, l.linetype as u32, 0, 0];
                    d.style_a = [
                        (1.0 / (PIXELS_PER_POINT * 32.0)) as f32,
                        l.width as f32,
                        0.0,
                        0.0,
                    ];
                }
                Some(StyleParams::Fill(f)) => {
                    d.style = [3, f.filled as u32, f.hatched as u32, 0];
                    d.style_a = [
                        f.hatch_color[0] as f32,
                        f.hatch_color[1] as f32,
                        f.hatch_color[2] as f32,
                        0.0,
                    ];
                }
                None => {}
            }

            if let Some(dm) = &item.depth {
                d.flags3[1] = 1;
                d.depth_range[0] = dm.range[0] as f32;
                d.depth_range[1] = dm.range[1] as f32;
            }

            // FogContainer.setShaderUniforms: the draw's fog is the innermost LocalFog, else the bound Fog;
            // X3D_FOG_COORDS when the geometry has fog depths.
            if let Some(f) = item.fog.as_ref().or(frame.fog.as_ref()) {
                d.fog_color_type = [f.color[0], f.color[1], f.color[2], f.fog_type as f32];
                d.fog_range = [f.visibility_start, f.visibility_range, 0.0, 0.0];
                d.fog_m0 = [f.matrix[0], f.matrix[1], f.matrix[2], 0.0];
                d.fog_m1 = [f.matrix[3], f.matrix[4], f.matrix[5], 0.0];
                d.fog_m2 = [f.matrix[6], f.matrix[7], f.matrix[8], 0.0];
            }

            d.fog_flags[0] = (!g.fog_depths.is_empty()) as u32;

            let set_material = |d: &mut DrawU, m: &MaterialParams| {
                d.diffuse_ambient = [
                    m.diffuse[0] as f32,
                    m.diffuse[1] as f32,
                    m.diffuse[2] as f32,
                    m.ambient_intensity as f32,
                ];
                d.specular_shininess = [
                    m.specular[0] as f32,
                    m.specular[1] as f32,
                    m.specular[2] as f32,
                    m.shininess as f32,
                ];
                d.emissive_transparency = [
                    m.emissive[0] as f32,
                    m.emissive[1] as f32,
                    m.emissive[2] as f32,
                    m.transparency as f32,
                ];
                let lit = m.shading != Shading::Unlit;
                let has_normals = !g.normals.is_empty();
                // X3D_LIGHTING: lights and normals (the physical shaders run without normals, too).
                let num_lights = d.light_count[0];
                let lighting = if m.pbr != 0 {
                    num_lights > 0 && has_normals
                } else {
                    lit && num_lights > 0
                };
                d.flags = [
                    lit as u32,
                    lighting as u32,
                    (!g.colors.is_empty()) as u32,
                    !item.textures.is_empty() as u32,
                ];
                // ColorSpace: LINEAR_WHEN_PHYSICAL_MATERIAL by default.
                let linear = match frame.color_space {
                    0 => false,
                    1 => true,
                    _ => m.pbr != 0,
                };
                d.pbr = [
                    m.metallic as f32,
                    m.roughness as f32,
                    m.normal_scale as f32,
                    m.occlusion_strength as f32,
                ];
                d.pbr_flags = [
                    m.pbr,
                    linear as u32,
                    (!g.tangents.is_empty()) as u32,
                    has_normals as u32,
                ];
                let (_, _, mat_tex, _) =
                    assign_slots(&item.textures, &m.textures, &projector_textures);
                d.mat_tex = mat_tex;
                // PhysicalMaterial extensions (X3DMaterialExtensionNode.setShaderUniforms).
                d.ext_flags = [0; 4];
                d.ext = [[0.0; 4]; 10];
                if let Some(x) = m.extensions.as_deref().filter(|_| m.pbr == 1) {
                    let f = |v: f64| v as f32;
                    d.ext_flags[0] = x.flags;
                    d.ext[0] = [
                        f(x.emissive_strength),
                        f(x.ior),
                        f(x.specular),
                        f(x.specular_strength),
                    ];
                    d.ext[1] = [
                        f(x.specular_color[0]),
                        f(x.specular_color[1]),
                        f(x.specular_color[2]),
                        f(x.clearcoat),
                    ];
                    d.ext[2] = [
                        f(x.sheen_color[0]),
                        f(x.sheen_color[1]),
                        f(x.sheen_color[2]),
                        f(x.sheen_roughness),
                    ];
                    d.ext[3] = [
                        f(x.clearcoat_roughness),
                        f(x.iridescence),
                        f(x.iridescence_ior),
                        f(x.iridescence_thickness[0]),
                    ];
                    d.ext[4] = [
                        f(x.iridescence_thickness[1]),
                        f(x.transmission),
                        f(x.thickness),
                        f(x.attenuation_distance),
                    ];
                    d.ext[5] = [
                        f(x.attenuation_color[0]),
                        f(x.attenuation_color[1]),
                        f(x.attenuation_color[2]),
                        f(x.diffuse_transmission),
                    ];
                    d.ext[6] = [
                        f(x.diffuse_transmission_color[0]),
                        f(x.diffuse_transmission_color[1]),
                        f(x.diffuse_transmission_color[2]),
                        0.0,
                    ];
                    d.ext[7] = [
                        f(x.anisotropy[0]),
                        f(x.anisotropy[1]),
                        f(x.anisotropy[2]),
                        0.0,
                    ];
                    d.ext[8] = [f(x.dispersion), f(x.retroreflection), 0.0, 0.0];
                    d.ext[9] = [
                        f(x.multiscatter_color[0]),
                        f(x.multiscatter_color[1]),
                        f(x.multiscatter_color[2]),
                        scatter_id as f32,
                    ];
                }
                // X3D_PHONG_MATERIAL: getMaterialKey 2, or a legacy Material with the Shading PHONG.
                d.flags3[2] = (m.shading == Shading::Phong
                    || (m.shading == Shading::Gouraud && shading_phong))
                    as u32;
            };

            let push_draw =
                |d: DrawU,
                 draws: &mut Vec<DrawU>,
                 debug_draws: &mut Vec<Option<(NodeId, Matrix4)>>| {
                    let mut d = d;
                    if self.debug_ids {
                        d.diffuse_ambient[0] = (draws.len() + 1) as f32;
                        debug_draws.resize(draws.len() + 1, None);
                        debug_draws[draws.len()] = Some((item.geometry_node, item.model_view));
                    }
                    draws.push(d);
                    draws.len() - 1
                };

            let topology = match g.geometry_type {
                0 => Topology::Points,
                1 => Topology::Lines,
                _ => Topology::Triangles,
            };

            // X3DGeometryNode.display: draw front and back faces with their own materials when
            // the geometry is not solid and the appearance has a back material.
            let two_sided =
                topology == Topology::Triangles && !g.solid && item.back_material.is_some();
            // InstancedShape: one draw per instance (the instance matrix is applied in the vertex
            // shader, X3D_INSTANCING); each pass below draws all instances in turn, as
            // gl.drawArraysInstanced does.
            let mut back_draws: Vec<usize> = vec![];
            let mut front_draws: Vec<usize> = vec![];
            // X3D_INSTANCING: one draw of every instance (gl.drawArraysInstanced), their matrices, normal matrices,
            // colors and ramp corners in the instance buffer. A ComposedShader still draws once per instance.
            let num_instances = item.instances.as_ref().map_or(1, |i| i.matrices.len());
            let mut call_instances = if item.custom.is_some() {
                1
            } else {
                num_instances as u32
            };
            let mut di = d;
            let particle = item.instances.as_ref().is_some_and(|i| i.particle);

            if let Some(i) = &item.instances {
                if i.particle {
                    particle_instances += i.matrices.len();
                }

                di.inst_flags = [
                    1,
                    !i.colors.is_empty() as u32,
                    if i.tex_all {
                        2
                    } else {
                        !i.tex_coords.is_empty() as u32
                    },
                    instance_data.len() as u32,
                ];

                for (k, m) in i.matrices.iter().enumerate() {
                    let n = i.normals[k];
                    instance_data.push(InstanceU {
                        matrix: f32x16(m),
                        n0: [n[0], n[1], n[2], 0.0],
                        n1: [n[3], n[4], n[5], 0.0],
                        n2: [n[6], n[7], n[8], 0.0],
                        color: i.colors.get(k).copied().unwrap_or_default(),
                        tex: i.tex_coords.get(k).copied().unwrap_or_default(),
                    });
                }
            }

            let repeat = if item.custom.is_some() {
                num_instances
            } else {
                1
            };

            if let Some(b) = item.back_material.as_ref().filter(|_| two_sided) {
                let mut bd = di;
                set_material(&mut bd, b);
                let index = push_draw(bd, &mut draws, &mut self.debug_draws);
                back_draws.extend(std::iter::repeat_n(index, repeat));
                particle_draws += particle as usize;
            }

            set_material(&mut di, &item.material);
            let index = push_draw(di, &mut draws, &mut self.debug_draws);
            front_draws.extend(std::iter::repeat_n(index, repeat));
            particle_draws += particle as usize;

            let back_index = back_draws.first().copied();

            let style_tex = item.style_texture.as_ref().map(|t| {
                used_styles.push(t.clone());
                Rc::as_ptr(t)
            });

            // X3DGeometryNode.display: frontFace from ccw, swapped for a negative determinant.
            let positive_scale = determinant3(&mv) >= 0.0;
            let ccw = if positive_scale { g.ccw } else { !g.ccw };
            // Point sprites are drawn as triangles.
            let topology = if sprites {
                Topology::Triangles
            } else {
                topology
            };
            let mut base = PipelineKey::new(
                topology,
                Cull::None,
                ccw,
                is_transparent,
                !is_transparent,
                true,
            );
            base.alpha_to_coverage = g.geometry_type == 0 && !is_transparent;
            base.volume = slot_key.1;

            // getPrimitiveMode: POINT draws lines and triangles as points; WIREFRAME draws
            // triangles with the polygon mode LINE. Point sprites are GL points in X_ITE.
            if !sprites {
                if points {
                    base.topology = Topology::Points;
                } else if wireframe && topology == Topology::Triangles {
                    base.polygon_line = true;
                }
            }
            let mut blend_constant = None;

            // BlendMode.enable, DepthMode.enable around the draw.
            if let Some(b) = &item.blend {
                let (state, constant) = blend_state(b);
                base.blend_mode = Some(state);
                blend_constant = Some(constant);
            }

            if let Some(dm) = &item.depth {
                base.depth_write = dm.mask;

                // gl.disable (DEPTH_TEST) also stops depth writes.
                if dm.test {
                    base.depth_func = Some(compare_function(dm.func));
                } else {
                    base.depth_test = false;
                    base.depth_write = false;
                }

                // POLYGON_OFFSET_FILL: polygons only.
                if topology == Topology::Triangles {
                    base.bias = (
                        dm.polygon_offset[1].round() as i32,
                        (dm.polygon_offset[0] as f32).to_bits(),
                    );
                }
            }

            let mut source = if sprites {
                Source::Sprites(gp)
            } else {
                Source::Geometry(gp)
            };
            let mut count = count;
            let mut range = (0, 0);
            // Instanced draws: the scratch range of each instance (front_draws and back_draws order).
            let mut instance_ranges: Vec<(u64, u64)> = vec![];

            // LineProperties: stippled or wide lines with the per-frame x3d_LineStipple attribute
            // (X3DLineGeometryNode.updateLengthSoFar, gl.lineWidth, displayThick).
            if let Some(StyleParams::Line(l)) = item.style.filter(|_| g.geometry_type == 1) {
                if l.linetype != 1 || l.width > 1.0 {
                    // POINT: gl.lineWidth does not apply to points, transformed lines are drawn
                    // with TRIANGLES -> POINTS.
                    let l = if points && l.width <= MAX_LINE_WIDTH {
                        LineParams { width: 1.0, ..l }
                    } else {
                        l
                    };
                    // Window coordinates of the shape's gl.viewport (gl_FragCoord of the stipple).
                    let r = item.viewport.unwrap_or(layer_rect);
                    let mut style_vertices = |instance: Option<&Matrix4>| {
                        let (verts, triangles) = line_style_vertices(
                            g,
                            &item.model_view,
                            &frame.projection,
                            r,
                            &l,
                            samples,
                            instance,
                        );
                        let start = scratch_data.len() as u64 * 4;
                        scratch_data.extend_from_slice(&verts);
                        if particle {
                            particle_scratch += verts.len();
                        }
                        (
                            (start, scratch_data.len() as u64 * 4),
                            (verts.len() / STIPPLE_FLOATS_PER_VERTEX) as u32,
                            triangles,
                        )
                    };
                    // X3DLineGeometryNode.displayInstanced: every instance has its own segments in window space, one
                    // after the other in the scratch buffer: one draw of all of them, the instance of a vertex is
                    // gl_VertexID / the vertices of one (inst_params.x).
                    let mut computed = None;
                    let triangles = match &item.instances {
                        // displayInstancedThick of the transformed lines: their vertices from a compute pass.
                        Some(i)
                            if !i.matrices.is_empty()
                                && l.width > MAX_LINE_WIDTH
                                && !points
                                && item.custom.is_none() =>
                        {
                            let (buffer, per) = self.thick_display_vertices(
                                encoder,
                                g,
                                &item.model_view,
                                &frame.projection,
                                r,
                                &l,
                                &i.matrices,
                            );
                            for &d in front_draws.iter().chain(&back_draws) {
                                draws[d].inst_params[0] = per;
                            }
                            computed = Some(Source::Buffer(buffer));
                            count = per * i.matrices.len() as u32;
                            call_instances = 1;
                            true
                        }
                        Some(i) if !i.matrices.is_empty() => {
                            let mut triangles = false;
                            for m in &i.matrices {
                                let (r, c, t) = style_vertices(Some(m));
                                instance_ranges.push(r);
                                (count, triangles) = (c, t);
                            }
                            range = instance_ranges[0];

                            if item.custom.is_none() {
                                for &d in front_draws.iter().chain(&back_draws) {
                                    draws[d].inst_params[0] = count;
                                }
                                range = (range.0, instance_ranges.last().unwrap().1);
                                count *= i.matrices.len() as u32;
                                call_instances = 1;
                                instance_ranges.clear();
                            }
                            triangles
                        }
                        _ => {
                            let (r, c, t) = style_vertices(None);
                            (range, count) = (r, c);
                            t
                        }
                    };
                    source = computed.unwrap_or(Source::Scratch);
                    base.stipple = true;
                    base.topology = if points {
                        Topology::Points
                    } else if triangles {
                        Topology::Triangles
                    } else {
                        Topology::Lines
                    };
                    // WIREFRAME: only the transformed thick lines are triangles in X_ITE.
                    base.polygon_line = wireframe && l.width > MAX_LINE_WIDTH;
                    base.bias = (0, 0);
                }
            }

            if count == 0 {
                continue;
            }

            // POLYGON_OFFSET_FILL: not for points, nor for the polygon mode LINE.
            if base.topology != Topology::Triangles || base.polygon_line {
                base.bias = (0, 0);
            }

            let calls_before = calls.len();
            // `instance`: the instance of the draw (its position in front_draws or back_draws), whose scratch range
            // it draws when the instances have their own.
            let call = |key: PipelineKey, draw_index: usize, instance: usize| Call {
                key,
                blend_constant,
                style_texture: style_tex,
                source,
                range: instance_ranges.get(instance).copied().unwrap_or(range),
                count,
                instances: call_instances,
                draw_index,
                textures: slot_key,
                viewport: item.viewport,
            };

            if topology != Topology::Triangles || sprites {
                for (instance, &i) in front_draws.iter().enumerate() {
                    calls.push(call(base, i, instance));
                }
            } else if back_index.is_some() {
                // displayGeometry (back shader, back only), then (front shader, front only); the
                // back material's own material textures.
                let back_slots = assign_slots(
                    &item.textures,
                    &item.back_material.as_ref().unwrap().textures,
                    &projector_textures,
                )
                .0;
                for (instance, &bi) in back_draws.iter().enumerate() {
                    calls.push(Call {
                        textures: back_slots,
                        ..call(
                            PipelineKey {
                                cull: Cull::Front,
                                ..base
                            },
                            bi,
                            instance,
                        )
                    });
                }
                for (instance, &i) in front_draws.iter().enumerate() {
                    calls.push(call(
                        PipelineKey {
                            cull: Cull::Back,
                            ..base
                        },
                        i,
                        instance,
                    ));
                }
            } else if is_transparent {
                if !g.solid {
                    for (instance, &i) in front_draws.iter().enumerate() {
                        calls.push(call(
                            PipelineKey {
                                cull: Cull::Front,
                                ..base
                            },
                            i,
                            instance,
                        ));
                    }
                }
                for (instance, &i) in front_draws.iter().enumerate() {
                    calls.push(call(
                        PipelineKey {
                            cull: Cull::Back,
                            ..base
                        },
                        i,
                        instance,
                    ));
                }
            } else {
                let cull = if g.solid { Cull::Back } else { Cull::None };
                for (instance, &i) in front_draws.iter().enumerate() {
                    calls.push(call(PipelineKey { cull, ..base }, i, instance));
                }
            }

            // ComposedShader (Batch 08): the calls of this item draw with the program of its shader.
            if let Some(cd) = &item.custom {
                used_textures.extend(cd.user_textures.iter().flatten().cloned());

                for c in &mut calls[calls_before..] {
                    let k = c.key;
                    let back = back_draws.contains(&c.draw_index);
                    let by_geometry = sprites || k.stipple;
                    custom_inputs.push(crate::shader_gpu::CustomInput {
                        draw: &**cd,
                        item,
                        frame,
                        back,
                        state: crate::shader_gpu::DrawState {
                            topology: if by_geometry {
                                match g.geometry_type {
                                    0 => 2,
                                    1 => 1,
                                    _ => 0,
                                }
                            } else {
                                match k.topology {
                                    Topology::Triangles => 0,
                                    Topology::Lines => 1,
                                    Topology::Points => 2,
                                }
                            },
                            cull: match k.cull {
                                Cull::None => 0,
                                Cull::Front => 1,
                                Cull::Back => 2,
                            },
                            ccw: k.ccw,
                            blend: k.blend,
                            blend_mode: k.blend_mode,
                            depth_write: k.depth_write,
                            depth_test: k.depth_test,
                            depth_func: k.depth_func,
                            bias: k.bias,
                        },
                        target: (width, height),
                        layer_rect,
                        projection: f32x16(&projection),
                        normal_matrix: normal,
                        flip,
                        reversed: self.depth_mode == DepthMode::Reversed,
                    });
                    c.source = Source::Custom(custom_inputs.len() - 1, back);
                }
            } else if calls.len() > calls_before && !item.textures.is_empty() {
                builtin_draws.push((custom_inputs.len(), item.textures.clone()));
            }
        }

        // --- GPU resources that depend on the frame
        // Textures: uploaded once each, kept while used; a bind group per slot combination.
        for t in used_textures {
            let p = Rc::as_ptr(&t);

            if let Some(key) = t.generated {
                let view = if t.cube {
                    self.generated_texture(key, t.width).view.clone()
                } else {
                    self.rendered_gpu(key, t.width, t.height, t.float)
                        .view
                        .clone()
                };
                if !self.textures.contains_key(&p) {
                    let sampler = Self::make_sampler(&self.device, &t, 1);
                    self.textures.insert(
                        p,
                        TexGpu {
                            _image: t,
                            view,
                            sampler,
                            used: frame_no,
                        },
                    );
                }
                self.textures.get_mut(&p).unwrap().used = frame_no;
                continue;
            }

            match self.textures.get_mut(&p) {
                Some(e) => e.used = frame_no,
                None => {
                    let (view, sampler) = Self::upload_texture(&self.device, &self.queue, &t);
                    self.textures.insert(
                        p,
                        TexGpu {
                            _image: t,
                            view,
                            sampler,
                            used: frame_no,
                        },
                    );
                    self.stats.texture_uploads += 1;
                }
            }
        }

        for t in used_styles {
            let p = Rc::as_ptr(&t);

            match self.style_groups.get_mut(&p) {
                Some(e) => e.2 = frame_no,
                None => {
                    let group = Self::make_texture_group(
                        &self.device,
                        &self.queue,
                        &self.texture_layout,
                        &t,
                    );
                    self.style_groups.insert(p, (t, group, frame_no));
                }
            }
        }

        for c in calls.iter().filter(|c| {
            !matches!(
                c.source,
                Source::Volume(_) | Source::Splats(_) | Source::Custom(..)
            )
        }) {
            match self.slot_groups.get_mut(&c.textures) {
                Some(e) => e.1 = frame_no,
                None => {
                    let volume = c.textures.1;
                    let resources: Vec<(&wgpu::TextureView, &wgpu::Sampler)> = c
                        .textures
                        .0
                        .iter()
                        .enumerate()
                        .map(|(i, t)| {
                            match (
                                t.and_then(|p| self.textures.get(&p)),
                                slot_dimension(i, volume),
                            ) {
                                (Some(e), _) => (&e.view, &e.sampler),
                                (None, wgpu::TextureViewDimension::D2) => {
                                    (&self.white_2d.0, &self.white_2d.1)
                                }
                                (None, wgpu::TextureViewDimension::D3) => {
                                    (&self.white_3d.0, &self.white_3d.1)
                                }
                                (None, _) => (&self.white_cube.0, &self.white_cube.1),
                            }
                        })
                        .collect();
                    let entries: Vec<wgpu::BindGroupEntry> = resources
                        .iter()
                        .enumerate()
                        .flat_map(|(i, (view, sampler))| {
                            [
                                wgpu::BindGroupEntry {
                                    binding: 2 * i as u32,
                                    resource: wgpu::BindingResource::TextureView(view),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 2 * i as u32 + 1,
                                    resource: wgpu::BindingResource::Sampler(sampler),
                                },
                            ]
                        })
                        .collect();
                    let layout = if volume {
                        &self.slots_layout_3d
                    } else {
                        &self.slots_layout
                    };
                    let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("texture slots"),
                        layout,
                        entries: &entries,
                    });
                    self.slot_groups.insert(c.textures, (group, frame_no));
                }
            }
        }

        // Draw uniforms and instances: one buffer each, grown when a frame needs more.
        let more_instances = instance_data.len() > self.instance_capacity;
        if more_instances {
            self.instance_capacity = instance_data.len().next_power_of_two();
            self.instance_buffer = Self::make_instance_buffer(&self.device, self.instance_capacity);
        }
        if draws.len() > self.draw_capacity || more_instances {
            self.draw_capacity = self.draw_capacity.max(draws.len().next_power_of_two());
            let (b, g) = Self::make_draw_buffer(
                &self.device,
                &self.draw_layout,
                self.draw_capacity,
                &self.instance_buffer,
            );
            self.draw_buffer = b;
            self.draw_group = g;
        }
        if !instance_data.is_empty() {
            self.queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&instance_data),
            );
            crate::transfer::split_upload(
                instance_data.len() * std::mem::size_of::<InstanceU>(),
                particle_instances * std::mem::size_of::<InstanceU>(),
            );
        }

        self.draw_bytes.clear();
        self.draw_bytes
            .resize(draws.len().max(1) * DRAW_STRIDE as usize, 0);
        for (i, d) in draws.iter().enumerate() {
            let b = bytemuck::bytes_of(d);
            self.draw_bytes[i * DRAW_STRIDE as usize..i * DRAW_STRIDE as usize + b.len()]
                .copy_from_slice(b);
        }
        self.queue
            .write_buffer(&self.draw_buffer, 0, &self.draw_bytes);
        crate::transfer::split_upload(self.draw_bytes.len(), particle_draws * DRAW_STRIDE as usize);

        // Scratch vertices (background): re-uploaded only when they changed.
        let scratch_bytes = scratch_data.len() as u64 * 4;

        if scratch_bytes > self.scratch_capacity {
            self.scratch_capacity = scratch_bytes.next_power_of_two();
            self.scratch = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("scratch"),
                size: self.scratch_capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.scratch_data.clear();
        }

        if scratch_bytes > 0 && scratch_data != self.scratch_data {
            self.queue
                .write_buffer(&self.scratch, 0, bytemuck::cast_slice(&scratch_data));
            crate::transfer::split_upload(scratch_data.len() * 4, particle_scratch * 4);
        }

        self.scratch_data = scratch_data;

        // Pipelines for the formats and states of this frame.
        for c in calls.iter().filter(|c| {
            !matches!(
                c.source,
                Source::Volume(_) | Source::Splats(_) | Source::Custom(..)
            )
        }) {
            if !self.pipelines.contains_key(&(c.key, format, samples, flip)) {
                let p = self.take_or_make_pipeline((c.key, format, samples, flip));
                self.pipelines.insert((c.key, format, samples, flip), p);
                self.stats.pipelines_created += 1;
            }
        }

        // VolumeRendering: programs, pipelines, uniforms and bind groups of the volumes.
        {
            let (textures, white_2d, white_3d) = (&self.textures, &self.white_2d, &self.white_3d);
            self.volume.prepare(
                &self.device,
                &self.queue,
                &self.frame_layout,
                format,
                samples,
                flip,
                self.depth_mode.depth_format(),
                self.depth_mode == DepthMode::Reversed,
                frame.color_space == 1,
                frame_no,
                &volume_inputs,
                &|p, three_d| match textures.get(&p) {
                    Some(e) => (&e.view, &e.sampler),
                    None if three_d => (&white_3d.0, &white_3d.1),
                    None => (&white_2d.0, &white_2d.1),
                },
            );
        }

        // GaussianSplats: data buffers, sorted indices, uniforms, pipelines and bind groups.
        self.splats.prepare(
            &self.device,
            &self.queue,
            &self.frame_layout,
            format,
            samples,
            self.depth_mode.depth_format(),
            self.depth_mode == DepthMode::Reversed,
            &fu.projection,
            frame_no,
            &splat_inputs,
        );

        // ComposedShader: programs, pipelines, vertex buffers, uniforms and bind groups.
        {
            let (textures, white_2d, white_3d, white_cube) = (
                &self.textures,
                &self.white_2d,
                &self.white_3d,
                &self.white_cube,
            );
            self.custom.prepare(
                &self.device,
                &self.queue,
                format,
                self.depth_mode.depth_format(),
                samples,
                frame_no,
                &custom_inputs,
                &builtin_draws,
                &|p| textures.get(&p).map(|e| (&e.view, &e.sampler)),
                [
                    (&white_2d.0, &white_2d.1),
                    (&white_3d.0, &white_3d.1),
                    (&white_cube.0, &white_cube.1),
                ],
            );
        }

        // Depth buffer sized to the target.
        let depth_format = self.depth_mode.depth_format();

        if face {
            if self.face_depth.as_ref().is_none_or(|d| {
                d.2 != width || d.0.height() != height || d.0.format() != depth_format
            }) {
                let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("face depth"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: depth_format,
                    // TEXTURE_BINDING: the volume scatter buffer's depth is read back.
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                let view = texture.create_view(&Default::default());
                self.face_depth = Some((texture, view, width));
            }
        } else if self.depth.as_ref().is_none_or(|d| {
            d.2 != width || d.3 != height || d.4 != samples || d.0.format() != depth_format
        }) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("depth"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: depth_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            self.depth = Some((texture, view, width, height, samples));
        }

        // MultiSampleFramebuffer: renderbufferStorageMultisample (RGBA8) next to the depth buffer.
        if face {
        } else if samples > 1
            && self
                .msaa_color
                .as_ref()
                .is_none_or(|m| m.2 != format || m.3 != width || m.4 != height || m.5 != samples)
        {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("msaa color"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            self.msaa_color = Some((texture, view, format, width, height, samples));
        } else if samples <= 1 {
            self.msaa_color = None;
        }

        // --- the pass
        {
            let depth_view = if face {
                &self.face_depth.as_ref().unwrap().1
            } else {
                &self.depth.as_ref().unwrap().1
            };
            let msaa_view = if samples > 1 {
                self.msaa_color.as_ref().map(|m| &m.1)
            } else {
                None
            };
            let timestamp_writes = self
                .timer
                .as_ref()
                .filter(|_| !face && !pass_target.more)
                .map(|t| wgpu::RenderPassTimestampWrites {
                    query_set: &t.queries,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                });
            let [r, g, b, a] = self.clear_color;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("x3d"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    // MultiSampleFramebuffer.blit: the multisampled color renderbuffer is resolved
                    // into the default framebuffer after drawShapes; the samples are not kept.
                    view: msaa_view.unwrap_or(target),
                    depth_slice: None,
                    resolve_target: msaa_view.map(|_| target),
                    // drawShapes: gl.clearColor (0, 0, 0, 0)
                    ops: wgpu::Operations {
                        load: if pass_target.load {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a })
                        },
                        store: if msaa_view.is_some() && !pass_target.more {
                            wgpu::StoreOp::Discard
                        } else {
                            wgpu::StoreOp::Store
                        },
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(if self.depth_mode == DepthMode::Reversed {
                            0.0
                        } else {
                            1.0
                        }),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes,
                ..Default::default()
            });

            pass.set_bind_group(0, &self.frame_group, &[]);
            let white_group = &self.white_group;
            // X3DGeometryNode.display: gl.viewport and gl.scissor per shape.
            let mut current: Option<[u32; 4]> = None;

            for c in &calls {
                let Some(rect) = target_rect(&c.viewport.unwrap_or(layer_rect), width, height)
                else {
                    continue;
                };

                if current != Some(rect) {
                    let [x, y, w, h] = rect;
                    // Without a Viewport the rectangle is the whole target: no change for a
                    // single layer, and a flipped texture pass covers it whole.
                    let y = if flip { height - y - h } else { y };
                    pass.set_viewport(x as f32, y as f32, w as f32, h as f32, 0.0, 1.0);
                    pass.set_scissor_rect(x, y, w, h);
                    current = Some(rect);
                }

                if let Source::Volume(i) = c.source {
                    self.volume.draw(&mut pass, i);
                    continue;
                }

                if let Source::Custom(i, _) = c.source {
                    self.custom.draw(&mut pass, i);
                    // The program's own uniforms replaced group 0: the frame group again for the next draw.
                    pass.set_bind_group(0, &self.frame_group, &[]);
                    continue;
                }

                if let Source::Splats(i) = c.source {
                    self.splats.draw(&mut pass, i);
                    continue;
                }

                pass.set_pipeline(&self.pipelines[&(c.key, format, samples, flip)]);
                if let Some([r, g, b, a]) = c.blend_constant {
                    pass.set_blend_constant(wgpu::Color { r, g, b, a });
                }
                pass.set_bind_group(
                    1,
                    &self.draw_group,
                    &[(c.draw_index as u64 * DRAW_STRIDE) as u32],
                );
                pass.set_bind_group(2, &self.slot_groups[&c.textures].0, &[]);
                let sg = c
                    .style_texture
                    .and_then(|p| self.style_groups.get(&p))
                    .map(|t| &t.1)
                    .unwrap_or(white_group);
                pass.set_bind_group(3, sg, &[]);

                match c.source {
                    Source::Scratch => {
                        pass.set_vertex_buffer(0, self.scratch.slice(c.range.0..c.range.1))
                    }
                    Source::Geometry(p) => {
                        pass.set_vertex_buffer(0, self.geometries[&p].buffer.slice(..))
                    }
                    Source::Sprites(p) => {
                        pass.set_vertex_buffer(0, self.sprites[&p].buffer.slice(..))
                    }
                    Source::Buffer(i) => pass.set_vertex_buffer(0, self.frame_buffers[i].slice(..)),
                    Source::Volume(_) | Source::Splats(_) | Source::Custom(..) => {
                        unreachable!("volumes, splats and custom shaders are drawn above")
                    }
                }

                pass.draw(0..c.count, 0..c.instances);
            }
        }

        self.stats.draw_calls = calls.len();
        self.calls = calls;

        if let Some(t) = self.timer.as_mut().filter(|_| !face && !pass_target.more) {
            // The previous readback must be finished (or skipped) before the buffer is reused.
            if t.pending.is_none() {
                encoder.resolve_query_set(&t.queries, 0..2, &t.resolve, 0);
                encoder.copy_buffer_to_buffer(&t.resolve, 0, &t.read, 0, 16);
                t.recorded = true;
            }
        }

        // Forget GPU resources the viewer stopped using.
        if frame_no % 30 == 0 {
            self.geometries
                .retain(|_, e| e.used + KEEP_FRAMES >= frame_no);
            self.sprites.retain(|_, e| e.used + KEEP_FRAMES >= frame_no);
            self.free_vertices
                .retain(|f| f.freed + FREE_KEEP_FRAMES >= frame_no);
            self.textures
                .retain(|_, e| e.used + KEEP_FRAMES >= frame_no);
            self.style_groups
                .retain(|_, e| e.2 + KEEP_FRAMES >= frame_no);
            self.generated
                .retain(|_, e| e.used + KEEP_FRAMES >= frame_no);
            self.rendered
                .retain(|_, e| e.used + KEEP_FRAMES >= frame_no);
            self.volume.prune(frame_no);
            self.splats.prune(frame_no);
            self.custom.prune(frame_no);

            if self
                .transmission
                .as_ref()
                .is_some_and(|t| t.used + KEEP_FRAMES < frame_no)
            {
                self.transmission = None;
            }

            if self
                .shadow
                .as_ref()
                .is_some_and(|e| e.used + KEEP_FRAMES < frame_no)
            {
                self.shadow = None;
            }

            if self
                .scatter
                .as_ref()
                .is_some_and(|e| e.used + KEEP_FRAMES < frame_no)
            {
                self.scatter = None;
                // The frame group still names the buffers: back to the dummies, with the environment's maps.
                let maps = match &self.env {
                    Some(e) => e.maps.clone(),
                    None => self.white_env.clone(),
                };
                self.frame_group = Self::make_frame_group(
                    &self.device,
                    &self.frame_layout,
                    &self.frame_buffer,
                    &maps,
                    &self.no_transmission,
                    &self.lut,
                    &self.shadow_dummy,
                    &self.scatter_dummy,
                );
            }

            if self
                .env
                .as_ref()
                .is_some_and(|e| e.used + KEEP_FRAMES < frame_no)
            {
                self.env = None;
            }
            // A slot group refers to texture views: drop it with any of its textures.
            let textures = &self.textures;
            self.slot_groups.retain(|k, e| {
                e.1 + KEEP_FRAMES >= frame_no
                    && k.0.iter().flatten().all(|p| textures.contains_key(p))
            });
        }

        Ok(())
    }
}

impl GpuTimer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<GpuTimer> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }

        Some(GpuTimer {
            queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("x3d timing"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("x3d timing"),
                size: 16,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            read: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("x3d timing"),
                size: 16,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            period_ns: queue.get_timestamp_period(),
            pending: None,
            last_ms: None,
            recorded: false,
        })
    }
}

/// Floats per vertex with the x3d_LineStipple attribute: start point x, y, lengthSoFar, 0.
const STIPPLE_FLOATS_PER_VERTEX: usize = FLOATS_PER_VERTEX + 4;

/// ALIASED_LINE_WIDTH_RANGE [1] of the oracle GPU (ANGLE on OpenGL, NVIDIA): wider lines are
/// transformed into triangles (LineProperties.transformLines), narrower ones use gl.lineWidth.
pub const MAX_LINE_WIDTH: f64 = 10.0;

/// The vertices of a line geometry with LineProperties, in the stipple layout. `projection` is the
/// GL projection, window coordinates have their origin at the lower left (gl_FragCoord).
///
/// * The x3d_LineStipple values of updateLengthSoFar (linetype other than 1, else zeros): the
///   segment's start point and the length so far in pixels (flat), and as midPoint the other end
///   of the segment at each end (smooth, in the texCoord's x and y).
/// * Width 1: the segments as lines.
/// * Up to ALIASED_LINE_WIDTH_RANGE: gl.lineWidth wide lines as GL rasterizes them, two triangles
///   per segment: without multisampling a parallelogram whose ends are the rounded width along the
///   minor axis, with multisampling a rectangle of the exact width.
/// * Wider: LineTransform2.vs pass 0, the rectangle with the near plane sign flip; its stipple
///   values are always the projected end points.
///
/// Like X_ITE, the triangle corners are unprojected back into the geometry's own space.
///
/// `instance` is the instance matrix of one instance of an instanced draw (X3DLineGeometryNode.displayInstanced):
/// the segment is projected as `x3d_InstanceMatrix * vertex` and the corners are unprojected with
/// `inverse (x3d_InstanceMatrix) * invModelViewProjectionMatrix` (LineTransform2.vs), so they are in the geometry's
/// own space again and the draw applies the instance matrix once. updateLengthSoFar ignores the instance matrix: the
/// length so far of the transformed lines comes from the geometry under the model view matrix alone, shared by every
/// instance. Instanced gl.LINES (width up to ALIASED_LINE_WIDTH_RANGE [1]) have no x3d_LineStipple attribute.
fn line_style_vertices(
    g: &crate::geometry::Geometry,
    model_view: &Matrix4,
    projection: &Matrix4,
    viewport: [f64; 4],
    l: &LineParams,
    samples: u32,
    instance: Option<&Matrix4>,
) -> (Vec<f32>, bool) {
    let [x0, y0, width, height] = viewport;
    let mut mvp = *model_view;
    mvp.mult_right(projection);
    let instance_mvp = instance.map(|m| {
        let mut m = *m;
        m.mult_right(&mvp);
        m
    });
    let mut inv = instance_mvp.unwrap_or(mvp);
    inv.inverse();

    let base = geometry_vertices(g);
    let vertex = |i: usize| &base[i * FLOATS_PER_VERTEX..(i + 1) * FLOATS_PER_VERTEX];

    // ViewVolume.projectPointMatrix
    let project_with = |mvp: &Matrix4, v: &[f32]| {
        let mut p = Vector4::new(v[0] as f64, v[1] as f64, v[2] as f64, 1.0);
        mvp.mult_vec_matrix4(&mut p);
        let d = 1.0 / (2.0 * p.w);
        [
            (p.x * d + 0.5) * width + x0,
            (p.y * d + 0.5) * height + y0,
            p.z * d + 0.5,
        ]
    };
    let unproject = |w: [f64; 3]| {
        let mut p = Vector4::new(
            (w[0] - x0) / width * 2.0 - 1.0,
            (w[1] - y0) / height * 2.0 - 1.0,
            w[2] * 2.0 - 1.0,
            1.0,
        );
        inv.mult_vec_matrix4(&mut p);
        [(p.x / p.w) as f32, (p.y / p.w) as f32, (p.z / p.w) as f32]
    };

    let stippled = l.linetype != 1;
    let thick = l.width > MAX_LINE_WIDTH;
    let triangles = l.width > 1.0;
    let mut out = Vec::new();
    let mut length_so_far = 0.0;

    let push = |out: &mut Vec<f32>,
                v: &[f32],
                position: Option<[f32; 3]>,
                mid: [f64; 2],
                stipple: [f64; 3]| {
        out.extend_from_slice(&position.unwrap_or([v[0], v[1], v[2]]));
        out.extend_from_slice(&v[3..6]);
        out.extend_from_slice(&[mid[0] as f32, mid[1] as f32, 0.0, 1.0]);
        out.extend_from_slice(&v[10..FLOATS_PER_VERTEX]);
        out.extend_from_slice(&[stipple[0] as f32, stipple[1] as f32, stipple[2] as f32, 0.0]);
    };

    for k in 0..base.len() / FLOATS_PER_VERTEX / 2 {
        let (v0, v1) = (vertex(2 * k), vertex(2 * k + 1));
        // The segment as drawn (with the instance matrix) and as updateLengthSoFar sees it (without).
        let (s0, s1) = (project_with(&mvp, v0), project_with(&mvp, v1));
        let (p0, p1) = match &instance_mvp {
            Some(m) => (project_with(m, v0), project_with(m, v1)),
            None => (s0, s1),
        };
        // gl.LINES read x3d_LineStipple as updateLengthSoFar wrote it, the transformed lines get the end points of
        // LineTransform2.vs and its x3d_LengthSoFar.
        let (e0, e1) = if thick { (p0, p1) } else { (s0, s1) };

        // updateLengthSoFar (not called for linetype 1, so the attribute stays zero). The gl.LINES of
        // displayInstanced do not enable x3d_LineStipple at all: it reads zero, and the stipple pattern is solid.
        let (mid0, mid1, stipple) = if thick || (stippled && instance.is_none()) {
            (
                [e1[0], e1[1]],
                [e0[0], e0[1]],
                [e0[0], e0[1], if stippled { length_so_far } else { 0.0 }],
            )
        } else {
            ([0.0; 2], [0.0; 2], [0.0; 3])
        };

        if stippled {
            length_so_far += ((s1[0] - s0[0]).powi(2) + (s1[1] - s0[1]).powi(2)).sqrt();
        }

        if !triangles {
            push(&mut out, v0, None, mid0, stipple);
            push(&mut out, v1, None, mid1, stipple);
            continue;
        }

        let (dx, dy) = (p1[0] - p0[0], p1[1] - p0[1]);

        let corners: [([f64; 2], [f64; 2]); 2] = if thick {
            // LineTransform2.vs: offset = perpendicular × linewidthScaleFactor / 2, with the sign
            // flipped for an end behind the near plane.
            let n = (dx * dx + dy * dy).sqrt();
            let (ox, oy) = (-dy / n * l.width / 2.0, dx / n * l.width / 2.0);
            let neg0 = if p0[2] > 1.0 { -1.0 } else { 1.0 };
            let neg1 = if p1[2] > 1.0 { -1.0 } else { 1.0 };
            // Triangle 1 (neg1): p0 + o, p0 - o, p1 - o. Triangle 2 (neg0): p0 + o, p1 - o, p1 + o.
            let a = [
                [p0[0] + ox * neg1, p0[1] + oy * neg1],
                [p0[0] - ox * neg1, p0[1] - oy * neg1],
                [p1[0] - ox * neg1, p1[1] - oy * neg1],
            ];
            let b = [
                [p0[0] + ox * neg0, p0[1] + oy * neg0],
                [p1[0] - ox * neg0, p1[1] - oy * neg0],
                [p1[0] + ox * neg0, p1[1] + oy * neg0],
            ];
            let ends = [(0, 0), (0, 1), (1, 2)];
            let ends2 = [(0, 0), (1, 1), (1, 2)];

            for (tri, ends) in [(a, ends), (b, ends2)] {
                for (end, i) in ends {
                    let (v, p, mid) = if end == 0 {
                        (v0, p0, mid0)
                    } else {
                        (v1, p1, mid1)
                    };
                    push(
                        &mut out,
                        v,
                        Some(unproject([tri[i][0], tri[i][1], p[2]])),
                        mid,
                        stipple,
                    );
                }
            }

            continue;
        } else if samples > 1 {
            // Multisample rasterization: a rectangle of the line width.
            let n = (dx * dx + dy * dy).sqrt();
            let (ox, oy) = (-dy / n * l.width / 2.0, dx / n * l.width / 2.0);
            ([p0[0] + ox, p0[1] + oy], [p0[0] - ox, p0[1] - oy]).into_corners(p1, ox, oy)
        } else {
            // Aliased wide lines: the minor axis column of the rounded width.
            let w = l.width.round().max(1.0) / 2.0;
            let (ox, oy) = if dx.abs() >= dy.abs() {
                (0.0, w)
            } else {
                (w, 0.0)
            };
            ([p0[0] + ox, p0[1] + oy], [p0[0] - ox, p0[1] - oy]).into_corners(p1, ox, oy)
        };

        // Two triangles: (0+, 0-, 1-), (0+, 1-, 1+).
        let [(a0, a1), (b0, b1)] = corners;
        let quad = [
            (v0, p0, mid0, a0),
            (v0, p0, mid0, a1),
            (v1, p1, mid1, b1),
            (v0, p0, mid0, a0),
            (v1, p1, mid1, b1),
            (v1, p1, mid1, b0),
        ];

        for (v, p, mid, c) in quad {
            push(
                &mut out,
                v,
                Some(unproject([c[0], c[1], p[2]])),
                mid,
                stipple,
            );
        }
    }

    (out, triangles)
}

/// LineProperties.getTransformLines (a width above ALIASED_LINE_WIDTH_RANGE [1]): the triangles X3DLineGeometryNode
/// draws instead of gl.LINES, also in the pointing pass (displaySimpleThick), as object-space positions, three per
/// triangle, two triangles per segment. `viewport` and `projection` are those of the DISPLAY pass that made them.
pub fn transformed_line_positions(
    g: &crate::geometry::Geometry,
    model_view: &Matrix4,
    projection: &Matrix4,
    viewport: [f64; 4],
    l: &LineParams,
) -> Vec<[f32; 4]> {
    let (verts, _) = line_style_vertices(g, model_view, projection, viewport, l, 1, None);

    verts
        .chunks_exact(STIPPLE_FLOATS_PER_VERTEX)
        .map(|v| [v[0], v[1], v[2], 1.0])
        .collect()
}

/// gl.lineWidth lines (a width up to ALIASED_LINE_WIDTH_RANGE [1]) of an instanced draw in a pass with its own
/// projection and viewport: the shadow pass (X3DRenderObject.depth) sets gl.lineWidth through
/// LineProperties.setShaderUniforms (measured, the collision depth buffer does not widen them). The aliased
/// rasterization's parallelograms (see line_style_vertices) as positions in the geometry's own space, six per segment,
/// for a draw that applies `instance` and `model_view` again.
pub fn aliased_line_positions(
    g: &crate::geometry::Geometry,
    model_view: &Matrix4,
    projection: &Matrix4,
    viewport: [f64; 4],
    width: f64,
    instance: Option<&Matrix4>,
) -> Vec<[f32; 4]> {
    let l = LineParams { linetype: 1, width };
    let (verts, _) = line_style_vertices(g, model_view, projection, viewport, &l, 1, instance);

    verts
        .chunks_exact(STIPPLE_FLOATS_PER_VERTEX)
        .map(|v| [v[0], v[1], v[2], 1.0])
        .collect()
}

/// GLSL `m * v` of a column-major float32 matrix.
pub(crate) fn mat_vec_f32(m: &[f32; 16], v: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|r| m[r] * v[0] + m[4 + r] * v[1] + m[8 + r] * v[2] + m[12 + r] * v[3])
}

/// GLSL `a * b` of column-major float32 matrices.
fn mat_mul_f32(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    std::array::from_fn(|i| {
        let (c, r) = (i / 4, i % 4);
        a[r] * b[c * 4]
            + a[4 + r] * b[c * 4 + 1]
            + a[8 + r] * b[c * 4 + 2]
            + a[12 + r] * b[c * 4 + 3]
    })
}

/// GLSL `inverse (mat4)` in float32: the adjugate over the determinant (the 2 × 2 sub-factor form).
fn inverse_f32(m: &[f32; 16]) -> [f32; 16] {
    let a = |c: usize, r: usize| m[c * 4 + r];
    let s0 = a(0, 0) * a(1, 1) - a(1, 0) * a(0, 1);
    let s1 = a(0, 0) * a(1, 2) - a(1, 0) * a(0, 2);
    let s2 = a(0, 0) * a(1, 3) - a(1, 0) * a(0, 3);
    let s3 = a(0, 1) * a(1, 2) - a(1, 1) * a(0, 2);
    let s4 = a(0, 1) * a(1, 3) - a(1, 1) * a(0, 3);
    let s5 = a(0, 2) * a(1, 3) - a(1, 2) * a(0, 3);
    let c5 = a(2, 2) * a(3, 3) - a(3, 2) * a(2, 3);
    let c4 = a(2, 1) * a(3, 3) - a(3, 1) * a(2, 3);
    let c3 = a(2, 1) * a(3, 2) - a(3, 1) * a(2, 2);
    let c2 = a(2, 0) * a(3, 3) - a(3, 0) * a(2, 3);
    let c1 = a(2, 0) * a(3, 2) - a(3, 0) * a(2, 2);
    let c0 = a(2, 0) * a(3, 1) - a(3, 0) * a(2, 1);
    let det = s0 * c5 - s1 * c4 + s2 * c3 + s3 * c2 - s4 * c1 + s5 * c0;
    let inv = 1.0 / det;
    let mut out = [0.0f32; 16];
    let mut set = |c: usize, r: usize, v: f32| out[c * 4 + r] = v * inv;

    set(0, 0, a(1, 1) * c5 - a(1, 2) * c4 + a(1, 3) * c3);
    set(0, 1, -a(0, 1) * c5 + a(0, 2) * c4 - a(0, 3) * c3);
    set(0, 2, a(3, 1) * s5 - a(3, 2) * s4 + a(3, 3) * s3);
    set(0, 3, -a(2, 1) * s5 + a(2, 2) * s4 - a(2, 3) * s3);
    set(1, 0, -a(1, 0) * c5 + a(1, 2) * c2 - a(1, 3) * c1);
    set(1, 1, a(0, 0) * c5 - a(0, 2) * c2 + a(0, 3) * c1);
    set(1, 2, -a(3, 0) * s5 + a(3, 2) * s2 - a(3, 3) * s1);
    set(1, 3, a(2, 0) * s5 - a(2, 2) * s2 + a(2, 3) * s1);
    set(2, 0, a(1, 0) * c4 - a(1, 1) * c2 + a(1, 3) * c0);
    set(2, 1, -a(0, 0) * c4 + a(0, 1) * c2 - a(0, 3) * c0);
    set(2, 2, a(3, 0) * s4 - a(3, 1) * s2 + a(3, 3) * s0);
    set(2, 3, -a(2, 0) * s4 + a(2, 1) * s2 - a(2, 3) * s0);
    set(3, 0, -a(1, 0) * c3 + a(1, 1) * c1 - a(1, 2) * c0);
    set(3, 1, a(0, 0) * c3 - a(0, 1) * c1 + a(0, 2) * c0);
    set(3, 2, -a(3, 0) * s3 + a(3, 1) * s1 - a(3, 2) * s0);
    set(3, 3, a(2, 0) * s3 - a(2, 1) * s1 + a(2, 2) * s0);
    out
}

/// X3DLineGeometryNode.displayInstancedThick of one instance: LineTransform2.vs pass 0 with X3D_INSTANCING, in
/// float32 as the DISPLAY pass's transform feedback computes it. `model_view` is renderContext .modelViewMatrix (a
/// Float32Array), the matrices are `Float32Array (modelViewMatrix × projection)` and its inverse taken in doubles,
/// `instance` is x3d_InstanceMatrix. Six positions per segment in the instance's own space (the draw applies
/// `instance` again): triangle 1 (p0 + o, p0 - o, p1 - o) with the sign of end 1, triangle 2 (p0 + o, p1 - o,
/// p1 + o) with that of end 0, where an end behind the near plane flips the offset.
pub fn instanced_thick_line_positions(
    g: &crate::geometry::Geometry,
    model_view: &Matrix4,
    projection: &Matrix4,
    viewport: [f64; 4],
    width: f64,
    instance: &[f32; 16],
) -> Vec<[f32; 4]> {
    let lines = thick_lines(g, model_view, projection, viewport, width, vec![]);
    let mut out = Vec::with_capacity(g.vertex_count() * 3);

    thick_line_instance(&ThickView::of(&lines), instance, &mut out);
    out
}

/// The inputs of [`instanced_thick_line_positions`] for every instance of a DISPLAY pass, without the triangles
/// ([`thick_line_positions`] or thick_lines.wgsl make them).
pub fn thick_lines(
    g: &crate::geometry::Geometry,
    model_view: &Matrix4,
    projection: &Matrix4,
    viewport: [f64; 4],
    width: f64,
    instances: Vec<[f32; 16]>,
) -> x3d_runtime::particles::ThickLines {
    let mut mvp = Matrix4(model_view.0.map(|v| v as f32 as f64));
    mvp.mult_right(projection);
    let mut inv = mvp;
    inv.inverse();

    x3d_runtime::particles::ThickLines {
        num_lines: g.vertex_count() / 2 * instances.len(),
        vertices: g.vertices[..g.vertex_count() / 2 * 8].to_vec(),
        mvp: mvp.0.map(|v| v as f32),
        inverse: inv.0.map(|v| v as f32),
        viewport: viewport.map(|v| v as f32),
        half_width: (width / 2.0) as f32,
        instances,
        positions: Default::default(),
    }
}

/// The triangles of `lines` (six positions per segment and instance, in instance order), made on the CPU on first
/// use, on half the cores for many instances.
pub fn thick_line_positions(lines: &x3d_runtime::particles::ThickLines) -> &[[f32; 4]] {
    lines.positions.get_or_init(|| {
        let view = &ThickView::of(lines);
        let per = lines.vertices.len() / 8 * 6;
        let threads = std::thread::available_parallelism()
            .map_or(1, |n| n.get() / 2)
            .clamp(1, lines.instances.len().div_ceil(256).max(1));
        let chunk = lines.instances.len().div_ceil(threads).max(1);
        let mut out = Vec::with_capacity(per * lines.instances.len());

        std::thread::scope(|scope| {
            let parts: Vec<_> = lines
                .instances
                .chunks(chunk)
                .map(|instances| {
                    scope.spawn(move || {
                        let mut out = Vec::with_capacity(per * instances.len());

                        for m in instances {
                            thick_line_instance(view, m, &mut out);
                        }
                        out
                    })
                })
                .collect();

            for part in parts {
                out.extend(part.join().unwrap());
            }
        });
        out
    })
}

/// The inputs of the triangles of a [`x3d_runtime::particles::ThickLines`] (which is not `Sync`).
struct ThickView<'a> {
    vertices: &'a [f32],
    mvp: [f32; 16],
    inverse: [f32; 16],
    viewport: [f32; 4],
    half_width: f32,
}

impl ThickView<'_> {
    fn of(lines: &x3d_runtime::particles::ThickLines) -> ThickView<'_> {
        ThickView {
            vertices: &lines.vertices,
            mvp: lines.mvp,
            inverse: lines.inverse,
            viewport: lines.viewport,
            half_width: lines.half_width,
        }
    }
}

/// [`instanced_thick_line_positions`] of one instance (thick_lines.wgsl `main` does the same on the GPU).
fn thick_line_instance(lines: &ThickView, instance: &[f32; 16], out: &mut Vec<[f32; 4]>) {
    let unproject_matrix = mat_mul_f32(&inverse_f32(instance), &lines.inverse);
    let (vp, half, mvp32) = (lines.viewport, lines.half_width, &lines.mvp);

    let project = |p: [f32; 4]| {
        let v = mat_vec_f32(mvp32, p);
        let w2 = 2.0 * v[3];
        let (x, y, z) = (v[0] / w2 + 0.5, v[1] / w2 + 0.5, v[2] / w2 + 0.5);
        [x * vp[2] + vp[0], y * vp[3] + vp[1], z]
    };
    let unproject = |x: f32, y: f32, z: f32| {
        let v = [
            (x - vp[0]) / vp[2] * 2.0 - 1.0,
            (y - vp[1]) / vp[3] * 2.0 - 1.0,
            z * 2.0 - 1.0,
            1.0,
        ];
        let v = mat_vec_f32(&unproject_matrix, v);
        [v[0] / v[3], v[1] / v[3], v[2] / v[3], 1.0]
    };

    for k in 0..lines.vertices.len() / 8 {
        let vertex = |i: usize| {
            let v = &lines.vertices[i * 4..i * 4 + 4];
            mat_vec_f32(instance, [v[0], v[1], v[2], v[3]])
        };
        let (p0, p1) = (project(vertex(2 * k)), project(vertex(2 * k + 1)));
        let neg0: f32 = if p0[2] > 1.0 { -1.0 } else { 1.0 };
        let neg1: f32 = if p1[2] > 1.0 { -1.0 } else { 1.0 };
        // normalize (): v * inversesqrt (dot (v, v)).
        let (dx, dy) = (p1[0] - p0[0], p1[1] - p0[1]);
        let r = 1.0 / (dx * dx + dy * dy).sqrt();
        let (ox, oy) = (-(dy * r) * half, dx * r * half);

        out.push(unproject(p0[0] + ox * neg1, p0[1] + oy * neg1, p0[2]));
        out.push(unproject(p0[0] - ox * neg1, p0[1] - oy * neg1, p0[2]));
        out.push(unproject(p1[0] - ox * neg1, p1[1] - oy * neg1, p1[2]));
        out.push(unproject(p0[0] + ox * neg0, p0[1] + oy * neg0, p0[2]));
        out.push(unproject(p1[0] - ox * neg0, p1[1] - oy * neg0, p1[2]));
        out.push(unproject(p1[0] + ox * neg0, p1[1] + oy * neg0, p1[2]));
    }
}

trait IntoCorners {
    fn into_corners(self, p1: [f64; 3], ox: f64, oy: f64) -> [([f64; 2], [f64; 2]); 2];
}

impl IntoCorners for ([f64; 2], [f64; 2]) {
    /// The corners at both ends: ((end 0 +, end 0 -), (end 1 +, end 1 -)).
    fn into_corners(self, p1: [f64; 3], ox: f64, oy: f64) -> [([f64; 2], [f64; 2]); 2] {
        [self, ([p1[0] + ox, p1[1] + oy], [p1[0] - ox, p1[1] - oy])]
    }
}

/// RenderingProperties PixelsPerPoint of the oracle page: a 10in element is 960 px, / 720.
const PIXELS_PER_POINT: f64 = 960.0 / 720.0;

/// Point sprites: each point as two triangles of six vertices with the same position, normal and
/// color; the corner (-1 or 1 per axis) goes in the texCoord's x and y (points read no texCoord
/// attribute, Vertex2.glsl), the vertex shader offsets it by the point size in pixels.
fn sprite_vertices(g: &crate::geometry::Geometry) -> Vec<f32> {
    let points = geometry_vertices(g);
    let mut out = Vec::with_capacity(points.len() * 6);

    for p in points.chunks_exact(FLOATS_PER_VERTEX) {
        for (cx, cy) in [
            (-1.0, -1.0),
            (1.0, -1.0),
            (1.0, 1.0),
            (-1.0, -1.0),
            (1.0, 1.0),
            (-1.0, 1.0),
        ] {
            out.extend_from_slice(&p[..6]);
            out.extend_from_slice(&[cx, cy, 0.0, 1.0]);
            out.extend_from_slice(&p[10..]);
        }
    }

    out
}

fn geometry_vertices(g: &crate::geometry::Geometry) -> Vec<f32> {
    let count = g.vertex_count();
    let mut out = Vec::with_capacity(count * FLOATS_PER_VERTEX);

    for i in 0..count {
        let v = &g.vertices[i * 4..i * 4 + 3];
        let nrm = if g.normals.len() >= (i + 1) * 3 {
            [g.normals[i * 3], g.normals[i * 3 + 1], g.normals[i * 3 + 2]]
        } else {
            [0.0; 3]
        };
        let t = if g.tex_coords.len() >= (i + 1) * 4 {
            [
                g.tex_coords[i * 4],
                g.tex_coords[i * 4 + 1],
                g.tex_coords[i * 4 + 2],
                g.tex_coords[i * 4 + 3],
            ]
        } else {
            [0.0, 0.0, 0.0, 1.0]
        };
        let c = if g.colors.len() >= (i + 1) * 4 {
            [
                g.colors[i * 4],
                g.colors[i * 4 + 1],
                g.colors[i * 4 + 2],
                g.colors[i * 4 + 3],
            ]
        } else {
            [1.0; 4]
        };
        out.extend_from_slice(&[
            v[0], v[1], v[2], nrm[0], nrm[1], nrm[2], t[0], t[1], t[2], t[3], c[0], c[1], c[2],
            c[3],
        ]);

        for k in 0..3 {
            match g.tex_coords_extra.get(k).filter(|e| e.len() >= (i + 1) * 4) {
                Some(e) => out.extend_from_slice(&e[i * 4..i * 4 + 4]),
                None => out.extend_from_slice(&[0.0, 0.0, 0.0, 1.0]),
            }
        }

        match g.tangents.get(i * 4..i * 4 + 4) {
            Some(tangent) => out.extend_from_slice(tangent),
            None => out.extend_from_slice(&[1.0, 0.0, 0.0, 1.0]),
        }

        // x3d_FogDepth (FogCoordinate; zero without).
        out.push(g.fog_depths.get(i).copied().unwrap_or(0.0));
    }

    out
}

/// Headless renderer: its own Vulkan device (the discrete GPU), an RGBA8 target and a CPU
/// readback. Used by `x3d-shot`, the oracle tools and the tests.
pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_info: wgpu::AdapterInfo,
    pub renderer: Renderer,
}

impl Gpu {
    /// Vulkan adapter, high performance (the discrete GPU), no surface.
    pub fn new() -> Result<Gpu, String> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::VULKAN;
        let instance = wgpu::Instance::new(desc);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        }))
        .map_err(|e| format!("no Vulkan adapter: {e}"))?;
        let adapter_info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("x3d-render"),
            required_features: Renderer::wanted_features(&adapter),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let mut renderer = Renderer::new(device.clone(), queue.clone());
        renderer.set_supported_samples(Renderer::supported_sample_counts(
            &adapter,
            &device,
            wgpu::TextureFormat::Rgba8Unorm,
        ));
        Ok(Gpu {
            device,
            queue,
            adapter_info,
            renderer,
        })
    }

    /// Render a frame; returns tightly packed RGBA8 rows, top row first.
    pub fn render(&mut self, frame: &Frame, width: u32, height: u32) -> Vec<u8> {
        self.renderer.render_to_rgba(frame, width, height)
    }

    /// Render without the readback and return the GPU time of the pass in milliseconds (None
    /// without timestamp support). For benchmarks.
    pub fn render_timed(&mut self, frame: &Frame, width: u32, height: u32) -> Option<f64> {
        self.renderer.render_offscreen_timed(frame, width, height)
    }
}

/// Near and far of a projection built by `Camera.perspective` (column-major: z row at 10 and 14,
/// w row -1 at 11) or `Camera.ortho`.
fn near_far(p: &Matrix4) -> (f64, f64) {
    let (c, d) = (p.0[10], p.0[14]);

    if p.0[11] == -1.0 {
        // c = -(f + n) / (f - n), d = -2 n f / (f - n)
        let n = d / (c - 1.0);
        (n, n * (c - 1.0) / (c + 1.0))
    } else {
        // c = -2 / (f - n), d = -(f + n) / (f - n)
        let (n, f) = ((d + 1.0) / c, (d - 1.0) / c);
        (n, f)
    }
}

/// Reversed-Z: rewrite the z row so that z / w in [0, 1] (WebGPU clip space, the shader's
/// 0.5 * (z + w) mapping is patched out) is 1 at the near plane and 0 at the far plane. The
/// `n f / (f - n)` term carries the depth, so a 32-bit float keeps its relative precision.
fn reverse_depth(p: &mut Matrix4) {
    let (n, f) = near_far(p);

    if p.0[11] == -1.0 {
        // d_r = n f / ((f - n) z) - n / (f - n), z = -z_v = w
        p.0[10] = n / (f - n);
        p.0[14] = n * f / (f - n);
    } else {
        // d_r = (f + z_v) / (f - n), w = 1
        p.0[10] = 1.0 / (f - n);
        p.0[14] = f / (f - n);
    }
}

fn f32x16(m: &Matrix4) -> [f32; 16] {
    m.0.map(|v| v as f32)
}

/// Matrix4.determinant3 of the (Float32) model-view matrix.
fn determinant3(m: &[f32; 16]) -> f64 {
    let m: Vec<f64> = m.iter().map(|v| *v as f64).collect();
    m[0] * (m[5] * m[10] - m[6] * m[9]) - m[1] * (m[4] * m[10] - m[6] * m[8])
        + m[2] * (m[4] * m[9] - m[5] * m[8])
}

/// X3DProgrammableShaderObject.setUniforms: transpose the 3×3 of the Float32 model-view matrix,
/// invert it in doubles, upload as Float32 (column-major).
fn normal_matrix(mv: &[f32; 16]) -> [f32; 9] {
    let a: Vec<f64> = mv.iter().map(|v| *v as f64).collect();
    let mut n = Matrix3([0.0; 9]);
    n.0[0] = a[0];
    n.0[3] = a[1];
    n.0[6] = a[2];
    n.0[1] = a[4];
    n.0[4] = a[5];
    n.0[7] = a[6];
    n.0[2] = a[8];
    n.0[5] = a[9];
    n.0[8] = a[10];
    n.inverse();
    n.0.map(|v| v as f32)
}

#[cfg(test)]
mod line_instance_tests {
    use super::*;

    const VIEWPORT: [f64; 4] = [0.0, 0.0, 512.0, 512.0];

    fn segments(vertices: &[[f64; 3]]) -> crate::geometry::Geometry {
        crate::geometry::Geometry {
            geometry_type: 1,
            vertices: vertices
                .iter()
                .flat_map(|v| [v[0] as f32, v[1] as f32, v[2] as f32, 1.0])
                .collect(),
            ..Default::default()
        }
    }

    const LINE: [[f64; 3]; 4] = [
        [-0.3, 0.1, 0.0],
        [0.4, -0.2, 0.1],
        [0.4, -0.2, 0.1],
        [0.1, 0.5, -0.2],
    ];

    fn model_view() -> Matrix4 {
        let mut m = Matrix4::IDENTITY;
        m.0[14] = -3.0;
        m
    }

    /// glFrustum (-0.1, 0.1, -0.1, 0.1, 0.1, 100).
    fn projection() -> Matrix4 {
        let (n, f) = (0.1, 100.0);
        Matrix4([
            n / 0.1,
            0.0,
            0.0,
            0.0,
            0.0,
            n / 0.1,
            0.0,
            0.0,
            0.0,
            0.0,
            -(f + n) / (f - n),
            -1.0,
            0.0,
            0.0,
            -2.0 * f * n / (f - n),
            0.0,
        ])
    }

    /// A particle-like instance matrix: rotation, non-uniform scale and translation.
    fn instance() -> Matrix4 {
        let (s, c) = 0.7f64.sin_cos();
        Matrix4([
            1.5 * c,
            1.5 * s,
            0.0,
            0.0,
            -0.5 * s,
            0.5 * c,
            0.0,
            0.0,
            0.0,
            0.0,
            2.0,
            0.0,
            0.2,
            0.1,
            0.3,
            1.0,
        ])
    }

    fn transform(m: &Matrix4, v: [f64; 3]) -> [f64; 3] {
        let mut p = Vector4::new(v[0], v[1], v[2], 1.0);
        m.mult_vec_matrix4(&mut p);
        [p.x / p.w, p.y / p.w, p.z / p.w]
    }

    fn vertices(out: &[f32]) -> Vec<&[f32]> {
        out.chunks_exact(STIPPLE_FLOATS_PER_VERTEX).collect()
    }

    /// An instanced draw of a wide line is the draw of the instance-transformed line: the corners are computed from
    /// `instance * vertex` in window space and come back in the geometry's own space, so that the instance matrix the
    /// draw applies afterwards (x3d.wgsl) is applied exactly once.
    #[test]
    fn instanced_wide_lines_apply_the_instance_matrix_before_the_width() {
        let (mv, projection, m) = (model_view(), projection(), instance());
        let moved: Vec<[f64; 3]> = LINE.iter().map(|&v| transform(&m, v)).collect();

        for (width, samples) in [(6.0, 1), (6.0, 4), (24.0, 1), (24.0, 4)] {
            let l = LineParams { linetype: 1, width };
            let (a, ta) = line_style_vertices(
                &segments(&LINE),
                &mv,
                &projection,
                VIEWPORT,
                &l,
                samples,
                Some(&m),
            );
            let (b, tb) = line_style_vertices(
                &segments(&moved),
                &mv,
                &projection,
                VIEWPORT,
                &l,
                samples,
                None,
            );

            assert!(ta && tb);
            assert_eq!(a.len(), b.len());

            for (k, (a, b)) in vertices(&a).into_iter().zip(vertices(&b)).enumerate() {
                let p = transform(&m, [a[0] as f64, a[1] as f64, a[2] as f64]);

                for c in 0..3 {
                    assert!(
                        (p[c] - b[c] as f64).abs() < 1e-4,
                        "width {width} samples {samples} vertex {k}: {p:?} vs {:?}",
                        &b[..3]
                    );
                }

                // The transformed lines' end points (x3d_LineStipple of LineTransform2.vs) are those of the
                // instance-transformed segment.
                for c in [6, 7, FLOATS_PER_VERTEX, FLOATS_PER_VERTEX + 1] {
                    assert!(
                        (a[c] - b[c]).abs() < 1e-2,
                        "width {width} vertex {k} float {c}"
                    );
                }
            }
        }
    }

    /// thick_lines.wgsl `display` makes the vertices line_style_vertices makes on the CPU for every instance:
    /// positions, attributes, end points and length so far.
    #[test]
    fn gpu_thick_display_vertices_match_the_cpu() {
        let Ok(mut gpu) = Gpu::new() else {
            return eprintln!("skipped: no GPU");
        };
        let (mv, projection) = (model_view(), projection());
        let mut g = segments(&LINE);
        g.normals = (0..12).map(|i| i as f32 * 0.1).collect();
        g.colors = (0..16).map(|i| i as f32 / 16.0).collect();
        let mut second = instance();
        second.0[12] = -0.4;
        second.0[5] = 1.2;
        let matrices = [instance(), Matrix4::IDENTITY, second];

        for linetype in [1, 3] {
            let l = LineParams {
                linetype,
                width: 24.0,
            };
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            let (index, per) = gpu.renderer.thick_display_vertices(
                &mut encoder,
                &g,
                &mv,
                &projection,
                VIEWPORT,
                &l,
                &matrices,
            );
            let size = (per as usize * matrices.len() * STIPPLE_FLOATS_PER_VERTEX * 4) as u64;
            let read = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            encoder.copy_buffer_to_buffer(&gpu.renderer.frame_buffers[index], 0, &read, 0, size);
            gpu.queue.submit([encoder.finish()]);
            read.slice(..).map_async(wgpu::MapMode::Read, |_| {});
            let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
            let data = read.slice(..).get_mapped_range().expect("map");
            let out: &[f32] = bytemuck::cast_slice(&data);

            assert_eq!(per, 12);

            for (i, m) in matrices.iter().enumerate() {
                let (cpu, triangles) =
                    line_style_vertices(&g, &mv, &projection, VIEWPORT, &l, 1, Some(m));
                let n = per as usize * STIPPLE_FLOATS_PER_VERTEX;
                let gpu_out = &out[i * n..(i + 1) * n];

                assert!(triangles);
                assert_eq!(cpu.len(), n);

                for (k, (a, b)) in vertices(gpu_out)
                    .into_iter()
                    .zip(vertices(&cpu))
                    .enumerate()
                {
                    for c in 0..STIPPLE_FLOATS_PER_VERTEX {
                        // Positions and window coordinates in f32 on the GPU, f64 on the CPU.
                        let tol = if c < 3 { 1e-4 } else { 2e-3 };
                        assert!(
                            (a[c] - b[c]).abs() <= tol * (1.0 + b[c].abs()),
                            "linetype {linetype} instance {i} vertex {k} float {c}: {} vs {}",
                            a[c],
                            b[c]
                        );
                    }
                }
            }
        }
    }

    /// The window-space width of a transformed line does not depend on the instance's scale.
    #[test]
    fn instanced_transformed_lines_keep_the_window_space_width() {
        let (mv, projection, m) = (model_view(), projection(), instance());
        let l = LineParams {
            linetype: 1,
            width: 24.0,
        };
        let (out, _) = line_style_vertices(
            &segments(&LINE[..2]),
            &mv,
            &projection,
            VIEWPORT,
            &l,
            1,
            Some(&m),
        );
        let mut mvp = m;
        mvp.mult_right(&mv).mult_right(&projection);
        let window = |v: &[f32]| {
            let p = transform(&mvp, [v[0] as f64, v[1] as f64, v[2] as f64]);
            [(p[0] / 2.0 + 0.5) * 512.0, (p[1] / 2.0 + 0.5) * 512.0]
        };
        let v = vertices(&out);
        // Triangle 1: p0 + o, p0 - o, p1 - o.
        let (a, b) = (window(v[0]), window(v[1]));
        let width = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();

        assert!((width - 24.0).abs() < 1e-2, "{width}");
    }

    /// updateLengthSoFar ignores the instance matrix; instanced gl.LINES have no x3d_LineStipple attribute.
    #[test]
    fn instanced_stipple_values_follow_x_ite() {
        let (mv, projection, m) = (model_view(), projection(), instance());
        let g = segments(&LINE);

        for width in [1.0, 6.0] {
            let l = LineParams { linetype: 2, width };
            let (a, _) = line_style_vertices(&g, &mv, &projection, VIEWPORT, &l, 1, Some(&m));
            let (b, _) = line_style_vertices(&g, &mv, &projection, VIEWPORT, &l, 1, None);

            assert!(vertices(&a).iter().all(|v| v[6] == 0.0
                && v[7] == 0.0
                && v[FLOATS_PER_VERTEX..FLOATS_PER_VERTEX + 3] == [0.0; 3]));
            assert!(vertices(&b).iter().any(|v| v[FLOATS_PER_VERTEX] != 0.0));
        }

        // Transformed lines: the length so far of the segment without the instance matrix.
        let l = LineParams {
            linetype: 2,
            width: 24.0,
        };
        let (a, _) = line_style_vertices(&g, &mv, &projection, VIEWPORT, &l, 1, Some(&m));
        let (b, _) = line_style_vertices(&g, &mv, &projection, VIEWPORT, &l, 1, None);
        let length = |out: &[f32]| vertices(out).last().unwrap()[FLOATS_PER_VERTEX + 2];

        assert!(length(&b) > 0.0);
        assert_eq!(length(&a), length(&b));
    }
}
