# WebXR: runtime options and what is implemented (issue #83)

Rule BR4.1 (the options, their licenses and the choice recorded before WebXR code) and BR4.2 (the part that needs no headset is
implemented and tested with an emulated device; what remains is listed with its blocker). The choice below is **proposed and
pending owner approval**: issue #83 says the owner chooses the runtime or approves WebXR as a difference.

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

## Proposed choice

1. The web build uses **browser WebXR**, like the pinned X_ITE. No native runtime is needed for it.
2. A native OpenXR viewer is **not started**: it needs an owner decision, a runtime to test against and the wgpu/OpenXR Vulkan
   interop, none of which exist in this work. Recording it as an approved difference for `x3d-view` is the alternative.

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

## Not implemented, and the specific blocker

* **Drawing into the session.** X_ITE draws into an `XRWebGLLayer`. X_ITE_R draws with WebGPU, and the WebGPU layer for immersive
  sessions (`XRGPUBinding`) is not available in the stable browser the tests use (to be verified when a browser that ships it is
  chosen). `prepare` and `reshape` are the seams. Resource needed: a browser with WebXR-WebGPU support, or the owner's choice to
  render XR through a WebGL path.
* **The input ray, hit point drawing and hit-driven pointing** (`xrUpdatePointers` after the target-ray loop): they need the XR render path.
* **Wiring to the public API.** `getRenderingProperty("XRSession")` and the `xrSessionMode` option still behave as documented in
  `web/API.md` (no XR); they change when the draw path exists.
* **A native OpenXR session:** see the proposed choice.

## Known difference from the pinned X_ITE

`XRFrame.getViewerPose()` returns `null` when the device has no pose for the frame (tracking lost). The pinned `xrUpdatePose` reads
`originalPose.transform` and would throw. `XRSessionController.updatePose` holds the last good pose and views, sets `tracking` to
false until a frame has a pose, still applies the frame's planes and gamepads, and keeps the session (tested on the emulated device,
not on hardware). This is a deliberate robustness difference, not a copy of the pinned behavior.
