// Degenerate triangle 01 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: TriangleSet worlds
// with zero-area triangles and their controls (degenerate01_case.mjs), compared with the pinned X_ITE 16.5.0 recording
// in oracle/web-browser-degenerate-01-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { test } from "node:test";
import assert from "node:assert/strict";
import { CASE_TIMEOUT, openPage, referenceCases } from "./runtime02_harness.mjs";
import { WORLDS } from "./degenerate01_case.mjs";

referenceCases ("degenerate01.test.mjs");

// Finding F1 of PR #124: a world whose triangles all have two equal points made the wasm abort in the tangent
// generator (`unreachable`); the browser object stayed borrowed, and every later call threw "recursive use of an
// object detected which would lead to unsafe aliasing in rust". The case messages leave X_ITE's RuntimeError lines
// out, so this test checks that X_ITE_R logs none.
test ("a degenerate world neither aborts the wasm nor leaves the browser borrowed", { timeout: CASE_TIMEOUT }, async () =>
{
   const page = await openPage ();

   try
   {
      const results = await page .evaluate (async urls =>
      {
         const browser = document .querySelector ("x3d-canvas") .browser;
         const results = [ ];

         for (const url of urls)
         {
            results .push (await browser .loadURL (new window .X3D_NS .MFString (url)) .then (() => "resolved", e => e .message));

            for (let i = 0; i < 3; ++ i)
               await new Promise (requestAnimationFrame);
         }

         return results;
      },
      [ WORLDS .twoEqual, WORLDS .threeEqual, WORLDS .severalEqual, WORLDS .finding, WORLDS .valid ]);

      assert .deepEqual (results, [ "resolved", "resolved", "resolved", "resolved", "resolved" ]);
      assert .deepEqual (page .log .filter (line => /RuntimeError|unreachable|recursive use|pageerror/ .test (line)), [ ]);
   }
   finally
   {
      await page .context () .close ();
   }
});
