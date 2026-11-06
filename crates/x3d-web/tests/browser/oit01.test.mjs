// OIT 01 test of X_ITE_R's <x3d-canvas> (issue #479): the page's OrderIndependentTransparency option reaches the
// renderer, before a world loads, on the live world and after loadURL, and the last one set is in force, the page's or
// a world Script's.
//
// Pinned X_ITE 16.5.0 has one BrowserOptions for the page and its world: set_OrderIndependentTransparency__ calls
// browser .reshape (BrowserOptions.js:369) and X3DRenderObject has an interest on the field (X3DRenderObject.js:85).
// The test compares picture hashes: a world Script's OrderIndependentTransparency true already reaches the renderer
// (RenderingChanges::apply), so its picture is the "OIT on" picture, and the page's option must give the same.

import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { CASE_TIMEOUT, DESKTOP, ONLY, openPage } from "./runtime02_harness.mjs";

// Two overlapping transparent quads: sorted blending and weighted blended OIT mix them differently.
const WORLD = `#X3D V4.0 utf8
PROFILE Immersive
COMPONENT Scripting:1
Background { skyColor 1 1 1 }
Viewpoint { position 0 0 4 }
Transform { translation -0.3 0 0 children Shape {
   appearance Appearance { material Material { diffuseColor 0 0 0 emissiveColor 1 0 0 transparency 0.3 } }
   geometry Box { size 1.2 1.2 0.01 } } }
Transform { translation 0.3 0 0.5 children Shape {
   appearance Appearance { material Material { diffuseColor 0 0 0 emissiveColor 0 0 1 transparency 0.3 } }
   geometry Box { size 1.2 1.2 0.01 } } }
`;

// WORLD with a Script that sets OrderIndependentTransparency in initialize.
const scripted = on => WORLD + `Script { url "ecmascript: function initialize () { Browser .setBrowserOption ('OrderIndependentTransparency', ${on}); }" }\n`;

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

// Runs `before` in the page, loads `world` (null: the world stays), lets frames pass and returns the picture's hash.
async function draw (page, { before = () => { }, world = WORLD } = { })
{
   return page .evaluate (async ([ text, beforeText ]) =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const engine  = browser [Symbol .for ("X_ITE_R.engine")];
      const frames  = async n => { for (let i = 0; i < n; ++ i) await new Promise (r => requestAnimationFrame (r)); };

      await (0, eval) (beforeText) (browser);

      if (text !== null)
         await browser .loadURL (new window .X3D_NS .MFString ("data:model/x3d-vrml," + encodeURIComponent (text)));

      await frames (20);

      const pixels = await engine .readPixels ();
      let hash = 0;

      for (let i = 0; i < pixels .length; i += 4)
         hash = (hash * 31 + pixels [i] + pixels [i + 1] * 3 + pixels [i + 2] * 7) % 1000003;

      return { hash, option: browser .getBrowserOption ("OrderIndependentTransparency"), backend: engine .backend };
   }, [ world, `(${before .toString ()})` ]);
}

const BACKENDS = [ "webgpu", "webgl2" ];

describe ("oit 01", { skip: Boolean (ONLY) }, () =>
{
   for (const backend of BACKENDS)
   {
      // The "OIT off" and "OIT on" pictures: the page's option false, and a world Script's option true.
      async function references (page)
      {
         const off = await draw (page, { world: scripted (false) });
         const on  = await draw (page, { world: scripted (true) });

         assert .equal (off .backend, backend);
         assert .notEqual (on .hash, off .hash, "OIT on and off give different pictures");

         return { off, on };
      }

      test (`${backend}: the attribute set before the world loads`, { timeout: CASE_TIMEOUT }, async () =>
      {
         const page = await open (backend, { orderIndependentTransparency: "true", scripts: "true" });

         try
         {
            const first = await draw (page);
            const { off, on } = await references (page);

            assert .equal (first .option, true);
            assert .equal (first .hash, on .hash, `the page's option draws with OIT: ${JSON .stringify ({ first, on, off })}`);
         }
         finally
         {
            await page .context () .close ();
         }
      });

      test (`${backend}: setBrowserOption on the live world, then loadURL`, { timeout: CASE_TIMEOUT }, async () =>
      {
         const page = await open (backend, { scripts: "true" });

         try
         {
            const { off, on } = await references (page);
            const plain = await draw (page);
            const live  = await draw (page, { world: null, before: b => b .setBrowserOption ("OrderIndependentTransparency", true) });
            const next  = await draw (page);
            const back  = await draw (page, { world: null, before: b => b .setBrowserOption ("OrderIndependentTransparency", false) });

            assert .equal (plain .hash, off .hash, "without the option the page draws without OIT");
            assert .equal (live .hash, on .hash, `the option set on the live world: ${JSON .stringify ({ live, on, off })}`);
            assert .equal (next .hash, on .hash, `the option stays after loadURL: ${JSON .stringify ({ next, on, off })}`);
            assert .equal (back .hash, off .hash, "the option set back to false");
         }
         finally
         {
            await page .context () .close ();
         }
      });

      test (`${backend}: the last one set is in force, the page's or a world Script's`, { timeout: CASE_TIMEOUT }, async () =>
      {
         const page = await open (backend, { scripts: "true" });

         try
         {
            const { off, on } = await references (page);
            const script = await draw (page, { world: scripted (false), before: b => b .setBrowserOption ("OrderIndependentTransparency", true) });
            const later  = await draw (page, { world: null, before: b => b .setBrowserOption ("OrderIndependentTransparency", true) });

            assert .equal (script .hash, off .hash, "the Script's false, set after the page's true, is in force");
            assert .equal (later .hash, on .hash, `the page's true, set after the Script's false, is in force: ${JSON .stringify ({ later, on, off })}`);
         }
         finally
         {
            await page .context () .close ();
         }
      });
   }
});
