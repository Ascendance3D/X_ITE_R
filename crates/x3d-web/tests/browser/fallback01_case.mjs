// The web resource fallback 01 cases: what FileLoader does after a candidate URL that downloads but does not load
// (no parser takes it, it does not parse, an image does not decode, a glTF buffer fails), and when the load is
// replaced, cancelled or disposed while a later candidate is in flight. oracle/x_ite-web-runtime-02.mjs
// (--suite fallback01) runs them against pinned X_ITE 16.5.0 and records the values; fallback01.test.mjs runs them
// against X_ITE_R and compares (runtime02_harness.mjs). The worlds are in tests/browser/worlds/fallback/.

import { DESKTOP, loadMessages, pageLoad, served, settle } from "./runtime02_case.mjs";

// The served paths of each node, in request order: `tags` maps a name to a pattern of the paths of one node. The
// order of the requests of two nodes is the engine's, not the loading contract (see loadMessages).
function byNode (paths, tags)
{
   return Object .fromEntries (Object .entries (tags) .map (([ name, pattern ]) => [ name, paths .filter (path => pattern .test (path)) ]));
}

// Wait until the server got a request whose path starts with `prefix` (a held candidate is in flight).
async function requested (env, prefix, from)
{
   for (let i = 0; i < 200; ++ i)
   {
      if (env .server .requests .slice (from) .some (r => r .path .startsWith (prefix)))
         return;

      await new Promise (r => setTimeout (r, 50));
   }

   throw new Error (`no request of ${prefix}`);
}

// loadURL of `urls` in the page; the result is "resolved" or the rejection's message.
function pageLoadURL (urls)
{
   const browser = document .querySelector ("x3d-canvas") .browser;

   window .pending = browser .loadURL (new window .X3D_NS .MFString (... urls)) .then (() => "resolved", e => e .message);
}

// A world with a glTF Inline: whether loadURL settles within 5 s, then the requests, the messages and (with
// `picture`) the picture. X_ITE's world never settles after a buffer failed, and it still draws the world before.
async function gltfCase (page, env, world, picture)
{
   page .log .length = 0;

   const from = env .server .requests .length;

   await page .evaluate (pageLoadURL, [ `${env .worlds}fallback/${world}` ]);

   const result = await page .evaluate (() => Promise .race ([ window .pending, new Promise (r => setTimeout (() => r ("pending"), 5000)) ]));

   await settle (page, 300);

   return { result, served: served (env, from), console: loadMessages (page .log, env), ... picture && { picture: await env .picture (page) } };
}

const race = page => page .evaluate (() => Promise .race ([ window .pending, new Promise (r => setTimeout (() => r ("pending"), 300)) ]));

const worldName = page => page .evaluate (() => document .querySelector ("x3d-canvas") .browser .currentScene .worldURL .replace (/^.*\/worlds\//, ""));

export const fallbackCases = {
   // Inline and ImageTexture candidates that download but do not load, then candidates that do.
   async content (page, env)
   {
      page .log .length = 0;

      const from   = env .server .requests .length;
      const events = await page .evaluate (pageLoad, `${env .worlds}fallback/content.wrl`);

      await settle (page, 300);

      return {
         events,
         served: byNode (served (env, from), {
            a: /\/a-|\/a\.wrl$/, b: /\/b-|\/b\.wrl$/, c: /red\.png$|\/c\.wrl$/, d: /\/d-|\/d\.wrl$/,
            texture: /\/t-first|green\.png$/, e: /\/e-/,
         }),
         console: loadMessages (page .log, env),
         picture: await env .picture (page),
      };
   },

   // The control: a glTF file whose external buffer loads.
   gltfLoads: (page, env) => gltfCase (page, env, "gltf-loads.wrl", true),

   // A glTF file whose external buffer fails, then one that loads.
   gltfFallback: (page, env) => gltfCase (page, env, "gltf-fallback.wrl"),

   // The only candidate is a glTF file whose buffers fail.
   gltfFailed: (page, env) => gltfCase (page, env, "gltf-failed.wrl"),

   // loadURL: a world candidate that does not parse, then one that loads.
   async world (page, env)
   {
      page .log .length = 0;

      const from = env .server .requests .length;

      await page .evaluate (pageLoadURL, [ `${env .base}/garbage/w-first.wrl`, `${env .worlds}fallback/parts/w.wrl` ]);

      const result = await page .evaluate (() => window .pending);

      await settle (page, 300);

      return { result, world: await worldName (page), served: served (env, from), console: loadMessages (page .log, env), picture: await env .picture (page) };
   },

   // Two Inlines of one world with the same candidates; the first does not parse.
   async repeated (page, env)
   {
      page .log .length = 0;

      const from   = env .server .requests .length;
      const events = await page .evaluate (pageLoad, `${env .worlds}fallback/repeated.wrl`);

      await settle (page, 300);

      const paths = served (env, from);

      return {
         events,
         first: paths .filter (path => path .endsWith ("/r-first.wrl")) .length,
         second: paths .filter (path => path .endsWith ("/r.wrl")) .length,
         console: loadMessages (page .log, env),
         picture: await env .picture (page),
      };
   },

   // Two scenes (createX3DFromString) whose Inlines have the same candidates; the first does not parse.
   async twoScenes (page, env)
   {
      page .log .length = 0;

      const from = env .server .requests .length;
      const out  = await page .evaluate (async ([ first, second ]) =>
      {
         const browser = document .querySelector ("x3d-canvas") .browser;
         const text    = `#VRML V2.0 utf8\nViewpoint { position 0 0 4 }\nInline { url [ "${first}" "${second}" ] }\n`;
         const scenes  = await Promise .all ([ browser .createX3DFromString (text), browser .createX3DFromString (text) ]);

         await browser .replaceWorld (scenes [1]);
         return { replaced: browser .currentScene === scenes [1] };
      },
      [ `${env .base}/garbage/s-first.wrl`, `${env .worlds}fallback/parts/s.wrl` ]);

      await settle (page, 300);

      const paths = served (env, from);

      return {
         ... out,
         first: paths .filter (path => path .endsWith ("/s-first.wrl")) .length,
         second: paths .filter (path => path .endsWith ("/s.wrl")) .length,
         console: loadMessages (page .log, env),
         picture: await env .picture (page),
      };
   },

   // A newer loadURL while an Inline's second candidate (after one that did not parse) is in flight; the response
   // comes after the older world was replaced.
   async replacedDuringFallback (page, env)
   {
      env .server .arm ("fb");
      page .log .length = 0;

      const from = env .server .requests .length;

      await page .evaluate (pageLoadURL, [ `${env .worlds}fallback/held.wrl` ]);
      await requested (env, "/hold/fb/", from);

      const second = await page .evaluate (url => document .querySelector ("x3d-canvas") .browser .loadURL (new window .X3D_NS .MFString (url)) .then (() => "resolved", e => e .message),
         `${env .worlds}fallback/parts/d.wrl`);

      env .server .release ("fb");
      await settle (page, 500);

      return {
         first: await race (page),
         second,
         world: await worldName (page),
         served: served (env, from),
         console: loadMessages (page .log, env),
         picture: await env .picture (page),
      };
   },

   // A newer loadURL while an Inline's first candidate is in flight; that response does not parse. No load of the
   // older world goes on to the next candidate unless the reference does.
   async cancelledBeforeFallback (page, env)
   {
      env .server .arm ("fc");
      page .log .length = 0;

      const from = env .server .requests .length;

      await page .evaluate (pageLoadURL, [ `${env .worlds}fallback/held-first.wrl` ]);
      await requested (env, "/hold/fc/", from);

      const second = await page .evaluate (url => document .querySelector ("x3d-canvas") .browser .loadURL (new window .X3D_NS .MFString (url)) .then (() => "resolved", e => e .message),
         `${env .worlds}fallback/parts/d.wrl`);

      env .server .release ("fc");
      await settle (page, 500);

      return {
         first: await race (page),
         second,
         world: await worldName (page),
         served: served (env, from),
         console: loadMessages (page .log, env),
      };
   },

   // dispose () while an Inline's second candidate (after one that did not parse) is in flight.
   async disposedDuringFallback (page, env)
   {
      env .server .arm ("fb");
      page .log .length = 0;

      const from = env .server .requests .length;

      await page .evaluate (pageLoadURL, [ `${env .worlds}fallback/held.wrl` ]);
      await requested (env, "/hold/fb/", from);
      await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .dispose ());
      env .server .release ("fb");
      await page .waitForTimeout (500);

      return {
         result: await race (page),
         served: served (env, from),
         errors: page .log .filter (line => /pageerror/ .test (line)),
      };
   },
};

export const CASE_GROUPS = {
   fallback: { context: DESKTOP, cases: fallbackCases },
};
