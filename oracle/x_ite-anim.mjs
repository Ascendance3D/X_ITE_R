/**
 * X10 oracle: drive the pinned X_ITE frame by frame with scripted input and record what it does.
 *
 *   node oracle/x_ite-anim.mjs <world.wrl inside oracle/> <script.json> [--out result.json]
 *                              [--png-dir dir]
 *
 * The script format is the one crates/x3d-viewer's `x3d-drive` reads (see its header): load the
 * world, then for frame k = 1..frames apply the input events of frame k and step the browser
 * clock to t0 + k * dt. Output has the same shape as x3d-drive's: poses (user position,
 * orientation, centerOfRotation, bound viewpoint) and scene text at the captured frames,
 * console log, and PNGs of the WebGL drawing buffer.
 *
 * Headless only (--headless=new, real GPU through ANGLE); non-local requests are aborted.
 * Does not touch dump.html, shot.html, x_ite-dump.mjs or x_ite-shot.mjs; the page is
 * x_ite-pin/src/anim.html. The browser's own frame loop is neutralized (see anim.html), time is
 * virtual: getCurrentTime, Date.now and setTimeout follow the script's clock.
 */
import { createServer } from "node:http";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { extname, join, normalize, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { deflateSync } from "node:zlib";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
const { chromium } = createRequire (join (ROOT, "../../../x_ite-tests/")) ("playwright");

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css",
   ".json": "application/json", ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".gif": "image/gif" };

const argv = process .argv .slice (2);
const opt  = (name, def) => { const i = argv .indexOf (name); return i >= 0 ? argv [i + 1] : def; };
const [ fileArg, scriptArg ] = argv .filter ((a, i) => !a .startsWith ("--") && !argv [i - 1] ?.startsWith ("--"));
const script = JSON .parse (await readFile (scriptArg, "utf8"));

const [ W, H ] = script .size ?? [ 512, 512 ];
const t0       = script .t0 ?? 946684800;
const dt       = script .dt ?? 1 / 60;
const frames   = script .frames ?? 1;
const warmup   = script .warmup ?? 8;
const capture  = new Set (script .capture ?? [ ]);
const dump     = new Set (script .dump ?? [ ]);
const events   = script .events ?? [ ];
const pngDir   = opt ("--png-dir", "");

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
const browser = await chromium .launch ({
   executablePath: "/usr/bin/google-chrome",
   headless: true,
   args: [ "--headless=new", "--no-sandbox", "--use-gl=angle", "--use-angle=gl", "--enable-gpu", "--ignore-gpu-blocklist" ],
});

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

const result = { ok: false, poses: { }, vrml: { }, prints: [ ], log: [ ], anchors: [ ] };

// Script key "console": true records every console.log / warn / error message in the order it was
// logged ("log: text", "warn: first line", "error: first line"), over the whole run. Playwright's
// console listener sees them in order, so no page code is needed (anim.html only keeps the sorted
// warn / error lines in `log`). Script key "dumpInlines": true adds, to every dumped frame, the
// scene text of each DEF-named Inline scene: result .inlines [frame] [path] = scene .toVRMLString ().
const consoleLines = [ ];

// Script key "settle": true makes loads deterministic: after the load and after every event and frame
// step the harness waits until no request is in flight and the page's continuations (parse, setup,
// Script initialize, nested loads) have run, so a fetch started in frame k always completes between
// frame k and frame k + 1. x3d-drive models the same rule (Runtime::settle). Without it the frame in
// which a file fetched at run time arrives depends on the wall clock.
let inFlight = 0;

const context = await browser .newContext ({ viewport: { width: W, height: H }, deviceScaleFactor: 1 });
await context .route (url => !url .href .startsWith (base), route => route .abort ());

const page = await context .newPage ();

if (script .settle)
{
   page .on ("request", () => ++ inFlight);
   page .on ("requestfinished", () => -- inFlight);
   page .on ("requestfailed", () => -- inFlight);
}

if (script .console)
{
   page .on ("console", m =>
   {
      const type = m .type ();

      if (type !== "log" && type !== "warning" && type !== "error")
         return;

      const text = m .text () .replaceAll (base, "http://oracle");

      consoleLines .push (type === "log" ? `log: ${text}` : `${type === "warning" ? "warn" : "error"}: ${text .split ("\n") [0]}`);
   });
}

try
{
   await page .goto (`${base}/x_ite-pin/src/anim.html`);
   await page .waitForFunction (() => window .ready, null, { timeout: 30000 });

   // Script key "pinFrameRate": true pins the SAI property `browser .currentFrameRate` (what Followers'
   // X3DDamperNode reads) to 1 / dt before the world loads. The page stubs advanceTime and
   // getCurrentFrameRate, but the property is bound to the prototype method and would report the
   // wall-clock rate of the page's early frames (non-deterministic). Off by default.
   if (script .pinFrameRate)
      await page .evaluate (rate => Object .defineProperty (document .getElementById ("c") .browser, "currentFrameRate", { get: () => rate, configurable: true }), 1 / Math .max (dt, 1e-9));

   const abs = resolve (fileArg);
   const url = `${base}/${relative (ROOT, abs) .split ("/") .map (encodeURIComponent) .join ("/")}`;
   const loaded = await page .evaluate (([u, t, d, w]) => window .load (u, t, d, w), [ url, t0, dt, warmup ]);

   result .loadFrames = loaded .loadFrames;
   result .warmupFrames = loaded .frames;

   if (!loaded .ok)
      throw new Error (loaded .error);

   await page .evaluate (d => window .setDt (d), dt);
   result .geometry = await page .evaluate (() => window .geometry ());
   await page .evaluate (() => window .focusCanvas ());

   const yieldTask = () => page .evaluate (() => new Promise (resolve =>
   {
      const channel = new MessageChannel ();
      channel .port1 .onmessage = () => resolve ();
      channel .port2 .postMessage (0);
   }));

   const settle = async () =>
   {
      if (!script .settle)
         return;

      for (let quiet = 0; quiet < 3;)
      {
         while (inFlight > 0)
            await new Promise (r => setTimeout (r, 2));

         await yieldTask ();
         await yieldTask ();

         quiet = inFlight > 0 ? 0 : quiet + 1;
      }
   };

   await settle ();

   const mouseButton = b => ({ 0: "left", 1: "middle", 2: "right" }) [b] ?? "left";

   for (let k = 1; k <= frames; ++ k)
   {
      const prev = t0 + (k - 1) * dt;

      for (const e of events .filter (e => e .frame === k))
      {
         await page .evaluate (ms => window .setDate (ms), (prev + (e .t ?? 0)) * 1000);

         switch (e .type)
         {
            case "mousedown": await page .mouse .move (e .x, H - e .y); await page .mouse .down ({ button: mouseButton (e .button) }); break;
            case "mouseup":   await page .mouse .move (e .x, H - e .y); await page .mouse .up ({ button: mouseButton (e .button) }); break;
            case "mousemove": await page .mouse .move (e .x, H - e .y); break;
            case "wheel":     await page .mouse .move (e .x, H - e .y); await page .mouse .wheel (0, e .deltaY); break;
            case "dblclick":  await page .mouse .dblclick (e .x, H - e .y); break; // move, down/up (clickCount 1), down/up (clickCount 2): the browser fires "dblclick"
            case "viewer":    await page .evaluate (n => window .setViewer (n), e .name); break;
            case "key":       await page .keyboard .press (e .key); break;
            case "mods":
            {
               for (const [ prop, key ] of [ [ "shift", "Shift" ], [ "ctrl", "Control" ], [ "alt", "Alt" ] ])
                  await (e [prop] ? page .keyboard .down (key) : page .keyboard .up (key));
               break;
            }
            default: break;
         }
      }

      await settle ();
      await page .evaluate (ms => window .setDate (ms), (t0 + k * dt) * 1000);
      await page .evaluate (t => window .step (t), t0 + k * dt);
      await settle ();

      if (capture .has (k) || dump .has (k) || k === frames)
         result .poses [k] = await page .evaluate (() =>
         {
            // fieldOfViewScale / fieldOfViewOffset (PLANE zoom, OrthoViewpoint) are read from the page's browser object;
            // anim.html (inside the read-only pin) is untouched.
            const vp = document .getElementById ("c") .browser .getActiveViewpoint ();

            return Object .assign (window .pose (),
            {
               fieldOfViewScale: vp ._fieldOfViewScale .getValue (),
               fieldOfViewOffset: vp ._fieldOfViewOffset ? Array .from (vp ._fieldOfViewOffset .getValue ()) : undefined,
            });
         });

      if (dump .has (k) || k === frames)
         result .vrml [k] = await page .evaluate (() => window .vrml ());

      if (script .dumpInlines && (dump .has (k) || k === frames))
      {
         result .inlines ??= { };
         result .inlines [k] = await page .evaluate (() =>
         {
            const
               browser = document .getElementById ("c") .browser,
               out     = { };

            const walk = (scene, prefix) =>
            {
               for (const node of scene .namedNodes)
               {
                  const n = node .getValue ();

                  if (n ?.getTypeName () !== "Inline")
                     continue;

                  const inner = n .getInternalScene ();

                  if (inner === browser .getDefaultScene ())
                  {
                     out [prefix + n .getName ()] = null;
                     continue;
                  }

                  out [prefix + n .getName ()] = inner .toVRMLString ();
                  walk (inner, prefix + n .getName () + "/");
               }
            };

            walk (browser .currentScene, "");
            return out;
         });
      }

      if (capture .has (k) && pngDir)
      {
         const r = await page .evaluate (() => window .pixels ());
         const bottomUp = Buffer .from (r .rgba, "base64");
         const rgba     = Buffer .alloc (bottomUp .length);
         const stride   = r .width * 4;

         for (let y = 0; y < r .height; ++ y)
            bottomUp .copy (rgba, y * stride, (r .height - 1 - y) * stride, (r .height - y) * stride);

         await mkdir (pngDir, { recursive: true });
         await writeFile (join (pngDir, `${k}.png`), encodePNG (r .width, r .height, rgba));
      }
   }

   result .log = (await page .evaluate (() => window .getLog ())) .map (l => l .replaceAll (base, "http://oracle"));

   if (script .console)
      result .console = consoleLines .slice ();
   result .ok  = true;
}
catch (e)
{
   result .error = String (e ?.message ?? e) .split ("\n") [0];
}
finally
{
   await context .close ();
   await browser .close ();
   server .close ();
}

const text = JSON .stringify (result, null, 2);

if (opt ("--out", ""))
   await writeFile (opt ("--out"), text);
else
   console .log (text);

process .exit (result .ok ? 0 : 1);
