// The check of a split DevTools touch event (touchdelivery.mjs): a pure test, no browser.

import { test } from "node:test";
import assert from "node:assert/strict";
import { wholeDelivery } from "./touchdelivery.mjs";

const move = [ [ 140, 150 ], [ 260, 150 ] ];

test ("one event with both fingers is whole", () =>
{
   assert .ok (wholeDelivery ([ [ "touchmove", move ] ], "touchmove", move));
});

test ("two events, one finger each, are split (the failure that was measured)", () =>
{
   const log = [ [ "touchmove", [ [ 140, 150 ], [ 245, 150 ] ] ], [ "touchmove", [ [ 140, 150 ], [ 260, 150 ] ] ] ];

   assert .equal (wholeDelivery (log, "touchmove", move), false);
});

test ("a different number of touches is split", () =>
{
   assert .equal (wholeDelivery ([ [ "touchmove", [ [ 140, 150 ] ] ] ], "touchmove", move), false);
});

test ("events of another type are not judged", () =>
{
   assert .ok (wholeDelivery ([ [ "touchstart", [ [ 1, 2 ] ] ] ], "touchmove", move));
   assert .ok (wholeDelivery ([ ], "touchmove", move));
});

test ("half a pixel is the same pixel, a pixel is not", () =>
{
   assert .ok (wholeDelivery ([ [ "touchmove", [ [ 140.4, 150 ], [ 260, 149.6 ] ] ] ], "touchmove", move));
   assert .equal (wholeDelivery ([ [ "touchmove", [ [ 141, 150 ], [ 260, 150 ] ] ] ], "touchmove", move), false);
});
