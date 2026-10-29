# Rust port plan — X_ITE

Written 2026-10-11. Plan only. No Rust code exists yet.

## Hard rules for this lane

- **Private until proven.** Nothing from `scratch/rust-port/` goes to GitHub or any
  other remote until Ryan says every part works. No `git remote add`, no push, no
  gist, no issue, no PR. `scratch/` is gitignored by the lane root (`.gitignore:14`).
- The upstream copies under `upstream/` are read-only references. Never edit them.
- Do not touch `X_ITE/x_ite` (the fork). It is on `fix/firefox-vrml-xml-parser`
  and belongs to the upstream-contribution lane. Read it only.
- No Cybertown or blaxxun assets in Rust test fixtures that could ever ship.
  Corpus files stay in `recon/fixtures/` and are read by path.

## Inputs (pinned)

| project | path | version | size | license |
| --- | --- | --- | --- | --- |
| X_ITE | `X_ITE/x_ite/src` | 16.5.0, branch `fix/firefox-vrml-xml-parser` @ `34cdf9f97` | 764 JS files, ~129k lines | MIT |

X_ITE is MIT, so a Rust port may keep MIT. Keep its copyright notice.

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
│   └── x3d-browser/      browser loop, navigation, loading (native + wasm)
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
- Corpus: the 14,607 Cybertown files indexed in `recon/index/` are the real
  acceptance test for the VRML parser. Read via `recon/scan.py` rules (gzip in memory,
  `rb`, never write to the archives).

## What REA does here (and what it does not)

REA is built for programs that ship *without* source. Here we have the source, so
REA does not drive the port. It helps in two places:

1. **Inventory.** `analyze_javascript_application` maps modules and call flow.
   The X_ITE whole-tree run
   (evidence `ev_dbd2afb6…bb0f`) parsed 765 JS files with 0 parse failures, but is
   also **partial**: 1,280 application unknowns, 15,704 semantic unknowns, 1,297
   limitation notes. Scan one folder per phase.
2. **Proof ledger.** `build_reconstruction_obligation_ledger` and
   `evaluate_reconstruction_coverage` keep a checklist: one obligation per behavior,
   each tied to an original case, a Rust owner, and a passing verifier. A phase is
   "done" only when its ledger closes with no open unknowns.

## Decisions (made by Ryan 2026-10-11)

- Render target: **wgpu**.
- Script engine: **rquickjs** (QuickJS).

### Original options, for the record

1. **Render target for X_ITE.** Recommended: `wgpu` (one renderer for native
   Vulkan/Metal/DX12 and browser WebGPU/WebGL2 via wasm). Alternative: raw WebGL2
   via `web-sys`, browser only.
2. **Script node engine.** X_ITE `Script` runs ECMAScript. Recommended: `rquickjs`
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

## Rough effort

Line counts are not effort, but for scale: X_ITE X0–X4 (parse + load, no pixels) is weeks. X5–X10 is months.

## Status

- 2026-10-11: **X0 done** —
  X_ITE dump is stable on 50 corpus files (47 parse, 3 rejected). See `oracle/README.md`.
- 2026-10-11: **X1–X2 done** — crates `x3d-fields`, `x3d-scene`, `x3d-parse`. Corpus: 45/47
  byte-exact, 3/3 rejections reproduced; 2 one-line diffs need Script + events (X5+X6),
  listed in `crates/x3d-parse/tests/x0-known.tsv`.
- 2026-10-11: **X3–X6 done (no pixels)** — crates `x3d-runtime` (scene setup, PROTO
  realize, routing/event engine, TimeSensor/interpolators/event utilities, FileLoader,
  Inline/EXTERNPROTO/texture/movie/font loads) and `x3d-script` (Script on rquickjs).
  Items 606/606 exact and 606/606 console logs; x0 47/47 exact (07-bookcase and
  43-villasantzip fixed) and 50/50 logs; x1 470/473 exact (3 wall-clock `Date` scripts)
  and 497/498 logs (1 render-time WASM trap). Known lists in `crates/x3d-script/tests/`.
- 2026-10-11: **X7 renderer core done** — crate `x3d-render` + `x3d-shot` (wgpu 30 on Vulkan,
  offscreen, no window). Render oracle `oracle/x_ite-shot.mjs` (headless GPU Chrome, noise
  floor 0). Within tolerance (RGB delta > 8 on ≤ 0.5% of pixels): items 604/606, x0 46/47
  rendered (3 known rejections). Geometry 23/23 against the pinned JS. Gaps: Text, Fog,
  MovieTexture, background textures, libtess (ear clipping instead), NVIDIA NPOT mipmaps.
- 2026-10-11: **X10 first version** — crate `x3d-viewer`, binary `x3d-view`: live runtime,
  EXAMINE/WALK/FLY, viewpoints, Touch/Plane/Cylinder/Sphere sensors. Matches X_ITE on the
  animation, navigation and sensor oracles (`oracle/tools/x10_compare.py`). Gaps: no in-window
  text, two GPU devices.
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
- 2026-10-11: **NPOT mipmaps: 2 known picture differences, accepted (Ryan's decision).** corpus-items2
  pictures stay **294/296** (MSAA off and 4x); this is not 296/296 parity. Misses: `art/rainbow-room`
  (800×450 JPEG, max 82, 2.54% over) and `art/djascendance-live` (509×339 JPEG, max 94, 1.85% over);
  threshold unchanged (RGB delta > 8 on ≤ 0.5%). Cause: on NVIDIA RTX 3060 Ti, driver 595.91.07,
  Chrome 155.0.8059.39 (ANGLE on OpenGL 4.5), X_ITE pin `34cdf9f97`, `gl.generateMipmap` shifts the
  even side of some mixed-parity (even × odd) levels by one texel. The filter shapes are proven; the
  size rule is closed-driver math with no found formula. Shifted levels here: 400×225, 25×14, 12×7
  and 254×169, 127×84, 63×42, 15×10. A size table would be a fudge, so the mipmap algorithm stays as
  in `e0f613e`. Evidence: `oracle/evidence/npot-mipmaps-2026-10-11/README.md`.
- 2026-10-11: **PLANE and LOOKAT viewers ported (X10), plus OrthoViewpoint in the viewer.** `crates/x3d-viewer`: `nav/plane.rs`
  (PlaneViewer.js), `nav/lookat.rs` (LookAtViewer.js), `lookat.rs` (X3DViewer.lookAtPoint / lookAtBBox, X3DViewpointNode.lookAt*,
  OrthoViewpoint.lookAt*, Box3). The EXAMINE fallback for `PLANE` / `LOOKAT` is gone. Shared look-at code is in place for the
  later Examine double-click step (not wired). OrthoViewpoint (fieldOfView extents + fieldOfViewOffset, projection, 1e4 far,
  viewAll, transitions) was missing from the viewer and is now ported; scene text prints a legacy MFFloat `fieldOfView` as SFVec4f
  like `OrthoViewpoint.convertFields`. 27 new x10 scripts (`oracle/x10-scripts/{plane,lookat}-*.json`, worlds `plane-nav`,
  `lookat-nav`, `plane-ortho`, `lookat-ortho`, generator `oracle/tools/gen_x10_plane_lookat.py`), all PASS: poses 0 (24 scripts),
  1.2e-7 (click-twice) and up to 1.6e-5 (the two Alt double clicks, see below), images 0 over. Harness: new `dblclick` event (both sides), pose now carries `fieldOfViewScale` / `fieldOfViewOffset`.
  Regression: items2 296/296 exact + 296/296 logs; shots 294/296 at 0x and 4x (same 2 NPOT files); clip suite 48 cases, 0
  unexpected (1 known XFAIL); click, walk, examine, sensors, 8 rotate scripts PASS; cargo test all ok. Findings (JS): PLANE pans
  with the middle button or Alt + left (a plain left drag is a no-op, `mousedown` case 0 binds no `mousemove`); PlaneViewer
  `dblclick` works only because `DEVELOPMENT.js` is `true` in `src/` (dist: `false`), kept as `nav::DEVELOPMENT`; LOOKAT has no
  single-click lookAt, it is the double click. Open: touch handlers are ported but have no oracle (the harness sends no touch
  events); Alt double click (lookAtPoint) differs from X_ITE by 6e-6 to 1.6e-5 because X_ITE reads the hit point from a float32 GPU
  pointing buffer; static `x3d-shot` still ignores OrthoViewpoint; `x3d-view` maps a second left release within 500 ms and 4 px
  to `dblclick` (not run in a window).
- 2026-10-11: **EXAMINE double-click lookAt ported (X10).** `nav.rs` `Examine::dblclick` = ExamineViewer.dblclick (`disconnect`
  = stop position / center-of-rotation / rotation chasers and the spin, then `lookAtPoint` with Alt, else `lookAtBBox`, through the
  shared `lookat.rs` request path); `x3d-view`'s double-click detection (window.rs) reaches it unchanged. Also ported, without an
  oracle (no touch events in the harness): Examine's touchstart / touchmove / touchend incl. the `tapedTwice` 300 ms dblclick.
  7 new x10 scripts on `oracle/x10-scripts/examine-nav.wrl` (generator: `oracle/tools/gen_x10_plane_lookat.py`):
  `examine-dblclick{,-miss,-alt,-during-spin,-during-chaser,-then-drag,-drag-interrupt}`, all PASS; poses 0 to 1e-9 except the Alt
  case (dcenter 6.2e-6, dpos 3.1e-6: float32 GPU pointing buffer vs f64 ray cast, as in LOOKAT). Added to `tools/nightly.sh`
  (not installed). Ryan decided 2026-10-11 to keep nav::DEVELOPMENT = true (matches the src oracle).
- 2026-10-11: **Followers component ported (X5/X10).** `crates/x3d-runtime/src/nodes/followers.rs` = Components/Followers
  (X3DFollowerNode, X3DChaserNode, X3DDamperNode, the 14 nodes) + Browser/Followers/X3DArray{Follower,Chaser}Template for
  Coordinate and TexCoord2D. The followers tick from the browser's `prepareEvents` interests (a Map: re-adding keeps the
  position, so they run in the order they became active; they register only once the scene is live, `Runtime::go_live`, like
  `scene .setLive (true)` after the load frames). The viewer's navigation chasers (`x3d-viewer/src/chaser.rs`) are unchanged.
  Oracle: 10 synthetic worlds (`oracle/synthetic-followers/`, generator `oracle/tools/gen_followers.py`, runner
  `oracle/tools/run_followers_suite.py`): all 14 nodes, durations, order 0..5, tau/tolerance variants, set_value jumps,
  set_destination in flight, array lengths 0..7, activation order and chains, 24/30/60 fps and coarse frame intervals,
  tolerance 0, negative tau/duration, duration 0 (NaN): scene text exact at every dumped frame (2,100+ frames), poses 0,
  images within tolerance. Three infrastructure causes found on the way (details `oracle/README.md` "Followers"):
  (1) **Chrome's `Math` is correctly rounded** (matches CORE-MATH on 100,000 inputs per function; glibc differs in 0.05-0.25%,
  log10 4%): `jsmath` Chrome flavor now uses the `core-math` crate (new C build dependency of `x3d-fields`; ask Ryan before any
  crates.io release); (2) **ICU's number-format fast path** (`Intl.NumberFormat` 15 digits prints 0.9999999999999994 as `1`):
  `x3d-fields/src/format.rs` now ports `DecimalQuantity` (`_setToDoubleFast`, `roundToMagnitude`), 250,000 + 62,100 formatted
  values identical to Chrome; (3) the anim oracle's `browser .currentFrameRate` (what Damper reads) is the page's wall-clock rate
  (stubs only cover `getCurrentFrameRate`): script key `"pinFrameRate": true` pins it to 1/dt on both sides. Regression: items2
  296/296 exact + 296/296 logs, items 606/606, x0 47 + 3 known, x1 470 + 3 known + 25 rejected, x6 8/8, synthetic-followers 10/10;
  shots 294/296 at 0x and 4x (same 2 NPOT files); clip suite 48 cases, 0 unexpected (1 known XFAIL); 46 x10 scripts PASS;
  `cargo test --release --workspace` all green.
- Test inputs: Ryan's Mall items, **`oracle/corpus-items2/` (296 `.wrl`, live tree without `.edit.wrl`, qa folders and tmp/bak files; keys `oracle/items2-baseline.json`, `render-items2-baseline`, `render-msaa4-items2-baseline`) is the primary test set** (2026-10-11: scene text 296/296 exact, pictures 294/296 at 0x and 4x, the 2 misses are the known NVIDIA NPOT mipmap differences; details in `oracle/README.md`). The older `oracle/corpus-items/` (606 files, frozen earlier) is secondary and keeps its baselines.
- 2026-10-11: **Scripts in Inlines done** — an Inline's scene is merged into the one runtime (X_ITE has one browser,
  one event queue, one script realm), with the Inline load timeline of Inline.js/FileLoader.js (fetch macrotask, Script
  `initialize ()` at parse, `loadCount`, `nextFrame ()`, setInternalScene, live state), load FALSE/TRUE and url changes,
  nested Inlines, `Browser.*` from an inlined Script, directOutput/mustEvaluate, and IMPORT/EXPORT (routes reconnect on
  the Inline's `loadState`). New oracle `oracle/synthetic-inline-scripts/` (7 worlds): ordered console and each Inline's
  own scene text exact at every dumped frame. See `oracle/README.md`.

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

When the port is 1:1 with X_ITE: a dedicated nightly machine runs
the build + every oracle check (draft: `tools/nightly.sh`, not installed anywhere), and a
weekly job catches up with the newest upstream X_ITE release. Everything stays local.
The viewer should become a drop-in part for WRL Forge (Tauri + Rust).

## Suggested order

1. X0 (the oracle).
2. X1–X2. These have no UI and test cleanly.
3. Stop and review with Ryan before any renderer work.
