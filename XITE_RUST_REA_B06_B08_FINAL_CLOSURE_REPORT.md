# X_ITE Rust REA Batches 06-08 Final Closure Report

**Lane:** `XITE_RUST_REA_B06_B08_FINAL_INTEGRATION`
**Verdict (running):** `XITE_RUST_REA_B06_B08_FINAL_PARTIAL` (work in progress; sections marked TODO are not yet run)
**Pinned X_ITE:** 16.5.0 at `34cdf9f97`
**Integration branch:** `integration/xite-rust-b06-b08-final` (worktree `~/Projects/cybertown/.worktrees/rust-port-private/integration-b06-b08`), local only, not pushed.

## 1. Overall verdict

PARTIAL so far. Integration, the full workspace build, and the workspace tests are done. Two real shader defects were found and fixed. Three required gates are still open (see section 16). Nothing is promoted in the ledger yet.

## 2. Starting source states

| Source | State |
|---|---|
| Batch 08 recovery | `batch-08-snapshot` @ `f9d3223760aded9ca4cc4fb0287db1eb6ad0235c` (child of snapshot `a91f0a4`). Existed only on the `x_ite_r` remote; fetched read-only. |
| Codex repair | `/tmp/XITE_R_B06_B07_REPAIR_01`, branch `fix/xite-rust-b06-b07-qa` @ `9901c16`; code commit `3342611`. Fetched into the stage repo as a local branch. |
| Stage repo | `/home/ryan/x_ite_r_stage`, branch `feature/r0-3-e-retention-cleanup` @ `a91f0a4`, untracked `.env.local`, `AGENTS.md`, `CLAUDE.md`, `README.md`, three `untracked.tgz`. Not modified. |
| Original private worktree | `~/Projects/cybertown/.worktrees/rust-port-private/r0-3-e-retention-cleanup` @ `03f7a28`, 364 dirty entries. Not modified. |
| Recovery backup (before any change) | `~/backups/xite-rust-b06-b08/recovery/` : `private-r0-3-e.tar.zst`, `stage.tar.zst`, `codex-repair.tar.zst` (all exclude `target/`), `SHA256SUMS`; each archive passed `zstd -t`. Earlier backups untouched. |

## 3. Final integration SHA

TODO (set at the last commit).

## 4. Files changed on the integration branch

- `c884afb` cherry-pick (-x) of Codex `3342611`: `crates/x3d-runtime/src/nodes/mod.rs`, `crates/x3d-render/src/volume.rs`, `crates/x3d-runtime/tests/runtime.rs`.
- `5b3296a` shader fixes: `crates/x3d-runtime/src/lib.rs`, `crates/x3d-runtime/src/nodes/shaders.rs`, `crates/x3d-render/src/shader_gpu.rs`, `crates/x3d-render/src/traverse_shaders.rs`, and `oracle/rea-batch08-shaders-baseline/` (16 cases from pinned X_ITE, 2.0 MB).
- Tooling: `crates/x3d-render/examples/gpu_identity.rs`, `oracle/x_ite-gl-identity.mjs`.
- Restored (uncommitted, gitignored) symlinks for existing local prerequisites: `oracle/{items,items2,x1}-baseline.json`, `oracle/render-*-baseline`, `oracle/x_ite-pin/node_modules`, copied with `cp -P` from the original private worktree.

## 5. Codex repair integration evidence

- Applied with `git cherry-pick -x 3342611`; no conflicts. Batch 08 (`f9d3223`) does not touch any of the three files (`git diff a91f0a4 f9d3223 --stat` for them is empty).
- Semantic review: the old render-time `VOXELS` cache is gone; `State::Volume { voxels }` is set in `initialize_node` from `pre_value`. `VolumeData` still reads the live field. Open edge: slot reuse and disposal of `State::Volume` (TODO: explicit test).
- `initialized_volume_types_capture_voxels_once`: pass.
- `cargo build --release --workspace --locked`: exit 0.

## 6. Physics and particle findings

TODO. Not yet run: seven-rate rigid-body matrix, three picking inputs, VolumeEmitter/BoundedPhysicsModel replays (preserved inputs are listed in the Codex report section 6), WindPhysicsModel (stays PARTIAL).

## 7. Volume initialization parity

TODO. Pinned-X_ITE cases (initial, ROUTE pre-draw, Script pre-draw, post-draw, reload, parent removal, id reuse, cleanup; MSAA 0 and 4) are not yet written. Until they pass, Iso and Segmented stay PARTIAL.

## 8. Shader parity

Stepped X_ITE baselines were generated with `local_finish.sh shaders` (read first; it only builds and writes X_ITE-derived baselines; `WT` was overridden to the integration worktree).

| Run | Result |
|---|---|
| first run | 2 of 18 cases (case x MSAA) matched |
| after fix 1 | 14 of 18 |
| after fix 2 | 16 of 18 |

Fixes, both found by those comparisons against pinned X_ITE:

1. `ComposedShader`'s private `LoadSensor` held the parts, so Rust named Script-created parts `DEF _1`/`DEF _2` in the scene text. X_ITE's `collectCloneCount` skips private parents. `Runtime::clone_count` now skips fields of a private ComposedShader LoadSensor. Fixed 7 cases x 2 MSAA.
2. X_ITE writes only enabled clip planes to a custom program's `x3d_ClipPlane`; a plane switched off keeps clipping until relink (GL uniform state is per program). Rust rebuilt the uniform block per draw. `ComposedState.clip_planes` now holds the resident values (reset on each successful link). Fixed `environment-runtime`.

Still different (NOT recorded, no baseline, evidence in `evidence/b06b08-final/textures-runtime-msaa{0,4}/`): `textures-runtime`, frames 64/72/80, 64.8% of pixels differ. X_ITE: when an SFNode sampler field becomes null it only does `this.textures.delete(location)` and never rebinds, so the `map` sampler keeps its last unit and samples whatever texture is bound there (here the appearance's 2x1 texture). Rust treats the sampler as unbound. Same class as fix 2 (program-resident GL state) but needs texture-unit pool and binding emulation. Not implemented.

Caveats: `rea_batch08_shader_cases_match_x_ite` passing means 16 of 18 cases match, not 18. `cases.txt` can only contain cases Rust passed. Static shader shots (168) were not rerun on the GPU yet. Not yet tested here: shader compile errors beyond `xml-invalid`, shared shader nodes, reload, Group removal, stale ids, `#include`.

## 9. Batch 08 remaining entry results

TODO.

## 10. GPU MSAA 0/4 results

GPU identity (evidence `evidence/b06b08-final/gpu-identity.txt`):

- Rust headless `x3d_render::gpu::Gpu::new` (used by `x3d-drive`, so by `x10_compare.py`): `NVIDIA GeForce RTX 3060 Ti`, Vulkan, DiscreteGpu, driver 595.91.07. The machine also exposes `llvmpipe`; it was not selected.
- X_ITE side (system Chrome, `--use-gl=angle --use-angle=gl`): `ANGLE (NVIDIA Corporation, NVIDIA GeForce RTX 3060 Ti/PCIe/SSE2, OpenGL 4.5.0)`.
- So the 16 stepped shader baselines at MSAA 0 and 4 are real-GPU on both sides. Viewer integration tests create their own default-options wgpu instance; their adapter is not logged (TODO: log it).

Other GPU gates: TODO. Known failure: `multi_viewer_b08 x_ite_two_canvases_rigid_bodies_msaa4`, frame 40, `rb-a`: X_ITE pair vs alone 0, Rust pair vs alone 0, Rust vs X_ITE 0.0058 in both pair and alone. So it is a single-canvas rigid-body MSAA 4 difference, not a multi-viewer defect. Not yet investigated.

## 11. Workspace test results

`cargo test --release --workspace --no-fail-fast --locked` on the integration tree:

| Run | Pass | Fail | Ignored | Failing tests |
|---|---|---|---|---|
| 1 (before fixes, before restoring prerequisites) | 957 | 4 | 1 | `items_runtime_matches_x_ite`, `x1_runtime_matches_x_ite` (missing baseline symlinks), `x_ite_two_canvases_rigid_bodies_msaa4`, `rea_batch08_shader_cases_match_x_ite` (stepped baseline missing) |
| 2 (after fixes) | 960 | 1 | 1 | `x_ite_two_canvases_rigid_bodies_msaa4` |

The two corpus failures were missing local prerequisites (symlinks to `X_ITE/scratch/rust-port/oracle`), not defects; after restoring them `corpus` is 6/6. The ignored test has not been identified yet (TODO). Logs: `evidence/b06b08-final/b06b08-test*.log`.

## 12. Oracle comparison results

TODO: the Batch 08 gate runner and the established oracle tools have not been run. `_batch08_context/reports/tools/local_finish.sh` `gates`, `backup` and `apply` have not been run; `apply` is unnecessary (the recovery commit is the base).

## 13. Final REA ledger totals

TODO. Authoritative start: COMPLETE 214, PARTIAL 52, DIFFERENT 25, NOT_TESTED 33, MISSING 22 = 346. No change has been applied.

## 14. Outstanding PARTIAL and DIFFERENT entries

TODO (final). Known so far: `sub:physics.engine` stays PARTIAL (owner decision), WindPhysicsModel PARTIAL, InlineGeometry PARTIAL, all five shader entries PARTIAL, FontLibrary (CFF) PARTIAL, multi-viewer PARTIAL, GaussianSplats PARTIAL.

## 15. Final backup and restore evidence

TODO. Must include the symlink targets in `X_ITE/scratch/rust-port/oracle` (items, items2, x1 baselines and render baselines), which are in no earlier backup, or record their checksums.

## 16. Owner decisions still needed

TODO (final).

## 17. Recommendations for Batch 09

TODO (final).
