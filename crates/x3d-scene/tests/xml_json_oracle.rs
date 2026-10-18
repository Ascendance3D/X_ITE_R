//! The XML and JSON oracle: every world in the baselines must serialize to exactly the text the
//! pinned X_ITE 16.5.0 produces (scene.toXMLString / scene.toJSONString).
//!
//! Inputs and baselines are read through oracle/ (never copied). The baselines come from
//! oracle/x_ite-xmljson.mjs in the real worktree. A mismatch fails the test unless it is listed in
//! KNOWN with its reason. A listed file that now matches is also a failure, so the list cannot rot.

use std::path::Path;

/// (file, format, reason): a mismatch that cannot be fixed in this port, and why.
const KNOWN_SYNTHETIC: &[(&str, &str, &str)] = &[
    (
        "import-export-main.wrl",
        "xml",
        "IMPORT needs the Inline loaded; x3d_parse::load_file only parses, so the IMPORT records and the imported instance's exported node are missing",
    ),
    (
        "import-export-main.wrl",
        "json",
        "IMPORT needs the Inline loaded; x3d_parse::load_file only parses, so the IMPORT records and the imported instance's exported node are missing",
    ),
];

/// (file, format, reason) for the corpus-x0 baseline.
const KNOWN_X0: &[(&str, &str, &str)] = &[
    ("07-bookcase.wrl", "xml", "runtime: X_ITE runs the Script at load (count becomes 1); the parse-only Rust side runs no scripts (x0-known.tsv X5+X6)"),
    ("07-bookcase.wrl", "json", "runtime: X_ITE runs the Script at load (count becomes 1); the parse-only Rust side runs no scripts (x0-known.tsv X5+X6)"),
    ("43-villasantzip.wrl", "xml", "runtime: a ROUTE from a TimeSensor sets translation to NaN at load; the parse-only Rust side runs no routes (x0-known.tsv X5+X6)"),
    ("43-villasantzip.wrl", "json", "runtime: a ROUTE from a TimeSensor sets translation to NaN at load; the parse-only Rust side runs no routes (x0-known.tsv X5+X6)"),
];

/// The first line where the two texts differ, for the failure report.
fn first_difference(expected: &str, actual: &str) -> String {
    for (i, (e, a)) in expected.lines().zip(actual.lines()).enumerate() {
        if e != a {
            return format!("line {}: x_ite {e:?} rust {a:?}", i + 1);
        }
    }

    format!("line counts differ: x_ite {} rust {}", expected.lines().count(), actual.lines().count())
}

/// Compare one format of one file. `expected` is the baseline text, `None` when X_ITE failed to serialize.
fn compare(file: &str, format: &str, expected: Option<&str>, actual: &str, known: &[(&str, &str, &str)], known_hits: &mut Vec<String>, failures: &mut Vec<String>) -> bool {
    let Some(expected) = expected else {
        return false;
    };

    let listed = known.iter().find(|(f, fmt, _)| *f == file && *fmt == format);

    match (expected == actual, listed) {
        (true, None) => true,
        (true, Some((_, _, reason))) => {
            failures.push(format!("{file} {format}: listed as known ({reason}) but now matches; remove the entry"));
            true
        }
        (false, Some((_, _, reason))) => {
            known_hits.push(format!("{file} {format}: {reason}"));
            false
        }
        (false, None) => {
            failures.push(format!("{file} {format}: {}", first_difference(expected, actual)));
            false
        }
    }
}

/// Run one baseline file against its inputs; returns the failures (empty when the oracle passes).
fn run(name: &str, baseline_file: &str, corpus: &Path, known: &[(&str, &str, &str)]) -> Vec<String> {
    let baseline = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-xml").join(baseline_file);
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&baseline).unwrap()).unwrap();
    let results = json["results"].as_array().expect("baseline results");

    let mut failures = vec![];
    let mut known_hits = vec![];
    let mut skipped = vec![];
    let (mut loaded, mut xml_exact, mut json_exact) = (0, 0, 0);

    for entry in results {
        let file = entry["file"].as_str().unwrap();

        // X_ITE could not load the world, so there is no text to compare with.
        if !entry["ok"].as_bool().unwrap_or(false) {
            skipped.push(format!("{file} (X_ITE: {})", entry["error"].as_str().unwrap_or("?")));
            continue;
        }

        let scene = match x3d_parse::load_file(&corpus.join(file)) {
            Ok(scene) => scene,
            Err(e) => {
                failures.push(format!("{file}: Rust could not parse it: {e}"));
                continue;
            }
        };

        loaded += 1;

        let xml = scene.to_xml_string();
        let expected_xml = entry["xml"].as_str();

        if entry["xmlError"].is_string() {
            skipped.push(format!("{file} xml (X_ITE: {})", entry["xmlError"]));
        } else if compare(file, "xml", expected_xml, &xml, known, &mut known_hits, &mut failures) {
            xml_exact += 1;
        }

        let json_text = scene.to_json_string();
        let expected_json = entry["json"].as_str();

        if entry["jsonError"].is_string() {
            skipped.push(format!("{file} json (X_ITE: {})", entry["jsonError"]));
        } else if compare(file, "json", expected_json, &json_text, known, &mut known_hits, &mut failures) {
            json_exact += 1;
        }
    }

    println!("{name}: {loaded} loaded, XML exact {xml_exact}, JSON exact {json_exact}");

    for hit in &known_hits {
        println!("  known mismatch: {hit}");
    }

    for skip in &skipped {
        println!("  not compared: {skip}");
    }

    failures
}

#[test]
fn synthetic_worlds_match_x_ite() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-xml");
    let failures = run("synthetic-rea-xml", "xml-baseline.json", &corpus, KNOWN_SYNTHETIC);

    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn corpus_x0_matches_x_ite() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/corpus-x0");
    let failures = run("corpus-x0", "x0-xml-baseline.json", &corpus, KNOWN_X0);

    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
