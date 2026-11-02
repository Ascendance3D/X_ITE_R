// Browser user interface 01 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: the descriptions the browser
// sets itself, the splash screen, the timings panel, the context menu, Shift+F8 and the cleanup (ui01_case.mjs, issue #141),
// compared with the pinned X_ITE 16.5.0 recording in oracle/web-browser-ui-01-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("ui01.test.mjs");
