// The viewpoints 01 cases (issue #135 part 2): the viewpoint functions of X3DBrowser with the layerNode argument
// (bindViewpoint, first/previous/next/lastViewpoint, changeViewpoint, viewAll), beginUpdate / endUpdate with isLive / setLive,
// and what the browser has of getUserViewpoints.
// oracle/x_ite-web-runtime-02.mjs (--suite viewpoints01) runs them against pinned X_ITE 16.5.0 and records the values;
// viewpoints01.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).
//
// Every value is a string, a number or a plain list so both engines compare equal. Each scenario loads a fresh page of the
// world below: layer 0 holds R1 R2 RH(no description) R3; the LayerSet holds L1 (A1 A2 AH A3), L2 (B1 B2) and L3 (no viewpoint);
// the active layer is L1.

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { pageLoadURL } from "./loadurl01_case.mjs";

const WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Layering:1
COMPONENT Navigation:3
COMPONENT EnvironmentalSensor:1
DEF R1 Viewpoint { description "R1" position 0 0 10 }
DEF R2 Viewpoint { description "R2" position 0 0 12 }
DEF RH Viewpoint { position 0 1 10 }
DEF R3 Viewpoint { description "R3" position 0 0 14 }
DEF RO OrthoViewpoint { description "RO" position 0 0 20 }
DEF RG ViewpointGroup { description "RG" children DEF RG1 Viewpoint { description "RG1" position 0 0 16 } }
DEF PS0 ProximitySensor { size 1000 1000 1000 }
DEF T TimeSensor { loop TRUE cycleInterval 0.5 }
DEF TR Transform { }
DEF TR2 Transform { }
DEF Box Shape { geometry Box { size 1 1 1 } }
LayerSet {
  activeLayer 1
  order [ 0 1 2 3 ]
  layers [
    DEF L1 Layer { children [
      DEF A1 Viewpoint { description "A1" position 0 0 7 }
      DEF A2 Viewpoint { description "A2" position 0 0 8 }
      DEF AH Viewpoint { position 0 2 7 }
      DEF A3 Viewpoint { description "A3" position 0 0 9 }
      DEF PS1 ProximitySensor { size 1000 1000 1000 }
      Shape { geometry Box { size 2 2 2 } }
    ] }
    DEF L2 Layer { children [
      DEF B1 Viewpoint { description "B1" position 0 0 5 }
      DEF B2 Viewpoint { description "B2" position 0 0 6 }
      DEF PS2 ProximitySensor { size 1000 1000 1000 }
      Shape { geometry Sphere { radius 3 } }
    ] }
    DEF L3 Layer { children [ DEF PS3 ProximitySensor { size 1000 1000 1000 } Shape { geometry Box { size 1 1 1 } } ] }
  ]
}
ROUTE TR.translation_changed TO TR2.set_translation
`)}`;

// Load the world; resolve with "resolved" or the error.
async function load (page)
{
   await page .evaluate (pageLoadURL, [ WORLD ]);

   const result = await page .evaluate (() => window .pending);

   if (result !== "resolved")
      throw new Error (`the world did not load: ${result}\n${page .log .join ("\n")}`);

   await settle (page, 400);
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

   return page .evaluate (async ([ name, WORLD ]) =>
   {
      const X3D     = window .X3D_NS;
      const attempt = window .attempt;
      const browser = X3D .getBrowser (document .querySelector ("x3d-canvas"));
      const scene   = browser .currentScene;
      const node    = n => scene .getNamedNode (n);
      const NAMES   = [ "R1", "R2", "RH", "R3", "RO", "RG1", "A1", "A2", "AH", "A3", "B1", "B2" ];
      const [ L1, L2, L3, Box, T, TR, TR2, PS0, PS1, PS2, PS3 ] = [ "L1", "L2", "L3", "Box", "T", "TR", "TR2", "PS0", "PS1", "PS2", "PS3" ] .map (node);
      const frame   = () => new Promise (r => requestAnimationFrame (() => r ()));
      const spin    = async n => { for (let i = 0; i < n; ++ i) await frame (); };
      const round   = v => String (v) .split (" ") .map (x => (Math .round (+x * 100) / 100) + 0) .join (" ");
      const logs    = [ ];
      const warn    = console .warn, error = console .error;

      console .warn  = (... a) => logs .push (`warn: ${a .join (" ") .split ("\n") [0]}`);
      console .error = (... a) => logs .push (`error: ${a .join (" ") .split ("\n") [0]}`);

      // The names of the bound viewpoints, and the active viewpoint's name.
      // The user position each layer's ProximitySensor saw last (read in the viewAll scenario only: a transition moves it).
      const pos = { };

      for (const [ label, ps ] of [ [ "PS0", PS0 ], [ "PS1", PS1 ], [ "PS2", PS2 ], [ "PS3", PS3 ] ])
         ps .addFieldCallback ("pos", "position_changed", v => pos [label] = round (v));

      const bound  = () => NAMES .filter (n => node (n) .isBound) .join (",");
      const active = () => browser .activeViewpoint ?.getNodeName () ?? null;
      const state  = () => [ bound (), active () ];
      const out    = { };

      // A call, the state right after it and four frames later.
      const step = async (label, fn) =>
      {
         const result = attempt (fn);
         const sync   = state ();

         await spin (4);
         out [label] = [ result === undefined ? "undefined" : result, sync, state () ];
      };

      try
      {
         switch (name)
         {
            case "api":
            {
               const names = [ "bindViewpoint", "getUserViewpoints", "beginUpdate", "endUpdate", "isLive", "setLive", "firstViewpoint", "previousViewpoint",
                               "nextViewpoint", "lastViewpoint", "changeViewpoint", "viewAll" ];

               out .types   = names .map (n => `${n}:${typeof browser [n]}:${browser [n] ?.length}`);
               out .layers  = [ L1, L2, L3 ] .map (n => [ n .getNodeTypeName (), Array .from (n .getNodeType ()) .includes (X3D .X3DConstants .X3DLayerNode) ]);
               out .vps     = [ "R1", "RO", "RG", "RG1", "A1" ] .map (n => [ n, Array .from (node (n) .getNodeType ()) .includes (X3D .X3DConstants .X3DViewpointNode) ]);
               out .initial = state ();
               out .live    = [ browser .isLive (), attempt (() => browser .isLive () .constructor .name) ];
               out .user    = attempt (() => browser .getUserViewpoints ());
               break;
            }
            case "bind":
            {
               await step ("bindL1A2",   () => browser .bindViewpoint (L1, node ("A2")));
               await step ("bindL1A3",   () => browser .bindViewpoint (L1, node ("A3")));
               await step ("bindAgain",  () => browser .bindViewpoint (L1, node ("A3")));
               await step ("bindL2B2",   () => browser .bindViewpoint (L2, node ("B2")));
               await step ("bindL1AH",   () => browser .bindViewpoint (L1, node ("AH")));
               await step ("bindOther",  () => browser .bindViewpoint (L1, node ("B1")));
               await step ("bindRoot",   () => browser .bindViewpoint (L2, node ("R2")));
               await step ("bindRH",     () => browser .bindViewpoint (L3, node ("RH")));
               await step ("bindOrtho",  () => browser .bindViewpoint (L1, node ("RO")));
               await step ("bindGroupChild", () => browser .bindViewpoint (L1, node ("RG1")));
               await step ("bindL3",     () => browser .bindViewpoint (L3, node ("A1")));
               out .bounds = NAMES .map (n => [ n, node (n) .isBound ]) .filter (([ , b ]) => b) .map (([ n ]) => n);
               break;
            }
            case "errors":
            {
               const A1 = node ("A1");

               await step ("none",       () => browser .bindViewpoint ());
               await step ("layerOnly",  () => browser .bindViewpoint (L1));
               await step ("vpOnly",     () => browser .bindViewpoint (A1));
               await step ("nullLayer",  () => browser .bindViewpoint (null, A1));
               await step ("nullVp",     () => browser .bindViewpoint (L1, null));
               await step ("notLayer",   () => browser .bindViewpoint (Box, A1));
               await step ("notVp",      () => browser .bindViewpoint (L1, Box));
               await step ("vpAsLayer",  () => browser .bindViewpoint (A1, A1));
               await step ("group",      () => browser .bindViewpoint (L1, node ("RG")));
               await step ("number",     () => browser .bindViewpoint (5, 6));
               await step ("string",     () => browser .bindViewpoint ("L1", "A1"));
               await step ("undef",      () => browser .bindViewpoint (undefined, undefined));
               await step ("object",     () => browser .bindViewpoint ({ }, { }));
               await step ("sfnodeNull", () => browser .bindViewpoint (new X3D .SFNode (), A1));
               await step ("extra",      () => browser .bindViewpoint (L1, A1, 7));
               out .errorsLogged = logs .length;
               break;
            }
            case "steps":
            {
               for (const [ label, layer ] of [ [ "L1", L1 ], [ "L2", L2 ], [ "L3", L3 ] ])
               {
                  await step (`${label}.first`,  () => browser .firstViewpoint (layer));
                  await step (`${label}.next`,   () => browser .nextViewpoint (layer));
                  await step (`${label}.next2`,  () => browser .nextViewpoint (layer));
                  await step (`${label}.next3`,  () => browser .nextViewpoint (layer));
                  await step (`${label}.next4`,  () => browser .nextViewpoint (layer));
                  await step (`${label}.prev`,   () => browser .previousViewpoint (layer));
                  await step (`${label}.last`,   () => browser .lastViewpoint (layer));
                  await step (`${label}.prev2`,  () => browser .previousViewpoint (layer));
               }

               // Not a layer node: the active layer (L1).
               await step ("noArg.first",  () => browser .firstViewpoint ());
               await step ("noArg.next",   () => browser .nextViewpoint ());
               await step ("box.next",     () => browser .nextViewpoint (Box));
               await step ("number.last",  () => browser .lastViewpoint (5));
               await step ("null.prev",    () => browser .previousViewpoint (null));
               await step ("vpAsLayer.first", () => browser .firstViewpoint (node ("B2")));
               await step ("string.next",  () => browser .nextViewpoint ("L2"));
               break;
            }
            case "change":
            {
               await step ("L1.A3",      () => browser .changeViewpoint (L1, "A3"));
               await step ("L1.AH",      () => browser .changeViewpoint (L1, "AH"));
               await step ("L2.A1",      () => browser .changeViewpoint (L2, "A1"));
               await step ("L2.B2",      () => browser .changeViewpoint (L2, "B2"));
               await step ("L1.B1",      () => browser .changeViewpoint (L1, "B1"));
               await step ("name.A2",    () => browser .changeViewpoint ("A2"));
               await step ("name.B1",    () => browser .changeViewpoint ("B1"));
               await step ("L3.A1",      () => browser .changeViewpoint (L3, "A1"));
               await step ("L1.none",    () => browser .changeViewpoint (L1, "None"));
               await step ("L1.root",    () => browser .changeViewpoint (L1, "R2"));
               await step ("box.A1",     () => browser .changeViewpoint (Box, "A1"));
               await step ("null.A3",    () => browser .changeViewpoint (null, "A3"));
               await step ("one.layer",  () => browser .changeViewpoint (L2));
               await step ("none",       () => browser .changeViewpoint ());
               await step ("number",     () => browser .changeViewpoint (L1, 5));
               break;
            }
            case "viewAll":
            {
               // The user position of each layer from its ProximitySensor (positions are in `state`).
               await spin (4);
               out .initial = { ... pos };

               await step ("viewAllNoLayer", () => browser .viewAll (0));
               out .afterNoLayer = { ... pos };
               await step ("viewAllL2", () => browser .viewAll (L2, 0));
               out .afterL2 = { ... pos };
               await step ("bindA3", () => browser .bindViewpoint (L1, node ("A3")));
               await step ("viewAllL1", () => browser .viewAll (L1, 0));
               out .afterL1 = { ... pos };
               await step ("bindA2", () => browser .bindViewpoint (L1, node ("A2")));
               await step ("viewAllBox", () => browser .viewAll (Box, 0));
               out .afterBox = { ... pos };
               await step ("bindA1", () => browser .bindViewpoint (L1, node ("A1")));
               await step ("viewAllNull", () => browser .viewAll (null, 0));
               await step ("viewAllL3", () => browser .viewAll (L3, 0));
               out .afterL3 = { ... pos };
               await step ("viewAllNaN", () => browser .viewAll (L2, "x"));
               await step ("viewAllTime", () => browser .viewAll (L2, 1));
               await step ("viewAllLayerOnly", () => browser .viewAll (L2));
               out .end = { ... pos };
               break;
            }
            case "update":
            {
               // A TimeSensor counts its events; a route carries an event of TR to TR2.
               let ticks = 0, cycles = 0;
               T .addFieldCallback ("t", "fraction_changed", () => ++ ticks);
               T .addFieldCallback ("c", "cycleTime", () => ++ cycles);

               const probe = async (label, frames = 8) =>
               {
                  const before = [ ticks, cycles ];

                  await spin (frames);
                  out [label] = [ ticks > before [0], cycles >= before [1], browser .isLive (), T .isActive, String (TR2 .translation) ];
               };

               await spin (4);
               await probe ("running");

               out .begin0 = attempt (() => browser .beginUpdate ());
               await probe ("beginWhileLive");

               out .end = attempt (() => browser .endUpdate ());
               out .afterEnd = [ browser .isLive (), T .isActive ];
               await probe ("stopped");

               // A field set by the page and a route while stopped.
               TR .translation = new X3D .SFVec3f (1, 2, 3);
               await probe ("setWhileStopped");

               // The viewpoint binds while stopped?
               out .stoppedBind = [ attempt (() => browser .bindViewpoint (L1, node ("A2"))) ];
               await spin (6);
               out .stoppedBind .push (state ());
               attempt (() => browser .nextViewpoint (L1));
               await spin (6);
               out .stoppedBind .push (state ());

               out .end2 = attempt (() => browser .endUpdate ());
               await probe ("end2");

               out .begin = attempt (() => browser .beginUpdate ());
               out .afterBegin = [ browser .isLive (), T .isActive ];
               await probe ("resumed");
               out .resumedBind = state ();

               // setLive.
               out .setLiveFalse = attempt (() => browser .setLive (false));
               await probe ("setLiveFalse");
               TR .translation = new X3D .SFVec3f (4, 5, 6);
               await probe ("setLiveFalseSet");
               out .setLiveTrue = attempt (() => browser .setLive (true));
               await probe ("setLiveTrue");
               out .begin2 = attempt (() => browser .beginUpdate ());
               await probe ("beginAgain");
               break;
            }
            case "world":
            {
               // endUpdate across a replaceWorld: the new world is not running; beginUpdate starts it.
               browser .endUpdate ();
               await spin (3);
               out .before = [ browser .isLive () ];

               await browser .loadURL (new X3D .MFString (WORLD));
               await spin (6);

               const scene2 = browser .currentScene;
               const T2 = scene2 .getNamedNode ("T");
               let ticks = 0;
               T2 .addFieldCallback ("t", "fraction_changed", () => ++ ticks);
               await spin (8);
               out .afterLoad = [ browser .isLive (), ticks > 0, T2 .isActive ];
               browser .beginUpdate ();
               await spin (8);
               out .afterBegin = [ browser .isLive (), ticks > 0, T2 .isActive ];
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
   }, [ name, WORLD ]);
}

const cases = Object .fromEntries ([ "api", "bind", "errors", "steps", "change", "viewAll", "update", "world" ] .map (name => [ name, page => scenario (page, name) ]));

for (const [ name, run ] of Object .entries (cases))
   cases [name] = async (page, env) => { await page .evaluate (pageAttempt); return run (page, env); };

export const CASE_GROUPS = {
   // The four layers of the world take 100 s to 180 s for one case on CI's SwiftShader (1 s to 3 s on a GPU): the cap is
   // 600 s, and the file has a CI shard of its own (shards.mjs).
   viewpointsApi: { context: DESKTOP, cases, timeout: 600_000 },
};
