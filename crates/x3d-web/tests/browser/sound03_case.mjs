// The sound 03 cases (issue #140): Mute and SoundIntensity, an AudioClip's pitch, the Sound ranges and priority, the
// audio listener, the life cycle of the page's AudioContext, an AudioDestination, a world replaced while a clip plays, and
// whether a page can be heard at all. sound01_case.mjs has the graph capture, the normalization and the first cases.
// oracle/x_ite-web-runtime-02.mjs (--suite sound03) runs them against pinned X_ITE 16.5.0 (five runs) and records the
// values; sound03.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).
//
// Two kinds of case, kept apart:
// * graph-state cases read the Web Audio graph of the page (X_ITE: the capture; X_ITE_R: the engine's `audioGraph ()`):
//   gains, rates, connections, the listener and the AudioContext facts. They show what the engine would give to an output.
// * the `audible` cases tap every connection into the page's AudioContext destination and measure the signal there: what
//   the page's output would carry. A graph can be right and the page still silent.

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { sceneCase } from "./loadurl01_case.mjs";
import { NODE_CLASSES, WORLD, gesture, installCapture, normalizeGraph, pageFacts, rawGraph, state as state01 } from "./sound01_case.mjs";

export const WIDE = "minFront 20 minBack 20 maxFront 40 maxBack 40";
export const FRONT = "Viewpoint { position 0 0 10 }";
export const CLIP = (fields = "loop TRUE") => `AudioClip { url "BASE/media/tone48k-stereo.wav" ${fields} }`;
export const SOUND = (fields, clip = CLIP ()) => `Sound { ${fields}\n  source ${clip} }`;
export const WORLD3 = (viewpoint, body, component = "Sound:2") => `#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT ${component}\n${viewpoint}\n${body}\n`;

// Capture installed, world loaded and settled.
async function load (page, env, text)
{
   await page .evaluate (installCapture, NODE_CLASSES);

   const c = await sceneCase (page, env, text);

   return { loaded: await c .loaded (), console: c .console () };
}

export const setOption = (page, name, value) => page .evaluate (([ name, value ]) => document .querySelector ("x3d-canvas") .browser .setBrowserOption (name, value), [ name, value ]);

// The listener of the page's AudioContext: position, forward and up. X_ITE: the context's AudioListener; X_ITE_R: `listener` of
// audioGraph (). Rounded to 5 decimals.
const listener = page => page .evaluate (() =>
{
   const engine = document .querySelector ("x3d-canvas") .browser [Symbol .for ("X_ITE_R.engine")];
   const values = engine ? (() => { const json = engine .audioGraph (); return json === undefined ? null : JSON .parse (json) .listener ?? null; }) () : window .audioCapture .listener ();

   return values ?.map (v => Number (v .toFixed (5)) || 0) ?? null;
});

// The normalized graph. The AudioDestinationNode has numberOfOutputs 0 in Chromium and 1 in the Web Audio port of X_ITE_R (the
// specification and the web-audio-api crate; web/API.md "Sound"): the count is set to 0 on both sides, as it is not a behavior.
async function graphOf (page)
{
   const graph = normalizeGraph (await rawGraph (page));

   if (graph)
      graph .nodes [0] .numberOfOutputs = 0;

   return graph;
}

async function state (page)
{
   return { ... await state01 (page), graph: await graphOf (page) };
}

// The gain of the node that feeds the context's destination (the sound destination's gain: !Mute * clamp (SoundIntensity, 0, 1)).
const destinationGain = async page => (await graphOf (page)) ?.nodes [1] ?.gain ?? null;

// Every node kind and every connection of the graph by kind, not numbered from the destination: an AudioDestination node feeds
// its own stream, not the context's destination.
async function wholeGraph (page)
{
   const raw = await rawGraph (page);

   if (!raw)
      return null;

   const kind = new Map (raw .nodes .map (n => [ n .id, n .kind ]));

   return {
      kinds: raw .nodes .map (n => n .kind) .sort (),
      edges: [ ... new Set (raw .connections .map (([ a, , b ]) => `${kind .get (a)} > ${kind .get (b)}`)) ] .sort (),
   };
}

// Measures what reaches the page's output. installTap runs BEFORE installCapture: it makes each AudioContext's `destination` a
// GainNode that feeds the real destination and an AnalyserNode (pulled through a silent gain: Chromium does not process a node
// that no destination pulls), and it also taps a connection into a MediaStreamAudioDestinationNode (the output of an
// AudioDestination node). `window .tapMax` is the largest root mean square seen by any tap; `window .tapped` counts the
// connections made into a destination.
export function installTap ()
{
   const Gain          = window .GainNode;
   const Analyser      = window .AnalyserNode;
   const connect       = AudioNode .prototype .connect;
   const destinationOf = Object .getOwnPropertyDescriptor (BaseAudioContext .prototype, "destination") .get;
   const mixes         = new WeakMap ();
   const own           = new WeakSet ();
   const analysers     = [ ];

   window .tapped = 0;
   window .tapMax = 0;

   const tap = (node, context) =>
   {
      const analyser = new Analyser (context, { fftSize: 2048 });
      const silent   = new Gain (context, { gain: 0 });

      own .add (analyser) .add (silent);
      connect .call (node, analyser);
      connect .call (analyser, silent);
      connect .call (silent, destinationOf .call (context));
      analysers .push (analyser);
   };

   Object .defineProperty (BaseAudioContext .prototype, "destination",
   {
      configurable: true,
      get ()
      {
         let mix = mixes .get (this);

         if (!mix)
         {
            mix = new Gain (this);
            own .add (mix);
            connect .call (mix, destinationOf .call (this));
            tap (mix, this);
            mixes .set (this, mix);
         }

         return mix;
      },
   });

   AudioNode .prototype .connect = function (destination, ... rest)
   {
      const result = connect .call (this, destination, ... rest);

      if (!own .has (this) && destination === mixes .get (destination ?.context))
         ++ window .tapped;

      if (!own .has (this) && destination instanceof MediaStreamAudioDestinationNode)
      {
         ++ window .tapped;
         tap (this, destination .context);
      }

      return result;
   };

   const buffer = new Float32Array (2048);

   setInterval (() =>
   {
      for (const analyser of analysers)
      {
         analyser .getFloatTimeDomainData (buffer);

         let sum = 0;

         for (const v of buffer)
            sum += v * v;

         window .tapMax = Math .max (window .tapMax, Math .sqrt (sum / buffer .length));
      }
   },
   20);
}

async function audible (page, env, text, setup)
{
   await page .evaluate (installTap);
   await page .evaluate (installCapture, NODE_CLASSES);

   const c = await sceneCase (page, env, text);

   await gesture (page);
   await setup ?.(page);
   await settle (page, 300);

   // Only what arrives after the setup counts: the tap keeps the largest level it has seen.
   await page .evaluate (() => { window .tapMax = 0; });
   await settle (page, 1500);

   const facts = await pageFacts (page);
   const tap   = await page .evaluate (() => ({ tapped: window .tapped, rms: window .tapMax }));

   return {
      loaded: await c .loaded (),
      contextStates: facts .states,
      // A context that is not running proves nothing about the output.
      contextRunning: facts .states .length > 0 && facts .states .every (s => s === "running"),
      connectedToOutput: tap .tapped > 0,
      audible: tap .rms > 1e-3,
   };
}

const rangeCase = (viewpoint, fields, sound = "spatialize FALSE") => async (page, env) =>
{
   const text = WORLD3 (viewpoint, SOUND (`${sound} ${fields}`));
   const l    = await load (page, env, text);

   await gesture (page);

   return { ... l, graph: await graphOf (page) };
};

const BACK = "Viewpoint { position 0 0 -10 orientation 0 1 0 3.14159265358979 }";

export const soundCases = {
   // Mute and SoundIntensity: the gain of the sound destination after each change, with a world that is live.
   async mute (page, env)
   {
      const l = await load (page, env, WORLD ("loop TRUE"));

      await gesture (page);

      const steps = [ ];

      for (const [ name, value ] of [ [ null ], [ "SoundIntensity", 0.5 ], [ "Mute", true ], [ "Mute", false ], [ "SoundIntensity", 3 ], [ "SoundIntensity", -1 ], [ "SoundIntensity", 1 ] ])
      {
         if (name)
            await setOption (page, name, value);

         await settle (page, 300);
         steps .push ({ set: name ? `${name}=${value}` : "initial", destinationGain: await destinationGain (page) });
      }

      return { ... l, steps, graph: await graphOf (page) };
   },

   // The options are set before the world exists: a Sound that starts later reads them.
   async muteBefore (page, env)
   {
      await page .evaluate (installCapture, NODE_CLASSES);
      await setOption (page, "Mute", true);
      await setOption (page, "SoundIntensity", 0.5);

      const c = await sceneCase (page, env, WORLD ("loop TRUE"));

      await gesture (page);

      const muted = await destinationGain (page);

      await setOption (page, "Mute", false);
      await settle (page, 300);

      return { loaded: await c .loaded (), muted, unmuted: await destinationGain (page), graph: await graphOf (page) };
   },

   // An AudioClip's pitch is the source's playbackRate (negative: 0).
   async pitchFast (page, env)
   {
      const l = await load (page, env, WORLD ("loop TRUE pitch 1.5"));

      await gesture (page);
      return { ... l, ... await state (page) };
   },

   async pitchNegative (page, env)
   {
      const l = await load (page, env, WORLD ("loop TRUE pitch -1"));

      await gesture (page);
      return { ... l, ... await state (page) };
   },

   async pitchSlow (page, env)
   {
      const l = await load (page, env, WORLD ("loop TRUE pitch 0.5"));

      await gesture (page);
      return { ... l, ... await state (page) };
   },

   // Pitch changed while the clip plays.
   async pitchChanged (page, env)
   {
      const l = await load (page, env, WORLD ("loop TRUE"));

      await gesture (page);
      await page .evaluate (() => { document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("A") .pitch = 2; });
      await settle (page, 300);
      return { ... l, ... await state (page) };
   },

   // Sound ranges: the viewer 10 away on the axis, in front of the sound and behind it (a viewer turned around at z = -10).
   rangeFront: rangeCase (FRONT, "minFront 2 minBack 4 maxFront 12 maxBack 20"),
   rangeBack: rangeCase (BACK, "minFront 2 minBack 4 maxFront 12 maxBack 20"),
   rangeInside: rangeCase (FRONT, WIDE),
   rangeOutside: rangeCase (FRONT, "minFront 1 minBack 1 maxFront 5 maxBack 5"),
   rangeOnMinimum: rangeCase (FRONT, "minFront 10 minBack 10 maxFront 20 maxBack 20"),
   rangeOnMaximum: rangeCase (FRONT, "minFront 2 minBack 2 maxFront 10 maxBack 10"),
   rangeSpatial: rangeCase (FRONT, "minFront 2 minBack 4 maxFront 12 maxBack 20", "spatialize TRUE location 3 0 0"),

   // Priority: three Sounds with different priorities. The graph is the same as with equal priorities.
   async priority (page, env)
   {
      const body = [ 0, 0.5, 1 ] .map (p => SOUND (`${WIDE} priority ${p} location ${p * 4 - 2} 0 0`)) .join ("\n");
      const l    = await load (page, env, WORLD3 (FRONT, body));

      await gesture (page);

      return { ... l, graph: await graphOf (page), priorities: await page .evaluate (() => Array .from (document .querySelector ("x3d-canvas") .browser .currentScene .rootNodes) .filter (n => n .getNodeTypeName () === "Sound") .map (n => n .priority)) };
   },

   // The audio listener: the default (the viewpoint), a ListenerPointSource that does not track the view, one that does.
   async listenerDefault (page, env)
   {
      const l = await load (page, env, WORLD3 ("Viewpoint { position 1 2 3 orientation 0 1 0 1.5707963267949 }", SOUND (WIDE)));

      await gesture (page);

      return { ... l, listener: await listener (page) };
   },

   async listenerFixed (page, env)
   {
      const text = WORLD3 ("Viewpoint { position 0 0 10 }", `ListenerPointSource { position 1 2 3 orientation 0 1 0 1.5707963267949 trackCurrentView FALSE }\n${SOUND (WIDE)}`, "Sound:3");
      const l    = await load (page, env, text);

      await gesture (page);

      return { ... l, listener: await listener (page) };
   },

   async listenerTrack (page, env)
   {
      const text = WORLD3 ("Viewpoint { position 4 5 6 orientation 1 0 0 -0.5 }", `ListenerPointSource { position 1 2 3 trackCurrentView TRUE }\n${SOUND (WIDE)}`, "Sound:3");
      const l    = await load (page, env, text);

      await gesture (page);

      return { ... l, listener: await listener (page) };
   },

   // The page's AudioContext over the life of the browser: a world with no gesture, the gesture, a second playing world, an
   // empty world, dispose ().
   async lifecycle (page, env)
   {
      const steps = [ ];
      const text  = WORLD ("loop TRUE") .replaceAll ('"BASE/', `"${env .base}/`);
      const empty = "#X3D V4.0 utf8\nPROFILE Interchange\nGroup { }\n";
      const note  = async name => steps .push ({ name, page: await pageFacts (page), graph: await graphOf (page) });
      const swap  = async text => { await page .evaluate (async text => { const b = document .querySelector ("x3d-canvas") .browser; await b .replaceWorld (await b .createX3DFromString (text)); }, text); await settle (page, 700); };

      await page .evaluate (installCapture, NODE_CLASSES);
      await swap (text);
      await note ("loaded");
      await gesture (page);
      await note ("gesture");
      await swap (WORLD ("loop TRUE", "intensity 0.5") .replaceAll ('"BASE/', `"${env .base}/`));
      await note ("replacedPlaying");
      await swap (empty);
      await note ("replacedEmpty");
      await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .dispose ());
      await settle (page, 500);
      steps .push ({ name: "disposed", page: await pageFacts (page) });

      return { steps };
   },

   // An AudioDestination node with an OscillatorSource: the nodes and connections the page builds for it.
   async audioDestination (page, env)
   {
      const l = await load (page, env, WORLD3 (FRONT, "AudioDestination { gain 0.5 channelCount 1 children DEF O OscillatorSource { frequency 220 } }", "Sound:3"));

      await gesture (page);
      await page .evaluate (() => { const b = document .querySelector ("x3d-canvas") .browser; b .currentScene .getNamedNode ("O") .startTime = b .currentTime; });
      await settle (page, 500);

      return { ... l, graph: await wholeGraph (page), page: await pageFacts (page) };
   },

   // A world replaced by another playing world while the first plays: the graph of each, and the one context.
   async replacePlaying (page, env)
   {
      const l      = await load (page, env, WORLD ("loop TRUE", "intensity 0.5"));

      await gesture (page);

      const before = await state (page);

      await page .evaluate (async text =>
      {
         const b = document .querySelector ("x3d-canvas") .browser;

         await b .replaceWorld (await b .createX3DFromString (text));
      },
      WORLD ("loop TRUE pitch 2", "intensity 0.8") .replaceAll ('"BASE/', `"${env .base}/`));
      await settle (page, 1000);

      // The start offset of the new world's clip is a race in X_ITE itself (0 or a few samples: the clip starts in the frame that
      // installs the world, against the audio clock). Pinned X_ITE gave 0 once in about 15 runs and a positive offset the rest.
      const after = await state (page);

      for (const node of after .graph ?.nodes ?? [ ])
      {
         if (node .started)
            node .started .offset = "timing";
      }

      return { ... l, before, after };
   },

   // Whether the page carries a signal into its AudioContext destination: a looping clip, and the same with Mute.
   async audibleClip (page, env)
   {
      return audible (page, env, WORLD3 (FRONT, SOUND (WIDE)));
   },

   async audibleMuted (page, env)
   {
      return audible (page, env, WORLD3 (FRONT, SOUND (WIDE)), page => setOption (page, "Mute", true));
   },

   async audibleOscillator (page, env)
   {
      return audible (page, env, WORLD3 (FRONT, "AudioDestination { children DEF O OscillatorSource { frequency 440 } }", "Sound:3"),
         page => page .evaluate (() => { const b = document .querySelector ("x3d-canvas") .browser; b .currentScene .getNamedNode ("O") .startTime = b .currentTime; }));
   },
};

// Headless Chromium lets an AudioContext start at once unless the policy asks for a gesture (see sound01_case.mjs).
export const CASE_GROUPS = {
   soundMore: { context: DESKTOP, browserArgs: [ "--autoplay-policy=document-user-activation-required" ], cases: soundCases },
};
