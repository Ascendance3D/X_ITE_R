//! The first events of a LoadSensor whose url objects load at world load, against pinned X_ITE 16.5.0
//! (oracle/synthetic-loadsensor-baseline, recorded with Chrome 156.0.8078.47; PROVENANCE.json there).
//!
//! The world (oracle/synthetic-loadsensor/ls-first-events.wrl) has an ImageTexture, an Inline and an AudioClip, each
//! with its own LoadSensor, and a Script that logs every output event. X_ITE gives for each LoadSensor
//! `isActive TRUE, progress 0` at its initialize (the url object is IN_PROGRESS_STATE), then `isActive FALSE,
//! isLoaded TRUE, progress 1, loadTime`. Before the fix the ImageTexture was COMPLETE_STATE when its fetch ended, in
//! the same event cascade as the initialize, so its LoadSensor gave only `isActive FALSE, progress 1, isLoaded TRUE`.
//! X_ITE sets COMPLETE_STATE after the image decode (ImageTexture.js:78-129).
//!
//! The order of the events of one LoadSensor is compared. The order between the LoadSensors follows the fetch
//! timing of the browser and is not compared.

use serde_json::Value;
use std::path::Path;
use std::process::Command;

/// The Script's `log` field in the scene text of a dumped frame.
fn log_of(result: &Value, frame: &str) -> String {
    let text = result["vrml"][frame].as_str().expect("scene text");
    let start = text.find("log \"").expect("log field") + 5;
    let end = start + text[start..].find('"').unwrap();

    text[start..end].to_string()
}

/// The events of one LoadSensor (log entries that start with `prefix`), in order.
fn events(log: &str, prefix: char) -> Vec<&str> {
    log.split_whitespace()
        .filter(|e| e.starts_with(prefix))
        .collect()
}

#[test]
fn each_load_sensor_reports_in_progress_before_loaded_as_x_ite() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let baseline = root.join("oracle/synthetic-loadsensor-baseline/js.json");
    let js: Value = serde_json::from_str(&std::fs::read_to_string(&baseline).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!("ls-first-events-{}.json", std::process::id()));
    let status = Command::new(env!("CARGO_BIN_EXE_x3d-drive"))
        .current_dir(&root)
        .args([
            "oracle/synthetic-loadsensor/ls-first-events.wrl",
            "oracle/synthetic-loadsensor/ls-first-events.json",
            "--oracle",
            "oracle",
        ])
        .arg("--out")
        .arg(&out)
        .arg("--warmup-from")
        .arg(&baseline)
        .status()
        .unwrap();

    assert!(status.success(), "x3d-drive: {status}");

    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    let _ = std::fs::remove_file(&out);

    for frame in ["1", "2", "3", "5", "10", "20", "39"] {
        let (x, r) = (log_of(&js, frame), log_of(&rs, frame));

        assert!(
            r.starts_with("I "),
            "frame {frame}: the Script initializes first: {r}"
        );

        for prefix in ['T', 'I', 'C'] {
            assert_eq!(
                events(&r, prefix),
                events(&x, prefix),
                "frame {frame}, LoadSensor {prefix}: Rust {r:?}, X_ITE {x:?}"
            );
        }
    }

    // The values X_ITE gives, so that a changed baseline cannot hide a defect.
    let x = log_of(&js, "1");

    assert_eq!(
        events(&x, 'T'),
        ["TAtrue", "TP0", "TAfalse", "TLtrue", "TP1", "TT"]
    );
    assert_eq!(
        events(&x, 'C'),
        ["CAtrue", "CP0", "CAfalse", "CLtrue", "CP1", "CT"]
    );
    assert_eq!(
        events(&x, 'I'),
        ["I", "IAtrue", "IP0", "IAfalse", "ILtrue", "IP1", "IT"]
    );
}
