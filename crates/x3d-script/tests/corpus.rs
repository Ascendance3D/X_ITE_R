//! X3-X6 against the X_ITE oracles. Corpora are read in place under oracle/ (never copied).
//! Every scene-text or console-log difference must be listed in tests/<corpus>-known.tsv.

use std::path::Path;
use x3d_script::compare::{Outcome, compare_corpus};

struct Expect {
    files: usize,
    exact: usize,
    expected_failures: usize,
    oracle_timeouts: usize,
    logs_identical: usize,
}

fn run(corpus: &str, expect: Expect) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle");
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8)
        .max(8);
    let baseline = format!("{corpus}-baseline.json");
    // A gitignored key (oracle/answer-keys.tsv) is found and verified by x3d-evidence; a tracked one is
    // read in place. An absent key keeps the old path, so compare_corpus fails on it as before.
    let baseline = match x3d_evidence::resolve(&baseline) {
        Ok(p) => p,
        Err(x3d_evidence::KeyError::NotInManifest(_) | x3d_evidence::KeyError::Absent { .. }) => {
            root.join(&baseline)
        }
        Err(e) => panic!("{e}"),
    };
    let results = compare_corpus(&baseline, &root.join(corpus_dir(corpus)), true, threads).unwrap();
    let known = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/{corpus}-known.tsv")),
    )
    .unwrap();
    let known: Vec<&str> = known
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| l.split('\t').next().unwrap())
        .collect();

    let (mut exact, mut xfail, mut timeouts, mut logs) = (0, 0, 0, 0);
    let mut unexplained = vec![];

    for r in &results {
        let is_known = known.contains(&r.file.as_str());

        match &r.outcome {
            Outcome::Match => exact += 1,
            Outcome::ExpectedFailure(_) => xfail += 1,
            Outcome::OracleTimeout => timeouts += 1,
            Outcome::Mismatch { .. } if is_known => {}
            Outcome::Mismatch {
                line,
                expected,
                actual,
            } => unexplained.push(format!(
                "{} line {line}: x_ite {expected:?} rust {actual:?}",
                r.file
            )),
            other => unexplained.push(format!("{}: {other:?}", r.file)),
        }

        if matches!(r.outcome, Outcome::OracleTimeout) {
            continue;
        }

        if r.log_match == Some(true) {
            logs += 1;
        } else if !is_known {
            unexplained.push(format!(
                "{}: console log differs\n  x_ite {:?}\n  rust  {:?}",
                r.file, r.expected_log, r.rust_log
            ));
        }
    }

    assert!(unexplained.is_empty(), "{}", unexplained.join("\n"));
    assert_eq!(results.len(), expect.files);
    assert_eq!(exact, expect.exact);
    assert_eq!(xfail, expect.expected_failures);
    assert_eq!(timeouts, expect.oracle_timeouts);
    assert_eq!(logs, expect.logs_identical);
}

fn corpus_dir(corpus: &str) -> String {
    if corpus.starts_with("synthetic") {
        corpus.to_string()
    } else {
        format!("corpus-{corpus}")
    }
}

#[test]
fn x0_runtime_matches_x_ite() {
    run(
        "x0",
        Expect {
            files: 50,
            exact: 47,
            expected_failures: 3,
            oracle_timeouts: 0,
            logs_identical: 50,
        },
    );
}

#[test]
fn x1_runtime_matches_x_ite() {
    run(
        "x1",
        Expect {
            files: 500,
            exact: 470,
            expected_failures: 25,
            oracle_timeouts: 2,
            logs_identical: 497,
        },
    );
}

#[test]
fn items_runtime_matches_x_ite() {
    run(
        "items",
        Expect {
            files: 606,
            exact: 606,
            expected_failures: 0,
            oracle_timeouts: 0,
            logs_identical: 606,
        },
    );
}

/// Clean-room scenes for router order, TimeSensor/interpolator initialize, Script field
/// marshalling, the Browser object, error reporting, PROTO Scripts and external .js.
#[test]
fn synthetic_x6_matches_x_ite() {
    run(
        "synthetic-x6",
        Expect {
            files: 8,
            exact: 8,
            expected_failures: 0,
            oracle_timeouts: 0,
            logs_identical: 7,
        },
    );
}

/// Clean-room EXTERNPROTO worlds (oracle/synthetic-externproto/, hand-authored): the interface a
/// loaded PROTO gives, PROTO defaults vs explicit values, the URL fragment and first-PROTO rules,
/// URL fallback, a missing PROTO and a missing file, a Script and a nested EXTERNPROTO in a body.
#[test]
fn synthetic_externproto_matches_x_ite() {
    run(
        "synthetic-externproto",
        Expect {
            files: 12,
            exact: 12,
            expected_failures: 0,
            oracle_timeouts: 0,
            logs_identical: 12,
        },
    );
}

/// Clean-room Followers worlds (oracle/synthetic-followers/, generated by oracle/tools/gen_followers.py):
/// the initial state of every follower node (value_changed and isActive events at load, routed to
/// Transforms, Materials, Coordinates and Scripts that record them). The frame-by-frame
/// behavior is compared with oracle/tools/run_followers_suite.py.
#[test]
fn synthetic_followers_matches_x_ite() {
    run(
        "synthetic-followers",
        Expect {
            files: 10,
            exact: 10,
            expected_failures: 0,
            oracle_timeouts: 0,
            logs_identical: 10,
        },
    );
}
