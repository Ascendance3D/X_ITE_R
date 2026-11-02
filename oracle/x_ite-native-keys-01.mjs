/**
 * Native keys 01: record what the pinned X_ITE 16.5.0 does for each keyboard shortcut of X3DCoreContext (`_keydown` / `_keyup`),
 * so crates/x3d-viewer/tests/native_keys_01.rs can replay the same keys on the native viewer and compare.
 *
 *   node oracle/x_ite-native-keys-01.mjs <out_dir> [--runs 3]
 *
 * Writes <out_dir>/native-keys-01-baseline.json (the values, identical in every run or the run fails). Headless Chrome with
 * the real keyboard of Playwright; the keys Playwright cannot produce (keyCode 171, the numeric keypad's plus 107, Command 91, Alt
 * Gr 225) are dispatched as KeyboardEvents with that keyCode, which is all X_ITE reads. A page of its own (served here, no
 * file of the pinned source is touched) holds one <x3d-canvas> with the host page's option defaults except the splash screen.
 *
 * Needs oracle/x_ite-pin/node_modules (see oracle/README.md, "Recording from a worktree").
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

const { chromium } = createRequire (join (ROOT, "../../../x_ite-tests/")) ("playwright");

const argv = process .argv .slice (2);
const opt  = (name, def) => { const i = argv .indexOf (name); return i >= 0 ? argv [i + 1] : def; };
const OUT  = argv .find ((a, i) => !a .startsWith ("--") && !argv [i - 1] ?.startsWith ("--"));
const RUNS = Number (opt ("--runs", "3"));

if (!OUT)
   throw new Error ("usage: node oracle/x_ite-native-keys-01.mjs <out_dir> [--runs 3]");

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json", ".png": "image/png" };

const WORLD = `#VRML V2.0 utf8
DEF V1 Viewpoint { description "A" position 1 2 10 orientation 0 1 0 0.5 centerOfRotation 0 1 0 fieldOfView 0.6 }
DEF V2 Viewpoint { description "B" position 0 0 12 }
DEF V3 Viewpoint { description "C" position 0 0 14 }
Shape { appearance Appearance { material Material { } } geometry Box { } }
`;

const PAGE = `<!DOCTYPE html>
<meta charset="utf-8">
<style>html, body { margin: 0; padding: 0; background: #000; }</style>
<script src="/x_ite-pin/node_modules/libtess/libtess.cat.js"></script>
<script src="/x_ite-pin/node_modules/@create3000/libgif/libgif.js"></script>
<script src="/x_ite-pin/node_modules/apng-js/lib/index.js"></script>
<x3d-canvas id="c" splashScreen="false" contentScale="1" tabindex="0" style="display:block;width:256px;height:256px"></x3d-canvas>
<script type="module">
await import ("./x_ite.js");
window .ready = true;
</script>
`;

const server = createServer (async (req, res) =>
{
   const url = new URL (req .url, "http://x");

   if (url .pathname === "/x_ite-pin/src/native-keys-01.html")
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
const D = (key, code) => ({ type: "down", key, code, real: true });
const U = (key, code) => ({ type: "up",   key, code, real: true });
const SD = (key, code) => ({ type: "down", key, code, real: false });
const SU = (key, code) => ({ type: "up",   key, code, real: false });
const OPT = (name, value) => ({ type: "option", name, value });
const WAIT = ms => ({ type: "wait", ms });

const CTRL = D ("Control", 17), CTRL_UP = U ("Control", 17), SHIFT = D ("Shift", 16), SHIFT_UP = U ("Shift", 16);
const tap = (key, code) => [ D (key, code), U (key, code) ];

const CASES = {
   timings: [ CTRL, ... tap ("=", 187), ... [ SD ("+", 171), SU ("+", 171) ], ... tap ("=", 187), CTRL_UP, ... tap ("=", 187),
      CTRL, SD ("+", 107), SU ("+", 107), CTRL_UP ],
   debugOff: [ CTRL, ... tap ("1", 49), ... tap ("5", 53), ... tap ("s", 83), CTRL_UP ],
   debugShading: [ OPT ("Debug", true), CTRL, ... tap ("1", 49), ... tap ("2", 50), ... tap ("3", 51), ... tap ("4", 52), ... tap ("5", 53),
      CTRL_UP, ... tap ("1", 49), ... tap ("3", 51) ],
   debugLive: [ OPT ("Debug", true), CTRL, ... tap ("s", 83), ... tap ("s", 83), CTRL_UP, ... tap ("s", 83) ],
   viewpoints: [ ... tap ("End", 35), ... tap ("Home", 36), ... tap ("PageDown", 34), ... tap ("PageDown", 34), ... tap ("PageDown", 34),
      ... tap ("PageUp", 33), ... tap ("PageUp", 33), ... tap ("PageUp", 33) ],
   modifierFlags: [ SHIFT, CTRL, D ("Alt", 18), SD ("Meta", 91), SD ("AltGraph", 225), U ("Alt", 18), SU ("Meta", 91), SU ("AltGraph", 225),
      SHIFT_UP, CTRL_UP ],
   shiftF8: [ ... tap ("F8", 119), SHIFT, ... tap ("F8", 119), SHIFT_UP, ... tap ("PageDown", 34), WAIT (4000), SHIFT, ... tap ("F8", 119), SHIFT_UP ],
};

const frames = page => page .evaluate (() => new Promise (r => requestAnimationFrame (() => requestAnimationFrame (() => requestAnimationFrame (() => r ())))));

const state = page => page .evaluate (async () =>
{
   const canvas  = document .querySelector ("x3d-canvas");
   const browser = canvas .browser;
   const bound   = [ "V1", "V2", "V3" ] .filter (name => browser .currentScene .getNamedNode (name) .isBound);
   const live    = browser .isLive ();

   let clipboard = null;

   try { clipboard = await navigator .clipboard .readText (); } catch (error) { clipboard = `error ${error .message}`; }

   return {
      timings: browser .getBrowserOption ("Timings"), shading: browser .getBrowserOption ("Shading"), debug: browser .getBrowserOption ("Debug"),
      live: typeof live === "boolean" ? live : live .getValue (), description: browser .description, bound: bound .join (","),
      keys: [ "controlKey", "shiftKey", "altKey", "commandKey", "altGrKey" ] .map (name => browser [`get${name [0] .toUpperCase () + name .slice (1)}`] ()),
      clipboard,
   };
});

async function run (page, name, actions)
{
   // A fresh world for each case; the options are the host page's defaults.
   await page .evaluate (async world =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;

      browser .setBrowserOption ("Debug", false);
      browser .setBrowserOption ("Timings", false);
      browser .setBrowserOption ("Shading", "GOURAUD");
      browser .beginUpdate ();
      browser .description = "";
      await browser .replaceWorld (await browser .createX3DFromString (world));
   }, WORLD);

   await page .locator ("x3d-canvas") .focus ();
   await page .evaluate (() => navigator .clipboard .writeText ("before"));
   await frames (page);
   await frames (page);

   const out = [ { action: "start", state: await state (page) } ];

   for (const action of actions)
   {
      if (action .type === "wait")
         await page .waitForTimeout (action .ms);
      else if (action .type === "option")
         await page .evaluate (a => document .querySelector ("x3d-canvas") .browser .setBrowserOption (a .name, a .value), action);
      else if (action .real)
         await (action .type === "down" ? page .keyboard .down (action .key) : page .keyboard .up (action .key));
      else
         await page .evaluate (a => document .querySelector ("x3d-canvas") .dispatchEvent (new KeyboardEvent (a .type === "down" ? "keydown" : "keyup",
            { key: a .key, keyCode: a .code, which: a .code, bubbles: true, cancelable: true })), action);

      await frames (page);

      // The clipboard write is asynchronous: wait for the description it sets.
      if (action .key === "F8" && action .type === "down")
         await frames (page);

      await page .waitForTimeout (action .key === "F8" ? 150 : 0);
      out .push ({ action: `${action .type} ${action .key ?? action .name ?? ""} ${action .code ?? action .value ?? action .ms ?? ""}`.replace (/ +/g, " ") .trim (), state: await state (page) });
   }

   return out;
}

async function once ()
{
   const browser = await chromium .launch ({
      executablePath: "/usr/bin/google-chrome",
      headless: true,
      args: [ "--headless=new", "--no-sandbox", "--use-gl=angle", "--use-angle=gl", "--enable-gpu", "--ignore-gpu-blocklist" ],
   });

   try
   {
      const context = await browser .newContext ({ viewport: { width: 320, height: 320 }, deviceScaleFactor: 1 });

      await context .grantPermissions ([ "clipboard-read", "clipboard-write" ], { origin: base });
      await context .route (url => !url .href .startsWith (base), route => route .abort ());

      const page = await context .newPage ();

      page .on ("console", m => { if (process .env .KEYS_DEBUG) console .log ("page:", m .text ()); });
      page .on ("pageerror", e => { if (process .env .KEYS_DEBUG) console .log ("pageerror:", e .message); });
      page .on ("requestfailed", r => { if (process .env .KEYS_DEBUG) console .log ("failed:", r .url ()); });
      await page .goto (`${base}/x_ite-pin/src/native-keys-01.html`);
      await page .waitForFunction (() => window .ready, null, { timeout: 30000 });
      await page .waitForFunction (() => document .querySelector ("x3d-canvas") ?.browser ?.currentScene, null, { timeout: 30000 });
      await frames (page);

      const values = { };

      for (const [ name, actions ] of Object .entries (CASES))
         values [name] = await run (page, name, actions);

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
await writeFile (join (OUT, "native-keys-01-baseline.json"), JSON .stringify ({ pinned: "X_ITE 16.5.0 34cdf9f97", runs: RUNS, cases: first }, null, 1) + "\n");
server .close ();
console .log (`${RUNS} runs, identical values, ${Object .keys (first) .length} cases`);
