# wgpu-hal 30.0.1, patched so the Vulkan backend never creates a descriptor pool with an empty pool-size array, and the GL backend claims compute shaders only where naga can write them

Two patches, each one a separate `.patch` file in this directory: `x3d-descriptor-pool.patch` (Vulkan, issue #89, below) and
`x3d-gles-compute.patch` (GL, owner decision 2026-11-12, section "The GLES compute patch").

| | |
| --- | --- |
| upstream | `wgpu-hal` 30.0.1 on crates.io, <https://github.com/gfx-rs/wgpu>, `MIT OR Apache-2.0` (see `LICENSE.MIT`, `LICENSE.APACHE`) |
| registry | `registry+https://github.com/rust-lang/crates.io-index`, download <https://static.crates.io/crates/wgpu-hal/wgpu-hal-30.0.1.crate> |
| upstream `.crate` sha256 | `b6b7fb58561a792bc237628ba0792e332de418fefe145f13b5ed8201e6d52f58` (the checksum that `Cargo.lock` held for it) |
| used by | `wgpu-core` and `wgpu` 30.0.1 (`crates/x3d-render`, `crates/x3d-viewer`), through `[patch.crates-io]` in the root `Cargo.toml` |
| a fork? | No. Nothing is published; this directory is a patched copy kept in this repository. |

## The problem (issue #89)

On the Intel UHD Graphics (Jasper Lake, "Intel(R) HD Graphics Gen11", Vulkan 1.3.215) with driver 31.0.101.2140 on Windows 11, every
Vulkan GPU test aborted with `0xc0000005` in 0.13 to 0.18 s. Crash site, from a throwaway vectored exception handler in a build with
line tables: `igvk64.dll` (the Intel Vulkan driver) inside `vkCreateDescriptorPool`, called from
`wgpu_hal::vulkan::descriptor::create_descriptor_pool` (`descriptor.rs`) <- `Bucket::create_pool` <- `DescriptorAllocator::register_layout`
<- `vulkan::Device::create_bind_group_layout` <- `wgpu_core::device::resource::Device::new` (the internal empty bind group layout
`empty_bgl`, made for every device) <- `wgpu::Adapter::request_device`. It happens in `request_device`, before any render code runs.

The call, as wgpu-hal 30.0.1 makes it: `sType` DESCRIPTOR_POOL_CREATE_INFO, `pNext` null, `flags` FREE_DESCRIPTOR_SET, `maxSets` 64,
`poolSizeCount` 0, `pPoolSizes` non-null (an empty `ArrayVec`). An empty layout has no descriptors, so `pool_sizes` is empty.

Minimal reproduction without wgpu: `tools/vk_pool_repro/` (an `ash` program, outside the workspace). Five runs per case, same program on both machines:

| case (`maxSets` 64, no `pNext`) | Intel UHD 31.0.101.2140 (Windows) | NVIDIA RTX 3060 Ti (Linux, one run each) |
| --- | --- | --- |
| device only (no pool) | 5 of 5 exit 0 | exit 0 |
| FREE_DESCRIPTOR_SET, `poolSizeCount` 0, `pPoolSizes` non-null | 5 of 5 `0xc0000005` in `igvk64.dll` (+0x630e71 four times, +0x64ea82 once) | exit 0 |
| FREE_DESCRIPTOR_SET, `poolSizeCount` 0, `pPoolSizes` null | 5 of 5 `0xc0000005` in `igvk64.dll` (+0x630e71 three times, +0x64ea82 twice) | exit 0 |
| flags 0, `poolSizeCount` 0 | 5 of 5 exit 0 | exit 0 |
| FREE_DESCRIPTOR_SET, `poolSizeCount` 1 (UNIFORM_BUFFER 1) | 5 of 5 exit 0 | exit 0 |

What this shows, and no more: the Intel driver 31.0.101.2140 crashes when `vkCreateDescriptorPool` gets FREE_DESCRIPTOR_SET and an empty
pool-size array; it does not crash with one pool size or without the flag. The same calls succeed on the NVIDIA driver. We did not
establish whether the Vulkan specification forbids an empty array at 1.3.215 or at the current version (no offline copy of the valid-usage
text: UNVERIFIED), so this is described as a driver compatibility problem, not as a wgpu-hal defect.

## The change

One added statement in `create_descriptor_pool` (`src/vulkan/descriptor.rs`), after `pool_sizes` is built: if it is empty, push one
`{ UNIFORM_BUFFER, descriptorCount 1 }`. A pool with one unused descriptor holds the same sets of a zero-descriptor layout, so
descriptor-set ownership, lifetimes and counting (`capacity`, `available`) do not change. A non-empty `pool_sizes` is untouched. No shader,
pipeline, buffer or texture code changes. The patch is `x3d-descriptor-pool.patch` (unified diff, the file uses CRLF line endings, so the
patch lines do too). Everything else is byte-identical to the crate, except `src/gles/adapter.rs` (the GLES compute patch below) and what is left out: `.cargo-ok` (not part of the package).

The first pool of every bind group layout type is made upfront (`register_layout`), so every device on Vulkan makes the empty-layout
pool at startup. After the change the original failing test (`rea_batch12_pick_cases_match_x_ite`) passes device creation on the Intel
machine and runs to its assertions.

## The GLES compute patch (owner decision 2026-11-12)

### The problem

On Linux with the NVIDIA driver (RTX 3060 Ti, 595.99.02), `request_device` on the GL backend failed with `Parent device is lost`
(`crates/x3d-viewer/tests/backend_gpu.rs`, `gl_backend_cannot_run_the_renderer_and_says_so`, in strict mode without the Mesa EGL vendor).
Measured with a standalone wgpu 30.0.1 program and `RUST_LOG=wgpu_hal=debug,wgpu_core=debug`:

1. NVIDIA's EGL supports desktop OpenGL, so `src/gles/egl.rs` creates a desktop OpenGL 3.3 core context (`driver_info` "3.3.0
   NVIDIA 595.99.02", GLSL 330).
2. `src/gles/adapter.rs` set `supports_compute` from `GL_ARB_compute_shader` alone, so the adapter reported `COMPUTE_SHADERS` and
   `INDIRECT_EXECUTION`.
3. wgpu-core 30.0.1 (`device/resource.rs`, `enable_indirect_validation`) then makes its indirect-call validation compute pipeline in
   `request_device`. naga 30.0.1's GLSL writer needs version 420 (ES 310) for a compute shader (`back/glsl/features.rs`,
   `check_feature!(COMPUTE_SHADER, 420, 310)`), so it failed: `indirect-validation error: ComputePipeline(Internal("The selected
   version doesn't support Features(BUFFER_STORAGE | COMPUTE_SHADER | DYNAMIC_ARRAY_SIZE)"))`.
4. wgpu-core maps the hal error to `DeviceError::Lost` (`device/mod.rs`): `RequestDeviceError { inner: Core(Device(Lost)) }`.

Mesa's EGL gives a context with GLSL 420 or later, so the same test passed with
`__EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json`.

### The change

In `Adapter::expose` (`src/gles/adapter.rs`), `supports_compute` also needs a shading language version that naga can write a compute
shader for: desktop GLSL 420 or later, or GLSL ES 310 or later. Nothing else changes. The values that follow from `supports_compute`
follow it: `COMPUTE_SHADERS`, `INDIRECT_EXECUTION` (through `GL_ARB_draw_indirect`), `SHADER_BINDING_LAYOUT` and the compute limits.
On a context with GLSL 420 or later, or on GLES 3.1 or later, the result is the same as before. On the NVIDIA GLSL 330 context, the adapter
now has no compute shaders, wgpu-core does not make the validation pipeline, and the viewer reports `the Gl backend cannot run this
renderer: the adapter has no compute shaders` (`x3d_viewer::backend::check_adapter`).

### Removal condition

Remove this patch when a published `wgpu-hal` checks the shading language version before it claims compute shaders (read the
`supports_compute` lines of its `src/gles/adapter.rs`), and `gl_backend_cannot_run_the_renderer_and_says_so` passes in strict mode on
the NVIDIA Linux driver without it.

## Removal condition (the descriptor-pool patch)

Remove the patch when EITHER a published `wgpu-hal` passes a non-empty pool-size array for an empty layout (read its `create_descriptor_pool`)
OR the Intel driver is fixed and `tools/vk_pool_repro` passes on the Intel machine, AND in both cases the 16 GPU tests (`GPU_TESTS` in
`.github/workflows/rust-tests.yml`) pass on the Windows Vulkan setup without the patch.

## Return to the upstream crate

1. Delete the `wgpu-hal = { path = "vendor/wgpu-hal" }` line (and its comment) from `[patch.crates-io]` in the root `Cargo.toml`.
2. Remove both patches first, each by its own removal condition. Then delete `vendor/wgpu-hal/` and the `tools/check_vendored_wgpu_hal.py` step in `.github/workflows/rust-tests.yml`, and the
   `vendor/wgpu-hal/...` entries in `EXCLUDE_PATTERNS` of `.github/workflows/code-review.yml`.
3. Run `cargo update -p wgpu-hal --precise 30.0.1` (or the newer version). The `Cargo.lock` entry is then:

```
[[package]]
name = "wgpu-hal"
version = "30.0.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "b6b7fb58561a792bc237628ba0792e332de418fefe145f13b5ed8201e6d52f58"
```

The patched entry differs from it by the two lines `source` and `checksum` only.

## Check it

```
python3 -I -B tools/check_vendored_wgpu_hal.py                       # downloads the crate, checks its sha256, compares every file
python3 -I -B tools/check_vendored_wgpu_hal.py <file.crate>          # offline, from a copy of the .crate
python3 -I -B tools/check_vendored_wgpu_hal.py --self-test           # the patch applier and the comparison, on synthetic data
```

The check fails on a changed file, a missing or extra file, a patch that does not apply, a vendored `descriptor.rs` that is not the
crate's file with the patch applied, a root `Cargo.toml` without the `[patch]` entry, and a `Cargo.lock` that still has the registry
source for `wgpu-hal`. It runs in "Fast checks" of `rust-tests.yml`.

## Maintenance

* Moving to a newer `wgpu-hal`: take the new crate, look whether `create_descriptor_pool` still builds an empty `pool_sizes` for a layout
  without descriptors (if it passes a non-empty array, delete the patch, see above), update the version and checksum here and in
  `tools/check_vendored_wgpu_hal.py`, regenerate `x3d-descriptor-pool.patch`, and run the 16 GPU tests on the Intel Windows machine.
* The review gate (`code-review.yml`) skips the files that the check proves identical to the crate. The modified `descriptor.rs`,
  `Cargo.toml`, `Cargo.toml.orig`, `build.rs`, the licenses, this file and the patch stay in the review.
