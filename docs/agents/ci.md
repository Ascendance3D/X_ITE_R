# CI on pull requests

Three workflows run on a pull request: `rust-tests.yml`, `web-tests.yml` and `code-review.yml`. The required checks are `Rust tests result` and `Web tests result`. They always report.

## Which suites run

`tools/ci_changes.py` reads the changed paths and decides whether the Rust suite and the browser suite are needed. Its docstring lists the rules, and `--self-test` checks them.

- Documentation, repository metadata and the tools that only "Fast checks" runs start no suite.
- A path that the script does not know runs both suites.
- "Fast checks" (reference pins, answer-key tests, the candidate gate, rustfmt, actionlint) runs on every pull request.

## How the Rust suite runs (issue #185)

CI and the local strict suite run the tests with cargo-nextest (`.config/nextest.toml`), then the doctests with `cargo test --doc`. nextest does not run doctests, so a pass needs both steps.

- **CI** (`rust-tests.yml`, profile `ci`): as many tests at once as the runner has CPUs. The "Runner resources" step prints the CPUs and the memory. The GPU tests are excluded by name (`GPU_TESTS`, 16 names: the batch 09 to 22 GPU tests and the three rea_b24_nav shards), because the CI runners have no GPU. CI does not claim GPU coverage.
- **Local** (the RTX machine): `X3D_ORACLE_KEYS=<evidence store> tools/strict-suite.sh`. It runs 8 tests at once by default (`XITE_R_TEST_JOBS`, at least 6) and 12 compiler jobs (`XITE_R_BUILD_JOBS`). It runs strict and includes the GPU tests. It prints the wall-clock time, the CPU time and the counts of each step, and it exits non-zero when nextest or the doctests fail.
- Tests that share a resource run in a nextest test group, not skipped. Today that is only `rea_batch08_net` (fixed UDP ports 41234–41236).

### Build isolation

`~/Projects/cybertown/.cargo/config.toml` sets `build.build-dir` to a cache that every worktree shares. **`CARGO_TARGET_DIR` does not move `build.build-dir`.** A second build in another worktree can therefore overwrite the files of a running suite: the file hashes do not depend on the worktree path. This caused false doctest and compile failures (issue #185).

- To isolate a build, set both `CARGO_BUILD_BUILD_DIR` and `CARGO_TARGET_DIR`. `tools/strict-suite.sh` does this (`XITE_R_SUITE_DIR`, default `/tmp/xiter-suite-<worktree>`).
- It holds a lock on that directory, so a second `strict-suite.sh` run cannot share it. A cargo command started by hand with the same `CARGO_BUILD_BUILD_DIR` is not locked out. The script stops when such a process is already running at the start, but this scan is advisory only.
- It needs a systemd user session (`systemd-run --user`), `flock` and GNU time (`/usr/bin/time`), and it stops when one is missing. The memory peak comes from cgroup v2; when it is not available, the summary says "memory peak unavailable".
- Check the effective build directory in the `Running … (path)` lines of the log, or in `/proc/<cargo pid>/environ`.
- Do not start another cargo command in the same build directory while a suite runs.

### NVIDIA shader cache

When the NVIDIA shader disk cache is full, a changed shader is compiled again in every test process. `tools/strict-suite.sh` sets `__GL_SHADER_DISK_CACHE_SIZE` for its own run only, so it changes no file and no profile. Do not clear the cache without owner approval.

## Where the jobs run (issue #162)

Owner ruling 2026-11-01: the jobs of `rust-tests.yml` and `web-tests.yml` run on the self-hosted `xite-ci` runners (`gh-runner-ascendance3d-2` and `-3`, labels `self-hosted, Linux, X64`) when the run is trusted, and on GitHub-hosted `ubuntu-24.04` otherwise. `code-review.yml` and its external AI service are unchanged and stay on `ubuntu-latest`.

- **Trusted** is: a pull request whose head repository is this repository, a push to master, and `workflow_dispatch`. A pull request from a fork is not trusted and stays on `ubuntu-24.04`.
- Every job has `runs-on: ${{ fromJSON((github.event_name != 'pull_request' || github.event.pull_request.head.repo.full_name == github.repository) && (vars.XITE_RUNNER_LABELS || '["self-hosted","Linux","X64"]') || '"ubuntu-24.04"') }}`. `github.event.pull_request` is empty for push and dispatch runs, so the event name is tested first.
- **Switch back without a commit:** set the repository variable `XITE_RUNNER_LABELS` to a JSON array, for example `["ubuntu-24.04"]`, to move the trusted runs to GitHub-hosted runners; delete the variable to return to self-hosted. A fork's run ignores the variable. A run that is already queued keeps the labels it had.
- The two runners are one VM (8 vCPU, 13.6 GB RAM, no swap), so jobs of two runs can overlap. Steps that write shared state use `flock` on `/tmp/xite-ci-*.lock`: apt installs (only the missing packages are installed), `playwright install`, `cargo install wasm-bindgen-cli`, and `rea_batch08_net` (fixed UDP ports 41234 to 41236; it runs apart from the rest of the suite). Temporary files use `$RUNNER_TEMP`. The apt `.deb` cache is skipped on a self-hosted runner. A workspace is removed after each job.
- Rust build caches (`Swatinem/rust-cache`) restore into the shared `~/.cargo` of the runner user; the browser shards queue on two runners.

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

The browser suite runs in shards (`crates/x3d-web/tests/browser/shards.mjs`). Issue #180 measured the five-shard layout of that time against one job. Six recent runs took 14–22 minutes of wall-clock time and 40–52 runner minutes. Each shard adds 34–54 seconds of setup to its test step. One job would take 33–44 minutes of wall-clock time (2.1 to 2.4 times as long) and would save 4–6 runner minutes per run (about 11%). The shards stayed.

Issue #187 split the two longest shards by measured times: `cases` into `cases-1` and `cases-2` (whole files, about 439 s of tests each), and the 8 cases of `viewpoints01.test.mjs` into `viewpoints-1` to `viewpoints-3`. A shard with part of a file's cases sets `XR_CASES`, and the harness registers only those cases, so no case shows as skipped. `shards.mjs` refuses a case that is in no shard or in two. The layout is now 8 shards. Run 41002388704 (one run) measured the longest shard at 7:23 and the Web tests run at 9.0 minutes of wall-clock time and 42.8 runner minutes; 3 runs before took 15:01–20:04, 16.7–21.6 minutes and 37.2–51.4 runner minutes. `shards_check.mjs` tests the check.

## GPU runner

`gpu-tests.yml` runs the GPU tests (the `GPU_TESTS` names) on the organization's GPU runner, `gh-runner-ascendance3d-3` (labels `self-hosted, Linux, X64, proxmox, gpu`, runner group `xite-ci`, which allows public repositories since 2026-11-01). It runs on a push to master and by hand, never for a pull request: a pull request could run its code on the GPU host, so the GPU jobs have no pull request trigger. The adapter is strict (`X3D_EVIDENCE_ALLOW` lists only the private keys that are not on the runner). `tools/check_gpu_test_lists.py` (Fast checks) keeps the list equal to `GPU_TESTS` in `rust-tests.yml`. The `browser-video` job runs `movie01.test.mjs` with `CI` unset, so the `movieFrames.*` cases (all skipped on the hosted runner, where their wall-clock sampling flaked) run there; it needs Node 24 and Playwright's Chromium, installed per run without sudo.

### Keeping the GPU runner from failing on simple things

- **Preflight** (`.github/actions/runner-preflight`) runs first in both jobs: GPU and Vulkan ICD, cargo, rustup, Node 24, python3 fail the job at once with the missing item named; clang/lld, the wasm32 target, 30 GiB free and network reach are warnings. The step summary prints CPUs, memory, the job's cgroup `memory.max`, disk and the GPU.
- **Allowed absent keys** come from `oracle/answer-keys.tsv` through `tools/gpu_evidence_allow.py`, not from a hand-kept list: the runner never holds the private keys, so every manifest key may be absent. A missing GPU adapter still fails.
- **Test a change to `gpu-tests.yml` before merging it:** `gh workflow run gpu-tests.yml --ref <branch>` runs the branch's version on the runner (manual dispatch only; the file must already exist on master).
- **Nightly run** (03:17 UTC, master) shows runner drift (driver or package updates) before a merge depends on it.
- The runner's per-job hook wipes the workspace, so every run builds from scratch; the service's memory cap is in its systemd unit on the VM, not in this repository.

