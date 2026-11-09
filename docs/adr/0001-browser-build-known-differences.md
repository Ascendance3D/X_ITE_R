# 0001. Known differences of the browser build from X_ITE 16.5.0

Status: **Accepted (2026-11-02).** The engineering decisions below follow the standing rule: match pinned X_ITE 16.5.0 unless it breaks the applicable standard; where the standard is silent, keep X_ITE's observable behavior when that is technically possible. Nothing here changes code or a test.

Reference: X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin`). 16.6.0 is out of scope.

## Context

The browser build (`crates/x3d-web`) runs the Rust engine in WebAssembly behind `x_ite_r.mjs`. Its tests compare each case
with a pinned X_ITE recording. Where the two differ on purpose, the case is listed in `KNOWN_DIFFERENCES`
(`crates/x3d-web/tests/browser/runtime02_harness.mjs`; `api.test.mjs` lists none), and
`crates/x3d-web/web/API.md` ("Known differences inside the supported part") says so in prose.

Without a decision record a later lane cannot tell an approved difference from a gap. This record groups the
differences, names the evidence for each group, and states what the owner is asked to decide: approve the difference, or
accept it as a gap and track it in an issue. It follows the intent of #63 (native `DIFFERENT` entries as ADRs); `docs/adr/`
had no earlier record when this one was written, so this file sets the format: Status, Context, Decision per group,
Consequences.

How to read a group: **Difference** is what X_ITE_R does against X_ITE. **Evidence** names the `KNOWN_DIFFERENCES` keys
(`group.case`) and the `API.md` bullet. **Ask** is the single decision requested.

## Decisions requested

| # | Group | Difference | Evidence | Ask |
| --- | --- | --- | --- | --- |
| 1 | Touch cancel and element removal | A `pointercancel` releases the touch, and removing the element stops the frame loop and releases buttons and touches. X_ITE has no `touchcancel` handler (its TouchSensor stays active) and keeps drawing. After remove and add with an unmoved pointer, a TouchSensor stays `isOver` (X_ITE ends it). | `input.interrupted`; API.md bullets on `pointercancel`, element removal, `isOver` | Approve (better behavior on touch devices, no content relies on the stuck state) |
| 2 | Blob URL in image errors | An image that does not decode fails with `Couldn't load image '<file URL>': error.`; X_ITE names the blob URL of the data. | `loading.failures`, `fallback.content`, `dataURLImage.tabBeforeBase64`; API.md "An image that does not decode" | Approve (the engine has no blob URL to name) |
| 3 | One fetch per URL per world | A URL used by several nodes is requested once per world; X_ITE requests once per node and lets the HTTP cache answer. | `loading.repeated`, `fallback.repeated`; API.md "A URL is requested once per world" | Approve (fewer requests, same content) |
| 4 | `dispose ()` | `loadURL` rejects with `Loading of X3D file aborted.` after `dispose ()` (X_ITE never settles); no frame is drawn after dispose (X_ITE draws one and throws `Couldn't create frame buffer.`). `dispose ()` sends the `shutdown` event when a load is in flight, as X_ITE; it also gives the world's Scripts `shutdown ()` (ISO/IEC 14772-1 4.12.3; X_ITE calls none, `shutdown01.test.mjs`). | `loading.disposedDuringLoad`, `loadURL.disposedWhilePending`, `fallback.disposedDuringFallback`, `scriptShutdown.dispose`; API.md bullets on `dispose ()` and "Script `shutdown`" | Approve (a promise that never settles is not a feature to keep; `shutdown ()` follows the standard) |
| 5 | GLTF2Parser leaked loading object | A world whose Inline glTF has a failed buffer finishes loading. In X_ITE the failed buffer stays a loading object of the browser, so `loadURL` never settles, also when the Inline loads its next candidate. | `fallback.gltfFallback`, `dataURLGltf.badBase64`, `gltfWorld.badBase64Inline`, `gltfWorld.missingInline`, `gltfWorld.fallback`, `gltfWorldHeld.inlineFailThenHeld`, `gltfWorldHeld.fallback`; API.md "A world with an Inline whose glTF file has a buffer that fails" | Approve as an upstream defect not to be copied; the followed-buffer behavior is tracked in #131 |
| 6 | A newer `loadURL` ends the older world's loads | X_ITE's Inline FileLoaders of the older world go on: a candidate that responds after the replacement and does not parse is reported, and the next candidate is fetched. X_ITE_R stops them. | `fallback.cancelledBeforeFallback`; API.md "A newer `loadURL` ends the loads" | Approve (no messages for a world nobody sees) |
| 7 | Font and audio load holding | X_ITE's FontStyle and AudioClip hold the scene's `loadCount`; X_ITE_R's scene did not wait. **This is a gap, not a difference to keep**: the decision recorded in #144 is to match X_ITE. PR #279 (Refs #144) makes the cases match and removes `font.pending` and `audio.decodeFallbackHeld`. | `font.pending`, `audio.decodeFallbackHeld` (removed by #279); API.md bullet "A scene does not wait for its FontStyle font files" (to be removed after #279 merges) | None once #279 merges: close as fixed. Until then, tracked in #144 |
| 8 | Chromium's own network error line for a bad data URL | X_ITE's `fetch` of a data URL whose base64 does not decode logs `Failed to load resource: net::ERR_INVALID_URL` first. X_ITE_R decodes the URL itself (no request) and gives the same rejection without that line. | `dataURL.worldBadBase64`, `dataURLWorld.badBase64`, `gltfWorld.badBase64`; API.md bullet on `data:` URLs | Approve (a browser log line, not an X3D message) |
| 9 | An empty document is an empty world | X_ITE rejects an empty document; in the recording page it fails with `ReferenceError: CharLS is not defined` (the DICOM parser of GoldenGate needs a component the page does not load). X_ITE_R takes empty text as an empty world (existing parser behavior). | `dataURL.worldEmpty`, `dataURLWorld.rawHashHeaderUpper`; API.md bullet on an empty document | Approve (the X_ITE result depends on the host page, not on the document) |
| 10 | Anchor script-scheme blocking (security) | An Anchor or `loadURL` never opens a `javascript:`, `vbscript:`, `ecmascript:`, `vrmlscript:` or `data:` URL in a window or as the page location, in any letter case. X_ITE checks lower case only and opens the others. | API.md bullet on Anchor; REA read of `Anchor.js` (below) | Approve (stricter than X_ITE for safety; this is a security decision and should stay with the owner) |
| 11 | Node handles bound to one world | A node, scene or route belongs to one world (the engine runs one world at a time). Assigning a node of another world to a field, a route across worlds, and using a node of a replaced world are refused; X_ITE allows them. `rootNodes.push` is a no-op on the engine (#151 Part C). | `nodeOwnership.nodeOwnership`, `routesApi.scenes`, `routesApi.crossScene`, `routesApi.browser`; API.md bullets on node ownership, routes, stale nodes | Decide: approve for cross-world use; live values and `rootNodes` are tracked in #137 and #151, and the viewpoint `layerNode` in #135 |
| 12 | Frame loop paused when hidden or removed | A hidden page cancels the frame loop and showing it restarts it; X_ITE keeps its loop scheduled and relies on `requestAnimationFrame` pausing. | API.md bullets on hidden pages and element removal (no `KNOWN_DIFFERENCES` key) | Approve (same visible result, less work) |
| 13 | Names and startup | The browser is named `X_ITE_R`, version `0.2.0`, and has `ready ()`; the first empty world fires `initialized load` when the engine is ready (asynchronous: WebAssembly and WebGPU); X_ITE starts synchronously. | API.md bullets on `ready ()`, startup and the first `load` | Approve (the engine load is asynchronous by nature) |

### Other `KNOWN_DIFFERENCES` entries

These are not in the issue's thirteen groups but are in the harness. Each needs the same decision.

| Group | Difference | Evidence | Ask |
| --- | --- | --- | --- |
| Annotation component | `supportedComponents` has Annotation after X_ITE's 39 (40 in all). X_ITE 16.5.0 ships it but does not register it. | `sceneCreate.supportedLists`; REA batch 04 owner decision, `crates/x3d-scene/src/nodes.rs` | Already an owner decision (REA batch 04); confirm |
| Sound | Closed in #293: after `replaceWorld` the page's context stays with the destination alone, `dispose ()` leaves the context running and `AudioDestination` feeds a `MediaStreamAudioDestinationNode`, all as in X_ITE. | `soundMore.lifecycle`, `soundMore.audioDestination` and `sound.replace` are compared with no known difference | Matches X_ITE |
| Field callbacks | Callbacks run after the frame that processed their events; a field a callback sets sends its event in the next frame. | API.md first bullet (no `KNOWN_DIFFERENCES` key) | Decide: live values are #137 |

## Decision

The rule: a difference stays only when (1) X_ITE breaks the applicable standard, (2) X_ITE has a defect that no test or page can depend on, or (3) the engine design makes the X_ITE behavior impossible. Everything else is a gap with an issue.

| # | Group | Decision | Reason |
| --- | --- | --- | --- |
| 1 | Touch cancel and element removal | Keep the difference. | ISO/IEC 19775-1, Pointing device sensor component (TouchSensor): `isActive` ends when the pointing device is released or lost. A cancelled touch is a lost pointer, so a TouchSensor that stays active (X_ITE has no `touchcancel` handler) breaks the standard. |
| 2 | Blob URL in image errors | Keep the difference. | The standard does not define error text. X_ITE names a blob URL that is random in each run, so no page or test can depend on it. |
| 3 | One fetch per URL per world | Keep the difference. | The content is equal. X_ITE depends on the HTTP cache for the same result; request counts are not defined by the standard. |
| 4 | `dispose ()` | Keep the rejection and the missing frame. The `shutdown` event is matched (#292). Script `shutdown ()` follows the standard on dispose, on a replaced world and on an unloaded Inline scene; X_ITE's none is listed in `KNOWN_DIFFERENCES` per event (matrix in API.md). | A promise that never settles and a frame that throws `Couldn't create frame buffer.` are X_ITE defects. Script `shutdown ()` is required by ISO/IEC 14772-1 4.12.3 and X_ITE breaks it in three of the five life cycle events; a node removed from a live scene is not deleted, so X_ITE's behavior stays there. |
| 5 | GLTF2Parser leaked loading object | Keep the difference. | A world that never finishes loading after a failed glTF buffer is an X_ITE defect; ISO/IEC 19775-1 requires a failed load to end (LoadSensor, Inline `load` events). |
| 6 | A newer `loadURL` ends the older world's loads | Keep the difference. | ISO/IEC 19775-2 `replaceWorld` unloads the old scene; messages for a world nobody sees are not observable content. |
| 7 | Font and audio load holding | Gap, fixed by #279 and closed in #144. | Matches X_ITE (`X3DUrlObject` `loadCount`). |
| 8 | Chromium's network error line for a bad data URL | Keep the difference. | A browser log line, not X3D behavior. The rejection is equal. |
| 9 | An empty document is an empty world | Keep the difference. | The X_ITE result in the recording page is `ReferenceError: CharLS is not defined`, an artifact of a page that does not load the DICOM component. ISO/IEC 19775-1 defines an empty scene as valid. |
| 10 | Anchor script-scheme blocking | Keep the difference. | URI schemes are case-insensitive (RFC 3986, section 3.1). X_ITE checks lower case only, so `JavaScript:` opens. Blocking every letter case follows the RFC and is the safe reading. |
| 11 | Node handles bound to one world | Keep the difference. | The approved architecture runs one world at a time. A node of another world has no meaning to the engine. |
| 12 | Frame loop paused when hidden or removed | Keep the difference. | The visible result is equal. `requestAnimationFrame` pauses hidden pages in both. |
| 13 | Names and startup | Keep the difference. | The name `X_ITE_R` and version identify the product. Loading WebAssembly and WebGPU is asynchronous. |
| 14 | MovieTexture with a static or non-PNG `.png` url | A `MovieTexture` url that ends in `.png` (or `data:image/png`) and is not an animated PNG fails like any other load error: the warning `Error loading movie '<url>': Not an animated PNG` (`Not a PNG` without a PNG signature), the next url, then FAILED_STATE; the scene finishes loading. In X_ITE 16.5.0 apng-js `parseAPNG` returns (does not throw) that error, `MovieTexture.js:98` calls `this .setAPNG (apng)` without `await`, the `apng .getPlayer` TypeError is an unhandled rejection, `loadData` returns with no load state, and the node stays IN_PROGRESS for good: `createX3DFromURL` of a glTF whose `EXT_texture_video` source is a `.png` file never settles (3 of 3 runs, no console message), and every later load in the caller's chain is blocked. X_ITE's own catch (`MovieTexture.js:124-137`) shows the intended behavior. | `crates/x3d-viewer/tests/movie_anim.rs` `KNOWN_DIFFERENCES` (cases `static`, `notpng`, `noframes` of `movie-load-errors-baseline`); `crates/x3d-web/tests/async_loading.rs`; API.md "A `MovieTexture` url that is a static PNG" | Approve as an upstream defect not to be copied (a missing `await`, not a compatibility requirement). The size-limit case (`DecodeError::TooLarge`) is unchanged |
| A | Annotation component | Keep (ADR 0002). | Existing decision. |
| B | `importDocument (htmlElement)` | Gap, tracked in #146 (part 1 landed in #291). | Matches X_ITE. |
| C | Sound after `replaceWorld`; `AudioDestination` | Gap, tracked in #293. | Matches X_ITE; `node:AudioClip` is `PARTIAL`. |
| D | Field callbacks | Gap, tracked in #137 and #202; the route-chain callbacks of #269 and #285 already run inside the cascade. | Matches X_ITE. |

Groups 1, 4 and 10 name the standards that decide them. A kept difference stays in `KNOWN_DIFFERENCES` so the test keeps comparing with the pinned value. A gap is removed from `KNOWN_DIFFERENCES` only when the case matches the pinned recording.

## Consequences

- A later lane can tell an approved difference from a gap: a row marked approved stays in `KNOWN_DIFFERENCES`; a row marked
  as a gap is fixed in the issue that is linked.
- The test cases keep comparing against the pinned X_ITE value, so an approved difference cannot widen unnoticed.
- This file changes no code, test, baseline or ledger entry.

## Evidence

- `crates/x3d-web/tests/browser/runtime02_harness.mjs` (`KNOWN_DIFFERENCES`), `crates/x3d-web/web/API.md`.
- Pinned source read with REA (summary view `ev_7ef30e117641bccc0c3cd8b44fa784f4c537cf6264035dee3bc13fc871ddc22b`, parent
  `ev_87e700dd42d5fd38cede619ca192c5042df5665a4981380e3d7ddb32d804e7ec`): `Parser/GLTF2Parser.js`,
  `InputOutput/FileLoader.js`, `Components/Networking/Anchor.js`, `Browser/DOMIntegration.js`,
  `Components/Networking/X3DUrlObject.js` of `oracle/x_ite-pin/src`. Five files parsed, no parse failures; REA's graph is
  partial (773 files outside the selected scope), so relationships through unselected sources are unknown.
- For group 7: REA `ev_dc4545267a41348acbe1e465c1d19da854557d0f7e896e9f6c9e76b36bb62d39` (PR #279).
- PRs that introduced the entries: #116, #122, #123, #124, #126. They predate the REA rule and carry no Evidence IDs.
