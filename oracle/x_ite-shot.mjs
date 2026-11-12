/**
 * X7 oracle: render each VRML file with the pinned X_ITE on the real GPU (headless system
 * Chrome) and save the WebGL drawing buffer as PNG. The Rust renderer must draw the same picture.
 *
 *   node oracle/x_ite-shot.mjs <corpus_dir> <out_dir> [--size 512x512] [--jobs 6]
 *                              [--frames 10] [--settle 400] [--only <substring>] [--oit] [--canvas]
 *
 * Writes <out_dir>/<file>.png (same relative path, ".png" appended) and <out_dir>/index.json
 * (renderer string, size, sha256 of the raw RGBA and of the PNG, console log, errors).
 *
 * --canvas also writes <out_dir>/<file>.canvas.json: every 2D canvas the page filled and read back (ScreenText draws its glyphs
 * with `fill ()` and takes the pixels with `getImageData`), as { width, height, calls, alpha }: the path and transform calls since the
 * canvas was cleared, and the alpha of every pixel in the canvas' row order. The page's own canvas methods run unchanged; they are
 * only wrapped to keep their arguments. Without the option nothing is wrapped. A world fails when the page uses another call
 * that changes the canvas (clearRect, a transform other than translate and scale, rect, arc, stroke, text, drawImage, ...) or
 * a `fill` with arguments between the clear and the read: the record would not hold what made the pixels.
 *
 * Headless only: --headless=new, never a window. All non-local requests are aborted.
 * Never touches dump.html / x_ite-dump.mjs; the page is x_ite-pin/src/shot.html.
 */
import { createServer } from "node:http";
import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { dirname, extname, join, normalize, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { checkOutput } from "./check-output.mjs";
import { deflateSync } from "node:zlib";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
const { chromium } = createRequire (join (process .env .XITE_TESTS ?? join (ROOT, "../../../x_ite-tests"), "/")) ("playwright");

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css",
   ".json": "application/json", ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".gif": "image/gif" };

const argv = process .argv .slice (2);
const opt  = (name, def) => { const i = argv .indexOf (name); return i >= 0 ? argv [i + 1] : def; };
const [ corpusArg, outArg ] = argv .filter ((a, i) => !a .startsWith ("--") && !argv [i - 1] ?.startsWith ("--"));
const [ W, H ] = opt ("--size", "512x512") .split ("x") .map (Number);
const JOBS     = Number (opt ("--jobs", "6"));
const FRAMES   = Number (opt ("--frames", "10"));
const SETTLE   = Number (opt ("--settle", "400"));
const ONLY     = opt ("--only", "");
const OIT      = argv .includes ("--oit");   // OrderIndependentTransparency browser option on (sub:render.transparency-sorting)
const CANVAS   = argv .includes ("--canvas"); // record the 2D canvas calls and pixels (ScreenText raster)

// --canvas: the calls a record holds (fillRect starts a record), and the calls that change a canvas' pixels, path or transform
// in a way the record cannot hold. A rejected call between fillRect and getImageData fails the world.
const CANVAS_CALLS    = [ "save", "restore", "translate", "scale", "beginPath", "moveTo", "lineTo", "quadraticCurveTo", "bezierCurveTo", "closePath", "fill", "fillRect" ];
const CANVAS_REJECTED = [ "clearRect", "reset", "setTransform", "resetTransform", "transform", "rotate", "rect", "roundRect", "arc", "arcTo", "ellipse", "clip", "stroke", "strokeRect", "fillText", "strokeText", "drawImage", "putImageData" ];

const corpus = resolve (corpusArg);
const outDir = resolve (outArg);

checkOutput (outDir);
const files  = (await readdir (corpus, { recursive: true, withFileTypes: true }))
   .filter (d => d .isFile ())
   .map (d => relative (corpus, join (d .parentPath, d .name)))
   .filter (f => /\.(wrl|wrz)(\.gz)?$/i .test (f))
   .filter (f => !ONLY || ONLY .split (",") .some (s => f .includes (s)))
   .sort ();

// node:http is event-driven: many parallel ESM connections are served concurrently.
const server = createServer (async (req, res) =>
{
   const path = normalize (join (ROOT, decodeURIComponent (new URL (req .url, "http://x") .pathname)));

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
// XITE_CHROME and XITE_ANGLE_BACKEND let another machine record (as in x_ite-anim.mjs): the physical M1 Mac uses
// "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" and "metal" (issue #461). Unset, the recording is the one of the
// NVIDIA host (system Chrome, ANGLE on OpenGL).
const browser = await chromium .launch ({
   executablePath: process .env .XITE_CHROME ?? "/usr/bin/google-chrome",
   headless: true,
   args: [ "--headless=new", "--no-sandbox",
      ...(process .env .XITE_ANGLE_BACKEND ? [ `--use-angle=${process .env .XITE_ANGLE_BACKEND}` ] : [ "--use-gl=angle", "--use-angle=gl" ]),
      "--enable-gpu", "--ignore-gpu-blocklist" ],
});

// --- PNG encoder (RGBA8, no filter), so the oracle has no extra dependencies.

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

// --- one page per worker, a fresh context per file.

let info = null;

async function renderOne (file)
{
   const context = await browser .newContext ({ viewport: { width: W, height: H }, deviceScaleFactor: 1 });

   await context .route (url => !url .href .startsWith (base), route => route .abort ());

   const page = await context .newPage ();

   if (CANVAS)
   {
      await page .addInitScript (names =>
      {
         window .canvasLog = [ ];

         const prototype = CanvasRenderingContext2D .prototype;

         for (const name of names)
         {
            const method = prototype [name];

            if (!method)
               continue;

            prototype [name] = function (... args) { window .canvasLog .push ([ name, ... args ]); return method .apply (this, args); };
         }

         const getImageData = prototype .getImageData;

         prototype .getImageData = function (... args)
         {
            const data  = getImageData .apply (this, args);
            const alpha = [ ];

            for (let i = 3; i < data .data .length; i += 4)
               alpha .push (data .data [i]);

            window .canvasLog .push ([ "getImageData", data .width, data .height, alpha ]);
            return data;
         };
      }, [ ... CANVAS_CALLS, ... CANVAS_REJECTED ]);
   }

   try
   {
      await page .goto (`${base}/x_ite-pin/src/shot.html`);
      await page .waitForFunction (() => window .ready, null, { timeout: 30000 });
      await page .evaluate (([w, h]) => window .setSize (w, h), [ W, H ]);

      // The option reshapes the framebuffers (MultiSampleFramebuffer with the OIT accumulation buffers) before the world loads.
      if (OIT)
         await page .evaluate (() => document .getElementById ("c") .browser .setBrowserOption ("OrderIndependentTransparency", true));

      if (!info)
         info = await page .evaluate (() => window .info ());

      const url = `${base}/${relative (ROOT, join (corpus, file)) .split ("/") .map (encodeURIComponent) .join ("/")}`;
      const r   = await Promise .race ([
         page .evaluate (([u, f, s]) => window .shot (u, f, s), [ url, FRAMES, SETTLE ]),
         new Promise (r => setTimeout (() => r ({ ok: false, error: "TIMEOUT 45s", log: [ ] }), 45000)),
      ]);

      const entry = { file, ok: r .ok, error: r .error, log: JSON .parse (JSON .stringify (r .log ?? [ ]) .replaceAll (base, "http://oracle")) };

      if (CANVAS)
      {
         // One record for each canvas that was cleared (fillRect), drawn and read back; equal records are kept once, in sorted order.
         const canvases = [ ];
         let calls = [ ], open = false;

         // ScreenText sets the size of its canvas (which resets the state), clears it (fillRect), draws and reads it back. A
         // record is the calls from the clear to the read; a path or transform call outside a record would be lost, so it
         // stops the recording. A read or a rejected call outside a record belongs to another use of a 2D canvas.
         for (const [ name, ... args ] of await page .evaluate (() => window .canvasLog))
         {
            if (name === "fillRect")
            {
               if (open)
                  throw new Error (`--canvas: a fillRect after ${calls .length} canvas calls that were not read back`);

               calls = [ ];
               open  = true;
            }
            else if (name === "getImageData")
            {
               if (!open)
                  continue;

               open = false;

               // The record must hold everything that made the pixels: a call that the replay does not know stops the recording.
               const unknown = calls .find (([ name, ... args ]) => CANVAS_REJECTED .includes (name) || (name === "fill" && args .length));

               if (unknown)
                  throw new Error (`--canvas: the page called ${unknown [0]} (${unknown .slice (1) .length} arguments) on a 2D canvas before getImageData; the record cannot hold it`);

               const record = JSON .stringify ({ width: args [0], height: args [1], calls, alpha: args [2] });

               if (!canvases .includes (record))
                  canvases .push (record);
            }
            else if (open)
               calls .push ([ name, ... args ]);
            else if (CANVAS_CALLS .includes (name))
               throw new Error (`--canvas: the page called ${name} on a 2D canvas outside a record (no fillRect before it)`);
         }

         // Fonts arrive in any order, so the order in which the texts are drawn is not part of the record.
         canvases .sort ();

         const text = `{"canvases":[\n${canvases .join (",\n")}\n]}\n`;
         const out  = join (outDir, file + ".canvas.json");

         await mkdir (dirname (out), { recursive: true });
         await writeFile (out, text);

         entry .canvases     = canvases .length;
         entry .canvasSha256 = createHash ("sha256") .update (text) .digest ("hex");
      }

      if (r .ok)
      {
         // readPixels rows are bottom-up; PNG rows are top-down.
         const bottomUp = Buffer .from (r .rgba, "base64");
         const rgba     = Buffer .alloc (bottomUp .length);
         const stride   = r .width * 4;

         for (let y = 0; y < r .height; ++ y)
            bottomUp .copy (rgba, y * stride, (r .height - 1 - y) * stride, (r .height - y) * stride);

         const png = encodePNG (r .width, r .height, rgba);
         const out = join (outDir, file + ".png");

         await mkdir (dirname (out), { recursive: true });
         await writeFile (out, png);

         Object .assign (entry, { width: r .width, height: r .height, rgbaSha256: createHash ("sha256") .update (rgba) .digest ("hex"),
            pngSha256: createHash ("sha256") .update (png) .digest ("hex"), splashVisible: r .splashVisible, canvasDisplay: r .canvasDisplay });
      }

      return entry;
   }
   catch (e)
   {
      return { file, ok: false, error: String (e ?.message ?? e) .split ("\n") [0], log: [ ] };
   }
   finally
   {
      await context .close ();
   }
}

const results = [ ];
let next = 0;

await Promise .all (Array .from ({ length: JOBS }, async () =>
{
   while (next < files .length)
   {
      const file  = files [next ++];
      const entry = await renderOne (file);

      results .push (entry);
      console .log (`${entry .ok ? "ok  " : "FAIL"} ${file}${entry .ok ? "" : "  " + entry .error}`);
   }
}));

const chromeVersion = browser .version ();

await browser .close ();
server .close ();

results .sort ((a, b) => a .file < b .file ? -1 : a .file > b .file);

await mkdir (outDir, { recursive: true });
await writeFile (join (outDir, "index.json"), JSON .stringify ({
   pinned: (await readFile (join (ROOT, "x_ite-pin/PINNED_COMMIT"), "utf8")) .trim (),
   chrome: chromeVersion,
   oit: OIT,
   size: [ W, H ], frames: FRAMES, settleMs: SETTLE,
   gl: info,
   results,
}, null, 1));

console .log (`\n${results .filter (r => r .ok) .length} of ${results .length} rendered; renderer: ${info ?.renderer}`);
