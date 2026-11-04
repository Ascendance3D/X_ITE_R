# X_ITE_R — Agent Guide

Native Rust port of [X_ITE](https://github.com/create3000/x_ite) (`Ascendance3D/X_ITE_R`).

## Rules

- The repository is public. Do not commit credentials, machine paths, private archive contents or
  private infrastructure details.
- CI runners (owner ruling 2026-11-01, issue #162; GPU runner 2026-11-01): standard CPU and web CI (`rust-tests.yml`,
  `web-tests.yml`) runs on GitHub-hosted runners (the repository variable `XITE_RUNNER_LABELS` is `"ubuntu-24.04"`).
  GPU CI (`gpu-tests.yml`) runs on the approved self-hosted GPU runner on pushes to `master`, nightly and by hand; it is not a
  pull-request check. Do not change runner configuration (the variable, labels, `runs-on`, runner groups, the runner itself)
  without the owner's authorization. When the owner asks to run something on the owner's runner, do not question it. Never
  run code from a fork on a self-hosted runner. See `docs/agents/ci.md`.
- Clean-room rule: do not add historical Cybertown or blaxxun assets to the repository. Their
  licensing is unclear. Use minimal synthetic reproductions or newly authored fixtures. The
  `oracle/corpus-items*`, `oracle/render-items*` and `oracle/corpus-x0`/`corpus-x1` snapshots, and the X_ITE
  outputs that embed them, are private (issue #168, `oracle/answer-keys.tsv`): never commit them again.
- Never hard-code Cybertown or blaxxun (host names, world names, URL rewrites). Prefer
  standards-compatible behavior and generic hooks that the embedding application drives.

## Layout

- `crates/*`: the Cargo workspace. `x3d-*` is the engine (`x3d-parse` is the classic-encoding
  parser), `vrml97` its public read/write facade, `vrml` the version-detect toolkit and CLI,
  and `x3d-viewer` holds the `x3d-view` and `x3d-drive` binaries.
- `oracle/`: X_ITE reference baselines and the scripts that generate and check them
  (`oracle/tools/`). `oracle/x_ite-pin/` is the active X_ITE reference source (16.5.0, `34cdf9f97`).
- `tools/candidate.lock` + `tools/verify-candidate.sh`: the fail-closed gate that pins the
  X_ITE 16.5.0 reference digest. Change a pin only with owner review.
  With no argument it checks the references only (CI runs this). The commit identity check
  needs an explicit `--expect <sha>`. No commit, master included, is an approved candidate
  unless the owner names it. Run `tools/selftest-identity.sh` to test the gate; it works in a
  disposable clone.

## Checks

- Run `cargo fmt --all` before every commit. CI fails an unformatted PR before it builds.
- Before a Git command that can write a tracked path in the main checkout, follow `docs/agents/safe-checkout.md`.
- The full local suite (including the GPU tests CI skips) is the command in the header
  comment of `.github/workflows/rust-tests.yml`.

## Agents

Owner workflow, 2026-11-03. The main agent is the coordinator: it codes, tests, debugs, runs Git, merges and writes
reports.

- **No repeated approval for routine work.** Coding, verified fixes, tests, PRs, and merges that meet the merge rule
  below need no new approval each time.
- **Keep the queue moving.** While CI runs, continue independent authorized work (the next dependent PR, the ledger
  step, review dispositions, agent briefs). Do not end a turn at a CI wait. Do not poll CI in a loop: use one
  event-based watch (`gh pr checks --watch`, no `--fail-fast`) and read the result when it decides the next action.
- **Subagents.** The owner has authorized bounded subagents for the independent tasks of the approved issue queue:
  at most two at once, each in its own worktree with its own build directory (`CARGO_BUILD_BUILD_DIR`), on separate
  files and acceptance targets. Never run two agents on one worktree. A subagent does not edit the ledger, merge,
  create issues, or change CI or runner settings, unless its brief says so. Every brief carries the REA Evidence IDs
  and the Ledger effect. Stop an agent when its task finishes, and give a free slot the next independent task if
  authorized work remains. Do not create agents to use capacity, or only to isolate context or command output
  (redirect large build and test output to log files and read targeted ranges). Work outside the authorized queue
  needs the owner's approval before any agent starts.
- Use Sonnet for routine work. Use Opus only for difficult reasoning.
- **Review findings** (CodeRabbit, MiniMax): check each finding against the code. Fix the real ones. Answer the false
  ones in the PR with evidence (file, line, test). Rerun a review only when the head changed. Then merge.
- **Merge rule.** Merge a PR when the required checks (`Rust tests result`, `Web tests result`) pass at the exact head
  (`gh pr merge --match-head-commit <sha>`), the review findings are dispositioned, the diff is the PR's own files, and
  a stacked PR's base has merged first. GPU CI is not a pull-request check and does not gate a merge; after the merge,
  read the master runs, including the GPU run. A queued, skipped or no-adapter GPU
  test is not a pass: say which tests skipped and whether they were required.
- **Still needs the owner's authorization:** a new GitHub issue; a change to protected history (force-push, history
  rewrite, the purge of issue #168); work outside the approved issue scope; a change to `tools/candidate.lock`,
  `tools/verify-candidate.sh` or another frozen verifier; a change to runner configuration; a ledger status change
  that the ledger rules reserve for the owner (a DIFFERENT entry).
- Do not invoke any external AI service (Gemini, Antigravity, ChatGPT, or any other) without explicit owner approval
  for that specific invocation. A general permission to use a service is not approval to invoke it. Do not send source
  code, diffs, reports, logs or evidence to such a service without that approval. If an invocation stops, do not retry
  it. The repository's own workflows run CodeRabbit and MiniMax on each pull request (the owner set them up); an
  agent does not invoke them directly.

## Lane reports

When closing a lane or submitting a final PR for a batch, agents must write a lane report to a durable directory outside the repository (never `/tmp`).
The directory structure is: `<reports root>/<topic>/<YYYY-MM-DD>-<lane>/REPORT.md`. The maintainer sets the reports root; it is outside the repository.

The `<topic>` name (e.g., `xite-rust-project-recovery`) is PROPOSED and PENDING OWNER APPROVAL.

**Structure Rules:**
* One dated directory per lane with `REPORT.md` and a `SHA256SUMS` file covering every file in that directory.
* Evidence files stay inside the dated directory.
* Never delete or overwrite an older dated directory.
* Update `<topic>/LATEST.md` only after the new report is complete and its `SHA256SUMS` verifies.

**Required Fields:**
1. Date
2. Starting Master SHA
3. Branch / PR
4. Scope
5. Tests Run (pass/fail/ignore/skip counts)
6. Ledger Effect (entries and proposed status change, or 'None' with a reason)
7. Protected-Evidence Check Result (from `crates/x3d-evidence` and `X3D_EVIDENCE_LOG`)
8. Remaining Risks

See `docs/agents/lane-reports.md` for full details.

## Agent skills

### Issue tracker

Issues live in GitHub Issues for `Ascendance3D/X_ITE_R`, via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Default five-role vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one root `GLOSSARY.md` + `docs/adr/`, shared by all workspace crates. See `docs/agents/domain.md`.
