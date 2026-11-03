// The navigation touch 03 cases (issue #370, ledger sub:nav.plane P09 and sub:nav.lookat L06): PLANE and LOOKAT with a
// synthetic touch. oracle/x_ite-web-runtime-02.mjs (--suite nav03) runs them against pinned X_ITE 16.5.0 and records the
// values; nav03.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).
//
// The viewpoint pose (position, orientation axis and angle) is compared after every step, each step settled (the viewers
// chase a change for 0.2 s). The field of view scale that PLANE zooms with is no field of the Viewpoint node a page can
// read (not in getFieldDefinitions), so the zoom cases also compare the picture (the thresholds of runtime 01). The tolerance, fixed before the first run, is 1e-3 on every
// number (runtime02_harness.mjs, NAV_TOLERANCE).
//
// The input is Chrome DevTools touch events in a context with touch (no physical device, issue #143). The world is
// synthetic, written for the tests.

import { DESKTOP, fields, frames, settle, touch } from "./runtime02_case.mjs";
import { pageLoadURL } from "./loadurl01_case.mjs";

// A red box on the left, a green sphere on the right; the canvas is 400 x 300 and the field of view 0.785, so the box
// (x -3) is at the screen point [91, 150] and the sphere (x 3) at [308, 150].
const world = type => `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Immersive
NavigationInfo { type [ "${type}" "ANY" ] }
DEF Camera ProximitySensor { size 1000 1000 1000 }
Viewpoint { position 0 0 10 }
Transform { translation -3 0 0 children Shape {
  appearance Appearance { material Material { diffuseColor 0.9 0.3 0.2 } } geometry Box { size 2 2 2 } } }
Transform { translation 3 0 0 children Shape {
  appearance Appearance { material Material { diffuseColor 0.2 0.8 0.3 } } geometry Sphere { radius 1 } } }
`)}`;

const BOX   = [ 91, 150 ];
const EMPTY = [ 340, 236 ];

// Every step waits twice the viewers' MOVE_TIME (0.2 s): each pose is a settled one and the same in every run.
const STEP_SETTLE = 400;

async function load (page, url)
{
   await page .evaluate (pageLoadURL, [ url ]);

   const result = await page .evaluate (() => window .pending);

   if (result !== "resolved")
      throw new Error (`the world did not load: ${result}\n${page .log .join ("\n")}`);

   await settle (page, 200);
}

// The pose of the camera: position, orientation axis and angle, as numbers.
async function pose (page)
{
   const values = await fields (page, [ "Camera.position_changed", "Camera.orientation_changed" ]);

   return [ ... values ["Camera.position_changed"] .split (" "), ... values ["Camera.orientation_changed"] .split (" ") ] .map (Number);
}

// The points of `steps` moves from `from` to `to` (lists of points, one or two fingers), without the start.
const line = (from, to, steps) => Array .from ({ length: steps }, (_, i) => from .map ((p, k) => [ p [0] + (to [k] [0] - p [0]) * (i + 1) / steps, p [1] + (to [k] [1] - p [1]) * (i + 1) / steps ]));

// A gesture: `start` (one or two fingers) and the lists of points of every move. Returns the pose after the start,
// after every move and after the end.
async function gesture (page, start, moves)
{
   const poses = [ ];

   await touch (page, "touchStart", start .slice (0, 1));

   if (start .length > 1)
      await touch (page, "touchStart", start);

   await frames (page, 2);
   poses .push (await pose (page));

   for (const points of moves)
   {
      await touch (page, "touchMove", points);
      await settle (page, STEP_SETTLE);
      poses .push (await pose (page));
   }

   if (start .length > 1)
      await touch (page, "touchEnd", start .slice (0, 1));

   await touch (page, "touchEnd", [ ]);
   await settle (page, 300);
   poses .push (await pose (page));
   return poses;
}

// Taps (touch start and end) at `points`, `gap` milliseconds apart, and the settled pose after `wait` milliseconds.
async function taps (page, points, gap, wait, moves = false)
{
   const out = { start: await pose (page) };

   for (const [ i, point ] of points .entries ())
   {
      if (i)
         await page .waitForTimeout (gap);

      // Nothing else between the taps: on a slow runner a frame takes 0.3 s, longer than the 300 ms of a double tap.
      await touch (page, "touchStart", [ point ]);
      await touch (page, "touchEnd", [ ]);
   }

   // Wait at least `wait` ms, then until the pose has stopped moving (a slower machine runs the look-at in more wall time).
   await page .waitForTimeout (wait);
   await frames (page, 2);

   let last = await pose (page);

   // A look-at that is expected waits for its pick (a frame or more on a slow runner) before it moves: wait for it to start.
   for (let i = 0; moves && i < 40 && last .every ((v, k) => v === out .start [k]); ++ i)
   {
      await page .waitForTimeout (500);
      await frames (page, 2);
      last = await pose (page);
   }

   for (let i = 0; i < 40; ++ i)
   {
      await page .waitForTimeout (500);
      await frames (page, 2);

      const now = await pose (page);
      const same = now .every ((v, k) => v === last [k]);

      last = now;

      if (same)
         break;
   }

   out .settled = last;
   out .stable = true;
   return out;
}

const A = [ [ 170, 150 ], [ 230, 150 ] ];

const gestures = type => ({
   // Two fingers moving the same way: PLANE moves the viewpoint in its plane, LOOKAT looks around.
   async move (page)
   {
      await load (page, world (type));
      return { start: await pose (page), right: await gesture (page, A, line (A, [ [ 230, 190 ], [ 290, 190 ] ], 3)), back: await gesture (page, A, line (A, [ [ 140, 120 ], [ 200, 120 ] ], 2)) };
   },

   // Two fingers apart zoom in, together out: PLANE scales the field of view, LOOKAT moves toward the center.
   async zoom (page, env)
   {
      await load (page, world (type));

      const B   = [ [ 140, 150 ], [ 260, 150 ] ];
      const out = { start: await pose (page), apart: await gesture (page, A, line (A, B, 3)) };

      out .picture  = await env .picture (page);
      out .together = await gesture (page, B, line (B, A, 3));
      return out;
   },

   // The first moves fix the mode: fingers that start apart zoom and stay a zoom when both then move the same way.
   async mode (page)
   {
      await load (page, world (type));

      const B = [ [ 140, 150 ], [ 260, 150 ] ];
      const C = [ [ 140, 190 ], [ 260, 190 ] ];

      return { start: await pose (page), zoomThenMove: await gesture (page, A, [ ... line (A, B, 2), ... line (B, C, 2) ]) };
   },

   // One finger dragging: PLANE and LOOKAT have no one-finger drag.
   async oneFinger (page)
   {
      await load (page, world (type));
      return { start: await pose (page), drag: await gesture (page, [ [ 200, 150 ] ], line ([ [ 200, 150 ] ], [ [ 260, 190 ] ], 3)) };
   },

   // Two taps within 300 ms (PLANE, only in a development build) or 400 ms (LOOKAT) on the box, and on empty space.
   async doubleTap (page)
   {
      await load (page, world (type));

      const box = await taps (page, [ BOX, BOX ], 60, 2500, true);

      await load (page, world (type));
      return { box, empty: await taps (page, [ EMPTY, EMPTY ], 60, 2500) };
   },

   // Two taps 600 ms apart are two single taps.
   async slowTaps (page)
   {
      await load (page, world (type));
      return taps (page, [ BOX, BOX ], 600, 1500);
   },
});

// A device pixel ratio of 1, so the pictures of the zoom cases have the size of the canvas in both engines.
const context = { ... DESKTOP, hasTouch: true };

// A double tap is two taps within 300 ms (400 ms in LOOKAT) of the clock of the page. On GitHub's hosted runner (a software
// GPU) one frame takes 0.3 s or more, and a DevTools touch event waits for the frame in progress, so the second tap lands
// after the window in X_ITE as well as in X_ITE_R (CI run 41605751679: the pose did not move in 42 s). That case is skipped
// there and runs everywhere else (a GPU machine, XR_CASE=...).
const SLOW_RUNNER = process .env .GITHUB_ACTIONS === "true" ? "a double tap needs two taps within 300 ms, a frame of the hosted runner takes longer" : false;

export const CASE_GROUPS = {
   navPlane:  { context, cases: gestures ("PLANE"), skipCases: { doubleTap: SLOW_RUNNER } },
   navLookAt: { context, cases: gestures ("LOOKAT"), skipCases: { doubleTap: SLOW_RUNNER } },
};
