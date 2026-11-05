# X_ITE_R Browser Preview 0.1.0 — compatibility

Reference: X_ITE 16.5.0 (`34cdf9f97`). This is a pre-release. It is **not** a drop-in replacement for X_ITE.
`docs/API.md` lists each API member and its state. This page is the short form.

## Works (tested against pinned X_ITE 16.5.0 in the repository's browser tests, unless noted)

- VRML97 classic encoding, X3D classic (`.x3dv`), XML (`.x3d`) and JSON worlds; glTF worlds.
- Rendering on WebGPU: geometry, materials (including PhysicalMaterial), lights, textures, Background, text.
- Rendering on WebGL2 when WebGPU cannot start (no `navigator.gpu`, no adapter, an adapter below the WebGPU default
  limits, or a device that cannot make the renderer's shaders). The same Rust renderer draws through wgpu's WebGL2
  backend; the preview shows the backend in its "Graphics" row. `backend="webgpu"` or `backend="webgl2"` on the element
  (`?backend=` on the preview page) forces one. The fallback is made once, at start, before the canvas has a context.
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
- A lost graphics device stops the engine (X_ITE's CONNECTION_ERROR) and the element shows why. The preview page offers
  one restart (on WebGL2 after WebGPU); the engine never restarts by itself.
- The other kept differences are in `docs/API.md`, "Known differences inside the supported part".

## WebGL2 limits

WebGL2 has no storage buffers, no compute shaders and no cube map arrays. On WebGL2:

- No shadows (`shadows` of lights): every fragment is lit.
- No volume scattering (`VolumeScatterMaterialExtension`): the subsurface term is zero.
- Thick lines of particle systems are made on the CPU, not in a compute pass (same triangles).
- GaussianSplats are not drawn.
- Every uniform block fits in 16 KiB (the WebGL2 minimum; ANGLE on Apple GPUs has exactly that). So an InstancedShape
  or ParticleSystem is drawn in windows of 84 instances (all instances are drawn; a large one needs more draws).
- A frame has at most 64 lights and 8 TextureProjector nodes (192 and 32 on WebGPU). The lights after the first 64 do
  not light anything.
- Environment lights sample their cube maps as faces of a 2D array: the filtering does not cross cube faces.

## Not available

- WebXR (issue #83).
- WebGPU compatibility mode for OpenGL ES 3.1 class GPUs (issue #142).
- The WebGL rendering properties (`MaxTextureSize`, `MaxLights`, ...), `getUserViewpoints` and the other
  X3DBrowserContext functions.
- Not tested on real mobile devices (issue #143).

## Browsers

| browser | state |
| --- | --- |
| Chrome or Edge 113 or later, desktop, with a WebGPU adapter | supported; the release check runs in Chrome |
| Firefox 153 (Linux), no WebGPU | WebGL2: the smoke test passes (headless, NVIDIA GPU) |
| Chromium 151 without WebGPU flags (Linux) | WebGL2: the smoke test passes |
| Safari | not tested yet |
| browsers with neither WebGPU nor WebGL2 | not supported: the element shows its fallback text |
