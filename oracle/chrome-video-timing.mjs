/**
 * Which picture of a video the pinned browser shows at which media time (issue #90, playback timing).
 *
 *   node oracle/chrome-video-timing.mjs <video.webm> <out.json> [--pictures 8] [--runs 3]
 *
 * X_ITE 16.5.0 puts the picture of a `video` element on the MovieTexture (MovieTexture.js setVideo / set_time), so
 * what the element presents is what X_ITE shows. The video is one whose picture k is a uniform gray of luma
 * 16 + 28 k (oracle/tools/gen_movie_codecs.py, vp8-ramp), played muted in headless Chrome. For every presented
 * picture (requestVideoFrameCallback) the script records its media time and the picture index read from the canvas.
 * The run is repeated and every run must give the same list. Output: [{ mediaTime, index }, ...].
 *
 * Headless only, local requests only (the video is a data: URL).
 */
import { readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { checkOutput } from "./check-output.mjs";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
const { chromium } = createRequire (join (ROOT, "../../../x_ite-tests/")) ("playwright");

const argv = process .argv .slice (2);
const opt  = (name, def) => { const i = argv .indexOf (name); return i >= 0 ? argv [i + 1] : def; };
const [ videoArg, outArg ] = argv .filter ((a, i) => !a .startsWith ("--") && !argv [i - 1] ?.startsWith ("--"));
const PICTURES = Number (opt ("--pictures", "8"));
const RUNS     = Number (opt ("--runs", "3"));

checkOutput (dirname (resolve (outArg)));

const data    = readFileSync (resolve (videoArg)) .toString ("base64");
const browser = await chromium .launch ({
   executablePath: "/usr/bin/google-chrome",
   headless: true,
   args: [ "--headless=new", "--no-sandbox", "--autoplay-policy=no-user-gesture-required" ],
});

const runs = [ ];

for (let r = 0; r < RUNS; ++ r)
{
   const page = await browser .newPage ();

   await page .goto ("about:blank");

   runs .push (await page .evaluate (async ([ d, n ]) =>
   {
      const video  = document .createElement ("video");
      const canvas = new OffscreenCanvas (8, 8);
      const c2d    = canvas .getContext ("2d", { willReadFrequently: true });
      const seen   = [ ];

      video .muted = true;
      video .preload = "auto";
      video .src = `data:video/webm;base64,${d}`;

      await new Promise (resolve => { video .onloadeddata = resolve; });

      const done = new Promise (resolve =>
      {
         const tick = (now, metadata) =>
         {
            c2d .drawImage (video, 0, 0, 8, 8);

            const [ red ] = c2d .getImageData (4, 4, 1, 1) .data;

            // luma 16 + 28 k gives gray 1.164 * 28 k = 32.6 k.
            seen .push ({ mediaTime: Math .round (metadata .mediaTime * 1000) / 1000, index: Math .round (red / 32.6) });

            if (seen .length >= n || video .ended)
               resolve ();
            else
               video .requestVideoFrameCallback (tick);
         };

         video .requestVideoFrameCallback (tick);
         video .onended = () => resolve ();
      });

      await video .play ();
      await Promise .race ([ done, new Promise (resolve => setTimeout (resolve, 5000)) ]);

      return seen;
   },
   [ data, PICTURES ]));

   await page .close ();
}

const version = browser .version ();

await browser .close ();

for (const run of runs)
{
   if (JSON .stringify (run) !== JSON .stringify (runs [0]))
   {
      console .error ("the runs differ:", JSON .stringify (runs));
      process .exit (1);
   }
}

writeFileSync (resolve (outArg), JSON .stringify ({ chrome: version, video: videoArg .split ("/") .pop (), runs: RUNS, pictures: runs [0] }, null, 1) + "\n");
console .log (`${runs [0] .length} pictures, ${RUNS} identical runs, Chrome ${version}`);
