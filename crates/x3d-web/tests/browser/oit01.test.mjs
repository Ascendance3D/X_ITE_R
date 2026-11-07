// OIT 01 test of X_ITE_R's <x3d-canvas> (issue #479): the page's OrderIndependentTransparency option reaches the
// renderer, before a world loads, on the live world and after loadURL and replaceWorld, and the last one set is in
// force, the page's or a world Script's.
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

// `signal`: the test's signal; the page closes when the test is cancelled (openPage).
async function open (backend, extra, signal)
{
   const page = await openPage (DESKTOP, { init: `(${attributes .toString ()}) (${JSON .stringify ({ backend, extra })})`, signal });

   const error = await page .evaluate (() => window .startError);

   assert .equal (error, undefined, `X_ITE_R did not start: ${error}\n${page .log .join ("\n")}`);

   return page;
}

// Runs `before` in the page, loads `world` (null: the world stays; with `replace`, by replaceWorld instead of loadURL),
// lets frames pass until the picture is the same after two frames in sequence (at most 20 frames), and returns its hash.
//
// readPixels draws the current state into its own texture, so the canvas frames only let the world's events run. A
// fixed 20 frames per picture made the WebGPU case with 7 pictures take 105 s to more than 180 s on the CI runners'
// SwiftShader adapter (about 0.4 s to 1.5 s a frame): the picture was final after the first frame in every measured
// picture, on a GPU and on SwiftShader.
async function draw (page, { before = () => { }, world = WORLD, replace = false } = { })
{
   return page .evaluate (async ([ text, beforeText, replace ]) =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const engine  = browser [Symbol .for ("X_ITE_R.engine")];
      const frame   = () => new Promise (r => requestAnimationFrame (r));

      const picture = async () =>
      {
         const pixels = await engine .readPixels ();
         let hash = 0;

         for (let i = 0; i < pixels .length; i += 4)
            hash = (hash * 31 + pixels [i] + pixels [i + 1] * 3 + pixels [i + 2] * 7) % 1000003;

         return hash;
      };

      await (0, eval) (beforeText) (browser);

      if (text !== null && replace)
         await browser .replaceWorld (await browser .createX3DFromString (text));
      else if (text !== null)
         await browser .loadURL (new window .X3D_NS .MFString ("data:model/x3d-vrml," + encodeURIComponent (text)));

      await frame ();
      await frame ();

      let hash = await picture ();

      for (let frames = 2; frames < 20; ++ frames)
      {
         await frame ();

         const next = await picture ();

         if (next === hash)
            break;

         hash = next;
      }

      return { hash, option: browser .getBrowserOption ("OrderIndependentTransparency"), backend: engine .backend };
   }, [ world, `(${before .toString ()})`, replace ]);
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

      test (`${backend}: the attribute set before the world loads`, { timeout: CASE_TIMEOUT }, async t =>
      {
         const page = await open (backend, { orderIndependentTransparency: "true", scripts: "true" }, t .signal);

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

      test (`${backend}: setBrowserOption on the live world, then loadURL and replaceWorld`, { timeout: CASE_TIMEOUT }, async t =>
      {
         const page = await open (backend, { scripts: "true" }, t .signal);

         try
         {
            const { off, on } = await references (page);
            // The page sets false first: in X_ITE the Script's true of the second reference world stays for the next world.
            const plain = await draw (page, { before: b => b .setBrowserOption ("OrderIndependentTransparency", false) });
            const live  = await draw (page, { world: null, before: b => b .setBrowserOption ("OrderIndependentTransparency", true) });
            const next  = await draw (page);
            const replaced = await draw (page, { replace: true });
            const back  = await draw (page, { world: null, before: b => b .setBrowserOption ("OrderIndependentTransparency", false) });

            assert .equal (plain .hash, off .hash, "with the option false the page draws without OIT");
            assert .equal (live .hash, on .hash, `the option set on the live world: ${JSON .stringify ({ live, on, off })}`);
            assert .equal (next .hash, on .hash, `the option stays after loadURL: ${JSON .stringify ({ next, on, off })}`);
            assert .equal (replaced .hash, on .hash, `the option stays after replaceWorld: ${JSON .stringify ({ replaced, on, off })}`);
            assert .equal (back .hash, off .hash, "the option set back to false");
         }
         finally
         {
            await page .context () .close ();
         }
      });

      // Pinned X_ITE 16.5.0 (Chrome 156): BrowserOptions.reset runs only when the browser starts, so a world Script's
      // OrderIndependentTransparency true stays for the next loadURL (the plain world after it draws with OIT on, and
      // getBrowserOption gives true). X_ITE_R starts each world with the page's options.
      test (`${backend}: a world Script's option stays for the next world`,
         { timeout: CASE_TIMEOUT, todo: "a world Script's browser option does not stay for the next world, as in X_ITE" }, async t =>
      {
         const page = await open (backend, { scripts: "true" }, t .signal);

         try
         {
            const { off, on } = await references (page);
            const after = await draw (page);

            assert .equal (after .hash, on .hash, `the plain world after the Script's true: ${JSON .stringify ({ after, on, off })}`);
         }
         finally
         {
            await page .context () .close ();
         }
      });

      test (`${backend}: the last one set is in force, the page's or a world Script's`, { timeout: CASE_TIMEOUT }, async t =>
      {
         const page = await open (backend, { scripts: "true" }, t .signal);

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

   // When node:test cancels a test at its timeout, the test function runs on and its page draws on until the function
   // ends. In a CI run (SwiftShader) the next test's page then did not start in 30 s. The page of a cancelled test closes.
   test ("the page of a cancelled test closes", { timeout: CASE_TIMEOUT }, async () =>
   {
      const cancel = new AbortController ();
      const page   = await open ("webgl2", { }, cancel .signal);
      const closed = new Promise (resolve => page .context () .once ("close", resolve));

      let timer;

      cancel .abort ();

      const result = await Promise .race ([ closed .then (() => "closed"),
         new Promise (resolve => timer = setTimeout (resolve, 10_000, "open after 10 s")) ]);

      clearTimeout (timer);

      if (result !== "closed")
         await page .context () .close ();

      assert .equal (result, "closed", "the page of the cancelled test");
      assert .ok (page .isClosed (), "the page is closed");
   });
});
