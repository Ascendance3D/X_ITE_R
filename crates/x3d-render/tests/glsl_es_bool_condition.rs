//! GLSL ES 1.00 and 3.00 sections 6.2 and 6.3: the condition of an `if` is a scalar bool. naga rejects a float
//! condition only as "Function [1] '_x3d_main' is invalid" at line 1. ANGLE, the compiler of the pinned X_ITE,
//! reports `'' : boolean expression expected` at the line of the condition (case k of
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
fn a_condition_that_is_not_bool_is_an_error() {
    assert_eq!(
        frag("void main() { if (1.0) { gl_FragColor = vec4 (1.0); } }"),
        Err("ERROR: 0:3: '' : boolean expression expected".into())
    );
    assert_eq!(
        frag("void main()\n{\n   float a = 1.0;\n   if (a)\n      gl_FragColor = vec4 (1.0);\n}"),
        Err("ERROR: 0:6: '' : boolean expression expected".into())
    );
    assert_eq!(
        frag("void main() { int i = 1; if (i) { gl_FragColor = vec4 (1.0); } }"),
        Err("ERROR: 0:3: '' : boolean expression expected".into())
    );
    assert_eq!(
        frag("void main() { if (true) { gl_FragColor = vec4 (1.0); } }"),
        Ok(())
    );
}
