# X_ITE_R browser API — supported and unsupported

Reference: X_ITE 16.5.0 (`oracle/x_ite-pin`, commit `34cdf9f97`). Names are X_ITE's.
"Compared" means the browser test checks the value against the pinned X_ITE recording (`oracle/web-browser-01-baseline`).

This is a small subset. It is not full X_ITE API compatibility.

## Supported

### `<x3d-canvas>`

| member | state |
| --- | --- |
| `browser` | supported, compared |
| `src` property and attribute | supported, compared |
| `url` attribute (`"a.wrl" "b.wrl"`) | supported: the first URL that loads; not yet compared |
| events `initialized`, `load`, `error`, `shutdown` | supported, order compared (`shutdown initialized load` for a world, `error` for a failure) |
| `oninitialized`, `onshutdown` properties | supported (called before the event, like X_ITE); not yet compared. The attribute forms are not read |
| `toDataURL`, `toBlob` | present; a WebGPU canvas can read back empty where the browser does not composite it |
| `Symbol.toStringTag` | `"X3DCanvasElement"`, compared |

### `X3D` (default export of `x_ite_r.mjs`, also `globalThis.X3D` when free)

| member | state |
| --- | --- |
| `getBrowser ([selector or element])` | supported, compared |
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
| `loadURL (url [, parameter])` | supported, compared: MFString type check, Promise, rejection text, world kept on failure. `parameter` is checked and then ignored |
| `addBrowserCallback`, `removeBrowserCallback`, `getBrowserCallbacks` | supported for `INITIALIZED_EVENT`, `SHUTDOWN_EVENT`, `INITIALIZED_ERROR`; `CONNECTION_ERROR` and `BROWSER_EVENT` never fire |
| `getCanvas ()` | supported |
| `dispose ()` | supported: stops the frame loop and frees the engine |
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

## Known differences inside the supported part

- A vector value is a copy: `node.diffuseColor.r = 1` does not change the node. Assign the whole value: `node.diffuseColor = new X3D.SFColor (1, 0, 0)`. In X_ITE the value object is live.
- Removing the element from the document stops the frame loop; adding it again restarts it. X_ITE keeps drawing unless `update` says otherwise.
- The browser fires its first `initialized load` (the empty world) when the engine is ready, which is asynchronous (WebAssembly and the WebGPU device). Use `browser.ready ()` or wait for the `load` of your world.
- The page needs WebGPU. There is no WebGL fallback.
- Only `src` and `url` attributes are read. The other X_ITE attributes (`antialiased`, `contentScale`, `splashScreen`, `update`, ...) have no effect.

## Not implemented

- Script nodes: the browser build has no script engine (QuickJS does not build for wasm32-unknown-unknown). A Script logs `Couldn't load URL ... no script engine` and does not run; the rest of the world runs.
- Pointer and keyboard input: no navigation, no TouchSensor, no Anchor, no picking from the page. The engine has them (x3d-viewer); the adapter does not forward events yet.
- `createScene`, `createX3DFromString`, `createX3DFromURL`, `createVrmlFromString`, `createVrmlFromURL`, `replaceWorld`, `importDocument`, `importJS`.
- Viewpoint functions (`firstViewpoint`, `nextViewpoint`, `changeViewpoint`, `viewAll`, ...), `addRoute`, `deleteRoute`, `beginUpdate`, `endUpdate`.
- Browser options and properties (`getBrowserOption`, `setBrowserOption`, `getBrowserProperty`, `getRenderingProperty`), `print`, `println`, `toVRMLString` and the other stream functions.
- `X3D.createBrowser`, the `X3D (callback)` form, `X3D.noConflict`, all other field classes, live MF arrays, `addFieldCallback`, `getFieldDefinitions`.
- Splash screen, context menu, notifications, timings, XR.
- Sound: the engine's Web Audio port is built in but was not tested in a page.
- Inline, EXTERNPROTO and textures load with a synchronous XMLHttpRequest, which blocks the page while it runs; not yet tested in a page.
