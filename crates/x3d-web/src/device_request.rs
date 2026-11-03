//! The order in which the web build asks the browser for a graphics adapter (issue #142, rule BR3.1).
//!
//! WebGPU lets a page ask for the `compatibility` feature level (`GPURequestAdapterOptions.featureLevel`), which a browser
//! can serve on hardware that has no `core` WebGPU support. The rule:
//!
//! * when the graphics library supports requesting the feature level, request it first;
//! * otherwise keep the current request, with no change and no diagnostic;
//! * when the compatibility request fails, or the adapter it returns lacks the feature level, retry **once** with the
//!   current request and log a diagnostic that names the reason. A failure of that retry is the request's error.
//!
//! This module is the platform-neutral decision, so that the three cases are tested natively. `wasm` calls it with the
//! two requests.
//!
//! # What the pinned dependency offers
//!
//! `wgpu` 30.0.1 (Cargo.lock) has a `wgpu_types::FeatureLevel` enum (`Core`, `Compatibility`), but nothing uses it:
//! `RequestAdapterOptions` has no `feature_level` field, `wgpu-core` and the WebGPU backend never read it, and the type's
//! own documentation says "`wgpu` does not support compatibility-level adapters per se". The WebGPU backend's generated
//! `GpuRequestAdapterOptions::set_feature_level` is private to that crate. So today
//! [`WGPU_REQUESTS_COMPATIBILITY_FEATURE_LEVEL`] is `false` and the web build keeps its current request. A wgpu release
//! that adds the option changes that one constant and the `Attempt::Compatibility` arm of `wasm::request_adapter`; the
//! tests below already cover the supported path.

use std::future::Future;

/// Whether the pinned `wgpu` can request the compatibility feature level. See the module documentation; a test pins the
/// `wgpu` version this was checked against, so that an upgrade makes someone look again.
pub const WGPU_REQUESTS_COMPATIBILITY_FEATURE_LEVEL: bool = false;

/// The `wgpu` version for which [`WGPU_REQUESTS_COMPATIBILITY_FEATURE_LEVEL`] was checked (Cargo.lock).
pub const CHECKED_WGPU_VERSION: &str = "30.0.1";

/// Which request is being made.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Attempt {
    /// The `compatibility` feature level.
    Compatibility,
    /// The request of today: no feature level named, the browser's `core` default.
    Current,
}

/// What a successful start used, and what was logged on the way.
#[derive(Debug, PartialEq, Eq)]
pub struct Outcome<T> {
    pub value: T,
    /// The request that produced `value`.
    pub used: Attempt,
    /// One line per diagnostic, in order. Empty unless a compatibility request failed.
    pub diagnostics: Vec<String>,
}

/// Request with `request(Attempt::Compatibility)` first when `compatibility_supported`, and with
/// `request(Attempt::Current)` otherwise or after a failure. `request` returns the reason on failure; an adapter that lacks
/// the feature level is a failure with that reason. `Attempt::Current` is made at most once.
pub async fn request_with_compatibility<T, F, Fut>(
    compatibility_supported: bool,
    mut request: F,
) -> Result<Outcome<T>, String>
where
    F: FnMut(Attempt) -> Fut,
    Fut: Future<Output = Result<T, String>>,
{
    let mut diagnostics = vec![];

    if compatibility_supported {
        match request(Attempt::Compatibility).await {
            Ok(value) => {
                return Ok(Outcome {
                    value,
                    used: Attempt::Compatibility,
                    diagnostics,
                });
            }
            Err(reason) => diagnostics.push(format!(
                "X_ITE_R: the WebGPU compatibility request failed ({reason}); retrying once with the current request."
            )),
        }
    }

    request(Attempt::Current).await.map(|value| Outcome {
        value,
        used: Attempt::Current,
        diagnostics,
    })
}
