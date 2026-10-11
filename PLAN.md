# Rust port plan — X_ITE and CodeMirror 5

Written 2026-10-11. Plan only. No Rust code exists yet.

## Hard rules for this lane

- **Private until proven.** Nothing from `scratch/rust-port/` goes to GitHub or any
  other remote until Ryan says every part works. No `git remote add`, no push, no
  gist, no issue, no PR. `scratch/` is gitignored by the lane root (`.gitignore:14`).
- The upstream copies under `upstream/` are read-only references. Never edit them.
  `upstream/codemirror5` still has its `origin` remote; never push from it.
- Do not touch `X_ITE/x_ite` (the fork). It is on `fix/firefox-vrml-xml-parser`
  and belongs to the upstream-contribution lane. Read it only.
- No Cybertown or blaxxun assets in Rust test fixtures that could ever ship.
  Corpus files stay in `recon/fixtures/` and are read by path.

## Inputs (pinned)

| project | path | version | size | license |
| --- | --- | --- | --- | --- |
| X_ITE | `X_ITE/x_ite/src` | 16.5.0, branch `fix/firefox-vrml-xml-parser` @ `34cdf9f97` | 764 JS files, ~129k lines | MIT |
| CodeMirror 5 | `scratch/rust-port/upstream/codemirror5` | 5.65.21 @ `4ca3d05` | core `src/` 10.4k lines; `mode/` 38k (123 modes); `addon/` 8.7k; `keymap/` 1.3k | MIT |

Both are MIT, so a Rust port may keep MIT. Keep both copyright notices.

## Layout (when work starts)

```
scratch/rust-port/
├── PLAN.md               this file
├── upstream/             read-only reference sources
├── Cargo.toml            one workspace
├── crates/
│   ├── x3d-fields/       SF*/MF* field types, math (Vector/Matrix/Rotation)
│   ├── x3d-parse/        VRML classic + XML + JSON parsers
│   ├── x3d-scene/        nodes, execution context, routes, PROTO/EXTERNPROTO
│   ├── x3d-render/       wgpu renderer + shaders
│   ├── x3d-browser/      browser loop, navigation, loading (native + wasm)
│   ├── cm-core/          CodeMirror document model (no UI)
│   ├── cm-modes/         mode API + ported modes
│   └── cm-view/          CodeMirror view + input
└── oracle/               scripts that run the original JS and dump results
```

A local `git init` inside `scratch/rust-port` (no remote) is fine for history.

## How we prove it works: the oracle

Every phase compares the Rust output to the original JavaScript output.
The JS is the "oracle" — the answer key.

- X_ITE: reuse `scratch/qa/run.mjs` (headless chromium, `Cache-Control: no-store`,
  threaded server). Add a dump mode: parse a scene, print a canonical text dump of
  the node tree, fields, and routes. The Rust side prints the same dump. Diff them.
  Later, compare screenshots with a pixel tolerance.
- CodeMirror: the upstream `test/` suite (`test.js`, `doc_test.js`, `mode_test.js`,
  …) runs in headless chromium. Port the test cases to Rust tests one by one.
  For modes, `mode_test.js` already encodes "text in → token styles out". That is
  a perfect diff target.
- Corpus: the 14,607 Cybertown files indexed in `recon/index/` are the real
  acceptance test for the VRML parser. Read via `recon/scan.py` rules (gzip in memory,
  `rb`, never write to the archives).

## What REA does here (and what it does not)

REA is built for programs that ship *without* source. Here we have the source, so
REA does not drive the port. It helps in two places:

1. **Inventory.** `analyze_javascript_application` maps modules and call flow.
   First run on CodeMirror `src/` (evidence `ev_0f874a7a…4162`): 67 files parsed,
   0 parse failures, but coverage is **partial** — the result budget ran out before
   `model/Doc.js`, `model/history.js`, `model/selection.js` and others got semantic
   analysis. Use it per-folder, not whole-tree. The X_ITE whole-tree run
   (evidence `ev_dbd2afb6…bb0f`) parsed 765 JS files with 0 parse failures, but is
   also **partial**: 1,280 application unknowns, 15,704 semantic unknowns, 1,297
   limitation notes. Same lesson — scan one folder per phase.
2. **Proof ledger.** `build_reconstruction_obligation_ledger` and
   `evaluate_reconstruction_coverage` keep a checklist: one obligation per behavior,
   each tied to an original case, a Rust owner, and a passing verifier. A phase is
   "done" only when its ledger closes with no open unknowns.

## Decisions (made by Ryan 2026-10-11)

- Render target: **wgpu**.
- CodeMirror view: **native desktop first** (C5 targets a native window, not wasm/DOM).
- Script engine: **rquickjs** (QuickJS).

### Original options, for the record

1. **Render target for X_ITE.** Recommended: `wgpu` (one renderer for native
   Vulkan/Metal/DX12 and browser WebGPU/WebGL2 via wasm). Alternative: raw WebGL2
   via `web-sys`, browser only.
2. **UI target for CodeMirror view.** Recommended: wasm + DOM (`web-sys`) first, because
   CM5's view logic is built around DOM measurement; a native view (egui or a custom
   wgpu text view) later. Alternative: native first.
3. **Script node engine.** X_ITE `Script` runs ECMAScript. Recommended: `rquickjs`
   (QuickJS; small, fast, mature). Alternative: `boa` (pure Rust, slower, less complete).

## X_ITE phases

Ordered so each phase gives something testable on the Cybertown corpus.

| # | phase | JS sources | ~lines | done when |
| --- | --- | --- | --- | --- |
| X0 | Oracle + dump format | `scratch/qa/` | — | JS dump is stable across 2 runs on 50 corpus files |
| X1 | Fields + math | `x_ite/Fields`, `x_ite/Base`, `standard/Math` | 14k | field parse/print round-trip matches JS on all field types |
| X2 | VRML classic parser | `Parser/VRMLParser.js`, `X3DParser.js`, `Expressions.js`, `GoldenGate.js` | ~4k | node-tree dump matches JS on every corpus file the JS parses; same errors on the rest |
| X3 | Scene model | `Execution`, `Routing`, `Prototype`, `Configuration`, `Components/Core` | 10k | PROTO/EXTERNPROTO expansion and ROUTE tables match JS dumps |
| X4 | Loading | `InputOutput/FileLoader.js`, `lib/helper.js`, `Networking` | 3k | gzip, MFString fallback, Inline resolution match JS on corpus |
| X5 | Event engine (no pixels) | Time, Interpolation, Followers, EventUtilities, sensors | 5k | event cascade trace matches JS over N simulated frames |
| X6 | Script node | `Components/Scripting`, SAI in `Browser/` | 2k+ | Cybertown script fixtures produce same event trace |
| X7 | Renderer core | `Rendering`, `Shape`, `Geometry3D`, `Lighting`, `Texturing`, `Grouping`, `Navigation`, shaders in `assets/shaders/webgl2` | 25k | screenshot diff under tolerance on 20 chosen corpus worlds |
| X8 | Remaining components | NURBS, Text, Sound, Particles, HAnim, Geospatial, Layering, Layout, Picking, Physics, Volume, … | 30k | each component: X3D example scenes match by dump + screenshot |
| X9 | Other formats | XML, JSON, glTF/GLB, OBJ, PLY, STL, SVG parsers | 10k | parse dumps match JS |
| X10 | Browser shell | `Browser/` UI, context menu, keyboard, native window + wasm page | 15k | a person can walk around a Cybertown world in both targets |

Library swaps (no line-by-line port): `ammojs` → `rapier3d`; `libtess.js` → `lyon_tessellation`
or port; `opentype` fonts → `ttf-parser` + `rustybuzz`; Web Audio → `kira` or `cpal`;
GLSL shaders → keep GLSL, compile with `naga`.

## CodeMirror 5 phases

| # | phase | JS sources | ~lines | done when |
| --- | --- | --- | --- | --- |
| C0 | Oracle | `test/` in headless chromium | — | upstream suite runs green headless here |
| C1 | Utilities | `util/StringStream.js`, `util/bidi.js`, `util/misc.js` | 0.9k | unit tests ported and green |
| C2 | Document model | `model/` (Doc, chunk B-tree, changes, history, selection, mark_text) | 2.0k | `doc_test.js` cases green in Rust |
| C3 | Lines + highlighting | `line/`, `modes.js`, mode API | 1.4k | state/tokenize API matches JS on sample text |
| C4 | Modes | `mode/` — only needed ones first: javascript, xml, css, htmlmixed, clike | 38k total | `mode_test.js` token streams match for each ported mode |
| C5 | View + input | `display/`, `input/`, `measurement/`, `edit/` | 6.2k | `test.js` view tests green in wasm build; manual edit smoke test |
| C6 | Addons + keymaps | `addon/`, `keymap/` on demand | 10k | per addon test file green |

Note: CM5 has no VRML mode. If the goal is a VRML editor, a new VRML mode is a
C4 add-on, written fresh, not ported.

## Rough effort

Line counts are not effort, but for scale: CodeMirror core C0–C5 is weeks.
X_ITE X0–X4 (parse + load, no pixels) is weeks. X5–X10 is months.

## Status

- 2026-10-11: **C0 done** — 1483 CodeMirror tests pass headless. **X0 done** —
  X_ITE dump is stable on 50 corpus files (47 parse, 3 rejected). See `oracle/README.md`.
- 2026-10-11: **X1–X2 done** — crates `x3d-fields`, `x3d-scene`, `x3d-parse`. Corpus: 45/47
  byte-exact, 3/3 rejections reproduced; 2 one-line diffs need Script + events (X5+X6),
  listed in `crates/x3d-parse/tests/x0-known.tsv`.
- 2026-10-11: **C1–C3 done** — crates `cm-core`, `cm-modes` (toml mode as proof). 77 tests
  pass; 600 random edit sequences match real CM5 exactly (13,000 more in extra runs);
  toml tokens match on 427 inputs. View-dependent upstream tests are not ported yet (C5).
- 2026-10-11: **C4 done (first set)** — javascript/json/typescript, xml/html, css/scss/less/gss,
  htmlmixed, plus a **new VRML mode** (JS for real CM5 + Rust). 0 token and 0 indent mismatches
  across ~37.7M callbacks; VRML mode checked on all 606 Mall item files. 174/174 upstream mode
  tests pass. One known divergence: htmlmixed `<pre>` `Pass + n` (JS gives a string, Rust NaN).
- 2026-10-11: **X3–X6 done (no pixels)** — crates `x3d-runtime` (scene setup, PROTO
  realize, routing/event engine, TimeSensor/interpolators/event utilities, FileLoader,
  Inline/EXTERNPROTO/texture/movie/font loads) and `x3d-script` (Script on rquickjs).
  Items 606/606 exact and 606/606 console logs; x0 47/47 exact (07-bookcase and
  43-villasantzip fixed) and 50/50 logs; x1 470/473 exact (3 wall-clock `Date` scripts)
  and 497/498 logs (1 render-time WASM trap). Known lists in `crates/x3d-script/tests/`.
- 2026-10-11: **C5 done (first version)** — crate `cm-view`: native editor (winit + wgpu +
  glyphon), binaries `cm-edit` and `cm-render`. 55 upstream view/command tests pass; 600
  command sequences match real CM5 (0 unexplained). Gaps: bidi layout, folding, widgets,
  overlays, find/replace, IME preedit, wrapping not browser-checked.
- 2026-10-11: **X7 renderer core done** — crate `x3d-render` + `x3d-shot` (wgpu 30 on Vulkan,
  offscreen, no window). Render oracle `oracle/x_ite-shot.mjs` (headless GPU Chrome, noise
  floor 0). Within tolerance (RGB delta > 8 on ≤ 0.5% of pixels): items 604/606, x0 46/47
  rendered (3 known rejections). Geometry 23/23 against the pinned JS. Gaps: Text, Fog,
  MovieTexture, background textures, libtess (ear clipping instead), NVIDIA NPOT mipmaps.
- 2026-10-11: **X10 first version** — crate `x3d-viewer`, binary `x3d-view`: live runtime,
  EXAMINE/WALK/FLY, viewpoints, Touch/Plane/Cylinder/Sphere sensors. Matches X_ITE on the
  animation, navigation and sensor oracles (`oracle/tools/x10_compare.py`). Gaps: no in-window
  text, two GPU devices, no Followers/PLANE/LOOKAT/double-click lookAt, Scripts in Inlines.
- 2026-10-11: **X8 subset** — crate `x3d-text` (fonts, libtess port), Text/FontStyle, Fog,
  Background textures: synthetic-x8 125/125. x0 43-villasantzip still over (static shot has no
  WALK gravity step; text itself is fine).
- 2026-10-11: **X10 drag/clipping + one-device viewer** — EXAMINE drag report ("picture was
  clipping"): not reproducible as a projection/near-far/pose divergence (12 stress scripts x 10
  worlds, 512x512 and 1024x768, poses to 1e-4 and images within tolerance; see
  `oracle/x10-scripts/clip-*.json`). Found and fixed a real drag bug: several mouse moves in one
  frame lost all but the last in pan (chaser `set_destination` field changes at once in X_ITE).
  The viewer now renders straight into the window surface on one wgpu device (`x3d_render::gpu::
  Renderer`, no readback/blit), keeps geometry/textures/bind groups between frames (write stamps
  on `Runtime`), and exposes `x3d_viewer::embed::EmbeddedViewer` for a host application (see
  `crates/x3d-viewer/README.md`). Offscreen path (`Gpu`) is bit-identical to before.
- 2026-10-11: **MSAA** — `x3d_render::gpu::Renderer` has X_ITE's `Multisampling` (MultiSampleFramebuffer:
  multisampled color + depth, resolved into the target): `set_sample_count`, clamp to the highest
  supported count (1/2/4/8 on NVIDIA). Viewer/embed default 4 like X_ITE (`x3d-view --msaa N`, key
  `m`); `x3d-shot` / `x3d-drive` stay at 0 unless `--msaa N`. MSAA oracle (`oracle/README.md`): with
  `--gl-samples` Rust matches X_ITE 4x/8x on 604/606 items (same 2 as MSAA off) and byte-for-byte on
  synthetic edge scenes. The orrery "tearing" was first called aliasing plus a hatch; the hatch is z-fighting (next entry).
- 2026-10-11: **Orrery z-fighting resolved (inherent to the item, X_ITE does it too).** Two sources, found with a
  debug ID-buffer (`x3d-zfight`, `Renderer::set_debug_ids`) and the same camera-jitter sequence in X_ITE
  (`oracle/x_ite-zfight.mjs`): (1) the brass and bronze rings (both instances of `RING_GEOM`) lie in the same
  plane up to 1.2e-4 rad (the bronze `rotation 0 0 1 0.61` spins the ring about its own axis, mapping the circle
  onto itself; the 1.571 approximation of pi/2 leaves a 2e-4 rad tilt), so their axial-face triangles (e.g. 20,
  68, 69, 76, 77) are 0 to 1.2e-4 apart against a 24-bit step of 5e-5 at distance 10 (the default view); (2) the
  four foot cylinders' bottoms coincide exactly with the walnut base box's bottom (y = -1.75), visible from
  below. X_ITE flickers at the same pixels (overlap 64% standard, 99% with `LogarithmicDepthBuffer`). Added
  `gpu::DepthMode` {Standard (default, X_ITE), Logarithmic (X_ITE's property, ported: 604/606 vs X_ITE log),
  Reversed (beyond X_ITE)}, `x3d-view --depth`, key `z`, embed `set_depth_mode`. Closed-spine Extrusion caps
  match X_ITE (100 triangles). Details: `oracle/README.md` "Depth fighting".
- 2026-10-11: **Examine zoom/rotate drift fixed.** Cause: X_ITE runs the chasers in the order they became
  active (the `prepareEvents` interests are a Map, delete + add appends); Rust ticked position, center,
  rotation in fixed order, so an overlapping rotation chaser overwrote the zoom chaser's `positionOffset`.
  `Chaser::activated_at` + sorted tick in `nav.rs` `Examine::prepare`. New `x10-scripts/rotate-*.json`
  (8 cases, `tools/gen_x10_rotate_zoom.py`) agree with X_ITE to 0.0 pose delta; clip suite 48/0, all
  other x10 scripts and `cargo test --release` pass. Details: `oracle/README.md` "Zoom/rotate drift".
- Test inputs: Ryan's Mall items, **`oracle/corpus-items2/` (296 `.wrl`, live tree without `.edit.wrl`, qa folders and tmp/bak files; keys `oracle/items2-baseline.json`, `render-items2-baseline`, `render-msaa4-items2-baseline`) is the primary test set** (2026-10-11: scene text 296/296 exact, pictures 294/296 at 0x and 4x; details in `oracle/README.md`). The older `oracle/corpus-items/` (606 files, frozen earlier) is secondary and keeps its baselines.

## Published crates and release rule

`vrml97`, `x3d`, `vrml` 0.1.0 are public on crates.io (local tags `*-v0.1.0`). Updates are
patch releases only (0.1.1, 0.1.2, ...), published only when the oracle checks prove full
1:1 parity with X_ITE for what the release claims. An API break would need 0.2: ask Ryan first.

## Later extras (after 1:1 parity; optional, off by default, not 1:1-testable)

X_ITE has neither, so there is no oracle; they are "enhanced" modes on top of the faithful port.
- **DLSS** (NVIDIA RTX only): mainly DLAA for smoothing, via the `dlss_wgpu` crate (wgpu 30,
  Vulkan). Ryan downloads the NVIDIA DLSS SDK and accepts its license himself; shipping needs
  NVIDIA's library + license text. Not in the public crates by default.
- **Ray tracing**: wgpu's experimental ray queries (Vulkan) for real shadows, reflections and
  bounced light; pairs with DLSS Ray Reconstruction. Never the default, so worlds keep looking
  the way their authors and X_ITE show them.

## Automation (later, not on this workstation)

When both ports are 1:1 with X_ITE and CodeMirror: a nightly VM on Ryan's Proxmox VE runs
the build + every oracle check (draft: `tools/nightly.sh`, not installed anywhere), and a
weekly job catches up with the newest upstream X_ITE release. Everything stays local.
The viewer and editor should become drop-in parts for WRL Forge (Tauri + Rust).

## Suggested order

1. X0 + C0 (both oracles) in parallel.
2. C1–C3 and X1–X2 in parallel. These have no UI and test cleanly.
3. Stop and review with Ryan before any renderer or view work.
