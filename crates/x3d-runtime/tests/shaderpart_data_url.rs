//! A ShaderPart data URL gives the shader compiler the text pinned X_ITE 16.5.0 gives `gl.shaderSource` in Chrome
//! 156.0.8078.47 (PR #124 correction pass, `evidence/shaderpart/shaderpart-xite-5runs.json` of the lane report: 5
//! runs, identical values). Each case is a FRAGMENT ShaderPart with `url [ case, FALLBACK ]`.
//!
//! X_ITE's ShaderPart loads with `dataAsString` (FileLoader's default). A URL that starts with lower-case `data:`
//! after `\s*` and is not `;base64` goes through FileLoader's text branch: the authored text after the comma,
//! `decodeURIComponent`ed, with its raw tabs, newlines and `#`. Every other data URL goes through
//! `fetch (new URL (url))`, where Chrome's URL parser and data URL processor decide the bytes.
//!
//! The rows after `crExample` come from the CodeRabbit verification of PR #124 (`evidence/coderabbit/cr-xite-5runs.json`:
//! 5 runs, identical values): raw tab, LF and CR after `;base64` keep the text branch (the header does not end with
//! `;base64`), and `\s*` is JavaScript's, which takes U+FEFF and not U+0085.

use std::cell::RefCell;
use std::rc::Rc;

use x3d_runtime::nodes::shaders::{ShaderCompiler, ShaderSource};
use x3d_runtime::{Fetcher, Runtime};

const WORLD: &str = "http://oracle/t.x3dv";
const VALID_FS: &str =
    "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n";

/// The recording server answers 404 for every URL that is not a data URL.
struct NotFound;

impl Fetcher for NotFound {
    fn fetch(&self, _: &url::Url) -> Result<Vec<u8>, String> {
        Err("Error: Not Found".into())
    }
}

/// Records the FRAGMENT sources. It compiles what the GPU compiled in the recording: a source that starts with
/// a `#version 300 es` line and a precision line (the recorded logs of the others are driver text, not compared).
struct Recorder(Rc<RefCell<Vec<String>>>);

impl ShaderCompiler for Recorder {
    fn compile_part(&self, fragment: bool, source: &str, _: &[String]) -> Result<(), String> {
        if fragment {
            self.0.borrow_mut().push(source.to_string());
        }

        if source.starts_with("#version 300 es\nprecision mediump float;\n") {
            Ok(())
        } else {
            Err("ERROR: 0:1: 'recorder' : syntax error".into())
        }
    }

    fn link(
        &self,
        _: &[ShaderSource],
        _: &[ShaderSource],
        _: &[String],
    ) -> Result<Vec<String>, String> {
        Ok(vec![])
    }
}

struct Row {
    name: &'static str,
    url: &'static str,
    state: &'static str,
    sources: &'static [&'static str],
    message: &'static str,
}

/// An MFString element in the classic encoding: only `"` and `\` are escaped, every other character is authored.
fn mfstring(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn fallback() -> String {
    let text = format!("{VALID_FS}// fallback\n");
    let encoded: String = text
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect();

    format!("data:x-shader/x-fragment,{encoded}")
}

fn run(url: &str) -> (Vec<String>, String, Vec<String>) {
    let body = format!(
        "#X3D V4.0 utf8\nPROFILE Interchange\nCOMPONENT Shaders : 1\nShape {{\n  appearance Appearance {{\n    shaders ComposedShader {{\n      language \"GLSL\"\n      parts [\n        DEF F ShaderPart {{ type \"FRAGMENT\" url [ {} {} ] }}\n      ]\n    }}\n  }}\n}}\n",
        mfstring(url),
        mfstring(&fallback())
    );
    let scene = x3d_parse::parse_bytes(body.as_bytes(), WORLD).unwrap();
    let mut rt = Runtime::new(scene, Box::new(NotFound));
    let sources = Rc::new(RefCell::new(vec![]));

    rt.shaders.compiler = Some(Box::new(Recorder(sources.clone())));
    rt.load_world();

    let node = rt.scene.get_named_node(x3d_scene::ROOT, "F").unwrap();
    let state = format!("{:?}", rt.url_load_state(node)).to_ascii_uppercase();
    let log = rt.log.clone();

    (sources.take(), state, log)
}

/// The first ShaderPart message, as the recording keeps it: the compile log cut, the world URL named `<world>`.
fn first_message(log: &[String]) -> String {
    let Some(line) = log.iter().find(|l| l.contains("ShaderPart")) else {
        return String::new();
    };
    let line = line
        .replacen("warn: ", "warning: ", 1)
        .replace(WORLD, "<world>");
    let cut = [", line ", ", ERROR: "]
        .iter()
        .filter_map(|p| line.find(p))
        .min()
        .unwrap_or(line.len());

    line[..cut].to_string()
}

#[test]
fn shader_part_data_urls_give_the_text_of_pinned_x_ite() {
    let mut differ = vec![];

    for row in ROWS {
        let (sources, state, log) = run(row.url);
        let sources: Vec<&str> = sources.iter().map(String::as_str).collect();
        let message = first_message(&log);

        if sources != row.sources {
            differ.push(format!(
                "{}: sources {sources:?}, X_ITE {:?}",
                row.name, row.sources
            ));
        }

        if state != row.state {
            differ.push(format!("{}: state {state}, X_ITE {}", row.name, row.state));
        }

        if message != row.message {
            differ.push(format!(
                "{}: message {message:?}\n    X_ITE {:?}",
                row.name, row.message
            ));
        }
    }

    assert!(
        differ.is_empty(),
        "{} differences:\n{}",
        differ.len(),
        differ.join("\n")
    );
}

// ROWS (generated from the recording by gen_rows.py of the lane report).
#[rustfmt::skip]
const ROWS: &[Row] = &[
    Row { name: "schemeTab", url: "da\tta:,XBa\tbXE", state: "COMPLETE", sources: &["XBabXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:,XBabXE'" },
    Row { name: "spaceUpper", url: " DATA:,XBa\tbXE", state: "COMPLETE", sources: &["XBabXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:,XBabXE'" },
    Row { name: "upper", url: "DATA:,XBa\tbXE", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:,XBa%09bXE'" },
    Row { name: "tabUpper", url: "\tDATA:,XBa\tbXE", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:,XBa%09bXE'" },
    Row { name: "lower", url: "data:,XBa\tbXE", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "spaceLower", url: " data:,XBa\tbXE", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "tabLower", url: "\tdata:,XBa\tbXE", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "mixed", url: "Data:,XBa\tbXE", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:,XBa%09bXE'" },
    Row { name: "schemeTabEnd", url: "data\t:,XBa\tbXE", state: "COMPLETE", sources: &["XBabXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:,XBabXE'" },
    Row { name: "schemeTabNewline", url: "\tda\nta:,XBa\tbXE", state: "COMPLETE", sources: &["XBabXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:,XBabXE'" },
    Row { name: "upperTabAfter", url: "DATA:\t,XBa\tbXE", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:%09,XBa%09bXE'" },
    Row { name: "lowerTabAfter", url: "data:\t,XBa\tbXE", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "lowerNewlineAfter", url: "data:\n,XBa\tbXE", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "upperPercent", url: "DATA:,XBa%09b%20c%0AXE", state: "COMPLETE", sources: &["XBa\tb c\nXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:,XBa%09b%20c%0AXE'" },
    Row { name: "lowerPercent", url: "data:,XBa%09b%20c%0AXE", state: "COMPLETE", sources: &["XBa\tb c\nXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "upperBadEscape", url: "DATA:,XBa%E0%ZZXE", state: "COMPLETE", sources: &["XBa\u{fffd}%ZZXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:,XBa%E0%ZZXE'" },
    Row { name: "lowerBadEscape", url: "data:,XBa%E0%ZZXE", state: "COMPLETE", sources: &["XBa%E0%ZZXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "lowerBom", url: "data:,%EF%BB%BFXBabXE", state: "COMPLETE", sources: &["\u{feff}XBabXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "upperBom", url: "DATA:,%EF%BB%BFXBabXE", state: "COMPLETE", sources: &["XBabXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:,%EF%BB%BFXBabXE'" },
    Row { name: "upperNewlines", url: "DATA:,XBa\nb\r\ncXE", state: "COMPLETE", sources: &["XBa\nb\r\ncXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:,XBa%0Ab%0D%0AcXE'" },
    Row { name: "lowerNewlines", url: "data:,XBa\nb\r\ncXE", state: "COMPLETE", sources: &["XBa\nb\r\ncXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "spaceUpperNewlines", url: " DATA:,XBa\nb\r\ncXE", state: "COMPLETE", sources: &["XBabcXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:,XBabcXE'" },
    Row { name: "upperHash", url: "DATA:,XBa#bXE", state: "COMPLETE", sources: &["XBa\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:,XBa#bXE'" },
    Row { name: "lowerHash", url: "data:,XBa#bXE", state: "COMPLETE", sources: &["XBa#bXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "lowerBase64", url: "data:;base64,WEJhCWJYRQ==", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64,WEJhCWJYRQ=='" },
    Row { name: "upperBase64", url: "DATA:;base64,WEJhCWJYRQ==", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64,WEJhCWJYRQ=='" },
    Row { name: "lowerBASE64", url: "data:;BASE64,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "lowerSpaceBase64", url: "data:; base64,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "lowerBase64Ws", url: "data:;base64,WEJh \nCWJY \nRQ== \n", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64,WEJh %0ACWJY %0ARQ=='" },
    Row { name: "lowerBase64Pct", url: "data:;base64,WEJhYlhFIQ%3D%3D", state: "COMPLETE", sources: &["XBabXE!\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64,WEJhYlhFIQ%3D%3D'" },
    Row { name: "lowerTabBase64", url: "data:;\tbase64,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "schemeTabBase64", url: "da\tta:;base64,WEJhCWJYRQ==", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64,WEJhCWJYRQ=='" },
    Row { name: "base64UrlAlphabet", url: "data:;base64,XB-_", state: "COMPLETE", sources: &["#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. TypeError: Failed to fetch" },
    Row { name: "base64BadPadding", url: "data:;base64,QUJDRA=", state: "COMPLETE", sources: &["#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. TypeError: Failed to fetch" },
    Row { name: "base64BadChar", url: "data:;base64,QUJD*", state: "COMPLETE", sources: &["#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. TypeError: Failed to fetch" },
    Row { name: "base64Len1", url: "data:;base64,QUJDR", state: "COMPLETE", sources: &["#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. TypeError: Failed to fetch" },
    Row { name: "upperNoComma", url: "DATA:XB", state: "COMPLETE", sources: &["#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. TypeError: Failed to fetch" },
    Row { name: "lowerNoComma", url: "data:XB", state: "COMPLETE", sources: &["#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. TypeError: Failed to fetch" },
    Row { name: "validLowerRaw", url: "data:x-shader/x-fragment,#version 300 es\nprecision mediump float;\n\tout vec4 c;\r\nvoid main ()\n{\n\tc = vec4 (1.0);\n}\n", state: "COMPLETE", sources: &["#version 300 es\nprecision mediump float;\n\tout vec4 c;\r\nvoid main ()\n{\n\tc = vec4 (1.0);\n}\n\n"], message: "" },
    Row { name: "validUpperRaw", url: "DATA:x-shader/x-fragment,%23version 300 es\nprecision mediump float;\n\tout vec4 c;\r\nvoid main ()\n{\n\tc = vec4 (1.0);\n}\n", state: "COMPLETE", sources: &["#version 300 es\nprecision mediump float;\n\tout vec4 c;\r\nvoid main ()\n{\n\tc = vec4 (1.0);\n}\n"], message: "" },
    Row { name: "validSpaceUpperRaw", url: " DATA:x-shader/x-fragment,%23version 300 es\nprecision mediump float;\n\tout vec4 c;\r\nvoid main ()\n{\n\tc = vec4 (1.0);\n}\n", state: "COMPLETE", sources: &["#version 300 esprecision mediump float;out vec4 c;void main (){c = vec4 (1.0);}\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:x-shader/x-fragment,%23version 300 esprecision mediump float;out vec4 c;void main (){c = vec4 (1.0);}'" },
    Row { name: "validUpperHash", url: "DATA:x-shader/x-fragment,#version 300 es\nprecision mediump float;\n\tout vec4 c;\r\nvoid main ()\n{\n\tc = vec4 (1.0);\n}\n", state: "COMPLETE", sources: &["\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:x-shader/x-fragment,#version%20300%20es%0Aprecision%20mediump%20float;%0A%09out%20vec4%20c;%0D%0Avoid%20main%20()%0A{%0A%09c%20=%20vec4%20(1.0);%0A}'" },
    Row { name: "validBase64", url: "data:x-shader/x-fragment;base64,I3ZlcnNpb24gMzAwIGVzCnByZWNpc2lvbiBtZWRpdW1wIGZsb2F0OwoJb3V0IHZlYzQgYzsNCnZvaWQgbWFpbiAoKQp7CgljID0gdmVjNCAoMS4wKTsKfQo=", state: "COMPLETE", sources: &["#version 300 es\nprecision mediump float;\n\tout vec4 c;\r\nvoid main ()\n{\n\tc = vec4 (1.0);\n}\n\n"], message: "" },
    Row { name: "validEncoded", url: "DATA:x-shader/x-fragment,%23version%20300%20es%0Aprecision%20mediump%20float%3B%0A%09out%20vec4%20c%3B%0D%0Avoid%20main%20()%0A%7B%0A%09c%20%3D%20vec4%20(1.0)%3B%0A%7D%0A", state: "COMPLETE", sources: &["#version 300 es\nprecision mediump float;\n\tout vec4 c;\r\nvoid main ()\n{\n\tc = vec4 (1.0);\n}\n\n"], message: "" },
    Row { name: "invalidThenFallback", url: "data:x-shader/x-fragment,void main () { syntax error }", state: "COMPLETE", sources: &["void main () { syntax error }\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    // CodeRabbit verification of PR #124 (XITE_R_PR124_FINAL_REVIEW_01).
    Row { name: "crExample", url: "data:text/plain;base64\r,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "lowerB64Cr", url: "data:;base64\r,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "lowerB64Lf", url: "data:;base64\n,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "lowerB64Tab", url: "data:;base64\t,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "lowerB64CrLf", url: "data:;base64\r\n,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "upperB64Cr", url: "DATA:;base64\r,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64%0D,WEJhCWJYRQ=='" },
    Row { name: "upperB64Lf", url: "DATA:;base64\n,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64%0A,WEJhCWJYRQ=='" },
    Row { name: "upperB64Tab", url: "DATA:;base64\t,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64%09,WEJhCWJYRQ=='" },
    Row { name: "mixedB64Cr", url: "Data:;base64\r,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64%0D,WEJhCWJYRQ=='" },
    Row { name: "mixedB64Lf", url: "dAtA:;base64\n,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64%0A,WEJhCWJYRQ=='" },
    Row { name: "lowerB64InnerCr", url: "data:;base\r64,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "upperB64InnerTab", url: "DATA:;base\t64,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base%0964,WEJhCWJYRQ=='" },
    Row { name: "lowerB64BodyCtl", url: "data:;base64,WEJh\tCWJY\nRQ==\r", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64,WEJh%09CWJY%0ARQ=='" },
    Row { name: "upperB64BodyCtl", url: "DATA:;base64,WEJh\tCWJY\nRQ==\r", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64,WEJh%09CWJY%0ARQ=='" },
    Row { name: "lowerTextBodyCtl", url: "data:,XB\ta\nb\rXE", state: "COMPLETE", sources: &["XB\ta\nb\rXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "upperTextBodyCtl", url: "DATA:,XB\ta\nb\rXE", state: "COMPLETE", sources: &["XB\ta\nb\rXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:,XB%09a%0Ab%0DXE'" },
    Row { name: "spaceLowerB64Cr", url: " data:;base64\r,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "tabLowerB64Cr", url: "\tdata:;base64\r,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "lfLowerB64", url: "\ndata:;base64,WEJhCWJYRQ==", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64,WEJhCWJYRQ=='" },
    Row { name: "bomLower", url: "\u{feff}data:,XBa\tbXE", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load URL 'http://oracle/\u{feff}data:,XBabXE', trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "bomLowerB64", url: "\u{feff}data:;base64,WEJhCWJYRQ==", state: "COMPLETE", sources: &["#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load URL 'http://oracle/\u{feff}data:;base64,WEJhCWJYRQ==', trying URL 2 of 2. Error: Not Found" },
    Row { name: "bomLowerB64Cr", url: "\u{feff}data:;base64\r,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load URL 'http://oracle/\u{feff}data:;base64,WEJhCWJYRQ==', trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "bomUpper", url: "\u{feff}DATA:,XBa\tbXE", state: "COMPLETE", sources: &["#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load URL 'http://oracle/\u{feff}DATA:,XBabXE', trying URL 2 of 2. Error: Not Found" },
    Row { name: "nbspLower", url: "\u{a0}data:,XBa\tbXE", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load URL 'http://oracle/\u{a0}data:,XBabXE', trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "ideoSpaceLower", url: "\u{3000}data:,XBa\tbXE", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load URL 'http://oracle/\u{3000}data:,XBabXE', trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "nelLower", url: "\u{85}data:,XBa\tbXE", state: "COMPLETE", sources: &["#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load URL 'http://oracle/\u{85}data:,XBabXE', trying URL 2 of 2. Error: Not Found" },
    Row { name: "lsepLower", url: "\u{2028}data:,XBa\tbXE", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load URL 'http://oracle/\u{2028}data:,XBabXE', trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "lowerB64PctCr", url: "data:;base64%0D,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "lowerB64PctLf", url: "data:;base64%0A,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "lowerB64PctTab", url: "data:;base64%09,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "upperB64PctCr", url: "DATA:;base64%0D,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64%0D,WEJhCWJYRQ=='" },
    Row { name: "upperB64PctTab", url: "DATA:;base64%09,WEJhCWJYRQ==", state: "COMPLETE", sources: &["WEJhCWJYRQ==\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64%09,WEJhCWJYRQ=='" },
    Row { name: "lowerB64BodyPctCtl", url: "data:;base64,WEJh%09CWJY%0ARQ==%0D", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64,WEJh%09CWJY%0ARQ==%0D'" },
    Row { name: "upperB64BodyPctCtl", url: "DATA:;base64,WEJh%09CWJY%0ARQ==%0D", state: "COMPLETE", sources: &["XBa\tbXE\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64,WEJh%09CWJY%0ARQ==%0D'" },
    Row { name: "lowerPctData", url: "%64ata:,XBa\tbXE", state: "COMPLETE", sources: &["#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load URL 'http://oracle/data:,XBabXE', trying URL 2 of 2. Error: Not Found" },
    Row { name: "lowerB64CrBad", url: "data:;base64\r,QUJD*", state: "COMPLETE", sources: &["QUJD*\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "upperB64CrBad", url: "DATA:;base64\r,QUJD*", state: "COMPLETE", sources: &["QUJD*\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64%0D,QUJD*'" },
    Row { name: "upperB64LfBadPad", url: "DATA:;base64\n,QUJDRA=", state: "COMPLETE", sources: &["QUJDRA=\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:;base64%0A,QUJDRA='" },
    Row { name: "validLowerB64Cr", url: "data:x-shader/x-fragment;base64\r,I3ZlcnNpb24gMzAwIGVzCnByZWNpc2lvbiBtZWRpdW1wIGZsb2F0OwoJb3V0IHZlYzQgYzsNCnZvaWQgbWFpbiAoKQp7CgljID0gdmVjNCAoMS4wKTsKfQo=", state: "COMPLETE", sources: &["I3ZlcnNpb24gMzAwIGVzCnByZWNpc2lvbiBtZWRpdW1wIGZsb2F0OwoJb3V0IHZlYzQgYzsNCnZvaWQgbWFpbiAoKQp7CgljID0gdmVjNCAoMS4wKTsKfQo=\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "validUpperB64Cr", url: "DATA:x-shader/x-fragment;base64\r,I3ZlcnNpb24gMzAwIGVzCnByZWNpc2lvbiBtZWRpdW1wIGZsb2F0OwoJb3V0IHZlYzQgYzsNCnZvaWQgbWFpbiAoKQp7CgljID0gdmVjNCAoMS4wKTsKfQo=", state: "COMPLETE", sources: &["I3ZlcnNpb24gMzAwIGVzCnByZWNpc2lvbiBtZWRpdW1wIGZsb2F0OwoJb3V0IHZlYzQgYzsNCnZvaWQgbWFpbiAoKQp7CgljID0gdmVjNCAoMS4wKTsKfQo=\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:x-shader/x-fragment;base64%0D,I3ZlcnNpb24gMzAwIGVzCnByZWNpc2lvbiBtZWRpdW1wIGZsb2F0OwoJb3V0IHZlYzQgYzsNCnZvaWQgbWFpbiAoKQp7CgljID0gdmVjNCAoMS4wKTsKfQo='" },
    Row { name: "validLowerB64Lf", url: "data:x-shader/x-fragment;base64\n,I3ZlcnNpb24gMzAwIGVzCnByZWNpc2lvbiBtZWRpdW1wIGZsb2F0OwoJb3V0IHZlYzQgYzsNCnZvaWQgbWFpbiAoKQp7CgljID0gdmVjNCAoMS4wKTsKfQo=", state: "COMPLETE", sources: &["I3ZlcnNpb24gMzAwIGVzCnByZWNpc2lvbiBtZWRpdW1wIGZsb2F0OwoJb3V0IHZlYzQgYzsNCnZvaWQgbWFpbiAoKQp7CgljID0gdmVjNCAoMS4wKTsKfQo=\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL '<world>'" },
    Row { name: "validMixedB64Tab", url: "Data:x-shader/x-fragment;base64\t,I3ZlcnNpb24gMzAwIGVzCnByZWNpc2lvbiBtZWRpdW1wIGZsb2F0OwoJb3V0IHZlYzQgYzsNCnZvaWQgbWFpbiAoKQp7CgljID0gdmVjNCAoMS4wKTsKfQo=", state: "COMPLETE", sources: &["I3ZlcnNpb24gMzAwIGVzCnByZWNpc2lvbiBtZWRpdW1wIGZsb2F0OwoJb3V0IHZlYzQgYzsNCnZvaWQgbWFpbiAoKQp7CgljID0gdmVjNCAoMS4wKTsKfQo=\n", "#version 300 es\nprecision mediump float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n// fallback\n\n"], message: "warning: ShaderPart: Couldn't load data URL, trying URL 2 of 2. Error: Error in ShaderPart 'F' in URL 'data:x-shader/x-fragment;base64%09,I3ZlcnNpb24gMzAwIGVzCnByZWNpc2lvbiBtZWRpdW1wIGZsb2F0OwoJb3V0IHZlYzQgYzsNCnZvaWQgbWFpbiAoKQp7CgljID0gdmVjNCAoMS4wKTsKfQo='" },
    Row { name: "validBomLowerRaw", url: "\u{feff}data:x-shader/x-fragment,#version 300 es\nprecision mediump float;\n\tout vec4 c;\r\nvoid main ()\n{\n\tc = vec4 (1.0);\n}\n", state: "COMPLETE", sources: &["#version 300 es\nprecision mediump float;\n\tout vec4 c;\r\nvoid main ()\n{\n\tc = vec4 (1.0);\n}\n\n"], message: "" },
];
