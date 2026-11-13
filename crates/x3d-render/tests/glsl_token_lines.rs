//! Two compile-error texts of the pinned vertex-known case (oracle/rea-b24-shaderpart-baseline/vertex-known-msaa4.json):
//! a declaration whose semicolon is missing at the line end, which ANGLE reports at the next token (`ERROR: 0:3:
//! 'attribute' : syntax error`), and a function that returns a value of another type, which ANGLE reports at the
//! `return` (`ERROR: 0:8: 'return' : function return is not matching type:`).

use x3d_render::glsl_compat::{Stage, compile_part};

const MAIN: &str = "void main() { gl_Position = x3d_Vertex; }\n";

#[test]
fn a_missing_semicolon_is_reported_at_the_next_token() {
    let source = format!("\nattribute vec4 x3d_Vertex\nattribute vec3 x3d_Normal;\n{MAIN}");
    assert_eq!(
        compile_part(Stage::Vertex, &source, &[]),
        Err("ERROR: 0:3: 'attribute' : syntax error".into())
    );
    // On one line, the line stays the same.
    let source = format!("\nattribute vec4 x3d_Vertex attribute vec3 x3d_Normal;\n{MAIN}");
    assert_eq!(
        compile_part(Stage::Vertex, &source, &[]),
        Err("ERROR: 0:2: 'attribute' : syntax error".into())
    );
}

#[test]
fn a_return_of_another_type_is_reported_at_the_return() {
    let source =
        format!("\nattribute vec4 x3d_Vertex;\nfloat f (vec2 a)\n{{\n   return a;\n}}\n{MAIN}");
    assert_eq!(
        compile_part(Stage::Vertex, &source, &[]),
        Err("ERROR: 0:5: 'return' : function return is not matching type:".into())
    );
}
