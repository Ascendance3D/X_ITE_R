// Touch snapshot 01 test of X_ITE_R's <x3d-canvas> that X_ITE has no reference for (issue #370, CI run 41620751198): a
// two-finger gesture reaches the viewers as the one snapshot of a `touchmove`, as in X_ITE, not as the pointermoves of the
// fingers one by one.
//
// X_ITE's viewers listen for `touchmove`, whose `touches` hold every finger. The page glue also hears one pointermove per
// finger and flushes once per frame. On the hosted runner's SwiftShader (a frame takes 0.3 s or more) the PLANE zoom
// case once saw the first finger of the last step moved and the second not (Rust x = -0.003346939 where the pinned x is 0,
// which is exactly a pinch of fingers at 140 and 250 instead of 140 and 260), and kept that offset. The case sends the
// pointermove of the first finger alone (a synthetic PointerEvent, the delivery the runner is suspected of), lets
// frames pass, and only then sends the real touchmove with both fingers: the pose must not move before it, and the
// zoom must end where a pinch with both fingers moving ends (no x offset).

import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { CASE_TIMEOUT, DESKTOP, ONLY, openPage } from "./runtime02_harness.mjs";
import { fields, frames, touch } from "./runtime02_case.mjs";

// Issue #472. Chromium delivers a DevTools touch event with two points as one `touchmove`, or, now and then, as two DOM
// events 28 ms apart (touchdelivery.mjs): the first has the new position of one finger and the old of the other. The
// page reads each `touchmove` whole, so a frame between the two halves drew the half step of the pinch: the last step
// of this test ended with the fingers at 140 and 250 for one frame, and PLANE zoomed at the center 195 and then at 200,
// which left x = -0.003346939 (reproduced by sending the halves as two events with frames between). The hosted runner's
// SwiftShader draws a frame in 0.3 s, so a 28 ms gap holds a frame boundary about one run in ten. This is the delivery of
// the browser, not the gesture the test is about (X_ITE, which handles each DOM event at once, would see the same
// half step). The test holds the page's animation frames while it sends a two-finger event, until the page has received a
// DOM touch event with every finger at its new position, so that the page draws no frame between the halves and a frame
// sees the whole step. HOLD_LIMIT only bounds the wait: a step that never arrives whole fails the test.
const HOLD_LIMIT = 5000;

// Sends a two-finger event with the page's frames held. `requestAnimationFrame` is wrapped for the hold only and restored
// after it; a callback asked for during the hold runs at the first frame after it.
async function holdFrames (page, type, points, send)
{
   await page .evaluate (([ domType ]) =>
   {
      const real  = window .requestAnimationFrame;
      const state = { held: true, touches: null };

      state .listener = event => { state .touches = Array .from (event .targetTouches, t => [ t .clientX, t .clientY ]); };
      state .restore  = () =>
      {
         state .held = false;
         window .requestAnimationFrame = real;
         removeEventListener (domType, state .listener, { capture: true });
      };

      addEventListener (domType, state .listener, { capture: true });
      window .requestAnimationFrame = callback => real .call (window, function again (stamp) { state .held ? real .call (window, again) : callback (stamp); });
      window .frameHold = state;
   }, [ type === "touchStart" ? "touchstart" : "touchmove" ]);

   try
   {
      await send ();

      // The whole step has come when a DOM event has every finger at its new position (CDP x, y are client pixels).
      await page .waitForFunction (points =>
      {
         const touches = window .frameHold .touches;

         return touches ?.length === points .length &&
            points .every (([ x, y ]) => touches .some (([ tx, ty ]) => Math .abs (tx - x) <= 0.5 && Math .abs (ty - y) <= 0.5));
      }, points, { timeout: HOLD_LIMIT });
   }
   finally
   {
      await page .evaluate (() => { window .frameHold .restore (); delete window .frameHold; });
   }
}

// `touch` for the events with two points: the page draws no frame until the whole event has come.
const touch2 = (page, type, points) => holdFrames (page, type, points, async () =>
{
   const cdp = page .cdp ??= await page .context () .newCDPSession (page);

   await cdp .send ("Input.dispatchTouchEvent",
   {
      type,
      touchPoints: points .map (([ x, y ], id) => ({ x, y, id, radiusX: 1, radiusY: 1, force: 1 })),
   });
});

const WORLD = `#X3D V4.0 utf8
PROFILE Immersive
NavigationInfo { type [ "PLANE" "ANY" ] }
DEF Camera ProximitySensor { size 1000 1000 1000 }
Viewpoint { position 0 0 10 }
Transform { translation -3 0 0 children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.3 0.2 } } geometry Box { size 2 2 2 } } }
`;

// The pose of the camera as numbers.
async function position (page)
{
   const values = await fields (page, [ "Camera.position_changed" ]);

   return values ["Camera.position_changed"] .split (" ") .map (Number);
}

describe ("touch snapshot 01", { skip: Boolean (ONLY), timeout: CASE_TIMEOUT }, () =>
{
   test ("a pointermove of one finger is not a move of the gesture, the touchmove with both fingers is", async () =>
   {
      const page = await openPage ({ ... DESKTOP, hasTouch: true });

      try
      {
         await page .evaluate (async text =>
         {
            const browser = document .querySelector ("x3d-canvas") .browser;

            await browser .replaceWorld (await browser .createX3DFromString (text));

            // The pointer ids Chromium gave the fingers, in the order they went down.
            window .ids = [ ];
            addEventListener ("pointerdown", event => window .ids .push (event .pointerId), { capture: true });
         }, WORLD);

         await frames (page, 3);

         const A = [ [ 170, 150 ], [ 230, 150 ] ];

         await touch (page, "touchStart", A .slice (0, 1));
         await touch2 (page, "touchStart", A);
         await frames (page, 2);

         // Two full steps of the pinch fix the zoom mode: fingers at 160/240, then 150/250.
         await touch2 (page, "touchMove", [ [ 160, 150 ], [ 240, 150 ] ]);
         await frames (page, 3);
         await touch2 (page, "touchMove", [ [ 150, 150 ], [ 250, 150 ] ]);
         await frames (page, 3);
         assert .deepEqual (await position (page), [ 0, 0, 10 ], "a centered pinch does not move the viewpoint");

         // The first finger alone, as a pointermove of the canvas, and some frames before the touchmove of the step.
         const target = await page .evaluate (() =>
         {
            const canvas = document .querySelector ("x3d-canvas") .shadowRoot .querySelector ("canvas");

            canvas .dispatchEvent (new PointerEvent ("pointermove",
            {
               pointerId: window .ids [0], pointerType: "touch", isPrimary: false, bubbles: true,
               clientX: 140, clientY: 150,
            }));

            return window .ids .length;
         });

         assert .ok (target >= 2, "two pointerdowns were seen");
         await frames (page, 4);
         assert .deepEqual (await position (page), [ 0, 0, 10 ], "the first finger alone has not moved the viewpoint");

         await touch2 (page, "touchMove", [ [ 140, 150 ], [ 260, 150 ] ]);
         await frames (page, 4);
         assert .deepEqual (await position (page), [ 0, 0, 10 ], "the pinch ends where a pinch with both fingers moving ends");

         await touch (page, "touchEnd", [ [ 140, 150 ] ]);
         await touch (page, "touchEnd", [ ]);
      }
      finally
      {
         await page .context () .close ();
      }
   });
});
