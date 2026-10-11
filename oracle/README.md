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

## X1 — X_ITE node table, fields, number formatting, math

All cases here are synthetic, and both scripts run against `x_ite-pin/src` unchanged.

- `x_ite-nodes.mjs` → `x_ite-nodes.json` (2026-10-11). Headless, with the same server
  pattern as `x_ite-dump.mjs`. It records two things:
  - `vrml2Available`: `browser.getConcreteNodes()` after creating a `#VRML V2.0` scene
    (146 classes).
  - For all 263 node types, each type's field definitions, aliases, instance initial
    values and component/profile tables. Each node is recorded as a VRML 2.0 instance and
    as an X3D 4.0 instance.

  Components are loaded one at a time. Texturing3D and VolumeRendering fail with
  "CharLS is not defined" and are listed in `failedComponents`.
  `tools/trim_nodes.py` turns the result into the embedded table
  `crates/x3d-scene/data/x_ite-nodes.json`.
- `x_ite-fields.mjs` → `x_ite-fields.json` (2026-10-11). Runs in plain node with no
  browser. The parser's module graph needs stub `document`/`libtess` globals, and
  X3DScene has to be imported first to avoid a module cycle. It records:
  - FloatFormat/DoubleFormat for 3049 numbers;
  - `fromVRMLString` → `toVRMLString` for 106 cases covering all 42 non-node field types;
  - Vector2/3/4, Rotation4, Quaternion, Matrix3/4 and Color3/4 results;
  - V8 `Math.*` results on 2022 inputs each.

```
node oracle/x_ite-nodes.mjs oracle/x_ite-nodes.json
python3 oracle/tools/trim_nodes.py oracle/x_ite-nodes.json crates/x3d-scene/data/x_ite-nodes.json
node oracle/x_ite-fields.mjs oracle/x_ite-fields.json
```

## Test sets added 2026-10-11

| set | source | files | X_ITE answer key | result |
| --- | --- | --- | --- | --- |
| `corpus-x1/` | 500 more archive files, `tools/pick_corpus.py` (archive snapshots `*-rustport-x1-{before,after}.tsv` identical) | 500 | `x1-baseline.json` (not in git, sha256 `fbe54fa2a0e89f3a…`) | 473 ok, 25 rejected, 2 `TIMEOUT 30s` (timeouts are unreliable tests) |
| `corpus-items/` | **Primary.** Frozen snapshot of Ryan's Mall items, `new-items/item-categories` @ `ef891a64b` + 62 dirty paths, `tools/snapshot_items.py` | 606 `.wrl`, 21 `.js`, 231 `.json` | `items-baseline.json` (not in git, sha256 `8f4bb11593e98f85…`) | 606 of 606 ok |

Both answer keys were made twice in parallel and saved only when byte-identical.
`items-baseline.json` first held 13 extra entries for top-level directory names
(runner filter bug, fixed: `x_ite-dump.mjs` now lists files only); those were removed.

## X5/X6 — event engine and Script node

- `synthetic-x6/` — 8 newly authored scenes (no archive content) plus `js/s08.js`:
  router order and route loops (`s01`), interpolator setup (`s02`), TimeSensor initialize
  in a never-live scene (`s03`), Script field marshalling for every VRML97 field type (`s04`),
  the Browser object (`s05`), Script error reporting (`s06`), Scripts inside PROTOs plus a
  missing EXTERNPROTO and Inline (`s07`), external `.js` with URL fallback (`s08`). Results
  are written into Script fields, which `toVRMLString` prints (`print` goes to
  `console.log`, which `dump.html` does not record).
- `synthetic-x6-baseline.json` — 2026-10-11, `node oracle/x_ite-dump.mjs oracle/synthetic-x6
  oracle/synthetic-x6-baseline.json`: 8 of 8 ok. A second run was byte-identical
  (sha256 `bf0d7d3dee695610…`).

Findings that the Rust side depends on:

- **Math in the dump oracle is not V8's fdlibm.** On 2000 random inputs per function,
  headless Chromium's `Math.sin` matched glibc bit for bit and `cos/tan/acos/asin/atan2/exp/log`
  differed from glibc in 1–7 inputs, while node (V8 fdlibm, which `x_ite-fields.json`
  records) differed from Chromium in 60–1000 inputs. `x3d_fields::jsmath` therefore has a
  `Chrome` flavor (platform libm) that the corpus comparison selects; node-based oracle tests
  keep the default `Node` flavor. Found through a last-digit rotation difference in x1
  `125-shapesofart.wrl`.
- `dump.html` sorts the log, but a message logged between `log .sort ()` and the result's
  serialization can leave it unsorted (x1 `331-couch.wrl`). Comparisons sort both sides.
- Scripts that read `new Date ()` bake the oracle's wall clock into the baseline
  (x1 `244-tardis666`, `265-tardis666`, `87-weaver_anime_final_fixed2`); these cannot match.

## C4 — CodeMirror modes

`cm5-modes-tokens.mjs <xml|javascript|css|htmlmixed|vrml>` writes `cm5-<mode>-baseline.json`
from `runmode.node.js`; Rust tests in `crates/cm-modes/tests/*_oracle.rs` replay them.
`cm5-vrml-mode.js` is the new VRML mode for real CM5 (the Rust `vrml.rs` must match it).
Upstream mode tests are generated by `tools/gen_mode_tests.mjs`.
