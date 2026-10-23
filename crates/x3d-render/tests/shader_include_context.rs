//! The ShaderRegistry includes that need macro / uniform context, linked in the contexts X_ITE itself
//! gives them: the built-in shader sources (Default2.vs, Pointing2.vs, Material2.fs, Unlit2.fs,
//! Physical2.fs ...) with the options `X3DMaterialNode.getShaderOptions` produces. `PROBE=1` prints.

use x3d_render::glsl_compat::link_cached;
use x3d_runtime::nodes::shader_include::process;

/// The 11 options every user ShaderPart gets (ShaderPart.js customOptions). X_ITE defines them after the
/// source's head, so a source drops the ones it does not want with `#undef` in its body, and the macros it
/// wants the X_ITE types to see go in the head.
const CUSTOM: [&str; 11] = [
    "X3D_GEOMETRY_0D",
    "X3D_GEOMETRY_1D",
    "X3D_GEOMETRY_2D",
    "X3D_GEOMETRY_3D",
    "X3D_FOG",
    "X3D_STYLE_PROPERTIES",
    "X3D_UNLIT_MATERIAL",
    "X3D_PHONG_MATERIAL",
    "X3D_LIGHTING",
    "X3D_TEXTURE",
    "X3D_MULTI_TEXTURING",
];

fn src(include: &str, defs: &str) -> String {
    let wanted: Vec<&str> = defs
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let head: String = wanted.iter().map(|d| format!("#define {d}\n")).collect();
    let undef: String = CUSTOM
        .iter()
        .filter(|c| {
            !wanted
                .iter()
                .any(|w| w.split_whitespace().next() == Some(**c))
        })
        .map(|c| format!("#undef {c}\n"))
        .collect();

    process(&format!(
        "#version 300 es\n{head}\nprecision highp int;\nprecision highp float;\nprecision highp sampler2D;\nprecision highp sampler3D;\nprecision highp samplerCube;\n{undef}#include <{include}>\n"
    ))
}

fn link(vs: &str, fs: &str, vdefs: &str, fdefs: &str) -> Result<(), String> {
    link_cached(&[(src(vs, vdefs), vec![])], &[(src(fs, fdefs), vec![])]).map(|_| ())
}

const BASE: &str =
    "X3D_COLORSPACE_SRGB, X3D_NORMALS, X3D_NUM_TEXTURE_TRANSFORMS 1, X3D_NUM_TEXTURE_COORDINATES 1";

/// (vertex include, fragment include, macros): the built-in shaders of X_ITE as a user shader writes them.
fn contexts() -> Vec<(&'static str, &'static str, String)> {
    vec![
        (
            "Vertex",
            "Unlit",
            format!("{BASE}, X3D_GEOMETRY_3D, X3D_UNLIT_MATERIAL"),
        ),
        (
            "Vertex",
            "Material",
            format!("{BASE}, X3D_GEOMETRY_3D, X3D_PHONG_MATERIAL"),
        ),
        (
            "Vertex",
            "Material",
            format!("{BASE}, X3D_GEOMETRY_3D, X3D_PHONG_MATERIAL, X3D_LIGHTING, X3D_NUM_LIGHTS 2"),
        ),
        (
            "Vertex",
            "Unlit",
            format!("{BASE}, X3D_GEOMETRY_3D, X3D_UNLIT_MATERIAL, X3D_FOG, X3D_FOG_LINEAR"),
        ),
        (
            "Vertex",
            "Unlit",
            format!(
                "{BASE}, X3D_GEOMETRY_3D, X3D_UNLIT_MATERIAL, X3D_TEXTURE, X3D_NUM_TEXTURES 1, X3D_TEXTURE_0_2D"
            ),
        ),
        (
            "Vertex",
            "PBR",
            format!(
                "{BASE}, X3D_GEOMETRY_3D, X3D_PHYSICAL_MATERIAL, X3D_MATERIAL_METALLIC_ROUGHNESS, X3D_LIGHTING, X3D_NUM_LIGHTS 1"
            ),
        ),
        (
            "Vertex",
            "Unlit",
            format!("{BASE}, X3D_GEOMETRY_0D, X3D_UNLIT_MATERIAL, X3D_STYLE_PROPERTIES"),
        ),
        (
            "Vertex",
            "Unlit",
            format!("{BASE}, X3D_GEOMETRY_1D, X3D_UNLIT_MATERIAL, X3D_STYLE_PROPERTIES"),
        ),
    ]
}

#[test]
fn the_context_dependent_includes_link_in_the_contexts_x_ite_gives_them() {
    let list: Vec<&str> = include_str!("../../x3d-runtime/src/nodes/shader_includes/list.txt")
        .lines()
        .collect();
    let mut reached = std::collections::BTreeSet::new();

    for (v, f, o) in contexts() {
        assert_eq!(link(v, f, &o, &o), Ok(()), "{v}+{f} [{o}]");

        for text in [src(v, &o), src(f, &o)] {
            for (i, file) in list.iter().enumerate() {
                if text.contains(&format!("#line 1 {}\n", i + 1)) {
                    reached.insert(file[3..file.len() - 5].to_string());
                }
            }
        }
    }

    // The 11 files of the 26 that do not link on their own (shader_include.rs probe_all_includes) all take part.
    for name in [
        "Fog",
        "Fragment",
        "Material",
        "Normal",
        "PointSize",
        "Stipple",
        "Texture",
        "Unlit",
        "Vertex",
        "MaterialInfo",
        "PBR",
    ] {
        assert!(
            reached.contains(name),
            "{name} is not part of any context: {reached:?}"
        );
    }
}
