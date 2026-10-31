// Sound 02 tests of X_ITE_R's <x3d-canvas> that X_ITE has no reference for (issue #140): the page's audio clock across a
// replaceWorld while the canvas is detached, and the page-owned AudioContext at disposal. No pinned recording applies:
// the cases look at X_ITE_R's page integration (web/x_ite_r.mjs), not at what a Sound node does.

import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { CASE_TIMEOUT, DESKTOP, ONLY, openPage, server } from "./runtime02_harness.mjs";

// Keeps every AudioContext the page makes in window .contexts, before the world loads.
const KEEP_CONTEXTS = () =>
{
   const Base = window .AudioContext;

   window .contexts = [ ];
   window .AudioContext = class extends Base
   {
      constructor (... args)
      {
         super (... args);
         window .contexts .push (this);
      }
   };
};

// A clip that does not loop (0.25 s); the test starts it.
const WORLD = base => `#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Sound:2
Viewpoint { position 0 0 10 }
Sound {
  minFront 20 minBack 20 maxFront 40 maxBack 40
  source DEF A AudioClip { url "${base}/media/tone48k-stereo.wav" }
}
`;

const open = () => openPage (DESKTOP, { init: `(${KEEP_CONTEXTS}) ()` });

describe ("sound 02", { skip: Boolean (ONLY), timeout: CASE_TIMEOUT }, () =>
{
   test ("a world replaced while the canvas is detached keeps the page's audio clock", async () =>
   {
      const page = await open ();

      try
      {
         const result = await page .evaluate (async text =>
         {
            const canvas  = document .querySelector ("x3d-canvas");
            const browser = canvas .browser;
            const engine  = browser [Symbol .for ("X_ITE_R.engine")];
            const later   = ms => new Promise (resolve => setTimeout (resolve, ms));
            const clock   = () => JSON .parse (engine .audioGraph ()) .currentTime;

            await browser .replaceWorld (await browser .createX3DFromString (text));

            // The page's context runs (this evaluate has a user activation) and its clock passes the clip's length.
            while (!window .contexts .length || window .contexts [0] .currentTime < 1.5)
               await later (50);

            await later (200);

            const before = clock ();
            const parent = canvas .parentNode;

            canvas .remove ();

            // No frame runs while the canvas is detached, but the new world is installed (replaceWorld waits for a frame).
            const replaced = browser .replaceWorld (await browser .createX3DFromString (text));

            await later (500);

            const detached = clock ();

            parent .append (canvas);
            await replaced;

            const events = [ ];

            browser .currentScene .getNamedNode ("A") .addFieldCallback ("sound02", "isActive", value => events .push ({ value, at: performance .now () }));
            browser .currentScene .getNamedNode ("A") .startTime = browser .currentTime;
            await later (1000);

            return { before, detached, events, contexts: window .contexts .length };
         },
         WORLD (server .base));

         assert .equal (result .contexts, 1);
         assert .ok (result .before >= 1.5, `the clock before: ${result .before}`);
         assert .ok (result .detached >= result .before, `the new world's clock ${result .detached} is not the page's ${result .before}`);
         assert .deepEqual (result .events .map (e => e .value), [ true, false ], "the clip starts and ends once");
         assert .ok (result .events [1] .at - result .events [0] .at >= 150, `the clip played ${result .events [1] .at - result .events [0] .at} ms of its 250 ms`);
         assert .deepEqual (page .log .filter (line => line .startsWith ("pageerror")), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("dispose closes the page's AudioContext once, and not an AudioContext of the page's own", async () =>
   {
      const page = await open ();

      try
      {
         const result = await page .evaluate (async text =>
         {
            const browser = document .querySelector ("x3d-canvas") .browser;
            const later   = ms => new Promise (resolve => setTimeout (resolve, ms));

            await browser .replaceWorld (await browser .createX3DFromString (text));

            while (!window .contexts .length)
               await later (50);

            // The context stays through a replaceWorld.
            const context = window .contexts [0];

            await browser .replaceWorld (await browser .createX3DFromString (text));
            await later (200);

            const reused = { count: window .contexts .length, state: context .state, same: window .contexts [0] === context };
            const own    = new AudioContext ();

            browser .dispose ();
            browser .dispose ();

            for (let i = 0; i < 100 && context .state !== "closed"; ++ i)
               await later (20);

            return { reused, closed: context .state, own: own .state, count: window .contexts .length };
         },
         WORLD (server .base));

         assert .deepEqual (result .reused, { count: 1, state: result .reused .state, same: true });
         assert .notEqual (result .reused .state, "closed");
         assert .equal (result .closed, "closed");
         assert .notEqual (result .own, "closed");
         assert .equal (result .count, 2, "the page's own context and the browser's, and no new one after dispose");
         assert .deepEqual (page .log .filter (line => line .startsWith ("pageerror")), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   // The following tests check the engine's graph against values worked out from the source of the pinned X_ITE 16.5.0
   // (Sound.traverse, AudioClip.set_pitch__, X3DSoundContext.addSoundDestination, BrowserOptions.set_SoundIntensity__,
   // ListenerPointSource.traverse). There is no pinned recording: they read the engine's graph (audioGraph ()), so they
   // show what the engine would give to the page's audio output, not what the page hears.

   // Loads `text` and gives the graph once `ready (graph)` holds.
   const graphOf = (page, text, ready = "graph .nodes .length > 1") => page .evaluate (async ([ text, ready ]) =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const engine  = browser [Symbol .for ("X_ITE_R.engine")];
      const later   = ms => new Promise (resolve => setTimeout (resolve, ms));
      const test    = new Function ("graph", `return ${ready};`);

      await browser .replaceWorld (await browser .createX3DFromString (text));

      let graph = null;

      for (let i = 0; i < 200; ++ i)
      {
         const raw = engine .audioGraph ();

         graph = raw && JSON .parse (raw);

         if (graph && test (graph))
            break;

         await later (50);
      }

      return graph;
   },
   [ text, ready ]);

   // The Sound node's gains: the stereo gain (spatialize FALSE: the pan is 0.5 and the rotation 0).
   const stereoGain = graph =>
   {
      const into = graph .connections .filter (([ , , to ]) => to === graph .destination) .map (([ from ]) => from);
      const sd   = graph .nodes .find (n => n .id === into [0]);
      const next = graph .connections .filter (([ , , to ]) => to === sd .id) .map (([ from ]) => graph .nodes .find (n => n .id === from));

      return { sd: sd .gain, stereo: next .find (n => n .kind === "GainNode") .gain };
   };

   const SOUND = (viewpoint, sound, clip = "loop TRUE") => `#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Sound:2
${viewpoint}
Sound { spatialize FALSE ${sound}
  source AudioClip { ${clip} url "${server .base}/media/tone48k-stereo.wav" } }
`;

   test ("Sound range: the gain follows minFront/maxFront in front of the sound and minBack/maxBack behind it", async () =>
   {
      const page = await open ();

      try
      {
         const range = "minFront 2 minBack 4 maxFront 12 maxBack 20";

         // The viewer is 10 away on the axis. Gain: (max - distance) / (max - min) on the side of the viewer.
         const front  = await graphOf (page, SOUND ("Viewpoint { position 0 0 10 }", range), "graph .nodes .some (n => n .kind === 'GainNode' && n .gain > 0)");
         const back   = await graphOf (page, SOUND ("Viewpoint { position 0 0 -10 orientation 0 1 0 3.14159265358979 }", range), "graph .nodes .some (n => n .kind === 'GainNode' && n .gain > 0)");
         const inside = await graphOf (page, SOUND ("Viewpoint { position 0 0 10 }", "minFront 20 minBack 20 maxFront 40 maxBack 40"), "graph .nodes .some (n => n .kind === 'GainNode' && n .gain > 0)");
         const out    = await graphOf (page, SOUND ("Viewpoint { position 0 0 10 }", "minFront 1 minBack 1 maxFront 5 maxBack 5"), "graph .nodes .length > 1");

         assert .ok (Math .abs (stereoGain (front) .stereo - 0.2) < 1e-5, `front: ${stereoGain (front) .stereo}`);
         assert .ok (Math .abs (stereoGain (back) .stereo - 0.625) < 1e-5, `back: ${stereoGain (back) .stereo}`);
         assert .equal (stereoGain (inside) .stereo, 1, "inside the minimum ellipsoid");
         assert .equal (stereoGain (out) .stereo, 0, "outside the maximum ellipsoid");
         assert .deepEqual (page .log .filter (line => line .startsWith ("pageerror")), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("Sound priority is stored and does not change the graph; the pitch of an AudioClip sets the playback rate", async () =>
   {
      const page = await open ();

      try
      {
         const ready  = "graph .nodes .some (n => n .kind === 'AudioBufferSourceNode' && n .source .started)";
         const wide   = "minFront 20 minBack 20 maxFront 40 maxBack 40";
         const plain  = await graphOf (page, SOUND ("Viewpoint { position 0 0 10 }", wide), ready);
         const high   = await graphOf (page, SOUND ("Viewpoint { position 0 0 10 }", `${wide} priority 1`), ready);
         const stored = await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .currentScene .rootNodes [1] .priority);
         const fast   = await graphOf (page, SOUND ("Viewpoint { position 0 0 10 }", wide, "loop TRUE pitch 1.5"), ready);
         const slow   = await graphOf (page, SOUND ("Viewpoint { position 0 0 10 }", wide, "loop TRUE pitch -1"), ready);
         const rate   = graph => graph .nodes .find (n => n .kind === "AudioBufferSourceNode") .playbackRate;
         const gains  = graph => graph .nodes .map (n => [ n .kind, n .gain ]);

         assert .deepEqual (gains (high), gains (plain), "priority is not read by X_ITE");
         assert .equal (rate (plain), 1);
         assert .equal (rate (fast), 1.5);
         assert .equal (rate (slow), 0, "AudioClip.set_pitch__: Math .max (pitch, 0)");

         assert .equal (stored, 1, "the field keeps its value");
         assert .deepEqual (page .log .filter (line => line .startsWith ("pageerror")), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("Mute and SoundIntensity set the gain of the sound destinations", async () =>
   {
      const page = await open ();

      try
      {
         const wide  = "minFront 20 minBack 20 maxFront 40 maxBack 40";
         const graph = await graphOf (page, SOUND ("Viewpoint { position 0 0 10 }", wide), "graph .nodes .some (n => n .kind === 'GainNode' && n .gain > 0)");

         assert .equal (stereoGain (graph) .sd, 1);

         const read = async options =>
         {
            return page .evaluate (async options =>
            {
               const browser = document .querySelector ("x3d-canvas") .browser;
               const engine  = browser [Symbol .for ("X_ITE_R.engine")];
               const later   = ms => new Promise (resolve => setTimeout (resolve, ms));
               const result  = [ ];

               for (const [ name, value ] of options)
               {
                  browser .setBrowserOption (name, value);
                  await later (300);

                  const graph = JSON .parse (engine .audioGraph ());
                  const into  = graph .connections .filter (([ , , to ]) => to === graph .destination) .map (([ from ]) => from);

                  result .push (graph .nodes .find (n => n .id === into [0]) .gain);
               }

               return result;
            },
            options);
         };

         const gains = await read ([ [ "SoundIntensity", 0.5 ], [ "Mute", true ], [ "Mute", false ], [ "SoundIntensity", 3 ], [ "SoundIntensity", -1 ] ]);

         assert .deepEqual (gains, [ 0.5, 0, 0.5, 1, 0 ]);
         assert .deepEqual (page .log .filter (line => line .startsWith ("pageerror")), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("ListenerPointSource sets the audio listener's position, forward and up", async () =>
   {
      const page = await open ();

      try
      {
         const text = `#X3D V4.0 utf8
PROFILE Full
COMPONENT Sound:3
Viewpoint { position 0 0 0 }
ListenerPointSource { position 1 2 3 orientation 0 1 0 1.5707963267949 trackCurrentView FALSE }
Sound { source AudioClip { loop TRUE url "${server .base}/media/tone48k-stereo.wav" } }
`;
         const graph = await graphOf (page, text, "graph .listener .some (v => v !== 0)");
         const near  = (a, b) => a .length === b .length && a .every ((v, i) => Math .abs (v - b [i]) < 1e-5);

         // forward = orientation * (0, 0, -1), up = orientation * (0, 1, 0): a quarter turn about y.
         assert .ok (near (graph .listener, [ 1, 2, 3, -1, 0, 0, 0, 1, 0 ]), JSON .stringify (graph .listener));
         assert .deepEqual (page .log .filter (line => line .startsWith ("pageerror")), [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });
});
