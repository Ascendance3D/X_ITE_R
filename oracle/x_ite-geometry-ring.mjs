/**
 * X7 geometry oracle: build synthetic geometry nodes with the pinned X_ITE (headless system
 * Chrome, page x_ite-pin/src/shot.html) and record the triangle arrays the renderer uploads
 * (X3DGeometryNode vertices / normals / texCoords / colors, geometryType, solid, ccw).
 *
 *   node oracle/x_ite-geometry-ring.mjs oracle/x_ite-geometry-ring.json
 *
 * Derived from x_ite-geometry.mjs for the orrery ring (closed-spine Extrusion). crates/x3d-render/tests/geometry_ring.rs compares against the JSON.
 */
import { createServer } from "node:http";
import { readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
const { chromium } = createRequire (join (ROOT, "../../../x_ite-tests/")) ("playwright");

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json" };

const CASES = {
   // The ring of oracle/render-items/antiques/chronos-orrery (closed spine, closed square cross-section, default caps).
   orrery_ring_caps:   `Extrusion {
   crossSection [ -0.038 -0.038, 0.038 -0.038, 0.038 0.038, -0.038 0.038, -0.038 -0.038 ]
   spine [ 0.95 0 0, 0.823 0 0.475, 0.475 0 0.823, 0 0 0.95, -0.475 0 0.823, -0.823 0 0.475, -0.95 0 0, -0.823 0 -0.475, -0.475 0 -0.823, 0 0 -0.95, 0.475 0 -0.823, 0.823 0 -0.475, 0.95 0 0 ]
   creaseAngle 0.7  }`,
   orrery_ring_nocaps: `Extrusion {
   crossSection [ -0.038 -0.038, 0.038 -0.038, 0.038 0.038, -0.038 0.038, -0.038 -0.038 ]
   spine [ 0.95 0 0, 0.823 0 0.475, 0.475 0 0.823, 0 0 0.95, -0.475 0 0.823, -0.823 0 0.475, -0.95 0 0, -0.823 0 -0.475, -0.475 0 -0.823, 0 0 -0.95, 0.475 0 -0.823, 0.823 0 -0.475, 0.95 0 0 ]
   creaseAngle 0.7 beginCap FALSE endCap FALSE }`,
};

const server = createServer (async (req, res) =>
{
   const path = normalize (join (ROOT, decodeURIComponent (new URL (req .url, "http://x") .pathname)));

   if (!path .startsWith (ROOT)) { res .writeHead (403); return res .end (); }

   try
   {
      const body = await readFile (path);
      res .writeHead (200, { "Content-Type": TYPES [extname (path) .toLowerCase ()] ?? "application/octet-stream", "Cache-Control": "no-store" });
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

try
{
   const context = await browser .newContext ({ viewport: { width: 512, height: 512 } });

   await context .route (url => !url .href .startsWith (base), route => route .abort ());

   const page = await context .newPage ();

   await page .goto (`${base}/x_ite-pin/src/shot.html`);
   await page .waitForFunction (() => window .ready === true, null, { timeout: 30000 });

   const result = await page .evaluate (async cases =>
   {
      const browser = document .getElementById ("c") .browser;
      const out     = { };

      for (const [name, text] of Object .entries (cases))
      {
         const scene = await browser .createX3DFromString (`#VRML V2.0 utf8\nDEF G ${text}`);
         const node  = scene .getNamedNode ("G") .getValue ();

         node .rebuild ();

         const arr = a => Array .from (a .getValue ());

         out [name] = {
            geometryType: node .getGeometryType (),
            solid: node .isSolid (),
            ccw: node .getCCW (),
            vertices: arr (node .getVertices ()),
            normals: arr (node .getNormals ()),
            texCoords: node .getMultiTexCoords () .length ? arr (node .getMultiTexCoords () [0]) : [ ],
            colors: arr (node .getColors ()),
            min: Array .from (node .getMin ()),
            max: Array .from (node .getMax ()),
         };
      }

      return out;
   },
   CASES);

   const json = { pinned: "oracle/x_ite-pin (X_ITE 16.5.0)", browser: browser .version (), cases: { } };

   for (const name of Object .keys (CASES))
      json .cases [name] = { vrml: CASES [name], ... result [name] };

   await writeFile (resolve (process .argv [2] ?? "oracle/x_ite-geometry-ring.json"), JSON .stringify (json, null, 1) + "\n");
   console .log (`${Object .keys (CASES) .length} cases`);
}
finally
{
   await browser .close ();
   server .close ();
}
