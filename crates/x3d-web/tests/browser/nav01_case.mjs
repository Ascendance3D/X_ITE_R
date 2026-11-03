// The navigation touch 01 cases (issue #370, ledger sub:nav.*): synthetic touch and pointer scripts whose viewpoint pose is
// compared after every step. oracle/x_ite-web-runtime-02.mjs (--suite nav01) runs them against pinned X_ITE 16.5.0 and
// records the values; nav01.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).
//
// The tolerance is fixed before the first run: every number of a pose (position, orientation axis and angle) is equal
// within 1e-3 (runtime02_harness.mjs, NAV_TOLERANCE). The values are recorded unrounded.
//
// The input is Chrome DevTools touch events in a context with touch (the emulation of runtime02_case.mjs: no physical
// device, issue #143). The world is synthetic, written for the tests.

import { DESKTOP, fields, frames, settle, touch } from "./runtime02_case.mjs";
import { pageLoadURL } from "./loadurl01_case.mjs";

// A red box on the left, a green sphere on the right, a ProximitySensor that reports the camera. The canvas is 400 x 300
// and the field of view 0.785, so the box (x -3) is at the screen point [91, 150] and the sphere (x 3) at [308, 150].
const world = (type, viewpoint = "position 0 0 10") => `data:model/x3d-vrml,${encodeURIComponent (`#VRML V2.0 utf8
NavigationInfo { type [ "${type}" "ANY" ] }
DEF Camera ProximitySensor { size 1000 1000 1000 }
Viewpoint { ${viewpoint} }
Transform { translation -3 0 0 children Shape {
  appearance Appearance { material Material { diffuseColor 0.9 0.3 0.2 } } geometry Box { size 2 2 2 } } }
Transform { translation 3 0 0 children Shape {
  appearance Appearance { material Material { diffuseColor 0.2 0.8 0.3 } } geometry Sphere { radius 1 } } }
`)}`;

// The viewers chase a move or a zoom step for MOVE_TIME (0.2 s), and how far the chase is after a frame depends on the frame
// timing. Every step waits twice that, so each pose is a settled one and the same in every run.
const MOVE_SETTLE = 400;

const BOX    = [ 91, 150 ];
const EMPTY  = [ 340, 236 ];

async function load (page, url)
{
   await page .evaluate (pageLoadURL, [ url ]);

   const result = await page .evaluate (() => window .pending);

   if (result !== "resolved")
      throw new Error (`the world did not load: ${result}\n${page .log .join ("\n")}`);

   await settle (page, 200);
}

// The pose of the camera: position and orientation (axis and angle), as numbers.
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
      await settle (page, MOVE_SETTLE);
      poses .push (await pose (page));
   }

   if (start .length > 1)
      await touch (page, "touchEnd", start .slice (0, 1));

   await touch (page, "touchEnd", [ ]);
   await settle (page, 300);
   poses .push (await pose (page));
   return poses;
}

// Taps (touch start and end) at `points`, `gap` milliseconds apart; the pose after each, and after `wait` milliseconds.
async function taps (page, points, gap, wait)
{
   const out = { start: await pose (page), taps: [ ] };

   for (const [ i, point ] of points .entries ())
   {
      if (i)
         await page .waitForTimeout (gap);

      await touch (page, "touchStart", [ point ]);
      await touch (page, "touchEnd", [ ]);
      await frames (page, 2);

      // The pose after the first tap only: after the second the look-at runs, and its frames depend on the timing.
      if (!i)
         out .taps .push (await pose (page));
   }

   // Wait at least `wait` ms, then until the pose has stopped moving (a slower machine runs the look-at in more wall time).
   await page .waitForTimeout (wait);
   await frames (page, 2);

   let last = await pose (page);

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

const examine = {
   // Two fingers moving the same way (an angle between their moves above 0.7) move the viewpoint (button 1): X_ITE's
   // ExamineViewer.touchmove; then the other way back.
   async pan (page)
   {
      await load (page, world ("EXAMINE"));

      const out = { start: await pose (page) };

      const A = [ [ 170, 150 ], [ 230, 150 ] ];

      out .right = await gesture (page, A, line (A, [ [ 230, 190 ], [ 290, 190 ] ], 4));
      out .back  = await gesture (page, A, line (A, [ [ 140, 120 ], [ 200, 120 ] ], 3));
      return out;
   },

   // The first moves fix the mode (touchMode): fingers that start apart zoom, and stay a zoom when both then move the
   // same way; fingers that start moving the same way stay a move when they then turn toward each other.
   async panMode (page)
   {
      await load (page, world ("EXAMINE"));

      const out = { start: await pose (page) };
      const A   = [ [ 160, 150 ], [ 240, 150 ] ];
      const B   = [ [ 140, 150 ], [ 260, 150 ] ];

      out .zoomThenMove = await gesture (page, A, [ ... line (A, B, 2), ... line (B, [ [ 140, 190 ], [ 260, 190 ] ], 2) ]);
      out .moveThenZoom = await gesture (page, A, [ ... line (A, [ [ 160, 180 ], [ 240, 180 ] ], 2), ... line ([ [ 160, 180 ], [ 240, 180 ] ], [ [ 190, 180 ], [ 210, 180 ] ], 2) ]);
      return out;
   },

   // Two taps within 300 ms on the box: ExamineViewer.dblclick, lookAtBBox (the box is looked at, the center of
   // rotation follows). The pose after each tap and after the look-at.
   async doubleTap (page)
   {
      await load (page, world ("EXAMINE"));
      return taps (page, [ BOX, BOX ], 60, 2500);
   },

   // Two taps 450 ms apart are two single taps: no look-at.
   async slowTaps (page)
   {
      await load (page, world ("EXAMINE"));
      return taps (page, [ BOX, BOX ], 450, 1500);
   },

   // A double tap on empty space: lookAtBBox of nothing under the finger.
   async doubleTapEmpty (page)
   {
      await load (page, world ("EXAMINE"));
      return taps (page, [ EMPTY, EMPTY ], 60, 2500);
   },
};

export const CASE_GROUPS = {
   navExamine: { context: { ... DESKTOP, deviceScaleFactor: 2, hasTouch: true }, cases: examine },
};
