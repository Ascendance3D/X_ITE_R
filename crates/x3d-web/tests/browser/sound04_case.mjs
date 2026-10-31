// The sound 04 cases (issue #140): what the page's output carries, as a level. sound03_case.mjs has the `audible` cases that say
// only whether the output is above 1e-3; these measure the level, so a clip's gain, Mute and SoundIntensity, a stopped clip and a
// world that is replaced can be compared with the pinned X_ITE 16.5.0 recording in oracle/web-browser-sound-04-baseline.
//
// The statistic (fixed before any X_ITE_R measurement): installTap (sound03_case.mjs) puts an AnalyserNode (fftSize 2048, a
// window of 2048 samples) on every connection into the page's AudioContext destination. Every 20 ms the page takes the root
// mean square of each window and keeps the loudest. A case resets the sums after its setup and settles, waits 1500 ms, and
// takes the mean of those values (about 75 polls). `rms` is that mean rounded to 4 decimals. A first version took the largest
// value instead, which varied by 0.004 from run to run in X_ITE itself (0.215 to 0.219); the mean varies less.
// X_ITE's own runs differ by up to 0.003 (8 runs, measured before any X_ITE_R run); the recorder keeps the mean.
// The comparison allows a tolerance (runtime02_harness.mjs).

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { sceneCase } from "./loadurl01_case.mjs";
import { NODE_CLASSES, gesture, installCapture, pageFacts } from "./sound01_case.mjs";
import { CLIP, FRONT, SOUND, WIDE, WORLD3, installTap, setOption } from "./sound03_case.mjs";

const named = (page, name, field, value) => page .evaluate (([ name, field, value ]) =>
{
   const b = document .querySelector ("x3d-canvas") .browser;

   b .currentScene .getNamedNode (name) [field] = value ?? b .currentTime;
},
[ name, field, value ]);

const replace = async (page, text) =>
{
   await page .evaluate (async text =>
   {
      const b = document .querySelector ("x3d-canvas") .browser;

      await b .replaceWorld (await b .createX3DFromString (text));
   },
   text);
};

const NAMED_CLIP = "DEF C AudioClip { url \"BASE/media/tone48k-stereo.wav\" loop TRUE }";

// Tap installed, `before` run (an option set before the world exists), world loaded, gesture, `setup` run; then the level.
async function level (page, env, text, { before, setup } = { })
{
   await page .evaluate (installTap);
   await page .evaluate (installCapture, NODE_CLASSES);
   await before ?.(page);

   const c = await sceneCase (page, env, text);

   await gesture (page);
   await setup ?.(page);
   await settle (page, 300);
   await page .evaluate (() => { window .tapMax = 0; window .tapSum = 0; window .tapCount = 0; });
   await settle (page, 1500);

   const facts = await pageFacts (page);
   const tap   = await page .evaluate (() => ({ tapped: window .tapped, rms: window .tapSum / Math .max (window .tapCount, 1) }));

   return {
      loaded: await c .loaded (),
      contextRunning: facts .states .length > 0 && facts .states .every (s => s === "running"),
      connectedToOutput: tap .tapped > 0,
      rms: Number (tap .rms .toFixed (Number (process .env .XR_RMS_DIGITS ?? 4))),
   };
}

export const soundCases = {
   // A looping clip in range: the reference level.
   audibleClip: (page, env) => level (page, env, WORLD3 (FRONT, SOUND (WIDE))),

   // Mute set after the clip plays.
   audibleMuted: (page, env) => level (page, env, WORLD3 (FRONT, SOUND (WIDE)), { setup: page => setOption (page, "Mute", true) }),

   // SoundIntensity set after the clip plays: a gain of 0.5 halves the level.
   audibleIntensity: (page, env) => level (page, env, WORLD3 (FRONT, SOUND (WIDE)), { setup: page => setOption (page, "SoundIntensity", 0.5) }),

   // Mute set before the world is loaded: the stored option applies when the engine becomes ready.
   audibleMutedBeforeLoad: (page, env) => level (page, env, WORLD3 (FRONT, SOUND (WIDE)), { before: page => setOption (page, "Mute", true) }),
   audibleIntensityBeforeLoad: (page, env) => level (page, env, WORLD3 (FRONT, SOUND (WIDE)), { before: page => setOption (page, "SoundIntensity", 0.5) }),

   // Mute set on a canvas the moment it is made, before its engine is ready: the stored option applies when it becomes ready.
   async audibleMutedBeforeReady (page, env)
   {
      await page .evaluate (installTap);
      await page .evaluate (installCapture, NODE_CLASSES);
      await page .evaluate (async text =>
      {
         const element = document .createElement ("x3d-canvas");

         element .setAttribute ("style", "display:block;width:200px;height:150px");
         document .body .append (element);
         // At once: the element has no engine yet in X_ITE_R (it is made asynchronously).
         element .browser .setBrowserOption ("Mute", true);

         const browser = element .browser;

         await browser .replaceWorld (await browser .createX3DFromString (text));
      },
      WORLD3 (FRONT, SOUND (WIDE)) .replaceAll ('"BASE/', `"${env .base}/`));

      await gesture (page);
      await settle (page, 300);
      await page .evaluate (() => { window .tapMax = 0; window .tapSum = 0; window .tapCount = 0; });
      await settle (page, 1500);

      const facts = await pageFacts (page);
      const tap   = await page .evaluate (() => ({ tapped: window .tapped, rms: window .tapSum / Math .max (window .tapCount, 1) }));

      return {
         contextRunning: facts .states .length > 0 && facts .states .every (s => s === "running"),
         connectedToOutput: tap .tapped > 0,
         rms: Number (tap .rms .toFixed (Number (process .env .XR_RMS_DIGITS ?? 4))),
      };
   },

   // Mute set, then cleared: the signal comes back.
   async audibleUnmuted (page, env)
   {
      return level (page, env, WORLD3 (FRONT, SOUND (WIDE)), { setup: async page => { await setOption (page, "Mute", true); await settle (page, 300); await setOption (page, "Mute", false); } });
   },

   // The clip is stopped by its stopTime: silent afterwards.
   audibleStopped: (page, env) => level (page, env, WORLD3 (FRONT, SOUND (WIDE, NAMED_CLIP)), { setup: page => named (page, "C", "stopTime") }),

   // The clip is stopped, then started again: audible again.
   async audibleRestarted (page, env)
   {
      return level (page, env, WORLD3 (FRONT, SOUND (WIDE, NAMED_CLIP)),
         { setup: async page => { await named (page, "C", "stopTime"); await settle (page, 400); await named (page, "C", "startTime"); } });
   },

   // The world is replaced by an empty one while the clip plays: silent afterwards.
   async audibleReplacedByEmpty (page, env)
   {
      return level (page, env, WORLD3 (FRONT, SOUND (WIDE)), { setup: page => replace (page, "#X3D V4.0 utf8\nPROFILE Interchange\nGroup { }\n") });
   },

   // The world is replaced by another playing world (half intensity): the level of the new one alone.
   async audibleReplacedByPlaying (page, env)
   {
      return level (page, env, WORLD3 (FRONT, SOUND (WIDE)),
         { setup: page => replace (page, WORLD3 (FRONT, SOUND (`${WIDE} intensity 0.5`)) .replaceAll ('"BASE/', `"${env .base}/`)) });
   },

   // An OscillatorSource in an AudioDestination node.
   audibleOscillator: (page, env) => level (page, env, WORLD3 (FRONT, "AudioDestination { children DEF O OscillatorSource { frequency 440 } }", "Sound:3"),
      { setup: page => named (page, "O", "startTime") }),
};

// The cases of Sound 04 run with the autoplay policy that asks for a gesture, as the sound 03 cases do.
export const CASE_GROUPS = {
   soundLevel: { context: DESKTOP, browserArgs: [ "--autoplay-policy=document-user-activation-required" ], cases: soundCases },
};
