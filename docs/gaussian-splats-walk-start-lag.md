# GaussianSplats: the WALK avatar starts two frames later in X_ITE (issue #69)

Status: a named known difference with an established cause. Rust does not reproduce it.

## Observation

In the pinned X_ITE 16.5.0 page, a WALK avatar falls under gravity. When the scene holds a GaussianSplats node, the
fall is two frames behind the same scene without splats. At probe frame 1 the avatar is at y = -0.2288 (with splats) or
-0.3923 (without). The splat fall at frame k + 2 equals the control fall at frame k. All 14 recorded frames agree to 1e-6.

Recorded cases (oracle/b69-gs-collision-baseline, five byte-identical X_ITE runs each):

| case | splats | lag |
| --- | --- | --- |
| gc-walk-lag-control | 0 | 0 |
| gc-walk-floor | 49 | 2 |
| gc-walk-lag-n441 | 441 | 2 |
| gc-walk-lag-n3969 | 3969 | 2 |

The splat count does not change the lag.

## Cause

The fall does not wait for the splat worker directly. The chain, in the pinned source, is:

1. `X3DNavigationContext.set_world__` runs on the browser's `initialized` event. It binds the active layer, and
   `set_activeNavigationInfo__` then sets the `viewer` field from the NavigationInfo ("WALK"). Before that, the viewer is
   "EXAMINE", for which the browser does not run the collision and gravity pass.
2. `X3DBrowser.checkLoadCount` fires `initialized` only when `loadCount` is 0.
3. `GaussianSplatsShape` adds a loading object when it is created and removes it on the worker's "ready" message. A second
   listener adds one again until the first "sorted" message. Both messages come from the worker between frames.

Trace of the pinned page (a temporary `advanceTime` log in anim.html, not kept), frames after the scene replaced the old
world. "viewer" is the browser's viewer at the frame:

| frame after scene replace | no splats: loadCount, viewer | 49, 400 or 4000 splats: loadCount, viewer |
| --- | --- | --- |
| 1 | 0, EXAMINE | 1, EXAMINE |
| 2 | 0, WALK | 1, EXAMINE |
| 3 | - | 0, EXAMINE |
| 4 | - | 0, WALK |

With splats, loadCount stays 1 for two more traversed frames, so WALK and gravity start two frames later. The harness
(oracle anim.html) counts a fixed number of traversed frames, so the avatar has fallen two frames less at every probe.
The same two frames appeared in the traces of 49, 400 and 4000 splats. Each message needs one frame turn; this is
consistent with the lag and is not separately proved.

The lag is in traversed frames since scene replacement, as the worker answers within those frames. A real browser would
show it as the time the worker needs, not as a fixed frame count.

## Decision

Rust loads splats synchronously and its avatar is WALK from the first frame. ISO/IEC 19775-1 V4.0 (23.4.4
NavigationInfo, 23.3.1 viewpoint and navigation text) does not define when, relative to loading, navigation starts or how
long a node load may take. The standard is silent on this point. GaussianSplats is an X_ITE extension and has no clause.
So Rust keeps its own timing and the lag is recorded as a known difference:

* name: `gaussian-splats-walk-start-lag`
* observable effect: at traversed frame k after the scene replaced the old world, a WALK avatar in a scene with
  GaussianSplats has the pose that the same scene without splats has at frame k - 2.
* tests: `b69_gs_collision_matches_x_ite` compares Rust frame k with X_ITE frame k + 2 (`LAGGED`);
  `walk_start_lag_follows_the_splat_load` pins the 0 and 2 frame lag in the baselines (no GPU needed).

## Ruled out

* The splat count (49, 441 and 3969 splats in the baselines, and 49, 400 and 4000 in the traces, give the same lag).
* Gravity speed: the first gravity frames of both cases have equal speed (-9.8e-6, then -0.3269), so the formula in
  `X3DRenderObject.gravitate` is the same; only the first frame differs.
* A different frame rate: the harness sets `getCurrentFrameRate` to 1/dt for both.
* A race: five identical runs of each case.

## REA evidence

ev_aad3c956e0dbf8c94af074a7eba4d76b28815998d9d267a0311a39b4729c5e96 (summary view of
ev_6bd862809642f227951ab669890876f0a3a353bd73ae0ef547310a4fbdfc6c72) on oracle/x_ite-pin: GaussianSplatsShape.js,
X3DBrowser.js, X3DNavigationContext.js, X3DNetworkingContext.js, anim.html. The inline script of anim.html was not
analyzed by REA; it was read directly.
