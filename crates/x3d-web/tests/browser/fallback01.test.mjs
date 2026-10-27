// Web resource fallback 01 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: the fallback after
// a candidate URL that downloads but does not load, glTF buffers, and loads that are replaced, cancelled or disposed
// meanwhile (fallback01_case.mjs), compared with the pinned X_ITE 16.5.0 recording in
// oracle/web-browser-fallback-01-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("fallback01.test.mjs");
