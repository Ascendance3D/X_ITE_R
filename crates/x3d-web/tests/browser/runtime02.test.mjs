// Browser runtime 02 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: the resource loading and
// scene API reference cases, compared with the pinned X_ITE 16.5.0 recording (runtime02_harness.mjs). The input, touch
// and coordinate cases are in runtime02_input.test.mjs, the checks X_ITE has no reference for in
// runtime02_lifecycle.test.mjs, runtime02_phone.test.mjs and runtime02_tablet.test.mjs.
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("runtime02.test.mjs");
