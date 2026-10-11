# Oracles — the answer keys for the Rust port

The original JavaScript is the answer key. Rust output must match it.
Everything here is headless and local. Nothing here goes to any remote.

## C0 — CodeMirror 5 test suite

- `cm5-build/` — copy of `upstream/codemirror5` (5.65.21 @ 4ca3d05), built with
  rollup 1 (`npm install --ignore-scripts`; `npx rollup -c`). Not the reference copy.
- `cm5-run.mjs` — replaces upstream `test/run.js` (which needs puppeteer 1.x).
  Uses playwright from `../../../x_ite-tests`.
- `cm5-c0-baseline.json` — 2026-10-11: **1483 tests ran, all passed**.

```
node oracle/cm5-run.mjs [name-prefix] [--json out.json]
```

## X0 — X_ITE parse + re-serialize

- `x_ite-pin/` — `git archive` of `X_ITE/x_ite` at `34cdf9f97` (`src/`, `package.json`).
  `node_modules` is a symlink to the fork's. The fork itself is never touched.
- `x_ite-pin/src/dump.html` — loads one file with `createX3DFromURL`, returns
  `scene.toVRMLString()` plus sorted console errors/warnings. Two things make it
  deterministic:
  - `update="false"` on the canvas: no frame loop.
  - the browser instance's `getCurrentTime` is frozen at 2000-01-01. Looping
    TimeSensors otherwise bake wall-clock time into the dump. Do **not** freeze
    `Date.now` — loading stalls (every file times out).
- `x_ite-dump.mjs` — one fresh browser context per file; all non-local requests aborted.
- `corpus-x0/` — 50 files copied out of the read-only archives by
  `tools/pick_corpus.py` (exact bytes, gzip kept; provenance + sha256 in
  `PROVENANCE.json`). Backup/IVN trees excluded. Archive snapshots
  `recon/manifests/*-rustport-x0-{before,after}.tsv` are identical.
- `x0-baseline.json` — 2026-10-11: **47 of 50 parse**. Two parallel runs are
  byte-identical.

```
node oracle/x_ite-dump.mjs oracle/corpus-x0 out.json
```

The 3 failures are all X_ITE rejecting an IS mapping between fields of different
types (`06-kue.wrl`, `31-av0.wrl`, `42-taharutben1.wrl`). The Rust port must
reproduce that behavior first. Whether X_ITE should accept it is a question for the
X_ITE upstream lane, not this one.
