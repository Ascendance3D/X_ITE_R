// WebM and Ogg sound 01 (Phase 9A, after ADR 0015 and mp4sound01): the sound of a WebM or Ogg MovieTexture in X_ITE_R's
// <x3d-canvas>, in real headless Chromium. As for an MP4, the page reads the file once, plays the pictures in a muted
// <video> element and decodes the sound with decodeAudioData (Chromium's Opus and Vorbis decoders); the engine plays the
// sound through the node's audio source on the movie time, with the leading silence of the container
// (web/video_mirror.mjs, Runtime::video_sound). As in X_ITE (MovieTexture.js:118-119, Sound.js:180-209), only a Sound node
// whose source is the MovieTexture makes it heard. No pinned recording applies: X_ITE's own element path is not compared.
//
// Both timings are run: the test page sets movietiming="pinned"; the ISO cases (ISO/IEC 19775-1 18.4.2, the default of
// the element) change the attribute before the canvas starts. In ISO timing the element is only seeked, so before Phase
// 9A its sound was not heard at all (the #384 note).
//
// The files are the synthetic ones of oracle/tools/gen_movie_websound.py: 2 s, 8 segments of 0.25 s, 300 to 1000 Hz.
// Measured in this Chromium (151): the WebM pictures play; Theora is not decoded (canPlayType is "", the element has no
// picture, 0 x 0), but the Ogg audio plays and decodeAudioData decodes it. The Ogg cases check the sound and that there is
// no picture. The level is measured by the tap of sound03_case.mjs on the page's AudioContext destination.

import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { CASE_TIMEOUT, DESKTOP, ONLY, openPage, server } from "./runtime02_harness.mjs";
import { installTap } from "./sound03_case.mjs";

// Keeps every <video> element the page makes in window .videos, before the world loads.
const KEEP_VIDEOS = () =>
{
   const create = Document .prototype .createElement;

   window .videos = [ ];
   Document .prototype .createElement = function (name, ... rest)
   {
      const element = create .call (this, name, ... rest);

      if (String (name) .toLowerCase () === "video")
         window .videos .push (element);

      return element;
   };
};

// ISO timing: the canvas reads `movietiming` when its module is ready, after the parser inserts it.
const ISO_TIMING = () =>
{
   new MutationObserver ((records, observer) =>
   {
      const canvas = document .querySelector ("x3d-canvas");

      if (!canvas)
         return;

      canvas .setAttribute ("movietiming", "iso");
      observer .disconnect ();
   })
   .observe (document, { childList: true, subtree: true });
};

const WORLD = (base, file, sound) => `#X3D V4.0 utf8
PROFILE Full
Viewpoint { position 0 0 4 }
Shape { appearance Appearance { texture DEF M MovieTexture { url "${base}/tests/browser/worlds/movie/${file}" loop TRUE } } geometry Rectangle2D { size 2 2 } }
${sound ? "Sound { spatialize FALSE minFront 100 minBack 100 maxFront 100 maxBack 100 source USE M }" : ""}
`;

const EMPTY_WORLD = "#X3D V4.0 utf8\nPROFILE Full\nViewpoint { position 0 0 4 }\n";

const open = timing => openPage (DESKTOP, {
   init: `(${KEEP_VIDEOS}) (); (${installTap}) (); ${timing === "iso" ? `(${ISO_TIMING}) ();` : ""}`,
});

// Loads the world, waits until the movie has played for a while, and returns what the page and the node show.
async function play (page, text)
{
   return page .evaluate (async text =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const later   = ms => new Promise (resolve => setTimeout (resolve, ms));

      await browser .replaceWorld (await browser .createX3DFromString (text));

      // The empty world has no M: getNamedNode throws, as X_ITE's does.
      let m = null;

      try { m = browser .currentScene .getNamedNode ("M"); } catch { }

      for (let i = 0; m && i < 100 && !(m .duration_changed > 0); ++ i)
         await later (50);

      await later (800);
      window .tapMax = 0;
      await later (1500);

      return {
         timing: document .querySelector ("x3d-canvas") .getAttribute ("movietiming"),
         hasSound: m ?.hasSound,
         duration: m && Number (m .duration_changed .toFixed (1)),
         isActive: m ?.isActive,
         tapped: window .tapped,
         level: window .tapMax,
         videos: window .videos .map (v => ({ muted: v .muted, blob: v .src .startsWith ("blob:"), width: v .videoWidth })),
      };
   },
   text);
}

// The level over `ms` after a time field of M is set to the browser's time now. The node takes the field in the next
// frame, and the page makes the engine's calls after that frame; on a slow runner (SwiftShader) a frame can take longer
// than the settle time. So the level is measured `wait` ms after M shows the state the field gives (isPaused for
// pauseTime and resumeTime, isActive for stopTime and startTime), within 10 s.
const STATE = { pauseTime: [ "isPaused", true ], resumeTime: [ "isPaused", false ], stopTime: [ "isActive", false ], startTime: [ "isActive", true ] };

async function levelAfter (page, field, wait = 400, ms = 1000)
{
   return page .evaluate (async ({ field, wait, ms, state }) =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const later   = ms => new Promise (resolve => setTimeout (resolve, ms));
      const m       = browser .currentScene .getNamedNode ("M");

      m [field] = browser .currentTime;

      for (let i = 0; i < 200 && m [state [0]] !== state [1]; ++ i)
         await later (50);

      await later (wait);
      window .tapMax = 0;
      await later (ms);

      return { level: window .tapMax, isActive: m .isActive, isPaused: m .isPaused };
   },
   { field, wait, ms, state: STATE [field] });
}

const TIMINGS = [ "iso", "pinned" ];
const FILES   = [
   { file: "vp8-opus.webm",     picture: true },
   { file: "vp8-vorbis.webm",   picture: true },
   { file: "theora-vorbis.ogv", picture: false },
   { file: "theora-opus.ogv",   picture: false },
];

async function withPage (timing, run)
{
   const page = await open (timing);

   try
   {
      await run (page);
   }
   finally
   {
      await page .context () .close ();
   }
}

describe ("webm and ogg sound 01", { skip: Boolean (ONLY) }, () =>
{
   for (const timing of TIMINGS)
   {
      for (const { file, picture } of FILES)
      {
         test (`${timing}: the sound of ${file} is heard through a Sound node, and the element stays muted`, { timeout: CASE_TIMEOUT }, () => withPage (timing, async page =>
         {
            const r = await play (page, WORLD (server .base, file, true));

            assert .equal (r .timing, timing);
            assert .equal (r .hasSound, true);
            assert .equal (r .duration, 2);
            assert .equal (r .isActive, true);
            assert .equal (r .videos .length, 1);
            assert .deepEqual (r .videos .map (v => [ v .muted, v .blob ]), [ [ true, true ] ]);
            // Measured: Chromium 151 decodes no Theora picture (the element is 0 x 0).
            assert .equal (r .videos [0] .width, picture ? 64 : 0, "the picture");
            assert .ok (r .tapped > 0, "the graph reaches the destination");
            assert .ok (r .level > 0.05, `the output carries the sound (level ${r .level})`);
         }));
      }

      for (const file of [ "vp8-opus.webm", "theora-vorbis.ogv" ])
      {
         test (`${timing}: without a Sound node ${file} is not heard`, { timeout: CASE_TIMEOUT }, () => withPage (timing, async page =>
         {
            const r = await play (page, WORLD (server .base, file, false));

            assert .equal (r .hasSound, true);
            assert .deepEqual (r .videos .map (v => [ v .muted, v .blob ]), [ [ true, true ] ]);
            assert .ok (r .level < 1e-4, `the output is silent (level ${r .level})`);
         }));
      }

      test (`${timing}: pause, resume and stop act on the sound`, { timeout: CASE_TIMEOUT }, () => withPage (timing, async page =>
      {
         const r = await play (page, WORLD (server .base, "vp8-opus.webm", true));

         assert .ok (r .level > 0.05, `playing (level ${r .level})`);

         const paused = await levelAfter (page, "pauseTime");

         assert .equal (paused .isPaused, true);
         assert .ok (paused .level < 1e-4, `paused: silent (level ${paused .level})`);

         const resumed = await levelAfter (page, "resumeTime");

         assert .equal (resumed .isPaused, false);
         assert .ok (resumed .level > 0.05, `resumed: heard (level ${resumed .level})`);

         const stopped = await levelAfter (page, "stopTime");

         assert .equal (stopped .isActive, false);
         assert .ok (stopped .level < 1e-4, `stopped: silent (level ${stopped .level})`);

         const started = await levelAfter (page, "startTime");

         assert .equal (started .isActive, true);
         assert .ok (started .level > 0.05, `started again: heard (level ${started .level})`);
      }));

      test (`${timing}: after a scene replacement only the new world's movie is heard, and the old element is released`, { timeout: CASE_TIMEOUT }, () => withPage (timing, async page =>
      {
         const first  = await play (page, WORLD (server .base, "vp8-opus.webm", true));
         const second = await play (page, WORLD (server .base, "theora-vorbis.ogv", true));

         assert .ok (first .level > 0.05, `first world (level ${first .level})`);
         assert .ok (second .level > 0.05, `the replacing world is heard (level ${second .level})`);
         // Two elements were made, both muted; the first one has no source any more.
         assert .deepEqual (second .videos .map (v => [ v .muted, v .blob ]), [ [ true, false ], [ true, true ] ]);

         // A world without a movie: no sound of an old world goes on.
         const empty = await play (page, EMPTY_WORLD);

         assert .ok (empty .level < 1e-4, `the empty world is silent (level ${empty .level})`);
         assert .deepEqual (empty .videos .map (v => v .blob), [ false, false ]);
      }));
   }
});
