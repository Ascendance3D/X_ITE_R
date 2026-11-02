// Script shutdown 01 tests of X_ITE_R's <x3d-canvas> with `scripts="true"`, in real headless Chromium with WebGPU: when a Script's
// `shutdown ()` runs for each life cycle event, and the browser `shutdown` event of `dispose ()` (shutdown01_case.mjs, issue #292),
// compared with the pinned X_ITE 16.5.0 recording in oracle/web-browser-shutdown-01-baseline (runtime02_harness.mjs). The
// differences are listed in KNOWN_DIFFERENCES (runtime02_harness.mjs) and in web/API.md.
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && node --test tests/browser/shutdown01.test.mjs)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("shutdown01.test.mjs");
