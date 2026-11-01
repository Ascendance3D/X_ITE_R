//! Issue #95: compile error text of a fragment ShaderPart that includes a ShaderRegistry file without the macros
//! and uniforms it needs, against the log of pinned X_ITE 16.5.0 (oracle/rea-b24-shaderpart-baseline/include-*,
//! worlds oracle/synthetic-rea-b24-shaderpart/include-*.wrl). ANGLE reports `<file number>:<line in the file>` from
//! the `#line` directives of ShaderCompiler.process, and X_ITE names the include file; the log of this port counts
//! physical lines, so `remap_log` rewrites it. The stepped comparison of the same worlds (pictures, scene text and
//! the whole log) is crates/x3d-viewer/tests/rea_b24_shaderpart.rs; the includes whose text differs are KNOWN there.

use x3d_render::glsl_compat::{Stage, compile_part};
use x3d_runtime::nodes::shader_include::{process, remap_log, source_file_name};

const PRECISION: &str = "precision highp int;\nprecision highp float;\nprecision highp sampler2D;\nprecision highp sampler3D;\nprecision highp samplerCube;\n";

fn source(include: &str) -> String {
    format!(
        "#version 300 es\n{PRECISION}#include <{include}>\nout vec4 fragColor;\nvoid main() {{ fragColor = vec4 (0.3, 0.7, 0.9, 1.0); }}\n"
    )
}

/// The `line N, ERROR: ...` tail and the file of X_ITE's one error line of `include-<name>`.
fn x_ite(name: &str) -> (String, String) {
    let path = format!(
        "{}/../../oracle/rea-b24-shaderpart-baseline/include-{name}-msaa0/js.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let log = v["log"].as_array().unwrap();
    assert_eq!(log.len(), 1, "{log:?}");
    let line = log[0].as_str().unwrap();
    let (_, rest) = line.split_once("in URL '").unwrap();
    let (file, tail) = rest.split_once("', ").unwrap();

    (file.to_string(), tail.to_string())
}

/// What the ShaderPart error message of this port is made of for a compile error.
fn rust(name: &str) -> (String, String) {
    let processed = process(&source(name));
    let log = compile_part(Stage::Fragment, &processed, &[]).expect_err("must not compile");
    let log = remap_log(&processed, &log);
    let m = log.split_once(": ").unwrap().1;
    let (file, line) = m.split_once(':').unwrap();
    let line = line.split(':').next().unwrap();

    (
        source_file_name(file).unwrap_or("<world>").to_string(),
        format!("line {line}, {log}"),
    )
}

#[test]
fn include_compile_errors_name_the_include_file_and_line_like_x_ite() {
    for name in ["Fog", "Normal", "MaterialInfo"] {
        assert_eq!(rust(name), x_ite(name), "{name}");
    }
}

#[test]
fn remap_log_follows_the_line_directives() {
    let src = "a\n#line 1 2\nb\nc\n#line 4 0\nd\n";
    assert_eq!(remap_log(src, "ERROR: 0:3: x"), "ERROR: 2:1: x");
    assert_eq!(remap_log(src, "ERROR: 0:4: y"), "ERROR: 2:2: y");
    assert_eq!(remap_log(src, "ERROR: 0:6: z"), "ERROR: 0:4: z");
    assert_eq!(remap_log(src, "ERROR: 0:1: w"), "ERROR: 0:1: w");
    assert_eq!(source_file_name("2"), Some("Fog"));
    assert_eq!(source_file_name("0"), None);
}
