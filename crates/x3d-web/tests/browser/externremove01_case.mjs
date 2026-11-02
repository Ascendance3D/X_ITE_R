// The EXTERNPROTO removal cases (issue #58): X3DExecutionContext.removeExternProtoDeclaration only takes the declaration out of
// the scene's table. X_ITE 16.5.0 neither disposes it nor aborts its FileLoader, so a load in flight finishes and rebuilds the
// bodies of the instances, and a declaration the page still holds keeps working. oracle/x_ite-web-runtime-02.mjs
// (--suite externremove01) records the pinned X_ITE; externremove01.test.mjs compares X_ITE_R (with `scripts="true"`).
// The standards say nothing about it (ISO/IEC 14772-1 4.9, SAI of ISO/IEC 19775-2), so there is no known difference.
//
// The world has one EXTERNPROTO `Ext` and one instance `I`; the first version of the file is published (the server's
// `publish`) at the held path, the second is worlds/extern-remove.wrl. Each version prints from a Script in the body, so a
// rebuilt body shows as a line: `B1 initialize` (the world), `B2 initialize` (a reload). The held request waits for `release`.

import { DESKTOP } from "./runtime02_case.mjs";

const V1 = "#VRML V2.0 utf8\nPROTO Ext [ ] { Script { url \"javascript: function initialize () { print ('B1 initialize'); }\" } }\nGroup { }\n";

async function scenario (page, env, name)
{
   const key  = `xr${name}${Date .now ()}`;
   const path = `/hold/${key}/tests/browser/worlds/extern-remove.wrl`;

   env .server .publish (path, V1);
   page .log .length = 0;

   const from = env .server .requests .length;
   const text = `#VRML V2.0 utf8\nEXTERNPROTO Ext [ ] [ "${env .base}${path}#Ext" ]\nDEF I Ext { }\n`;

   const out = await page .evaluate (async ([ name, text ]) =>
   {
      const X3D     = window .X3D_NS;
      const canvas  = document .querySelector ("x3d-canvas");
      const browser = X3D .getBrowser (canvas);
      const spin    = async n => { for (let i = 0; i < n; ++ i) await new Promise (r => requestAnimationFrame (() => r ())); };

      canvas .setAttribute ("scripts", "true");
      browser .setBrowserOption ("Cache", false);

      window .seq = [ ];
      window .xr  = { browser, spin };
      console .log = (... args) => window .seq .push (`log: ${args .join (" ") .split ("\n") [0]}`);
      console .warn = console .error = () => { };

      await browser .replaceWorld (await browser .createX3DFromString (text));
      await spin (6);
      window .seq .push ("page: loaded");

      return null;
   }, [ name, text ]);

   env .server .unpublish ();

   const step = async (label, body) =>
   {
      const value = await page .evaluate (async ([ label, body ]) =>
      {
         const { browser, spin } = window .xr;
         const scene = browser .currentScene;
         const decl  = () => scene .getExternProtoDeclaration ("Ext");
         const names = () => Array .from (scene .externprotos, p => p .name);
         const state = d => d .loadState;
         // The load again (the `Cache` option is off, so the file is asked for again): `loadNow ()`. It is over when the state
         // is not IN_PROGRESS_STATE (1007) any more, or after 1.5 s.
         const reload = d => { d .loadNow () .catch (() => { }); };
         const done   = async d => { const end = Date .now () + 1500; while (state (d) === 1007 && Date .now () < end) await spin (1); return state (d); };

         window .seq .push (`page: ${label}`);

         return new Function ("browser", "spin", "scene", "decl", "names", "state", "reload", "done", `return (async () => { ${body} })()`) (browser, spin, scene, decl, names, state, reload, done);
      },
      [ label, body ]);

      return value;
   };

   const results = { };
   let early = null;

   switch (name)
   {
      // The load is in flight, the declaration is removed, then the response arrives.
      case "inFlight":
         results .held   = await step ("reload", "window.d = decl (); reload (window.d); await spin (6); return [ state (window.d), names () ];");
         results .remove = await step ("remove", "scene .removeExternProtoDeclaration ('Ext'); await spin (6); return [ state (window.d), names () ];");
         env .server .release (key);
         results .after  = await step ("released", "const r = await done (window.d); await spin (10); return [ r, state (window.d), names () ];");
         break;
      // The page still holds the declaration: it loads again, and makes instances.
      case "held":
         results .held   = await step ("remove", "window.d = decl (); scene .removeExternProtoDeclaration ('Ext'); await spin (4); return [ names (), state (window.d) ];");
         env .server .release (key);
         results .again  = await step ("reload", "reload (window.d); await spin (2); const r = await done (window.d); await spin (8); return [ r, state (window.d) ];");
         // The console is read before: a Script of the page's own `newInstance ()` prints its `initialize` in X_ITE, not here (not
         // part of the removal).
         early = await page .evaluate (() => window .seq .slice ());
         results .create = await step ("newInstance", "const n = window.d .newInstance (); return [ n .getNodeTypeName (), window.d .name, window.d .getProtoDeclaration () ?.name ];");
         break;
      // The declaration is removed first, and its url is set after.
      case "beforeLoad":
         results .remove = await step ("remove", "window.d = decl (); scene .removeExternProtoDeclaration ('Ext'); return names ();");
         env .server .release (key);
         results .load   = await step ("reload", "reload (window.d); await spin (2); const r = await done (window.d); await spin (8); return [ r, state (window.d) ];");
         break;
      // The world is replaced while the removed declaration's load is in flight.
      case "replaced":
         results .held   = await step ("reload", "window.d = decl (); reload (window.d); await spin (6); return state (window.d);");
         results .remove = await step ("remove", "scene .removeExternProtoDeclaration ('Ext'); return names ();");
         // The old world's nodes are gone for the page of X_ITE_R (a declaration of a replaced world has no state to read), so only
         // the console and the requests are observed after.
         results .world  = await step ("replaceWorld", "await browser .replaceWorld (await browser .createX3DFromString ('#VRML V2.0 utf8\\nDEF G Group { }\\n')); await spin (6); return Array .from (browser .currentScene .rootNodes, n => n .getNodeName ());");
         env .server .release (key);
         results .after  = await step ("released", "await spin (30); return null;");
         break;
   }

   const seq = early ?? await page .evaluate (() => window .seq);

   return { results, seq, served: env .server .requests .slice (from) .map (r => r .path) .filter (p => p === path || p === `${path}`) .length };
}

export const CASE_GROUPS = {
   externRemove: { context: DESKTOP, cases: Object .fromEntries ([ "inFlight", "held", "beforeLoad", "replaced" ] .map (name => [ name, (page, env) => scenario (page, env, name) ])) },
};
