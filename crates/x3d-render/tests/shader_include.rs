//! `#include <Name>` of a ShaderPart (X_ITE's ShaderCompiler.process): the includes a user shader
//! can use with the supported GLSL subset link. `PROBE=1` prints the result for every include.

use x3d_render::glsl_compat::link_cached;
use x3d_runtime::nodes::shader_include::process;

const VERTEX: &str = "attribute vec4 x3d_Vertex;\nuniform mat4 x3d_ProjectionMatrix;\nuniform mat4 x3d_ModelViewMatrix;\nvarying vec3 vP;\nvoid main() { vec4 p = x3d_ModelViewMatrix * x3d_Vertex; vP = p.xyz; gl_Position = x3d_ProjectionMatrix * p; }\n";

fn link(name: &str) -> Result<(), String> {
    let fragment = format!(
        "precision mediump float;\n#include <{name}>\nvarying vec3 vP;\nvoid main() {{ gl_FragColor = vec4(vP, 1.0); }}\n"
    );
    link_cached(
        &[(process(VERTEX), vec![])],
        &[(process(&fragment), vec![])],
    )
    .map(|_| ())
}

#[test]
fn includes_that_are_plain_glsl_link() {
    for name in ["Utils", "Perlin", "ToneMapping"] {
        assert_eq!(link(name), Ok(()), "{name}");
    }
}

#[test]
fn an_unknown_include_is_an_empty_file() {
    assert_eq!(link("NoSuchFile"), Ok(()));
}

#[test]
fn probe_all_includes() {
    if std::env::var_os("PROBE").is_none() {
        return;
    }

    let list = include_str!("../../x3d-runtime/src/nodes/shader_includes/list.txt");

    for f in list.lines() {
        let name = f[3..f.len() - 5].to_string();
        println!(
            "{name:14} {:?}",
            link(&name).map_err(|e| e.lines().next().unwrap_or("").to_string())
        );
    }
}
