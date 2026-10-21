//! ComposedShader in the DISPLAY pass (a child module of `traverse`, Batch 08 shaders lane):
//! the shape's Appearance selected a valid shader (`Appearance.shaderNode`, kept by the runtime,
//! nodes/shaders.rs); its user-defined fields become uniforms
//! (X3DProgrammableShaderObject.addShaderFields / set_field__) and its texture fields the
//! sampler uniforms. The built-in uniforms are written by the renderer per draw
//! ([`crate::shader_gpu::fill_builtins`]).

use super::*;
use crate::glsl_compat::{Program, Scalar, Ty};
use crate::shader_gpu::{link_sources, CustomDraw};
use x3d_fields::FieldValue;
use x3d_fields::access;

/// The uniform class a field value is set with (X3DProgrammableShaderObject.addShaderFields).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    /// uniform1i / uniform1iv
    Int,
    /// uniform1f / uniform1fv
    Float,
    F2,
    F3,
    F4,
    M3,
    M4,
}

fn compatible(class: Class, ty: &Ty) -> bool {
    match (class, ty) {
        (Class::Int, Ty::Scalar(Scalar::Int | Scalar::Bool)) => true,
        (Class::Float, Ty::Scalar(Scalar::Float | Scalar::Bool)) => true,
        (Class::F2, Ty::Vec(Scalar::Float | Scalar::Bool, 2)) => true,
        (Class::F3, Ty::Vec(Scalar::Float | Scalar::Bool, 3)) => true,
        (Class::F4, Ty::Vec(Scalar::Float | Scalar::Bool, 4)) => true,
        (Class::M3, Ty::Mat(3)) => true,
        (Class::M4, Ty::Mat(4)) => true,
        _ => false,
    }
}

fn f32s<T: Copy + Into<f64>>(v: &[T]) -> Vec<f32> {
    v.iter().map(|x| (*x).into() as f32).collect()
}

/// A field value as uniform elements: the class and one flat vector per element, and whether it
/// is an array (an MF field).
fn elements(v: &FieldValue) -> Option<(Class, Vec<Vec<f32>>, bool)> {
    use FieldValue::*;

    let chunks = |flat: Vec<f32>, n: usize| -> Vec<Vec<f32>> { flat.chunks_exact(n).map(|c| c.to_vec()).collect() };

    Some(match v {
        SFBool(b) => (Class::Int, vec![vec![*b as u32 as f32]], false),
        SFInt32(i) => (Class::Int, vec![vec![*i as f32]], false),
        SFFloat(f) | SFDouble(f) | SFTime(f) => (Class::Float, vec![vec![*f as f32]], false),
        SFVec2f(x) | SFVec2d(x) => (Class::F2, vec![vec![x.x as f32, x.y as f32]], false),
        SFColor(c) => (Class::F3, vec![vec![c.r() as f32, c.g() as f32, c.b() as f32]], false),
        SFVec3f(x) | SFVec3d(x) => (Class::F3, vec![vec![x.x as f32, x.y as f32, x.z as f32]], false),
        SFColorRGBA(c) => (Class::F4, vec![c.components().iter().map(|v| *v as f32).collect()], false),
        SFVec4f(x) | SFVec4d(x) => (Class::F4, vec![vec![x.x as f32, x.y as f32, x.z as f32, x.w as f32]], false),
        SFMatrix3f(m) | SFMatrix3d(m) => (Class::M3, vec![f32s(&m.0)], false),
        SFMatrix4f(m) | SFMatrix4d(m) => (Class::M4, vec![f32s(&m.0)], false),
        SFRotation(r) => (Class::M3, vec![f32s(&r.get_matrix().0)], false),
        SFQuaternion(q) => (Class::M3, vec![f32s(&q.get_matrix3().0)], false),
        MFBool(b) => (Class::Int, b.iter().map(|x| vec![*x as u32 as f32]).collect(), true),
        MFInt32(i) => (Class::Int, i.iter().map(|x| vec![*x as f32]).collect(), true),
        MFFloat(f) => (Class::Float, f.iter().map(|x| vec![*x]).collect(), true),
        MFDouble(f) | MFTime(f) => (Class::Float, f.iter().map(|x| vec![*x as f32]).collect(), true),
        MFVec2f(f) => (Class::F2, chunks(f.clone(), 2), true),
        MFVec2d(f) => (Class::F2, chunks(f32s(f), 2), true),
        MFColor(f) | MFVec3f(f) => (Class::F3, chunks(f.clone(), 3), true),
        MFVec3d(f) => (Class::F3, chunks(f32s(f), 3), true),
        MFColorRGBA(f) | MFVec4f(f) => (Class::F4, chunks(f.clone(), 4), true),
        MFVec4d(f) => (Class::F4, chunks(f32s(f), 4), true),
        MFMatrix3f(f) => (Class::M3, chunks(f.clone(), 9), true),
        MFMatrix3d(f) => (Class::M3, chunks(f32s(f), 9), true),
        MFMatrix4f(f) => (Class::M4, chunks(f.clone(), 16), true),
        MFMatrix4d(f) => (Class::M4, chunks(f32s(f), 16), true),
        MFRotation(f) => (
            Class::M3,
            f.chunks_exact(4).map(|c| f32s(&x3d_fields::math::Rotation4::new(c[0], c[1], c[2], c[3]).get_matrix().0)).collect(),
            true,
        ),
        MFQuaternion(f) => (
            Class::M3,
            f.chunks_exact(4).map(|c| f32s(&x3d_fields::math::Quaternion::new(c[0], c[1], c[2], c[3]).get_matrix3().0)).collect(),
            true,
        ),
        _ => return None,
    })
}

/// set_field__ for a non-texture field: write the value into the slots of the uniform of the
/// same name when the types agree (a mismatch is a GL error and the uniform keeps its value).
fn write_field(p: &Program, buf: &mut [[f32; 4]], name: &str, value: &FieldValue) {
    let Some(decl) = p.uniform(name) else { return };

    // SFImage / MFImage: int arrays [width, height, components, pixels...].
    let images: Option<Vec<f32>> = match value {
        FieldValue::SFImage(i) => Some([i.width as f32, i.height as f32, i.comp as f32].into_iter().chain(i.array.iter().map(|v| *v as f32)).collect()),
        FieldValue::MFImage(list) => Some(list.iter().flat_map(|i| [i.width as f32, i.height as f32, i.comp as f32].into_iter().chain(i.array.iter().map(|v| *v as f32))).collect()),
        _ => None,
    };

    if let Some(flat) = images {
        if let (Some(len), Ty::Scalar(Scalar::Int)) = (decl.array, &decl.ty) {
            for k in 0..len as usize {
                p.set(buf, &format!("{name}[{k}]"), &[flat.get(k).copied().unwrap_or(0.0)]);
            }
        }

        return;
    }

    let Some((class, elems, is_array)) = elements(value) else { return };

    if !compatible(class, &decl.ty) {
        return;
    }

    match (decl.array, is_array) {
        // An MF field needs an array uniform (getLocationLength); extra elements are ignored,
        // missing ones are 0.
        (Some(len), true) => {
            for k in 0..len as usize {
                match elems.get(k) {
                    Some(e) => p.set(buf, &format!("{name}[{k}]"), e),
                    None => p.set(buf, &format!("{name}[{k}]"), &[0.0; 16]),
                };
            }
        }
        (None, true) => {}
        // An SF field sets the uniform, or the first element of an array uniform.
        (Some(_), false) => {
            p.set(buf, &format!("{name}[0]"), &elems[0]);
        }
        (None, false) => {
            p.set(buf, name, &elems[0]);
        }
    }
}

impl Builder<'_> {
    /// The CustomDraw of an Appearance whose `shaders` selected a valid ComposedShader.
    pub(super) fn custom_draw(&mut self, sg: &Sg, appearance: Option<NodeId>) -> Option<Rc<CustomDraw>> {
        let a = appearance?;
        let shader = sg.rt.appearance_shader(a)?;
        let (vs, fs) = sg.rt.composed_shader_sources(shader)?;
        let program = link_sources(vs, fs).ok()?;
        let mut user = program.new_buffer();
        let mut user_textures: Vec<Option<Rc<TextureImage>>> = vec![None; program.samplers.len()];
        let node = sg.rt.scene.node(shader);
        let gl = sg.rt.composed_gl(shader).unwrap_or_default();
        // The texture locations that hold a texture node now, in field order (the Set's members).
        let mut present: Vec<String> = vec![];

        for f in node.user_fields.iter().filter(|f| f.access != access::OUTPUT_ONLY) {
            match &f.value {
                FieldValue::SFNode(n) => {
                    if let Some(t) = n {
                        let tex = self.shader_texture(sg, *t);

                        if let Some(k) = program.samplers.iter().position(|s| s.name == f.name && s.index.is_none()) {
                            if tex.is_some() {
                                present.push(f.name.clone());
                            }

                            user_textures[k] = tex;
                        }
                    }
                }
                FieldValue::MFNode(list) => {
                    for (i, n) in list.iter().enumerate() {
                        let Some(t) = n else { continue };
                        let tex = self.shader_texture(sg, *t);

                        if let Some(k) = program.samplers.iter().position(|s| s.name == f.name && s.index == Some(i as u32)) {
                            if tex.is_some() {
                                present.push(format!("{}[{i}]", f.name));
                            }

                            user_textures[k] = tex;
                        }
                    }
                }
                v => write_field(&program, &mut user, &f.name, v),
            }
        }

        // set_field__: a field that holds a texture node joins the Set (at the end), one that does
        // not leaves it. The traversal sees the fields once per frame.
        {
            let mut g = gl.borrow_mut();
            g.textures.retain(|k| present.contains(k));

            for k in present {
                if !g.textures.contains(&k) {
                    g.textures.push(k);
                }
            }
        }

        // Appearance.getAlphaCutoff () (x3d_AlphaCutoff).
        program.set(&mut user, "x3d_AlphaCutoff", &[sg.f64(a, "alphaCutoff", 0.5) as f32]);

        Some(Rc::new(CustomDraw { program, user, user_textures, clip_planes: sg.rt.composed_clip_planes(shader).unwrap_or_default(), gl }))
    }

    /// X3DCast (X3DTextureNode, field): the texture a field names; one that has not loaded binds the
    /// default white 1x1 texture like an appearance texture.
    fn shader_texture(&mut self, sg: &Sg, n: NodeId) -> Option<Rc<TextureImage>> {
        let texture = self.texture(sg, n);

        match texture {
            None if is_single_texture(sg, n) && can_load(sg, n) => Some(self.white_texture()),
            t => t,
        }
    }
}
