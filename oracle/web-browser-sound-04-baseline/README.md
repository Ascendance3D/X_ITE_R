# web-browser-sound-04-baseline

The level that the page's AudioContext output carries in pinned X_ITE 16.5.0 (`34cdf9f97`), for issue #140.
It is a new, additive reference. `web-browser-sound-01-baseline` and `web-browser-sound-03-baseline` are unchanged.

- Cases: `crates/x3d-web/tests/browser/sound04_case.mjs` (12 cases).
- Record: `node oracle/x_ite-web-runtime-02.mjs <dir> --suite sound04 --runs 8` with `XITE_TESTS` set.
- Statistic: the mean, over about 75 polls of 20 ms in 1500 ms, of the largest root mean square of a 2048-sample analyser window on the tap.
- Runs: 8 per case. All fields except `rms` are identical in every run. `levels` has the raw `rms` of each run, and `rms` in `values` is their mean.
- Spread: the largest range in one case was 0.0029 (a looping clip, level 0.141). Silent cases are exactly 0.
- Tolerance for X_ITE_R: 0.005 (`RMS_TOLERANCE`, `runtime02_harness.mjs`), set from this spread before the first X_ITE_R measurement.
- `audibleMutedBeforeReady` was recorded in a second run of 8 (same machine, same Chrome) and merged into this file.

## Runner reference (`sound04-runner-baseline.json`)

Added for PR #199 with the owner's approval (option 2). The level of the page's output depends on the machine, and pinned X_ITE reads about 3% lower on the CI runner than on the owner's machine.

- Source: pinned X_ITE 16.5.0 (`34cdf9f97`), not X_ITE_R. Same cases, same statistic, same recorder.
- Environment: GitHub-hosted `ubuntu-24.04` runners (the runner type of the browser CI jobs), Chromium 156 on SwiftShader.
- Runs: 15 per case (3 runners, 5 rounds each). `levels` has the raw `rms` of each run; `rms` in `values` is their mean. The largest range in one case was 0.0032 (unmuted again). The three runners agree within 0.001.
- Tolerance: 0.005, unchanged.
- Use: the harness reads this file when `GITHUB_ACTIONS` is `true`, and `sound04-baseline.json` otherwise (`XR_SOUND04_REFERENCE=local|runner` overrides). The local reference is unchanged.
- The sample rate of the runner's AudioContext was not read. A forced 44.1 kHz context on the owner's machine reproduced the lower level (pinned 0.137), which is the likely cause.
