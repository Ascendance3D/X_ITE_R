// The Script `shutdown` cases (issue #292): when a Script's `shutdown ()` runs, for each life cycle event on its own, and the
// browser `shutdown` event of `dispose ()`. ISO/IEC 14772-1 4.12.3 asks for `shutdown ()` "when the corresponding Script node is
// deleted or the world containing the Script node is unloaded or replaced by another world"; 4.12.2 and 4.10.4 ask for it when
// a `set_url` replaces the code. The cases use newly authored synthetic worlds. oracle/x_ite-web-runtime-02.mjs
// (--suite shutdown01) runs them against pinned X_ITE 16.5.0 and records the values; shutdown01.test.mjs runs them against
// X_ITE_R (with `<x3d-canvas scripts="true">`); runtime02_harness.mjs lists the differences (KNOWN_DIFFERENCES).
//
// Each case records one ordered list, `seq`: the `print` lines of the Scripts, the notes of the page, and the browser events.

import { DESKTOP } from "./runtime02_case.mjs";

const script = (name, extra = "") => `Script { ${extra} url "javascript: function initialize () { print ('${name} initialize'); } function shutdown () { print ('${name} shutdown'); }" }`;

const WORLDS = {
   // Two Scripts that are root nodes, one of them DEF-named, in the order A, B.
   main: `#VRML V2.0 utf8\nDEF A ${script ("A")}\nDEF B ${script ("B")}\n`,
   // A Script in an Inline (a data URL of a VRML file), and one in the world.
   inline: `#VRML V2.0 utf8\nDEF W ${script ("W")}\nDEF I Inline { url "INLINE_URL" }\n`,
   // A Script that did not start: `load` is FALSE; one whose code does not load.
   idle: `#VRML V2.0 utf8\nDEF L ${script ("L", "load FALSE")}\nDEF F Script { url "data:text/javascript;base64,!" }\nDEF X Script { url [ ] }\n`,
};

const INNER_A = `#VRML V2.0 utf8\nDEF N ${script ("N")}\n`;
const INNER_B = `#VRML V2.0 utf8\nDEF M ${script ("M")}\n`;

async function scenario (page, name)
{
   return page .evaluate (async ([ name, worlds, innerA, innerB ]) =>
   {
      const X3D     = window .X3D_NS;
      const canvas  = document .querySelector ("x3d-canvas");
      const browser = X3D .getBrowser (canvas);
      const frame   = () => new Promise (r => requestAnimationFrame (() => r ()));
      const spin    = async n => { for (let i = 0; i < n; ++ i) await frame (); };
      const seq     = [ ];
      const out     = { };
      const saved   = Object .fromEntries ([ "log", "warn", "error" ] .map (type => [ type, console [type] ]));
      const data    = text => "data:model/vrml," + encodeURIComponent (text) .replace (/'/g, "%27");
      const note    = text => seq .push (`page: ${text}`);
      const scene   = () => browser .currentScene;
      const node    = id => scene () .getNamedNode (id);
      const load    = async text => { await browser .replaceWorld (await browser .createX3DFromString (text)); await spin (4); };
      const attempt = fn => { try { return fn (); } catch (error) { return `${error ?.constructor ?.name}: ${error ?.message}`; } };

      canvas .setAttribute ("scripts", "true");

      // Only the Scripts' prints are the record: the console lines of loads differ in their hosts and are not the subject.
      console .log = (... args) => seq .push (`log: ${args .join (" ") .split ("\n") [0]}`);
      console .warn = console .error = () => { };

      canvas .addEventListener ("shutdown", () => seq .push ("event: shutdown (element)"));
      canvas .addEventListener ("initialized", () => seq .push ("event: initialized (element)"));
      browser .addBrowserCallback ("cb", X3D .X3DConstants .SHUTDOWN_EVENT, () => seq .push ("event: shutdown (callback)"));

      try
      {
         const inlineWorld = worlds .inline .replace ("INLINE_URL", data (innerA));

         // (b3) The Script element is removed from the page (DOM integration): the world is the <X3D> element of the canvas.
         if (name === "domRemove")
         {
            const code = "javascript: function initialize () { print ('D initialize'); } function shutdown () { print ('D shutdown'); }";

            canvas .innerHTML = `<X3D profile='Full'><Scene><Script DEF='D' url="&quot;${code}&quot;"></Script><Group DEF='G'></Group></Scene></X3D>`;
            await new Promise (r => setTimeout (r, 1000));
            await spin (4);
            note ("remove the Script element");
            canvas .querySelector ("script") .remove ();
            await spin (8);
            note ("remove the Group element");
            canvas .querySelector ("group") .remove ();
            await spin (8);
            out .seq = seq;
            Object .assign (console, saved);

            return JSON .parse (JSON .stringify (out));
         }

         await load (name .startsWith ("inline") || name .startsWith ("replaceInline") ? inlineWorld : name .startsWith ("idle") ? worlds .idle : worlds .main);

         switch (name)
         {
            // (a) The world is replaced.
            case "replaceWorld":
               note ("replaceWorld");
               await load ("#VRML V2.0 utf8\nDEF G Group { }\n");
               note ("replaceWorld again");
               await load ("#VRML V2.0 utf8\nDEF H Group { }\n");
               break;
            case "loadURL":
               note ("loadURL");
               await browser .loadURL (new X3D .MFString (data ("#VRML V2.0 utf8\nDEF G Group { }\n")));
               await spin (4);
               break;
            case "replaceInline":
               note ("replaceWorld");
               await load ("#VRML V2.0 utf8\nDEF G Group { }\n");
               break;
            // (b) The Script node is deleted from the live scene.
            case "removeRootNode":
               note ("removeRootNode A");
               scene () .removeRootNode (node ("A"));
               await spin (6);
               note ("setRootNodes");
               scene () .setRootNodes (new X3D .MFNode ());
               await spin (6);
               break;
            case "inlineLoadFalse":
               note ("load false");
               node ("I") .load = false;
               await spin (8);
               note ("load true");
               node ("I") .load = true;
               await spin (8);
               break;
            case "inlineUrl":
               note ("url");
               node ("I") .url = new X3D .MFString (data (innerB));
               await spin (8);
               break;
            case "inlineRemove":
               note ("removeRootNode I");
               scene () .removeRootNode (node ("I"));
               await spin (8);
               break;
            // (c) The browser is disposed.
            case "dispose":
               note ("dispose");
               browser .dispose ();
               await spin (8);
               break;
            case "disposeReplacing":
            {
               const next = await browser .createX3DFromString ("#VRML V2.0 utf8\nDEF G Group { }\n");

               note ("replaceWorld, then dispose");
               browser .replaceWorld (next) .catch (() => { });
               browser .dispose ();
               await spin (8);
               break;
            }
            // The page ends (X_ITE listens for `unload` on the window: its Scripts get `shutdown ()` then).
            case "unload":
               note ("unload");
               window .dispatchEvent (new Event ("unload"));
               await spin (8);
               break;
            case "remove":
               note ("remove the element");
               canvas .remove ();
               await spin (8);
               break;
            // (d) The Script's code is replaced.
            case "codeReplaced":
               note ("url");
               node ("A") .url = new X3D .MFString ("javascript: function initialize () { print ('A2 initialize'); } function shutdown () { print ('A2 shutdown'); }");
               await spin (8);
               break;
            // (e) A Script that did not start.
            case "idleReplace":
               note ("replaceWorld");
               await load ("#VRML V2.0 utf8\nDEF G Group { }\n");
               break;
         }
      }
      finally
      {
         Object .assign (console, saved);
      }

      out .seq = seq;

      return JSON .parse (JSON .stringify (out, (key, value) => value === undefined ? null : value));
   },
   [ name, WORLDS, INNER_A, INNER_B ]);
}

const NAMES = [ "replaceWorld", "loadURL", "replaceInline", "removeRootNode", "inlineLoadFalse", "inlineUrl", "inlineRemove", "dispose", "disposeReplacing", "domRemove", "unload", "remove", "codeReplaced", "idleReplace" ];

export const CASE_GROUPS = {
   scriptShutdown: { context: DESKTOP, cases: Object .fromEntries (NAMES .map (name => [ name, page => scenario (page, name) ])) },
};
