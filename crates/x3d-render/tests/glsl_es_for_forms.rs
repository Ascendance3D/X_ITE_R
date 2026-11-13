//! GLSL ES 1.00 Appendix A section 4 `for` loops in a fragment ShaderPart (a source without `#version`) against the log
//! of the pinned X_ITE (ANGLE on the owner's GPU), the cases of `errors-for-runtime.wrl` (oracle/tools/gen_rea_b08_shaders.py
//! FOR_FRAG; X_ITE 16.5.0 `34cdf9f97`, Chrome 156.0.8078.47, baseline oracle/rea-b08-shaders-baseline/errors-for-runtime-msaa0,
//! the key of each text from one run per key).
//!
//! ANGLE (ValidateLimitations) accepts a `for` loop only with one loop index of type int or float, declared and
//! initialized in the init with a constant expression, a condition `index relop constant`, an expression `++`, `--`,
//! `+=` or `-=` a constant, and no assignment to the index in the body (also not as an `out` or `inout` argument).
//! naga compiles every form. GLSL ES 3.00 sources have no such limits.

use x3d_render::glsl_compat::{Stage, compile_part};

/// (key, source, X_ITE's log; `None`: X_ITE compiled the source).
const CASES: &[(&str, &str, Option<&str>)] = &[
    (
        "a uniform bound",
        "\nprecision mediump float;\nuniform float n;\nvoid main() { float s = 0.0;\n for (float i = 0.0; i < n; i += 1.0) { s += 0.1; }\n gl_FragColor = vec4 (s); }\n",
        Some("ERROR: 0:5: 'i' : Loop index cannot be compared with non-constant expression"),
    ),
    (
        "b index changed in the body",
        "\nprecision mediump float;\nvoid main() { float s = 0.0;\n for (int i = 0; i < 2; ++ i) { i = 3; s += 0.5; }\n gl_FragColor = vec4 (s); }\n",
        Some(
            "ERROR: 0:4: 'i' : Loop index cannot be statically assigned to within the body of the loop",
        ),
    ),
    (
        "c index not declared in the init",
        "\nprecision mediump float;\nvoid main() { float s = 0.0; int i;\n for (i = 0; i < 2; ++ i) { s += 0.5; }\n gl_FragColor = vec4 (s); }\n",
        Some("ERROR: 0:4: 'for' : Invalid init declaration"),
    ),
    (
        "d constant on the left",
        "\nprecision mediump float;\nvoid main() { float s = 0.0;\n for (int i = 0; 2 > i; ++ i) { s += 0.5; }\n gl_FragColor = vec4 (s); }\n",
        Some("ERROR: 0:4: 'for' : Invalid condition"),
    ),
    (
        "e index multiplied",
        "\nprecision mediump float;\nvoid main() { float s = 0.0;\n for (int i = 1; i < 8; i *= 2) { s += 0.2; }\n gl_FragColor = vec4 (s); }\n",
        Some("ERROR: 0:4: '*=' : Invalid operator"),
    ),
    (
        "f no condition",
        "\nprecision mediump float;\nvoid main() { float s = 0.0;\n for (int i = 0; ; ++ i) { s += 0.5; break; }\n gl_FragColor = vec4 (s); }\n",
        Some("ERROR: 0:4: 'for' : Missing condition"),
    ),
    (
        "g variable init",
        "\nprecision mediump float;\nuniform int k;\nvoid main() { float s = 0.0;\n for (int i = k; i < 2; ++ i) { s += 0.5; }\n gl_FragColor = vec4 (s); }\n",
        Some("ERROR: 0:5: 'i' : Loop index cannot be initialized with non-constant expression"),
    ),
    (
        "h valid forms",
        "\nprecision mediump float;\nconst int N = 3;\nvoid main() { float s = 0.0;\n for (int i = N; i >= 0; i -= 1) { s += 0.1; }\n for (float f = 0.0; f != 1.0; f += 0.5) { s += 0.1; }\n gl_FragColor = vec4 (s, 0.5, 0.2, 1.0); }\n",
        None,
    ),
    (
        "i index as inout argument",
        "\nprecision mediump float;\nvoid bump (inout int j) { j += 1; }\nvoid main() { float s = 0.0;\n for (int i = 0; i < 2; ++ i) { bump (i); s += 0.5; }\n gl_FragColor = vec4 (s); }\n",
        Some(
            "ERROR: 0:5: 'i' : Loop index cannot be statically assigned to within the body of the loop",
        ),
    ),
    (
        "j two index variables",
        "\nprecision mediump float;\nvoid main() { float s = 0.0;\n for (int i = 0, j = 1; i < 2; ++ i) { s += 0.5; }\n gl_FragColor = vec4 (s); }\n",
        Some("ERROR: 0:4: 'for' : Invalid init declaration"),
    ),
    (
        "k no init",
        "\nprecision mediump float;\nvoid main() { float s = 0.0; int i = 0;\n for (; i < 2; ++ i) { s += 0.5; }\n gl_FragColor = vec4 (s); }\n",
        Some("ERROR: 0:4: 'for' : Invalid init declaration"),
    ),
    (
        "l no expression",
        "\nprecision mediump float;\nvoid main() { float s = 0.0;\n for (int i = 0; i < 2;) { s += 0.5; break; }\n gl_FragColor = vec4 (s); }\n",
        Some("ERROR: 0:4: 'for' : Missing expression"),
    ),
    (
        "m macro and const bounds",
        "\nprecision mediump float;\n#define N 2\nconst float H = 0.5;\nvoid main() { float s = 0.0;\n for (int i = 0; i < N * 2 + 1; i ++) { s += 0.1; }\n for (float f = 0.0; f < H + float (N); f += H) { s += 0.1; }\n for (int i = x3d_MaxLights; i > 0; i --) { s += 0.01; }\n gl_FragColor = vec4 (s, 0.4, 0.7, 1.0); }\n",
        None,
    ),
    (
        "n step not constant",
        "\nprecision mediump float;\nuniform int k;\nvoid main() { float s = 0.0;\n for (int i = 0; i < 8; i += k) { s += 0.2; }\n gl_FragColor = vec4 (s); }\n",
        Some("ERROR: 0:5: 'i' : Loop index cannot be modified by non-constant expression"),
    ),
    (
        "o inner bound on the outer index",
        "\nprecision mediump float;\nvoid main() { float s = 0.0;\n for (int i = 0; i < 2; ++ i) {\n for (int j = 0; j < i; ++ j) { s += 0.5; } }\n gl_FragColor = vec4 (s); }\n",
        Some("ERROR: 0:5: 'j' : Loop index cannot be compared with non-constant expression"),
    ),
    (
        "p declaration without value",
        "\nprecision mediump float;\nvoid main() { float s = 0.0;\n for (int i; i < 2; ++ i) { s += 0.5; }\n gl_FragColor = vec4 (s); }\n",
        Some("ERROR: 0:4: 'for' : Invalid init declaration"),
    ),
    (
        "q index of type bool",
        "\nprecision mediump float;\nvoid main() { float s = 0.0;\n for (bool b = true; b == true; b = false) { s += 0.5; }\n gl_FragColor = vec4 (s); }\n",
        Some("ERROR: 0:4: 'bool' : Invalid type for loop index"),
    ),
    (
        "r condition with &&",
        "\nprecision mediump float;\nvoid main() { float s = 0.0;\n for (int i = 0; i < 2 && s < 1.0; ++ i) { s += 0.5; }\n gl_FragColor = vec4 (s); }\n",
        Some("ERROR: 0:4: 'for' : Invalid condition"),
    ),
    (
        "s condition not relational",
        "\nprecision mediump float;\nvoid main() { float s = 0.0;\n for (int i = 0; i + 1; ++ i) { s += 0.5; }\n gl_FragColor = vec4 (s); }\n",
        Some("ERROR: 0:4: '' : boolean expression expected"),
    ),
    (
        "t index read and used in the body",
        "\nprecision mediump float;\nuniform float a [3];\nvoid main() { float s = 0.0;\n for (int i = 0; i < 3; ++ i) { float t = float (i); s += a [i] + t; int k = i; k += 1; }\n gl_FragColor = vec4 (s, 0.3, 0.3, 1.0); }\n",
        None,
    ),
];

fn rust(source: &str) -> Option<String> {
    // ShaderCompiler.process runs first in ShaderPart.loadData (it ends every line with a newline).
    compile_part(
        Stage::Fragment,
        &x3d_runtime::nodes::shader_include::process(source),
        &[],
    )
    .err()
}

#[test]
fn for_loop_forms_match_x_ite() {
    let wrong: Vec<String> = CASES
        .iter()
        .filter_map(|(name, source, want)| {
            let got = rust(source);
            (got.as_deref() != *want).then(|| format!("{name}: got {got:?}, X_ITE {want:?}"))
        })
        .collect();

    assert!(
        wrong.is_empty(),
        "{} of {} differ:\n{}",
        wrong.len(),
        CASES.len(),
        wrong.join("\n")
    );
}

/// GLSL ES 3.00 has no Appendix A: the same loops compile.
#[test]
fn es3_keeps_every_for_form() {
    for body in [
        "uniform float n;\nvoid main() { float s = 0.0;\n for (float i = 0.0; i < n; i += 1.0) { s += 0.1; }\n c = vec4 (s); }",
        "void main() { float s = 0.0;\n for (int i = 0; i < 2; ++ i) { i = 3; s += 0.5; }\n c = vec4 (s); }",
        "void main() { float s = 0.0; int i;\n for (i = 0; i < 2; ++ i) { s += 0.5; }\n c = vec4 (s); }",
        "void main() { float s = 0.0;\n for (int i = 1; i < 8; i *= 2) { s += 0.2; }\n c = vec4 (s); }",
    ] {
        let source = format!("#version 300 es\nprecision mediump float;\nout vec4 c;\n{body}\n");

        assert_eq!(rust(&source), None, "{body}");
    }
}
