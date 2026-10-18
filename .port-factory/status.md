# Port Factory setup summary (2026-10-16)

Lightweight local factory for the private X_ITE 16.5.0 to Rust port. One main Claude agent, at most two Haiku 5.5 workers, no per-job worktrees, no LLM reviewers. The reviewer is the X_ITE oracle.

## Files

- `.claude/skills/port-factory/SKILL.md`: the skill (`/port-factory next|status|record|sync`).
- `.port-factory/state.json`: self-describing state (see its `schema` field). Plain JSON.
- `.port-factory/backlog.md`: ranked backlog grouped by component, with the ordering rule.
- `.port-factory/dashboard.html`: static dashboard, polls `state.json` every 3 s, no CDN.
- `.port-factory/tools/`: `sync.py`, `next.py`, `record.py`, `common.py` (stdlib; run with `python3 -I`).
- Gap authority (unchanged, not copied): `/home/ryan/Projects/cybertown/.reports/xite-rust-full-port-rea-completion/` (`ledger/status.json` overrides, `ledger/ledger.json` built, `tools/set_status.py`, `tools/build_ledger.py`).

## Start the factory

Start Claude Code in the worktree `/home/ryan/Projects/cybertown/.worktrees/rust-port-private/r0-3-e-retention-cleanup`, then run `/port-factory next`. It selects the first ready gap, marks it active, and runs READ GAP, IMPLEMENT, TEST, RECORD RESULT, NEXT GAP. `/port-factory status` shows progress; `/port-factory sync` re-reads the ledger after the main agent changes statuses or code.

## View the dashboard

```
cd /home/ryan/Projects/cybertown/.worktrees/rust-port-private/r0-3-e-retention-cleanup
python3 -m http.server --bind 127.0.0.1 8765 -d .port-factory
```

Open http://127.0.0.1:8765/dashboard.html (localhost only). Stop with Ctrl-C.

## Resume full-port work

1. `python3 -I .port-factory/tools/sync.py` (also rebuilds the REA ledger).
2. Read `.port-factory/backlog.md` and `state.json` `active`.
3. Build once into the single release dir: `CARGO_TARGET_DIR=/tmp/fp-rel cargo build --release -p x3d-render -p x3d-viewer -p x3d-script --bins -j 16`.
4. `/port-factory next`. Uncommitted Rust work stays uncommitted; commits, merges, pushes are owner-only.

## Task selection

Ready items before blocked (blocked = Sound, Picking, RigidBodyPhysics nodes until their engine subsystem is COMPLETE, or a recorded blocker); then NOT_TESTED, PARTIAL, DIFFERENT, MISSING; then components with fewer open items; then nodes before subsystems; then id.

## Resource rules

No more than two Haiku workers; no concurrent full builds; one primary Cargo build dir (`/tmp/fp-rel`); no concurrent GPU jobs; never copy oracle datasets; headless browsers only.

## Not done

- No Cargo builds or tests were run; no Rust, oracle, `crates/`, `tools/`, or existing files were edited.
- No commits, merges, pushes, or publishing. New files are untracked.
- `sync.py` rebuilds `ledger.json` (the REA lane's own cheap script), so coverage reflects the current worktree scan at that moment (82/346 at setup).
- Dependency blocking is a coarse component-level heuristic; refine `DEPENDS` in `tools/common.py` as needed.
- `record.py PASS` does not itself mark COMPLETE; set the ledger status with `set_status.py` first.

## Context rule (added 2026-10-16)

Factory sessions started in this worktree auto-compact at 45% context: `.claude/settings.local.json` sets `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE=45` (local, not committed; the file is git-ignored). It takes effect for sessions started after the file was written. Check with `/context` or `/status`. A skill or hook cannot trigger `/compact`; only auto-compact or the owner can.
