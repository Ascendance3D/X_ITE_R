/**
 * REA batch 06R: read the real per-particle state of a ParticleSystem from the pinned X_ITE
 * (oracle/x_ite-pin/src/anim.html, headless Chrome, virtual time like x_ite-anim.mjs).
 *
 *   node oracle/tools/x_ite_particles.mjs <world inside oracle/> <script.json> [--out result.json]
 *
 * script.json:
 *   { "dt": 0.0166667, "t0": 946684800, "frames": 40, "warmup": 8, "node": "PS",
 *     "seed": 12345,                 // mulberry32 seed of the replaced Math.random
 *     "dump": [1, 10, 40],           // frames after which the particle buffer is read
 *     "dumpAll": false,              // true: dump every frame 1..frames
 *     "filter": "born" }             // optional: dump only records with elapsedTime === 0 (created this frame)
 *
 * Output (result.json plus <result>.f32 beside it):
 *   { ok, dt, t0, frames, warmup, seed, stride: 28, node, geometryType, maxParticles,
 *     particleLifetime, lifetimeVariation, emitter: <type>, fileF32,
 *     init: { animateCalls, deltaTimeBeforeReset },
 *     steps: [ { frame, time, randomSeed, numParticles, deltaTime, creationTime, createParticles } ... ],
 *     dumps: { "<frame>": { numParticles (records written), total (numParticles of the system), offset } },      // offset in floats into the .f32 file
 *     log: [...] }
 * One .f32 record is stride (28) little-endian float32 per particle, in the order of X_ITE's
 * interleaved transform-feedback buffer (7 x vec4):
 *   [0..3]   life (int as float), lifetime, elapsedTime, texCoordIndex0
 *   [4..7]   color rgba
 *   [8..11]  velocity xyz, 0
 *   [12..23] scale/rotation matrix rows (3 x vec4)
 *   [24..27] position xyz, w (NaN for hidden particles)
 *
 * Determinism protocol (the page is anim.html, untouched):
 *   * Date.now is pinned to t0 * 1000 before the page loads, so creationTime = t0 and the load
 *     frames create no particles; after load it is the harness's virtual clock.
 *   * Math.random is replaced before any page script runs by mulberry32(seed) and RESEEDED right
 *     after the load + warmup frames, so the per-frame emitter seeds do not depend on how many
 *     real frames the load took.
 *   * ParticleSystem.deltaTime (moving average, starts at 0 and converges to dt) depends on the
 *     number of animateParticles calls during load; it is reset to 0 after the warmup so frame 1
 *     is the first animate. The pre-reset value is reported in init.deltaTimeBeforeReset.
 *   * The GLSL randomSeed uniform of frame k is  ToInt32 (trunc (Math.random () * 0xffffffff)),
 *     recorded per step as steps[k-1].randomSeed (the number passed to gl.uniform1i).
 */
import { createServer } from "node:http";
import { readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { extname, join, normalize, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = normalize (join (fileURLToPath (import.meta.url), "../.."));
const { chromium } = createRequire (join (ROOT, "../../../x_ite-tests/")) ("playwright");
const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".json": "application/json", ".png": "image/png", ".jpg": "image/jpeg" };

const argv = process .argv .slice (2);
const opt  = (name, def) => { const i = argv .indexOf (name); return i >= 0 ? argv [i + 1] : def; };
const [ fileArg, scriptArg ] = argv .filter ((a, i) => !a .startsWith ("--") && !argv [i - 1] ?.startsWith ("--"));
const script  = JSON .parse (await readFile (scriptArg, "utf8"));
const outPath = resolve (opt ("--out", "particles.json"));

const t0     = script .t0 ?? 946684800;
const dt     = script .dt ?? 1 / 60;
const frames = script .frames ?? 1;
const warmup = script .warmup ?? 8;
const seed   = script .seed ?? 1;
const nodeName = script .node ?? "PS";
const dump   = new Set (script .dumpAll ? Array .from ({ length: frames }, (_, i) => i + 1) : script .dump ?? [ frames ]);
const STRIDE = 28;

const server = createServer (async (req, res) =>
{
   const path = normalize (join (ROOT, decodeURIComponent (new URL (req .url, "http://x") .pathname)));
   if (!path .startsWith (ROOT)) { res .writeHead (403); return res .end (); }
   try { const body = await readFile (path); res .writeHead (200, { "Content-Type": TYPES [extname (path) .toLowerCase ()] ?? "model/vrml", "Cache-Control": "no-store" }); res .end (body); }
   catch { res .writeHead (404, { "Cache-Control": "no-store" }); res .end (); }
});

await new Promise (r => server .listen (0, "127.0.0.1", r));
const base = `http://127.0.0.1:${server .address () .port}`;
const chrome = await chromium .launch ({ executablePath: "/usr/bin/google-chrome", headless: true, args: [ "--headless=new", "--no-sandbox", "--use-gl=angle", "--use-angle=gl", "--enable-gpu", "--ignore-gpu-blocklist" ] });
const context = await chrome .newContext ({ viewport: { width: 256, height: 256 }, deviceScaleFactor: 1 });
await context .route (url => !url .href .startsWith (base), route => route .abort ());
const page = await context .newPage ();

await page .addInitScript (([ s, date0 ]) =>
{
   // Seeded Math.random (mulberry32), reseedable.
   let a = s >>> 0;
   window .__reseed = v => { a = v >>> 0; };
   Math .random = () =>
   {
      a = (a + 0x6D2B79F5) >>> 0;
      let t = a;
      t = Math .imul (t ^ (t >>> 15), t | 1);
      t ^= t + Math .imul (t ^ (t >>> 7), t | 61);
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
   };
   // X_ITE's ResizeObserver calls browser.reshape () -> a full traverse (and an extra animateParticles)
   // at wall-clock-dependent moments. Make it inert; the canvas size is fixed.
   window .ResizeObserver = class { observe () { } unobserve () { } disconnect () { } };
   // Pin Date.now until anim.html installs its virtual clock.
   Date .now = () => date0;
   // Record every value handed to a randomSeed uniform.
   window .__seedLocs = new Set ();
   window .__seeds = [ ];
   const u1i = WebGL2RenderingContext .prototype .uniform1i;
   WebGL2RenderingContext .prototype .uniform1i = function (loc, v)
   {
      if (loc && window .__seedLocs .has (loc))
         window .__seeds .push (v);
      return u1i .call (this, loc, v);
   };
}, [ seed, t0 * 1000 ]);

const result = { ok: false, dt, t0, frames, warmup, seed, stride: STRIDE, node: nodeName, steps: [ ], dumps: { }, log: [ ] };
const chunks = [ ];
let floats = 0;

try
{
   await page .goto (`${base}/x_ite-pin/src/anim.html`);
   await page .waitForFunction (() => window .ready, null, { timeout: 30000 });

   const url = `${base}/${relative (ROOT, resolve (fileArg)) .split ("/") .map (encodeURIComponent) .join ("/")}`;
   const loaded = await page .evaluate (([u, t, d, w]) => window .load (u, t, d, w), [ url, t0, dt, warmup ]);
   if (!loaded .ok) throw new Error (loaded .error);
   await page .evaluate (d => window .setDt (d), dt);

   // Normalize: reseed Math.random, reset deltaTime, report what the load did.
   result .init = await page .evaluate (([name, s, t0]) =>
   {
      const browser = document .getElementById ("c") .browser;
      const ps = browser .currentScene .getNamedNode (name) .getValue () .getInnerNode ();
      const out = { deltaTimeBeforeReset: ps .deltaTime, numParticlesAfterLoad: ps .numParticles, creationTimeAfterLoad: ps .creationTime };
      // Back to the state before the first animateParticles: nothing alive, zeroed buffers, deltaTime 0.
      // creationTime is Date.now () at set_maxParticles for ordinary emitters (= t0, pinned) but stays 0
      // for explosive ones, whose first burst therefore happens at the first animate call.
      const gl = browser .getContext (), explosive = ps .emitterNode .isExplosive ();
      out .explosive = explosive;
      ps .deltaTime    = 0;
      ps .numParticles = 0;
      ps .creationTime = explosive ? 0 : t0;
      for (const buf of [ ps .inputParticles, ps .outputParticles ])
      {
         gl .bindBuffer (gl .ARRAY_BUFFER, buf);
         gl .bufferData (gl .ARRAY_BUFFER, new Uint8Array (ps .maxParticles * ps .particlesStride), gl .DYNAMIC_DRAW);
      }
      window .__reseed (s);
      window .__seeds .length = 0;
      window .__ps = ps;
      window .__calls = 0;
      window .__stack = [ ]; window .__debug = false;
      const en = ps .emitterNode, orig = en .animate;
      en .animate = function (... a) { ++ window .__calls; if (window .__debug) window .__stack .push (new Error () .stack .split ("\n") .slice (1, 16) .join (" | ")); return orig .apply (this, a); };
      return out;
   }, [ nodeName, seed, t0 ]);

   result .info = await page .evaluate (() =>
   {
      const ps = window .__ps;
      return { geometryType: ps .geometryType, maxParticles: ps .maxParticles, particleLifetime: ps .particleLifetime,
         lifetimeVariation: ps .lifetimeVariation, emitter: ps .emitterNode .getTypeName (), numForces: ps .numForces,
         boundedHierarchyRoot: ps .boundedHierarchyRoot, stride: ps .particlesStride / 4 };
   });
   if (result .info .stride !== STRIDE) throw new Error ("unexpected particle stride " + result .info .stride);

   for (let k = 1; k <= frames; ++ k)
   {
      const t = t0 + k * dt;

      const step = await page .evaluate (([tt, name]) =>
      {
         const ps = window .__ps;
         for (const p of ps .emitterNode .programs .values ()) window .__seedLocs .add (p .randomSeed);
         window .__seeds .length = 0;
         const before = window .__calls;
         window .setDate (tt * 1000);
         window .step (tt);
         return { calls: window .__calls - before, randomSeeds: window .__seeds .slice (), numParticles: ps .numParticles, deltaTime: ps .deltaTime,
            creationTime: ps .creationTime, createParticles: !! ps .createParticles };
      }, [ t, nodeName ]);

      if (step .calls !== 1) result .log .push (`frame ${k}: ${step .calls} emitter.animate calls`);
      if (step .randomSeeds .length !== 1)
         result .log .push (`frame ${k}: ${step .randomSeeds .length} randomSeed uniform writes`);

      result .steps .push ({ frame: k, time: t, randomSeed: step .randomSeeds [0] ?? null, numParticles: step .numParticles,
         deltaTime: step .deltaTime, creationTime: step .creationTime, createParticles: step .createParticles });

      if (dump .has (k))
      {
         const b64 = await page .evaluate (born =>
         {
            const ps = window .__ps, browser = document .getElementById ("c") .browser, gl = browser .getContext ();
            const n = ps .numParticles, data = new Float32Array (n * 28);
            const prev = gl .getParameter (gl .ARRAY_BUFFER_BINDING);
            gl .bindBuffer (gl .ARRAY_BUFFER, ps .outputParticles);
            if (n) gl .getBufferSubData (gl .ARRAY_BUFFER, 0, data);
            gl .bindBuffer (gl .ARRAY_BUFFER, prev);
            let out = data, count = n;
            if (born)
            {
               // Keep the records created this frame (elapsedTime === 0), in buffer order.
               const keep = [ ];
               for (let i = 0; i < n; ++ i) if (data [i * 28 + 2] === 0) keep .push (i);
               out = new Float32Array (keep .length * 28);
               keep .forEach ((i, j) => out .set (data .subarray (i * 28, i * 28 + 28), j * 28));
               count = keep .length;
            }
            const bytes = new Uint8Array (out .buffer);
            let bin = "";
            for (let i = 0; i < bytes .length; i += 0x8000) bin += String .fromCharCode .apply (null, bytes .subarray (i, i + 0x8000));
            return { n: count, total: n, b64: btoa (bin) };
         }, script .filter === "born");

         const buf = Buffer .from (b64 .b64, "base64");
         result .dumps [k] = { numParticles: b64 .n, total: b64 .total, offset: floats };
         chunks .push (buf);
         floats += buf .length / 4;
      }
   }

   // The geometry textures X_ITE built for the emitter / bounded models (what the shader samples).
   result .geometry = await page .evaluate (() =>
   {
      const ps = window .__ps, en = ps .emitterNode, arr = a => a ? Array .from (a) : null, g = { };
      const tex = (name, array, rows) => { g [name] = { array: arr (array) }; };
      if (en .surfaceArray) g .surface = { verticesIndex: en .verticesIndex, normalsIndex: en .normalsIndex, array: arr (en .surfaceArray) };
      if (en .volumeArray)  g .volume  = { verticesIndex: en .verticesIndex, normalsIndex: en .normalsIndex, hierarchyIndex: en .hierarchyIndex, hierarchyRoot: en .hierarchyRoot, array: arr (en .volumeArray) };
      if (ps .boundedHierarchyRoot > -1) g .bounded = { vertices: arr (ps .boundedVertices), normals: arr (ps .boundedNormals), hierarchyIndex: ps .boundedHierarchyIndex, hierarchyRoot: ps .boundedHierarchyRoot };
      g .numForces = ps .numForces;
      return g;
   });

   // Every step must be exactly one emitter.animate (anything else is a nondeterminism source).
   result .animateCalls = await page .evaluate (() => window .__calls);
   if (result .animateCalls !== frames)
      result .log .push (`animate called ${result .animateCalls} times in ${frames} steps`);

   result .ok = true;
}
catch (e)
{
   result .error = String (e ?.message ?? e);
}
finally
{
   result .log .push (... (await page .evaluate (() => window .getLog ?.() ?? [ ]) .catch (() => [ ])));
   await chrome .close ();
   server .close ();
}

result .fileF32 = outPath .replace (/\.json$/, "") + ".f32";
await writeFile (result .fileF32, Buffer .concat (chunks));
result .fileF32 = result .fileF32 .split ("/") .pop ();
await writeFile (outPath, JSON .stringify (result, null, 1));
console .log (result .ok ? `ok ${outPath} (${floats} floats)` : `FAILED: ${result .error}`);
process .exit (result .ok ? 0 : 1);
