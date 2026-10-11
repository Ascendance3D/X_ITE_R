/**
 * X7 geometry oracle: build synthetic geometry nodes with the pinned X_ITE (headless system
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

const CASES = {
   box_default:       "Box { }",
   box_size:          "Box { size 1 2 3 }",
   sphere_default:    "Sphere { }",
   sphere_r:          "Sphere { radius 2.5 }",
   cone_default:      "Cone { }",
   cone_nobottom:     "Cone { bottomRadius 0.5 height 3 bottom FALSE }",
   cone_noside:       "Cone { side FALSE }",
   cylinder_default:  "Cylinder { }",
   cylinder_parts:    "Cylinder { radius 0.25 height 4 top FALSE }",
   cylinder_side:     "Cylinder { top FALSE bottom FALSE }",
   ifs_quad:          "IndexedFaceSet { coord Coordinate { point [ 0 0 0, 1 0 0, 1 1 0, 0 1 0 ] } coordIndex [ 0 1 2 3 -1 ] }",
   ifs_cube_crease0:  "IndexedFaceSet { coord Coordinate { point [ -1 -1 1, 1 -1 1, 1 1 1, -1 1 1, -1 -1 -1, 1 -1 -1, 1 1 -1, -1 1 -1 ] } coordIndex [ 0 1 2 3 -1, 1 5 6 2 -1, 5 4 7 6 -1, 4 0 3 7 -1, 3 2 6 7 -1, 4 5 1 0 -1 ] }",
   ifs_cube_crease1:  "IndexedFaceSet { creaseAngle 1 coord Coordinate { point [ -1 -1 1, 1 -1 1, 1 1 1, -1 1 1, -1 -1 -1, 1 -1 -1, 1 1 -1, -1 1 -1 ] } coordIndex [ 0 1 2 3 -1, 1 5 6 2 -1, 5 4 7 6 -1, 4 0 3 7 -1, 3 2 6 7 -1, 4 5 1 0 -1 ] }",
   ifs_cube_crease2:  "IndexedFaceSet { creaseAngle 2 coord Coordinate { point [ -1 -1 1, 1 -1 1, 1 1 1, -1 1 1, -1 -1 -1, 1 -1 -1, 1 1 -1, -1 1 -1 ] } coordIndex [ 0 1 2 3 -1, 1 5 6 2 -1, 5 4 7 6 -1, 4 0 3 7 -1, 3 2 6 7 -1, 4 5 1 0 -1 ] }",
   ifs_tent_crease:   "IndexedFaceSet { creaseAngle 0.8 coord Coordinate { point [ 0 0 0, 1 0.5 0, 2 0 0, 0 0 1, 1 0.5 1, 2 0 1 ] } coordIndex [ 0 3 4 1 -1, 1 4 5 2 -1 ] }",
   ifs_colors_tc:     "IndexedFaceSet { colorPerVertex FALSE color Color { color [ 1 0 0, 0 0 1 ] } texCoord TextureCoordinate { point [ 0 0, 1 0, 1 1, 0 1 ] } texCoordIndex [ 0 1 2 -1, 0 2 3 -1 ] coord Coordinate { point [ 0 0 0, 1 0 0, 1 1 0, 0 1 0 ] } coordIndex [ 0 1 2 -1, 0 2 3 -1 ] }",
   ifs_ccw_false:     "IndexedFaceSet { ccw FALSE solid FALSE coord Coordinate { point [ 0 0 0, 1 0 0, 1 1 0 ] } coordIndex [ 0 1 2 ] }",
   ifs_pentagon:      "IndexedFaceSet { coord Coordinate { point [ 0 0 0, 2 0 0, 2.5 1 0, 1 2 0, -0.5 1 0 ] } coordIndex [ 0 1 2 3 4 -1 ] }",
   ils:               "IndexedLineSet { coord Coordinate { point [ 0 0 0, 1 0 0, 1 1 0 ] } coordIndex [ 0 1 2 -1, 2 0 ] }",
   pointset:          "PointSet { coord Coordinate { point [ 0 0 0, 1 2 3 ] } color Color { color [ 1 0 0, 0 1 0 ] } }",
   elevation_grid:    "ElevationGrid { xDimension 3 zDimension 2 xSpacing 0.5 zSpacing 2 height [ 0 1 0, 0.5 0 0.25 ] creaseAngle 0.5 }",
   extrusion_default: "Extrusion { }",
   extrusion_twist:   "Extrusion { crossSection [ 1 0, 0 1, -1 0, 0 -1, 1 0 ] spine [ 0 0 0, 0 1 0, 0.5 2 0 ] scale [ 1 1, 0.5 0.5, 0.2 0.2 ] orientation [ 0 1 0 0, 0 1 0 0.5, 0 1 0 1 ] creaseAngle 0.3 }",
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

   await writeFile (resolve (process .argv [2] ?? "oracle/x_ite-geometry.json"), JSON .stringify (json, null, 1) + "\n");
   console .log (`${Object .keys (CASES) .length} cases`);
}
finally
{
   await browser .close ();
   server .close ();
}
