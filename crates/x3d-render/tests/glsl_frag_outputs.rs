//! Fragment outputs of a GLSL ES 3.00 ShaderPart without `layout (location)`. ESSL 3.00 section 4.3.8.2: with more than
//! one fragment output, every output must have a location. ANGLE (ValidateOutputs, run after the shader parses) reports
//! each output that has none, at its declaration; the pinned X_ITE 16.5.0 page logs the first one
//! (oracle/rea-b24-shaderpart-baseline/include-PointSize-msaa4.json: `ERROR: 14:6: 'pointSize' : when
//! EXT_blend_func_extended extension is not enabled, must explicitly specify all locations when using multiple fragment
//! outputs`). One output without a location is allowed (it is location 0).

use x3d_render::glsl_compat::{Stage, compile_part};

fn frag(decls: &str) -> Result<(), String> {
    let source = format!(
        "#version 300 es\nprecision mediump float;\n{decls}\nvoid main() {{ fragColor = vec4 (1.0); }}\n"
    );
    compile_part(Stage::Fragment, &source, &[])
}

fn unlocated(line: u32, name: &str) -> Result<(), String> {
    Err(format!(
        "ERROR: 0:{line}: '{name}' : when EXT_blend_func_extended extension is not enabled, must explicitly specify all locations when using multiple fragment outputs"
    ))
}

#[test]
fn one_output_needs_no_location() {
    assert_eq!(frag("out vec4 fragColor;"), Ok(()));
    assert_eq!(frag("layout (location = 0) out vec4 fragColor;"), Ok(()));
}

#[test]
fn two_outputs_without_locations_are_an_error() {
    assert_eq!(
        frag("out float pointSize;\nout vec4 fragColor;"),
        unlocated(3, "pointSize")
    );
    assert_eq!(
        frag("out vec4 fragColor;\nout float pointSize;"),
        unlocated(3, "fragColor")
    );
}

#[test]
fn an_output_without_a_location_beside_one_with_a_location_is_an_error() {
    assert_eq!(
        frag("layout (location = 1) out float extra;\nout vec4 fragColor;"),
        unlocated(4, "fragColor")
    );
}

#[test]
fn outputs_with_locations_are_accepted() {
    assert_eq!(
        frag("layout (location = 0) out vec4 fragColor;\nlayout (location = 1) out vec4 extra;"),
        Ok(())
    );
}

#[test]
fn an_earlier_compile_error_comes_first() {
    // The output check runs after the shader parses, as in ANGLE.
    let source = "#version 300 es\nprecision mediump float;\nout float pointSize;\nout vec4 fragColor;\nvoid main() { fragColor = vec4 (1.0) }\n";
    assert_eq!(
        compile_part(Stage::Fragment, source, &[]),
        Err("ERROR: 0:5: '}' : syntax error".into())
    );
}
