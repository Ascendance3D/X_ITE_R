// Navigation touch 03 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: PLANE and LOOKAT with a synthetic touch: the
// viewpoint pose after each step (nav03_case.mjs, issue #370), compared within a tolerance fixed before the run with the
// pinned X_ITE 16.5.0 recording in oracle/web-browser-nav-03-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("nav03.test.mjs");
