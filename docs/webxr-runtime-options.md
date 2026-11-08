# WebXR: runtime options and what is implemented (issue #83)

Rule BR4.1 (the options, their licenses and the choice recorded before WebXR code) and BR4.2 (the part that needs no headset is
implemented and tested with an emulated device; what remains is listed with its blocker). The choice below is the **owner decision
of 2026-11-08** (issue #83): WebXR for the browser build, OpenXR for native XR applications, one shared Rust engine.

## What the pinned X_ITE does

`src/x_ite/Browser/WebXR/X3DWebXRContext.js` (X_ITE 16.5.0, REA evidence ev_6d1006c986f3a88f313085a5a5574eaebb947524bfaf8a6c5ed1ea206a7c8dfe, parent
ev_bfcbb5fafed071f5bd39547b8a7d9097aba28f8bc072995878d7cc4c61991eb3) is browser WebXR: `navigator.xr.requestSession(mode)` with the mode from the
browser option `XRSessionMode`, a `local` reference space, an `XRWebGLLayer` on its WebGL context, and the three session events
`visibilitychange`, `inputsourceschange` and `end`. It has no native runtime. So the reference for "session start and pose input" is
the browser API, and the browser supplies the runtime (a headset runtime, or a WebXR emulator extension).

## Options

| option | what it is | license | fit |
| --- | --- | --- | --- |
| Browser WebXR (the web build) | the page calls `navigator.xr`; the browser talks to the runtime | the browser's | matches the pinned design; no native code |
| OpenXR through the `openxr` crate 0.22.0 (and `openxr-sys` 0.14.0) | native session, swapchains and poses for `x3d-view`; needs a wgpu Vulkan interop for the swapchain images | MIT OR Apache-2.0 (checked with `cargo info`, 2026-11-03) | the way to a native viewer; needs an OpenXR runtime installed |
| An OpenXR runtime on Linux (Monado) | open-source runtime, also with a simulated device | BSL-1.0 (recalled, not verified here) | would allow a headset-free native test; not checked |
| macOS | no OpenXR runtime from the platform (recalled, not verified here) | n/a | native XR on macOS is unavailable; the web build is the only route |

## Owner decision (2026-11-08)

1. The web build uses **browser WebXR**, like the pinned X_ITE. No native runtime is needed for it.
2. Native XR applications use **OpenXR** as the runtime interface, on the same Rust engine (no second renderer). It is not
   started: it needs a runtime to test against and the wgpu/OpenXR Vulkan interop. It does not block an initial browser
   release unless the release contract requires it.
3. Order: the browser behavior of the pinned X_ITE first (established with REA), the shared engine interfaces before any
   hardware-specific code, focused tests for each step. No hardware support is claimed without physical-device evidence;
   unsupported devices and runtimes are listed as unsupported.

## Implemented without a headset

`crates/x3d-web/web/xr_session.mjs` ports the part of `X3DWebXRContext.js` that does not draw: start and stop under a lock, the mode
from the option, the `local` reference space, the three session events, visibility, input-source add and remove with the
four-axis gamepad filter, near and far planes (viewpoint, else 0.1 and 10000), the viewer pose and per-view matrices (a view of
width zero is left out), the target-ray poses, the button 0 press and release rule and the haptic pulse rule.

Tests (`crates/x3d-web/tests/xr`, `node --test 'tests/xr/*.test.mjs'`, no browser):

* `xr_session.test.mjs`: 16 tests on an emulated device (`mock_xr.mjs`: `navigator.xr`, a session that fires events, input sources, frames).
* `pinned_compare.test.mjs`: 7 tests that drive the pinned `X3DWebXRContext` itself (loaded by Node from `oracle/x_ite-pin`, with
  `ScreenLine`, `ScreenPoint`, the page globals and `libtess` stubbed) and the port with the same device and compare the requests to
  `navigator.xr`, the reference spaces, the listened events, the render state, the gamepads handed to the viewer, the framebuffer
  reshapes, and the viewer pose and every view matrix number for number.

## Drawing the views of a pose (PR for issue #83, no headset)

`crates/x3d-render/src/xr.rs` and `render_file_xr` draw a world once per view of an XR pose into the view's viewport, the way
`X3DRenderObject.draw` does with `browser .getPose ()`: the view's projection matrix; the layer's view matrix starting with
`pose .viewMatrix` (`X3DLayerNode.traverse`), which also moves the point, spot and directional lights (not the headlight);
and `x3d_EyeMatrix` (`pose .cameraSpaceMatrix .multRight (view .viewMatrix)`) on the positions only (`Renderer::set_xr_eye`;
the normal matrix stays the one of the model-view matrix). Reference: the pinned page `oracle/xr-shot.html` drives the pinned
`X3DWebXRContext` itself through a mock session (`oracle/x_ite-xr-shot.mjs`, three identical runs per case; the mock replaces
`navigator.xr`, `XRWebGLLayer` and `makeXRCompatible`, and its layer has no framebuffer of its own, so X_ITE draws each view
into the canvas at the viewport that the script gives). `crates/x3d-render/tests/xr_views.rs` compares the Rust views with those
pictures (at most 0.5% of the pixels more than 8 off), at MSAA 0 (`oracle/xr-shot-baseline`) and at MSAA 4
(`oracle/xr-shot-baseline-msaa4`, X_ITE `Multisampling` 4 for each view; the pinned page makes one `MultiSampleFramebuffer` per view):
12 cases (symmetric stereo, eyes turned in, off-axis frusta, a viewer pose that is not the identity, all together,
point/spot/directional lights with a PhysicalMaterial, fog and a Background, a DirectionalLight, a SpotLight and a PointLight with
`shadows TRUE`, ClipPlanes, and PointSet and IndexedLineSet with wide lines). On the RTX 3060 Ti: 0 pixels over 8 in 20 of the 24
comparisons; shadow-point has 1 pixel over 8 (max delta 13 at MSAA 0, 9 at MSAA 4); points-lines has 57 pixels (0.043%, MSAA 0)
and 92 pixels (0.070%, MSAA 4) over 8, all single pixels on the edges of the thin and wide lines (max delta 255 at MSAA 0, 64 at
MSAA 4; rasterizer differences in the lines, not a placement error: the points match). On lavapipe (software) at most 6 pixels
over 8 per case, except points-lines (56 and 113). Controls that must fail: leaving out the eye offsets, swapping the views,
dropping the eye rotation, the pose or the off-axis frusta each takes the picture over the bound, as does each of the first
three for the five added cases, and a Rust picture drawn at MSAA 0 against a key of MSAA 4.

The comparison found two differences, both fixed in `view_frame`. (1) The shadow matrix: X_ITE builds it as
`view .inverse · cameraSpaceMatrix · invLightSpaceProjectionMatrix` with `view .inverse` = the view's own transform then the pose's
inverse, and the layer's camera space matrix starts with the pose; the Rust frame had `cameraSpaceMatrix · invLightSpaceProjectionMatrix`
only (38%, 14% and 26% of the pixels over 8 for the directional, spot and point lights). The shadow map is drawn once from the
light, as in X_ITE. (2) The clip planes: the plane is in the space of the model-view matrix (after the pose) and then gets the eye
matrix (`ClipPlaneContainer.setShaderUniforms`, `planeView .assign (plane) .multRight (view .matrix)`); the Rust planes had neither
(2.2% over 8).

This is the render half that every XR route needs (WebXR with the WebGPU layer, or a native OpenXR viewer). Not covered, and
UNVERIFIED: projective textures (TextureProjector under a pose), the Background and the other state of a layer that is not the
active one (the pose applies to the active layer only), more than one light with a shadow map in one frame, other MSAA counts than
0 and 4, and anything that needs a session: `XRGPUBinding` or `XRWebGLLayer` framebuffers (the mock layer has no framebuffer, so
the multisample resolve of each view into a real layer is not tested), the device's real poses and frusta, input. Limits of the
mock: one viewer pose, two views side by side in one canvas, `getViewport` from the script, `antialias` false and `depth` false in
the layer request as X_ITE sends them. This machine has
no XR runtime (Chrome 156 on Linux reports `immersive-vr` unsupported, `XRGPUBinding` only behind
`--enable-features=WebXRWebGPUBinding`; no OpenXR loader or Monado is installed).

## Not implemented, and the specific blocker

* **Drawing into the session: the WebGL2 path exists, the session does not use it yet.** `XRGPUBinding` (the WebGPU layer) is
  not in the stable browser the tests use, so XR draws on the engine's WebGL2 backend. `engine.xrFrame` (`src/wasm.rs`) draws
  each view of a pose into its viewport, with the shared `x3d_render::xr::view_frame` and `set_xr_eye`. `web/xr_layer.mjs`
  makes the context XR-compatible, makes the `XRWebGLLayer` with X_ITE's options, and copies the canvas into
  `baseLayer.framebuffer`. `tests/browser/xrdraw01.test.mjs` checks this in headless Chromium with an emulated layer that has
  a real framebuffer (two views, each in its own viewport); the device and `makeXRCompatible` are emulated. Remaining: connect
  `XRSessionController` and `xr_layer.mjs` in `x_ite_r.mjs` (an immersive session switches the element to the WebGL2 backend).
  No headset has run it.
* **The input ray, hit point drawing and hit-driven pointing** (`xrUpdatePointers` after the target-ray loop): they need the XR render path.
* **Wiring to the public API.** `getRenderingProperty("XRSession")` and the `xrSessionMode` option still behave as documented in
  `web/API.md` (no XR); they change when the draw path exists.
* **A native OpenXR session:** see the owner decision, item 2.

## Known difference from the pinned X_ITE

`XRFrame.getViewerPose()` returns `null` when the device has no pose for the frame (tracking lost). The pinned `xrUpdatePose` reads
`originalPose.transform` and would throw. `XRSessionController.updatePose` holds the last good pose and views, sets `tracking` to
false until a frame has a pose, still applies the frame's planes and gamepads, and keeps the session (tested on the emulated device,
not on hardware). This is a deliberate robustness difference, not a copy of the pinned behavior.
