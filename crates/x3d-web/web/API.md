# X_ITE_R browser API — supported and unsupported

Reference: X_ITE 16.5.0 (`oracle/x_ite-pin`, commit `34cdf9f97`). Names are X_ITE's.
"Compared" means a browser test checks the value against a pinned X_ITE recording: `oracle/web-browser-01-baseline` (runtime 01) or `oracle/web-browser-02-baseline` (runtime 02, 30 cases).

This is a small subset. It is not full X_ITE API compatibility.
The Rust engine does all X3D work: parsing, events, sensors, picking, navigation and rendering. The JavaScript adapter only forwards page events and calls.

## Supported

### `<x3d-canvas>`

| member | state |
| --- | --- |
| `browser` | supported, compared; read-only like X_ITE's |
| `src` property and attribute | supported, compared |
| `url` property (MFString) and attribute (`"a.wrl", "b.wrl"`) | supported, tested: the first URL that loads |
| `contentScale` attribute | supported, compared: a number, or `auto` for `devicePixelRatio`; the default is 1. The framebuffer is clamped to the device's maximum texture size, like X_ITE's `limitFramebufferSize` |
| events `initialized`, `load`, `error`, `shutdown` | supported, order compared (`shutdown initialized load` for a world, `error` for a failure); also for a world an Anchor loads |
| `oninitialized`, `onshutdown` properties | supported (called before the event, like X_ITE). The attribute forms are not read |
| `toDataURL`, `toBlob` | present; a WebGPU canvas can read back empty where the browser does not composite it |
| `Symbol.toStringTag` | `"X3DCanvasElement"`, compared |

### Input (the canvas)

| input | state |
| --- | --- |
| mouse move, press, release (Pointer Events) | supported, compared: TouchSensor `isOver`, `isActive`, `hitPoint_changed`, `touchTime`; the nearest of two overlapping sensors; PlaneSensor drags |
| drag outside the canvas | supported: the pointer is captured while a button is down |
| wheel | supported, compared (EXAMINE zoom). `preventDefault ()` only when the viewer navigates |
| double click | forwarded to the viewer (X_ITE's `dblclick`) |
| keyboard (`keydown`, `keyup` on the element) | supported, compared: KeySensor while the canvas has the focus, nothing from other elements. The canvas is focusable (`tabIndex` 0) and takes the focus on a press |
| cursor | supported, compared: `pointer` over a sensor or Anchor, `grabbing` while a drag sensor is active, `move` while the viewer navigates, `default` otherwise |
| Anchor | supported, compared: a `#viewpoint` URL binds the viewpoint; a world URL replaces the world (with the browser events); a page URL (`.html`, `.htm`, `.xhtml`) or a `target` parameter opens it with `window .open` or `location`, without a fetch |
| EXAMINE navigation | supported, compared: rotation (mouse, one finger), zoom (wheel, two-finger pinch). The other viewers of the engine also get the events |
| touch (Pointer Events with `pointerType` touch) | supported, compared: tap on a TouchSensor or Anchor, one-finger rotation, two-finger pinch. Touch moves go to the engine once per frame with all touches, as X_ITE's TouchEvent handlers get them |
| coordinates | compared: X_ITE's `getPointerFromEvent` (framebuffer pixels, origin at the bottom left) at any canvas size, offset and contentScale; there is no DPR 1 assumption |
| `touch-action` | `none` while the world's viewer navigates, `auto` for NavigationInfo `NONE`: the page scrolls and zooms only where the canvas does not take the gesture |

### `X3D` (default export of `x_ite_r.mjs`, also `globalThis.X3D` when free)

| member | state |
| --- | --- |
| `getBrowser ([selector or element])` | supported, compared; the same lookup as X_ITE (`undefined` when nothing matches) |
| `X3DConstants` | access types and the five browser events only |
| `SFColor`, `SFColorRGBA`, `SFRotation`, `SFVec2d/2f/3d/3f/4d/4f` | constructor, components (`r g b a`, `x y z w`, `angle`), `getTypeName`, `toString` |
| `MFString` | constructor, `getTypeName`, `toString` (an `Array`) |

### `X3DBrowser`

| member | state |
| --- | --- |
| `name`, `getName ()` | `"X_ITE_R"` (X_ITE: `"X_ITE"`) |
| `version`, `getVersion ()` | `"0.1.0"` (X_ITE: `"16.5.0"`) |
| `currentScene` | supported, compared |
| `getWorldURL ()`, `baseURL` | supported |
| `loadURL (url [, parameter])` | supported, compared: MFString type check, Promise, rejection text, world kept on failure; a newer `loadURL` aborts an older one, which then never replaces the world. `parameter` is checked and then ignored |
| `createX3DFromString (x3dSyntax)` | supported, compared: a Promise of the scene after its Inline, texture and EXTERNPROTO loads; the scene is not the world; its `worldURL` is the page's base URL; a syntax error rejects |
| `replaceWorld (scene)` | supported, compared: `null` or `undefined` gives an empty world; the event order `SHUTDOWN_EVENT shutdown INITIALIZED_EVENT initialized load`, the shutdown in the next frame; a newer `replaceWorld` rejects an older one with `Replacing world aborted.` |
| `firstViewpoint ()`, `previousViewpoint ()`, `nextViewpoint ()`, `lastViewpoint ()` | supported, compared: the user viewpoints (those with a description), bound in the next frame |
| `changeViewpoint (name)` | supported, compared: any viewpoint of the layer; an unknown name does nothing |
| `viewAll ([transitionTime = 1])` | supported, compared |
| `activeViewpoint` | supported, compared: the bound viewpoint, `null` for the default viewpoint |
| `addBrowserCallback`, `removeBrowserCallback`, `getBrowserCallbacks` | supported for `INITIALIZED_EVENT`, `SHUTDOWN_EVENT`, `INITIALIZED_ERROR` and `CONNECTION_ERROR` (a lost WebGPU device, like X_ITE's `webglcontextlost`); `BROWSER_EVENT` never fires |
| `getCanvas ()` | supported |
| `dispose ()` | supported: stops the frame loop, removes the input listeners, ends the requests of its worlds and frees the engine |
| `ready ()` | X_ITE_R only: a Promise of the engine and its WebGPU device |

### `X3DScene`

| member | state |
| --- | --- |
| `worldURL`, `getWorldURL ()` | supported, compared |
| `rootNodes` | supported, compared; an `Array` of SFNode or `null` (X_ITE: an `MFNode`) |
| `getNamedNode (name)` | supported, compared, including the error text |

### `SFNode`

| member | state |
| --- | --- |
| `getNodeTypeName ()`, `getNodeName ()` | supported, compared |
| `node.field` read | supported, compared: numbers, booleans and strings as JavaScript values, vector types as value objects, `SFNode` as a node, `MFNode` as an `Array`; other types as an `X3DField` (text only). An inputOnly field reads `undefined` |
| `node.field = value` | supported, compared: SFBool, SFFloat, SFDouble, SFTime, SFInt32, SFString, the vector types and MFString. The setter returns `false` for an outputOnly field, like X_ITE's proxy. Other types throw a `TypeError` |
| `getField (name)` | `getName`, `getTypeName`, `getAccessType`, `getValue`, `setValue`, `toString`; compared. X_ITE also prints a deprecation warning; X_ITE_R does not |
| `toString ()` | `TypeName { }` |

### Resource loading

Inline, ImageTexture and EXTERNPROTO files load with the page's `fetch` (`cache: "default"`, an `AbortSignal`), without blocking the page.
A world waits for its loads before it replaces the current world, like X_ITE's scene `loadCount`; the current world runs meanwhile.
Compared: relative URLs, HTTP errors (`Error: Not Found`), data that is no X3D or image, MFString fallbacks in order, cross-origin requests without CORS (`TypeError: Failed to fetch`), EXTERNPROTO fallbacks and waiting, and the console messages and their levels.
Tested: requests that end after a newer `loadURL`, after `dispose ()` or after the element left the document never reach another world.

## Known differences inside the supported part

Each difference below is listed in `KNOWN_DIFFERENCES` of `tests/browser/api.test.mjs` or `tests/browser/runtime02.test.mjs` when a compared case shows it.

- A vector value is a copy: `node.diffuseColor.r = 1` does not change the node. Assign the whole value: `node.diffuseColor = new X3D.SFColor (1, 0, 0)`. In X_ITE the value object is live.
- Removing the element from the document stops the frame loop and releases the buttons and touches that are down; adding it again restarts the loop. X_ITE keeps drawing, and its document `mouseup` listener releases later.
- A `pointercancel` (for example a touch the browser takes for scrolling) releases the touch: the sensor or the viewer ends its drag. X_ITE has no `touchcancel` handler, and its TouchSensor stays active.
- After the element is removed and added again while the pointer did not move, a TouchSensor stays `isOver`. X_ITE ends `isOver`, because its release checks the pointer against the detached canvas (an empty rectangle).
- A hidden page (`visibilityState` hidden) cancels the frame loop, and showing the page starts it again. X_ITE keeps its loop scheduled and relies on the browser to pause `requestAnimationFrame`.
- A URL is requested once per world, also when several nodes use it. X_ITE sends one request per node (the HTTP cache may answer).
- An image that does not decode fails with `Couldn't load image '<file URL>': error.`. X_ITE names the blob URL of the data.
- `loadURL` rejects with `Loading of X3D file aborted.` after `dispose ()`. X_ITE's never settles.
- An Anchor or `loadURL` never opens a `javascript:`, `vbscript:`, `ecmascript:`, `vrmlscript:` or `data:` URL in a window or as the page location, in any letter case; it logs `X_ITE_R: Anchor URL '...' was not opened.`. X_ITE checks for script URLs in lower case only and opens the others. A new window opens with `noopener`.
- The browser fires its first `initialized load` (the empty world) when the engine is ready, which is asynchronous (WebAssembly and the WebGPU device). Use `browser.ready ()` or wait for the `load` of your world.
- A node handle belongs to one world. After the world is replaced, using an old handle throws `X_ITE_R: the node belongs to a world that was replaced.` In X_ITE the old node object stays usable but is no longer in the scene.
- `ready ()` is not an X_ITE name. X_ITE starts synchronously and needs no such call.
- The `layerNode` argument of the viewpoint functions is not supported: they act on the active layer, and a layer node throws.
- Only the `src`, `url` and `contentScale` attributes are read. The other X_ITE attributes (`antialiased`, `splashScreen`, `update`, ...) have no effect.

## Not implemented

- Script nodes: the browser build has no script engine (QuickJS does not build for wasm32-unknown-unknown). A Script logs `Couldn't load URL ... no script engine` and does not run; the rest of the world runs.
- `createScene`, `createX3DFromURL`, `createVrmlFromString`, `createVrmlFromURL`, `importDocument`, `importJS`.
- `bindViewpoint`, `addRoute`, `deleteRoute`, `beginUpdate`, `endUpdate`, `getUserViewpoints` and the other X3DBrowserContext functions.
- Browser options and properties (`getBrowserOption`, `setBrowserOption`, `getBrowserProperty`, `getRenderingProperty`), `print`, `println`, `toVRMLString` and the other stream functions.
- `X3D.createBrowser`, the `X3D (callback)` form, `X3D.noConflict`, all other field classes, live MF arrays, `addFieldCallback`, `getFieldDefinitions`.
- `captureStream` on the element.
- Splash screen, context menu, notifications, timings, XR.
- Sound: the engine's Web Audio port is built in but was not tested in a page.
- After a candidate that downloads but does not parse, the next MFString candidate is not fetched ahead: it fails with `TypeError: Failed to fetch`. The candidates before it are fetched in order.
- The buffers and images of a glTF file are not fetched ahead; they fail like a request without a response.
- A `replaceWorld` while a `loadURL` waits for its Inline and texture responses does not stop that load: like X_ITE, the older world then replaces the newer one.

## Graphics

The renderer is WebGPU only (wgpu 30 `Backends::BROWSER_WEBGPU`). There is no WebGL renderer.
Before it makes the renderer, the engine checks the adapter's limits against what the renderer needs. A shortfall stops the start with `X_ITE_R cannot start: ...` in the element and names the limit.
A lost device (driver reset, GPU process crash, a background tab the browser reclaims) stops the frame loop, calls the `CONNECTION_ERROR` callbacks and shows `X_ITE_R stopped: the WebGPU device is lost.`.

WebGPU compatibility mode (`requestAdapter ({ featureLevel: "compatibility" })`, for devices with OpenGL ES 3.1 class GPUs) is not used. wgpu 30's `RequestAdapterOptions` has no `featureLevel` field and its WebGPU backend always requests a core adapter, so the engine cannot ask for one. This is the blocker until wgpu has the option.

## Mobile support

Pointer Events give mouse, pen and touch input; the engine gets touch as X_ITE's TouchEvent handlers do. Rotation and resize change the framebuffer on the next frame.

The WebGPU column is the vendors' published state as known in mid-2026; this lane did not verify it on devices.

| browser | WebGPU (vendor documentation) | state |
| --- | --- | --- |
| Chrome for Android 121 and later, Android 12 and later, with a supported GPU (Qualcomm Adreno, ARM Mali) | yes, by default | expected to run; **not tested on a real device** |
| Safari on iOS and iPadOS 26 and later | yes, by default | expected to run; **not tested on a real device** |
| Safari on iOS and iPadOS 18 and earlier | no (a feature flag only) | the element shows its fallback message |
| Firefox for Android | no (not yet enabled by default) | the element shows its fallback message |
| Samsung Internet, other Chromium browsers | follows its Chromium version and the device's GPU | **not tested** |
| devices with only OpenGL ES 3.1 class GPUs | needs WebGPU compatibility mode | not supported (see Graphics) |

Tested in desktop Chrome emulation only (`tests/browser/runtime02.test.mjs`): a phone (390 × 844, DPR 3) and a tablet (820 × 1180, DPR 2, rotated to landscape), with touch input. These tests check input, coordinates, framebuffer size, rotation and cleanup. They are no proof of a mobile browser or a mobile GPU.
