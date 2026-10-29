// Viewpoints 01 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: the viewpoint API of the browser (bindViewpoint, layerNode,
// beginUpdate, endUpdate) (viewpoints01_case.mjs, issue #135 part 2), compared with the pinned
// X_ITE 16.5.0 recording in oracle/web-browser-viewpoints-01-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("viewpoints01.test.mjs");
