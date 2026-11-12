// Analyser 01: the data fields of an Analyser node in X_ITE_R's <x3d-canvas>, in real headless Chromium.
//
// X_ITE's Analyser.set_time (Analyser.js:60-87) reads its four data fields every frame from the page's AnalyserNode
// (getByteFrequencyData, getByteTimeDomainData, getFloatFrequencyData, getFloatTimeDomainData). In the page the engine's own
// graph only records the calls (web/audio_mirror.mjs makes them on the page's nodes; no signal goes through the engine's
// graph), so the fields must come from the page's AnalyserNode, as in X_ITE.
//
// The world: an OscillatorSource (sine) of FREQUENCY Hz into an Analyser (fftSize 2048) into an AudioDestination; the test
// sets the startTime of both (neither has a loop field). It reads the fields through the SAI and checks them against the
// frequency, the page context's sample rate and the analyser's size. A page AnalyserNode is recorded when the engine's
// mirror makes it (createAnalyser).

import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { CASE_TIMEOUT, DESKTOP, ONLY, openPage, server } from "./runtime02_harness.mjs";

const FREQUENCY = 1500;

// Keeps the page's AnalyserNodes in window .pageAnalysers.
const KEEP_ANALYSERS = () =>
{
   const create = BaseAudioContext .prototype .createAnalyser;

   window .pageAnalysers = [ ];
   BaseAudioContext .prototype .createAnalyser = function (... args)
   {
      const analyser = create .apply (this, args);

      window .pageAnalysers .push (analyser);
      return analyser;
   };
};

const WORLD = (fftSize, frequency) => `#X3D V4.0 utf8
PROFILE Full
COMPONENT Sound:2
Viewpoint { position 0 0 4 }
AudioDestination { children DEF A Analyser { fftSize ${fftSize} children DEF O OscillatorSource { frequency ${frequency} } } }
`;

const open = () => openPage (DESKTOP, { init: `(${KEEP_ANALYSERS}) ();` });

// Loads the world, waits `ms`, and returns the fields of A and what the page shows.
async function read (page, text, ms = 1500)
{
   return page .evaluate (async ({ text, ms }) =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const later   = ms => new Promise (resolve => setTimeout (resolve, ms));

      if (text)
      {
         await browser .replaceWorld (await browser .createX3DFromString (text));

         // Neither node has a loop field: both start on their startTime (X3DTimeDependentNode.set_startTime__).
         for (const name of [ "O", "A" ])
            browser .currentScene .getNamedNode (name) .startTime = browser .currentTime;
      }

      await later (ms);

      const a = browser .currentScene .getNamedNode ("A");

      return {
         analysers: window .pageAnalysers .length,
         rate: window .pageAnalysers [0] ?.context .sampleRate,
         pageBins: window .pageAnalysers .at (-1) ?.frequencyBinCount,
         bins: a .frequencyBinCount,
         bf: Array .from (a .byteFrequencyData),
         bt: Array .from (a .byteTimeDomainData),
         ff: Array .from (a .floatFrequencyData),
         ft: Array .from (a .floatTimeDomainData),
      };
   },
   { text, ms });
}

const argmax = values => values .reduce ((best, v, i) => v > values [best] ? i : best, 0);

function assertTone (r, fftSize, frequency)
{
   const bins = fftSize / 2;

   assert .equal (r .analysers >= 1, true, "the mirror made a page AnalyserNode");
   assert .equal (r .bins, bins);
   assert .equal (r .pageBins, bins);

   for (const name of [ "bf", "bt", "ff", "ft" ])
      assert .equal (r [name] .length, bins, `${name}: one value per bin`);

   // The float peak is the bin of the tone (one bin of tolerance: a sine between two bins). The bytes saturate at 255 over
   // several bins near a loud tone (maxDecibels -30), so they are checked at the tone's bin and far from it.
   const expected  = frequency * fftSize / r .rate;
   const floatPeak = argmax (r .ff);
   const toneBin   = Math .round (expected);

   assert .ok (Math .abs (floatPeak - expected) <= 1, `floatFrequencyData peak ${floatPeak}, expected ${expected}`);
   assert .ok (r .bf [toneBin] > 200, `byteFrequencyData at the tone ${r .bf [toneBin]}`);
   assert .ok (r .bf [Math .round (bins * 0.9)] < 50, `byteFrequencyData far from the tone ${r .bf [Math .round (bins * 0.9)]}`);

   // The time domain is the sine (amplitude 1): bytes around 128, floats within [-1, 1] with a large swing.
   const ft = r .ft;

   const [ low, high ] = [ Math .min (... ft), Math .max (... ft) ];

   assert .ok (high > 0.9 && low < -0.9, `float swing ${low} ${high}`);
   // Chromium's oscillator can go a little above 1 (1.0076 measured at 44.1 kHz and 3000 Hz).
   assert .ok (ft .every (v => Math .abs (v) <= 1.05), "floats about within [-1, 1]");
   const [ byteLow, byteHigh ] = [ Math .min (... r .bt), Math .max (... r .bt) ];

   assert .ok (byteHigh > 240 && byteLow < 16, `byte swing ${byteLow} ${byteHigh}`);
}

describe ("analyser 01", { skip: Boolean (ONLY) }, () =>
{
   test ("the data fields of an Analyser come from the page's AnalyserNode", { timeout: CASE_TIMEOUT }, async () =>
   {
      const page = await open ();

      try
      {
         assertTone (await read (page, WORLD (2048, FREQUENCY)), 2048, FREQUENCY);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("an fftSize change gives the new bin count, and a new world its tone", { timeout: CASE_TIMEOUT }, async () =>
   {
      const page = await open ();

      try
      {
         assertTone (await read (page, WORLD (2048, FREQUENCY)), 2048, FREQUENCY);

         await page .evaluate (() =>
         {
            document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("A") .fftSize = 512;
         });
         assertTone (await read (page, null, 800), 512, FREQUENCY);

         assertTone (await read (page, WORLD (1024, 3000)), 1024, 3000);
      }
      finally
      {
         await page .context () .close ();
      }
   });
});
