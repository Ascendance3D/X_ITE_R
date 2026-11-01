/**
 * decodeAudioData oracle: decode each audio file with the AudioContext of the pinned X_ITE
 * (`browser.getAudioContext ()`, the context BufferAudioSource.setArrayBuffer and AudioClip decode with)
 * in headless Chromium, and record length, sample rate, channel count, a sha256 of each channel's float32
 * bytes, a few samples and the maximum absolute value. The samples themselves are recorded as base64
 * float32 (little endian) so the Rust test can measure differences.
 *
 *   node oracle/x_ite-audio-decode.mjs <dir> <out.json> [--runs N]
 *
 * With --runs N the whole recording is repeated in fresh browsers and the run fails unless all N
 * recordings are byte-identical. Loads the pinned src/dump.html page unedited (as x_ite-formats.mjs).
 * Prepare a worktree as in oracle/README.md "Recording from a worktree".
 */
import { createServer } from "node:http";
import { readFile, readdir, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { extname, join, normalize, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
const { chromium } = createRequire (join (process .env .XITE_TESTS ?? join (ROOT, "../../../x_ite-tests"), "/")) ("playwright");

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json" };

const args   = process .argv .slice (2);
const runsAt = args .indexOf ("--runs");
const runs   = runsAt >= 0 ? Number (args .splice (runsAt, 2) [1]) : 1;
const [ corpusArg, outArg ] = args;
const corpus = normalize (join (process .cwd (), corpusArg));
const files  = (await readdir (corpus))
   .filter (f => /\.(wav|mp3|ogg|flac|opus)$/i .test (f))
   .sort ();

const server = createServer (async (req, res) =>
{
   const path = normalize (join (ROOT, decodeURIComponent (new URL (req .url, "http://x") .pathname)));

   if (!path .startsWith (ROOT)) { res .writeHead (403); return res .end (); }

   try
   {
      const body = await readFile (path);
      res .writeHead (200, { "Content-Type": TYPES [extname (path)] ?? "application/octet-stream", "Cache-Control": "no-store" });
      res .end (body);
   }
   catch { res .writeHead (404, { "Cache-Control": "no-store" }); res .end (); }
});

await new Promise (r => server .listen (0, "127.0.0.1", r));

const base = `http://127.0.0.1:${server .address () .port}`;

async function record ()
{
   const browser = await chromium .launch ({ headless: true });
   const context = await browser .newContext ();
   await context .route (url => !url .href .startsWith (base), route => route .abort ());

   const page = await context .newPage ();
   await page .goto (`${base}/x_ite-pin/src/dump.html`);
   await page .waitForFunction (() => window .ready, null, { timeout: 30000 });

   const results = [ ];

   for (const file of files)
   {
      const url = `${base}/${relative (ROOT, join (corpus, file))}`;
      const r   = await page .evaluate (async u =>
      {
         const hex = async bytes => [ ... new Uint8Array (await crypto .subtle .digest ("SHA-256", bytes)) ] .map (b => b .toString (16) .padStart (2, "0")) .join ("");
         const b64 = bytes => { let s = ""; for (let i = 0; i < bytes .length; i += 8192) s += String .fromCharCode (... bytes .subarray (i, i + 8192)); return btoa (s); };
         const bits = f => { const v = new DataView (new ArrayBuffer (4)); v .setFloat32 (0, f); return v .getUint32 (0) .toString (16) .padStart (8, "0"); };

         try
         {
            const browser = document .getElementById ("c") .browser;
            const ctx     = browser .getAudioContext ();
            const data    = await (await fetch (u)) .arrayBuffer ();
            const buffer  = await ctx .decodeAudioData (data);
            const channels = [ ];

            for (let c = 0; c < buffer .numberOfChannels; ++ c)
            {
               const d     = buffer .getChannelData (c);
               const bytes = new Uint8Array (d .buffer, d .byteOffset, d .byteLength);
               const mid   = d .length >> 1;
               let max = 0;

               for (const v of d) max = Math .max (max, Math .abs (v));

               channels .push ({
                  sha256: await hex (bytes),
                  maxAbs: max,
                  maxAbsBits: bits (max),
                  first: [ ... d .subarray (0, 4) ] .map (bits),
                  mid: [ ... d .subarray (mid, mid + 4) ] .map (bits),
                  last: [ ... d .subarray (d .length - 4) ] .map (bits),
                  data: b64 (bytes),
               });
            }

            return { ok: true, contextSampleRate: ctx .sampleRate, length: buffer .length, sampleRate: buffer .sampleRate,
                     numberOfChannels: buffer .numberOfChannels, duration: buffer .duration, channels };
         }
         catch (e)
         {
            return { ok: false, error: `${e ?.name}: ${e ?.message}` };
         }
      }, url);

      results .push ({ file, ... r });
   }

   const userAgent = await page .evaluate (() => navigator .userAgent);
   await browser .close ();
   return { pinned: (await readFile (join (ROOT, "x_ite-pin/PINNED_COMMIT"), "utf8")) .trim (), userAgent, results };
}

const texts = [ ];

for (let i = 0; i < runs; ++ i)
{
   texts .push (JSON .stringify (await record (), null, 1));
   console .log (`run ${i + 1}: ${texts [i] .length} bytes`);
}

server .close ();

if (texts .some (t => t !== texts [0]))
{
   console .error ("recordings differ between runs");
   process .exit (1);
}

await writeFile (outArg, texts [0]);

for (const r of JSON .parse (texts [0]) .results)
   console .log (r .ok ? `ok   ${r .file} rate=${r .sampleRate} ch=${r .numberOfChannels} len=${r .length} ctx=${r .contextSampleRate}` : `FAIL ${r .file} ${r .error}`);
console .log (`${runs} identical run(s)`);
