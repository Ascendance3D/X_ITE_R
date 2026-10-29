// Web script URL whitespace 01 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: the characters
// before `ecmascript:` in the url of a Script, the world's loadURL, an Inline and an EXTERNPROTO (scripturl01_case.mjs,
// issue #127), compared with the pinned X_ITE 16.5.0 recording in oracle/web-browser-script-url-01-baseline
// (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("scripturl01.test.mjs");
