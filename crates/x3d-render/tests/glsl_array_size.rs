//! An identifier in a global array size that is neither a macro nor a declared name. ANGLE reports `'<name>' :
//! undeclared identifier` at its line; the pinned X_ITE 16.5.0 page logs it for the Texture include without its context
//! (oracle/rea-b24-shaderpart-baseline/include-Texture-msaa4.json: `ERROR: 17:6: 'X3D_NUM_TEXTURE_COORDINATES' :
//! undeclared identifier`, the line `vec4 texCoords [X3D_NUM_TEXTURE_COORDINATES];`).

use x3d_render::glsl_compat::{Stage, compile_part};

fn frag(decls: &str) -> Result<(), String> {
    let source = format!(
        "#version 300 es\nprecision mediump float;\n{decls}\nout vec4 fragColor;\nvoid main() {{ fragColor = vec4 (1.0); }}\n"
    );
    compile_part(Stage::Fragment, &source, &[])
}

#[test]
fn an_undeclared_identifier_in_a_global_array_size_is_named() {
    let want = Err("ERROR: 0:3: 'UNDEF_N' : undeclared identifier".to_string());
    assert_eq!(frag("vec4 texCoords [UNDEF_N];"), want);
    assert_eq!(frag("uniform vec4 colors [UNDEF_N];"), want);
    assert_eq!(frag("vec4 texCoords [UNDEF_N + 1];"), want);
}

#[test]
fn a_macro_array_size_is_accepted() {
    assert_eq!(frag("#define N 2\nvec4 texCoords [N];"), Ok(()));
    assert_eq!(frag("#define N 2\nuniform vec4 colors [N];"), Ok(()));
}

#[test]
fn a_declared_constant_is_not_called_undeclared() {
    // A global constant is declared: ANGLE accepts it as a size. This port does not evaluate GLSL constants (a known
    // gap), but it must not call the name undeclared.
    let r = frag("const int N = 2;\nuniform vec4 colors [N];");
    assert!(
        !r.as_ref().is_err_and(|e| e.contains("undeclared")),
        "{r:?}"
    );
    let r = frag("uniform vec4 colors [gl_MaxDrawBuffers];");
    assert!(
        !r.as_ref().is_err_and(|e| e.contains("undeclared")),
        "{r:?}"
    );
}

#[test]
fn errors_come_in_source_order() {
    // ANGLE preprocesses while it parses: the array size error on line 3 comes before the condition on line 4.
    assert_eq!(
        frag("vec4 texCoords [UNDEF_N];\n#if UNDEF_A\n#endif"),
        Err("ERROR: 0:3: 'UNDEF_N' : undeclared identifier".to_string())
    );
    // The condition on line 3 comes before the array size error on line 4.
    assert_eq!(
        frag("#if UNDEF_A\n#endif\nvec4 texCoords [UNDEF_N];"),
        Err("ERROR: 0:3: 'UNDEF_A' : unexpected token after conditional expression".to_string())
    );
}
