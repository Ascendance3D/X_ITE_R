//! Loops and conditions of a GLSL ES 1.00 fragment ShaderPart (a source without `#version`) against the log of the
//! pinned X_ITE (ANGLE on the owner's GPU), the cases of `errors-loops-runtime.wrl` (oracle/tools/gen_rea_b08_shaders.py
//! LOOP_FRAG; X_ITE 16.5.0 `34cdf9f97`, Chrome 156.0.8078.47, baseline oracle/rea-b08-shaders-baseline/errors-loops-runtime-msaa0,
//! the key of each text from one run per key).
//!
//! - GLSL ES 1.00 Appendix A section 4: only `for` loops are required. ANGLE (ValidateLimitations) applies it to the
//!   WebGL shaders of this version and rejects `while` and `do` with `'<keyword>' : This type of loop is not
//!   allowed`. naga compiles both.
//! - GLSL ES 1.00 sections 6.3 and 5.7: the condition of a `for` loop, of a `while` loop and of `?:` is a scalar bool.
//!   ANGLE reports `'' : boolean expression expected`; naga gives "Function [1] '_x3d_main' is invalid" or
//!   compiles. A float `while` condition gets the loop text: ANGLE checks the loop type first.
//!
//! The sources start with an empty line, as the data URL of the world does. `#version 300 es` sources keep every
//! loop (GLSL ES 3.00 has no Appendix A limits).

use x3d_render::glsl_compat::{Stage, compile_part};

/// (key, source, X_ITE's log; `None`: X_ITE compiled the source).
const CASES: &[(&str, &str, Option<&str>)] = &[
    (
        "a while loop",
        "\nprecision mediump float;\nvoid main() { float x = 1.0;\n while (x > 0.5) { x = 0.0; }\n gl_FragColor = vec4 (x); }\n",
        Some("ERROR: 0:4: 'while' : This type of loop is not allowed"),
    ),
    (
        "b for float condition",
        "\nprecision mediump float;\nvoid main() { float s = 0.0;\n for (float i = 1.0; i; i -= 1.0) { s += 1.0; }\n gl_FragColor = vec4 (s); }\n",
        Some("ERROR: 0:4: '' : boolean expression expected"),
    ),
    (
        "c do-while loop",
        "\nprecision mediump float;\nvoid main() { float x = 1.0;\n do { x = 0.0; } while (x > 0.5);\n gl_FragColor = vec4 (x); }\n",
        Some("ERROR: 0:4: 'do' : This type of loop is not allowed"),
    ),
    (
        "d ternary float condition",
        "\nprecision mediump float;\nvoid main() { float x = 1.0;\n float y = x ? 1.0 : 0.0;\n gl_FragColor = vec4 (y); }\n",
        Some("ERROR: 0:4: '' : boolean expression expected"),
    ),
    (
        "e bool conditions",
        "\nprecision mediump float;\nvoid main() { float x = 0.0;\n for (int i = 0; i < 2; ++ i) { x += 0.4; }\n gl_FragColor = vec4 (x > 0.5 ? 1.0 : 0.0, x, 0.4, 1.0); }\n",
        None,
    ),
    (
        "f while float condition",
        "\nprecision mediump float;\nvoid main() { float x = 1.0;\n while (x) { x = 0.0; }\n gl_FragColor = vec4 (x); }\n",
        Some("ERROR: 0:4: 'while' : This type of loop is not allowed"),
    ),
    (
        "g ternary bvec2 condition",
        "\nprecision mediump float;\nvoid main() { bvec2 b = bvec2 (true);\n float y = b ? 1.0 : 0.0;\n gl_FragColor = vec4 (y); }\n",
        Some("ERROR: 0:4: '' : boolean expression expected"),
    ),
    (
        "h while from a macro",
        "\nprecision mediump float;\n#define W while\nvoid main() { float x = 1.0;\n W (x > 0.5) { x = 0.0; }\n gl_FragColor = vec4 (x); }\n",
        Some("ERROR: 0:5: 'while' : This type of loop is not allowed"),
    ),
    (
        "i unused loop macros",
        "\nprecision mediump float;\n#define LOOP while (true) { }\n#define DO do { } while (false);\nvoid main() { gl_FragColor = vec4 (0.3, 0.6, 0.9, 1.0); }\n",
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
fn loop_and_condition_errors_match_x_ite() {
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

/// The words `while` and `do` in a comment, an identifier or a `#define` that is not used are not loops.
#[test]
fn only_a_loop_statement_is_rejected() {
    let ok = [
        "\nprecision mediump float;\n// while (true) do { }\nvoid main() { float whiled = 1.0; gl_FragColor = vec4 (whiled); }\n",
        "\nprecision mediump float;\n#if 0\nvoid f () { while (true) { } }\n#endif\nvoid main() { gl_FragColor = vec4 (1.0); }\n",
    ];

    for source in ok {
        assert_eq!(rust(source), None, "{source}");
    }
}

/// As case h (recorded): a macro that expands to another macro with a loop, and a function-like macro with a `do` loop,
/// are loops where
/// they are used.
#[test]
fn a_loop_through_macros_is_rejected() {
    assert_eq!(
        rust("\nprecision mediump float;\n#define W while\n#define LOOP W (false)\nvoid main() {\n LOOP { }\n gl_FragColor = vec4 (1.0); }\n").as_deref(),
        Some("ERROR: 0:6: 'while' : This type of loop is not allowed")
    );
    assert_eq!(
        rust("\nprecision mediump float;\n#define ONCE(s) do { s; } while (false)\nvoid main() { float x = 0.0;\n ONCE (x = 1.0);\n gl_FragColor = vec4 (x); }\n").as_deref(),
        Some("ERROR: 0:5: 'do' : This type of loop is not allowed")
    );
}

/// A macro removed by `#undef`, a macro parameter with the name of a loop macro, and a loop macro in an inactive `#if`
/// branch are not loops (review of #599).
#[test]
fn macros_that_are_not_loops() {
    let ok = [
        "\nprecision mediump float;\n#define W while\n#undef W\nfloat W (float v) { return v; }\nvoid main() { gl_FragColor = vec4 (W (1.0)); }\n",
        "\nprecision mediump float;\n#define x while\n#define F(x) x\nvoid main() { float y = F (1.0); gl_FragColor = vec4 (y); }\n",
        "\nprecision mediump float;\n#if 0\n#define W while\n#endif\n#define F(v) v\nvoid main() { gl_FragColor = vec4 (F (1.0)); }\n",
    ];

    for source in ok {
        assert_eq!(rust(source), None, "{source}");
    }
}

/// GLSL ES 3.00 has no Appendix A: while and do-while compile.
#[test]
fn es3_keeps_while_and_do() {
    let es3 = |body: &str| {
        format!(
            "#version 300 es\nprecision mediump float;\nout vec4 color;\nvoid main() {{ float x = 1.0;\n{body}\n color = vec4 (x); }}\n"
        )
    };

    assert_eq!(rust(&es3(" while (x > 0.5) { x = 0.0; }")), None);
    assert_eq!(rust(&es3(" do { x = 0.0; } while (x > 0.5);")), None);
}

/// A `!` that the user wrote on a float is not a loop condition: it keeps naga's text.
#[test]
fn a_user_not_on_a_float_is_not_a_loop_condition() {
    // naga's text (no ANGLE recording of a `!` on a float exists); not the loop condition text.
    for not in ["!x", "! x", "!(x)"] {
        let source = format!(
            "\nprecision mediump float;\nvoid main() {{ float x = 1.0;\n bool b = {not};\n gl_FragColor = vec4 (1.0); }}\n"
        );

        assert_eq!(
            rust(&source).as_deref(),
            Some("ERROR: 0:1: Function [1] '_x3d_main' is invalid"),
            "{not}"
        );
    }
}
