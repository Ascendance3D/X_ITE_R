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
async function open (backend, extra = { }, signal)
{
   const page = await openPage (DESKTOP, { init: `(${attributes .toString ()}) (${JSON .stringify ({ backend, extra })})`, signal });

   const error = await page .evaluate (() => window .startError);

   assert .equal (error, undefined, `X_ITE_R did not start: ${error}\n${page .log .join ("\n")}`);

   return page;
}

// Runs `before` in the page, loads `world` (null: the world stays; with `replace`, by replaceWorld instead of loadURL),
// and returns the hash of the picture of that state. `option` is the page's OrderIndependentTransparency that the page
// itself set (the attribute or `before`), or null when the world's Script sets the option: in pinned X_ITE a Script's
// option is the page's too, in X_ITE_R it is the world's (the todo case), so the page's value is not a fact to check there.
//
// A picture counts only when the state is the expected one: the new scene is the current scene (the same scene when no
// world loads), the page's option has the expected value (when `option` is not null), and the engine has processed 2 frames since the load (the
// world's Scripts have run their initialize and the rendering changes are applied). Then the picture must be the same
// after one more processed frame (all RGBA bytes equal). With no such picture in 20 frames, the case fails.
//
// readPixels draws the current state into its own texture, so the canvas frames only let the world's events run. A
// fixed 20 frames per picture made the WebGPU case with 7 pictures take 105 s to more than 180 s on the CI runners'
// SwiftShader adapter (about 0.4 s to 1.5 s a frame): the picture was final after the first frame in every measured
// picture, on a GPU and on SwiftShader.
async function draw (page, { before = () => { }, world = WORLD, replace = false, option })
{
   assert .ok (option === true || option === false || option === null, "draw needs the expected page option or null");

   const result = await page .evaluate (async ([ text, beforeText, replace, option ]) =>
   {
      const browser   = document .querySelector ("x3d-canvas") .browser;
      const engine    = browser [Symbol .for ("X_ITE_R.engine")];
      const processed = () => browser [Symbol .for ("X_ITE_R.frameLoop")] .processed;
      const frame     = () => new Promise (r => requestAnimationFrame (r));
      const previous  = browser .currentScene;

      // The pixels (RGBA, compared byte for byte for the stability) and the hash that the cases compare.
      const picture = async () =>
      {
         const pixels = await engine .readPixels ();
         let hash = 0;

         for (let i = 0; i < pixels .length; i += 4)
            hash = (hash * 31 + pixels [i] + pixels [i + 1] * 3 + pixels [i + 2] * 7) % 1000003;

         return { pixels, hash };
      };

      const same = (a, b) => a .pixels .length === b .pixels .length && a .pixels .every ((v, i) => v === b .pixels [i]);

      await (0, eval) (beforeText) (browser);

      let scene = previous;

      if (text !== null && replace)
      {
         scene = await browser .createX3DFromString (text);

         await browser .replaceWorld (scene);
      }
      else if (text !== null)
      {
         await browser .loadURL (new window .X3D_NS .MFString ("data:model/x3d-vrml," + encodeURIComponent (text)));

         scene = browser .currentScene;
      }

      const start = processed ();

      const state = () => ({
         scene: text === null ? browser .currentScene === previous
            : replace ? browser .currentScene === scene
            : browser .currentScene === scene && scene !== previous,
         option: option === null || browser .getBrowserOption ("OrderIndependentTransparency") === option,
         processed: processed () - start,
      });

      let shot = null, at = 0;

      for (let frames = 0; frames < 20; ++ frames)
      {
         await frame ();

         const now = state ();

         if (!now .scene || !now .option || now .processed < 2)
            continue;

         if (shot === null || now .processed === at)
         {
            shot ??= await picture ();
            at   = now .processed;
            continue;
         }

         const next = await picture ();

         if (same (next, shot))
            return { hash: next .hash, option: browser .getBrowserOption ("OrderIndependentTransparency"), backend: engine .backend, frames: frames + 1 };

         shot = next;
         at   = now .processed;
      }

      return { error: `no stable picture of the expected state in 20 frames: ${JSON .stringify (state ())}` };
   }, [ world, `(${before .toString ()})`, replace, option ]);

   assert .equal (result .error, undefined, result .error);

   return result;
}

const BACKENDS = [ "webgpu", "webgl2" ];

describe ("oit 01", { skip: Boolean (ONLY) }, () =>
{
   for (const backend of BACKENDS)
   {
      // The "OIT off" and "OIT on" pictures: the page's option false, and a world Script's option true.
      async function references (page)
      {
         const off = await draw (page, { world: scripted (false), option: null });
         const on  = await draw (page, { world: scripted (true), option: null });

         assert .equal (off .backend, backend);
         assert .notEqual (on .hash, off .hash, "OIT on and off give different pictures");

         return { off, on };
      }

      test (`${backend}: the attribute set before the world loads`, { timeout: CASE_TIMEOUT }, async t =>
      {
         const page = await open (backend, { orderIndependentTransparency: "true", scripts: "true" }, t .signal);

         try
         {
            const first = await draw (page, { option: true });
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
            const plain = await draw (page, { before: b => b .setBrowserOption ("OrderIndependentTransparency", false), option: false });
            const live  = await draw (page, { world: null, before: b => b .setBrowserOption ("OrderIndependentTransparency", true), option: true });
            const next  = await draw (page, { option: true });
            const replaced = await draw (page, { replace: true, option: true });
            const back  = await draw (page, { world: null, before: b => b .setBrowserOption ("OrderIndependentTransparency", false), option: false });

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
      // getBrowserOption gives true). X_ITE_R's page takes a world Script's option, so the next world starts with it.
      test (`${backend}: a world Script's option stays for the next world`,
         { timeout: CASE_TIMEOUT }, async t =>
      {
         const page = await open (backend, { scripts: "true" }, t .signal);

         try
         {
            const { off, on } = await references (page);
            const after = await draw (page, { option: null });

            assert .equal (after .hash, on .hash, `the plain world after the Script's true: ${JSON .stringify ({ after, on, off })}`);
         }
         finally
         {
            await page .context () .close ();
         }
      });

      // Pinned X_ITE has one BrowserOptions, so a page field callback that runs in the route chain right after a world
      // Script's setBrowserOption reads the Script's value at once (issue #480). The callback runs inside the engine's
      // frame, where the page takes the options through the chain (chainTakeScriptOptions); the option's effect follows
      // at the end of the chain.
      test (`${backend}: a page callback in the route chain sees a world Script's option at once`, { timeout: CASE_TIMEOUT }, async t =>
      {
         const page = await open (backend, { scripts: "true" }, t .signal);

         try
         {
            const result = await page .evaluate (async text =>
            {
               const browser = document .querySelector ("x3d-canvas") .browser;
               const frame   = () => new Promise (r => requestAnimationFrame (r));

               await browser .replaceWorld (await browser .createX3DFromString (text));

               const scene = browser .currentScene;
               const [ A, B ] = [ "A", "B" ] .map (n => scene .getNamedNode (n));
               const seen = [ ];

               B .getField ("translation") .addFieldCallback ("seen", () => seen .push (browser .getBrowserOption ("OrderIndependentTransparency")));

               const before = browser .getBrowserOption ("OrderIndependentTransparency");

               A .translation = new window .X3D_NS .SFVec3f (1, 2, 3);

               for (let i = 0; i < 4 && !seen .length; ++ i)
                  await frame ();

               await frame ();

               return { before, seen, after: browser .getBrowserOption ("OrderIndependentTransparency") };
            }, `#X3D V4.0 utf8
PROFILE Immersive
COMPONENT Scripting:1
DEF A Transform { }
DEF B Transform { }
DEF S Script {
   inputOnly SFVec3f set_go
   outputOnly SFVec3f go_changed
   url "ecmascript: function set_go (value) { Browser .setBrowserOption ('OrderIndependentTransparency', true); go_changed = value; }"
}
ROUTE A.translation_changed TO S.set_go
ROUTE S.go_changed TO B.set_translation
`);

            assert .deepEqual (result, { before: false, seen: [ true ], after: true });
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
            const script = await draw (page, { world: scripted (false), before: b => b .setBrowserOption ("OrderIndependentTransparency", true), option: null });
            const later  = await draw (page, { world: null, before: b => b .setBrowserOption ("OrderIndependentTransparency", true), option: true });

            assert .equal (script .hash, off .hash, "the Script's false, set after the page's true, is in force");
            assert .equal (later .hash, on .hash, `the page's true, set after the Script's false, is in force: ${JSON .stringify ({ later, on, off })}`);
         }
         finally
         {
            await page .context () .close ();
         }
      });
   }

   // draw accepts no picture of a state other than the expected one: here the page's option stays false where the
   // caller expects true, so two equal pictures of that state do not count, and draw fails.
   test ("a stable picture of an unexpected state is not accepted", { timeout: CASE_TIMEOUT }, async t =>
   {
      const page = await open ("webgl2", { scripts: "true" }, t .signal);

      try
      {
         await draw (page, { option: false });
         await assert .rejects (draw (page, { world: null, option: true }), /no stable picture of the expected state in 20 frames: .*"option":false/);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   // When node:test cancels a test at its timeout, the test function runs on and its page draws on until the function
   // ends. In a CI run (SwiftShader) the next test's page then did not start in 30 s. The page of a cancelled test closes.
   // node:test cancels a test by aborting its signal (TestContext.signal, Node 24); this case aborts a signal of its own,
   // because a case that times out fails. The path through a real timeout was checked apart (lane report).
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
