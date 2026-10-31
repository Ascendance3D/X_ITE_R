// Script life cycle 01 tests of X_ITE_R's <x3d-canvas> with `scripts="true"`, in real headless Chromium with WebGPU:
// Script `initialize`, field events, `eventsProcessed`, `shutdown`, errors, and the root nodes changed by a Script and
// the `rootNodes` callbacks (scriptlife01_case.mjs, issue #133), compared with the pinned X_ITE 16.5.0 recording in
// oracle/web-browser-script-life-01-baseline (runtime02_harness.mjs). The differences that remain are listed in
// KNOWN_DIFFERENCES (runtime02_harness.mjs) and in web/API.md.
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && node --test tests/browser/scriptlife01.test.mjs)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("scriptlife01.test.mjs");
