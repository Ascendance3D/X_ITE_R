//! GLSL ES 1.00 and 3.00 section 5.4.2: a scalar or vector constructor with an argument after the last one it uses is
//! an error. naga follows GLSL 4.50 and drops the extra components, so the shader compiled. ANGLE, the compiler of the
//! pinned X_ITE, logs `'constructor' : too many arguments` (case d of crates/x3d-render/tests/glsl_errors_b08.rs,
//! oracle/rea-b08-shaders-baseline/errors-runtime-msaa0/js.json). Only case d is recorded; the other cases use the
//! same rule.

use x3d_render::glsl_compat::{Stage, compile_part};

fn frag(body: &str) -> Result<(), String> {
    compile_part(
        Stage::Fragment,
        &format!("\nprecision mediump float;\n{body}\n"),
        &[],
    )
}

const TOO_MANY: &str = "'constructor' : too many arguments";

#[test]
fn an_unused_argument_is_an_error() {
    assert_eq!(
        frag("void main() { gl_FragColor = vec4 (1.0, 2.0, 3.0, 4.0, 5.0); }"),
        Err(format!("ERROR: 0:3: {TOO_MANY}"))
    );
    // A vector argument that fills the constructor, then one more; a scalar constructor; a later line.
    assert_eq!(
        frag("void main() { gl_FragColor = vec4 (vec4 (1.0), -1.0); }"),
        Err(format!("ERROR: 0:3: {TOO_MANY}"))
    );
    assert_eq!(
        frag("void main()\n{\n   float x = float (1.0, 2.0);\n   gl_FragColor = vec4 (x);\n}"),
        Err(format!("ERROR: 0:5: {TOO_MANY}"))
    );
}

#[test]
fn a_constructor_that_uses_each_argument_compiles() {
    // The last argument may be used in part (vec3 takes 1 + 1 + 1 of vec2's components).
    assert_eq!(
        frag(
            "void main() { vec2 v = vec2 (1.0, 2.0); gl_FragColor = vec4 (1.0, vec3 (1.0, 2.0, vec2 (3.0, 4.0))); }"
        ),
        Ok(())
    );
    assert_eq!(
        frag("void main() { gl_FragColor = vec4 (vec2 (0.5), 0.0, 1.0); }"),
        Ok(())
    );
    // An argument of unknown size (a variable) is not counted, so no text is made.
    assert_eq!(
        frag("void main() { vec2 v = vec2 (1.0); gl_FragColor = vec4 (v, 0.0, 1.0); }"),
        Ok(())
    );
}
