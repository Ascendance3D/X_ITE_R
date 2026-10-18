# Batch 08 — rules for every lane agent

Read these first:
- `/home/ryan/Projects/cybertown/.reports/xite-rust-rea-batch-08/selection/SELECTION.md` (selection, probes, acceptance criteria)
- `/home/ryan/Projects/cybertown/.reports/xite-rust-rea-batch-08/selection/CONTRACT.md` (COMPLETE contract for nodes)
- `/home/ryan/Projects/cybertown/.reports/xite-rust-rea-batch-07/REPORT.md` and the lane reports beside it
  (`materials/REPORT.md`, `volume/REPORT.md`, `particles/REPORT.md`): they show the oracle tools, test layout and commands.
- `/home/ryan/Projects/cybertown/X_ITE/CLAUDE.md` and `/home/ryan/Projects/cybertown/CLAUDE.md`.

## Place

- Worktree (shared with the other lanes, uncommitted work everywhere):
  `/home/ryan/Projects/cybertown/.worktrees/rust-port-private/r0-3-e-retention-cleanup`
- Reference: pinned X_ITE 16.5.0 at `oracle/x_ite-pin` (read only; `PINNED_COMMIT` = 34cdf9f97).
- Your lane report and logs: `/home/ryan/Projects/cybertown/.reports/xite-rust-rea-batch-08/<lane>/`.

## Git and safety

- Never commit, stage, push, stash, reset, checkout, clean, tag, or rebase. Never revert a file. Other lanes edit the same tree.
- Do not edit or delete existing baselines, existing tests, or other lanes' files. Do not change NURBS code.
- Do not read the archives (Skate.FM, wb-ct-scrape). All fixtures are synthetic, or public samples with recorded provenance.
- Do not change production WRL Forge. Do not start any other batch.
- No new crate dependencies. Exception: the shaders lane adds naga feature `glsl-in` (and `wgsl-in` if needed).
- Never add `pub mod` to `x3d-fields` or `x3d-scene` (the vrml97 api_surface test pins the module list).

## Builds (isolation)

Always set both, with your lane name:
```
export CARGO_TARGET_DIR=/tmp/b08/<lane>/target
export CARGO_BUILD_BUILD_DIR=/tmp/b08/<lane>/build
```
Use `-j 8`. Release builds (`--release`) like the gates. Short timeouts on runs that can hang (about 60 s per world).

## Shared files

- New code goes in new files. Shared files (`nodes/mod.rs`, `lib.rs`, `traverse.rs`, `gpu.rs`, `x3d.wgsl`, `walk.rs`,
  `loader.rs`, parser dispatch) get small registration hunks only.
- Re-read a shared file right before you edit it (another lane may have changed it). Use exact small Edit calls, never
  whole-file rewrites of shared files.
- The tree must compile after each of your edits. If the build breaks in a file you did not touch, wait a minute and
  retry; do not "fix" another lane's code. If it stays broken over 10 minutes, note it in your report.

## Evidence

- Reference results come only from pinned X_ITE runs. Keep them in new baseline dirs (`oracle/<name>-baseline`).
  Never copy Rust output into a baseline.
- Pictures: at most 0.5% of pixels more than 8 levels off, at MSAA 0 and MSAA 4. With `x3d-shot batch --msaa 4`,
  always pass `--gl-samples` (without it, false mismatches).
- Scene text and console log equal where the node has them.
- Test valid and invalid input, field changes during execution (ROUTE and Script), node removal from a live Group,
  a Script-created instance, scene reload, unload with resources back to 0.
- Do not widen a tolerance to make a test pass. If a limit is needed, record the number and the measured reason.
- Do not reproduce an X_ITE crash or hang on purpose. If you find one, stop that case and record it.
- Do not mark anything COMPLETE in the ledger. The lead sets the ledger. You recommend a status per entry.

## Report (your last step)

Write `<lane>/REPORT.md`: recommended status per entry with the contract items each meets, files changed
(new and shared hunks), commands to re-run every test, X_ITE comparison results with numbers, lifecycle and
memory results, known limits, X_ITE quirks reproduced, owner decisions needed. Plain short sentences.
Your final message to the lead: a short summary plus the report path. Use at most one Haiku helper agent.
