//! ScreenFontStyle with an OpenType-CFF font (ledger node:FontLibrary and sub:text.fonts). The glyphs W and V of the authored font
//! oracle/synthetic-rea-b24-cff/cff-basic.otf have a charstring width that differs from their hmtx advance. opentype.js replaces a glyph's
//! advanceWidth by the charstring width when it builds the glyph's path (ScreenText.build draws every glyph), and X_ITE builds a Text a second
//! time when its scene goes live, because the scene load waits for the FontStyle's font (X3DUrlObject loadCount). A loaded ScreenText is therefore
//! laid out with the charstring widths ("WVWV" at pointSize 16: textBounds 38 1, lineBounds 36.26667; hmtx alone gives 33 and 31.57333), a ScreenText
//! whose font arrives after the scene went live keeps the hmtx layout until the next build, and the glyphs built by one Text count for the next
//! (qs-late). Worlds in oracle/synthetic-rea-q-screen-cff (oracle/tools/gen_rea_q_screen_cff.py) go through the FontLibrary route, the
//! deprecated font-file `family` route and a mix; a Script writes origin, textBounds and lineBounds of every Text into a field, so the scene
//! text of the dumped frame carries them at full double precision. The baselines oracle/rea-q-screen-cff-baseline/<case>.json are three
//! identical recordings of the pinned X_ITE 16.5.0 (`oracle/tools/rea_q_screen_cff.py record`). Compared: the scene text of the dumped frame (exact), the sorted
//! log (exact) and `ok`; poses and the recorded warm-up count are not compared. No rendering and no GPU. `REA_ONLY=<substring>` restricts the cases.

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
    let baseline = root.join(format!("oracle/rea-q-screen-cff-baseline/{name}.json"));
    let js: Value = serde_json::from_str(&std::fs::read_to_string(&baseline).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!("rea-q-screen-cff-{}-{name}", std::process::id()));
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
fn screen_cff_cases_match_x_ite() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let cases =
        std::fs::read_to_string(root.join("oracle/rea-q-screen-cff-baseline/cases.txt")).unwrap();
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
        only.is_some() || checked == 4,
        "expected 4 cases, checked {checked}"
    );
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
