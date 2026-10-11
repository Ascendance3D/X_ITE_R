/**
 * X8 fog oracle (headless system Chrome, page x_ite-pin/src/shot.html):
 *  - `nodes`: what X_ITE's Fog nodes compute from their fields (getFogType, visibilityStart,
 *    visibilityRange: X3DFogObject.set_fogType__ / set_visibilityRange__);
 *  - `interpolant`: the fog factor that the pinned shader chunk assets/shaders/webgl2/common/
 *    Fog2.glsl.js (getFogInterpolant, the real source, compiled in WebGL2 on the GPU) returns for a
 *    grid of fog types, ranges, matrices and vertices.
 *
 *   node oracle/x_ite-fog.mjs oracle/x_ite-fog.json
 */
import { createServer } from "node:http";
import { readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
const { chromium } = createRequire (join (ROOT, "../../../x_ite-tests/")) ("playwright");

const SHADERS = join (ROOT, "x_ite-pin/src/assets/shaders");
const types   = (await import (pathToFileURL (join (SHADERS, "Types.glsl.js")))) .default ();
const fog2    = (await import (pathToFileURL (join (SHADERS, "webgl2/common/Fog2.glsl.js")))) .default ();

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json" };

const NODE_CASES = [ ];

for (const fogType of [ "LINEAR", "EXPONENTIAL", "FOO", "exponential", "" ])
   for (const [ start, range ] of [ [ 0, 0 ], [ 0, 5 ], [ 0, -3 ], [ 2, 10 ], [ 20, 10 ], [ -1, 10 ], [ 3, 0 ], [ 5, 5 ] ])
      NODE_CASES .push ({ fogType, start, range });

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

   const nodes = await page .evaluate (async cases =>
   {
      const browser = document .getElementById ("c") .browser;
      const out     = [ ];

      for (const c of cases)
      {
         const scene = await browser .createX3DFromString (`#VRML V2.0 utf8\nDEF F Fog { fogType ${JSON .stringify (c .fogType)} visibilityStart ${c .start} visibilityRange ${c .range} }`);
         const node  = scene .getNamedNode ("F") .getValue ();

         out .push ({ ... c, jsFogType: node .getFogType (), jsStart: node .visibilityStart, jsRange: node .visibilityRange });
      }

      return out;
   },
   NODE_CASES);

   const interpolant = await page .evaluate (({ types, fog2 }) =>
   {
      const canvas = new OffscreenCanvas (1, 1);
      const gl     = canvas .getContext ("webgl2");

      if (!gl .getExtension ("EXT_color_buffer_float"))
         throw new Error ("EXT_color_buffer_float missing");

      function program (fogDefine)
      {
         const vs = `#version 300 es
            uniform vec3 testVertex;
            out vec3 vertex;
            void main () { vertex = testVertex; gl_Position = vec4 (0.0, 0.0, 0.0, 1.0); gl_PointSize = 1.0; }`;

         const fs = `#version 300 es
            precision highp float;
            #define X3D_FRAGMENT_SHADER
            #define X3D_FOG
            #define ${fogDefine}
            ${types}
            in vec3 vertex;
            ${fog2}
            out vec4 result;
            void main () { result = vec4 (getFogInterpolant (), 0.0, 0.0, 1.0); }`;

         const p = gl .createProgram ();

         for (const [ type, src ] of [ [ gl .VERTEX_SHADER, vs ], [ gl .FRAGMENT_SHADER, fs ] ])
         {
            const s = gl .createShader (type);
            gl .shaderSource (s, src);
            gl .compileShader (s);

            if (!gl .getShaderParameter (s, gl .COMPILE_STATUS))
               throw new Error (gl .getShaderInfoLog (s));

            gl .attachShader (p, s);
         }

         gl .linkProgram (p);

         if (!gl .getProgramParameter (p, gl .LINK_STATUS))
            throw new Error (gl .getProgramInfoLog (p));

         return p;
      }

      const programs = { 1: program ("X3D_FOG_LINEAR"), 2: program ("X3D_FOG_EXPONENTIAL") };

      const tex = gl .createTexture ();
      gl .bindTexture (gl .TEXTURE_2D, tex);
      gl .texImage2D (gl .TEXTURE_2D, 0, gl .RGBA32F, 1, 1, 0, gl .RGBA, gl .FLOAT, null);

      const fbo = gl .createFramebuffer ();
      gl .bindFramebuffer (gl .FRAMEBUFFER, fbo);
      gl .framebufferTexture2D (gl .FRAMEBUFFER, gl .COLOR_ATTACHMENT0, gl .TEXTURE_2D, tex, 0);
      gl .viewport (0, 0, 1, 1);

      const matrices = [
         [ 1, 0, 0, 0, 1, 0, 0, 0, 1 ],
         [ 0.5, 0, 0, 0, 0.5, 0, 0, 0, 0.5 ],
         [ 2, 0, 0, 0, 1, 0, 0, 0, 0.25 ],
         [ 0.36, 0.48, 0, -0.48, 0.36, 0, 0, 0, 1 ],
      ];
      const vertices = [ [ 0, 0, 0 ], [ 1, 0, 0 ], [ 0, 2.5, 0 ], [ 3, 4, 0 ], [ 0, 0, -7 ], [ -12, 5, 3 ], [ 40, 0, 0 ], [ 0.001, 0.002, 0.003 ] ];
      const ranges   = [ [ 0, 10 ], [ 0, 3 ], [ 2, 10 ], [ 5, 5.5 ], [ 0.5, 100 ], [ 0, 0.5 ] ];
      const out      = [ ];

      for (const type of [ 1, 2 ])
      {
         const p = programs [type];

         gl .useProgram (p);

         for (const [ start, range ] of ranges)
         {
            for (const m of matrices)
            {
               for (const v of vertices)
               {
                  gl .uniform3f (gl .getUniformLocation (p, "testVertex"), ... v);
                  gl .uniform1f (gl .getUniformLocation (p, "x3d_Fog.visibilityStart"), start);
                  gl .uniform1f (gl .getUniformLocation (p, "x3d_Fog.visibilityRange"), range);
                  gl .uniformMatrix3fv (gl .getUniformLocation (p, "x3d_Fog.matrix"), false, m);
                  gl .uniform3f (gl .getUniformLocation (p, "x3d_Fog.color"), 1, 1, 1);
                  gl .uniform1i (gl .getUniformLocation (p, "x3d_Fog.type"), type);
                  gl .clearColor (0, 0, 0, 0);
                  gl .clear (gl .COLOR_BUFFER_BIT);
                  gl .drawArrays (gl .POINTS, 0, 1);

                  const px = new Float32Array (4);
                  gl .readPixels (0, 0, 1, 1, gl .RGBA, gl .FLOAT, px);
                  out .push ({ type, start, range, matrix: m, vertex: v, value: px [0] });
               }
            }
         }
      }

      return out;
   },
   { types, fog2 });

   const json = { pinned: "oracle/x_ite-pin (X_ITE 16.5.0)", browser: browser .version (), nodes, interpolant };

   await writeFile (resolve (process .argv [2] ?? "oracle/x_ite-fog.json"), JSON .stringify (json) + "\n");
   console .log (`${nodes .length} fog nodes, ${interpolant .length} interpolant samples`);
}
finally
{
   await browser .close ();
   server .close ();
}
