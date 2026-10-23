//! The X0 oracle: oracle/corpus-x0 (50 files, read by path, never copied) against
//! oracle/x0-baseline.json. Mismatches must be listed in tests/x0-known.tsv.

use std::path::Path;
use x3d_parse::compare::{Outcome, compare_corpus};

#[test]
fn x0_corpus_matches_x_ite() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle");
    let results = compare_corpus(&root.join("x0-baseline.json"), &root.join("corpus-x0")).unwrap();
    let known =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/x0-known.tsv"))
            .unwrap();
    let known: Vec<&str> = known
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| l.split('\t').next().unwrap())
        .collect();

    let mut matched = 0;
    let mut expected_failures = 0;
    let mut unexplained = vec![];

    for r in &results {
        match &r.outcome {
            Outcome::Match => matched += 1,
            Outcome::ExpectedFailure(_) => expected_failures += 1,
            Outcome::Mismatch {
                line,
                expected,
                actual,
            } if !known.contains(&r.file.as_str()) => unexplained.push(format!(
                "{} line {line}: x_ite {expected:?} rust {actual:?}",
                r.file
            )),
            Outcome::Mismatch { .. } => {}
            other => unexplained.push(format!("{}: {other:?}", r.file)),
        }
    }

    assert!(unexplained.is_empty(), "{}", unexplained.join("\n"));
    assert_eq!(results.len(), 50);
    assert_eq!(expected_failures, 3);
    assert_eq!(matched, 45);
}
