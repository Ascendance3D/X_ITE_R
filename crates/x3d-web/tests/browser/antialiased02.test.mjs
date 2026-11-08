// Antialiased 02 test of X_ITE_R's <x3d-canvas> (issue #480): a world Script's Multisampling and Antialiased options
// and the page's antialiased attribute share one BrowserOptions, as in X_ITE.
//
// Pinned X_ITE 16.5.0 (BrowserOptions.js set_Multisampling__: Multisampling = Antialiased ? samples : 0, and
// set_Antialiased__ calls set_Multisampling__) has one BrowserOptions for the page and its world. With the page's
// antialiased="false", a Script's Multisampling 4 keeps 0 samples, and a Script's Antialiased true gives the
// Multisampling option's samples again. Measured with pinned X_ITE in Chrome 156 on the NVIDIA host (same triangle,
// edge pixels as in antialiased01): A 0 samples / 0 edge pixels, B 4 / 335, C 4 / 335, D 0 / 0, E 4 / 340.
// The test looks at the pixels and the renderer's sample count, as antialiased01 does.
//
// The page's own getRenderingProperty ("Multisampling") sees a world Script's option change too (B and C: 4): the
// page takes the options a world Script set (X3DBrowser.#syncWorldOptions), so page and world are one BrowserOptions.

import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { CASE_TIMEOUT, DESKTOP, ONLY, openPage, webgpuOnly } from "./runtime02_harness.mjs";

const WORLD = `#X3D V4.0 utf8
PROFILE Immersive
COMPONENT Scripting:1
Background { skyColor 1 1 1 }
Viewpoint { position 0 0 4 }
Transform { rotation 0 0 1 0.3 children Shape {
   appearance Appearance { material Material { emissiveColor 0 0 0 diffuseColor 0 0 0 } }
   geometry IndexedFaceSet { coord Coordinate { point [ -1 -1 0, 1 -0.8 0, 0.2 1 0 ] } coordIndex [ 0 1 2 -1 ] solid FALSE } } }
`;

// WORLD with a Script that runs `body` in initialize.
const scripted = body => WORLD + `Script { url "ecmascript: function initialize () { ${body} }" }\n`;

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

// Loads the world, lets frames pass, runs `after` in the page, lets frames pass and reads the picture back.
async function draw (page, world, after = () => { })
{
   return page .evaluate (async ([ text, afterText ]) =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const engine  = browser [Symbol .for ("X_ITE_R.engine")];
      const frames  = async n => { for (let i = 0; i < n; ++ i) await new Promise (r => requestAnimationFrame (r)); };

      await browser .loadURL (new window .X3D_NS .MFString ("data:model/x3d-vrml," + encodeURIComponent (text)));
      await frames (20);
      await (0, eval) (afterText) (browser);
      await frames (20);

      const pixels = await engine .readPixels ();
      let edge = 0, dark = 0, white = 0;

      for (let i = 0; i < pixels .length; i += 4)
      {
         const v = pixels [i];

         if (v === 0) ++ dark; else if (v === 255) ++ white; else ++ edge;
      }

      return { edge, dark, white, multisampling: browser .getRenderingProperty ("Multisampling"), sampleCount: engine .sampleCount, maxSamples: engine .maxSamples, backend: engine .backend };
   }, [ world, `(${after .toString ()})` ]);
}

const BACKENDS = [ "webgpu", "webgl2" ];

describe ("antialiased 02", { skip: Boolean (ONLY) }, () =>
{
   for (const backend of BACKENDS)
   {
      // [ name, page antialiased attribute, world, after the load, X_ITE's samples, page report gap ]
      const cases =
      [
         [ "A: antialiased=\"false\", a Script sets Multisampling 4: no multisampling", "false",
            scripted ("Browser .setBrowserOption ('Multisampling', 4);"), undefined, 0 ],
         [ "B: antialiased=\"false\", a Script sets Antialiased true: 4 samples", "false",
            scripted ("Browser .setBrowserOption ('Antialiased', true);"), undefined, 4, true ],
         [ "C: antialiased=\"false\", a Script sets Multisampling 4, then Antialiased true: 4 samples", "false",
            scripted ("Browser .setBrowserOption ('Multisampling', 4); Browser .setBrowserOption ('Antialiased', true);"), undefined, 4, true ],
         [ "D: a Script sets Multisampling 4, then the page sets antialiased=\"false\": no multisampling", "true",
            scripted ("Browser .setBrowserOption ('Multisampling', 4);"), b => b .getElement () .setAttribute ("antialiased", "false"), 0 ],
         [ "E: antialiased=\"false\", a Script sets Multisampling 4, then the page sets Antialiased true: 4 samples", "false",
            scripted ("Browser .setBrowserOption ('Multisampling', 4);"), b => b .setBrowserOption ("Antialiased", true), 4 ],
      ];

      for (const [ name, antialiased, world, after, samples, gap ] of cases)
      {
         if (gap)
         {
            test (`${backend}: ${name}: the page reports the Script's Multisampling`,
               { timeout: CASE_TIMEOUT }, async () =>
            {
               const page = await open (backend, { antialiased, scripts: "true" });

               try
               {
                  const r = await draw (page, world, after);

                  assert .equal (r .multisampling, Math .min (samples, r .maxSamples), JSON .stringify (r));
               }
               finally
               {
                  await page .context () .close ();
               }
            });
         }

         // This case asserts the engine's backend and its sample count on WebGPU.
         test (`${backend}: ${name}`, { skip: webgpuOnly (backend), timeout: CASE_TIMEOUT }, async () =>
         {
            const page = await open (backend, { antialiased, scripts: "true" });

            try
            {
               const r = await draw (page, world, after);

               assert .equal (r .backend, backend);
               const expected = Math .min (samples, r .maxSamples);

               if (!gap)
                  assert .equal (r .multisampling, expected, `the reported Multisampling: ${JSON .stringify (r)}`);

               assert .equal (r .sampleCount, Math .max (expected, 1), `the sample count of the renderer: ${JSON .stringify (r)}`);
               assert .ok (r .dark > 100 && r .white > 100, JSON .stringify (r));

               if (expected <= 1)
                  assert .equal (r .edge, 0, `a world drawn with one sample has no edge pixels: ${JSON .stringify (r)}`);
               else
                  assert .ok (r .edge > 100, `a world drawn with ${expected} samples has edge pixels: ${JSON .stringify (r)}`);
            }
            finally
            {
               await page .context () .close ();
            }
         });
      }
   }
});
