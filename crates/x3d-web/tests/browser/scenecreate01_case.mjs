// The scene creation 01 cases (issue #134): X3DBrowser.createScene, createVrmlFromString, createX3DFromURL (promise form
// and the callback form), createVrmlFromURL, importJS and importDocument: arguments, results, errors and messages.
// oracle/x_ite-web-runtime-02.mjs (--suite scenecreate01) runs them against pinned X_ITE 16.5.0 and records the values;
// scenecreate01.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).
//
// Every value is a string, a number or a plain list so both engines compare equal. `Name: message` is an error.

import { DESKTOP } from "./runtime02_case.mjs";
import { served } from "./loadurl01_case.mjs";

const W = "/tests/browser/worlds/fallback/parts/w.wrl";
const WORLDS = "/tests/browser/worlds/";

// The page side: `window .scenario (name, base)` runs one scenario and returns its record. `base` is the server's origin.
async function install (page, base)
{
   await page .evaluate (base =>
   {
      window .base = base;

      window .attempt = fn =>
      {
         try
         {
            return fn ();
         }
         catch (error)
         {
            return window .short (`${error ?.constructor ?.name}: ${error ?.message}`);
         }
      };

      // The result of a promise, or its rejection as `Name: message`.
      window .settled = promise => Promise .resolve (promise) .then (value => value, error => window .short (`${error ?.constructor ?.name}: ${error ?.message}`));

      // The URLs without the server's origin.
      window .short = text => String (text) .replaceAll (location .href, "PAGE") .replaceAll (location .href .replace (/[^/]*$/, "") + "null", "PAGEDIR/null") .replaceAll (window .base, "http://test");
   },
   base);
}

// One scenario per page. The console is captured into `logs` (one line each, origin removed).
async function scenario (page, env, name, args = { })
{
   await install (page, env .base);
   page .log .length = 0;

   const from = env .server .requests .length;
   const out  = await page .evaluate (async ([ name, args ]) =>
   {
      const X3D     = window .X3D_NS;
      const attempt = window .attempt;
      const settled = window .settled;
      const short   = window .short;
      const browser = X3D .getBrowser (document .querySelector ("x3d-canvas"));
      const frame   = () => new Promise (r => requestAnimationFrame (() => r ()));
      const spin    = async n => { for (let i = 0; i < n; ++ i) await frame (); };
      const logs    = [ ];
      const warn    = console .warn, error = console .error;

      console .warn  = (... a) => logs .push (`warn: ${short (a .join (" ") .split ("\n") [0])}`);
      console .error = (... a) => logs .push (`error: ${short (a .join (" ") .split ("\n") [0])}`);

      const url   = (... list) => new X3D .MFString (... list .map (u => u .startsWith ("/") ? `${window .base}${u}` : u));
      const types = nodes => Array .from (nodes, n => n ? n .getNodeTypeName () : null);
      const info  = scene => scene instanceof Object ?
      ({
         ctor: scene .constructor ?.name,
         worldURL: short (scene .worldURL),
         roots: types (scene .rootNodes),
         isWorld: scene === browser .currentScene,
      })
      : scene;
      const out = { };

      try
      {
         switch (name)
         {
            case "createScene":
            {
               const world = browser .currentScene;
               const p     = browser .createScene ();

               out .promise = [ p instanceof Promise, p .constructor .name ];

               const scene = await p;
               const other = await browser .createScene ();

               out .scene   = info (scene);
               out .distinct = [ scene !== other, scene !== world, browser .currentScene === world ];
               out .keys    = [ Object .keys (scene) .length, typeof scene .getWorldURL, scene .getWorldURL ?.() === scene .worldURL ];

               out .replace = await settled (browser .replaceWorld (scene) .then (() => "resolved"));
               out .after   = [ browser .currentScene === scene, short (browser .currentScene .worldURL), browser .currentScene .rootNodes .length ];

               // The replaced world's scene object stays usable.
               out .old = [ world .rootNodes .length, short (world .worldURL) ];
               break;
            }
            case "createSceneErrors":
            {
               // Any argument takes the profile branch (`arguments .length`).
               const bad = [ [ 1 ], [ null ], [ undefined ], [ "Full" ], [ { } ], [ [ ] ], [ new X3D .SFNode () ], [ "Full", 1 ], [ undefined, undefined ] ];

               out .errors = [ ];

               for (const args of bad)
                  out .errors .push (await settled (browser .createScene (... args)));

               out .afterWorld = browser .currentScene .rootNodes .length;
               break;
            }
            case "createSceneProfile":
            {
               // Needs ProfileInfo and ComponentInfo: browser .getProfile and getComponent.
               if (typeof browser .getProfile !== "function" || typeof browser .getComponent !== "function")
               {
                  out .result = "no getProfile";
                  break;
               }

               const profile   = browser .getProfile ("Interchange");
               const component = browser .getComponent ("Text", 1);
               const result    = { profile: [ profile .constructor .name, profile .name ] };
               const list      = scene => [ scene .profile ?.name, Array .from (scene .components, c => `${c .name}:${c .level}`) ];

               result .scene        = list (await browser .createScene (profile));
               result .scene2       = list (await browser .createScene (profile, component));
               result .badComponent = await settled (browser .createScene (profile, "Text"));
               result .badProfile   = await settled (browser .createScene (component));
               out .result = result;
               break;
            }
            case "vrmlString":
            {
               const world = browser .currentScene;
               const texts =
               [
                  "Transform { translation 1 2 3 children Shape { geometry Box { } } }",
                  "DEF A Transform { } USE A",
                  "PROTO P [ field SFFloat f 1 ] { Transform { } } P { }",
                  "Group { } Transform { } Shape { }",
                  "",
                  "   \n  ",
                  "# a comment",
                  "DEF A Transform { } DEF B Transform { } ROUTE A.translation_changed TO B.set_translation",
                  "#X3D V4.0 utf8\nTransform { }",
                  "WorldInfo { info [ \"äö\" ] }",
               ];

               out .results = [ ];

               for (const text of texts)
               {
                  const result = attempt (() => browser .createVrmlFromString (text));

                  out .results .push (typeof result === "string" ? result :
                  [
                     result .constructor .name,
                     result instanceof X3D .MFNode,
                     result .length,
                     types (result),
                     Array .from (result, n => n ?.getNodeName ()),
                  ]);
               }

               // The nodes are usable.
               const nodes = browser .createVrmlFromString ("DEF T Transform { translation 1 2 3 }");

               out .usable = [ String (nodes [0] .translation), nodes [0] .getNodeName () ];
               nodes [0] .translation = new X3D .SFVec3f (4, 5, 6);
               out .set    = String (nodes [0] .translation);

               const again = browser .createVrmlFromString ("DEF T Transform { translation 1 2 3 }");

               out .distinct = [ again [0] !== nodes [0], String (again [0] .translation) ];
               out .world    = [ browser .currentScene === world, browser .currentScene .rootNodes .length ];
               out .isNamed  = attempt (() => String (browser .currentScene .getNamedNode ("T")));
               break;
            }
            case "vrmlStringErrors":
            {
               const world = browser .currentScene;
               const texts = [ "Foo { }", "Transform {", "Transform { foo 1 }", "Transform { translation a }", 5, null, undefined, { }, "USE Nope", "ROUTE A.x TO B.y", "PROTO P [ ] { }  Q { }" ];

               out .results = [ ];

               for (const text of texts)
               {
                  logs .length = 0;

                  const result = attempt (() => browser .createVrmlFromString (text));

                  out .results .push ([ typeof result === "string" ? result : [ "returned", result .length ], logs .slice () ]);
               }

               logs .length = 0;
               out .world = [ browser .currentScene === world, browser .currentScene .rootNodes .length ];
               break;
            }
            case "vrmlStringWorld":
            {
               // The root nodes into a node of the world (the world is the page's empty one: a Group from a loaded scene).
               await settled (browser .loadURL (url ("data:model/x3d-vrml," + encodeURIComponent ("#X3D V4.0 utf8\nPROFILE Interchange\nDEF G Group { }\n"))));
               await spin (2);

               const G     = browser .currentScene .getNamedNode ("G");
               const nodes = browser .createVrmlFromString ("DEF N Transform { translation 1 0 0 } Transform { translation 2 0 0 }");

               out .assign = attempt (() => { G .children = nodes; return G .children .length; });
               out .types  = attempt (() => types (G .children));
               out .push   = attempt (() => { G .children .push (browser .createVrmlFromString ("Shape { }") [0]); return G .children .length; });
               out .add    = attempt (() => { G .addChildren = browser .createVrmlFromString ("Group { }"); return G .children .length; });
               out .world  = browser .currentScene .rootNodes .length;
               break;
            }
            case "x3dURL":
            {
               const world   = browser .currentScene;
               const worldN  = world .rootNodes .length;
               const results = [ ];
               const lists   =
               {
                  file: [ `${args .worlds}viewpoints.wrl` ],
                  hash: [ `${args .worlds}viewpoints.wrl#Two` ],
                  fallback: [ "/status/404/s1.wrl", `${args .worlds}viewpoints.wrl` ],
                  second: [ `${args .w}`, `${args .worlds}viewpoints.wrl` ],
                  emptyThen: [ "", `${args .worlds}viewpoints.wrl` ],
                  extern: [ `${args .worlds}extern.wrl` ],
                  data: [ "data:model/x3d-vrml," + encodeURIComponent ("#X3D V4.0 utf8\nPROFILE Interchange\nDEF A Transform { }\nShape { geometry Box { } }\n") ],
                  dataVrml: [ "data:model/vrml," + encodeURIComponent ("#VRML V2.0 utf8\nDEF V Viewpoint { }\n") ],
                  base64: [ "data:model/x3d-vrml;base64," + btoa ("#X3D V4.0 utf8\nPROFILE Interchange\nDEF B Transform { }\n") ],
               };

               for (const [ key, list ] of Object .entries (lists))
               {
                  logs .length = 0;

                  const p      = browser .createX3DFromURL (url (... list));
                  const result = await settled (p);

                  results .push ([ key, p instanceof Promise, info (result), result instanceof Object ? Array .from (
                     [ "A", "B", "V", "One", "Two", "Instance" ], name => attempt (() => String (result .getNamedNode (name) .getNodeTypeName ()))) : null, logs .slice () ]);
               }

               out .results = results;
               out .world   = [ browser .currentScene === world, browser .currentScene .rootNodes .length === worldN ];
               break;
            }
            case "x3dURLNodes":
            {
               // The scene's nodes are alive: a field set, the named node, a PROTO instance of an EXTERNPROTO.
               const scene = await browser .createX3DFromURL (url (`${args .worlds}extern.wrl`));
               const inst  = scene .getNamedNode ("Instance");

               out .inst   = [ inst .getNodeTypeName (), String (inst .color) ];
               inst .color = new X3D .SFColor (1, 0, 0);
               out .set    = String (inst .color);
               out .roots  = types (scene .rootNodes);
               break;
            }
            case "x3dURLErrors":
            {
               const world = browser .currentScene;
               const lists =
               {
                  one404: [ "/status/404/e1.wrl" ],
                  garbage: [ "/garbage/e2.wrl" ],
                  all: [ "/status/404/e3.wrl", "/garbage/e4.wrl", "/status/500/e5.wrl" ],
                  none: [ ],
                  empty: [ "" ],
                  emptyTwo: [ "", "" ],
                  hash404: [ "/status/404/e6.wrl#Fragment" ],
                  scriptURL: [ "javascript:Browser.print(1)" ],
               };

               out .results = [ ];

               for (const [ key, list ] of Object .entries (lists))
               {
                  logs .length = 0;
                  out .results .push ([ key, await settled (browser .createX3DFromURL (url (... list))), logs .slice () ]);
               }

               out .world = [ browser .currentScene === world ];
               break;
            }
            case "x3dURLArguments":
            {
               // Wrong arguments throw before a promise exists.
               const bad = [ [ ], [ undefined ], [ null ], [ "a.wrl" ], [ [ "a.wrl" ] ], [ new X3D .MFNode () ], [ { } ], [ 5 ] ];

               out .errors = bad .map (args => attempt (() => { const r = browser .createX3DFromURL (... args); return r instanceof Promise ? "promise" : typeof r; }));

               // Two arguments are the promise form; the second is ignored.
               const two = browser .createX3DFromURL (url (`${args .worlds}viewpoints.wrl`), "ignored");

               out .two = [ two instanceof Promise, info (await settled (two)) ];

               // Four or more: the promise form too.
               const four = browser .createX3DFromURL (url (`${args .worlds}viewpoints.wrl`), 1, 2, 3);

               out .four = [ four instanceof Promise, info (await settled (four)) ];
               break;
            }
            case "vrmlURLArguments":
            {
               const world = await settled (browser .loadURL (url ("data:model/x3d-vrml," + encodeURIComponent ("#X3D V4.0 utf8\nPROFILE Interchange\nDEF G Group { }\nDEF T Transform { }\n"))));

               await spin (2);

               const G    = browser .currentScene .getNamedNode ("G");
               const T    = browser .currentScene .getNamedNode ("T");
               const good = url (`${args .worlds}viewpoints.wrl`);
               const call = (name, ... a) => attempt (() => { const r = browser [name] (... a); return r === undefined ? "undefined" : typeof r; });

               out .world = world;
               out .createX3DFromURL = [
                  call ("createX3DFromURL", "a.wrl", G, "addChildren"),
                  call ("createX3DFromURL", good, null, "addChildren"),
                  call ("createX3DFromURL", good, { }, "addChildren"),
                  call ("createX3DFromURL", good, 5, "addChildren"),
                  call ("createX3DFromURL", good, G, "nope"),
                  call ("createX3DFromURL", good, G, "bboxSize"),
                  call ("createX3DFromURL", good, T, "translation"),
                  call ("createX3DFromURL", good, T, "children_changed"),
                  call ("createX3DFromURL", good, G, undefined),
                  call ("createX3DFromURL", good, G, "children_changed"),
                  call ("createX3DFromURL", good, G, "set_children"),
               ];
               out .createVrmlFromURL = [
                  call ("createVrmlFromURL"),
                  call ("createVrmlFromURL", good),
                  call ("createVrmlFromURL", good, G),
                  call ("createVrmlFromURL", "a.wrl", G, "addChildren"),
                  call ("createVrmlFromURL", good, G, "nope"),
                  call ("createVrmlFromURL", good, G, "children_changed"),
                  call ("createVrmlFromURL", good, G, "bboxSize"),
                  call ("createVrmlFromURL", good, T, "translation"),
                  call ("createVrmlFromURL", good, T, "set_translation"),
                  call ("createVrmlFromURL", good, T, "addChildren"),
                  call ("createVrmlFromURL", new X3D .MFString (), G, "addChildren"),
               ];
               await spin (2);
               break;
            }
            case "vrmlURLLoad":
            {
               // The callback form: the scene's root nodes are written to the field after the scene loaded.
               await settled (browser .loadURL (url ("data:model/x3d-vrml," + encodeURIComponent ("#X3D V4.0 utf8\nPROFILE Interchange\nDEF G Group { }\n"))));
               await spin (2);

               const G      = browser .currentScene .getNamedNode ("G");
               const events = [ ];

               G .getField ("children") .addFieldCallback ("t", v => events .push (`children:${v .length}`));

               const r = browser .createX3DFromURL (url (`${args .worlds}viewpoints.wrl`), G, "addChildren");

               out .returned = [ typeof r ];
               out .sync     = G .children .length;

               for (let i = 0; i < 60 && G .children .length === 0; ++ i)
                  await new Promise (r => setTimeout (r, 25));

               await spin (3);
               out .children = [ G .children .length, types (G .children) ];
               out .events   = events;

               // createVrmlFromURL is the same.
               logs .length = 0;
               browser .createVrmlFromURL (url (`${args .worlds}viewpoints.wrl`), G, "addChildren");

               for (let i = 0; i < 60 && G .children .length === out .children [0]; ++ i)
                  await new Promise (r => setTimeout (r, 25));

               await spin (3);
               out .second = G .children .length;

               // A failed load adds nothing.
               browser .createX3DFromURL (url ("/status/404/v1.wrl"), G, "addChildren");
               await new Promise (r => setTimeout (r, 500));
               await spin (2);
               out .failed = G .children .length;
               break;
            }
            case "importJS":
            {
               const json  = { X3D: { "@profile": "Interchange", "@version": "4.0", Scene: { "-children": [ { Transform: { "@DEF": "A", "@translation": [ 1, 2, 3 ] } }, { Shape: { "-geometry": { Box: { } } } } ] } } };
               const world = browser .currentScene;
               const inputs =
               {
                  object: json,
                  string: JSON .stringify (json),
                  empty: { X3D: { "@profile": "Interchange", "@version": "4.0", Scene: { } } },
                  noProfile: { X3D: { Scene: { "-children": [ { Group: { } } ] } } },
               };

               out .results = [ ];

               for (const [ key, input ] of Object .entries (inputs))
               {
                  logs .length = 0;

                  const p      = browser .importJS (input);
                  const result = await settled (p);

                  out .results .push ([ key, p instanceof Promise, info (result), result instanceof Object ? attempt (() => String (result .getNamedNode ("A") .translation)) : null, logs .slice () ]);
               }

               // The scene is not the world.
               out .world = [ browser .currentScene === world ];
               break;
            }
            case "importJSErrors":
            {
               const inputs = [ "{", "", "[ ]", "5", "null", "{ \"X3D\": 5 }", "{ \"Y\": { } }", { Y: { } }, { X3D: 5 }, 5, null, undefined ];

               out .results = [ ];

               for (const input of inputs)
               {
                  logs .length = 0;
                  out .results .push ([ await settled (browser .importJS (input) .then (info)), logs .slice () ]);
               }

               break;
            }
            case "importDocument":
            {
               const xml   = `<?xml version="1.0" encoding="UTF-8"?><X3D profile="Interchange" version="4.0"><Scene><Transform DEF="A" translation="1 2 3"/><Shape><Box/></Shape></Scene></X3D>`;
               const doc   = new DOMParser () .parseFromString (xml, "application/xml");
               const world = browser .currentScene;
               const inputs =
               {
                  string: xml,
                  document: doc,
                  element: doc .documentElement,
                  empty: `<X3D profile="Interchange" version="4.0"><Scene/></X3D>`,
               };

               out .results = [ ];

               for (const [ key, input ] of Object .entries (inputs))
               {
                  logs .length = 0;

                  const p      = browser .importDocument (input);
                  const result = await settled (p);

                  out .results .push ([ key, p instanceof Promise, info (result), result instanceof Object ? attempt (() => String (result .getNamedNode ("A") .translation)) : null, logs .slice () ]);
               }

               out .world = [ browser .currentScene === world ];
               break;
            }
            case "importDocumentErrors":
            {
               const inputs = [ "<X3D", "", "<Y/>", "5", 5, null, undefined, { }, `<X3D><Scene><Foo/></Scene></X3D>` ];

               out .results = [ ];

               for (const input of inputs)
               {
                  logs .length = 0;
                  out .results .push ([ await settled (browser .importDocument (input) .then (info)), logs .slice () ]);
               }

               break;
            }
            case "importDocumentHTML":
            {
               // An HTML element, as the X3D element of a page: the HTMLParser reads it.
               const div = document .createElement ("div");

               div .innerHTML = `<X3D profile="Interchange" version="4.0"><Scene><Transform DEF="A" translation="1 2 3"></Transform><Shape><Box></Box></Shape></Scene></X3D>`;

               out .result = await settled (browser .importDocument (div .firstElementChild) .then (info));
               break;
            }
         }
      }
      finally
      {
         console .warn  = warn;
         console .error = error;
      }

      out .logs = logs;

      return JSON .parse (JSON .stringify (out, (key, value) => value === undefined ? null : value));
   },
   [ name, { ... args, worlds: `${env .base}${WORLDS}`, w: `${env .base}${W}` } ]);

   if (name .startsWith ("x3dURL") || name .startsWith ("vrmlURL"))
      out .served = served (env, from);

   return out;
}

const NAMES =
[
   "createScene", "createSceneErrors", "createSceneProfile",
   "vrmlString", "vrmlStringErrors", "vrmlStringWorld",
   "x3dURL", "x3dURLNodes", "x3dURLErrors", "x3dURLArguments",
   "vrmlURLArguments", "vrmlURLLoad",
   "importJS", "importJSErrors", "importDocument", "importDocumentErrors", "importDocumentHTML",
];

const cases = Object .fromEntries (NAMES .map (name => [ name, (page, env) => scenario (page, env, name) ]));

export const CASE_GROUPS = {
   sceneCreate: { context: DESKTOP, cases },
};
