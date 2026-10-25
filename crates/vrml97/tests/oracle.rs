//! Workspace-only check against X_ITE answer keys (oracle/ is not part of the package; the
//! test skips when it is missing). Parse-only: files whose X_ITE output depends on running
//! Scripts/events differ by design and are counted, not failed.

use std::path::Path;

fn run(baseline: &str, corpus: &str) -> Option<(usize, usize, usize, usize, Vec<String>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle");
    // A gitignored key (oracle/answer-keys.tsv) is found and verified by x3d-evidence; a tracked one is read in place.
    let path = match x3d_evidence::resolve(baseline) {
        Ok(p) => p,
        Err(x3d_evidence::KeyError::NotInManifest(_)) => root.join(baseline),
        Err(e @ x3d_evidence::KeyError::Absent { .. }) => {
            eprintln!("{e}");
            return None;
        }
        Err(e) => panic!("{e}"),
    };
    let text = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let (mut exact, mut differ, mut rejected_both, mut wrong_ok) = (0, 0, 0, 0);
    let mut differing = vec![];

    for r in json["results"].as_array().unwrap() {
        let file = r["file"].as_str().unwrap();
        let ours = vrml97::load_file(&root.join(corpus).join(file)).map(|s| s.to_vrml_string());

        match (r["ok"].as_bool().unwrap(), ours) {
            (true, Ok(text)) if text == r["vrml"].as_str().unwrap() => exact += 1,
            (true, Ok(_)) => {
                differ += 1;
                differing.push(file.to_string())
            }
            (false, Err(_)) => rejected_both += 1,
            _ => {
                wrong_ok += 1;
                differing.push(format!("{file} (ok mismatch)"))
            }
        }
    }

    Some((exact, differ, rejected_both, wrong_ok, differing))
}

#[test]
fn x0_matches_x_ite() {
    let Some((exact, differ, rejected, wrong, list)) = run("x0-baseline.json", "corpus-x0") else {
        return;
    };
    eprintln!(
        "x0: {exact} exact, {differ} differ {list:?}, {rejected} rejected by both, {wrong} wrong"
    );
    // 07-bookcase and 43-villasantzip need Scripts to run (crates/x3d-parse/tests/x0-known.tsv).
    assert_eq!((exact, differ, rejected, wrong), (45, 2, 3, 0));
}

#[test]
fn items_parse_only() {
    let Some((exact, differ, rejected, wrong, list)) = run("items-baseline.json", "corpus-items")
    else {
        return;
    };
    eprintln!("items: {exact} exact, {differ} differ, {rejected} rejected by both, {wrong} wrong");
    for f in list.iter().take(20) {
        eprintln!("  {f}");
    }
    assert_eq!(wrong, 0);
}
