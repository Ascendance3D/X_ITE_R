// Field API 03 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: node ownership and assignment
// between scenes (fields03_case.mjs, issue #137 part C), compared with the pinned X_ITE 16.5.0 recording in
// oracle/web-browser-fields-03-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("fields03.test.mjs");
