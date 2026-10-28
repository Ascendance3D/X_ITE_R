// glTF world buffers 01 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: glTF and GLB files
// loaded as the world and through an Inline, with buffers that load, fail, come late or are held (gltfworld01_case.mjs),
// compared with the pinned X_ITE 16.5.0 recording in oracle/web-browser-gltf-world-01-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("gltfworld01.test.mjs");
