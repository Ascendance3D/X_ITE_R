# Batch 09 navigation + pointer-picking lane

Worktree `.worktrees/rust-port-private/integration-b06-b08`, branch `integration/xite-rust-b06-b08-final`.
Base `12d0e46`, commit `4146c68` (local only, not pushed). Pinned X_ITE 16.5.0 oracle.

## Result

`cargo test --workspace --release -j 12`: 987 passed, 0 failed, 1 ignored (gen_oracle_worlds). Baseline 983 + 4 new shard tests.
`python3 -I oracle/tools/rea_b09_nav.py run --jobs 6`: 123 of 123 runs pass (stored as `oracle/rea-batch09-nav-baseline`, 123 entries + cases.txt).

## What changed in behavior

1. Pointer picking is now bit-exact with X_ITE. A CPU ray cast still chooses the shape and candidate triangles; a new wgpu (Vulkan) pointing pass reproduces `Pointing2.vs/.fs` (1x1 RGBA32F, shifted viewport, instance normal matrix) and supplies point, normal and texCoord. 1849 + 3721 probe grids (pick-hits, pick-instanced) match bit for bit.
2. Collision and gravity use a wgpu replica of X_ITE's 16x16 depth pass (`depth_gpu.rs`, `depth.wgsl`), including InstancedShape instance matrices, clip planes, readDepth unprojection and the normal transform. Poses now match X_ITE within 1e-6 (was up to 4.8e-4).
3. Bug fixes found by the new oracle: `add_roll` (wheel roll with a rotated viewpoint and chaser interruption), the `ANY` viewer name in the pose dump, InstancedShape double-click look-at (hit.model_view is the shape's own matrix; bbox is the union over instances), and explicit Shape `bboxSize`/`bboxCenter` in look-at (examine/lookat on such a shape was wrong).
4. Alt double-click lookAtPoint drift is now 0.0.

## Tests added

- `crates/x3d-viewer/tests/rea_batch09_nav.rs`: `rea_batch09_nav_shard0..3_match_x_ite`, 4 passed, 0 failed (each re-runs `x3d-drive` and compares with the stored X_ITE output: poses 1e-4, text exact, log exact, pickProbes bit exact, collisionProbes within 1e-6, images threshold 8 / 0.5%). Covers 123 runs (69 generated cases at MSAA 0 and the image cases also at MSAA 4, plus the older x10 plane/lookat/examine scripts, pick-hits, pick-instanced, touch-hit-events, sensors).
- Case families: walk/fly forward, back, turn, shift, ctrl-look, alt-pan, middle-pan, wheel roll (also rotated), wall, wall slide, two buttons, corner, fall, steps, small avatar, uncollide, ramp, pit, ANY, NONE, viewer switching, examine variants, plane, lookat, 12 transition worlds x2, navigationinfo switch, instanced wall/steps, examine on InstancedShape, examine with explicit bbox.
- Largest observed pose delta over all cases 5.8e-7 (tolerance 1e-4 unchanged). Largest collision probe distance delta 5.5e-7.
- Untouched suites still pass: physics, rendering, multi_viewer_b08 17/17, physics_rem_fall, shaders.

## Recommended ledger status (recommendation only)

| entry | status | basis |
| --- | --- | --- |
| sub:pointer.picking | COMPLETE | bit-exact point/normal/texCoord/id on 2 probe grids, MSAA independent (picking pass has no MSAA), with GPU |
| node:GeoTouchSensor | COMPLETE | geo-touch suite 121/121, touch-hit-events case; the "repeat hit events skipped" ledger note is stale |
| node:InstancedShape | COMPLETE for pointer picking and collision, rendering status unchanged | pick-instanced, walk/fly-instanced-*, examine-instanced-dblclick |
| sub:nav.examine | COMPLETE | variants + MSAA 0/4 |
| sub:nav.walk-fly | COMPLETE | MSAA 0/4 |
| sub:nav.plane, sub:nav.lookat | COMPLETE | x10 plane/lookat suites, alt-click, bbox look-at |
| sub:nav.none | COMPLETE | |
| sub:nav.collision-gravity | COMPLETE (GPU adapter required) | see decisions 1 and 3 |
| sub:nav.transitions | COMPLETE | 12 worlds x2 plus interruption cases |

## Known gaps (not hidden)

- Points and lines (geometry type < 2) are not picked or depth-drawn on the GPU path.
- HAnim skin and particle shapes use CPU-prepared geometry in both passes.
- Ties between different shapes in the depth pass are decided with CPU double distances.
- Touch-event navigation (TouchStart/Move/End in lookat/plane/examine) has no oracle: the harness sends no touch events.
- The wireframe collision path (`Fill::Edges`) is still CPU.
- Collision distance differs from X_ITE by up to a few float32 ulp of the window z (Vulkan vs ANGLE-GL rasterizer/depth); pointing is exact. Candidate for a DIFFERENT note: depth-buffer ulp noise 5.5e-7 on distances.

## Files changed

New: `crates/x3d-render/src/{pointing.wgsl,pointing_gpu.rs,depth.wgsl,depth_gpu.rs}`, `crates/x3d-viewer/tests/rea_batch09_nav.rs`, `oracle/tools/{gen_rea_b09_nav.py,rea_b09_nav.py}`, `oracle/synthetic-rea-b09-nav/`, `oracle/synthetic-rea/{pick-hits,pick-instanced}.wrl` + scripts, `oracle/rea-batch09-nav-baseline/`.
Edited: `crates/x3d-render/src/lib.rs`, `crates/x3d-viewer/src/{pointer,walk,viewer,collision,nav,lookat}.rs`, `crates/x3d-viewer/src/bin/x3d-drive.rs`, `oracle/{x_ite-anim.mjs,x_ite-anim-msaa.mjs}`, `oracle/tools/x10_compare.py` (pickProbes, collisionProbes). No new pub modules in x3d-fields or x3d-scene; no new dependencies; no ledger files touched.

## Findings about pinned X_ITE

- InstancedShape is collidable per instance (one collision shape, instance matrix in the vertex shader).
- `X3DFlyViewer.fly` uses `Date.now` for dt.
- InstancedShape requires an X3D V4.0 header with `COMPONENT X_ITE:1`; VRML97 headers silently drop the node.
- GL (wgpu) backend on this machine fails with Device lost; Vulkan is used (`X3D_POINTING_BACKEND=gl` selects GL).

## Owner decisions needed

1. Bit-exact pointing and collision depend on a GPU adapter; without one `x3d-drive` falls back to the CPU ray cast / depth (not bit-exact). Accept as the documented requirement, or keep the CPU path as the fallback of record?
2. Points/lines picking on the GPU path: implement later or accept the CPU double values?
3. Collision-depth ulp noise (max 5.5e-7 distance, probes compared at 1e-6 absolute, pose tolerance untouched at 1e-4): record as DIFFERENT or accept as COMPLETE?
4. Touch-event navigation: harness extension needed (new event types) before COMPLETE can be claimed for touch input.
5. Ledger: the 15 `sub:nav.*` rows with no recorded gap text need the evidence column pointed at `rea-batch09-nav-baseline`.
