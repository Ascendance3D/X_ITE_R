# x3d-web — X_ITE_R in a web page

`x3d-web` puts the Rust engine of X_ITE_R behind an `<x3d-canvas>` element. The engine is built as WebAssembly and draws with WebGPU.
No X_ITE JavaScript runs: `web/x_ite_r.mjs` is a thin adapter that defines the element and the X_ITE-named API.

This is the first browser batch. It loads and draws a world, and it reads and sets fields from JavaScript.
It is not a full X_ITE replacement yet: [`web/API.md`](web/API.md) lists what exists and what does not.

## Run the example

You need the `wasm32-unknown-unknown` Rust target and the `wasm-bindgen` CLI of the version in `Cargo.lock`.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
tools/build-web.sh                      # writes crates/x3d-web/web/pkg (gitignored)
python3 -m http.server -d crates/x3d-web/web 8000
```

Open <http://localhost:8000/example/> in a browser with WebGPU (Chrome or Edge 113 or later).
The page shows a blue Box, and the buttons set the Material's `diffuseColor` through the JavaScript API.

`example/index.html` uses the element like an X_ITE page does:

```html
<script type="module" src="../x_ite_r.mjs"></script>
<x3d-canvas src="box.wrl"></x3d-canvas>
<script type="module">
  import X3D from "../x_ite_r.mjs";
  const browser = document.querySelector ("x3d-canvas").browser;
  // ... after the "load" event:
  browser.currentScene.getNamedNode ("BoxMaterial").diffuseColor = new X3D.SFColor (1, 0, 0);
</script>
```

## Architecture

| layer | where | what it does |
| --- | --- | --- |
| page adapter | `web/x_ite_r.mjs` | `<x3d-canvas>`, `X3DBrowser`, `X3DScene`, `SFNode` proxies and field values with X_ITE's names. It fetches the world, runs the `requestAnimationFrame` loop and converts values. |
| wasm module | `src/wasm.rs` | `WebBrowser`: the WebGPU device and canvas surface (wgpu, `Backends::BROWSER_WEBGPU`), the renderer, and the loaded world. Exported with wasm-bindgen. |
| session | `src/session.rs` | One world: x3d-runtime's `Runtime` and x3d-viewer's `Viewer3d` frame loop, from bytes and a URL. Platform-neutral and tested natively. |
| engine | `x3d-parse`, `x3d-runtime`, `x3d-viewer`, `x3d-render` | The same crates as the native viewer. |

The engine crates needed four changes to build for `wasm32-unknown-unknown`:

- `x3d-render` and `x3d-viewer` have a `script` feature (on by default). It brings in QuickJS (`x3d-script`) and the file loaders. QuickJS is C code that does not build for `wasm32-unknown-unknown` (`rquickjs-sys`: `stdlib.h` not found), so this crate turns the feature off.
- `std::time::Instant` and `SystemTime` panic on `wasm32-unknown-unknown`. `x3d-render`, `x3d-runtime` and `x3d-viewer` use the `web-time` crate, which is std's own type on other targets.
- `x3d-render` creates its first pipeline on a second thread. wasm32 has no threads and its wgpu objects are not `Send`, so the first frame creates the pipeline there.
- `std::process::id` panics on wasm32. The particle random seed uses 0 for it there.

Native builds keep every default feature, so the native viewer and the test suite do not change.

The runtime loads Inline, EXTERNPROTO and texture URLs through a synchronous `Fetcher`. In the page, that is a synchronous `XMLHttpRequest` (`x-user-defined` bytes).
The world itself is fetched asynchronously by the adapter.

### Evidence for the choice

- `cargo check --target wasm32-unknown-unknown`: `x3d-fields`, `x3d-scene`, `x3d-text`, `x3d-parse` and `x3d-runtime` built unchanged. `x3d-script` failed in `rquickjs-sys`, and `x3d-render` failed only through it.
- With QuickJS off, `x3d-render` had one error (the pipeline thread). `x3d-viewer` had none after the feature gate.
- wgpu 30's default features include `webgpu`. Headless Chromium 151 (Playwright) and Chrome 156 expose WebGPU on Vulkan, also on Mesa lavapipe without a GPU.
- WebGL2 was not tried. The renderer targets WebGPU limits, and the WebGPU path works.

## Tests

| test | where | needs |
| --- | --- | --- |
| session (native) | `tests/session.rs` | `cargo test -p x3d-web` (also part of the workspace suite) |
| browser integration | `tests/browser/api.test.mjs` | the built module, Node, Playwright 1.62.1 Chromium, a Vulkan driver |

```sh
tools/build-web.sh
cd crates/x3d-web && npm ci && npm test
# no GPU: VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json npm test
```

The browser test runs `tests/browser/api_case.mjs` in the page and compares every value with the recording of pinned X_ITE 16.5.0 in `oracle/web-browser-01-baseline` (`oracle/x_ite-web-api.mjs`).
It also compares the pictures before and after the field update with X_ITE's. The rules are: the same background, a silhouette IoU of at least 0.97, and a mean color difference of at most 8 of 255.

Headless Chromium does not always composite a WebGPU canvas, so the test reads the picture from the GPU (`WebBrowser.readPixels`).
`XR_WEB_EVIDENCE=<dir>` writes the values, the pictures and the image metrics of a run.
CI runs the browser tests in `.github/workflows/web-tests.yml` on lavapipe.
