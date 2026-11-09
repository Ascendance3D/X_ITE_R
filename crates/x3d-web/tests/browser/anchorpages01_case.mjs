// The web Anchor pages 01 cases: what an Anchor does with a URL that is not an X3D world for this page: a page (.html), a
// `target` parameter, an unreachable URL, a viewpoint fragment. oracle/x_ite-web-runtime-02.mjs (--suite anchorpages01) runs
// them against pinned X_ITE 16.5.0 (Anchor.js requestImmediateLoad, FileLoader.js loadDocumentAsync) and records the values;
// anchorpages01.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).
//
// Each case loads a world with one Anchor over a box, clicks the box with the real mouse, and records what the page saw:
// the arguments of window.open (replaced by a recorder; nothing opens), the main-frame navigation requests (Chrome's own "Failed to load resource" lines are left out; the page
// navigates for `location = url`; the context aborts every request that is not to the local server), the requests the
// server got, the browser events, whether the world is still the same world, and the console lines.

import { DESKTOP, settle } from "./runtime02_case.mjs";

const WORLD = anchor => `#VRML V2.0 utf8
Viewpoint { position 0 0 4 }
DEF V1 Viewpoint { position 0 0 6 description "V1" }
DEF M Transform { children Anchor { ${anchor} children Shape { geometry Box { size 3 3 0.1 } } } }
`;

const q = list => list .map (s => `"${s}"`) .join (" ");

// The URL without the origins and the engines' page paths.
const clean = (text, env) => String (text)
   .replaceAll (env .base, "http://test")
   .replaceAll (env .foreign, "http://foreign")
   .replace (/\/x_ite\/src\/__x_ite_r_web_runtime_02__\.html|\/tests\/browser\/runtime02\.html/g, "/PAGE")
   .replace (/\/x_ite\/src\/worlds\/|\/tests\/browser\/worlds\//g, "/worlds/");

// The console lines that concern the Anchor and its load.
const lines = (log, env) => log
   .map (line => clean (line, env))
   .filter (line => !line .startsWith ("error: Failed to load resource"))
   .filter (line => /Anchor|Couldn't|Failed to fetch|not opened|Error:|Empty URL/i .test (line));

// Load the world and wait for a drawn frame (picking needs one).
async function openWorld (page, env, anchor)
{
   await page .evaluate (async text =>
   {
      const canvas  = document .querySelector ("x3d-canvas");
      const browser = canvas .browser;

      window .opened = [ ];
      window .open   = (... args) => { window .opened .push (args); return null; };
      window .events = [ ];

      for (const name of [ "initialized", "load", "error", "shutdown" ])
         canvas .addEventListener (name, event => window .events .push (event .type));

      await browser .replaceWorld (await browser .createX3DFromString (text));
      window .events .length = 0;

      for (let i = 0; i < 3; ++ i)
         await new Promise (requestAnimationFrame);

      window .marker = browser .currentScene .getNamedNode ("M");
   },
   WORLD (anchor));

   await settle (page, 500);
}

// One click on the Anchor. `result` is what the page and the server saw.
async function anchorCase (page, env, anchor, { wait = 1500 } = { })
{
   const navigations = [ ];

   page .on ("request", request =>
   {
      if (request .isNavigationRequest () && request .frame () === page .mainFrame ())
         navigations .push (clean (request .url (), env));
   });

   await openWorld (page, env, anchor);

   page .log .length = 0;

   const from = env .server .requests .length;

   await page .mouse .click (200, 150);

   // The click is done when the Anchor did something: opened, navigated, or logged.
   for (let i = 0; i < wait / 50; ++ i)
   {
      const opened = await page .evaluate (() => window .opened .length) .catch (() => 1);

      if (opened || navigations .length || lines (page .log, env) .length)
         break;

      await page .waitForTimeout (50);
   }

   await page .waitForTimeout (600);

   const same = await page .evaluate (() =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;

      return { opened: window .opened .map (([ url, target, features ]) => ({ url, target, features: features ?? null })), events: window .events,
               sameWorld: (() => { try { return browser .currentScene .getNamedNode ("M") === window .marker; } catch { return false; } }) () };
   })
   .catch (() => null);

   const bound = same && await page .evaluate (() =>
   {
      const scene = document .querySelector ("x3d-canvas") .browser .currentScene;

      try { return scene .getNamedNode ("V1") .isBound; } catch { return null; }
   })
   .catch (() => null);

   const own = env .server .requests .slice (from) .map (r => clean (r .path, env)) .filter (p => /\/worlds\/anchor\/|\/status\/|\/garbage\//.test (p));

   return {
      // null: the page navigated away, its document is gone.
      page: same === null ? "navigated" : "stayed",
      opened: same ?.opened .map (o => ({ ... o, url: clean (o .url, env) })) ?? null,
      sameWorld: same ?.sameWorld ?? null,
      events: same ?.events ?? null,
      viewpointBound: bound,
      navigations: navigations .filter (url => !url .endsWith ("/PAGE")),
      served: own,
      lines: lines (page .log, env),
   };
}

const W = env => `${env .worlds}anchor/`;

export const anchorPagesCases = {
   // A page URL without a target: `location = url`.
   page: (page, env) => anchorCase (page, env, `url "${W (env)}page.html"`),

   // `.htm` and `.xhtml` are pages too; `.HTML` is not (the extension test is case sensitive).
   pageHtm: (page, env) => anchorCase (page, env, `url "${W (env)}page.htm"`),
   pageUpper: (page, env) => anchorCase (page, env, `url "${W (env)}page.HTML"`),

   // A page with a query: not by extension, so the request is made and the response's Content-Type (the server answers
   // text/html for page.html) hands it to the page.
   pageQuery: (page, env) => anchorCase (page, env, `url "${W (env)}page.html?a=1"`),

   // The same page with a target.
   targetBlank: (page, env) => anchorCase (page, env, `url "${W (env)}page.html" parameter "target=_blank"`),
   targetName: (page, env) => anchorCase (page, env, `url "${W (env)}page.html" parameter "target=frame1"`),
   targetSelf: (page, env) => anchorCase (page, env, `url "${W (env)}page.html" parameter "target=_self"`),
   targetEmpty: (page, env) => anchorCase (page, env, `url "${W (env)}page.html" parameter "target="`),

   // The parameter list: the first `target=` pair wins; an entry that is not one `name=value` pair is skipped; the name is
   // case sensitive.
   paramFirstWins: (page, env) => anchorCase (page, env, `url "${W (env)}page.html" parameter [ ${q ([ "description=x", "target=first", "target=second" ])} ]`),
   paramMalformed: (page, env) => anchorCase (page, env, `url "${W (env)}page.html" parameter [ ${q ([ "target", "target=a=b", "Target=up" ])} ]`),

   // An X3D URL with a target is not loaded: it goes to the target, and this world stays.
   x3dTargetBlank: (page, env) => anchorCase (page, env, `url "${W (env)}other.wrl" parameter "target=_blank"`),
   x3dTargetName: (page, env) => anchorCase (page, env, `url "${W (env)}other.wrl" parameter "target=win2"`),
   x3dTargetSelf: (page, env) => anchorCase (page, env, `url "${W (env)}other.wrl" parameter "target=_self"`),

   // A viewpoint fragment of this world binds the viewpoint; the target does not make it a window: the fragment test comes first.
   fragment: (page, env) => anchorCase (page, env, `url "#V1"`),
   fragmentTarget: (page, env) => anchorCase (page, env, `url "#V1" parameter "target=_blank"`),

   // An URL list: the first that loads is used. A missing X3D file, then a page.
   missingThenPage: (page, env) => anchorCase (page, env, `url [ "${env .base}/status/404/missing.wrl" "${W (env)}page.html" ]`),
   pageThenX3d: (page, env) => anchorCase (page, env, `url [ "${W (env)}page.html" "${W (env)}other.wrl" ]`),

   // Unreachable URLs (the context aborts every request outside the local server). A page by extension is handed to the page
   // without a request; with a target it is opened; an unreachable X3D file is a failed load.
   unreachablePage: (page, env) => anchorCase (page, env, `url "http://unreachable.invalid/page.html"`),
   unreachablePageTarget: (page, env) => anchorCase (page, env, `url "http://unreachable.invalid/page.html" parameter "target=_blank"`),
   unreachableWorld: (page, env) => anchorCase (page, env, `url "http://unreachable.invalid/world.wrl"`),
   unreachableWorldTarget: (page, env) => anchorCase (page, env, `url "http://unreachable.invalid/world.wrl" parameter "target=_blank"`),
   notFoundText: (page, env) => anchorCase (page, env, `url "${env .base}/status/404/doc.txt"`),
   notFoundPage: (page, env) => anchorCase (page, env, `url "${env .base}/status/404/page.html"`),
   emptyUrl: (page, env) => anchorCase (page, env, `url ""`),

   // A URL on the other origin of the test server.
   foreignOrigin: (page, env) => anchorCase (page, env, `url "${env .foreign}/tests/browser/worlds/anchor/page.html" parameter "target=_blank"`),
};

export const CASE_GROUPS = {
   anchorPages: { context: DESKTOP, cases: anchorPagesCases },
};
