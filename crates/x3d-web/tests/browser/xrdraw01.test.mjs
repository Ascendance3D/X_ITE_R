// Drawing a WebXR frame into the session's layer (issue #83), in real headless Chromium on the WebGL2 backend (the page
// has no `navigator.gpu`). The session and its XRWebGLLayer are emulated (no headset): the layer owns a real WebGL
// framebuffer in the engine's own context, so the test reads back what `drawXRFrame` (web/xr_layer.mjs) put into it.
// Each view of the pose is drawn into its viewport (X_ITE 16.5.0 X3DWebXRContext.xrUpdatePose with
// `baseLayer .getViewport (view)`); the layer options are X_ITE's xrUpdateBaseLayer options.
// The X_ITE draw itself is compared with the pinned X_ITE in the native tests (crates/x3d-render/tests/xr_views.rs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && node --test tests/browser/xrdraw01.test.mjs)

import { test } from "node:test";
import assert from "node:assert/strict";
import { CASE_TIMEOUT, DESKTOP, openPage, webgpuOnly } from "./runtime02_harness.mjs";

// A red box (emissive, so the headlight does not matter) in front of a blue sky, seen from z = 10 (synthetic).
const WORLD = `#X3D V3.3 utf8
PROFILE Interchange
Background { skyColor 0 0 1 }
Viewpoint { position 0 0 10 }
Shape { appearance Appearance { material Material { diffuseColor 0 0 0 emissiveColor 1 0 0 } } geometry Box { size 6 6 6 } }
`;

// The page has no WebGPU, so the engine prints its WebGL2 fallback notices at start (the FALLBACK_NOTICES of
// runtime02_harness.mjs). Only these exact lines are left out of the log.
const FALLBACK = [
   /^warning: X_ITE_R: graphics adapter request \d+ of \d+ failed \(no WebGPU adapter: .*\); asking again\.$/,
   /^warning: X_ITE_R: WebGPU cannot start \(no WebGPU adapter: .*\); starting with WebGL2\.$/,
   /^warning: X_ITE_R: the WebGL2 adapter is below the WebGPU default limits: .*$/,
];

const noWebGPU = () => { delete Navigator .prototype .gpu; delete globalThis .WorkerNavigator ?.prototype .gpu; };

test ("an XR frame draws each view into its viewport of the XRWebGLLayer (#83)", { timeout: CASE_TIMEOUT }, async t =>
{
   const page = await openPage (DESKTOP, { init: noWebGPU, signal: t .signal });

   try
   {
      const result = await page .evaluate (async (world) =>
      {
         const { prepareLayer, drawXRFrame, layerOptions } = await import ("../../web/xr_layer.mjs");
         const element = document .querySelector ("x3d-canvas");
         const browser = element .browser;
         const engine  = browser [Symbol .for ("X_ITE_R.engine")];

         await browser .replaceWorld (await browser .createX3DFromString (world));
         await new Promise (r => requestAnimationFrame (() => requestAnimationFrame (r)));

         const canvas = element .shadowRoot .querySelector ("canvas");
         const W = 200, H = 100;

         // An emulated XRWebGLLayer: a real framebuffer of the context it is given.
         class Layer
         {
            constructor (session, gl, options)
            {
               this .options           = options;
               this .framebufferWidth  = W;
               this .framebufferHeight = H;
               this .texture           = gl .createTexture ();
               gl .bindTexture (gl .TEXTURE_2D, this .texture);
               gl .texStorage2D (gl .TEXTURE_2D, 1, gl .RGBA8, W, H);
               gl .bindTexture (gl .TEXTURE_2D, null);
               this .framebuffer = gl .createFramebuffer ();
               gl .bindFramebuffer (gl .FRAMEBUFFER, this .framebuffer);
               gl .framebufferTexture2D (gl .FRAMEBUFFER, gl .COLOR_ATTACHMENT0, gl .TEXTURE_2D, this .texture, 0);
               gl .bindFramebuffer (gl .FRAMEBUFFER, null);
            }

            getViewport (view) { return view .viewport; }
         }

         // Headless Chromium has no XR device, so its makeXRCompatible rejects (InvalidStateError): the emulated device
         // resolves it, and the test records that the engine's context was the one made compatible.
         const compatible = [ ];

         WebGL2RenderingContext .prototype .makeXRCompatible = function () { compatible .push (this); return Promise .resolve (); };

         const session = { states: [ ], updateRenderState (s) { this .states .push (s); } };
         const { gl, layer } = await prepareLayer (canvas, session, { XRWebGLLayer: Layer });

         // Perspective, 90° field of view, aspect 1, column-major as WebXR gives it.
         const near = 0.1, far = 100;
         const projectionMatrix = Float32Array .of (1, 0, 0, 0,  0, 1, 0, 0,  0, 0, (far + near) / (near - far), -1,
            0, 0, 2 * far * near / (near - far), 0);
         const identity = Float32Array .of (1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1);
         const rigid    = { matrix: identity, inverse: { matrix: identity } };
         const view     = viewport => ({ projectionMatrix, transform: rigid, viewport });
         const pose     = { transform: rigid, views: [ view ({ x: 0, y: 0, width: 100, height: 100 }),
            view ({ x: 100, y: 0, width: 100, height: 100 }) ] };

         drawXRFrame (engine, gl, layer, pose, performance .now () / 1000);

         const pixels = new Uint8Array (W * H * 4);

         gl .bindFramebuffer (gl .READ_FRAMEBUFFER, layer .framebuffer);
         gl .readPixels (0, 0, W, H, gl .RGBA, gl .UNSIGNED_BYTE, pixels);
         gl .bindFramebuffer (gl .READ_FRAMEBUFFER, null);

         const at   = (x, y) => Array .from (pixels .subarray ((y * W + x) * 4, (y * W + x) * 4 + 4));
         const half = x0 => { let red = 0, blue = 0; for (let y = 0; y < H; ++ y) for (let x = x0; x < x0 + 100; ++ x) { const [ r, g, b ] = at (x, y); if (r > 200 && g < 60 && b < 60) ++ red; else if (b > 200 && r < 60 && g < 60) ++ blue; } return { red, blue }; };
         let differ = 0;

         for (let y = 0; y < H; ++ y)
            for (let x = 0; x < 100; ++ x)
               if (at (x, y) .some ((v, i) => Math .abs (v - at (x + 100, y) [i]) > 2)) ++ differ;

         const left = half (0), right = half (100);
         const centers = [ at (50, 50), at (150, 50) ], corners = [ at (2, 2), at (197, 97) ];

         // A second frame whose right view is left out (width 0): its half shows nothing of the first frame.
         drawXRFrame (engine, gl, layer, { transform: rigid, views: [ view ({ x: 0, y: 0, width: 100, height: 100 }),
            view ({ x: 100, y: 0, width: 0, height: 100 }) ] }, performance .now () / 1000);
         gl .bindFramebuffer (gl .READ_FRAMEBUFFER, layer .framebuffer);
         gl .readPixels (0, 0, W, H, gl .RGBA, gl .UNSIGNED_BYTE, pixels);
         gl .bindFramebuffer (gl .READ_FRAMEBUFFER, null);

         return {
            backend: engine .backend,
            options: layer .options,
            expected: layerOptions (1),
            compatible: compatible .length === 1 && compatible [0] === gl && gl .canvas === canvas,
            baseLayerSet: session .states .length === 1 && session .states [0] .baseLayer === layer,
            left, right, differ, second: { left: half (0), right: half (100) },
            centers, corners,
            glError: gl .getError (),
         };
      },
      WORLD);

      assert .equal (result .backend, "webgl2");
      assert .deepEqual (result .options, result .expected);
      assert .deepEqual (result .options, { antialias: false, alpha: true, depth: false, ignoreDepthValues: true, framebufferScaleFactor: 1 });
      assert .ok (result .compatible, "the engine's WebGL2 context was not made XR-compatible");
      assert .ok (result .baseLayerSet, "the session's base layer was not set");
      assert .equal (result .glError, 0);

      // Each view drew the box in the middle of its own viewport, on the sky.
      for (const [ name, half ] of [ [ "left", result .left ], [ "right", result .right ] ])
      {
         assert .ok (half .red > 600 && half .red < 2000, `${name} view: ${half .red} red pixels`);
         assert .ok (half .blue > 7000, `${name} view: ${half .blue} sky pixels`);
      }

      for (const c of result .centers) assert .ok (c [0] > 200 && c [2] < 60, `centre ${c}`);
      for (const c of result .corners) assert .ok (c [2] > 200 && c [0] < 60, `corner ${c}`);

      // The same pose and projection in both views: the two halves are the same picture.
      assert .equal (result .differ, 0);

      // The second frame: the left view drew again; the right half, with no view, kept nothing of the first frame.
      assert .ok (result .second .left .red > 600 && result .second .left .blue > 7000, `second frame left: ${JSON .stringify (result .second .left)}`);
      assert .deepEqual (result .second .right, { red: 0, blue: 0 }, "the right half kept the first frame");
      assert .deepEqual (page .log .filter (l => !l .startsWith ("log:") && !l .startsWith ("info:") && !FALLBACK .some (r => r .test (l))), [ ]);
   }
   finally
   {
      await page .context () .close ();
   }
});

test ("xrFrame refuses the WebGPU backend (#83)", { timeout: CASE_TIMEOUT, skip: webgpuOnly () }, async t =>
{
   const page = await openPage (DESKTOP, { signal: t .signal });

   try
   {
      const error = await page .evaluate (() =>
      {
         const engine = document .querySelector ("x3d-canvas") .browser [Symbol .for ("X_ITE_R.engine")];

         if (engine .backend !== "webgpu")
            return `backend ${engine .backend}`;

         try { engine .xrFrame (0, 10, 10, new Float64Array (32), new Float64Array (36)); return "no error"; }
         catch (e) { return String (e .message ?? e); }
      });

      assert .match (error, /needs the WebGL2 backend/);
   }
   finally
   {
      await page .context () .close ();
   }
});
