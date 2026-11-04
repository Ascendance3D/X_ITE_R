//! End-to-end OpenType-CFF fonts (ledger node:FontLibrary and sub:text.fonts): the runtime path, not `Font::parse`. Worlds in
//! oracle/synthetic-rea-p-cff-runtime (oracle/tools/gen_rea_p_cff_runtime.py) load the authored CFF font of oracle/synthetic-rea-b24-cff
//! through `FontLibrary`, through the deprecated font-file `family`, through url lists with a bare CFF and a missing file, and change
//! the FontStyle at run time. A Script writes origin, textBounds and lineBounds of every Text into a field of its own, so the scene text
//! of the dumped frame carries them. The baselines oracle/rea-p-cff-runtime-baseline/<case>.json are three identical recordings of
//! the pinned X_ITE 16.5.0 (`oracle/tools/rea_p_cff_runtime.py record`). Compared like rea_batch11_text.rs: the scene text exact, the
//! sorted log exact. No rendering and no GPU. `REA_ONLY=<substring>` restricts the cases.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn compare_case(root: &Path, name: &str, world: &str, script: &str, failures: &mut Vec<String>) {
    let baseline = root.join(format!("oracle/rea-p-cff-runtime-baseline/{name}.json"));
    let js: Value = serde_json::from_str(&std::fs::read_to_string(&baseline).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!("rea-p-cff-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
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

    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn cff_font_runtime_cases_match_x_ite() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let cases =
        std::fs::read_to_string(root.join("oracle/rea-p-cff-runtime-baseline/cases.txt")).unwrap();
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
    assert!(
        only.is_some() || checked == 5,
        "expected 5 cases, checked {checked}"
    );
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
