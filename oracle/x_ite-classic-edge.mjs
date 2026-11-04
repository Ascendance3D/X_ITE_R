/**
 * Classic-encoding edge-case oracle (sub:parse.vrml-classic): parse each authored input with the pinned X_ITE source and
 * record the scene re-serialized by X_ITE's own generators (toVRMLString, toXMLString, toJSONString), the detected
 * encoding, profile, components, units and meta data, the exact rejection text, and the console output (every level, full
 * text, in print order).
 *
 *   node oracle/x_ite-classic-edge.mjs <cases.json> <out.json>
 *
 * cases.json is a list of { name, text } (createX3DFromString) or { name, b64 } (createX3DFromURL of those exact bytes
 * served as model/x3d+vrml). oracle/tools/classic_edge.py makes the file and records three runs.
 *
 * Same page as x_ite-dump.mjs and x_ite-formats.mjs: the pinned src/dump.html (left unedited), a fresh browser context per
 * case, headless, every non-local request aborted. Needs oracle/x_ite-pin/node_modules and XITE_TESTS (oracle/README.md,
 * "Recording from a worktree").
 */
import { createServer } from "node:http";
import { existsSync } from "node:fs";
import { readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { checkOutput } from "./check-output.mjs";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));

if (!existsSync (join (ROOT, "x_ite-pin", "node_modules")))
   throw new Error ("oracle/x_ite-pin/node_modules is missing: link it first (oracle/README.md, Recording from a worktree)");

const { chromium } = createRequire (join (process .env .XITE_TESTS ?? join (ROOT, "../../../x_ite-tests"), "/")) ("playwright");

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json" };

const [ casesArg, outArg ] = process .argv .slice (2);

checkOutput (outArg);

const cases = JSON .parse (await readFile (casesArg, "utf8"));
const bytes = new Map (cases .filter (c => c .b64 !== undefined) .map (c => [ `/__case/${encodeURIComponent (c .name)}`, Buffer .from (c .b64, "base64") ]));

const server = createServer (async (req, res) =>
{
   const url = new URL (req .url, "http://x");

   if (bytes .has (url .pathname))
   {
      res .writeHead (200, { "Content-Type": "model/x3d+vrml", "Cache-Control": "no-store" });
      return res .end (bytes .get (url .pathname));
   }

   const path = normalize (join (ROOT, decodeURIComponent (url .pathname)));

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
const results = new Array (cases .length);

async function record (index, attempt = 1)
{
   const c       = cases [index];
   const context = await browser .newContext ();

   await context .route (url => !url .href .startsWith (base), route => route .abort ());

   const page = await context .newPage ();

   try
   {
      await page .goto (`${base}/x_ite-pin/src/dump.html`);
      await page .waitForFunction (() => window .ready, null, { timeout: 90000 });
   }
   catch (error)
   {
      // A page that does not start under load is started again; it says nothing about the case.
      await context .close ();

      if (attempt >= 3)
         throw error;

      return record (index, attempt + 1);
   }

   const r = await Promise .race ([
      page .evaluate (async ([ text, url ]) =>
      {
         const log  = [ ];
         const orig = { };

         for (const m of [ "log", "info", "warn", "error", "debug" ])
         {
            orig [m] = console [m];
            console [m] = (... a) => log .push (`${m}: ${a .map (String) .join (" ")}`);
         }

         try
         {
            const X3D     = window [Symbol .for ("X_ITE.X3D")];
            const browser = document .getElementById ("c") .browser;
            const scene   = text !== null
               ? await browser .createX3DFromString (text)
               : await browser .createX3DFromURL (new X3D .MFString (url));

            // Each serializer is tried on its own so one failure does not hide the others.
            const attempt = fn =>
            {
               try { return { value: fn (), error: null }; }
               catch (e) { return { value: null, error: String (e ?.message ?? e) }; }
            };

            const xml  = attempt (() => scene .toXMLString ());
            const json = attempt (() => scene .toJSONString ());
            const vrml = attempt (() => scene .toVRMLString ());

            return {
               ok: true,
               xml: xml .value, xmlError: xml .error,
               json: json .value, jsonError: json .error,
               vrml: vrml .value, vrmlError: vrml .error,
               encoding: scene .encoding,
               specificationVersion: scene .specificationVersion,
               profile: scene .profile ?.name ?? null,
               components: Array .from (scene .components, c => [ c .name, c .level ]),
               units: Array .from (scene .units, u => [ u .category, u .name, Number .isFinite (u .conversionFactor) ? u .conversionFactor : String (u .conversionFactor) ]),
               meta: scene .getMetaDatas (),
               rootNodes: scene .rootNodes .length,
               log,
            };
         }
         catch (e)
         {
            return { ok: false, error: String (e ?.message ?? e), log };
         }
         finally
         {
            Object .assign (console, orig);
         }
      }, [ c .text ?? null, `${base}/__case/${encodeURIComponent (c .name)}` ]),
      new Promise (r => setTimeout (() => r ({ ok: false, error: "TIMEOUT 30s", log: [ ] }), 30000) .unref ()),
   ]);

   // A module of a component that the page could not fetch while the machine was busy, or a case that timed out under load, says
   // nothing about the parser: run the case again (the last result stays, whatever it is).
   if (!r .ok && /Failed to fetch dynamically imported module|TIMEOUT/ .test (r .error) && attempt < 4)
   {
      await context .close ();

      return record (index, attempt + 1);
   }

   // X_ITE's own progress notices (the component loader's "Done loading ..." in the order the modules arrive, the lazy shader
   // "Initializing ...") depend on timing and are not parser output.
   r .log = r .log .filter (line => !/^info: (Done loading |Initializing )/ .test (line));

   // Local URLs vary by port; normalize them so two runs are comparable.
   results [index] = { name: c .name, ... JSON .parse (JSON .stringify (r) .replaceAll (base, "http://oracle")) };

   await context .close ();
}

let next = 0;

await Promise .all (Array .from ({ length: Number (process .env .CLASSIC_EDGE_JOBS ?? 6) }, async () =>
{
   while (next < cases .length)
      await record (next ++);
}));

await browser .close ();
server .close ();
await writeFile (outArg, JSON .stringify ({ pinned: (await readFile (join (ROOT, "x_ite-pin/PINNED_COMMIT"), "utf8")) .trim (), results }, null, 1));
console .log (`${results .filter (r => r .ok) .length} of ${results .length} parsed`);
