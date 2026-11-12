//! An undefined identifier in an `#if` / `#elif` expression. ESSL 3.00 section 3.4: "Undefined identifiers not
//! consumed by the defined operator do not default to '0'. Use of such identifiers causes an error", and an operand
//! that is not evaluated (the right side of a short-circuited `&&` / `||`) causes no error. The pinned X_ITE page
//! compiles through ANGLE, whose preprocessor reports `'<name>' : unexpected token after conditional expression` at the
//! directive's line (oracle/rea-b24-shaderpart-baseline/include-Fragment-msaa4.json: `ERROR: 3:45:
//! 'X3D_NUM_TEXTURE_COORDINATES' : unexpected token after conditional expression`). An unevaluated division by zero
//! is no error either.

use x3d_render::glsl_compat::{Stage, compile_part};

fn frag(directives: &str) -> Result<(), String> {
    let source = format!(
        "\nprecision mediump float;\n{directives}\nvoid main() {{ gl_FragColor = vec4 (1.0); }}\n"
    );
    compile_part(Stage::Fragment, &source, &[])
}

fn unexpected(line: u32, name: &str) -> Result<(), String> {
    Err(format!(
        "ERROR: 0:{line}: '{name}' : unexpected token after conditional expression"
    ))
}

#[test]
fn an_undefined_identifier_in_if_is_an_error() {
    assert_eq!(frag("#if UNDEF_A > 0\n#endif"), unexpected(3, "UNDEF_A"));
    assert_eq!(frag("#if UNDEF_A\n#endif"), unexpected(3, "UNDEF_A"));
    assert_eq!(frag("#if 1 && UNDEF_A\n#endif"), unexpected(3, "UNDEF_A"));
    assert_eq!(frag("#if 0 || UNDEF_A\n#endif"), unexpected(3, "UNDEF_A"));
    assert_eq!(frag("#if (UNDEF_A + 1)\n#endif"), unexpected(3, "UNDEF_A"));
}

#[test]
fn an_undefined_identifier_in_elif_is_an_error_when_evaluated() {
    assert_eq!(
        frag("#if 0\n#elif UNDEF_A\n#endif"),
        unexpected(4, "UNDEF_A")
    );
    // A branch was taken: the #elif is not evaluated.
    assert_eq!(frag("#if 1\n#elif UNDEF_A\n#endif"), Ok(()));
}

#[test]
fn a_macro_body_with_an_undefined_identifier_is_an_error() {
    assert_eq!(
        frag("#define A_MACRO UNDEF_B\n#if A_MACRO\n#endif"),
        unexpected(4, "UNDEF_B")
    );
    assert_eq!(frag("#define A_MACRO 2\n#if A_MACRO > 1\n#endif"), Ok(()));
}

#[test]
fn a_short_circuited_operand_is_not_evaluated() {
    assert_eq!(frag("#if defined (UNDEF_A) && UNDEF_A > 0\n#endif"), Ok(()));
    assert_eq!(frag("#if defined UNDEF_A && UNDEF_A\n#endif"), Ok(()));
    assert_eq!(frag("#if 1 || UNDEF_A\n#endif"), Ok(()));
    assert_eq!(frag("#if 0 && (UNDEF_A > 1)\n#endif"), Ok(()));
    assert_eq!(frag("#if 0 && 1 / 0\n#endif"), Ok(()));
    assert_eq!(frag("#if 1 || 1 % 0\n#endif"), Ok(()));
    // Only the short-circuited side is ignored.
    assert_eq!(
        frag("#if (0 && UNDEF_A) || UNDEF_B\n#endif"),
        unexpected(3, "UNDEF_B")
    );
}

#[test]
fn an_inactive_group_is_not_evaluated() {
    assert_eq!(frag("#if 0\n#if UNDEF_A\n#endif\n#endif"), Ok(()));
    assert_eq!(frag("#ifdef UNDEF_A\n#if UNDEF_A\n#endif\n#endif"), Ok(()));
}
