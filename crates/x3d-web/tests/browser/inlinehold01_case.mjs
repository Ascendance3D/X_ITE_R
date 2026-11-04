// The Inline hold 01 cases (ADR 0014, section 4): an Inline whose request is still in flight when its url is set again must never
// show that request's scene. X_ITE's Inline.loadData () calls `fileLoader ?.abort ()` before it starts the new FileLoader
// (Components/Networking/Inline.js; InputOutput/FileLoader.js abort ()), so a response of the older request that comes later is
// dropped. Native synthetic fetches are synchronous and cannot hold a request, so only the browser build, whose page fetch is
// asynchronous and whose server can hold a response (server.mjs `arm` and `release`), shows it.
// oracle/x_ite-web-runtime-02.mjs (--suite inlinehold01) runs the cases against pinned X_ITE 16.5.0 and records the values;
// inlinehold01.test.mjs runs them against X_ITE_R (with `<x3d-canvas scripts="true">`) and compares (runtime02_harness.mjs).
//
// The world is one Inline `Inl` whose url starts as scene A (served at once). The scenes A, B and C are
// worlds/inline-hold/{a,b,c}.wrl: a root named RootA, RootB or RootC and a Script that prints `A initialize`, `B initialize` or
// `C initialize`. B (and in one case C) is requested through a hold, so the server answers it only when the case releases it.
// What a step records: the console lines it produced (the prints and the engines' messages), the Inline's url, the scene that is
// shown (the color at the center of the canvas picture: A red, B blue, C green, none), and at the end the requests the server got for
// the three scenes. A page cannot read an Inline's load state or its load count (private fields), so those are not recorded.

import { DESKTOP } from "./runtime02_case.mjs";

const DIR = "/tests/browser/worlds/inline-hold/";

async function scenario (page, env, name)
{
   const stamp = Date .now ();
   const key   = letter => `xrih${name}${letter}${stamp}`;
   const url   = (letter, held) => `${env .base}${held ? `/hold/${key (letter)}` : ""}${DIR}${letter}.wrl`;
   const text  = `#VRML V2.0 utf8\nDEF Inl Inline { url "${url ("a")}" }\n`;

   page .log .length = 0;

   const from = env .server .requests .length;

   await page .evaluate (async ([ text, origin ]) =>
   {
      const X3D     = window .X3D_NS;
      const canvas  = document .querySelector ("x3d-canvas");
      const browser = X3D .getBrowser (canvas);
      const spin    = async n => { for (let i = 0; i < n; ++ i) await new Promise (r => requestAnimationFrame (() => r ())); };

      canvas .setAttribute ("scripts", "true");

      window .seq = [ ];
      window .xr  = { browser, spin, origin };

      for (const type of [ "log", "warn", "error" ])
         console [type] = (... args) => window .seq .push (`${type}: ${args .join (" ") .split ("\n") [0] .split (origin) .join ("http://test")}`);

      await browser .replaceWorld (await browser .createX3DFromString (text));
      await spin (20);
   },
   [ text, env .base ]);

   const requested = async letter =>
   {
      for (let i = 0; i < 100; ++ i)
      {
         if (env .server .requests .slice (from) .some (r => r .path .includes (`${DIR}${letter}.wrl`)))
            return;

         await new Promise (r => setTimeout (r, 50));
      }

      throw new Error (`no request of ${letter}.wrl`);
   };

   const steps = { };

   // The color at the center of the picture, named by its strongest channel when that is above 200 and the others are below 60.
   const shown = png => page .evaluate (async base64 =>
   {
      const bitmap = await createImageBitmap (new Blob ([ Uint8Array .from (atob (base64), c => c .charCodeAt (0)) ], { type: "image/png" }));
      const canvas = new OffscreenCanvas (bitmap .width, bitmap .height);
      const gl     = canvas .getContext ("2d");

      gl .drawImage (bitmap, 0, 0);

      const [ r, g, b ] = gl .getImageData (bitmap .width >> 1, bitmap .height >> 1, 1, 1) .data;

      if (r > 200 && g < 60 && b < 60) return "red";
      if (g > 200 && r < 60 && b < 60) return "green";
      if (b > 200 && r < 60 && g < 60) return "blue";

      return `other ${r} ${g} ${b}`;
   },
   png .toString ("base64"));

   // The state after `label`: the console since the last step and what the Inline reports.
   const snapshot = async label =>
   {
      await page .evaluate (() => window .xr .spin (30));
      await page .waitForTimeout (400);

      steps [label] = await page .evaluate (() =>
      {
         const node = window .xr .browser .currentScene .getNamedNode ("Inl");

         return { url: Array .from (node .url) .map (u => u .split (window .xr .origin) .join ("http://test") .replace (/\/hold\/[^/]+/, "/hold/K")), seq: window .seq .splice (0) };
      });

      steps [label] .shown = await shown (await env .picture (page));
   };

   const setUrl = (letter, held) => page .evaluate (u => { window .xr .browser .currentScene .getNamedNode ("Inl") .url = new window .X3D_NS .MFString (u); }, url (letter, held));

   await snapshot ("world");

   // A failed step must not leave a held response open: the server's close () would wait for it.
   try {
   switch (name)
   {
      // The request for B is held, then C is made and arrives, then B is released.
      case "newerFirst":
         env .server .arm (key ("b"));
         await setUrl ("b", true);
         await requested ("b");
         await snapshot ("bHeld");
         await setUrl ("c", false);
         await snapshot ("cArrived");
         env .server .release (key ("b"));
         await snapshot ("bReleased");
         break;
      // C is made while B is held, and C is held too; B is released first, then C.
      case "olderFirst":
         env .server .arm (key ("b"));
         env .server .arm (key ("c"));
         await setUrl ("b", true);
         await requested ("b");
         await snapshot ("bHeld");
         await setUrl ("c", true);
         await requested ("c");
         await snapshot ("cHeld");
         env .server .release (key ("b"));
         await snapshot ("bReleased");
         env .server .release (key ("c"));
         await snapshot ("cReleased");
         break;
      // The url goes back to A while B is held, then B is released.
      case "backToA":
         env .server .arm (key ("b"));
         await setUrl ("b", true);
         await requested ("b");
         await snapshot ("bHeld");
         await setUrl ("a", false);
         await snapshot ("aAgain");
         env .server .release (key ("b"));
         await snapshot ("bReleased");
         break;
   }
   } finally {
      for (const letter of ["b", "c"])
         env .server .release (key (letter));
   }

   const served = env .server .requests .slice (from)
      .map (r => r .path .replace (/\/hold\/[^/]+/, "/hold/K"))
      .filter (path => path .includes (DIR));

   for (const letter of [ "a", "b", "c" ])
      env .server .release (key (letter));

   return { steps, served };
}

export const CASE_GROUPS = {
   inlineHold: { context: DESKTOP, cases: Object .fromEntries ([ "newerFirst", "olderFirst", "backToA" ] .map (name => [ name, (page, env) => scenario (page, env, name) ])) },
};
