// Script nodes in the browser build (issue #133), in real headless Chromium with WebGPU. The page lets a world's
// Scripts run with `<x3d-canvas scripts="true">`, read when the world starts; they run in the page's own engine with
// the page's privileges (web/API.md, "Script nodes").
//
// Compared with existing pinned X_ITE 16.5.0 recordings, with no new baseline:
//
// * x6.*: the 8 synthetic Script worlds of oracle/synthetic-x6 (router order and route loops, interpolators, TimeSensor,
//   field marshalling for every VRML97 field type, the Browser object, Script errors, Scripts in PROTOs, external .js
//   with URL fallback). oracle/synthetic-x6-baseline.json holds X_ITE's `toVRMLString ()` of each world after
//   createX3DFromURL with the time frozen at 946684800 (oracle/x_ite-dump.mjs), and its sorted console messages.
// * scriptURL.*: the Script cases of scripturl01 (scripturl01_case.mjs), whose X_ITE recording has what the Script did
//   (`detail`: `which` and the console) that scripturl01.test.mjs cannot compare with Scripts off.
//
// The other tests check what no recording has: the default (Scripts off), a Content Security Policy without
// `unsafe-eval`, the Script lifecycle and event order on the page console, the page's rootNodes field when a Script
// changes the root nodes (both rootNodes findings of the issue #133 gate), and that the page's built-ins stay its own.
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && node --test tests/browser/script01.test.mjs)

import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { CASE_TIMEOUT, DESKTOP, openPage, server } from "./runtime02_harness.mjs";
import { scriptCases } from "./scripturl01_case.mjs";

const REPO = join (new URL (".", import.meta.url) .pathname, "../../../..");
const X6   = JSON .parse (readFileSync (join (REPO, "oracle/synthetic-x6-baseline.json"), "utf8"));
const URL1 = JSON .parse (readFileSync (join (process .env .XR_SCRIPTURL_BASELINE ?? join (REPO, "oracle/web-browser-script-url-01-baseline"), "scripturl01-baseline.json"), "utf8"));

const env = () => ({ base: server .base, foreign: server .foreign, worlds: `${server .base}/tests/browser/worlds/`, server });

// The page lets the Scripts of the next world run (or not).
const allowScripts = (page, on = true) => page .evaluate (on => on
   ? document .querySelector ("x3d-canvas") .setAttribute ("scripts", "true")
   : document .querySelector ("x3d-canvas") .removeAttribute ("scripts"), on);

const pageErrors = page => page .log .filter (line => line .startsWith ("pageerror"));

// A world from text replaces the current one; the frames after it run.
async function loadText (page, text, ms = 300)
{
   await page .evaluate (async text =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;

      await browser .replaceWorld (await browser .createX3DFromString (text));
   },
   text);

   await page .waitForTimeout (ms);
}

const prints = page => page .log .filter (line => line .startsWith ("log: ")) .map (line => line .slice (5));

describe ("Scripts of the synthetic X6 worlds, compared with pinned X_ITE", () =>
{
   for (const { file, vrml, log } of X6 .results)
   {
      test (`x6.${file}`, { timeout: CASE_TIMEOUT }, async () =>
      {
         const page = await openPage (DESKTOP);

         try
         {
            await allowScripts (page);
            // X_ITE's dump froze the browser time (getCurrentTime) at 946684800.
            await page .clock .setFixedTime (new Date (946684800000));
            page .log .length = 0;

            const text = await page .evaluate (async url =>
            {
               const X3D   = window .X3D_NS;
               const scene = await X3D .getBrowser () .createX3DFromURL (new X3D .MFString (url));

               return scene .toVRMLString ();
            },
            `${server .base}/synthetic-x6/${file}`);

            assert .equal (text .replaceAll (`${server .base}/synthetic-x6`, "http://oracle/synthetic-x6"), vrml, file);

            // The recording has the console sorted, levels named as console's functions; Chromium's own resource
            // messages are not X_ITE's.
            const messages = page .log
               .filter (line => !line .startsWith ("error: Failed to load resource"))
               .map (line => line .replace (/^warning: /, "warn: ") .replaceAll (`${server .base}/synthetic-x6`, "http://oracle/synthetic-x6"))
               .sort ();

            assert .deepEqual (messages, log, `${file} console`);
            assert .deepEqual (pageErrors (page), [ ]);
         }
         finally
         {
            await page .context () .close ();
         }
      });
   }
});

describe ("Script URL cases with Scripts on, compared with pinned X_ITE including what the Script did", () =>
{
   for (const [ name, run ] of Object .entries (scriptCases))
   {
      const key = `scriptURLScript.${name}`;

      test (`scriptURL.${name}`, { timeout: CASE_TIMEOUT }, async () =>
      {
         const page = await openPage (DESKTOP);

         try
         {
            await allowScripts (page);

            assert .deepEqual (await run (page, env ()), URL1 .values [key], key);
            assert .deepEqual (pageErrors (page), [ ]);
         }
         finally
         {
            await page .context () .close ();
         }
      });
   }
});

const LIFECYCLE = `#VRML V2.0 utf8
DEF T TimeSensor { loop TRUE }
DEF S Script
{
  eventIn SFInt32 set_x
  eventOut SFInt32 x_changed
  field SFInt32 x 0
  url "javascript:
function initialize () { print ('initialize ' + x); }
function set_x (value) { x = value; x_changed = value * 10; print ('set_x ' + value); }
function eventsProcessed () { print ('eventsProcessed ' + x); }
function shutdown () { print ('shutdown ' + x); }
"
}
DEF R Script
{
  eventIn SFInt32 set_y
  url "javascript: function set_y (value) { print ('routed ' + value); }"
}
ROUTE S.x_changed TO R.set_y
`;

describe ("Script lifecycle and switch of the page", () =>
{
   test ("scripts.default", { timeout: CASE_TIMEOUT }, async () =>
   {
      const page = await openPage (DESKTOP);

      try
      {
         // No `scripts` attribute: the Scripts do not run and say so.
         await loadText (page, LIFECYCLE);

         assert .deepEqual (prints (page), [ ]);
         assert .ok (page .log .some (line => line .includes ("no script engine")), page .log .join ("\n"));
         assert .deepEqual (pageErrors (page), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("scripts.lifecycle", { timeout: CASE_TIMEOUT }, async () =>
   {
      const page = await openPage (DESKTOP);

      try
      {
         await allowScripts (page);
         await loadText (page, LIFECYCLE);

         // An event of the page, routed on to the second Script, then eventsProcessed.
         await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("S") .set_x = 7);
         await page .waitForTimeout (300);

         // The page reads the Script's field.
         assert .equal (await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("S") .x), 7);

         // A world that replaces it: shutdown, once.
         await loadText (page, "#VRML V2.0 utf8\nGroup { }\n");
         await loadText (page, "#VRML V2.0 utf8\nGroup { }\n");

         assert .deepEqual (prints (page), [ "initialize 0", "set_x 7", "routed 70", "eventsProcessed 7", "shutdown 7" ]);
         assert .deepEqual (pageErrors (page), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("scripts.pageBuiltins", { timeout: CASE_TIMEOUT }, async () =>
   {
      const page = await openPage (DESKTOP);

      try
      {
         await allowScripts (page);
         await loadText (page, `#VRML V2.0 utf8
DEF S Script
{
  field SFString title ""
  url "javascript: function initialize () { title = document .title; document .scriptCall = function () { return Browser .getName (); }; }"
}
`);

         const before = await page .evaluate (() => ({
            title: document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("S") .title,
            call: document .scriptCall (),
            x: typeof window .__x,
            page: typeof window .__page,
            builtins: [ Date .now, Math .sin, Error .captureStackTrace ] .every (f => /\[native code\]/ .test (String (f))),
         }));

         // A Script has the page's privileges: it reads the document and leaves a function there.
         assert .deepEqual (before, { title: await page .title (), call: "X_ITE", x: "undefined", page: "undefined", builtins: true });

         // Its world gone, the function's call into the engine throws.
         await loadText (page, "#VRML V2.0 utf8\nGroup { }\n");

         const after = await page .evaluate (() => { try { return document .scriptCall (); } catch (error) { return error .message; } });

         assert .equal (after, "X_ITE_R: the Script's world is gone.");
         assert .deepEqual (pageErrors (page), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("scripts.csp", { timeout: CASE_TIMEOUT }, async () =>
   {
      const page = await openPage (DESKTOP);

      try
      {
         // The page again, with a policy that allows its modules, its inline module and WebAssembly, but no eval. The
         // page records each violation.
         const policy = "script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'";

         await page .route (/\/runtime02\.html$/, async route =>
         {
            const response = await route .fetch ();
            const body     = (await response .text ()) .replace ('<meta charset="utf-8">', `<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="${policy}">
<script>window .violations = [ ]; addEventListener ("securitypolicyviolation", e => violations .push (e .violatedDirective));</script>`);

            await route .fulfill ({ response, body });
         });

         await page .reload ();
         await page .waitForFunction (() => window .ready || window .startError, null, { timeout: 30000 });
         assert .equal (await page .evaluate (() => window .startError), undefined);

         await allowScripts (page);
         page .log .length = 0;

         // The world starts from a task of the page: code that Playwright evaluates may eval (CDP Runtime.evaluate,
         // allowUnsafeEvalBlockedByCSP).
         await page .evaluate (text => new Promise ((resolve, reject) => setTimeout (() =>
         {
            const browser = document .querySelector ("x3d-canvas") .browser;

            browser .createX3DFromString (text) .then (scene => browser .replaceWorld (scene)) .then (resolve, reject);
         })),
         LIFECYCLE);

         await page .waitForTimeout (300);

         // The Script realm cannot be made: the engine says why, the Scripts do not run, and nothing else breaks.
         assert .deepEqual (prints (page), [ ]);
         assert .ok (page .log .some (line => /the Scripts of this world cannot run: EvalError/ .test (line)), page .log .join ("\n"));
         assert .ok (page .log .some (line => line .includes ("no script engine")), page .log .join ("\n"));
         assert .ok ((await page .evaluate (() => window .violations)) .includes ("script-src"));
         assert .deepEqual (pageErrors (page), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });
});

describe ("The page's rootNodes field when a Script changes the root nodes", () =>
{
   const WORLD = `#VRML V2.0 utf8
DEF A Transform { }
DEF S Script
{
  eventIn SFTime add
  url "javascript: function add () { Browser .currentScene .addRootNode (Browser .currentScene .createNode ('Group')); }"
}
`;

   // The Script adds a root node by itself 1.5 s after it started: the page needs no node object to make it.
   const DELAYED = `#VRML V2.0 utf8
DEF S Script
{
  url "javascript:
var start = 0, done = false;
function prepareEvents (time) {
  if (! start) start = time;
  if (done || time - start < 1.5) return;
  done = true;
  Browser .currentScene .addRootNode (Browser .currentScene .createNode ('Group'));
}
"
}
`;

   const add = page => page .evaluate (() => document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("S") .add = 1);

   test ("rootNodes.callback", { timeout: CASE_TIMEOUT }, async () =>
   {
      const page = await openPage (DESKTOP);

      try
      {
         await allowScripts (page);
         await loadText (page, WORLD);

         await page .evaluate (() =>
         {
            window .rootNodesEvents = [ ];
            document .querySelector ("x3d-canvas") .browser .currentScene .rootNodes
               .addFieldCallback ("test", value => window .rootNodesEvents .push (value .length));
         });

         await add (page);
         await page .waitForTimeout (300);

         // The field callback of the page has the event the Script's change made.
         assert .deepEqual (await page .evaluate (() => window .rootNodesEvents), [ 3 ]);
         assert .deepEqual (pageErrors (page), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("rootNodes.retained", { timeout: CASE_TIMEOUT }, async () =>
   {
      const page = await openPage (DESKTOP);

      try
      {
         await allowScripts (page);
         await loadText (page, DELAYED, 0);

         // The page keeps the field object and holds no node object of the scene.
         assert .equal (await page .evaluate (() => (window .rootNodes = document .querySelector ("x3d-canvas") .browser .currentScene .rootNodes) .length), 1);

         // The kept object has the Script's change after a frame, without a new read of rootNodes.
         await page .waitForFunction (() => window .rootNodes .length === 2, null, { timeout: 15000 });
         assert .deepEqual (pageErrors (page), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });
});
