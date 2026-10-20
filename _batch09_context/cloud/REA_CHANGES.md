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
| (this checkpoint) | The Browser object: X_ITE's X3DBrowser class chain and the 30 members it lacked; getExecutionContext before the world is installed; BrowserOptions / BrowserProperties / RenderingProperties nodes; bindViewpoint, resize, dispose reach the viewer |

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
