# Batch 09 cloud: REA changes

Branch `feature/xite-rust-rea-b09-cloud` from `f008c67`. Reference: pinned X_ITE 16.5.0 @ `34cdf9f97`. Every new X_ITE baseline
below was recorded three times and kept only when the three were identical; the case tool then compared the Rust side exactly
(scene text at every frame, sorted log with level; poses at 1e-4).

## Commits

| commit | change |
| --- | --- |
| `63dc4d4` | X3DField.addFieldCallback on node and Script fields (runtime field observers, ScriptHost::field_event); fcb-basic, fcb-routes, fcb-reentry, fcb-script-fields baselines |
| `6a1305c` | X_ITE's own field object model in the Script realm (prelude generated from the pin); X3DNodeRef boundary; node lifetime (release.rs asks the Script host last); `-0` kept across the boundary; old field natives removed; fldx-*, meta-nodes, scr-classes, scr-field-access, scr-field-defaults, scr-image-events-open baselines |
| `8915e10` | Owner decisions 1-3: huge MF length stays a RangeError (approved safety difference in sub:fields.types, test on a node field); a throwing field callback ends that event's callbacks, is reported at error level, the runtime goes on (test); environment-blocked tests listed with the local validation script. Ledger: node:MetadataSet COMPLETE |
| `0f3de08` | The Browser object: X_ITE's X3DBrowser class chain and the 30 members it lacked; getExecutionContext before the world is installed; BrowserOptions / BrowserProperties / RenderingProperties nodes; bindViewpoint, resize, dispose reach the viewer |
| `d5178fd` | Owner decision 2026-10-20: the 8 hardware-dependent tests validated locally (RTX 3060 Ti, commit `0f3de08`, 8 passed) accepted as PASS; historical x1/items answer keys kept; the three time-dependent x1 worlds documented (OPEN_WORK.md) |

## The Browser object (scr-browser-open)

- `crates/x3d-script/src/prelude/rest.js`: `Browser` is an X3DBrowser. Its prototype chain is X3DBrowser → X3DBrowserContext →
  X3DBaseNode → X3DChildObject → X3DObject, the last two X_ITE's own classes; each member sits on the class that defines it in
  X_ITE (oracle brx-api lists the own names per level), methods not enumerable, accessors enumerable, `advanceTime`,
  `getCurrentFrameRate`, `getCurrentTime` own properties as in X_ITE. `String (Browser)` is `[object X3DBrowser]` through
  X3DObject's own toString.
- New members: bindViewpoint, getActiveLayer, getActiveNavigationInfo, getActiveViewpoint, getCurrentViewer,
  getBrowserOptions, getBrowserProperties, getRenderingProperties, isContextLost, resize, dispose, toVRMLStream,
  toXMLStream, toJSONStream, getField, getFields, getFieldDefinitions, getUserDefinedFields, getType, addEvent, and the
  `activeLayer` / `activeNavigationInfo` / `activeViewpoint` accessors and fields. Owner decision 2026-10-20: getCanvas,
  getElement, getNotification, getContextMenu, getViewer, getBrowserTime, getBrowserTimings return null; getLocalStorage
  throws a TypeError as in X_ITE's Script realm; resize and dispose have their effect in the viewer.
- getScriptNode returns the Script node (its X3DNodeRef), not an SFNode, as X_ITE returns the node.
- `crates/x3d-runtime`: `BrowserActive` (the viewer's active layer, NavigationInfo, Viewpoint and viewer type, for Scripts),
  `default_node` (the browser's default bindables, in a private X3D 4.1 scene no world disposal reaches),
  `browser_execution_context` / `world_installed` (X3DBrowser.replaceWorld installs the world after its Scripts initialized:
  until then getExecutionContext is the page's first, empty world), `browser_options::default_fields`.
- `crates/x3d-viewer`: `sync_browser_active` each phase; requests `bindViewpoint:<id>`, `resize:<w>,<h>`, `dispose`;
  `Viewer3d::disposed` stops stepping, `EmbeddedViewer` unloads the world (its Scripts' shutdown runs) and offers
  `take_resize_request` to the host that owns the canvas.
- Evidence: rea_batch09_sai cases scr-browser (re-recorded), brx-api, brx-call, brx-obj, brx-data (new; generator
  `oracle/tools/gen_rea_b09_browser_cloud.py`), all exact. `crates/x3d-viewer/tests/browser_requests_b09.rs`: bindViewpoint
  binds, getActiveViewpoint reports it, resize resizes, dispose stops the browser (the oracle page cannot observe these).
- Kept open, documented: brx-api-open, brx-call-open, brx-obj-open, X3DConstants probes of scr-browser-open
  (OPEN_WORK.md items 1-3).

## Execution context / SAI timing (differences A-C)

Each found by tracing the pinned X_ITE (a scratch copy with logging; the pin is untouched) and fixed in the port's model:

- A, `ie-err-string-frames`: `createX3DFromString` parsed at once in the port. X_ITE's parsers run their statements after
  `browser .loadComponents (scene)` resolves, a microtask, so a request made during a frame is parsed after the frame and its
  promise resolves a frame later than a rejection. `crates/x3d-runtime`: `Task::BrowserParse` (browser_load.rs, lib.rs).
- B, `ctxev-startup-proto`: the extra `sceneGraph_changed` in frame 2 is SFNodeCache: the first read of a node through an MFNode
  proxy makes its SFNode, and `new SFNode (node)` is a parentsChanged of the node. The page's `toVRMLString ()` (the oracle's own
  dump) reads the root nodes of each PROTO declaration body for the first time; each body fires once. toVRMLStream and
  toJSONStream read through the proxy, toXMLStream does not. `Runtime::ctx_streamed` (ctx_events.rs), called by the Script's
  `toVRMLString` / `toJSONString` and by x3d-drive's dumps (as the page does). New probes `ctxev-stream-vrml`, `-xml`, `-json`
  (a Script streams the scene; dumped at the last frame only, `sai-last.json`).
- C, `ctxev-inline`: the change events that `Inline.setInternalScene` (a `nextFrame ()` continuation, after the frame) sets were
  delivered with the frame's end in the port; X_ITE processes them with the next frame's first events. events.rs
  (`process_phase` finishEvents), ctx_events.rs (`ctx_events_after_frame`).
- D, `cfg-scene-odd`: stays open (OPEN_WORK.md item 1).

New baselines (each 3 identical X_ITE recordings, a 4th equal, Rust exact): ie-err-string-frames, ctxev-startup-proto,
ctxev-inline, ctxev-stream-vrml, ctxev-stream-xml, ctxev-stream-json. cases.txt: 138.

## Components, X3DScene class chain, Script globals (after e62be31)

- Components (owner decision 2026-10-20: X_ITE's component state): X3DScene.addComponent / updateComponent keep the
  ComponentInfo a Script gives as it is, any name and level; the printers write `String (level)` (x3d-scene
  `Scene::component_level_texts`, engine.rs `set_component_level_text`, rest.js `componentObject`). cfg-comp: 33 probes (no,
  null, string and number arguments, plain objects with integer, fractional, string, negative, NaN and missing levels, unknown
  and missing names, add twice, update, remove, getComponent edge cases), exact. cfg-scene-odd keeps only the engine's TypeError
  text.
- X3DScene / X3DExecutionContext: X_ITE's classes (rest.js `makeExecutionContext`): the members on the class X_ITE defines them
  on (the own names per prototype level, recorded from the pin), own properties as X_ITE's (`_namedNodes_changed`,
  `_components_changed` once set, the change fields not enumerable), getType, getExecutionContext (ctx_api `parentCtx`),
  getLocalScene, getOuterNode, isLive, getLive, getBrowser, setup, initialize, addChildObjects. scx-api (122 member owners for
  each of four kinds of scene, behavior of each), brx-obj (now without its open part), exact.
- Script globals: every global X_ITE's Script.js defines (scr-globals: what it is, its statics, its prototype chain, what `new`
  gives). ComponentInfo, ProfileInfo, UnitInfo and the info arrays are X_ITE's own classes, generated from the pin into
  prelude/config.js (gen_prelude_xite.py CONFIG); an execution context's arrays are instances of them, filled from the runtime
  when its values change. X3DRoute, X3DImportedNode, X3DExportedNode, X3DProtoDeclarationNode, X3DProtoDeclaration,
  X3DExternProtoDeclaration have X_ITE's names, lengths, statics and chains; print / trace are `Browser .println .bind`.
- Members of the objects an execution context hands out (scx-members): X3DRoute, X3DImportedNode, X3DExportedNode,
  X3DProtoDeclarationNode / X3DProtoDeclaration / X3DExternProtoDeclaration carry their public members on X_ITE's prototypes
  (methods not enumerable, accessors enumerable, no own keys); a declaration's `fields` is a FieldDefinitionArray of
  X3DFieldDefinition; appInfo / documentation; X3DExternProtoDeclaration.loadState; X3DExportedNode.setDescription (ctx_api
  `exportedDescription`). The full layers, engine internals included, are scx-layers-open (documented open).
- Ledger: sub:exec.import-export, sub:exec.bindable-stacks, sub:config.components-profiles COMPLETE on their evidence (14, 13 and
  8 cases); sub:exec.context-api PARTIAL (owner decision). 235 COMPLETE, 51 PARTIAL, 25 DIFFERENT, 33 NOT_TESTED, 2 MISSING.

## Parser and timing (after aeb2d36)

- A Script's toVRMLString / toXMLString / toJSONString, a node's and a node field's string read each node's clone count as it is
  then (X3DGenerator.NeedsName): a PROTO instance's node field value that its body also holds is named (`meta DEF _1 …`).
  engine.rs `with_live_clone_counts`. meta-parse-proto (was meta-parse-open), exact.
- SVG: the OrthoViewpoint's fieldOfView is an SFVec4f in the SVG parser's X3D 4.1 scene (an MFFloat only up to 4.0, X_ITE's
  legacy changeField), so it keeps double precision. parse-formats (was documented open), exact.
- XML: the MFString warning names the field object (`field .getName ()`, "" for a `<field>` definition's value). New
  parse-xml-mfstring (node field, Script and ProtoInterface field definitions, fieldValue, the GeoViewpoint navType alias), exact.
- Script URLs: a data URL that is not base64 initializes the Script in its load microtask, as an `ecmascript:` URL does
  (FileLoader.loadDocumentAsync before its first await). New scr-url-order-sync (unsorted initialize order), exact.
- scr-image-events (was baselined as scr-image-events-open): renamed; it has matched since the field object model.

## Owner decisions 1-2 (2026-10-20, after 1bd58cb)

- Decision 1 (no DOM): `window`, `document`, `XMLHttpRequest`, `fetch` and the page's `globalThis` stay outside the Script
  environment; no stand-in objects; Browser methods that hand out page objects keep returning null. scr-classes-open and
  brx-call-open stay documented open; node:Script stays PARTIAL.
- Decision 2 (no string-to-NaN difference): new fldx-internal, 3 identical X_ITE recordings, exact. X_ITE's internal value
  objects keep any JavaScript value written through `getValue ()`; the port keeps the Script realm's raw components with the
  runtime value (Scene::raw_tokens, token and `String (component)` texts), passes them along routes, drops them on any other
  set or on release, treats such a value as not default, and prints `String (raw)` for a field without a unit category and
  ToNumber (raw) / factor for one with it. Only SFVec2/3/4 f/d, SFColor and SFColorRGBA carry raw components (an MFString
  value is not raw: picking-groups, picking-layers, vpick-layers caught that). data_numbers uses ToNumber for every
  component (null → 0, "" → 0, true → 1).
  sub:fields.types stays PARTIAL. Ledger notes only; totals unchanged.

## Checkpoint after b2e5b9f (cloud agent items 1-6)

1. Raw components in VRML, XML and JSON: `String (raw)` wherever X3DGenerator.ToUnit leaves a component as it is (a field
   without a unit category, or a node's / field's own toXXXString: X3DObject pushes a `null` execution context, so no unit
   converts, numbers included); JSONGenerator.Number quotes NaN / Infinity texts; XML / JSON user fields with raw components are
   never default. New fldx-print, fldx-units, fldx-cross (3 identical X_ITE recordings each, exact).
2. Text that changes after assignment: the texts are taken again when a host's Scripts have run and before a Script prints, so
   the current text prints (fldx-print `mut`). The number of toString calls differs (fldx-print-count-open, documented).
3. IterableWeakSet (tests/weak_set_engine.rs): in the bundled QuickJS a FinalizationRegistry target that is its own unregister
   token stays alive, so X_ITE's own class holds its members strongly here too; freeing a cycle with a WeakRef to such a target
   corrupts the QuickJS heap (reproduced without port code), so the WeakRef stays replaced. Unreachable members are freed
   (memory flat over 80 000 dropped cycles). No unsafe collection was added.
4. Script URL candidates step as X_ITE's FileLoader: a throwing callback reaches the next candidate three microtask ticks
   later, a URL fetch rejects at once two ticks later, an unparsable URL one, a fetched one after its fetch; the failure prints
   when the next starts. New scr-url-order-micro (exact). Still open: the frame network fetches complete in (oracle page).
5. Geometry: oracle/tools/geom_xite.mjs runs the pinned X_ITE's standard/Math/Geometry modules on 2434 cases;
   tests/geom_math.rs: the port is bit-exact on all of them.
6. NOT_TESTED nodes: 12 node-* worlds (field definitions, created text, events of every settable field on a live node), each
   3 identical X_ITE recordings, exact. Fix: OrthoViewpoint's fieldOfView prints as `new SFVec4f (... fieldOfView)` for any
   length (missing components 0 0 0 1); a FontStyle whose family and style change together loads once (X3DUrlObject buffers
   the reload in its `_loadData` field), so a missing family warns once. Open: Text's layout output events (node-text-outputs-open).
   Ledger: the 33 node entries NOT_TESTED -> PARTIAL (visual evidence still missing). Ledger tool fixed: `apply` rebuilds each
   entry from the cloud-start snapshot (a note was appended twice in b2e5b9f, node:Script's missing text was not taken).

## Checkpoint after 804802d (work order D1-D7)

1. Text: the runtime runs X3DGeometryNode's rebuild cascade for Text and sends origin, textBounds, lineBounds as X_ITE does;
   FontStyle loadData copies family and style when it begins, waits for its `sleep (0)` timer and an uncached font file. SFNode
   field callbacks fire (fcb-sfnode). New text-events, text-fontstyle, text-family (exact after the oracle's warm-up).
2. Load timing: investigated to Chrome's Blob / stream reads after `settle` (OPEN_WORK item 1); no oracle change.
3. Geometry: geom2.rs (Box2, Triangle2, Spheroid3, Box3, Camera, ViewVolume instance, SAT, Bezier, eigen, sorts), 3567 cases
   bit-exact. The earlier oracle's random matrices were NaN (1000 cases compared NaN with NaN); fixed.
4. Raw components are converted when printed, in print order (String without unit, ToNumber through ToUnit), in Script prints and
   in the per-frame dumps; nothing when the value is set. fldx-count exact.
5. scx-print: route / imported node prints throw X_ITE's errors, exported nodes and PROTO / EXTERNPROTO declarations print,
   getProtoDeclaration of a loaded EXTERNPROTO.
Ledger: notes and tests only; totals unchanged (235 COMPLETE / 84 PARTIAL / 25 DIFFERENT / 0 NOT_TESTED / 2 MISSING).
