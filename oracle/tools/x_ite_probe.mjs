/**
 * REA batch 03 investigation probe: load a world in the pinned X_ITE (oracle/x_ite-pin/src/anim.html,
 * headless Chrome, virtual time like x_ite-anim.mjs), step it, and print the value of a page
 * expression after chosen frames. The expression sees `browser` (the X3DBrowser of the canvas).
 *
 *   node oracle/tools/x_ite_probe.mjs <world inside oracle/> <frames,comma,separated> '<expression>' [dt]
 *
 * Read-only: nothing is written. Used to read internal state (for example the pick sensor order)
 * that the scene text does not show.
 */
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { extname, join, normalize, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = normalize (join (fileURLToPath (import.meta.url), "../.."));
const { chromium } = createRequire (join (ROOT, "../../../x_ite-tests/")) ("playwright");
const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".json": "application/json", ".png": "image/png", ".jpg": "image/jpeg" };
const [ fileArg, framesArg, expr, dtArg = String (1 / 30) ] = process .argv .slice (2);
const frames = framesArg .split (",") .map (Number);
const dt = Number (dtArg), t0 = 946684800;

const server = createServer (async (req, res) =>
{
   const path = normalize (join (ROOT, decodeURIComponent (new URL (req .url, "http://x") .pathname)));
   if (!path .startsWith (ROOT)) { res .writeHead (403); return res .end (); }
   try { const body = await readFile (path); res .writeHead (200, { "Content-Type": TYPES [extname (path) .toLowerCase ()] ?? "model/vrml", "Cache-Control": "no-store" }); res .end (body); }
   catch { res .writeHead (404); res .end (); }
});

await new Promise (r => server .listen (0, "127.0.0.1", r));
const base = `http://127.0.0.1:${server .address () .port}`;
const browser = await chromium .launch ({ executablePath: "/usr/bin/google-chrome", headless: true, args: [ "--headless=new", "--no-sandbox", "--use-gl=angle", "--use-angle=gl", "--enable-gpu", "--ignore-gpu-blocklist" ] });
const context = await browser .newContext ({ viewport: { width: 512, height: 512 }, deviceScaleFactor: 1 });
await context .route (url => !url .href .startsWith (base), route => route .abort ());
const page = await context .newPage ();

try
{
   await page .goto (`${base}/x_ite-pin/src/anim.html`);
   await page .waitForFunction (() => window .ready, null, { timeout: 30000 });
   const url = `${base}/${relative (ROOT, resolve (fileArg)) .split ("/") .map (encodeURIComponent) .join ("/")}`;
   const loaded = await page .evaluate (([u, t, d, w]) => window .load (u, t, d, w), [ url, t0, dt, 8 ]);
   if (!loaded .ok) throw new Error (loaded .error);
   await page .evaluate (d => window .setDt (d), dt);

   for (let k = 0; k <= Math .max (... frames); ++ k)
   {
      if (k > 0)
      {
         await page .evaluate (ms => window .setDate (ms), (t0 + k * dt) * 1000);
         await page .evaluate (t => window .step (t), t0 + k * dt);
      }

      if (frames .includes (k))
         console .log (k, JSON .stringify (await page .evaluate (e => { const browser = document .getElementById ("c") .browser; return eval (e); }, expr)));
   }
}
finally
{
   await browser .close ();
   server .close ();
}
