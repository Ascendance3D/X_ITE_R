//! End-to-end CID-keyed CFF and CFF2 fonts (ledger node:FontLibrary and sub:text.fonts): the runtime path, not `Font::parse`. Worlds in
//! oracle/synthetic-rea-t-cff-cid (oracle/tools/gen_rea_t_cff_cid.py, fonts from oracle/tools/gen_cff_cid_fonts.py) load the authored fonts
//! through `FontLibrary` and through the deprecated font-file `family`, load fonts opentype.js rejects (each with its own message) and a CFF2 font
//! whose charstrings use `return` and `endchar`, and change the FontStyle at run time. A Script writes origin, textBounds and lineBounds of
//! every Text into a field of its own, so the scene text of the dumped frame carries them. The baselines oracle/rea-t-cff-cid-baseline/<case>.json
//! are three identical recordings of the pinned X_ITE 16.5.0 (`oracle/tools/rea_t_cff_cid.py record`). Compared: the scene text of the dumped
//! frame (exact), the sorted log (exact) and `ok`. Poses and the recorded warm-up count are not compared (the count only sets the warm-up of the
//! Rust run). No rendering and no GPU. `REA_ONLY=<substring>` restricts the cases.
//!
//! KNOWN DIFFERENCE (known.txt, tc-novstore): a CFF2 font without a VariationStore parses, and every glyph path then throws a TypeError in
//! opentype.js; the pinned X_ITE never finishes loading such a world (three recordings time out after 60 s: no scene text, no log). The Rust
//! runtime finishes: the glyphs have no outline and keep their `hmtx` advances. That is pinned below as a difference (it fails when the Rust
//! output changes), not as a pass; the pinned side is UNVERIFIED beyond the timeout and the TypeError text of fonts.json.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The temporary directory of a case, removed when the case ends, early failures included.
struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn compare_case(root: &Path, name: &str, world: &str, script: &str, failures: &mut Vec<String>) {
    let baseline = root.join(format!("oracle/rea-t-cff-cid-baseline/{name}.json"));
    let js: Value = serde_json::from_str(&std::fs::read_to_string(&baseline).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!("rea-t-cff-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let _guard = TempDir(out.clone());
    let rs_json = out.join("rs.json");
    let status = Command::new(env!("CARGO_BIN_EXE_x3d-drive"))
        .current_dir(root)
        .args([world, script, "--oracle", "oracle"])
        .arg("--out")
        .arg(&rs_json)
        .arg("--warmup-from")
        .arg(&baseline)
        .status()
        .unwrap();
    let mut fail = |what: String| failures.push(format!("{name}: {what}"));

    if !status.success() {
        return fail(format!("x3d-drive exited with {status}"));
    }

    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&rs_json).unwrap()).unwrap();

    if js["ok"] != Value::Bool(true) || rs["ok"] != Value::Bool(true) {
        return fail(format!("ok js {} rs {}", js["ok"], rs["ok"]));
    }

    let mut bad = vec![];

    for (k, a) in js["vrml"].as_object().unwrap() {
        if rs["vrml"].get(k) != Some(a) {
            bad.push(k.clone());
        }
    }

    if !bad.is_empty() {
        let k = &bad[0];
        let (a, b) = (
            js["vrml"][k].as_str().unwrap(),
            rs["vrml"][k].as_str().unwrap_or(""),
        );
        let line = a
            .lines()
            .zip(b.lines().chain(std::iter::repeat("<missing>")))
            .find(|(x, y)| x != y);
        fail(format!(
            "scene text differs at {} dumped frames, first {k}: {line:?}",
            bad.len()
        ));
    }

    if js.get("log") != rs.get("log") {
        fail(format!("log differs: {} / {}", js["log"], rs["log"]));
    }
}

#[test]
fn cff_cid_and_cff2_runtime_cases_match_x_ite() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let cases =
        std::fs::read_to_string(root.join("oracle/rea-t-cff-cid-baseline/cases.txt")).unwrap();
    let only = std::env::var("REA_ONLY").ok();
    let mut failures = vec![];
    let mut checked = 0;

    for line in cases.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();

        if only.as_deref().is_some_and(|o| !f[0].contains(o)) {
            continue;
        }

        compare_case(&root, f[0], f[1], f[2], &mut failures);
        checked += 1;
    }

    eprintln!("{checked} cases checked");
    assert!(checked > 0, "REA_ONLY matched no case");
    assert!(
        only.is_some() || checked == 6,
        "expected 6 cases, checked {checked}"
    );
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The pinned X_ITE times out on this world (known.txt): there is nothing to compare, so the Rust run is pinned as a documented DIFFERENCE. The
/// pinned side is the baseline file (three timeouts) and the TypeError of every glyph in fonts.json; the Rust side is the output below. If the Rust
/// output changes, this test fails and the difference must be re-read, not the expectation edited to match.
#[test]
fn cff2_without_variation_store_is_a_known_difference() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let known =
        std::fs::read_to_string(root.join("oracle/rea-t-cff-cid-baseline/known.txt")).unwrap();
    let f: Vec<&str> = known.split_whitespace().collect();

    assert_eq!(f[0], "tc-novstore");

    let pinned: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("oracle/rea-t-cff-cid-baseline/tc-novstore.json"))
            .unwrap(),
    )
    .unwrap();

    assert_eq!(
        pinned,
        serde_json::json!({"timeout": 60, "runs": 3}),
        "pinned: three timeouts"
    );

    let fonts: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("oracle/rea-t-cff-cid-baseline/fonts.json")).unwrap(),
    )
    .unwrap();

    for (ch, g) in fonts["fonts"]["cff2-novstore.otf"]["glyphs"]
        .as_object()
        .unwrap()
    {
        assert_eq!(
            g["error"],
            "TypeError: Cannot read properties of undefined (reading 'itemVariationStore')",
            "pinned: the path build of {ch:?} throws"
        );
    }

    let out = std::env::temp_dir().join(format!("rea-t-cff-known-{}", std::process::id()));

    std::fs::create_dir_all(&out).unwrap();

    let _guard = TempDir(out.clone());
    let rs_json = out.join("rs.json");
    let status = Command::new(env!("CARGO_BIN_EXE_x3d-drive"))
        .current_dir(&root)
        .args([f[1], f[2], "--oracle", "oracle"])
        .arg("--out")
        .arg(&rs_json)
        .arg("--warmup-from")
        .arg(root.join("oracle/rea-t-cff-cid-baseline/tc-bad.json"))
        .status()
        .unwrap();

    assert!(status.success());

    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&rs_json).unwrap()).unwrap();
    let scene = rs["vrml"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .as_str()
        .unwrap();

    assert_eq!(rs["ok"], Value::Bool(true));
    assert_eq!(rs["log"], serde_json::json!([]));

    for want in [
        "\"000 T0.origin = SFVec3:0 0 0 raw=0 0 0\"",
        "\"001 T0.textBounds = SFVec2:1.8 1 raw=1.8 1\"",
        "\"002 T0.lineBounds = ArrayField:[\n  1.8 0.2,\n  1 1\n] raw=(1.7999999523162842 0.20000000298023224) (1 1)\"",
    ] {
        assert!(
            scene.contains(want),
            "Rust output of the known difference changed: {want} missing"
        );
    }
}
