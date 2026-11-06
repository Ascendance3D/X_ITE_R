/**
 * Timing of the GaussianSplats sort worker of the pinned X_ITE 16.5.0 (lane gs/worker-timing).
 *
 *   node oracle/x_ite-splat-sort-timing.mjs <out.json> <count> [<count> ...]
 *        [--runs 5] [--chrome PATH] [--angle gl|swiftshader|metal|default] [--playwright DIR]
 *        [--order-out order.json]
 *
 * Measures two things separately.
 *
 * Timing: for each count, the same synthetic world (a fixed pseudo-random cloud, the generator of
 * x_ite-splat-sort-trace.mjs), the same camera path (three steps: a half turn about Y, a quarter turn about X, a
 * small turn about Y), the same 512 x 512 size. A step starts when the page sets the Transform rotation. The page
 * records in ms (performance.now, one clock) the time of the step, the time of the "sort" request that X_ITE posts to
 * its worker, the time the page receives "sorted", every instanced draw of the splat quads (time, running draw number
 * and the checksum of the index buffer the draw reads) and every requestAnimationFrame callback. Frames and ms are
 * kept separate: the frames drawn with the old order are the draws that read an index buffer other than the one the
 * answer carries.
 *
 * Order: the checksum (FNV-1a 32 bit) of every sorted index array and the model-view matrix the request carried. They
 * do not depend on timing; --order-out writes them (checked to be identical in every run) for the Rust comparison.
 *
 * Headless only; no network. The page is oracle/x_ite-pin/src/shot.html. The tool reports a request time as seen on
 * the page main thread: the "recv" time includes any delay of that thread.
 */
import { writeFile } from "node:fs/promises";
import { checkOutput } from "./check-output.mjs";
import { resolve } from "node:path";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { join, normalize, extname } from "node:path";
import { fileURLToPath } from "node:url";
import os from "node:os";

const argv = process .argv .slice (2);
const opt = (name, dflt) => argv .includes (name) ? argv [argv .indexOf (name) + 1] : dflt;
const OPTS = new Set (["--runs", "--chrome", "--angle", "--playwright", "--order-out"]);
const RUNS = Number (opt ("--runs", 5));
const CHROME = opt ("--chrome", "/usr/bin/google-chrome");
const ANGLE = opt ("--angle", "gl");
const ORDER_OUT = opt ("--order-out", null);
const [ outArg, ... counts ] = argv .filter ((a, i) => !OPTS .has (a) && !OPTS .has (argv [i - 1]));

if (!outArg || !counts .length)
   throw new Error ("usage: node oracle/x_ite-splat-sort-timing.mjs <out.json> <count> [<count> ...] [--runs 5] [--chrome PATH] [--angle gl|swiftshader|metal|default] [--playwright DIR] [--order-out order.json]");

checkOutput (resolve (outArg));
if (ORDER_OUT) checkOutput (resolve (ORDER_OUT));

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
const PW_DIR = opt ("--playwright", process .env .XITE_TESTS ?? join (ROOT, "../../../x_ite-tests"));
const req = createRequire (join (PW_DIR, "/"));
const { chromium } = (() => { try { return req ("playwright"); } catch { return req ("playwright-core"); } })();
const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json", ".png": "image/png", ".wasm": "application/wasm" };
const server = createServer (async (req, res) => { const p = normalize (join (ROOT, decodeURIComponent (new URL (req .url, "http://x") .pathname)));
  try { const b = await readFile (p); res .writeHead (200, { "Content-Type": TYPES [extname (p)] ?? "text/plain", "Cache-Control": "no-store" }); res .end (b); } catch { res .writeHead (404); res .end (); } });
await new Promise (r => server .listen (0, "127.0.0.1", r));
const base = `http://127.0.0.1:${server .address () .port}`;

const ANGLE_ARGS = {
   gl: ["--use-gl=angle", "--use-angle=gl", "--enable-gpu", "--ignore-gpu-blocklist"],
   swiftshader: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"],
   metal: ["--use-gl=angle", "--use-angle=metal", "--enable-gpu", "--ignore-gpu-blocklist"],
   default: [],
};
const browser = await chromium .launch ({ executablePath: CHROME, headless: true, args: ["--headless=new", "--no-sandbox", ... ANGLE_ARGS [ANGLE]] });

function world (n)
{
   let s = 12345, r = () => (s = (s * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff;
   const pos = [], ori = [], sc = [], op = [], sh = [];
   for (let i = 0; i < n; ++i) { pos .push (`${((r () - .5) * 3) .toFixed (3)} ${((r () - .5) * 3) .toFixed (3)} ${((r () - .5) * 3) .toFixed (3)}`); ori .push ("0 0 0 1"); const q = (0.05 + r () * 0.15) .toFixed (3); sc .push (`${q} ${q} ${q}`); op .push ((0.5 + r () * 0.5) .toFixed (2)); sh .push (`${((r () - .5) * 3) .toFixed (2)} ${((r () - .5) * 3) .toFixed (2)} ${((r () - .5) * 3) .toFixed (2)}`); }
   return `#X3D V4.1 utf8\nPROFILE Interchange\nCOMPONENT GaussianSplats:1\nViewpoint { position 0 0 6 }\nBackground { skyColor 0.2 0.2 0.28 }\nDEF T Transform { children GaussianSplats { positions [ ${pos .join (", ")} ] orientations [ ${ori .join (", ")} ] scales [ ${sc .join (", ")} ] opacities [ ${op .join (", ")} ] sphericalHarmonicsDegree0Coef0 [ ${sh .join (", ")} ] } }\n`;
}

// The page code. Returns { env, steps: [...] }.
async function pageRun (text)
{
   const browser = document .getElementById ("c") .browser;
   const X3D = window [Symbol .for ("X_ITE.X3D")];
   const gl0 = browser .getContext ();
   const dbg = gl0 .getExtension ("WEBGL_debug_renderer_info");
   const env = { renderer: dbg ? gl0 .getParameter (dbg .UNMASKED_RENDERER_WEBGL) : gl0 .getParameter (gl0 .RENDERER), vendor: dbg ? gl0 .getParameter (dbg .UNMASKED_VENDOR_WEBGL) : gl0 .getParameter (gl0 .VENDOR), userAgent: navigator .userAgent };

   const ev = [];             // all events, one clock
   let draws = 0, raf = 0, cur = { id: -1 };
   const bufs = [];           // index buffers seen: { id, arr, t }
   const now = () => performance .now ();

   const Base = window .Worker;
   window .Worker = class extends Base
   {
      constructor (... a)
      {
         super (... a);
         this .addEventListener ("message", e =>
         {
            const t = now ();
            if (e .data .type === "sorted") ev .push ({ k: "recv", t, draws, raf, bufId: bufs .length });
            else ev .push ({ k: "recv-" + e .data .type, t, draws, raf });
         });
         const post = this .postMessage .bind (this);
         this .postMessage = (m, ... rest) =>
         {
            const t = now ();
            if (m .type === "sort") ev .push ({ k: "post", t, draws, raf, mv: Array .from (m .viewMatrix) });
            else ev .push ({ k: "post-" + m .type, t, draws, raf });
            return post (m, ... rest);
         };
      }
   };

   const bd = WebGL2RenderingContext .prototype .bufferData;
   WebGL2RenderingContext .prototype .bufferData = function (target, data, ... r)
   {
      if (target === this .ARRAY_BUFFER && data instanceof Uint32Array)
      {
         cur = { id: bufs .length };
         bufs .push ({ arr: data, t: now () });
      }
      return bd .call (this, target, data, ... r);
   };

   const draw = WebGL2RenderingContext .prototype .drawArraysInstanced;
   WebGL2RenderingContext .prototype .drawArraysInstanced = function (mode, first, count, k)
   {
      if (count === 6) { ++ draws; ev .push ({ k: "draw", t: now (), n: draws, raf, bufId: cur .id, inst: k }); }
      return draw .call (this, mode, first, count, k);
   };

   let stop = false;
   const loop = t => { ++ raf; ev .push ({ k: "raf", t: now (), ts: t, raf }); if (!stop) requestAnimationFrame (loop); };

   const fnv = a => { let h = 0x811c9dc5; for (let i = 0; i < a .length; ++ i) { h ^= a [i]; h = Math .imul (h, 0x01000193) >>> 0; } return h >>> 0; };
   const sleep = ms => new Promise (r => setTimeout (r, ms));

   const t0 = now ();
   await browser .replaceWorld (await browser .createX3DFromString (text));
   const loadMs = now () - t0;

   requestAnimationFrame (loop);

   const waitFor = async (pred, cap) => { const t = now (); while (!pred () && now () - t < cap) await sleep (5); return pred (); };

   // Wait for "ready" and the first answer (the initial sort), then 20 idle frames.
   await waitFor (() => ev .some (e => e .k === "recv"), 60000);
   { const r0 = raf; await waitFor (() => raf >= r0 + 20, 30000); }

   const steps = [];
   const path = [
      ["flipY", new X3D .SFRotation (0, 1, 0, Math .PI)],
      ["turnX", new X3D .SFRotation (1, 0, 0, Math .PI / 2)],
      ["turnY", new X3D .SFRotation (0, 1, 0, 0.3)],
   ];

   for (const [name, rot] of path)
   {
      const from = ev .length;
      const tSet = now (), rafSet = raf, drawSet = draws;
      browser .currentScene .getNamedNode ("T") .rotation = rot;
      const gotAnswer = await waitFor (() => ev .slice (from) .some (e => e .k === "recv"), 60000);
      { const r0 = raf; await waitFor (() => raf >= r0 + 10, 30000); } // frames after the answer
      steps .push ({ name, from, tSet, rafSet, drawSet, gotAnswer, to: ev .length });
   }

   stop = true;
   await sleep (50);

   const hashes = bufs .map (b => ({ len: b .arr .length, h: fnv (b .arr), first: Array .from (b .arr .subarray (0, 4)) }));
   const out = steps .map (s =>
   {
      const e = ev .slice (s .from, s .to);
      const post = e .find (x => x .k === "post"), recv = e .find (x => x .k === "recv");
      const dr = e .filter (x => x .k === "draw");
      // recv.bufId is the id the next bufferData (the answer upload) takes; a draw that reads a smaller id shows the old order.
      const stale = recv ? dr .filter (d => d .bufId < recv .bufId) : dr;
      const rafs = e .filter (x => x .k === "raf");
      const dts = [];
      for (let i = 1; i < dr .length; ++ i) dts .push (dr [i] .t - dr [i - 1] .t);
      return {
         name: s .name, gotAnswer: s .gotAnswer,
         requestMs: post ? post .t - s .tSet : null,          // step set -> request posted
         answerMs: post && recv ? recv .t - post .t : null,   // request -> answer received (round trip)
         setToAnswerMs: recv ? recv .t - s .tSet : null,
         requestFrame: post ? post .raf - s .rafSet : null,    // rAF callbacks between step set and the request
         answerFrames: post && recv ? recv .raf - post .raf : null, // rAF callbacks between request and answer
         drawsBetween: post && recv ? dr .filter (d => d .t >= post .t && d .t <= recv .t) .length : null,
         drawsFromSetToAnswer: recv ? dr .filter (d => d .t <= recv .t) .length : null,
         drawFrameMs: { median: dts .slice () .sort ((a, b) => a - b) [dts .length >> 1] ?? null, max: Math .max (0, ... dts) },
         rafMs: (() => { const d = []; for (let i = 1; i < rafs .length; ++ i) d .push (rafs [i] .t - rafs [i - 1] .t); return { median: d .slice () .sort ((a, b) => a - b) [d .length >> 1] ?? null, max: Math .max (0, ... d) }; })(),
         mv: post ? post .mv : null,
         draws: dr .map (d => ({ n: d .n, dt: d .t - s .tSet, buf: hashes [d .bufId] ?.h ?? null, bufId: d .bufId, len: hashes [d .bufId] ?.len })),
         posted: post ? post .t - s .tSet : null,
         recvAt: recv ? recv .t - s .tSet : null,
         staleDraws: stale .length,                              // draws from step set that still read the old index buffer
      };
   });

   // The answer of each step: the sorted array uploaded straight after its recv event.
   const recvs = ev .filter (e => e .k === "recv");
   const posts = ev .filter (e => e .k === "post") .map (e => e .mv);
   return { env, loadMs, steps: out, posts, sortedHashes: recvs .map (r => hashes [r .bufId] ?? null), initialBufs: hashes .slice (0, 2), nBufs: hashes .length };
}

const results = { };
const orders = { };
let ok = true;

for (const n of counts .map (Number))
{
   const runs = [];
   let text = world (n);

   for (let run = 0; run < RUNS; ++ run)
   {
      const ctx = await browser .newContext ({ viewport: { width: 512, height: 512 }, deviceScaleFactor: 1 });
      const page = await ctx .newPage ();
      page .on ("pageerror", e => console .error ("pageerror", String (e) .slice (0, 200)));
      await page .goto (`${base}/x_ite-pin/src/shot.html`);
      await page .waitForFunction (() => window .ready, null, { timeout: 60000 });
      const r = await page .evaluate (`(${pageRun .toString ()})(${JSON .stringify (text)})`);
      runs .push (r);
      await ctx .close ();
      console .log (n, "run", run, r .steps .map (s => `${s .name} req ${s .requestMs ?.toFixed (1)}ms ans ${s .answerMs ?.toFixed (1)}ms stale-draws ${s .drawsBetween}`) .join (" | "));
   }

   // Order identity: the posted matrices and the checksums of the buffers the answers carried.
   const key = r => JSON .stringify ({ mv: r .posts, n: r .nBufs, sorted: r .sortedHashes });
   const same = new Set (runs .map (key)) .size === 1;
   ok &&= same;
   results [n] = { runs, orderIdentical: same };
   orders [n] = same ? { matrices: runs [0] .posts, sortedHashes: runs [0] .sortedHashes } : null;
}

const info = { node: process .version, os: `${os .type ()} ${os .release ()} ${os .arch ()}`, cpus: os .cpus () .length, cpuModel: os .cpus () [0] ?.model, chrome: browser .version (), chromeArgs: ANGLE_ARGS [ANGLE], angle: ANGLE };
await browser .close (); server .close ();
const pinned = (await import ("node:fs")) .readFileSync (join (ROOT, "x_ite-pin/PINNED_COMMIT"), "utf8") .trim ();
await writeFile (resolve (outArg), JSON .stringify ({ pinned, runs: RUNS, info, path: "flipY (0 1 0 pi), turnX (1 0 0 pi/2), turnY (0 1 0 0.3)", results }, null, 1) + "\n");
if (ORDER_OUT && ok)
   await writeFile (resolve (ORDER_OUT), JSON .stringify ({ pinned, generator: "x_ite-splat-sort-timing.mjs world(n)", path: "flipY, turnX, turnY", orders }, null, 1) + "\n");
if (!ok) { console .error ("the order data differ between runs; no order file written"); process .exit (1); }
