//! REA batch 08: the glTF 2.0 and GLB parsers (GLTF2Parser.js, GLB2Parser.js) against pinned
//! X_ITE. Baseline: `oracle/rea-batch08-gltf-baseline/gltf-baseline.json`, written by
//! `node oracle/x_ite-gltf.mjs synthetic-rea-b08-gltf rea-batch08-gltf-baseline/gltf-baseline.json`.
//! For every input X_ITE loaded, the Rust scene must print the same toVRMLString, toXMLString and
//! toJSONString text; an input X_ITE rejected must be rejected too, with the same error text.
//!
//! `GLTF_ONLY=<substring>` restricts the run; `GLTF_OUT=<dir>` writes `.rust` / `.xite` files for
//! each difference.

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
fn gltf_matches_x_ite() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-b08-gltf");
    let baseline_path = std::env::var("GLTF_BASELINE").unwrap_or_else(|_| "../../oracle/rea-batch08-gltf-baseline/gltf-baseline.json".into());
    let baseline: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(baseline_path)).unwrap()).unwrap();
    let only = std::env::var("GLTF_ONLY").ok();
    let out = std::env::var("GLTF_OUT").ok();
    let mut failures = vec![];
    let mut checks = 0;

    for entry in baseline["results"].as_array().unwrap() {
        let file = entry["file"].as_str().unwrap();

        if only.as_deref().is_some_and(|o| !file.contains(o)) {
            continue;
        }

        // Draco is an external decoder library in X_ITE; the Rust port skips it (recorded as missing).
        if file == "codec/draco-garbage.gltf" {
            assert!(!entry["ok"].as_bool().unwrap());
            continue;
        }

        let path = root.join(file);
        let data = std::fs::read(&path).unwrap();
        let url = format!("file://{}", path.canonicalize().unwrap().display());
        let parsed = x3d_parse::parse_bytes(&data, &url);

        if !entry["ok"].as_bool().unwrap() {
            checks += 1;

            match parsed {
                Ok(_) => failures.push(format!("{file}: X_ITE rejected it ({}), Rust parsed it", entry["error"])),
                Err(e) => {
                    let expected = logged_error(entry);
                    let expected = expected.as_str();
                    let actual = e.to_string();

                    if !error_matches(expected, &actual) {
                        failures.push(format!("{file}: error text\n  X_ITE: {expected}\n  Rust:  {actual}"));
                    }
                }
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

/// The text X_ITE logs after "Couldn't load URL '<url>'. ", from the last error line of the log.
fn logged_error(entry: &serde_json::Value) -> String {
    let line = entry["log"].as_array().and_then(|l| l.iter().rev().find_map(|v| v.as_str().filter(|s| s.starts_with("error: Couldn't load")))).unwrap_or("");
    let rest = line.strip_prefix("error: Couldn't load data URL. ").unwrap_or_else(|| line.split("'. ").nth(1).unwrap_or(line));

    rest.replace("http://oracle/synthetic-rea-b08-gltf/", "")
}

fn error_matches(expected: &str, actual: &str) -> bool {
    let actual = actual.strip_prefix("Parser error at line 0: ").unwrap_or(actual);

    if actual == "Couldn't parse X3D. No suitable file handler found." {
        return expected.starts_with("Error: Couldn't parse X3D. No suitable file handler found for '");
    }

    expected == actual
}
