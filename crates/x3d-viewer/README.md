# x3d-viewer

Interactive X3D/VRML viewer ported from X_ITE (MIT): live runtime (TimeSensors, interpolators,
Scripts, routes), EXAMINE / WALK / FLY navigation, viewpoints, pointer sensors. Two ways to use it:

* `x3d-view` (feature `gpu`, default): a winit window. A thin host over the embedding API below.
* `x3d_viewer::embed::EmbeddedViewer` (feature `embed`): the viewer inside a host application, on
  the host's own `wgpu::Device` and `wgpu::Queue`, drawing into a texture view the host gives it.

Binaries: `x3d-view` (window), `x3d-drive` (scripted headless run for the oracle comparison),
`x3d-bench` (frame-time benchmark).

## Embedding API

```text
EmbeddedViewer::new(device, queue, (w, h))     the host's wgpu device and queue
  .load(path, LoadOptions { root, start_time }) / .reload() / .unload()
  .input(UiEvent)                              pointer, wheel, key, modifiers: plain structs
  .resize(w, h)                                (also implied by frame()'s target size)
  .frame(now, &target_view, format, w, h)      step the world to `now`, draw into the view
  .navigation_type() / .available_navigation_types() / .set_navigation_type("WALK")
  .viewpoints() / .bind_viewpoint(i) / .step_viewpoint(+1|-1) / .first_viewpoint() / .last_viewpoint()
  .title() / .status() / .log() / .take_prints() / .take_anchor_activations()
  .screenshot_rgba(w, h)                       offscreen render + readback (not the interactive path)
  .core() -> &mut Viewer3d                     everything else (pose, runtime, scene text)
```

* **No toolkit types.** `UiEvent` is `MouseDown { button, x, y }`, `MouseUp`, `MouseMove { x, y }`,
  `Wheel { delta_y, x, y }`, `Key(Home | End | PageUp | PageDown)`, `Mods { shift, ctrl, alt }`.
  Coordinates are canvas pixels with the origin at the **bottom left**. Buttons are 0 left,
  1 middle, 2 right. `Wheel.delta_y` follows the DOM (`> 0` scrolls down = zoom out, a notch is
  100). The viewer uses the same event model as the X_ITE oracle, so a host can replay DOM events.
* **The target.** `frame()` clears the view and draws the whole frame into it: no CPU readback, no
  second device. The format must be a **non-sRGB** color format (`Bgra8Unorm`, `Rgba8Unorm`, ...)
  because X_ITE's framebuffer holds display-encoded values; an sRGB target returns an `Err`. If a
  surface only offers an sRGB format, configure it with the non-sRGB sibling in `view_formats` and
  create the view with that format (`x3d-view`'s `window.rs` does this). The renderer clears to
  `viewer.renderer().clear_color` (default transparent black, X_ITE's `gl.clearColor (0, 0, 0, 0)`;
  use alpha 1 on an opaque surface).
* **Time.** `now` is seconds, the browser clock (`getCurrentTime`): pass `embed::realtime_now()`
  for a live view, or a virtual clock for deterministic runs (`LoadOptions::start_time` sets the
  clock at load). Call `frame()` once per redraw; stepping and drawing happen together.
* **Threading.** The viewer is `!Send` (Rc scene data) and runs Scripts in QuickJS, which needs a
  large stack. Own it on one thread and give that thread at least 256 MB of stack
  (`embed::spawn_big_stack`). Do not call it from a small-stack UI thread.
* **Resources.** Pipelines, uniform buffers and bind groups are created once and reused; each
  geometry is uploaded once and replaced only when its node (or a node it references) was written
  since the last frame (write stamps on `x3d_runtime::Runtime`); textures get one bind group; the
  depth texture is reallocated only on a resize. `FrameInfo` reports CPU step/build/submit
  milliseconds, the GPU milliseconds of the pass (when the device was created with
  `EmbeddedViewer::wanted_features(&adapter)`, i.e. `TIMESTAMP_QUERY`), draw calls and uploads.
* **Not covered yet.** In-window text, Followers, PLANE / LOOKAT navigation, double-click lookAt,
  Scripts inside Inlines (see `PLAN.md`); MovieTexture is not drawn.
* **Multisampling.** Like X_ITE's `Multisampling` rendering property the viewer defaults to 4 samples
  (`embed::DEFAULT_MULTISAMPLING`): a multisampled color + `Depth24Plus` target resolved into your
  view (`MultiSampleFramebuffer`). `set_multisampling(0|1|2|4|8|16)` requests a count; the viewer uses
  the highest supported count not above it (X_ITE clamps to `getMaxSamples ()`). Only 1 and 4 are
  guaranteed. For 2, 8 and 16 create the device with `EmbeddedViewer::wanted_features(&adapter)`
  (adds `TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES`) and call
  `configure_samples(&adapter, target_format)` once; `supported_multisampling()` lists what you got
  and `cycle_multisampling()` steps through it. NVIDIA reports 1, 2, 4, 8.

* **Depth buffer.** `set_depth_mode(DepthMode::Standard | Logarithmic | Reversed)`. `Standard` (default) is
  X_ITE's `DEPTH_COMPONENT24`, `LESS_EQUAL`. `Logarithmic` is X_ITE's `LogarithmicDepthBuffer` property
  ported exactly (far plane 1e10). `Reversed` is beyond X_ITE: reversed-Z with a 32-bit float depth buffer
  and the same near and far planes; it resolves near-coplanar surfaces (planes a few 1e-5 apart at
  distance 10) that the 24-bit buffer ties, but exactly coincident faces flicker more with it (the
  24-bit quantization snaps their equal depths to a tie). Set the mode before `load`; it is
  process-wide, like X_ITE's browser-wide property. `cycle_depth_mode()` steps through the three.

### Minimal host

`examples/embed_minimal.rs` (run: `cargo run --release -p x3d-viewer --example embed_minimal --
world.wrl out.png`) is a complete windowless host. The essential part:

```rust
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, realtime_now, spawn_big_stack};
use x3d_viewer::viewer::UiEvent;

spawn_big_stack(move || {
    // The host's device/queue (a Tauri app already owns them).
    let mut viewer = EmbeddedViewer::new(device.clone(), queue.clone(), (800, 600));
    viewer.load(std::path::Path::new("world.wrl"), LoadOptions::default()).unwrap();

    // Per redraw: forward input, then draw into the host's texture view.
    viewer.input(UiEvent::MouseDown { button: 0, x: 400.0, y: 300.0 });
    viewer.input(UiEvent::MouseMove { x: 420.0, y: 310.0 });
    let info = viewer.frame(realtime_now(), &target_view, wgpu::TextureFormat::Bgra8Unorm, 800, 600).unwrap();
    // info.step_ms / build_ms / render_ms / gpu_ms / draw_calls

    // Viewer menu items.
    for vp in viewer.viewpoints() { println!("{} {}", vp.index, vp.description); }
    viewer.set_navigation_type("WALK");
    viewer.reload().unwrap(); // the file changed
});
```

For a window the host owns the surface: acquire the surface texture, `create_view` with the
non-sRGB format, `frame()`, then present. `x3d-view` is exactly that (`src/window.rs`).

## x3d-view

`x3d-view <file.wrl> [--oracle <dir>] [--size WxH] [--msaa 0|1|2|4|8|16] [--script script.json --png out.png]`.
`--msaa` defaults to X_ITE's 4 (clamped to what the adapter supports); Ctrl/Command + `m` cycles 1, 2, 4, 8 live.
`--depth standard|log|reversed` picks the depth buffer (default standard, X_ITE's); Ctrl/Command + `z` cycles it live.
Keys go to the world as DOM keydown / keyup (KeySensor, StringSensor; repeats too); Home / End
first / last viewpoint, PageUp / PageDown previous / next, as in X_ITE. Viewer shortcuts take Ctrl
(Linux) or Command (macOS): `1` EXAMINE, `2` WALK, `3` FLY, `4` NONE, `l` list viewpoints, `m`
multisampling, `z` depth buffer, `r` reload, `s` screenshot to `/tmp/x3d-view-N.png`, `q` quit.
Esc is a world key and never quits. Mouse: left drag rotates (EXAMINE), middle drag pans, wheel zooms; WALK / FLY as in
X_ITE. Environment: `X3D_VIEW_NOVSYNC=1` (do not wait for vsync), `X3D_VIEW_STATS=1` (adapter, surface
format, alpha mode, and per-frame CPU/GPU averages on exit).

A `--script` run counts a frame only once its world step is drawn and presented; a surface that is
not ready delays the frame (its events and its step), it does not skip it. The run exits with an
error (status 1) when the surface is not ready for 120 redraws in a row, a frame fails to render, or
the window closes before the last frame.

Desktop use: `tools/x3d-view-launch.sh [world] [options]` finds the release binary and opens the
world (no world: `samples/desktop-demo/demo.wrl`, a small world written for this repository with a
texture, lights, three viewpoints, an animation, a TouchSensor + Script and a looping Sound).
`tools/package-desktop.sh <dir>` builds the release binary and packs it with the launcher, the
sample and a README listing the runtime packages (Vulkan driver, ALSA, X11 or Wayland libraries).

## Benchmark

`x3d-bench <file.wrl> [--oracle dir] [--size 1024x768] [--frames 100] [--readback]` runs an EXAMINE
drag headless on the discrete GPU (every frame is a real redraw) and prints, per frame in ms: step,
build, submit (CPU encode + submit), gpu-idle (CPU wait for the GPU), GPU pass time (timestamp
queries), and with `--readback` the offscreen render + readback path the screenshot tools use.

## Tests

`cargo test -p x3d-viewer` (viewer core: animation, navigation, sensors, render cache; embedding
API on a real device, skipped without an adapter). Oracle comparison against X_ITE:
`python3 -I oracle/tools/x10_compare.py <world> <script.json> <out> [--target dir]`; the clipping
and stress scripts are `oracle/x10-scripts/clip-*.json` (generated by `oracle/tools/gen_x10_clip.py`;
scripts with a size other than 512x512 use the sized oracle page `anim-sized.html`).

The drag / zoom regression suite (48 cases, X_ITE as the truth, one documented known difference):
`python3 -I oracle/tools/run_x10_clip_suite.py`.
