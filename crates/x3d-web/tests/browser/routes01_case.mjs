// The routes 01 cases (issue #135 part 1): the route API of X3DExecutionContext / X3DScene and X3DBrowser: addRoute, deleteRoute,
// getRoute, routes and the X3DRoute object, with events over an added route, the errors of bad arguments and the routes
// between two scenes.
// oracle/x_ite-web-runtime-02.mjs (--suite routes01) runs them against pinned X_ITE 16.5.0 and records the values;
// routes01.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).
//
// Every value is a string, a number or a plain list so both engines compare equal. Each scenario loads a fresh page of the
// world below: A -> B is a parsed ROUTE; C, D and E are loose.

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { pageLoadURL } from "./loadurl01_case.mjs";

const WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Interchange
DEF A Transform { }
DEF B Transform { }
DEF C Transform { }
DEF D Transform { }
DEF E Transform { }
DEF G Group { }
DEF S Shape { geometry DEF X Box { } }
DEF PI PositionInterpolator { key [ 0 1 ] keyValue [ 0 0 0, 1 1 1 ] }
ROUTE A.translation_changed TO B.set_translation
`)}`;

// Load the world; resolve with "resolved" or the error.
async function load (page)
{
   await page .evaluate (pageLoadURL, [ WORLD ]);

   const result = await page .evaluate (() => window .pending);

   if (result !== "resolved")
      throw new Error (`the world did not load: ${result}\n${page .log .join ("\n")}`);

   await settle (page, 100);
}

// The result of `fn`, or its error as `Name: message`; the same text in both engines.
function pageAttempt ()
{
   window .attempt = fn =>
   {
      try
      {
         return fn ();
      }
      catch (error)
      {
         return `${error ?.constructor ?.name}: ${error ?.message}`;
      }
   };
}

// One scenario per page. `out` is the record; the console is captured into `logs` (one line each).
async function scenario (page, name)
{
   await load (page);

   return page .evaluate (async name =>
   {
      const X3D     = window .X3D_NS;
      const attempt = window .attempt;
      const browser = X3D .getBrowser (document .querySelector ("x3d-canvas"));
      const scene   = browser .currentScene;
      const node    = n => scene .getNamedNode (n);
      const [ A, B, C, D, E, G, S, X, PI ] = [ "A", "B", "C", "D", "E", "G", "S", "X", "PI" ] .map (node);
      const frame   = () => new Promise (r => requestAnimationFrame (() => r ()));
      const logs    = [ ];
      const warn    = console .warn, error = console .error;

      console .warn  = (... a) => logs .push (`warn: ${a .join (" ") .split ("\n") [0]}`);
      console .error = (... a) => logs .push (`error: ${a .join (" ") .split ("\n") [0]}`);

      // The route as its four public properties, the nodes by name.
      const desc = route => route ? [ route .sourceNode ?.getNodeName (), route .sourceField, route .destinationNode ?.getNodeName (), route .destinationField ] : route;
      const list = ctx => Array .from (ctx .routes, desc);
      const ends = route => attempt (() => desc (route));
      const setX = (n, x) => { n .translation = new X3D .SFVec3f (x, 0, 0); };
      const tr   = n => String (n .translation);
      const out  = { };

      let seq = [ ], frames = 0;

      const note = (key, v) => seq .push ([ key, String (v), frames ] .join ("|"));
      const spin = async n => { for (let i = 0; i < n; ++ i) { await frame (); ++ frames; } };
      const watch = (... nodes) => { for (const n of nodes) n .getField ("translation") .addFieldCallback (n .getNodeName (), v => note (n .getNodeName (), v)); };

      try
      {
         switch (name)
         {
            case "objects":
            {
               // The initial routes, the returned route and its properties.
               out .initial = [ scene .routes .length, list (scene), scene .routes .constructor ?.typeName ?? scene .routes .getTypeName ?.() ];
               out .parsed = [ scene .routes [0] .constructor .typeName, scene .routes [0] .getTypeName ?.(), typeof scene .routes [0] .getId ];

               const route = scene .addRoute (A, "translation", C, "translation");

               out .added = [ route ?.constructor ?.typeName, route ?.getTypeName ?.(), ends (route), typeof route ?.getId, typeof route ?.getSourceField ];
               out .keys = [ Object .keys (route), (() => { const k = [ ]; for (const key in route) k .push (key); return k; }) (), Object .keys (scene .routes), JSON .stringify (route), JSON .stringify (scene .routes) ];
               out .nodes = [ route .sourceNode === A, route .destinationNode === C, route .sourceNode .valueOf () === A .valueOf (), route .sourceNode instanceof X3D .SFNode, String (route .destinationNode .getNodeName ()) ];
               out .list = [ scene .routes .length, list (scene), scene .routes [1] === route, scene .routes [0] === route, scene .routes [5], 1 in scene .routes, 5 in scene .routes ];
               out .iterate = [ Array .from (scene .routes) .length, [ ... scene .routes ] .indexOf (route ), Object .keys (scene .routes) ];
               out .readonly = [ attempt (() => { scene .routes [0] = null; return scene .routes .length; }), attempt (() => { route .sourceField = "x"; return route .sourceField; }),
                                 attempt (() => { scene .routes = [ ]; return scene .routes .length; }) ];
               out .string = [ typeof String (route), attempt (() => route .toVRMLString ()), attempt (() => route .toXMLString ()), attempt (() => route .toJSONString ()) ];
               break;
            }
            case "flow":
            {
               // Events over an added route; deleteRoute (route) and the 4-argument form stop them; the route comes back.
               watch (B, C, D);
               out .parsedOnly = [ ];

               const r1 = scene .addRoute (A, "translation_changed", C, "set_translation");
               const r2 = scene .addRoute (C, "translation", D, "translation");

               setX (A, 1);
               out .sync = [ tr (A), tr (B), tr (C), tr (D) ];
               await spin (1); out .frame1 = [ tr (A), tr (B), tr (C), tr (D) ];
               await spin (3); out .frame4 = [ tr (A), tr (B), tr (C), tr (D) ];
               out .seq1 = seq .slice (); seq = [ ];

               scene .deleteRoute (r1);
               out .afterDelete = [ list (scene), r1 .sourceField, r1 .destinationField, ends (r1) ];

               setX (A, 2);
               await spin (4);
               out .seq2 = seq .slice (); seq = [ ];
               out .stopped = [ tr (A), tr (B), tr (C), tr (D) ];

               scene .deleteRoute (C, "translation_changed", D, "set_translation");
               out .afterDelete4 = list (scene);
               setX (C, 3);
               await spin (4);
               out .seq3 = seq .slice (); seq = [ ];
               out .stopped2 = [ tr (C), tr (D) ];

               const r3 = scene .addRoute (A, "translation", C, "translation");
               out .readded = [ r3 === r1, list (scene) ];
               setX (A, 4);
               await spin (4);
               out .seq4 = seq .slice (); seq = [ ];
               out .final = [ tr (B), tr (C) ];

               // The parsed route goes the same way.
               scene .deleteRoute (A, "translation_changed", B, "set_translation");
               setX (A, 5);
               await spin (4);
               out .parsedDeleted = [ list (scene), tr (B), tr (C) ];
               break;
            }
            case "order":
            {
               // Two routes from one source, one parsed and one added, in the order of the events of a parsed ROUTE:
               // the page's world has A -> B parsed; an added A -> C and A -> D, with a chain C -> E, are delivered in the
               // order they were added, and the parsed route's own order is kept.
               const doc = await browser .createX3DFromString (`#X3D V4.0 utf8
PROFILE Interchange
DEF A Transform { }
DEF B Transform { }
DEF C Transform { }
DEF D Transform { }
DEF E Transform { }
ROUTE A.translation_changed TO B.set_translation
ROUTE A.translation_changed TO C.set_translation
ROUTE A.translation_changed TO D.set_translation
ROUTE C.translation_changed TO E.set_translation
`);
               browser .replaceWorld (doc);
               for (let i = 0; i < 100 && browser .currentScene !== doc; ++ i) await new Promise (r => setTimeout (r, 100));
               await new Promise (r => setTimeout (r, 300));

               const named = n => doc .getNamedNode (n);

               watch (... [ "B", "C", "D", "E" ] .map (named));
               frames = 0;
               setX (named ("A"), 1);
               await spin (4);
               out .parsed = seq .slice (); seq = [ ];

               // The same graph made with addRoute, the routes added in the same order.
               const doc2 = await browser .createX3DFromString (`#X3D V4.0 utf8
PROFILE Interchange
DEF A Transform { }
DEF B Transform { }
DEF C Transform { }
DEF D Transform { }
DEF E Transform { }
`);
               browser .replaceWorld (doc2);
               for (let i = 0; i < 100 && browser .currentScene !== doc2; ++ i) await new Promise (r => setTimeout (r, 100));
               await new Promise (r => setTimeout (r, 300));

               const n2 = n => doc2 .getNamedNode (n);

               watch (... [ "B", "C", "D", "E" ] .map (n2));
               frames = 0;
               doc2 .addRoute (n2 ("A"), "translation_changed", n2 ("B"), "set_translation");
               doc2 .addRoute (n2 ("A"), "translation_changed", n2 ("C"), "set_translation");
               doc2 .addRoute (n2 ("A"), "translation_changed", n2 ("D"), "set_translation");
               doc2 .addRoute (n2 ("C"), "translation_changed", n2 ("E"), "set_translation");
               setX (n2 ("A"), 1);
               await spin (4);
               out .added = seq .slice (); seq = [ ];
               out .same = JSON .stringify (out .parsed) === JSON .stringify (out .added);
               break;
            }
            case "errors":
            {
               const N = { A, C, G, S, X };

               out .addNodes = [ undefined, null, 5, "A", { }, [ ], scene .routes, scene ]
                  .map (v => [ attempt (() => scene .addRoute (v, "translation", C, "translation")), attempt (() => scene .addRoute (A, "translation", v, "translation")) ]);
               out .addMissing = [ attempt (() => scene .addRoute ()), attempt (() => scene .addRoute (A)), attempt (() => scene .addRoute (A, "translation")), attempt (() => scene .addRoute (A, "translation", C)) ];
               out .addFields = [ "nope", "", undefined, null, 5, "Translation", "translation_changed_changed", "set_nope" ]
                  .map (v => [ attempt (() => String (desc (scene .addRoute (A, v, C, "translation")))), attempt (() => String (desc (scene .addRoute (A, "translation", C, v)))) ]);
               out .routesAfterFields = list (scene);
               out .types = [ attempt (() => scene .addRoute (A, "translation", C, "rotation")),
                              attempt (() => scene .addRoute (A, "rotation", C, "translation")),
                              attempt (() => scene .addRoute (A, "children", G, "children")),
                              attempt (() => scene .addRoute (G, "children_changed", S, "geometry")),
                              attempt (() => scene .addRoute (S, "geometry", G, "children")) ];
               out .access = [ attempt (() => scene .addRoute (A, "addChildren", C, "translation")),
                               attempt (() => scene .addRoute (A, "translation", C, "translation_changed")),
                               attempt (() => scene .addRoute (A, "translation", C, "bboxSize")),
                               attempt (() => scene .addRoute (A, "bboxSize", C, "bboxSize")),
                               attempt (() => scene .addRoute (G, "addChildren", G, "removeChildren")),
                               attempt (() => scene .addRoute (G, "children_changed", G, "addChildren") .destinationField),
                               attempt (() => scene .addRoute (PI, "value_changed", A, "set_translation") .sourceField),
                               attempt (() => scene .addRoute (PI, "value_changed", PI, "set_fraction")) ];
               out .routesAfterAccess = list (scene);
               out .extra = attempt (() => desc (scene .addRoute (A, "translation", D, "translation", "extra")));

               out .getNodes = [ undefined, null, 5, "A", { } ]
                  .map (v => [ attempt (() => scene .getRoute (v, "translation", C, "translation")), attempt (() => scene .getRoute (A, "translation", v, "translation")) ]);
               out .getFields = [ attempt (() => scene .getRoute (A, "nope", C, "translation")), attempt (() => scene .getRoute (A, "translation", C, "nope")),
                                  attempt (() => scene .getRoute (A, "translation", C, "rotation")), attempt (() => scene .getRoute (A, "addChildren", C, "translation")),
                                  attempt (() => scene .getRoute (A, "translation", C, "translation_changed")), attempt (() => scene .getRoute ()), attempt (() => scene .getRoute (A, "translation", C)) ];
               out .getFound = [ desc (scene .getRoute (A, "translation", B, "translation")), desc (scene .getRoute (A, "translation_changed", B, "set_translation")),
                                 scene .getRoute (A, "translation", C, "translation"), scene .getRoute (B, "translation", A, "translation") ];

               out .deleteBad = [ undefined, null, 5, "A", { }, A, scene .routes ]
                  .map (v => attempt (() => scene .deleteRoute (v)));
               out .deleteFour = [ attempt (() => scene .deleteRoute (A, "nope", C, "translation")), attempt (() => scene .deleteRoute (A, "translation", C, "nope")),
                                   attempt (() => scene .deleteRoute (5, "translation", C, "translation")), attempt (() => scene .deleteRoute (A, "translation", null, "translation")),
                                   attempt (() => scene .deleteRoute (A, "translation", C, "rotation")), attempt (() => scene .deleteRoute (A, "translation", C, "translation")),
                                   attempt (() => scene .deleteRoute (A, "translation", B, "translation")) ];
               out .deleteThree = attempt (() => scene .deleteRoute (A, "translation", B));
               out .deleteTwo = attempt (() => scene .deleteRoute (scene .routes [0], "extra"));
               out .routesAfterDelete = list (scene);

               const r = scene .addRoute (A, "translation", D, "translation");
               out .deleteTwice = [ scene .deleteRoute (r), scene .deleteRoute (r), list (scene) ];
               out .logs = logs .length;
               out .N = Object .keys (N);
               break;
            }
            case "names":
            {
               // Which field names reach which field, and what the route calls them.
               const variants = [ [ "translation", "translation" ], [ "translation_changed", "translation" ], [ "translation", "set_translation" ],
                                  [ "translation_changed", "set_translation" ], [ "set_translation", "translation" ], [ "translation", "translation_changed" ],
                                  [ "set_translation", "set_translation" ] ];
               out .variants = variants .map (([ s, d ]) => attempt (() =>
               {
                  const route = scene .addRoute (D, s, E, d);
                  const result = [ ends (route), list (scene) .length ];
                  scene .deleteRoute (route);
                  return result;
               }));
               out .after = list (scene);

               // The same route given in the other spellings is the same route.
               const first = scene .addRoute (D, "translation", E, "translation");
               out .same = [ "translation_changed", "set_translation" ] .map (n => [ scene .addRoute (D, n === "set_translation" ? "translation" : n, E, n === "translation_changed" ? "translation" : n) === first ]);
               out .same2 = [ scene .addRoute (D, "translation_changed", E, "set_translation") === first, scene .getRoute (D, "translation", E, "translation") === first, list (scene) .length ];
               // A non-string field name is made a string.
               out .stringified = attempt (() => ends (scene .addRoute (A, { toString () { return "translation"; } }, C, [ "translation" ])));
               // An interpolator and an inputOutput target.
               out .interpolator = attempt (() => ends (scene .addRoute (PI, "value_changed", D, "translation")));
               out .routes = list (scene);
               break;
            }
            case "scenes":
            {
               // Two scenes, each with routes of its own: the scene `other` is not the world.
               const other = await browser .createX3DFromString ("#X3D V4.0 utf8\nPROFILE Interchange\nDEF OA Transform { }\nDEF OB Transform { }\nROUTE OA.translation_changed TO OB.set_translation\n");
               const OA = other .getNamedNode ("OA"), OB = other .getNamedNode ("OB");

               out .before = [ list (scene), list (other) ];
               out .own = [ attempt (() => ends (other .addRoute (OA, "translation", OB, "translation"))), list (other) .length, other .routes [0] === scene .routes [0] ];
               out .getOther = attempt (() => desc (other .getRoute (OA, "translation", OB, "translation")));
               out .getNone = attempt (() => other .getRoute (OB, "translation", OA, "translation"));
               out .routesOf = [ scene .routes .length, other .routes .length ];

               // A route object belongs to its scene: the other scene's deleteRoute does nothing with it.
               out .deleteForeign = attempt (() => { other .deleteRoute (scene .routes [0]); return [ list (scene), list (other) ]; });
               out .deleteOwn = attempt (() => { other .deleteRoute (other .routes [0]); return [ list (scene), list (other) ]; });
               out .deleteOwnFour = attempt (() => { scene .deleteRoute (A, "translation", B, "translation"); return [ list (scene), list (other) ]; });

               // Events in each scene; the page's callbacks.
               watch (C);
               OB .getField ("translation") .addFieldCallback ("OB", v => note ("OB", v));
               scene .addRoute (A, "translation", C, "translation");
               other .addRoute (OA, "translation", OB, "translation");
               setX (A, 7);
               setX (OA, 8);
               await spin (4);
               out .events = [ seq .slice (), tr (C), String (OB .translation) ];
               break;
            }
            case "crossScene":
            {
               // Nodes of two scenes in one route (the deferred fields03 s06). X_ITE does not look at the scene of a node:
               // the route lives in the scene that was asked and carries events between the scenes.
               const other = await browser .createX3DFromString ("#X3D V4.0 utf8\nPROFILE Interchange\nDEF OA Transform { }\nDEF OB Transform { }\n");
               const OA = other .getNamedNode ("OA"), OB = other .getNamedNode ("OB");

               out .mixed = attempt (() => ends (scene .addRoute (A, "translation", OB, "translation")));
               out .afterMixed = [ list (scene), list (other) ];
               out .reverse = attempt (() => ends (other .addRoute (OA, "translation", C, "translation")));
               out .afterReverse = [ list (scene), list (other) ];
               out .getMixed = attempt (() => desc (scene .getRoute (A, "translation", OB, "translation")));
               out .getPageIn = attempt (() => desc (other .getRoute (A, "translation", B, "translation")));
               out .addPageIn = attempt (() => ends (other .addRoute (A, "translation", B, "translation")));
               out .sameRoute = attempt (() => scene .addRoute (A, "translation", OB, "translation") === scene .routes [1]);
               out .deleteMixed = attempt (() => { scene .deleteRoute (A, "translation", OB, "translation"); return [ list (scene), list (other) ]; });
               out .browserMixed = attempt (() => browser .addRoute (A, "translation", OB, "translation"));

               // Events across the scenes.
               OB .getField ("translation") .addFieldCallback ("OB", v => note ("OB", v));
               watch (C);
               attempt (() => scene .addRoute (A, "translation", OB, "translation"));
               attempt (() => other .addRoute (OA, "translation", C, "translation"));
               setX (A, 7);
               setX (OA, 8);
               await spin (4);
               out .events = [ seq .slice (), tr (C), String (OB .translation) ];
               break;
            }
            case "browser":
            {
               out .add = [ attempt (() => browser .addRoute (A, "translation", C, "translation")), list (scene) ];
               out .addAgain = [ attempt (() => browser .addRoute (A, "translation", C, "translation")), list (scene) .length ];
               out .addBad = [ attempt (() => browser .addRoute (A, "nope", C, "translation")), attempt (() => browser .addRoute (null, "translation", C, "translation")), attempt (() => browser .addRoute ()) ];
               setX (A, 3);
               await spin (4);
               out .flow = [ tr (B), tr (C) ];
               out .getRoute = [ typeof browser .getRoute, typeof browser .routes ];
               out .deleteBad = [ attempt (() => browser .deleteRoute (scene .routes [0])), attempt (() => browser .deleteRoute ()), attempt (() => browser .deleteRoute (A, "translation", C)),
                                  attempt (() => browser .deleteRoute (A, "nope", C, "translation")) ];
               out .delete = [ attempt (() => browser .deleteRoute (A, "translation", C, "translation")), list (scene) ];
               out .deleteAgain = [ attempt (() => browser .deleteRoute (A, "translation", C, "translation")), list (scene) .length ];
               setX (A, 4);
               await spin (4);
               out .stopped = [ tr (B), tr (C) ];

               // Delegation: the browser's routes are those of the current scene, also after the world changes.
               const doc = await browser .createX3DFromString ("#X3D V4.0 utf8\nPROFILE Interchange\nDEF P Transform { }\nDEF Q Transform { }\n");
               browser .replaceWorld (doc);
               for (let i = 0; i < 100 && browser .currentScene !== doc; ++ i) await new Promise (r => setTimeout (r, 100));
               await new Promise (r => setTimeout (r, 300));
               out .onDoc = [ attempt (() => browser .addRoute (doc .getNamedNode ("P"), "translation", doc .getNamedNode ("Q"), "translation")), list (doc), attempt (() => list (scene)) ];
               out .oldNodes = attempt (() => browser .addRoute (A, "translation", C, "translation"));
               out .afterOld = [ list (doc), attempt (() => list (scene)) ];
               break;
            }
            case "lifecycle":
            {
               // The route object after deleteRoute, routes of a scene that is not the world, and a route of a deleted route again.
               const route = scene .addRoute (A, "translation", D, "translation");
               scene .deleteRoute (route);
               out .deleted = [ ends (route), scene .routes .length, scene .routes .has ?.(route), typeof route .getId ];
               const again = scene .addRoute (A, "translation", D, "translation");
               out .again = [ again === route, ends (again), scene .routes .length ];
               out .oldStillDeletes = [ attempt (() => scene .deleteRoute (route)), scene .routes .length ];
               out .newDeletes = [ attempt (() => scene .deleteRoute (again)), scene .routes .length ];

               // A route on a node that came out of an SFNode field; the nodes of a route, found again.
               const found = scene .getRoute (A, "translation", B, "translation");
               out .parsedNodes = [ found .sourceNode === A, found .destinationNode === B, found .sourceNode .valueOf () === A .valueOf () ];
               out .iterOrder = [ list (scene) ];

               // Route events during an event: a callback that adds and deletes a route.
               watch (B);
               B .getField ("translation") .addFieldCallback ("adder", () =>
               {
                  attempt (() => scene .addRoute (B, "translation", E, "translation"));
                  note ("added", scene .routes .length);
               });
               setX (A, 1);
               await spin (3);
               out .during = [ seq .slice (), tr (E), list (scene) ];
               seq = [ ];
               setX (A, 2);
               await spin (3);
               out .during2 = [ seq .slice (), tr (E) ];
               break;
            }
         }
      }
      finally
      {
         console .warn  = warn;
         console .error = error;
      }

      out .log = logs;

      return JSON .parse (JSON .stringify (out, (key, value) => value === undefined ? null : value));
   }, name);
}

const cases = Object .fromEntries ([ "objects", "flow", "order", "errors", "names", "scenes", "crossScene", "browser", "lifecycle" ] .map (name => [ name, page => scenario (page, name) ]));

for (const [ name, run ] of Object .entries (cases))
   cases [name] = async (page, env) => { await page .evaluate (pageAttempt); return run (page, env); };

export const CASE_GROUPS = {
   routesApi: { context: DESKTOP, cases },
};
