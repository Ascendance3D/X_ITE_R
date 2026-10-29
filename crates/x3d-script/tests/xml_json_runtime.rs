//! scene.toXMLString () / toJSONString () after a full load (Inline, Script, ROUTE), against the
//! baselines oracle/x_ite-xmljson.mjs wrote in oracle/synthetic-rea-xml/. Exact text. The parse-only
//! test in x3d-scene lists the files that need the runtime; this one must match them all.

use std::path::{Path, PathBuf};
use x3d_script::compare::{DumpFormat, run_world_as};

fn oracle() -> PathBuf {
    std::fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle")).unwrap()
}

fn check(corpus: &Path, baseline: &Path) -> (usize, Vec<String>) {
    let root = oracle();
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(baseline).unwrap()).unwrap();
    let name = corpus.file_name().unwrap().to_string_lossy();
    let mut failures = vec![];
    let mut compared = 0;

    for r in json["results"].as_array().unwrap() {
        if !r["ok"].as_bool().unwrap_or(false) {
            continue;
        }

        let file = r["file"].as_str().unwrap();
        let path: PathBuf = corpus.join(file);

        for (format, key) in [(DumpFormat::Xml, "xml"), (DumpFormat::Json, "json")] {
            let (root2, path2) = (root.clone(), path.clone());
            let got = std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn(move || run_world_as(&root2, Path::new(&path2), true, format))
                .unwrap()
                .join()
                .unwrap();
            let want = r[key].as_str().unwrap();
            compared += 1;

            match got.vrml {
                Ok(text) if text == want => {}
                Ok(text) => {
                    let line = want
                        .lines()
                        .zip(text.lines())
                        .position(|(a, b)| a != b)
                        .map(|i| i + 1)
                        .unwrap_or(0);
                    failures.push(format!(
                        "{name}/{file} {key}: first difference at line {line}"
                    ));
                }
                Err(e) => failures.push(format!("{name}/{file} {key}: {e}")),
            }
        }
    }

    (compared, failures)
}

#[test]
fn synthetic_xml_json_after_load_match_x_ite() {
    let corpus = oracle().join("synthetic-rea-xml");
    let (n, failures) = check(&corpus, &corpus.join("xml-baseline.json"));
    assert!(n > 0);
    assert!(
        failures.is_empty(),
        "{} of {n} differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn x0_xml_json_after_load_match_x_ite() {
    // The corpus and its baseline are private (issue #168): found and verified, or skipped by name.
    let root = oracle();
    let Some(baseline) =
        x3d_evidence::key_or_tracked("x0-xml-baseline.json", &root.join("x0-xml-baseline.json"))
    else {
        return;
    };
    let Some(corpus) = x3d_evidence::key_or_tracked("corpus-x0", &root.join("corpus-x0")) else {
        return;
    };
    let (n, failures) = check(&corpus, &baseline);
    assert!(n > 0);
    assert!(
        failures.is_empty(),
        "{} of {n} differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
