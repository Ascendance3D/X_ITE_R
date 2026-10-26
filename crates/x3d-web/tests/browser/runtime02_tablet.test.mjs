// Browser runtime 02 tablet emulation test of X_ITE_R's <x3d-canvas> (runtime02_harness.mjs). It has a file of its own
// because it takes about 3 minutes on CI's lavapipe (shards.mjs).

import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { MOBILE_TIMEOUT, ONLY, TABLET, loadURL, mobileViewport, openPage, server } from "./runtime02_harness.mjs";

describe ("runtime 02 phone and tablet emulation (no proof of mobile GPU support)", { skip: Boolean (ONLY), timeout: MOBILE_TIMEOUT }, () =>
{
   test ("tablet: rotation resizes the framebuffer, a tap selects in both orientations, removal cleans up", async () =>
   {
      const page = await openPage (TABLET);

      try
      {
         await mobileViewport (page);
         // The canvas fills the page width; its height is a fraction of the page height.
         await page .evaluate (() =>
         {
            const canvas = document .querySelector ("x3d-canvas");

            canvas .setAttribute ("contentScale", "auto");
            canvas .style .cssText = "display:block;width:100vw;height:60vh";
         });

         await loadURL (page, `${server .base}/tests/browser/worlds/sensors.wrl`);

         const cdp   = await page .context () .newCDPSession (page);
         const touch = (type, points) => cdp .send ("Input.dispatchTouchEvent", { type, touchPoints: points .map (([ x, y ], id) => ({ x, y, id })) });
         const frames = () => page .evaluate (async () => { for (let i = 0; i < 4; ++ i) await new Promise (requestAnimationFrame); });
         const state  = () => page .evaluate (() =>
         {
            const browser = document .querySelector ("x3d-canvas") .browser;
            const canvas  = browser .getCanvas ();
            const rect    = canvas .getBoundingClientRect ();

            return { size: [ canvas .width, canvas .height ], css: [ rect .width, rect .height ] };
         });

         // A tap a little left of and below the center: the BackTouch box of sensors.wrl is there in every aspect ratio
         // (the field of view is that of the smaller side), the FrontTouch box (right of and above the center) is not.
         const tapCenter = async () =>
         {
            const { css } = await state ();
            const d       = Math .min (... css) * 0.03;

            await touch ("touchStart", [ [ css [0] / 2 - d, css [1] / 2 + d ] ]);
            await frames ();

            const active = await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("BackTouch") .isActive);

            await touch ("touchEnd", [ ]);
            await frames ();
            return active;
         };

         await frames ();
         assert .deepEqual ((await state ()) .size, [ 1640, 1416 ], "portrait framebuffer");
         assert .equal (await tapCenter (), true, "portrait tap");

         await page .setViewportSize ({ width: 1180, height: 820 });
         await frames ();
         assert .deepEqual ((await state ()) .size, [ 2360, 984 ], "landscape framebuffer");
         assert .equal (await tapCenter (), true, "landscape tap");

         // Removal while a touch is down releases it; the loop stops; restoration starts it again.
         const { css } = await state ();

         await touch ("touchStart", [ [ css [0] / 2 - Math .min (... css) * 0.03, css [1] / 2 + Math .min (... css) * 0.03 ] ]);
         await frames ();

         const out = await page .evaluate (async () =>
         {
            const canvas = document .querySelector ("x3d-canvas");
            const key    = Symbol .for ("X_ITE_R.frameLoop");

            canvas .remove ();

            const removed = { loop: canvas .browser [key] .running, active: canvas .browser .currentScene .getNamedNode ("BackTouch") .isActive };

            document .body .prepend (canvas);
            for (let i = 0; i < 3; ++ i) await new Promise (requestAnimationFrame);

            return { removed, restored: canvas .browser [key] .running };
         });

         await touch ("touchEnd", [ ]);
         assert .deepEqual (out, { removed: { loop: false, active: false }, restored: true });
         assert .deepEqual (page .log .filter (line => line .startsWith ("pageerror")), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });
});
