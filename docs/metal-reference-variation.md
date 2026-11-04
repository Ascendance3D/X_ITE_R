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
| OPEN: Rust differs from the pinned page on the same Mac (not shown to be a Rust defect, not shown to be variation) | 5: walk-steps, walk-instanced-steps, pk-lines, pk-points-style, pk-events |
| UNVERIFIED | none of the 29; `gs-empty` and `gs-visible-false` time out on both platforms |

## ParticleSystem pointing and collision (`rea_b14`, `rea_b15`)

Recorded on the M1 with `x3d-oracle particles` (`X3D_ORACLE_ANGLE=metal`, `oracle/tools/record_platform_particles.py`), three identical runs per case, 0 unstable. Stored flat as `oracle/rea-b14-particle-pick-apple-m1-baseline/NAME.json` + `NAME.f32` (23 of 24 cases differ from the NVIDIA recording) and `oracle/rea-b15-particle-collision-apple-m1-baseline/` (12 of 14).
* `rea_b15_particle_collision_matches_x_ite`: **PASS** against the M1 recordings (14 cases); it failed on Metal against the NVIDIA recordings (111 differences).
* `rea_b14_particle_pick_matches_x_ite`: 21 of 24 cases pass; **OPEN**: `pp-row-point-props` (3 values, 1 ulp allowed), `pp-slant-line` (8 values, 0 ulps allowed), `pp-slant-point-props` (15 values, 1 ulp allowed): one float32 step in the hit point of a point or line, the same family as `pk-lines`, `pk-events` and `pk-points-style`. It failed on 21 cases against the NVIDIA recordings.

With the particle tests the full run on the M1 (`X3D_REFERENCE_PLATFORM=apple-m1`, 11 tests of 6 files): 7 passed, 4 failed (`rea_batch09_nav` shards 0 and 3, `rea_batch12`, `rea_b14`), no skip recorded.

## The five open cases (same Mac, Rust vs the M1 recording)

* **walk-steps** (MSAA 0 and 4): 63 of 64 collision probes within 4e-7 of the pinned distance; probe 46 differs by 4.14e-5 (0.12766 vs 0.12762). Limit 1e-6.
* **walk-instanced-steps** (MSAA 0): all probes within 1.3e-6; probe 15 is 1.28e-6 over the 1e-6 limit.
* **pk-events** (MSAA 0 and 4): scene text differs at frames 41, 85, 86 in one printed line hit point, `0.4126096` vs `0.4126097` and `0.3964289` vs `0.396429` (one float32 ulp).
* **pk-lines** (MSAA 0 and 4): 184 of 2583 probes differ; 14 are hit/miss flips (the page hits, Rust misses), all on one diagonal line, at pixels where the line's staircase steps (a half-pixel coverage tie). Rust expands pointing-pass lines in the vertex stage to the NVIDIA GL rule (`pointing.wgsl`), on every backend. The rest differ by float32 ulps.
* **pk-points-style** (MSAA 0 and 4): 140 of 2479 probes differ; id and point agree, `texCoord` of attenuated point sprites differs by 1.0e-3; Rust equals the NVIDIA recording there.

Common to the open cases: a point or a line, or a probe at the edge of a depth step, where one float32 step or one pixel of coverage decides. Next: the point and line coverage rules on Metal, then `walk-steps` probe 46.
