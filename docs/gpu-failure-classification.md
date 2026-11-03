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
| Metal, virtualized | GitHub-hosted `macos-15` arm64 (run 41557728237, PR #342) | the virtual machine's paravirtual device | not recorded by the run |

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
| `b69_gs_pick::splat_picks_match_pinned_x_ite` | hosted macOS | **numeric difference** | Hit or miss exact on all 2646 probes; only the hit distance (z) of gs-pick-ortho differs, by 1.7e-4 to 1.9e-4 at a distance of about 10.7 (relative 1.8e-5), the paravirtual device's depth rounding. Tolerance for macOS, fixed before the next run: 3e-4 (`VALUE_TOL_MACOS`); hit set exact; Linux keeps 1e-4. The pinned image's structure (which probes hit, the normal and the texture coordinates) is unchanged. |
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

No Linux failure exists: nothing on the NVIDIA GPU or on llvmpipe needs a classification.

## What stays open

* A physical Mac (Metal on Apple silicon or discrete hardware) is not available to this work. The 16 `GPU_TESTS` have never run on
  Metal, virtualized or not: the macOS job skips them by name. Running them on the virtual machine is a possible next step, but
  its result is virtualized evidence only.
* The eleven comparisons above are unclassified: they need a physical Mac or a diagnostic job (specific missing resource).

## REA evidence

Pinned X_ITE 16.5.0 (`oracle/x_ite-pin`, `src/x_ite/Browser/GaussianSplats/GaussianSplatsShape.js`):
ev_1d69a090391a33ce36dd9422fe885dd3c6a3ba5c0c9ceaa6c2ef0132377241db (summary
ev_b28037c89221c266c643477fbf5bc96984e3fc5f91f0c01170e6aea2cc082087). Baselines for the splat pick cases are the existing
`oracle/b69-gs-pick-baseline` (five byte-identical runs each); this change records no new baseline.
