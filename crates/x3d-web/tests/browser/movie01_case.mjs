// The web MovieTexture 01 cases: the frames a MovieTexture shows over time (GIF, APNG, WebM and MP4 video) and the
// candidates after a video that does not play. oracle/x_ite-web-runtime-02.mjs (--suite movie01) runs them against
// pinned X_ITE 16.5.0 and records the values; movie01.test.mjs runs them against X_ITE_R and compares
// (runtime02_harness.mjs).
//
// The media are clean synthetic files (worlds/movie, oracle/tools/gen_movie_web.py): 16 x 16 pixels (GIF, PNG) or
// 64 x 64 (video), four solid frames of one second each: red, green, blue, yellow. The world shows them on an unlit
// rectangle. The time is the browser's: the MovieTexture starts at a time the page reads, and the cases read the
// color at the middle of each frame, so the values do not depend on the frame timing of either engine.

import { readFileSync } from "node:fs";
import { DESKTOP, settle } from "./runtime02_case.mjs";
import { orderedMessages, sceneCase } from "./loadurl01_case.mjs";

const COLORS = { red: [ 255, 0, 0 ], green: [ 0, 255, 0 ], blue: [ 0, 0, 255 ], yellow: [ 255, 255, 0 ] };

// The name of the color at the middle of the canvas picture: a color of COLORS within 40 per channel, "white" for the
// background, else the RGB values.
async function centerColor (page, env)
{
   const png = await env .picture (page);

   const [ r, g, b ] = await page .evaluate (async base64 =>
   {
      const bitmap = await createImageBitmap (await (await fetch (`data:image/png;base64,${base64}`)) .blob ());
      const canvas = new OffscreenCanvas (bitmap .width, bitmap .height);
      const c2d    = canvas .getContext ("2d");

      c2d .drawImage (bitmap, 0, 0);

      return Array .from (c2d .getImageData (bitmap .width >> 1, bitmap .height >> 1, 1, 1) .data .slice (0, 3));
   },
   png .toString ("base64"));

   for (const [ name, c ] of Object .entries (COLORS))
   {
      if (Math .abs (c [0] - r) < 40 && Math .abs (c [1] - g) < 40 && Math .abs (c [2] - b) < 40)
         return name;
   }

   return r > 215 && g > 215 && b > 215 ? "white" : `${r},${g},${b}`;
}

const MOVIE_WORLD = (urls, fields = "") => `#X3D V4.0 utf8
PROFILE Full
NavigationInfo { type "NONE" headlight FALSE }
Viewpoint { position 0 0 4 }
Shape { appearance Appearance { texture DEF M MovieTexture { url [ ${urls .map (u => `"${u}"`) .join (" ")} ] ${fields} } } geometry Rectangle2D { size 2 2 } }
`;

// The fields of the node that tell what it loaded.
const nodeState = page => page .evaluate (() =>
{
   const m = document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("M");

   return {
      width: m .width,
      height: m .height,
      colorDepth: m .colorDepth,
      duration: Number (m .duration_changed .toFixed (2)),
   };
});

// The video, GIF or PNG at `url` plays from a start time the page sets (with `loop`). The color before the start, then at the
// middle of each of the four frames and once more in the next loop, and the node's state; `fields` goes into the node.
async function framesCase (page, env, url, fields = "", loop = true)
{
   const c = await sceneCase (page, env, MOVIE_WORLD ([ url ], fields), 1500);

   const state  = await nodeState (page);
   const before = await centerColor (page, env);

   // The start time is the page's clock, which both engines read as Date.now ().
   await page .evaluate (loop =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const t0      = Date .now () / 1000;
      const m       = browser .currentScene .getNamedNode ("M");

      m .loop      = loop;
      m .startTime = t0;
      window .movieT0 = t0;
   },
   loop);

   const colors = [ ];

   for (const offset of [ 0.5, 1.5, 2.5, 3.5, 4.5 ])
   {
      await page .evaluate (offset => new Promise (resolve =>
      {
         const wait = () => Date .now () / 1000 - window .movieT0 >= offset ? resolve () : setTimeout (wait, 5);

         wait ();
      }),
      offset);

      colors .push (await centerColor (page, env));
   }

   const active = await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("M") .isActive);

   return { loaded: await c .loaded (0), state, before, colors, active, console: orderedMessages (page .log, env) .map (line => line .replace (/[?&]_=\d+/, "")) };
}

const GIF  = "BASE/tests/browser/worlds/movie/steps4.gif";
const PNG  = "BASE/tests/browser/worlds/movie/steps4.png";
const WEBM = "BASE/tests/browser/worlds/movie/steps4.webm";
const MP4  = "BASE/tests/browser/worlds/movie/steps4.mp4";
const BAD  = "BASE/tests/browser/worlds/movie/broken.webm";

const fixture = path => readFileSync (new URL (`./worlds/movie/${path}`, import.meta.url));

export const movieFramesCases = {
   // A GIF: the frame comes from the node's elapsed time in equal steps.
   gif: (page, env) => framesCase (page, env, GIF),

   // An APNG: apng-js plays it on the wall clock.
   apng: (page, env) => framesCase (page, env, PNG),

   // A WebM video (VP8) in a video element.
   webm: (page, env) => framesCase (page, env, WEBM),

   // An MP4 video (H.264).
   mp4: (page, env) => framesCase (page, env, MP4),

   // The GIF as a data URL.
   gifData: (page, env) => framesCase (page, env, `data:image/gif;base64,${fixture ("steps4.gif") .toString ("base64")}`),

   // The GIF does not loop: it stops on its last frame.
   gifOnce: (page, env) => framesCase (page, env, GIF, "", false),

   // The GIF at speed 0.5: each frame lasts two seconds (the frames at 0.5, 1.5, ... are red, red, green, green, blue).
   gifSlow: (page, env) => framesCase (page, env, GIF, "speed 0.5"),
};

// A candidate after a candidate that does not play or load.
async function candidatesCase (page, env, urls, wait = 4000)
{
   const c = await sceneCase (page, env, MOVIE_WORLD (urls), wait);

   const state = await nodeState (page);

   await page .evaluate (() => { document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("M") .startTime = 1; });
   await settle (page, 500);

   return { loaded: await c .loaded (0), state, color: await centerColor (page, env), served: c .served (), console: c .console () .map (line => line .replace (/[?&]_=\d+/, "")) };
}

export const movieFallbackCases = {
   // A video file that is no video, then a GIF: the video element fails and the GIF is requested after it.
   brokenThenGif: (page, env) => candidatesCase (page, env, [ BAD, GIF ]),

   // A video that is no video, then a video that plays.
   brokenThenWebm: (page, env) => candidatesCase (page, env, [ BAD, WEBM ]),

   // A video that does not exist, then an MP4 that plays.
   missingThenMp4: (page, env) => candidatesCase (page, env, [ "BASE/status/404/missing.webm", MP4 ]),

   // A GIF that is no GIF, then a video.
   notGifThenWebm: (page, env) => candidatesCase (page, env, [ "BASE/garbage/n1.gif", WEBM ]),

   // The first candidate plays: the second is never requested.
   firstPlays: (page, env) => candidatesCase (page, env, [ WEBM, GIF ]),

   // No candidate plays.
   noneplay: (page, env) => candidatesCase (page, env, [ BAD, "BASE/status/404/missing.mp4" ]),
};

export const CASE_GROUPS = {
   movieFrames: { context: DESKTOP, browserArgs: [ "--autoplay-policy=no-user-gesture-required" ], cases: movieFramesCases },
   movieFallback: { context: DESKTOP, browserArgs: [ "--autoplay-policy=no-user-gesture-required" ], cases: movieFallbackCases },
};
