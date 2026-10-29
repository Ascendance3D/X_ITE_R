# CI on pull requests

Three workflows run on a pull request: `rust-tests.yml`, `web-tests.yml` and `code-review.yml`. The required checks are `Rust tests result` and `Web tests result`. They always report.

## Which suites run

`tools/ci_changes.py` reads the changed paths and decides whether the Rust suite and the browser suite are needed. Its docstring lists the rules, and `--self-test` checks them.

- Documentation, repository metadata and the tools that only "Fast checks" runs start no suite.
- A path that the script does not know runs both suites.
- "Fast checks" (reference pins, answer-key tests, the candidate gate, rustfmt, actionlint) runs on every pull request.

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
