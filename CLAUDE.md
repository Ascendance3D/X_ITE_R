# X_ITE_R — Agent Guide

Native Rust port of [X_ITE](https://github.com/create3000/x_ite) (`Ascendance3D/X_ITE_R`).
The lane guide at `../CLAUDE.md` loads alongside this file and carries the lane-wide rules:
read-only archives, the clean-room rule, and no Cybertown/blaxxun hard-coding.

## Layout

- `crates/*`: the Cargo workspace. `x3d-*` is the engine (`x3d-parse` is the classic-encoding
  parser), `vrml97` its public read/write facade, `vrml` the version-detect toolkit and CLI,
  `cm-*` the CodeMirror 5 port, and `x3d-viewer` holds the `x3d-view` and `x3d-drive` binaries.
- `oracle/`: X_ITE reference baselines and the scripts that generate and check them
  (`oracle/tools/`). `oracle/x_ite-pin/` is the active X_ITE reference source (16.5.0, `34cdf9f97`).
- `tools/candidate.lock` + `tools/verify-candidate.sh`: the fail-closed gate that pins the
  X_ITE 16.5.0 and CodeMirror 5.65.21 reference digests. Change a pin only with owner review.
  With no argument it checks the references only (CI runs this). The commit identity check
  needs an explicit `--expect <sha>`. No commit, master included, is an approved candidate
  unless the owner names it. Run `tools/selftest-identity.sh` to test the gate; it works in a
  disposable clone.

## Checks

- Run `cargo fmt --all` before every commit. CI fails an unformatted PR before it builds.
- Before a Git command that can write a tracked path in the main checkout, follow `docs/agents/safe-checkout.md`.
- The full local suite (including the GPU tests CI skips) is the command in the header
  comment of `.github/workflows/rust-tests.yml`.

## Agent skills

### Issue tracker

Issues live in GitHub Issues for `Ascendance3D/X_ITE_R`, via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Default five-role vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one root `GLOSSARY.md` + `docs/adr/`, shared by all workspace crates. See `docs/agents/domain.md`.
