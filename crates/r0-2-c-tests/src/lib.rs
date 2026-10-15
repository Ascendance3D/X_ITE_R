//! R0.2-C compatibility test foundation, hardened in R0.2-C1.
//!
//! ## Layout
//!
//! - `tests/api_surface.rs` — Suite 1. Compiler checks of every public module path and
//!   the crate-root set of `vrml97`, `vrml` and `x3d` 0.1.0.
//! - `tests/type_identity.rs` — Suite 2. Current-difference records between the
//!   published and the engine types. Absent items are proven with compile probes.
//! - `tests/field_math.rs` — Suite 3. Field, matrix and float-format results compared
//!   with recorded X_ITE / Chrome / Node outputs (Node math flavor, the default).
//! - `tests/js_math_chrome.rs` — Suite 3b. The Chrome math flavor. A separate test binary,
//!   because `jsmath::set_flavor` is process-wide.
//! - `tests/parser_scene.rs` — Suites 4 and 5. Synthetic VRML fixtures compared with the
//!   pinned X_ITE oracle in `tests/fixtures/xite/ORACLE.json`.
//! - `tests/error_compat.rs` — Suite 6. Public `ParseError` and `LoadError`.
//! - `tests/future_gate.rs`, `tests/future_gate_chrome.rs` — Suite 7. Real future
//!   requirements behind the `future_gate` feature. They do not compile today.
//!   `future-gate.sh` is the one command that runs them after R0.2-D.
//! - `tests/future_gate_contract.rs` — guards the gate itself (no `#[ignore]`, exact
//!   names, recorded compile failure).
//!
//! Expected values and their sources are listed in `tests/fixtures/PROVENANCE.md`.
//! This library holds test support only.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// R0.2-C lane identifier.
pub const LANE: &str = "XITE_RUST_R0_2_C_COMPAT_TESTS";

/// Workspace root of the checkout this crate was built from.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("workspace root")
}

/// Directory of this crate.
pub fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A fixture file under `tests/fixtures/`.
pub fn fixture(rel: &str) -> Vec<u8> {
    let p = crate_dir().join("tests/fixtures").join(rel);
    std::fs::read(&p).unwrap_or_else(|e| panic!("fixture {}: {e}", p.display()))
}

/// A scratch directory below `CARGO_TARGET_TMPDIR` that belongs to this checkout only.
/// The workspace build-dir (`~/Projects/cybertown/.cargo/config.toml`) is shared by every
/// worktree, so `CARGO_TARGET_TMPDIR` is shared too; the FNV-1a hash of the workspace root
/// keeps two checkouts from writing the same probe crate.
pub fn scratch_dir(tmp: &str) -> PathBuf {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in workspace_root().to_string_lossy().bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    Path::new(tmp).join(format!("r02c1-{h:016x}"))
}

/// The result of compiling one probe snippet.
#[derive(Debug)]
pub struct ProbeResult {
    pub compiled: bool,
    /// Error codes such as `E0599`, in the order rustc printed them, without duplicates.
    pub codes: Vec<String>,
    pub stderr: String,
}

/// Compile `body` as the body of `fn main` in a throw-away binary crate that depends on
/// the workspace crates `vrml97`, `x3d-fields`, `x3d-scene` and `x3d-parse` by path.
///
/// `tmp` is the integration test's `CARGO_TARGET_TMPDIR`. All probes share one target
/// directory below it, so the dependencies compile once. The workspace `Cargo.lock` is
/// copied in, so the probe uses the same dependency versions, and `--offline` is used.
pub fn compile_probe(tmp: &str, name: &str, body: &str) -> ProbeResult {
    let root = workspace_root();
    let dir = scratch_dir(tmp).join("probes").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).expect("probe dir");

    let dep = |c: &str| format!("{c} = {{ path = {:?} }}\n", root.join("crates").join(c));
    let manifest = format!(
        "[package]\nname = {name:?}\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n\n[dependencies]\n{}{}{}{}",
        dep("vrml97"),
        dep("x3d-fields"),
        dep("x3d-scene"),
        dep("x3d-parse"),
    );
    std::fs::write(dir.join("Cargo.toml"), manifest).unwrap();
    std::fs::copy(root.join("Cargo.lock"), dir.join("Cargo.lock")).unwrap();
    std::fs::write(
        dir.join("src/main.rs"),
        format!("#![allow(unused, unreachable_code)]\nfn main() {{\n{body}\n}}\n"),
    )
    .unwrap();

    let out = Command::new(env!("CARGO"))
        .args(["check", "--offline", "--quiet", "--color", "never", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", scratch_dir(tmp).join("probe-target"))
        .output()
        .expect("run cargo check");

    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let mut codes: Vec<String> = Vec::new();

    for part in stderr.split("error[").skip(1) {
        if let Some(code) = part.split(']').next() {
            if !codes.iter().any(|c| c == code) {
                codes.push(code.to_string());
            }
        }
    }

    ProbeResult { compiled: out.status.success(), codes, stderr }
}

/// Assert that a probe fails with exactly the given set of rustc error codes.
pub fn assert_probe_fails(tmp: &str, name: &str, body: &str, codes: &[&str]) {
    let r = compile_probe(tmp, name, body);
    assert!(
        !r.compiled,
        "{name}: the probe now compiles. The recorded difference is closed; \
         retire this current-difference record in the same change.\n{body}"
    );
    let mut got = r.codes.clone();
    got.sort();
    let mut want: Vec<String> = codes.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(got, want, "{name}: unexpected rustc errors\n{}", r.stderr);
}

/// Assert that a probe compiles.
pub fn assert_probe_compiles(tmp: &str, name: &str, body: &str) {
    let r = compile_probe(tmp, name, body);
    assert!(r.compiled, "{name}: the probe does not compile\n{}", r.stderr);
}
