// Web data URL compatibility 01 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: raw tab, LF
// and CR characters in and after the data scheme of a world, an image and a glTF buffer (dataurl01_case.mjs),
// compared with the pinned X_ITE 16.5.0 recording in oracle/web-browser-data-url-01-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("dataurl01.test.mjs");
