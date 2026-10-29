# X_ITE_R — Agent Guide

Native Rust port of [X_ITE](https://github.com/create3000/x_ite) (`Ascendance3D/X_ITE_R`).

## Rules

- The repository is public. Do not commit credentials, machine paths, private archive contents or
  private infrastructure details.
- Pull request CI runs on GitHub-hosted runners. Do not move pull request jobs to self-hosted runners.
- Clean-room rule: do not add historical Cybertown or blaxxun assets to the repository. Their
  licensing is unclear. Use minimal synthetic reproductions or newly authored fixtures. The existing
  `oracle/corpus-items*` snapshots wait for an owner licensing decision.
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
