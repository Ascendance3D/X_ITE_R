# X_ITE_R Browser Preview 0.1.0

A pre-release of the X_ITE_R browser runtime: the Rust engine of X_ITE_R as WebAssembly, drawn with WebGPU, behind an
`<x3d-canvas>` element with the X_ITE-named JavaScript API. It is for testing. It is **not** a full replacement for
X_ITE 16.5.0; `COMPATIBILITY.md` tells what works and what does not.

`build-info.json` records the Git commit, the Rust and wasm-bindgen versions and the SHA-256 of the module.
`SHA256SUMS` has the SHA-256 of every file: `sha256sum -c SHA256SUMS`.

## Contents

| path | what it is |
| --- | --- |
| `index.html` | start page with links |
| `preview/` | the preview test page, its clean-room test worlds (`worlds/`) and textures (`textures/`) |
| `example/` | the two example pages of the repository |
| `x_ite_r.mjs` | the page module: `<x3d-canvas>` and the JavaScript API |
| `audio_mirror.mjs`, `video_mirror.mjs` | Web Audio and `<video>` support, imported by `x_ite_r.mjs` |
| `pkg/` | the generated engine: `x3d_web_bg.wasm`, its glue `x3d_web.js`, the X_ITE field classes `fields.mjs`, the Script realm `script.mjs` (used only with `scripts="true"`) |
| `docs/API.md` | the API list: supported, different, not implemented |
| `htaccess.example` | Apache MIME types, if the server does not send them |

All files are newly authored for X_ITE_R or generated from its source (MIT, `LICENSE`; X_ITE's field classes, MIT,
`THIRD_PARTY_NOTICES.md`). The package holds no historical world content.

## Deploy

1. Copy the whole directory to the web server. Keep the layout: every URL in it is relative.
2. Serve it over HTTPS. WebGPU needs a secure context (HTTPS, or `http://localhost`).
3. Check the MIME types: `.wasm` must be `application/wasm` (the browser compiles it as a stream only then), `.mjs`
   and `.js` a JavaScript type. `htaccess.example` has the Apache lines; for nginx, `application/wasm wasm;` and
   `text/javascript mjs;` in `types`.
4. Open `preview/` in Chrome or Edge 113 or later on a desktop.

No server code, no CORS headers and no special cross-origin isolation headers are needed. The page loads only files of
its own origin. The test page sets its own Content Security Policy in a `<meta>` element: `script-src 'self'
'wasm-unsafe-eval'` (no `'unsafe-eval'`) and `style-src 'self' 'unsafe-inline'` (the element's shadow `<style>`). Do not
add `'unsafe-eval'` to a shared site's policy for this preview.

## Script nodes are off

Script nodes do not run unless the page sets `scripts="true"` on `<x3d-canvas>`. A Script then runs with the page's
privileges, and its CSP needs `'unsafe-eval'`. The preview page keeps Scripts off. Do not turn them on for worlds you
do not trust.

## Test

The preview page shows the version, the build commit, the WebGPU result, the world URL and the load status. Its
"How to test" list names the action for each feature. The automated release check is in the repository:

```sh
cd crates/x3d-web && npm ci
node tests/preview/smoke.mjs https://<server>/<path>/ --channel chrome --out evidence
```

It loads the page in a new browser profile, drives the worlds with the mouse and the JavaScript API, reads values and
pixels, and writes `results.json`, `console.txt` and screenshots.

## Rebuild

From the commit in `build-info.json`, with Rust (the version in `build-info.json`), the `wasm32-unknown-unknown` target
and `wasm-bindgen-cli` of the same version:

```sh
tools/package-web-preview.sh /tmp/out
```
