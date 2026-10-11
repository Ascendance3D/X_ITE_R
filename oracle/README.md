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

## MSAA oracle (2026-10-11): X_ITE with antialiasing on

Everything above renders with `antialiased="false"` (Multisampling 0) for pixel-exact checks, and
stays that way: `x3d-shot`, `x3d-drive` and `x3d-bench` default to 0 samples. X_ITE's own default
is `antialiased=true`, `Multisampling 4` (clamped to `getMaxSamples ()`, 8 on this GPU).

New files (nothing existing was edited): `x_ite-pin/src/shot-msaa.html` + `x_ite-shot-msaa.mjs`
(like shot.html / x_ite-shot.mjs, `--ms N`, default 4), `x_ite-pin/src/anim-msaa.html` +
`x_ite-anim-msaa.mjs` (like the sized anim page; `--aa` = X_ITE default, `--ms N`), the scripts
`x10-scripts/msaa-orrery-{rotate,zoom12,zoom22}.json` (1024x768, rotate drag, 45 frames of rest,
then 0 / 12 / 22 wheel notches, 60 settle frames), and `synthetic-msaa/` (4 edge scenes: slanted
box, thin sliver, sphere, fine lines).

GL here: ANGLE / NVIDIA GeForce RTX 3060 Ti, Chrome 155, X_ITE 16.5.0 (pin `34cdf9f97`).
`x3d-shot ... --msaa N` / `x3d-drive ... --msaa N` render Rust with N samples; `--gl-samples` makes
the *offscreen* path draw mirrored and flip the readback (see "Why `--gl-samples`").

| set | JS command | answer key | `index.json` sha256 |
| --- | --- | --- | --- |
| render-items 4x | `node oracle/x_ite-shot-msaa.mjs oracle/render-items <dir> --jobs 8` | `render-msaa4-items-baseline/` (18 MB, gitignored) | `8dda1bc4366eeabd…` |
| render-items 8x | `... --ms 8` | `render-msaa8-items-baseline/` (18 MB, gitignored) | `928b554c00807900…` |
| synthetic-x8 4x / 8x | `... oracle/synthetic-x8 <dir> --size 512x512 [--ms 8]` | `render-msaa4-x8-baseline/`, `render-msaa8-x8-baseline/` (2.5 MB each, gitignored) | `b52169d90233f312…`, `9ec6f6ff03819298…` |
| corpus-x0 4x | `... oracle/corpus-x0 <dir>` | `render-msaa4-x0-baseline/` (512 KB, tracked) | `4ad72bdea59ed3c2…` |
| synthetic-msaa 4x / 8x | `... oracle/synthetic-msaa <dir> [--ms 8]` | `render-msaa4-synth-baseline/`, `render-msaa8-synth-baseline/` (tracked, used by `crates/x3d-render/tests/msaa.rs`) | `b9c59a3a078ca2fd…`, `08e0b3398e96443c…` |

Noise floor (render-items captured twice in parallel): **not zero, but 1 level.** 4x: 597 of 606
raw-RGBA hashes identical; the other 9 differ by at most 1 in at most 18 pixels. 8x: the same
(0 pixels over 8 in all 606; max difference 1). So MSAA resolves on ANGLE/GL are deterministic to
+-1 and a tolerance only has to cover real differences between the two implementations.

Rust vs X_ITE on the same GPU (RTX 3060 Ti; ANGLE/GL vs wgpu/Vulkan), threshold 8 / 0.5%:

| comparison | within tolerance | pixels over 8: median / p99 / max |
| --- | --- | --- |
| render-items, 4x, `--gl-samples` | 604/606 (the same 2 files as MSAA off: `art/rainbow-room`, `art/djascendance-live`) | 0.000 / 0.067 / 2.545 % |
| render-items, 8x, `--gl-samples` | 604/606 (same 2) | 0.000 / 0.088 / 2.544 % |
| render-items, 4x, native Vulkan sample positions | 550/606 | 0.089 / 1.517 / 2.546 % |
| render-items, 8x, native Vulkan sample positions | 561/606 | 0.080 / 0.984 / 2.818 % |
| render-items, Rust 0x vs X_ITE 4x (what the viewer was before) | 0.531 / 3.840 / 4.879 % | |
| synthetic-x8, 4x and 8x, `--gl-samples` | 125/125 | 0 / 0 / 0 % (4x: mean absolute difference at most 0.0023 per channel) |
| synthetic-msaa, 4x and 8x, `--gl-samples` | 4/4 **byte-identical** | 0 |
| synthetic-msaa, 4x and 8x, native | 3 to 4 of 4 | one-sample steps on nearly every edge pixel (max diff 64 at 4x) |
| corpus-x0, 4x, `--gl-samples` | 46/47 (43-villasantzip, the known static-shot gravity case) | |
| orrery poses (rotate / zoom12 / zoom22), `--gl-samples` | poses identical to 1e-16; images 0.007 to 0.065 % over at 4x and 8x | |
| orrery poses, native | 0.16 to 0.46 % over | |

**Tolerance, chosen:** the existing one (RGB delta > 8 on at most 0.5% of pixels) when Rust draws
with `--gl-samples`, because then the resolves are byte-identical on synthetic edges and the
only misses are the two files that already miss with MSAA off. For the native (windowed)
orientation use `--max-pct 3`: the failures are single-sample coverage flips on edge pixels (0.25
of a contrast step at 4x), the worst file is 2.8% over 8, and the images are visually the same.

### Why `--gl-samples`

The hardware sample pattern is fixed in framebuffer memory. GL framebuffers are bottom-up and
wgpu/Vulkan ones top-down, so on the same GPU the 4x/8x pattern is mirrored vertically between
the two. Drawing mirrored (clip y negated, front face reversed) and flipping the readback makes
the Vulkan resolve **byte-identical** to the GL one on slanted-edge scenes (synthetic-msaa), and
removes the edge-pixel noise from the 606 items. This is an oracle option of the offscreen path
(`Renderer::set_gl_sample_orientation`); the window path never mirrors.

### Zoom/rotate drift (fixed 2026-10-11)

Symptom: when wheel notches came within about 2 frames of a rotate-drag release, Rust's final zoom
distance ended 0.5% off X_ITE's (pose delta 0.02, orientation identical). `x10-scripts/msaa-orrery-*`
had waited 45 frames to avoid it.

Cause: `ExamineViewer`'s rotation chaser (`set_rotation__`) rewrites `positionOffset` from
`initialPositionOffset` every tick, and so does the zoom position chaser (`set_positionOffset__`),
so whichever runs last in a frame wins. X_ITE runs chasers from `browser.prepareEvents`, whose
interests are a `Map`: `X3DFollowerNode.set_active__` adds the interest when a chaser becomes active
and removes it when it settles (`removeInterest` deletes, `addInterest` appends). The chasers
therefore tick in the order they last became active, not in the viewer's field order. Rust ticked
position, center of rotation, rotation, always. Fix: `Chaser::activated_at` (sequence number of the
last inactive-to-active transition, `chaser.rs`) and a sorted tick in `Examine::prepare` (`nav.rs`);
a chaser activated by this frame's events still waits for the next frame, as before.

Regression scripts (`tools/gen_x10_rotate_zoom.py`, world `render-items/antiques/chronos-orrery`):
`x10-scripts/rotate-then-zoom-{in-r0,in-r1,in-r2,in-r5,out-r0,out-r2}.json` (release at frame 15,
wheel from release+0/1/2/5, zoom in and out) and `rotate-during-zoom-{in,out}-d9.json` (wheel from
frame 9, inside the drag). All eight: pose delta 0.0, images within tolerance. Before the fix r2 was
off by 2.5e-2. Note for script authors: X_ITE's harness moves the mouse to a wheel event's x,y first,
so a wheel inside a drag must sit on the current pointer position, or X_ITE also sees a rotate step.


## Depth fighting (2026-10-11): which triangles fight, X_ITE's LogarithmicDepthBuffer, reversed-Z

Question: "tearing around the rim when dragging" on `render-items/antiques/chronos-orrery`. Answer
(measured, see PLAN.md Status): two sources, both inherent to the item, and X_ITE flickers at the
same pixels as the Rust viewer.

New files (nothing existing edited): `crates/x3d-viewer/src/bin/x3d-zfight.rs` (debug ID-buffer
render, `Renderer::set_debug_ids`, kept out of every default path: draw and triangle index per
pixel, camera jitter, analytic depth of the two competing triangles), `x_ite-zfight.mjs` +
`x_ite-pin/src/zfight.html` (the same jitter sequence in X_ITE, `--log-depth` turns on
`LogarithmicDepthBuffer`), `tools/zfight_compare.py`, `x_ite-shot-log.mjs` + `x_ite-pin/src/shot-log.html`
(shot.html with `logarithmicDepthBuffer="true"`), `synthetic-zfight/` (tilt-pair, far-range) with
baselines `render-zfight-baseline/` (standard) and `render-zfight-log-baseline/` (log), and
`x_ite-geometry-ring.{mjs,json}` (the orrery's closed-spine ring Extrusion against X_ITE:
`crates/x3d-render/tests/geometry_ring.rs`, 300 vertices with caps, 288 without).

```
x3d-zfight <world> oracle/x10-scripts/msaa-orrery-zoom22.json --frames 1,59,142 --out /tmp/zf --n 12 [--depth standard|log|reversed]
x3d-zfight ... --color-dir D --jitters j.json          # shaded PNGs, one per camera jitter
node oracle/x_ite-zfight.mjs <world> <script> --frames 59 --jitters j.json --out D2 [--log-depth]   # one frame per run
python3 -I oracle/tools/zfight_compare.py D D2 --frames 59
```

Use one frame per X_ITE run and only frames where the navigation chaser is idle (59, 142 of the
zoom22 script): a chaser still running rewrites `positionOffset` and swallows the jitter.

`x3d-shot ... --depth log|reversed` and `x3d-view --depth standard|log|reversed` (key `z` cycles)
select the mode. `LogarithmicDepthBuffer` is ported exactly (Logarithmic2.js: vertex
`depth = 1 + gl_Position.w`, fragment `gl_FragDepth = log2 (depth) / log2 (far + 1)`, far 1e10 from
`getMaxFarValue`): render-items 604/606 against `x_ite-shot-log.mjs` (the same 2 files as standard),
`synthetic-zfight` 2/2, and on the orrery pose pair the flicker pixels overlap 99% with X_ITE's
(standard: 64%). Reversed-Z (`Depth32Float`, GreaterEqual, same near/far) is not in X_ITE and is
checked only against its own properties (`crates/x3d-render/tests/depth.rs`).


## corpus-items2 (2026-10-11): the new primary item set

Taken from the live tree `new-items/item-categories` (HEAD `cb1ad41cb`, 63 dirty paths) with
`python3 -I oracle/tools/snapshot_items.py --no-edit oracle/corpus-items2`, then
`snapshot_item_media.py oracle/corpus-items2 oracle/corpus-items2-media`, merged by
`cp -al` into `oracle/render-items2/` (provenance kept as `PROVENANCE-items.json` / `PROVENANCE-media.json`).
`--no-edit` skips `*.edit.wrl` (plain-text editing copies; the real item is the gzipped `.wrl` beside it),
`qa` / `qa-*` folders, and scratch/backup files (`tmp_*`, `tmp-*`, `*.bak*`), besides the old dot-dir /
`node_modules` / `preview` rules. 296 `.wrl`, 185 `.json`, 1 `.js`; 67 media files (6 referenced ones
are missing in the live tree). No kept file references a `.edit.wrl`. The old `corpus-items/` set and
its baselines are unchanged (secondary now). It still contains `work/`, `morph-fails/`, `backups/`,
`old/` and `preserved/` folders; those are items too.

| key | file | `index.json` / file sha256 |
| --- | --- | --- |
| scene text + logs | `items2-baseline.json` (gitignored) | `49632805…` |
| picture, MSAA off | `render-items2-baseline/` (gitignored) | `c1ccb313…` |
| picture, MSAA 4x | `render-msaa4-items2-baseline/` (gitignored) | `ab357137…` |

All three were captured twice (concurrently with 4 other Chrome runs): scene text byte-identical,
rgba hashes identical for 296/296 at both 0 and 4 samples (4x was not +-1 here).

```
node oracle/x_ite-dump.mjs oracle/corpus-items2 oracle/items2-baseline.json
node oracle/x_ite-shot.mjs oracle/render-items2 oracle/render-items2-baseline --jobs 4
node oracle/x_ite-shot-msaa.mjs oracle/render-items2 oracle/render-msaa4-items2-baseline --jobs 4
x3d-run compare oracle/items2-baseline.json oracle/corpus-items2 --known crates/x3d-script/tests/items2-known.tsv --logs
x3d-shot batch oracle/render-items2 out && x3d-shot compare oracle/render-items2-baseline out
x3d-shot batch oracle/render-items2 out4 --msaa 4 --gl-samples && x3d-shot compare oracle/render-msaa4-items2-baseline out4
```

Rust results: scene text 296/296 exact and 296/296 logs; pictures 294/296 at 0x and at 4x
`--gl-samples`; the two misses are `art/rainbow-room` (2.5% over) and `art/djascendance-live`
(1.8%), the same non-power-of-two texture cause as before.

**Known differences, accepted 2026-10-11 (not 296/296 parity).** The 2 misses are kept as known
NVIDIA NPOT mipmap differences. Do not change the mipmap algorithm or the threshold for them.

| item | MSAA off: max / over | MSAA 4x: max / over | GPU-shifted mip levels |
| --- | --- | --- | --- |
| `art/rainbow-room` (`art.jpg` 800×450) | 82 / 2.539% | 82 / 2.545% | 400×225, 25×14, 12×7 |
| `art/djascendance-live` (509×339) | 94 / 1.847% | 94 / 1.842% | 254×169, 127×84, 63×42, 15×10 |

Oracle: NVIDIA GeForce RTX 3060 Ti, driver 595.91.07, Chrome 155.0.8059.39 with
`--use-gl=angle --use-angle=gl` (renderer `ANGLE (NVIDIA Corporation, NVIDIA GeForce RTX 3060
Ti/PCIe/SSE2, OpenGL 4.5.0)`), X_ITE pin `34cdf9f97`, Rust `e0f613e`. Threshold: RGB delta > 8 on
at most 0.5% of pixels. For some even × odd level sizes the driver's `generateMipmap` uses a box
shifted by one texel on the even side; which sizes is closed-driver behavior (2,048 measured
pairs, no formula found). Full record and scripts: `evidence/npot-mipmaps-2026-10-11/README.md`.
