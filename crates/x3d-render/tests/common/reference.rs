//! Which pinned X_ITE picture an x3d-render test reads (owner decision 2026-11-04, issues #69, #89 and #461). The rules are
//! those of `x3d-viewer/tests/common/reference.rs`:
//!
//! * `oracle/<key dir>/<file>` holds the picture recorded on the NVIDIA host. It stays the reference of every test unless
//!   `X3D_REFERENCE_PLATFORM` names another platform. The NVIDIA pictures are never overwritten.
//! * `oracle/<stem>-<platform>-baseline/<file>` holds the picture of the same pinned X_ITE 16.5.0 page recorded on that
//!   platform (three identical runs), kept only where it differs from the NVIDIA picture. A file without such a picture
//!   keeps the NVIDIA one.
//! * The platform is never chosen from the adapter: it is named, and an unknown name stops the test. A named platform must
//!   also be the adapter the test runs on ([`check_adapter`]), so a platform picture is never read on another GPU.

use std::path::{Path, PathBuf};

/// The platforms that have recordings.
pub const PLATFORMS: &[&str] = &["apple-m1"];

/// `render-point-tie-baseline` and `apple-m1` give `render-point-tie-apple-m1-baseline`.
pub fn platform_dir(key_dir: &str, platform: &str) -> String {
    let stem = key_dir.strip_suffix("-baseline").unwrap_or(key_dir);

    format!("{stem}-{platform}-baseline")
}

/// The platform named by `X3D_REFERENCE_PLATFORM`, or `None`. An unknown name panics: a typo must not fall back to the NVIDIA
/// pictures without a word.
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

/// Whether the adapter can be `platform`: apple-m1 needs a Metal adapter of Apple (vendor 0x106b, or "Apple" in the name).
pub fn adapter_matches(platform: &str, backend: wgpu::Backend, vendor: u32, name: &str) -> bool {
    match platform {
        "apple-m1" => {
            backend == wgpu::Backend::Metal && (vendor == 0x106b || name.starts_with("Apple"))
        }
        _ => false,
    }
}

/// Stops the test when a platform is named and the adapter is not that platform.
pub fn check_adapter(platform: Option<&str>, info: &wgpu::AdapterInfo) {
    if let Some(p) = platform {
        assert!(
            adapter_matches(p, info.backend, info.vendor, &info.name),
            "X3D_REFERENCE_PLATFORM={p}, but the adapter is {:?} on {:?} (vendor {:#x}): the {p} pictures are not the reference here",
            info.name,
            info.backend,
            info.vendor
        );
    }
}

/// The picture `file` of `key_dir` below `oracle`, with the platform given by the caller.
pub fn png_for(oracle: &Path, key_dir: &str, file: &str, platform: Option<&str>) -> PathBuf {
    let own = oracle.join(key_dir).join(file);

    let Some(p) = platform else {
        return own;
    };

    let over = oracle.join(platform_dir(key_dir, p)).join(file);

    if over.is_file() {
        eprintln!("reference: {key_dir}/{file} uses the {p} recording");
        over
    } else {
        own
    }
}
