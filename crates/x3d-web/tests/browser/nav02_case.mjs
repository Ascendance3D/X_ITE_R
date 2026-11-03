// The navigation touch 02 cases (issue #370, ledger sub:nav.walk-fly, F16): WALK and FLY with a synthetic touch.
// oracle/x_ite-web-runtime-02.mjs (--suite nav02) runs them against pinned X_ITE 16.5.0 and records the values;
// nav02.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).
//
// One finger held away from where it went down walks or flies (X3DFlyViewer.touchstart, touchmove); two fingers look around.
// Walking and flying integrate over the real frame times (Date .now () in X3DFlyViewer.fly), so the pose after a hold
// depends on the frame timing. The cases therefore record the speed of the hold, the displacement or the angle divided
// by the seconds between the touch start and the touch end (a key that ends in `Rate`), and the direction; the tolerances,
// fixed before the first run, are in runtime02_harness.mjs: 5% relative for a Rate, 1e-3 absolute for every other number.
// The look-around is event driven (an orientation chaser): its pose, settled, is compared at 1e-3.
//
// The input is Chrome DevTools touch events in a context with touch (no physical device, issue #143). The world is
// synthetic, written for the tests.

import { DESKTOP, fields, frames, settle, touch } from "./runtime02_case.mjs";
import { pageLoadURL } from "./loadurl01_case.mjs";

// WALK has gravity: a ground whose top is the avatar height (1.6, avatarSize) below the viewpoint keeps it level.
const world = type => `data:model/x3d-vrml,${encodeURIComponent (`#VRML V2.0 utf8
NavigationInfo { type [ "${type}" "ANY" ] }
DEF Camera ProximitySensor { size 100000 100000 100000 }
Viewpoint { position 0 0 10 }
Transform { translation 0 -2.1 0 children Shape { geometry Box { size 1000 1 1000 } } }
Transform { translation -3 0 0 children Shape {
  appearance Appearance { material Material { diffuseColor 0.9 0.3 0.2 } } geometry Box { size 2 2 2 } } }
Transform { translation 3 0 0 children Shape {
  appearance Appearance { material Material { diffuseColor 0.2 0.8 0.3 } } geometry Sphere { radius 1 } } }
`)}`;

const CENTER = [ 200, 150 ];
const HOLD   = 1500;

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

const norm = v => Math .hypot (... v);

// One finger down at `from`, moved to `to`, held for HOLD ms, lifted. The speeds of the hold.
async function hold (page, type, from, to, curved = false)
{
   await load (page, world (type));

   const start = await pose (page);

   await touch (page, "touchStart", [ from ]);
   await frames (page, 2);

   const t0 = Date .now ();

   await touch (page, "touchMove", [ to ]);
   await page .waitForTimeout (HOLD);
   await touch (page, "touchEnd", [ ]);

   const seconds = (Date .now () - t0) / 1000;

   await frames (page, 3);

   const end   = await pose (page);
   const moved = [ 0, 1, 2 ] .map (i => end [i] - start [i]);
   const angle = end [6] - start [6];
   const out   = { start };

   // A path curved by a turn in progress (`curved`) has the direction of its net displacement in a `Dir` key (looser).
   out [curved ? "travelDir" : "travel"] = norm (moved) < 1e-6 ? [ 0, 0, 0 ] : moved .map (v => v / norm (moved));
   out .speedRate   = norm (moved) / seconds;
   out .axis        = end .slice (3, 6);
   out .angleRate   = Math .abs (angle) / seconds;
   out .holdOk      = seconds > 1.2 && seconds < 3;

   // Nothing moves after the touch ends.
   await page .waitForTimeout (400);
   out .stopped = (await pose (page)) .every ((v, i) => v === end [i]);
   return out;
}

// Two fingers from A to B (the middle moves by the same amount): the settled orientation.
async function lookAround (page, type)
{
   await load (page, world (type));

   const out = { start: await pose (page) };
   const A   = [ [ 170, 150 ], [ 230, 150 ] ];
   const steps = [ [ [ 180, 155 ], [ 240, 155 ] ], [ [ 200, 165 ], [ 260, 165 ] ], [ [ 220, 160 ], [ 280, 160 ] ] ];

   out .steps = [ ];

   await touch (page, "touchStart", A .slice (0, 1));
   await touch (page, "touchStart", A);
   await frames (page, 2);

   for (const points of steps)
   {
      await touch (page, "touchMove", points);
      await settle (page, 400);
      out .steps .push (await pose (page));
   }

   await touch (page, "touchEnd", A .slice (0, 1));
   await touch (page, "touchEnd", [ ]);
   await settle (page, 400);
   out .end = await pose (page);
   return out;
}

const cases = {
   // A finger up from where it went down walks forward; sideways turns and does not walk.
   walkForward: page => hold (page, "WALK", CENTER, [ 200, 100 ]),
   walkBack:    page => hold (page, "WALK", CENTER, [ 200, 190 ]),
   walkTurn:    page => hold (page, "WALK", CENTER, [ 280, 150 ]),
   flyForward:  page => hold (page, "FLY", CENTER, [ 200, 100 ]),
   flyTurn:     page => hold (page, "FLY", CENTER, [ 280, 150 ]),
   // A diagonal move walks and turns.
   walkDiagonal: page => hold (page, "WALK", CENTER, [ 250, 100 ], true),
   // Two fingers look around, in WALK and in FLY.
   lookAroundWalk: page => lookAround (page, "WALK"),
   lookAroundFly:  page => lookAround (page, "FLY"),
};

export const CASE_GROUPS = {
   navWalkFly: { context: { ... DESKTOP, deviceScaleFactor: 2, hasTouch: true }, cases },
};
