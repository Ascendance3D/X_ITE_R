//! Batch 08 shaders lane: the GLSL ES -> GLSL 450 -> naga -> WGSL rewriter (src/glsl_compat.rs).

use x3d_render::glsl_compat::{Stage, compile_part, link};

fn parts(vs: &str, fs: &str) -> Result<x3d_render::glsl_compat::Program, String> {
    link(&[(vs.to_string(), vec![])], &[(fs.to_string(), vec![])])
}

const VS100: &str = "
attribute vec4 x3d_Vertex;
attribute vec3 x3d_Normal;
attribute vec4 x3d_TexCoord;
uniform mat4 x3d_ProjectionMatrix;
uniform mat4 x3d_ModelViewMatrix;
uniform mat3 x3d_NormalMatrix;
varying vec3 n;
varying vec2 uv;
void main() {
  n = normalize(x3d_NormalMatrix * x3d_Normal);
  uv = x3d_TexCoord.st;
  gl_Position = x3d_ProjectionMatrix * x3d_ModelViewMatrix * x3d_Vertex;
}
";

const FS100: &str = "
precision mediump float;
uniform vec3 tint;
uniform sampler2D tex;
varying vec3 n;
varying vec2 uv;
void main() {
  gl_FragColor = vec4(tint * (0.3 + 0.7 * abs(n.z)), 1.0) * texture2D(tex, uv);
}
";

#[test]
fn es100_pair_links() {
    let p = parts(VS100, FS100).unwrap_or_else(|e| panic!("{e}"));
    assert!(p.has("x3d_ModelViewMatrix") && p.has("tint"));
    assert_eq!(p.samplers.len(), 1);
    assert_eq!(
        p.attributes
            .iter()
            .map(|a| (a.name.as_str(), a.location))
            .collect::<Vec<_>>(),
        vec![("x3d_Vertex", 0), ("x3d_Normal", 1), ("x3d_TexCoord", 2)]
    );
    assert!(p.vs_wgsl.contains("fn main"), "{}", p.vs_wgsl);
}

#[test]
fn es300_in_out_structs_arrays_bool() {
    let vs = "#version 300 es
in vec4 x3d_Vertex;
in vec4 x3d_Color;
uniform mat4 x3d_ProjectionMatrix;
uniform mat4 x3d_ModelViewMatrix;
uniform x3d_FogParameters x3d_Fog;
uniform float weights[3];
uniform bool flip;
out vec4 color;
flat out int id;
void main() {
  color = x3d_Color * weights[2] * x3d_Fog.visibilityRange;
  id = flip ? 1 : 2;
  gl_Position = x3d_ProjectionMatrix * x3d_ModelViewMatrix * x3d_Vertex;
}
";
    let fs = "#version 300 es
precision highp float;
precision highp int;
in vec4 color;
flat in int id;
uniform x3d_LightSourceParameters x3d_LightSource[2];
out vec4 fragColor;
void main() { fragColor = id == 1 ? color * x3d_LightSource[1].intensity : vec4(1.0); }
";
    let p = parts(vs, fs).unwrap_or_else(|e| panic!("{e}"));
    assert!(p.has("x3d_LightSource[1].intensity"));
    assert!(p.has("x3d_Fog.color"));
    assert!(p.has("weights[2]"));
    assert!(p.has("flip"));
}

#[test]
fn syntax_error_names_the_user_line() {
    let fs = "precision mediump float;
// comment

void main() {
  gl_FragColor = vec4(1.0) foo;
}
";
    let e = compile_part(Stage::Fragment, fs, &[]).unwrap_err();
    assert!(
        e.starts_with("ERROR: 0:5:") || e.starts_with("ERROR: 0:6:"),
        "{e}"
    );
}

#[test]
fn undeclared_gl_fragcolor_in_vertex_is_an_error() {
    let vs = "attribute vec4 x3d_Vertex;\nvoid main() { gl_FragColor = vec4(1.0); gl_Position = x3d_Vertex; }\n";
    assert!(compile_part(Stage::Vertex, vs, &[]).is_err());
}

#[test]
fn conditionals_and_macros() {
    let fs = "precision mediump float;
#define N 3
uniform float a[N];
#ifdef X3D_FOG
uniform vec3 fogc;
#else
uniform vec3 nofog;
#endif
#if defined (X3D_FRAGMENT_SHADER) && x3d_MaxTextures == 4
const float four = 4.0;
#endif
void main() { gl_FragColor = vec4(a[2] * four + fogc.x); }
";
    let vs = "attribute vec4 x3d_Vertex; void main() { gl_Position = x3d_Vertex; }";
    let p = parts(vs, fs).unwrap_or_else(|e| panic!("{e}"));
    assert!(p.has("fogc") && !p.has("nofog") && p.has("a[2]"));
}

#[test]
fn varying_without_vertex_output_is_a_link_error() {
    let vs = "attribute vec4 x3d_Vertex; void main() { gl_Position = x3d_Vertex; }";
    let fs =
        "precision mediump float; varying vec3 q; void main() { gl_FragColor = vec4(q, 1.0); }";
    assert!(parts(vs, fs).is_err());
}

#[test]
fn missing_stage_message() {
    let e = link(
        &[(
            "attribute vec4 x3d_Vertex; void main() { gl_Position = x3d_Vertex; }".into(),
            vec![],
        )],
        &[],
    )
    .unwrap_err();
    assert_eq!(
        e,
        "The program must contain objects to form both a vertex and fragment shader."
    );
}

#[test]
fn matrix_attribute_is_assembled_from_columns() {
    let vs = "attribute vec4 x3d_Vertex; attribute mat3 rot; varying vec3 c;
void main() { c = rot * vec3 (1.0, 0.5, 0.25); gl_Position = x3d_Vertex; }";
    let fs =
        "precision mediump float; varying vec3 c; void main() { gl_FragColor = vec4 (c, 1.0); }";
    let p = parts(vs, fs).unwrap_or_else(|e| panic!("{e}"));
    let rot = p.attributes.iter().find(|a| a.name == "rot").unwrap();
    assert_eq!((rot.components, rot.location), (9, 9));
    assert!(p.vs_wgsl.contains("@location(9)") && p.vs_wgsl.contains("@location(11)"));
}

#[test]
fn max_lights_macro_sizes_arrays_and_loops() {
    let vs = "attribute vec4 x3d_Vertex; void main() { gl_Position = x3d_Vertex; }";
    let fs = "precision mediump float;
uniform x3d_LightSourceParameters x3d_LightSource [x3d_MaxLights];
void main()
{
   float c = 0.0;
   for (int i = 0; i < x3d_MaxLights; ++ i)
   {
      if (x3d_LightSource [i] .type == x3d_NoneLight)
         break;
      c += x3d_LightSource [i] .intensity;
   }
   gl_FragColor = vec4 (c);
}";
    let p = parts(vs, fs).unwrap_or_else(|e| panic!("{e}"));
    assert!(p.has("x3d_LightSource[7].intensity") && !p.has("x3d_LightSource[8].intensity"));
}

#[test]
fn precision_statement_for_a_sampler_type_is_accepted() {
    let vs = "#version 300 es
in vec4 x3d_Vertex; in vec4 x3d_TexCoord0; out vec2 vT;
void main() { vT = x3d_TexCoord0.st; gl_Position = x3d_Vertex; }";
    let fs = "#version 300 es
precision mediump float;
precision mediump sampler3D;
in vec2 vT; uniform sampler3D vol; out vec4 fragColor;
void main() { fragColor = texture (vol, vec3 (vT, 0.5)); }";
    let p = parts(vs, fs).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(p.samplers.len(), 1);
}

#[test]
fn several_parts_of_one_stage_link_the_first() {
    let vs = "attribute vec4 x3d_Vertex; void main() { gl_Position = x3d_Vertex; }".to_string();
    let fs = "precision mediump float; void main() { gl_FragColor = vec4 (1.0); }".to_string();
    assert!(link(&[(vs.clone(), vec![]), (vs, vec![])], &[(fs, vec![])]).is_ok());
}

// ---- REA remediation (integration-b06-b08): context-dependent includes, link texts, transform feedback ----

#[test]
fn a_function_declared_first_and_defined_after_its_caller_compiles() {
    // naga numbers functions in definition order and wants the callee first; X_ITE's Fragment include does this.
    let fs = "precision mediump float;\nvec4 f ();\nvoid main() { gl_FragColor = f (); }\nvec4 f () { return vec4 (1.0); }\n";
    assert_eq!(compile_part(Stage::Fragment, fs, &[]), Ok(()));
}

#[test]
fn a_switch_case_that_returns_compiles() {
    let fs = "precision mediump float;\nuniform int i;\nvec4 g (const in int k) { switch (k) { case 0: { return vec4 (0.0); } case 1: { return vec4 (1.0); } default: { return vec4 (0.5); } } }\nvoid main() { gl_FragColor = g (i); }\n";
    assert_eq!(compile_part(Stage::Fragment, fs, &[]), Ok(()));
}

#[test]
fn a_define_in_the_head_selects_the_x_ite_types() {
    // X_ITE inserts its options and types after the head (version, comments, directives) of the source:
    // X3D_PHYSICAL_MATERIAL there makes x3d_PhysicalMaterialParameters known, one in the body does not.
    let body = "#version 300 es\nprecision mediump float;\n#define X3D_PHYSICAL_MATERIAL\nuniform x3d_PhysicalMaterialParameters m;\nout vec4 c;\nvoid main() { c = vec4 (m.transparency); }\n";
    let head = "#version 300 es\n#define X3D_PHYSICAL_MATERIAL\nprecision mediump float;\nuniform x3d_PhysicalMaterialParameters m;\nout vec4 c;\nvoid main() { c = vec4 (m.transparency); }\n";
    assert!(compile_part(Stage::Fragment, body, &[]).is_err());
    assert_eq!(compile_part(Stage::Fragment, head, &[]), Ok(()));
}

#[test]
fn link_logs_use_the_driver_texts() {
    let vs = "attribute vec4 x3d_Vertex;\nuniform mat4 x3d_ProjectionMatrix;\nuniform mat4 x3d_ModelViewMatrix;\nvarying vec3 vN;\nvoid main() { vN = vec3 (1.0); gl_Position = x3d_ProjectionMatrix * x3d_ModelViewMatrix * x3d_Vertex; }\n";
    let fs = |decl: &str, body: &str| {
        format!("precision mediump float;\n{decl}\nvoid main() {{ gl_FragColor = {body}; }}\n")
    };

    for (decl, body, want) in [
        (
            "varying vec3 vX;",
            "vec4 (vX, 1.0)",
            "FRAGMENT varying vX does not match any VERTEX varying",
        ),
        (
            "varying vec4 vN;",
            "vN",
            "Types of varying 'vN' differ between VERTEX and FRAGMENT shaders.",
        ),
        (
            "uniform vec3 x3d_ModelViewMatrix;",
            "vec4 (x3d_ModelViewMatrix, 1.0)",
            "Types of uniform 'x3d_ModelViewMatrix' differ between VERTEX and FRAGMENT shaders.",
        ),
        (
            "uniform mediump mat4 x3d_ModelViewMatrix;",
            "x3d_ModelViewMatrix [0]",
            "Precisions of uniform 'x3d_ModelViewMatrix' differ between VERTEX and FRAGMENT shaders.",
        ),
    ] {
        assert_eq!(parts(vs, &fs(decl, body)).err().as_deref(), Some(want));
    }
}

#[test]
fn transform_feedback_varyings_follow_the_driver() {
    use x3d_render::glsl_compat::check_transform_feedback;

    let vs = "attribute vec4 x3d_Vertex;\nuniform mat4 x3d_ProjectionMatrix;\nvarying vec3 vP;\nvarying float vArr [2];\nvoid main() { vP = x3d_Vertex.xyz; vArr [0] = 1.0; vArr [1] = 2.0; gl_Position = x3d_ProjectionMatrix * x3d_Vertex; }\n";
    let fs = "precision mediump float;\nvarying vec3 vP;\nvoid main() { gl_FragColor = vec4 (vP, 1.0); }\n";
    let p = parts(vs, fs).unwrap();
    let check = |names: &[&str]| {
        check_transform_feedback(&p, &names.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    };

    assert_eq!(check(&[]), Ok(()));
    assert_eq!(check(&["vP", "gl_Position", ""]), Ok(()));
    assert_eq!(
        check(&["nope"]),
        Err("Transform feedback varying nope does not exist in the vertex shader.".into())
    );
    assert_eq!(
        check(&["vP", "vP"]),
        Err("Two transform feedback varyings specify the same output variable (vP).".into())
    );
    assert_eq!(
        check(&["vArr"]),
        Err("Capture of arrays is undefined and not supported.".into())
    );
    assert_eq!(
        check(&["vArr[0]"]),
        Err("Capture of array elements is undefined and not supported.".into())
    );
}
