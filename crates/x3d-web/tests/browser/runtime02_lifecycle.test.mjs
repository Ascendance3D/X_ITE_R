// Browser runtime 02 tests of X_ITE_R's <x3d-canvas> that X_ITE has no reference for: the frame loop lifecycle, the
// review cases, and the canvas size and scroll behavior (runtime02_harness.mjs).

import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { CASE_TIMEOUT, DESKTOP, ONLY, frameLoop, loadURL, openPage, server } from "./runtime02_harness.mjs";

describe ("runtime 02 lifecycle", { skip: Boolean (ONLY), timeout: CASE_TIMEOUT }, () =>
{
   test ("repeated loads keep one frame loop and end on the last world", async () =>
   {
      const page = await openPage (DESKTOP);

      try
      {
         for (let i = 0; i < 6; ++ i)
            await loadURL (page, `${server .base}/tests/browser/worlds/${i % 2 ? "loading" : "viewpoints"}.wrl`);

         // The frames of the loop against the page's own animation frames over the same time: one loop, one frame each.
         const { loop, raf } = await page .evaluate (async () =>
         {
            const browser = document .querySelector ("x3d-canvas") .browser;
            const key     = Symbol .for ("X_ITE_R.frameLoop");
            const start   = browser [key] .frames;
            let raf       = 0;

            for (const end = performance .now () + 500; performance .now () < end;)
               await new Promise (r => requestAnimationFrame (() => r (++ raf)));

            return { loop: browser [key] .frames - start, raf };
         });

         assert .ok (loop <= raf + 1 && loop >= raf - 1, `loop frames ${loop}, page frames ${raf}`);
         assert .equal (await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .currentScene .worldURL .replace (/^.*\//, "")), "loading.wrl");
         assert .deepEqual (page .log .filter (line => line .startsWith ("pageerror")), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("a hidden page draws no frames; it draws again when shown", async () =>
   {
      const page = await openPage (DESKTOP);

      try
      {
         const setVisibility = state => page .evaluate (state =>
         {
            Object .defineProperty (document, "visibilityState", { configurable: true, get: () => state });
            document .dispatchEvent (new Event ("visibilitychange"));
         },
         state);

         await setVisibility ("hidden");

         const hidden = await frameLoop (page);

         await page .waitForTimeout (300);
         assert .equal (hidden .running, false);
         assert .equal ((await frameLoop (page)) .frames, hidden .frames, "frames while hidden");

         await setVisibility ("visible");
         await page .waitForTimeout (300);

         const shown = await frameLoop (page);

         assert .equal (shown .running, true);
         assert .ok (shown .frames > hidden .frames, "frames after shown");
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("removal stops the loop, restoration starts one loop again", async () =>
   {
      const page = await openPage (DESKTOP);

      try
      {
         await page .evaluate (() => { window .removed = document .querySelector ("x3d-canvas"); window .removed .remove (); });

         const removed = await page .evaluate (() => window .removed .browser [Symbol .for ("X_ITE_R.frameLoop")]);

         assert .equal (removed .running, false);

         for (let i = 0; i < 3; ++ i)
         {
            await page .evaluate (() => window .removed .remove ());
            await page .evaluate (() => document .body .prepend (window .removed));
         }

         const { loop, raf } = await page .evaluate (async () =>
         {
            const browser = document .querySelector ("x3d-canvas") .browser;
            const key     = Symbol .for ("X_ITE_R.frameLoop");
            const start   = browser [key] .frames;
            let raf       = 0;

            for (const end = performance .now () + 400; performance .now () < end;)
               await new Promise (r => requestAnimationFrame (() => r (++ raf)));

            return { loop: browser [key] .frames - start, raf };
         });

         assert .ok (loop <= raf + 1 && loop >= raf - 1, `loop frames ${loop}, page frames ${raf}`);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("dispose stops the loop, removes the input listeners and frees the engine", async () =>
   {
      const page = await openPage (DESKTOP);

      try
      {
         await loadURL (page, `${server .base}/tests/browser/worlds/sensors.wrl`);

         const out = await page .evaluate (async () =>
         {
            const browser = document .querySelector ("x3d-canvas") .browser;

            browser .dispose ();
            browser .getCanvas () .dispatchEvent (new PointerEvent ("pointerdown", { clientX: 91, clientY: 150, pointerId: 1 }));
            await new Promise (r => setTimeout (r, 200));

            return {
               loop:   browser [Symbol .for ("X_ITE_R.frameLoop")] .running,
               engine: browser [Symbol .for ("X_ITE_R.engine")],
               load:   await browser .loadURL (new window .X3D_NS .MFString ("worlds/viewpoints.wrl")) .then (() => "resolved", e => e .message),
            };
         });

         assert .deepEqual (out, { loop: false, engine: null, load: "Loading of X3D file aborted." });
         assert .deepEqual (page .log .filter (line => line .startsWith ("pageerror")), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("a lost device calls CONNECTION_ERROR, stops the loop and shows a message", async () =>
   {
      const page = await openPage (DESKTOP);

      try
      {
         const out = await page .evaluate (async () =>
         {
            const X3D     = window .X3D_NS;
            const canvas  = document .querySelector ("x3d-canvas");
            const browser = canvas .browser;
            const events  = [ ];

            browser .addBrowserCallback ("lost", event => events .push (event === X3D .X3DConstants .CONNECTION_ERROR ? "CONNECTION_ERROR" : event));
            browser [Symbol .for ("X_ITE_R.engine")] .loseDevice ();

            for (let i = 0; i < 60 && !events .length; ++ i)
               await new Promise (r => setTimeout (r, 50));

            return {
               events,
               loop:    browser [Symbol .for ("X_ITE_R.frameLoop")] .running,
               message: canvas .shadowRoot .querySelector ("slot") ?.textContent,
               canvas:  canvas .shadowRoot .contains (browser .getCanvas ()),
            };
         });

         assert .deepEqual (out, { events: [ "CONNECTION_ERROR" ], loop: false, message: "X_ITE_R stopped: the WebGPU device is lost.", canvas: false });
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("without WebGPU the element shows a message, not an empty canvas", async () =>
   {
      const page = await openPage (DESKTOP, { init: () => { delete Navigator .prototype .gpu; } });

      try
      {
         const out = await page .evaluate (async () =>
         {
            await new Promise (r => setTimeout (r, 300));

            const canvas = document .querySelector ("x3d-canvas");

            return { startError: window .startError ?? null, message: canvas .shadowRoot .querySelector ("slot") ?.textContent ?? null };
         });

         assert .match (out .startError, /WebGPU|gpu|adapter/i);
         assert .match (out .message, /^X_ITE_R cannot start: /);
      }
      finally
      {
         await page .context () .close ();
      }
   });
});

describe ("runtime 02 review cases", { skip: Boolean (ONLY), timeout: CASE_TIMEOUT }, () =>
{
   test ("replaceWorld twice with one scene, and a replaced page scene stays usable", async () =>
   {
      const page = await openPage (DESKTOP);

      try
      {
         const out = await page .evaluate (async () =>
         {
            const browser = document .querySelector ("x3d-canvas") .browser;
            const a       = await browser .createX3DFromString ("#VRML V2.0 utf8\nDEF A Transform { }");
            const b       = await browser .createX3DFromString ("#VRML V2.0 utf8\nDEF B Transform { }");
            const first   = browser .replaceWorld (a) .then (() => "resolved", e => e .message);
            const second  = browser .replaceWorld (a) .then (() => "resolved", e => e .message);
            const same    = { first: await first, second: await second, current: browser .currentScene === a };

            // A newer replaceWorld (c) replaces the call for b, which was never the world: b is still usable.
            const c      = await browser .createX3DFromString ("#VRML V2.0 utf8\nDEF C Transform { }");
            const third  = browser .replaceWorld (b) .then (() => "resolved", e => e .message);
            const fourth = browser .replaceWorld (c) .then (() => "resolved", e => e .message);

            await third;
            await fourth;

            const later = await browser .replaceWorld (b) .then (() => "resolved", e => e .message);

            return { same, bNode: b .getNamedNode ("B") .getNodeTypeName (), later, current: browser .currentScene === b };
         });

         assert .deepEqual (out, {
            same: { first: "Replacing world aborted.", second: "resolved", current: true },
            bNode: "Transform",
            later: "resolved",
            current: true,
         });
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("an Anchor to a script URL in any case is not opened; a new window has no opener", async () =>
   {
      const page = await openPage (DESKTOP);

      try
      {
         await page .evaluate (() =>
         {
            window .opened = [ ];
            window .open   = (... args) => { window .opened .push (args); return null; };
         });

         const click = async (url, parameter) =>
         {
            await page .evaluate (async ([ url, parameter ]) =>
            {
               const browser = document .querySelector ("x3d-canvas") .browser;

               await browser .replaceWorld (await browser .createX3DFromString (`#VRML V2.0 utf8
                  Viewpoint { position 0 0 4 }
                  Anchor { url "${url}" parameter "${parameter}" children Shape { geometry Box { size 3 3 0.1 } } }`));
            },
            [ url, parameter ]);

            // Picking needs a drawn frame of the new world; a CPU renderer (lavapipe) draws its first frames slowly.
            await page .evaluate (async () =>
            {
               const browser = document .querySelector ("x3d-canvas") .browser;
               const key     = Symbol .for ("X_ITE_R.frameLoop");
               const start   = browser [key] .frames;

               while (browser [key] .frames < start + 3)
                  await new Promise (requestAnimationFrame);
            });

            const before = page .log .length;

            await page .mouse .click (200, 150);

            // The click is done when the Anchor's URL was handled: opened, refused or failed.
            for (let i = 0; i < 100; ++ i)
            {
               const opened = await page .evaluate (() => window .opened .length);

               if (opened || page .log .slice (before) .some (line => /was not opened|Couldn't load URL/ .test (line)))
                  break;

               await page .waitForTimeout (100);
            }
         };

         await click ("JaVaScRiPt:window.hacked=1", "target=_blank");
         await click ("JAVASCRIPT:window.hacked=2", "");
         await click ("http://other.test/world.wrl", "target=_blank");

         const out = await page .evaluate (() => ({ hacked: window .hacked ?? null, opened: window .opened, url: location .pathname }));

         assert .deepEqual (out, { hacked: null, opened: [ [ "http://other.test/world.wrl", "_blank", "noopener" ] ], url: "/tests/browser/runtime02.html" });

         // With a target the URL is foreign and the adapter does not open it. Without one it is a world URL: fetch
         // refuses the scheme and the load fails, like X_ITE's for a script URL that is not lower case.
         assert .equal (page .log .filter (line => /Anchor URL 'javascript:window.hacked=1' was not opened/ .test (line)) .length, 1, page .log .join ("\n"));
         assert .equal (page .log .filter (line => /Anchor: Couldn't load URL 'javascript:window.hacked=2'/ .test (line)) .length, 1, page .log .join ("\n"));
      }
      finally
      {
         await page .context () .close ();
      }
   });
});

describe ("runtime 02 canvas size and scroll", { skip: Boolean (ONLY), timeout: CASE_TIMEOUT }, () =>
{
   test ("framebuffer follows CSS size, contentScale and the device limit", async () =>
   {
      const page = await openPage ({ viewport: { width: 1200, height: 900 }, deviceScaleFactor: 2 });

      try
      {
         const size = async (style, scale) =>
         {
            await page .evaluate (([ style, scale ]) =>
            {
               const canvas = document .querySelector ("x3d-canvas");

               canvas .style .cssText = style;

               if (scale === null)
                  canvas .removeAttribute ("contentScale");
               else
                  canvas .setAttribute ("contentScale", scale);
            },
            [ style, scale ]);

            await page .evaluate (async () => { for (let i = 0; i < 4; ++ i) await new Promise (requestAnimationFrame); });

            return page .evaluate (() =>
            {
               const browser = document .querySelector ("x3d-canvas") .browser;
               const canvas  = browser .getCanvas ();

               return [ canvas .width, canvas .height ];
            });
         };

         const max = await page .evaluate (() => document .querySelector ("x3d-canvas") .browser [Symbol .for ("X_ITE_R.engine")] .maxTextureDimension);

         assert .deepEqual (await size ("display:block;width:400px;height:300px", null), [ 400, 300 ], "landscape, contentScale 1");
         assert .deepEqual (await size ("display:block;width:300px;height:400px", null), [ 300, 400 ], "portrait (rotation)");
         assert .deepEqual (await size ("display:block;width:300px;height:400px", "auto"), [ 600, 800 ], "contentScale auto, DPR 2");
         assert .deepEqual (await size ("display:block;width:1px;height:1px", "auto"), [ 2, 2 ], "a small canvas");

         await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .loadURL (new window .X3D_NS .MFString ("worlds/loading.wrl")));
         await size ("display:block;width:400px;height:300px", "auto");
         assert .deepEqual (page .log .filter (line => /pageerror|error:/ .test (line)), [ ]);

         // The device limit, last: a framebuffer of max x max / 2 (16384 x 8192 on this machine) can use more GPU
         // memory than there is. Then the device is lost, and the lost path must take it (message, no page error).
         assert .deepEqual (await size ("display:block;width:1000px;height:500px", String (max / 250)), [ max, max / 2 ], "the device limit");
         await page .waitForTimeout (500);

         const lost = await page .evaluate (() => document .querySelector ("x3d-canvas") .shadowRoot .querySelector ("slot") ?.textContent ?? null);

         assert .ok (lost === null || lost === "X_ITE_R stopped: the WebGPU device is lost.", `after the device limit: ${lost}`);
         assert .deepEqual (page .log .filter (line => line .startsWith ("pageerror")), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("touch-action and wheel: the page scrolls only where the canvas does not navigate", async () =>
   {
      const page = await openPage (DESKTOP);

      try
      {
         const state = async world =>
         {
            await page .evaluate (async world =>
            {
               const browser = document .querySelector ("x3d-canvas") .browser;

               await browser .replaceWorld (await browser .createX3DFromString (world));
            },
            world);

            await page .evaluate (async () => { for (let i = 0; i < 3; ++ i) await new Promise (requestAnimationFrame); });

            return page .evaluate (() =>
            {
               const canvas = document .querySelector ("x3d-canvas") .browser .getCanvas ();
               const wheel  = new WheelEvent ("wheel", { deltaY: 100, clientX: 200, clientY: 150, cancelable: true, bubbles: true });

               canvas .dispatchEvent (wheel);
               return { touchAction: canvas .style .touchAction, wheelPrevented: wheel .defaultPrevented };
            });
         };

         assert .deepEqual (await state ("#VRML V2.0 utf8\nNavigationInfo { type \"EXAMINE\" }\nShape { geometry Box { } }"), { touchAction: "none", wheelPrevented: true });
         assert .deepEqual (await state ("#VRML V2.0 utf8\nNavigationInfo { type \"NONE\" }\nShape { geometry Box { } }"), { touchAction: "auto", wheelPrevented: false });
      }
      finally
      {
         await page .context () .close ();
      }
   });
});
