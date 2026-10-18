/**
 * REA batch 08 multiview oracle: several x3d-canvas on one page of the pinned X_ITE, one world each, one virtual clock.
 *
 *   node oracle/x_ite-multi-b08.mjs <out-dir> <msaa 0|4> <w> <h> <frames,comma> <world.x3dv>...
 *
 * One canvas per world, all on one page (a page with a single world is the "one each" run). Each frame k the clock
 * goes to t0 + k/60 and every browser traverses once (canvas 0 first). Writes <out-dir>/c<i>-f<k>.rgba (top row
 * first, RGBA8) and <out-dir>/log.json. Math.random is the constant 0.37, like the Rust test's seed lists, so
 * particle passes do not depend on the order the browsers draw in. Headless Chrome on the GPU. Pages, assets and
 * worlds come from a local server on oracle/; non-local requests are aborted. The page is oracle/multiview-b08/multi.html,
 * served as /x_ite-pin/src/multi-b08.html (the pin is not written).
 */
import { createServer } from "node:http";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { extname, join, normalize, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
const { chromium } = createRequire (join (ROOT, "../../../x_ite-tests/")) ("playwright");
const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json", ".png": "image/png" };

const [ outDir, msaa, W, H, framesArg, ... worlds ] = process .argv .slice (2);
const frames = framesArg .split (",") .map (Number);
const t0 = 946684800, dt = 1 / 60, warmup = 8;

const server = createServer (async (req, res) =>
{
   const pathname = new URL (req .url, "http://x") .pathname;
   const path = pathname === "/x_ite-pin/src/multi-b08.html" ? join (ROOT, "multiview-b08/multi.html") : normalize (join (ROOT, decodeURIComponent (pathname)));

   if (!path .startsWith (ROOT)) { res .writeHead (403); return res .end (); }

   try
   {
      const body = await readFile (path);
      res .writeHead (200, { "Content-Type": TYPES [extname (path) .toLowerCase ()] ?? "model/vrml", "Cache-Control": "no-store" });
      res .end (body);
   }
   catch { res .writeHead (404, { "Cache-Control": "no-store" }); res .end (); }
});

await new Promise (r => server .listen (0, "127.0.0.1", r));

const base    = `http://127.0.0.1:${server .address () .port}`;
const browser = await chromium .launch ({ executablePath: "/usr/bin/google-chrome", headless: true,
   args: [ "--headless=new", "--no-sandbox", "--use-gl=angle", "--use-angle=gl", "--enable-gpu", "--ignore-gpu-blocklist" ] });
const result  = { ok: false };

try
{
   const context = await browser .newContext ({ viewport: { width: (+W + 8) * worlds .length, height: +H }, deviceScaleFactor: 1 });
   await context .route (url => !url .href .startsWith (base), route => route .abort ());
   const page = await context .newPage ();

   await page .goto (`${base}/x_ite-pin/src/multi-b08.html?n=${worlds .length}&w=${W}&h=${H}&ms=${msaa}&rand=0.37`);
   await page .waitForFunction (() => window .ready, null, { timeout: 30000 });
   result .info = await page .evaluate (() => window .info ());

   for (const [ i, w ] of worlds .entries ())
   {
      const url = `${base}/${relative (ROOT, resolve (w)) .split ("/") .map (encodeURIComponent) .join ("/")}`;
      const r = await page .evaluate (([ i, u, t, d, wu ]) => window .load (i, u, t, d, wu), [ i, url, t0, dt, warmup ]);
      if (!r .ok) { result .log = r .log; throw new Error (r .error); }
   }

   result .warmup = await page .evaluate (n => window .start (n), warmup);
   await mkdir (outDir, { recursive: true });

   for (let k = 1; k <= Math .max (... frames); ++ k)
   {
      await page .evaluate (ms => window .setDate (ms), (t0 + k * dt) * 1000);
      await page .evaluate (([ t, d ]) => window .step (t, d), [ t0 + k * dt, dt ]);

      if (frames .includes (k))
         for (let i = 0; i < worlds .length; ++ i)
         {
            const r = await page .evaluate (i => window .pixels (i), i);
            const bottomUp = Buffer .from (r .rgba, "base64");
            const rgba = Buffer .alloc (bottomUp .length), stride = r .width * 4;
            for (let y = 0; y < r .height; ++ y) bottomUp .copy (rgba, y * stride, (r .height - 1 - y) * stride, (r .height - y) * stride);
            await writeFile (join (outDir, `c${i}-f${k}.rgba`), rgba);
         }
   }

   result .log = (await page .evaluate (() => window .getLog ())) .map (l => l .replaceAll (base, "http://oracle"));
   result .ok = true;
   await context .close ();
}
catch (e) { result .error = String (e ?.message ?? e) .split ("\n") [0]; }
finally { await browser .close (); server .close (); }

await mkdir (outDir, { recursive: true });
await writeFile (join (outDir, "log.json"), JSON .stringify (result, null, 2));
console .log (JSON .stringify (result));
process .exit (result .ok ? 0 : 1);
