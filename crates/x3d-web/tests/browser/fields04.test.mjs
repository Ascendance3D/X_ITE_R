// Field API 04 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: field callbacks and the event
// cascade (fields04_case.mjs, issue #137 part D), compared with the pinned X_ITE 16.5.0 recording in
// oracle/web-browser-fields-04-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("fields04.test.mjs");
