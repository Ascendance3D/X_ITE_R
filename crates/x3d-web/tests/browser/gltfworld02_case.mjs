// The glTF world buffers 02 cases (the CodeRabbit finding of PR #126): a glTF world with two buffers, one of which
// fails while the other is still held by the test server. GLTF2Parser `await`s `Promise .all` of the buffers, which
// rejects at the first failure; the held buffer shows whether the candidate is rejected before it comes. The holds
// make the order independent of the machine's speed: a held response comes only when the case releases it.
// oracle/x_ite-web-runtime-02.mjs (--suite gltfworld02) runs them against pinned X_ITE 16.5.0 and records the values;
// gltfworld02.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { orderedMessages, pageLoadURL, served, worldName } from "./loadurl01_case.mjs";

const DIR = "/tests/browser/worlds/gltfworld/";

// window .pending, or "pending" after `ms` milliseconds.
const race = (page, ms) => page .evaluate (ms => Promise .race ([ window .pending, new Promise (r => setTimeout (() => r ("pending"), ms)) ]), ms);

// Wait until the server got a request for every hold of `keys` since `from`.
async function requested (env, keys, from)
{
   for (let i = 0; i < 200; ++ i)
   {
      const paths = env .server .requests .slice (from) .map (r => r .path);

      if (keys .every (key => paths .some (path => path .startsWith (`/hold/${key}/`))))
         return;

      await new Promise (r => setTimeout (r, 50));
   }

   throw new Error (`the held buffers ${keys} were not requested`);
}

// What a world-level load left: the world, its browser events, the requests and the messages.
async function state (page, env, from)
{
   return {
      world: await worldName (page),
      events: await page .evaluate (() => window .events),
      served: served (env, from),
      console: orderedMessages (page .log, env),
   };
}

// loadURL of the files `names`, whose buffers of the holds `keys` are held: the state while they are held (loadURL
// settled within 2 s or not), then the state after each hold was released in turn.
async function heldCase (page, env, names, keys)
{
   keys .forEach (key => env .server .arm (key));
   page .log .length = 0;

   const from = env .server .requests .length;

   await page .evaluate (pageLoadURL, names .map (name => `${env .base}${DIR}${name}`));
   await requested (env, keys, from);

   const during = { result: await race (page, 2000), ... await state (page, env, from) };

   for (const key of keys)
   {
      env .server .release (key);
      await page .waitForTimeout (500);
   }

   const result = await race (page, 5000);

   await settle (page, 300);

   return { during, result, ... await state (page, env, from) };
}

export const gltfCases = {
   // Buffer A fails while buffer B is held, then B loads.
   failThenHeld: (page, env) => heldCase (page, env, [ "fail-held.gltf" ], [ "gx-a" ]),

   // Buffer A is held while buffer B fails, then A loads.
   heldThenFail: (page, env) => heldCase (page, env, [ "held-fail.gltf" ], [ "gx-b" ]),

   // Both buffers fail: B first, A after it is released.
   bothFailSecondFirst: (page, env) => heldCase (page, env, [ "held-fail-fail.gltf" ], [ "gx-c" ]),

   // Both buffers fail: A first, B after it is released.
   bothFailFirstFirst: (page, env) => heldCase (page, env, [ "fail-fail-held.gltf" ], [ "gx-d" ]),

   // Buffer A fails while B is held, and a glTF that loads is the next candidate.
   fallback: (page, env) => heldCase (page, env, [ "fail-held-fallback.gltf", "external.gltf" ], [ "gx-e" ]),

   // Both buffers are held, then a new loadURL replaces the world; then A fails and B loads.
   async replaced (page, env)
   {
      env .server .arm ("gx-f");
      env .server .arm ("gx-g");
      page .log .length = 0;

      const from = env .server .requests .length;

      await page .evaluate (pageLoadURL, [ `${env .base}${DIR}held-held.gltf` ]);
      await requested (env, [ "gx-f", "gx-g" ], from);
      await page .evaluate (() => { window .first = window .pending; });
      await page .evaluate (pageLoadURL, [ `${env .base}${DIR}external.gltf` ]);

      const second = await race (page, 5000);

      env .server .release ("gx-f");
      await page .waitForTimeout (500);
      env .server .release ("gx-g");
      await page .waitForTimeout (500);

      const first = await page .evaluate (() => Promise .race ([ window .first, new Promise (r => setTimeout (() => r ("pending"), 300)) ]));

      await settle (page, 300);

      return { first, second, ... await state (page, env, from) };
   },
};

export const CASE_GROUPS = {
   gltfWorldHeld: { context: DESKTOP, cases: gltfCases },
};
