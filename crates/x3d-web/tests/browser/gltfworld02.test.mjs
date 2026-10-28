// glTF world buffers 02 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: a glTF world with two
// buffers, one of which fails while the other is held by the test server (gltfworld02_case.mjs), compared with the
// pinned X_ITE 16.5.0 recording in oracle/web-browser-gltf-world-02-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("gltfworld02.test.mjs");
