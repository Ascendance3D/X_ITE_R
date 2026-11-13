//! GLSL ES has no implicit conversion (1.00 and 3.00 section 4.1.10); naga follows GLSL 4.50 and converts. An
//! integer literal that initializes a float variable is an error in ANGLE, the compiler of the pinned X_ITE. Case h of
//! crates/x3d-render/tests/glsl_errors_b08.rs is the recorded text (oracle/rea-b08-shaders-baseline/
//! errors-runtime-msaa0/js.json: `'=' : cannot convert from 'const int' to 'mediump float'`). The other texts here
//! (highp, const, vector) use the same ANGLE type format; no recording has them.

use x3d_render::glsl_compat::{Stage, compile_part};

fn frag(body: &str) -> Result<(), String> {
    compile_part(
        Stage::Fragment,
        &format!("\nprecision mediump float;\n{body}\n"),
        &[],
    )
}

#[test]
fn an_int_literal_does_not_initialize_a_float() {
    assert_eq!(
        frag("void main() { float x = 1; gl_FragColor = vec4 (x); }"),
        Err("ERROR: 0:3: '=' : cannot convert from 'const int' to 'mediump float'".into())
    );
    // An explicit precision, a const variable, a vector, a negative literal, a later line.
    assert_eq!(
        frag("void main() { highp float x = 2; gl_FragColor = vec4 (x); }"),
        Err("ERROR: 0:3: '=' : cannot convert from 'const int' to 'highp float'".into())
    );
    assert_eq!(
        frag("void main() { const float x = 1; gl_FragColor = vec4 (x); }"),
        Err("ERROR: 0:3: '=' : cannot convert from 'const int' to 'const mediump float'".into())
    );
    assert_eq!(
        frag("void main()\n{\n   vec3 c = -1;\n   gl_FragColor = vec4 (c, 1.0);\n}"),
        Err(
            "ERROR: 0:5: '=' : cannot convert from 'const int' to 'mediump 3-component vector of float'"
                .into()
        )
    );
}

#[test]
fn a_float_initializer_still_compiles() {
    assert_eq!(
        frag(
            "void main() { float x = 1.0; float y = 1e2; float z = float (1); gl_FragColor = vec4 (x + y + z); }"
        ),
        Ok(())
    );
    // An int variable takes an int literal.
    assert_eq!(
        frag("void main() { int i = 1; gl_FragColor = vec4 (float (i)); }"),
        Ok(())
    );
}

#[test]
fn a_vertex_float_defaults_to_highp() {
    assert_eq!(
        compile_part(
            Stage::Vertex,
            "\nvoid main() { float x = 0; gl_Position = vec4 (x); }\n",
            &[]
        ),
        Err("ERROR: 0:2: '=' : cannot convert from 'const int' to 'highp float'".into())
    );
}
