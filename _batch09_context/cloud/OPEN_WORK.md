# Batch 09 cloud: open work

Branch `feature/xite-rust-rea-b09-cloud`. X_ITE 16.5.0 @ `34cdf9f97` → native Rust only.

## Hardware-dependent tests: locally validated (owner decision 2026-10-20)

The cloud container has no Vulkan adapter, lacks two gitignored answer keys and blocks a UDP probe, so these 8 tests cannot run
there. The owner ran them with `_batch09_context/cloud/local_validate.sh` and accepted the results as PASS:

- Environment: Linux desktop, NVIDIA GeForce RTX 3060 Ti, pinned X_ITE 16.5.0 @ `34cdf9f97`.
- Validation commit: `0f3de084d8c0630018f8f15e65dd15c9ad0aa749`.
- Result: 8 passed, 0 failed, 0 blocked.
- Answer keys: the historical `oracle/x1-baseline.json` (sha256 `fbe54fa2a0e89f3a…`) and `oracle/items-baseline.json`
  (`8f4bb11593e98f85…`) were used unchanged; their hashes match the recorded ones. They are not re-recorded.

| test | binary | why the cloud cannot run it |
| --- | --- | --- |
| `items_runtime_matches_x_ite` | `x3d-script --test corpus` | `oracle/items-baseline.json` not in git (228 MB) |
| `x1_runtime_matches_x_ite` | `x3d-script --test corpus` | `oracle/x1-baseline.json` not in git (86 MB) |
| `rea_batch09_nav_shard0` … `shard3` (4) | `x3d-viewer --test rea_batch09_nav` | pointing and collision passes need a Vulkan adapter |
| `rea_batch09_inputs_cases_match_x_ite` | `x3d-viewer --test rea_batch09_inputs` | collision-walk needs the GPU path |
| `rea_batch08_net_cases_match_x_ite` | `x3d-viewer --test rea_batch08_net` | the container's loopback does not answer every UDP send to 41234/41235 with port unreachable |

### The three time-dependent x1 worlds

`244-tardis666.wrl`, `265-tardis666.wrl` and `87-weaver_anime_final_fixed2.wrl` read `new Date ()` in a Script: the first two
write the wall clock's hours:minutes:seconds into a Text node, the third seeds a clock whose hand rotations follow from it. The
historical baseline holds the wall clock of the moment X_ITE recorded it ("8:05:17", "8:05:47"), so no new X_ITE capture, and no
Rust run, can reproduce it byte for byte. This is a property of the worlds, not a Rust defect.

- They stay listed in `crates/x3d-script/tests/x1-known.tsv` (X6 nondeterministic) against the historical baseline.
- `Date.now` is not frozen (owner decision; `oracle/README.md`: freezing it stalls X_ITE's loading). The oracle freezes only the
  browser's `getCurrentTime`.
- The three-identical-recordings rule stays for every deterministic case. For these three the established historical baseline
  is the reference and the reason above is why a new capture cannot equal it.

## Remaining work, hardest first

1. Engine internals of the objects an execution context hands out (scx-layers-open, documented, not PASS): X_ITE's X3DRoute,
   X3DImportedNode, X3DExportedNode, X3DProtoDeclaration and X3DExternProtoDeclaration prototypes also carry the engine's own
   members (connect, refreshInstance, newInstance, loadNow, the X3DBaseNode field machinery, the private `_url`, `_load` …
   child fields); the port's carry the public ones (scx-members: owner, kind and value of each, exact). Also not passed to
   Scripts: a loaded EXTERNPROTO's appinfo and documentation (the declaration's own are), toVRMLString / toXMLString /
   toJSONString of a route, an imported or an exported node (X_ITE prints the statement), X3DScene.dispose tearing a scene down.
2. Node class chains: a node X_ITE hands out (SFNode.getValue (), getActiveViewpoint …) is an instance of its node class
   (Viewpoint → X3DViewpointNode → X3DBindableNode → …); the port's is an X3DNodeRef (owner boundary 2026-10-20). The type list
   is the same (getType ()), the constructor names and internal members are not (brx-call-open).
3. Browser boundary kept open (owner decision 2026-10-20, documented, not PASS): brx-api-open (the Browser's ~590 engine
   members: WebGL, DOM, dashboard), brx-call-open (getCanvas, getElement, getNotification, getContextMenu, getViewer,
   getBrowserTime, getBrowserTimings return null; X_ITE returns its page objects), X3DConstants of external-component node
   types in scr-browser-open (X_ITE's load order).
4. OWNER DECISION NEEDED, scr-classes-open: the web page's own globals a Script reaches in X_ITE (Script.js evaluates
   `with (globalObject)` in the page): `window`, `document`, `globalThis` (the page's window: name "", length 0),
   `XMLHttpRequest`, `fetch`. They are not X_ITE's; the port has none. Every global X_ITE's Script.js defines now matches
   (scr-globals, 150 probes).
5. Field representation differences (sub:fields.types): an SF component that is neither number, undefined nor null is stored as
   NaN by the runtime; IterableWeakSet strong members (QuickJS); external-component X3DConstants order (X_ITE nondeterministic).
6. sub:exec.context-api stays PARTIAL (owner decision 2026-10-20). Its component state (cfg-comp) and the X3DScene /
   X3DExecutionContext class chain (scx-api) and the objects it hands out (scx-members) now match; what is left in it is item 1.
7. scr-url-order-open (documented open): the order Scripts initialize in when a URL candidate fails or needs a fetch is Chrome's
   microtask and task order (a candidate whose callback throws reaches the next one after the async callback's hops, a failed
   `fetch ("foo:bar")` sooner; a base64 data URL's fetch completes after the files'); the port runs a failed candidate's
   fallback at once and fetches in request order. The synchronous part matches (scr-url-order-sync). parse-def-proto,
   parse-loads (documented open): a scene a Script asks for in initialize () resolves one to three frames later in X_ITE; the
   port's world-load loop keeps running frames while such scenes load (X_ITE's world loadCount does not count them) and the
   oracle page's warm-up frames run without returning to the event loop.
8. sub:fields.math: compare the Rust Geometry math (Box3, Line3, Plane3, Triangle, ViewVolume, Spline) with X_ITE's
   standard/Math/Geometry directly.
9. The rest of the REA queue.

Kept documented (not defects of the port's model): the text of TypeErrors the engine words (cfg-scene-odd: QuickJS "cannot read
property 'name' of undefined", V8 "Cannot read properties of undefined (reading 'name')"; the thrown class and the state are
compared in cfg-comp).
