// Canvas attributes 01 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: the attributes that set
// browser options, contentScale, maximumFrameRate and update (attributes01_case.mjs, issue #136), compared with the
// pinned X_ITE 16.5.0 recording in oracle/web-browser-attributes-01-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("attributes01.test.mjs");
