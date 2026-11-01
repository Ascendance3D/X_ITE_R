//! Each `tests/fixtures/NAME.wrl` is a synthetic file written for this crate (no historical
//! assets). `NAME.expected.x3dv` is what the x_ite-vrml1-parser 1.1.0 extension produced for it
//! under X_ITE 16.7.0 (`scene .toVRMLString ()`, headless Chromium), captured once and stored;
//! `NAME.expected.err` is the first line of the error the extension threw. The first line of an
//! expected `.x3dv` is X_ITE's version banner and is not compared.

use std::path::{Path, PathBuf};

fn fixtures() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "wrl"))
        .collect();

    files.sort();
    files
}

fn body(text: &str) -> String {
    text.split_once('\n')
        .map(|x| x.1)
        .unwrap_or("")
        .trim_end()
        .to_string()
}

/// Fixtures where X_ITE 16.7.0 and the extension's source disagree in a way this port does not
/// copy. Each is checked separately below.
const KNOWN_DIVERGENT: &[&str] = &["19_ortho_camera"];

#[test]
fn matches_the_package_output() {
    let mut checked = 0;

    for path in fixtures() {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let input = std::fs::read_to_string(&path).unwrap();
        let ok = path.with_extension("expected.x3dv");
        let err = path.with_extension("expected.err");

        if KNOWN_DIVERGENT.contains(&name.as_str()) {
            continue;
        }

        let result = x3d_vrml1::parse_str(&input, "file:///fixture.wrl");

        if ok.exists() {
            let scene = result.unwrap_or_else(|e| panic!("{name}: {e}"));
            let expected = body(&std::fs::read_to_string(&ok).unwrap());
            let mut s = scene;

            s.clone_counts = Some(s.parse_clone_counts());
            assert!(expected.contains("PROFILE Interchange"), "{name}");
            assert_eq!(body(&s.to_vrml_string()), expected, "{name}");
        } else {
            let expected = std::fs::read_to_string(&err).unwrap();
            let e = result
                .err()
                .unwrap_or_else(|| panic!("{name}: expected an error"));

            assert_eq!(e.message, expected.trim_end(), "{name}");
        }

        checked += 1;
    }

    assert!(checked >= 20, "only {checked} fixtures");
}

/// X_ITE 16.7.0 prints `fieldOfView 0 0 0 1` for the extension's OrthoViewpoint whatever the
/// camera height is (reproduced for heights 0.5, 3, 4 and 10); the extension's source assigns
/// `[ -h/2, -h/2, h/2, h/2 ]`, which is what this port sets. Everything else matches.
#[test]
fn orthographic_camera_differs_only_in_field_of_view() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let input = std::fs::read_to_string(dir.join("19_ortho_camera.wrl")).unwrap();
    let mut s = x3d_vrml1::parse_str(&input, "file:///fixture.wrl").unwrap();

    s.clone_counts = Some(s.parse_clone_counts());

    let ours = body(&s.to_vrml_string());
    let theirs = body(&std::fs::read_to_string(dir.join("19_ortho_camera.expected.x3dv")).unwrap());
    let strip = |t: &str| -> Vec<String> {
        t.lines()
            .filter(|l| !l.trim_start().starts_with("fieldOfView"))
            .map(String::from)
            .collect()
    };

    assert_eq!(strip(&ours), strip(&theirs));
    assert!(ours.contains("fieldOfView -2 -2 2 2"), "{ours}");
}
