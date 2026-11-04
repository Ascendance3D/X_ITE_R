# Navigation subsystem item lists (issue #91)

Reference: pinned X_ITE 16.5.0 (`34cdf9f97`), `oracle/x_ite-pin/src/x_ite/Browser/Navigation/*.js`
(`ExamineViewer`, `WalkViewer`, `FlyViewer`, `X3DFlyViewer`, `PlaneViewer`, `LookAtViewer`, `X3DViewer`) and, for collision,
terrain following, gravity and step-up, `Rendering/X3DRenderObject.js` (`constrainTranslation`, `getClosestObject`,
`gravitate`), `Components/Layering/X3DLayerNode.js` (`collision`) and `Components/Navigation/NavigationInfo.js`
(`avatarSize`). Rust: `crates/x3d-viewer/src/nav.rs`, `walk.rs`, `nav/plane.rs`, `nav/lookat.rs`, `collision.rs`.

An item is one observable behavior. Status:

* **compared**: a committed test replays the same scripted input in the pinned X_ITE (headless Chrome, recorded baseline) and in
  Rust (`x3d-drive`) and compares scene text, the bound viewpoint's position, orientation, centerOfRotation and field-of-view scale
  (within 1e-4), and, where the case has them, collision probes and pictures.
* **missing**: no comparison; the reason is given.

Test names. `rea_batch09_nav` = `x3d-viewer::rea_batch09_nav rea_batch09_nav_shard{0..3}_match_x_ite`, cases in
`oracle/rea-batch09-nav-baseline/cases.txt`. `rea_b24_nav` = `x3d-viewer::rea_b24_nav rea_b24_nav_shard{0..2}_match_x_ite`
(new in this lane), cases in `oracle/rea-batch24-nav-baseline/cases.txt`, worlds and scripts in `oracle/synthetic-rea-b24-nav`
(generator `oracle/tools/gen_rea_b24_nav.py`, recorder `oracle/tools/rea_b24_nav.py`: three identical X_ITE recordings per case).
`rea_batch01` = `x3d-viewer::rea_batch01 rea_batch01_cases_match_x_ite`. `rea_batch09_inputs` =
`x3d-viewer::rea_batch09_inputs rea_batch09_inputs_cases_match_x_ite`. A case name in parentheses is the case inside that test.

The worlds of the cases are synthetic and authored for this repository.

## sub:nav.examine (ExamineViewer.js, X3DViewer.js)

| id | behavior | status | test (case) |
| --- | --- | --- | --- |
| E01 | left drag rotates around the center of rotation, horizon locked (StraightenHorizon true) | compared | rea_batch09_nav (examine-horizontal, examine-spin); rea_b24_nav (examine-cor-rotate, examine-ortho-rotate-wheel) |
| E02 | rotation near the poles: CRITICAL_ANGLE clamp and DISK_ANGLE sign switch | compared | rea_batch09_nav (examine-pole) |
| E03 | trackball rotation (StraightenHorizon false) | compared | rea_batch01 (browser-options-horizon-true, browser-options-horizon-false); rea_b24_nav (examine-trackball-pole) |
| E04 | spin on a quick release (SPIN_RELEASE_TIME, SPIN_ANGLE, SPIN_FACTOR), horizon locked; a slow release does not spin | compared | rea_batch09_nav (examine-spin, examine-spin-stopped); rea_b24_nav (examine-cor-spin) |
| E05 | spin on a quick release, trackball | compared | rea_b24_nav (examine-trackball-spin) |
| E06 | a wheel step removes the spin | compared | rea_b24_nav (examine-spin-wheel) |
| E07 | middle-button drag pans in the center plane (perspective and OrthoViewpoint) | compared | rea_batch09_nav (examine-pan-middle); rea_b24_nav (examine-cor-pan, examine-ortho-pan) |
| E08 | Alt + left drag acts as the middle button | compared | rea_batch09_nav (examine-alt-drag) |
| E09 | Shift or Ctrl + left drag and the right button have no extra action | compared | rea_batch09_nav (examine-shift-drag, examine-ctrl-drag, examine-zoom-right) |
| E10 | wheel zoom: a fraction (1/20) of the distance to the center of rotation, position and center chasers (MOVE_TIME) | compared | rea_batch09_nav (examine-wheel, examine-wheel-shift); rea_b24_nav (examine-cor-wheel, examine-ortho-rotate-wheel) |
| E11 | Viewpoint.centerOfRotation as rotation, zoom and pan center | compared | rea_b24_nav (examine-cor-rotate, examine-cor-wheel, examine-cor-pan, examine-cor-spin) |
| E12 | double click looks at the picked shape's bounding box (also an InstancedShape and explicit bboxSize); Alt looks at the point; miss; interrupt of spin and chasers | compared | rea_batch09_nav (x10-examine-dblclick, -alt, -miss, -during-spin, -during-chaser, -then-drag, -drag-interrupt, examine-instanced-dblclick, examine-bbox-dblclick) |
| E13 | a bound viewpoint change and a transition start disconnect chasers and spin | compared | rea_batch09_nav (examine-viewpoint-switch, trans-linear-examine, trans-animate-examine, trans-teleport-examine and their -interrupt cases) |
| E14 | one-finger touch rotate and two-finger pinch zoom | compared in the browser lane only (not `x3d-viewer`) | `crates/x3d-web/tests/browser/runtime02_case.mjs` `navigate` (README of `x3d-web`) |
| E15 | two-finger touch pan, double tap look at | missing | the stepped harness has no touch events; only one-finger rotate and pinch are compared (E14) |
| E16 | pointer press outside the viewport rectangle is ignored (`isPointerInRectangle` with a Viewport node) | missing | no multi-viewport navigation case |
| E17 | SLOW_SCROLL scroll factor 1/120 on macOS and Oculus user agents | missing | platform branch of `navigator.userAgent`; the oracle runs on Linux |
| E18 | rotate and zoom regressions of a private world (`x10-scripts/clip-*`, `rotate-then-zoom-*`, `msaa-orrery-*`) | compared by `oracle/tools/x10_compare.py` only | world is private and not committed, so no cargo test |

compared 13 (E01 to E13), 2 more compared outside the `x3d-viewer` tests (E14 browser lane, E18 tool only), missing 3 (E15 to E17).

## sub:nav.walk-fly (WalkViewer.js, FlyViewer.js, X3DFlyViewer.js)

| id | behavior | status | test (case) |
| --- | --- | --- | --- |
| F01 | left press and pointer offset drive forward and back (rubber band direction, speed from its angle and length) | compared | rea_batch09_nav (walk-forward, walk-back, fly-forward, fly-back) |
| F02 | turning: rubber band rotation weight (ROTATION_SPEED_FACTOR, ROTATION_LIMIT) | compared | rea_batch09_nav (walk-turn-left, walk-turn-right, fly-turn-left, fly-turn-right) |
| F03 | NavigationInfo.speed (0, 1, 4, 8, 16) | compared | rea_b24_nav (walk-speed-0, walk-speed-1, walk-speed-16); rea_batch09_nav (walk-forward, any-forward) |
| F04 | Viewpoint speed factor (GeoViewpoint elevation) | compared | rea_batch01 (geoviewpoint-walk, geoviewpoint-fly) |
| F05 | Shift multiplies the speed by 4 (SHIFT_SPEED_FACTOR) | compared | rea_batch09_nav (walk-shift, fly-shift); rea_b24_nav (fly-speed-1-shift) |
| F06 | Ctrl + left drag looks around (trackball, CRITICAL_ANGLE 0.99 stops at the vertical) | compared | rea_batch09_nav (walk-ctrl-look, fly-ctrl-look); rea_b24_nav (fly-ctrl-look-pole, fly-ctrl-look-no-straighten) |
| F07 | middle drag and Alt + left drag pan; WALK removes the downward component | compared | rea_batch09_nav (walk-alt-pan, walk-middle-pan, fly-alt-pan, fly-middle-pan) |
| F08 | wheel rolls the view by ROLL_ANGLE through the orientation chaser, also from a rotated viewpoint | compared | rea_batch09_nav (walk-wheel-roll, fly-wheel-roll, walk-wheel-roll-rotated, fly-wheel-roll-rotated) |
| F09 | a second button while one is held is ignored | compared | rea_batch09_nav (walk-two-buttons, fly-two-buttons) |
| F10 | StraightenHorizon false: FLY turns and looks around freely, WALK always straightens | compared | rea_b24_nav (fly-turn-no-straighten, walk-turn-no-straighten) |
| F11 | press at the border of the canvas | compared | rea_batch09_nav (walk-corner, fly-corner) |
| F12 | type list (`FLY WALK`), ANY, NONE, switching the viewer at run time | compared | rea_batch09_nav (any-forward, none-drag-wheel, viewer-switch) |
| F13 | transitions LINEAR, ANIMATE, TELEPORT, with drag, wheel and bind interrupts, NavigationInfo switch | compared | rea_batch09_nav (trans-linear-walk, trans-animate-walk, trans-teleport-walk, their -interrupt cases, trans-navigationinfo-switch) |
| F14 | FLY collides only while the button is held, no gravity in FLY | compared | rea_batch09_nav (fly-wall, fly-no-gravity) |
| F15 | Rubberband browser option (the drawn rubber band) | missing | overlay drawing; no picture comparison of it |
| F16 | touch: one finger drives, two fingers look around | missing | the stepped harness has no touch events |
| F17 | direction divided by the ContentScale rendering property (HiDPI pointer) | missing | the stepped harness runs at content scale 1 |
| F18 | ROLL_ANGLE for macOS and Oculus user agents (PI/512) | missing | platform branch of `navigator.userAgent` |

compared 14, missing 4.

## sub:nav.plane (PlaneViewer.js)

| id | behavior | status | test (case) |
| --- | --- | --- | --- |
| P01 | middle drag pans position and centerOfRotation together | compared | rea_batch09_nav (x10-plane-drag, x10-plane-drag-multi, plane-b09-pan) |
| P02 | Alt + left drag pans | compared | rea_batch09_nav (x10-plane-drag-alt) |
| P03 | a plain left drag does nothing | compared | rea_batch09_nav (x10-plane-drag-left, x10-plane-ortho-drag-left) |
| P04 | wheel changes fieldOfViewScale by 1/20 and keeps the point under the pointer | compared | rea_batch09_nav (x10-plane-wheel, x10-plane-wheel-in-drag) |
| P05 | zoom-out limit: fieldOfView * scale below PI for Viewpoint and GeoViewpoint | compared | rea_batch09_nav (x10-plane-wheel-out) |
| P06 | zoom-in limit: fieldOfViewScale not below 0.00001 | compared | rea_b24_nav (plane-zoom-in-limit) |
| P07 | OrthoViewpoint pan and zoom (no PI limit) | compared | rea_batch09_nav (x10-plane-ortho, x10-plane-ortho-drag, x10-plane-ortho-drag-alt, x10-plane-ortho-wheel) |
| P08 | double click looks at the shape's box, Alt at the point (only because `DEVELOPMENT` is true in `src/`) | compared | rea_b24_nav (plane-dblclick, plane-dblclick-alt) |
| P09 | touch: two-finger pan and pinch zoom, double tap | missing | the stepped harness has no touch events |
| P10 | scroll factor 1/160 on macOS and Oculus user agents | missing | platform branch of `navigator.userAgent` |

compared 8, missing 2.

## sub:nav.lookat (LookAtViewer.js)

| id | behavior | status | test (case) |
| --- | --- | --- | --- |
| L01 | double click looks at the picked shape's box (MOVE_TIME, ROTATE_TIME 0.3 s), Alt at the picked point | compared | rea_batch09_nav (x10-lookat-click, x10-lookat-click-alt) |
| L02 | miss, second click, interrupt of a look at, ortho variants | compared | rea_batch09_nav (x10-lookat-click-miss, -twice, -interrupt, x10-lookat-ortho-click, -ortho-click-alt, -ortho-click-miss, -ortho-click-twice, -ortho-click-interrupt) |
| L03 | left drag moves the view (orientation chaser), several moves per frame, drag then click | compared | rea_batch09_nav (x10-lookat-drag, x10-lookat-drag-multi, x10-lookat-drag-then-click, lookat-b09-drag) |
| L04 | wheel zoom toward the center of rotation, also inside a drag | compared | rea_batch09_nav (x10-lookat-wheel, x10-lookat-wheel-in-drag) |
| L05 | Viewpoint.centerOfRotation as the zoom and drag center | compared | rea_b24_nav (lookat-cor-wheel, lookat-cor-drag) |
| L06 | touch: two-finger pan and pinch, double tap (DBL_TAP_TIME) | missing | the stepped harness has no touch events |
| L07 | scroll factor 1/120 on macOS and Oculus user agents | missing | platform branch of `navigator.userAgent` |

compared 5, missing 2.

## sub:nav.collision-gravity (X3DRenderObject, X3DLayerNode.collision, NavigationInfo.avatarSize)

Non-particle only; the particle collision of PhysicsModels is in the B15 suites.

| id | behavior | status | test (case) |
| --- | --- | --- | --- |
| C01 | walk into a wall: stop at collisionRadius from the depth probe | compared | rea_batch09_nav (walk-wall, fly-wall); rea_b24_nav (walk-radius-large) |
| C02 | slide along a wall (project on the wall plane and up plane) | compared | rea_batch09_nav (walk-wall-slide, fly-wall-slide) |
| C03 | WallFriction option | compared | rea_batch01 (browser-options-friction-none, browser-options-friction-some) |
| C04 | `Collision.enabled` / `collide FALSE`, proxy, nested Collision, enabled switched at run time | compared | rea_batch09_nav (walk-uncollide); rea_batch09_inputs (collision-walk, collision-walk-disabled) |
| C05 | collision with every instance of an InstancedShape | compared | rea_batch09_nav (walk-instanced-wall, fly-instanced-wall) |
| C06 | ceiling: the collision volume reaches collisionRadius above the eye (a beam at 1.7 blocks, at 2.0 passes) | compared | rea_b24_nav (walk-ceiling-block, walk-ceiling-pass) |
| C07 | step-up: a step below stepHeight is climbed, above it is a wall | compared | rea_batch09_nav (walk-steps, walk-instanced-steps, walk-small-avatar); rea_b24_nav (walk-step-below-limit 0.7, walk-step-above-limit 0.8 with stepHeight 0.75) |
| C08 | stepHeight 0: a 0.1 curb is a wall | compared | rea_b24_nav (walk-step-zero) |
| C09 | stepHeight larger than avatarHeight (volume bottom limited to -radius) | compared | rea_b24_nav (walk-step-large) |
| C10 | gravity: fall to avatarHeight over the floor, acceleration from the Gravity option, frame-rate dependent | compared | rea_batch09_nav (walk-fall, walk-fall-then-walk); rea_b24_nav (walk-fall-gravity-20, walk-fall-gravity-0) |
| C11 | avatarHeight: fall to it (3), step up when less than stepHeight below it, stay when more | compared | rea_b24_nav (walk-tall-fall, walk-tall-pop, walk-tall-low) |
| C12 | terrain following: ramp (up) and pit (down) | compared | rea_batch09_nav (walk-ramp, walk-pit) |
| C13 | no gravity in FLY and ANY (FLY first) | compared | rea_batch09_nav (fly-no-gravity, any-forward) |
| C14 | avatarSize values: small avatar, radius 0, radius 1, defaults for a short or empty field, negative values (clamped to 0) | compared | rea_batch09_nav (walk-small-avatar); rea_b24_nav (walk-radius-zero, walk-radius-large, walk-avatar-size-short, walk-avatar-size-empty, walk-avatar-size-negative) |
| C15 | up vector is the viewpoint's local y axis (WALK inside a tilted Transform: forward, fall) | compared | rea_b24_nav (walk-tilted-forward, walk-tilted-fall) |
| C16 | GeoViewpoint up vector (WALK on the globe) | compared | rea_batch01 (geoviewpoint-walk) |
| C17 | gravity and the viewer's constrained moves only for the active layer; collide (Collision.isActive) and the collision shapes of every drawn layer, each with its own viewpoint, NavigationInfo and fall speed | compared | rea_gravity_layers (layers-gravity-active0/1/2, layers-gravity-switch, layers-gravity-midfall, layers-walls-active0/1) |
| C18 | collision and gravity are skipped while a transition is active | compared | rea_gravity_layers (trans-gravity-skip, trans-layer-skip: poses, the Collision wall's isActive count and the probes after the transition; trans-skip-probe-start); rea_batch09_nav (trans-linear-walk, trans-animate-walk, trans-teleport-walk). A getClosestObject call in the middle of a transition reads X_ITE's stale collision shapes: known gap, trans-skip-probe and trans-layer-probe |
| C19 | rendering modes of the collision pass (point and wire shading, LINE particles) | compared | rea_batch01 (shading-point-collision, shading-wireframe-collision) |

compared 19 (C18 with one known gap: the probe in the middle of a transition), missing 0.

## Differences found

* avatarSize[0] 0 (also a negative value): `Camera.ortho (-0, 0, ...)` has a zero width, X_ITE's depth read is NaN, the avatar
  cannot move and gravity does nothing (`walk-radius-zero`, `walk-avatar-size-negative`: position stays at z 12). Rust treated
  the avatar as free to move (pose delta 7.7 at frame 30, 75 at frame 110). Fixed in `collision.rs` (`collision_queries` returns a NaN distance for
  radius 0). This is an upstream degenerate case that Rust now reproduces.
* Collision probe distances (`collisionProbes`, float32 window z): Rust differs from X_ITE by at most 1.3e-6 in the ceiling,
  step and radius cases and 5.7e-6 in the tilted-Transform case (a slanted wall at 0.19, where the shape id also differs: X_ITE
  1, Rust 0; only the hit flag and the normal are compared). `rea_b24_nav` allows 1e-5, batch 09 allows 1e-6.

## Mutation checks (each reverted afterwards; the named case fails)

| mutation | failing case |
| --- | --- |
| step-up threshold `stepHeight` x 0.5 (`collision.rs`) | walk-step-below-limit (pose delta 0.70 at frame 60) |
| gravity acceleration x 2 | walk-fall (pose delta 0.72 and 1.14; pictures 3.2% and 24% over) |
| collision volume top `radius` x 0.2 | walk-ceiling-block, walk-ceiling-pass |
| radius 0 NaN depth removed | walk-radius-zero |
| PLANE zoom factor x 2 (`nav/plane.rs`) | x10-plane-wheel |
| LOOKAT zoom factor x 2 (`nav/lookat.rs`) | x10-lookat-wheel |
| EXAMINE zoom factor 1/20 to 1/10 (`nav.rs`) | examine-cor-wheel |
| walk and fly `SPEED_FACTOR` 0.007 to 0.0075 | walk-speed-1, walk-speed-16 |
