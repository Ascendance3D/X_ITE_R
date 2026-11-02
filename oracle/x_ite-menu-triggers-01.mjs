/**
 * Menu triggers 01 (issue #287): what opens the pinned X_ITE 16.5.0 context menu. ContextMenu.js listens to the element's
 * `contextmenu` event only (no `touchstart` timer, no Ctrl check), so the triggers are the host's: this records which
 * input makes headless Chrome (Linux) fire `contextmenu` at the element and what the menu does for each event, so
 * crates/x3d-viewer/tests/native_menu_triggers.rs can compare the native viewer.
 *
 *   node oracle/x_ite-menu-triggers-01.mjs <out_dir> [--runs 3]
 *
 * Writes <out_dir>/menu-triggers-01-baseline.json (identical in every run or the run fails). Needs oracle/x_ite-pin/node_modules
 * (see oracle/README.md, "Recording from a worktree") and Playwright (XITE_TESTS, or crates/x3d-web/node_modules).
 */
import { createServer } from "node:http";
import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));

if (!existsSync (join (ROOT, "x_ite-pin", "node_modules")))
   throw new Error ("oracle/x_ite-pin/node_modules is missing: link it first (ln -s ~/Projects/cybertown/X_ITE/x_ite/node_modules oracle/x_ite-pin/node_modules, see oracle/README.md, Recording from a worktree)");

const { chromium } = createRequire (join (process .env .XITE_TESTS ?? join (ROOT, "../crates/x3d-web"), "/")) ("playwright");

const argv = process .argv .slice (2);
const opt  = (name, def) => { const i = argv .indexOf (name); return i >= 0 ? argv [i + 1] : def; };
const OUT  = argv .find ((a, i) => !a .startsWith ("--") && !argv [i - 1] ?.startsWith ("--"));
const RUNS = Number (opt ("--runs", "3"));

if (!OUT)
   throw new Error ("usage: node oracle/x_ite-native-keys-01.mjs <out_dir> [--runs 3]");

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json", ".png": "image/png" };

const SENSOR_WORLD = `#VRML V2.0 utf8
Group { children [ TouchSensor { } Shape { appearance Appearance { material Material { } } geometry Box { } } ] }
`;

const WORLD = `#VRML V2.0 utf8
Shape { appearance Appearance { material Material { } } geometry Box { } }
`;

const PAGE = `<!DOCTYPE html>
<meta charset="utf-8">
<style>html, body { margin: 0; padding: 0; background: #000; }</style>
<script src="/x_ite-pin/node_modules/libtess/libtess.cat.js"></script>
<script src="/x_ite-pin/node_modules/@create3000/libgif/libgif.js"></script>
<script src="/x_ite-pin/node_modules/apng-js/lib/index.js"></script>
<x3d-canvas id="c" splashScreen="false" contentScale="1" tabindex="0" style="display:block;position:absolute;left:0;top:0;width:320px;height:320px"></x3d-canvas>
<script type="module">
await import ("./x_ite.js");
window .ready = true;
</script>
`;

const server = createServer (async (req, res) =>
{
   const url = new URL (req .url, "http://x");

   if (url .pathname === "/x_ite-pin/src/menu-triggers-01.html")
   {
      res .writeHead (200, { "Content-Type": "text/html", "Cache-Control": "no-store" });
      return res .end (PAGE);
   }

   const path = normalize (join (ROOT, decodeURIComponent (url .pathname)));

   if (!path .startsWith (ROOT)) { res .writeHead (403); return res .end (); }

   try
   {
      const body = await readFile (path);
      res .writeHead (200, { "Content-Type": TYPES [extname (path) .toLowerCase ()] ?? "application/octet-stream", "Cache-Control": "no-store" });
      res .end (body);
   }
   catch { res .writeHead (404, { "Cache-Control": "no-store" }); res .end (); }
});

await new Promise (r => server .listen (0, "127.0.0.1", r));

const base = `http://127.0.0.1:${server .address () .port}`;

// The keys. `real`: Playwright's keyboard (key name); `synth`: a KeyboardEvent with the keyCode. `code` is the keyCode the native
// test replays.
const frames = page => page .evaluate (() => new Promise (r => requestAnimationFrame (() => requestAnimationFrame (() => requestAnimationFrame (() => r ())))));

// The page keeps a log of the events that reach the element, and reads the state of the menu.
const INSTALL = () =>
{
   const canvas = document .querySelector ("x3d-canvas");

   window .log = [ ];

   for (const type of [ "pointerdown", "mousedown", "touchstart", "touchmove", "touchend", "touchcancel", "contextmenu", "mouseup", "click" ])
   {
      canvas .addEventListener (type, event => window .log .push ({
         type, button: event .button ?? null, ctrl: event .ctrlKey ?? null, x: Math .round (event .clientX ?? event .touches ?.[0] ?.clientX ?? -1),
         y: Math .round (event .clientY ?? event .touches ?.[0] ?.clientY ?? -1), trusted: event .isTrusted, pointer: event .pointerType ?? null,
      }), true);
   }
};

const menuState = page => page .evaluate (() =>
{
   const shadow = document .querySelector ("x3d-canvas") .shadowRoot;
   const layer  = shadow .querySelector (".context-menu-layer");
   const ul     = shadow .querySelector ("ul.context-menu-root");
   const rect   = ul ?.getBoundingClientRect ();

   return {
      open: !!layer && !!ul && ul .style .display === "block",
      left: ul ? Math .round (rect .left) : null, top: ul ? Math .round (rect .top) : null,
      first: ul ?.querySelector (".context-menu-item span") ?.textContent ?? null,
   };
});

const reset = async page =>
{
   await page .evaluate (() =>
   {
      const layer = document .querySelector ("x3d-canvas") .shadowRoot .querySelector (".context-menu-layer");

      layer ?.dispatchEvent (new MouseEvent ("mousedown", { bubbles: true }));
   });
   await page .waitForTimeout (1200);
   await page .evaluate (() => { window .log = [ ]; });
};

const synth = (page, events) => page .evaluate (events =>
{
   const canvas = document .querySelector ("x3d-canvas");

   for (const e of events)
      canvas .dispatchEvent (new MouseEvent (e .type, { bubbles: true, cancelable: true, composed: true, button: e .button, buttons: e .buttons ?? 0,
         ctrlKey: !!e .ctrl, clientX: e .x, clientY: e .y }));
}, events);

async function touch (page, cdp, { x = 100, y = 100, hold, move = 0 })
{
   await cdp .send ("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [ { x, y } ] });

   if (move)
   {
      await page .waitForTimeout (50);
      await cdp .send ("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [ { x: x + move, y } ] });
   }

   await page .waitForTimeout (hold);
   await cdp .send ("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [ ] });
}

const CASES = {
   rightClick:        async page => page .mouse .click (100, 100, { button: "right" }),
   leftClick:         async page => page .mouse .click (100, 100),
   // Chrome on Linux and Windows: Ctrl+click is a left click (macOS gives it a `contextmenu` event).
   ctrlClickReal:     async page => { await page .keyboard .down ("Control"); await page .mouse .click (100, 100); await page .keyboard .up ("Control"); },
   // What Chrome on macOS sends for a Ctrl+click (assumed from Chromium's behavior, not recorded on macOS): a left press with
   // ctrlKey, the `contextmenu` event of button 0 with ctrlKey, then the release.
   ctrlClickMacSequence: async page => synth (page, [ { type: "mousedown", button: 0, buttons: 1, ctrl: true, x: 100, y: 100 },
      { type: "contextmenu", button: 0, buttons: 1, ctrl: true, x: 100, y: 100 }, { type: "mouseup", button: 0, ctrl: true, x: 100, y: 100 } ]),
   contextmenuButton0: async page => synth (page, [ { type: "contextmenu", button: 0, x: 140, y: 120 } ]),
   contextmenuCtrl:   async page => synth (page, [ { type: "contextmenu", button: 0, ctrl: true, x: 140, y: 120 } ]),
   touchTap:          async (page, cdp) => touch (page, cdp, { hold: 50 }),
   touchHold400:      async (page, cdp) => touch (page, cdp, { hold: 400 }),
   touchHold700:      async (page, cdp) => touch (page, cdp, { hold: 700 }),
   // PointingDevice.js: CONTEXT_MENU_TIME is 1200 ms; the hold below brackets it (the touchend comes after the menu opened).
   touchHold1000:     async (page, cdp) => touch (page, cdp, { hold: 1000 }),
   touchHold1100:     async (page, cdp) => touch (page, cdp, { hold: 1100 }),
   touchHold1300:     async (page, cdp) => touch (page, cdp, { hold: 1300 }),
   touchHold1500:     async (page, cdp) => touch (page, cdp, { hold: 1500 }),
   // A finger that moves more than 7 px from the start cancels it (the move is 50 ms after the start).
   touchHold1500Move6:  async (page, cdp) => touch (page, cdp, { hold: 1500, move: 6 }),
   touchHold1500Move7:  async (page, cdp) => touch (page, cdp, { hold: 1500, move: 7 }),
   touchHold1500Move8:  async (page, cdp) => touch (page, cdp, { hold: 1500, move: 8 }),
   touchHold1500Move40: async (page, cdp) => touch (page, cdp, { hold: 1500, move: 40 }),
   // A touch that a sensor takes does not start the timer: the box under the finger has a TouchSensor.
   touchHold1500OnSensor: { world: SENSOR_WORLD, run: async (page, cdp) => touch (page, cdp, { x: 160, y: 160, hold: 1500 }) },
   // The same hold on the box without a sensor: the menu opens.
   touchHold1500OnShape: { world: WORLD, run: async (page, cdp) => touch (page, cdp, { x: 160, y: 160, hold: 1500 }) },
};

async function once ()
{
   const browser = await chromium .launch ({
      executablePath: "/usr/bin/google-chrome",
      headless: true,
      args: [ "--headless=new", "--no-sandbox", "--use-gl=angle", "--use-angle=gl", "--enable-gpu", "--ignore-gpu-blocklist" ],
   });

   try
   {
      const context = await browser .newContext ({ viewport: { width: 320, height: 320 }, deviceScaleFactor: 1, hasTouch: true });

      await context .route (url => !url .href .startsWith (base), route => route .abort ());

      const page = await context .newPage ();
      const cdp  = await context .newCDPSession (page);

      await page .goto (`${base}/x_ite-pin/src/menu-triggers-01.html`);
      await page .waitForFunction (() => window .ready, null, { timeout: 30000 });
      await page .waitForFunction (() => document .querySelector ("x3d-canvas") ?.browser ?.currentScene, null, { timeout: 30000 });
      await page .evaluate (async world =>
      {
         const browser = document .querySelector ("x3d-canvas") .browser;

         await browser .replaceWorld (await browser .createX3DFromString (world));
      }, WORLD);
      await page .evaluate (INSTALL);
      await frames (page);

      const values = { };

      for (const [ name, entry ] of Object .entries (CASES))
      {
         const action = typeof entry === "function" ? entry : entry .run;

         await page .evaluate (async world =>
         {
            const browser = document .querySelector ("x3d-canvas") .browser;

            await browser .replaceWorld (await browser .createX3DFromString (world));
         }, entry .world ?? WORLD);
         await frames (page);
         await page .evaluate (() => { window .log = [ ]; });
         await action (page, cdp);
         await frames (page);
         await page .waitForTimeout (200);
         values [name] = { events: await page .evaluate (() => window .log), menu: await menuState (page) };
         await reset (page);
      }

      return values;
   }
   finally
   {
      await browser .close ();
   }
}

let first = null;

for (let run = 1; run <= RUNS; ++ run)
{
   console .log (`run ${run} of ${RUNS}`);

   const values = await once ();

   if (first === null)
      first = values;
   else if (JSON .stringify (first) !== JSON .stringify (values))
      throw new Error (`run ${run} differs from run 1`);
}

await mkdir (OUT, { recursive: true });
await writeFile (join (OUT, "menu-triggers-01-baseline.json"), JSON .stringify ({ pinned: "X_ITE 16.5.0 34cdf9f97", runs: RUNS, cases: first }, null, 1) + "\n");
server .close ();
console .log (`${RUNS} runs, identical values, ${Object .keys (first) .length} cases`);
