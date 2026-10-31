// Sound 03 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: Mute and SoundIntensity, an AudioClip's
// pitch, the Sound ranges and priority, the audio listener, the AudioContext life cycle, an AudioDestination, a world replaced
// while a clip plays, and whether the page can be heard (sound03_case.mjs, issue #140), compared with the pinned X_ITE 16.5.0
// recording in oracle/web-browser-sound-03-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("sound03.test.mjs");
