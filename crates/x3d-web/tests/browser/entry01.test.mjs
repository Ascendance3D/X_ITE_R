// Entry point 01 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: X3D (onfulfilled, onrejected),
// X3D.noConflict, X3D.createBrowser, X3D.getBrowser and the element's captureStream (entry01_case.mjs, issue #138),
// compared with the pinned X_ITE 16.5.0 recording in oracle/web-browser-entry-01-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("entry01.test.mjs");
