// The web loadURL messages 01 cases: the candidates of the world-level loadURL (FileLoader of the X3DWorld: its
// messages, their order and the attempt numbers, abort and dispose while a candidate is in flight), and the loads
// that must wait for their responses: a FontStyle font file, a MovieTexture, and an AudioClip candidate after one
// that did not decode. oracle/x_ite-web-runtime-02.mjs (--suite loadurl01) runs them against pinned X_ITE 16.5.0 and
// records the values; loadurl01.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).

import { DESKTOP, settle } from "./runtime02_case.mjs";

// The paths the server served since `from`, without the engines' own files.
function served (env, from)
{
   return env .server .requests .slice (from)
      .map (r => r .path)
      .filter (path => /^\/(?:hold|status|garbage|fonts|media)\/|\/worlds\//.test (path));
}

// Console lines without the origins, and only load messages. Chrome's own "Failed to load resource" lines come first;
// the engine's lines keep their order: each case has one FileLoader chain, whose messages come in order.
function orderedMessages (log, env)
{
   const lines = log
      .map (line => line .replaceAll (env .base, "http://test"))
      .filter (line => /Couldn't|Failed to fetch|Not Found|Internal Server Error|Error:|Error loading|requested before|abort/i .test (line));
   const network = line => line .startsWith ("error: Failed to load resource:");

   return [ ... lines .filter (network), ... lines .filter (line => !network (line)) ];
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

// loadURL of `urls` in the page, with the browser events it fires; window .pending is "resolved" or the rejection's
// message.
function pageLoadURL (urls)
{
   const canvas  = document .querySelector ("x3d-canvas");
   const browser = canvas .browser;
   const names   = [ "initialized", "load", "error", "shutdown" ];

   window .events = [ ];
   names .forEach (name => canvas .addEventListener (name, event => window .events .push (event .type)));
   window .pending = browser .loadURL (new window .X3D_NS .MFString (... urls)) .then (() => "resolved", e => e .message);
}

const race = (page, ms = 300) => page .evaluate (ms => Promise .race ([ window .pending, new Promise (r => setTimeout (() => r ("pending"), ms)) ]), ms);

// The world's URL under the test worlds, or "page" for the empty world of the page (the engines' pages differ).
const worldName = page => page .evaluate (() =>
{
   const url = document .querySelector ("x3d-canvas") .browser .currentScene ?.worldURL ?? "";

   return url .includes ("/worlds/") ? url .replace (/^.*\/worlds\//, "") : "page";
});

// A world-level loadURL of `urls` (page-relative to the worlds); the result, the world, the browser events, the
// requests and the messages.
async function worldCase (page, env, urls)
{
   page .log .length = 0;

   const from = env .server .requests .length;

   await page .evaluate (pageLoadURL, urls .map (url => url .startsWith ("/") ? `${env .base}${url}` : url && `${env .worlds}${url}`));

   const result = await page .evaluate (() => window .pending);

   await settle (page, 300);

   return {
      result,
      world: await worldName (page),
      events: await page .evaluate (() => window .events),
      served: served (env, from),
      console: orderedMessages (page .log, env),
   };
}

// A world-level loadURL whose first candidate is held; `during (page, env)` runs while it is in flight, then the held
// response comes.
async function heldWorldCase (page, env, key, urls, during)
{
   env .server .arm (key);
   page .log .length = 0;

   const from = env .server .requests .length;

   await page .evaluate (pageLoadURL, urls .map (url => `${env .base}${url}`));
   await requested (env, `/hold/${key}/`, from);

   const out = await during (page, env);

   env .server .release (key);
   await page .waitForTimeout (500);

   return {
      ... out,
      first: await race (page),
      events: await page .evaluate (() => window .events),
      served: served (env, from),
      console: orderedMessages (page .log, env),
   };
}

// The world of `text` (createX3DFromString, replaceWorld), started without waiting: a held response can hold the
// scene's loadCount. `loaded ()` is "loaded" once the scene is the world, or "pending" after `ms` more milliseconds.
async function sceneCase (page, env, text, ms = 1000)
{
   page .log .length = 0;

   const from = env .server .requests .length;

   await page .evaluate (text =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;

      window .scenePending = browser .createX3DFromString (text)
         .then (scene => browser .replaceWorld (scene))
         .then (() => "loaded", e => e .message);
   },
   text .replaceAll ("BASE", env .base));

   const loaded = (ms = 5000) => page .evaluate (ms => Promise .race ([ window .scenePending, new Promise (r => setTimeout (() => r ("pending"), ms)) ]), ms);

   await loaded (ms);
   await settle (page, ms);

   return { loaded, served: () => served (env, from), console: () => orderedMessages (page .log, env) };
}

// A field of a named node of the world as text, numbers rounded to `digits` decimals.
const fieldText = (page, name, field, digits = 2) => page .evaluate (([ name, field, digits ]) =>
{
   const value = document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode (name) [field];

   return String (value) .replace (/-?\d+(?:\.\d+)?(?:e[-+]?\d+)?/gi, n => String (Number (Number (n) .toFixed (digits)) || 0));
},
[ name, field, digits ]);

const W = "/tests/browser/worlds/fallback/parts/w.wrl";
const D = "/tests/browser/worlds/fallback/parts/d.wrl";

export const loadURLCases = {
   // The first candidate loads; the second is never requested.
   firstLoads: (page, env) => worldCase (page, env, [ W, "/tests/browser/worlds/fallback/parts/never.wrl" ]),

   // A candidate that fails (404), then one that loads.
   failedThenLoads: (page, env) => worldCase (page, env, [ "/status/404/u1.wrl", W ]),

   // Several candidates fail in different ways (500, no file format, 404, the empty URL), then one loads.
   severalThenLoads: (page, env) => worldCase (page, env, [ "/status/500/u2.wrl", "/garbage/u3.wrl", "/status/404/u4.wrl", "", W ]),

   // The only candidate fails.
   oneFails: (page, env) => worldCase (page, env, [ "/status/404/u5.wrl" ]),

   // Every candidate fails.
   allFail: (page, env) => worldCase (page, env, [ "/status/404/u6.wrl", "/garbage/u7.wrl", "/status/500/u8.wrl" ]),

   // No candidate.
   noURL: (page, env) => worldCase (page, env, [ ]),

   // Only the empty URL: it fails before any request.
   emptyURL: (page, env) => worldCase (page, env, [ "" ]),

   // The empty URL, then one that loads.
   emptyThenLoads: (page, env) => worldCase (page, env, [ "", W ]),

   // A newer loadURL while the first candidate is in flight: FileLoader.abort ().
   replacedWhilePending: (page, env) => heldWorldCase (page, env, "la", [ `/hold/la${W}`, D ], async page =>
   ({
      second: await page .evaluate (url => document .querySelector ("x3d-canvas") .browser .loadURL (new window .X3D_NS .MFString (url)) .then (() => "resolved", e => e .message), `${env .base}${D}`),
      world: await worldName (page),
   })),

   // A newer loadURL while the second candidate is in flight, after the first failed.
   replacedAfterFailure: (page, env) => heldWorldCase (page, env, "lb", [ "/status/404/u9.wrl", `/hold/lb${W}`, D ], async page =>
   ({
      second: await page .evaluate (url => document .querySelector ("x3d-canvas") .browser .loadURL (new window .X3D_NS .MFString (url)) .then (() => "resolved", e => e .message), `${env .base}${W}`),
      world: await worldName (page),
   })),

   // replaceWorld (null) while the first candidate is in flight.
   replaceWorldWhilePending: (page, env) => heldWorldCase (page, env, "lc", [ `/hold/lc${W}`, D ], async page =>
   ({
      replaced: await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .replaceWorld (null) .then (() => "resolved", e => e .message)),
   })),

   // dispose () while the first candidate is in flight.
   disposedWhilePending: async (page, env) =>
   {
      const out = await heldWorldCase (page, env, "ld", [ `/hold/ld${W}`, D ], async page =>
      {
         await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .dispose ());
         return { };
      });

      // Without the page errors of a frame after dispose ().
      return { ... out, console: out .console .filter (line => !line .startsWith ("pageerror")) };
   },
};

const FONT_WORLD = family => `#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Text:1
Viewpoint { position 0 0 10 }
Shape { geometry DEF T Text { string "Hg" fontStyle FontStyle { family [ ${family .map (f => `"${f}"`) .join (" ")} ] } } }
`;

export const fontCases = {
   // A font file that is missing, one that is no font, then one that loads (X3DFontStyleNode.loadData family loop).
   async fallback (page, env)
   {
      const c = await sceneCase (page, env, FONT_WORLD ([ "BASE/status/404/f1.ttf", "BASE/fonts/notfont.ttf", "BASE/fonts/PTSans-Regular.ttf" ]));

      return { loaded: await c .loaded (), served: c .served (), console: c .console (), textBounds: await fieldText (page, "T", "textBounds") };
   },

   // The font file's response is held: the FontStyle waits for it.
   async pending (page, env)
   {
      env .server .arm ("fh");

      const c      = await sceneCase (page, env, FONT_WORLD ([ "BASE/hold/fh/fonts/PTSans-Regular.ttf" ]), 300);
      const before = { loaded: await c .loaded (0), console: c .console () };

      env .server .release ("fh");

      const loaded = await c .loaded ();

      await settle (page, 800);

      return { before, loaded, served: c .served (), console: c .console (), textBounds: await fieldText (page, "T", "textBounds") };
   },
};

const MOVIE_WORLD = url => `#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Texturing:3
Viewpoint { position 0 0 4 }
Shape { appearance Appearance { texture DEF M MovieTexture { url [ ${url .map (u => `"${u}"`) .join (" ")} ] } } geometry Box { } }
`;

export const movieCases = {
   // A GIF that is missing, a video that is missing, then a GIF that loads.
   async fallback (page, env)
   {
      const c = await sceneCase (page, env, MOVIE_WORLD ([ "BASE/status/404/m1.gif", "BASE/status/404/m2.webm", "BASE/tests/browser/worlds/tex/green.gif" ]));

      return { loaded: await c .loaded (), served: c .served (), console: c .console () };
   },

   // A video that downloads but does not play, then a GIF: the GIF is requested after the video failed.
   async notPlayable (page, env)
   {
      const c = await sceneCase (page, env, MOVIE_WORLD ([ "BASE/garbage/m4.avi", "BASE/tests/browser/worlds/tex/green.gif" ]));

      return { loaded: await c .loaded (), served: c .served (), console: c .console () };
   },

   // The only candidate is missing.
   async failed (page, env)
   {
      const c = await sceneCase (page, env, MOVIE_WORLD ([ "BASE/status/404/m3.gif" ]));

      return { loaded: await c .loaded (), served: c .served (), console: c .console () };
   },

   // The GIF's response is held: the MovieTexture waits for it.
   async pending (page, env)
   {
      env .server .arm ("mh");

      const c      = await sceneCase (page, env, MOVIE_WORLD ([ "BASE/hold/mh/tests/browser/worlds/tex/green.gif" ]), 300);
      const before = { loaded: await c .loaded (0), console: c .console () };

      env .server .release ("mh");

      const loaded = await c .loaded ();

      await settle (page, 800);

      return { before, loaded, served: c .served (), console: c .console () };
   },
};

const AUDIO_WORLD = url => `#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Sound:2
Sound { source DEF A AudioClip { url [ ${url .map (u => `"${u}"`) .join (" ")} ] } }
`;

export const audioCases = {
   // A file that does not decode, then one that does: the second is requested after the first decode failed.
   async decodeFallback (page, env)
   {
      const c = await sceneCase (page, env, AUDIO_WORLD ([ "BASE/garbage/a1.wav", "BASE/media/tone22k-mono.wav" ]));

      return { loaded: await c .loaded (), served: c .served (), console: c .console (), duration: await fieldText (page, "A", "duration_changed", 3) };
   },

   // The candidate after the one that did not decode is held.
   async decodeFallbackHeld (page, env)
   {
      env .server .arm ("ah");

      const c      = await sceneCase (page, env, AUDIO_WORLD ([ "BASE/garbage/a2.wav", "BASE/hold/ah/media/tone22k-mono.wav" ]), 500);
      const before = { loaded: await c .loaded (0), console: c .console () };

      env .server .release ("ah");

      const loaded = await c .loaded ();

      await settle (page, 800);

      return { before, loaded, served: c .served (), console: c .console (), duration: await fieldText (page, "A", "duration_changed", 3) };
   },

   // No candidate decodes.
   async allFail (page, env)
   {
      const c = await sceneCase (page, env, AUDIO_WORLD ([ "BASE/garbage/a3.wav", "BASE/garbage/a4.wav" ]));

      return { loaded: await c .loaded (), served: c .served (), console: c .console (), duration: await fieldText (page, "A", "duration_changed", 3) };
   },
};

export const CASE_GROUPS = {
   loadURL: { context: DESKTOP, cases: loadURLCases },
   font: { context: DESKTOP, cases: fontCases },
   movie: { context: DESKTOP, cases: movieCases },
   audio: { context: DESKTOP, cases: audioCases },
};
