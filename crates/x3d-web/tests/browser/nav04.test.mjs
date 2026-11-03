// Navigation press 04 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: a press and a wheel outside the layer Viewport, and the
// viewpoint pose after each step (nav04_case.mjs, issue #370), compared within a tolerance fixed before the run with the
// pinned X_ITE 16.5.0 recording in oracle/web-browser-nav-04-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("nav04.test.mjs");
