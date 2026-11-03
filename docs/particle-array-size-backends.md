# Particle ARRAY_SIZE clamp and the GPU tests on each backend (issue #89)

Rule BR2.4: record the clamp value and the GPU test results for each backend used; the pass condition is that the value
matches the pinned reference, or that the difference is classified under BR2.2.

**Hosted macOS is virtualized.** The `macos-15` runner is a virtual machine with a paravirtual graphics device. A result from
it is never a physical-Mac or physical-Metal validation. No physical Mac is available to this work.

## The clamp value

| source | value |
| --- | --- |
| pinned X_ITE 16.5.0, `src/x_ite/Components/ParticleSystems/X3DParticleEmitterNode.js:338`: `const int ARRAY_SIZE = 32;` (the arrays `points [ARRAY_SIZE]`, `normals [ARRAY_SIZE]`) | 32 |
| X_ITE_R, `crates/x3d-runtime/src/particles.rs`: `const ARRAY_SIZE: usize = 32;` and `clamped_index(i) = min(i, ARRAY_SIZE - 1)` | 32, clamp index 31 |

The value matches. What the clamp does past 32 (WebGL clamps an out-of-range array index; every hit after the 31st lands in the
last entry) is X_ITE behavior that the port reproduces **on the CPU**, in the runtime. The Rust particle emitter and bounded
physics tests (`rea_b16_bounded_physics`, 27 tests; `rea_b17_emitter_hits`, 21 tests) replay the pinned X_ITE recordings
through the runtime and do not use the GPU, so their result cannot depend on the backend. They pass on all three backends below.
What does depend on the graphics backend is the X_ITE side: the pinned page's GLSL out-of-range write is undefined behavior
that the recordings measured on one GPU and driver (RTX 3060 Ti, Chrome and ANGLE), and on SwiftShader (#314). A Metal measurement
of the pinned page needs Chromium on a Metal device (see "Not done").

## GPU test results per backend

| backend | adapter | the particle GPU tests |
| --- | --- | --- |
| Vulkan, discrete (Linux) | NVIDIA GeForce RTX 3060 Ti, driver 595.99.02 | `rea_b14_particle_pick_matches_x_ite` pass (103 s), `rea_b15_particle_collision_matches_x_ite` pass (9 s); all 16 `GPU_TESTS` pass (160 s); `rea_b16` 27 and `rea_b17` 21 pass |
| Vulkan, software (Linux) | llvmpipe (LLVM 20.1.2, 256 bits), Mesa 25.2.8, selected with `VK_ICD_FILENAMES` set to `lvp_icd.json` | see below; `rea_b16` 27 and `rea_b17` 21 pass, `b69_gs_pick` 2 pass (worst value difference 3.84e-5) |
| Metal, virtualized (hosted `macos-15`) | "Apple Paravirtual device", backend Metal | `rea_b16` and `rea_b17` (CPU replay) pass. `rea_b14_particle_pick` (318 s) and `rea_b15_particle_collision` (4.2 s) **fail strictly** (run 41577822925 of PR #351, the first run in which they start a device on macOS; before PR #351 they skipped as "no adapter" because the headless device asked for Vulkan). The differences were not kept by that run: **unclassified**. The information step of `macos-tests.yml` now keeps the failure text. This branch does not contain the Metal default of PR #351, so its own macOS run shows the old skip |

### llvmpipe, classification (BR2.2)

Tolerance fixed before the run: absolute **1e-4** on every number (`X3D_TEST_SOFTWARE_TOL`). Final behavior: the variable is off by default, so the RTX and CI
runs stay strict. A test reads it **once at its start**: a value that is not a finite number above zero fails the run (even when every
compared value matches strictly), and so does the variable on a **hardware adapter** (a device type other than CPU, and a name
without llvmpipe, SwiftShader, softpipe or lavapipe), checked against the adapter the pointing pass takes. It applies only to the
comparison with the X_ITE records of `rea_b14` (the comparison with the port's own records stays strict) and to `rea_b15`; hit or
miss, the selected particle and the shape name stay exact. The decision is unit-tested without a GPU.

| test | result | classification |
| --- | --- | --- |
| `rea_b15_particle_collision_matches_x_ite` | strict (1e-6): fails, worst distance difference 5.24e-5, normals exact, hit states exact. At 1e-4: **pass** | numeric difference: the software rasterizer's depth rounding; within 1e-4 |
| `rea_b14_particle_pick_matches_x_ite` | strict: fails in 22 of the cases with 0 hit or selection differences in all of them. At 1e-4: 19 of the 22 cases pass; `pp-cone-geometry` (9 probes), `pp-cone-geometry-sphere` (136) and `pp-overlap-equal-depth` (3) still differ | the 19 cases: numeric difference within 1e-4. The three: **not within the tolerance and not closed.** The selected particle and the point agree (points within 4e-4 at most in the probes shown); the texture coordinate differs at a quad-edge pixel (0.995 against 0.727 in one probe) and the sphere normals by more than 1e-4. This looks like the software rasterizer's coverage and attribute interpolation at an edge pixel, but a second software backend has not confirmed it, so it stays open: it is not called a hardware constraint without that evidence. Owner decision: widen the software tolerance for these three, or leave the software backend out of the criterion |

## Not done, and why

* The pinned X_ITE's WebGL clamp has not been measured on Metal. It needs the oracle recorder (`x3d-oracle particles`, Chromium
  under Playwright) on a Metal device. The hosted macOS runner could run Chromium, but no recorder job exists for it, and its
  result would be virtualized. Physical Metal needs a physical Mac (owner approval: provide the second machine).
* The 16 `GPU_TESTS` on the virtualized macOS device: several of them need private answer keys that a hosted runner does not hold
  (`oracle/answer-keys.tsv`). The particle tests above need none, so only they run.
