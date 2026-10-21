// Finding C probe: does a Chromium AudioBufferSourceNode advance while its output is not
// pulled by the destination? Plain Web Audio, OfflineAudioContext, headless Chrome.
// The buffer is a ramp: sample i has value (i + 1) / 2^20, so one output sample gives the
// read position.
// crates/x3d-runtime/tests/audio_pull.rs asserts the same numbers for the Rust engine.
// Usage: node oracle/tools/webaudio_pull_probe.mjs > oracle/webaudio-pull/chrome.json
import { createRequire } from "node:module";
import { join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = normalize (join (fileURLToPath (import.meta.url), "../.."));
const { chromium } = createRequire (join (ROOT, "../../../x_ite-tests/")) ("playwright");

const browser = await chromium .launch ({ executablePath: "/usr/bin/google-chrome", headless: true, args: [ "--headless=new", "--no-sandbox" ] });
const page    = await browser .newPage ();

const result = await page .evaluate (async () =>
{
   const SR = 48000, LEN = SR * 2, SCALE = 2 ** 20;

   async function run (name, plan)
   {
      const ctx = new OfflineAudioContext ({ numberOfChannels: 1, length: LEN, sampleRate: SR });
      const buf = ctx .createBuffer (1, SR, SR);
      const d   = buf .getChannelData (0);
      for (let i = 0; i < d .length; ++ i) d [i] = (i + 1) / SCALE;
      const src  = new AudioBufferSourceNode (ctx, { buffer: buf, loop: !!plan .loop });
      const gain = new GainNode (ctx);           // X3DSoundSourceNode .audioSource
      const sink = plan .analyser ? new AnalyserNode (ctx) : new GainNode (ctx); // Sound .gainNode
      src .connect (gain);
      if (!plan .analyser) sink .connect (ctx .destination);
      const tap = new GainNode (ctx); tap .connect (ctx .destination);
      if (plan .analyser) ctx .suspend (plan .analyser) .then (() => { sink .connect (tap); ctx .resume (); });
      if (plan .connectedAtStart) gain .connect (sink);
      src .start (0);
      for (const [t, op] of plan .events)
         ctx .suspend (t) .then (() => { op === "connect" ? gain .connect (sink) : gain .disconnect (sink); ctx .resume (); });
      const out = (await ctx .startRendering ()) .getChannelData (0);
      // Position read at the first sample after each event (and at sample 0).
      const probes = [ 0, ... plan .events .map (([t]) => Math .round (t * SR)), ... (plan .analyser ? [ Math .round (plan .analyser * SR) ] : [ ]) ] .map (s =>
      {
         let k = s; while (k < out .length && out [k] === 0) ++ k;
         return { event: s, firstNonZero: k, bufferIndex: k < out .length ? Math .round (out [k] * SCALE) - 1 : null };
      });
      return { name, probes };
   }

   return [ { chrome: navigator .userAgent },
      await run ("connected-whole-time", { connectedAtStart: true, events: [ ] }),
      await run ("late-connect-0.25", { connectedAtStart: false, events: [ [ 0.25, "connect" ] ] }),
      await run ("analyser-unconnected-pulls-then-out-at-0.5", { connectedAtStart: true, analyser: 0.5, events: [ ] }),
      await run ("noloop-late-connect-1.5", { connectedAtStart: false, events: [ [ 1.5, "connect" ] ] }),
      await run ("loop-late-connect-1.5", { connectedAtStart: false, loop: true, events: [ [ 1.5, "connect" ] ] }),
      await run ("disconnect-0.25-reconnect-0.5", { connectedAtStart: true, events: [ [ 0.25, "disconnect" ], [ 0.5, "connect" ] ] }),
   ];
});

console .log (JSON .stringify (result, null, 1));
await browser .close ();
