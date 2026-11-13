//! GLSL ES has no implicit conversion: an assignment of a vector to a vector or scalar of another component count is
//! an error. naga rejects it only as "Function [1] '_x3d_main' is invalid" at line 1. ANGLE, the compiler of the
//! pinned X_ITE, logs `'=' : dimension mismatch` at the line of the assignment (case a of
//! crates/x3d-render/tests/glsl_errors_b08.rs, oracle/rea-b08-shaders-baseline/errors-runtime-msaa0/js.json).

use x3d_render::glsl_compat::{Stage, compile_part};

fn frag(body: &str) -> Result<(), String> {
    compile_part(
        Stage::Fragment,
        &format!("\nprecision mediump float;\n{body}\n"),
        &[],
    )
}

#[test]
fn a_vector_of_another_size_is_a_dimension_mismatch() {
    assert_eq!(
        frag("void main() { gl_FragColor = vec3 (1.0); }"),
        Err("ERROR: 0:3: '=' : dimension mismatch".into())
    );
    assert_eq!(
        frag(
            "void main()\n{\n   vec2 v;\n   v = vec3 (1.0);\n   gl_FragColor = vec4 (v, 0.0, 1.0);\n}"
        ),
        Err("ERROR: 0:6: '=' : dimension mismatch".into())
    );
}

#[test]
fn a_vector_of_the_same_size_compiles() {
    assert_eq!(
        frag("void main() { vec3 c; c = vec3 (1.0); gl_FragColor = vec4 (c, 1.0); }"),
        Ok(())
    );
}
