// Tests of wheel_facts.mjs (no browser): node --test tests/preview/wheel_facts.test.mjs
import test from "node:test";
import assert from "node:assert/strict";
import { scrollFactor, wheelZoomFacts, MIN_STEP_PART } from "./wheel_facts.mjs";

// Playwright WebKit 26.5 on Linux, as the page reported it in the canary 0.2.0 acceptance run (smoke.mjs facts.userAgent):
// Playwright WebKit sends a macOS user agent on every OS.
const webkit   = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.5 Safari/605.1.15";
const chromium = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/151.0.0.0 Safari/537.36";
const firefox  = "Mozilla/5.0 (X11; Linux x86_64; rv:153.0) Gecko/20100101 Firefox/153.0";
const quest    = "Mozilla/5.0 (X11; Linux x86_64; Quest 3) AppleWebKit/537.36 (KHTML, like Gecko) OculusBrowser/40.0 Chrome/150.0 Safari/537.36";

test ("SCROLL_FACTOR follows X_ITE's SLOW_SCROLL user agents", () =>
{
   assert .equal (scrollFactor (webkit), 1 / 120);
   assert .equal (scrollFactor (quest), 1 / 120);
   assert .equal (scrollFactor (chromium), 1 / 20);
   assert .equal (scrollFactor (firefox), 1 / 20);
});

test ("the limit for the 1/20 step at distance 10 is the earlier 0.2", () =>
{
   assert .ok (Math .abs (wheelZoomFacts (chromium, 10, 9.5) .detail .limit - 0.2) < 1e-12);
   assert .equal (MIN_STEP_PART, 0.4);
});

test ("the WebKit step measured in the canary acceptance (10 -> 9.917) passes", () =>
{
   assert .equal (wheelZoomFacts (webkit, 10, 10 - 10 / 120) .pass, true);
});

test ("the WebKit limit is 0.4 of its step, on both sides", () =>
{
   assert .equal (wheelZoomFacts (webkit, 10, 10 - 0.39 * 10 / 120) .pass, false);
   assert .equal (wheelZoomFacts (webkit, 10, 10 - 0.41 * 10 / 120) .pass, true);
});

test ("a step away from the center fails (deltaY < 0 zooms in)", () =>
{
   assert .equal (wheelZoomFacts (webkit, 10, 10 + 10 / 120) .pass, false);
   assert .equal (wheelZoomFacts (chromium, 10, 10 + 10 / 20) .pass, false);
});

test ("the 1/120 step does not pass for a 1/20 browser", () =>
{
   assert .equal (wheelZoomFacts (chromium, 10, 10 - 10 / 120) .pass, false);
});

test ("no zoom fails for every user agent", () =>
{
   for (const ua of [ webkit, chromium, firefox, quest ])
      assert .equal (wheelZoomFacts (ua, 10, 10) .pass, false);
});

test ("a missing position fails", () =>
{
   assert .equal (wheelZoomFacts (chromium, 10, NaN) .pass, false);
   assert .equal (wheelZoomFacts (chromium, 0, 0) .pass, false);
});
