// Sound 02 tests of X_ITE_R's <x3d-canvas> that X_ITE has no reference for (issue #140): the page's audio clock across a
// replaceWorld while the canvas is detached, and the page-owned AudioContext at disposal. No pinned recording applies:
// the cases look at X_ITE_R's page integration (web/x_ite_r.mjs), not at what a Sound node does.

import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { CASE_TIMEOUT, DESKTOP, ONLY, openPage, server } from "./runtime02_harness.mjs";

// Keeps every AudioContext the page makes in window .contexts, before the world loads.
const KEEP_CONTEXTS = () =>
{
   const Base = window .AudioContext;

   window .contexts = [ ];
   window .AudioContext = class extends Base
   {
      constructor (... args)
      {
         super (... args);
         window .contexts .push (this);
      }
   };
};

// A clip that does not loop (0.25 s); the test starts it.
const WORLD = base => `#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Sound:2
Viewpoint { position 0 0 10 }
Sound {
  minFront 20 minBack 20 maxFront 40 maxBack 40
  source DEF A AudioClip { url "${base}/media/tone48k-stereo.wav" }
}
`;

const open = () => openPage (DESKTOP, { init: `(${KEEP_CONTEXTS}) ()` });

describe ("sound 02", { skip: Boolean (ONLY), timeout: CASE_TIMEOUT }, () =>
{
   test ("a world replaced while the canvas is detached keeps the page's audio clock", async () =>
   {
      const page = await open ();

      try
      {
         const result = await page .evaluate (async text =>
         {
            const canvas  = document .querySelector ("x3d-canvas");
            const browser = canvas .browser;
            const engine  = browser [Symbol .for ("X_ITE_R.engine")];
            const later   = ms => new Promise (resolve => setTimeout (resolve, ms));
            const clock   = () => JSON .parse (engine .audioGraph ()) .currentTime;

            await browser .replaceWorld (await browser .createX3DFromString (text));

            // The page's context runs (this evaluate has a user activation) and its clock passes the clip's length.
            while (!window .contexts .length || window .contexts [0] .currentTime < 1.5)
               await later (50);

            await later (200);

            const before = clock ();
            const parent = canvas .parentNode;

            canvas .remove ();

            // No frame runs while the canvas is detached, but the new world is installed (replaceWorld waits for a frame).
            const replaced = browser .replaceWorld (await browser .createX3DFromString (text));

            await later (500);

            const detached = clock ();

            parent .append (canvas);
            await replaced;

            const events = [ ];

            browser .currentScene .getNamedNode ("A") .addFieldCallback ("sound02", "isActive", value => events .push ({ value, at: performance .now () }));
            browser .currentScene .getNamedNode ("A") .startTime = browser .currentTime;
            await later (1000);

            return { before, detached, events, contexts: window .contexts .length };
         },
         WORLD (server .base));

         assert .equal (result .contexts, 1);
         assert .ok (result .before >= 1.5, `the clock before: ${result .before}`);
         assert .ok (result .detached >= result .before, `the new world's clock ${result .detached} is not the page's ${result .before}`);
         assert .deepEqual (result .events .map (e => e .value), [ true, false ], "the clip starts and ends once");
         assert .ok (result .events [1] .at - result .events [0] .at >= 150, `the clip played ${result .events [1] .at - result .events [0] .at} ms of its 250 ms`);
         assert .deepEqual (page .log .filter (line => line .startsWith ("pageerror")), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("dispose closes the page's AudioContext once, and not an AudioContext of the page's own", async () =>
   {
      const page = await open ();

      try
      {
         const result = await page .evaluate (async text =>
         {
            const browser = document .querySelector ("x3d-canvas") .browser;
            const later   = ms => new Promise (resolve => setTimeout (resolve, ms));

            await browser .replaceWorld (await browser .createX3DFromString (text));

            while (!window .contexts .length)
               await later (50);

            // The context stays through a replaceWorld.
            const context = window .contexts [0];

            await browser .replaceWorld (await browser .createX3DFromString (text));
            await later (200);

            const reused = { count: window .contexts .length, state: context .state, same: window .contexts [0] === context };
            const own    = new AudioContext ();

            browser .dispose ();
            browser .dispose ();

            for (let i = 0; i < 100 && context .state !== "closed"; ++ i)
               await later (20);

            return { reused, closed: context .state, own: own .state, count: window .contexts .length };
         },
         WORLD (server .base));

         assert .deepEqual (result .reused, { count: 1, state: result .reused .state, same: true });
         assert .notEqual (result .reused .state, "closed");
         assert .equal (result .closed, "closed");
         assert .notEqual (result .own, "closed");
         assert .equal (result .count, 2, "the page's own context and the browser's, and no new one after dispose");
         assert .deepEqual (page .log .filter (line => line .startsWith ("pageerror")), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });
});
