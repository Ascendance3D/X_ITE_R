# Metal reference variation (issues #69 and #89)

Owner decision 2026-11-04: platform-specific pinned X_ITE baselines are approved for reproducible hardware differences. This page records
the result of the 29-case campaign on the physical M1 (Apple M1, macOS 27.0.1, Chrome 156, ANGLE Metal, Rust on wgpu/Metal, master
`b5aef382` plus this branch). Reference variation is not a REA DIFFERENT; a case where Rust differs from the pinned page on the same
Mac is open until its cause is shown.

## Method

1. The pinned page (16.5.0, `34cdf9f97`) was recorded on the M1 three times per case with `oracle/x_ite-anim.mjs` / `x_ite-anim-msaa.mjs`
   (`oracle/tools/record_platform_baseline.py`). A recording is kept only if the three runs are identical (`loadFrames` follows Chrome's I/O
   and is left out) and it differs from the NVIDIA recording. 37 recordings of 37 cases were stable (0 unstable): `oracle/rea-b09-nav-apple-m1-baseline`
   (17), `rea-b24-nav-apple-m1-baseline` (8), `rea-b12-apple-m1-baseline` (10), `rea-b09-inputs-apple-m1-baseline` (2).
2. `X3D_BACKEND=metal X3D_REFERENCE_PLATFORM=apple-m1 X3D_EVIDENCE=ci`, `cargo nextest run -p x3d-viewer --release` on the Mac, the nine
   tests of the four files: 6 passed, 3 failed, no skip recorded. The thresholds are the existing ones; none was changed.
3. For comparison the same cases ran against the NVIDIA recordings (the 16-test GPU suite on Metal, issue #89: 5 passed, 11 failed, of which
   9 are in these four files; the other two are rea_b14 and rea_b15).

## Classification of the 29 case names (every one differs between the NVIDIA and the M1 recording)

| class | cases |
| --- | --- |
| PASS against the M1 recording; REFERENCE VARIATION (NVIDIA and M1 recordings differ reproducibly, Rust equals the M1 one) | 24: fly-middle-pan, fly-wall, geo-touch, pick-hits, pick-instanced, pk-overlap, pk-points, sensors, sensors-drag, sensors-touch, touch-hit-events, walk-ceiling-block, walk-ceiling-pass, walk-fall-then-walk, walk-pit, walk-radius-large, walk-ramp, walk-small-avatar, walk-step-above-limit, walk-step-below-limit, walk-step-large, walk-step-zero, walk-tilted-forward, walk-wall |
| FIXED: a Rust defect on Metal, shown with synthetic worlds recorded on the same M1, now PASS against the M1 recording | 3: pk-lines, pk-points-style, pk-events (see the last section) |
| FIXED: a Rust defect that is not specific to Metal (the viewer clock), now PASS against the M1 recording | 2: walk-steps, walk-instanced-steps (see "The two walk cases") |
| OPEN: Rust differs from the pinned page on the same Mac (not shown to be a Rust defect, not shown to be variation) | 0 |
| UNVERIFIED | none of the 29; `gs-empty` and `gs-visible-false` time out on both platforms |

## ParticleSystem pointing and collision (`rea_b14`, `rea_b15`)

Recorded on the M1 with `x3d-oracle particles` (`X3D_ORACLE_ANGLE=metal`, `oracle/tools/record_platform_particles.py`), three identical runs per case, 0 unstable. Stored flat as `oracle/rea-b14-particle-pick-apple-m1-baseline/NAME.json` + `NAME.f32` (23 of 24 cases differ from the NVIDIA recording) and `oracle/rea-b15-particle-collision-apple-m1-baseline/` (12 of 14).
* `rea_b15_particle_collision_matches_x_ite`: **PASS** against the M1 recordings (14 cases); it failed on Metal against the NVIDIA recordings (111 differences).
* `rea_b14_particle_pick_matches_x_ite`: 23 of 24 cases match the M1 recordings (21 at the first campaign; `pp-slant-point-props` and the sprite coverage of `pp-row-point` pass since the Metal point-sprite rule, `pp-slant-line` and `pp-row-line` since the Metal line primitive, last section; `pp-row-line` is now 0 ulps off on the M1, its 3 ulp allowance is the NVIDIA recording's); the 24th, `pp-row-point-props`, is a **KNOWN UNRESOLVED DIFFERENCE**, pinned in `M1_KNOWN_DIFFS` of the test (see the last section), and the test passes with it pinned. It failed on 21 cases against the NVIDIA recordings.

With the particle tests the full run on the M1 (`X3D_REFERENCE_PLATFORM=apple-m1`, 11 tests of 6 files) before the line change: 7 passed, 4 failed (`rea_batch09_nav` shards 0 and 3, `rea_batch12`, `rea_b14`), no skip recorded.

## The two walk cases (same Mac, Rust vs the M1 recording)

Before the fix: **walk-steps** (MSAA 0 and 4) had 63 of 64 collision probes within 4e-7 of the pinned distance and probe 46 at 4.14e-5
(0.12766 vs 0.12762); **walk-instanced-steps** (MSAA 0) had probe 15 at 1.28e-6. Limit 1e-6. Both were probes at the edge of a depth step.

Cause, measured on the M1 (the pinned page and Rust, same pose probes, `oracle/x_ite-anim.mjs` extended with a read of the 16x16 depth
texture and of the uniform matrices of each probe's draws, scratch copy, nothing committed):

1. With the **page's own uniform matrices** (x3d_ProjectionMatrix and x3d_ModelViewMatrix of each probe, captured from the page) fed to the
   Rust depth pass, all 64 probes of `walk-steps` are **bit for bit** the page's: 16,384 depth values and 64 distances, 0 differences. The
   Rust depth shader, the Metal rasterization and the read-back are therefore not the cause. (162 reformulations of the vertex arithmetic,
   fused and unfused, were no closer to the page than the shader as it is.)
2. The matrices Rust fed were not the page's: every model-view matrix (640 of 640) and 25 of 64 projections were one float32 step off in
   an element. The avatar pose differed from the page's by up to 1e-7 from the first walking frame (frame 41) on, identically on the
   RTX (Rust on RTX and on the M1 have the same poses) and independent of the GPU (the page's own poses are the same on NVIDIA and on
   the M1). A pixel on the near-plane clip edge of a stair face turns a 1e-9 pose difference into 4e-5 in the probe.
3. The pose difference was the viewer clock. X3DFlyViewer takes `dt = (Date .now () - startTime) / 1000` of two **millisecond**
   numbers (`Date .now ()` is `(t0 + k * dt) * 1000`, about 9.5e11, one float64 step 1.2e-4 ms). The Rust viewer subtracted two
   **seconds** numbers (about 9.5e8, one step 1.2e-7 s) that round differently, so each step was off by up to 1.2e-7 s of
   walking. `nav.rs` now keeps `startTime` and `now` in milliseconds like the page. `rea_batch09_nav_clock` pins the poses of
   `walk-steps` (frames 35 to 100) to the page's to 1e-13 without a GPU.

After the fix on the M1: `walk-steps` largest probe difference 2.84e-7 (34 of 64 probes exact), `walk-instanced-steps` 1.89e-7 (43 of 64
exact); both within the existing 1e-6 limit, no tolerance changed. They are not bit for bit: from frame 116 on (the stair step-up reads
the f32 depth) the poses differ again by up to 3e-7, and the page's depth values at the probes differ from Rust's by up to a few hundred
float32 ulps in single pixels. That residue stays inside the limit.

## Line and point-sprite coverage on Metal (issue #89, `pk-lines`, `pk-points-style`)

Both cases were open in the earlier campaign; each cause is now measured on the M1 with minimal synthetic worlds
(`oracle/synthetic-metal-coverage`, made by `oracle/tools/gen_metal_coverage.py`), recorded with the pinned page on the M1
(`oracle/tools/record_metal_coverage.py`, three identical runs per world, 0 unstable; `oracle/metal-coverage-apple-m1-baseline/`,
`cases.txt` and `PROVENANCE.json` list the run hashes). Every probe is a pixel centre, so a probe is one coverage sample. The test is
`crates/x3d-viewer/tests/metal_coverage.rs`; it compares only on a Metal device with `X3D_REFERENCE_PLATFORM=apple-m1`.

* **pk-lines, cause: the line width limit.** ANGLE on Metal reports `ALIASED_LINE_WIDTH_RANGE` as `[1, 1]` (the NVIDIA host: `[1, 10]`), and the
  pinned page (`LineProperties.transformLines`) turns every line wider than the limit into triangles. The 14 hit/miss flips were the one
  `lineWidth` > 1 line of the world, which the page drew as triangles and Rust as a `gl.lineWidth` line. Rust now takes the limit from the
  adapter backend (`gpu::max_line_width()`: 1 on Metal, 10 elsewhere, set when a renderer or a pointing buffer takes its device); the
  NVIDIA/Vulkan path is the same branch with the same constant. 1 px lines cover the same pixels on the M1 and on the NVIDIA host (the
  same probes hit in both recordings: `ml-lines-a`, `ml-lines-b`); what differed from Rust there is in the 1 px line bullet below. After the change `pk-lines` passes on the M1: no hit/miss flip, 407 of 2583 probes differ in the hit point by at most 243 float32 ulps (before: 184 probes, 14 flips,
  up to 45,784 ulps; the existing allowance is 398). With the 1 px line primitive of the bullet below, the 311 hits that still differ in `pk-lines` at MSAA 0 (of 1280, at most 243 ulps, no hit/miss gap)
  are all on the two lines the page draws as triangles (`linewidthScaleFactor` 14: 127 hits, 243 ulps; 5: 184 hits, 50 ulps); every 1 px line hit is bit for bit.
* **pk-points-style, cause: the Metal point-sprite rule.** The rasterizer covers the pixel centres between the corners `centre -+ size / 2`
  rounded to 1/256 pixel (inside the round point mask where there is one; a plain `PointSet`, gl_PointSize 2, has none), and gl_PointCoord is
  `(pixel centre - origin) / size`, with `origin` = `centre - size / 2` rounded to 1/256 pixel and `size` the difference of the corners
  rounded to 1/16 pixel. The NVIDIA host floors the size to 1/32 pixel instead, which is what Rust did on every backend. A size above
  `ALIASED_POINT_SIZE_RANGE[1]` (511 on Metal, 2047 on the NVIDIA host) is drawn as 511 on Metal. Rust now uses this rule on a Metal
  device only (a branch decided by the backend; no tolerance). After the change `pk-points-style` passes on the M1: no hit/miss flip, 700 of 2479 probes differ in texCoord by at most 2 float32 ulps (before: 140 probes, up to
  294,337 ulps; the existing allowance is 3), and `pp-slant-point-props` of `rea_b14` passes. Across the sprite worlds all ids, hit points and
  normals are bit for bit; the texCoord is within one float32 step (1.3e-7) except for 9 sprites (2 of `ml-sprite-size`, 1 of `ml-sprite-low`, 6 of `ml-sprite-pos`, of 256 each) that sit on a tie of the 1/256 grid
  (`SPRITE_TIES`: the page and Rust pick the neighbouring grid value, up to 1 / (256 * size) off; one hit at the mask rim).
* **`rea_b21` on Metal.** The wide-line particle recordings of the M1 (`oracle/rea-b21-wide-lines-apple-m1-baseline`, 17 recordings and 3
  pictures of `pw-shadow-w6-behind`, which the page draws as triangles on Metal) are used with `X3D_REFERENCE_PLATFORM=apple-m1`; the known-gap
  tables of the test have an M1 variant (`KNOWN_GAPS_APPLE_M1`). Both tests pass on the M1. The CPU depth stand-in has no device to read the
  limit from, so the test passes `--aliased-line-width-max 1` to `x3d-particles-drive` for the M1 recordings.
* **`rea_b22` on Metal (two-layer LINE particle shadows, issue #89): REFERENCE VARIATION.** The test failed on Metal against the NVIDIA
  recordings: 40 failures, all in `tl-shared-w6` (layer 1 and 2 of 20 frames, 58 to 172 pixels over 8 against the limit 16); the other 5
  cases passed. The pinned page was recorded on the M1 three times per case (`record_platform_particles.py --base
  rea-b22-two-layer-shadows-baseline`, 6 cases, 0 unstable). The records (`js.json`, `js.f32`) of all six cases equal the NVIDIA ones, so
  no record override is kept. 20 of the 21 pictures of `tl-shared-w6` differ from the NVIDIA pictures (frames 3 to 20, 30, 45: 59 to 173 pixels
  over 8 per layer half; frame 2 does not) and are kept as `oracle/rea-b22-two-layer-shadows-apple-m1-baseline/tl-shared-w6.<frame>.png`; the
  pictures of the five 24 px cases are equal to the NVIDIA ones within the rule. Cause: the same as `pw-shadow-w6-behind` of `rea_b21`; the
  6 px shadow lines are over the `ALIASED_LINE_WIDTH_RANGE` limit of ANGLE Metal (1) and the page draws them as triangles, on NVIDIA (limit
  10) as `gl.lineWidth` lines. Rust on Metal already takes the limit from the adapter. Result with `X3D_REFERENCE_PLATFORM=apple-m1` on the
  M1: `rea_b22_two_layer_shadows_match_x_ite` passes (6 cases), Rust differs from the M1 pictures by at most 1 pixel in layer 1 and 2 in
  layer 2 per frame (maximum channel difference 21), against 172 and 167 (194) for the NVIDIA pictures. Negative control: the
  M1 picture of frame 10 replaced by the NVIDIA picture makes the test fail with 82 and 67 pixels over 8 in frame 10; without
  `X3D_REFERENCE_PLATFORM` the 40 failures return. No tolerance changed, no platform tolerance added, no NVIDIA recording changed.
* **1 px lines (`pk-events`, `pp-slant-line`, `pp-row-line`, `ml-lines-*`), cause: Rust drew them as two triangles on Metal.** The page's
  `gl.LINES` are Metal line primitives, so Metal's own rasterizer decides which pixels a line covers and how the varyings are interpolated
  along it. Rust emulated GL's diamond-exit rule with a quad of two triangles, a different path in the same hardware. Measured on the M1
  with `ml-lines-a` and `ml-lines-b` (2755 hits): the emulation missed 65 hit/miss probes (27 and 38) and 49 of the 2724 common hits were
  1 or 2 float32 ulps off in the point; the printed `0.4126096` vs `0.4126097` of `pk-events` is the same effect. With a real line primitive in
  the pointing pass on a Metal device (`gpu::metal_native_lines()`, a branch decided by the backend, set where `max_line_width` is; Vulkan and
  the others keep the emulation) the two worlds are bit for bit in id, point and normal: 0 hit/miss gaps, 0 differing hit points. The same
  change makes `pk-events` (scene text, MSAA 0 and 4), `pp-slant-line` (8 values of 0 ulps allowed before) and `pp-row-line` (0 ulps) match.
  `metal_coverage.rs` now requires the lines exact (its `LINE_ID_GAPS` table is gone). Particle LINE draws take the same branch (two
  vertices per segment instead of six).
* **`pp-row-point-props`: KNOWN UNRESOLVED DIFFERENCE, pinned (`M1_KNOWN_DIFFS` of `rea_b14_particle_pick.rs`).** Ids, hit points and normals
  are bit for bit. Of the 216 hits' texCoord numbers 300 are 1 to 3 ulps off (294 at 1 ulp), and 6 probes (5 at 2 ulps, 1 at 3 ulps) are more
  than the 1 ulp the case allows. The page's gl_PointCoord is a function of (pixel centre - origin) and the size alone (in the 3760 sprite
  coordinates of `ml-sprite-size`, 1 of 1475 (difference, size) groups has two page values), but it is not the correctly rounded quotient
  (2370 of 3760 equal it), and none of the float32 formulas tried reproduces it: the quotient and the product with the reciprocal
  rounded to nearest, down, up or toward zero (best 2736), the same with a per-size best reciprocal (fails for every size that is not a power of 2: 4.0 is exact, 3.0 matches 17 of 29), fused and unfused
  `p * r - o * r`, perspective-correct forms with w = 10, and corner interpolation by the two triangles of the sprite quad with the
  hardware doing the interpolation (2485 of 3760, a different error pattern, not exact). It is the interpolation arithmetic of the
  GPU, not a rule that is known. The test fails when the 6 probes or the 3 ulps change, including when they become exact.
* **Still open on the M1:** nothing of the 29 cases; the pinned known differences are the point-sprite coordinate of `pp-row-point-props` and the nine tie sprites (above).

## GaussianSplats picking (`b69_gs_pick`, issue #69)

The pinned page was recorded on the Apple M1 (Chrome 156, ANGLE Metal), 3 byte-identical runs, 6 cases at MSAA 0 and 4, plain and
logarithmic depth (`oracle/tools/b69_gs_pick_record.py platform apple-m1`). Against the NVIDIA recording, every hit or miss is equal,
and five cases differ by at most 7.1e-5. In gs-pick-ortho the hit distance differs by up to 4.1e-4 (304 of 433 hits over 1e-4).

Cause: `GaussianSplats.vs.js:344` computes `inverse (x3d_ProjectionMatrix) * clipSplatCenter` with the driver's GLSL `inverse ()`.
ANGLE Metal's result equals an f32 cofactor inverse. NVIDIA's equals the f64 inverse that Rust computes on the CPU
(`crates/x3d-render/src/splats_gpu.rs`, `inverse_f32`). With the OrthoViewpoint far value 1e4 the distance is a difference of two
terms near 5000, so the rounding of the inverse shows as about 1.7e-4. A shader cofactor inverse made Rust equal the M1 page bit for bit and
then differ from the NVIDIA page by 1.68e-4 (experiment, not kept).

Owner decision 2026-11-04: Rust keeps the CPU inverse. Under `X3D_REFERENCE_PLATFORM=apple-m1` the five other cases are compared
with the M1 recordings at 1e-4 on every component (Rust on Metal: at most 2.2e-5 plain, 1.9e-6 log). gs-pick-ortho has no M1 file:
it is compared with the NVIDIA recording and `DISTANCE_TOL_MACOS` (3e-4) on macOS (measured worst 2.43e-4).
