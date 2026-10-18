---
name: port-factory
description: Drive the private X_ITE 16.5.0 to native Rust port one REA coverage gap at a time. Use for `/port-factory next|status|record|sync`. Orchestrator loop READ GAP, IMPLEMENT, TEST, RECORD RESULT, NEXT GAP, with the deterministic X_ITE oracle as the only reviewer.
---

# Port Factory

One main Claude agent (you, the sole writer of factory state) and at most two Haiku 5.5 workers for mechanical subtasks (for example drafting a synthetic oracle world or scanning upstream source). No other agents, no reviewers, no approver. The reviewer is the deterministic X_ITE oracle test.

State lives in `.port-factory/` (project root = this worktree, `/home/ryan/Projects/cybertown/.worktrees/rust-port-private/r0-3-e-retention-cleanup`). Never create another worktree. Gap authority is the REA ledger in `/home/ryan/Projects/cybertown/.reports/xite-rust-full-port-rea-completion/` (do not fork it, do not hand-edit `ledger.json` or `status.json`). Change statuses only with its `tools/set_status.py <node|sub> <STATUS> "<names>" [--tests --oracle --missing --notes --rust]` (for ids `node:X` use kind `node` and name `X`; for `sub:a.b` use kind `sub` and name `a.b`). Only COMPLETE counts; PARTIAL is not COMPLETE.

## Commands

All scripts are stdlib python, run as `python3 -I .port-factory/tools/<name>.py`. Never hand-edit `state.json` or `backlog.md`.

- `/port-factory sync`: run `sync.py`. Rebuilds ledger.json, regenerates `backlog.md`, merges `state.json` (history kept).
- `/port-factory status`: read `state.json`; print counts, coverage %, active task, last 5 test results, open blockers, top 5 backlog items.
- `/port-factory next`: run `next.py`. It keeps an already-active task (use `--force` only if the owner says to replace it), otherwise picks the first ready backlog item and marks it active. Then run the task loop below on it.
- `/port-factory record <id> <PASS|FAIL|BLOCKED> <note>`: run `record.py <id> <verdict> "<note>" [--kind focused|integration] [--command "..."] [--evidence PATH]`. `record.py <id> UNBLOCK "<note>"` clears a blocker. PASS moves a task to `completed` only if the ledger now says COMPLETE.

## Task loop

1. READ GAP: read the item in the ledger (`upstream` file under `upstream/`, `missing`, `rust` files) and the upstream X_ITE 16.5.0 source. Do not guess behavior; the oracle decides.
2. IMPLEMENT: edit the Rust crates. Small, focused changes. Add or extend an oracle world (`oracle/synthetic-rea/<world>.wrl` plus `scripts/<world>.json`) when none covers the gap.
3. TEST (focused first, see table). On failure, fix and re-run; no owner approval is needed for ordinary implementation steps. Run integration tests at integration points (end of a component, before declaring a subsystem done).
4. RECORD RESULT: update the ledger via `set_status.py` (COMPLETE only when the oracle matches), then `record.py ... PASS|FAIL|BLOCKED` with evidence paths.
5. NEXT GAP: `next.py`.

## Tests

Focused (after each feature):
- Focused oracle world: `python3 -I oracle/tools/run_rea_suite.py --target /tmp/fp-rel/release --no-images <world>` (X_ITE side is headless Chromium)
- Geometry oracle: `node oracle/x_ite-geometry-rea.mjs oracle/synthetic-rea/geometry-cases.json oracle/synthetic-rea/geometry-baseline.json` then `cargo test -p x3d-render --test geometry_rea`
- Reload: `cargo test -p x3d-script --test extern_reload --test inline_reload`
- Followers: `python3 -I oracle/tools/run_followers_suite.py --target /tmp/fp-rel/release`

Build (needed before oracle runs): `CARGO_TARGET_DIR=/tmp/fp-rel cargo build --release -p x3d-render -p x3d-viewer -p x3d-script --bins -j 16`

Integration (at integration points only):
- Workspace: `cargo test --workspace -j 16`
- Future gate: `bash crates/r0-2-c-tests/future-gate.sh`
- Corpus: `/tmp/fp-rel/release/x3d-run compare oracle/<x>-baseline.json oracle/<dir> --logs --known crates/x3d-script/tests/<x>-known.tsv`
- GPU (never concurrent with any other GPU work): `x3d-shot batch oracle/render-items2 OUT [--msaa 4 --gl-samples]` then `x3d-shot compare oracle/render-items2-baseline OUT`

## Rules

- Context: factory sessions auto-compact at 45% of the context window (`.claude/settings.local.json`, `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE=45`). A skill cannot run `/compact` itself, so keep `state.json` current after every task (active task, last test result, blockers) and write any half-done finding into the task's `record.py` note before starting long test runs, so a compaction loses nothing.

- No commit, merge, push, publish, stash, reset, or checkout. Preserve all uncommitted Rust work (the tree is dirty on purpose). Do not touch upstream X_ITE, CodeMirror, or public WRL Forge.
- One primary Cargo build dir (`/tmp/fp-rel` for release; dev `target/`). No concurrent expensive full builds. No concurrent GPU jobs. Never copy large oracle datasets; reuse existing links to source and test inputs.
- Never weaken, skip, or loosen an oracle test or baseline to get a pass. A blocked or unrunnable oracle is BLOCKED, not PASS. A matching Rust-only unit test is not an oracle pass.
- Headless browsers only (never a visible window). Run independent test suites concurrently on up to 8+ cores except where listed above.
- Record owner decisions verbatim in `state.json` `owner_decisions` (`{date, decision}`) and keep them; do this by editing through a small python one-liner that appends only, never rewriting other fields.
- Haiku workers: at most two at once, mechanical tasks only; you review their output before use.
- American English in code, comments, and docs. Ledger and reports stay plain JSON/Markdown so ChatGPT can read them.
- If stuck after reasonable attempts, `record ... BLOCKED` with the exact missing prerequisite and move to the next gap.
