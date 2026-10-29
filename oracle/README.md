# Oracles — the answer keys for the Rust port

The original JavaScript is the answer key. Rust output must match it.
Everything here is headless and local. Nothing here goes to any remote.

## Where the gitignored answer keys are

Twelve answer keys are too large for Git: 4 JSON files and 8 render directories. `answer-keys.tsv` lists each
one with its kind, file count, byte count and sha256 (for a directory, the sha256 of its sorted `sha256sum` list).

The keys stay in the main checkout. `tools/answer_keys.py` (Python and shell) and the `x3d-evidence` crate (Rust
tests) find a key in this order: `$X3D_ORACLE_KEYS` (only there, when set), a real file or directory in this
worktree's `oracle/`, then the main worktree's `oracle/`. A key is used only when it equals its row.

`$X3D_ORACLE_KEYS` must be an absolute path to an existing directory outside every worktree of this repository,
and it must hold every key in `answer-keys.tsv`. Otherwise the resolver stops with exit 4 (Rust: `KeyError::Unsafe`) and searches
no other location. There is no default store.

`guard` refuses (exit 4) when real data sits at a key path or `upstream/` that a commit tracks. A checkout,
reset, restore, stash or merge to that commit could delete it. It also refuses such data that `.gitignore`
does not cover, because `git clean -fd` deletes it. Run it before such an operation; today it
refuses the main checkout, whose render keys and `upstream/` are real directories at tracked symlink paths.

```
python3 -I -B tools/answer_keys.py verify                       # every key
python3 -I -B tools/answer_keys.py resolve render-items2-baseline
python3 -I -B tools/answer_keys.py guard . HEAD origin/master   # before a Git operation to origin/master
```

## X0 — X_ITE parse + re-serialize

- `x_ite-pin/` — `git archive` of `X_ITE/x_ite` at `34cdf9f97` (`src/`, `package.json`).
  `node_modules` is a symlink to the fork's. The fork itself is never touched.

### Recording from a worktree

`x_ite-pin/node_modules` is ignored (`.gitignore`), so a new worktree does not have it. Without it the
pages under `x_ite-pin/src/` cannot load their scripts (`inline.html` lines 27-29), and every recorder case
times out. Prepare a worktree under `~/Projects/cybertown/.worktrees/X_ITE_R/<lane>` like this, never in the
main checkout:

```
ln -s ~/Projects/cybertown/X_ITE/x_ite/node_modules oracle/x_ite-pin/node_modules
git check-ignore -v oracle/x_ite-pin/node_modules    # must name .gitignore; never commit the link
cat oracle/x_ite-pin/PINNED_COMMIT                   # must print 34cdf9f97
export XITE_TESTS=~/Projects/cybertown/X_ITE/x_ite-tests   # Playwright for the recorders
```

Then prove the recorder: re-record two existing cases into `/tmp` and compare them with the stored baseline,
for example `node oracle/x_ite-inline.mjs oracle/synthetic-rea-b08-gltf-inline /tmp/sanity.json r-missing-bin
r-bad-base64` against `rea-batch08-gltf-baseline/inline-baseline.json`. Record new cases into a new case
directory and a new baseline file, at least five times; use whole-list console equality only when all
recordings are byte-identical.
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


## PLANE and LOOKAT viewers, OrthoViewpoint (2026-10-11)

Ported (crates/x3d-viewer): `nav/plane.rs` = Browser/Navigation/PlaneViewer.js, `nav/lookat.rs` = LookAtViewer.js (chasers tick
in activation order, like `Examine`), `lookat.rs` = X3DViewer.lookAtPoint / lookAtBBox + X3DViewpointNode.lookAt* +
OrthoViewpoint.lookAt* + Box3 (shared with the Examine double click, which is not wired yet), and OrthoViewpoint in the viewer
(`layer.rs`: fieldOfView extents/offset, projection, max far 1e4, viewAll, transitions, `Vp::set_interpolators`). Look-at
transitions start lazily (TimeSensor.set_start runs in the next frame's first event processing) and use float32 key values like
the MF interpolator fields. Facts from the JS: PLANE pans with the middle button or Alt + left, a plain left drag does nothing;
a LOOKAT/PLANE double click is the look at (PLANE only because `DEVELOPMENT.js` is `true` in `src/`); a wheel on a pan in
progress must sit on the pointer (the harness moves the mouse there first).

Harness (both sides, new event only): `{"type": "dblclick", "x", "y"}` = Playwright `mouse.dblclick` (move, down/up, down/up) on
the oracle, mousemove + 2x (mousedown, mouseup) + `dblclick` in `x3d-drive`. Oracle poses also report `fieldOfViewScale`
(and `fieldOfViewOffset` for an OrthoViewpoint); `x10_compare.py` compares them (same tolerance). The extra pose fields are read
in `x_ite-anim*.mjs` through `canvas.browser`; `anim*.html` in the pin are untouched.

Scripts (512x512, dt 1/30; `tools/gen_x10_plane_lookat.py` writes them; worlds are `x10-scripts/{plane,lookat}-nav.wrl`, an
`#X3D V3.3` `{plane,lookat}-ortho.wrl` with `OrthoViewpoint { fieldOfView [ -5 -5 5 5 ] }`):

| world | scripts |
| --- | --- |
| plane-nav | `plane-drag` (middle drag), `plane-drag-left` (no-op), `plane-drag-alt`, `plane-drag-multi` (6 moves/frame), `plane-wheel`, `plane-wheel-out` (fov-scale clamp), `plane-wheel-in-drag` |
| plane-ortho | `plane-ortho`, `plane-ortho-drag`, `-drag-left`, `-drag-alt`, `-wheel` |
| lookat-nav | `lookat-click` (double click on a shape), `-click-miss`, `-click-alt` (lookAtPoint), `-click-twice`, `-click-interrupt`, `lookat-drag`, `-drag-multi`, `-drag-then-click`, `lookat-wheel`, `-wheel-in-drag` |
| lookat-ortho | `lookat-ortho-click`, `-click-miss`, `-click-alt`, `-click-twice`, `-click-interrupt` |

```
python3 -I oracle/tools/x10_compare.py oracle/x10-scripts/<world>.wrl oracle/x10-scripts/<script>.json /tmp/x10-plane/<script> --target target-viewer/release
```

Result: 27 of 27 PASS (scene text equal, images 0 over). Pose deltas: 0 for 24 scripts (`lookat-click-twice` 1.2e-7); the two
Alt double clicks (`lookat-click-alt` 6.2e-6 center / 3.1e-6 position, `lookat-ortho-click-alt` 1.6e-5 center) differ because
X_ITE reads the hit point from a float32 GPU pointing buffer and Rust ray-casts in f64 (the tolerance stays 1e-4). The same commit's regression run: items2 296/296 exact and 296/296 logs; render-items2 294/296 at 0x and
`--msaa 4 --gl-samples` (the 2 known NPOT files); clip suite 48 cases, 0 unexpected (1 known XFAIL); click, walk, examine,
sensors and the 8 rotate scripts PASS. Not covered by an oracle: the touch handlers of both viewers (no touch events in the
harness), `x3d-view`'s double-click detection, static `x3d-shot` renders of OrthoViewpoint worlds (the static camera pass
still takes only `Viewpoint`).

### EXAMINE double click (2026-10-11)

`x10-scripts/examine-dblclick{,-miss,-alt,-during-spin,-during-chaser,-then-drag,-drag-interrupt}.json` on the synthetic world
`examine-nav.wrl` (NavigationInfo EXAMINE; generated by `tools/gen_x10_plane_lookat.py`). ExamineViewer.dblclick = `disconnect ()`
(position / centerOfRotation / rotation chasers and the spin) then lookAtPoint (Alt) or lookAtBBox. All 7 PASS; the Alt case differs by
6.2e-6 (center) / 3.1e-6 (position) from X_ITE's float32 pointing buffer, the others by at most 1e-9. The Examine touch handlers have no oracle.

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
x3d-run compare "$(python3 -I -B tools/answer_keys.py resolve items2-baseline.json)" oracle/corpus-items2 --known crates/x3d-script/tests/items2-known.tsv --logs
x3d-shot batch oracle/render-items2 out && x3d-shot compare "$(python3 -I -B tools/answer_keys.py resolve render-items2-baseline)" out
x3d-shot batch oracle/render-items2 out4 --msaa 4 --gl-samples && x3d-shot compare "$(python3 -I -B tools/answer_keys.py resolve render-msaa4-items2-baseline)" out4
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


## Followers (2026-10-11): Chaser/Damper nodes, Chrome's Math and number formatting

Ported: `crates/x3d-runtime/src/nodes/followers.rs` (Components/Followers/*, Browser/Followers/X3DArray*Template.js).
`Kind` = getVector/equals/interpolate/step/assign per node type (scalar, Vector2/3, HSV color, Rotation4, arrays of
vectors); `Chaser<K>` / `Damper<K>` = X3DChaserNode / X3DDamperNode. Floats: SFFloat, SFVec3f, SFRotation ... are JS doubles
(no rounding); only MFVec3f/MFVec2f `value_changed` (Float32Array) is rounded to float32. An MF output sends no event when it
is empty and was empty (the typed array setters add none). `X3DDamperNode.set_order__` is unreachable (initializeOnly) and
throws in JS (`length` is block scoped), so it is not ported. The viewer's own `chaser.rs` is untouched.

Test set `oracle/synthetic-followers/` (new, clean-room; `python3 -I oracle/tools/gen_followers.py` writes it; JSON
run scripts in `scripts/`): `chasers` (15 chasers, durations 0.05-2.5, 320 frames), `dampers` (24 dampers, order 0..5,
tau 0..1 and negative, tolerance), `arrays` (Coordinate/TexCoord2D, lengths 0..7, set_value with another length),
`initial` (initialValue != initialDestination and equal), `order` (two followers writing one Transform, chains, damper
into chaser: the prepareEvents Map order), `edge` (tolerance 0, tau < 0, order -2/9, duration 0/-1/0.01, NaN),
`dampers30`, `frames-24`, `frames-quarter`, `frames-coarse` (Damper at other frame rates; Chaser buffer steps larger than one,
`seconds >= 60`). Every value_changed goes to Transform/Material/TextureTransform/PointSet Coordinate/TextureCoordinate and to
a tracer Script (event count, running full-precision checksum, the last value with all digits, frames at which isActive
changed). A driver Script counts frames from timestamps and sends set_value / set_destination at fixed frames; TimeSensor
bursts drive interpolators into set_destination.

| check | command | result |
| --- | --- | --- |
| initial state at load (static dump) | `node oracle/x_ite-dump.mjs oracle/synthetic-followers oracle/synthetic-followers-baseline.json` then `x3d-run compare oracle/synthetic-followers-baseline.json oracle/synthetic-followers --logs` | 10/10 exact, 10/10 logs (baseline sha256 `5c42f698671e472e...`, two runs identical) |
| every frame | `python3 -I oracle/tools/run_followers_suite.py --jobs 3` | 10 of 10 worlds: scene text exact at every dumped frame, poses 0, images within tolerance (threshold 8, 0.5%) |

Oracle script keys (new, off by default): `"pinFrameRate": true` pins the SAI property `browser .currentFrameRate` to 1/dt
before the world loads, in `x_ite-anim.mjs` and `x_ite-anim-sized.mjs`, and `x3d-drive` does the same
(`Viewer3d::load_pinned`). Reason: the anim pages stub `advanceTime` and `getCurrentFrameRate`, but `X3DBrowser.currentFrameRate`
is bound to the prototype method and returns the wall-clock rate of the page's early frames (71.4 in one probe, 60 from the
stub): a Damper's `alpha = exp (-1 / currentFrameRate / tau)` would be non-deterministic. `Viewer3d::pinned_frame_rate = None`
(the windowed viewer) uses the measured frame interval like X_ITE.

Follower registration. `X3DFollowerNode.set_live__` adds the node to `browser .prepareEvents` only while
`getLive () && isActive`. A freshly parsed scene is not live (`X3DScene .setLive (false)`); `replaceWorld` makes it live after
the first frame has processed the init events. `Runtime::go_live` (end of `load_world`, and after an Inline's first frame)
registers the active followers in initialization order. Before this, `acts` of a Damper whose first prepare ends the
animation read `0- 0-` instead of the oracle's `0+ 0-` (the isActive event is read when processed).

Two findings in shared code, both found from last-digit rotation axes in these traces (`0 0 1 ...` vs `0 0 0.999999999999999`):

* **Chrome 155 `Math.sin/cos/tan/asin/acos/atan2/exp/log/log10` are correctly rounded.** Probed with 100,000 random inputs per
  function (`Math.*` in headless Chrome, hex bits): equal to the `core-math` crate (CORE-MATH) in 100% and (acos, sin, cos, exp) to a 200-bit mpmath
  reference in 40,000 of 40,000; glibc (Rust `f64::sin`, Python `math`) differs in 65-246 of 100,000 (log10: 4,026), node's
  fdlibm in 3,300-12,100. `jsmath` Chrome flavor now calls `core_math`; the Node flavor is unchanged. Tests:
  `crates/x3d-fields/tests/chrome_math.rs`. (Rotation4 set/slerp/inverse/mult/normalize and the OrientationChaser step then
  match Chrome on 20,000 random inputs bit for bit.)
* **ICU's `Intl.NumberFormat` is not "shortest digits, then round".** For doubles it builds an approximate decimal first
  (`round (n * 10^fracLength)`, `DecimalQuantity::_setToDoubleFast`) and only converts to the shortest digits when the rounding
  digit lies in the zone of uncertainty (`roundToMagnitude`). `0.9999999999999994` times 1e15 is exactly `...999.5`, so 15
  significant digits print `1`, and 16 digits print `0.999999999999999`. `format.rs` ports the algorithm (ICU `main`
  number_decimalquantity.cpp, HALF_UP, no nickel rounding); it equals Chrome on 62,100 + 250,000 values across exponents and
  precisions 7, 12, 15, 16. Tests: `crates/x3d-fields/tests/chrome_format.rs`.

Regression after the change (same commit): items2 296/296 exact and 296/296 logs; items 606/606; x0 47 exact + 3 known, logs
50/50; x1 470 exact + 3 known + 25 rejected, logs 497/498; synthetic-x6 8/8 (logs 7/8, as before); shots items2 294/296 at 0x
and `--msaa 4 --gl-samples` (the 2 NPOT files); clip suite 48 cases, 0 unexpected (1 known XFAIL); 46 x10 scripts (click, walk,
examine, sensors with `--text-tol 1e-3`, plane-*, lookat-*, examine-dblclick-*, rotate-*) PASS; `cargo build --release
--workspace` and `cargo test --release --workspace` green (409 tests).


## Scripts in Inlines (2026-10-11)

Before: an Inline's scene ran in a separate detached runtime (own queue, own script realm, forked Script host), so
Script nodes inside an Inline did not share time, event order, console order, `Browser` state or routes with the parent.
Now the scene is merged into the parent runtime (`crates/x3d-runtime/src/inline.rs`) as a context of its own.

Test set `oracle/synthetic-inline-scripts/` (clean-room): `basic` (TimeSensor -> Script -> Transform inside the Inline),
`order` (six ticker Scripts, parent/inline/nested print order), `reload` (load FALSE -> TRUE, url a -> b -> empty,
shutdown/initialize order), `nested` (P -> A -> B -> C), `browser` (getCurrentTime, currentScene/getExecutionContext,
createVrmlFromString, addRoute inside an Inline), `output` (directOutput/mustEvaluate), `exportimport` (IMPORT/EXPORT routes
both ways, kept across unload/load). The harness (`x_ite-anim.mjs` keys `console`, `settle`, `dumpInlines`) records the
ordered console and each DEF-named Inline's `toVRMLString ()`; `x10_compare.py` compares both exactly.

| check | command | result |
| --- | --- | --- |
| initial state (static dump) | `node oracle/x_ite-dump.mjs oracle/synthetic-inline-scripts oracle/synthetic-inline-scripts-baseline.json` then `x3d-run compare ... --logs` | 18/18 exact, 18/18 logs (two baseline runs byte-identical) |
| every frame | `python3 -I oracle/tools/x10_compare.py oracle/synthetic-inline-scripts/<w>.wrl oracle/synthetic-inline-scripts/scripts/<w>.json <out> --target target-viewer/release` | 7 of 7: scene text, Inline scene text, console exact; poses 0 |

JS facts reproduced: a scene's Scripts initialize at parse time, before the scene is attached; unloading disposes the
scene's routes but not its nodes (no `shutdown ()`, and a zombie Script keeps its `prepareEvents` interest); a TimeSensor in
an Inline ticks only once the world is live; the world is live after `replaceWorld`'s extra frame.

## Running the X_ITE reference without Node.js: x3d-oracle (2026-10-23)

`crates/x3d-oracle` drives the pinned page in headless Chrome over the DevTools protocol, from Rust.
No Node, npm or Playwright is involved. Details are in its README.

| mode | replaces | equivalence (both tools, same pinned X_ITE, same Chrome 156, same flags) |
| --- | --- | --- |
| `anim` / `anim-msaa` / `anim-sized` | `x_ite-anim.mjs`, `x_ite-anim-msaa.mjs`, `x_ite-anim-sized.mjs` | 805 cases from every `cases.txt` plus REA b05 audio and b06 physics. js.json, `.audio.f32` and exit status are byte-identical, and PNGs pixel-identical, except outputs that depend on wall-clock time in the Node tool as well (below). |
| `particles --variant 06r / events / b07` | `tools/x_ite_particles.mjs`, `_events.mjs`, `_b07.mjs` | 143 / 143 cases (b06 particles, b06 events, b07-ps): result JSON, `.f32`, stdout line and exit status are byte-identical. |

```
cargo build --release -p x3d-oracle
target/release/x3d-oracle anim WORLD SCRIPT --out js.json --png-dir js          # = node oracle/x_ite-anim.mjs …
target/release/x3d-oracle anim-msaa WORLD SCRIPT --ms 4 --out js.json --png-dir js
target/release/x3d-oracle particles WORLD SCRIPT --out js.json [--png-dir png [--ms 4]]
```

`x3d-oracle particles` also reads an additive script key that the Node tools do not have:
`"pickProbes": { "frames": [...], "points": [[x, y], ...] }`. After each listed frame it records `browser .touch`
at every point (id, the DEF name of the hit shape, point, normal, texCoord). Scripts without the key give
byte-identical output, checked on pr-quad of `rea-batch13-particles-baseline`. Pass `--png-dir` with it: without that
the drawing buffer stays 1 x 1 and `touch` hits nothing. The ParticleSystem pointing baselines
`rea-batch14-particle-pick-baseline/` come from `tools/rea_b14_pick.py record`. Their worlds are
`synthetic-rea-b14-particle-pick/` (`tools/gen_rea_b14_particle_pick.py`), and
`crates/x3d-viewer/tests/rea_b14_particle_pick.rs` compares against them.

A second additive key, `"collisionProbes": { "frames": [...], "directions": [[x, y, z], ...], "translations": [[x, y,
z], ...], "nodes": [DEF, ...] }`, records after each listed frame (after its pick probes) the DEF names of the frame's
collision shapes, `getClosestObject` along every direction (id, shape, distance, normal), `constrainTranslation` of
every translation, the user pose, the viewpoint's positionOffset and isActive / collideTime of the named Collision
nodes. The ParticleSystem avatar collision baselines `rea-batch15-particle-collision-baseline/` come from
`tools/rea_b15_collision.py record`. Their worlds are `synthetic-rea-b15-particle-collision/`
(`tools/gen_rea_b15_particle_collision.py`), and `crates/x3d-viewer/tests/rea_b15_particle_collision.rs` compares
against them.

The BoundedPhysicsModel replay baselines `rea-batch16-bounded-baseline/` (every frame's transform feedback buffer)
come from `tools/rea_b16_bounded.py record` (three identical runs). Their worlds are `synthetic-rea-b16-bounded/`
(`tools/gen_rea_b16_bounded.py`), and `crates/x3d-viewer/tests/rea_b16_bounded_physics.rs` replays them through
particles_b06_common with the batch 06R tolerances. A Script there is run by a field event on its input: one started
by a TimeSensor (startTime between two frames) did not run under this protocol, in X_ITE or in Rust.

The wide and stippled instanced line baselines `rea-batch18-particle-lines-baseline/` (ParticleSystem LINE particles
and InstancedShape IndexedLineSets with LineProperties) come from `tools/rea_b18_particle_lines.py record`: three runs
per image pass (`png`, `msaa4`) with identical records and identical decoded pixels. Their worlds are
`synthetic-rea-b18-particle-lines/` (`tools/gen_rea_b18_particle_lines.py`; the InstancedShape worlds hold a disabled
ParticleSystem PS so the particle tools can step them), and `crates/x3d-viewer/tests/rea_b18_particle_lines.rs`
compares them with the rules of rea_b13_particles.rs.

Fields that differ between two Node runs as well, so they differ between tools too:

* `loadFrames`: the real frames the load took.
* The frame in which a run-time URL change resolves when the script has no `settle`
  (anchor-load-*, b05 `clip-url-change`, which `rea_b05.py` already lists as BLOCKED).

Everything else matched byte for byte.

The VolumeEmitter intersection-limit baselines `rea-batch17-emitter-hits-baseline/` (every frame's transform feedback
buffer) come from `tools/rea_b17_emitter_hits.py record` (three identical runs). Their worlds are
`synthetic-rea-b17-emitter-hits/` (`tools/gen_rea_b17_emitter_hits.py`: emission lines through 31 to 128 quads), and
`crates/x3d-viewer/tests/rea_b17_emitter_hits.rs` replays them through particles_b06_common with the batch 06R
tolerances.

The wide LINE particle baselines `rea-batch21-wide-lines-baseline/` (pick probes, every frame's collision probes and
the shadow images of 1 to 24 px lines) come from `tools/rea_b21_wide_lines.py record` (three identical runs). Their
worlds are `synthetic-rea-b21-wide-lines/` (`tools/gen_rea_b21_wide_lines.py`), and
`crates/x3d-viewer/tests/rea_b21_wide_lines.rs` compares them with the B13, B14, B15 and B18 rules.

The two-layer wide LINE particle shadow baselines `rea-batch22-two-layer-shadows-baseline/` (images of two layers side
by side) come from `tools/rea_b22_two_layer_shadows.py record` (three identical runs). Their worlds are
`synthetic-rea-b22-two-layer-shadows/` (`tools/gen_rea_b22_two_layer_shadows.py`), and
`crates/x3d-viewer/tests/rea_b22_two_layer_shadows.rs` compares the records, the whole images (B18 rule) and each layer's
half of them.

When Chrome is needed: only to make or refresh an X_ITE reference (a baseline, an
x10_compare run, a particle reference). The Rust tests compare against committed baselines.
`cargo test --workspace --release`
runs no Node.js, npm or Python: it passed with `node`, `npm`, `npx`, `python`, `python3`
and `perl` replaced on `PATH` by failing shims.

### Migration plan for the remaining Node tools

Keep every `.mjs` until its Rust equivalent passes the same old-against-new comparison.

1. Done: `x_ite-anim*.mjs` and `tools/x_ite_particles*.mjs`. The next step is to switch the
   Python wrappers (`tools/x10_compare.py`, `rea_b0*.py`, `rea_b1*.py`) from
   `node oracle/x_ite-anim*.mjs` to `x3d-oracle anim*`, keeping the same arguments.
2. The same page (anim.html) plus `page.evaluate`: `tools/x_ite_probe.mjs`,
   `x_ite-anim-ammotrace.mjs`, `tools/rea_b08_transform_feedback.mjs`, `x_ite-multi-b08.mjs`.
3. The still-image pages (shot.html, shot-msaa.html, shot-log.html, zfight.html):
   `x_ite-shot*.mjs`, `x_ite-svg-shot*.mjs`, `x_ite-zfight.mjs`, `x_ite-text.mjs`,
   `x_ite-fog.mjs`, `x_ite-geometry*.mjs`.
4. The dump / table pages (dump.html, nodes.html, inline.html, extern.html):
   `x_ite-dump.mjs`, `x_ite-formats/gltf/svg/html/xmljson/optimizer.mjs`,
   `x_ite-nodes*.mjs`, `x_ite-node-types.mjs`, `x_ite-component-constants.mjs`,
   `x_ite-inline.mjs`, `x_ite-extern.mjs`, `x_ite-css-colors.mjs`,
   `x_ite-gl-identity.mjs`, `tools/webaudio_pull*_probe.mjs`.
5. Tools that run JavaScript under plain Node (no browser): `x_ite-fields.mjs`, `tools/ammo_*.mjs`,
   `tools/geom_xite.mjs`, `tools/gen_*.mjs`. These can run in the same Chrome page through
   `x3d-oracle`, or be regenerated once and kept as data.

## Web browser lane 01 (2026-10-26): the `<x3d-canvas>` API case

- `x_ite-web-api.mjs` runs `crates/x3d-web/tests/browser/api_case.mjs` against the pinned X_ITE in headless Chrome
  and records `web-browser-01-baseline/api-baseline.json` (values, world sha256, Chrome and WebGL renderer) and the
  canvas screenshots `before.png` and `after.png` (before and after the field update).
- The page is served from memory at a path inside `x_ite-pin/src/`, so the pinned source and its digest in
  `tools/candidate.lock` do not change. Prepare the worktree as in "Recording from a worktree" above.
- Five runs must give byte-identical values; the run fails otherwise. The baseline was recorded 5 times with
  identical values and identical screenshots (Chrome 156.0.8078.47, RTX 3060 Ti through ANGLE GL).
- `crates/x3d-web/tests/browser/api.test.mjs` runs the same case against X_ITE_R and compares
  (`crates/x3d-web/README.md`).

```
node oracle/x_ite-web-api.mjs oracle/web-browser-01-baseline --runs 5
```

## Web browser lane 02 (2026-10-26): input, loading and scene API cases

- `x_ite-web-runtime-02.mjs` runs the 30 cases of `crates/x3d-web/tests/browser/runtime02_case.mjs` (input, touch,
  coordinates, resource loading, scene API) against the pinned X_ITE in headless Chrome and records
  `web-browser-02-baseline/runtime02-baseline.json` and one PNG per picture.
- The server is the one of the tests (`crates/x3d-web/tests/browser/server.mjs`: held, delayed, failing and garbage
  responses, and a second origin for cross-origin requests). The pinned source is mounted at `/x_ite/`, and the page is
  served from memory, so the pinned source and its digest do not change. Prepare the worktree as in "Recording from a
  worktree" above.
- Five runs must give identical values; the picture hashes of each run are recorded, not compared.
- The first recording had different end positions after the wheel and pinch zoom (4th decimal). Each zoom step is a
  fraction of the *current* distance to the center of rotation, and a step during the PositionChaser's motion (MOVE_TIME
  0.2 s) started from a position that depends on frame timing. The cases now wait 0.4 s after each step; then 18 runs
  gave identical values. No value is rounded beyond the 4 decimals of the other camera values.

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-02-baseline --runs 5
```

## Web resource fallback 01 (2026-10-26): candidates after a response that does not load

- `x_ite-web-runtime-02.mjs --suite fallback01` runs the 10 cases of `crates/x3d-web/tests/browser/fallback01_case.mjs`
  (worlds in `crates/x3d-web/tests/browser/worlds/fallback/`) and records `web-browser-fallback-01-baseline/fallback01-baseline.json`
  and one PNG per picture. The cases cover Inline and ImageTexture candidates that do not parse or decode, glTF files whose
  external buffers fail, a world candidate that does not parse, repeated URLs, two scenes, and loads that a newer
  `loadURL` or `dispose ()` ends while a candidate is held.
- Three runs gave identical values and identical picture hashes (Chrome 156.0.8078.47, RTX 3060 Ti through ANGLE GL).
- In X_ITE a world never finishes loading after a glTF buffer of one of its Inlines failed: GLTF2Parser adds the buffer to
  the loading objects and does not remove it after a failure. So the glTF failure cases record whether `loadURL` settles
  within 5 s, and they have no picture (X_ITE still draws the world before). `gltfLoads` is the control: it settles.

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-fallback-01-baseline --runs 3 --suite fallback01
```

## Web loadURL messages 01 (2026-10-26): the world's candidates, fonts, MovieTexture and audio

- `x_ite-web-runtime-02.mjs --suite loadurl01` runs the 41 cases of `crates/x3d-web/tests/browser/loadurl01_case.mjs` and
  records `web-browser-loadurl-01-baseline/loadurl01-baseline.json` (no pictures). The cases cover the candidates of the
  world-level `loadURL` (messages, their order and attempt numbers; no candidate; the empty URL; a newer `loadURL`,
  `replaceWorld (null)` and `dispose ()` while a candidate is held), FontStyle font files, MovieTexture and AudioClip
  candidates, also with a held response.
- The test server mounts two clean fixture sets for both engines: `/fonts/` (`synthetic-rea-b08-net/fonts`, see its
  `PROVENANCE.txt`) and `/media/` (`synthetic-rea-b05/media`). `tests/browser/worlds/tex/green.gif` (2 × 2) was made
  with Pillow for these tests.
- Three runs gave identical values (Chrome 156.0.8078.47, RTX 3060 Ti through ANGLE GL).
- The cases start a scene with `createX3DFromString` and record whether it loaded, without waiting for it: X_ITE's FontStyle,
  MovieTexture and AudioClip hold the scene's loadCount while a response is held. A scene's MovieTexture needs its
  `COMPONENT Texturing:3` line; without it X_ITE rejects with `CharLS is not defined`.

- PR #123 corrections (2026-10-26): 11 `dataURL` cases were added (32 in total). They load the world from `data:`
  URLs (percent escapes, `;base64`, `;BASE64`, base64 that does not decode, a malformed escape, an empty document) and
  load an image, a font, a sound and a MovieTexture GIF from `;BASE64` data URLs, each with a second candidate that
  must not be requested. The 21 earlier values did not change.
- PR #123 final review (2026-10-27): 4 `dataURL` cases were added (36 in total). They load the world from `DATA:` and
  `Data:` URLs (percent escapes, `;base64`, a document that no parser handles) and an image from a `DATA:` URL. X_ITE's
  match of `data:` is case-sensitive, so `fetch` decodes these URLs, and GoldenGate names no data URL. Three runs gave
  identical values (Chrome 156.0.8078.47). The 32 earlier values did not change.
- PR #123 merge gate (2026-10-27): 5 `dataURL` cases were added (41 in total). They load the world from `data:` URLs
  with UTF-8 escapes: a malformed sequence (`%E0%A4%A`), and an XML world whose WorldInfo title has a literal `é` and
  valid escapes, then the same title with a malformed sequence, an overlong form (`%C0%AF`) and a surrogate
  (`%ED%A0%80`). Where `decodeURIComponent` throws, X_ITE keeps the text as it is. `font.pending` now checks that the
  font request is in flight before its response is released. Three runs gave identical values (Chrome 156.0.8078.47).
  The 36 earlier values did not change.

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-loadurl-01-baseline --runs 3 --suite loadurl01
```

## Web data URL compatibility 01 (2026-10-27): raw tab, LF and CR in data URLs

- `x_ite-web-runtime-02.mjs --suite dataurl01` runs the 31 cases of `crates/x3d-web/tests/browser/dataurl01_case.mjs` and
  records `web-browser-data-url-01-baseline/dataurl01-baseline.json` (no pictures). The cases put raw tab, LF and CR
  characters in and after the scheme of a world's data URL (a WorldInfo title shows whether they were kept), an
  ImageTexture's data URL (whether the next candidate is requested) and a glTF buffer's data URL (the coordinates, or
  the Inline's message for a buffer that fails), with `;<TAB>base64`, base64 that does not decode, a raw `#VRML`
  header and an http control.
- Chrome 156.0.8078.47 keeps a tab or newline after the scheme only when the URL starts with `data:` in any case,
  after any leading tabs and newlines. A leading space or a tab in the scheme makes it remove all of them, as the URL
  standard does. Chromium 151.0.7922.34 (Playwright's) kept them only after an exact lower-case `data:`; the probes are
  in the lane report.
- The glTF worlds and the Inline worlds in `crates/x3d-web/tests/browser/worlds/dataurl/` were written for these tests.
- Five runs gave identical values (Chrome 156.0.8078.47, RTX 3060 Ti through ANGLE GL).

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-data-url-01-baseline --runs 5 --suite dataurl01
```

- `x_ite-web-runtime-02.mjs --suite scripturl01` runs the 44 cases of `crates/x3d-web/tests/browser/scripturl01_case.mjs`
  (issue #127) and records `web-browser-script-url-01-baseline/scripturl01-baseline.json` (no pictures). Each of 11
  prefixes (none, U+0020, U+0009, U+FEFF, U+0085, U+00A0, U+2028, U+3000, U+2003, U+200B, U+180E) goes before
  `ecmascript:` in the url of a Script node, the world's loadURL, an Inline and an EXTERNPROTO, each followed by a
  second candidate that tells the outcomes apart. X_ITE 16.5.0 (`/^\s*(?:ecmascript|javascript|vrmlscript):/s`) takes
  U+0009, U+0020, U+FEFF, U+00A0, U+2028, U+3000 and U+2003 as source text, and requests U+0085, U+200B and U+180E as
  a file (404, then the second candidate). The compared values are `requests` (and, for loadURL, the world and its
  title); the rest is under `detail`, which the harness records but does not compare: X_ITE_R's web build has no
  script engine, and its Inline and EXTERNPROTO take a script URL as no scene.
- Three runs gave identical values (Chrome 156.0.8078.47).

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-script-url-01-baseline --runs 3 --suite scripturl01
```

- `x_ite-web-runtime-02.mjs --suite degenerate01` runs the 9 cases of `crates/x3d-web/tests/browser/degenerate01_case.mjs`
  and records `web-browser-degenerate-01-baseline/degenerate01-baseline.json` and 12 pictures. The cases are
  TriangleSet worlds with zero-area triangles (two or three equal points, several of them, finding F1 of PR #124) and
  controls (a valid triangle, collinear points, a very thin triangle, a valid triangle next to a degenerate one), and
  the failing world loaded three times on one page before the valid one. Each case keeps the loadURL result, the
  browser events, the TriangleSet's number of points, the load messages and the picture.
- X_ITE's MikkTSpace wasm traps when every triangle has two equal points (`memory access out of bounds` or
  `unreachable`); X3DGeometryNode.generateTangents catches it and logs it only when DEVELOPMENT is true (the pinned
  source; the dist build sets it false). The cases leave those "RuntimeError" lines out of the messages;
  `degenerate01.test.mjs` checks separately that X_ITE_R logs none.
- Five runs gave identical values (Chrome 156.0.8078.47, RTX 3060 Ti through ANGLE GL).

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-degenerate-01-baseline --runs 5 --suite degenerate01
```

- `x_ite-web-runtime-02.mjs --suite gltfworld01` runs the 16 cases of `crates/x3d-web/tests/browser/gltfworld01_case.mjs`
  and records `web-browser-gltf-world-01-baseline/gltfworld01-baseline.json` and 5 pictures. The cases are glTF and
  GLB files loaded as the world with loadURL (finding F2 of PR #124) and, for comparison, through an Inline: an
  external buffer, a data URL buffer, a missing buffer, base64 that does not decode, two buffers (one late, one
  missing), a buffer 600 ms late, a failed glTF candidate before one that loads, a failed world before a valid one,
  and a world whose held buffer is replaced by a newer loadURL. Each case keeps the loadURL result, the world, the
  browser events, the coordinates, the requests and the load messages. The test worlds are synthetic
  (`crates/x3d-web/tests/browser/worlds/gltfworld/`).
- GLTF2Parser adds each buffer to the loading objects and does not remove one that failed. For an Inline, and for a
  world candidate after one whose buffer failed, X_ITE's loadURL then never settles; X_ITE_R's does
  (`runtime02_harness.mjs`, the same known difference as `fallback.gltfFailed`).
- Five runs gave identical values (Chrome 156.0.8078.47, RTX 3060 Ti through ANGLE GL).

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-gltf-world-01-baseline --runs 5 --suite gltfworld01
```

- `x_ite-web-runtime-02.mjs --suite gltfworld02` runs the 6 cases of `crates/x3d-web/tests/browser/gltfworld02_case.mjs`
  and records `web-browser-gltf-world-02-baseline/gltfworld02-baseline.json` (the CodeRabbit finding of PR #126). Each
  case loads a glTF world with two buffers, one of which fails while the test server holds the other: the first or the
  second fails, both fail in either order, a later candidate follows, and a newer loadURL replaces the world while both
  are held. A case keeps the state while the buffer is held (`during`) and after it was released. The holds make the
  order independent of the machine's speed.
- X_ITE rejects the candidate at the first failure (`Promise .all`), before the held buffer comes: loadURL fails, or
  the next candidate is requested at once. The held buffer's FileLoader goes on and logs its own failure later.
- Five runs gave identical values (Chrome 156.0.8078.47, RTX 3060 Ti through ANGLE GL).

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-gltf-world-02-baseline --runs 5 --suite gltfworld02
```

### Web browser field API 01 (`web-browser-fields-01-baseline/`)

- `x_ite-web-runtime-02.mjs --suite fields01` runs the 7 cases of `crates/x3d-web/tests/browser/fields01_case.mjs`
  (issue #137) and records `web-browser-fields-01-baseline/fields01-baseline.json`: the field classes of the X3D
  namespace and X3DConstants, the field classes as values, live SF and MF field objects of nodes (one object per field,
  changed components and elements reach the node and its routes, routed values reach a held object), assignments of
  every kind of field through the SFNode proxy, `addFieldCallback` (the callbacks of each event, in event order, with
  the event's value; `removeFieldCallback`), `getFieldDefinitions`, and `rootNodes` as an MFNode. The test world is
  synthetic and inline (a `data:` URL).
- The world needs `COMPONENT Scripting:1` for its Script node: X_ITE rejects a Script under `PROFILE Interchange`
  without it.
- Five runs gave identical values (Chrome 156.0.8078.47, RTX 3060 Ti through ANGLE GL).

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-fields-01-baseline --runs 5 --suite fields01
```

### Web browser scene text 01 (`web-browser-scene-text-01-baseline/`)

- `x_ite-web-runtime-02.mjs --suite scenetext01` runs the 17 cases of `crates/x3d-web/tests/browser/scenetext01_case.mjs`
  (issue #136) and records `web-browser-scene-text-01-baseline/scenetext01-baseline.json` (no pictures): the exact
  `toVRMLString`, `toXMLString` and `toJSONString` of the scene `createX3DFromString` returns and of `currentScene`
  after `loadURL` (a `data:` URL) of 16 synthetic worlds (empty, Box, DEF/USE, PROTO, EXTERNPROTO, ROUTEs, META and
  WorldInfo with metadata, EXPORT, PROFILE and COMPONENT lines, UNIT lines, a Script, an Inline, non-ASCII text, a
  `#VRML V2.0` world and an XML world), and of the empty world of a canvas before any load (and after
  `replaceWorld (null)`). The test server's origin is replaced by `http://test`.
- Pinned X_ITE 16.5.0: the header is `#X3D V4.1 utf8 X_ITE V16.5.0`; the empty world has no PROFILE line in its VRML
  text and `Full` in XML and JSON. The pinned page cannot load a world whose profile is `Full` (or has no PROFILE
  statement): it throws `ReferenceError: CharLS is not defined`, so no case uses them.
- Three runs gave identical values (Chrome 156.0.8078.47). Prepare the worktree as in "Recording from a worktree".

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-scene-text-01-baseline --runs 3 --suite scenetext01
```

### Web browser field API 02 (`web-browser-fields-02-baseline/`)

- `x_ite-web-runtime-02.mjs --suite fields02` runs the cases of `crates/x3d-web/tests/browser/fields02_case.mjs`
  (issue #137) and records `web-browser-fields-02-baseline/fields02-baseline.json`: `toString`, `toVRMLString`,
  `toXMLString` and `toJSONString` of an SFNode, an MFNode (a field and `rootNodes`), NULL and an empty MFNode, with
  DEF and USE; and `fromString`, `fromVRMLString` and `fromXMLString` of every kind of field, their error texts, the
  nodes of a value read with a scene and without one, and a value read into a node's field. The test world is
  synthetic and inline (a `data:` URL). `fields01-baseline.json` is not changed.
- PR #150 QA added the case `fieldText.noScene` (3 cases now): SFNode and MFNode values read with no scene and the
  console lines printed first. Pinned X_ITE 16.5.0: the XML parser sets NULL or `[ ]` for any text and never needs a
  scene; the VRML parser throws a `TypeError` at the first node statement; text with no node statement throws
  `Couldn't read value for field ''.`; a node value that fails prints no warning, but a typed array such as `MFVec3f` `[1 2`
  warns first and then throws `Expected ']'.`. The cases of the earlier recording are unchanged.
- Five runs gave identical values (Chrome 156.0.8078.47). Prepare the worktree as in "Recording from a worktree".

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-fields-02-baseline --runs 5 --suite fields02
```

### Web browser routes 01 (`web-browser-routes-01-baseline/`)

- `x_ite-web-runtime-02.mjs --suite routes01` runs the 9 cases of `crates/x3d-web/tests/browser/routes01_case.mjs`
  (issue #135, part 1: routes only) and records `web-browser-routes-01-baseline/routes01-baseline.json` (no pictures):
  `X3DScene .addRoute`, `deleteRoute` (route and four arguments), `getRoute`, `routes` (`RouteArray`) and the `X3DRoute`
  object; events over an added route against a parsed `ROUTE` (same frames, same order); the exact errors of bad nodes,
  bad field names, type mismatches and access types (`addRoute` says `source node must be ...`, `getRoute` says
  `sourceNode must be ...`; unknown fields throw, nothing is logged); `X3DBrowser .addRoute` / `deleteRoute`, which
  delegate to `currentScene` and return nothing (so `browser .deleteRoute (route)` throws the `getRoute` error); and routes
  between two scenes (`routesApi.crossScene`: X_ITE does not look at the scene of a node, takes the route and carries events
  between the scenes). The test world is synthetic and inline (a `data:` URL).
- Three runs gave identical values (Chrome 156.0.8078.47).

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-routes-01-baseline --runs 3 --suite routes01
```

### Web browser scene creation 01 (`web-browser-scene-create-01-baseline/`)

- `x_ite-web-runtime-02.mjs --suite scenecreate01` runs the 23 cases of `crates/x3d-web/tests/browser/scenecreate01_case.mjs`
  (issue #134) and records `web-browser-scene-create-01-baseline/scenecreate01-baseline.json` (no pictures):
  `createScene` (a Promise of an empty scene that is not the world, `worldURL` is the page's; every argument takes the
  profile branch and rejects with `Couldn't create scene: profile must be of type ProfileInfo.`; with `getProfile` and
  `getComponent` the profile and components); `createVrmlFromString` (an `MFNode`, `#VRML V2.0 utf8` in front, the parser
  warnings and errors and the thrown `Error`, nodes that go into a world field); `createX3DFromURL (url)` (fallback chain,
  fragment kept in `worldURL`, data and base64 URLs, EXTERNPROTO, the failure messages and `Couldn't load X3D file.`, the
  argument errors thrown at once, two or four arguments are the promise form); `createX3DFromURL (url, node, event)` and
  `createVrmlFromURL` (the argument errors and the field set after the load); `importJS` (a string or an object; a value that
  is no X3D JSON gives an empty scene); `importDocument` (string, `XMLDocument`, XML element, HTML element; invalid input gives
  an empty scene, `undefined` a TypeError). The recorder page loads the Texturing3D libraries (CharLS, dicom-parser, jpeg-js,
  OpenJPEG) as upstream's `x_ite.html` does: `createScene` loads every component of the Full profile.
- Six cases were added to the first 17 (2026-10-29), whose recorded values did not change: `profileInfo`,
  `componentInfo` and `supportedLists` (`getProfile`, `getComponent`, `supportedProfiles`, `supportedComponents`, the
  `ProfileInfo` / `ComponentInfo` objects and their arrays, their errors and aliases; the `providerURL` of an external
  component is compared by its file only), `createSceneInfo` (`createScene (profile, ...components)` keeps the objects
  given), `sceneProfile` (the `profile` and `components` of parsed scenes: classic VRML, VRML97, XML, JSON, a loaded
  world) and `vrmlURLNodes` (the callback form delivers usable nodes of the world: an EXTERNPROTO scene, a fallback
  list, `set_children`, every candidate failing).
- Five runs gave identical values (Chrome 156.0.8078.47). Prepare the worktree as in "Recording from a worktree".

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-scene-create-01-baseline --runs 5 --suite scenecreate01
```

### Web browser viewpoints 01 (`web-browser-viewpoints-01-baseline/`)

- `x_ite-web-runtime-02.mjs --suite viewpoints01` runs the 8 cases of `crates/x3d-web/tests/browser/viewpoints01_case.mjs`
  (issue #135, part 2) and records `web-browser-viewpoints-01-baseline/viewpoints01-baseline.json` (no pictures): the
  `layerNode` argument of `bindViewpoint`, `firstViewpoint`, `previousViewpoint`, `nextViewpoint`, `lastViewpoint`,
  `changeViewpoint` and `viewAll` on a world with the root layer (four viewpoints, one without description, an
  `OrthoViewpoint`, a `ViewpointGroup`) and a `LayerSet` of three layers (the second has no viewpoint); the exact errors of
  `bindViewpoint` (`Browser.bindViewpoint: layerNode must be of type X3DLayerNode.`, `... viewpointNode must be of type
  X3DViewpointNode.`; extra arguments are ignored); a value that is no layer node means the active layer; the layer of
  `bindViewpoint` is only type checked; `browser .getUserViewpoints` is `undefined`; `endUpdate` / `beginUpdate` /
  `setLive` / `isLive` against a running TimeSensor, a route and a field assignment (the TimeSensor stops, `isActive`
  stays `true`, assignments and routes and viewpoint binding still work), and the live state over a `loadURL`.
  The test world is synthetic and inline (a `data:` URL).
- Three runs gave identical values (Chrome 156.0.8078.47). Positions of a transition are not recorded: they depend on timing.

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-viewpoints-01-baseline --runs 3 --suite viewpoints01
```

## Web browser options 01 (2026-10-28): options, properties, rendering properties, print

- `x_ite-web-runtime-02.mjs --suite options01` runs the 9 cases of `crates/x3d-web/tests/browser/options01_case.mjs`
  against the pinned X_ITE and records `web-browser-options-01-baseline/options01-baseline.json` (no pictures):
  the default of all 35 options, each set to a valid value, the coercion of `SFBool`, `SFInt32`, `SFFloat`,
  `SFDouble` and `SFString` values, the unknown-name errors of the four getters and the setter, the eight
  properties, the rendering properties (also after the options are set and X_ITE has run its frames), and the
  console messages of `print` and `println`.
- The page sets `splashScreen`, `contextMenu` and `notifications` to false and X_ITE reads attributes into the
  options, so these three recorded defaults are false. The rendering properties `Shading`, `Antialiased`,
  `ContentScale` and `Multisampling` are derived values (`POINTSET` gives `POINT`, `Antialiased` stays true after the
  option is set false, `Multisampling` is clamped to the context's samples); the WebGL context values
  (`MaxTextureSize` ...) are those of the recording GPU (RTX 3060 Ti through ANGLE).
- Five runs gave identical values (Chrome 156.0.8078.47). Prepare the worktree as in "Recording from a worktree".

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-options-01-baseline --runs 5 --suite options01
```

## Web browser attributes 01 (2026-10-28): canvas attributes, contentScale, maximumFrameRate, update

- `x_ite-web-runtime-02.mjs --suite attributes01` runs the 10 cases of `crates/x3d-web/tests/browser/attributes01_case.mjs`
  against the pinned X_ITE and records `web-browser-attributes-01-baseline/attributes01-baseline.json` (no pictures):
  the option after each attribute value at start, on change and when removed (19 attributes), the options that leave
  the attributes alone, the framebuffer size of `contentScale` (attribute and `ContentScale` option, at device pixel
  ratio 1 and 2, `auto`, invalid values, a resized element), `MaximumFrameRate` (attribute and option), and the `update`
  attribute with the `AutoUpdate` option (hidden and shown element, a `loadURL` and a new element while stopped).
- Frames are observed as `time` events of a looping TimeSensor. The recording keeps ceilings (`ticks <= rate * seconds + 1`)
  and "none" or "some", never counts, so it holds on a slow machine. A stopped window must give none.
- X_ITE uses a `ContentScale` of 0, a negative or a NaN value as it is, which gives a 1 x 1 framebuffer; `update="auto"` on a
  stopped browser leaves it stopped; `baseURL` sets `browser.baseURL` (X_ITE_R does not, see `web/API.md`).
- Five runs gave identical values. Prepare the worktree as in "Recording from a worktree".

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-attributes-01-baseline --runs 5 --suite attributes01
```

## Web browser entry 01 (2026-10-28): X3D entry points and captureStream

- `x_ite-web-runtime-02.mjs --suite entry01` runs the 12 cases of `crates/x3d-web/tests/browser/entry01_case.mjs`
  against the pinned X_ITE and records `web-browser-entry-01-baseline/entry01-baseline.json` (no pictures):
  `typeof X3D`, its length and members; `X3D ()` with and without callbacks (the throwing `onfulfilled` is the only
  `onrejected` path a page can reach); `X3D.noConflict ()` with `window.X3D` absent, an object, or `undefined` before
  the library loads (an init script sets it); `X3D.createBrowser ()` with no argument, an MFString URL, a bad URL
  and other arguments; `X3D.getBrowser` with every kind of argument; and `captureStream` of the page's canvas, of an
  element that is not attached, and of `document.createElement ("x3d-canvas")`.
- The unbundled source of the pin never assigns `window.X3D` (its dist bundle does), so the noConflict cases assign it
  themselves, as the bundle would. A bad or non-MFString URL given to `createBrowser` is an unhandled rejection; the
  cases record it from the page's `unhandledrejection` event. The pinned canvas fires no `initialized` or `load` for
  its first, empty world (X_ITE_R does; `KNOWN_DIFFERENCES` in `runtime02_harness.mjs`).
- The recorded frame rate of `captureStream ()` is 60 (Chrome 156.0.8078.47, headless). Five runs gave identical
  values. Prepare the worktree as in "Recording from a worktree".

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-entry-01-baseline --runs 5 --suite entry01
```
