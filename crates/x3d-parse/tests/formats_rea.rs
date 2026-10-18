//! REA batch 01: the XML, JSON, STL, PLY and OBJ parsers (GoldenGate dispatch) against pinned
//! X_ITE. Baseline: `oracle/synthetic-rea-formats/formats-baseline.json`, written by
//! `node oracle/x_ite-formats.mjs synthetic-rea-formats synthetic-rea-formats/formats-baseline.json`.
//! For every input X_ITE loaded, the Rust scene must print the same toVRMLString, toXMLString and
//! toJSONString text; an input X_ITE rejected must be rejected too.
//!
//! `FORMATS_ONLY=<substring>` restricts the run; `FORMATS_OUT=<dir>` writes `.rust` / `.xite`
//! files for each difference.

use std::path::Path;

fn first_difference(expected: &str, actual: &str) -> String {
    let (e, a): (Vec<&str>, Vec<&str>) = (expected.split('\n').collect(), actual.split('\n').collect());

    for i in 0..e.len().max(a.len()) {
        if e.get(i) != a.get(i) {
            return format!("line {}:\n  X_ITE: {}\n  Rust:  {}", i + 1, e.get(i).unwrap_or(&"<EOF>"), a.get(i).unwrap_or(&"<EOF>"));
        }
    }

    String::new()
}

#[test]
fn formats_match_x_ite() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-formats");
    let baseline: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root.join("formats-baseline.json")).unwrap()).unwrap();
    let only = std::env::var("FORMATS_ONLY").ok();
    let out = std::env::var("FORMATS_OUT").ok();
    let mut failures = vec![];
    let mut checks = 0;

    for entry in baseline["results"].as_array().unwrap() {
        let file = entry["file"].as_str().unwrap();

        if only.as_deref().is_some_and(|o| !file.contains(o)) {
            continue;
        }

        let data = std::fs::read(root.join(file)).unwrap();
        // OBJ reads its mtllib files next to the model: the parse-only path reads local files.
        let url = if file.ends_with(".obj") {
            format!("file://{}", root.join(file).canonicalize().unwrap().display())
        } else {
            format!("http://oracle/synthetic-rea-formats/{file}")
        };
        let parsed = x3d_parse::parse_bytes(&data, &url);

        if !entry["ok"].as_bool().unwrap() {
            checks += 1;
            if parsed.is_ok() {
                failures.push(format!("{file}: X_ITE rejected it ({}), Rust parsed it", entry["error"]));
            }
            continue;
        }

        let scene = match parsed {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{file}: Rust rejected it: {e}"));
                continue;
            }
        };

        let encoding = entry["encoding"].as_str().unwrap_or("");
        checks += 1;
        if scene.encoding != encoding {
            failures.push(format!("{file}: encoding X_ITE {encoding}, Rust {}", scene.encoding));
        }

        for (key, actual) in [("vrml", scene.to_vrml_string()), ("xml", scene.to_xml_string()), ("json", scene.to_json_string())] {
            let Some(expected) = entry[key].as_str() else { continue };

            checks += 1;

            if expected != actual {
                failures.push(format!("{file} [{key}]: {}", first_difference(expected, &actual)));

                if let Some(dir) = &out {
                    let name = file.replace('/', "__");
                    std::fs::create_dir_all(dir).unwrap();
                    std::fs::write(Path::new(dir).join(format!("{name}.{key}.rust")), &actual).unwrap();
                    std::fs::write(Path::new(dir).join(format!("{name}.{key}.xite")), expected).unwrap();
                }
            }
        }
    }

    println!("{checks} checks, {} failed", failures.len());
    assert!(failures.is_empty(), "{} of {checks} checks failed:\n{}", failures.len(), failures.join("\n"));
}
