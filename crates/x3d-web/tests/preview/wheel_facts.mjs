// The "EXAMINE: drag turns the view, wheel zooms" check of the release smoke test (smoke.mjs). One wheel event moves an
// EXAMINE viewer by one step: the distance to the center of rotation times SCROLL_FACTOR, whatever the size of deltaY
// (pinned X_ITE 16.5.0, oracle/x_ite-pin/src/x_ite/Browser/Navigation/ExamineViewer.js: SLOW_SCROLL, SCROLL_FACTOR,
// wheel, zoom; X_ITE_R crates/x3d-viewer/src/nav.rs: scroll_factor, slow_scroll_user_agent). SCROLL_FACTOR is 1/120
// for a user agent that matches /Mac OS X|OculusBrowser/i (Safari, and Playwright WebKit on every OS) and 1/20 for the
// others. The check expected the 1/20 step for every browser.

const SLOW_SCROLL = /Mac OS X|OculusBrowser/i;

// The check passes when the zoom is at least this part of the expected step. 0.4 is the limit the check had before
// (an absolute 0.2 for the 1/20 step at the start distance 10, that is 0.2 / 0.5): the same tolerance, for each factor.
export const MIN_STEP_PART = 0.4;

/** X_ITE's ExamineViewer SCROLL_FACTOR for this user agent. */
export function scrollFactor (userAgent)
{
   return SLOW_SCROLL .test (String (userAgent ?? "")) ? 1 / 120 : 1 / 20;
}

/** `{ pass, detail }`: the distance to the center before and after one wheel event with deltaY < 0. X_ITE's zoom moves
 *  the viewer toward the center for deltaY < 0 (`if (deltaY < 0) this .addMove (translation .negate (), ...)`), so the
 *  distance must get smaller: a step away from the center fails. */
export function wheelZoomFacts (userAgent, before, after)
{
   const factor   = scrollFactor (userAgent);
   const expected = before * factor;
   const zoom     = before - after;
   const pass     = Number .isFinite (zoom) && expected > 0 && zoom > MIN_STEP_PART * expected;

   return { pass, detail: { factor: factor === 1 / 120 ? "1/120" : "1/20", expected, zoom, limit: MIN_STEP_PART * expected } };
}
