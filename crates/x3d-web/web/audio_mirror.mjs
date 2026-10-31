// The page's Web Audio nodes of the engine's Sound nodes (issue #140).
//
// X_ITE's Sound nodes call the browser's Web Audio API on the page's AudioContext. The engine of X_ITE_R makes the same calls
// on its own graph (crates/x3d-runtime/src/sound) and records each one as an op (sound/ops.rs). This module makes every op
// on a real node of the page's AudioContext, so the nodes are browser nodes, connected to the context's destination, and the
// page plays what the pinned X_ITE plays. The engine's own graph stays the record of what was asked (audioGraph (), the
// tests); nothing here renders or simulates a signal.
//
// Nodes are named by (engine, slot): an engine numbers its nodes from 0, and a slot is reused after a `release` (the ops
// keep the order). Slot 0 of every engine is the engine's destination: here the page's one `context .destination`.

const DESTINATION = 0;

// Web Audio throws for what the engine's port skips or clamps (a value out of range, a node that already started); the call
// then has no effect, as the same call has none in X_ITE's try-less JavaScript after the exception ends the callback.
const quiet = f => { try { f (); } catch { } };

export class AudioMirror
{
   #context;
   #engines = new Map ();     // uid -> Map (slot -> AudioNode)
   #read;

   // `context`: the page's AudioContext. `read (index)`: [ sampleRate, ... Float32Array ] of an op's buffer argument.
   constructor (context, read)
   {
      this .#context = context;
      this .#read    = read;
   }

   // How many nodes the mirror holds (for the tests).
   get size () { return [ ... this .#engines .values () ] .reduce ((n, nodes) => n + nodes .size, 0); }

   #nodes (uid)
   {
      let nodes = this .#engines .get (uid);

      if (!nodes)
         this .#engines .set (uid, nodes = new Map ());

      return nodes;
   }

   // The batch of `takeAudioOps` (parsed): the calls of each engine, then the engines that no longer exist.
   apply (batch)
   {
      for (const { uid, ops } of batch .engines)
      {
         const nodes = this .#nodes (uid);

         for (const op of ops)
            quiet (() => this .#op (nodes, op));
      }

      for (const uid of batch .gone)
         this .#teardown (uid);
   }

   // An engine that goes away (a replaced world, a discarded load) takes its sources with it: a source that was told to stop
   // later would play on, connected, with nothing left to ask for the stop.
   #teardown (uid)
   {
      const nodes = this .#engines .get (uid);

      this .#engines .delete (uid);

      for (const node of nodes ?.values () ?? [ ])
      {
         if (node instanceof AudioScheduledSourceNode)
            quiet (() => node .stop ());

         quiet (() => node .disconnect ());
      }
   }

   // The browser goes away: every engine's nodes.
   dispose ()
   {
      for (const uid of [ ... this .#engines .keys () ])
         this .#teardown (uid);
   }

   #node (nodes, id)
   {
      return id === DESTINATION ? this .#context .destination : nodes .get (id);
   }

   #buffer (data)
   {
      const [ rate, ... channels ] = this .#read (data .buffer);
      const buffer = this .#context .createBuffer (channels .length, channels [0] .length, rate);

      channels .forEach ((channel, i) => buffer .copyToChannel (channel, i));

      return buffer;
   }

   #op (nodes, [ name, id, ... a ])
   {
      const context = this .#context;
      const node    = () => this .#node (nodes, id);
      const make    = node => nodes .set (id, node);

      switch (name)
      {
         // Constructors.
         case "gain":       return make (new GainNode (context, { gain: a [0] }));
         case "oscillator": return make (context .createOscillator ());
         case "bufferSource":
         {
            const source = context .createBufferSource ();

            if (a [0] !== false)
               quiet (() => { source .buffer = this .#buffer (a [0]); });

            return make (source);
         }
         case "biquad":      return make (context .createBiquadFilter ());
         case "delay":       return make (new DelayNode (context, { maxDelayTime: a [0], delayTime: a [1] }));
         case "waveShaper":  return make (context .createWaveShaper ());
         case "convolver":   return make (context .createConvolver ());
         case "compressor":  return make (context .createDynamicsCompressor ());
         case "analyser":    return make (context .createAnalyser ());
         case "splitter":    return make (context .createChannelSplitter (a [0]));
         case "merger":      return make (context .createChannelMerger (a [0]));
         case "panner":      return make (context .createPanner ());

         // Lifetime and connections. `release` has disconnected the node before (its own ops); `releaseKeep` leaves the
         // connections: a source told to stop plays on until then.
         case "release":
         case "releaseKeep":
            nodes .delete (id);
            return;
         case "connect":      return node () .connect (this .#node (nodes, a [1]), a [0], a [2]);
         case "disconnect":   return node () .disconnect ();
         case "disconnectDest": return node () .disconnect (this .#node (nodes, a [0]));
         case "disconnectTo": return node () .disconnect (this .#node (nodes, a [1]), a [0], a [2]);

         // AudioNode channel attributes.
         case "channelCount":          node () .channelCount = a [0]; return;
         case "channelCountMode":      node () .channelCountMode = a [0]; return;
         case "channelInterpretation": node () .channelInterpretation = a [0]; return;

         // AudioParams: `.value = v`, and the events of a GainNode's gain.
         case "param":     node () [a [0]] .value = a [1]; return;
         case "gainEvent": return a [0] === "setValue"
            ? node () .gain .setValueAtTime (a [1], a [2])
            : node () .gain .exponentialRampToValueAtTime (a [1], a [2]);

         // OscillatorNode, AudioBufferSourceNode.
         case "oscillatorType":  node () .type = a [0]; return;
         case "oscillatorWave":  node () .setPeriodicWave (context .createPeriodicWave (Float32Array .from (a [0]), Float32Array .from (a [1]))); return;
         case "oscillatorStart": return node () .start ();
         case "oscillatorStop":  return node () .stop (a [0]);
         case "sourceStart":     return node () .start (a [0], a [1]);
         case "sourceStop":      return node () .stop (a [0]);
         case "sourceLoop":      node () .loop = a [0]; return;
         case "sourceLoopStart": node () .loopStart = a [0]; return;
         case "sourceLoopEnd":   node () .loopEnd = a [0]; return;

         // The other node kinds.
         case "biquadType":           node () .type = a [0]; return;
         case "waveShaperCurve":      node () .curve = Float32Array .from (a [0]); return;
         case "waveShaperOversample": node () .oversample = a [0]; return;
         case "convolverNormalize":   node () .normalize = a [0]; return;
         case "convolverBuffer":      node () .buffer = this .#buffer (a [0]); return;
         case "analyserFftSize":      node () .fftSize = a [0]; return;
         case "analyserSmoothing":    node () .smoothingTimeConstant = a [0]; return;
         case "analyserDecibels":
         {
            // minDecibels, then maxDecibels, in the order that never leaves min >= max.
            const analyser = node ();

            if (a [0] >= analyser .maxDecibels)
               analyser .maxDecibels = a [1], analyser .minDecibels = a [0];
            else
               analyser .minDecibels = a [0], analyser .maxDecibels = a [1];

            return;
         }
         case "pannerAttributes":
         {
            const panner = node ();

            panner .distanceModel   = a [0];
            panner .refDistance     = a [1];
            panner .maxDistance     = a [2];
            panner .rolloffFactor   = a [3];
            panner .coneInnerAngle  = a [4];
            panner .coneOuterAngle  = a [5];
            panner .coneOuterGain   = a [6];
            return;
         }
         case "pannerModel": node () .panningModel = a [0] ? "HRTF" : "equalpower"; return;

         // The one listener of the page's AudioContext. `id` is its first value.
         case "listener":
         {
            const l      = context .listener;
            const values = [ id, ... a ];

            [ l .positionX, l .positionY, l .positionZ, l .forwardX, l .forwardY, l .forwardZ, l .upX, l .upY, l .upZ ]
               .forEach ((param, i) => { param .value = values [i]; });
            return;
         }
      }
   }
}
