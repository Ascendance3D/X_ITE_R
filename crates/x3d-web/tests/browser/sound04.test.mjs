// Sound 04 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: the level the page's AudioContext output
// carries (a looping clip, Mute, SoundIntensity, a stopped and a restarted clip, a replaced world, an AudioDestination),
// measured by a tap on the destination (sound04_case.mjs, issue #140) and compared, within a tolerance, with the pinned X_ITE
// 16.5.0 recording in oracle/web-browser-sound-04-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("sound04.test.mjs");
