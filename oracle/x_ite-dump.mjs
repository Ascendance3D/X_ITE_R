/**
 * X0 oracle: parse each VRML file with the pinned X_ITE source and print the scene
 * re-serialized by X_ITE's own VRMLGenerator. The Rust port must produce the same text.
 *
 *   node oracle/x_ite-dump.mjs <corpus_dir> <out.json>
 *
 * Headless. All non-local requests are aborted, so dead 1990s hosts cost nothing
 * and results do not depend on the network.
 */
import { createServer } from "node:http";
import { readFile, readdir, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { basename, extname, join, normalize, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { checkOutput } from "./check-output.mjs";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
const { chromium } = createRequire (join (ROOT, "../../../x_ite-tests/")) ("playwright");

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json" };

const [ corpusArg, outArg ] = process .argv .slice (2);

checkOutput (outArg);
const corpus = normalize (join (process .cwd (), corpusArg));
// Recursive, so corpora that keep their directory layout (corpus-items) work too.
// For a flat corpus the relative path is just the file name, as before.
const files  = (await readdir (corpus, { recursive: true, withFileTypes: true }))
   .filter (d => d .isFile ())
   .map (d => relative (corpus, join (d .parentPath, d .name)))
   .filter (f => f !== "PROVENANCE.json" && (!f .includes ("/") || /\.(wrl|wrz)(\.gz)?$/i .test (f)))
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
      page .evaluate (u => window .dump (u), url),
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
