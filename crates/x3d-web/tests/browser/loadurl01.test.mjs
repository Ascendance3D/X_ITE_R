// Web loadURL messages 01 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: the candidates of
// the world-level loadURL and their messages, and the font, MovieTexture and audio loads that wait for their responses
// (loadurl01_case.mjs), compared with the pinned X_ITE 16.5.0 recording in oracle/web-browser-loadurl-01-baseline
// (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { CASE_TIMEOUT, openPage, server } from "./runtime02_harness.mjs";
import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("loadurl01.test.mjs");

// MiniMax P2 of PR #123: the page's loadURL keeps its load record (WebBrowser.loadURL) until the adapter read its last
// messages; then it discards it. The world that parsed has its own record and shares the load's response cache, so
// the discard does not end the world's own requests.
describe ("loadURL load records", () =>
{
   test ("a discarded load record leaves the new world and its requests; repeated loads keep no record", { timeout: CASE_TIMEOUT }, async () =>
   {
      const page = await openPage ();

      try
      {
         server .arm ("lr");

         const from  = server .requests .length;
         const inner = `${server .base}/hold/lr/tests/browser/worlds/fallback/parts/d.wrl`;
         const world = `data:model/vrml,${encodeURIComponent (`#VRML V2.0 utf8\nDEF Outer Transform { children Inline { url "${inner}" } }\n`)}`;

         await page .evaluate (url =>
         {
            const browser = document .querySelector ("x3d-canvas") .browser;

            window .pending = browser .loadURL (new window .X3D_NS .MFString (url)) .then (() => "resolved", e => e .message);
         },
         world);

         // The world parsed and its Inline is in flight: the load record is gone, the world's is there.
         for (let i = 0; i < 200 && !server .requests .slice (from) .some (r => r .path .startsWith ("/hold/lr/")); ++ i)
            await new Promise (r => setTimeout (r, 50));

         const during = await page .evaluate (() =>
         {
            const engine = document .querySelector ("x3d-canvas") .browser [Symbol .for ("X_ITE_R.engine")];

            return { loads: engine .loadRecords, scenes: engine .sceneRecords };
         });

         server .release ("lr");

         const result = await page .evaluate (() => window .pending);

         // Repeated loads: one that loads, one that fails, one replaced by a newer one.
         const repeated = await page .evaluate (async ([ a, b ]) =>
         {
            const browser = document .querySelector ("x3d-canvas") .browser;
            const load    = urls => browser .loadURL (new window .X3D_NS .MFString (... urls)) .then (() => "resolved", e => e .message);
            const results = [ await load ([ a ]), await load ([ b ]) ];
            const older   = load ([ b, a ]);

            results .push (await load ([ a ]), await older);

            const engine = browser [Symbol .for ("X_ITE_R.engine")];

            return { results, loads: engine .loadRecords, scenes: engine .sceneRecords, box: browser .currentScene .rootNodes .length };
         },
         [ `${server .base}/tests/browser/worlds/fallback/parts/w.wrl`, `${server .base}/status/404/lr.wrl` ]);

         // The world is usable after the cleanup: a field of a node, and a frame.
         const usable = await page .evaluate (async () =>
         {
            const browser = document .querySelector ("x3d-canvas") .browser;

            await new Promise (requestAnimationFrame);
            await new Promise (requestAnimationFrame);

            return browser .currentScene .rootNodes [0] .getNodeTypeName ();
         });

         // The page starts without a world: the one scene is the new world.
         assert .deepEqual (during, { loads: 0, scenes: 1 }, "while the Inline of the new world is in flight");
         assert .equal (result, "resolved");
         assert .ok (server .requests .slice (from) .some (r => r .path === "/hold/lr/tests/browser/worlds/fallback/parts/d.wrl"));
         assert .deepEqual (page .log .filter (line => /Couldn't load|abort/i .test (line)) .filter (line => !line .includes ("lr.wrl")), [ ], page .log .join ("\n"));
         assert .deepEqual (repeated, {
            results: [ "resolved", "Couldn't load X3D file.", "resolved", "Loading of X3D file aborted." ],
            loads: 0,
            scenes: 1,
            box: 1,
         });
         assert .equal (usable, "Shape");
         assert .deepEqual (page .log .filter (line => line .startsWith ("pageerror")), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });
});
