// Field API 05 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: the PROTO and EXTERNPROTO declarations
// of an X3DScene (fields05_case.mjs, issue #137), compared with the pinned X_ITE 16.5.0 recording in
// oracle/web-browser-fields-05-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("fields05.test.mjs");
