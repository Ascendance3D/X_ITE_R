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
// * inlineScripts.*, externproto.*, followers.*: the other clean-room Script sets of the native suite, in the same
//   format (oracle/synthetic-inline-scripts: Scripts in Inline worlds, nesting, order, output, Browser access and
//   import/export; oracle/synthetic-externproto: EXTERNPROTO worlds with a Script, fallback and missing files;
//   oracle/synthetic-followers: the initial state of every follower node routed into Scripts). The reload sets
//   (oracle/synthetic-inline-reload, oracle/synthetic-externproto-reload) are step scenarios an embedding application
//   drives, with files published while the world runs, so they are not in this file.
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
// The clean-room Script sets, each with X_ITE's recording in the dump format of oracle/x_ite-dump.mjs. `prefix` names
// the test; `dir` is the set's directory under oracle/ (the server mounts it as /<dir>/).
const SETS = [
   { prefix: "x6",            dir: "synthetic-x6" },
   { prefix: "inlineScripts", dir: "synthetic-inline-scripts" },
   { prefix: "externproto",   dir: "synthetic-externproto" },
   { prefix: "followers",     dir: "synthetic-followers" },
]
.map (set => ({ ... set, results: JSON .parse (readFileSync (join (REPO, `oracle/${set .dir}-baseline.json`), "utf8")) .results }));
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

describe ("Scripts of the clean-room Script sets, compared with pinned X_ITE", () =>
{
   for (const { prefix, dir, results } of SETS)
   {
      for (const { file, vrml, log } of results)
      {
         test (`${prefix}.${file}`, { timeout: CASE_TIMEOUT }, async () =>
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
               `${server .base}/${dir}/${file}`);

               assert .equal (text .replaceAll (`${server .base}/${dir}`, `http://oracle/${dir}`), vrml, file);

               // The recording (oracle/x_ite-pin/src/dump.html) has only `console.error` and `console.warn`, sorted, levels
               // named as console's functions; what the Scripts print (`log:`) is not in it. Chromium's own resource
               // messages are not X_ITE's.
               const messages = page .log
                  .filter (line => /^(error|warning): /.test (line) && !line .startsWith ("error: Failed to load resource"))
                  .map (line => line .replace (/^warning: /, "warn: ") .replaceAll (`${server .base}/${dir}`, `http://oracle/${dir}`))
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

         // A world that replaces it: no shutdown, as in X_ITE 16.5.0 (scriptlife01 compares it with the recording).
         await loadText (page, "#VRML V2.0 utf8\nGroup { }\n");
         await loadText (page, "#VRML V2.0 utf8\nGroup { }\n");

         assert .deepEqual (prints (page), [ "initialize 0", "set_x 7", "routed 70", "eventsProcessed 7" ]);
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

// The order of the page's field callbacks and the lines of the Scripts (PR 196). The expected lists are the order of
// pinned X_ITE 16.5.0, taken from its console in a one-off run of these worlds and steps (no new baseline).
describe ("The page's field callbacks and the Script lines, in the order of X_ITE", () =>
{
   const WORLD = handler => `#VRML V2.0 utf8
DEF S Script
{
  eventIn SFInt32 set_x
  eventOut SFInt32 x_changed
  url "javascript:
function set_x (value) { print ('S handler set_x ' + value); ${handler} x_changed = value + 1; print ('S handler done'); }
"
}
DEF R Script
{
  eventIn SFInt32 set_y
  url "javascript: function set_y (value) { print ('R route set_y ' + value); }"
}
ROUTE S.x_changed TO R.set_y
`;

   // Runs the steps in the page with `note`, `S`, `scene` and `make`; answers the page and Script lines.
   async function run (world, steps)
   {
      const page = await openPage (DESKTOP);

      try
      {
         await allowScripts (page);
         await loadText (page, world);
         page .log .length = 0;

         await page .evaluate (async steps =>
         {
            const X3D     = window .X3D_NS;
            const browser = document .querySelector ("x3d-canvas") .browser;
            const frame   = () => new Promise (r => requestAnimationFrame (() => r ()));
            const spin    = async n => { for (let i = 0; i < n; ++ i) await frame (); };
            const scene   = () => browser .currentScene;
            const S       = scene () .getNamedNode ("S");
            const note    = text => console .log (`page: ${text}`);
            const make    = vrml => { const f = new X3D .SFNode (); f .fromVRMLString (vrml, scene ()); return f .valueOf (); };

            await new Function ("S", "scene", "note", "make", "spin", `return (async () => { ${steps} })()`) (S, scene, note, make, spin);
         },
         steps);

         assert .deepEqual (pageErrors (page), [ ]);
         return prints (page);
      }
      finally
      {
         await page .context () .close ();
      }
   }

   test ("order.input", { timeout: CASE_TIMEOUT }, async () =>
   {
      // CodeRabbit finding 1: the Script's handler is an interest of the field, so its lines come before the page's callback.
      const lines = await run (WORLD (""), `
         S .addFieldCallback ("page", "set_x", v => note (\`callback set_x \${v}\`));
         S .addFieldCallback ("page", "x_changed", v => note (\`callback x_changed \${v}\`));
         note ("set_x 1");
         S .set_x = 1;
         await spin (3);`);

      assert .deepEqual (lines, [ "page: set_x 1", "S handler set_x 1", "S handler done", "page: callback set_x 1", "page: callback x_changed 2", "R route set_y 2" ]);
   });

   test ("order.rootNodes", { timeout: CASE_TIMEOUT }, async () =>
   {
      // CodeRabbit finding 2: the rootNodes callback comes in its turn, in both orders of the two changes in one task.
      const lines = await run (WORLD (""), `
         scene () .rootNodes .addFieldCallback ("page", v => note (\`callback rootNodes \${v .length}\`));
         S .addFieldCallback ("page", "x_changed", v => note (\`callback x_changed \${v}\`));
         note ("addRootNode and set_x 1 in one task");
         scene () .addRootNode (make ("Group { }"));
         S .set_x = 1;
         await spin (3);
         note ("set_x 2 and then addRootNode in one task");
         S .set_x = 2;
         scene () .addRootNode (make ("Group { }"));
         await spin (3);`);

      assert .deepEqual (lines, [
         "page: addRootNode and set_x 1 in one task", "page: callback rootNodes 3", "S handler set_x 1", "S handler done", "page: callback x_changed 2", "R route set_y 2",
         "page: set_x 2 and then addRootNode in one task", "S handler set_x 2", "S handler done", "page: callback rootNodes 4", "page: callback x_changed 3", "R route set_y 3",
      ]);
   });

   test ("order.rootNodesInHandler", { timeout: CASE_TIMEOUT }, async () =>
   {
      // A Script's handler changes the root nodes: the page's callback follows the handler's lines.
      const lines = await run (WORLD ("Browser .currentScene .addRootNode (Browser .currentScene .createNode ('Group'));"), `
         scene () .rootNodes .addFieldCallback ("page", v => note (\`callback rootNodes \${v .length}\`));
         S .addFieldCallback ("page", "x_changed", v => note (\`callback x_changed \${v}\`));
         note ("set_x 1");
         S .set_x = 1;
         await spin (3);`);

      assert .deepEqual (lines, [ "page: set_x 1", "S handler set_x 1", "S handler done", "page: callback rootNodes 3", "page: callback x_changed 2", "R route set_y 2" ]);
   });
});
