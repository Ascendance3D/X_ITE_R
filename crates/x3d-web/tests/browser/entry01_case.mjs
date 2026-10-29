// The entry point 01 cases (issue #138): X3D as a function (X3D (onfulfilled, onrejected)), X3D.noConflict,
// X3D.createBrowser, X3D.getBrowser and the element's captureStream.
// oracle/x_ite-web-runtime-02.mjs (--suite entry01) runs them against pinned X_ITE 16.5.0 and records the values;
// entry01.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).
//
// The unbundled X_ITE source of the pin never assigns window.X3D (its dist bundle does), so a case that needs the
// global assigns it itself, as the bundle would, and both engines run the same steps.

import { DESKTOP, settle } from "./runtime02_case.mjs";

const WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Interchange
DEF S Shape { geometry Box { } }
`)}`;

// The page helpers: the result of `fn`, or its error as `Name: message` (NaN and undefined as text, which the JSON of
// the recording would turn into null); `settled` waits for frames and a delay.
function pageHelpers ()
{
   const show = value => typeof value === "number" && !Number .isFinite (value) ? `number:${value}` : value === undefined ? "undefined" : value;

   window .show    = show;
   window .attempt = fn =>
   {
      try
      {
         return show (fn ());
      }
      catch (error)
      {
         return `${error ?.constructor ?.name}: ${error ?.message}`;
      }
   };

   // A rejection nobody handles is recorded here (and not left to fail the page).
   window .unhandled = [ ];
   window .addEventListener ("unhandledrejection", event =>
   {
      event .preventDefault ();
      window .unhandled .push (`${event .reason ?.constructor ?.name}: ${event .reason ?.message}`);
   });

   // The description of an element made by X3D.createBrowser.
   window .describe = element => ({
      nodeName: element .nodeName, localName: element .localName, tagName: element .tagName,
      isConnected: element .isConnected, hasParent: element .parentNode !== null,
      isHTMLElement: element instanceof HTMLElement, isCustomElement: element instanceof (customElements .get ("x3d-canvas")),
      toString: Object .prototype .toString .call (element),
      hasBrowser: element .browser !== undefined && element .browser !== null, browserType: typeof element .browser,
      getBrowserSame: window .X3D_NS .getBrowser (element) === element .browser,
      hasShadowRoot: element .shadowRoot !== null,
      ownsBrowser: Object .keys (element) .includes ("browser"),
   });

   window .waited = async (ms = 300) =>
   {
      for (let i = 0; i < 4; ++ i)
         await new Promise (requestAnimationFrame);

      await new Promise (resolve => setTimeout (resolve, ms));
   };

   // The world URL without the origin; the pages of the two engines differ, so "page" stands for their own page.
   window .world = browser =>
   {
      const url = browser .currentScene ?.worldURL ?? "";

      return /^(data:|about:)/ .test (url) ? url .slice (0, 40) : url .includes ("/worlds/") ? url .replace (/^.*\/worlds\//, "") : "page";
   };
}

async function ready (page)
{
   await page .waitForFunction (() => window .ready || window .startError, null, { timeout: 30000 });
   await page .evaluate (pageHelpers);
   await settle (page, 100);
}

// The page again with `init` run before any of its scripts (a value in window.X3D before the library loads).
async function reloadWith (page, init)
{
   await page .addInitScript (init);
   await page .reload ();
   await ready (page);
}

const cases = { };

// typeof X3D, its length, name and the other names the namespace object has.
cases .typeOf = async page =>
{
   await ready (page);

   return page .evaluate (() =>
   {
      const X3D = window .X3D_NS;

      return {
         typeof: typeof X3D, length: X3D .length, callable: typeof X3D === "function",
         isFunction: X3D instanceof Function, toString: Object .prototype .toString .call (X3D),
         string: String (X3D), tag: X3D [Symbol .toStringTag], ownToString: Object .hasOwn (X3D, "toString"),
         members: [ "noConflict", "createBrowser", "getBrowser", "X3DConstants", "X3DField", "X3DArrayField", "X3DFieldDefinition",
            "FieldDefinitionArray", "SFVec3f", "MFString" ] .map (name => [ name, typeof X3D [name] ]),
         lengths: [ "noConflict", "createBrowser", "getBrowser" ] .map (name => [ name, X3D [name] .length ]),
      };
   });
};

// X3D () is a promise that resolves, with and without callbacks; the onrejected path through a throwing onfulfilled.
cases .call = async page =>
{
   await ready (page);

   return page .evaluate (async () =>
   {
      const X3D = window .X3D_NS;
      const out = { order: [ ] };

      const a = X3D ();
      const b = X3D ();

      out .isPromise = a instanceof Promise;
      out .isThenable = typeof a .then;
      out .samePromise = a === b;
      out .value = show (await a);
      out .valueAgain = show (await b);

      const r1 = await X3D (value => { out .order .push ([ "fulfilled", show (value) ]); return "result"; },
                            error => { out .order .push ([ "rejected", error ?.message ]); return "recovered"; });

      out .withCallbacks = r1;

      // onfulfilled throws: onrejected gets the error and its return value is the result.
      out .rejectedPath = await X3D (() => { throw new Error ("boom"); }, error => { out .order .push ([ "rejected", error .message ]); return `recovered ${error .message}`; });

      // onfulfilled throws and there is no onrejected: the promise rejects.
      out .noOnrejected = await X3D (() => { throw new Error ("boom"); }) .then (v => `resolved ${show (v)}`, error => `rejected ${error .message}`);

      // onrejected only; non-function callbacks.
      out .onlyOnrejected = show (await X3D (undefined, () => out .order .push ("never")));
      out .nonFunctions = show (await X3D (5, "x"));
      out .nullCallbacks = show (await X3D (null, null));

      // A callback runs after the browser exists.
      out .browserInCallback = await X3D (() => document .querySelector ("x3d-canvas") .browser !== undefined);

      // The result of the chain: a callback's return value, and a rejection of the onrejected.
      out .chain = await X3D (() => 1) .then (v => v + 1);
      out .rejectedRethrown = await X3D (() => { throw new Error ("one"); }, error => { throw new Error (`two after ${error .message}`); })
         .then (v => `resolved ${v}`, error => `rejected ${error .message}`);

      out .unhandled = window .unhandled;

      return out;
   });
};

// X3D.noConflict when window.X3D was absent before the library loaded.
cases .noConflictAbsent = async page =>
{
   await ready (page);

   return page .evaluate (() =>
   {
      const X3D = window .X3D_NS;
      const out = { };

      // The bundle's assignment (X_ITE's dist assigns window.X3D, the unbundled source does not).
      window .X3D = X3D;
      out .assigned = window .X3D === X3D;

      const r1 = X3D .noConflict ();

      out .first = { returnsX3D: r1 === X3D, has: "X3D" in window, value: show (window .X3D) };

      const r2 = X3D .noConflict ();

      out .second = { returnsX3D: r2 === X3D, has: "X3D" in window, value: show (window .X3D) };

      // The library still works through the returned reference.
      out .stillWorks = typeof r1 .getBrowser () === "object";

      // A page value assigned later is left alone.
      window .X3D = "later";
      out .later = { returnsX3D: X3D .noConflict () === X3D, value: window .X3D };

      // The page assigned the library again after a noConflict; a call restores the state of the start again.
      window .X3D = X3D;
      X3D .noConflict ();
      out .again = { has: "X3D" in window };

      return out;
   });
};

// X3D.noConflict when window.X3D was present (an object) before the library loaded.
cases .noConflictPresent = async page =>
{
   await reloadWith (page, () => { window .prior = { prior: true }; window .X3D = window .prior; });

   return page .evaluate (() =>
   {
      const X3D = window .X3D_NS;
      const out = { };

      out .rawIsPrior = window .X3D === window .prior;
      out .rawHas     = "X3D" in window;
      out .rawIsLibrary = window .X3D === X3D;

      // The bundle's assignment over the prior value.
      window .X3D = X3D;

      const r1 = X3D .noConflict ();

      out .first = { returnsX3D: r1 === X3D, isPrior: window .X3D === window .prior, value: show (JSON .stringify (window .X3D)) };

      const r2 = X3D .noConflict ();

      out .second = { returnsX3D: r2 === X3D, isPrior: window .X3D === window .prior };

      // With the library not in window.X3D a call changes nothing.
      window .X3D = "page";
      out .notLibrary = { returnsX3D: X3D .noConflict () === X3D, value: window .X3D };

      window .X3D = X3D;
      X3D .noConflict ();
      out .restoredAgain = window .X3D === window .prior;

      return out;
   });
};

// window.X3D was present but undefined before load: the property exists, so noConflict restores undefined, not a delete.
cases .noConflictUndefined = async page =>
{
   await reloadWith (page, () => { window .X3D = undefined; });

   return page .evaluate (() =>
   {
      const X3D = window .X3D_NS;
      const out = { };

      window .X3D = X3D;

      out .returnsX3D = X3D .noConflict () === X3D;
      out .has        = "X3D" in window;
      out .value      = show (window .X3D);
      out .second     = { returnsX3D: X3D .noConflict () === X3D, has: "X3D" in window };

      return out;
   });
};

// Observers for the load events of an element, and the state of its browser.
function pageWatch (element)
{
   const events = [ ];

   for (const name of [ "initialized", "load", "error", "shutdown" ])
      element .addEventListener (name, () => events .push (name));

   return {
      events,
      state: () => ({ events: events .slice (), world: show (world (element .browser)), hasScene: typeof element .browser .currentScene }),
   };
}

// X3D.createBrowser () with no arguments.
cases .createBrowser = async page =>
{
   await ready (page);

   return page .evaluate (async ([ watch ]) =>
   {
      const X3D     = window .X3D_NS;
      const before  = document .querySelectorAll ("x3d-canvas") .length;
      const element = X3D .createBrowser ();
      const watched = new Function ("element", `return (${watch}) (element)`) (element);
      const out     = { };

      out .detached = describe (element);
      out .canvases = [ before, document .querySelectorAll ("x3d-canvas") .length ];
      out .twoElements = X3D .createBrowser () !== element;
      out .beforeAttach = watched .state ();

      await waited ();

      out .stillDetached = watched .state ();

      document .body .append (element);
      await waited (500);

      out .attached = describe (element);
      out .canvasesAttached = document .querySelectorAll ("x3d-canvas") .length;
      out .afterAttach = watched .state ();
      out .getBrowserDefault = X3D .getBrowser () === document .querySelector ("x3d-canvas") .browser;
      out .unhandled = window .unhandled;

      return out;
   }, [ pageWatch .toString () ]);
};

// A page function that creates the browser with `args` (given as source), watches it, attaches it and reports.
function pageCreateWith ([ watch, argsSource ])
{
   return (async () =>
   {
      const X3D     = window .X3D_NS;
      const args    = new Function ("X3D", `return (${argsSource})`) (X3D);
      const out     = { };
      let   element;

      try
      {
         element = X3D .createBrowser (... args);
      }
      catch (error)
      {
         return { threw: `${error ?.constructor ?.name}: ${error ?.message}` };
      }

      const watched = new Function ("element", `return (${watch}) (element)`) (element);

      out .threw = false;
      out .isElement = element instanceof HTMLElement && element .localName === "x3d-canvas" && !element .isConnected;

      await waited (500);

      out .beforeAttach = watched .state ();

      document .body .append (element);
      await waited (800);

      out .afterAttach = watched .state ();
      out .connected = element .isConnected;
      out .unhandled = window .unhandled;

      return out;
   }) ();
}

const withArgs = (argsSource) => async page =>
{
   await ready (page);

   return page .evaluate (pageCreateWith, [ pageWatch .toString (), argsSource ]);
};

const urlSource = url => `[ new X3D .MFString (${JSON .stringify (url)}) ]`;

// X3D.createBrowser (url): loadURL is called on the browser; what the world is after attaching and waiting.
cases .createBrowserUrl = withArgs (urlSource (WORLD));

// A URL that does not load (the page cannot reach other hosts, so the request fails), and one the server answers 404.
cases .createBrowserBadUrl = withArgs (urlSource ("http://127.0.0.1:1/missing.x3dv"));

// Arguments other than an MFString: loadURL's own checks.
cases .createBrowserArgs = async page =>
{
   await ready (page);

   const out = { };

   for (const [ name, source ] of [
      [ "string", `[ "${"data:model/x3d-vrml,"}#X3D V4.0 utf8" ]` ],
      [ "undefined", `[ undefined ]` ],
      [ "null", `[ null ]` ],
      [ "emptyMFString", `[ new X3D .MFString () ]` ],
      [ "withParameter", `[ new X3D .MFString (${JSON .stringify (WORLD)}), new X3D .MFString () ]` ],
   ])
   {
      out [name] = await page .evaluate (pageCreateWith, [ pageWatch .toString (), source ]);
      await page .evaluate (() => { window .unhandled .length = 0; });
   }

   return out;
};

// X3D.getBrowser with every kind of argument.
cases .getBrowser = async page =>
{
   await ready (page);

   return page .evaluate (() =>
   {
      const X3D    = window .X3D_NS;
      const canvas = document .querySelector ("x3d-canvas");
      const made   = X3D .createBrowser ();
      const check  = value => value === canvas .browser ? "page browser" : value === undefined ? "undefined" : value === made .browser ? "made browser" : "other";

      document .body .append (Object .assign (document .createElement ("div"), { id: "plain" }));

      return {
         none:        attempt (() => check (X3D .getBrowser ())),
         element:     attempt (() => check (X3D .getBrowser (canvas))),
         made:        attempt (() => check (X3D .getBrowser (made))),
         selector:    attempt (() => check (X3D .getBrowser ("x3d-canvas"))),
         upper:       attempt (() => check (X3D .getBrowser ("X3DCanvas"))),
         id:          attempt (() => check (X3D .getBrowser ("#plain"))),
         noMatch:     attempt (() => check (X3D .getBrowser ("#nothing"))),
         badSelector: attempt (() => check (X3D .getBrowser ("!!"))),
         empty:       attempt (() => check (X3D .getBrowser (""))),
         zero:        attempt (() => check (X3D .getBrowser (0))),
         null:        attempt (() => check (X3D .getBrowser (null))),
         undefined:   attempt (() => check (X3D .getBrowser (undefined))),
         body:        attempt (() => check (X3D .getBrowser (document .body))),
         number:      attempt (() => check (X3D .getBrowser (5))),
         object:      attempt (() => check (X3D .getBrowser ({ }))),
      };
   });
};

// The MediaStream captureStream returns.
function pageStream (stream)
{
   const tracks = stream .getTracks ();

   return {
      isMediaStream: stream instanceof MediaStream, active: stream .active,
      tracks: tracks .length, videoTracks: stream .getVideoTracks () .length, audioTracks: stream .getAudioTracks () .length,
      kinds: tracks .map (track => track .kind), readyState: tracks .map (track => track .readyState),
      enabled: tracks .map (track => track .enabled), muted: tracks .map (track => track .muted),
      trackIsCanvasCapture: tracks .map (track => track .constructor .name),
      frameRate: tracks .map (track => track .getSettings () .frameRate ?? "undefined"),
   };
}

// element.captureStream () on the page's canvas, with a frame rate of 0 and 30 and unusual arguments; toBlob and
// toDataURL exist.
cases .captureStream = async page =>
{
   await ready (page);

   return page .evaluate (async ([ describeStream ]) =>
   {
      const canvas   = document .querySelector ("x3d-canvas");
      const describe = new Function (`return ${describeStream}`) ();
      const out      = { };
      const kept     = [ ];

      out .types = { captureStream: typeof canvas .captureStream, toBlob: typeof canvas .toBlob, toDataURL: typeof canvas .toDataURL, length: canvas .captureStream .length };

      for (const [ name, args ] of [ [ "none", [ ] ], [ "zero", [ 0 ] ], [ "thirty", [ 30 ] ], [ "negative", [ -1 ] ],
                                      [ "string", [ "x" ] ], [ "nan", [ NaN ] ], [ "infinity", [ Infinity ] ] ])
      {
         try
         {
            const stream = canvas .captureStream (... args);

            kept .push (stream);
            out [name] = describe (stream);
         }
         catch (error)
         {
            out [name] = `${error ?.constructor ?.name}: ${error ?.message}`;
         }
      }

      out .toDataURL = attempt (() => canvas .toDataURL () .startsWith ("data:image/png;base64,"));
      out .toBlob = await new Promise (resolve => { try { canvas .toBlob (blob => resolve (blob === null ? "null" : blob .type)); } catch (error) { resolve (`${error ?.constructor ?.name}: ${error ?.message}`); } });

      for (const stream of kept)
         stream .getTracks () .forEach (track => track .stop ());

      return out;
   }, [ pageStream .toString () ]);
};

// captureStream (and toDataURL, toBlob) on elements whose browser is not ready: not attached, and made by
// document.createElement.
cases .captureStreamNotReady = async page =>
{
   await ready (page);

   return page .evaluate (async ([ describeStream ]) =>
   {
      const X3D      = window .X3D_NS;
      const describe = new Function (`return ${describeStream}`) ();
      const out      = { };

      const each = {
         createBrowser: X3D .createBrowser (),
         createElement: document .createElement ("x3d-canvas"),
      };

      for (const [ name, element ] of Object .entries (each))
      {
         const run = (fn) => attempt (() => { const value = fn (); return value === undefined ? "undefined" : value instanceof MediaStream ? describe (value) : typeof value === "string" ? value .slice (0, 22) : typeof value; });

         out [name] = {
            detached: {
               captureStream: run (() => element .captureStream ()),
               captureStream30: run (() => element .captureStream (30)),
               toDataURL: run (() => element .toDataURL ()),
               toBlob: run (() => element .toBlob (() => { })),
            },
         };
      }

      document .body .append (each .createBrowser);
      await waited (500);

      out .createBrowser .attached = {
         captureStream: attempt (() => { const s = each .createBrowser .captureStream (); return s instanceof MediaStream ? describe (s) : typeof s; }),
      };

      return out;
   }, [ pageStream .toString () ]);
};

export const CASE_GROUPS = {
   entryPoints: { context: DESKTOP, cases },
};
