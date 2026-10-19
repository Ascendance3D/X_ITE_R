//! The GPU side of ComposedShader (Batch 08, shaders lane): the compiler the runtime asks
//! ([`NagaCompiler`], on [`crate::glsl_compat`]), the built-in uniforms X3DProgrammableShaderObject
//! sets for every draw ([`fill_builtins`]), and one pipeline per linked program and draw state
//! ([`ShaderGpu`]). Programs are linked once (memoized by source), pipelines, vertex buffers and
//! bind groups are created when a frame first needs them and dropped when no frame used them for
//! [`KEEP_FRAMES`] frames. Same shape as [`crate::volume_gpu`].
//!
//! Bind groups: 0 = the `vec4 _u [N]` uniform block (dynamic offset, one slice per draw), 1 = the
//! program's textures and samplers. The vertex buffer holds one vec4 per declared attribute.

use crate::glsl_compat::{AttrDecl, Program, SamplerDecl, SamplerDim};
use crate::texture::TextureImage;
use crate::traverse::{DrawItem, Frame, MaterialParams, Shading, StyleParams};
use std::collections::HashMap;
use std::rc::Rc;
use x3d_fields::math::Matrix4;
use x3d_runtime::nodes::shaders::{ShaderCompiler, ShaderSource};

/// Frames a pipeline, vertex buffer or bind group stays after its last use.
pub const KEEP_FRAMES: u64 = 120;

// ---- compiler ---------------------------------------------------------------------------------

/// The compiler the runtime links ShaderParts with.
pub struct NagaCompiler;

impl ShaderCompiler for NagaCompiler {
    fn compile_part(&self, fragment: bool, source: &str, options: &[String]) -> Result<(), String> {
        crate::glsl_compat::compile_part(if fragment { crate::glsl_compat::Stage::Fragment } else { crate::glsl_compat::Stage::Vertex }, source, options)
    }

    fn link(&self, vertex: &[ShaderSource], fragment: &[ShaderSource]) -> Result<Vec<String>, String> {
        link_sources(vertex, fragment).map(|p| p.warnings.clone())
    }
}

/// Install the compiler on a runtime (next to `script_host`).
pub fn install(rt: &mut x3d_runtime::Runtime) {
    rt.shaders.compiler = Some(Box::new(NagaCompiler));
}

pub fn link_sources(vertex: &[ShaderSource], fragment: &[ShaderSource]) -> Result<Rc<Program>, String> {
    let v: Vec<(String, Vec<String>)> = vertex.iter().map(|s| (s.source.clone(), s.options.clone())).collect();
    let f: Vec<(String, Vec<String>)> = fragment.iter().map(|s| (s.source.clone(), s.options.clone())).collect();
    crate::glsl_compat::link_cached(&v, &f)
}

// ---- what the traversal hands over ------------------------------------------------------------

/// A shape drawn with a custom shader: the linked program, the values of the shader's user fields
/// (a slot array, slot 0 reserved) and the textures of its SFNode / MFNode fields, one per
/// sampler of the program (`None`: not a user texture).
#[derive(Clone, Debug)]
pub struct CustomDraw {
    pub program: Rc<Program>,
    pub user: Vec<[f32; 4]>,
    pub user_textures: Vec<Option<Rc<TextureImage>>>,
    /// The program's resident `x3d_ClipPlane` uniforms (see `ComposedState::clip_planes`).
    pub clip_planes: Rc<std::cell::RefCell<[[f32; 4]; 6]>>,
    /// The program's resident texture locations and sampler units (see `ComposedState::gl`).
    pub gl: Rc<std::cell::RefCell<x3d_runtime::nodes::shaders::GlResident>>,
}

/// The pipeline state of one draw (the parts of `gpu::PipelineKey` a custom draw uses).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DrawState {
    /// 0 triangles, 1 lines, 2 points.
    pub topology: u8,
    /// 0 none, 1 front, 2 back.
    pub cull: u8,
    pub ccw: bool,
    pub blend: bool,
    pub blend_mode: Option<wgpu::BlendState>,
    pub depth_write: bool,
    pub depth_test: bool,
    pub depth_func: Option<wgpu::CompareFunction>,
    pub bias: (i32, u32),
}

/// A custom draw of the frame being recorded.
pub struct CustomInput<'a> {
    pub draw: &'a CustomDraw,
    pub item: &'a DrawItem,
    pub frame: &'a Frame,
    pub back: bool,
    pub state: DrawState,
    pub target: (u32, u32),
    pub layer_rect: [f64; 4],
    pub projection: [f32; 16],
    pub normal_matrix: [f32; 9],
    pub flip: bool,
    pub reversed: bool,
}

// ---- built-in uniforms ------------------------------------------------------------------------

fn f16(m: &Matrix4) -> [f32; 16] {
    m.0.map(|v| v as f32)
}

fn set3(p: &Program, buf: &mut [[f32; 4]], path: &str, v: [f64; 3]) {
    p.set(buf, path, &[v[0] as f32, v[1] as f32, v[2] as f32]);
}

fn material_uniforms(p: &Program, buf: &mut [[f32; 4]], m: &MaterialParams) {
    // Unlit and the default material set only the emissive color and the transparency
    // (X3DOneSidedMaterialNode.setShaderUniforms); the others keep zero.
    let legacy = |new: &str, old: &str| if p.has(new) { new.to_string() } else { old.to_string() };

    p.set(buf, &legacy("x3d_Material.emissiveColor", "x3d_FrontMaterial.emissiveColor"), &m.emissive.map(|v| v as f32));
    p.set(buf, &legacy("x3d_Material.transparency", "x3d_FrontMaterial.transparency"), &[m.transparency as f32]);

    match m.pbr {
        1 => {
            set3(p, buf, "x3d_Material.baseColor", m.diffuse);
            p.set(buf, "x3d_Material.metallic", &[m.metallic as f32]);
            p.set(buf, "x3d_Material.roughness", &[m.roughness as f32]);
        }
        2 => {
            set3(p, buf, "x3d_Material.diffuseColor", m.diffuse);
            set3(p, buf, "x3d_Material.specularColor", m.specular);
            p.set(buf, "x3d_Material.glossiness", &[m.metallic as f32]);
        }
        _ if m.shading != Shading::Unlit => {
            p.set(buf, &legacy("x3d_Material.ambientIntensity", "x3d_FrontMaterial.ambientIntensity"), &[m.ambient_intensity as f32]);
            p.set(buf, &legacy("x3d_Material.diffuseColor", "x3d_FrontMaterial.diffuseColor"), &m.diffuse.map(|v| v as f32));
            p.set(buf, &legacy("x3d_Material.specularColor", "x3d_FrontMaterial.specularColor"), &m.specular.map(|v| v as f32));
            p.set(buf, &legacy("x3d_Material.shininess", "x3d_FrontMaterial.shininess"), &[m.shininess as f32]);
        }
        _ => {}
    }
}

/// X3DProgrammableShaderObject.setUniforms for the program of `input`: the slots of the uniform
/// block. The user fields come first, then the values X_ITE sets per draw; a uniform the program
/// does not declare is skipped (X_ITE: a null location).
pub fn fill_builtins(input: &CustomInput) -> Vec<[f32; 4]> {
    let p = &*input.draw.program;
    let item = input.item;
    let frame = input.frame;
    let mut buf = input.draw.user.clone();

    if buf.len() < p.slots as usize {
        buf.resize(p.slots as usize, [0.0; 4]);
    }

    // Slot 0 (renderer): target height, reversed depth, y scale of clip space, y-flipped target.
    buf[0] = [input.target.1 as f32, input.reversed as u32 as f32, if input.flip { -1.0 } else { 1.0 }, input.flip as u32 as f32];

    // Global.
    p.set(&mut buf, "x3d_ProjectionMatrix", &input.projection);
    p.set(&mut buf, "x3d_EyeMatrix", &f16(&Matrix4::IDENTITY));
    p.set(&mut buf, "x3d_ViewMatrix", &f16(&frame.view));

    if p.has("x3d_CameraSpaceMatrix") {
        let mut camera = frame.view;
        camera.inverse();
        p.set(&mut buf, "x3d_CameraSpaceMatrix", &f16(&camera));
    }

    let rect = item.viewport.unwrap_or(input.layer_rect);
    p.set(&mut buf, "x3d_Viewport", &[rect[0] as f32, rect[1] as f32, rect[2] as f32, rect[3] as f32]);
    p.set(&mut buf, "x3d_ModelViewMatrix", &f16(&item.model_view));
    p.set(&mut buf, "x3d_NormalMatrix", &input.normal_matrix);
    p.set(&mut buf, "x3d_Exposure", &[frame.exposure.max(0.0) as f32]);

    // Fog.
    if let Some(f) = &frame.fog {
        let legacy = |new: &str, old: &str| if p.has(new) { new.to_string() } else { old.to_string() };
        p.set(&mut buf, &legacy("x3d_Fog.color", "x3d_FogColor"), &f.color);
        p.set(&mut buf, &legacy("x3d_Fog.visibilityStart", "x3d_FogVisibilityStart"), &[f.visibility_start]);
        p.set(&mut buf, &legacy("x3d_Fog.visibilityRange", "x3d_FogVisibilityRange"), &[f.visibility_range]);
        p.set(&mut buf, &legacy("x3d_Fog.matrix", "x3d_FogMatrix"), &f.matrix);
    }

    // Lights: the shape's lights in order, at most maxLights (x3d_MaxLights).
    for (i, &li) in item.lights.iter().take(8).enumerate() {
        let Some(l) = frame.lights.get(li as usize) else { continue };
        // The legacy names (x3d_LightType [i] ...) when the program has no x3d_LightSource.
        let at = |f: &str| {
            let new = format!("x3d_LightSource[{i}].{f}");

            if p.has(&new) {
                return new;
            }

            let old = match f {
                "type" => "Type",
                "color" => "Color",
                "ambientIntensity" => "AmbientIntensity",
                "intensity" => "Intensity",
                "attenuation" => "Attenuation",
                "location" => "Location",
                "direction" => "Direction",
                "beamWidth" => "BeamWidth",
                "cutOffAngle" => "CutOffAngle",
                "radius" => "Radius",
                "matrix" => "Matrix",
                _ => return new,
            };
            let legacy = format!("x3d_Light{old}[{i}]");

            if p.has(&legacy) { legacy } else { new }
        };

        p.set(&mut buf, &at("type"), &[l.kind as f32]);
        p.set(&mut buf, &at("color"), &l.color.map(|v| v as f32));
        p.set(&mut buf, &at("intensity"), &[l.intensity as f32]);
        p.set(&mut buf, &at("ambientIntensity"), &[l.ambient_intensity as f32]);
        p.set(&mut buf, &at("shadowIntensity"), &[0.0]);

        match l.kind {
            1 => {
                p.set(&mut buf, &at("direction"), &[l.direction.x as f32, l.direction.y as f32, l.direction.z as f32]);
                p.set(&mut buf, &at("radius"), &[-1.0]);
            }
            _ => {
                p.set(&mut buf, &at("attenuation"), &l.attenuation.map(|v| v as f32));
                p.set(&mut buf, &at("location"), &[l.location.x as f32, l.location.y as f32, l.location.z as f32]);
                p.set(&mut buf, &at("radius"), &[l.radius as f32]);
                p.set(&mut buf, &at("matrix"), &l.matrix);

                if l.kind == 3 {
                    p.set(&mut buf, &at("direction"), &[l.direction.x as f32, l.direction.y as f32, l.direction.z as f32]);
                    p.set(&mut buf, &at("beamWidth"), &[l.beam_width as f32]);
                    p.set(&mut buf, &at("cutOffAngle"), &[l.cut_off_angle as f32]);
                }
            }
        }
    }

    // Clip planes.
    // X_ITE writes the enabled planes only; the uniforms of the others keep their last value.
    {
        let mut resident = input.draw.clip_planes.borrow_mut();

        for (i, c) in item.clip_planes.iter().take(6).enumerate() {
            resident[i] = c.map(|v| v as f32);
        }

        for (i, c) in resident.iter().enumerate() {
            p.set(&mut buf, &format!("x3d_ClipPlane[{i}]"), c);
        }
    }

    // Material (front, or the back material of a back pass).
    let material = if input.back { item.back_material.as_ref().unwrap_or(&item.material) } else { &item.material };
    material_uniforms(p, &mut buf, material);

    // Textures: matrices, coordinate generators, multi-texturing.
    for (i, m) in item.texture_matrices.iter().take(4).enumerate() {
        p.set(&mut buf, &format!("x3d_TextureMatrix[{i}]"), m);
    }

    for (i, c) in item.geometry.tex_channels.iter().take(4).enumerate() {
        p.set(&mut buf, &format!("x3d_TextureCoordinateGenerator[{i}].mode"), &[c.mode as f32]);
        p.set(&mut buf, &format!("x3d_TextureCoordinateGenerator[{i}].parameter"), &c.parameter);

        for (k, v) in c.parameter.iter().enumerate() {
            p.set(&mut buf, &format!("x3d_TextureCoordinateGenerator[{i}].parameter[{k}]"), &[*v]);
        }
    }

    if let Some(mt) = &item.multi_texture {
        p.set(&mut buf, "x3d_MultiTextureColor", &mt.color.map(|v| v as f32));

        for (i, u) in mt.units.iter().take(4).enumerate() {
            p.set(&mut buf, &format!("x3d_MultiTexture[{i}].mode"), &[u[0] as f32]);
            p.set(&mut buf, &format!("x3d_MultiTexture[{i}].alphaMode"), &[u[1] as f32]);
            p.set(&mut buf, &format!("x3d_MultiTexture[{i}].source"), &[u[2] as f32]);
            p.set(&mut buf, &format!("x3d_MultiTexture[{i}].function"), &[u[3] as f32]);
        }
    }

    // Style properties.
    match item.style {
        Some(StyleParams::Point(s)) => {
            p.set(&mut buf, "x3d_PointProperties.pointSizeScaleFactor", &[s.scale_factor as f32]);
            p.set(&mut buf, "x3d_PointProperties.pointSizeMinValue", &[s.min_value as f32]);
            p.set(&mut buf, "x3d_PointProperties.pointSizeMaxValue", &[s.max_value as f32]);
            p.set(&mut buf, "x3d_PointProperties.attenuation", &s.attenuation.map(|v| v as f32));
        }
        Some(StyleParams::Line(s)) => {
            p.set(&mut buf, "x3d_LineProperties.linetype", &[s.linetype as f32]);
            p.set(&mut buf, "x3d_LineProperties.lineStippleScale", &[(1.0 / (960.0 / 720.0 * 32.0)) as f32]);
        }
        Some(StyleParams::Fill(s)) => {
            p.set(&mut buf, "x3d_FillProperties.filled", &[s.filled as u32 as f32]);
            p.set(&mut buf, "x3d_FillProperties.hatched", &[s.hatched as u32 as f32]);
            p.set(&mut buf, "x3d_FillProperties.hatchColor", &s.hatch_color.map(|v| v as f32));
        }
        None => {}
    }

    buf
}

/// The first texture unit of the pool X_ITE pops units from. The default units
/// (`DEFAULT_UNIT`) and GL's unit 0, the initial value of a user sampler, lie outside it.
const POOL: u32 = 100;

/// The default 2D / 3D / cube unit of `getDefaultTexture2DUnit` and its siblings: a sampler
/// uniform that was never written reads these (white 1x1 textures).
fn default_unit(dim: SamplerDim) -> u32 {
    match dim {
        SamplerDim::D2 => 1,
        SamplerDim::D3 => 2,
        SamplerDim::Cube => 3,
    }
}

fn texture_dim(t: &TextureImage) -> SamplerDim {
    if t.cube {
        SamplerDim::Cube
    } else if t.volume {
        SamplerDim::D3
    } else {
        SamplerDim::D2
    }
}

/// `x3d_Texture2D[i]`, `x3d_Texture3D[i]`, `x3d_TextureCube[i]` for the built-in arrays (the
/// `x3d_CubeMapTexture` alias is the same uniform), else `name` / `name[i]`.
fn sampler_key(name: &str, index: Option<u32>) -> String {
    let name = if name == "x3d_CubeMapTexture" { "x3d_TextureCube" } else { name };

    match index {
        Some(i) => format!("{name}[{i}]"),
        None => name.to_string(),
    }
}

/// What the GL context has bound on each texture unit, per target. A binding stays until another
/// texture is bound there, so a sampler whose texture field was cleared keeps reading the last
/// texture of its unit. The entries are weak: a texture that no longer exists binds nothing.
#[derive(Default)]
pub struct Units {
    bound: HashMap<(u32, SamplerDim), std::rc::Weak<TextureImage>>,
}

impl Units {
    fn bind(&mut self, unit: u32, t: &Rc<TextureImage>) {
        self.bound.insert((unit, texture_dim(t)), Rc::downgrade(t));
    }

    fn get(&self, unit: u32, dim: SamplerDim) -> Option<Rc<TextureImage>> {
        self.bound.get(&(unit, dim)).and_then(|w| w.upgrade())
    }

    /// A built-in shader draw: the appearance textures take the pool's units in order
    /// (`X3DTextureNode.setShaderUniforms` pops one each).
    pub fn builtin_draw(&mut self, textures: &[Rc<TextureImage>]) {
        for (k, t) in textures.iter().enumerate() {
            self.bind(POOL + k as u32, t);
        }
    }

    /// A custom shader draw: `enable` pops a unit for each texture location in the program's Set,
    /// then the appearance textures pop theirs, each writing its sampler uniform. Returns the
    /// texture each sampler of the program reads.
    fn custom_draw(&mut self, input: &CustomInput) -> Vec<Option<Rc<TextureImage>>> {
        let p = &*input.draw.program;
        let mut gl = input.draw.gl.borrow_mut();
        let mut pop = POOL;

        for key in gl.textures.clone() {
            let Some((k, s)) = p.samplers.iter().enumerate().find(|(_, s)| sampler_key(&s.name, s.index) == key) else { continue };
            let Some(t) = input.draw.user_textures.get(k).and_then(|t| t.as_ref()) else { continue };

            self.bind(pop, t);
            gl.units.insert(key, pop);
            pop += 1;
            let _ = s;
        }

        for (i, t) in input.item.textures.iter().enumerate() {
            let name = match texture_dim(t) {
                SamplerDim::D2 => "x3d_Texture2D",
                SamplerDim::D3 => "x3d_Texture3D",
                SamplerDim::Cube => "x3d_TextureCube",
            };

            self.bind(pop, t);
            gl.units.insert(sampler_key(name, Some(i as u32)), pop);
            pop += 1;
        }

        p.samplers
            .iter()
            .map(|s| {
                let unit = gl.units.get(&sampler_key(&s.name, s.index)).copied();

                match unit {
                    Some(u) => self.get(u, s.dim),
                    None => None,
                }
            })
            .collect()
    }
}

// ---- vertex data ------------------------------------------------------------------------------

/// One vec4 per attribute location the program declares, per vertex.
fn vertex_slots(attributes: &[AttrDecl]) -> Vec<(u32, &AttrDecl, u32)> {
    // (slot index, attribute, column)
    let mut out = vec![];

    for a in attributes {
        let cols = match a.components {
            9 => 3,
            16 => 4,
            _ => 1,
        };

        for c in 0..cols {
            out.push((out.len() as u32, a, c));
        }
    }

    out
}

pub fn custom_vertices(g: &crate::geometry::Geometry, attributes: &[AttrDecl]) -> Vec<f32> {
    let slots = vertex_slots(attributes);
    let count = g.vertex_count();
    let mut out = Vec::with_capacity(count * slots.len() * 4);

    for v in 0..count {
        for (_, a, col) in &slots {
            let mut x = [0.0f32, 0.0, 0.0, 1.0];
            let take = |src: &[f32], n: usize, x: &mut [f32; 4]| {
                if src.len() >= (v + 1) * n {
                    for k in 0..n.min(4) {
                        x[k] = src[v * n + k];
                    }
                    true
                } else {
                    false
                }
            };

            match a.name.as_str() {
                "x3d_Vertex" => {
                    take(&g.vertices, 4, &mut x);
                }
                "x3d_Normal" => {
                    take(&g.normals, 3, &mut x);
                }
                "x3d_Color" => {
                    take(&g.colors, 4, &mut x);
                }
                "x3d_TexCoord" | "x3d_TexCoord0" => {
                    take(&g.tex_coords, 4, &mut x);
                }
                "x3d_TexCoord1" | "x3d_TexCoord2" | "x3d_TexCoord3" => {
                    let k = (a.name.as_bytes()[a.name.len() - 1] - b'1') as usize;

                    if let Some(e) = g.tex_coords_extra.get(k) {
                        take(e, 4, &mut x);
                    }
                }
                "x3d_Tangent" => {
                    take(&g.tangents, 4, &mut x);
                }
                "x3d_FogDepth" => {
                    take(&g.fog_depths, 1, &mut x);
                }
                name => {
                    if let Some(arr) = g.attribs.iter().find(|r| r.name == name) {
                        let n = arr.components;

                        if arr.values.len() >= (v + 1) * n {
                            let base = v * n;

                            if n == 9 {
                                for k in 0..3 {
                                    x[k] = arr.values[base + 3 * *col as usize + k];
                                }
                            } else if n == 16 {
                                for k in 0..4 {
                                    x[k] = arr.values[base + 4 * *col as usize + k];
                                }
                            } else {
                                for k in 0..n.min(4) {
                                    x[k] = arr.values[base + k];
                                }
                            }
                        }
                    }
                }
            }

            out.extend_from_slice(&x);
        }
    }

    out
}

// ---- GPU objects ------------------------------------------------------------------------------

struct Module {
    vs: wgpu::ShaderModule,
    fs: wgpu::ShaderModule,
    layout0: wgpu::BindGroupLayout,
    layout1: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    bytes: u64,
    used: u64,
    _program: Rc<Program>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PipelineKey {
    program: u64,
    state: DrawState,
    format: wgpu::TextureFormat,
    depth_format: wgpu::TextureFormat,
    samples: u32,
    flip: bool,
    reversed: bool,
}

struct Prepared {
    pipeline: PipelineKey,
    vertices: (usize, u64),
    count: u32,
    offset: u32,
    group1: (u64, Vec<usize>),
}

struct VertexBuffer {
    buffer: wgpu::Buffer,
    count: u32,
    used: u64,
    _geometry: Rc<crate::geometry::Geometry>,
}

#[derive(Default)]
pub struct ShaderGpu {
    modules: HashMap<u64, Module>,
    pipelines: HashMap<PipelineKey, (wgpu::RenderPipeline, u64)>,
    vertices: HashMap<(usize, u64), VertexBuffer>,
    groups: HashMap<(u64, Vec<usize>), (wgpu::BindGroup, u64)>,
    uniform: Option<(wgpu::Buffer, u64)>,
    uniform_groups: HashMap<u64, wgpu::BindGroup>,
    /// 1x1 black textures (2D, 3D, cube) for the sampler uniforms no texture was given to.
    black: Option<[(wgpu::TextureView, wgpu::Sampler); 3]>,
    /// The texture units of the GL context, across draws and frames.
    units: Units,
    prepared: Vec<Prepared>,
    pub pipelines_created: usize,
    pub buffers_created: usize,
}

fn dimension(d: SamplerDim) -> wgpu::TextureViewDimension {
    match d {
        SamplerDim::D2 => wgpu::TextureViewDimension::D2,
        SamplerDim::D3 => wgpu::TextureViewDimension::D3,
        SamplerDim::Cube => wgpu::TextureViewDimension::Cube,
    }
}

impl ShaderGpu {
    /// GPU objects held: modules, pipelines, vertex buffers, bind groups, uniform buffer.
    pub fn resources(&self) -> [usize; 5] {
        [self.modules.len(), self.pipelines.len(), self.vertices.len(), self.groups.len() + self.uniform_groups.len(), self.uniform.is_some() as usize]
    }

    /// Custom draws recorded for the last frame.
    pub fn draws(&self) -> usize {
        self.prepared.len()
    }

    fn module(&mut self, device: &wgpu::Device, p: &Rc<Program>, frame_no: u64) {
        if let Some(m) = self.modules.get_mut(&p.key) {
            m.used = frame_no;
            return;
        }

        let vs = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("custom vertex"), source: wgpu::ShaderSource::Wgsl(p.vs_wgsl.as_str().into()) });
        let fs = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("custom fragment"), source: wgpu::ShaderSource::Wgsl(p.fs_wgsl.as_str().into()) });
        let bytes = p.slots.max(1) as u64 * 16;
        let layout0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("custom uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: true, min_binding_size: wgpu::BufferSize::new(bytes) },
                count: None,
            }],
        });
        let mut entries = vec![];

        for s in &p.samplers {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: s.binding,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: dimension(s.dim), multisampled: false },
                count: None,
            });
            entries.push(wgpu::BindGroupLayoutEntry { binding: s.binding + 1, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None });
        }

        let layout1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("custom textures"), entries: &entries });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("custom"), bind_group_layouts: &[Some(&layout0), Some(&layout1)], immediate_size: 0 });
        self.modules.insert(p.key, Module { vs, fs, layout0, layout1, pipeline_layout, bytes, used: frame_no, _program: p.clone() });
    }

    /// Record the custom draws of a frame. `texture` finds the uploaded view and sampler of a
    /// texture and the dimension it is wanted as (a missing one gives the default).
    #[allow(clippy::too_many_arguments)]
    pub fn prepare<'a>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        samples: u32,
        frame_no: u64,
        inputs: &[CustomInput],
        // The built-in shader draws between the custom ones: (number of custom draws before it, its textures).
        builtin: &[(usize, Vec<Rc<TextureImage>>)],
        texture: &dyn Fn(*const TextureImage) -> Option<(&'a wgpu::TextureView, &'a wgpu::Sampler)>,
        whites: [(&'a wgpu::TextureView, &'a wgpu::Sampler); 3],
    ) {
        self.prepared.clear();

        // The draws in order: each one leaves its textures on the units for the next.
        let mut resolved = vec![];
        let mut next = 0;

        for (i, input) in inputs.iter().enumerate() {
            while next < builtin.len() && builtin[next].0 <= i {
                self.units.builtin_draw(&builtin[next].1);
                next += 1;
            }

            resolved.push(self.units.custom_draw(input));
        }

        for (_, textures) in &builtin[next..] {
            self.units.builtin_draw(textures);
        }

        if inputs.is_empty() {
            return;
        }

        if self.black.is_none() {
            self.black = Some([default_texture(device, queue, SamplerDim::D2), default_texture(device, queue, SamplerDim::D3), default_texture(device, queue, SamplerDim::Cube)]);
        }

        // Uniform slices.
        let mut blobs: Vec<Vec<u8>> = vec![];
        let mut offsets = vec![];
        let mut total = 0u64;

        for input in inputs {
            let p = &input.draw.program;
            self.module(device, p, frame_no);
            let stride = self.modules[&p.key].bytes.div_ceil(256) * 256;
            let values = fill_builtins(input);
            let mut bytes: Vec<u8> = bytemuck::cast_slice(&values).to_vec();
            bytes.resize(stride as usize, 0);
            offsets.push(total);
            total += stride;
            blobs.push(bytes);
        }

        let max_bytes = self.modules.values().map(|m| m.bytes).max().unwrap_or(16);

        if self.uniform.as_ref().is_none_or(|u| u.1 < total) {
            let capacity = total.next_power_of_two().max(4096);
            let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("custom uniforms"), size: capacity, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
            self.uniform = Some((buffer, capacity));
            self.uniform_groups.clear();
        }

        let _ = max_bytes;

        // The uniform bind group of each program (the buffer slice is the program's block).
        for input in inputs {
            let id = input.draw.program.key;

            if !self.uniform_groups.contains_key(&id) {
                let m = &self.modules[&id];
                let u = self.uniform.as_ref().unwrap();
                let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("custom uniforms"),
                    layout: &m.layout0,
                    entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &u.0, offset: 0, size: wgpu::BufferSize::new(m.bytes) }) }],
                });
                self.uniform_groups.insert(id, group);
            }
        }
        let mut all = vec![0u8; total as usize];

        for (i, b) in blobs.iter().enumerate() {
            all[offsets[i] as usize..offsets[i] as usize + b.len()].copy_from_slice(b);
        }

        queue.write_buffer(&self.uniform.as_ref().unwrap().0, 0, &all);

        // Per draw: pipeline, vertex buffer, bind groups.
        for (i, input) in inputs.iter().enumerate() {
            let p = input.draw.program.clone();
            let key = PipelineKey { program: p.key, state: input.state, format, depth_format, samples, flip: input.flip, reversed: input.reversed };

            if !self.pipelines.contains_key(&key) {
                let m = &self.modules[&p.key];
                let slots = vertex_slots(&p.attributes);
                let attributes: Vec<wgpu::VertexAttribute> = slots.iter().map(|(slot, a, col)| wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 16 * *slot as u64, shader_location: a.location + col }).collect();
                let buffers = [Some(wgpu::VertexBufferLayout { array_stride: 16 * slots.len().max(1) as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attributes })];
                let s = input.state;
                let blend = s.blend.then(|| {
                    s.blend_mode.unwrap_or(wgpu::BlendState {
                        color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
                        alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
                    })
                });
                let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("custom"),
                    layout: Some(&m.pipeline_layout),
                    vertex: wgpu::VertexState { module: &m.vs, entry_point: Some("main"), compilation_options: Default::default(), buffers: if slots.is_empty() { &[] } else { &buffers } },
                    primitive: wgpu::PrimitiveState {
                        topology: match s.topology {
                            0 => wgpu::PrimitiveTopology::TriangleList,
                            1 => wgpu::PrimitiveTopology::LineList,
                            _ => wgpu::PrimitiveTopology::PointList,
                        },
                        front_face: if s.ccw != input.flip { wgpu::FrontFace::Ccw } else { wgpu::FrontFace::Cw },
                        cull_mode: match (s.topology, s.cull) {
                            (0, 1) => Some(wgpu::Face::Front),
                            (0, 2) => Some(wgpu::Face::Back),
                            _ => None,
                        },
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: depth_format,
                        depth_write_enabled: Some(s.depth_write),
                        depth_compare: Some(if !s.depth_test {
                            wgpu::CompareFunction::Always
                        } else if let Some(f) = s.depth_func {
                            f
                        } else if input.reversed {
                            wgpu::CompareFunction::GreaterEqual
                        } else {
                            wgpu::CompareFunction::LessEqual
                        }),
                        stencil: Default::default(),
                        bias: wgpu::DepthBiasState { constant: s.bias.0, slope_scale: f32::from_bits(s.bias.1), clamp: 0.0 },
                    }),
                    multisample: wgpu::MultisampleState { count: samples, mask: !0, alpha_to_coverage_enabled: false },
                    fragment: Some(wgpu::FragmentState {
                        module: &m.fs,
                        entry_point: Some("main"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
                    }),
                    multiview_mask: None,
                    cache: None,
                });
                self.pipelines.insert(key, (pipeline, frame_no));
                self.pipelines_created += 1;
            }

            self.pipelines.get_mut(&key).unwrap().1 = frame_no;

            // Vertex buffer: per geometry and program attribute set.
            let g = &input.item.geometry;
            let vkey = (Rc::as_ptr(g) as usize, attribute_key(&p.attributes));
            let count = g.vertex_count() as u32;

            match self.vertices.get_mut(&vkey) {
                Some(v) => v.used = frame_no,
                None => {
                    let data = custom_vertices(g, &p.attributes);
                    let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("custom vertices"), size: (data.len() * 4).max(16) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });

                    if !data.is_empty() {
                        queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&data));
                    }

                    self.vertices.insert(vkey, VertexBuffer { buffer, count, used: frame_no, _geometry: g.clone() });
                    self.buffers_created += 1;
                }
            }

            // Texture bind group.
            let textures = resolved[i].clone();
            let ptrs: Vec<usize> = textures.iter().map(|t| t.as_ref().map_or(0, |t| Rc::as_ptr(t) as usize)).collect();
            let gkey = (p.key, ptrs);

            match self.groups.get_mut(&gkey) {
                Some(g) => g.1 = frame_no,
                None => {
                    let group = {
                        let m = &self.modules[&p.key];
                        let black = self.black.as_ref().unwrap();
                        let index = |d: SamplerDim| match d {
                            SamplerDim::D2 => 0,
                            SamplerDim::D3 => 1,
                            SamplerDim::Cube => 2,
                        };
                        let mut entries = vec![];
                        let resources: Vec<(&wgpu::TextureView, &wgpu::Sampler)> = p
                            .samplers
                            .iter()
                            .zip(&textures)
                            .map(|(s, t): (&SamplerDecl, &Option<Rc<TextureImage>>)| match t.as_ref().and_then(|t| texture(Rc::as_ptr(t))) {
                                Some(r) => r,
                                // Measured against X_ITE: an unset user sampler2D reads white, a samplerCube
                                // and a sampler3D read black.
                                None if s.name.starts_with("x3d_") || s.dim == SamplerDim::D2 => whites[index(s.dim)],
                                None => (&black[index(s.dim)].0, &black[index(s.dim)].1),
                            })
                            .collect();

                        for (s, (view, sampler)) in p.samplers.iter().zip(&resources) {
                            entries.push(wgpu::BindGroupEntry { binding: s.binding, resource: wgpu::BindingResource::TextureView(view) });
                            entries.push(wgpu::BindGroupEntry { binding: s.binding + 1, resource: wgpu::BindingResource::Sampler(sampler) });
                        }

                        device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("custom textures"), layout: &m.layout1, entries: &entries })
                    };
                    self.groups.insert(gkey.clone(), (group, frame_no));
                }
            }

            self.prepared.push(Prepared { pipeline: key, vertices: vkey, count, offset: offsets[i] as u32, group1: gkey });
        }

    }

    /// Draw prepared call `i`.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, i: usize) {
        let p = &self.prepared[i];
        let Some(vb) = self.vertices.get(&p.vertices) else { return };

        pass.set_pipeline(&self.pipelines[&p.pipeline].0);
        pass.set_bind_group(0, &self.uniform_groups[&p.pipeline.program], &[p.offset]);
        pass.set_bind_group(1, &self.groups[&p.group1].0, &[]);
        pass.set_vertex_buffer(0, vb.buffer.slice(..));
        pass.draw(0..p.count.min(vb.count), 0..1);
    }

    /// Drop what no frame used for [`KEEP_FRAMES`] frames.
    pub fn prune(&mut self, frame_no: u64) {
        self.pipelines.retain(|_, e| e.1 + KEEP_FRAMES >= frame_no);
        self.groups.retain(|_, e| e.1 + KEEP_FRAMES >= frame_no);
        self.vertices.retain(|_, v| v.used + KEEP_FRAMES >= frame_no);
        self.modules.retain(|_, m| m.used + KEEP_FRAMES >= frame_no);
        let modules = &self.modules;
        self.uniform_groups.retain(|k, _| modules.contains_key(k));

        if self.modules.is_empty() && self.pipelines.is_empty() {
            self.uniform = None;
            self.uniform_groups.clear();
            self.groups.clear();
        }
    }
}

/// A 1x1 opaque black texture of a dimension with a nearest sampler.
fn default_texture(device: &wgpu::Device, queue: &wgpu::Queue, dim: SamplerDim) -> (wgpu::TextureView, wgpu::Sampler) {
    let layers = if dim == SamplerDim::Cube { 6 } else { 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("custom default texture"),
        size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: layers },
        mip_level_count: 1,
        sample_count: 1,
        dimension: if dim == SamplerDim::D3 { wgpu::TextureDimension::D3 } else { wgpu::TextureDimension::D2 },
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let data = [0u8, 0, 0, 255].repeat(layers as usize);
    queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        &data,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) },
        wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: layers },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor { dimension: Some(dimension(dim)), ..Default::default() });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Nearest, min_filter: wgpu::FilterMode::Nearest, ..Default::default() });
    (view, sampler)
}

fn attribute_key(attributes: &[AttrDecl]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();

    for a in attributes {
        a.name.hash(&mut h);
        a.location.hash(&mut h);
        a.components.hash(&mut h);
    }

    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_binding_stays_until_another_texture_takes_the_unit() {
        let mut u = Units::default();
        let (a, b) = (Rc::new(TextureImage::white()), Rc::new(TextureImage::white()));

        u.builtin_draw(&[a.clone()]);
        assert!(Rc::ptr_eq(&u.get(POOL, SamplerDim::D2).unwrap(), &a));
        // A draw without textures leaves the unit alone.
        u.builtin_draw(&[]);
        assert!(Rc::ptr_eq(&u.get(POOL, SamplerDim::D2).unwrap(), &a));
        u.builtin_draw(&[b.clone()]);
        assert!(Rc::ptr_eq(&u.get(POOL, SamplerDim::D2).unwrap(), &b));
        // Another target of the same unit is a different binding.
        assert!(u.get(POOL, SamplerDim::D3).is_none());
    }

    #[test]
    fn a_released_texture_leaves_no_binding_and_the_table_stays_bounded() {
        let mut u = Units::default();

        for _ in 0..1000 {
            let t = Rc::new(TextureImage::white());
            u.builtin_draw(&[t.clone(), t.clone()]);
        }

        // Every texture is gone: the units read the default, and one entry per unit remains.
        assert!(u.get(POOL, SamplerDim::D2).is_none());
        assert!(u.get(POOL + 1, SamplerDim::D2).is_none());
        assert_eq!(u.bound.len(), 2);
    }

    #[test]
    fn sampler_keys_name_the_cube_alias_once() {
        assert_eq!(sampler_key("x3d_CubeMapTexture", Some(1)), "x3d_TextureCube[1]");
        assert_eq!(sampler_key("map", None), "map");
        assert_eq!(sampler_key("maps", Some(2)), "maps[2]");
    }
}
