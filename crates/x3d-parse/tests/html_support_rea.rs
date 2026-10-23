//! REA batch 08: HTMLSupport.js and the XMLParser's HTML mode against pinned X_ITE.
//!
//! * `oracle/synthetic-rea-html/*.html`: `div .innerHTML = text; browser .importDocument (div
//!   .querySelector ("x3d"))` in X_ITE; Rust: `parse_html_x3d`. Baseline `html-baseline.json`
//!   (`node oracle/x_ite-html.mjs synthetic-rea-html synthetic-rea-html/html-baseline.json`).
//! * `oracle/synthetic-rea-html/xml/*.x3d`: the same names and fields in an XML document, where
//!   HTMLSupport must NOT apply. Baseline `xml-baseline.json` (x_ite-formats.mjs).
//!
//! Every loaded world must print the same toVRMLString, toXMLString and toJSONString text, and
//! the same console log (X_ITE warnings and errors); a rejected input must be rejected by both.

use std::path::Path;

fn first_difference(expected: &str, actual: &str) -> String {
    let (e, a): (Vec<&str>, Vec<&str>) =
        (expected.split('\n').collect(), actual.split('\n').collect());

    for i in 0..e.len().max(a.len()) {
        if e.get(i) != a.get(i) {
            return format!(
                "line {}:\n  X_ITE: {:.300}\n  Rust:  {:.300}",
                i + 1,
                e.get(i).unwrap_or(&"<EOF>"),
                a.get(i).unwrap_or(&"<EOF>")
            );
        }
    }

    String::new()
}

fn run(dir: &str, baseline: &str, html: bool) -> (usize, Vec<String>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .join(dir);
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join(baseline)).unwrap()).unwrap();
    let mut failures = vec![];
    let mut checks = 0;

    for entry in json["results"].as_array().unwrap() {
        let file = entry["file"].as_str().unwrap();
        let data = std::fs::read(root.join(file)).unwrap();
        // X_ITE parses the DOM element with the page as the world URL.
        let url = if html {
            "http://oracle/x_ite-pin/src/dump.html".to_string()
        } else {
            format!("http://oracle/{dir}/{file}")
        };
        let parsed = if html {
            x3d_parse::parse_html_x3d(&String::from_utf8_lossy(&data), &url)
        } else {
            x3d_parse::parse_bytes(&data, &url)
        };
        let log: Vec<String> = entry["log"]
            .as_array()
            .map(|l| l.iter().map(|s| s.as_str().unwrap().to_string()).collect())
            .unwrap_or_default();

        checks += 1;

        if !entry["ok"].as_bool().unwrap() {
            if parsed.is_ok() {
                failures.push(format!(
                    "{file}: X_ITE rejected it ({}), Rust parsed it",
                    entry["error"]
                ));
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

        for (key, actual) in [
            ("vrml", scene.to_vrml_string()),
            ("xml", scene.to_xml_string()),
            ("json", scene.to_json_string()),
        ] {
            let Some(expected) = entry[key].as_str() else {
                continue;
            };

            checks += 1;

            if expected != actual {
                failures.push(format!(
                    "{file} [{key}]: {}",
                    first_difference(expected, &actual)
                ));
            }
        }

        checks += 1;

        // Console log. X_ITE's lines are `warn: <message>` / `error: <message>`; the scene records
        // the message only (the existing log model of the XML parser, Scene::warnings), without the
        // level, without the `Error: ` the catch blocks of the loaders add, and once per distinct
        // message. The levels and X_ITE's duplicate logging of one failure are therefore not
        // compared; the message text and the set of distinct messages are. (Scene::error entries carry a level
        // mark, removed here; the level of the XML unknown-node error is compared by the rea_batch09_sai case
        // parse-xml-unknown. This corpus still has other DEVELOPMENT double logs and an `Invalid XML.` level gap.)
        let norm = |l: &str| -> String {
            let l = x3d_scene::console_level(l).1;
            let l = l
                .strip_prefix("warn: ")
                .or_else(|| l.strip_prefix("error: "))
                .unwrap_or(l);
            let l = l.strip_prefix("Error: ").unwrap_or(l);

            l.strip_prefix("XML Parser: ").unwrap_or(l).to_string()
        };
        let mut want: Vec<String> = log.iter().map(|l| norm(l)).collect();
        let mut got: Vec<String> = scene.warnings.iter().map(|l| norm(l)).collect();

        want.sort();
        want.dedup();
        got.sort();
        got.dedup();

        if want != got {
            failures.push(format!(
                "{file}: console log\n  X_ITE: {log:?}\n  Rust:  {:?}",
                scene.warnings
            ));
        }
    }

    (checks, failures)
}

#[test]
fn html_support_matches_x_ite() {
    let (c1, mut f1) = run("synthetic-rea-html", "html-baseline.json", true);
    let (c2, f2) = run("synthetic-rea-html/xml", "../xml-baseline.json", false);

    println!(
        "{} checks ({c1} html, {c2} xml-twin), {} failed",
        c1 + c2,
        f1.len() + f2.len()
    );
    f1.extend(f2);
    assert!(
        f1.is_empty(),
        "{} checks failed:\n{}",
        f1.len(),
        f1.join("\n")
    );
}
