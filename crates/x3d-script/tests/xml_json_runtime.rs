//! scene.toXMLString () / toJSONString () after a full load (Inline, Script, ROUTE), against the
//! baselines oracle/x_ite-xmljson.mjs wrote in oracle/synthetic-rea-xml/. Exact text. The parse-only
//! test in x3d-scene lists the files that need the runtime; this one must match them all.

use std::path::{Path, PathBuf};
use x3d_script::compare::{DumpFormat, run_world_as};

fn oracle() -> PathBuf {
    std::fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle")).unwrap()
}

fn check(corpus: &str, baseline: &str) -> (usize, Vec<String>) {
    let root = oracle();
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join("synthetic-rea-xml").join(baseline)).unwrap()).unwrap();
    let mut failures = vec![];
    let mut compared = 0;

    for r in json["results"].as_array().unwrap() {
        if !r["ok"].as_bool().unwrap_or(false) {
            continue;
        }

        let file = r["file"].as_str().unwrap();
        let path: PathBuf = root.join(corpus).join(file);

        for (format, key) in [(DumpFormat::Xml, "xml"), (DumpFormat::Json, "json")] {
            let (root2, path2) = (root.clone(), path.clone());
            let got = std::thread::Builder::new().stack_size(64 << 20).spawn(move || run_world_as(&root2, Path::new(&path2), true, format)).unwrap().join().unwrap();
            let want = r[key].as_str().unwrap();
            compared += 1;

            match got.vrml {
                Ok(text) if text == want => {}
                Ok(text) => {
                    let line = want.lines().zip(text.lines()).position(|(a, b)| a != b).map(|i| i + 1).unwrap_or(0);
                    failures.push(format!("{corpus}/{file} {key}: first difference at line {line}"));
                }
                Err(e) => failures.push(format!("{corpus}/{file} {key}: {e}")),
            }
        }
    }

    (compared, failures)
}

#[test]
fn synthetic_xml_json_after_load_match_x_ite() {
    let (n, failures) = check("synthetic-rea-xml", "xml-baseline.json");
    assert!(n > 0);
    assert!(failures.is_empty(), "{} of {n} differ:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn x0_xml_json_after_load_match_x_ite() {
    let (n, failures) = check("corpus-x0", "x0-xml-baseline.json");
    assert!(n > 0);
    assert!(failures.is_empty(), "{} of {n} differ:\n{}", failures.len(), failures.join("\n"));
}
