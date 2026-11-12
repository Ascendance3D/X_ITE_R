// Microphone 01: a MicrophoneSource in X_ITE_R's <x3d-canvas>, in real headless Chromium with Chromium's fake capture
// device (--use-fake-device-for-media-stream) and its permission prompt accepted (--use-fake-ui-for-media-stream).
//
// X_ITE's MicrophoneSource.set_start (MicrophoneSource.js:32-69) asks the page's getUserMedia ({ audio: { deviceId } }) and
// connects a MediaStreamAudioSourceNode into the node's audio source; set_stop stops the stream's tracks. In the page the
// engine records its calls (op "microphone"), and web/audio_mirror.mjs asks getUserMedia and connects the stream into a
// stand-in node. The level is measured by the tap of sound03_case.mjs on the page's AudioContext destination. Chromium's
// fake device plays a short beep every second, so a level is the largest over 2.5 s.

import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { CASE_TIMEOUT, DESKTOP, ONLY, openPage, useBrowserArgs } from "./runtime02_harness.mjs";
import { installTap } from "./sound03_case.mjs";

useBrowserArgs ([ "--use-fake-device-for-media-stream", "--use-fake-ui-for-media-stream" ]);

// Keeps the MediaStreams that getUserMedia gives in window .streams, and the requests in window .requests.
const KEEP_STREAMS = () =>
{
   const getUserMedia = MediaDevices .prototype .getUserMedia;

   window .streams  = [ ];
   window .requests = [ ];
   MediaDevices .prototype .getUserMedia = async function (constraints)
   {
      window .requests .push (JSON .stringify (constraints));

      const stream = await getUserMedia .call (this, constraints);

      window .streams .push (stream);
      return stream;
   };
};

const WORLD = sound => `#X3D V4.0 utf8
PROFILE Full
COMPONENT Sound:2
Viewpoint { position 0 0 4 }
${sound ? "Sound { spatialize FALSE minFront 100 minBack 100 maxFront 100 maxBack 100 source DEF M MicrophoneSource { } }" : "DEF M MicrophoneSource { }"}
`;

// getUserMedia refuses, as for a denied permission; console .error lines go to window .errors as well.
const REFUSE = () =>
{
   const error = console .error;

   window .errors = [ ];
   console .error = (... args) => { window .errors .push (args .join (" ")); error .apply (console, args); };
   MediaDevices .prototype .getUserMedia = () => Promise .reject (new DOMException ("Permission denied", "NotAllowedError"));
};

const open = (refuse = false) => openPage (DESKTOP, { init: `(${KEEP_STREAMS}) (); ${refuse ? `(${REFUSE}) ();` : ""} (${installTap}) ();` });

// Sets a time field of M to the browser's time (or loads `text` and starts M), waits, and returns the level and state.
async function after (page, { text, field }, ms = 2500)
{
   return page .evaluate (async ({ text, field, ms }) =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const later   = ms => new Promise (resolve => setTimeout (resolve, ms));

      if (text)
      {
         await browser .replaceWorld (await browser .createX3DFromString (text));
         field = "startTime";
      }

      browser .currentScene .getNamedNode ("M") [field] = browser .currentTime;

      await later (500);
      window .tapMax = 0;
      await later (ms);

      const m = browser .currentScene .getNamedNode ("M");

      return {
         level: window .tapMax,
         isActive: m .isActive,
         requests: window .requests,
         tracks: window .streams .map (s => s .getAudioTracks () .map (t => t .readyState) .join (",")),
         errors: window .errors,
      };
   },
   { text, field, ms });
}

describe ("microphone 01", { skip: Boolean (ONLY) }, () =>
{
   test ("the microphone is heard through a Sound node, and stopTime stops its tracks", { timeout: CASE_TIMEOUT }, async () =>
   {
      const page = await open ();

      try
      {
         const started = await after (page, { text: WORLD (true) });

         assert .equal (started .isActive, true);
         assert .deepEqual (started .requests, [ JSON .stringify ({ audio: { deviceId: "" } }) ]);
         assert .deepEqual (started .tracks, [ "live" ]);
         assert .ok (started .level > 0.01, `the fake device is heard (level ${started .level})`);

         const stopped = await after (page, { field: "stopTime" });

         assert .equal (stopped .isActive, false);
         assert .deepEqual (stopped .tracks, [ "ended" ], "set_stop stops the tracks");
         assert .ok (stopped .level < 1e-4, `stopped: silent (level ${stopped .level})`);

         // A new start asks again (MicrophoneSource.set_start).
         const again = await after (page, { field: "startTime" });

         assert .equal (again .requests .length, 2);
         assert .deepEqual (again .tracks, [ "ended", "live" ]);
         assert .ok (again .level > 0.01, `started again: heard (level ${again .level})`);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("without a Sound node the microphone is not heard, and a replaced world stops it", { timeout: CASE_TIMEOUT }, async () =>
   {
      const page = await open ();

      try
      {
         const alone = await after (page, { text: WORLD (false) });

         assert .deepEqual (alone .tracks, [ "live" ]);
         assert .ok (alone .level < 1e-4, `the output is silent (level ${alone .level})`);

         const heard = await after (page, { text: WORLD (true) });

         assert .deepEqual (heard .tracks, [ "ended", "live" ], "the old world's stream stopped");
         assert .ok (heard .level > 0.01, `the new world is heard (level ${heard .level})`);
      }
      finally
      {
         await page .context () .close ();
      }
   });

   test ("a refused getUserMedia logs its message, and the node stays silent and active", { timeout: CASE_TIMEOUT }, async () =>
   {
      const page   = await open (true);
      const thrown = [ ];

      page .on ("pageerror", error => thrown .push (error .message));

      try
      {
         const refused = await after (page, { text: WORLD (true) });

         assert .equal (refused .isActive, true);
         assert .deepEqual (refused .tracks, [ ]);
         assert .ok (refused .errors .includes ("Permission denied"), `the message is logged: ${JSON .stringify (refused .errors)}`);
         assert .ok (refused .level < 1e-4, `silent (level ${refused .level})`);
         assert .deepEqual (thrown, [ ], "no uncaught error");
      }
      finally
      {
         await page .context () .close ();
      }
   });
});
