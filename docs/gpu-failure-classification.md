# GPU test classification (issues #69 and #89)

Rule BR2.2: a failing GPU test is an implementation defect, a numeric difference (with the tolerance fixed before the run, and
the pinned image's structure kept) or a hardware constraint (with its reason). This document records the classification of the
GPU tests on the two backends that exist today, and what each result does and does not show.

**Hosted macOS is virtualized.** The `macos-15` runner is a virtual machine with a paravirtual graphics device. Nothing here
is a physical-Mac or physical-Metal-device validation, and the ledger criterion "the GPU tests run on Metal" stays open for a
physical device (owner approval: provide the second machine).

## Backends and machines

| backend | machine | adapter | driver |
| --- | --- | --- | --- |
| Vulkan, discrete GPU | Linux workstation | NVIDIA GeForce RTX 3060 Ti | NVIDIA 595.99.02 |
| Vulkan, software | same machine (`VK_ICD_FILENAMES` set to the Mesa `lvp_icd.json`) | llvmpipe (LLVM 20.1.2, 256 bits) | Mesa 25.2.8 |
| Metal, virtualized | GitHub-hosted `macos-15` arm64 (runs 41557728237 and 41575177567) | "Apple Paravirtual device", backend Metal (printed by `b69_gs_pick`) | no driver string reported |

## Linux, NVIDIA (strict, answer keys present, `--profile ci`)

The 16 tests of `GPU_TESTS` (the tests the hosted CI runs skip), at master `f0cdaf6c`: **16 passed, 0 failed** in 160 s.
Slow (over 60 s) but passing: `rea_batch08_net_cases_match_x_ite` (141 s), `rea_b14_particle_pick_matches_x_ite` (103 s),
`rea_batch09_inputs_cases_match_x_ite` (72 s).

The GaussianSplats pick and collision tests, `gpu_start`, `rea_b16_bounded_physics` and `rea_b17_emitter_hits`
(61 tests): **61 passed** on the RTX 3060 Ti and **61 passed** on llvmpipe.

| `b69_gs_pick` (2646 probes, 1229 hits) | worst value difference | bit-identical probes |
| --- | --- | --- |
| RTX 3060 Ti, MSAA-independent pinned baselines | 7.06e-5 (gs-pick-default) | 1060 of 1229 hits |
| llvmpipe | 3.84e-5 (gs-pick-basic) | 0 of 1229 hits |
| hosted macOS (virtualized), first run | 1.9e-4, hit distance only, gs-pick-ortho | not recorded |

## Classification of the failures that exist

| test | where | classification | tolerance or reason |
| --- | --- | --- | --- |
| `gpu_start::renderer_probes_and_device_starts_share_the_lock` | hosted macOS | **implementation defect** | `x3d_render::gpu::headless_backends` returned Vulkan only, so `Gpu::new()` and `PointingGpu::new_headless()` found no adapter on macOS while every other start (default backends) found Metal. The assertion "the probe and the device start disagree on the adapter" is the symptom. The same defect made every test that starts its device through `Gpu::new()` skip as "no adapter" on macOS instead of running. Fixed in this change: the default is Metal on macOS (`headless_backends_on`, unit tests `macos_defaults_to_metal_and_still_accepts_a_named_backend` and `the_host_default_is_the_one_for_the_host_os`). The comparison that fails before and passes after is the hosted macOS run: before, run 41557728237; after, the macOS tests check of this pull request. |
| `b69_gs_pick::splat_picks_match_pinned_x_ite` | hosted macOS | **numeric difference** | Hit or miss exact on all 2646 probes; only the hit distance (z) of gs-pick-ortho differs, by 1.7e-4 to 1.9e-4 at a distance of about 10.7 (relative 1.8e-5), the paravirtual device's depth rounding. Limit for macOS, fixed before the next run: 3e-4 on the hit distance only (`DISTANCE_TOL_MACOS`, component z of the hit point); every other component keeps 1e-4 on every platform (test `only_the_distance_gets_the_macos_limit`); hit set exact. The pinned image's structure (which probes hit, the normal and the texture coordinates) is unchanged. |
| `b69_gs_pick::splat_picks_under_logarithmic_depth_match_pinned_x_ite` | hosted macOS | **numeric difference** | The same, the same case and the same values. Same tolerance. |
| `audio_pull::late_connected_oscillator_keeps_its_phase`, `rea_b24_audio_decode::same_rate_mp3_and_flac_are_bit_identical`, `vrml_audio_oracle::vrml_audio_matches_x_ite` | hosted macOS | not a GPU test (issue #88, owner of the audio lane) | listed only so that the 11 failures of the run are accounted for; they stay skipped by name |
| `x3d-evidence` unit tests (5) | hosted macOS | not a GPU test | listed for the same reason; not touched here |

### Eleven comparisons that the fix made visible (hosted macOS, run 41575177567 of PR #351)

With the headless device on Metal, the tests that start through `Gpu::new()` ran on the virtual machine for the first time. 1276 of
1287 tests passed; the two splat pick tests and `renderer_probes_and_device_starts_share_the_lock` now pass (the numeric
difference and the defect above are closed on the virtualized device). Eleven rendering comparisons fail:

| test | largest difference reported |
| --- | --- |
| `depth::standard_and_log_match_x_ite` | standard tilt pair: 2590 pixels over 8 |
| `depth::log_shows_what_standard_cannot` | a pixel is red where it must be green (wrong depth order) |
| `render_b69_gs_log::splats_in_logarithmic_depth_match_x_ite` | gs-ortho 71% of the pixels over 8, max 72; gs-linear 11.7%, max 106 |
| `render_rea_b08_gs::rea_b08_gs_renders_match_x_ite` | gs-linear 11.7% over 8, max 106 |
| `render_rea_b08_vs::rea_b08_vs_renders_match_x_ite` | vs-transparent-ibl 3.7%, max 255 |
| `render_rea_b09_inputs::rea_b09_inputs_renders_match_x_ite` | texgen-modes 8.9%, max 141; texgen-params 7.5%, max 169 |
| `render_rea_i92_items::rea_i92_render_items_match_x_ite` | i92-tr-opaque MSAA 4: 0.9%, max 64 |
| `render_rea_b07_vol::rea_b07_vol_renders_match_x_ite` | 0.6% to 1.1% over 8, max 98 |
| `rea_batch07_vol::rea_batch07_vol_cases_match_x_ite` | 14 failures, 0.6% over 8, max 112 |
| `rea_batch01::rea_batch01_cases_match_x_ite` | shading-wireframe MSAA 4: 0.9%, max 109 |
| `rea_b24_anchor_collision_instanced::rea_b24_anchor_collision_instanced_match_x_ite` | 14 failures, 516 of 2601 pick probes differ |

Classification: **unclassified, and not claimed as a hardware constraint.** These are structure-level differences (wrong depth order,
whole regions), not rounding, so they are outside any numeric tolerance. The evidence does not yet say whether the cause is the
virtual machine's paravirtual graphics device (a limit of the device) or a Metal-specific defect of the renderer: the same worlds
pass strictly on the RTX and on llvmpipe, and the logarithmic-depth tests failing together points at depth output (`frag_depth`)
or depth format handling on this device. Deciding needs a physical Mac, or a diagnostic job on the hosted runner that prints the
adapter, its features and limits, and the depth formats it supports; neither exists yet. Until then the eleven are skipped by
name in `macos-tests.yml` (comment there), and the criterion "the GPU tests run on Metal" stays open.

Run 41577822925 (information step, same branch, virtualized device): `b69_gs_pick::splat_picks_match_pinned_x_ite` passes (4.5 s);
`rea_b14_particle_pick_matches_x_ite` (318 s) and `rea_b15_particle_collision_matches_x_ite` (4.2 s) fail strictly. The step kept only
the result lines, not the differences, so those two are **unclassified** too; the step is to print the failure text in the next run.

No Linux failure exists: nothing on the NVIDIA GPU or on llvmpipe needs a classification.

## What stays open

* (Superseded by the section "Physical Metal run" below: the owner ran the 16 `GPU_TESTS` on a physical Apple M1 on 2026-11-02/03.)
  Before that run: a physical Mac was not available to this work, and the macOS job skips the 16 tests by name.
* The eleven comparisons above are unclassified: they need a physical Mac or a diagnostic job (specific missing resource).

## REA evidence

Pinned X_ITE 16.5.0 (`oracle/x_ite-pin`, `src/x_ite/Browser/GaussianSplats/GaussianSplatsShape.js`):
ev_1d69a090391a33ce36dd9422fe885dd3c6a3ba5c0c9ceaa6c2ef0132377241db (summary
ev_b28037c89221c266c643477fbf5bc96984e3fc5f91f0c01170e6aea2cc082087). Baselines for the splat pick cases are the existing
`oracle/b69-gs-pick-baseline` (five byte-identical runs each); this change records no new baseline.

## Hosted macOS diagnostic result (run 41583048922, "Apple Paravirtual device", Metal, virtualized)

Information-only job; not physical validation. Artifact `macos-diag` (logs and rust/pinned PNGs). Adapter: `IntegratedGpu`, backend Metal,
all depth formats and Rgba8/16F/32F support MSAA 4 and resolve; no GL adapter exists on the runner (`X3D_BACKEND=gl`: "no adapter", so the
GL comparison could not be made and the 11 tests skip there).

| group | evidence | classification |
| --- | --- | --- |
| `depth` x2, zfight worlds (`standard_and_log_match_x_ite`, `log_shows_what_standard_cannot`) | `Depth24Plus` resolves a 3e-8 depth difference at 0.1 on this device (float32 behavior); on the RTX it ties (24-bit behavior), as the pinned WebGL baseline does. Standard far-range: 61504 px wrong (a 250x250 block, red/green swapped); Logarithmic far-range: 0 px | **Hardware/platform constraint**: wgpu-hal maps `Depth24Plus` to `Depth32Float` here (no `Depth24Unorm_Stencil8` use). A tie that the pinned image depends on cannot occur. Proven for the format; the Logarithmic tilt-pair edge (2031 px, x 250..255) is the same cause by the same mechanism, not separately proven |
| `render_b69_gs_log`, `rea_b08_gs` | gs-linear 30592 px over 8 (max 106), same with Standard and Logarithmic; gs-ortho 71% (max 72) | **Unclassified.** Not explained by the depth format; candidate Metal renderer defect or paravirtual behavior, not demonstrated |
| `rea_b07_vol` | 0.6% to 1.1% of pixels, max 30 to 98, all cases | **Unclassified** |
| `rea_b08_vs` | vs-transparent-ibl 9576 px (3.65%), max 255 | **Unclassified** |
| `rea_b09_inputs` | texgen-modes 8.9%, texgen-params 7.5% (max 141, 169) | **Unclassified**; texture generation is plain shader math, so a device quirk is not the first guess |
| `rea_batch07_vol`, `rea_i92_items`, `rea_batch01`, `rea_b24` | failing as listed; per-case numbers in `failing_eleven_metal.log` | **Unclassified** |

None of the unclassified groups is claimed as a device constraint or as a renderer defect. The next step needs per-shader bisection on a
Metal machine (for example render with the texgen shader forced to a constant); a physical Mac is the sound comparison. They remain
skipped by name on the hosted runner.

## Final classification of the eleven comparisons (runs 41589129511 and 41594226558, hosted macOS, virtualized)

Supersedes the "Unclassified" rows above. Hosted macOS is a virtual machine with the "Apple Paravirtual device" (Metal); none of this
is physical-Metal validation. The probes are `device_diag.rs` and `device_math.rs` (information only, never gating).

| group | measured | classification | outcome |
| --- | --- | --- | --- |
| `gs-linear`, `vs-transparent-ibl` (`rea_b08_gs`, `rea_b08_vs`, `render_b69_gs_log`) | single colour channels of single splats and the refracting spheres differed (gs-linear 30592 px over 8, vs-transparent-ibl 9576). The tone map took `pow` of a negative linear color (NaN); on the RTX and llvmpipe a NaN source term blends as 0 (`nan_color_storage`) and the compiled Metal pipeline gave something else. With `pow (max (color, 0), ..)`: gs-linear max delta 4, vs-transparent-ibl 159 px over 8 (RTX: 145) | **implementation defect** (undefined `pow`) | fixed in PR #378; `rea_b08_gs` and `rea_b08_vs` un-skipped |
| `gs-ortho` (`render_b69_gs_log`) | the Background was black (0,0,0, expected 51,51,71) in logarithmic depth under an OrthoViewpoint, standard depth fine, the sphere form fine; the cube is scaled by the far plane (1e10), so clip coordinates reach 1e9. A full-screen triangle with 1e6 covers 4096 of 4096 pixels, with 1e7 3657. The comparison also failed on llvmpipe at master (33.5% over 8) | **implementation defect** (relies on huge clip coordinates) | fixed in PR #379 (a flat cube is scaled to the view); `splats_in_logarithmic_depth_match_x_ite` un-skipped after #378 |
| `texgen-modes`, `texgen-params` (`rea_b09_inputs`) | the four NOISE cubes: the hash chain `rand1` (`fract (sin (..) * 43758.5453)`, three deep) turns a few units in the last place of `sin` into a different value. On the lattice of its inputs, `sin` differs between the RTX and the macOS device in 74 of 81 values (largest 2.1e-5 absolute), the 3-deep chain in 81 of 81 (largest 0.92). `sin` itself is far from the CPU's on both (RTX worst 5675 ulps, macOS 5661 on the same input, near a zero crossing). The RTX differs from the pinned image too (404 px over 8, inside the test's limit); the macOS device differs by 23354 px (8.9%) | **device behavior** (`sin` precision, amplified by the hash) | a precision-safe hash cannot match the pinned image: that image is itself one GPU's noise. Skip stays, reason above; a tolerance would have to admit whole cubes, so none is added |
| `i92-tr-opaque`, `shading-wireframe` (MSAA 4 only) | 2392 and 2390 px over 8 (0.912%), edge pixels, max delta 64 and 109; the single-sample passes pass; mirroring the target (`set_gl_sample_orientation`) helps on the macOS device (texgen-prims 0.46% against 0.94%) but does not reach the RTX result | **device behavior** (4x sample positions are not Vulkan's; wgpu cannot set them) | measured limit 1.0% for multisampled passes on macOS, fixed before the run: PR #380; both un-skipped there |
| `depth` x2 (zfight) | `Depth24Plus` resolves a 3e-8 depth difference at 0.1 (float32 behavior); the RTX ties | **device behavior** (depth format precision) | skip stays |
| `rea_b24_anchor_collision_instanced` | `pickProbes` are compared bit for bit; the first differing probe (#332) differs by 6e-8 relative in `point`, `texCoord` (same id, same normal), 516 of 2601 probes differ, and the text of three events follows. Consistent with the depth format difference above (the pick point is unprojected from the depth buffer); not isolated by a probe of its own | **numeric difference, cause consistent with the depth format, not proven** | skip stays; no tolerance (the comparison is exact by design) |
| `rea_b07_vol`, `rea_batch07_vol` | 0.58% to 1.14% of the pixels over 8 (max 30 to 112), only along the contours of the volume styles (4x amplified difference image); the same worlds are within 0.002% on the RTX | **numeric difference, cause not isolated** | skip stays; a limit would not be a measured one for an unexplained cause |

Open for a physical Mac: whether the `sin` precision, the 4x sample positions, the depth format and the clip precision above are properties of the
paravirtual device or of Metal on Apple silicon.

## Physical Metal run (Apple M1, 2026-11-02/03), recorded next to the RTX and lavapipe results (issue #89)

Source of the Metal column: the owner's run on a physical MacBook (Apple M1, 8-core GPU, Metal 4, macOS 27.0.1 build 26A434, master
`fa7e8e49`, `X3D_BACKEND=metal`, strict evidence mode with the private keys allowed absent), 162 s, as posted on issue #89. It is a
physical Mac, not a virtual machine. macOS 27 meets "macOS 15+" but is not macOS 15. This document did not re-run it. The RTX column is
run 41731278473 (self-hosted GPU runner, 16 passed, 0 failed). The lavapipe column is the measurement of 2026-11-01 on issue #89
(software Vulkan, llvmpipe LLVM 20.1.2).

| test | RTX 3060 Ti, Vulkan | lavapipe, Vulkan | Apple M1, Metal |
| --- | --- | --- | --- |
| `x1_runtime_matches_x_ite` | pass | pass | pass |
| `items_runtime_matches_x_ite` | pass | pass | pass |
| `rea_b21_wide_lines_match_x_ite` | pass | fail | pass |
| `rea_b22_two_layer_shadows_match_x_ite` | pass | pass | pass |
| `rea_batch08_net_cases_match_x_ite` | pass | pass | **fail** (UDP ports 41234 to 41236: sent 5, refused 4) |
| `rea_b15_particle_collision_matches_x_ite` | pass | fail | **fail** (111 probes, distance 1e-6 to 3e-6) |
| `rea_b24_nav_shard0_match_x_ite`, `shard1`, `shard2` | pass | fail | **fail** (collision distances 2e-5 to 5.8e-4; one probe returns another shape id) |
| `rea_batch09_nav_shard0_match_x_ite` to `shard3` | pass | fail | **fail** (scene text differs from the first touched frame) |
| `rea_batch09_inputs_cases_match_x_ite` | pass | fail | **fail** (same) |
| `rea_batch12_pick_cases_match_x_ite` | pass | fail | **fail** (same) |
| `rea_b14_particle_pick_matches_x_ite` | pass | fail | **fail** (same) |
| total | 16 of 16 | 4 of 16 | 4 of 16 |

The two sets of 12 failures are not the same set: lavapipe fails `rea_b21_wide_lines` and passes `rea_batch08_net`; Metal passes
`rea_b21_wide_lines` and fails `rea_batch08_net`.

### What a reading of the source supports

Only these statements come from the source code. Nothing was run on Metal for this section.

1. **`rea_batch08_net` is not a numeric rendering comparison, and "sent 5" is a clue that `x3d-drive` ended early (unverified).**
   `crates/x3d-viewer/tests/rea_batch08_net.rs` starts a sender thread for the `--udp` cases. Each round of that thread waits at
   least 20 ms (the listener's read timeout, then a 1 ms sleep). A count of 5 datagrams per port is therefore about 0.1 s of the
   drive. The test checks the UDP verdict (needs at least 5 sent, at least 5 refused, refused at least half of sent) before it
   checks the exit status of the drive. A drive that ended at once on Metal gives exactly this text. This is a reading, not a
   result: it needs the `x3d-drive exited with ...` line, or the drive's stderr, from the Mac log. If the drive ran its full time,
   the cause is a macOS UDP behavior (ICMP port-unreachable on a connected loopback socket), and not the GPU.
2. **The probe tolerance of `rea_b15_particle_collision` is 1e-6** (`PROBE_TOL` in `rea_b15_particle_collision.rs`, line 34, with a few
   wider per-case entries in `PROBE_TOLERANCES`). The Metal differences, 1e-6 to 3e-6, are inside a factor 3 of it. The
   tolerance of `rea_b24_nav` is 1e-5 (`PROBE_TOL`, `rea_b24_nav.rs`, line 118); the Metal differences of that test, 2e-5 to 5.8e-4,
   are 2 to 58 times larger. No tolerance is widened here.
3. **The shader compile options of the Metal path in the wgpu-hal version of this workspace (30.0.1,
   `src/metal/device.rs`, lines 272 to 278) set the language version and `setPreserveInvariance(true)`, and nothing else.** The
   source has no call that turns fast math off. Metal's own default for fast math is on (Apple's `MTLCompileOptions`; not re-read
   for this section). Fast math lets the compiler use reduced-precision division, reciprocal and square-root forms. That is a
   candidate cause for the difference of the interpolated distance and hit position, on top of the rasterizer difference that the
   lavapipe diagnosis found (issue #89, comment on the reproduction). It is a candidate only: no run has tested it, and the
   separating test needs a Mac (the same worlds with fast math off, if the wgpu version offers a way to set it).
4. **The depth and pointing passes do not use blending or multisampling on `Rgba32Float`** (`blend: None` and `sample_count: 1` in
   `depth_gpu.rs` and `pointing_gpu.rs`), so the Apple GPUs' limits on blending and filtering of 32-bit float color formats are not
   the cause of those failures.
5. The lavapipe diagnosis (rasterizer interpolation of the NVIDIA reference, issue #89) is the only code-level cause that was
   tested by a controlled experiment. It was tested on lavapipe. For Metal it is a likely class and not a result: the owner's
   next step, the pinned X_ITE 16.5.0 in Chrome on the same Mac compared with the Rust output on that Mac, separates "Metal
   differs from the NVIDIA recording" from "the Rust code differs from X_ITE on the same device".

## Windows self-hosted runner with an Intel GPU (probe and one attempt, issue #89)

This section records what is known. It is **not** a result for the 16 tests: the test run did not finish.

**Adapter (probe run 41745913769 on PR #399, runner `gh-runner-ascendance3d-windows`, printed by the job):** Windows 11 Pro build 26200;
Intel Celeron N5095 (Jasper Lake), 4 cores, 8 GB of memory; Win32_VideoController "Intel(R) UHD Graphics" (PCI 8086:4E55), driver
31.0.101.2140, WDDM 3.1, DX feature levels up to 12_1, hardware GPU (dxdiag: `HARDWARE_TYPE_GPU`), 128 MB dedicated and 4018 MB shared
memory. `vulkaninfo --summary`: Vulkan 1.3, device "Intel(R) HD Graphics Gen11", integrated GPU, `DRIVER_ID_INTEL_PROPRIETARY_WINDOWS`,
conformance 1.3.1.1. It is a real GPU driver, not WARP and not software. The job runs in session 0 as NETWORK SERVICE. The runner has VS 2022
Build Tools, Node and Git; it has no cargo, rustup or Python.

**Attempt (run 41746630722, draft PR #399, not for merge):** the one-off workflow installed Rust 1.95.0 and cargo-nextest in the job's temp
directory and built the test binaries of `x3d-script` and `x3d-viewer` in release mode with MSVC: the build **succeeded** in 24 min 36 s
(00:25:52 to 00:50:28 UTC). The Vulkan step (the headless default backend) then ended after 9 s; its log was not readable while the
run was open, so its pass and fail counts are **not known**. The DX12 step (`X3D_BACKEND=dx12`) was still running 30 minutes after it
started (00:50:37 UTC) with no readable log; the `ci` nextest profile has no kill timeout. No per-test result exists from this runner.

What this shows: the native Windows build of the workspace compiles on this runner, and a hardware, non-NVIDIA, non-Apple GPU exists
for a third data point. What it does not show: any pass or fail of the 16 tests on that GPU. This runner is Vulkan and DX12, so even a
complete result would not satisfy the Metal criterion of issue #89.
