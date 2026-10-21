/**
 * REA batch 08 shaders lane: what the pinned X_ITE does with `ComposedShader.setTransformFeedbackVaryings`
 * (only X_ITE's own shaders set it, through `browser .createShader ({ ..., transformFeedbackVaryings })`;
 * there is no field). One shader per case, 400 ms to link, then `isValid` and the console warnings are
 * written to oracle/rea-batch08-shaders-baseline/transform-feedback.json. The Rust port compares in
 * crates/x3d-viewer/tests/rea_b08_transform_feedback.rs.
 *
 *   node oracle/tools/rea_b08_transform_feedback.mjs
 *
 * Headless system Chrome on the GPU (as x_ite-shot.mjs), requests outside the local server are aborted.
 */
import { createServer } from "node:http";
import { readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { dirname, extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const ORACLE = normalize (join (dirname (fileURLToPath (import.meta.url)), ".."));
const { chromium } = createRequire (join (ORACLE, "../../../x_ite-tests/")) ("playwright");
const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json" };

const server = createServer (async (req, res) =>
{
   const path = normalize (join (ORACLE, decodeURIComponent (new URL (req .url, "http://x") .pathname)));

   try { const b = await readFile (path); res .writeHead (200, { "Content-Type": TYPES [extname (path)] ?? "model/vrml", "Cache-Control": "no-store" }); res .end (b); }
   catch { res .writeHead (404); res .end (); }
});

await new Promise (r => server .listen (0, "127.0.0.1", r));

const base    = `http://127.0.0.1:${server .address () .port}`;
const browser = await chromium .launch ({ executablePath: "/usr/bin/google-chrome", headless: true, args: [ "--headless=new", "--no-sandbox", "--use-gl=angle", "--use-angle=gl", "--enable-gpu", "--ignore-gpu-blocklist" ] });
const context = await browser .newContext ();

await context .route (url => !url .href .startsWith (base), route => route .abort ());

const page = await context .newPage ();
const logs = [ ];

page .on ("console", m => logs .push ({ type: m .type (), text: m .text () }));
await page .goto (`${base}/x_ite-pin/src/shot.html`);
await page .waitForFunction (() => window .ready, null, { timeout: 30000 });

// GLSL ES 1.00 (a `#` ends a data URL and createShader encodes the URL once more).
const VS = `
attribute vec4 x3d_Vertex;
uniform mat4 x3d_ProjectionMatrix;
uniform mat4 x3d_ModelViewMatrix;
varying vec3 vP;
varying vec4 vC;
varying float vArr [2];
void main() { vec4 p = x3d_ModelViewMatrix * x3d_Vertex; vP = p.xyz; vC = vec4 (1.0); vArr [0] = 1.0; vArr [1] = 2.0; gl_Position = x3d_ProjectionMatrix * p; }
`;
const FS = `
precision mediump float;
varying vec3 vP;
void main() { gl_FragColor = vec4 (vP, 1.0); }
`;

const cases = [
   ["none", [ ]],
   ["one", ["vP"]],
   ["two", ["vP", "vC"]],
   ["builtin", ["gl_Position"]],
   ["empty-name", [""]],
   ["missing", ["nope"]],
   ["duplicate", ["vP", "vP"]],
   ["array", ["vArr"]],
   ["array-element", ["vArr[0]"]],
   ["not-read-by-fragment", ["vC"]],
];

const results = [ ];

for (const [name, varyings] of cases)
{
   logs .length = 0;

   const valid = await page .evaluate (async ([name, varyings, VS, FS]) =>
   {
      const shader = document .getElementById ("c") .browser .createShader ({
         name,
         vertexShader: "data:x-shader/x-vertex," + VS,
         fragmentShader: "data:x-shader/x-fragment," + FS,
         transformFeedbackVaryings: varyings,
      });

      await new Promise (r => setTimeout (r, 400));

      return shader ._isValid .getValue ();
   }, [name, varyings, VS, FS]);

   results .push ({ name, varyings, valid, warnings: logs .filter (l => l .type === "warning") .map (l => l .text .trim ()) });
}

await writeFile (join (ORACLE, "rea-batch08-shaders-baseline/transform-feedback.json"), JSON .stringify ({ pinned: "x_ite-pin", vertex: VS, fragment: FS, cases: results }, null, 1) + "\n");
console .log (JSON .stringify (results));
await browser .close ();
server .close ();
