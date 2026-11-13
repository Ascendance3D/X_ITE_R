//! An interpolation qualifier (`flat`, `smooth`, `centroid`) on a fragment output or a vertex input. ESSL 3.00 section
//! 4.3.9: interpolation qualifiers apply to vertex outputs and fragment inputs only. ANGLE (QualifierTypes.cpp
//! JoinVariableStorageQualifier) rejects the combination with `'<storage>' : invalid qualifier combination`; the pinned
//! X_ITE 16.5.0 page logs it for the Vertex include in a fragment part (oracle/rea-b24-shaderpart-baseline/
//! include-Vertex-msaa4.json: `ERROR: 20:114: 'out' : invalid qualifier combination`, the line
//! `flat out float lengthSoFar;`).

use x3d_render::glsl_compat::{Stage, compile_part};

fn part(stage: Stage, decls: &str, main: &str) -> Result<(), String> {
    let source =
        format!("#version 300 es\nprecision mediump float;\n{decls}\nvoid main() {{ {main} }}\n");
    compile_part(stage, &source, &[])
}

fn invalid(line: u32, storage: &str) -> Result<(), String> {
    Err(format!(
        "ERROR: 0:{line}: '{storage}' : invalid qualifier combination"
    ))
}

const FRAG_MAIN: &str = "fragColor = vec4 (1.0);";

#[test]
fn an_interpolated_fragment_output_is_invalid() {
    for q in ["flat", "smooth", "centroid"] {
        assert_eq!(
            part(
                Stage::Fragment,
                &format!("{q} out vec4 fragColor;"),
                FRAG_MAIN
            ),
            invalid(3, "out"),
            "{q}"
        );
    }
}

#[test]
fn an_interpolated_vertex_input_is_invalid() {
    assert_eq!(
        part(
            Stage::Vertex,
            "flat in vec4 position;",
            "gl_Position = position;"
        ),
        invalid(3, "in")
    );
}

#[test]
fn interpolated_varyings_are_valid() {
    assert_eq!(
        part(
            Stage::Fragment,
            "flat in float a;\nsmooth in float b;\ncentroid in float c;\nout vec4 fragColor;",
            "fragColor = vec4 (a + b + c);"
        ),
        Ok(())
    );
    assert_eq!(
        part(
            Stage::Vertex,
            "flat out float a;\nsmooth out float b;\ncentroid out float c;",
            "a = 1.0; b = 1.0; c = 1.0; gl_Position = vec4 (1.0);"
        ),
        Ok(())
    );
}
