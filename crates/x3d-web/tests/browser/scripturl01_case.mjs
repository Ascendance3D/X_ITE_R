// The web script URL whitespace 01 cases: the characters before `ecmascript:` in a script URL (issue #127). The URL
// is taken as source text when FileLoader's /^\s*(?:ecmascript|javascript|vrmlscript):/s matches (JavaScript's \s: a
// leading U+FEFF, U+00A0, U+2028, U+3000 match; U+0085 and U+200B do not), and is fetched as a URL otherwise. The cases
// put each code point before `ecmascript:` in a Script node's url, the world's loadURL, an Inline and an EXTERNPROTO,
// each followed by a candidate that tells the two outcomes apart, with a no-space control and a U+0020 control.
// oracle/x_ite-web-runtime-02.mjs (--suite scripturl01) runs them against pinned X_ITE 16.5.0 and records the values;
// scripturl01.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { orderedMessages, pageLoadURL, sceneCase, worldName } from "./loadurl01_case.mjs";

// The prefixes: name → the characters before `ecmascript:`.
const PREFIXES = {
   none: "",
   space: " ",
   tab: "\u0009",
   feff: "﻿",
   nel: "\u0085",
   nbsp: " ",
   ls: " ",
   ideographicSpace: "　",
   emSpace: " ",
   zwsp: "​",
   mongolianVowelSeparator: "᠎",
};

// Requests whose path names a script URL, as the last path segment (the engines' pages are in different directories).
// The server logs decoded paths.
const scriptRequests = (env, from) => env .server .requests .slice (from)
   .map (r => r .path)
   .filter (path => path .includes ("ecmascript:"))
   .map (path => path .replace (/^.*\//, ""));

// Script, Inline and EXTERNPROTO: X_ITE_R's web build has no script engine and its Inline and EXTERNPROTO take a script URL
// as no scene (Rust fix of #127 changes only which URLs count as script URLs), so what the script did (`which`, the
// color, the second candidate's file, the messages) is X_ITE's alone: it is recorded under `detail`, which the harness
// does not compare (runtime02_harness.mjs). What both engines share is the classification: whether the URL was
// requested as a file (`requests`).

// The messages of a failed script URL, without the page directory in its resolved URL.
const messages = (page, env) => orderedMessages (page .log, env)
   .map (line => line .replace (/http:\/\/test\/(?:[^\s'"]*\/)?(?=[^\s'"\/]*ecmascript:)/g, "http://test/"))
   .map (line => line .replace (/'[^']*?ecmascript:[^']*'/g, "'<script url>'"));

// The text as code points, for the record of what was sent (the prefix is invisible).
const hex = text => Array .from (text, ch => ch .codePointAt (0) .toString (16) .padStart (4, "0")) .join (" ");

const BASE_W = "BASE/tests/browser/worlds/fallback/parts/w.wrl";
const TITLE  = '#VRML V2.0 utf8\nDEF Info WorldInfo { title "from script url" }\n';

// A Script node: the first candidate sets `which` to 1, the second to 2.
async function scriptCase (page, env, prefix)
{
   const from = env .server .requests .length;
   const c    = await sceneCase (page, env, `#VRML V2.0 utf8
DEF S Script
{
  field SFInt32 which 0
  url [ "${prefix}ecmascript: function initialize () { which = 1; }" "ecmascript: function initialize () { which = 2; }" ]
}
`);

   const loaded = await c .loaded ();
   const which  = await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("S") .which);

   return { prefix: hex (prefix), requests: scriptRequests (env, from), detail: { loaded, which, console: messages (page, env) } };
}

// A world-level loadURL whose first candidate is `prefix` + `ecmascript:` + a world, the second a test world.
async function worldCase (page, env, prefix)
{
   page .log .length = 0;

   const from = env .server .requests .length;

   await page .evaluate (pageLoadURL, [ `${prefix}ecmascript:${TITLE}`, `${env .base}/tests/browser/worlds/fallback/parts/w.wrl` ]);

   const result = await page .evaluate (() => Promise .race ([ window .pending, new Promise (r => setTimeout (() => r ("pending"), 5000)) ]));

   await settle (page, 300);

   const title = await page .evaluate (() =>
   {
      const scene = document .querySelector ("x3d-canvas") .browser .currentScene;

      try { return scene .getNamedNode ("Info") .title; } catch { return null; }
   });

   return { prefix: hex (prefix), result, world: await worldName (page), title, requests: scriptRequests (env, from), console: messages (page, env) };
}

// An Inline whose first candidate is `prefix` + `ecmascript:` + a world, the second a test world. The Inline's scene is
// not reachable from the API of both engines, so the outcome is whether the second candidate's file was requested.
async function inlineCase (page, env, prefix)
{
   const from = env .server .requests .length;
   const c    = await sceneCase (page, env, `#VRML V2.0 utf8
DEF I Inline { url [ "${prefix}ecmascript:${TITLE .replaceAll ('"', '\\"')}" "${BASE_W}" ] }
`);

   const loaded = await c .loaded ();

   return { prefix: hex (prefix), requests: scriptRequests (env, from), detail: { loaded, served: c .served (), console: messages (page, env) } };
}

// An EXTERNPROTO whose first candidate is `prefix` + `ecmascript:` + a PROTO, the second the PROTO file. The instance's
// color tells the candidates apart: 1 0 0 from the script URL, 0.9 0.5 0.1 from the file.
async function externCase (page, env, prefix)
{
   const from = env .server .requests .length;
   const c    = await sceneCase (page, env, `#VRML V2.0 utf8
EXTERNPROTO ColorBox [ exposedField SFColor color ]
[ "${prefix}ecmascript:#VRML V2.0 utf8
PROTO ColorBox [ exposedField SFColor color 1 0 0 ] { Shape { geometry Box { } } }
" "BASE/tests/browser/worlds/protos.wrl#ColorBox" ]
DEF Instance ColorBox { }
`);

   const loaded = await c .loaded ();
   const color  = await page .evaluate (() =>
   {
      try
      {
         const node = document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("Instance");

         return String (node .color) .replace (/-?\d+(?:\.\d+)?/g, n => String (Number (Number (n) .toFixed (2))));
      }
      catch { return null; }
   });

   return { prefix: hex (prefix), requests: scriptRequests (env, from), detail: { loaded, color, served: c .served (), console: messages (page, env) } };
}

const make = run => Object .fromEntries (Object .entries (PREFIXES) .map (([ name, prefix ]) => [ name, (page, env) => run (page, env, prefix) ]));

export const scriptCases = make (scriptCase);
export const worldCases  = make (worldCase);
export const inlineCases = make (inlineCase);
export const externCases = make (externCase);

export const CASE_GROUPS = {
   scriptURLScript: { context: DESKTOP, cases: scriptCases },
   scriptURLWorld: { context: DESKTOP, cases: worldCases },
   scriptURLInline: { context: DESKTOP, cases: inlineCases },
   scriptURLExtern: { context: DESKTOP, cases: externCases },
};
