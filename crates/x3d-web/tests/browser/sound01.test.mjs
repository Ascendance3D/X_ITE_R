// Sound 01 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: an AudioClip that plays through a
// Sound node, its audio graph, the audio context's clock and the autoplay unlock (sound01_case.mjs, issue #140),
// compared with the pinned X_ITE 16.5.0 recording in oracle/web-browser-sound-01-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("sound01.test.mjs");
