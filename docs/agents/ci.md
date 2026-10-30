# CI on pull requests

Three workflows run on a pull request: `rust-tests.yml`, `web-tests.yml` and `code-review.yml`. The required checks are `Rust tests result` and `Web tests result`. They always report.

## Which suites run

`tools/ci_changes.py` reads the changed paths and decides whether the Rust suite and the browser suite are needed. Its docstring lists the rules, and `--self-test` checks them.

- Documentation, repository metadata and the tools that only "Fast checks" runs start no suite.
- A path that the script does not know runs both suites.
- "Fast checks" (reference pins, answer-key tests, the candidate gate, rustfmt, actionlint) runs on every pull request.

## How the Rust suite runs (issue #185)

CI and the local strict suite run the tests with cargo-nextest (`.config/nextest.toml`), then the doctests with `cargo test --doc`. nextest does not run doctests, so a pass needs both steps.

- **CI** (`rust-tests.yml`, profile `ci`): as many tests at once as the runner has CPUs. The "Runner resources" step prints the CPUs and the memory. The GPU tests are excluded by name (`GPU_TESTS`, the same 13 names as before), because the hosted runner has no GPU. CI does not claim GPU coverage.
- **Local** (the RTX machine): `X3D_ORACLE_KEYS=<evidence store> tools/strict-suite.sh`. It runs 8 tests at once by default (`XITE_R_TEST_JOBS`, at least 6) and 12 compiler jobs (`XITE_R_BUILD_JOBS`). It runs strict and includes the GPU tests. It prints the wall-clock time, the CPU time and the counts of each step, and it exits non-zero when nextest or the doctests fail.
- Tests that share a resource run in a nextest test group, not skipped. Today that is only `rea_batch08_net` (fixed UDP ports 41234–41236).

### Build isolation

`~/Projects/cybertown/.cargo/config.toml` sets `build.build-dir` to a cache that every worktree shares. **`CARGO_TARGET_DIR` does not move `build.build-dir`.** A second build in another worktree can therefore overwrite the files of a running suite: the file hashes do not depend on the worktree path. This caused false doctest and compile failures (issue #185).

- To isolate a build, set both `CARGO_BUILD_BUILD_DIR` and `CARGO_TARGET_DIR`. `tools/strict-suite.sh` does this (`XITE_R_SUITE_DIR`, default `/tmp/xiter-suite-<worktree>`).
- It holds a lock on that directory. It refuses to start when another run holds the lock, or when another cargo process builds into the same directory.
- Check the effective build directory in the `Running … (path)` lines of the log, or in `/proc/<cargo pid>/environ`.
- Do not start another cargo command in the same build directory while a suite runs.

### NVIDIA shader cache

When the NVIDIA shader disk cache is full, a changed shader is compiled again in every test process. `tools/strict-suite.sh` sets `__GL_SHADER_DISK_CACHE_SIZE` for its own run only, so it changes no file and no profile. Do not clear the cache without owner approval.

## Draft pull requests

A draft PR runs "Fast checks" and the two result checks only. The result checks report "suite not needed".

To run the suites on a draft, do one of these:

- Mark the PR ready for review. The `ready_for_review` event starts a new run.
- Start a full run on the branch: `gh workflow run "Rust tests" --ref <branch>` and `gh workflow run "Web tests" --ref <branch>`. A `workflow_dispatch` run runs every suite.

## AI review

CodeRabbit is the normal review. It reviews a PR to the default branch. It skips a draft, and it skips a PR to another base (for example a stacked PR).

MiniMax (`code-review.yml`) runs only on demand: add the label `needs-minimax-review` to the PR. Use it when CodeRabbit is rate-limited or skips the PR, or when you want a second review. To review again after a push, remove the label and add it again. Adding a label starts no Rust or browser suite.

To start CodeRabbit by hand, for example after its limit resets, comment `@coderabbitai review` on the PR.

## Browser shards

The browser suite runs in 5 shards (`crates/x3d-web/tests/browser/shards.mjs`). Issue #180 measured this layout against one job. Six recent runs took 14–22 minutes of wall-clock time and 40–52 runner minutes. Each shard adds 34–54 seconds of setup to its test step. One job would take 33–44 minutes of wall-clock time (2.1 to 2.4 times as long) and would save 4–6 runner minutes per run (about 11%). The 5 shards stay.
