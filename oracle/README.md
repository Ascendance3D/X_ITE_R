# Oracles — the answer keys for the Rust port

The original JavaScript is the answer key. Rust output must match it.
Everything here is headless and local. Nothing here goes to any remote.

## Where the gitignored answer keys are

Twelve answer keys are too large for Git: 4 JSON files and 8 render directories. Twelve private paths
(issue #168, below) use the same manifest. `answer-keys.tsv` lists each
one with its kind, file count, byte count and sha256 (for a directory, the sha256 of its sorted `sha256sum` list).

The keys are in one external evidence store, outside the repository: a private, dated directory
`<backups root>/X_ITE_R/<date>-evidence-store/` that holds every row of `answer-keys.tsv` as real files and
directories, with `README.md` and `SHA256SUMS`. The maintainer sets the backups root. Point `X3D_ORACLE_KEYS` at
the store. The main checkout does not hold the 8 render keys (its render-key paths are empty and untracked). It holds copies of the 4 JSON keys and the 12 private paths below as
ignored files and directories. `docs/agents/safe-checkout.md` describes the layout and the rules for a Git operation in the main
checkout. Do not move a key back to a key path. To give a command an `oracle/<name>` path, link the key:
`X3D_ORACLE_KEYS=<store> python3 -I -B tools/answer_keys.py link . <name>...`. A link is for reading only; remove
it (`rm -- oracle/<name>`, no trailing slash) before a Git operation.

### Regenerate a key

The store and the key paths are reference evidence. Tests read them; nothing writes them. A regenerated key is a
candidate in a staging directory, not a key.

1. Make a new, empty staging directory outside every worktree and the store: `mkdir -- <staging>` (no `-p`).
2. Check each output path: `X3D_ORACLE_KEYS=<store> python3 -I -B tools/answer_keys.py check-output <staging>/<name>`.
   It refuses (exit 4) the store, a key path `oracle/<name>` of any registered worktree (`git worktree list`; the
   names of this manifest and of that worktree's one) and the target of a link at a key path, and any path below
   or above one of them. It also refuses a path at or below a directory that holds every key (even one that is not
   a store), but not a path above it: without `X3D_ORACLE_KEYS`, the parent of a store passes. Set the variable.
   A separate clone is not protected. The check is for accidental writes, not a security boundary: a process that
   puts a symlink at a checked path after the check and before the write is not stopped (an accepted residual
   risk, issue #65). It checks the path as given and
   with every symlink resolved. `x_ite-shot.mjs`, `x_ite-shot-msaa.mjs`, `x_ite-dump.mjs` and `x_ite-text.mjs` run
   this check before they write, and write nothing when it fails. `x_ite-text.mjs` needs an output argument: its
   default, `oracle/x_ite-text.json`, is a key path.
3. Record into `<staging>/<name>`, and compare the candidate with the key.
4. Promotion is a separate change that the owner approves: a new dated store with its own `SHA256SUMS`, and a
   reviewed change of the row in `answer-keys.tsv`. Never write a candidate into the store or to a key path.

`tools/answer_keys.py` (Python and shell) and the `x3d-evidence` crate (Rust tests) find a key in this order:
`$X3D_ORACLE_KEYS` (only there, when set), a real file or directory in this worktree's `oracle/`,
then the main worktree's `oracle/`. A key is used only when it equals its row.

`$X3D_ORACLE_KEYS` must be an absolute path to an existing directory outside every worktree of this repository,
and it must hold every key in `answer-keys.tsv`. Otherwise the resolver stops with exit 4 (Rust: `KeyError::Unsafe`) and searches
no other location. The resolver has no default store, so set the variable for every strict run in the main
checkout.

`guard` refuses (exit 4) when real data sits at a key path or `upstream/` that a commit tracks. A checkout,
reset, restore, stash or merge to that commit could delete it. It also refuses such data that `.gitignore`
does not cover, because `git clean -fd` deletes it. Run it before such an operation. It passes in the main
checkout with the external store, because no real data is at a key path.

### Private corpora (issue #168)

`corpus-items`, `corpus-items2`, `corpus-items-media`, `corpus-items2-media`, `render-items` and `render-items2`
are historical Mall item files with no redistribution permission. They were removed from the public repository at
`3841786a`. `answer-keys.tsv` lists all six, and the resolver finds and verifies them like the keys. Removal from
`master` does not erase them from older commits.

The tests that read them fail by name when they are absent, and skip by name when `X3D_EVIDENCE_ALLOW` lists
them. The other sections of this file still name these paths; they are now found through the resolver. The
external store holds them; the main checkout's `oracle/` holds ignored copies.

`corpus-x0` and `corpus-x1` are Cybertown files copied out of the read-only archives (see their `PROVENANCE.json`),
with no redistribution permission. They, and the X_ITE outputs that embed or render them (`x0-baseline.json`,
`x0-xml-baseline.json`, `render-x0-baseline`, `render-msaa4-x0-baseline`), are private rows of `answer-keys.tsv`
too. `x0-xml-baseline.json` was `synthetic-rea-xml/x0-xml-baseline.json`; it is now `oracle/x0-xml-baseline.json`
(same bytes), because a manifest name is one path component.

```
X3D_ORACLE_KEYS=<store> python3 -I -B tools/answer_keys.py verify   # every key, in the store only
env -u X3D_ORACLE_KEYS python3 -I -B tools/answer_keys.py verify # without the store; fails on a key that no checkout holds
python3 -I -B tools/answer_keys.py resolve render-items2-baseline
python3 -I -B tools/answer_keys.py guard . HEAD origin/master   # before a Git operation to origin/master
```

## X0 — X_ITE parse + re-serialize

- `x_ite-pin/` — `git archive` of `X_ITE/x_ite` at `34cdf9f97` (`src/`, `package.json`).
  `node_modules` is a symlink to the fork's. The fork itself is never touched.

### Recording from a worktree

`x_ite-pin/node_modules` is ignored (`.gitignore`), so a new worktree does not have it. Without it the
pages under `x_ite-pin/src/` cannot load their scripts (`inline.html` lines 27-29), and every recorder case
times out (the `x_ite-anim*.mjs` drivers now stop at once with a message naming the missing link, instead of ending after a 30 s wait with an empty log). Prepare a worktree under `~/Projects/cybertown/.worktrees/X_ITE_R/<lane>` like this, never in the
main checkout:

```
ln -s ~/Projects/cybertown/X_ITE/x_ite/node_modules oracle/x_ite-pin/node_modules
git check-ignore -v oracle/x_ite-pin/node_modules    # must name .gitignore; never commit the link
cat oracle/x_ite-pin/PINNED_COMMIT                   # must print 34cdf9f97
export XITE_TESTS=~/Projects/cybertown/X_ITE/x_ite-tests   # Playwright for the recorders
```

Then prove the recorder: re-record two existing cases into `/tmp` and compare them with the stored baseline,
for example `node oracle/x_ite-inline.mjs oracle/synthetic-rea-b08-gltf-inline /tmp/sanity.json r-missing-bin
r-bad-base64` against `rea-b08-gltf-baseline/inline-baseline.json`. Record new cases into a new case
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
node oracle/x_ite-dump.mjs oracle/corpus-items2 <staging>/items2-baseline.json          # a candidate; see "Regenerate a key"
node oracle/x_ite-shot.mjs oracle/render-items2 <staging>/render-items2-baseline --jobs 4
node oracle/x_ite-shot-msaa.mjs oracle/render-items2 <staging>/render-msaa4-items2-baseline --jobs 4
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
byte-identical output, checked on pr-quad of `rea-b13-particles-baseline`. Pass `--png-dir` with it: without that
the drawing buffer stays 1 x 1 and `touch` hits nothing. The ParticleSystem pointing baselines
`rea-b14-particle-pick-baseline/` come from `tools/rea_b14_pick.py record`. Their worlds are
`synthetic-rea-b14-particle-pick/` (`tools/gen_rea_b14_particle_pick.py`), and
`crates/x3d-viewer/tests/rea_b14_particle_pick.rs` compares against them.

A second additive key, `"collisionProbes": { "frames": [...], "directions": [[x, y, z], ...], "translations": [[x, y,
z], ...], "nodes": [DEF, ...] }`, records after each listed frame (after its pick probes) the DEF names of the frame's
collision shapes, `getClosestObject` along every direction (id, shape, distance, normal), `constrainTranslation` of
every translation, the user pose, the viewpoint's positionOffset and isActive / collideTime of the named Collision
nodes. The ParticleSystem avatar collision baselines `rea-b15-particle-collision-baseline/` come from
`tools/rea_b15_collision.py record`. Their worlds are `synthetic-rea-b15-particle-collision/`
(`tools/gen_rea_b15_particle_collision.py`), and `crates/x3d-viewer/tests/rea_b15_particle_collision.rs` compares
against them.

The BoundedPhysicsModel replay baselines `rea-b16-bounded-baseline/` (every frame's transform feedback buffer)
come from `tools/rea_b16_bounded.py record` (three identical runs). Their worlds are `synthetic-rea-b16-bounded/`
(`tools/gen_rea_b16_bounded.py`), and `crates/x3d-viewer/tests/rea_b16_bounded_physics.rs` replays them through
particles_b06_common with the batch 06R tolerances. `bp-rebuild` and `bp-replace` run their Script by a field event.
The `bp-ts-*` and `bp-tick-*` cases run it from a TimeSensor, at a time the Script chooses (a TimeSensor `isActive`,
or a looping TimeSensor's `time` that the Script tests). Measured on the pinned page: X_ITE 16.5.0 schedules a start
timeout only for a `startTime` event, not for the value read from the file, so a future `startTime` in the file never
starts the sensor (`bp-ts-file-rebuild`; this, not the oracle protocol, is why the first attempt saw no Script run).
The `bp-ts-*` cases set `startTime` by an event at frame 1 and use the page's virtual timers; such a timer fires in the
first step at or after its `startTime` (`bp-ts-hit` 22.9 frames, `bp-ts-miss` 23.1 frames). The records of these cases
are flat: `rea-b16-bounded-baseline/NAME.js.json` and `NAME/js.f32`.

The wide and stippled instanced line baselines `rea-b18-particle-lines-baseline/` (ParticleSystem LINE particles
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

The VolumeEmitter intersection-limit baselines `rea-b17-emitter-hits-baseline/` (every frame's transform feedback
buffer) come from `tools/rea_b17_emitter_hits.py record` (three identical runs). Their worlds are
`synthetic-rea-b17-emitter-hits/` (`tools/gen_rea_b17_emitter_hits.py`: emission lines through 31 to 128 quads), and
`crates/x3d-viewer/tests/rea_b17_emitter_hits.rs` replays them through particles_b06_common with the batch 06R
tolerances.

Issue #93 added cases to the B13 and B17 sets. B17 `ve-speed-*`: the same quad stacks as `ve-hits-*` with direction,
speed 2 and variation 0.3 (the particles move), and `ve-speed-64-bounded` (hits past 32 in the emitter and in a
BoundedPhysicsModel at once). B13 `pr-script-created-node` and `pr-script-created-geometry` (a root Script makes the
ParticleSystem with `createNode` and names it PS with `updateNamedNode`; `createX3DFromString` cannot do this because
`updateNamedNode` refuses a node of another scene), `pr-inline` and `pr-inline-transform` (the ParticleSystem is in an
Inline child file under `inline/`; the world's PS, the node the tools step, is a disabled stub as in B18's InstancedShape
worlds, so only the images and the per-frame seeds carry those cases), and `pr-geometry-texture`, `pr-geometry-texramp`,
`pr-geometry-texramp-rgb` (a texCoord ramp on a GEOMETRY system: X_ITE samples the zero texel of the ramp for every
vertex, `texCoordCount` being 0). They come from `tools/rea_b13_particles.py record` (a copy of the B18 recorder: three
identical runs per image pass) and `tools/rea_b17_emitter_hits.py record`. A BVH deeper than the GLSL stack of 32 cannot be
built (the median split makes N triangles `ceil (log2 N)` deep); `crates/x3d-runtime/tests/particles_bvh_depth.rs`
pins the Rust depth.

The wide LINE particle baselines `rea-b21-wide-lines-baseline/` (pick probes, every frame's collision probes and
the shadow images of 1 to 24 px lines) come from `tools/rea_b21_wide_lines.py record` (three identical runs). Their
worlds are `synthetic-rea-b21-wide-lines/` (`tools/gen_rea_b21_wide_lines.py`), and
`crates/x3d-viewer/tests/rea_b21_wide_lines.rs` compares them with the B13, B14, B15 and B18 rules.

The two-layer wide LINE particle shadow baselines `rea-b22-two-layer-shadows-baseline/` (images of two layers side
by side) come from `tools/rea_b22_two_layer_shadows.py record` (three identical runs). Their worlds are
`synthetic-rea-b22-two-layer-shadows/` (`tools/gen_rea_b22_two_layer_shadows.py`), and
`crates/x3d-viewer/tests/rea_b22_two_layer_shadows.rs` compares the records, the whole images (B18 rule) and each layer's
half of them.

The ParticleSystem shadow-culling baselines `rea-b25-particle-shadow-cull-baseline/` (MSAA 0 and 4 images of a SpotLight, a
PointLight and a DirectionalLight over a floor, with a system whose bbox is outside the light's view volume) come from
`tools/rea_b25_particle_shadow_cull.py record` (three identical runs per pass; the record of a case is `NAME.js.json` beside the `NAME/` directory, so the Review gate's one-level `oracle/*-baseline/*.json` exclusion covers it). Their worlds are
`synthetic-rea-b25-particle-shadow-cull/` (`tools/gen_rea_b25_particle_shadow_cull.py`), and
`crates/x3d-viewer/tests/rea_b25_particle_shadow_cull.rs` compares the records and at most 8 pixels per image. The records
of a system inside an Inline (`pr-inline-records*` in `synthetic-rea-b13-particles/`) are dumped by naming it with a path in
the script's `node`, `"INLINE/NAME"` (DEF names; `x3d-oracle particles` and `x3d-particles-drive` descend through
Inline `.getInternalScene ()`); the texCoord-ramp-on-POINT/LINE cases are `pr-point-texramp*` and `pr-line-texramp`.

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
- Issue #144 (2026-11-01): `font.pending` and `audio.decodeFallbackHeld` were recorded again, 5 runs each, identical values
  and equal to the committed ones (Chrome 156.0.8078.47). X_ITE_R's scene now holds its load for both (the two
  `KNOWN_DIFFERENCES` entries are gone), so the baseline file is unchanged.

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-loadurl-01-baseline --runs 3 --suite loadurl01
```

## Web MovieTexture 01 (2026-10-31): frames and video fallback (issue #139)

- `x_ite-web-runtime-02.mjs --suite movie01` runs the 13 cases of `crates/x3d-web/tests/browser/movie01_case.mjs`
  (media in `crates/x3d-web/tests/browser/worlds/movie/`, made by `oracle/tools/gen_movie_web.py` and ffmpeg) and records
  `web-browser-movie-01-baseline/movie01-baseline.json` (no pictures). `movieFrames.*` start the node from the page's clock
  and read the color at the middle of the canvas 0.5 s into each of media frames 0 to 4 (a sample counts only when the clock
  readings before and after the capture are 0.5 s inside its frame, else it is taken one loop later): a GIF, an APNG, a WebM and an MP4 of four
  four-second frames (red, green, blue, yellow), a data URL, `loop FALSE` and `speed 0.5`. `movieFallback.*` are the
  candidates after a video that is no video, a missing one and a GIF that is no GIF, with the requests and the messages.
- Three runs gave identical values (Chrome 156.0.8078.47). The browser runs with `--autoplay-policy=no-user-gesture-required`.
  All `movieFrames.*` cases are skipped when `CI` is set: they sample a wall clock, and the hosted GPU-less runner's video
  decode (webm, mp4) and load (gif, apng flaked on unrelated pull requests) break the sampling window. They run locally and in
  gpu-tests.yml (`browser-video`, `env -u CI`); `movieFallback.*` stays on the hosted runner.

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-movie-01-baseline --runs 3 --suite movie01
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

- `x_ite-web-runtime-02.mjs --suite gltfworld02` runs the 9 cases of `crates/x3d-web/tests/browser/gltfworld02_case.mjs`
  and records `web-browser-gltf-world-02-baseline/gltfworld02-baseline.json` (the CodeRabbit finding of PR #126). Each
  case loads a glTF world with two buffers, one of which fails while the test server holds the other: the first or the
  second fails, both fail in either order, a later candidate follows, and a newer loadURL replaces the world while both
  are held. A case keeps the state while the buffer is held (`during`) and after it was released. The holds make the
  order independent of the machine's speed.
- X_ITE rejects the candidate at the first failure (`Promise .all`), before the held buffer comes: loadURL fails, or
  the next candidate is requested at once. The held buffer's FileLoader goes on and logs its own failure later.
- Issues #128, #130 and #131 added 3 cases (`inlineFailThenHeld`, `followedReverse`, `supersededFollowed`; synthetic
  files `inline-fail-held*.{gltf,wrl}`, `fail-held-held-fail-fail.gltf`, `fail-held-fail.gltf`). The 6 older values
  are identical in the new recording.
  - #128 (Inline, one buffer failed, one held): X_ITE logs the failure and requests the Inline's next url
    (`external.gltf`) while the buffer is still held; its loadURL stays pending (`fallback.gltfFailed`). X_ITE_R waited
    for the held buffer (`inline.rs` did not call `gltf_buffer_failed_first`); fixed, now only the known
    `fallback.gltfFailed` difference remains (entry `gltfWorldHeld.inlineFailThenHeld`). An independent capture of
    pinned X_ITE with the REA `capture_browser_scenario` tool (evidence
    `ev_8669c3cff95e3aec39e21480e4968ddb1d76a6d0b91356437d782845df44a691`, a 3 s server hold) shows the same order:
    failure message, then the `external.gltf` request, then the held response.
  - #130 (two followed failures released together, third buffer first): X_ITE logs them in arrival order (the third,
    then the second); X_ITE_R matches. No difference.
  - #131 (newer loadURL while a followed buffer is held and missing): X_ITE does not abort the buffer's FileLoader and
    logs its failure after the world was replaced; X_ITE_R matches. No difference. (The pre-failure variant is the
    `replaced` case, which also matched.)
- Five runs gave identical values (Chrome 156.0.8078.47, RTX 3060 Ti through ANGLE GL).

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-gltf-world-02-baseline --runs 5 --suite gltfworld02
```

### Web browser field API 01 (`web-browser-fields-01-baseline/`)

- `x_ite-web-runtime-02.mjs --suite fields01` runs the 14 cases of `crates/x3d-web/tests/browser/fields01_case.mjs`
  (issue #137) and records `web-browser-fields-01-baseline/fields01-baseline.json`: the field classes of the X3D
  namespace and X3DConstants, the field classes as values, live SF and MF field objects of nodes (one object per field,
  changed components and elements reach the node and its routes, routed values reach a held object), assignments of
  every kind of field through the SFNode proxy, `addFieldCallback` (the callbacks of each event, in event order, with
  the event's value; `removeFieldCallback`), `getFieldDefinitions`, and `rootNodes` as an MFNode. The test world is
  synthetic and inline (a `data:` URL).
- The world needs `COMPONENT Scripting:1` for its Script node: X_ITE rejects a Script under `PROFILE Interchange`
  without it.
- Round trip (2026-11-01, issue #137): `fieldRoundTrip.sf` and `fieldRoundTrip.mf` give a Script `E` and a routed Script `F` a
  field of every SF and MF type except SFNode/MFNode, and record after each step (assignment, a changed component or element,
  `setValue`, `push`, `splice`, `unshift`, `length`) the page's object, its components, the text form the world writes and the
  routed field. `fieldConstants.constants` records `X3DConstants` before any world, after `PROFILE Full` and after nodes of
  several components (names only for the added ones: X_ITE numbers them in module load order, which varies). `fieldEarly.*`
  records calls before `ready ()` (a fresh `X3D .createBrowser ()`, detached and attached) and `createNode` (Full and Interchange
  worlds). The earlier 7 cases were re-recorded unchanged.
- Five runs gave identical values (Chrome 156.0.8078.47, RTX 3060 Ti through ANGLE GL).

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-fields-01-baseline --runs 5 --suite fields01
```

- Case `fieldEarly.namedNodes` (2026-11-01, issue #137) was added to the baseline without a full re-record: it was recorded alone
  (`--only fieldEarly.namedNodes --runs 5` into a scratch directory; five identical runs, Chrome 156.0.8078.47) and its value was
  merged into `fieldEarly`'s `values`; the other 14 values are byte-identical to before. It records `addNamedNode`,
  `updateNamedNode`, `removeNamedNode` and `getNamedNode` with their errors (`createScene ()` returns a promise in 16.5.0).
- Case `fieldEarly.exportedNodes` (2026-11-01, issue #137) was added the same way (`--only fieldEarly.exportedNodes --runs 5`
  into a scratch directory; five identical runs, same Chrome) and merged; the other 15 values are byte-identical to before.
  It records `addExportedNode`, `updateExportedNode`, `removeExportedNode` and `getExportedNode` with their errors, the
  `EXPORT` lines of `toVRMLString ()` and descriptions.
- Case `fieldEarly.namedLists` (2026-11-01, issue #137) was added the same way (`--only fieldEarly.namedLists --runs 5`, five
  identical runs, same Chrome) and merged; the other 16 values are byte-identical. It records `getNamedNodes ()` and
  `getExportedNodes ()`: the array objects (type names, live length, index access, iteration, Array methods, write errors) and
  their `X3DExportedNode` items.
- Case `fieldEarly.createProto` (2026-11-01, issue #137) was added the same way (five identical runs) and merged; the other 17
  values are byte-identical. It records `createProto` of the world's PROTOs, the instance (type name, fields, node type) and
  the errors for unknown names.

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

- `x_ite-web-runtime-02.mjs --suite routes01` runs the 10 cases of `crates/x3d-web/tests/browser/routes01_case.mjs`
  (issue #135, part 1: routes only) and records `web-browser-routes-01-baseline/routes01-baseline.json` (no pictures):
  `X3DScene .addRoute`, `deleteRoute` (route and four arguments), `getRoute`, `routes` (`RouteArray`) and the `X3DRoute`
  object; events over an added route against a parsed `ROUTE` (same frames, same order); the exact errors of bad nodes,
  bad field names, type mismatches and access types (`addRoute` says `source node must be ...`, `getRoute` says
  `sourceNode must be ...`; unknown fields throw, nothing is logged); `X3DBrowser .addRoute` / `deleteRoute`, which
  delegate to `currentScene` and return nothing (so `browser .deleteRoute (route)` throws the `getRoute` error); and routes
  between two scenes (`routesApi.crossScene`: X_ITE does not look at the scene of a node, takes the route and carries events
  between the scenes). The test world is synthetic and inline (a `data:` URL).
  `routesApi.browserEvent` (issue #135): a `BROWSER_EVENT` callback (and one of each other event type) across a world load,
  `replaceWorld`, viewpoint binding, `setLive` / `beginUpdate` / `endUpdate`, a failing load and removal: X_ITE never fires
  `BROWSER_EVENT`; the registry calls (argument counts, unknown event type, `getBrowserCallbacks ()` without argument) are recorded too.
- Five runs gave identical values (Chrome 156.0.8078.47).

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-routes-01-baseline --runs 5 --suite routes01
```

### Web browser scene creation 01 (`web-browser-scene-create-01-baseline/`)

- `x_ite-web-runtime-02.mjs --suite scenecreate01` runs the 26 cases of `crates/x3d-web/tests/browser/scenecreate01_case.mjs`
  (issue #134; `x3dString`, `replaceWorldArgs` and `replaceWorldGuards` added: `createX3DFromString` arguments, text forms and failures, `replaceWorld` with null / undefined / none / extra argument; `replaceWorldGuards`: the scene that is already the world, a scene of a replaced world and 17 values that are no scene, among them `MFNode` forms, which X_ITE accepts and X_ITE_R matches while it logs an error for each, `KNOWN_DIFFERENCES` `sceneCreate.replaceWorldGuards`) and records `web-browser-scene-create-01-baseline/scenecreate01-baseline.json` (no pictures):
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

### Web browser root nodes 01 (`web-browser-rootnodes-01-baseline/`)

- `x_ite-web-runtime-02.mjs --suite rootnodes01` runs the 9 cases of `crates/x3d-web/tests/browser/rootnodes01_case.mjs`
  (issue #137) and records `web-browser-rootnodes-01-baseline/rootnodes01-baseline.json` and 1 picture: `X3DScene
  .rootNodes` as a live field (the same object on every read, `rootNodes [i] === getNamedNode (...)`, name `rootNodes`,
  access type inputOutput); `push`, `unshift`, `splice`, `pop`, `shift` and `length` changes; `rootNodes = value` and
  `setRootNodes` (a plain array throws `TypeError: field.getValue is not a function`); `addRootNode` (no event for a node
  that is a root node already) and `removeRootNode` (every copy removed, an event also when nothing was removed; NULL,
  `undefined` and a string throw `TypeError: Invalid value used as weak map key`); NULL entries and duplicates (`USE`);
  the `rootNodes` field callbacks in order with a node field's (two changes before a frame are one event, the callback
  gets the live field); `toVRMLString ()` after each change; and the drawn world after `removeRootNode` / `addRootNode`.
  The test world is synthetic and inline (a `data:` URL).
- Five runs gave identical values and identical picture hashes (Chrome 156.0.8078.47). Prepare the worktree as in
  "Recording from a worktree".

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-rootnodes-01-baseline --runs 5 --suite rootnodes01
```

### Web browser sound 01 (`web-browser-sound-01-baseline/`)

- `x_ite-web-runtime-02.mjs --suite sound01` runs the 12 cases of `crates/x3d-web/tests/browser/sound01_case.mjs`
  (issue #140) and records `web-browser-sound-01-baseline/sound01-baseline.json` (no pictures): an AudioClip that plays
  through a Sound node (the synthetic tone `synthetic-rea-b05/media/tone48k-stereo.wav`). Each case records the Web Audio
  graph of the page (a capture wraps the page's Web Audio constructors and `connect`, `disconnect`, `start`, `stop`,
  numbered from the destination), the AudioClip's `isActive`, `isPaused` and `duration_changed`, the AudioContexts the
  page made, their state and the states at each `resume ()` call, and whether the audio clock runs. The cases: a looping
  clip; the unlock (a document with no user activation, then a click on the canvas); the unlock by a clip that the page
  starts from script; a clip that does not loop (its `isActive` events); pause and resume (the resume offset); `stopTime`; `loop` turned off while playing; the order of the clip's field events; a url that does not load; a replaced world; and the Sound fields `intensity`, `location` and `spatialize`.
- The group runs in a browser with `--autoplay-policy=document-user-activation-required` (the group's `browserArgs`):
  headless Chrome otherwise starts an AudioContext with no gesture. Playwright's `page .evaluate` gives the page a user
  activation, so the unlock cases reload the page with an init script and take the state before the gesture in the page.
- Five runs gave identical values (Chrome 156.0.8078.47), and three earlier runs of the same cases gave the same values.

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-sound-01-baseline --runs 5 --suite sound01
```

### Web browser Script life cycle 01 (`web-browser-script-life-01-baseline/`)

- `x_ite-web-runtime-02.mjs --suite scriptlife01` runs the 51 cases of `crates/x3d-web/tests/browser/scriptlife01_case.mjs`
  (issues #133 and #202) and records `web-browser-script-life-01-baseline/scriptlife01-baseline.json` (no pictures). Group
  `scriptLife` (20 cases, inline synthetic worlds): Script `initialize` (events sent from it, the order of Scripts,
  `directOutput`); the events of Script fields (eventIns, eventOuts, a route chain, an interpolator, page events, two events in
  one task, several field types); `eventsProcessed`; `shutdown` (world replaced twice, Script code replaced, element removed);
  Script errors (in `initialize`, a handler, `eventsProcessed`, `shutdown`, at the top level, a syntax error, a thrown string
  and object, a wrong value type); `Browser .currentScene .rootNodes` in a Script (read, `addRootNode`, `removeRootNode`,
  `setRootNodes`, `push`) with the field callbacks of the page and of the Script; and, from issue #202, a Script that runs
  after `importJS` (a JSON string and object) and `importDocument` (an XML string, an `XMLDocument`, an element), a function
  and a node handle that a Script left in the page and uses after `replaceWorld` (`staleFunction`), and the order of a
  Script's `print` and `console` lines against its calls into the page (`printTiming`). Each case records one ordered list of
  the console messages and the page callbacks (`seq`). `staleFunction` compares equal in X_ITE_R since
  issue #202 (no entry in `KNOWN_DIFFERENCES`). `printTiming` has one narrowed entry: a Script's console lines reach the page
  console during its call as in X_ITE, but the page callback on a field of the route chain still runs after the engine call.
- Group `scriptReload` (27 cases): the reload sets `synthetic-inline-reload` and `synthetic-externproto-reload` driven by the
  page's API (`url`, `load`, `remove`, `publish` actions; the server's `publish ()` makes a file appear while the world runs).
  A page cannot step frames, so each case compares what is settled: after the world and after each frame with actions it waits
  400 ms, then in windows of 650 ms until two windows in a row have the same non-empty set of clock ticks (at most 8 windows,
  about 5 s: a slow runner starts and stops clocks late, PR #235) and records the `warn`, `error`, `initialize`, `shutdown`
  and `created` lines of the whole wait, which clocks ran in the last window (digit-normalized, so only that a clock ran),
  and the `url` and `load` of the driven Inline. A step in which nothing ticks (four of them) waits the full 5 s. The
  scenarios that need `loadNow`, `unloadNow`, `autoRefresh` timing, an EXTERNPROTO declaration, the Inline of an Inline, or two
  actions in adjacent frames are not cases (listed in the comment of `reloadScenarios`), nor are `externproto/c01-initial` (a
  clock tick that falls in one window or the next) and `inline/c35` (the autoRefresh error count grows with the time waited). The pinned X_ITE's recorded stepped baselines of the same sets are
  `synthetic-inline-reload-baseline.json` and `synthetic-externproto-reload-baseline.json` (`x_ite-inline.mjs`).
- Group `inlineRetry` (4 cases, issue #202, c17): a failed response and the next load attempt, counted in requests to the server
  and waited for by polling the page's console lines (a barrier of two later failing Inlines shows what was not requested).
  Pinned X_ITE fetches again when an Inline's url is set again (`inlineSameUrl`: 2 requests, the file loads), does not
  fetch a failed url again that nothing loads (`inlineNotSetAgain`: 1 request) or when `load` is switched off and on
  after the failure (`inlineLoadToggle`: 1 request), and keeps an EXTERNPROTO's failure for good, also when an Inline's
  url is set to another scene with the same EXTERNPROTO (`externprotoAgain`: 1 request, a `setWorldURL` TypeError;
  X_ITE_R repeats the request's own error instead, a known difference of the message only).
  X_ITE_R's `WebFetcher` forgets a failed response when an Inline's url is set (`Fetcher::forget_failure`), so
  `scriptReload.inline.c17-recover-same-url` is no known difference any more. The four cases were recorded five times
  with identical values and merged into the baseline file (the other 46 values are unchanged).
- X_ITE 16.5.0 does not call `shutdown` for a replaced world and throws `array[i].set is not a function` when a Script
  that is a root node sees a root node change; the cases record the first and avoid the second (the Script is in a Group).
- Five runs gave identical values (Chrome 156.0.8078.47) for the 12 cases of #133 (and five more runs in separate processes at
  the same time gave the same values) and again for the 46 cases of #202 before `inlineRetry`. `scriptlife01.test.mjs` compares X_ITE_R with `scripts="true"`; its known differences are in
  `KNOWN_DIFFERENCES` (`scriptLifeDifferences`, `runtime02_harness.mjs`) and `web/API.md`. Prepare the worktree as in
  "Recording from a worktree".

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-script-life-01-baseline --runs 5 --suite scriptlife01
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

- `x_ite-web-runtime-02.mjs --suite options01` runs the 9 cases (10 since issue #141) of `crates/x3d-web/tests/browser/options01_case.mjs`
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

- Case `browserOptions.notification` (2026-11-01, issue #141) was added to the options01 baseline without a full re-record: it was recorded
  alone (`--only browserOptions.notification --runs 5` into a scratch directory; five identical runs, Chrome 156.0.8078.47) and its value
  was merged into `values`; the other nine values are byte-identical to before. It records `browser.description` and the notification
  element of Notification.js (text, class, inline styles, coercion, the Notifications option) with the 5000 ms timer taken over by the page.
- Case `browserOptions.debugKeys` (2026-11-01, issue #141) was added to the options01 baseline without a full re-record: it was recorded
  alone (`--only browserOptions.debugKeys --runs 5` into a scratch directory; five identical runs, Chrome 156.0.8078.47) and its value
  was merged into `values`; the other values are byte-identical to before. It records the keys X3DCoreContext handles itself (Home, End,
  Page Up, Page Down, Ctrl+Plus, the Debug keys Ctrl+1..5 and Ctrl+S) sent with Playwright's keyboard.
- Case `browserOptions.loadUrlObjects` (2026-11-01, issue #136) was added the same way (`--only browserOptions.loadUrlObjects --runs 5` into a scratch
  directory; five identical runs, Chrome 156.0.8078.47) and merged; the other values are byte-identical. It records whether an Inline loads
  (`browser.firstViewpoint ()` binds the only viewpoint, which is in the Inline's document) with `LoadUrlObjects` true and false, a new `url`, and
  the option set back.
- Case `browserOptions.enableInlineViewpoints` (2026-11-01, issue #136) was added the same way (five identical runs) and merged; the other values are
  byte-identical. It records the bound viewpoint's description after `firstViewpoint`, `nextViewpoint`, `previousViewpoint` and `lastViewpoint` in a
  world with viewpoints of its own and of an Inline, with the option true, false and true again.
- Case `browserOptions.straightenHorizon` (2026-11-01, issue #136) was added the same way (five identical runs) and merged; the other values are
  byte-identical. It records a ProximitySensor's `orientation_changed` and `position_changed` (the user's view, four decimals) for a rolled
  viewpoint: the default, the option FALSE before the world, set TRUE (after the one second transition), and `viewAll (0)` with FALSE and TRUE.
- Case `browserOptions.gravity` (2026-11-01, issue #136) was added the same way (five identical runs) and merged; the other values are
  byte-identical. It records the resting position (ProximitySensor `position_changed`) of a WALK viewer that starts 3 m above a floor, with the
  default, with Gravity 0 before the world and with the default set again (a negative value was dropped: the climb depends on the frame rate).

## Web browser UI 01 (2026-11-01, issue #141): descriptions, splash screen, timings panel, context menu, Shift+F8
- Five runs gave identical values (Chrome 156.0.8078.47). Prepare the worktree as in "Recording from a worktree".

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-ui-01-baseline --runs 5 --suite ui01
```

## Web browser UI 02 (2026-11-01, issue #141): Shift+F8 for GeoViewpoint and encodings, "Loading n files", Debug keys
- Five runs gave identical values (Chrome 156.0.8078.47). Prepare the worktree as in "Recording from a worktree".

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-ui-02-baseline --runs 5 --suite ui02
```

- `crates/x3d-web/tests/browser/ui02_case.mjs` (group `browserUi02`, test `ui02.test.mjs`) has four cases. `debugKeys`: the Ctrl+1 to Ctrl+5
  and Ctrl+S notifications with `Debug` on and off. `shiftF8Geo`: the clipboard text of Shift+F8 for a GeoViewpoint (GD, UTM and GC, a GeoOrigin)
  in VRML, XML and JSON; the numbers are compared digit for digit (X_ITE's user position carries matrix noise: 10 20 1000000 comes out as
  9.99999999999995 20 999999.999999998; the engine reproduces it). `shiftF8Encodings`: the text follows `scene .encoding` (VRML, XML, JSON, SCRIPTED is XML), also for a world
  made by `createX3DFromString` and by `createScene` (a Viewpoint pushed into its `rootNodes` is bound), an OrthoViewpoint and a world without a viewpoint. `loading`: every `setDescription` call
  while Inlines, a bad Inline and a texture load ("Loading 1 file", "Loading 2 files", "Loading done"), with `Notifications` on and off.

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

## MovieTexture GIF and APNG playback (2026-10-30, issue #90)

- `tools/gen_movie_anim.py` writes `synthetic-movie-anim/`: 8 × 8 GIF and APNG media made with the standard library
  (exact frame rectangles, delays, disposal, transparency, dispose and blend operations), the world `movie-anim.wrl`
  (12 unlit MovieTexture quads) and its event script. A key press at frame 32 starts every MovieTexture through a
  Script: X_ITE starts a time-dependent node from the file only through `loop` at initialize, before the media are in,
  so the start would depend on the load time.
- `tools/movie_anim.py record` runs `x_ite-anim.mjs` (MSAA 0) and `x_ite-anim-msaa.mjs` (MSAA 4) five times and writes
  `movie-anim-baseline/` (`cases.txt`, `NAME-msaaM/js.json` and `js/*.png`) only when every run gave the same bytes;
  `recordings.json` keeps the sha256 of each run's files. The five runs of 2026-10-30 were identical.
- GIF frames follow `elapsedTime` (GifMedia: equal steps, not the frame delays). apng-js plays on
  `requestAnimationFrame` and the wall clock, not on the browser time the recorder steps; in the headless recorder its
  ticks are rare and irregular. A world that played an APNG once showed an arbitrary middle frame 8 virtual seconds
  after the start, and the five runs did not agree, so it is not a baseline. The APNG cases are the states that do not
  depend on that clock: frame 0 before the start, a first frame of 60 s, speed 0 and `enabled FALSE`.
- `crates/x3d-viewer/tests/movie_anim.rs` compares `x3d-drive` with the baseline. Prepare the worktree as in
  "Recording from a worktree".

```
python3 -I oracle/tools/gen_movie_anim.py
python3 -I oracle/tools/movie_anim.py record 5
```

### MovieTexture WebM pictures at fixed movie times (2026-11-03, issue #90)

- `tools/gen_movie_video.py` writes `synthetic-movie-video/` (a world of four unlit quads and its event script). The media are
  the synthetic WebM streams of `synthetic-movie-codecs/media` (ffmpeg test patterns; see `tools/gen_movie_codecs.py`):
  `vp8-ramp` (uniform gray pictures), `vp8-testsrc` (inter frames, key frames every 6), `vp8-parts` and `vp9-vorbis`. Every
  node plays at `speed 0`, so the video element holds its picture; the script starts the nodes with a key press and the event
  `movieSeek` (`node`, `time`) puts them at 0.25, 0.55, 0.05 and 0.35 s (the middle of a 0.1 s picture; the last two are
  backward seeks), three frames before each capture.
- `movieSeek` in `x_ite-anim.mjs` and `x_ite-anim-msaa.mjs`: the harness pauses the node's video element, sets `currentTime`
  and waits until the element is not seeking and is at that time (the answers are in `movieSeeks` of the result). In
  `x3d-drive` (`crates/x3d-viewer/src/script.rs`) the same event calls `Runtime::movie_seek`. Nothing in X_ITE is changed.
  Both recorders' servers now answer byte ranges (`206`): without them Chrome does not seek the element. Requests without a
  `Range` header get the same body as before; the earlier `movie-anim` recordings (MSAA 0 and 4) were re-recorded and are
  byte-identical to the stored baseline.
- `tools/movie_anim.py record 5 video` wrote `movie-video-baseline/` (MSAA 0 and 4). The five runs of 2026-11-03 were
  identical (js.json and every PNG). The baseline holds only synthetic worlds and media, so it is committed (it is not one of
  the private answer keys of `answer-keys.tsv`).
- `crates/x3d-viewer/tests/movie_anim.rs::movie_video_frames_match_x_ite` compares `x3d-drive` (pinned timing) on the GPU path:
  at most 0.5% of the pixels more than 8 off. Measured 2026-11-03 (RTX 3060 Ti): 0 pixels over 8, max delta 1, on all five
  frames at MSAA 0 and at MSAA 4. A control with one seek time changed fails the test (6.35% over 8). No adapter: the test
  fails in strict mode and records a `no-gpu-adapter` skip otherwise. A missing baseline directory fails.
- Not in the world: AV1 (the decoder is a separate pull request; its still frames are in `render-movie-codecs-baseline`).
  The animated middle frames of an APNG stay unrecorded (apng-js runs on the wall clock, see above).

```
python3 -I oracle/tools/gen_movie_video.py
python3 -I oracle/tools/movie_anim.py record 5 video
```

### MovieTexture PNG load errors (2026-10-30, PR #190 review)

- `tools/gen_movie_load_errors.py` writes `synthetic-movie-load-errors/`: 8 × 8 PNG media (static, not a PNG, acTL
  without fcTL, cut in the header, an APNG cut in its second frame, an APNG with invalid frame data, a good APNG) and
  one world. A MovieTexture that stays IN_PROGRESS at the first load stops X_ITE's `loadURL`, so a Script sets the
  `url` of one case for each key press; a LoadSensor and the node's outputs are printed.
- `tools/movie_anim.py record 5 load-errors` wrote `movie-load-errors-baseline/` (MSAA 0). The five runs of
  2026-10-30 were identical. X_ITE: IN_PROGRESS without a message for the static PNG, the non-PNG and the APNG without
  fcTL; a warning, the next URL and FAILED for the cut header; COMPLETE for both APNGs with a valid chunk structure
  (the invalid frame draws nothing). What the browser draws for the rows of a partly cut frame is not recorded:
  nothing is started, so only frame 0 is shown.

```
python3 -I oracle/tools/gen_movie_load_errors.py
python3 -I oracle/tools/movie_anim.py record 5 load-errors
```

### Anchor, Collision and InstancedShape pointer paths (REA batch 24, issue #94)

- `tools/gen_rea_b24_anchor_collision_instanced.py` writes `synthetic-rea-b24-anchor-collision-instanced/` (authored, no
  historical assets): `ci-pick` (InstancedShape hits as a 2601-probe grid, `pickProbes` compared bit for bit),
  `ci-events` (TouchSensor events over instances), `cl-render` (Collision `enabled`, `visible`, `proxy`, `bboxDisplay`,
  nesting: pictures and probes), `cl-events` (TouchSensor events under Collision nodes whose `enabled` and `visible`
  change at run time), `an-nested` (Anchor in Anchor, an Anchor with a TouchSensor child, an Anchor over instances).
- `tools/rea_b24.py record NAME WORLD SCRIPT MSAA` wrote `rea-b24-anchor-collision-instanced-baseline/` (MSAA 0 and
  4, each three identical X_ITE runs, `loadFrames` left out of the identity check). The pinned X_ITE needs the
  `x_ite-pin/node_modules` link of "Recording from a worktree".
- `crates/x3d-viewer/tests/rea_b24_anchor_collision_instanced.rs` compares `x3d-drive` with the baseline (GPU test; it
  skips without an adapter through `x3d_evidence::no_gpu`).
- The anchor-click, anchor-load and collision-walk cases of batch 09 (`synthetic-rea-b09-stepped`) and the
  `pick-instanced` and `nav-instanced-*` cases of batch 09 nav already record the click, load, avatar-collision and
  per-instance touch paths.
- One Rust defect was fixed with this batch: `pointerEvents FALSE` on a Shape or InstancedShape was ignored by
  the pointing pass (the shape stays a collision object).
- Known gap `an-nested`: a fragment Anchor click binds its Viewpoint one frame later in Rust than in X_ITE (X_ITE calls the
  Anchor's `set_active__` in the mouse handler). Moving the request ahead breaks the batch 09 anchor-load cases (owner
  decision, issue #94).
```
python3 -I -B oracle/tools/gen_rea_b24_anchor_collision_instanced.py
python3 -I -B oracle/tools/rea_b24.py record ci-pick oracle/synthetic-rea-b24-anchor-collision-instanced/ci-pick.wrl \
  oracle/synthetic-rea-b24-anchor-collision-instanced/ci-pick.json 0
```

## Render items (issue #92)

`oracle/synthetic-rea-i92-render-items/` holds 46 synthetic worlds (headlight `hl-*`, lighting model `lm-*`, transparency
sorting `tr-*`, fog `fg-*`), written by `oracle/tools/gen_rea_i92_render_items.py`. `oracle/tools/rea_i92_render_items.py shots`
records each one three times with the pinned X_ITE at MSAA 0 and at MSAA 4 and writes `oracle/rea-i92-render-items-baseline/{png,msaa4}`
only when the three runs are identical and no world logs an error. A world whose multisampled output differs between X_ITE's own runs gets
no MSAA 4 picture and is listed in `msaa4/unstable.json` (`fg-transparent`, `tr-many`, `tr-ortho`). `crates/x3d-render/tests/render_rea_i92_items.rs`
replays every world and fails on a missing reference, an error log or a picture over the bound (8 per channel, 0.5% of pixels).

## Order independent transparency (sub:render.transparency-sorting)

`oracle/synthetic-rea-oit-render-items/` holds 11 synthetic worlds (`oit-*`: listing order, opaque shapes in between, depth-intersecting
shapes, textures, PhysicalMaterial, the alpha range, ten to twenty layers, points, lines and other primitives, Fog, an OrthoViewpoint, a
LayerSet), written by `oracle/tools/gen_rea_oit_render_items.py`. `oracle/tools/rea_i92_render_items.py shots --oit` records each one three
times per pass with the pinned X_ITE (`x_ite-shot.mjs --oit` at MSAA 0, `x_ite-shot-msaa.mjs --oit` at MSAA 4: the `OrderIndependentTransparency`
browser option set with `setBrowserOption` before the world loads) and writes `oracle/rea-oit-render-items-baseline/{png,msaa4}` only when the
three runs are identical and no world logs an error; a world whose multisampled output differs between X_ITE's own runs has no MSAA 4 picture
and is listed in `msaa4/unstable.json`. `crates/x3d-render/tests/render_rea_oit_items.rs` replays every world with the option on (bound: 8 per
channel, 0.5% of pixels) and, as the negative control, with the option off (every world must then be over the bound).

`oracle/tools/rea_i92_render_items.py shots --oit-i92` records the transparency worlds of `synthetic-rea-i92-render-items` (`i92-tr-*`,
`i92-fg-transparent`) with the option on into `oracle/rea-oit-i92-render-items-baseline/{png,msaa4}` the same way (`--only` takes a comma-separated
list); `i92-tr-many` and `i92-tr-nested` have no MSAA 4 picture (`msaa4/unstable.json`). `crates/x3d-render/tests/render_rea_oit_i92_items.rs`
replays them with the option on (same bound).

## decodeAudioData lane (2026-10-31, issue #98): MP3, Ogg, FLAC, Opus, WAV

- `x_ite-audio-decode.mjs` decodes the files of `synthetic-rea-b24-audio/` (ffmpeg sine plus seeded noise, 0.3 s;
  clean-room) with `browser.getAudioContext ()` of the pinned X_ITE in headless Chromium and records length, rate,
  channels, per-channel sha256, a few samples, the peak and all samples (base64 float32) in
  `rea-b24-audio-decode-baseline/audio-decode-baseline.json`. Prepare the worktree as in "Recording from a
  worktree" above; `--runs 5` fails unless the five recordings are byte-identical.
- `node oracle/x_ite-audio-decode.mjs oracle/synthetic-rea-b24-audio oracle/rea-b24-audio-decode-baseline/audio-decode-baseline.json --runs 5`
- `crates/x3d-runtime/tests/rea_b24_audio_decode.rs` compares the port; its header lists the per-format results.
- Second corpus (issue #90, 2026-11-02): `synthetic-rea-b24-audio-2/` (13 files, ffmpeg 440 Hz sine plus seeded white noise
  `seed=7`, 0.3 s, clean-room: AAC in M4A with the moov atom at the end and first, A-law and mu-law WAV at 8, 44.1 and 48 kHz,
  MP3 11.025 and 32 kHz, FLAC 8 kHz, Ogg Vorbis 32 kHz) recorded into `rea-b24-audio-decode-2-baseline/audio-decode-baseline.json`
  with `node oracle/x_ite-audio-decode.mjs oracle/synthetic-rea-b24-audio-2 oracle/rea-b24-audio-decode-2-baseline/audio-decode-baseline.json --runs 5`
  (five byte-identical recordings, Chromium 151.0.7922.34). The recorder now also reads `.m4a`. The first baseline is unchanged.

## AudioDestination mediaDeviceID (issue #81)

- `x_ite-audio-sink.mjs` sets `mediaDeviceID` of one AudioDestination in the pinned X_ITE (headless Chromium) to `""`,
  the first enumerated output, `"default"`, an unknown id, `""`, the unknown id again and `"default"`, and records the
  node's `<audio>` element `sinkId` after each step, the enumerable outputs, unhandled rejections and the console
  (without `info` and GL driver lines) in `web-audio-sink-baseline/sink-baseline.json`.
- X_ITE: `""` and the current sink are left alone, `"default"` is accepted, an unknown id makes `setSinkId` reject and
  the sink stays where it was. Nothing is logged and no rejection is unhandled (`startAudioElements` catches it). The
  headless host has one output (id `""`), so a real device id is not recorded.
- Three runs gave identical bytes (Chrome 151.0.7922.34). `sound::sink::tests::follows_the_recorded_baseline`
  replays the steps. Prepare the worktree as in "Recording from a worktree".

```
node oracle/x_ite-audio-sink.mjs oracle/web-audio-sink-baseline/sink-baseline.json --runs 3
```

## Navigation items (issue #91, 2026-10-31)

`synthetic-rea-b24-nav/` (generator `tools/gen_rea_b24_nav.py`) holds 40 navigation cases that batch 09 does not have: ceiling,
step-up threshold, avatarSize variants, speed, Gravity, centerOfRotation, trackball (StraightenHorizon false), PLANE limits and
double click, WALK in a tilted Transform. `tools/rea_b24_nav.py record` records each case from the pinned X_ITE three times
(identical, no pictures) into `rea-b24-nav-baseline/`; `crates/x3d-viewer/tests/rea_b24_nav.rs` compares Rust with it.
The item lists of the five navigation subsystems are in `docs/reports/NAV_ITEM_LISTS.md`.

`synthetic-rea-gravity-layers/` (generator `tools/gen_rea_gravity_layers.py`) holds 12 cases for C17 and C18 of that list: a
LayerSet of two Layers with a ground, a Viewpoint and a NavigationInfo each (active layer 0, 1 and 2, switched at run time by a
Script, a switch in the middle of a fall, a walk toward walls that exist in one layer only) and a viewpoint transition that carries
the eye through a Collision wall and over a drop (WorldInfo text counts the wall's isActive changes). `tools/rea_gravity_layers.py
record` records each case from the pinned X_ITE three times (identical) into `rea-gravity-layers-baseline/NAME-msaa0.json` (one
directory level); `crates/x3d-viewer/tests/rea_gravity_layers.rs` compares Rust with it, with the thresholds of `rea_b24_nav.rs`.

`synthetic-rea-collision-time/` (generator `tools/gen_rea_collision_time.py`, recorder `tools/rea_collision_time.py`, test
`crates/x3d-viewer/tests/rea_collision_time.rs`) also holds `ct-wall-pick`, `ct-nested-pick` and `ct-bb-ctl-pick` (2026-11-04): the worlds
ct-wall, ct-nested and ct-bb-ctl with a 16 x 16 `pickProbes` grid, three identical recordings at MSAA 0 and 4. The pointing buffer's hit
points (0.27 to 0.38 from the surface) equal the page's bit for bit; the 3.05e-7 once measured there was the avatar's stop position
(before #423), not float32 depth.

The same group holds `ct-live-inline` and `ct-live-inline-two` (Collision.set_live__, ledger node:Collision): the wall Collision sits inside an Inline
(`ct-live-inline-inner.wrl`, EXPORTed and IMPORTed as C1); a parent Script sets the Inline's `load` FALSE at 2.6 s of page time (the avatar has been
stopped against the wall since frame 59) and TRUE at 4.3 s, and the avatar walks through the gap and back at the wall. The pinned page: the wall stops
the avatar, `isActive` TRUE and `collideTime` at 1.933 s; after `load` FALSE the avatar walks through; no `isActive` FALSE is ever seen (the Collision is
disposed with the Inline's scene: `set_live__` only calls `removeCollision`, and the parent's routes die with the scene; a Script inside the Inline
prints nothing more); after `load` TRUE the new Collision stops the avatar again, `isActive` TRUE at 6.233 s. `-two` adds a never-reached Collision
in the parent, so that the browser's collision count stays above 0 while the Inline is unloaded: the same events. The test also compares the
recorded `console`. Three identical recordings at MSAA 0 and 4; Rust equals them.

## ShaderPart at MSAA 4 (issue #95, 2026-11-04)

`rea-b24-shaderpart-baseline/` holds the 16 stepped ShaderPart cases at MSAA 0 (`NAME-msaa0/js.json` and pictures) and at MSAA 4
(`NAME-msaa4.json` flat, so that the review gate leaves it out, and pictures in `NAME-msaa4/js/`). `python3 -I -B
oracle/tools/rea_b24_shaderpart.py record NAME 3 --msaa 4` runs `x_ite-anim-msaa.mjs --ms 4` three times; only identical runs
(record and every picture hash) are kept. Prepare the worktree as in "Recording from a worktree". The MSAA 4 records equal the MSAA 0
records in scene text and log (only `info` differs); the pictures are the new evidence. Rust matches scene text, log (KNOWN
table as at MSAA 0), poses and every picture within 1 per channel. All of these fragment sources say `precision mediump float`,
and the pinned page on the RTX 3060 Ti (ANGLE, NVIDIA driver) runs mediump in half precision: `vec4 (0.9, 0.7, 0.2, 1.0)` is
(229, 179, 51) there and (229, 178, 51) in Rust (0.7 is 0.70019531 in half precision). With `highp` in both renderers
include-Fog, select-steps, url-steps and vertex-steps differ by 0 pixels at MSAA 4. The `PINNED` table of
`crates/x3d-viewer/tests/rea_b24_shaderpart.rs` holds the measured pixel counts per case; a change in either direction fails, on an adapter named RTX 3060 Ti (the measured one); on any other adapter the picture bounds apply only.
The test skips the MSAA 4 cases without a GPU adapter.

## Native keys 01 (issue #80)

`node oracle/x_ite-native-keys-01.mjs oracle/native-keys-01-baseline --runs 3` records what the pinned X_ITE does for each
keyboard shortcut of X3DCoreContext (Ctrl+Plus Timings, the Debug keys Ctrl+1..5 and Ctrl+S, Home / End / Page Up / Page Down,
the five modifier flags, Shift+F8 and its clipboard text), after every key: options, `isLive`, description, bound viewpoint, flags and
clipboard. `crates/x3d-viewer/tests/native_keys_01.rs` replays the same keys on the native viewer and compares each step. The
Shift+F8 case waits 4 s after the viewpoint change so the viewpoint transition has ended.

## Menu triggers 01 (issue #287)

`node oracle/x_ite-menu-triggers-01.mjs oracle/menu-triggers-01-baseline --runs 5` (prepare the worktree as in "Recording from a
worktree"; Playwright from `XITE_TESTS` or `crates/x3d-web/node_modules`) records what opens the context menu of the pinned X_ITE
16.5.0 in headless Chrome (Linux) with touch emulation (`hasTouch`, CDP `Input.dispatchTouchEvent`): the events that reach the
element and whether the menu is open, for the right button, a left click, a real Ctrl+click, a macOS-style event sequence, bare
`contextmenu` events, and touch holds of 50 ms to 1.5 s with moves of 6 to 40 px, on a box with and without a TouchSensor.

- `ContextMenu.js` listens only to the element's `contextmenu` event and has no Ctrl check. The long tap is
  `PointingDevice.js`: `CONTEXT_MENU_TIME` 1200 ms, cancelled by `touchend` or a move of more than 7 px (`Math.hypot`), and started only
  when the touched hit has no sensor. The recording brackets it: 1100 ms closed, 1300 ms open; 7 px open, 8 px closed.
- Real Ctrl+click on Linux gives a plain click (no `contextmenu`). The macOS sequence (`mousedown` button 0 with ctrlKey, `contextmenu`,
  `mouseup`) is synthetic: it is Chromium's documented macOS behavior, not recorded on macOS.
- `crates/x3d-viewer/tests/native_menu_triggers.rs` replays each case on the native viewer.

## Script fetched-candidate frame (issue #54)

- `oracle/synthetic-rea-b09-sai/scr-url-fetch-frame.wrl` (made by `oracle/tools/gen_rea_b09_script.py scr-url-fetch-frame`) has Scripts whose
  candidates need no fetch (U03, U04, U12) and Scripts whose candidates need one (a base64 data URL, files, a missing file with a second
  candidate), and a reporter that ticks every frame.
- Pinned X_ITE does not repeat: 14 runs gave four frame maps and five orders (`oracle/rea-b09-sai-baseline-variable/scr-url-fetch-frame/xite-runs.json`).
  It is therefore not in `rea-b09-sai-baseline/cases.txt` (the recorder's rule is five identical runs). To record again:
  `node oracle/x_ite-anim.mjs oracle/synthetic-rea-b09-sai/scr-url-fetch-frame.wrl oracle/synthetic-rea-b09-sai/sai.json --out /tmp/run.json`
  (prepare the worktree as in "Recording from a worktree").
- `crates/x3d-viewer/tests/rea_b54_script_fetch_frame.rs` checks X_ITE_R against ISO/IEC 19775-1 V4.0 29.2.3 and the recorded envelope;
  the decision is in `docs/adr/0007-script-no-dom.md`.

## Owner decisions batch (issue #68)

`oracle/synthetic-rea-b68-owner/` holds five worlds (BlendedVolumeStyle `ALPHA1`, `ALPHA2` and the CONSTANT twin; ShadedVolumeStyle with a `Material` and with a `PhysicalMaterial`) and `xite-runs.json`, the result of `node oracle/x_ite-shot.mjs oracle/synthetic-rea-b68-owner OUT` run five times against the pinned X_ITE (5 identical): `bl-iso1` and `sh-physicalmaterial` never draw (`TIMEOUT 45s`), `bl-iso2` draws the same picture as `bl-const`. Rust side: `crates/x3d-render/tests/render_b68_owner.rs`. Decisions: `docs/adr/0012-owner-decisions-batch.md`. The shot tool serves only paths under `oracle/`; it needs `oracle/x_ite-pin/node_modules` (a symlink, not committed).

### Web browser navigation touch 01 (`web-browser-nav-01-baseline/`)

- `x_ite-web-runtime-02.mjs --suite nav01` runs the cases of `crates/x3d-web/tests/browser/nav01_case.mjs` (issue #370,
  ledger `sub:nav.*`) and records `web-browser-nav-01-baseline/nav01-baseline.json`: synthetic Chrome DevTools touch
  scripts in a touch context, the viewpoint pose (position, axis, angle) after every step. Examine (E15): a two-finger
  pan (the same-way move mode, the mode locked by the first moves) and a double tap (look at the box under the finger;
  two taps 450 ms apart and a double tap on empty space look at nothing). The tolerance, fixed before the first run, is
  1e-3 on every number (`NAV01_TOLERANCE` in `runtime02_harness.mjs`). Each step waits 400 ms (twice MOVE_TIME), so
  the poses do not depend on frame timing. Three runs gave identical values. Prepare the worktree as in "Recording from
  a worktree".

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-nav-01-baseline --runs 3 --suite nav01
```

### Web browser navigation touch 02 (`web-browser-nav-02-baseline/`)

- `x_ite-web-runtime-02.mjs --suite nav02` runs the cases of `crates/x3d-web/tests/browser/nav02_case.mjs` (issue #370,
  ledger `sub:nav.walk-fly`, F16) and records `web-browser-nav-02-baseline/nav02-baseline.json`: WALK and FLY with a
  synthetic Chrome DevTools touch. One finger held away from where it went down for 1.5 s (forward, back, turn, a
  diagonal in WALK; forward and turn in FLY): the direction of travel, the speed and the turn rate (displacement and
  angle over the measured seconds; keys that end in `Rate`), because X_ITE integrates over the real frame times; two
  fingers looking around in both, the settled orientation after each step. The runs agree within the test tolerances,
  not bit for bit (the recorder takes the mean): 1e-3 for a number, 3% between runs and 5% against X_ITE_R for a
  `Rate`, 1e-2 for a `Dir` (the diagonal walk's net direction). WALK has gravity, so the world has a ground under the
  viewpoint. Three runs.

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-nav-02-baseline --runs 3 --suite nav02
```

### Web browser navigation touch 03 (`web-browser-nav-03-baseline/`)

- `x_ite-web-runtime-02.mjs --suite nav03` runs the 12 cases of `crates/x3d-web/tests/browser/nav03_case.mjs` (issue
  #370, ledger `sub:nav.plane` P09 and `sub:nav.lookat` L06) and records `web-browser-nav-03-baseline/nav03-baseline.json`
  and 2 pictures: PLANE and LOOKAT with a synthetic Chrome DevTools touch, the viewpoint pose after every step. Two
  fingers moving the same way, apart and together, the mode locked by the first moves, one finger dragging (no effect in
  either viewer), a double tap on a box and on empty space (a look-at on the box in both), taps too far apart. PLANE's
  zoom changes the field of view scale, which a page cannot read (it is not in the Viewpoint's `getFieldDefinitions`),
  so the zoom cases also compare the picture (the thresholds of runtime 01). The tolerance, fixed before the first run,
  is 1e-3 on every number. Three runs, identical values.

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-nav-03-baseline --runs 3 --suite nav03
```

### Web browser navigation press 04 (`web-browser-nav-04-baseline/`)

- `x_ite-web-runtime-02.mjs --suite nav04` runs the 5 cases of `crates/x3d-web/tests/browser/nav04_case.mjs` (issue #370,
  ledger `sub:nav.examine` E16) and records `web-browser-nav-04-baseline/nav04-baseline.json`: a layer whose Viewport
  (`clipBoundary 0.25 0.75 0.25 0.75`) covers the middle of the canvas, and EXAMINE's `isPointerInRectangle`. A mouse press
  outside or above the viewport rotates nothing and neither does the rest of that drag; a press inside rotates, and keeps
  rotating when the pointer leaves the viewport; a wheel step outside does not zoom, inside it does; a finger down
  outside rotates nothing, one inside rotates; two fingers pinching with their middle outside zoom nothing, inside they
  zoom. The pose after every step; the tolerance, fixed before the first run, is 1e-3 on every number. Three runs,
  identical values.

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-nav-04-baseline --runs 3 --suite nav04
```

## AudioClip and Sound output levels and timing (issue #90)

`oracle/rea-b05-baseline/NAME/{js.json,js.audio.f32}` holds, for the 14 worlds added to `oracle/synthetic-rea-b05`
(written by `oracle/tools/gen_rea_audio_repair.py`; media: the existing synthetic tones), the sound that the pinned X_ITE
16.5.0 (`34cdf9f97`, headless Chrome, `OfflineAudioContext`, one 768-sample block per frame) renders. They cover what
no earlier key compared:

| world | field changes |
| --- | --- |
| `clip-gain`, `clip-gain-runtime`, `clip-gain-pitch-intensity` | `AudioClip.gain` 0.5, 2, 0; changed to 0.25, 0, 3, -1, 1 while playing; with `pitch` and `Sound.intensity` |
| `sound-intensity-runtime` | `Sound.intensity` changed to 0.5, 0, 1.5 (clamped), -0.5 (clamped), 0.25 |
| `clip-pitch-edges` | `pitch` 4, then 0, -1, 1.5, 0.25 |
| `clip-enabled`, `sound-enabled` | `enabled` FALSE and TRUE while playing |
| `clip-loop-off`, `clip-stop-in-loop`, `clip-restart` | `loop` set FALSE while playing; `stopTime` inside a loop; `startTime` set while active and after the end |
| `sound-location-runtime`, `sound-priority`, `sound-mono-left`, `sound-viewpoint-turned` | `location`, `direction` and the ranges changed while playing; `priority`; a 22.05 kHz mono clip at the left; a turned and re-turned Viewpoint |

Each was recorded four times (three recordings and the one that wrote the key) with identical scene text, log, Script
prints, suspend frames and samples. `rea_b05.py check` and `cargo test -p x3d-viewer --test vrml_audio_oracle` compare
every sample (tolerance 0). The test fails when a key or world is missing, and `vrml_audio_oracle_keys_have_the_recorded_shape`
fails when a key is not what was recorded (frame count, silent frames, peak).

```
python3 -I oracle/tools/gen_rea_audio_repair.py
XITE_TESTS=~/Projects/cybertown/X_ITE/x_ite-tests python3 -I oracle/tools/rea_b05.py baseline clip-gain   # NAME: rewrites its key; git diff shows a changed one
python3 -I oracle/tools/rea_b05.py check --target target/release
```

## XR views against the pinned WebXR context (issue #83)

`oracle/xr-shot.html` + `oracle/x_ite-xr-shot.mjs` draw a world with the pinned X_ITE 16.5.0 through its own `X3DWebXRContext`,
on a mock session: no headset, no XR runtime. The mock replaces `navigator.xr` (`isSessionSupported`, `requestSession`),
`XRWebGLLayer` (no framebuffer of its own: X_ITE draws every view into the canvas at the viewport that the script gives) and
`gl.makeXRCompatible`, hands out one viewer pose with the script's views (projection matrix, transform, inverse, viewport), and runs
`browser .xrStartSession ()` and the pinned frame callback. The page is served at a URL inside `x_ite-pin/src` (X_ITE resolves its
assets relative to the page) but is not a file of the pin (`tools/verify-candidate.sh` digests that directory).

```
python3 -I oracle/tools/gen_xr_shot.py                  # oracle/synthetic-xr/NAME.json: size, pose, views (column-major, as WebXR)
XITE_TESTS=~/Projects/cybertown/X_ITE/x_ite-tests node oracle/x_ite-xr-shot.mjs oracle/synthetic-xr/NAME.json /tmp/xr-out --runs 3
cp /tmp/xr-out/NAME.{png,json} oracle/xr-shot-baseline/  # written only when all runs are identical
XITE_TESTS=... node oracle/x_ite-xr-shot.mjs oracle/synthetic-xr/NAME.json /tmp/xr-out4 --runs 3 --ms 4   # Multisampling 4 (page ?ms=4)
cp /tmp/xr-out4/NAME.{png,json} oracle/xr-shot-baseline-msaa4/
```

The recorder needs `oracle/x_ite-pin/node_modules` (see "Recording from a worktree"). `--ms N` (default 0, the page's `antialiased="false"`)
sets X_ITE `Multisampling`; the JSON of a key has `ms`.

Keys `oracle/xr-shot-baseline/NAME.png` (MSAA 0) and `oracle/xr-shot-baseline-msaa4/NAME.png` (MSAA 4) (synthetic worlds `oracle/synthetic-xr/*.x3dv`; the JSON beside each has the page's
requests of the mock, the renderer and the hashes) are compared by `cargo nextest run -p x3d-render --test xr_views`. A missing
key fails; no adapter fails in strict mode and records a skip otherwise. `XR_CASES=a,b` runs other cases, `XR_DUMP=dir` writes the
Rust pictures.

A script with `inputSources` (`[{ "matrix": target-ray pose, column-major, "pressed": bool }]`) gives the page mock tracked-pointer input sources
(`inputsourceschange`, `frame.getPose`): the pinned context then draws their rays and hit points (ScreenLine, ScreenPoint), and the key's JSON
records what it found per source (`inputSources`: `active`, `id`, `point`, `normal` = `hit .id`, `hit .originalPoint`, `hit .originalNormal`).
`xr_views.rs` draws from those recorded hits and `x3d-viewer`'s `xr_rays.rs` compares the viewer's own picks with them (case `stereo-pointers`).

## GoldenGate binary dispatch (sub:parse.golden-gate)

`oracle/synthetic-rea-golden-gate-binary/` holds authored binary inputs (binary STL, binary PLY little and big endian, GLB, each a few triangles
at most, plus malformed variants) and four worlds (`gg-stl`, `gg-ply`, `gg-glb`, `gg-dispatch`), all made by `oracle/tools/gen_golden_gate_binary.py`.
A Script in each world loads its URL lists one after the other with `Browser.createX3DFromURL` and records the scene head and `toVRMLString`,
or the rejection, in a field of its own. `gg-dispatch` has the same bytes under other extensions (`.wrl`, `.txt`, `.json`, none, a `.stl` that is a
PLY), gzipped binaries, bytes no parser takes, an empty file and URL lists whose first entries fail.

```
XITE_TESTS=... python3 -I oracle/tools/golden_gate_binary.py record   # three pinned recordings per world, written only when identical
cargo nextest run -p x3d-viewer --test golden_gate_binary
```

The baselines are `oracle/golden-gate-binary-baseline/gg-*.json`. "Identical" means every key but `warmupFrames` and `loadFrames`: the number of real
frames the pinned page runs before the scripted frames start varies from run to run (10 or 11 for `gg-stl`, 23 or 26 for `gg-dispatch`); the scene
text does not. The scene text of every dumped frame and the sorted `log` (the loader's `Couldn't load URL ...` lines with the parser's error) are compared
exactly. A NaN coordinate in a binary STL is left out of the cases: the pinned page logs `RangeError: Maximum call stack size exceeded` for it
(measured), an engine limit and not a parser result.

## glTF and GLB resource failures at the createX3DFromURL level (sub:parse.gltf2, sub:parse.glb2)

`oracle/synthetic-rea-gltf-fetch/` holds authored glTF and GLB files (one triangle, a 2 x 2 PNG made with zlib; nothing third-party) whose external
buffers (`buf/`), images (`img/`) or `EXT_texture_video` sources (`vid/`) are missing, empty, malformed or fine, and three worlds, `gf-buffer`,
`gf-image` and `gf-video`, made by `oracle/tools/gen_gltf_fetch.py`. Like the GoldenGate worlds, a Script loads its URL lists one after the other with
`Browser.createX3DFromURL` and records the scene head and `toVRMLString`, or the rejection. Two things differ from them:

- The loads start from a TimeSensor event after the first frame past the frozen load time. A glTF buffer that fails to load is never removed from the
  loading objects of the world in the pinned X_ITE, so the world's own `loadURL` never settles if the load starts during it (the recorder then waits
  for ever).
- The decoded-image error names a blob URL with a random UUID in the pinned page (`Couldn't load image 'blob:http://oracle/<uuid>': error.`). The
  recorder compares its three recordings with the UUID replaced, and the test does the same.

```
XITE_TESTS=... python3 -I oracle/tools/gltf_fetch.py record   # three pinned recordings per world, written only when identical (run the groups one after the other: they share /tmp/gltf-fetch)
cargo nextest run -p x3d-viewer --test gltf_fetch
```

The baselines are `oracle/gltf-fetch-baseline/gf-*.json`. The scene text of every dumped frame and the sorted `log` are compared exactly. Measured in the
pinned page (also pinned in the test):

- A glTF or GLB whose buffer fails (404, bad base64, a data URL, an invalid URL, a directory) rejects `createX3DFromURL` with `Couldn't load X3D file.`.
  Each failing buffer logs its own `Couldn't load URL '<buffer>'. Error: Not Found` (`Couldn't load data URL. TypeError: Failed to fetch` for a data URL),
  and the candidate's FileLoader logs nothing for it: the rejection has no error (`FileLoader.loadDocument`, `reject ()`, and `printError (undefined)`
  returns). It does not count as an attempt, so a later failing candidate says `trying URL 3 of 3` where the list has four entries
  (`gf-buffer` 35). An invalid buffer URL is the exception: `new URL` throws before any fetch and the candidate logs `TypeError: Failed to construct 'URL'`.
- An image or video that fails to load does not fail the scene: the `ImageTexture` / `MovieTexture` logs its own error afterwards (`ImageTexture: Couldn't
  load URL ...`, `Error loading movie ...`).
- An image with neither `uri` nor `bufferView` gets the url `"undefined"` (JavaScript turns `undefined` into a string), also for `EXT_lights_image_based`
  faces and `EXT_texture_video` sources.
- A texture with `EXT_texture_video` but no `images` array rejects with `TypeError: Cannot read properties of undefined (reading 'undefined')`.
- A `MovieTexture` whose video url is a PNG file (a 200 response that is no video) leaves `createX3DFromURL` unsettled for the whole 120 frames, with nothing logged (measured once, alone in its world); it is left out of the cases because a later list would wait behind it.

Known difference, pinned and not passed (`KNOWN_UNBLOBBED` in `crates/x3d-viewer/tests/gltf_fetch.rs`): where the pinned page names the blob URL of a decoded image that
fails, the engine names the file or data URL itself, as it has no blob URL. The test fails when the engine prints any other name.

## FileLoader at the createX3DFromURL level (sub:io.file-loader)

`oracle/synthetic-rea-file-loader/` holds authored text inputs (VRML97, classic X3D, XML, JSON, byte order marks, UTF-16, Latin-1, bad UTF-8, CR and
CRLF lines, headers, empty files, gzip and zlib variants) and three worlds made by `oracle/tools/gen_file_loader.py`: `ff-text` (what the response
holds), `ff-url` (url resolution, schemes, `data:` and `ecmascript:` candidates, the MFString fallback order, the failure text) and `ff-inline`
(Inline and EXTERNPROTO urls inside a loaded scene, which resolve against that scene). Like the GoldenGate worlds, a Script loads its lists one after
the other with `Browser.createX3DFromURL`; the scene head and `toVRMLString`, or the rejection, are in the scene text, and the first line of every
`console.error` / `console.warn` of FileLoader.printError is in the sorted `log`.

```
XITE_TESTS=... python3 -I oracle/tools/file_loader.py record   # three pinned recordings per world, written only when identical
cargo nextest run -p x3d-viewer --test file_loader
```

The baselines are `oracle/file-loader-baseline/ff-*.json` (Chrome 156.0.8078.47). No GPU is used, so the test runs on a hosted runner.

Three more groups (2026-11-04, agent D2, sub:io.file-loader; each recorded three times, identical):

- `ff-inline-data` (static world, 48 DEF-named Inline nodes, script key `dumpInlines`): `data:` URLs with a byte order mark in each form (`%EF%BB%BF`,
  the three Latin-1 characters `ï»¿` raw and as `%C3%AF%C2%BB%C2%BF`, U+FEFF, in base64, gzip, UTF-16, XML, JSON, classic, twice, after a space, after a
  header line, in a fallback list), and 9 EXTERNPROTOs with a data URL first and a plain one second (a failed first candidate is a warning in the log;
  one of them is a file no parser handles, behind a URL, whose message names the URL). The scene text of every Inline and the log are compared. Pinned page: FileLoader `dataAsString` percent-decodes
  the text and removes only a leading `ï»¿` (`data.replace (/^ï»¿/, "")`, three characters, not U+FEFF), and hands the STRING to the parsers, so a real
  U+FEFF at the start of a text data URL is no `#VRML` header (`No suitable file handler found for 'data:' URL.`) and the next candidate is tried;
  a base64 URL is fetched, and the decoded bytes lose a BOM. A U+FEFF alone is an empty scene (`PROFILE Full`).
- `ff-host` (67 `createX3DFromURL` lists): a space in the host of an absolute URL. Chrome parses it (`https://exa mple.invalid/x.wrl` has the host
  `exa%20mple.invalid`; also for U+00A0, U+2000 to U+200A, U+202F, U+205F and U+3000, which IDNA maps to a space, and for a `%20` escape), so the
  fetch fails with `TypeError: Failed to fetch`, where the URL standard (the Rust `url` crate) rejects the host. The log names the URL with the
  escape decoded (`decodeURI`). Other host characters, the user information (any scheme: `Request cannot be constructed from a URL that includes
  credentials: <href>`), `foo://` (opaque host: Invalid URL) and the characters both parsers reject are in the group too.
- `ff-host-nodes` (static world): the same space in the url of Inline, Script, EXTERNPROTO, ImageTexture and AudioClip (the log).
  MovieTexture is left out: its failure text comes from a `<video>` element, which x3d-drive has no equivalent of. Two EXTERNPROTOs with the same
  first url make the pinned page log `Cannot read properties of null (reading 'setWorldURL')` (its scene cache holds a null promise) or hang; not compared.

Chrome measurements behind the host rules (Chrome 156.0.8078.47, a sweep of U+0000 to U+007F in the host of `http`, `foo`, `file` and `ws` URLs against the
`url` crate 2.5.8): the only differences are the space (accepted as `%20`; also from IDNA), `*` (becomes `%2A`; the crate keeps it), and `#` or `?` in
the host of a `file:` URL (Chrome: Invalid URL; the crate starts the fragment or query). The last two are pinned in
`crates/x3d-runtime/src/loader.rs` (`host_differences_that_are_not_fixed_are_pinned`). A label with a space and other non-ASCII characters is
encoded after the space became `%20` (`exä mple` is `xn--ex%20mple-w2a`); with a space that IDNA maps (U+00A0) and another non-ASCII character
Chrome's punycode differs (`a<U+00A0>ä` is `xn--a%20-wia`, `a ä` is `xn--a%20-ooa`): not reproduced (`Invalid URL`), pinned by a unit test.

Left out on purpose:

- `#`, `#Frag`, `?x` and a blank url resolve to the world itself, whose Script would load the same lists again without end.
- HTTP redirects, response headers (`Content-Type`, `Content-Length`, `Content-Encoding`) and the foreign (`text/html`) hand-off of
  `Browser.loadURL`: the recorder's static server answers by extension only, and a headless page opens no window. Still open (2026-11-04): the Rust
  `Fetcher` returns bytes or an error text only, and the workspace has no HTTP client, so a redirecting server with headers needs a design decision
  (extend `Fetcher` with a response; add a client or a route table) before it can be measured on both sides.
- `GoldenGate.addParsers` / `removeParsers`: the port has only the per-call `x3d_parse::ParseOptions { vrml1 }`, no registration API of parsers
  (neither in the SAI nor in the Script API), so there is nothing to compare yet.
- `http://exa mple.invalid/x.wrl`: fixed (see `ff-host`).
- The sorted `log` is sorted in the page, where the server is `http://127.0.0.1:PORT`, before that is replaced by `http://oracle`. A case must not
  create a log line that begins `http://` followed by a character between `1` and `o` (the invalid hosts use `https://`).
- A malformed percent escape in a path (`text/missing%.wrl`) reaches the recorder's server, which answers 404 (`oracle/x_ite-anim.mjs` used to stop on it).

## Platform recordings of the pinned page (Apple M1, Metal)

The pinned X_ITE 16.5.0 gives other pointing, collision and touch results on ANGLE Metal than on ANGLE/OpenGL on the NVIDIA host (owner
decision 2026-11-04, issues #69 and #89; `docs/metal-reference-variation.md`). The NVIDIA recordings stay the reference of every test and are
never overwritten. `oracle/<stem>-apple-m1-baseline/<case>-msaa<N>.json` is the recording of the SAME pinned page on the physical M1 (three identical
runs), kept only for a case where it differs from the NVIDIA recording. A test reads it only when `X3D_REFERENCE_PLATFORM=apple-m1` is set
(`crates/x3d-viewer/tests/common/reference.rs`): `js.json` comes from the platform recording where there is one, the pictures (`js/*.png`) always
from the NVIDIA recording. An unknown platform name stops the test. Rust is never compared with a recording that Rust made.
Exceptions to "the pictures always come from the NVIDIA recording": `oracle/rea-b21-wide-lines-apple-m1-baseline/` also holds
`<case>.<frame>.png`, the M1 page's picture of a frame where it differs from the NVIDIA picture by more than the test's rule
(`record_platform_particles.py` keeps it only then, after three identical runs); `rea_b21_wide_lines` compares Rust with it on the M1.
`oracle/rea-b22-two-layer-shadows-apple-m1-baseline/` is the same for `rea_b22_two_layer_shadows` (its rule: more than 0.5% of the pixels
over 8, or more than 16 in one layer's half): 20 pictures of `tl-shared-w6`, no `js.json`/`js.f32` (the six M1 records equal the NVIDIA ones).

`oracle/synthetic-metal-coverage/` (made by `oracle/tools/gen_metal_coverage.py`, not committed: about 2 MB that the Review gate cannot cover, so the test and
the recorder generate it and `worlds.sha256` beside the recordings pins the bytes) holds minimal newly authored worlds whose probes sit on pixel
centres; `oracle/metal-coverage-<platform>-baseline/<case>-msaa<N>.json` is the pinned page's coverage of them on that machine
(`oracle/tools/record_metal_coverage.py`, three identical runs each, `cases.txt` and `PROVENANCE.json` beside them), used by
`crates/x3d-viewer/tests/metal_coverage.rs` on a Metal device. These have no NVIDIA counterpart; they measure the target's rasterizer.

```
XITE_CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" XITE_ANGLE_BACKEND=metal XITE_TESTS=<dir with node_modules/playwright> \
  python3 -I -B oracle/tools/record_platform_baseline.py --platform apple-m1 --base rea-b09-nav-baseline --hardware "Apple M1, macOS 27.0.1" --names FILE
X3D_BACKEND=metal X3D_REFERENCE_PLATFORM=apple-m1 cargo nextest run -p x3d-viewer --release -E 'test(rea_batch09_nav) | test(rea_b24_nav) | test(rea_batch12) | test(rea_batch09_inputs)'
```

A difference between Rust and the M1 recording is investigated as a possible Rust defect; no platform tolerance hides it.

## Classic encoding edge cases (sub:parse.vrml-classic)

`oracle/tools/classic_edge.py` holds the authored inputs, `oracle/x_ite-classic-edge.mjs` records the pinned page, and
`oracle/classic-edge-baseline/classic-edge.json` is the baseline (the input of each case is in it, so the Rust test reads no other file). Every
input is newly written (no historical Cybertown or blaxxun content). A `text` case goes through `Browser.createX3DFromString` (the parser's own
behavior, the page URL as world URL); a `b64` case is a file of exact bytes loaded with `Browser.createX3DFromURL` (byte order marks, UTF-16,
Latin-1 and bad UTF-8, CR / CRLF lines, NUL and Ctrl-Z bytes, gzip, truncated and nested gzip, other formats that look like VRML).

The groups: headers (`#VRML`, `#X3D`, versions, case, trailing text, byte order mark, CR); PROFILE, COMPONENT, UNIT and META statements in every
order; comments and separators in odd places (between a field name and its value, inside lists and strings, inside ROUTE / PROTO / Script
syntax; `\f`, `\v`, U+00A0 and other characters that are not whitespace); numbers of every field type (hex, exponents, signs, NaN and Infinity,
overflow, float32 rounding, SFImage pixels); strings and escapes; MFNode / MFString / MFInt32 brackets with and without commas, NULL, and nodes in
the wrong place; DEF and USE names (every character class of the Id grammar, reserved words, unicode, reuse, forward use); IS, access type
spellings (`eventIn`, `exposedField`, `inputOnly`, ...) and field name aliases (`set_`, `_changed`); ROUTE spellings, aliases, bad targets and
type / access mismatches; PROTO and EXTERNPROTO syntax; IMPORT and EXPORT; errors (unterminated strings, unknown nodes and fields, missing
braces, the error text at every column around the 80 character cut, with tabs, CR lines and UTF-16 surrogate pairs); a printing sweep of every list
type at lengths 0 to 30 and every single type; sweeps of every node of the pinned node table under each profile, specification version and
component level; XML and JSON escaping; legacy GeoViewpoint.

Per case the Rust result must equal the pinned one exactly: the verdict and the rejection text; `toVRMLString`, `toXMLString` and
`toJSONString`; encoding, specification version, profile, components, units, meta data and root node count; and the console output with its level,
in print order (every `log`, `warn` and `error` line; the recorder leaves out X_ITE's own `info` progress notices, `Done loading ...` and
`Initializing ...Shader`, whose order depends on the module loads). JSON cannot carry a lone UTF-16 surrogate, which X_ITE's error text can have
(it cuts a long line with `substring`); the recorder writes each as U+FFFD, and so does the port.

```
XITE_TESTS=... python3 -I oracle/tools/classic_edge.py record   # three pinned recordings (side by side), written only when identical
cargo nextest run -p x3d-parse --test classic_edge
cargo nextest run -p vrml97 --test classic_edge_facade
```

A difference with a principled cause is in `crates/x3d-parse/tests/classic-edge-known.tsv` with a hash of the port's output; the test fails when the
port's output changes and when the case matches (a stale entry). The entries today: the console lines that X_ITE prints when a node's resource loads
after the parse (Inline, AudioClip, Script url, the import of an Inline; these run in `x3d-runtime`, the parse-only test has no loads), and the
Annotation component (the node table of the pinned source has it, `SupportedComponents.js` has it commented out, so the pinned `COMPONENT
Annotation:1` is an error; the port supports it by the owner decision of REA batch 04).

Not covered: the runtime side of those loads, Script execution, and `x3d-drive` end to end (the file-loader and golden-gate baselines do that for
URL lists). A case that hangs the pinned page (a PixelTexture inside a PROTO, `PROTO P [ inputOutput SFImage i 1 1 1 0 ] { PixelTexture { image IS i } }`)
was left out: the pinned page did not finish it in 30 s (cause not investigated, so it is UNVERIFIED whether the parser is involved).

## Text under InstancedShape (`node:InstancedShape`)

- `tools/gen_instanced_text.py` writes `synthetic-instanced-text/` (authored, no historical assets): `it-static` (a plain Shape with Text and
  InstancedShapes with Text: translations, scales, rotations, scaleOrientations, centers, FontStyle, `length`, `maxExtent`, no instance, a
  transparent Material), `it-extra` (PixelTexture, `solid TRUE` under turned instances, `bboxDisplay`, explicit bbox, empty string,
  `visible FALSE`), `it-pick` (TouchSensor over instanced Text, 2601 `pickProbes`) and `it-runtime` (`string`, FontStyle, instance fields and
  `geometry` changed by key events, 11 pictures). All use `settle`, so the default font arrives between two frames in X_ITE and Rust alike.
- `tools/rea_instanced_text.py record` wrote `instanced-text-baseline/NAME-msaa{0,4}.json` and `instanced-text-baseline/png/NAME-msaaN/`:
  each case three times at MSAA 0 and at MSAA 4; the scene text, the console and every picture must be identical. The pinned X_ITE
  needs the `x_ite-pin/node_modules` link of "Recording from a worktree". The pinned page draws Text glyphs under InstancedShape (with
  `settle`; the earlier "no glyphs" note was a run without it).
- `crates/x3d-viewer/tests/instanced_text.rs` compares `x3d-drive` with the baseline (scene text exact, `pickProbes` bit for bit, pictures
  with at most 0.5% of the pixels more than 8 off) and runs four negative controls (a changed glyph, a scale, a moved instance, a changed
  run-time FontStyle size). GPU test; it skips without an adapter.
- Observed, not compared: in the pinned page, assigning to an InstancedShape's `geometry` at run time a node that another InstancedShape
  already uses leaves the old geometry on screen (reproduced with Cylinder and with Text; a geometry used by a plain Shape works). The cause
  is not found in `InstancedShape.js`, `X3DShapeNode.js` or `X3DGeometryNode.js`. The worlds avoid that operation.

## Plain Inline reload and abort paths against the browser shutdown rule (2026-11-04)

`oracle/synthetic-inline-abort/` (four newly authored worlds, `x_ite-inline.mjs`) holds Inline changes that start inside the completion event
of the load (a LoadSensor drives a Script that sets the Inline's `url`, switches `load` off, or switches it off and on). The pinned page was recorded
three times, byte-identical, into `oracle/synthetic-inline-abort-baseline/inline-abort.json`. `crates/x3d-script/tests/inline_abort_handlers.rs`
and `inline_reload_shutdown.rs` (the 38 scenarios of `synthetic-inline-reload`) compare every step in the browser build and in the native build.
The browser build equals the pinned page in the console, the load count, the Inlines and the scene text, and prints no Script `shutdown`. The
native build differs only by the pinned `shutdown ()` lines (owner-approved, ISO/IEC 14772-1 4.12.3): each affected step has its complete native
console in a table (`PINS`), so any other line, a missing or a moved `shutdown` line fails. `X3D_PRINT_PINS=1 cargo nextest run ... --no-capture`
prints the table. Four scenarios of `synthetic-inline-reload-known.tsv` differ for another reason (`c07`, `c09`, `c29`, `c35`); the test lists them
by name, so a fifth cannot be added unnoticed.

## FileLoader over HTTP: the Fetcher response contract (sub:io.file-loader)

`Fetcher::fetch_response` (crates/x3d-runtime/src/loader.rs) gives FileLoader the parts of a fetch `Response` that the pinned `FileLoader.js` reads:
final URL, status, reason text, headers, redirected flag and body. The default is `fetch` with status 200 and no header, so a fetcher that has only `fetch`
still works. `Content-Type` of the response decides the hand-off of `Browser.loadURL` and Anchor: `text/html` and `application/xhtml+xml` (exact text before
the first `;`) go to `foreign (resolvedURL.href, target)`, and the href is the URL that was requested, not `response.url` after a redirect. The HTTP and
HTTPS fetcher is the crate `x3d-http` (ureq with rustls; docs/http-client-admission.md). It follows redirects itself, at most 20, and refuses a redirect to
a scheme other than `http` and `https` and to a URL with credentials; it has limits for the body size and for time that a browser does not have.

`oracle/synthetic-rea-file-http/` holds `routes.json`, a table of routes for a local HTTP server, and four worlds, all made by
`oracle/tools/gen_file_http.py`. The recorder `oracle/x_ite-anim.mjs` serves the table from Node on two ports of 127.0.0.1 (script key `httpRoutes`; the
second port is a second origin) and the test serves the same table from `x3d_http::route_server` (std only). `x3d-drive --http-map http://oracle=ORIGIN`
sends the requests under `/h/` to that server over HTTP. The recorder also records the hand-off (script key `foreign`: `window .open` is replaced, the
`navigate` event of `location = href` is recorded and cancelled), as `foreign` in the result, and `x3d-drive` writes the same key.

| group | content |
| --- | --- |
| `fh-redirect` | 301, 302, 303, 307, 308; relative, absolute, protocol-relative and query `Location`; chains; a loop; 20 and 21 hops; a 3xx with no or an empty `Location`; 300 and 304; redirects to `file:`, `data:`, `ftp:`, `javascript:`, `ecmascript:`, `blob:`, `about:`, `ws:`, a URL with credentials, a bad URL, a closed port; a redirect across origins with CORS headers; fragments; fallback lists |
| `fh-status` | 400, 403, 404, 410, 500, 502, 503, 204, 201, an empty 200; eleven content types; custom and repeated headers; `Content-Encoding: gzip` once, twice and wrongly; a gzip file; a cut body; a closed connection |
| `fh-foreign` | `loadURL` with `text/html`, `application/xhtml+xml`, a charset, `TEXT/HTML`, a space before the `;`, no type, gzip, a `.html` URL, redirects to HTML, a `target` parameter, fallback lists, fragments |
| `fh-diff` | the cases where the port differs (below) |

```
XITE_TESTS=... python3 -I oracle/tools/file_http.py record     # three pinned recordings per world, written only when identical
cargo nextest run -p x3d-viewer --test file_http
cargo nextest run -p x3d-http -p x3d-runtime --test http_fetcher --test fetcher_response
```

The baselines are `oracle/file-http-baseline/fh-*.json` (Chrome 156.0.8078.47). No GPU is used. What the recordings showed, and the port matches:

- The world URL, and the base of the relative URLs in the loaded scene, is the requested URL, not the URL after the redirects.
- An empty `Location` is no redirect (`Error: Found`). A redirect to `#Frag` is a loop and fails. 20 redirects are followed, the 21st fails.
- A body that cannot be read to its end (cut off, or `Content-Encoding: gzip` on a body that is not gzip) rejects with `TypeError: network error`; a
  closed connection before the response head rejects with `TypeError: Failed to fetch`.
- A response with an unread body keeps the recorder's request counter above zero (`settle` would wait for ever), so the routes that are handed off
  or that fail have empty bodies, and a body that is read must not parse (an empty body is an empty scene, and `loadURL` would replace the world).

Known differences, pinned by `known_differences_are_pinned` (a change of either side fails it):

- The page refuses a redirect to another origin when the answer has no `Access-Control-Allow-Origin`. A native program has no origin, so the port follows.
- The page shows the reason phrase that the server sent (`Error: Gone Fishing`, `Error: 404` for an empty phrase, `Error: I'm a Teapot`). The HTTP client
  keeps no phrase, so the port shows the canonical phrase of the status.

Not compared: `Content-Encoding` values other than gzip (the port asks for gzip only), the progress fractions that `X-File-Size` and `Content-Length`
feed, a body above the size limit and the time limits (the page has none; `crates/x3d-http/tests/http_fetcher.rs` tests them), HTTPS and a certificate
failure (the recorder has no TLS server), and the web build, whose `WebFetcher` does not report the headers of the page's `fetch` (no hand-off by content type there).

### Malformed UTF-8 in a text `data:` URL (`ff-data-utf8`, and 19 cases of `ff-inline-data`)

FileLoader runs `$.try (() => decodeURIComponent (data)) ?? data`: one malformed sequence (`%FF`, a lone `%C3`, `%C3%28`, an overlong `%C0%80`, a
surrogate `%ED%A0%80`, a five-byte `%F8...`, a continuation byte `%80`) makes `decodeURIComponent` throw, and then the whole string stays undecoded, every
other escape too. The recordings confirm it for `createX3DFromURL` (`ff-data-utf8`: 24 lists, also as the first entry of a fallback list) and for Inline
(`ff-inline-data`, cases `utf8-*`), with and without a byte order mark before the malformed sequence. The port matches (`decode_uri_component` returns
`None`, and `data_url_text` keeps the string); it needed no change. A decoder that replaces the malformed bytes by U+FFFD fails both tests.

## Web Anchor pages 01: pages, targets, unreachable URLs (node:Anchor)

- `x_ite-web-runtime-02.mjs --suite anchorpages01` runs the 25 cases of `crates/x3d-web/tests/browser/anchorpages01_case.mjs`
  (fixtures in `crates/x3d-web/tests/browser/worlds/anchor/`) and records
  `web-browser-anchor-pages-01-baseline/anchorpages01-baseline.json` (no pictures). Each case clicks an Anchor with the real mouse
  and records the `window.open` arguments (a recorder replaces it), the main-frame navigation requests, the server's requests, the
  browser events, whether the world is the same, and the engine's console lines.
- Three invocations of `--runs 3` (run at the same time) gave identical values (Chrome 156). Known X_ITE_R differences, in
  `KNOWN_DIFFERENCES`: `window.open` gets the feature `noopener`; no `Done loading scene` info line.

```
node oracle/x_ite-web-runtime-02.mjs oracle/web-browser-anchor-pages-01-baseline --runs 3 --suite anchorpages01
```

## Touch input into pointing device sensors (sub:pointer.picking)

- `tools/gen_rea_pointer_touch.py` writes `synthetic-rea-pointer-touch/` (a world with a TouchSensor, PlaneSensor, CylinderSensor,
  SphereSensor and an Anchor to a Viewpoint, a Script that logs every sensor output event into the string `hist`, and 6 event scripts).
  `tools/rea_e14_e18_record.py record --cases oracle/synthetic-rea-pointer-touch/cases.txt --dest oracle/rea-pointer-touch-baseline`
  records the pinned X_ITE three times (identical, touch split 0) with Chrome DevTools touch events (`x_ite-anim.mjs` now also sends
  `touchcancel`, DevTools `touchCancel`). Prepare the worktree as in "Recording from a worktree".
- `crates/x3d-viewer/tests/rea_pointer_touch.rs` compares `x3d-drive` (which feeds `touchstart`, `touchmove`, `touchend`, `touchcancel`
  to the viewer, as the window and the browser build do) with it: taps, drags, leave and enter, a second finger and the EXAMINE viewer
  against a press are equal in scene text (the `hist` of every frame) and pose.
- Rust defect fixed: a touchend (and the two-finger touchstart that ends the press) is a TouchEvent without `pageX` and `pageY`, so the
  pinned `PointingDevice.touchend` releases with a NaN pointer: the release does not pick again and the verifying motion hits nothing
  (TouchSensor `isOver` ends at once). Rust released at the touch position (the hit values were sent again, `isOver` stayed until the
  next press). `mouseup` also verifies the motion when no sensor was active.
- Kept difference, pinned by name (case `cancel`, ADR 0001 row 1): X_ITE has no `touchcancel` handler, a pressed sensor stays active;
  X_ITE_R ends it.

## Touch navigation of WALK and FLY (sub:nav.walk-fly, F16)

- `tools/gen_rea_nav_touch.py` writes `synthetic-rea-nav-touch/` (three worlds: WALK with gravity and collision, WALK with `Gravity` 0,
  FLY; each with a floor, a low box (stepped onto with gravity), a wall, a TouchSensor box and a Script whose string `hist` lists the
  TouchSensor events and the ProximitySensor activity and event count; 8 event scripts each, 24 cases).
  `tools/rea_e14_e18_record.py record --cases oracle/synthetic-rea-nav-touch/cases.txt --dest oracle/rea-nav-touch-baseline` records the
  pinned X_ITE three times (identical, touch split 0; a touch event that lifts or presses two fingers at once reaches the page as two
  events, so the scripts lift and press one finger per event). Prepare the worktree as in "Recording from a worktree".
- `crates/x3d-viewer/tests/rea_nav_touch.rs` compares `x3d-drive` with it on every frame (pose 1e-4, scene text exact): one finger
  driving forward, back, turning and diagonally (into the box and the wall), two fingers looking around, one to two to one to three
  fingers, a touch that ends while moving, a finger on a TouchSensor, and a two-finger pitch followed by a drive. All are equal.
- Kept difference, pinned by name (cases `*-cancel-during-move`, ADR 0001 row 1): X_ITE's viewers have no `touchcancel` handler, so a
  drive goes on after a cancel (until a touchend, or a two-finger touchstart, reaches the viewer); X_ITE_R ends it with the cancel.
- `walk-nograv.wrl` sets `Browser .setBrowserOption ('Gravity', 0)` in a Script (the option is a `BrowserOptions` field, as in `rea-b24-nav`); the
  viewpoint starts 1.4 above the ground, so `walk` falls to the avatar height and `walk-nograv` and `fly` stay put (different in the pinned X_ITE).
- Not compared in the gravity world (`walk-*`): the count `PX_n` of ProximitySensor events. A standing avatar is settled by the f32 depth read
  from the GPU, whose noise (about 2e-7) moves the resting pose from frame to frame in X_ITE_R and not in the recording, one extra event per
  wobble; the poses (1e-4) and the rest of the scene text are compared. `walk-nograv` and `fly` compare it exactly.
- The `cancel-during-move` cases also compare the log (equal), the scene text before the cancel, and assert the deliberate difference in the
  event count (X_ITE goes on driving, so it has more `position_changed` events).

## glTF animation cases (sub:parse.gltf2)

`oracle/synthetic-rea-b08-gltf/anim/*-driver.wrl` (generator `tools/gen_rea_b08_gltf.py`) load one animation file through an
Inline; a click at frame 4 starts the exported `Timer1`; `anim/gltf-anim.json` dumps the scene text and the Inline scene text at nine
frames inside the animation. Cases: STEP, LINEAR and CUBICSPLINE of translation, rotation, scale and weights; the STEP pointer
animation (`pointer/roughness-step.gltf` does not load in the pinned X_ITE: `Cannot set properties of null (setting '_emissiveStrength')`,
so `roughness-step-plain` and `base-color-step-plain` animate a plain material); each at MSAA 0 and 4.
- Recorded with `oracle/tools/rea_e14_e18_record.py record --cases CASES --dest oracle/rea-gltf-anim-baseline --pictures` (three identical
  pinned runs each; prepare the worktree as in "Recording from a worktree"). `crates/x3d-viewer/tests/gltf_anim_step.rs` replays them and the
  three driver cases of `rea-b08-gltf-baseline`: scene text and Inline scene text exact (MSAA 0 only: the MSAA 4 page records neither), poses
  1e-4, pictures 8 / 0.5%. The pinned page's `console` holds only `log:` lines (anim.html replaces warn and error), so only those are compared.
- Pinned difference, not copied: STEP morph weights. Pinned `createArrayInterpolator` writes keys `[t0, t1, t1, t2, t2]` with values
  `[v0, v1, v1, v2, v2]` (linear over the first segment) where glTF 2.0 and the pinned `createNamedInterpolator` hold `[v0, v0, v1, v1, v2]`. The
  port holds. `KNOWN` in `gltf_anim_step.rs` (weights-step, MSAA 0 and 4: Inline text and pictures 14 and 26 not compared; the held points are
  asserted) and `KNOWN_STEP_WEIGHTS` in `crates/x3d-parse/tests/gltf_rea.rs` name it.

## CFF items: subroutine bias and depth, `seac`, variable CFF2 (`rea-t-cff-items-baseline/`)

- `tools/gen_cff_items_fonts.py` writes `synthetic-rea-t-cff-items/` (12 newly authored fonts, every table byte written by the script):
  `bias-1239`, `bias-1240`, `bias-33899`, `bias-33900` (N local and N global subrs: Type 2 bias 107, 1131, 1131, 32768), `depth` (local, global and
  alternating chains of 9 to 13 subrs and self-calling subrs), `seac-plain`, `seac-cid`, `seac-cid-fd` (`endchar` with four arguments), and
  `var-default`, `var-regular`, `var-avar`, `var-2axes` (CFF2 with `fvar`, `avar`, HVAR and `blend`).
- `tools/rea_t_cff_items.py record` runs `x_ite-cff-cid-fonts.mjs` (`FONT_DIR`, `FONT_CHARS`) three times against the pinned X_ITE and keeps
  `fonts.json` only if the three runs are identical. Prepare the worktree as in "Recording from a worktree".
- `crates/x3d-text/tests/rea_t_cff_items.rs` compares `x3d-text` with it glyph by glyph (advances, `getPath` commands, libtess vertices, exceptions,
  console lines).
- Pinned facts the test pins: opentype.js applies the Type 2 bias and the nesting limit of 10 (the 11th call is skipped with a `console.warn`);
  a CID-keyed font cannot resolve the components of `seac` (TypeError), and a plain font whose accent glyph is missing throws too; a font with `fvar`
  and CFF2 outlines gets a variation manager, whose default instance is the first instance equal to the axis defaults, else the instance named
  "Regular", else the axis defaults, so a charstring `blend` uses the `avar`-mapped normalized coordinates and every path coordinate is rounded
  (`Math.round`); X_ITE calls `glyph .getPath (0, 0, 1)` without the font, so HVAR advances are never applied. X_ITE has no way to set other coordinates.
