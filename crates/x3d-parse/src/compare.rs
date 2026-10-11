//! Compare Rust output against the X0 oracle (oracle/x0-baseline.json).

use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// X_ITE parsed the file and the Rust dump is byte-identical.
    Match,
    /// X_ITE parsed the file, Rust output differs. (line number, expected line, actual line)
    Mismatch { line: usize, expected: String, actual: String },
    /// X_ITE parsed the file, Rust rejected it.
    RustRejected(String),
    /// X_ITE rejected the file and so did Rust.
    ExpectedFailure(String),
    /// X_ITE rejected the file but Rust parsed it.
    UnexpectedSuccess,
}

#[derive(Debug, Clone)]
pub struct FileResult {
    pub file: String,
    pub outcome: Outcome,
    pub rust_output: Option<String>,
    pub expected_output: Option<String>,
}

fn first_difference(expected: &str, actual: &str) -> (usize, String, String) {
    let e: Vec<&str> = expected.split('\n').collect();
    let a: Vec<&str> = actual.split('\n').collect();

    for i in 0..e.len().max(a.len()) {
        let (x, y) = (e.get(i).copied(), a.get(i).copied());

        if x != y {
            return (i + 1, x.unwrap_or("<EOF>").to_string(), y.unwrap_or("<EOF>").to_string());
        }
    }

    (0, String::new(), String::new())
}

pub fn compare_corpus(baseline: &Path, corpus: &Path) -> Result<Vec<FileResult>, String> {
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(baseline).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;

    let entries = json["results"].as_array().ok_or("baseline has no results")?;

    // Parse files in parallel (each file is independent).
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).max(8);
    let chunks: Vec<Vec<serde_json::Value>> = {
        let mut c = vec![vec![]; threads];
        for (i, e) in entries.iter().enumerate() {
            c[i % threads].push(e.clone());
        }
        c
    };

    let mut results: Vec<FileResult> = std::thread::scope(|s| {
        let handles: Vec<_> = chunks
            .into_iter()
            .map(|chunk| {
                s.spawn(move || {
                    chunk
                        .iter()
                        .map(|entry| {
                            let file = entry["file"].as_str().unwrap_or("").to_string();
                            let ok = entry["ok"].as_bool().unwrap_or(false);
                            let expected = entry["vrml"].as_str().map(String::from);
                            let parsed = crate::load_file(&corpus.join(&file)).map(|s| s.to_vrml_string());

                            let outcome = match (ok, &parsed) {
                                (true, Ok(out)) => {
                                    let exp = expected.clone().unwrap_or_default();
                                    if *out == exp {
                                        Outcome::Match
                                    } else {
                                        let (line, expected, actual) = first_difference(&exp, out);
                                        Outcome::Mismatch { line, expected, actual }
                                    }
                                }
                                (true, Err(e)) => Outcome::RustRejected(e.to_string()),
                                (false, Err(e)) => Outcome::ExpectedFailure(e.to_string()),
                                (false, Ok(_)) => Outcome::UnexpectedSuccess,
                            };

                            FileResult { file, outcome, rust_output: parsed.ok(), expected_output: expected }
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();

        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });

    results.sort_by(|a, b| a.file.cmp(&b.file));
    Ok(results)
}
