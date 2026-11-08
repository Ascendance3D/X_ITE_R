// The GaussianSplats sort 01 cases (issue #69): X_ITE sorts the splat indices in a worker, asynchronously, and draws with the
// last sorted order until the worker answers (GaussianSplatsShape.sortIndices: one sort in flight, a view change during it waits
// for the answer); X_ITE_R sorts in the frame that draws. The cases compare what a page can see once the sorts are done, in
// both engines: the picture before and after a view change that flips the back-to-front order of two overlapping splats, and
// after a burst of view changes in consecutive frames (the last view must win). oracle/x_ite-web-runtime-02.mjs
// (--suite splatsort01) records the pinned X_ITE; splatsort01.test.mjs compares X_ITE_R (runtime02_harness.mjs).
//
// What no case records is the frames between a view change and the worker's answer: they depend on the wall clock (the worker's
// run time) and are not reproducible. web/API.md, "GaussianSplats", states the decision.

import assert from "node:assert/strict";
import { DESKTOP } from "./runtime02_case.mjs";

// Two large translucent splats that overlap at the middle of the picture, a red one near the viewer and a blue one far from it,
// inside a Transform that the cases turn: after a half turn about Y the red one is the far one, so the blend order flips.
const WORLD = `#X3D V4.1 utf8
PROFILE Interchange
COMPONENT GaussianSplats:1
Viewpoint { position 0 0 6 }
Background { skyColor 0.2 0.2 0.28 }
DEF T Transform
{
  children GaussianSplats
  {
    positions [ 0 0 0.6, 0.15 0 -0.6 ]
    orientations [ 0 0 0 1, 0 0 0 1 ]
    scales [ 0.7 0.7 0.1, 0.7 0.7 0.1 ]
    opacities [ 0.8, 0.8 ]
    sphericalHarmonicsDegree0Coef0 [ 2 -1.5 -1.5, -1.5 -1.5 2 ]
  }
}
`;

async function scenario (page, env, name)
{
   const out = { };

   // The calls that show X_ITE's asynchronous sort in the frames after a view change: the messages to and from its sort worker
   // and its instanced draws of the splat quads (WebGL2 drawArraysInstanced, or WebGPU pass.draw when the page runs X_ITE_R on
   // WebGPU: the same event on every backend, a draw of 6 vertices and n instances). X_ITE_R has no worker, so it records no
   // message; it draws the splats in every frame, so its trace is the first eight draws, WebGPU or WebGL2 alike.
   await page .evaluate (() =>
   {
      window .trace = [ ];

      const Base = window .Worker;

      window .Worker = class extends Base
      {
         constructor (... args)
         {
            super (... args);
            this .addEventListener ("message", event => window .trace .push (`recv ${event .data .type}`));

            const post = this .postMessage .bind (this);

            this .postMessage = (message, ... rest) => { window .trace .push (`post ${message .type}`); return post (message, ... rest); };
         }
      };

      const draw = WebGL2RenderingContext .prototype .drawArraysInstanced;

      WebGL2RenderingContext .prototype .drawArraysInstanced = function (mode, first, count, n)
      {
         if (count === 6)
            window .trace .push (`draw ${n}`);

         return draw .call (this, mode, first, count, n);
      };

      // On WebGPU the trace needs this patch: a page that has WebGPU but cannot be patched fails the case (below), it never
      // compares an empty trace.
      window .traceGpuPatched = false;

      if (window .GPURenderPassEncoder ?.prototype ?.draw)
      {
         window .traceGpuPatched = true;

         const gpuDraw = GPURenderPassEncoder .prototype .draw;

         GPURenderPassEncoder .prototype .draw = function (vertexCount, instanceCount, ... rest)
         {
            if (vertexCount === 6)
               window .trace .push (`draw ${instanceCount ?? 1}`);

            return gpuDraw .call (this, vertexCount, instanceCount, ... rest);
         };
      }
   });

   const patched = await page .evaluate (() => ({ backend: document .querySelector ("x3d-canvas") .browser ?.[Symbol .for ("X_ITE_R.engine")] ?.backend, patched: window .traceGpuPatched }));

   assert .ok (patched .backend !== "webgpu" || patched .patched, "splatsort01: X_ITE_R runs on WebGPU but GPURenderPassEncoder.prototype.draw cannot be patched, so the trace would be empty");

   await page .evaluate (async text =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;

      window .frames = async n => { for (let i = 0; i < n; ++ i) await new Promise (r => requestAnimationFrame (() => r ())); };
      await browser .replaceWorld (await browser .createX3DFromString (text));
      await window .frames (40);
   },
   WORLD);

   const turn = angle => page .evaluate (angle =>
   {
      const T = document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("T");

      T .rotation = new window .X3D_NS .SFRotation (0, 1, 0, angle);
   },
   angle);
   const settle = () => page .evaluate (() => window .frames (60));

   out .before = await env .picture (page);
   await page .evaluate (() => { window .trace .length = 0; });

   switch (name)
   {
      // One change: the order of the two splats flips.
      case "flip":
         await turn (Math .PI);
         await settle ();
         // X_ITE posts the sort, draws the frame with the old order, and draws again when the worker answers.
         out .trace = await page .evaluate (() => window .trace .slice (0, 8));
         out .after = await env .picture (page);
         break;
      // Ten changes in ten consecutive frames, the last of them the half turn: the picture is the one of the last view.
      case "burst":
         for (let i = 1; i <= 10; ++ i)
         {
            await turn (Math .PI * i / 10);
            await page .evaluate (() => window .frames (1));
         }

         await settle ();
         out .after = await env .picture (page);
         break;
      // There and back: the first order again.
      case "back":
         await turn (Math .PI);
         await settle ();
         await turn (0);
         await settle ();
         out .after = await env .picture (page);
         break;
   }

   return out;
}

// The frames of a case are the cost: 100 (flip), 110 (burst) and 160 (back), each drawn by SwiftShader on CI (0.5 s to 1.2 s a
// frame on the hosted runner, 2.4 times apart between runners: back took 77 s to 180 s in 9 passing runs and timed out at the
// default 180 s in a 10th, run 41408590062). The cap is 2.5 times the slowest run, as for the viewpoints cases; a hung case still fails
// with its name, and no tolerance or frame count changes.
export const CASE_GROUPS = {
   splatSort: { context: DESKTOP, timeout: 450_000, cases: Object .fromEntries ([ "flip", "burst", "back" ] .map (name => [ name, (page, env) => scenario (page, env, name) ])) },
};
