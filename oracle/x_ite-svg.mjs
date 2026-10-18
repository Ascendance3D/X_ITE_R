/**
 * SVG parser oracle: parse each input with the pinned X_ITE
 * source and record the scene re-serialized by X_ITE's own XMLGenerator (scene.toXMLString),
 * JSONGenerator (scene.toJSONString) and VRML generator (scene.toVRMLString), together with the
 * detected encoding and profile. The Rust port must produce the same text.
 *
 *   node oracle/x_ite-svg.mjs <dir> <out.json>
 *
 * Same loading path as x_ite-dump.mjs: the pinned src/dump.html page (left unedited), a fresh
 * browser context per file, headless, and every non-local request aborted. The only difference
 * is what is read from the scene: dump.html only exposes window.dump (toVRMLString), so the
 * X3D instance and browser are reached here through the same page globals it uses.
 */
import { createServer } from "node:http";
import { readFile, readdir, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { extname, join, normalize, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
const { chromium } = createRequire (join (ROOT, "../../../x_ite-tests/")) ("playwright");

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json" };

const [ corpusArg, outArg ] = process .argv .slice (2);
const corpus = normalize (join (process .cwd (), corpusArg));
// Recursive, like x_ite-dump.mjs, so corpora with directory layouts work too.
const files  = (await readdir (corpus, { recursive: true, withFileTypes: true }))
   .filter (d => d .isFile ())
   .map (d => relative (corpus, join (d .parentPath, d .name)))
   // Baselines written next to the inputs (*.json) are never inputs themselves.
   .filter (f => f !== "PROVENANCE.json" && !f .endsWith ("-baseline.json") && /\.(svg)$/i .test (f))
   .sort ();

const server = createServer (async (req, res) =>
{
   const path = normalize (join (ROOT, decodeURIComponent (new URL (req .url, "http://x") .pathname)));

   if (!path .startsWith (ROOT)) { res .writeHead (403); return res .end (); }

   try
   {
      const body = await readFile (path);
      res .writeHead (200, { "Content-Type": TYPES [extname (path)] ?? "model/vrml", "Cache-Control": "no-store" });
      res .end (body);
   }
   catch { res .writeHead (404, { "Cache-Control": "no-store" }); res .end (); }
});

await new Promise (r => server .listen (0, "127.0.0.1", r));

const base    = `http://127.0.0.1:${server .address () .port}`;
const browser = await chromium .launch ({ headless: true });
const results = new Array (files .length);

// Same page, evaluation and normalization as x_ite-formats.mjs. Several files run at once, and a
// page that does not come up (a loaded machine) is retried.
async function one (file)
{
   for (let attempt = 0; ; ++ attempt)
   {
      // Fresh context per file: no state, cache, or DNS memory leaks between files.
      const context = await browser .newContext ();

      try
      {
         await context .route (url => !url .href .startsWith (base), route => route .abort ());

         const page = await context .newPage ();

         await page .goto (`${base}/x_ite-pin/src/dump.html`);
         await page .waitForFunction (() => window .ready, null, { timeout: 60000 });

         const url = `${base}/${relative (ROOT, join (corpus, file))}`;
         const r   = await Promise .race ([
            page .evaluate (async ([u, q]) =>
            {
               const log  = [ ];
               const orig = { };

               for (const m of [ "error", "warn" ])
               {
                  orig [m] = console [m];
                  console [m] = (... a) => log .push (`${m}: ${a .map (String) .join (" ") .split ("\n") [0]}`);
               }

               try
               {
                  const X3D     = window [Symbol .for ("X_ITE.X3D")];
                  const browser = document .getElementById ("c") .browser;

                  // SVG_QUALITY="LOW,HIGH": PrimitiveQuality,TextureQuality set before the load.
                  if (q)
                  {
                     browser .setBrowserOption ("PrimitiveQuality", q [0]);
                     browser .setBrowserOption ("TextureQuality", q [1]);
                  }

                  const scene   = await browser .createX3DFromURL (new X3D .MFString (u));

                  // Each serializer is tried on its own so one failure does not hide the others.
                  const attempt = fn =>
                  {
                     try { return { value: fn (), error: null }; }
                     catch (e) { return { value: null, error: String (e ?.message ?? e) .split ("\n") [0] }; }
                  };

                  const xml  = attempt (() => scene .toXMLString ());
                  const json = attempt (() => scene .toJSONString ());
                  const vrml = attempt (() => scene .toVRMLString ());

                  return { ok: true, xml: xml .value, xmlError: xml .error, json: json .value, jsonError: json .error,
                           vrml: vrml .value, vrmlError: vrml .error, encoding: scene .encoding, profile: scene .profile ?.name, log: log .sort () };
               }
               catch (e)
               {
                  return { ok: false, error: String (e ?.message ?? e) .split ("\n") [0], log: log .sort () };
               }
               finally
               {
                  Object .assign (console, orig);
               }
            }, [url, process .env .SVG_QUALITY ?.split (",") ?? null]),
            new Promise (r => setTimeout (() => r ({ ok: false, error: "TIMEOUT 30s", log: [ ] }), 30000)),
         ]);

         // Local URLs vary by port; normalize them so two runs are comparable.
         const text = JSON .stringify (r) .replaceAll (base, "http://oracle");

         await context .close ();
         return { file, ... JSON .parse (text) };
      }
      catch (error)
      {
         await context .close () .catch (() => { });

         if (attempt >= 3)
            throw error;
      }
   }
}

let next = 0;

await Promise .all (Array .from ({ length: 6 }, async () =>
{
   while (next < files .length)
   {
      const i = next ++;

      results [i] = await one (files [i]);
      console .log (`${results [i] .ok ? "ok  " : "FAIL"} ${files [i]}${results [i] .ok ? "" : "  " + results [i] .error}`);
   }
}));

await browser .close ();
server .close ();
await writeFile (outArg, JSON .stringify ({ pinned: (await readFile (join (ROOT, "x_ite-pin/PINNED_COMMIT"), "utf8")) .trim (), results }, null, 1));
console .log (`\n${results .filter (r => r .ok) .length} of ${results .length} parsed`);
