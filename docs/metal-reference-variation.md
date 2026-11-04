# Metal reference variation (issues #69 and #89)

Owner decision 2026-11-04: platform-specific pinned X_ITE baselines are approved for reproducible hardware differences. This page records
the result of the 29-case campaign on the physical M1 (Apple M1, macOS 27.0.1, Chrome 156, ANGLE Metal, Rust on wgpu/Metal, master
`b5aef382` plus this branch). Reference variation is not a REA DIFFERENT; a case where Rust differs from the pinned page on the same
Mac is open until its cause is shown.

## Method

1. The pinned page (16.5.0, `34cdf9f97`) was recorded on the M1 three times per case with `oracle/x_ite-anim.mjs` / `x_ite-anim-msaa.mjs`
   (`oracle/tools/record_platform_baseline.py`). A recording is kept only if the three runs are identical (`loadFrames` follows Chrome's I/O
   and is left out) and it differs from the NVIDIA recording. 37 recordings of 37 cases were stable (0 unstable): `oracle/rea-b09-nav-baseline-apple-m1`
   (17), `rea-b24-nav-baseline-apple-m1` (8), `rea-b12-baseline-apple-m1` (10), `rea-b09-inputs-baseline-apple-m1` (2).
2. `X3D_BACKEND=metal X3D_REFERENCE_PLATFORM=apple-m1 X3D_EVIDENCE=ci`, `cargo nextest run -p x3d-viewer --release` on the Mac, the nine
   tests of the four files: 6 passed, 3 failed, no skip recorded. The thresholds are the existing ones; none was changed.
3. For comparison the same cases ran against the NVIDIA recordings (the 16-test GPU suite on Metal, issue #89: 5 passed, 11 failed of which
   10 are in these four files).

## Classification of the 29 case names (every one differs between the NVIDIA and the M1 recording)

| class | cases |
| --- | --- |
| PASS against the M1 recording; REFERENCE VARIATION (NVIDIA and M1 recordings differ reproducibly, Rust equals the M1 one) | 24: fly-middle-pan, fly-wall, geo-touch, pick-hits, pick-instanced, pk-overlap, pk-points, sensors, sensors-drag, sensors-touch, touch-hit-events, walk-ceiling-block, walk-ceiling-pass, walk-fall-then-walk, walk-pit, walk-radius-large, walk-ramp, walk-small-avatar, walk-step-above-limit, walk-step-below-limit, walk-step-large, walk-step-zero, walk-tilted-forward, walk-wall |
| OPEN: Rust differs from the pinned page on the same Mac (not shown to be a Rust defect, not shown to be variation) | 5: walk-steps, walk-instanced-steps, pk-lines, pk-points-style, pk-events |
| UNVERIFIED | none of the 29; `rea_b14_particle_pick` and `rea_b15_particle_collision` (recorded by `x3d-oracle particles`, `js.f32`) are not yet compared with M1 recordings; `gs-empty` and `gs-visible-false` time out on both platforms |

## The five open cases (same Mac, Rust vs the M1 recording)

* **walk-steps** (MSAA 0 and 4): 63 of 64 collision probes within 4e-7 of the pinned distance; probe 46 differs by 4.14e-5 (0.12766 vs 0.12762). Limit 1e-6.
* **walk-instanced-steps** (MSAA 0): all probes within 1.3e-6; probe 15 is 1.28e-6 over the 1e-6 limit.
* **pk-events** (MSAA 0 and 4): scene text differs at frames 41, 85, 86 in one printed line hit point, `0.4126096` vs `0.4126097` and `0.3964289` vs `0.396429` (one float32 ulp).
* **pk-lines** (MSAA 0 and 4): 184 of 2583 probes differ; 14 are hit/miss flips (the page hits, Rust misses), all on one diagonal line, at pixels where the line's staircase steps (a half-pixel coverage tie). Rust expands pointing-pass lines in the vertex stage to the NVIDIA GL rule (`pointing.wgsl`), on every backend. The rest differ by float32 ulps.
* **pk-points-style** (MSAA 0 and 4): 140 of 2479 probes differ; id and point agree, `texCoord` of attenuated point sprites differs by 1.0e-3; Rust equals the NVIDIA recording there.

Next: the point and line coverage rules on Metal (agent F works on the point-sprite tie rule), then `walk-steps` probe 46, then b14/b15.
