//! Guards for the future gate itself (default suite).
//!
//! * The gate sources contain no `#[ignore]` and define exactly the tests that
//!   `future-gate.sh` requires.
//! * Before R0.2-D the gate must not compile against the duplicate implementation, and it
//!   must fail for the recorded reasons. When Option B lands, the gate compiles, this
//!   record fails, and R0.2-D runs `future-gate.sh` and retires this record.

use std::process::Command;

fn read(rel: &str) -> String {
    std::fs::read_to_string(r0_2_c_tests::crate_dir().join(rel)).unwrap()
}

fn script_expected() -> Vec<String> {
    let s = read("future-gate.sh");
    let block = s.split("# BEGIN EXPECTED").nth(1).unwrap().split("# END EXPECTED").next().unwrap();
    block.lines().map(str::trim).filter(|l| l.starts_with("gate_")).map(str::to_string).collect()
}

fn gate_test_names() -> Vec<String> {
    let mut names = Vec::new();
    for f in ["tests/future_gate.rs", "tests/future_gate_chrome.rs"] {
        let src = read(f);
        let lines: Vec<&str> = src.lines().collect();
        for (i, l) in lines.iter().enumerate() {
            if l.trim() == "#[test]" {
                // The next `fn` line after the attribute (doc comments come before it).
                let f = lines[i + 1..].iter().find(|l| l.trim_start().starts_with("fn ")).unwrap();
                names.push(f.trim_start().trim_start_matches("fn ").split('(').next().unwrap().to_string());
            }
        }
    }
    names
}

#[test]
fn gate_sources_have_no_ignore() {
    for f in ["tests/future_gate.rs", "tests/future_gate_chrome.rs"] {
        let src = read(f);
        let bad: Vec<&str> = src.lines().filter(|l| l.trim_start().starts_with("#[ignore")).collect();
        assert!(bad.is_empty(), "{f} has an ignore attribute: {bad:?}");
    }
}

#[test]
fn gate_tests_equal_the_script_list() {
    let mut a = gate_test_names();
    let mut b = script_expected();
    assert_eq!(b.len(), 12);
    a.sort();
    b.sort();
    assert_eq!(a, b);
}

#[test]
fn gate_sources_are_feature_gated() {
    for f in ["tests/future_gate.rs", "tests/future_gate_chrome.rs"] {
        assert!(read(f).contains("#![cfg(feature = \"future_gate\")]"), "{f}");
    }
}

/// The rustc error codes the gate produces today, against R0.2-C `35ce9ec` sources:
/// E0599 (no method `factor` / `get_transform`), E0425 and E0531 (no variant `Imported`),
/// E0560 (struct has no such field), E0609 (no field on type),
/// E0308 (mismatched types: two Scene types).
const BEFORE_OPTION_B: &[&str] = &["E0308", "E0425", "E0531", "E0560", "E0599", "E0609"];

#[test]
fn gate_does_not_compile_against_the_duplicate_implementation() {
    let tmp = env!("CARGO_TARGET_TMPDIR");
    let out = Command::new(env!("CARGO"))
        .args(["test", "--offline", "--no-run", "--color", "never", "--features", "future_gate", "--test", "future_gate", "--test", "future_gate_chrome", "--manifest-path"])
        .arg(r0_2_c_tests::crate_dir().join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", r0_2_c_tests::scratch_dir(tmp).join("gate-target"))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "the future gate now compiles: Option B has landed. Run crates/r0-2-c-tests/future-gate.sh \
         and retire this record in the same change."
    );
    let mut codes: Vec<&str> = stderr.split("error[").skip(1).filter_map(|p| p.split(']').next()).collect();
    codes.sort();
    codes.dedup();
    assert_eq!(codes, BEFORE_OPTION_B, "{stderr}");
}
