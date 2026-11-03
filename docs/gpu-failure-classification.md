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

* A physical Mac (Metal on Apple silicon or discrete hardware) is not available to this work. The 16 `GPU_TESTS` have never run on
  Metal, virtualized or not: the macOS job skips them by name. Running them on the virtual machine is a possible next step, but
  its result is virtualized evidence only.
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
