// A DevTools touch event with two or more points is one `touchstart` or `touchmove` of the page. Under load (a slow software GPU
// on a busy runner) Chrome has delivered it as two DOM events, 28 ms apart, one for each finger: the first has the new position of
// one finger and the old position of the other (measured, issue #370 follow-up: `navPlane.mode` and `navLookAt.mode` failed
// when the first move of a two-finger gesture arrived split, because the viewers fix the mode from the first move of both
// fingers). A case whose input did not arrive as the case says is not a test of the viewer: the harness runs it again.

export const TOUCH_ATTEMPTS = 4;

const SAME_PIXEL = 0.5;

// `events`: [ type, [ [ x, y ], ... ] ] as the page saw them, in order. True when every event of `type` has the points of the
// request, in order. No event of the type: true (nothing arrived, nothing was split).
export function wholeDelivery (events, type, points)
{
   return events
      .filter (([ eventType ]) => eventType === type)
      .every (([ , touches ]) => touches .length === points .length && touches .every (([ x, y ], i) => Math .abs (x - points [i] [0]) <= SAME_PIXEL && Math .abs (y - points [i] [1]) <= SAME_PIXEL));
}

// The page side of the log: one entry per touchstart, touchmove, touchend and touchcancel event; made once per document.
export function installTouchLog ()
{
   if (window .touchLog)
      return window .touchLog .length;

   window .touchLog = [ ];

   for (const type of [ "touchstart", "touchmove", "touchend", "touchcancel" ])
      window .addEventListener (type, event => window .touchLog .push ([ type, [ ... event .touches ] .map (t => [ t .pageX, t .pageY ]) ]), { capture: true, passive: true });

   return 0;
}
