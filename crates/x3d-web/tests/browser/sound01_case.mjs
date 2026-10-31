// The sound 01 cases (issue #140): an AudioClip that plays through a Sound node in a page, the audio graph it builds,
// the clock of the audio context, and the autoplay unlock (X3DSoundContext: `audioContext .resume ()` when the context is
// made, again on a user gesture on the canvas).
// oracle/x_ite-web-runtime-02.mjs (--suite sound01) runs them against pinned X_ITE 16.5.0 and records the values;
// sound01.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).
//
// The graph is compared as a page reads Web Audio nodes. X_ITE's nodes are the page's own: installCapture wraps the
// page's Web Audio constructors and the connect, disconnect, start and stop methods before the world loads. X_ITE_R's
// nodes are the engine's (the Rust Web Audio port, x3d-runtime/src/sound): its `audioGraph ()` diagnostic gives the
// same snapshot. normalizeGraph numbers both the same way, from the destination. The page-level facts (`page`: the
// AudioContexts the page made, their state and resume calls) come from the capture in both engines.
//
// The clip is the synthetic tone of REA batch 05 (oracle/synthetic-rea-b05/media, served at /media/).

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { sceneCase } from "./loadurl01_case.mjs";

// The Web Audio classes a page can construct: the capture wraps each one.
const NODE_CLASSES = [
   "AnalyserNode", "AudioBufferSourceNode", "BiquadFilterNode", "ChannelMergerNode", "ChannelSplitterNode", "ConvolverNode",
   "DelayNode", "DynamicsCompressorNode", "GainNode", "MediaStreamAudioDestinationNode", "MediaStreamAudioSourceNode",
   "OscillatorNode", "PannerNode", "StereoPannerNode", "WaveShaperNode",
];

// Runs in the page before the world loads. window .audioCapture .graph () gives the snapshot of the first AudioContext
// in the form of X_ITE_R's audioGraph (); window .audioCapture .page () the page-level facts.
function installCapture (classes)
{
   const nodes    = [ ];
   const info     = new Map ();
   const edges    = [ ];
   const contexts = [ ];
   const resumes  = [ ];

   const add = (node, kind) =>
   {
      if (!info .has (node))
      {
         info .set (node, { id: nodes .length, serial: nodes .length, kind });
         nodes .push (node);
      }

      return info .get (node);
   };

   const BaseContext = window .AudioContext;

   window .AudioContext = class extends BaseContext
   {
      constructor (... args)
      {
         super (... args);
         contexts .push (this);
         add (this .destination, "AudioDestinationNode");
      }

      resume ()
      {
         resumes .push (this .state);
         return super .resume ();
      }
   };

   for (const name of classes)
   {
      const Base = window [name];

      if (!Base)
         continue;

      window [name] = class extends Base
      {
         constructor (... args)
         {
            super (... args);
            add (this, name);
         }
      };
   }

   const connect = AudioNode .prototype .connect;

   AudioNode .prototype .connect = function (destination, output = 0, input = 0)
   {
      const result = connect .call (this, destination, output, input);

      // A connection to an AudioParam is not a node edge; the cases make none.
      if (destination instanceof AudioNode)
      {
         const from = add (this, this .constructor .name) .id, to = add (destination, destination .constructor .name) .id;

         if (!edges .some (e => e [0] === from && e [1] === output && e [2] === to && e [3] === input))
            edges .push ([ from, output, to, input ]);
      }

      return result;
   };

   const disconnect = AudioNode .prototype .disconnect;

   AudioNode .prototype .disconnect = function (... args)
   {
      const result = disconnect .apply (this, args);
      const from   = add (this, this .constructor .name) .id;
      const [ a, output, input ] = args;
      const keep   = e =>
      {
         if (e [0] !== from)
            return true;

         if (!args .length)
            return false;

         if (typeof a === "number")
            return e [1] !== a;

         if (!(a instanceof AudioNode) || e [2] !== info .get (a) ?.id)
            return true;

         return (output !== undefined && e [1] !== output) || (input !== undefined && e [3] !== input);
      };

      edges .splice (0, edges .length, ... edges .filter (keep));
      return result;
   };

   const start = AudioBufferSourceNode .prototype .start;

   AudioBufferSourceNode .prototype .start = function (when = 0, offset = 0, ... rest)
   {
      add (this, "AudioBufferSourceNode") .started = [ when, offset ];
      return start .call (this, when, offset, ... rest);
   };

   const stop = AudioScheduledSourceNode .prototype .stop;

   AudioScheduledSourceNode .prototype .stop = function (when = 0)
   {
      add (this, this .constructor .name) .stopped = when;
      return stop .call (this, when);
   };

   window .audioCapture =
   {
      graph ()
      {
         const context = contexts [0];

         if (!context)
            return null;

         return {
            nodes: nodes .map (node =>
            {
               const i      = info .get (node);
               const source = i .kind === "AudioBufferSourceNode";
               const buffer = source ? node .buffer : null;

               return {
                  id: i .id,
                  serial: i .serial,
                  kind: i .kind,
                  numberOfInputs: node .numberOfInputs,
                  numberOfOutputs: node .numberOfOutputs,
                  channelCount: node .channelCount,
                  channelCountMode: node .channelCountMode,
                  channelInterpretation: node .channelInterpretation,
                  gain: i .kind === "GainNode" ? node .gain .value : null,
                  playbackRate: source ? node .playbackRate .value : null,
                  source: source ? {
                     buffer: buffer ? { numberOfChannels: buffer .numberOfChannels, length: buffer .length, sampleRate: buffer .sampleRate } : null,
                     loop: node .loop,
                     loopStart: node .loopStart,
                     loopEnd: node .loopEnd,
                     started: i .started ?? null,
                     stopped: i .stopped ?? null,
                  } : null,
               };
            }),
            connections: edges .map (e => [ ... e ]),
            destination: info .get (context .destination) .id,
            currentTime: context .currentTime,
            sampleRate: context .sampleRate,
         };
      },
      page ()
      {
         return {
            contexts: contexts .length,
            states: contexts .map (c => c .state),
            resumeCalls: [ ... resumes ],
         };
      },
   };
}

// The audio graph of the world, in the page: X_ITE's from the capture, X_ITE_R's from the engine.
function pageGraph ()
{
   const engine = document .querySelector ("x3d-canvas") .browser [Symbol .for ("X_ITE_R.engine")];

   if (engine)
   {
      const json = engine .audioGraph ();

      return json === undefined ? null : JSON .parse (json);
   }

   return window .audioCapture .graph ();
}

const rawGraph = page => page .evaluate (pageGraph);

// The fields of AudioClip A that do not depend on the frame times, in the page.
function pageFields ()
{
   const A = document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("A");

   return { isActive: A .isActive, isPaused: A .isPaused, duration: Number (A .duration_changed .toFixed (4)) };
}

// An init script (with installCapture before it): when the page is ready, load the world `text` and, a second later,
// keep the state in window .soundBefore and log "sound01: before". It runs only in the document, with no evaluate.
function loadWithoutActivation (text)
{
   const later = ms => new Promise (resolve => setTimeout (resolve, ms));

   (async () =>
   {
      while (!window .ready || !document .querySelector ("x3d-canvas") ?.browser)
         await later (50);

      const browser = document .querySelector ("x3d-canvas") .browser;

      await browser .replaceWorld (await browser .createX3DFromString (text));
      await later (1000);

      const engine = browser [Symbol .for ("X_ITE_R.engine")];
      const json   = engine ?.audioGraph ();
      const A      = browser .currentScene .getNamedNode ("A");

      window .soundBefore = {
         activated: navigator .userActivation .hasBeenActive,
         fields: { isActive: A .isActive, isPaused: A .isPaused, duration: Number (A .duration_changed .toFixed (4)) },
         graph: engine ? (json === undefined ? null : JSON .parse (json)) : window .audioCapture .graph (),
         page: window .audioCapture .page (),
      };

      console .log ("sound01: before");
   })();
}

// The graph numbered from the destination: a node's inputs in input order, then in the order the nodes were made. Only
// what feeds the destination counts. A buffer source told to stop is left out: X_ITE's ended sources stay connected,
// X_ITE_R's engine forgets them (AudioEngine::release_keep_connections). A start offset after a pause depends on how
// long the clip played, so it is recorded as "positive". The context clock is "running" once it has moved.
export function normalizeGraph (raw)
{
   if (!raw)
      return null;

   const byId  = new Map (raw .nodes .map (n => [ n .id, n ]));
   const ended = n => n .kind === "AudioBufferSourceNode" && n .source ?.stopped !== null;
   const edges = raw .connections .filter (([ a, , b ]) => byId .has (a) && byId .has (b) && !ended (byId .get (a)));
   const order = [ raw .destination ];

   for (let k = 0; k < order .length; ++ k)
   {
      const into = edges .filter (e => e [2] === order [k])
         .sort ((x, y) => x [3] - y [3] || byId .get (x [0]) .serial - byId .get (y [0]) .serial || x [1] - y [1]);

      for (const [ from ] of into)
      {
         if (!order .includes (from))
            order .push (from);
      }
   }

   const index = new Map (order .map ((id, i) => [ id, i ]));

   return {
      nodes: order .map (id =>
      {
         const { kind, numberOfInputs, numberOfOutputs, channelCount, channelCountMode, channelInterpretation, gain, playbackRate, source } = byId .get (id);
         const node = { kind, numberOfInputs, numberOfOutputs, channelCount, channelCountMode, channelInterpretation };

         if (gain !== null)
            node .gain = gain;

         if (source)
         {
            const offset = source .started ?.[1];

            Object .assign (node, {
               playbackRate,
               buffer: source .buffer,
               loop: source .loop,
               loopStart: source .loopStart,
               loopEnd: source .loopEnd,
               started: source .started ? { when: source .started [0], offset: offset > 0 ? "positive" : offset } : null,
            });
         }

         return node;
      }),
      connections: edges
         .filter (([ a, , b ]) => index .has (a) && index .has (b))
         .map (([ a, o, b, i ]) => [ index .get (a), o, index .get (b), i ])
         .sort ((x, y) => x [2] - y [2] || x [3] - y [3] || x [0] - y [0] || x [1] - y [1]),
      clock: raw .currentTime > 0 ? "running" : "stopped",
      sampleRate: raw .sampleRate,
   };
}

// The world: a Sound in front of the viewer (inside its inner ellipsoid: full gain) with AudioClip A. `clip` gives the
// AudioClip's other fields, `sound` the Sound's, `file` the media file.
const WORLD = (clip, sound = "", file = "tone48k-stereo.wav") => `#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Sound:2
Viewpoint { position 0 0 10 }
DEF S Sound {
  minFront 20 minBack 20 maxFront 40 maxBack 40 ${sound}
  source DEF A AudioClip { url "BASE/media/${file}" ${clip} }
}
`;

const clipFields = page => page .evaluate (pageFields);

// Set the time fields `names` of AudioClip A to the browser's current time.
const setTimes = (page, names) => page .evaluate (names =>
{
   const browser = document .querySelector ("x3d-canvas") .browser;
   const A       = browser .currentScene .getNamedNode ("A");

   for (const name of names)
      A [name] = browser .currentTime;
}, names);

// Keep the values of AudioClip A's isActive events in window .activeEvents. A clip of 0.25 seconds can end between two
// looks at the page, so the events are recorded, not a state while it plays.
const watchActive = page => page .evaluate (() =>
{
   const A = document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("A");

   window .activeEvents = [ ];
   A .addFieldCallback ("sound01", "isActive", value => window .activeEvents .push (value));
});

const activeEvents = page => page .evaluate (() => window .activeEvents);

// Keep the events of AudioClip A's isActive, isPaused and duration_changed, in the order they came, in window .clipEvents.
const watchAll = page => page .evaluate (() =>
{
   const A = document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("A");

   window .clipEvents = [ ];

   for (const name of [ "isActive", "isPaused", "duration_changed" ])
      A .addFieldCallback ("sound01", name, value => window .clipEvents .push (`${name} ${typeof value === "number" ? Number (value .toFixed (4)) : value}`));
});

const clipEvents = page => page .evaluate (() => window .clipEvents);

const pageFacts = page => page .evaluate (() => window .audioCapture .page ());

// What the audio of the world is now.
async function state (page)
{
   return { fields: await clipFields (page), graph: normalizeGraph (await rawGraph (page)), page: await pageFacts (page) };
}

// The world of `clip` with the capture in place, loaded and settled.
async function start (page, env, clip, sound, file)
{
   await page .evaluate (installCapture, NODE_CLASSES);

   const c = await sceneCase (page, env, WORLD (clip, sound, file));

   return { loaded: await c .loaded (), console: c .console () };
}

// A click in the middle of the canvas: a user gesture (user activation) for the autoplay policy.
async function gesture (page)
{
   const box = await page .locator ("x3d-canvas") .boundingBox ();

   await page .mouse .click (box .x + box .width / 2, box .y + box .height / 2);
   await settle (page, 500);
}

export const soundCases = {
   // A looping AudioClip plays as soon as the world is live: the graph, the clip's fields and the context, with no
   // user gesture.
   async loop (page, env)
   {
      const load = await start (page, env, "loop TRUE");

      return { ... load, ... await state (page) };
   },

   // The same clip in a document that has no user activation yet, then after a user gesture on the canvas (the unlock).
   // Playwright's page .evaluate gives the page a user activation (CDP userGesture), which lets an AudioContext start:
   // so the page is reloaded with an init script that installs the capture, loads the world and takes the `before`
   // state itself, and no evaluate runs in the new document before the click.
   async unlock (page, env)
   {
      const text = WORLD ("loop TRUE") .replaceAll ('"BASE/', `"${env .base}/`);

      await page .context () .addInitScript (`(${installCapture}) (${JSON .stringify (NODE_CLASSES)}); (${loadWithoutActivation}) (${JSON .stringify (text)});`);

      const before = page .waitForEvent ("console", { predicate: m => m .text () === "sound01: before", timeout: 60000 });

      await page .reload ();
      await before;
      await gesture (page);

      const recorded = await page .evaluate (() => window .soundBefore);

      const after = await state (page);

      // The click fires several canvas events (mousedown, focus, mouseup, pointerup, click), and each one calls resume ()
      // again until the first call has settled: how many come before that is a matter of timing. The calls are kept as
      // their states without repeats.
      after .page .resumeCalls = after .page .resumeCalls .filter ((state, i, all) => state !== all [i - 1]);

      return {
         before: { ... recorded, graph: normalizeGraph (recorded .graph) },
         after: { activated: await page .evaluate (() => navigator .userActivation .hasBeenActive), ... after },
      };
   },

   // A clip that does not loop, in a document with no user activation (see unlock): the context waits. Then the page
   // starts the clip from script (page .evaluate gives a user activation, but there is no event on the canvas): X_ITE's
   // startAudioElement of the clip tries the context's resume () again, which now succeeds.
   async unlockByStart (page, env)
   {
      const text = WORLD ("") .replaceAll ('"BASE/', `"${env .base}/`);

      await page .context () .addInitScript (`(${installCapture}) (${JSON .stringify (NODE_CLASSES)}); (${loadWithoutActivation}) (${JSON .stringify (text)});`);

      const before = page .waitForEvent ("console", { predicate: m => m .text () === "sound01: before", timeout: 60000 });

      await page .reload ();
      await before;

      const recorded = await page .evaluate (() => window .soundBefore);

      await watchActive (page);
      await setTimes (page, [ "startTime" ]);
      await settle (page, 1000);

      const { fields, graph, page: facts } = await state (page);

      return {
         before: { ... recorded, graph: normalizeGraph (recorded .graph) },
         started: { activated: await page .evaluate (() => navigator .userActivation .hasBeenActive), events: await activeEvents (page), fields, clock: graph .clock, page: facts },
      };
   },

   // A clip that does not loop, started by the page after a gesture: it ends when the audio clock passes its duration
   // (X3DSoundSourceNode.set_time reads the AudioElement's currentTime, the audio context's clock). Before the audio
   // clock of the page, X_ITE_R's clip never ended in a page: the engine's own clock stays at 0 there.
   async once (page, env)
   {
      const load = await start (page, env, "");

      await gesture (page);

      const idle = await clipFields (page);

      await watchActive (page);
      await setTimes (page, [ "startTime" ]);
      await settle (page, 1000);

      return { ... load, idle, events: await activeEvents (page), ended: await state (page) };
   },

   // Pause, then resume, after a gesture: the source starts again at the offset where it was paused (the audio clock).
   async pauseResume (page, env)
   {
      const load = await start (page, env, "loop TRUE");

      await gesture (page);
      await settle (page, 300);
      await setTimes (page, [ "pauseTime" ]);
      await settle (page, 300);

      const paused = await state (page);

      await setTimes (page, [ "resumeTime" ]);
      await settle (page, 300);

      return { ... load, paused, resumed: await state (page) };
   },
};

// More of the playback behavior and the Sound node (issue #140): stop, loop, the field events, an AudioClip that does not
// load, a world that is replaced, and the Sound's own fields.
Object .assign (soundCases,
{
   // A looping clip, stopped by stopTime: isActive goes FALSE and the source is stopped.
   async stop (page, env)
   {
      const load = await start (page, env, "loop TRUE");

      await gesture (page);
      await watchAll (page);
      await settle (page, 300);

      const playing = await state (page);

      await setTimes (page, [ "stopTime" ]);
      await settle (page, 500);

      return { ... load, playing, events: await clipEvents (page), stopped: await state (page) };
   },

   // loop is turned off while the clip plays: it ends at the end of the clip.
   async loopOff (page, env)
   {
      const load = await start (page, env, "loop TRUE");

      await gesture (page);
      await watchAll (page);
      await settle (page, 300);
      await page .evaluate (() => { document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("A") .loop = false; });
      await settle (page, 1000);

      return { ... load, events: await clipEvents (page), ended: await state (page) };
   },

   // Start, pause, resume and let the clip end: the order of the field events.
   async events (page, env)
   {
      const load = await start (page, env, "");

      await gesture (page);
      await watchAll (page);
      await setTimes (page, [ "startTime" ]);
      await settle (page, 100);
      await setTimes (page, [ "pauseTime" ]);
      await settle (page, 100);
      await setTimes (page, [ "resumeTime" ]);
      await settle (page, 1000);

      return { ... load, events: await clipEvents (page), ended: await state (page) };
   },

   // The url does not load: the messages, the fields and the graph.
   async badUrl (page, env)
   {
      const load = await start (page, env, "loop TRUE", "", "missing.wav");

      await gesture (page);

      return { ... load, ... await state (page) };
   },

   // The world is replaced while the clip plays: the source stops and leaves the graph; the page keeps its context.
   async replace (page, env)
   {
      const load = await start (page, env, "loop TRUE");

      await gesture (page);
      await settle (page, 300);

      const playing = await state (page);

      await page .evaluate (async () =>
      {
         const browser = document .querySelector ("x3d-canvas") .browser;

         await browser .replaceWorld (await browser .createX3DFromString ("#X3D V4.0 utf8\nPROFILE Interchange\nGroup { }\n"));
      });

      await settle (page, 500);

      return { ... load, playing, replaced: { graph: normalizeGraph (await rawGraph (page)), page: await pageFacts (page) } };
   },

   // The Sound's own fields: intensity, and a location off the viewer's axis (the pan gains).
   async intensity (page, env)
   {
      const load = await start (page, env, "loop TRUE", "intensity 0.5 location 5 0 0");

      await gesture (page);

      return { ... load, ... await state (page) };
   },

   async unspatialized (page, env)
   {
      const load = await start (page, env, "loop TRUE", "spatialize FALSE location 5 0 0 intensity 0.8");

      await gesture (page);

      return { ... load, ... await state (page) };
   },
});

// Chromium's autoplay policy of a desktop page: an AudioContext starts only after a user gesture. Headless Chromium
// lets it start at once otherwise (no gesture needed), so the unlock would not be seen.
export const CASE_GROUPS = {
   sound: { context: DESKTOP, browserArgs: [ "--autoplay-policy=document-user-activation-required" ], cases: soundCases },
};
