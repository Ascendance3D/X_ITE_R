//! REA batch 12 (node:Text warm-up): what every X_ITE 16.5.0 run of oracle/synthetic-rea-b11-text/warm-stamp*.wrl shares
//! (oracle/rea-batch12-baseline/text-warmup/patterns.txt, 42 runs over Batch 11 and 12). A Script stamps each origin / textBounds /
//! lineBounds event of a Text with its frame count. The first build, in frame 1, has no font yet: lineBounds [ 0 0, 0 0 ],
//! origin 0 0 0, textBounds 0 0, for the browser's default FontStyle and for a FontStyle of the world. The font's values come in a
//! later frame. Which frame, and how many builds there are, follows Chrome's I/O in the oracle and is not compared (documented
//! open: node-text-outputs-open). Not a baseline comparison: X_ITE is not repeatable in these frames.

use std::path::PathBuf;
use std::process::Command;

fn stamps(world: &str) -> Vec<String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let out = std::env::temp_dir().join(format!(
        "rea-batch12-text-warmup-{}-{world}.json",
        std::process::id()
    ));
    let status = Command::new(env!("CARGO_BIN_EXE_x3d-drive"))
        .current_dir(&root)
        .args([
            format!("oracle/synthetic-rea-b11-text/{world}.wrl"),
            format!("oracle/synthetic-rea-b11-text/{world}.json"),
        ])
        .args(["--oracle", "oracle", "--out"])
        .arg(&out)
        .status()
        .unwrap();

    assert!(status.success(), "{world}: x3d-drive exited with {status}");

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    let _ = std::fs::remove_file(&out);
    let vrml = json["vrml"].as_object().unwrap();
    let last = vrml
        .keys()
        .max_by_key(|k| k.parse::<u32>().unwrap())
        .unwrap();
    let text = vrml[last].as_str().unwrap();

    // The `out` field: "fN name=value" strings.
    text.split('"')
        .filter(|s| s.starts_with('f') && s.contains('=') && !s.starts_with("f0 "))
        .map(str::to_string)
        .collect()
}

fn check(world: &str, real: [&str; 3]) {
    let s = stamps(world);
    let empty = [
        "f1 lineBounds=[ 0 0, 0 0 ]",
        "f1 origin=0 0 0",
        "f1 textBounds=0 0",
    ];

    assert_eq!(
        s.get(..3),
        Some(&empty.map(String::from)[..]),
        "{world}: the first build is not the empty one: {s:?}"
    );

    // Nothing in frame 1 has the font yet.
    let f1: Vec<&String> = s.iter().filter(|e| e.starts_with("f1 ")).collect();
    assert_eq!(f1.len(), 3, "{world}: frame 1 builds again: {s:?}");

    // The font's values come later, all three, with the values of X_ITE.
    for v in real {
        assert!(
            s.iter().any(|e| e
                .split_once(' ')
                .is_some_and(|(f, rest)| f != "f1" && rest == v)),
            "{world}: no {v} after frame 1: {s:?}"
        );
    }
}

#[test]
fn rea_batch12_text_first_build_has_no_font() {
    check(
        "warm-stamp",
        [
            "lineBounds=[ 1.12207 1.052734, 0.453125 1 ]",
            "textBounds=1.12207 1.769531",
            "origin=0 0.7597656 0",
        ],
    );
    check(
        "warm-stamp-fs",
        [
            "lineBounds=[ 1.06 0.965, 0.445 1 ]",
            "textBounds=1.06 1.788",
            "origin=0 0.776 0",
        ],
    );
}
