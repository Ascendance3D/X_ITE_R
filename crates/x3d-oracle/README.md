# x3d-oracle

Runs the pinned X_ITE reference page (`oracle/x_ite-pin/src/anim*.html`) in headless Chrome
over the DevTools protocol. No Node.js, npm or Playwright is involved. It replaces the X_ITE
reference drivers that used to need Playwright from `x_ite-tests/node_modules`.

| mode | replaces | output |
| --- | --- | --- |
| `anim` | `oracle/x_ite-anim.mjs` | js.json, PNGs, `<out>.audio.f32` |
| `anim-msaa` | `oracle/x_ite-anim-msaa.mjs` | same |
| `anim-sized` | `oracle/x_ite-anim-sized.mjs` | same |
| `particles` (`--variant b07` (default) / `events` / `06r`) | `oracle/tools/x_ite_particles_b07.mjs` / `_events.mjs` / `x_ite_particles.mjs` | result JSON + `.f32` |

The arguments, script keys, page calls, call order, result JSON bytes and exit status are the
same as the Node tool's:

```
cargo build --release -p x3d-oracle
target/release/x3d-oracle anim oracle/synthetic-rea-b09-stepped/anchor-runtime.wrl \
    oracle/synthetic-rea-b09-stepped/anchor-click.json --out /tmp/js.json --png-dir /tmp/js
target/release/x3d-oracle anim-msaa WORLD SCRIPT --ms 4 --out js.json --png-dir js
target/release/x3d-oracle particles oracle/synthetic-rea-b07-ps/ps-force-cone.x3dv \
    oracle/synthetic-rea-b07-ps/scripts/ps-force-cone.json --out /tmp/p/js.json [--png-dir /tmp/p/png [--ms 4]]
```

Options of this tool (every mode):

* `--chrome PATH`: default `$X3D_ORACLE_CHROME`, otherwise `/usr/bin/google-chrome`, which is
  the executable the Node tools launch.
* `--root DIR`: the served `oracle/` directory. Default `$X3D_ORACLE_ROOT`, otherwise this
  repository's `oracle/`.
* `--timeout SECONDS`: the whole run, default 1800. A timeout is reported in `error`, the
  browser is killed, and the exit status is 1. A watchdog kills the browser group and exits 15
  seconds later if the process is still stuck.
* `--trace-cdp FILE`: logs every DevTools message, for diffing against `DEBUG=pw:protocol`.

Particles additions, which the Node tools do not have:

* `--png-dir DIR` writes the WebGL drawing buffer at each dump frame. It uses the same PNG
  method as `x_ite-anim.mjs`. Before the world loads, the tool calls one `browser.reshape ()`,
  because the protocol's inert ResizeObserver would otherwise leave the buffer at 1x1. The JSON
  and `.f32` output stay byte-identical.
* `--aa` / `--ms N` load `anim-msaa.html`, which is X_ITE antialiased with `N` samples.
* `--png-size WxH` sets the canvas size on that page. The default is 512x512.

## What is matched, and how

* **Launch**: Playwright 1.62's Chromium switches (`chromiumSwitches ()` plus the headless
  defaults), followed by the Node tools' `--headless=new --no-sandbox --use-gl=angle
  --use-angle=gl --enable-gpu --ignore-gpu-blocklist`. The profile directory is fresh and
  `--remote-debugging-pipe` is used (fd 3 / fd 4).
* **Page**: a new browser context and the `FrameSession` initialization Playwright sends: focus
  emulation, the viewport through `setDeviceMetricsOverride` and window bounds, font families,
  media features, `Network.setCacheDisabled`, and `Fetch` interception. Non-local requests are
  aborted (`context.route`), and `favicon.ico` is aborted the way Playwright does it.
* **Settle**: the request/finished/failed counting of `page.on ("request" …)` uses Playwright's
  pairing rules (`requestWillBeSent` plus `requestPaused`, no data: URLs, no favicon, frame or
  worker only).
* **Input**: `mouse.move/down/up/wheel/dblclick` and `keyboard.press/down/up` produce the same
  `Input.*` messages Playwright sends. That includes the US layout table, modifier text
  suppression, `force`, `buttons`, and `setInterceptDrags` around left-button moves.
* **Console** (`"console": true`): `Runtime.consoleAPICalled` and `Log.entryAdded`, with
  Playwright's `ConsoleMessage.text ()` preview rules.
* **Page JavaScript**: every `page.evaluate` / `addInitScript` body is copied byte for byte
  into `src/page_js.rs` and `src/particles_js.rs` (`tools/gen_page_js.py`). Unit tests fail if
  a Node tool's text changes.
* **JSON**: values come back from the page as `JSON.stringify` text and are re-indented
  without being re-encoded. The output is `JSON.stringify (result, null, 2)` (particles: `1`)
  byte for byte.
* **PNG**: the same encoder (RGBA, filter 0, one IDAT, deflate level 9 through zlib-rs).
  Pixels are always identical. Most files are byte-identical too. Node's zlib is Chromium's
  fork (`1.3.2.1-motley`), and for some images it picks different deflate matches than
  stock zlib 1.3, which zlib-rs reproduces. Every consumer compares decoded pixels.

Known textual differences: on a failure inside the browser, the `error` text uses the same
`page.evaluate: ` / `page.goto: ` / `page.waitForFunction: Timeout 30000ms exceeded.` forms.
Only the first line is meaningful, since Playwright appends a stack with its own script
positions. A launch failure exits 1 with a message on stderr, as the Node tool's uncaught
exception does.

## Platforms

The transport is `--remote-debugging-pipe` on fd 3 / fd 4, so the browser can be driven on
Unix (Linux, macOS). On Linux the browser also gets `PR_SET_PDEATHSIG`, and every Unix
browser exits when the pipe closes. The browser and its helpers share one process group,
which is killed on every exit path. The crate builds for Windows, but launching there
returns an error: passing the pipe handles needs either `--remote-debugging-io-pipes` with
inheritable handles or a WebSocket transport, and that is not implemented yet.

## Tests

`cargo test -p x3d-oracle` runs the unit tests. They need no browser: JSON / number
formatting, PNG and zlib bytes, input messages, console previews, HTTP server, argument
parsing, and page-JavaScript drift. Running the reference itself needs Chrome and a GPU. The
pinned page also loads its browser libraries (libtess, libgif, apng-js, CharLS, dicom-parser,
jpeg-js, OpenJPEG, opentype.js, jpeg-lossless-decoder-js) from `oracle/x_ite-pin/node_modules`.
Those are files served to the browser: nothing in them runs under Node. In a worktree, that
path is a symlink to `X_ITE/x_ite/node_modules`.
