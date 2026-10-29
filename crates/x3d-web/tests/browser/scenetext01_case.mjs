// The web scene text 01 cases (issue #136): X3DScene.toVRMLString, toXMLString and toJSONString of the scene that
// createX3DFromString returns and of the browser's currentScene after loadURL, for a set of worlds, and of the empty
// world of a canvas before any load. oracle/x_ite-web-runtime-02.mjs (--suite scenetext01) runs them against pinned
// X_ITE 16.5.0 and records the exact texts; scenetext01.test.mjs runs them against X_ITE_R and compares
// (runtime02_harness.mjs). Options of the text forms are not part of these cases (web/API.md).

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { pageLoadURL } from "./loadurl01_case.mjs";

const X3D = body => `#X3D V4.0 utf8\nPROFILE Interchange\n${body}`;

// The worlds: name → text. BASE/ is the test server's origin.
const WORLDS = {
   empty: X3D (""),
   box: X3D ("Shape { appearance Appearance { material Material { diffuseColor 1 0 0 } } geometry Box { size 1 2 3 } }\n"),
   defuse: X3D (`DEF T Transform {
   translation 1 2 3
   children [
      DEF S Shape { appearance Appearance { material DEF M Material { diffuseColor 0.2 0.4 0.9 } } geometry Box { } }
      USE S
   ]
}
Transform { translation 4 0 0 children USE S }
`),
   proto: X3D (`PROTO Pair [ exposedField SFColor color 1 0 0 field MFString names [ "a" "b" ] inputOnly SFBool on outputOnly SFFloat out ]
{
   Shape { appearance Appearance { material Material { diffuseColor IS color } } geometry Box { } }
}
DEF P Pair { color 0 1 0 }
Pair { }
`),
   externproto: X3D (`EXTERNPROTO ColorBox [ exposedField SFColor color ] [ "BASE/tests/browser/worlds/protos.wrl#ColorBox" "protos.wrl#ColorBox" ]
DEF E ColorBox { color 0 0 1 }
`),
   routes: X3D (`DEF TS TimeSensor { cycleInterval 2 enabled FALSE }
DEF PI PositionInterpolator { key [ 0 1 ] keyValue [ 0 0 0, 1 1 1 ] }
DEF T Transform { children Shape { geometry Sphere { } } }
ROUTE TS.fraction_changed TO PI.set_fraction
ROUTE PI.value_changed TO T.set_translation
`),
   meta: `#X3D V4.0 utf8
PROFILE Interchange
META "title" "scene text"
META "creator" "Ryan"
WorldInfo { title "A \\"quoted\\" title" info [ "one" "two" ] }
DEF W WorldInfo { metadata MetadataString { name "n" value [ "v1" "v2" ] } metadata MetadataInteger { name "i" value [ 1 2 3 ] } }
`,
   exportOnly: X3D (`DEF T Transform { }
DEF G Group { }
EXPORT T
EXPORT G AS Gx
`),
   immersive: `#X3D V4.0 utf8
PROFILE Immersive
COMPONENT Navigation : 2
COMPONENT Scripting : 1
META "k" "v"
Group { }
`,
   components: `#X3D V4.0 utf8
PROFILE Interactive
COMPONENT Lighting : 3
COMPONENT Navigation : 2
UNIT length millimeters 0.001
UNIT angle degrees 0.0174532925199433
UNIT force newtons 1
UNIT mass grams 0.001
DirectionalLight { }
NavigationInfo { speed 2 }
`,
   units: X3D (`UNIT length feet 0.3048
Transform { translation 1 2 3 }
`),
   script: `#X3D V4.0 utf8
PROFILE Immersive
` + (`DEF S Script
{
   url "ecmascript:
function initialize () { }
function set_a (value) { b = value * 2; }
"
   inputOnly SFFloat a
   outputOnly SFFloat b
   initializeOnly SFInt32 n 3
   inputOutput MFString names [ "x" ]
   directOutput TRUE
}
`),
   inline: X3D (`DEF I Inline { url [ "BASE/tests/browser/worlds/fallback/parts/w.wrl" "w.wrl" ] load FALSE bboxSize 1 1 1 }
`),
   nonAscii: `#X3D V4.0 utf8
PROFILE Immersive
` + (`WorldInfo { title "café über 日本語 😀" info [ "é" ] }
DEF É Group { }
Shape { geometry Text { string [ "äöü" "Ω" ] } }
`),
   vrml97: `#VRML V2.0 utf8
DEF T Transform { children Shape { appearance Appearance { material Material { } } geometry Cone { } } }
`,
   xml: `<?xml version="1.0" encoding="UTF-8"?>
<X3D profile="Immersive" version="4.0">
<head><meta name="title" content="xml world"/></head>
<Scene><Transform DEF="T"><Shape><Box size="2 2 2"/></Shape></Transform></Scene>
</X3D>`,
};

// The text with the test server's origin (a port that changes) as http://test.
const plain = (text, env) => typeof text === "string" ? text .replaceAll (env .base, "http://test") : text .map (item => plain (item, env));

// Run in the page: the three text forms of the scene `scene` (an error becomes `Name: message`).
function forms (scene)
{
   return [ "toVRMLString", "toXMLString", "toJSONString" ] .map (method =>
   {
      try { return scene [method] (); }
      catch (error) { return `${error ?.constructor ?.name}: ${error ?.message}`; }
   });
}

// The world `name`: the text forms of the createX3DFromString scene, then of the currentScene after loadURL of the
// world as a data URL (`load`).
async function worldCase (page, env, name)
{
   const text   = WORLDS [name] .replaceAll ('"BASE/', `"${env .base}/`);
   const result = { };

   result .create = plain (await page .evaluate (async ([ text, source ]) =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;

      try
      {
         const scene = await browser .createX3DFromString (text);

         return new Function ("scene", `return (${source}) (scene)`) (scene);
      }
      catch (error)
      {
         return `${error ?.constructor ?.name}: ${error ?.message}`;
      }
   },
   [ text, forms .toString () ]), env);

   await page .evaluate (pageLoadURL, [ `data:model/${name === "xml" ? "x3d+xml" : "x3d-vrml"},${encodeURIComponent (text)}` ]);

   const loaded = await page .evaluate (() => window .pending);

   await settle (page, 200);

   result .load = loaded === "resolved"
      ? plain (await page .evaluate (source => new Function ("scene", `return (${source}) (scene)`) (document .querySelector ("x3d-canvas") .browser .currentScene), forms .toString ()), env)
      : loaded;

   return result;
}

// The empty world of a canvas before any load: currentScene's text forms, its root nodes, and the same after
// replaceWorld (null).
async function initialCase (page)
{
   const source = forms .toString ();

   return page .evaluate (async source =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const run     = scene => new Function ("scene", `return (${source}) (scene)`) (scene);
      const out     = { };

      out .initial = run (browser .currentScene);
      out .roots   = browser .currentScene .rootNodes .length;

      await browser .replaceWorld (null);
      out .afterNull = run (browser .currentScene);

      return out;
   },
   source);
}

export const worldCases = Object .fromEntries (Object .keys (WORLDS) .map (name => [ name, (page, env) => worldCase (page, env, name) ]));

export const CASE_GROUPS = {
   sceneText: { context: DESKTOP, cases: worldCases },
   sceneTextInitial: { context: DESKTOP, cases: { initial: initialCase } },
};
