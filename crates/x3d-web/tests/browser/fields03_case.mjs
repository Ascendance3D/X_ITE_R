// The field API 03 cases (issue #137 part C): node ownership and assignment between scenes.
// oracle/x_ite-web-runtime-02.mjs (--suite fields03) runs them against pinned X_ITE 16.5.0 and records the values;
// fields03.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { pageLoadURL } from "./loadurl01_case.mjs";

const WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Interchange
DEF T Transform {
   translation 1 2 3
   children [
      DEF S Shape {
         appearance Appearance { material DEF M Material { diffuseColor 0.2 0.4 0.9 } }
         geometry Box { size 2 3 4 }
      }
      USE S
   ]
}
DEF G Group { }
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


// Node ownership and cross-scene assignment (issue #137 part C): twelve scenarios on what pinned X_ITE does when a node
// moves between fields and scenes. Every value is a string or a plain list so both engines compare equal.
async function nodeOwnership (page)
{
   await load (page);

   return page .evaluate (async () =>
   {
      const X3D     = window .X3D_NS;
      const attempt = window .attempt;
      const browser = X3D .getBrowser (document .querySelector ("x3d-canvas"));
      const scene   = browser .currentScene;
      const names   = nodes => Array .from (nodes, n => n ? `${n .getNodeTypeName ()}${n .getNodeName () ? ":" + n .getNodeName () : ""}` : null);
      // The public API has no owner accessor: a named node is "page" or "other" when that scene finds this node by its name.
      // A new node in the scene `ctx` (X3DField.fromVRMLString with a scene; createNode is the scene API of issue #134).
      const make    = (ctx, text) => { const f = new X3D .SFNode (); f .fromVRMLString (text, ctx); return f; };
      const owner   = node => !node ? null : [ "page", "other" ] .find (key => { const ctx = key === "page" ? scene : other, name = node .getNodeName (); try { return name && ctx .getNamedNode (name) .valueOf () === node .valueOf (); } catch { return false; } }) ?? "none";
      const out     = { };
      const logs    = [ ];
      const warn    = console .warn, error = console .error;

      console .warn  = (... a) => logs .push (`warn: ${a .join (" ")}`);
      console .error = (... a) => logs .push (`error: ${a .join (" ")}`);

      var other = await browser .createX3DFromString ("#X3D V4.0 utf8\nPROFILE Interchange\nDEF OG Group { children DEF OT Transform { } }\n");

      try
      {
         const T = scene .getNamedNode ("T"), G = scene .getNamedNode ("G"), OT = other .getNamedNode ("OT"), OG = other .getNamedNode ("OG");

         // 1 Assignment inside the original scene.
         out .s01 = attempt (() => { const f = new X3D .SFNode (); f .setValue (G); return [ names ([ f .valueOf () ]), owner (f .valueOf ()), f .valueOf () === G.valueOf () ]; });

         // 2 Cross-scene SFNode assignment: a node of another scene into a field of a page node.
         out .s02 = attempt (() =>
         {
            const holder = make (scene, "Collision { }");
            holder .proxy = OT;
            return [ names ([ holder .proxy ]), owner (holder .proxy), owner (OT) ];
         });

         // 3 Mixed-scene MFNode.
         out .s03 = attempt (() =>
         {
            const g = make (scene, "Group { }");
            g .children = [ G, OT, T ];
            return [ names (g .children), Array .from (g .children, owner) ];
         });

         // 4 Root-node insertion: a node of another scene into this scene's rootNodes.
         out .s04 = attempt (() =>
         {
            const before = scene .rootNodes .length;
            scene .rootNodes .push (OT);
            return [ before, scene .rootNodes .length, names (scene .rootNodes), owner (OT) ];
         });

         // 5 A reference that stays in another scene: its own scene still lists it, its parent is unchanged.
         out .s05 = attempt (() => [ names (other .rootNodes), names (OG .children), owner (OG .children [0]) ]);

         // 6 Routes across scenes (addRoute is the scene API of issue #135) are recorded in the PR text, not compared here.

         // 7 Removal and reuse: the same node removed from one MFNode and placed in another.
         out .s07 = attempt (() =>
         {
            const a = make (scene, "Group { }"), b = make (scene, "Group { }"), n = make (scene, "Transform { }");
            a .children = [ n ];
            a .children = [ ];
            b .children = [ n ];
            return [ names (a .children), names (b .children), b .children [0] === n, owner (n) ];
         });

         // 8 Failure and rollback: an invalid element in a list, and a wrong-typed node for a typed field.
         out .s08 = attempt (() =>
         {
            const g = make (scene, "Group { }");
            g .children = [ G ];
            const bad = attempt (() => { g .children = [ G, 5 ]; });
            const wrong = attempt (() => { g .children = [ G, make (scene, "Box { }") ]; });
            return [ bad, wrong, names (g .children) ];
         });

         // 9 Scene replacement: nodes of the replaced scene after replaceWorld.
         const kept = scene .getNamedNode ("T");
         browser .replaceWorld (other);
         await new Promise (r => setTimeout (r, 200));
         out .s09 = attempt (() => [ browser .currentScene === other, kept .getNodeTypeName (), owner (kept), String (kept .translation) ]);

         // 10 Invalid node references.
         out .s10 = attempt (() =>
         {
            const f = new X3D .SFNode ();
            return [ attempt (() => f .setValue (5)), attempt (() => f .setValue ("x")), attempt (() => f .setValue (null)), String (f),
                     attempt (() => f .setValue (undefined)), attempt (() => f .setValue ({ })) ];
         });

         // 11 Repeated assignment of the same node.
         out .s11 = attempt (() =>
         {
            const g = make (other, "Group { }");
            const t = make (other, "Transform { }");
            g .children = [ t, t, t ];
            const three = names (g .children);
            g .children = [ t ];
            g .children = [ t ];
            return [ three, names (g .children), g .children [0] === t ];
         });

         // 12 Serialization after assignment.
         out .s12 = attempt (() =>
         {
            const g = make (other, "Group { }");
            g .children = [ other .getNamedNode ("OT"), make (scene, "Transform { }") ];
            return [ g .toVRMLString () .replace (/\s+/g, " "), g .toXMLString () .replace (/\s+/g, " ") ];
         });
      }
      finally
      {
         console .warn  = warn;
         console .error = error;
      }

      out .log = logs;

      return out;
   });
}

const cases = { nodeOwnership };

for (const [ name, run ] of Object .entries (cases))
   cases [name] = async (page, env) => { await page .evaluate (pageAttempt); return run (page, env); };

export const CASE_GROUPS = {
   nodeOwnership: { context: DESKTOP, cases },
};
