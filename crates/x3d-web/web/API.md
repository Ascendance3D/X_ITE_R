# X_ITE_R browser API — supported and unsupported

Reference: X_ITE 16.5.0 (`oracle/x_ite-pin`, commit `34cdf9f97`). Names are X_ITE's.
"Compared" means a browser test checks the value against a pinned X_ITE recording: `oracle/web-browser-01-baseline` (runtime 01), `oracle/web-browser-02-baseline` (runtime 02, 30 cases), `oracle/web-browser-fallback-01-baseline` (10 cases) or `oracle/web-browser-loadurl-01-baseline` (21 cases).

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
| `X3DConstants` | X_ITE's: the access types, the browser events (`CONNECTION_ERROR` is 1001, as in X_ITE), the load states, the field types and the node types of the Core profile; the node types of other components are added when a node of one is first seen, compared (`tests/browser/fields01.test.mjs`) |
| every field class (`SFBool` ... `MFVec4f`, `SFNode`, `MFNode`, `SFImage`, `MFImage`, `SFMatrix3d/3f/4d/4f`, `SFQuaternion`, ...), `X3DField`, `X3DArrayField`, `X3DFieldDefinition`, `FieldDefinitionArray` | X_ITE's own classes, with all their constructors and methods: the generated files of the Script prelude (`crates/x3d-script/src/prelude`, verbatim X_ITE 16.5.0) as one module, `pkg/fields.mjs`; compared |

### `X3DBrowser`

| member | state |
| --- | --- |
| `name`, `getName ()` | `"X_ITE_R"` (X_ITE: `"X_ITE"`) |
| `version`, `getVersion ()` | `"0.1.0"` (X_ITE: `"16.5.0"`) |
| `currentScene` | supported, compared |
| `addRoute (...)`, `deleteRoute (...)` | supported, compared: those of the current scene, with its errors, returning nothing, as X_ITE. `deleteRoute (route)` with a route throws `Bad ROUTE specification: sourceNode must be of type X3DNode.`: the browser passes four arguments on. Not X_ITE members: `browser .getRoute`, `browser .routes` |
| `getWorldURL ()`, `baseURL` | supported |
| `loadURL (url [, parameter])` | supported, compared: MFString type check, Promise, rejection text, world kept on failure; a newer `loadURL` aborts an older one, which then never replaces the world. Each candidate that fails is reported with X_ITE's FileLoader messages (`Couldn't load URL '...', trying URL n of m. ...`, `Couldn't load any of the n URLs, tried in this order:`), in order, by the engine. An abort ends the request in flight, which is reported (`AbortError: signal is aborted without reason`). An empty MFString, or one of empty URLs only, rejects with `Loading of X3D file aborted.`, fires no event and aborts no older `loadURL`, like X_ITE. `parameter` is checked and then ignored |
| `createX3DFromString (x3dSyntax)` | supported, compared: a Promise of the scene after its Inline, texture and EXTERNPROTO loads; the scene is not the world; its `worldURL` is the page's base URL; a syntax error rejects |
| `createScene ()` | supported, compared: a Promise of a new empty scene that is not the world (`worldURL` is the page's URL); `replaceWorld` takes it. Any argument rejects with `Couldn't create scene: profile must be of type ProfileInfo.`, because X_ITE_R has no `ProfileInfo` or `ComponentInfo` (`getProfile`, `getComponent`), see the known differences |
| `createX3DFromURL (url)` | supported, compared: a Promise of the scene of the first url that loads; the world stays, no event fires; the failed candidates are reported with the messages of `loadURL`; every candidate failing rejects with `Couldn't load X3D file.`; a fragment stays in `worldURL`; a url that is not an `MFString` throws at once (not a rejection) |
| `createX3DFromURL (url, node, event)`, `createVrmlFromURL (url, node, event)` | the arguments and their errors are supported and compared (`url must be of type MFString`, `node must be of type X3DNode`, `Unknown field`, `must be a input field`, `must be of type MFNode`; thrown at once); the load and its messages are supported and compared; the root nodes are not put into the field, see the known differences |
| `createVrmlFromString (vrmlSyntax)` | supported, compared: the root nodes as an `MFNode`, made at once (`#VRML V2.0 utf8` is put in front); the parser's warnings and errors are logged and an error is thrown, like X_ITE. The nodes are the current world's, so they can be put into a field of the world (X_ITE's belong to a scene of their own). Needs the engine: call it after `ready ()` |
| `importJS (json)` | supported, compared: a Promise of the scene of X3D JSON (a string or an object); a value that is not X3D JSON gives an empty scene (no rejection), like X_ITE |
| `importDocument (dom)` | supported, compared for an XML string, an `XMLDocument` and an XML element; XML that does not parse, or is not an `X3D` document, gives an empty scene like X_ITE's; `undefined` throws X_ITE's `TypeError`. An HTML element is not supported, see the known differences |
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
| `rootNodes` | supported, compared; an `MFNode` (a new one on each read) |
| `getNamedNode (name)` | supported, compared, including the error text |
| `routes` | supported, compared: a live, read-only `RouteArray` (`length`, index, iteration, `has (id)`, `get (id)`, `getTypeName ()`; `X3D .RouteArray`), the routes in the order they were added, the parsed ones first. The same scene gives the same array |
| `addRoute (sourceNode, sourceField, destinationNode, destinationField)` | supported, compared (`tests/browser/routes01.test.mjs`): returns the `X3DRoute`; a route that exists (any spelling of the field names: `translation`, `translation_changed`, `set_translation`) returns the route it is. A bad node throws `Bad ROUTE specification: source node must be of type X3DNode.` (or `destination node`); a field that does not exist throws `Unknown field 'x' in node class T.`; a type mismatch or a source that is no output or a destination that is no input throws `Bad ROUTE specification: ...`. Nothing is logged. Events over the route go in the same frames, and in the same order, as those of a parsed `ROUTE`. Extra arguments are ignored |
| `getRoute (sourceNode, sourceField, destinationNode, destinationField)` | supported, compared: the route or `undefined`; checks as `addRoute`, with `sourceNode must be of type X3DNode.` (no space) for a bad node |
| `deleteRoute (route)`, `deleteRoute (sourceNode, sourceField, destinationNode, destinationField)` | supported, compared: with four arguments the route is found as `getRoute` finds it, and the errors are `getRoute`'s; anything that is not a route of this scene (another scene's route, a deleted one, a node, `undefined`) does nothing. The route object of a deleted route keeps its nodes and prints the field names it was given. Returns `undefined` |

### `X3DRoute`

| member | state |
| --- | --- |
| `sourceNode`, `destinationNode`, `sourceField`, `destinationField` | supported, compared: the nodes as `SFNode`, the fields as `translation_changed` / `set_translation` for an inputOutput field and as declared otherwise; after `deleteRoute` the names as given to `addRoute` or the `ROUTE` statement. Enumerable accessors on the prototype, read-only |
| `getTypeName ()`, `getId ()`, `getSourceNode ()`, `getSourceField ()`, `getDestinationNode ()`, `getDestinationField ()`, `getExecutionContext ()`, `dispose ()` | supported (the types are compared; `getId ()` is the engine's id of the route, a number; `getSourceNode ()` and `getDestinationNode ()` give the `SFNode`, where X_ITE's give the internal node; `dispose ()` is `deleteRoute`) |
| `toVRMLString ()`, `toXMLString ()`, `toJSONString ()` | throw `Source node does not exist in scene graph.`, as X_ITE's do for a route alone, compared |
| `X3D .X3DRoute` | supported (`instanceof` works) |

### `SFNode`

| member | state |
| --- | --- |
| `getNodeTypeName ()`, `getNodeName ()` | supported, compared |
| the SFNode | X_ITE's own SFNode and its proxy (`getNodeTypeName`, `getNodeName`, `getNodeType`, `getFieldDefinitions`, `getField`, `equals`, ...); compared |
| `node.field` read | supported, compared: `field .valueOf ()` as in X_ITE: numbers, booleans and strings as JavaScript values, every other type as the node's own field object, which is live. An inputOnly field reads `undefined` |
| `node.field = value` | supported for every field type, compared: X3DField.setValue. The setter returns `false` for an outputOnly field, like X_ITE's proxy |
| live field objects | supported, compared: one object per field; a changed component or element (`node .diffuseColor .r = 1`, `node .point [1] .x = 5`, `push`, `length`) reaches the node and its routes in the next frame; the engine's changes reach the object after each frame |
| `getField (name)` | X_ITE's X3DField with all its methods, and X_ITE's deprecation warning; compared |
| `addFieldCallback`, `removeFieldCallback`, `getFieldCallbacks` | supported, compared: each event of the field calls the callbacks with the value of that event, in event order |
| `getFieldDefinitions ()`, `getFieldDefinition (name)` | supported, compared: a `FieldDefinitionArray`; a predefined field's definition holds the class's default value, a user-defined field's definition the field |
| `toString ()` | `TypeName { }`, compared |
| `toVRMLString ()`, `toXMLString ()`, `toJSONString ()` | the node in full, also for `MFNode` and `X3DScene .rootNodes`, with DEF and USE; compared (`tests/browser/fields02.test.mjs`) |
| `fromString`, `fromVRMLString`, `fromXMLString` of every field class | supported, compared (`tests/browser/fields02.test.mjs`): values, errors (`Couldn't read value for field '...'.`) and the nodes of a value |

### Resource loading

The world of `loadURL`, Inline, ImageTexture, EXTERNPROTO, FontStyle and FontLibrary font files, MovieTexture, AudioClip and BufferAudioSource files load with the page's `fetch` (`cache: "default"`, an `AbortSignal`), without blocking the page. A `data:` URL is decoded by the engine, with no request, as the browser's `fetch` decodes it for every kind of file: `;base64` in any letter case, percent escapes, and a fetch error (`TypeError: Failed to fetch`) for base64 that does not decode. Raw tab, LF and CR characters follow Chrome 156: they are kept (as `%09`, `%0A`, `%0D`) when the URL starts with `data:` in any letter case, after any leading tabs and newlines, and removed otherwise (a leading space, a tab in the scheme), as the URL standard removes them in every other URL. Chromium 151 kept them only after an exact lower-case `data:`. A world or Inline document that is not base64 is read as X_ITE's FileLoader reads it: only a lower-case `;base64` is base64, and one malformed escape keeps the text undecoded.
The engine runs every fallback chain, also the world's; the JavaScript adapter only prints the engine's messages.
A world waits for its loads before it replaces the current world, like X_ITE's scene `loadCount`; the current world runs meanwhile.
Compared: relative URLs, HTTP errors (`Error: Not Found`), data that is no X3D or image, MFString fallbacks in order, cross-origin requests without CORS (`TypeError: Failed to fetch`), EXTERNPROTO fallbacks and waiting, and the console messages and their levels.
Tested: requests that end after a newer `loadURL`, after `dispose ()` or after the element left the document never reach another world.
Compared (`tests/browser/fallback01.test.mjs`): after a candidate that downloads but does not load (no parser takes it, it does not parse, an image does not decode, a glTF buffer fails), the next candidate is fetched then, in order, with X_ITE's messages; a candidate after the one that loads is never fetched. The external buffers of a glTF file are requested together and resolve against the glTF file. A load waits for its responses and never blocks the page.
Compared (`tests/browser/loadurl01.test.mjs`): the candidates of the world-level `loadURL` and their messages, also when a newer `loadURL`, `replaceWorld` or `dispose ()` comes while one is in flight; a FontStyle font file that is missing or no font, then one that loads; a MovieTexture GIF or video that fails, then a GIF; an AudioClip file that does not decode, then one that does, and when none does. Each of these waits for the response of the next candidate, also when it is held.

## Known differences inside the supported part

Each difference below is listed in `KNOWN_DIFFERENCES` of `tests/browser/api.test.mjs` or `tests/browser/runtime02_harness.mjs` when a compared case shows it.

- Field callbacks run after the frame that processed their events, not inside its event cascade. A callback gets the value of its event, as in X_ITE; a field it sets sends its event in the next frame, where X_ITE's goes on in the same cascade. A callback that throws is an uncaught page error, as in X_ITE: the callbacks and events after it never run, and the engine processes no event again (the frames go on). Registering a callback while another runs, and removing one in place, follow X_ITE's Map semantics. Compared in `tests/browser/fields04.test.mjs`. Refs #137.
- A node can only be assigned to a field of a node of the same world (a scene of `createX3DFromString` is a world of its own). X_ITE allows it: the node keeps its own scene, and the field holds it. X_ITE_R refuses the assignment, prints the warning `X_ITE_R: a node can only be used in the world it belongs to.`, and leaves the field as it was. The same holds for a node list (`MFNode`), which is refused as a whole, and for `rootNodes .push` of a node of another world, which is refused without a warning. As in X_ITE, an `SFNode` setter keeps a plain JavaScript property when it fails, so the page reads the node back from that property. The text forms and the field value do not show it. An operation on the field after a refused one goes on (the engine keeps its old value, and a text form shows it, see `s13`). A node of a replaced world is refused as stale, where X_ITE keeps it usable. Refs #137, #145. Compared in `tests/browser/fields03.test.mjs` (12 compared scenarios; the cross-scene `ROUTE` scenario, s06, is compared in `routes01.test.mjs` with `addRoute` of #135).
- A route belongs to one world (the engine runs one world at a time). X_ITE puts no limit on the scenes of the nodes of a route: `scene .addRoute (nodeOfThisScene, ..., nodeOfAnotherScene, ...)` works and carries events between the scenes, and `other .addRoute (nodeOfThisScene, ...)` puts a route of the other scene on nodes of this one. X_ITE_R refuses each of these (`addRoute`, `getRoute`, `deleteRoute`, `browser .addRoute`) with the warning `X_ITE_R: a node can only be used in the world it belongs to.` and the same error as a node of another world in a field, so a page that routes between scenes does not work. This closes the deferred cross-scene `ROUTE` scenario (s06) of `fields03`: compared in `tests/browser/routes01.test.mjs` (`routesApi.crossScene`). X_ITE also runs the events of a scene that is not the world (a `createX3DFromString` scene), so a route inside such a scene carries events; X_ITE_R steps only the world, and the routes of another scene carry none (`routesApi.scenes`, `events`). The routes of a replaced world: `routes`, `addRoute` and `getRoute` of its `X3DScene` throw the stale-world error, where X_ITE keeps them usable (`routesApi.browser`). `X3DScene .toVRMLString ()` is not supported, so a world's `ROUTE` statements cannot be printed. Refs #135.
- The text forms of nodes take no options: `toVRMLString ()`, `toXMLString ()` and `toJSONString ()` print with X_ITE's default generator options, and the options object (`style`, `indent`, ...) is ignored. `toString ()` needs none: X_ITE's `X3DNode .toStream` prints `TypeName { }`.
- `X3DField .fromVRMLString` and `fromXMLString` take an `X3DScene` of this adapter as `scene`; the nodes of the value are made in that scene's world. `fromJSONString` calls `fromVRMLString`, as in X_ITE. A value read without a scene works for every type that needs no node, in a page where the engine is loaded (use `browser .ready ()`). Before that it throws `X_ITE_R: the engine is not loaded yet; wait for browser .ready ().`.
- Removing the element from the document stops the frame loop and releases the buttons and touches that are down; adding it again restarts the loop. X_ITE keeps drawing, and its document `mouseup` listener releases later.
- A `pointercancel` (for example a touch the browser takes for scrolling) releases the touch: the sensor or the viewer ends its drag. X_ITE has no `touchcancel` handler, and its TouchSensor stays active.
- After the element is removed and added again while the pointer did not move, a TouchSensor stays `isOver`. X_ITE ends `isOver`, because its release checks the pointer against the detached canvas (an empty rectangle).
- A hidden page (`visibilityState` hidden) cancels the frame loop, and showing the page starts it again. X_ITE keeps its loop scheduled and relies on the browser to pause `requestAnimationFrame`.
- A URL is requested once per world, also when several nodes use it. X_ITE sends one request per node (the HTTP cache may answer).
- A `data:` URL whose base64 does not decode gives the same message as in X_ITE, without Chromium's own `Failed to load resource: net::ERR_INVALID_URL` line before it: X_ITE_R makes no request for it.
- An empty document (`data:model/vrml,`, or an empty file) is an empty world. X_ITE rejects it; in the recording page it fails with `ReferenceError: CharLS is not defined` (DICOMParser).
- An image that does not decode fails with `Couldn't load image '<file URL>': error.`. X_ITE names the blob URL of the data.
- A world with an Inline whose glTF file has a buffer that fails finishes loading. In X_ITE the world never does, also when the Inline loads its next candidate: GLTF2Parser does not remove the failed buffer from the scene's loading objects, so `loadURL` never settles.
- A newer `loadURL` ends the loads of the older world. X_ITE's Inline FileLoaders of the older world go on: a candidate that responds after the replacement and does not parse is reported, and the next candidate is fetched.
- After `dispose ()` X_ITE draws a frame and throws `Couldn't create frame buffer.`; X_ITE_R's frame loop has stopped.
- `loadURL` rejects with `Loading of X3D file aborted.` after `dispose ()`. X_ITE's never settles. X_ITE's `dispose ()` also fires `shutdown`; X_ITE_R's fires no event.
- A scene does not wait for its FontStyle font files or for an AudioClip's decode: it can go live while a font file or the next audio candidate is still in flight. The FontStyle and the AudioClip wait for their responses and complete later. In X_ITE these nodes hold the scene's loadCount, so `createX3DFromString` and `loadURL` settle only after them.
- An Anchor or `loadURL` never opens a `javascript:`, `vbscript:`, `ecmascript:`, `vrmlscript:` or `data:` URL in a window or as the page location, in any letter case; it logs `X_ITE_R: Anchor URL '...' was not opened.`. X_ITE checks for script URLs in lower case only and opens the others. A new window opens with `noopener`.
- The browser fires its first `initialized load` (the empty world) when the engine is ready, which is asynchronous (WebAssembly and the WebGPU device). Use `browser.ready ()` or wait for the `load` of your world.
- A node handle belongs to one world, and so does an `X3DScene` object. After the world is replaced (also by an Anchor), using an old node or scene throws `X_ITE_R: the node belongs to a world that was replaced.`; `currentScene` gives the new scene. In X_ITE the old objects stay usable but are no longer the world.
- The engine releases a node when its scene is disposed (an Inline unloads) and nothing refers to it any more, and a later node can take its id. Using a node object of a released node then throws `X_ITE_R: the node was released; it is no longer in the world.`, and its field callbacks get no more events; a node that takes the id is a new node object. In X_ITE the old node object stays usable.
- Field values the page assigns are checked: data of the wrong shape (too few components, a number for a boolean) or a node that is not in the world throws instead of being filled in.
- `ready ()` is not an X_ITE name. X_ITE starts synchronously and needs no such call.
- The `layerNode` argument of the viewpoint functions is not supported: they act on the active layer, and a layer node throws.
- Only the `src`, `url` and `contentScale` attributes are read. The other X_ITE attributes (`antialiased`, `splashScreen`, `update`, ...) have no effect.

- `createX3DFromURL (url, node, event)` and `createVrmlFromURL`: the scene of the url loads, is checked and discarded, but its root nodes are not put into the field: the nodes belong to a scene of their own, and a field of the world refuses them (the first difference above). The refusal is the warning `X_ITE_R: a node can only be used in the world it belongs to.`; the field keeps its value. The promise form (`createX3DFromURL (url)`) returns the scene, so a page can read it. What remains for #134: loading the url into the world's own runtime. Compared in `tests/browser/scenecreate01.test.mjs` (`vrmlURLArguments`, `vrmlURLLoad`).
- `createScene (profile, component, ...)`: X_ITE_R has no `ProfileInfo` or `ComponentInfo`, so no profile or component can be passed, and the scene has no `profile` or `components`. Any argument rejects with the profile error.
- `createVrmlFromString` before the first world (the page's empty one): the nodes belong to a scene of their own, which they keep alive, and not to the world (there is none to make them in). After a `loadURL` or `replaceWorld` they are the world's. The nodes of a world that was replaced are stale (the second difference above).
- `importDocument (htmlElement)`: X_ITE's HTMLParser reads the HTML element of an `<X3D>` in a page (tag and attribute names are case-insensitive there); X_ITE_R rejects an `HTMLElement` with `X_ITE_R: Browser.importDocument: an HTML element is not supported; ...`. It takes the XML string, an `XMLDocument` or an XML element (serialized and parsed by the engine's XML parser). The DOM integration of #146 is not here.
- `importJS`, `importDocument`: the scene's `worldURL` is the page's URL, like X_ITE's; no Script is run, since the browser build has no script engine (the JSON parser needs none).

## Not implemented

- Script nodes: the browser build has no script engine (QuickJS does not build for wasm32-unknown-unknown). A Script logs `Couldn't load URL ... no script engine` and does not run; the rest of the world runs.
- `getProfile`, `getComponent` and their `ProfileInfo` and `ComponentInfo`, so `createScene (profile, ...)` (#134).
- `bindViewpoint`, `beginUpdate`, `endUpdate`, `getUserViewpoints` and the other X3DBrowserContext functions.
- Browser options and properties (`getBrowserOption`, `setBrowserOption`, `getBrowserProperty`, `getRenderingProperty`), `print`, `println`, `toVRMLString` and the other stream functions.
- `X3D.createBrowser`, the `X3D (callback)` form, `X3D.noConflict`.
- `captureStream` on the element.
- Splash screen, context menu, notifications, timings, XR.
- Sound: the engine's Web Audio port is built in but was not tested in a page.
- MovieTexture frames are not decoded (the engine's `movie_load`): a GIF or PNG that downloads, or a `.webm`, `.ogv`, `.ogg`, `.mp4` or `.m4v` file that downloads, counts as loaded. X_ITE decodes the GIF or APNG and plays the video in a `<video>` element; a video that does not play fails there and the next candidate is tried. X_ITE_R tries the next candidate only after a video name it does not take as playable.
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

Tested in desktop Chrome emulation only (`tests/browser/runtime02_phone.test.mjs`, `tests/browser/runtime02_tablet.test.mjs`): a phone (390 × 844, DPR 3) and a tablet (820 × 1180, DPR 2, rotated to landscape), with touch input. These tests check input, coordinates, framebuffer size, rotation and cleanup. They are no proof of a mobile browser or a mobile GPU.
