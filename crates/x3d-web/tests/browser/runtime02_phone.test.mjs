// Browser runtime 02 phone emulation test of X_ITE_R's <x3d-canvas> (runtime02_harness.mjs). It has a file of its own
// because it takes about 4 minutes on CI's lavapipe: CI runs it in its own shard (shards.mjs).

import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { MOBILE_TIMEOUT, ONLY, PHONE, loadURL, mobileViewport, openPage, server } from "./runtime02_harness.mjs";

describe ("runtime 02 phone and tablet emulation (no proof of mobile GPU support)", { skip: Boolean (ONLY), timeout: MOBILE_TIMEOUT }, () =>
{
   test ("tap, one-finger rotation and pinch zoom at DPR 3 with contentScale auto", async () =>
   {
      const page = await openPage (PHONE);

      try
      {
         await mobileViewport (page);
         await page .evaluate (() =>
         {
            const canvas = document .querySelector ("x3d-canvas");

            canvas .setAttribute ("contentScale", "auto");
            canvas .style .cssText = "display:block;width:390px;height:600px";
         });

         await loadURL (page, `${server .base}/tests/browser/worlds/sensors.wrl`);

         const cdp   = await page .context () .newCDPSession (page);
         const touch = (type, points) => cdp .send ("Input.dispatchTouchEvent", { type, touchPoints: points .map (([ x, y ], id) => ({ x, y, id })) });
         const field = name => page .evaluate (name => { const [ n, f ] = name .split ("."); return String (document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode (n) [f]); }, name);
         const frame = () => page .evaluate (() => new Promise (requestAnimationFrame));

         assert .deepEqual (await page .evaluate (() => { const c = document .querySelector ("x3d-canvas") .browser .getCanvas (); return [ c .width, c .height ]; }), [ 1170, 1800 ]);

         const before = await field ("Camera.position_changed");

         // Points on empty space (no sensor under them): the viewer gets the gestures.
         await touch ("touchStart", [ [ 330, 520 ] ]);
         await frame ();

         for (let i = 1; i <= 5; ++ i) { await touch ("touchMove", [ [ 330 - i * 10, 520 ] ]); await frame (); }

         await page .waitForTimeout (200);
         await touch ("touchEnd", [ ]);
         await page .waitForTimeout (500);

         const rotated = await field ("Camera.position_changed");

         assert .notEqual (rotated, before, "one finger rotates");

         await touch ("touchStart", [ [ 300, 560 ] ]);
         await touch ("touchStart", [ [ 300, 560 ], [ 360, 560 ] ]);
         await frame ();

         for (let i = 1; i <= 5; ++ i) { await touch ("touchMove", [ [ 300 - i * 10, 560 ], [ 360 + i * 5, 560 ] ]); await frame (); await page .waitForTimeout (400); }

         await page .waitForTimeout (200);
         await touch ("touchEnd", [ [ 250, 560 ] ]);
         await touch ("touchEnd", [ ]);
         await page .waitForTimeout (800);

         const pinched = (await field ("Camera.position_changed")) .split (" ") .map (Number);
         const length  = v => Math .hypot (... v);

         assert .ok (length (pinched) < length (rotated .split (" ") .map (Number)) - 0.1, `two fingers apart zoom in: ${rotated} -> ${pinched}`);
         assert .equal (await page .evaluate (() => document .scrollingElement .scrollTop), 0, "the page did not scroll");
         assert .deepEqual (page .log .filter (line => line .startsWith ("pageerror")), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });
});
