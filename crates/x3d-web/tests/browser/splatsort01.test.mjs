// GaussianSplats sort tests of X_ITE_R's <x3d-canvas> (splatsort01_case.mjs, issue #69): the pictures after view changes that
// flip the order of two overlapping splats, compared with the pinned X_ITE 16.5.0 recording in
// oracle/web-browser-splat-sort-01-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && node --test tests/browser/splatsort01.test.mjs)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("splatsort01.test.mjs");
