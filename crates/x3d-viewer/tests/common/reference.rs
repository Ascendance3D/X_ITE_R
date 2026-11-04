//! Which pinned X_ITE recording a test reads (owner decision 2026-11-04, issues #69 and #89).
//!
//! `oracle/<base>/<name>-msaa<N>` holds the recording made on the NVIDIA host. It stays the reference of every test
//! unless `X3D_REFERENCE_PLATFORM` names another platform. Pointing, collision and touch results of the pinned page
//! depend on the GPU and the ANGLE backend (the same page gives other numbers on ANGLE Metal than on ANGLE NVIDIA), so a
//! platform can have its own recording of the same pinned X_ITE 16.5.0 for the cases where the NVIDIA recording differs:
//! `oracle/<stem>-<platform>-baseline/<name>-msaa<N>/js.json`, made by `oracle/tools/record_platform_baseline.py` with three
//! identical runs on that machine. A case without such a recording keeps the NVIDIA recording (it matched there).
//! The platform recording replaces nothing: the NVIDIA recordings are never overwritten, and Rust is never compared with
//! a recording that Rust made.
//!
//! Only `js.json` (poses, probes, text) can come from a platform recording. The pictures of the case (`js/*.png`) always
//! come from the NVIDIA recording, so the image comparison of a case never changes with the platform.

use std::path::{Path, PathBuf};

/// `rea-b12-baseline` and `apple-m1` give `rea-b12-apple-m1-baseline`: the name keeps the `-baseline` suffix of a recorded baseline.
pub fn platform_base(base: &str, platform: &str) -> String {
    let stem = base.strip_suffix("-baseline").unwrap_or(base);

    format!("{stem}-{platform}-baseline")
}

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

/// The `js.json` of a case, with the platform given by the caller (the tests of this module use it without the environment).
pub fn js_json_for(
    root: &Path,
    base: &str,
    name: &str,
    msaa: u32,
    platform: Option<&str>,
) -> PathBuf {
    let own = root.join(format!("oracle/{base}/{name}-msaa{msaa}/js.json"));

    let Some(p) = platform else {
        return own;
    };

    let over = root.join(format!(
        "oracle/{}/{name}-msaa{msaa}/js.json",
        platform_base(base, p)
    ));

    if over.is_file() {
        eprintln!("reference: {name} MSAA {msaa} uses the {p} recording");
        over
    } else {
        own
    }
}

pub fn js_json(root: &Path, base: &str, name: &str, msaa: u32) -> PathBuf {
    js_json_for(root, base, name, msaa, platform().as_deref())
}

/// The directory of a particle case (`js.json` and `js.f32` side by side, rea_b14 and rea_b15): the platform's directory
/// `oracle/<stem>-<platform>-baseline/<name>` when both files are there, else the NVIDIA one `oracle/<base>/<name>`.
pub fn pair_dir_for(root: &Path, base: &str, name: &str, platform: Option<&str>) -> PathBuf {
    let own = root.join(format!("oracle/{base}/{name}"));

    let Some(p) = platform else {
        return own;
    };

    let over = root.join(format!("oracle/{}/{name}", platform_base(base, p)));

    if over.join("js.json").is_file() && over.join("js.f32").is_file() {
        eprintln!("reference: {name} uses the {p} recording");
        over
    } else {
        own
    }
}

pub fn pair_dir(root: &Path, base: &str, name: &str) -> PathBuf {
    pair_dir_for(root, base, name, platform().as_deref())
}
