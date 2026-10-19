//! Compile error log text of a fragment ShaderPart against the log of the pinned X_ITE (ANGLE on the owner's
//! GPU), the cases of `errors-runtime.wrl` (oracle/tools/gen_rea_b08_shaders.py ERR_FRAG). The expected text
//! is what X_ITE logged (oracle/rea-batch08-shaders-baseline/errors-runtime-msaa0/js.json, `Error in ShaderPart
//! 'FP' ..., line N, <log>`). The sources start with an empty line, as a data URL of the corpus does.
//! `PROBE=1` prints every case.

use x3d_render::glsl_compat::{compile_part, Stage};
use x3d_runtime::nodes::shader_include::process;

/// (key, source, X_ITE's log; `None`: X_ITE compiled the source).
const CASES: &[(&str, &str, Option<&str>)] = &[
    ("a type mismatch", "\nprecision mediump float;\nvoid main() { gl_FragColor = vec3 (1.0); }\n", Some("ERROR: 0:3: '=' : dimension mismatch")),
    ("b missing semicolon", "\nprecision mediump float;\nvoid main() { gl_FragColor = vec4 (1.0) }\n", Some("ERROR: 0:3: '}' : syntax error")),
    ("c undeclared function", "\nprecision mediump float;\nvoid main() { gl_FragColor = foo (1.0); }\n", Some("ERROR: 0:3: 'foo' : no matching overloaded function found")),
    ("d constructor too many args", "\nprecision mediump float;\nvoid main() { gl_FragColor = vec4 (1.0, 2.0, 3.0, 4.0, 5.0); }\n", Some("ERROR: 0:3: 'constructor' : too many arguments")),
    ("e no main", "\nprecision mediump float;\nfloat f () { return 1.0; }\n", Some("ERROR: -1:-1: '' : Missing main()")),
    ("f #error", "\nprecision mediump float;\n#error custom failure\nvoid main() { gl_FragColor = vec4 (1.0); }\n", Some("ERROR: 0:3: '' :  custom failure")),
    ("g no fragment precision", "\nvoid main() { gl_FragColor = vec4 (1.0); }\n", None),
    ("h int to float", "\nprecision mediump float;\nvoid main() { float x = 1; gl_FragColor = vec4 (x); }\n", Some("ERROR: 0:3: '=' : cannot convert from 'const int' to 'mediump float'")),
    ("i redefinition", "\nprecision mediump float;\nfloat a;\nfloat a;\nvoid main() { gl_FragColor = vec4 (1.0); }\n", Some("ERROR: 0:4: 'a' : redefinition")),
    ("j gl_Position in fragment", "\nprecision mediump float;\nvoid main() { gl_Position = vec4 (1.0); gl_FragColor = vec4 (1.0); }\n", Some("ERROR: 0:3: 'gl_Position' : undeclared identifier")),
    ("k if not bool", "\nprecision mediump float;\nvoid main() { if (1.0) { gl_FragColor = vec4 (1.0); } }\n", Some("ERROR: 0:3: '' : boolean expression expected")),
    ("l assign to uniform", "\nprecision mediump float;\nuniform float u;\nvoid main() { u = 1.0; gl_FragColor = vec4 (u); }\n", Some("ERROR: 0:4: 'assign' : l-value required (can't modify a uniform \"u\")")),
    ("m bad swizzle", "\nprecision mediump float;\nvoid main() { vec2 v = vec2 (1.0); gl_FragColor = vec4 (v.z); }\n", Some("ERROR: 0:3: 'z' : vector field selection out of range")),
    ("n array index", "\nprecision mediump float;\nvoid main() { float a [2]; a [3] = 1.0; gl_FragColor = vec4 (a [0]); }\n", Some("ERROR: 0:3: '[]' : array index out of range")),
    ("o wrong arg type", "\nprecision mediump float;\nvoid main() { gl_FragColor = vec4 (sin (vec2 (1.0), 2.0)); }\n", Some("ERROR: 0:3: 'sin' : no matching overloaded function found")),
    ("p unbalanced brace", "\nprecision mediump float;\nvoid main() { gl_FragColor = vec4 (1.0); \n", Some("ERROR: 0:5: '' : syntax error")),
];

/// Cases whose log differs: naga type checks GLSL 4.50 (it converts int to float, accepts a constructor with too
/// many arguments, and reports a type error in a function as "Function is invalid" without a line), ANGLE checks
/// GLSL ES 1.00. A full ES type checker is not in this port; these four stay different (a shader that X_ITE
/// rejects and falls back from is then accepted, or rejected with another text).
const GAPS: &[&str] = &["a type mismatch", "d constructor too many args", "h int to float", "k if not bool"];

fn rust(source: &str) -> Option<String> {
    // ShaderCompiler.process runs first in ShaderPart.loadData (it ends every line with a newline).
    compile_part(Stage::Fragment, &process(source), &[]).err()
}

#[test]
fn compile_error_text_matches_x_ite() {
    let probe = std::env::var_os("PROBE").is_some();
    let mut wrong = vec![];
    let mut checked = 0;

    for (name, source, want) in CASES {
        let got = rust(source);

        if probe {
            println!("{name:28} got {got:?}\n{:28} want {want:?}", "");
        }

        if GAPS.contains(name) {
            continue;
        }

        checked += 1;

        if got.as_deref() != *want {
            wrong.push(format!("{name}: got {got:?}, X_ITE {want:?}"));
        }
    }

    assert_eq!(checked, CASES.len() - GAPS.len());
    assert!(wrong.is_empty(), "{} of {} differ:\n{}", wrong.len(), checked, wrong.join("\n"));
}

#[test]
fn known_gaps_still_differ() {
    for (name, source, want) in CASES.iter().filter(|c| GAPS.contains(&c.0)) {
        assert_ne!(rust(source).as_deref(), *want, "{name} now matches X_ITE: move it out of GAPS");
    }
}
