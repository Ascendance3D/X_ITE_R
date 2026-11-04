//! Where the pinned X_ITE recording of a case lives (owner decision 2026-11-04, issues #69 and #89).
//!
//! `oracle/<base>/<name>-msaa<N>` holds the recording made on the NVIDIA host. It stays the reference of every test
//! unless `X3D_REFERENCE_PLATFORM` names another platform. Pointing, collision and touch results of the pinned page
//! depend on the GPU and the ANGLE backend (the same page gives other numbers on ANGLE Metal than on ANGLE NVIDIA), so a
//! platform can have its own recording of the same pinned X_ITE 16.5.0 for the cases where the NVIDIA recording differs:
//! `oracle/<base>-<platform>/<name>-msaa<N>/js.json`, made by `oracle/tools/record_platform_baseline.py` with three
//! identical runs on that machine. A case without such a recording keeps the NVIDIA recording (it matched there).
//! The platform recording replaces nothing: the NVIDIA recordings are never overwritten, and Rust is never compared with
//! a recording that Rust made.

use std::path::{Path, PathBuf};

/// The platforms that have recordings. Any other value of `X3D_REFERENCE_PLATFORM` stops the test: a typo must not fall
/// back to the NVIDIA recordings without a word.
pub const PLATFORMS: &[&str] = &["apple-m1"];

pub fn platform() -> Option<String> {
    let p = std::env::var("X3D_REFERENCE_PLATFORM")
        .ok()
        .filter(|p| !p.is_empty())?;

    assert!(
        PLATFORMS.contains(&p.as_str()),
        "X3D_REFERENCE_PLATFORM={p}: unknown platform (known: {PLATFORMS:?})"
    );

    Some(p)
}

/// `case_dir` with the platform given by the caller (the tests of this module use it without the environment).
pub fn case_dir_for(
    root: &Path,
    base: &str,
    name: &str,
    msaa: u32,
    platform: Option<&str>,
) -> PathBuf {
    let own = root.join(format!("oracle/{base}/{name}-msaa{msaa}"));

    let Some(p) = platform else {
        return own;
    };

    let over = root.join(format!("oracle/{base}-{p}/{name}-msaa{msaa}"));

    if over.join("js.json").is_file() {
        eprintln!("reference: {name} MSAA {msaa} uses the {p} recording");
        over
    } else {
        own
    }
}

pub fn case_dir(root: &Path, base: &str, name: &str, msaa: u32) -> PathBuf {
    case_dir_for(root, base, name, msaa, platform().as_deref())
}
