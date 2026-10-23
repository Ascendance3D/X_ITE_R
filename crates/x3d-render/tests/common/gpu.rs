//! `Gpu::new()` for the x3d-render tests, one start at a time per test binary.
//!
//! libtest runs the tests of one binary on parallel threads. On the NVIDIA driver (595.99.02):
//!
//! - parallel `wgpu` instance starts fault inside `libvulkan` (`vkCreateInstance`, see PR #11);
//! - a `vkDestroyDevice` deadlocks with a thread exit on another thread: `vkDestroyDevice` joins a
//!   driver thread, and that thread and the exiting thread wait in the driver's thread clean-up.
//!
//! The viewer tests' helper (`x3d-viewer/tests/common/gpu.rs`) makes its own device. These tests
//! use the production `Gpu::new()`, which makes its own instance, adapter and device, so this
//! helper only orders the calls. `START` is locked for all of `Gpu::new()`, and is free again
//! before the test renders. Each test still gets its own `Gpu` and renderer, but `KEPT` holds a
//! clone of each device and queue until the process ends, so no device is destroyed while other
//! tests run. A renderer's buffers and textures are still freed when its `Gpu` drops.
//!
//! `START` is a `static`, so it orders the threads of one process only. It does not order separate
//! test binaries.

use std::sync::Mutex;
use x3d_render::gpu::Gpu;

static START: Mutex<()> = Mutex::new(());
static KEPT: Mutex<Vec<(wgpu::Device, wgpu::Queue)>> = Mutex::new(Vec::new());

/// `Gpu::new()` while no other start of this binary runs.
pub fn gpu() -> Result<Gpu, String> {
    let g = {
        let _start = START.lock().unwrap_or_else(|e| e.into_inner());
        Gpu::new()?
    };
    KEPT.lock()
        .unwrap_or_else(|e| e.into_inner())
        .push((g.device.clone(), g.queue.clone()));
    Ok(g)
}
