//! REA ledger entry sub:io.file-loader: FileLoader.js (`loadDocument`, `loadDocumentAsync`, `loadDocumentError`, `printError`) at the level of
//! `Browser.createX3DFromURL`, against pinned X_ITE 16.5.0, for text input: format sniffing by content (VRML97, classic X3D, XML, JSON, a
//! format under the wrong extension), byte order marks and encodings, headers, empty files, gzip / zlib / deflate input, url resolution
//! (relative, `..`, fragment, query, percent escapes, schemes), `data:` URLs, `ecmascript:` candidates, the fallback order of an MFString
//! url, the diagnostic text of a failed load, and Inline / EXTERNPROTO urls inside a loaded scene. Baselines are
//! oracle/file-loader-baseline/ff-*.json, recorded three times from the pinned page by `oracle/tools/file_loader.py record`; the authored
//! inputs and the worlds are in oracle/synthetic-rea-file-loader (made by oracle/tools/gen_file_loader.py).
//!
//! Each world's Script loads its URL lists one after the other and records, per list, the scene head and `toVRMLString` of the scene
//! or the rejection in a field of its own, so the scene text of every dumped frame carries it. The loader's diagnostic for a failed
//! candidate (`Couldn't load URL ..., Error: ...`, the warning when a later URL follows) is the sorted `log` (the first line of each).
//! Compared like oracle/tools/x10_compare.py: the scene text of every dumped frame exact, the sorted log exact. No rendering, no GPU.
//! Not compared: HTTP redirects, response headers and the foreign (`text/html`) hand-off, which the recorder's static server cannot
//! produce or a headless page cannot follow.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// The groups of the baseline directory (the json files in it, nothing nested).
fn groups(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root.join("oracle/file-loader-baseline"))
        .unwrap()
        .filter_map(|e| {
            let name = e.unwrap().file_name().into_string().unwrap();
            name.strip_suffix(".json").map(str::to_string)
        })
        .collect();

    names.sort();
    names
}

/// Run `x3d-drive` on the group's world and compare it with the baseline; the failures are returned.
fn compare_group(root: &Path, name: &str) -> Vec<String> {
    let baseline = root.join(format!("oracle/file-loader-baseline/{name}.json"));
    let js: Value = serde_json::from_str(&std::fs::read_to_string(&baseline).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!("file-loader-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let rs_json = out.join("rs.json");
    let status = Command::new(env!("CARGO_BIN_EXE_x3d-drive"))
        .current_dir(root)
        .args([
            &format!("oracle/synthetic-rea-file-loader/{name}.wrl"),
            "oracle/synthetic-rea-file-loader/ff.json",
            "--oracle",
            "oracle",
        ])
        .arg("--out")
        .arg(&rs_json)
        .arg("--warmup-from")
        .arg(&baseline)
        .status()
        .unwrap();
    let mut failures = vec![];
    let mut fail = |what: String| failures.push(format!("{name}: {what}"));

    if !status.success() {
        fail(format!("x3d-drive exited with {status}"));
        return failures;
    }

    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&rs_json).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(&out);

    if js["ok"] != Value::Bool(true) || rs["ok"] != Value::Bool(true) {
        fail(format!("ok js {} rs {}", js["ok"], rs["ok"]));
        return failures;
    }

    // The baseline is not vacuous: every list of the world finished, in the last dumped frame, and the dumped frames are all there.
    let frames = js["vrml"].as_object().unwrap();
    let last = frames
        .keys()
        .max_by_key(|k| k.parse::<u32>().unwrap())
        .unwrap();
    let text = frames[last].as_str().unwrap();

    if text.contains(": ...\"") || !text.contains(": RES ") || !text.contains(": REJ ") {
        fail(format!("the baseline's frame {last} is not a finished run"));
    }

    let mut bad = vec![];

    for (k, a) in frames {
        if rs["vrml"].get(k) != Some(a) {
            bad.push(k.clone());
        }
    }

    if !bad.is_empty() {
        bad.sort_by_key(|k| k.parse::<u32>().unwrap_or(0));
        let k = &bad[0];
        let (a, b) = (
            js["vrml"][k].as_str().unwrap(),
            rs["vrml"][k].as_str().unwrap_or(""),
        );
        let first = a
            .lines()
            .zip(b.lines())
            .position(|(x, y)| x != y)
            .unwrap_or_else(|| a.lines().count().min(b.lines().count()));
        fail(format!(
            "scene text differs at {} dumped frames, first {k}, first differing line {}:\n  pinned: {:?}\n  port:   {:?}",
            bad.len(),
            first + 1,
            a.lines().nth(first),
            b.lines().nth(first)
        ));
    }

    // `log`: the sorted warn / error lines.
    if js["log"] != rs["log"] {
        let strings = |v: &Value| -> Vec<String> {
            v.as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        let (a, b) = (strings(&js["log"]), strings(&rs["log"]));
        let only_js: Vec<&String> = a.iter().filter(|l| !b.contains(l)).collect();
        let only_rs: Vec<&String> = b.iter().filter(|l| !a.contains(l)).collect();
        fail(format!(
            "log differs ({} / {} lines)\n  only pinned: {only_js:?}\n  only port: {only_rs:?}",
            a.len(),
            b.len()
        ));
    }

    failures
}

fn check(group: &str) {
    let root = root();
    let failures = compare_group(&root, group);

    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn file_loader_text_matches_x_ite() {
    check("ff-text");
}

#[test]
fn file_loader_urls_match_x_ite() {
    check("ff-url");
}

#[test]
fn file_loader_inline_urls_match_x_ite() {
    check("ff-inline");
}

/// The baseline directory holds exactly the three groups, so a new baseline cannot be left without a test.
#[test]
fn file_loader_baseline_groups_are_the_tested_ones() {
    assert_eq!(groups(&root()), ["ff-inline", "ff-text", "ff-url"]);
}
