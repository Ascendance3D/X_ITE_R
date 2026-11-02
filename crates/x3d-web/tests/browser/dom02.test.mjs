// DOM integration 02 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: changes of the elements of X3D content
// written as HTML (dom02_case.mjs, issue #146), compared with the pinned X_ITE 16.5.0 recording in
// oracle/web-browser-dom-02-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("dom02.test.mjs");
