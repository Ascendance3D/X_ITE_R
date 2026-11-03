// VideoMirror 01: the ISO 19775-1 18.4.2 seek protocol of the page's <video> elements (video_mirror.mjs), with an
// EMULATED element and an EMULATED engine. No browser, no wasm, no wall time: the element completes a seek only when the
// test says so, in any order, so the test controls the order of completions.
//
// What this checks is the page side: the element is never played, each `seek` call is answered with the id of the request
// whose picture is sent, only one seek runs at a time, a request that waits while another runs supersedes the older
// waiting one, and the picture of a superseded seek is not sent. The engine side (an answer older than the newest request
// is dropped) is tested in crates/x3d-runtime/tests/movie_iso_time.rs and movie_time_pure.rs.

import test from "node:test";
import assert from "node:assert/strict";

// video_mirror.mjs reads `document` and `OffscreenCanvas` only when it makes an element or reads a picture.
class FakeVideo
{
   currentTime = 0;
   readyState  = 4;
   seeking     = false;
   played      = 0;
   listeners   = new Set ();
   videoWidth  = 2;
   videoHeight = 2;
   set src (value) { this .url = value; }
   addEventListener (name, f)    { if (name === "seeked") this .listeners .add (f); }
   removeEventListener (name, f) { this .listeners .delete (f); }
   pause () { }
   play () { this .played ++; return Promise .resolve (); }
   removeAttribute () { }
   load () { }
   setAttribute () { }
   // A seek that the test completes: `time` is the position it ends at.
   begin (time)  { this .seeking = true; this .target = time; }
   complete ()   { this .seeking = false; this .currentTime = this .target; for (const f of [ ... this .listeners ]) f (); }
}

class FakeCanvas
{
   width = 0; height = 0;
   getContext () { return { drawImage () { }, getImageData: (x, y, w, h) => ({ data: new Uint8ClampedArray (w * h * 4) .fill (this .fill ?? 0) }) }; }
}

globalThis .OffscreenCanvas = FakeCanvas;
globalThis .queueMicrotask  = globalThis .queueMicrotask ?? (f => Promise .resolve () .then (f));

const videos = [ ];

globalThis .document = { createElement: () =>
{
   const video = new FakeVideo ();

   // Setting currentTime starts a seek that the test completes.
   let time = 0;

   Object .defineProperty (video, "currentTime", { get: () => time, set: value => { video .begin (value); }, configurable: true });
   video .complete = function () { this .seeking = false; time = this .target; for (const f of [ ... this .listeners ]) f (); };
   videos .push (video);
   return video;
} };

const { VideoMirror } = await import ("../../web/video_mirror.mjs");

function setup ()
{
   videos .length = 0;

   const answers = [ ];
   const engine  = {
      videoLoaded: () => { },
      videoFailed: () => { },
      videoFrame: () => assert .fail ("ISO timing sends no videoFrame"),
      videoSeeked: (token, id, time, pixels) => answers .push ({ token, id, time, size: pixels .length }),
   };
   const mirror = new VideoMirror (() => engine);

   mirror .apply ({ live: [ 7 ], ops: [ [ "load", 7, "clip.webm" ] ] });

   const video = videos [0];

   video .onloadeddata ();
   return { mirror, video, answers, engine };
}

const seek = (mirror, id, time) => mirror .apply ({ live: [ 7 ], ops: [ [ "seek", 7, id, time ] ] });

test ("a seek call seeks the paused element and is answered with its id", () =>
{
   const { mirror, video, answers } = setup ();

   seek (mirror, 1, 2.5);
   assert .equal (video .target, 2.5);
   assert .equal (video .played, 0, "the element is never played");
   assert .equal (answers .length, 0, "no answer before the seek completes");

   video .complete ();
   assert .deepEqual (answers .map (a => [ a .id, a .time ]), [ [ 1, 2.5 ] ]);
   assert .equal (answers [0] .size, 16, "the picture came with the answer");
});

test ("a request that arrives while a seek runs waits and supersedes older waiting requests", () =>
{
   const { mirror, video, answers } = setup ();

   seek (mirror, 1, 1);
   seek (mirror, 2, 2);
   seek (mirror, 3, 3);
   assert .equal (video .target, 1, "one seek runs at a time");

   // Seek 1 completes after 2 and 3 were asked for: its picture is obsolete and not sent; 3 starts, 2 never does.
   video .complete ();
   assert .deepEqual (answers, [ ]);
   assert .equal (video .target, 3);

   video .complete ();
   assert .deepEqual (answers .map (a => a .id), [ 3 ]);
});

test ("answers carry the ids of their requests in the order the seeks completed, never an older id after a newer one", () =>
{
   const { mirror, video, answers } = setup ();

   seek (mirror, 10, 0);
   video .complete ();
   seek (mirror, 11, 1);
   video .complete ();
   seek (mirror, 12, 2);
   video .complete ();

   const ids = answers .map (a => a .id);

   assert .deepEqual (ids, [ 10, 11, 12 ]);
   assert .deepEqual ([ ... ids ] .sort ((a, b) => a - b), ids);
});

test ("a seek to the time the element is at still answers", async () =>
{
   const { mirror, video, answers } = setup ();

   seek (mirror, 1, 0);
   // The emulated element started a seek; complete it, then ask for the same time again: no seek starts, `seeking` is false.
   video .complete ();
   Object .defineProperty (video, "currentTime", { get: () => 0, set: () => { }, configurable: true });
   seek (mirror, 2, 0);
   await Promise .resolve ();
   assert .deepEqual (answers .map (a => a .id), [ 1, 2 ]);
});

test ("a released element is not answered", () =>
{
   const { mirror, video, answers } = setup ();

   seek (mirror, 1, 1);
   mirror .apply ({ live: [ ], ops: [ ] });
   video .complete ();
   assert .deepEqual (answers, [ ]);
});

test ("pinned calls still play the element", () =>
{
   const { mirror, video } = setup ();

   mirror .apply ({ live: [ 7 ], ops: [ [ "play", 7 ] ] });
   assert .equal (video .played, 1);
});
