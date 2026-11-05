# X_ITE_R Browser Preview 0.1.0 — compatibility

Reference: X_ITE 16.5.0 (`34cdf9f97`). This is a pre-release. It is **not** a drop-in replacement for X_ITE.
`docs/API.md` lists each API member and its state. This page is the short form.

## Works (tested against pinned X_ITE 16.5.0 in the repository's browser tests, unless noted)

- VRML97 classic encoding, X3D classic (`.x3dv`), XML (`.x3d`) and JSON worlds; glTF worlds.
- Rendering on WebGPU: geometry, materials (including PhysicalMaterial), lights, textures, Background, text.
- `<x3d-canvas>` with `src`, `url`, `contentScale`; the events `initialized`, `load`, `error`, `shutdown`.
- Inline, EXTERNPROTO, ImageTexture, MovieTexture, AudioClip and fonts load with `fetch`, without blocking the page,
  with X_ITE's MFString fallback order and console messages. Relative URLs resolve against the file that names them.
- Input: mouse, wheel, keyboard and touch (Pointer Events). TouchSensor, PlaneSensor and the other pointing-device
  sensors, Anchor (viewpoint, world and page URLs), KeySensor.
- Navigation: EXAMINE (compared with X_ITE). WALK, FLY, PLANE and LOOKAT are the native viewer's navigation, which the
  native suite compares with X_ITE; the preview smoke test checks only that WALK and FLY move the viewer in the page.
- TimeSensor, interpolators, routes, ProximitySensor and the other environment sensors.
- JavaScript API: `browser.loadURL`, `replaceWorld`, `createX3DFromString`, `createX3DFromURL`, `createScene`,
  `importJS`, `importDocument`, the viewpoint functions, `getNamedNode`, field reads and writes, `addRoute`,
  `addFieldCallback`, browser options and callbacks, `dispose ()`.
- Sound through Web Audio (compared graph and level).

## Different from X_ITE

- `browser.name` is `"X_ITE_R"`, `browser.version` is `"0.1.0"`.
- Script nodes are off by default (`scripts="true"` turns them on, with the page's privileges and `'unsafe-eval'`).
- WebGPU only: there is no WebGL renderer, so a browser without WebGPU shows the fallback text.
- The other kept differences are in `docs/API.md`, "Known differences inside the supported part".

## Not available

- WebXR (issue #83).
- WebGPU compatibility mode for OpenGL ES 3.1 class GPUs (issue #142).
- The WebGL rendering properties (`MaxTextureSize`, `MaxLights`, ...), `getUserViewpoints` and the other
  X3DBrowserContext functions.
- Not tested on real mobile devices (issue #143); not tested in Safari or Firefox.

## Browsers

| browser | state |
| --- | --- |
| Chrome or Edge 113 or later, desktop, with a WebGPU adapter | supported; the release check runs in Chrome |
| Safari 26, Firefox with WebGPU | not tested |
| browsers without WebGPU | not supported: the element shows its fallback text |
