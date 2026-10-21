// OscillatorNode pull probe (OscillatorSource): compare each render with the render that is
// connected all the time. crates/x3d-runtime/tests/audio_pull.rs asserts the same for the Rust
// engine. Usage: node oracle/tools/webaudio_pull_oscillator_probe.mjs > oracle/webaudio-pull/chrome-oscillator.json
import { createRequire } from "node:module";
import { join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = normalize (join (fileURLToPath (import.meta.url), "../.."));
const { chromium } = createRequire (join (ROOT, "../../../x_ite-tests/")) ("playwright");

const browser = await chromium .launch ({ executablePath: "/usr/bin/google-chrome", headless: true, args: [ "--headless=new", "--no-sandbox" ] });
const page    = await browser .newPage ();

const result = await page .evaluate (async () =>
{
   const SR = 48000, LEN = SR, N = 512;

   async function render (plan)
   {
      const ctx  = new OfflineAudioContext ({ numberOfChannels: 1, length: LEN, sampleRate: SR });
      const osc  = new OscillatorNode (ctx, { type: plan .type ?? "sine", frequency: 441 });
      const gain = new GainNode (ctx);
      const sink = new GainNode (ctx);
      osc .connect (gain);
      sink .connect (ctx .destination);
      if (plan .connected) gain .connect (sink);
      osc .start (0);
      if (plan .stop !== undefined) osc .stop (plan .stop);
      for (const [q, op] of plan .events ?? [ ])
         ctx .suspend (q * 128 / SR) .then (() => { op === "connect" ? gain .connect (sink) : gain .disconnect (sink); ctx .resume (); });
      return (await ctx .startRendering ()) .getChannelData (0);
   }

   const ref = await render ({ connected: true });
   const diff = (out, at, refAt) => { let m = 0; for (let i = 0; i < N; ++ i) m = Math .max (m, Math .abs (out [at + i] - ref [refAt + i])); return m; };
   const silent = (out, at, n) => out .slice (at, at + n) .every (v => v === 0);

   const late = await render ({ events: [ [ 94, "connect" ] ] });
   const re   = await render ({ connected: true, events: [ [ 94, "disconnect" ], [ 188, "connect" ] ] });
   const stopped = await render ({ stop: 0.1, events: [ [ 188, "connect" ] ] });

   return {
      chrome: navigator .userAgent,
      lateConnect94: { silentBefore: silent (late, 0, 12032), vsRunning: diff (late, 12032, 12032), vsFrozen: diff (late, 12032, 0) },
      reconnect188: { silentGap: silent (re, 12032, 12032), vsRunning: diff (re, 24064, 24064), vsFrozen: diff (re, 24064, 12032) },
      stoppedWhileUnpulled: { silentAfterConnect: silent (stopped, 24064, LEN - 24064) },
   };
});

console .log (JSON .stringify (result, null, 1));
await browser .close ();
