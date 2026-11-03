// Navigation touch 01 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: synthetic touch scripts and the
// viewpoint pose after each step (nav01_case.mjs, issue #370), compared within a tolerance fixed before the run with the
// pinned X_ITE 16.5.0 recording in oracle/web-browser-nav-01-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("nav01.test.mjs");
