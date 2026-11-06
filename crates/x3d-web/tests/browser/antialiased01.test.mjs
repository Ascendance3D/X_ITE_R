// Antialiased 01 test of X_ITE_R's <x3d-canvas> that X_ITE has no reference for (issue #471): the page's Antialiased,
// Multisampling and Shading options stay in force when a world is replaced.
//
// In X_ITE, BrowserOptions belong to the browser, not to a world: loadURL and replaceWorld do not reset them
// (BrowserOptions.js, set_Multisampling__: Multisampling = Antialiased ? clamp (Multisampling, 0, MaxSamples) : 0).
// The test looks at the pixels, not at the reported property. A black triangle on white is drawn and the edge pixels
// (red channel neither 0 nor 255) are counted: 0 edge pixels is one sample, 4-sample coverage gives the values 64, 127
// and 191. Shading FLAT of a lit sphere gives another picture than GOURAUD (WIREFRAME is not used: the renderer draws the same
// picture for it on the test device).

import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { CASE_TIMEOUT, DESKTOP, ONLY, openPage } from "./runtime02_harness.mjs";

const WORLD = `#X3D V4.0 utf8
PROFILE Interchange
Background { skyColor 1 1 1 }
Viewpoint { position 0 0 4 }
Transform { rotation 0 0 1 0.3 children Shape {
   appearance Appearance { material Material { emissiveColor 0 0 0 diffuseColor 0 0 0 } }
   geometry IndexedFaceSet { coord Coordinate { point [ -1 -1 0, 1 -0.8 0, 0.2 1 0 ] } coordIndex [ 0 1 2 -1 ] solid FALSE } } }
`;

const SPHERE = `#X3D V4.0 utf8
PROFILE Interchange
Background { skyColor 1 1 1 }
Viewpoint { position 0 0 4 }
DirectionalLight { direction -1 -1 -1 }
Shape { appearance Appearance { material Material { diffuseColor 1 0 0 specularColor 1 1 1 shininess 0.5 } } geometry Sphere { } }
`;

// An init script: the attributes are on the element before the page module upgrades it.
const attributes = ({ backend, extra }) =>
{
   new MutationObserver (() =>
   {
      const element = document .querySelector ("x3d-canvas");

      if (!element || element .hasAttribute ("data-set"))
         return;

      element .setAttribute ("data-set", "");

      if (backend !== "webgpu")
         element .setAttribute ("backend", backend);

      for (const [ name, value ] of Object .entries (extra))
         element .setAttribute (name, value);
   }) .observe (document, { childList: true, subtree: true });
};

async function open (backend, extra = { })
{
   const page = await openPage (DESKTOP, { init: `(${attributes .toString ()}) (${JSON .stringify ({ backend, extra })})` });

   const error = await page .evaluate (() => window .startError);

   assert .equal (error, undefined, `X_ITE_R did not start: ${error}\n${page .log .join ("\n")}`);

   return page;
}

// Loads the world, lets frames pass and reads the picture back. `before` runs in the page before the load.
async function draw (page, { before = () => { }, again = false, world = WORLD } = { })
{
   return page .evaluate (async ([ text, beforeText, again ]) =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const engine  = browser [Symbol .for ("X_ITE_R.engine")];
      const frames  = async n => { for (let i = 0; i < n; ++ i) await new Promise (r => requestAnimationFrame (r)); };
      const url     = "data:model/x3d-vrml," + encodeURIComponent (text);

      await (0, eval) (beforeText) (browser);
      await browser .loadURL (new window .X3D_NS .MFString (url));

      if (again)
         await browser .loadURL (new window .X3D_NS .MFString (url));

      await frames (20);

      const pixels = await engine .readPixels ();
      let edge = 0, dark = 0, white = 0, hash = 0;

      for (let i = 0; i < pixels .length; i += 4)
      {
         const v = pixels [i];

         hash = (hash * 31 + v + pixels [i + 1] * 3 + pixels [i + 2] * 7) % 1000003;

         if (v === 0) ++ dark; else if (v === 255) ++ white; else ++ edge;
      }

      return { edge, dark, white, hash, multisampling: browser .getRenderingProperty ("Multisampling"), backend: engine .backend };
   }, [ world, `(${before .toString ()})`, again ]);
}

const BACKENDS = [ "webgpu", "webgl2" ];

// The timeout is per case, not for the suite: on SwiftShader one case takes about 22 s (hosted CI), so 16 cases need
// more than one CASE_TIMEOUT.
describe ("antialiased 01", { skip: Boolean (ONLY) }, () =>
{
   for (const backend of BACKENDS)
   {
      const cases =
      [
         [ "antialiased=\"false\" before ready: no multisampling", { antialiased: "false" }, undefined, false, 0 ],
         [ "antialiased=\"false\" and a second loadURL", { antialiased: "false" }, undefined, true, 0 ],
         [ "antialiased=\"true\"", { antialiased: "true" }, undefined, false, 4 ],
         [ "no attribute (default 4 samples)", { }, undefined, false, 4 ],
         [ "antialiased=\"false\" set after ready", { }, b => b .getElement () .setAttribute ("antialiased", "false"), false, 0 ],
         [ "Antialiased option set to false after a first world", { }, async b =>
            {
               await b .replaceWorld (await b .createX3DFromString ("#X3D V4.0 utf8\nPROFILE Interchange\nShape { geometry Box { } }\n"));
               b .setBrowserOption ("Antialiased", false);
            }, true, 0 ],
         [ "Multisampling option 0", { }, b => b .setBrowserOption ("Multisampling", 0), false, 0 ],
      ];

      for (const [ name, extra, before, again, samples ] of cases)
      {
         test (`${backend}: ${name}`, { timeout: CASE_TIMEOUT }, async () =>
         {
            const page = await open (backend, extra);

            try
            {
               const r = await draw (page, { before, again });

               assert .equal (r .backend, backend);
               assert .equal (r .multisampling, samples, "the reported Multisampling");
               assert .ok (r .dark > 100 && r .white > 100, JSON .stringify (r));

               if (samples === 0)
                  assert .equal (r .edge, 0, `a world drawn with one sample has no edge pixels: ${JSON .stringify (r)}`);
               else
                  assert .ok (r .edge > 100, `a world drawn with ${samples} samples has edge pixels: ${JSON .stringify (r)}`);
            }
            finally
            {
               await page .context () .close ();
            }
         });
      }

      test (`${backend}: Shading FLAT set before the world loads stays in force`, { timeout: CASE_TIMEOUT }, async () =>
      {
         const page = await open (backend, { antialiased: "false" });

         try
         {
            const flat    = await draw (page, { world: SPHERE, before: b => b .setBrowserOption ("Shading", "FLAT") });
            const gouraud = await draw (page, { world: SPHERE, before: b => b .setBrowserOption ("Shading", "GOURAUD") });
            const again   = await draw (page, { world: SPHERE, before: b => b .setBrowserOption ("Shading", "FLAT"), again: true });

            assert .notEqual (flat .hash, gouraud .hash, "FLAT and GOURAUD give different pictures");
            assert .equal (again .hash, flat .hash, "FLAT again gives the same picture as the first FLAT");
         }
         finally
         {
            await page .context () .close ();
         }
      });
   }
});
