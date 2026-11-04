/**
 * XR oracle (issue #83): the pinned X_ITE 16.5.0 draws a world through its own WebXR context, driven by a mock
 * session (oracle/xr-shot.html, served inside the pinned src/), and the canvas is saved as PNG. No headset, no XR runtime.
 *
 *   node oracle/x_ite-xr-shot.mjs <xr-script.json> <out-dir> [--runs 3] [--ms N]
 *
 * `--ms N` (default 0): X_ITE Multisampling N (page xr-shot.html?ms=N); 0 is antialiased=false.
 *
 * The XR script (oracle/synthetic-xr/NAME.json): the `world` (oracle/synthetic-xr/WORLD.x3dv), canvas `size`, `frames`, the viewer `pose` (matrix, inverse) and the
 * `views` (projectionMatrix, matrix, inverse, viewport [x, y, width, height]), all column-major as WebXR gives them.
 * Writes <out-dir>/<script name>.png and <script name>.json (renderer, the requests the page made of the mock,
 * hashes), only when all runs gave identical pixels. Headless only; non-local requests are aborted.
 */
import { createServer } from "node:http";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { basename, dirname, extname, join, normalize, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { deflateSync } from "node:zlib";
import { checkOutput } from "./check-output.mjs";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
const { chromium } = createRequire (join (ROOT, "../../../x_ite-tests/")) ("playwright");

// The page is served at a URL inside the pinned src/ (X_ITE resolves its assets/ relative to the page), but it is not
// a file of the pin: tools/verify-candidate.sh digests every file of oracle/x_ite-pin, so a page added there would
// change the pin. It lives next to this script.
const PAGE_URL  = join (ROOT, "x_ite-pin/src/xr-shot.html");
const PAGE_FILE = join (ROOT, "xr-shot.html");

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css",
   ".json": "application/json", ".png": "image/png", ".jpg": "image/jpeg", ".gif": "image/gif" };

const argv = process .argv .slice (2);
const opt  = (name, def) => { const i = argv .indexOf (name); return i >= 0 ? argv [i + 1] : def; };
const [ scriptArg, outArg ] = argv .filter ((a, i) => !a .startsWith ("--") && !argv [i - 1] ?.startsWith ("--"));
const RUNS = Number (opt ("--runs", "3"));
const MS   = Number (opt ("--ms", "0"));

if (!Number .isInteger (MS) || MS < 0 || MS > 8)
   throw new Error (`--ms must be an integer from 0 to 8, got ${opt ("--ms", "0")}`);

if (!scriptArg || !outArg)
   throw new Error ("usage: node oracle/x_ite-xr-shot.mjs <xr-script.json> <out-dir> [--runs 3] [--ms N (0 to 8)]");

const script = JSON .parse (await readFile (resolve (scriptArg), "utf8"));
const world  = join (dirname (resolve (scriptArg)), `${script .world ?? basename (scriptArg, ".json")}.x3dv`);
const name   = basename (scriptArg, ".json");
const outDir = resolve (outArg);

checkOutput (outDir);

if (!world .startsWith (ROOT))
   throw new Error ("the world must be inside oracle/ (the page's server root)");

const server = createServer (async (req, res) =>
{
   const path = normalize (join (ROOT, decodeURIComponent (new URL (req .url, "http://x") .pathname)));

   if (!path .startsWith (ROOT)) { res .writeHead (403); return res .end (); }

   try
   {
      const data = await readFile (path === PAGE_URL ? PAGE_FILE : path);

      res .writeHead (200, { "Content-Type": TYPES [extname (path) .toLowerCase ()] ?? "model/x3d+vrml", "Cache-Control": "no-store" });
      res .end (data);
   }
   catch { res .writeHead (404, { "Cache-Control": "no-store" }); res .end (); }
});

await new Promise (r => server .listen (0, "127.0.0.1", r));

const base    = `http://127.0.0.1:${server .address () .port}`;
const browser = await chromium .launch ({ executablePath: "/usr/bin/google-chrome", headless: true,
   args: [ "--headless=new", "--no-sandbox", "--use-gl=angle", "--use-angle=gl", "--enable-gpu", "--ignore-gpu-blocklist" ] });

const CRC = new Int32Array (256) .map ((_, n) => { let c = n; for (let k = 0; k < 8; ++ k) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c; });
const crc32 = buf => { let c = -1; for (const b of buf) c = CRC [(c ^ b) & 0xff] ^ (c >>> 8); return (c ^ -1) >>> 0; };

function chunk (type, data)
{
   const out = Buffer .alloc (12 + data .length);
   out .writeUInt32BE (data .length, 0);
   out .write (type, 4, "latin1");
   data .copy (out, 8);
   out .writeUInt32BE (crc32 (out .subarray (4, 8 + data .length)), 8 + data .length);
   return out;
}

function encodePNG (w, h, rgba)
{
   const raw = Buffer .alloc ((w * 4 + 1) * h);
   for (let y = 0; y < h; ++ y)
      rgba .copy (raw, y * (w * 4 + 1) + 1, y * w * 4, (y + 1) * w * 4);
   const ihdr = Buffer .alloc (13);
   ihdr .writeUInt32BE (w, 0); ihdr .writeUInt32BE (h, 4); ihdr [8] = 8; ihdr [9] = 6;
   return Buffer .concat ([ Buffer .from ([137, 80, 78, 71, 13, 10, 26, 10]), chunk ("IHDR", ihdr), chunk ("IDAT", deflateSync (raw, { level: 9 })), chunk ("IEND", Buffer .alloc (0)) ]);
}

async function once ()
{
   const [ W, H ] = script .size;
   const context  = await browser .newContext ({ viewport: { width: W, height: H }, deviceScaleFactor: 1 });

   await context .route (url => !url .href .startsWith (base), route => route .abort ());

   const page = await context .newPage ();

   try
   {
      await page .goto (`${base}/x_ite-pin/src/xr-shot.html${MS ? `?ms=${MS}` : ""}`);
      await page .waitForFunction (() => window .ready, null, { timeout: 30000 });
      await page .evaluate (([w, h]) => window .setSize (w, h), [ W, H ]);

      // The sample count that X_ITE applied (BrowserOptions.set_Multisampling__: antialiased="false" gives 0) must be the requested one.
      const applied = await page .evaluate (() => document .getElementById ("c") .browser .getRenderingProperty ("Multisampling"));

      if (applied !== MS)
         throw new Error (`RenderingProperties.Multisampling is ${applied}, not the requested ${MS}`);

      const info = await page .evaluate (() =>
      {
         const gl  = document .getElementById ("c") .browser .getContext ();
         const ext = gl .getExtension ("WEBGL_debug_renderer_info");
         return { renderer: ext ? gl .getParameter (ext .UNMASKED_RENDERER_WEBGL) : gl .getParameter (gl .RENDERER) };
      });

      const url = `${base}/${relative (ROOT, world) .split ("/") .map (encodeURIComponent) .join ("/")}`;
      const r   = await Promise .race ([
         page .evaluate (([u, s]) => window .xrShot (u, s), [ url, script ]),
         new Promise (r => setTimeout (() => r ({ ok: false, error: "TIMEOUT 45s" }), 45000)),
      ]);

      if (!r .ok)
         throw new Error (r .error);

      const bottomUp = Buffer .from (r .rgba, "base64");
      const rgba     = Buffer .alloc (bottomUp .length);
      const stride   = r .width * 4;

      for (let y = 0; y < r .height; ++ y)
         bottomUp .copy (rgba, y * stride, (r .height - 1 - y) * stride, (r .height - y) * stride);

      return { r, info, rgba, png: encodePNG (r .width, r .height, rgba) };
   }
   finally
   {
      await context .close ();
   }
}

const runs = [ ];

try
{
   for (let i = 0; i < RUNS; ++ i)
      runs .push (await once ());
}
finally
{
   await browser .close ();
   server .close ();
}

const hashes = runs .map (r => createHash ("sha256") .update (r .rgba) .digest ("hex"));

if (new Set (hashes) .size !== 1)
{
   console .error (`the ${RUNS} runs differ: ${hashes .join (" ")}; nothing written`);
   process .exit (1);
}

const [ first ] = runs;

if (!first .r .xrSession || first .r .numViews !== script .views .filter (v => v .viewport [2] > 0) .length)
{
   console .error (`the page did not run an XR session with ${script .views .length} views: ${JSON .stringify ({ ... first .r, rgba: undefined })}`);
   process .exit (1);
}

await mkdir (outDir, { recursive: true });
await writeFile (join (outDir, `${name}.png`), first .png);
await writeFile (join (outDir, `${name}.json`), JSON .stringify ({
   pinned: (await readFile (join (ROOT, "x_ite-pin/PINNED_COMMIT"), "utf8")) .trim (),
   world: relative (ROOT, world), script: relative (ROOT, resolve (scriptArg)), runs: RUNS, ms: MS,
   renderer: first .info .renderer, size: script .size,
   requests: first .r .requests, numViews: first .r .numViews, log: first .r .log,
   rgbaSha256: hashes [0], pngSha256: createHash ("sha256") .update (first .png) .digest ("hex"),
}, null, 1) + "\n");

console .log (`${name}: ${RUNS} identical runs, ${first .r .numViews} views, ${hashes [0] .slice (0, 16)}; renderer ${first .info .renderer}`);
