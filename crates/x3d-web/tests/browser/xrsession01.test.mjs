// A WebXR session of the <x3d-canvas> (issue #83), in real headless Chromium on the WebGL2 backend, with an emulated
// device: `navigator.xr`, the session, its frames and an XRWebGLLayer that owns a real framebuffer of the engine's
// context. It checks X_ITE 16.5.0's session cycle (X3DRenderingContext.xrUpdateButton, X3DWebXRContext.xrAddButton,
// xrStartSession, xrFrame, xrStopSession): the button shows for a supported mode, the button starts the session, the
// frame loop runs on the session and draws each view into the layer, `XRSession` follows the session, and the loop goes
// back to the window when the session ends. No headset is involved.
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && node --test tests/browser/xrsession01.test.mjs)

import { test } from "node:test";
import assert from "node:assert/strict";
import { CASE_TIMEOUT, DESKTOP, openPage } from "./runtime02_harness.mjs";

// The page has no WebGPU, so the engine prints its WebGL2 fallback notices at start (FALLBACK_NOTICES of
// runtime02_harness.mjs). Only these exact lines are left out of the log.
const FALLBACK = [
   /^warning: X_ITE_R: graphics adapter request \d+ of \d+ failed \(no WebGPU adapter: .*\); asking again\.$/,
   /^warning: X_ITE_R: WebGPU cannot start \(no WebGPU adapter: .*\); starting with WebGL2\.$/,
   /^warning: X_ITE_R: the WebGL2 adapter is below the WebGPU default limits: .*$/,
   // The driver's notice about the test's own readPixels of the layer (NVIDIA through ANGLE), not an engine line.
   /^warning: \[\.WebGL-0x[0-9a-f]+\]GL Driver Message \(OpenGL, Performance, [A-Z_]+, High\): GPU stall due to ReadPixels$/,
];

// A red box (emissive) in front of a blue sky (synthetic).
const WORLD = `#X3D V3.3 utf8
PROFILE Interchange
Background { skyColor 0 0 1 }
Viewpoint { position 0 0 10 }
Shape { appearance Appearance { material Material { diffuseColor 0 0 0 emissiveColor 1 0 0 } } geometry Box { size 6 6 6 } }
`;

// The emulated device, installed before the page's scripts run.
function emulatedDevice ()
{
   delete Navigator .prototype .gpu;
   delete globalThis .WorkerNavigator ?.prototype .gpu;

   const W = 200, H = 100;
   const near = 0.1, far = 100;
   const projectionMatrix = Float32Array .of (1, 0, 0, 0,  0, 1, 0, 0,  0, 0, (far + near) / (near - far), -1,
      0, 0, 2 * far * near / (near - far), 0);
   const identity = Float32Array .of (1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1);
   const rigid    = { matrix: identity, inverse: { matrix: identity } };
   const views    = [ { projectionMatrix, transform: rigid, viewport: { x: 0, y: 0, width: 100, height: 100 } },
      { projectionMatrix, transform: rigid, viewport: { x: 100, y: 0, width: 100, height: 100 } } ];

   const record = globalThis .xrRecord = { requested: [ ], supported: [ ], compatible: 0, layers: [ ], session: null };

   class Session extends EventTarget
   {
      visibilityState = "visible";
      renderStates = [ ];
      callbacks = new Map ();
      nextId = 1;
      ended = false;

      async requestReferenceSpace (type) { return { type }; }
      updateRenderState (state) { this .renderStates .push (state); }
      requestAnimationFrame (callback) { const id = this .nextId ++; this .callbacks .set (id, callback); return id; }
      cancelAnimationFrame (id) { this .callbacks .delete (id); }

      async end ()
      {
         if (this .ended)
            return;

         this .ended = true;
         this .dispatchEvent (new Event ("end"));
      }

      // The device's next frame: every waiting callback gets the frame, as the user agent does.
      runFrame (time)
      {
         const frame = { session: this, getViewerPose: () => ({ transform: rigid, views }) };
         const callbacks = [ ... this .callbacks .values () ];

         this .callbacks .clear ();

         for (const callback of callbacks)
            callback (time, frame);

         return callbacks .length;
      }
   }

   Object .defineProperty (Navigator .prototype, "xr", { configurable: true, get: () => ({
      async isSessionSupported (mode) { record .supported .push (mode); return mode === "immersive-vr"; },
      async requestSession (mode) { record .requested .push (mode); return record .session = new Session (); },
   }) });

   WebGL2RenderingContext .prototype .makeXRCompatible = function () { ++ record .compatible; return Promise .resolve (); };

   globalThis .XRWebGLLayer = class
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
         this .gl = gl;
         record .layers .push (this);
      }

      getViewport (view) { return view .viewport; }

      // The test's read of the layer's framebuffer: counts of red and sky pixels in each half.
      halves ()
      {
         const gl = this .gl, pixels = new Uint8Array (W * H * 4);

         gl .bindFramebuffer (gl .READ_FRAMEBUFFER, this .framebuffer);
         gl .readPixels (0, 0, W, H, gl .RGBA, gl .UNSIGNED_BYTE, pixels);
         gl .bindFramebuffer (gl .READ_FRAMEBUFFER, null);

         return [ 0, 100 ] .map (x0 =>
         {
            let red = 0, blue = 0;

            for (let y = 0; y < H; ++ y)
            {
               for (let x = x0; x < x0 + 100; ++ x)
               {
                  const [ r, g, b ] = pixels .subarray ((y * W + x) * 4, (y * W + x) * 4 + 3);

                  if (r > 200 && g < 60 && b < 60) ++ red;
                  else if (b > 200 && r < 60 && g < 60) ++ blue;
               }
            }

            return { red, blue };
         });
      }
   };
}

test ("the XR button starts a session that draws into the layer, and its end returns to the window (#83)", { timeout: CASE_TIMEOUT }, async t =>
{
   const page = await openPage (DESKTOP, { init: emulatedDevice, signal: t .signal });

   try
   {
      const result = await page .evaluate (async (world) =>
      {
         const record  = globalThis .xrRecord;
         const element = document .querySelector ("x3d-canvas");
         const browser = element .browser;
         const engine  = browser [Symbol .for ("X_ITE_R.engine")];
         const loop    = () => browser [Symbol .for ("X_ITE_R.frameLoop")];
         const wait    = async (test, what) => { for (let i = 0; i < 200; ++ i) { if (test ()) return; await new Promise (r => setTimeout (r, 10)); } throw new Error (`timeout: ${what}`); };
         const windowFrames = () => new Promise (r => requestAnimationFrame (() => requestAnimationFrame (r)));

         await browser .replaceWorld (await browser .createX3DFromString (world));
         await windowFrames ();

         const button = () => element .shadowRoot .querySelector (".x_ite-private-xr-button");

         await wait (() => button (), "the XR button");

         const out = {
            backend: engine .backend,
            supported: [ ... record .supported ],
            buttonHidden: button () .classList .contains ("x_ite-private-hidden"),
            buttonTitle: button () .title,
            before: browser .getRenderingProperty ("XRSession"),
         };

         button () .dispatchEvent (new MouseEvent ("mouseup"));
         await wait (() => browser .getRenderingProperty ("XRSession"), "XRSession TRUE");

         const session = record .session;

         out .requested    = [ ... record .requested ];
         out .compatible   = record .compatible;
         out .layerOptions = record .layers [0] ?.options;
         out .baseLayer    = session .renderStates .some (s => s .baseLayer === record .layers [0]);
         out .stopTitle    = button () .title;

         // The window loop has stopped: no window frame advances the browser while the session runs.
         const processed = loop () .processed;

         await windowFrames ();
         out .windowFramesDuringSession = loop () .processed - processed;

         // Three device frames, 50 ms apart (more than 1 / MaximumFrameRate).
         let ran = 0;

         for (let i = 1; i <= 3; ++ i)
            ran += session .runFrame (performance .now () + i * 50);

         out .deviceCallbacks = ran;
         out .processedInSession = loop () .processed - processed;
         out .halves = record .layers [0] .halves ();
         out .planes = session .renderStates .filter (s => "depthNear" in s) .length > 0;

         await session .end ();
         await wait (() => !browser .getRenderingProperty ("XRSession"), "XRSession FALSE");

         const after = loop () .processed;

         await windowFrames ();
         await windowFrames ();

         out .windowFramesAfter = loop () .processed - after;
         out .sessionCallbacksLeft = session .callbacks .size;
         out .startTitle = button () .title;

         return out;
      },
      WORLD);

      assert .equal (result .backend, "webgl2");
      assert .deepEqual (result .supported .slice (0, 1), [ "immersive-vr" ]);
      assert .equal (result .buttonHidden, false);
      assert .equal (result .buttonTitle, "Start WebXR session.");
      assert .equal (result .before, false);

      assert .deepEqual (result .requested, [ "immersive-vr" ]);
      assert .equal (result .compatible, 1);
      assert .deepEqual (result .layerOptions, { antialias: false, alpha: true, depth: false, ignoreDepthValues: true, framebufferScaleFactor: 1 });
      assert .ok (result .baseLayer, "the session's base layer was not set");
      assert .equal (result .stopTitle, "Stop WebXR session.");

      assert .equal (result .windowFramesDuringSession, 0, "the window loop still ran during the session");
      assert .equal (result .deviceCallbacks, 3, "the browser did not wait for each device frame");
      assert .equal (result .processedInSession, 3);
      assert .ok (result .planes, "no depth planes were set");

      for (const [ i, half ] of result .halves .entries ())
      {
         assert .ok (half .red > 600 && half .red < 2000, `view ${i}: ${half .red} red pixels`);
         assert .ok (half .blue > 7000, `view ${i}: ${half .blue} sky pixels`);
      }

      assert .ok (result .windowFramesAfter > 0, "the window loop did not start again");
      assert .equal (result .sessionCallbacksLeft, 0, "a frame is still requested from the ended session");
      assert .equal (result .startTitle, "Start WebXR session.");
      assert .deepEqual (page .log .filter (l => !l .startsWith ("log:") && !l .startsWith ("info:") && !FALLBACK .some (r => r .test (l))), [ ]);
   }
   finally
   {
      await page .context () .close ();
   }
});

test ("the XR button stays hidden on the WebGPU backend (#83)", { timeout: CASE_TIMEOUT }, async t =>
{
   const page = await openPage (DESKTOP, { signal: t .signal });

   try
   {
      const result = await page .evaluate (async () =>
      {
         const element = document .querySelector ("x3d-canvas");
         const engine  = element .browser [Symbol .for ("X_ITE_R.engine")];

         await new Promise (r => setTimeout (r, 200));

         const button = element .shadowRoot .querySelector (".x_ite-private-xr-button");

         return { backend: engine .backend, visible: !! button && !button .classList .contains ("x_ite-private-hidden"),
            session: element .browser .getRenderingProperty ("XRSession") };
      });

      assert .equal (result .visible, false);
      assert .equal (result .session, false);
   }
   finally
   {
      await page .context () .close ();
   }
});
