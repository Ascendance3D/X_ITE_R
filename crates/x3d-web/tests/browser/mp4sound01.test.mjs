// MP4 sound 01 (ADR 0015): the sound of an MP4 MovieTexture in X_ITE_R's <x3d-canvas>, in real headless Chromium. The page
// reads the file once, plays the pictures in a muted <video> element and decodes the sound with decodeAudioData (the
// platform's AAC decoder); the engine plays the sound through the node's audio source on the movie time
// (web/video_mirror.mjs, Runtime::video_sound). As in X_ITE (MovieTexture.js:118-119, Sound.js:180-209), only a Sound node
// whose source is the MovieTexture makes it heard. No pinned recording applies: X_ITE's own element path is not compared.
//
// The file is the synthetic tests/data/mp4/h264-aac.mp4 of x3d-runtime (oracle/tools/gen_movie_mp4.py): 2 s, 8 segments
// of 0.25 s, 300 to 1000 Hz. The level is measured by the tap of sound03_case.mjs on the page's AudioContext destination.

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

const WORLD = (base, sound) => `#X3D V4.0 utf8
PROFILE Full
Viewpoint { position 0 0 4 }
Shape { appearance Appearance { texture DEF M MovieTexture { url "${base}/tests/browser/worlds/movie/h264-aac.mp4" loop TRUE } } geometry Rectangle2D { size 2 2 } }
${sound ? "Sound { spatialize FALSE minFront 100 minBack 100 maxFront 100 maxBack 100 source USE M }" : ""}
`;

const open = () => openPage (DESKTOP, { init: `(${KEEP_VIDEOS}) (); (${installTap}) ();` });

// Loads the world, waits until the movie has played for a while, and returns what the page and the node show.
async function play (page, text)
{
   return page .evaluate (async text =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const later   = ms => new Promise (resolve => setTimeout (resolve, ms));

      await browser .replaceWorld (await browser .createX3DFromString (text));

      const m = browser .currentScene .getNamedNode ("M");

      for (let i = 0; i < 100 && !(m .duration_changed > 0); ++ i)
         await later (50);

      await later (800);
      window .tapMax = 0;
      await later (1500);

      return {
         hasSound: m .hasSound,
         duration: Number (m .duration_changed .toFixed (2)),
         isActive: m .isActive,
         tapped: window .tapped,
         level: window .tapMax,
         videos: window .videos .map (v => ({ muted: v .muted, blob: v .src .startsWith ("blob:") })),
      };
   },
   text);
}

describe ("mp4 sound 01", { skip: Boolean (ONLY), timeout: CASE_TIMEOUT }, () =>
{
   test ("the sound of an MP4 is heard through a Sound node, and the element stays muted", async () =>
   {
      const page = await open ();

      try
      {
         const r = await play (page, WORLD (server .base, true));

         assert .equal (r .hasSound, true);
         assert .equal (r .duration, 2);
         assert .equal (r .isActive, true);
         assert .deepEqual (r .videos, [ { muted: true, blob: true } ]);
         assert .ok (r .tapped > 0, "the graph reaches the destination");
         assert .ok (r .level > 0.05, `the output carries the sound (level ${r .level})`);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("after a scene replacement the new world's MP4 is heard, and the old element is released", async () =>
   {
      const page = await open ();

      try
      {
         const first  = await play (page, WORLD (server .base, true));
         const second = await play (page, WORLD (server .base, true));

         assert .ok (first .level > 0.05, `first world (level ${first .level})`);
         assert .ok (second .level > 0.05, `the replacing world is heard (level ${second .level})`);
         // Two elements were made, both muted; the first one has no source any more.
         assert .equal (second .videos .length, 2);
         assert .ok (second .videos .every (v => v .muted));
         assert .deepEqual (second .videos .map (v => v .blob), [ false, true ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("without a Sound node the MP4 is not heard", async () =>
   {
      const page = await open ();

      try
      {
         const r = await play (page, WORLD (server .base, false));

         assert .equal (r .hasSound, true);
         assert .deepEqual (r .videos, [ { muted: true, blob: true } ]);
         assert .ok (r .level < 1e-4, `the output is silent (level ${r .level})`);
      }
      finally
      {
         await page .context () .close ();
      }
   });
});
