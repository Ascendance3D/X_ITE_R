// The root nodes 01 cases (issue #137): a live X3DScene .rootNodes, addRootNode, removeRootNode, setRootNodes and the
// field callbacks of `rootNodes`.
// oracle/x_ite-web-runtime-02.mjs (--suite rootnodes01) runs them against pinned X_ITE 16.5.0 and records the values;
// rootnodes01.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { pageLoadURL } from "./loadurl01_case.mjs";

// A blue box on the left (T), an empty Group (G) and a TouchSensor (S) as the root nodes (the Interactive profile has the
// TouchSensor).
const WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Interactive
DEF T Transform {
  translation -2 0 0
  children Shape { appearance Appearance { material Material { diffuseColor 0 0 1 } } geometry Box { } }
}
DEF G Group { }
DEF S TouchSensor { }
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

// Each scenario loads a fresh page. `seq` lists the callbacks of `rootNodes` (and of T .translation, where named) in the
// order they ran as `key|value|when|frames`: `value` is the names of the root nodes, `when` is "sync" if the callback ran
// before the operation returned, `frames` counts the animation frames that passed before it ran.
async function scenario (page, name)
{
   await load (page);

   return page .evaluate (async name =>
   {
      const X3D     = window .X3D_NS;
      const browser = X3D .getBrowser (document .querySelector ("x3d-canvas"));
      const scene   = browser .currentScene;
      const T       = scene .getNamedNode ("T");
      const G       = scene .getNamedNode ("G");
      const S       = scene .getNamedNode ("S");
      const frame   = () => new Promise (r => requestAnimationFrame (() => r ()));
      const logs    = [ ];
      const warn    = console .warn, error = console .error;

      console .warn  = (... a) => logs .push (`warn: ${a .join (" ") .split ("\n") [0]}`);
      console .error = (... a) => logs .push (`error: ${a .join (" ") .split ("\n") [0]}`);

      const errors  = [ ];
      const onError = event => { errors .push (String (event .error ?.message ?? event .message)); event .preventDefault (); };

      window .addEventListener ("error", onError);

      // The result of `fn`, or its error as `Name: message`.
      const attempt = fn => { try { return fn (); } catch (error) { return `${error ?.constructor ?.name}: ${error ?.message}`; } };
      const name1   = n => n ? `${n .getNodeTypeName ()}${n .getNodeName () ? ":" + n .getNodeName () : ""}` : "NULL";
      const names   = nodes => Array .from (nodes, name1);
      const text    = () => scene .toVRMLString () .replace (/\s+/g, " ") .trim ();
      // A new node of this scene (X3DField.fromVRMLString with the scene).
      const make    = vrml => { const f = new X3D .SFNode (); f .fromVRMLString (vrml, scene); return f .valueOf (); };

      let seq = [ ], inOp = false, frames = 0;

      const note  = (key, v) => seq .push ([ key, v instanceof X3D .MFNode ? names (v) .join (",") : String (v), inOp ? "sync" : "later", frames ] .join ("|"));
      const op    = fn => { inOp = true; try { return attempt (fn); } finally { inOp = false; } };
      const spin  = async n => { for (let i = 0; i < n; ++ i) { await frame (); ++ frames; } };
      const watch = () => scene .rootNodes .addFieldCallback ("roots", v => note ("roots", v));
      const out   = { };

      try
      {
         switch (name)
         {
            case "live":
            {
               // One field object, live: the same object on every read, and a node of it is the node getNamedNode gives.
               const roots = scene .rootNodes;

               out .same      = [ roots === scene .rootNodes, roots === scene .getRootNodes ?.(), roots [0] === T, roots [1] === G, roots [2] === S ];
               out .field     = [ roots instanceof X3D .MFNode, roots .getTypeName (), roots .getName (), roots .getAccessType () ];
               out .before    = [ roots .length, names (roots) ];
               out .pushed    = op (() => roots .push (G));
               out .after     = [ roots .length, scene .rootNodes .length, names (scene .rootNodes) ];
               out .text      = text ();
               await spin (2);
               out .later     = [ scene .rootNodes === roots, names (scene .rootNodes) ];
               out .text2     = text ();
               break;
            }
            case "push":
            {
               // Push a new node of the world (a red box on the right): it is drawn, printed and the same object.
               const R = make ("DEF R Transform { translation 2 0 0 children Shape { appearance Appearance { material Material { diffuseColor 1 0 0 } } geometry Sphere { } } }");

               watch ();
               out .pushed  = op (() => scene .rootNodes .push (R));
               out .after   = [ scene .rootNodes .length, names (scene .rootNodes), scene .rootNodes [3] === R ];
               await spin (3);
               out .text    = text ();
               out .named   = attempt (() => scene .getNamedNode ("R") === R);
               out .unshift = op (() => scene .rootNodes .unshift (S));
               await spin (3);
               out .final   = names (scene .rootNodes);
               break;
            }
            case "splice":
            {
               // splice, pop, shift and a length change remove root nodes.
               watch ();
               out .spliced = op (() => names (scene .rootNodes .splice (1, 1)));
               out .after   = names (scene .rootNodes);
               await spin (3);
               out .popped  = op (() => name1 (scene .rootNodes .pop ()));
               await spin (3);
               out .length  = op (() => { scene .rootNodes .length = 3; return names (scene .rootNodes); });
               await spin (3);
               out .shrink  = op (() => { scene .rootNodes .length = 0; return names (scene .rootNodes); });
               await spin (3);
               out .text    = text ();
               out .shift   = op (() => scene .rootNodes .shift ());
               await spin (3);
               break;
            }
            case "assign":
            {
               // `rootNodes = value` and setRootNodes: the same field object takes the value.
               const roots = scene .rootNodes;

               watch ();
               out .assign  = op (() => { scene .rootNodes = new X3D .MFNode (S, T); return names (scene .rootNodes); });
               out .same    = [ scene .rootNodes === roots, names (roots) ];
               await spin (3);
               out .set     = op (() => { scene .setRootNodes (new X3D .MFNode (G)); return names (scene .rootNodes); });
               await spin (3);
               out .array   = op (() => { scene .rootNodes = [ T, G ]; return names (scene .rootNodes); });
               await spin (3);
               out .setArr  = op (() => { scene .setRootNodes ([ S ]); return names (scene .rootNodes); });
               await spin (3);
               out .nulls   = op (() => { scene .setRootNodes (new X3D .MFNode (null, T)); return names (scene .rootNodes); });
               await spin (3);
               out .text    = text ();
               out .final   = [ scene .rootNodes === roots, names (scene .rootNodes) ];
               break;
            }
            case "addRemove":
            {
               // addRootNode adds a node that is not there; removeRootNode removes every copy of a node.
               const N = make ("DEF N Group { }");

               watch ();
               out .add      = op (() => scene .addRootNode (N));
               out .afterAdd = names (scene .rootNodes);
               await spin (3);
               out .again    = op (() => scene .addRootNode (T));
               out .afterAgain = names (scene .rootNodes);
               await spin (3);
               out .remove   = op (() => scene .removeRootNode (G));
               out .afterRemove = names (scene .rootNodes);
               await spin (3);
               out .missing  = op (() => scene .removeRootNode (G));
               out .afterMissing = names (scene .rootNodes);
               await spin (3);
               out .addNull     = op (() => scene .addRootNode (null));
               out .removeNull  = op (() => scene .removeRootNode (null));
               out .addText     = op (() => scene .addRootNode ("T"));
               out .removeUndef = op (() => scene .removeRootNode ());
               out .afterBad    = names (scene .rootNodes);
               await spin (3);
               out .text     = text ();
               break;
            }
            case "nulls":
            {
               // NULL entries: removeRootNode does not remove them, a push keeps them.
               watch ();
               out .push    = op (() => scene .rootNodes .push (null));
               out .after   = names (scene .rootNodes);
               await spin (3);
               out .remove  = op (() => scene .removeRootNode (S));
               out .after2  = names (scene .rootNodes);
               await spin (3);
               out .text    = text ();
               break;
            }
            case "duplicates":
            {
               // The same node twice: printed with USE, removed together.
               watch ();
               out .push    = op (() => scene .rootNodes .push (T, T));
               out .after   = names (scene .rootNodes);
               await spin (3);
               out .text    = text ();
               out .add     = op (() => scene .addRootNode (T));
               out .remove  = op (() => scene .removeRootNode (T));
               out .after2  = names (scene .rootNodes);
               await spin (3);
               out .text2   = text ();
               break;
            }
            case "callbacks":
            {
               // The rootNodes event among the events of a node field, and two changes in one frame.
               scene .rootNodes .addFieldCallback ("a", v => note ("a", v));
               scene .rootNodes .addFieldCallback ("b", v => note ("b", v));
               T .getField ("translation") .addFieldCallback ("t", v => note ("t", v));
               out .arg = [ ];
               scene .rootNodes .addFieldCallback ("arg", v => out .arg .push ([ v === scene .rootNodes, v instanceof X3D .MFNode ]));

               op (() => { T .translation = new X3D .SFVec3f (-2, 1, 0); scene .rootNodes .push (G); });
               await spin (3);
               op (() => { scene .rootNodes .pop (); scene .removeRootNode (S); T .translation = new X3D .SFVec3f (-2, 0, 0); });
               await spin (3);
               scene .rootNodes .removeFieldCallback ("a");
               op (() => scene .addRootNode (S));
               await spin (3);
               out .callbacks = Array .from (scene .rootNodes .getFieldCallbacks () .keys ());
               break;
            }
            case "picture":
            {
               // The drawn world follows: the blue box removed, a red sphere added.
               const R = make ("Transform { translation 2 0 0 children Shape { appearance Appearance { material Material { diffuseColor 1 0 0 } } geometry Sphere { } } }");

               scene .removeRootNode (T);
               scene .addRootNode (R);
               await spin (4);
               out .roots = names (scene .rootNodes);
               break;
            }
         }
      }
      finally
      {
         console .warn  = warn;
         console .error = error;
         window .removeEventListener ("error", onError);
      }

      out .errors = errors;
      out .seq    = seq;
      out .logs   = logs;

      return JSON .parse (JSON .stringify (out, (key, value) => value === undefined ? null : value));
   }, name);
}

const NAMES = [ "live", "push", "splice", "assign", "addRemove", "nulls", "duplicates", "callbacks" ];

const cases = Object .fromEntries (NAMES .map (name => [ name, page => scenario (page, name) ]));

// The picture case adds the picture of the canvas after the change.
cases .picture = async (page, env) =>
{
   const values = await scenario (page, "picture");

   await settle (page, 200);

   return { ... values, picture: await env .picture (page) };
};

export const CASE_GROUPS = {
   rootNodes: { context: DESKTOP, cases },
};
