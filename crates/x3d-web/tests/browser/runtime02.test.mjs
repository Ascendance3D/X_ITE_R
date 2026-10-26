// Browser runtime 02 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: input, touch,
// coordinates, resource loading and the scene API, compared with the pinned X_ITE 16.5.0 recording in
// oracle/web-browser-02-baseline (oracle/x_ite-web-runtime-02.mjs); then the lifecycle and mobile-emulation checks
// X_ITE has no reference for.
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)
//
// XR_CASE=<group.case> runs one reference case. XR_WEB_EVIDENCE=<dir> writes this run's values and pictures there.

import { after, before, describe, test } from "node:test";
import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { compareImages } from "./api_case.mjs";
import { CASE_GROUPS, splitPictures } from "./runtime02_case.mjs";
import { startServer } from "./server.mjs";

const CRATE    = normalize (join (fileURLToPath (import.meta.url), "../../.."));
const REPO     = join (CRATE, "../..");
const BASELINE = process .env .XR_BASELINE ?? join (REPO, "oracle/web-browser-02-baseline");
const EVIDENCE = process .env .XR_WEB_EVIDENCE;
const ONLY     = process .env .XR_CASE;

const reference = JSON .parse (await readFile (join (BASELINE, "runtime02-baseline.json"), "utf8"));

// The values where X_ITE_R knowingly differs from X_ITE 16.5.0: case → [ path, X_ITE's value, X_ITE_R's value ].
// The X_ITE value is checked against the recording, so a new recording cannot hide a difference. Each entry is
// listed in web/API.md.
const KNOWN_DIFFERENCES = {
   "input.interrupted": [
      // X_ITE has no touchcancel handler: the TouchSensor stays active. X_ITE_R releases it (pointercancel).
      [ [ "touchCancelled", "LeftTouch.isActive" ], true, false ],
      // X_ITE's mouseup listener is on the document and still gets the release after the canvas is removed;
      // X_ITE_R releases the buttons that are down when the canvas leaves the document.
      [ [ "removed" ], "true", "false" ],
      // X_ITE's release after the removal checks the pointer against the detached canvas (an empty rectangle, so a
      // NaN pointer and no hit) and ends isOver. X_ITE_R's pointer has not moved: it is still over the box.
      [ [ "restored", "LeftTouch.isOver" ], false, true ],
   ],
   "loading.failures": [
      // X_ITE names the blob URL of the image data that does not decode; the Rust engine has none and names the file.
      [ [ "loading-invalid", "console", 0 ],
         "error: ImageTexture: Couldn't load URL 'http://test/garbage/texture.png'. Error: Couldn't load image 'blob:http://test/ID': error.",
         "error: ImageTexture: Couldn't load URL 'http://test/garbage/texture.png'. Error: Couldn't load image 'http://test/garbage/texture.png': error." ],
   ],
   "loading.repeated": [
      // X_ITE fetches once per node (the HTTP cache may answer); X_ITE_R fetches a URL once per world.
      [ [ "texture" ], 2, 1 ],
      [ [ "inline" ], 2, 1 ],
   ],
   "loading.disposedDuringLoad": [
      // X_ITE's loadURL never settles after dispose (); X_ITE_R's rejects.
      [ [ "result" ], "pending", "Loading of X3D file aborted." ],
   ],
};

let server, browser;

before (async () =>
{
   server  = await startServer (CRATE);
   browser = await chromium .launch ({
      headless: true,
      args: [ "--enable-unsafe-webgpu", "--enable-features=Vulkan,VulkanFromANGLE", "--use-angle=vulkan",
         "--use-vulkan=native", "--ignore-gpu-blocklist", "--enable-gpu" ],
   });
});

after (async () =>
{
   await browser ?.close ();
   await server ?.close ();
});

// A new page with the test canvas in a new context, ready (engine and WebGPU device made).
async function openPage (contextOptions, { init } = { })
{
   const context = await browser .newContext (contextOptions);
   const allowed = [ server .base, server .foreign ];

   await context .route (url => !allowed .some (origin => url .href .startsWith (origin)), route => route .abort ());

   if (init)
      await context .addInitScript (init);

   const page = await context .newPage ();

   page .log = [ ];
   page .on ("console", m => page .log .push (`${m .type ()}: ${m .text () .split ("\n") [0]}`));
   page .on ("pageerror", e => page .log .push (`pageerror: ${e .message}`));

   await page .goto (`${server .base}/tests/browser/runtime02.html`);
   await page .waitForFunction (() => window .ready || window .startError, null, { timeout: 30000 });

   if (!init)
   {
      const startError = await page .evaluate (() => window .startError);

      assert .equal (startError, undefined, `X_ITE_R did not start: ${startError}\n${page .log .join ("\n")}`);
   }

   return page;
}

// The picture the Rust renderer made, read back from the GPU (WebBrowser.readPixels) as a PNG. Headless Chromium does
// not always composite a WebGPU canvas (no display compositor, e.g. on lavapipe), so a page screenshot could be empty.
async function picture (page)
{
   const base64 = await page .evaluate (async () =>
   {
      const canvas = document .querySelector ("x3d-canvas");
      const engine = canvas .browser [Symbol .for ("X_ITE_R.engine")];
      const pixels = await engine .readPixels ();
      const { width, height } = canvas .browser .getCanvas ();
      const image  = new OffscreenCanvas (width, height);

      image .getContext ("2d") .putImageData (new ImageData (new Uint8ClampedArray (pixels .buffer), width, height), 0, 0);

      const bytes = new Uint8Array (await (await image .convertToBlob ({ type: "image/png" })) .arrayBuffer ());

      return btoa (Array .from (bytes, b => String .fromCharCode (b)) .join (""));
   });

   return Buffer .from (base64, "base64");
}

const env = () => ({
   base: server .base,
   foreign: server .foreign,
   worlds: `${server .base}/tests/browser/worlds/`,
   server,
   picture,
});

const frameLoop = page => page .evaluate (() => document .querySelector ("x3d-canvas") .browser [Symbol .for ("X_ITE_R.frameLoop")]);
const loadURL   = (page, url) => page .evaluate (url => document .querySelector ("x3d-canvas") .browser .loadURL (new window .X3D_NS .MFString (url)), url);

async function evidence (name, data)
{
   if (!EVIDENCE)
      return;

   await mkdir (EVIDENCE, { recursive: true });
   await writeFile (join (EVIDENCE, name), data);
}

// The value at `path`, and a copy of `value` with `replacement` there.
const at = (value, path) => path .reduce ((v, k) => v ?.[k], value);

function withValue (value, path, replacement)
{
   const copy = structuredClone (value);

   path .slice (0, -1) .reduce ((v, k) => v [k], copy) [path .at (-1)] = replacement;
   return copy;
}

// --- reference cases ----------------------------------------------------------------------------------------------

for (const [ groupName, group ] of Object .entries (CASE_GROUPS))
{
   describe (`runtime 02 ${groupName} cases, compared with pinned X_ITE`, () =>
   {
      for (const [ name, run ] of Object .entries (group .cases))
      {
         const key = `${groupName}.${name}`;

         test (key, { skip: ONLY && ONLY !== key }, async () =>
         {
            const page = await openPage (group .context);

            try
            {
               const pictures = { };
               const ours     = splitPictures (await run (page, env ()), key, pictures);
               let   theirs   = reference .values [key];

               assert .ok (theirs, `no X_ITE recording of ${key}`);

               await evidence (`${key}.json`, JSON .stringify (ours, null, 1) + "\n");

               for (const [ path, xite, xiteR ] of KNOWN_DIFFERENCES [key] ?? [ ])
               {
                  assert .deepEqual (at (theirs, path), xite, `${key}: the recording no longer has the known difference at ${path .join (".")}`);
                  theirs = withValue (theirs, path, xiteR);
               }

               assert .deepEqual (ours, theirs, `${key}\n${page .log .join ("\n")}`);

               // The pictures: the same background, silhouette and colors as X_ITE's (the thresholds of runtime 01).
               for (const [ file, data ] of Object .entries (pictures))
               {
                  const recorded = await readFile (join (BASELINE, file));
                  const m        = await page .evaluate (compareImages, [ recorded .toString ("base64"), data .toString ("base64") ]);

                  await evidence (file, data);
                  await evidence (`${file}.metrics.json`, JSON .stringify (m, null, 1) + "\n");

                  assert .deepEqual (m .backgroundB, m .backgroundA, `${file}: background`);
                  assert .ok (m .silhouetteIoU >= 0.97, `${file}: silhouette IoU ${m .silhouetteIoU}`);
                  assert .ok (m .meanAbsDiff <= 8, `${file}: mean color difference ${m .meanAbsDiff}`);
               }

               assert .deepEqual (page .log .filter (line => line .startsWith ("pageerror")), [ ], "page errors");
            }
            finally
            {
               await page .context () .close ();
            }
         });
      }
   });
}

// --- lifecycle and mobile emulation -------------------------------------------------------------------------------

const DESKTOP = { viewport: { width: 400, height: 340 }, deviceScaleFactor: 1 };

// A phone-sized emulation: no proof of a mobile browser or a mobile GPU (see web/API.md, "Mobile support").
const PHONE = { viewport: { width: 390, height: 844 }, deviceScaleFactor: 3, isMobile: true, hasTouch: true };

// A mobile page's viewport: without `<meta name="viewport">` a mobile browser lays the page out 980 px wide.
const mobileViewport = page => page .evaluate (async () =>
{
   const meta = Object .assign (document .createElement ("meta"), { name: "viewport", content: "width=device-width, initial-scale=1" });

   document .head .append (meta);

   for (let i = 0; i < 3; ++ i)
      await new Promise (requestAnimationFrame);
});

// A tablet-sized emulation, portrait first.
const TABLET = { viewport: { width: 820, height: 1180 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true };

describe ("runtime 02 lifecycle", { skip: Boolean (ONLY) }, () =>
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

describe ("runtime 02 canvas size and scroll", { skip: Boolean (ONLY) }, () =>
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

describe ("runtime 02 phone and tablet emulation (no proof of mobile GPU support)", { skip: Boolean (ONLY) }, () =>
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
