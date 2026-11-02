// EXTERNPROTO removal tests of X_ITE_R's <x3d-canvas> with `scripts="true"` (externremove01_case.mjs, issue #58): a removed
// declaration's load finishes and a held declaration keeps working, compared with the pinned X_ITE 16.5.0 recording in
// oracle/web-browser-externproto-remove-01-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && node --test tests/browser/externremove01.test.mjs)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("externremove01.test.mjs");
