# X_ITE Rust REA Batches 06-08 Final Closure Report

**Lane:** `XITE_RUST_REA_B06_B08_FINAL_INTEGRATION`
**Verdict:** `XITE_RUST_REA_B06_B08_FINAL_PARTIAL`
**Pinned X_ITE:** 16.5.0 at `34cdf9f97`
**Integration branch:** `integration/xite-rust-b06-b08-final` (worktree `~/Projects/cybertown/.worktrees/rust-port-private/integration-b06-b08`), local only, not pushed.

## 1. Overall verdict

PARTIAL. Integration, build, the full gate runner, the seven-rate physics matrix and the 20,000-case picking sweep are done; two shader defects were found and fixed. Required closure gates not run: volume initialization parity, VolumeEmitter/BoundedPhysics replays, volume style tests, most shader and Batch 08 entry items. One workspace test fails (MSAA 4 multi-viewer). No ledger change was applied.

## 2. Starting source states

| Source | State |
|---|---|
| Batch 08 recovery | `batch-08-snapshot` @ `f9d3223760aded9ca4cc4fb0287db1eb6ad0235c` (child of snapshot `a91f0a4`). Existed only on the `x_ite_r` remote; fetched read-only. |
| Codex repair | `/tmp/XITE_R_B06_B07_REPAIR_01`, branch `fix/xite-rust-b06-b07-qa` @ `9901c16`; code commit `3342611`. Fetched into the stage repo as a local branch. |
| Stage repo | `/home/ryan/x_ite_r_stage`, branch `feature/r0-3-e-retention-cleanup` @ `a91f0a4`, untracked `.env.local`, `AGENTS.md`, `CLAUDE.md`, `README.md`, three `untracked.tgz`. Not modified. |
| Original private worktree | `~/Projects/cybertown/.worktrees/rust-port-private/r0-3-e-retention-cleanup` @ `03f7a28`, 364 dirty entries. Not modified. |
| Recovery backup (before any change) | `~/backups/xite-rust-b06-b08/recovery/` : `private-r0-3-e.tar.zst`, `stage.tar.zst`, `codex-repair.tar.zst` (all exclude `target/`), `SHA256SUMS`; each archive passed `zstd -t`. Earlier backups untouched. |

## 3. Final integration SHA

Code tip before this report commit: `git rev-parse integration/xite-rust-b06-b08-final~1` at the time of writing is the gates commit; the branch tip (which adds this report and its evidence) is the integration SHA. Read it with `git rev-parse integration/xite-rust-b06-b08-final` in the worktree; a report cannot contain its own commit hash.

## 4. Files changed on the integration branch

- `c884afb` cherry-pick (-x) of Codex `3342611`: `crates/x3d-runtime/src/nodes/mod.rs`, `crates/x3d-render/src/volume.rs`, `crates/x3d-runtime/tests/runtime.rs`.
- `5b3296a` shader fixes: `crates/x3d-runtime/src/lib.rs`, `crates/x3d-runtime/src/nodes/shaders.rs`, `crates/x3d-render/src/shader_gpu.rs`, `crates/x3d-render/src/traverse_shaders.rs`, and `oracle/rea-batch08-shaders-baseline/` (16 cases from pinned X_ITE, 2.0 MB).
- Tooling: `crates/x3d-render/examples/gpu_identity.rs`, `oracle/x_ite-gl-identity.mjs`.
- Restored (uncommitted, gitignored) symlinks for existing local prerequisites: `oracle/{items,items2,x1}-baseline.json`, `oracle/render-*-baseline`, `oracle/x_ite-pin/node_modules`, copied with `cp -P` from the original private worktree.

## 5. Codex repair integration evidence

- Applied with `git cherry-pick -x 3342611`; no conflicts. Batch 08 (`f9d3223`) does not touch any of the three files (`git diff a91f0a4 f9d3223 --stat` for them is empty).
- Semantic review: the old render-time `VOXELS` cache is gone; `State::Volume { voxels }` is set in `initialize_node` from `pre_value`. `VolumeData` still reads the live field. Open edge: slot reuse and disposal of `State::Volume`.
- `initialized_volume_types_capture_voxels_once`: pass.
- `cargo build --release --workspace --locked`: exit 0.

## 6. Physics and particle findings

All run on the integration tree with `evidence/b06b08-final/gates/run.sh` (derived from the B08 runner; read first; physics matrix logs redirected into `evidence/` so historical reports were not overwritten).

- Rigid-body frame-rate matrix against saved pinned-X_ITE baselines, tolerances unchanged: 11 stack worlds at 24/30/50/60/75/120/144 Hz = 11/11 at each of 7 rates; 47 physics worlds at 24/30/50/75/120/144 Hz = 47/47 at each of 6 rates, plus 47/47 at the base rate (`rea_b06` oracle). `bullet_dyn_scenes` 66/66, `bullet_box_box` and the frame-16 test pass. (60 Hz exists only in the stack set; that is how the historical matrix is built.) No divergence found, so no repair.
- Picking (`sub:physics.engine`, stays PARTIAL per owner): regenerated all 20,000 cases from pinned Ammo (`oracle/tools/ammo_pick_vectors.mjs`, seed 20261018; byte-identical to Batch 08's file). Rust: 3 differ, the same three. **Correction to the earlier report: they are not all cylinder pairs.** 4697 = cylinder vs triangle mesh, 5480 = cylinder vs cylinder, 11878 = cylinder vs height field. In all three Ammo reports no contact and Rust reports one. Every one has a cylinder on side `a`, and the 4000-case committed set passes at 0. One repair attempt (rounding the cylinder support point to float32) changed nothing and was reverted. Root cause still unproven; not promoted.
- ParticleSystem: 100-cycle removal test and the particle suites pass (`b06 particles` 37/37, `b07 ps` 103/103, 30+5+4 tests). Implementation untouched.
- VolumeEmitter (1.72e-5) and BoundedPhysicsModel (6.68e-6): **not reproduced this session.** The preserved worlds/controls exist (paths in the Codex report section 6) but are not staged in this tree's harness, and I did not stage them. Both remain proposed PARTIAL. No tolerance changed.
- WindPhysicsModel: PARTIAL, unchanged, not replayed.

## 7. Volume initialization parity

Not established. The Codex repair is integrated and its regression test passes; the 38-case Batch 07 volume suite passes at MSAA 0 and 4 in the same run (`b07 vol viewer`, images true), plus 290 render shots and the reload suite. The new pinned-X_ITE cases (ROUTE and Script writes before first draw, later writes, reload, parent removal, id reuse, cleanup) were **not written**. Iso and Segmented therefore stay PARTIAL pending that comparison (matches the Codex proposal). Slot reuse of `State::Volume` is not directly tested either; release.rs was not changed.

Volume styles: the Codex report confirms eight, not seven. I did not add per-style combined-sequence tests, so no style entry is promoted or changed.

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

Passing in this run: DIS/net suites (`rea_batch08_net`, `rea_b08_net_reload`, `render_rea_b08_net`) as part of the workspace run, gltf/svg/vs/gs/multiview suites except the one MSAA 4 case. Not done: InlineGeometry routed shared-geometry case, FontLibrary CFF, glTF extension/decoder audit, GaussianSplats beyond existing tests, VolumeScatter extra comparisons, shader items listed in section 8, `#include` support. Those entries stay as Batch 08 proposed (PARTIAL).

## 10. GPU MSAA 0/4 results

GPU identity (evidence `evidence/b06b08-final/gpu-identity.txt`):

- Rust headless `x3d_render::gpu::Gpu::new` (used by `x3d-drive`, so by `x10_compare.py`): `NVIDIA GeForce RTX 3060 Ti`, Vulkan, DiscreteGpu, driver 595.91.07. The machine also exposes `llvmpipe`; it was not selected.
- X_ITE side (system Chrome, `--use-gl=angle --use-angle=gl`): `ANGLE (NVIDIA Corporation, NVIDIA GeForce RTX 3060 Ti/PCIe/SSE2, OpenGL 4.5.0)`.
- So the 16 stepped shader baselines at MSAA 0 and 4 are real-GPU on both sides. Viewer integration tests create their own default-options wgpu instance; their adapter is not logged.

Item corpus on the GPU (`x3d-shot`, items2, 296 shots): MSAA 0 and MSAA 4 each 294 within tolerance, 2 over (`art/djascendance-live`, 1.85%/1.84%, and `art/rainbow-room`, 2.54%/2.55%, max level 94 and 82); these are the known GPU corpus exceptions, unchanged. Batch 02-07 render/viewer suites (images true) pass at both sample counts where they cover them; Batch 07 volume 38 cases, 290 shots. Known failure: `multi_viewer_b08 x_ite_two_canvases_rigid_bodies_msaa4`, frame 40, `rb-a`: X_ITE pair vs alone 0, Rust pair vs alone 0, Rust vs X_ITE 0.0058 in both pair and alone. So it is a single-canvas rigid-body MSAA 4 difference, not a multi-viewer defect. Not yet investigated.

## 11. Workspace test results

`cargo test --release --workspace --no-fail-fast --locked` on the integration tree:

| Run | Pass | Fail | Ignored | Failing tests |
|---|---|---|---|---|
| 1 (before fixes, before restoring prerequisites) | 957 | 4 | 1 | `items_runtime_matches_x_ite`, `x1_runtime_matches_x_ite` (missing baseline symlinks), `x_ite_two_canvases_rigid_bodies_msaa4`, `rea_batch08_shader_cases_match_x_ite` (stepped baseline missing) |
| 2 (after fixes) | 960 | 1 | 1 | `x_ite_two_canvases_rigid_bodies_msaa4` |

The two corpus failures were missing local prerequisites (symlinks to `X_ITE/scratch/rust-port/oracle`), not defects; after restoring them `corpus` is 6/6. The ignored test is `gen_oracle_worlds`, a world generator, not a gate. Logs: `evidence/b06b08-final/b06b08-test*.log`.

## 12. Oracle comparison results

From the gate run: items2 296/296 exact; items 606/606; x0 47 exact + 3 expected; x1 470 exact + 25 expected + 3 mismatch (0 unexplained) + 2 oracle timeouts; synthetic inline-scripts 18/18; x6 8/8; externproto 12/12; followers 10/10 worlds (649 checks); future-gate 12/12; reload suites pass; b05 sound 65 pass + 1 known; b06 physics 47/47; particles 37/37; b07 ps 103/103. Gate workspace step: 169 ok groups, 1 failing test (`x_ite_two_canvases_rigid_bodies_msaa4`). Batch 08 `local_finish.sh`: only `shaders` was run (read first); `apply`, `gates`, `backup` were not used (own derived runner and backup instead). CodeMirror (`cm5-*`) baselines exist; the cm5 suites were not part of this runner, NOT RUN. Ignored: 1 test, `gen_oracle_worlds` (generator, not a gate).

## 13. Final REA ledger totals

**Authoritative ledger NOT changed** (`xite-rust-full-port-rea-completion/ledger/status.json` stays 214/52/25/33/22 = 346). Proposal, in `LEDGER_PROPOSAL.md` beside this report: Batch 08's 20 selected entries as proposed there (5 DIS COMPLETE, 15 PARTIAL), then the four Codex downgrades (VolumeEmitter, BoundedPhysicsModel, IsoSurfaceVolumeData, SegmentedVolumeData: COMPLETE to PARTIAL). Resulting totals if accepted: COMPLETE 215, PARTIAL 71, DIFFERENT 25, NOT_TESTED 33, MISSING 2 = 346. `sub:physics.engine`, all five shader entries, WindPhysicsModel stay PARTIAL; WebXR and keyboard UI stay MISSING. The 215/71 figures are arithmetic from the two proposals, not re-verified with `ledger_verify.py`.

## 14. Outstanding PARTIAL and DIFFERENT entries

PARTIAL (relevant here): physics.engine (3 picking cases), WindPhysicsModel, VolumeEmitter, BoundedPhysicsModel, IsoSurfaceVolumeData, SegmentedVolumeData, five shader entries (textures-runtime sampler state, `#include`, error/shared/reload/Group-removal/Script cases), InlineGeometry, FontLibrary (CFF), glTF/GLB (extensions), multi-viewer (MSAA 4 single-canvas rigid body), GaussianSplats, VolumeScatterMaterialExtension. The 25 DIFFERENT entries are untouched.

## 15. Final backup and restore evidence

`~/backups/xite-rust-b06-b08/final/` (details in `BACKUP_EVIDENCE.md` there; earlier backups in `../recovery/` untouched). Integration worktree 12,660 files, restored to a disposable directory and compared file by file: 0 differences. `X_ITE/scratch/rust-port/oracle` (the baseline symlink targets, in no earlier backup) 9,924 files: 0 differences. Authoritative ledger dir: identical. Git bundle of the three branches verified. The backup was taken at `fc9587e`; the only later change is this section.

## 16. Owner decisions still needed

1. Accept the ledger proposal (215/71/25/33/2) or hold the four Codex downgrades.
2. Approve a deeper Ammo EPA/GJK trace for the three cylinder picking cases (about a day, per the Batch 08 report), or keep them as a known limit (not approved so far).
3. Approve emulating program-resident GL state (texture units and sampler bindings) so `textures-runtime` can match X_ITE; the clip-plane fix is the first instance.
4. Decide whether the stepped baseline set may stay at 16 of 18 cases.

## 17. Recommendations for Batch 09

1. Close the open gates first: volume Script/ROUTE ordering cases, VolumeEmitter/BoundedPhysics replays, per-style combined lifecycle tests, shader reload/Group-removal/stale-id/`#include`.
2. Model GL program state generally (uniforms, sampler units), not one uniform at a time.
3. Pull the cylinder-in-compound picking trace.
4. Log the adapter in every GPU test.
