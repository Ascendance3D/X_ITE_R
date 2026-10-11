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

## X7 — render oracle (pixels)

`x_ite-shot.mjs` + `x_ite-pin/src/shot.html` (new; `dump.html` is untouched). Headless system
Chrome (`--headless=new --use-gl=angle --use-angle=gl --enable-gpu --ignore-gpu-blocklist`),
served by an async `node:http` server with `Cache-Control: no-store`, non-local requests
aborted, a fresh browser context per file. `<x3d-canvas antialiased="false"
preserveDrawingBuffer="true" splashScreen="false" contentScale="1">` at 512×512 (WebGL2,
`antialias: false`, Multisampling 0). `browser.getCurrentTime` is frozen at 946684800;
real time passes (`loadURL`, 10 `nextFrame`, 400 ms, 2 `nextFrame`). Pixels come from
`gl.readPixels` on the default framebuffer, so DOM chrome (splash, notifications, menus)
can never be in the image. Alpha is 255 everywhere; comparisons use RGB.

GL: `ANGLE (NVIDIA Corporation, NVIDIA GeForce RTX 3060 Ti/PCIe/SSE2, OpenGL 4.5.0)`,
vendor `Google Inc. (NVIDIA Corporation)`, WebGL 2.0, Chrome 155.0.8059.39, X_ITE 16.5.0
(pin `34cdf9f97`).

| set | command | result | answer key |
| --- | --- | --- | --- |
| `render-items/` = `cp -al` of `corpus-items/` + `corpus-items-media/` (their `PROVENANCE.json` kept as `PROVENANCE-items.json` / `PROVENANCE-media.json`) | `node oracle/x_ite-shot.mjs oracle/render-items <out>` | 606 of 606 rendered | `render-items-baseline/` (not in git, 16 MB; `index.json` sha256 `1a2f640c2db228f5…`) |
| `corpus-x0/` (secondary) | `node oracle/x_ite-shot.mjs oracle/corpus-x0 <out>` | 47 of 50 (the 3 known IS-mapping rejections); 8 images are a single color | `render-x0-baseline/` (464 KB; `index.json` sha256 `f4af3634db639f4a…`) |

Noise floor: both sets were captured twice in parallel; every rendered file had the same
raw-RGBA sha256 in both runs (606/606, 47/47). The oracle is deterministic: noise floor 0.

`x_ite-geometry.mjs` → `x_ite-geometry.json` (sha256 `497ed1a29aea0def…`): 23 synthetic
geometry nodes (Box, Sphere, Cone, Cylinder, IFS with creaseAngle 0/0.8/1/2, colors,
texCoordIndex, ccw FALSE, a convex pentagon, ILS, PointSet, ElevationGrid, Extrusion) and
the arrays X_ITE uploads after `rebuild ()`. `crates/x3d-render/tests/geometry.rs` matches
all 23 within 1e-6.

Comparison (`x3d-shot compare`): a pixel is "over" when any RGB channel differs by more
than 8; a file is within tolerance when at most 0.5% of its pixels are over. Rationale: the
oracle's own noise is 0, so the tolerance only absorbs differences between two correct
implementations on different APIs — JPEG decoders (Chrome's libjpeg-turbo vs the `image`
crate: up to 4–8 per channel, measured on corpus textures), 8-bit rounding of interpolated
varyings, and coverage of silhouette edge pixels and mip-level choice (Vulkan vs ANGLE/GL
rasterization). With that threshold, 437 of 606 items have no pixel over at all, 598 have
≤ 0.05%, and all passing files have PSNR ≥ 35.0 dB and SSIM ≥ 0.994.

## X8: Text, Fog, Background textures

New files (nothing existing was edited): `tools/gen_synthetic_x8.py` writes `synthetic-x8/`
(125 scenes plus their images: 68 `t-` Text, 23 `f-` Fog, 34 `b-` Background);
`x_ite-text.mjs` -> `x_ite-text.json` (15 MB, gitignored, sha256 `fa34ba46d8e8fb98…`: font
metrics, kerning, glyph outlines and the Text/FontStyle layouts with libtess triangles from
the pinned X_ITE); `x_ite-fog.mjs` -> `x_ite-fog.json` (sha256 `6f61289ad66357b9…`: 40 Fog
node results and 384 samples of the real `Fog2.glsl.js` interpolant evaluated on the GPU).

| set | command | result |
| --- | --- | --- |
| `synthetic-x8/` | `node oracle/x_ite-shot.mjs oracle/synthetic-x8 <out> --size 512x512 --jobs 8` | 125 of 125; captured twice, all PNGs identical (sha256 of the sorted hashes `a451b855…`); only index.json differs, by a blob: URL uuid in one error message |
| answer key | `render-synthetic-x8-baseline/` (2.3 MB, gitignored; `index.json` sha256 `c42b8bf184c60fb1…`) | |

The Rust side: `x3d-shot batch oracle/synthetic-x8 <out> --size 512x512` then
`x3d-shot compare render-synthetic-x8-baseline <out>`: 125 within tolerance, 0 over
(worst file 0.007% over).

## X10 additions (2026-10-11): clipping stress, sized oracle

* `x10-scripts/clip-*.json` (generated by `tools/gen_x10_clip.py`, new files only): EXAMINE
  rotate-large, zoom-in-then-rotate, zoom-out-then-rotate and middle/left drag, each at 512x512,
  at 1024x768 (`-1024`, x3d-view's default window) and at 1024x768 with six mouse moves per frame
  (`-1024-multi`, like a real pointing device).
* `x_ite-anim-sized.mjs` + `x_ite-pin/src/anim-sized.html` (new; `x_ite-anim.mjs` / `anim.html`
  are untouched): the oracle canvas takes the script's `size` (the original is fixed at
  512x512, so a 1024x768 script compared a 1024x768 Rust frame with a 512x512 browser frame).
  `tools/x10_compare.py` picks it when the script's size is not 512x512, and now fails when
  `x3d-shot compare` reports an over-tolerance or size-mismatched image (it used to ignore the
  exit-code-0 "0 within tolerance" line).
* Known: the first ~6 frames of a pan drag with several moves per frame start one frame later
  than X_ITE (end state and all captured frames of the scripts agree); and with the camera inside
  a model (`vehicles/smartcar` zoom-in, frame 80) one huge near triangle shades differently
  (not near-plane clipping: a CPU near-plane clip of the geometry changed 2 of 295,899 pixels).
