/**
 * REA geometry oracle (full-port lane): same as x_ite-geometry.mjs, but the cases come from a
 * JSON file ({ name: "<node text>" }), the scene is X3D 4.0, Interchange profile plus COMPONENT statements (Full would load Texturing3D, whose DICOM parser needs the bundled CharLS global), and it also records
 * fogDepths, tangents and every texture coordinate channel.
 *
 *   node oracle/x_ite-geometry-rea.mjs oracle/synthetic-rea/geometry-cases.json oracle/synthetic-rea/geometry-baseline.json
 *
 * Derived from the X7 geometry oracle: build synthetic geometry nodes with the pinned X_ITE (headless system
 * Chrome, page x_ite-pin/src/shot.html) and record the triangle arrays the renderer uploads
 * (X3DGeometryNode vertices / normals / texCoords / colors, geometryType, solid, ccw).
 *
 *   node oracle/x_ite-geometry.mjs oracle/x_ite-geometry.json
 *
 * Every case is synthetic. crates/x3d-render/tests/geometry.rs compares against the JSON.
 */
import { createServer } from "node:http";
import { readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
const { chromium } = createRequire (join (ROOT, "../../../x_ite-tests/")) ("playwright");

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json" };

const [ casesArg, outArg ] = process .argv .slice (2);
const CASES = JSON .parse (await readFile (resolve (casesArg), "utf8"));
const HEADER = "#X3D V4.0 utf8\nPROFILE Interchange\nCOMPONENT Geometry2D:2\nCOMPONENT CADGeometry:2\nCOMPONENT Rendering:5\nCOMPONENT EnvironmentalEffects:4\nCOMPONENT Texturing:3\nCOMPONENT Shaders:1\nCOMPONENT NURBS:4\nCOMPONENT Geospatial:2\n";

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

   const result = await page .evaluate (async ([ cases, header ]) =>
   {
      const browser = document .getElementById ("c") .browser;
      const out     = { };

      // X_ITE loads its MikkTSpace wasm on the first generateTangents and skips the tangents of
      // that geometry until it is ready; warm it up so every case sees the steady state.
      {
         const scene = await browser .createX3DFromString (`${header}DEF G Box { }`);
         const node  = scene .getNamedNode ("G") .getValue ();

         for (let i = 0; i < 200 && !node .getTangents () .length; ++ i)
         {
            await new Promise (r => setTimeout (r, 25));
            node .rebuild ();
         }
      }

      for (const [name, text] of Object .entries (cases))
      {
         const scene = await browser .createX3DFromString (`${header}DEF G ${text}`);
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
            fogDepths: arr (node .getFogDepths ()),
            tangents: arr (node .getTangents ()),
            multiTexCoords: node .getMultiTexCoords () .map (arr),
            attribs: node .getAttribs () .map (arr),
            min: Array .from (node .getMin ()),
            max: Array .from (node .getMax ()),
         };
      }

      return out;
   },
   [ CASES, HEADER ]);

   const json = { pinned: "oracle/x_ite-pin (X_ITE 16.5.0)", browser: browser .version (), cases: { } };

   for (const name of Object .keys (CASES))
      json .cases [name] = { vrml: CASES [name], header: HEADER, ... result [name] };

   await writeFile (resolve (outArg), JSON .stringify (json, null, 1) + "\n");
   console .log (`${Object .keys (CASES) .length} cases`);
}
finally
{
   await browser .close ();
   server .close ();
}
