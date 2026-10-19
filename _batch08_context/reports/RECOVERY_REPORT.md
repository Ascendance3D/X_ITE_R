# Batch 08: provisional integration report (cloud recovery session)

Lane `XITE_RUST_REA_BATCH_08`, written 2026-10-18. This is **not** the final Batch 08 report. The final
`REPORT.md` is written on the owner's machine after the local gates in section G.

## A. Verdict

**`XITE_RUST_REA_BATCH_08_PARTIAL`** (provisional).

- Not COMPLETE_20: 15 of the 20 entries are proposed PARTIAL on the evidence.
- The GPU gates (MSAA 0/4 on the owner's GPU), the live pinned-X_ITE suites and the final backup/restore must
  run on the local machine.

## B. Source state

| Item | Value |
| --- | --- |
| Repository | `DJAscendance/X_ITE_R` (private; checked before push) |
| Branch | `batch-08-snapshot` (only this branch is pushed; `feature/r0-3-e-retention-cleanup` untouched) |
| Snapshot commit | `a91f0a48a1fd45a03cb6b5a662824e7b5b5b2520` on `03f7a28ae613d7a10fce8c13644f71f2906b231d` |
| Live worktree recorded in the snapshot | HEAD `03f7a28`, branch `feature/r0-3-e-retention-cleanup`, 5,980 status rows (`-uall`; 266 without `-uall` at Batch 08 start) |
| Recovery commit | the commit that adds this file (SHA in the hand-back message) |
| Start backup | `backup/start/SHA256SUMS`: HEAD.txt, status.txt, tracked.patch, untracked.list0 verified OK here; untracked.tgz (376,676,170 B, sha256 `8315a743…5c376`) verified on the owner's machine (same hash in `SHA256SUMS` and `_batch08_context/large_backups.sha256`). Start restore log: `compared=9697 differences=0`. |
| Not in the repo (by decision) | the three backup tarballs over 50 MB; the gitignored oracle baselines (`items*-baseline.json`, `x1-baseline.json`, `render-*items2*-baseline/` and others); `upstream/` (a symlink to a local directory) |

Batch 08 delta, computed against the start (`tracked.patch` applied to `03f7a28` and `untracked.list0`):
22 tracked files changed, 2,065 files new. Files that were untracked at the start and edited in Batch 08 (svg
`dom.rs`, `xml.rs`; net `load_sensor.rs`; physics `bullet.rs`, `physics/bt/broadphase.rs`, `world.rs`) cannot be
diffed here, because `untracked.tgz` is local only.

Recovery commit changes (code):

- `crates/x3d-runtime/src/release.rs`: prune the shader tables with their nodes. Drop a context's FontLibrary
  table with the context.
- `crates/x3d-viewer/src/viewer.rs`: top-level parse through `parse_bytes_with_fetcher` (glTF external buffers).
- `crates/x3d-viewer/tests/rea_b08_shaders_reload.rs`: the invalid-shader check reads draws, not cached objects.
- `crates/x3d-viewer/tests/rea_batch08_shaders.rs`: header, name and missing-baseline message.
- New tests: `crates/x3d-viewer/tests/rea_b08_release_tables.rs` and `rea_b08_gltf_viewer_buffers.rs`.

Recovery commit changes (reports): `shaders/REPORT.md`, this file, `gates/b08-extra.sh`, `tools/ledger_propose.py`,
`tools/ledger_verify.py`, `tools/local_finish.sh`, `ledger/proposed/`, `gates-cloud/` (logs of this session).

## C. The seven lanes

| Lane | Entries | Final review |
| --- | --- | --- |
| physics | sub:physics.engine | Dynamics evidence is complete (109/109 reached Ammo symbols tested, tolerance 0). There are 3 of 20,000 pick mismatches: case 4697 cylinder vs mesh, 5480 cylinder vs cylinder, 11878 cylinder vs height field. The report calls all three "cylinder pairs", which is wrong. In each, Ammo reports no hit and Rust reports a contact (depths 0.13 / 0.87 / 0.44 per the lane report; Rust numbers are not stored in `residual_pick_cases.json`). This is a yes/no flip, so a PickSensor event would differ, not only a float. **PARTIAL.** |
| multiview | sub:browser.multi-viewer-thread | Owner-selected PARTIAL kept. The report's section 1 still says COMPLETE; its sections 5 and 10 record the decision. It does not claim complete browser-thread behavior. Works: per-host script state, per-viewer browser options, independent reload/drop, resources per viewer. Unsupported: viewers on different threads, the process-global geometry provider, global seed hooks, sound, second device. |
| svg/html | sub:parse.svg, sub:parse.html-support | SVG **PARTIAL** (CSS Color 4 functions, 43 hsl ties, 1 gradient pixel). HTML **PARTIAL** pending owner decision 3 (parser contract or DOM layer); COMPLETE if the owner accepts the parser reading. |
| render2 | GaussianSplats, VolumeScatterMaterialExtension | GS **PARTIAL** (shadows, picking, collision, bboxDisplay, culling, depth modes). VS items 1–4, 7 have evidence; item 5 holds only under the clarified reading, which the owner has not accepted → **PARTIAL**, COMPLETE if accepted. |
| net | 5 DIS nodes, InlineGeometry, FontLibrary | DIS ×5 match the pin (no packet IO in X_ITE) → **COMPLETE** if the local GPU gates pass. InlineGeometry **PARTIAL**: with Cache on, X_ITE gives a second node on the same url a `copy()` without routes (`InlineGeometry.js:142-146`, "TODO: add routes"); Rust keeps the routes; untested. The Rust comment that this "is not visible" is wrong for routed geometry. FontLibrary **PARTIAL**: CFF fonts are not loaded (X_ITE uses opentype.js 2); the per-context font table leak was fixed in this session. |
| gltf | sub:parse.gltf2, sub:parse.glb2 | **PARTIAL** both. GLTF2Parser.js names 34 extensions, not the 30 in SELECTION.md. Missing: Draco (skipped), meshopt (stub), BasisU/AVIF/WebP (no decoder), EXT_texture_video (runtime not exercised). The viewer's top-level glTF fetch was fixed in this session. |
| shaders | 5 shader nodes | Recovered, three defects fixed, report written (`shaders/REPORT.md`). All five **PARTIAL**: the stepped X_ITE baseline was never made, and `#include` is unsupported. |

## D. Selected entries (before → proposed)

All 20 were MISSING before. The proposal comes from `tools/ledger_propose.py`. Diff: `ledger/proposed/CHANGES.md`.

| # | Entry | Before | Proposed | If the owner accepts |
| --- | --- | --- | --- | --- |
| 1 | sub:physics.engine | MISSING | PARTIAL | COMPLETE (accept the pick residue) |
| 2 | node:ComposedShader | MISSING | PARTIAL | |
| 3 | node:ShaderPart | MISSING | PARTIAL | |
| 4 | node:ShaderProgram | MISSING | PARTIAL | |
| 5 | node:ProgramShader | MISSING | PARTIAL | |
| 6 | node:PackagedShader | MISSING | PARTIAL | |
| 7 | sub:parse.gltf2 | MISSING | PARTIAL | |
| 8 | sub:parse.glb2 | MISSING | PARTIAL | |
| 9 | node:GaussianSplats | MISSING | PARTIAL | |
| 10 | node:VolumeScatterMaterialExtension | MISSING | PARTIAL | COMPLETE (item-5 reading) |
| 11 | node:DISEntityManager | MISSING | COMPLETE* | |
| 12 | node:DISEntityTypeMapping | MISSING | COMPLETE* | |
| 13 | node:ReceiverPdu | MISSING | COMPLETE* | |
| 14 | node:SignalPdu | MISSING | COMPLETE* | |
| 15 | node:TransmitterPdu | MISSING | COMPLETE* | |
| 16 | node:InlineGeometry | MISSING | PARTIAL | |
| 17 | node:FontLibrary | MISSING | PARTIAL | |
| 18 | sub:parse.svg | MISSING | PARTIAL | |
| 19 | sub:parse.html-support | MISSING | PARTIAL | COMPLETE (parser reading) |
| 20 | sub:browser.multi-viewer-thread | MISSING | PARTIAL | |

\* only if the local GPU gates for `render_rea_b08_net`, `rea_batch08_net` (images) and `rea_b08_net_reload` pass
on the final source.

The three stub shader nodes can move to COMPLETE if their stepped cases and the GPU renders pass locally
(`shaders/REPORT.md` section 10).

## E. Technical implementation

See each lane report (`physics/`, `multiview/`, `svg/`, `render2/`, `net/`, `gltf/`, `shaders/`). This session
added only the fixes listed in B.

## F. Reference evidence

Pinned X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin/PINNED_COMMIT`). No baseline was created or changed in this
session. Results in this container, all against existing X_ITE baselines:

Full workspace with lavapipe visible (`cargo test --release --workspace --no-fail-fast --locked -j 4 -- --nocapture --test-threads=2`,
log `gates-cloud/workspace-lavapipe.log`): exit 101, **945 passed, 15 failed, 1 ignored**.

| Batch 08 GPU test (lavapipe) | Result |
| --- | --- |
| render_rea_b08_shaders | 168 shots (84 worlds × MSAA 0/4) within bound; worst 0.077% |
| render_rea_b08_gltf / gs / vs / net / svg | pass (32 / 70 / 38 / 60 / 50 shots) |
| rea_batch08_gs / vs / net (stepped, images) | pass; net 64 cases incl. UDP loopback control |
| rea_b08_shaders_reload, gs_reload, vs_reload, net_reload, gltf_reload | pass (3 / 2 / 2 / 3 / 1 tests) |
| multi_viewer_b08 | 16 pass, 1 fail: `x_ite_two_canvases_rigid_bodies_msaa4` rb-a f40 0.5787% > 0.5% (same as the lane report; owner decision, PARTIAL) |
| rea_batch08_shaders | fail: baseline missing (NOT RUN) |

Failures on lavapipe that do not fail without a GPU and are **not** Batch 08 tests (cause not proven here;
**must be checked on the owner's GPU**):

- `x3d-render/tests/depth.rs`: `standard_and_log_match_x_ite` (1,480 pixels over 8 on the Standard tilt pair) and
  `log_shows_what_standard_cannot` (`[255,0,0]` vs `[0,255,0]`). Depth precision of the software rasterizer is
  the likely cause.
- B07 volume: `render_rea_b07_vol` (e.g. `bd-default` 4.87%, max 29), `rea_batch07_vol` (228 frame differences),
  `rea_b07_vol_reload` (2). A probe of `rea_b07_volume_survives_reload_and_is_released` (temporary test file,
  deleted) showed the "still drawn after removeChildren" picture is the empty background with 3,617 pixels at
  blue 77 and the rest at 76. That is one level of rasterizer noise, not a volume. The test requires an exactly
  uniform picture. The B07 materials suite (`render_rea_b07_mat`, 290 shots) passes on lavapipe.
- The render2 lane changed shared frame and depth bindings (scatter pass, depth `TEXTURE_BINDING`,
  `FLOAT32_BLENDABLE` on every device). A B07 volume regression from those changes is **not excluded** until the
  local GPU run passes.

The "lavapipe" rows ran on Mesa's software Vulkan. They show that the code draws what X_ITE drew on the owner's
GPU, within the bound. **They are not the GPU gate.**

## G. Tests

### Ran here (final source of the recovery commit, no GPU adapter visible)

| Gate | Command | Exit | Result |
| --- | --- | --- | --- |
| release build | `cargo build --release --workspace --locked -j 4` | 0 | warnings only |
| workspace | `VK_ICD_FILENAMES=/nonexistent __EGL_VENDOR_LIBRARY_FILENAMES=/nonexistent cargo test --release --workspace --no-fail-fast --locked -j 4` | 101 | 951 passed, 9 failed, 1 ignored (961) |
| corpus x0 | `x3d-run compare oracle/x0-baseline.json oracle/corpus-x0 --logs --known …` | 0 | 50 files: 47 exact, 3 expected failures reproduced, logs 50/50 |
| inline scripts | same, synthetic-inline-scripts | 0 | 18/18 exact, logs 18/18 |
| X6 | same, synthetic-x6 | 0 | 8/8 exact, logs 7/8 (0 unexplained) |
| EXTERNPROTO | same, synthetic-externproto | 0 | 12/12 exact, logs 12/12 |
| B05 sound | `python3 -I oracle/tools/rea_b05.py check --target target/release --jobs 3` | 0 | 65 pass, 1 known, 0 fail of 66 |
| B06 physics vs Ammo | `python3 -I oracle/tools/rea_b06.py check --target target/release --jobs 3` | 0 | 47/47 |
| B06 particles | `python3 -I oracle/tools/rea_b06_particles.py check --driver target/release/x3d-particles-drive` | 0 | 37/37 |
| B07 particles | `python3 -I oracle/tools/rea_b07_ps.py check --driver target/release/x3d-particles-drive` | 0 | 103/103 |
| future gate | `bash crates/r0-2-c-tests/future-gate.sh` | 0 | 12 of 12, 0 ignored |

The 9 workspace failures:

| Test | Cause | Class |
| --- | --- | --- |
| `cm-modes` css, htmlmixed, javascript, xml `*_tokens_match_cm5_runmode` (4) | `upstream/codemirror5` missing (`upstream` is a symlink to a local directory) | environment, NOT RUN |
| `winit_event_loop_under_xvfb` | no Xvfb in the container | environment, NOT RUN |
| `items_runtime_matches_x_ite`, `x1_runtime_matches_x_ite` | gitignored baselines not in the repo | environment, NOT RUN |
| `rea_batch08_net_cases_match_x_ite` | without a GPU, `x3d-drive` finishes in 0.037 s, so the UDP negative control sends one datagram per port (it needs 5). With lavapipe the same test passes 64/64, UDP control included. | environment (timing), see F |
| `rea_batch08_shader_cases_match_x_ite` | `oracle/rea-batch08-shaders-baseline` never made | NOT RUN (local step `shaders`) |

The 1 ignored test is `multi_viewer_b08::gen_oracle_worlds`, a world generator added in Batch 08. The earlier
"0 ignored" baseline did not count it.

Baseline comparison: the previous verified workspace was 858 pass, 0 fail, 0 ignored. This run has 961 tests,
103 more than that baseline. I did not list them one by one; they are taken to be the Batch 08 tests. No
failure is in a test that existed before Batch 08 for a code reason. The 7
environment failures above all need files or tools that only the local machine has. GPU tests in this run
returned early ("no adapter"), so their pass is not a pass. The full list is 40 test files
(`gates-cloud/gpu_tests.txt`), and they are covered by the lavapipe run in F and the local run below.

### Must run on the owner's machine (NOT RUN here)

- GPU gate at MSAA 0 and 4 (`x3d-shot batch` / `compare` on `render-items2`; the two known GPU exceptions and
  their pixel counts against the previous baseline).
- Every GPU test on the real adapter (all Batch 01–08 render, reload and stepped image cases).
- Suites that run pinned X_ITE live in headless Chrome: followers (`run_followers_suite.py`, 0/10 here because
  X_ITE's `node_modules` are absent), shader stepped baselines, the fresh 20,000-case pick sweep.
- Comparisons on gitignored baselines: items2, items, x1, render-items2 (MSAA 0/4).
- 06R physics stacks and rates (`.reports/xite-rust-rea-batch-06r-qa-closure`, not in the repo).
- Final backup and real restore comparison.

Exact sequence (on the owner's machine):

```
git -C ~/x_ite_r_stage fetch x_ite_r batch-08-snapshot
git -C ~/x_ite_r_stage show <recovery sha>:_batch08_context/reports/tools/local_finish.sh > ~/b08_local_finish.sh
S=~/b08_local_finish.sh
FINAL=<recovery sha> bash $S verify     # live worktree == snapshot (read only); must print VERIFY OK
FINAL=<recovery sha> bash $S apply      # backup "pre-recovery", then the recovery code diff + report files
bash $S shaders                         # stepped shader baselines from pinned X_ITE (MSAA 0 and 4)
bash $S gates                           # run.sh (+ b08-extra.sh): build, workspace, corpus, GPU 0/4, B01-08 suites
# ledger: apply ledger/proposed/CHANGES.md with set_status.py (full text, notes appended), then:
bash $S ledger [--accept ID ...]        # every unselected entry unchanged, totals 346
bash $S backup                          # final backup, restore into /tmp/b08-restore-final, compare, start backup re-check
```

## H. Performance

- Shader compile and link (CPU, release): 11 `glsl_compat_b08` cases in 0.01 s. Programs are memoized.
- Pipeline creation and frame time on a real GPU: **NOT RUN.**
- Memory and resources (no GPU): `rea_b08_release_tables` (shader and font tables bounded, 0 after unload),
  plus the existing Batch 01–07 retention tests in the workspace run.
- Memory and resources (lavapipe): see F (`rea_b08_shaders_reload` `[0,0,0,0,0]` after unload, and the lane
  reload tests).

## I. Ledger

| | COMPLETE | PARTIAL | DIFFERENT | NOT_TESTED | MISSING | Total | Coverage |
| --- | --- | --- | --- | --- | --- | --- | --- |
| before | 214 | 52 | 25 | 33 | 22 | 346 | 61.85% |
| proposed | 219 | 67 | 25 | 33 | 2 | 346 | 63.29% |
| if the owner accepts physics, VS, HTML | 222 | 64 | 25 | 33 | 2 | 346 | 64.16% |

Only the 20 selected IDs change (`ledger_propose.py` refuses any other change, and `ledger_verify.py` checks it
on the real ledger). No DIFFERENT is assigned. Remaining MISSING: `sub:browser.webxr` and
`sub:browser.keyboard-ui` (both deferred in SELECTION.md). The real ledger was **not** changed here.

## J. Backup and restore

The start backup was checked here: 4 of 5 checksums OK; `untracked.tgz` is local and matches the recorded
hash. The final backup and restore must run locally (`local_finish.sh backup`). No final backup was made in
this container: it would not survive the container and would not be the owner's worktree.

## K. Remaining problems

Defects (fixed in this session):

1. Shader runtime tables not pruned on release.
2. FontLibrary per-context table not dropped with its context (X_ITE: WeakMap).
3. Top-level glTF in Viewer3d/EmbeddedViewer could not fetch external buffers.
4. The shader reload test's invalid-shader check counted cached objects.

Defects (open, outside a small fix):

5. The SVG quality thread-local is not reset for a top-level world and not saved per viewer (multi-viewer leak).
6. InlineGeometry: the cache `copy()` rule (routes not copied) is not reproduced.
7. `x_ite_two_canvases_rigid_bodies_msaa4` fails at 0.579% (single-viewer resting-box edge difference).
8. Physics pick residue: 3 of 20,000 cases.
9. The `rea_batch08_net` UDP negative control depends on the drive taking at least about 5 ms.
10. Cosmetic: stale comments at `x3d-parse/src/lib.rs:138` ("glTF and GLB are not ported") and `x3d.wgsl:2130`; a
    misplaced doc comment at `gpu.rs:2198`; stray probe fixtures `oracle/synthetic-rea-b08-gltf/probe/*`, `probe2/pm.wrl`.

Unsupported behavior: shader `#include`, transform feedback, shadow maps for custom shaders; glTF Draco, meshopt,
BasisU, AVIF, WebP, video; CFF fonts; CSS Color 4 functions; GaussianSplats shadows, picking, collision,
bboxDisplay, culling, depth modes; DIS packet IO (also absent in X_ITE); multi-thread viewers.

Untested: shader stepped cases; every GPU result on the owner's GPU; the 20,000-case pick sweep on the final
source; frame time; InlineGeometry shared url with routes; EXT_texture_video runtime.

Disclosure note: no credentials, keys or tokens found (`git grep` for key patterns). 102 files hold local paths
`/home/ryan/...` (scripts, logs, port-factory state); the repository is private.

## L. Owner decisions

1. Physics: accept 3/20,000 pick residue as a known limit (→ COMPLETE), or order the instrumented Ammo trace.
2. VolumeScatter: accept the contract item 5 reading for attached nodes.
3. HTML support: judge on the parser contract, or require a DOM layer.
4. shaders: confirm the naga `wgsl-out` feature (same crate, no new package) alongside the approved `glsl-in`.
5. InlineGeometry: reproduce X_ITE's "copy without routes" (an upstream TODO), or record a DIFFERENT (needs
   your approval).
6. FontLibrary CFF and glTF Draco/meshopt need new crates (not allowed in Batch 08).
7. Lane-report decisions still open: geometry provider scope, DIS IO, CSS Color 4, the hsl ties, async splat
   sort, Khronos samples.

## M. Batch 09 recommendations (not started)

Based on the proposed ledger (2 MISSING, 33 NOT_TESTED, 67 PARTIAL):

1. Close the Batch 08 PARTIALs that need no new crate: the shader stepped cases and `#include`; the InlineGeometry
   cache rule; the SVG quality per viewer; the multiview MSAA 4 edge.
2. Work down the 33 NOT_TESTED entries: implementation exists, evidence is missing. They give the most
   coverage per unit of work.
3. Owner scope decisions before code: OpenXR (webxr), the keyboard UI oracle, the decoder crates (Draco,
   meshopt, BasisU, CFF).
