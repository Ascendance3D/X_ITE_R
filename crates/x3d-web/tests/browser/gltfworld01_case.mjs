// The glTF world buffers 01 cases (finding F2 of PR #124): a glTF or GLB file loaded as the world with loadURL, whose
// buffers are external files or data URLs that load, fail, come late or are held, and the same files through an
// Inline. GLTF2Parser `await`s every buffer (`Promise .all`) before it builds the scene, so the world waits for them.
// oracle/x_ite-web-runtime-02.mjs (--suite gltfworld01) runs them against pinned X_ITE 16.5.0 and records the values;
// gltfworld01.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { orderedMessages, pageLoadURL, served, worldName } from "./loadurl01_case.mjs";

const DIR = "/tests/browser/worlds/gltfworld/";

// The points of every Coordinate of the world, numbers to 7 decimals (an Inline's scene is not walked).
const coordinates = page => page .evaluate (() =>
{
   const found = [ ];
   const walk  = node =>
   {
      if (!node)
         return;

      if (node .getNodeTypeName () === "Coordinate")
         return found .push (String (node .point));

      for (const name of [ "children", "geometry", "coord" ])
      {
         let value;

         try { value = node [name]; } catch { continue; }

         if (value ?.getNodeTypeName)
            walk (value);
         else if (value && typeof value !== "string" && typeof value .length === "number")
            for (const child of value) walk (child);
      }
   };

   for (const node of document .querySelector ("x3d-canvas") .browser .currentScene ?.rootNodes ?? [ ])
      walk (node);

   return found .map (points => points .replace (/-?\d+(?:\.\d+)?(?:e[-+]?\d+)?/gi, n => String (Number (Number (n) .toFixed (7)) || 0)));
});

// window .pending, or "pending" after `ms` milliseconds.
const race = (page, ms) => page .evaluate (ms => Promise .race ([ window .pending, new Promise (r => setTimeout (() => r ("pending"), ms)) ]), ms);

// What a world-level load left: the world, its browser events, its coordinates, the requests and the messages.
async function state (page, env, from)
{
   return {
      world: await worldName (page),
      events: await page .evaluate (() => window .events),
      coordinates: await coordinates (page),
      served: served (env, from),
      console: orderedMessages (page .log, env),
   };
}

// loadURL of the files `names` of the test directory: its result (within 5 s), then the state; with `picture`, the
// picture.
async function worldCase (page, env, names, picture)
{
   page .log .length = 0;

   const from = env .server .requests .length;

   await page .evaluate (pageLoadURL, names .map (name => `${env .base}${DIR}${name}`));

   const result = await race (page, 5000);

   await settle (page, 300);

   return { result, ... await state (page, env, from), ... picture && { picture: await env .picture (page) } };
}

// A world whose buffer comes late: whether loadURL settled and which world is current while the buffer is in flight,
// then after it came.
async function delayedCase (page, env, name)
{
   page .log .length = 0;

   const from = env .server .requests .length;

   await page .evaluate (pageLoadURL, [ `${env .base}${DIR}${name}` ]);

   const during = { result: await race (page, 150), world: await worldName (page) };
   const result = await race (page, 5000);

   await settle (page, 300);

   return { during, result, ... await state (page, env, from), picture: await env .picture (page) };
}

export const gltfCases = {
   // An external buffer file, as the world and through an Inline.
   external: (page, env) => worldCase (page, env, [ "external.gltf" ], true),
   externalInline: (page, env) => worldCase (page, env, [ "inline-external-gltf.wrl" ], true),

   // A base64 data URL buffer.
   data: (page, env) => worldCase (page, env, [ "data.gltf" ], true),
   dataInline: (page, env) => worldCase (page, env, [ "inline-data-gltf.wrl" ]),

   // A GLB without a BIN chunk whose buffer is an external file.
   glb: (page, env) => worldCase (page, env, [ "external.glb" ]),
   glbInline: (page, env) => worldCase (page, env, [ "inline-external-glb.wrl" ]),

   // The buffer file is missing.
   missing: (page, env) => worldCase (page, env, [ "missing.gltf" ]),
   missingInline: (page, env) => worldCase (page, env, [ "inline-missing-gltf.wrl" ]),

   // The buffer's base64 does not decode.
   badBase64: (page, env) => worldCase (page, env, [ "bad-base64.gltf" ]),
   badBase64Inline: (page, env) => worldCase (page, env, [ "inline-bad-base64-gltf.wrl" ]),

   // Two buffers: one that loads and one that is missing.
   oneMissing: (page, env) => worldCase (page, env, [ "one-missing.gltf" ]),

   // A glTF whose buffer is missing, then a glTF that loads: the next candidate.
   fallback: (page, env) => worldCase (page, env, [ "missing.gltf", "external.gltf" ]),

   // The buffer comes after 600 ms.
   delayed: (page, env) => delayedCase (page, env, "delayed.gltf"),

   // Two buffer files, the first one 300 ms late: both are requested at once.
   two: (page, env) => delayedCase (page, env, "two.gltf"),

   // A world whose buffer failed, then a world that loads, on one page.
   async failedThenValid (page, env)
   {
      return [ await worldCase (page, env, [ "missing.gltf" ]), await worldCase (page, env, [ "external.gltf" ]) ];
   },

   // A world whose buffer is held, then another loadURL before the buffer comes: the later world stays.
   async replaced (page, env)
   {
      env .server .arm ("gw-held");
      page .log .length = 0;

      const from = env .server .requests .length;

      await page .evaluate (pageLoadURL, [ `${env .base}${DIR}held.gltf` ]);

      for (let i = 0; i < 200 && !env .server .requests .slice (from) .some (r => r .path .startsWith ("/hold/gw-held/")); ++ i)
         await new Promise (r => setTimeout (r, 50));

      await page .evaluate (() => { window .first = window .pending; });
      await page .evaluate (pageLoadURL, [ `${env .base}${DIR}external.gltf` ]);

      const second = await race (page, 5000);

      env .server .release ("gw-held");
      await page .waitForTimeout (500);

      const first = await page .evaluate (() => Promise .race ([ window .first, new Promise (r => setTimeout (() => r ("pending"), 300)) ]));

      await settle (page, 300);

      return { first, second, ... await state (page, env, from) };
   },
};

export const CASE_GROUPS = {
   gltfWorld: { context: DESKTOP, cases: gltfCases },
};
