// Browser user interface 02 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: the notifications of the Debug
// keys, Shift+F8 for a GeoViewpoint and for each encoding, and the "Loading n files" notifications (ui02_case.mjs, issue #141),
// compared with the pinned X_ITE 16.5.0 recording in oracle/web-browser-ui-02-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("ui02.test.mjs");
