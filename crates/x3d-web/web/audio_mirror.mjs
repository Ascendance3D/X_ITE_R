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
   #queue = Promise .resolve ();
   #disposed = false;
   #elements = new Map ();    // MediaStreamAudioDestinationNode -> the <audio> element that plays its stream
   #analyserData = new WeakMap (); // AnalyserNode -> the arrays readAnalysers fills
   #streams = new Map ();     // the stand-in GainNode of a MicrophoneSource -> its getUserMedia MediaStream

   // `context`: the page's AudioContext. `read .buffer (index)`: [ sampleRate, ... Float32Array ] of an op's `{ buffer }`
   // argument; `read .bytes (index)`: the Uint8Array of its `{ encoded }` argument (the file an AudioBuffer was decoded from).
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

   // The batch of `takeAudioOps` (parsed): the calls of each engine, then the engines that no longer exist. The big arguments
   // are read now (the engine keeps them only until its next batch); the calls run in order, and a call that needs the page to
   // decode a file waits for it, with the calls after it.
   apply (batch)
   {
      for (const { ops } of batch .engines)
      {
         for (const op of ops)
         {
            for (let i = 1; i < op .length; ++ i)
            {
               const a = op [i];

               if (a ?.buffer !== undefined)
                  op [i] = { buffer: this .#read .buffer (a .buffer) };
               else if (a ?.encoded !== undefined)
                  op [i] = { encoded: this .#read .bytes (a .encoded) };
            }
         }
      }

      this .#queue = this .#queue .then (async () =>
      {
         for (const { uid, ops } of batch .engines)
         {
            const nodes = this .#nodes (uid);

            for (const op of ops)
            {
               if (this .#disposed)
                  return;

               try { await this .#op (nodes, op); } catch { }
            }
         }

         for (const uid of batch .gone)
            this .#teardown (uid);
      });
   }

   // An engine that goes away (a replaced world, a discarded load) takes its sources with it: a source that was told to stop
   // later would play on, connected, with nothing left to ask for the stop.
   #teardown (uid)
   {
      const nodes = this .#engines .get (uid);

      this .#engines .delete (uid);

      for (const node of nodes ?.values () ?? [ ])
      {
         this .#stopElement (node);

         if (node instanceof AudioScheduledSourceNode)
            quiet (() => node .stop ());

         quiet (() => node .disconnect ());
      }
   }

   // MicrophoneSource.set_start (MicrophoneSource.js:32-69): getUserMedia ({ audio: { deviceId } }), then a
   // MediaStreamAudioSourceNode. The engine's calls on the node do not wait for the page's permission: they are made on a
   // stand-in GainNode at once, and the stream is connected into it when getUserMedia gives it. A refused or failed
   // request logs its message, as X_ITE does, and the node stays silent. The stream's tracks stop with the node
   // (MicrophoneSource.set_stop).
   #microphone (deviceId)
   {
      const standIn = new GainNode (this .#context);

      if (!navigator .mediaDevices)
         return standIn;

      // A getUserMedia that throws at once is handled as a rejection.
      Promise .resolve ()
      .then (() => navigator .mediaDevices .getUserMedia ({ audio: { deviceId } }))
      .then (mediaStream =>
      {
         if (this .#disposed || !this .#live (standIn))
            return mediaStream .getTracks () .forEach (track => track .stop ());

         // A stream that cannot be connected stops its tracks; the message is logged below.
         try
         {
            new MediaStreamAudioSourceNode (this .#context, { mediaStream }) .connect (standIn);
         }
         catch (error)
         {
            mediaStream .getTracks () .forEach (track => track .stop ());
            throw error;
         }

         this .#streams .set (standIn, mediaStream);
      })
      .catch (error => console .error (error ?.message ?? String (error)));

      return standIn;
   }

   // Whether a node is still one of an engine's nodes.
   #live (node)
   {
      return [ ... this .#engines .values () ] .some (nodes => [ ... nodes .values () ] .includes (node));
   }

   #stopElement (node)
   {
      const stream = this .#streams .get (node);

      if (stream)
      {
         this .#streams .delete (node);
         stream .getTracks () .forEach (track => track .stop ());
      }

      const element = this .#elements .get (node);

      if (!element)
         return;

      this .#elements .delete (node);
      quiet (() => element .pause ());
      element .srcObject = null;
   }

   // Analyser.set_time (Analyser.js:60-87) reads the four data arrays of the page's AnalyserNode every frame. No signal goes
   // through the engine's graph, so the page reads its own node of each engine Analyser and gives the data to the engine
   // (`setAnalyserData`), in X_ITE's order: byte frequency, byte time domain, float frequency, float time domain. Called
   // before each frame; a node the mirror makes in a frame is read from the next one.
   readAnalysers (engine)
   {
      for (const [ uid, nodes ] of this .#engines)
      {
         for (const [ slot, node ] of nodes)
         {
            if (!(node instanceof AnalyserNode))
               continue;

            const bins = node .frequencyBinCount;
            let data   = this .#analyserData .get (node);

            if (data ?.[0] .length !== bins)
            {
               data = [ new Uint8Array (bins), new Uint8Array (bins), new Float32Array (bins), new Float32Array (bins) ];
               this .#analyserData .set (node, data);
            }

            node .getByteFrequencyData  (data [0]);
            node .getByteTimeDomainData (data [1]);
            node .getFloatFrequencyData  (data [2]);
            node .getFloatTimeDomainData (data [3]);

            engine .setAnalyserData (uid, slot, ... data);
         }
      }
   }

   // X3DSoundContext.startAudioElements: the `<audio>` elements that are not playing yet are played again (the autoplay
   // policy refuses them until the page has a user activation).
   startElements ()
   {
      for (const element of this .#elements .values ())
      {
         if (element .paused)
            quiet (() => element .play () .catch (() => { }));
      }
   }

   // The browser goes away: every engine's nodes.
   dispose ()
   {
      this .#disposed = true;

      for (const uid of [ ... this .#engines .keys () ])
         this .#teardown (uid);
   }

   #node (nodes, id)
   {
      return id === DESTINATION ? this .#context .destination : nodes .get (id);
   }

   // An AudioBuffer: decoded by the page from the file it came from (resampled once, to the context's rate, as X_ITE's
   // decodeAudioData does), else made from its samples.
   async #buffer (data)
   {
      if (data .encoded)
         return this .#context .decodeAudioData (data .encoded .buffer .slice (data .encoded .byteOffset, data .encoded .byteOffset + data .encoded .byteLength));

      const [ rate, ... channels ] = data .buffer;
      const buffer = this .#context .createBuffer (channels .length, channels [0] .length, rate);

      channels .forEach ((channel, i) => buffer .copyToChannel (channel, i));

      return buffer;
   }

   async #op (nodes, [ name, id, ... a ])
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

            make (source);

            if (a [0] !== false)
               source .buffer = await this .#buffer (a [0]);

            return;
         }
         case "biquad":      return make (context .createBiquadFilter ());
         case "delay":       return make (new DelayNode (context, { maxDelayTime: a [0], delayTime: a [1] }));
         case "waveShaper":  return make (context .createWaveShaper ());
         case "convolver":   return make (context .createConvolver ());
         case "compressor":  return make (context .createDynamicsCompressor ());
         case "analyser":    return make (context .createAnalyser ());
         case "microphone":   return make (this .#microphone (a [0]));
         case "splitter":    return make (context .createChannelSplitter (a [0]));
         case "merger":      return make (context .createChannelMerger (a [0]));
         case "panner":      return make (context .createPanner ());

         // AudioDestination: `new MediaStreamAudioDestinationNode (audioContext)`, whose stream an `<audio>` element plays
         // (`audioElement .srcObject = node .stream`, played as X3DSoundContext.startAudioElement does).
         // `browser .startAudioElement (audioElement)`: X3DSoundContext.startAudioElements tries every element that waits, and the
         // context's `resume ()` is one of them while its first call is not settled.
         case "elementStart":
            quiet (() => context .resume () .catch (() => { }));
            this .startElements ();
            return;
         case "mediaStream":
         {
            const stream  = new MediaStreamAudioDestinationNode (context, { channelCount: 2, channelCountMode: "explicit", channelInterpretation: "speakers" });
            const element = new Audio ();

            element .srcObject = stream .stream;
            this .#elements .set (stream, element);
            quiet (() => element .play () .catch (() => { }));

            return make (stream);
         }

         // Lifetime and connections. `release` has disconnected the node before (its own ops); `releaseKeep` leaves the
         // connections: a source told to stop plays on until then.
         case "release":
         case "releaseKeep":
            this .#stopElement (nodes .get (id));
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
         case "convolverBuffer":      node () .buffer = await this .#buffer (a [0]); return;
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
