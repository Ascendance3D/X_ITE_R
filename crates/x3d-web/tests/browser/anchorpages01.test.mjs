// Web Anchor pages 01 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: what an Anchor does with a page URL, a
// `target` parameter, an unreachable URL and a viewpoint fragment (anchorpages01_case.mjs), compared with the pinned X_ITE 16.5.0
// recording in oracle/web-browser-anchor-pages-01-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("anchorpages01.test.mjs");
