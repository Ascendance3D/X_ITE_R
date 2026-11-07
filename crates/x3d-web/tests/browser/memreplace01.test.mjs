// Memory over world replacement (issue #462 section D), in real headless Chromium on WebGPU or, with XR_FORCE_WEBGL2=1,
// WebGL2. The page replaces a synthetic world again and again; after the first cycles the engine must keep exactly what
// one world needs: the same live WASM heap bytes and allocations, one world, no retired world, and the same renderer
// caches (geometries, textures, bind groups, pipelines, vertex buffers). The counts come from the engine itself
// (`resourceStats`), not from process RSS: the test browser enables the DevTools Network domain, which keeps response
// bodies in the renderer process, so RSS grows in every test page and does not show what the engine keeps.
// Not compared with X_ITE: X_ITE has no such counters.
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && node --test tests/browser/memreplace01.test.mjs)

import { test } from "node:test";
import assert from "node:assert/strict";
import { CASE_TIMEOUT, DESKTOP, openPage } from "./runtime02_harness.mjs";

// 200 shapes with their own geometry, material and an ImageTexture-free appearance, a Text node, and a Script-free
// TimeSensor route (newly authored, synthetic).
function world (seed)
{
   let s = "#X3D V4.0 utf8\nPROFILE Interchange\nCOMPONENT Text:1\nViewpoint { position 10 2 40 }\nTransform { children [\n";

   for (let i = 0; i < 200; ++ i)
   {
      const geometry = [ "Box { size 0.4 0.4 0.4 }", "Sphere { radius 0.25 }", "Cone { height 0.5 bottomRadius 0.2 }", "Cylinder { height 0.5 radius 0.2 }" ] [i % 4];

      s += `Transform { translation ${i % 20} ${Math .floor (i / 20)} 0 children Shape { appearance Appearance { material Material { diffuseColor ${(i % 7) / 7} ${(i % 5) / 5} ${seed % 2} } } geometry ${geometry} } }\n`;
   }

   s += "Shape { geometry Text { string \"memreplace\" } }\n";
   s += "DEF T Transform { children Shape { geometry Box { } } }\nDEF TS TimeSensor { loop TRUE cycleInterval 2 }\nDEF PI PositionInterpolator { key [ 0 1 ] keyValue [ 0 0 0 1 1 1 ] }\n";
   s += "] }\nROUTE TS.fraction_changed TO PI.set_fraction\nROUTE PI.value_changed TO T.set_translation\n";
   return s;
}

// The engine's counters that depend on the world kept, not on the frame number or on buffer capacities.
const KEPT = [ "heapLive", "heapAllocs", "worlds", "loads", "retired", "geometries", "geometryBytes", "sprites", "spriteBytes",
   "textures", "slotGroups", "styleGroups", "generated", "rendered", "thick", "pipelines", "frameBuffers", "volume",
   "splats", "custom" ];

test ("world replacement keeps no old world (#462 D)", { timeout: CASE_TIMEOUT }, async t =>
{
   const page = await openPage (DESKTOP, { signal: t .signal });

   try
   {
      const cdp = await page .context () .newCDPSession (page);

      // `n` replace cycles with the same two worlds in turn, 5 frames each; then a garbage collection, so the page's
      // FinalizationRegistry releases what the page dropped.
      const cycles = async (from, n) =>
      {
         await page .evaluate (async ({ texts, from, n }) =>
         {
            const browser = document .querySelector ("x3d-canvas") .browser;
            const frames = () => new Promise (r => { let i = 0; const f = () => (++ i >= 5 ? r () : requestAnimationFrame (f)); requestAnimationFrame (f); });

            for (let i = from; i < from + n; ++ i)
            {
               await browser .replaceWorld (await browser .createX3DFromString (texts [i % 2]));
               await frames ();
            }
         },
         { texts: [ world (0), world (1) ], from, n });

         await cdp .send ("HeapProfiler.collectGarbage");
         await page .waitForTimeout (200);
      };

      const stats = () => page .evaluate (() => Object .fromEntries (Object .entries (document .querySelector ("x3d-canvas") .browser [Symbol .for ("X_ITE_R.engine")] .resourceStats)));
      const kept = s => Object .fromEntries (KEPT .map (k => [ k, s [k] ]));

      // The first cycles fill the caches that live as long as the engine (pipelines, glyphs, shader modules).
      await cycles (0, 6);
      const first = await stats ();

      for (const k of KEPT)
         assert .equal (typeof first [k], "number", `resourceStats.${k} is missing`);

      assert .equal (first .worlds, 1);
      assert .equal (first .retired, 0);
      assert .ok (first .geometries > 0, "the world was not drawn");

      await cycles (6, 30);
      const last = await stats ();

      // Same parity of cycles: the same world is current, so every count is exactly the same.
      assert .deepEqual (kept (last), kept (first));
      assert .deepEqual (page .log .filter (l => !l .startsWith ("log:") && !l .startsWith ("info:")), [ ]);
   }
   finally
   {
      await page .context () .close ();
   }
});
