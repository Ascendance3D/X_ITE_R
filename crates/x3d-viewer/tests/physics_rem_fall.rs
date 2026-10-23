//! A box falling from rest (`oracle/synthetic-rea-rem/rb-a.x3dv`, one canvas) against the saved pinned X_ITE
//! run (`rb-a-xite.json`, 45 frames of `toVRMLString`): position and linear velocity of every frame within
//! 2e-6. P6 closed: `rb-a.json` pins `currentFrameRate` to 1 / dt (`pinFrameRate`) like the oracle page, so
//! X_ITE's physics step sees the same frame rate; unpinned, a clock-measured rate put frame 8 one substep off.

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn number(text: &str, pattern: &str, group: usize) -> f64 {
    let at = text
        .find(pattern)
        .unwrap_or_else(|| panic!("no {pattern:?}"));
    let rest = &text[at + pattern.len()..];
    rest.split_whitespace().nth(group).unwrap().parse().unwrap()
}

fn body(text: &str) -> (f64, f64) {
    // The Transform TR's translation y and the RigidBody R's linearVelocity y.
    let tr = &text[text.find("DEF TR Transform").unwrap()..];
    (
        number(tr, "translation ", 1),
        number(text, "linearVelocity ", 1),
    )
}

#[test]
fn a_falling_box_follows_x_ite_every_frame() {
    fall(45);
}

fn fall(frames: usize) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("oracle/synthetic-rea-rem");
    let out = std::env::temp_dir().join(format!("rem-fall-{}.json", std::process::id()));
    let status = Command::new(env!("CARGO_BIN_EXE_x3d-drive"))
        .current_dir(&root)
        .arg(dir.join("rb-a.x3dv"))
        .arg(dir.join("rb-a.json"))
        .arg("--oracle")
        .arg(root.join("oracle"))
        .arg("--out")
        .arg(&out)
        .arg("--warmup-from")
        .arg(dir.join("rb-a-xite.json"))
        .status()
        .unwrap();
    assert!(status.success());

    let js: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("rb-a-xite.json")).unwrap())
            .unwrap();
    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    let _ = std::fs::remove_file(&out);
    let mut worst = (0.0f64, 0.0f64);

    for frame in 1..=frames {
        let key = frame.to_string();
        let (jp, jv) = body(js["vrml"][&key].as_str().unwrap());
        let (rp, rv) = body(rs["vrml"][&key].as_str().unwrap());

        worst = (worst.0.max((jp - rp).abs()), worst.1.max((jv - rv).abs()));
        assert!(
            (jp - rp).abs() <= 2e-6 && (jv - rv).abs() <= 2e-6,
            "frame {frame}: position {jp} / {rp}, velocity {jv} / {rv}"
        );
    }

    eprintln!("worst position {:e}, velocity {:e}", worst.0, worst.1);
}
