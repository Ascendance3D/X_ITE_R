// The navigation press 04 cases (issue #370, ledger sub:nav.examine E16): a press and a wheel step outside the viewport of
// the layer are ignored by the viewers (X3DViewer.isPointerInRectangle; ExamineViewer.mousedown and wheel).
// oracle/x_ite-web-runtime-02.mjs (--suite nav04) runs the cases against pinned X_ITE 16.5.0 and records the values;
// nav04.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).
//
// The viewpoint pose (position, orientation axis and angle) is compared after every step, each step settled. The tolerance,
// fixed before the first run, is 1e-3 on every number (runtime02_harness.mjs, NAV_TOLERANCE).
//
// The pointer is Playwright's mouse; the touch is Chrome DevTools touch events in a context with touch (no physical
// device, issue #143). The world is synthetic, written for the tests.

import { DESKTOP, fields, frames, settle, touch } from "./runtime02_case.mjs";
import { pageLoadURL } from "./loadurl01_case.mjs";

// One layer whose Viewport covers the middle of the canvas: x 100 to 300 and y 75 to 225 of the 400 x 300 canvas (clipBoundary
// 0.25 0.75 0.25 0.75, a quarter of each side clipped).
const WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Full
COMPONENT Layering:2
LayerSet {
  order [ 1 ]
  activeLayer 1
  layers Layer {
    viewport Viewport { clipBoundary [ 0.25 0.75 0.25 0.75 ] }
    children [
      NavigationInfo { type [ "EXAMINE" "ANY" ] }
      DEF Camera ProximitySensor { size 1000 1000 1000 }
      Viewpoint { position 0 0 10 }
      Shape { appearance Appearance { material Material { diffuseColor 0.9 0.3 0.2 } } geometry Box { size 2 2 2 } }
    ]
  }
}
`)}`;

const INSIDE  = [ 150, 150 ];
const OUTSIDE = [ 350, 150 ];
const ABOVE   = [ 150, 40 ];

// Every step waits twice the viewers' MOVE_TIME (0.2 s): each pose is a settled one and the same in every run.
const STEP_SETTLE = 400;

async function load (page)
{
   await page .evaluate (pageLoadURL, [ WORLD ]);

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

// A mouse drag with the left button from `from` by `by`: the pose after the press, after the move and after the release.
async function drag (page, from, by)
{
   const poses = [ ];

   await page .mouse .move (... from, { steps: 2 });
   await page .mouse .down ();
   await settle (page, STEP_SETTLE);
   poses .push (await pose (page));
   await page .mouse .move (from [0] + by [0], from [1] + by [1], { steps: 5 });
   await settle (page, STEP_SETTLE);
   poses .push (await pose (page));
   await page .mouse .up ();
   await settle (page, STEP_SETTLE);
   poses .push (await pose (page));
   return poses;
}

// Three wheel steps at `at`: the pose after each.
async function wheel (page, at)
{
   const poses = [ ];

   await page .mouse .move (... at, { steps: 2 });

   for (let i = 0; i < 3; ++ i)
   {
      await page .mouse .wheel (0, -100);
      await frames (page, 2);
      await settle (page, STEP_SETTLE);
      poses .push (await pose (page));
   }

   return poses;
}

// One finger (`start`) dragged by `by`, or two fingers (start and the second 60 px right) pinched apart: poses after each step.
async function fingers (page, start, by, two = false)
{
   const poses = [ ];
   const a     = start;
   const b     = [ start [0] + 60, start [1] ];

   await touch (page, "touchStart", [ a ]);

   if (two)
      await touch (page, "touchStart", [ a, b ]);

   await frames (page, 2);
   poses .push (await pose (page));

   for (let i = 1; i <= 4; ++ i)
   {
      const d = [ by [0] * i / 4, by [1] * i / 4 ];

      await touch (page, "touchMove", two ? [ [ a [0] - d [0], a [1] ], [ b [0] + d [0], b [1] ] ] : [ [ a [0] + d [0], a [1] + d [1] ] ]);
      await settle (page, STEP_SETTLE);
      poses .push (await pose (page));
   }

   if (two)
      await touch (page, "touchEnd", [ a ]);

   await touch (page, "touchEnd", [ ]);
   await settle (page, STEP_SETTLE);
   poses .push (await pose (page));
   return poses;
}

const cases = {
   // A press inside the viewport rotates; a press outside, or above it, does not, and neither does the rest of that drag.
   async mouseDrag (page)
   {
      await load (page);

      const out = { start: await pose (page) };

      out .outside = await drag (page, OUTSIDE, [ -60, 30 ]);
      out .above   = await drag (page, ABOVE, [ 40, 60 ]);
      out .inside  = await drag (page, INSIDE, [ 40, -20 ]);
      return out;
   },

   // A press inside that moves outside the viewport keeps rotating.
   async mouseLeaves (page)
   {
      await load (page);
      return { start: await pose (page), inside: await drag (page, INSIDE, [ 200, 20 ]) };
   },

   // A wheel step outside the viewport does not zoom; inside it does.
   async mouseWheel (page)
   {
      await load (page);

      const out = { start: await pose (page) };

      out .outside = await wheel (page, OUTSIDE);
      out .inside  = await wheel (page, INSIDE);
      return out;
   },

   // A finger down outside the viewport rotates nothing; a finger inside rotates.
   async touchDrag (page)
   {
      await load (page);

      const out = { start: await pose (page) };

      out .outside = await fingers (page, OUTSIDE, [ -60, 30 ]);
      out .inside  = await fingers (page, INSIDE, [ 40, -20 ]);
      return out;
   },

   // Two fingers pinching outside the viewport (their middle is outside) zoom nothing; inside they zoom.
   async touchPinch (page)
   {
      await load (page);

      const out = { start: await pose (page) };

      out .outside = await fingers (page, [ 290, 150 ], [ 20, 0 ], true);
      out .inside  = await fingers (page, [ 120, 150 ], [ 20, 0 ], true);
      return out;
   },
};

export const CASE_GROUPS = {
   navPress: { context: { ... DESKTOP, deviceScaleFactor: 2, hasTouch: true }, cases },
};
