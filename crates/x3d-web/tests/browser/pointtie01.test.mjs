// Point tie 01 (issue #470): a point sprite at a pixel tie falls where the page's WebGL2 rasterizer puts it, as in the
// pinned X_ITE 16.5.0. X_ITE draws its points with WebGL2 `gl_PointSize` and has no tie handling of its own, so its
// pictures depend on how the browser's WebGL2 rasterizer rounds a point position to its 1/256 pixel grid: ANGLE on Metal
// rounds a half step up, GL on NVIDIA rounds it to even.
//
// * On every backend: the renderer's snap rule (engine .pointSnapHalfUp) equals the rule this test measures itself in a
//   WebGL2 context of the same page, with points at other positions than the page module's measurement.
// * On every backend: without a usable WebGL2 measurement (no context, an exception, or a picture without the points),
//   the page starts and keeps the old rule (round to even).
// * With XR_POINT_TIE_REFERENCE: the renderer uses the rule: the snap world with each rule gives other pictures. A
//   rasterizer that does not resolve a 1/256 pixel step draws both the same (CI's SwiftShader WebGPU: the same picture,
//   PR #481), so the case runs only on the platforms with pinned pictures and is NOT TESTED elsewhere.
// * The pictures of oracle/synthetic-point-tie against the pinned X_ITE pictures, only when XR_POINT_TIE_REFERENCE names
//   the platform the pictures were recorded on: `nvidia` (oracle/render-point-tie-baseline) or `apple-m1`
//   (oracle/render-point-tie-apple-m1-baseline, which keeps only the worlds that differ from the NVIDIA pictures). Other
//   GPUs (CI's SwiftShader) have no recorded pictures; their picture cases are skipped and say so.
import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { CASE_TIMEOUT, ONLY, openPage, webgpuOnly } from "./runtime02_harness.mjs";

const REPO      = join (dirname (fileURLToPath (import.meta .url)), "../../../..");
const WORLDS    = join (REPO, "oracle/synthetic-point-tie");
const NVIDIA    = join (REPO, "oracle/render-point-tie-baseline");
const APPLE_M1  = join (REPO, "oracle/render-point-tie-apple-m1-baseline");
const PLATFORM  = process .env .XR_POINT_TIE_REFERENCE;
const SIZE      = 256;
const VIEW      = { viewport: { width: SIZE, height: SIZE }, deviceScaleFactor: 1 };
const BACKENDS  = [ "webgpu", "webgl2" ];

if (PLATFORM !== undefined && ![ "nvidia", "apple-m1" ] .includes (PLATFORM))
   throw new Error (`XR_POINT_TIE_REFERENCE must be nvidia or apple-m1, not ${PLATFORM}`);

// The reference picture of a world on PLATFORM; apple-m1 falls back to the NVIDIA picture (its set keeps only the
// worlds that differ).
function reference (world)
{
   const apple = join (APPLE_M1, `${world}.png`);

   return PLATFORM === "apple-m1" && existsSync (apple) ? apple : join (NVIDIA, `${world}.png`);
}

// An init script: the attributes are on the element before the page module upgrades it. The pictures are 256 x 256
// and drawn without multisampling, as the pinned pictures were recorded.
const attributes = ({ backend, size, fault }) =>
{
   // A fault of the page's WebGL2 measurement (OffscreenCanvas only: the x3d-canvas itself is an HTMLCanvasElement).
   if (fault)
   {
      const getContext = OffscreenCanvas .prototype .getContext;

      OffscreenCanvas .prototype .getContext = function (type, ... rest)
      {
         if (type !== "webgl2")
            return getContext .call (this, type, ... rest);

         if (fault === "null")
            return null;

         if (fault === "throw")
            throw new Error ("test: no WebGL2");

         // "black": a context whose readPixels gives no lit pixel.
         const gl         = getContext .call (this, type, ... rest);
         const readPixels = gl .readPixels .bind (gl);

         gl .readPixels = (... args) => { readPixels (... args); args [6] .fill (0); };

         return gl;
      };
   }

   new MutationObserver (() =>
   {
      const element = document .querySelector ("x3d-canvas");

      if (!element || element .hasAttribute ("data-set"))
         return;

      element .setAttribute ("data-set", "");
      element .setAttribute ("style", `display:block;width:${size}px;height:${size}px`);
      element .setAttribute ("antialiased", "false");

      // Always named: with no attribute a WebGPU failure would fall back to WebGL2 and pass as WebGPU.
      element .setAttribute ("backend", backend);
   }) .observe (document, { childList: true, subtree: true });
};

async function open (backend, fault)
{
   const page  = await openPage (VIEW, { init: `(${attributes .toString ()}) (${JSON .stringify ({ backend, size: SIZE, fault })})` });
   const error = await page .evaluate (() => window .startError);

   assert .equal (error, undefined, `X_ITE_R did not start: ${error}\n${page .log .join ("\n")}`);
   assert .equal (await page .evaluate (() => document .querySelector ("x3d-canvas") .browser [Symbol .for ("X_ITE_R.engine")] .backend), backend);

   return page;
}

// The test's own measurement of the page's WebGL2 point rounding. It is written apart from the page module's
// (x_ite_r.mjs pointSnapHalfUp) on purpose, at another pixel edge (y 2, not 4) and in the reverse column order, so that
// the two do not share a mistake: keep them different. 1-pixel points at window y 2 + d on an 8 x 8 buffer
// (2 is a pixel edge). d = 0 is a tie, 2/1024 half a 1/256 step, 4/1024 one step and 1/1024 less than half a step.
// Returns the lit row of each point, or null without WebGL2.
const measureRows = () =>
{
   const size = 8;
   const gl   = new OffscreenCanvas (size, size) .getContext ("webgl2", { antialias: false, alpha: false, depth: false, stencil: false });

   if (!gl)
      return null;

   const compile = (type, source) => { const s = gl .createShader (type); gl .shaderSource (s, source); gl .compileShader (s); return s; };
   const program = gl .createProgram ();

   gl .attachShader (program, compile (gl .VERTEX_SHADER, "#version 300 es\nin vec2 p;\nvoid main () { gl_Position = vec4 (p, 0.0, 1.0); gl_PointSize = 1.0; }\n"));
   gl .attachShader (program, compile (gl .FRAGMENT_SHADER, "#version 300 es\nprecision highp float;\nout vec4 c;\nvoid main () { c = vec4 (1.0); }\n"));
   gl .bindAttribLocation (program, 0, "p");
   gl .linkProgram (program);

   const offsets = [ 0, 2 / 1024, 4 / 1024, 1 / 1024 ];
   const ndc     = w => w / size * 2 - 1;
   // Columns 6, 4, 2 and 0, in the other order than the page module.
   const column  = i => 6 - 2 * i;

   gl .bindBuffer (gl .ARRAY_BUFFER, gl .createBuffer ());
   gl .bufferData (gl .ARRAY_BUFFER, new Float32Array (offsets .flatMap ((d, i) => [ ndc (column (i) + 0.5), ndc (2 + d) ])), gl .STATIC_DRAW);
   gl .enableVertexAttribArray (0);
   gl .vertexAttribPointer (0, 2, gl .FLOAT, false, 0, 0);
   gl .viewport (0, 0, size, size);
   gl .clearColor (0, 0, 0, 1);
   gl .clear (gl .COLOR_BUFFER_BIT);
   gl .useProgram (program);
   gl .drawArrays (gl .POINTS, 0, offsets .length);

   const pixels = new Uint8Array (size * size * 4);

   gl .readPixels (0, 0, size, size, gl .RGBA, gl .UNSIGNED_BYTE, pixels);
   gl .getExtension ("WEBGL_lose_context") ?.loseContext ();

   return offsets .map ((_, i) =>
   {
      const lit = [ ];

      for (let y = 0; y < size; ++ y)
      {
         if (pixels [(y * size + column (i)) * 4] > 128)
            lit .push (y);
      }

      return lit;
   });
};

// Loads a world and compares the renderer's picture (readPixels, rows from the top) with a PNG: the pixels where a
// channel differs by more than 8, as the native point tie tests count them.
const drawAndCompare = async ([ world, png ]) =>
{
   const browser = document .querySelector ("x3d-canvas") .browser;
   const engine  = browser [Symbol .for ("X_ITE_R.engine")];

   await browser .loadURL (new window .X3D_NS .MFString ("data:model/x3d-vrml," + encodeURIComponent (world)));

   for (let i = 0; i < 20; ++ i)
      await new Promise (r => requestAnimationFrame (r));

   const pixels = await engine .readPixels ();
   const blob   = await (await fetch (`data:image/png;base64,${png}`)) .blob ();
   const bitmap = await createImageBitmap (blob, { colorSpaceConversion: "none", premultiplyAlpha: "none" });
   const canvas = new OffscreenCanvas (bitmap .width, bitmap .height);
   const ctx    = canvas .getContext ("2d");

   ctx .drawImage (bitmap, 0, 0);

   const ref    = ctx .getImageData (0, 0, bitmap .width, bitmap .height) .data;
   const { width, height } = browser .getCanvas ();
   let differ = 0;

   if (width !== bitmap .width || height !== bitmap .height || pixels .length !== ref .length)
      return { error: `size ${width} x ${height} (${pixels .length} bytes), reference ${bitmap .width} x ${bitmap .height}` };

   for (let i = 0; i < ref .length; i += 4)
   {
      if (Math .max (Math .abs (pixels [i] - ref [i]), Math .abs (pixels [i + 1] - ref [i + 1]), Math .abs (pixels [i + 2] - ref [i + 2])) > 8)
         ++ differ;
   }

   return { differ, backend: engine .backend, snapHalfUp: engine .pointSnapHalfUp };
};

const worlds = (await readdir (WORLDS)) .filter (f => f .endsWith (".wrl")) .sort ();

describe ("point tie 01", { skip: Boolean (ONLY) }, () =>
{
   for (const backend of BACKENDS)
   {
      test (`${backend}: the point snap rule is the page's WebGL2 rule`, { skip: webgpuOnly (backend), timeout: CASE_TIMEOUT }, async () =>
      {
         const page = await open (backend);

         try
         {
            const rows = await page .evaluate (`(${measureRows .toString ()}) ()`);
            const rule = await page .evaluate (() => document .querySelector ("x3d-canvas") .browser [Symbol .for ("X_ITE_R.engine")] .pointSnapHalfUp);
            const [ tie, half, step, below ] = (rows ?? [ ]) .map (r => r .length === 1 ? r [0] : -1);
            const halfUp = rows !== null && tie !== -1 && step !== -1 && tie !== step && below === tie && half === step;

            console .log (`${backend}: WebGL2 point rows ${JSON .stringify (rows)}: half up ${halfUp}; renderer ${rule}`);
            assert .equal (rule, halfUp, `the renderer's snap rule; the page's WebGL2 rows (tie, half, step, below): ${JSON .stringify (rows)}`);
         }
         finally
         {
            await page .context () .close ();
         }
      });

      for (const fault of [ "null", "throw", "black" ])
      {
         test (`${backend}: WebGL2 measurement fault "${fault}": the page starts and rounds to even`, { skip: webgpuOnly (backend), timeout: CASE_TIMEOUT }, async () =>
         {
            const page = await open (backend, fault);

            try
            {
               const rule = await page .evaluate (() => document .querySelector ("x3d-canvas") .browser [Symbol .for ("X_ITE_R.engine")] .pointSnapHalfUp);

               assert .equal (rule, false);
            }
            finally
            {
               await page .context () .close ();
            }
         });
      }

      const ruleSkip = PLATFORM === undefined && "no XR_POINT_TIE_REFERENCE: this rasterizer may not resolve a 1/256 pixel step (NOT TESTED)";

      test (`${backend}: the renderer draws with the rule it has`, { skip: webgpuOnly (backend) || ruleSkip, timeout: CASE_TIMEOUT }, async () =>
      {
         const world = await readFile (join (WORLDS, "point-tie-snap.wrl"), "utf8");
         const page  = await open (backend);

         try
         {
            const hashes = await page .evaluate (async world =>
            {
               const browser = document .querySelector ("x3d-canvas") .browser;
               const engine  = browser [Symbol .for ("X_ITE_R.engine")];
               const frames  = async n => { for (let i = 0; i < n; ++ i) await new Promise (r => requestAnimationFrame (r)); };
               const hash    = async () =>
               {
                  const p = await engine .readPixels ();
                  let h = 0, lit = 0;

                  for (let i = 0; i < p .length; i += 4)
                  {
                     h = (h * 31 + p [i] + p [i + 1] * 3 + p [i + 2] * 7) % 1000003;
                     lit += p [i] > 128;
                  }

                  return { h, lit };
               };

               const rule = engine .pointSnapHalfUp;

               await browser .loadURL (new window .X3D_NS .MFString ("data:model/x3d-vrml," + encodeURIComponent (world)));
               await frames (20);

               engine .setPointSnapHalfUp (false);
               await frames (5);
               const even = await hash ();

               engine .setPointSnapHalfUp (true);
               await frames (5);
               const up = await hash ();

               engine .setPointSnapHalfUp (rule);

               return { even, up };
            }, world);

            assert .ok (hashes .even .lit > 0 && hashes .up .lit > 0, JSON .stringify (hashes));
            assert .notEqual (hashes .even .h, hashes .up .h, `the two rules give the same picture: ${JSON .stringify (hashes)}`);
         }
         finally
         {
            await page .context () .close ();
         }
      });

      for (const file of worlds)
      {
         const skip = PLATFORM === undefined && "no XR_POINT_TIE_REFERENCE: no pinned pictures for this GPU (NOT TESTED)";

         test (`${backend}: ${file} matches the pinned X_ITE picture`, { skip: webgpuOnly (backend) || skip, timeout: CASE_TIMEOUT }, async () =>
         {
            const world = await readFile (join (WORLDS, file), "utf8");
            const png   = (await readFile (reference (file))) .toString ("base64");
            const page  = await open (backend);

            try
            {
               const r = await page .evaluate (drawAndCompare, [ world, png ]);

               assert .equal (r .error, undefined, r .error);
               assert .equal (r .backend, backend);
               assert .equal (r .differ, 0, `${file} on ${PLATFORM}: ${r .differ} pixels differ (${JSON .stringify (r)})`);
            }
            finally
            {
               await page .context () .close ();
            }
         });
      }
   }
});
