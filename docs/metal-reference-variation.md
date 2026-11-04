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
| FIXED: a Rust defect on Metal, shown with synthetic worlds recorded on the same M1, now PASS against the M1 recording | 2: pk-lines, pk-points-style (see the last section) |
| OPEN: Rust differs from the pinned page on the same Mac (not shown to be a Rust defect, not shown to be variation) | 3: walk-steps, walk-instanced-steps, pk-events |
| UNVERIFIED | none of the 29; `gs-empty` and `gs-visible-false` time out on both platforms |

## ParticleSystem pointing and collision (`rea_b14`, `rea_b15`)

Recorded on the M1 with `x3d-oracle particles` (`X3D_ORACLE_ANGLE=metal`, `oracle/tools/record_platform_particles.py`), three identical runs per case, 0 unstable. Stored flat as `oracle/rea-b14-particle-pick-apple-m1-baseline/NAME.json` + `NAME.f32` (23 of 24 cases differ from the NVIDIA recording) and `oracle/rea-b15-particle-collision-apple-m1-baseline/` (12 of 14).
* `rea_b15_particle_collision_matches_x_ite`: **PASS** against the M1 recordings (14 cases); it failed on Metal against the NVIDIA recordings (111 differences).
* `rea_b14_particle_pick_matches_x_ite`: 22 of 24 cases pass (21 at the first campaign; `pp-slant-point-props` and the sprite coverage of `pp-row-point` pass since the Metal point-sprite rule, last section); **OPEN**: `pp-row-point-props` (6 values, 1 ulp allowed) and `pp-slant-line` (8 values, 0 ulps allowed): the float32 steps of gl_PointCoord and of a 1 px line's hit point. It failed on 21 cases against the NVIDIA recordings.

With the particle tests the full run on the M1 (`X3D_REFERENCE_PLATFORM=apple-m1`, 11 tests of 6 files): 7 passed, 4 failed (`rea_batch09_nav` shards 0 and 3, `rea_batch12`, `rea_b14`), no skip recorded.

## The three open cases (same Mac, Rust vs the M1 recording)

* **walk-steps** (MSAA 0 and 4): 63 of 64 collision probes within 4e-7 of the pinned distance; probe 46 differs by 4.14e-5 (0.12766 vs 0.12762). Limit 1e-6.
* **walk-instanced-steps** (MSAA 0): all probes within 1.3e-6; probe 15 is 1.28e-6 over the 1e-6 limit.
* **pk-events** (MSAA 0 and 4): scene text differs at frames 41, 85, 86 in one printed line hit point, `0.4126096` vs `0.4126097` and `0.3964289` vs `0.396429` (one float32 ulp).
* **pk-lines** and **pk-points-style**: no longer open, see the next section.

Common to the open cases: a probe at the edge of a depth step, or a printed float32, where one float32 step decides. Next: `walk-steps` probe 46.

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
  same probes hit in both recordings: `ml-lines-a`, `ml-lines-b`), so there is no Metal rule for them; the remaining difference to Rust there
  is the pre-existing end/tie gap of the quad emulation of GL's diamond-exit rule (27 and 38 probes of 8449 and 8705, recorded in
  `LINE_ID_GAPS`, the same on both platforms). After the change `pk-lines` passes on the M1: no hit/miss flip, 407 of 2583 probes differ in the hit point by at most 243 float32 ulps (before: 184 probes, 14 flips,
  up to 45,784 ulps; the existing allowance is 398).
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
* **Still open on the M1:** `rea_b14` `pp-row-point-props` (6 values, 1 ulp allowed; the page's gl_PointCoord is up to 2 ulps from the correctly rounded
  division, and none of the float32 formulas tried (division, reciprocal product, interpolation between the corners) reproduces it bit for bit) and `pp-slant-line` (8 values, 0 ulps allowed; the
  1 px line end/tie gap above); `pk-events` (the printed ulp above).
