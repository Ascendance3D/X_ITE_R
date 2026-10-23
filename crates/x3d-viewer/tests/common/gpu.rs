//! GPU start for the viewer tests, one start at a time per test binary.
//!
//! libtest runs the tests of one binary on parallel threads. On the NVIDIA driver (595.99.02):
//!
//! - parallel `wgpu` instance starts fault inside `libvulkan` (`vkCreateInstance`, see PR #11);
//! - a `vkDestroyDevice` deadlocks with a thread exit on another thread: `vkDestroyDevice` joins a
//!   driver thread, and that thread and the exiting thread wait in the driver's thread clean-up.
//!
//! So the binary makes one instance and adapter, the first time a test asks. `START` makes the
//! device starts run in sequence, and is free again before the test renders. Each test still gets
//! its own device, but `KEPT` holds a clone of each device until the process ends, so no device is
//! destroyed while other tests run. A viewer's buffers and textures are still freed when it drops.
//!
//! `START` is a `static`, so it orders the threads of one process only. It does not order separate
//! test binaries, or child processes such as `x3d-drive`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use x3d_viewer::embed::EmbeddedViewer;

static START: Mutex<()> = Mutex::new(());
static KEPT: Mutex<Vec<(wgpu::Device, wgpu::Queue)>> = Mutex::new(Vec::new());

// Bookkeeping for the startup tests: starts now in `START`, the most at one time, and the count.
static INSIDE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static STARTS: AtomicUsize = AtomicUsize::new(0);

/// Runs one GPU start, `f`, while no other start of this binary runs.
pub fn serialized<T>(f: impl FnOnce() -> T) -> T {
    struct Inside;

    impl Drop for Inside {
        // Also on a panic in `f`, so a later start does not see a stale count.
        fn drop(&mut self) {
            INSIDE.fetch_sub(1, Ordering::SeqCst);
        }
    }

    let _start = START.lock().unwrap_or_else(|e| e.into_inner());
    let inside = INSIDE.fetch_add(1, Ordering::SeqCst) + 1;
    let _inside = Inside;
    PEAK.fetch_max(inside, Ordering::SeqCst);
    STARTS.fetch_add(1, Ordering::SeqCst);
    f()
}

/// The binary's one instance and adapter, or `None` with no adapter (hosted CI). Call inside `START`.
fn adapter() -> Option<&'static wgpu::Adapter> {
    static ADAPTER: OnceLock<Option<(wgpu::Instance, wgpu::Adapter)>> = OnceLock::new();
    ADAPTER
        .get_or_init(|| {
            let instance =
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let adapter = pollster::block_on(
                instance.request_adapter(&wgpu::RequestAdapterOptions::default()),
            )
            .ok()?;
            Some((instance, adapter))
        })
        .as_ref()
        .map(|(_, adapter)| adapter)
}

/// A new device and queue for one test, or `None` with no adapter (hosted CI).
pub fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let device = serialized(|| {
        let adapter = adapter()?;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: EmbeddedViewer::wanted_features(adapter),
            ..Default::default()
        }))
        .ok()
    })?;
    KEPT.lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(device.clone());
    Some(device)
}

/// `x3d_render::gpu::Gpu::new()` as a probe, once per binary. The probe `Gpu` is never dropped, for
/// the same reason as `KEPT`.
pub fn renderer_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE
        .get_or_init(|| serialized(|| x3d_render::gpu::Gpu::new().map(std::mem::forget).is_ok()))
}

/// The most starts that were inside `START` at one time.
pub fn peak() -> usize {
    PEAK.load(Ordering::SeqCst)
}

/// The number of devices that `KEPT` holds.
pub fn kept() -> usize {
    KEPT.lock().unwrap_or_else(|e| e.into_inner()).len()
}

/// The number of starts so far.
pub fn starts() -> usize {
    STARTS.load(Ordering::SeqCst)
}
