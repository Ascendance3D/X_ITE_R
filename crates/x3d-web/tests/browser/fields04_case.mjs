// The field API 04 cases (issue #137 part D): field callbacks and the event cascade.
// oracle/x_ite-web-runtime-02.mjs (--suite fields04) runs them against pinned X_ITE 16.5.0 and records the values;
// fields04.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { pageLoadURL } from "./loadurl01_case.mjs";

const WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Interchange
DEF A Transform { }
DEF B Transform { }
DEF C Transform { }
DEF D Transform { }
ROUTE A.translation_changed TO B.set_translation
ROUTE B.translation_changed TO C.set_translation
ROUTE A.translation_changed TO D.set_translation
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


// Field callbacks and the event cascade (issue #137 part D). Each scenario loads a fresh page: a routed chain
// A -> B -> C and A -> D, a field set from the page, and the callbacks of the four fields. `seq` lists the callbacks in the
// order they ran as `key|value|when|frames`: `when` is "sync" if the callback ran before the setter returned, `frames`
// counts the animation frames that passed before it ran.
async function scenario (page, name)
{
   await load (page);

   return page .evaluate (async name =>
   {
      const X3D     = window .X3D_NS;
      const browser = X3D .getBrowser (document .querySelector ("x3d-canvas"));
      const scene   = browser .currentScene;
      const nodes   = Object .fromEntries ([ "A", "B", "C", "D" ] .map (n => [ n, scene .getNamedNode (n) ]));
      const frame   = () => new Promise (r => requestAnimationFrame (() => r ()));
      const logs    = [ ];
      const warn    = console .warn, error = console .error;

      console .warn  = (... a) => logs .push (`warn: ${a .join (" ") .split ("\n") [0]}`);
      console .error = (... a) => logs .push (`error: ${a .join (" ") .split ("\n") [0]}`);

      let seq = [ ], inSetter = false, frames = 0;

      const field = n => nodes [n] .getField ("translation");
      const note  = (key, v) => seq .push ([ key, String (v), inSetter ? "sync" : "later", frames ] .join ("|"));
      const watch = n => field (n) .addFieldCallback (n, v => note (n, v));
      const spin  = async n => { for (let i = 0; i < n; ++ i) { await frame (); ++ frames; } };
      const setA  = x => { inSetter = true; nodes .A .translation = new X3D .SFVec3f (x, 0, 0); inSetter = false; };
      const out   = { };

      try
      {
         for (const n of [ "A", "B", "C", "D" ]) watch (n);

         switch (name)
         {
            case "chain":
               setA (1);
               out .afterSetter = seq .slice ();
               await spin (3);
               break;
            case "nested":
               // A callback of B sets D from inside the cascade.
               field ("B") .addFieldCallback ("nest", v => { nodes .D .translation = new X3D .SFVec3f (9, 9, 9); note ("nest", v); });
               setA (2);
               await spin (3);
               out .final = [ String (nodes .D .translation), String (nodes .C .translation) ];
               break;
            case "selfRemove":
               // A callback removes itself and adds one to C.
               field ("A") .addFieldCallback ("self", v => { note ("self", v); field ("A") .removeFieldCallback ("self"); field ("C") .addFieldCallback ("late", w => note ("late", w)); });
               setA (3);
               await spin (3);
               setA (4);
               await spin (3);
               break;
            case "thrown":
               // A callback that throws: the others still run, the error is reported.
               field ("B") .addFieldCallback ("boom", v => { throw new Error ("boom"); });
               setA (5);
               await spin (3);
               setA (6);
               await spin (3);
               break;
            case "duplicate":
               // The same key twice.
               field ("D") .addFieldCallback ("dup", v => note ("dup1", v));
               field ("D") .addFieldCallback ("dup", v => note ("dup2", v));
               setA (7);
               await spin (3);
               break;
            case "removeBeforeEvent":
               // A callback removed before its event is delivered.
               setA (8);
               field ("C") .removeFieldCallback ("C");
               await spin (3);
               break;
         }
      }
      finally
      {
         console .warn  = warn;
         console .error = error;
      }

      out .seq  = seq;
      out .logs = logs;

      return JSON .parse (JSON .stringify (out, (key, value) => value === undefined ? null : value));
   }, name);
}

const cases = Object .fromEntries ([ "chain", "nested", "selfRemove", "thrown", "duplicate", "removeBeforeEvent" ] .map (name => [ name, page => scenario (page, name) ]));

for (const [ name, run ] of Object .entries (cases))
   cases [name] = async (page, env) => { await page .evaluate (pageAttempt); return run (page, env); };

export const CASE_GROUPS = {
   fieldCascade: { context: DESKTOP, cases },
};
