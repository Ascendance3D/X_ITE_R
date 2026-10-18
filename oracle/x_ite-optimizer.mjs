/**
 * X3DOptimizer oracle: parse each world with the pinned X_ITE source, run X3DOptimizer
 * .optimizeSceneGraph on the scene's root nodes with each option set, and record the scene
 * printed by toVRMLString. X_ITE only calls the optimizer from OBJParser (removeEmptyGroups)
 * and GLTF2Parser (all options); this calls it directly so every path is observable.
 *
 *   node oracle/x_ite-optimizer.mjs <dir> <out.json>
 *
 * Same page, server and isolation as x_ite-xmljson.mjs.
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
   .filter (f => f !== "PROVENANCE.json" && /\.(wrl|wrz)(\.gz)?$/i .test (f))
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

const OPTIONS_PARAM = {
   none:      { },
   remove:    { removeEmptyGroups: true },
   combine:   { combineGroupingNodes: true },
   all:       { removeEmptyGroups: true, combineGroupingNodes: true, optimizeInterpolators: true },
};

const base    = `http://127.0.0.1:${server .address () .port}`;
const browser = await chromium .launch ({ headless: true });
const results = [ ];

for (const file of files)
{
   // Fresh context per file: no state, cache, or DNS memory leaks between files.
   const context = await browser .newContext ();

   await context .route (url => !url .href .startsWith (base), route => route .abort ());

   const page = await context .newPage ();

   await page .goto (`${base}/x_ite-pin/src/dump.html`);
   await page .waitForFunction (() => window .ready, null, { timeout: 30000 });

   const url = `${base}/${relative (ROOT, join (corpus, file))}`;
   const r   = await Promise .race ([
      page .evaluate (async ({ u, OPTIONS }) =>
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
            const scene   = await browser .createX3DFromURL (new X3D .MFString (u));

            // Each serializer is tried on its own so one failure does not hide the other.
            const attempt = fn =>
            {
               try { return { value: fn (), error: null }; }
               catch (e) { return { value: null, error: String (e ?.message ?? e) .split ("\n") [0] }; }
            };

            const { default: X3DOptimizer } = await import ("/x_ite-pin/src/x_ite/Parser/X3DOptimizer.js");
            const options = { };

            for (const [key, flags] of Object .entries (OPTIONS))
            {
               const scene = await browser .createX3DFromURL (new X3D .MFString (u));
               const optimizer = Object .create (X3DOptimizer .prototype);

               X3DOptimizer .call (optimizer);
               Object .assign (optimizer, flags);

               options [key] = attempt (() => { optimizer .optimizeSceneGraph (scene .rootNodes); return scene .toVRMLString (); });
            }

            return { ok: true, options, log: log .sort () };
         }
         catch (e)
         {
            return { ok: false, error: String (e ?.message ?? e) .split ("\n") [0], log: log .sort () };
         }
         finally
         {
            Object .assign (console, orig);
         }
      }, { u: url, OPTIONS: OPTIONS_PARAM }),
      new Promise (r => setTimeout (() => r ({ ok: false, error: "TIMEOUT 30s", log: [ ] }), 30000)),
   ]);

   // Local URLs vary by port; normalize them so two runs are comparable.
   const text = JSON .stringify (r) .replaceAll (base, "http://oracle");

   results .push ({ file, ... JSON .parse (text) });
   console .log (`${r .ok ? "ok  " : "FAIL"} ${file}${r .ok ? "" : "  " + r .error}`);

   await context .close ();
}

await browser .close ();
server .close ();
await writeFile (outArg, JSON .stringify ({ pinned: (await readFile (join (ROOT, "x_ite-pin/PINNED_COMMIT"), "utf8")) .trim (), results }, null, 1));
console .log (`\n${results .filter (r => r .ok) .length} of ${results .length} parsed`);
