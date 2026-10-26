// The browser runtime 02 cases: input, resource loading and the scene API. oracle/x_ite-web-runtime-02.mjs runs
// them against pinned X_ITE 16.5.0 and records the values; runtime02_harness.mjs runs them against X_ITE_R and
// compares. Each case is a Node function of a Playwright page and an environment:
//
//   env .base       the server origin of the page; env .foreign another origin of the same server
//   env .worlds     the URL of tests/browser/worlds/ on the server
//   env .server     the controlled server (server.mjs): release (key), arm (key), requests
//   env .picture    async (page) => PNG Buffer of what the browser draws
//
// The input is real: Playwright's mouse, wheel and keyboard, and Chrome DevTools touch events. The values are
// plain JSON without origins, so the two engines compare. Numbers are rounded where float32 fields print them.

// --- page helpers (they run in the page) ------------------------------------------------------------------------

// Load `url` with browser.loadURL and wait for its "load". Returns the browser events of the load.
async function pageLoad (url)
{
   const canvas  = document .querySelector ("x3d-canvas");
   const browser = canvas .browser;
   const events  = [ ];
   const names   = [ "initialized", "load", "error", "shutdown" ];
   const record  = event => events .push (event .type);

   names .forEach (name => canvas .addEventListener (name, record));

   try
   {
      await browser .loadURL (new window .X3D_NS .MFString (url));
   }
   finally
   {
      names .forEach (name => canvas .removeEventListener (name, record));
   }

   return events;
}

// Field values of named nodes: { "Node.field": String (value) }. A missing node or field gives its error message.
function pageFields (list)
{
   const scene = document .querySelector ("x3d-canvas") .browser .currentScene;
   const out   = { };

   for (const item of list)
   {
      const [ name, field ] = item .split (".");

      try
      {
         const value = scene .getNamedNode (name) [field];

         out [item] = value === null || value === undefined ? String (value) : typeof value === "object" ? String (value) : value;
      }
      catch (error)
      {
         out [item] = `error: ${error .message}`;
      }
   }

   return out;
}

// The cursor of the browser's surface (X_ITE: the surface element; X_ITE_R: the canvas).
function pageCursor ()
{
   const browser = document .querySelector ("x3d-canvas") .browser;

   return (browser .getSurface ?.() ?? browser .getCanvas ()) .style .cursor;
}

// `n` animation frames.
async function pageFrames (n)
{
   for (let i = 0; i < n; ++ i)
      await new Promise (requestAnimationFrame);
}

// --- node helpers -----------------------------------------------------------------------------------------------

const frames = (page, n = 3) => page .evaluate (pageFrames, n);
const settle = async (page, ms = 400) => { await frames (page); await page .waitForTimeout (ms); await frames (page); };
const fields = (page, list) => page .evaluate (pageFields, list);
const cursor = page => page .evaluate (pageCursor);

// "1.23456 -0 7" → "1.2346 0 7": rounds every number of a field's text to `digits` decimals.
export function roundText (text, digits = 4)
{
   if (typeof text !== "string")
      return text;

   return text .replace (/-?\d+(?:\.\d+)?(?:e[-+]?\d+)?/gi, n =>
   {
      const r = Number (Number (n) .toFixed (digits));

      return String (Object .is (r, -0) ? 0 : r);
   });
}

const rounded = (values, digits) => Object .fromEntries (Object .entries (values) .map (([ k, v ]) => [ k, roundText (v, digits) ]));

// Console lines without the origins, and only X3D load messages (not the engines' own start-up lines). Chrome's own
// "Failed to load resource" lines come first, then the engine's lines, sorted: the order of the messages of two
// nodes is the order their responses arrive in, not the engine's (X_ITE sends its texture requests later than its
// Inline requests; X_ITE_R sends them together). The order of one node's messages is tested natively
// (tests/async_loading.rs).
function loadMessages (log, env)
{
   const lines = log
      .map (line => line .replaceAll (env .base, "http://test") .replaceAll (env .foreign, "http://foreign") .replace (/blob:http:\/\/test\/[-0-9a-f]+/g, "blob:http://test/ID"))
      .filter (line => /Couldn't load|Failed to fetch|Not Found|Internal Server Error|Couldn't parse|Couldn't decode|Invalid|EXTERNPROTO|Error:/ .test (line));
   const network = line => line .startsWith ("error: Failed to load resource:");

   return [ ... lines .filter (network), ... lines .filter (line => !network (line)) .sort () ];
}

// The paths the server served since `from`, without the engine's own files. A request to the other origin is marked.
function served (env, from)
{
   const foreign = new URL (env .foreign) .host;

   return env .server .requests .slice (from)
      .map (r => r .host === foreign ? `foreign:${r .path}` : r .path)
      .filter (path => path .includes ("/worlds/") || path .startsWith ("/hold/") || path .startsWith ("/status/") || path .startsWith ("/garbage/"));
}

async function touch (page, type, points)
{
   const cdp = page .cdp ??= await page .context () .newCDPSession (page);

   await cdp .send ("Input.dispatchTouchEvent", {
      type,
      touchPoints: points .map (([ x, y ], id) => ({ x, y, id, radiusX: 1, radiusY: 1, force: 1 })),
   });
}

// ExamineViewer's MOVE_TIME: the PositionChaser of a zoom step reaches its destination in 0.2 s. A zoom step is a
// fraction of the distance to the center of rotation *now*, so a step during the chase starts from a position that
// depends on frame timing. The zoom cases wait twice MOVE_TIME after each step: every step starts from a settled
// position and the end position is the same in every run.
const ZOOM_SETTLE = 400;

// A one-finger or two-finger touch gesture from `from` to `to` (lists of points) in `steps` moves, with `pause`
// milliseconds after each move.
async function gesture (page, from, to, steps = 6, pause = 0)
{
   await touch (page, "touchStart", from .slice (0, 1));

   if (from .length > 1)
      await touch (page, "touchStart", from);

   await frames (page, 1);

   for (let i = 1; i <= steps; ++ i)
   {
      const t = i / steps;

      await touch (page, "touchMove", from .map ((p, k) => [ p [0] + (to [k] [0] - p [0]) * t, p [1] + (to [k] [1] - p [1]) * t ]));
      await frames (page, 1);

      if (pause)
         await page .waitForTimeout (pause);
   }

   await page .waitForTimeout (200);

   if (from .length > 1)
      await touch (page, "touchEnd", to .slice (0, 1));

   await touch (page, "touchEnd", [ ]);
}

const CAMERA = [ "Camera.position_changed", "Camera.orientation_changed" ];

// Screen points of sensors.wrl on the 400 x 300 canvas (Viewpoint "Front", fieldOfView 0.785).
const LEFT    = [ 91, 150 ];
const FRONT   = [ 220, 128 ];
const BACK    = [ 180, 170 ];
const SPHERE  = [ 308, 150 ];
const WORLD   = [ 200, 258 ];
const DRAG    = [ 200, 42 ];
const EMPTY   = [ 340, 236 ];
const OUTSIDE = [ 360, 40 ];

// --- input cases ------------------------------------------------------------------------------------------------

export const inputCases = {
   // TouchSensor over, press, release; the cursor over it; the hit point.
   async touchSensor (page, env)
   {
      await page .evaluate (pageLoad, `${env .worlds}sensors.wrl`);
      await settle (page);

      const out  = { };
      const list = [ "LeftTouch.isOver", "LeftTouch.isActive", "LeftTouch.hitPoint_changed", "BackTouch.isOver" ];

      await page .mouse .move (... OUTSIDE);
      await settle (page, 100);
      out .before = await fields (page, list);
      out .cursorBefore = await cursor (page);

      await page .mouse .move (... LEFT, { steps: 3 });
      await settle (page, 100);
      out .over = rounded (await fields (page, list), 3);
      out .cursorOver = await cursor (page);

      await page .mouse .down ();
      await settle (page, 100);
      out .pressed = rounded (await fields (page, list), 3);

      await page .mouse .up ();
      await settle (page, 100);
      out .released = rounded (await fields (page, list), 3);
      out .touchTime = (await fields (page, [ "LeftTouch.touchTime" ])) ["LeftTouch.touchTime"] > 0;

      await page .mouse .move (... OUTSIDE, { steps: 3 });
      await settle (page, 100);
      out .left = rounded (await fields (page, list), 3);
      out .cursorLeft = await cursor (page);
      return out;
   },

   // Two overlapping boxes: the nearer one gets the press; elsewhere the back one does.
   async overlap (page, env)
   {
      await page .evaluate (pageLoad, `${env .worlds}sensors.wrl`);
      await settle (page);

      const list = [ "FrontTouch.isActive", "BackTouch.isActive", "FrontTouch.isOver", "BackTouch.isOver" ];
      const out  = { };

      for (const [ name, point ] of [ [ "front", FRONT ], [ "back", BACK ] ])
      {
         await page .mouse .move (... point, { steps: 2 });
         await page .mouse .down ();
         await settle (page, 100);
         out [name] = await fields (page, list);
         await page .mouse .up ();
         await settle (page, 100);
      }

      return out;
   },

   // A PlaneSensor drag: its translation, and the cursor while it drags.
   async dragSensor (page, env)
   {
      await page .evaluate (pageLoad, `${env .worlds}sensors.wrl`);
      await settle (page);

      await page .mouse .move (... DRAG, { steps: 2 });
      await page .mouse .down ();
      await settle (page, 100);
      await page .mouse .move (DRAG [0] + 40, DRAG [1] + 10, { steps: 4 });
      await settle (page, 100);

      const out = { cursorDragging: await cursor (page), dragging: rounded (await fields (page, [ "Drag.isActive", "Drag.translation_changed" ]), 3) };

      await page .mouse .up ();
      await settle (page, 100);
      out .released = rounded (await fields (page, [ "Drag.isActive", "Drag.translation_changed", "DragBox.translation" ]), 3);
      return out;
   },

   // EXAMINE: a drag on empty space rotates around the center, the wheel zooms.
   async examine (page, env)
   {
      await page .evaluate (pageLoad, `${env .worlds}sensors.wrl`);
      await settle (page);

      const out = { start: rounded (await fields (page, CAMERA)) };

      await page .mouse .move (... EMPTY, { steps: 2 });
      await page .mouse .down ();
      await page .mouse .move (EMPTY [0] + 40, EMPTY [1] - 20, { steps: 5 });
      out .cursorRotating = await cursor (page);
      await page .waitForTimeout (300);
      await page .mouse .up ();
      await settle (page, 800);
      out .rotated = rounded (await fields (page, CAMERA));
      out .cursorAfter = await cursor (page);

      await page .mouse .move (... OUTSIDE, { steps: 2 });

      for (let i = 0; i < 3; ++ i)
      {
         await page .mouse .wheel (0, -100);
         await frames (page, 2);
         await page .waitForTimeout (ZOOM_SETTLE);
      }

      await settle (page, 800);
      out .zoomed = rounded (await fields (page, CAMERA));
      return out;
   },

   // An Anchor with a viewpoint fragment binds the viewpoint.
   async anchorViewpoint (page, env)
   {
      await page .evaluate (pageLoad, `${env .worlds}sensors.wrl`);
      await settle (page);

      await page .mouse .move (... SPHERE, { steps: 2 });
      await settle (page, 100);

      const out = { cursorOver: await cursor (page), over: await fields (page, [ "ViewAnchor.description" ]) };

      await page .mouse .down ();
      await page .mouse .up ();
      await settle (page, 300);
      out .bound = await fields (page, [ "Front.isBound", "Second.isBound" ]);
      return out;
   },

   // An Anchor with a world URL replaces the world.
   async anchorWorld (page, env)
   {
      await page .evaluate (pageLoad, `${env .worlds}sensors.wrl`);
      await settle (page);

      const events = page .evaluate (() => new Promise (resolve =>
      {
         const canvas = document .querySelector ("x3d-canvas");
         const list   = [ ];

         for (const name of [ "initialized", "shutdown", "error" ])
            canvas .addEventListener (name, () => list .push (name));

         canvas .addEventListener ("load", () => { list .push ("load"); resolve (list); }, { once: true });
      }));

      await page .mouse .move (... WORLD, { steps: 2 });
      await page .mouse .down ();
      await page .mouse .up ();

      const out = { events: await events };

      await settle (page, 200);
      out .world = await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .currentScene .worldURL .replace (/^.*\//, ""));
      out .target = await fields (page, [ "Target.isBound", "TargetBox.geometry" ]);
      return out;
   },

   // KeySensor gets the keys while the canvas has the focus, and not those of another element.
   async keyboard (page, env)
   {
      await page .evaluate (pageLoad, `${env .worlds}keys.x3dv`);
      await settle (page);

      const list = [ "Keys.keyPress", "Keys.keyRelease", "Keys.isActive", "Keys.shiftKey" ];

      await page .mouse .click (... EMPTY);
      await settle (page, 100);
      await page .keyboard .down ("a");
      await settle (page, 100);

      const out = { down: await fields (page, list) };

      await page .keyboard .up ("a");
      await page .keyboard .down ("Shift");
      await settle (page, 100);
      out .shift = await fields (page, list);
      await page .keyboard .up ("Shift");

      await page .focus ("#outside");
      await page .keyboard .press ("b");
      await settle (page, 100);
      out .outside = await fields (page, list);
      out .outsideValue = await page .evaluate (() => document .querySelector ("#outside") .value);
      return out;
   },

   // A touch cancellation and a canvas removal while a TouchSensor is pressed.
   async interrupted (page, env)
   {
      await page .evaluate (pageLoad, `${env .worlds}sensors.wrl`);
      await settle (page);

      const out = { };

      await touch (page, "touchStart", [ LEFT ]);
      await settle (page, 100);
      out .touchPressed = await fields (page, [ "LeftTouch.isActive" ]);
      await touch (page, "touchCancel", [ ]);
      await settle (page, 100);
      out .touchCancelled = await fields (page, [ "LeftTouch.isActive" ]);

      await page .mouse .move (... LEFT, { steps: 2 });
      await page .mouse .down ();
      await settle (page, 100);
      out .mousePressed = await fields (page, [ "LeftTouch.isActive" ]);
      await page .evaluate (() => { window .removed = document .querySelector ("x3d-canvas"); window .removed .remove (); });
      await page .waitForTimeout (100);
      out .removed = await page .evaluate (() => String (window .removed .browser .currentScene .getNamedNode ("LeftTouch") .isActive));
      await page .mouse .up ();
      await page .waitForTimeout (100);
      out .removedReleased = await page .evaluate (() => String (window .removed .browser .currentScene .getNamedNode ("LeftTouch") .isActive));
      await page .evaluate (() => document .body .prepend (window .removed));
      await settle (page, 100);
      out .restored = await fields (page, [ "LeftTouch.isActive", "LeftTouch.isOver" ]);
      return out;
   },
};

// --- touch cases (a context with touch) -------------------------------------------------------------------------

export const touchCases = {
   // A tap on the TouchSensor box, on the viewpoint Anchor.
   async tap (page, env)
   {
      await page .evaluate (pageLoad, `${env .worlds}sensors.wrl`);
      await settle (page);

      const list = [ "LeftTouch.isActive", "LeftTouch.hitPoint_changed" ];
      const out  = { };

      await touch (page, "touchStart", [ LEFT ]);
      await settle (page, 100);
      out .pressed = rounded (await fields (page, list), 3);
      await touch (page, "touchEnd", [ ]);
      await settle (page, 100);
      out .released = rounded (await fields (page, list), 3);
      out .touchTime = (await fields (page, [ "LeftTouch.touchTime" ])) ["LeftTouch.touchTime"] > 0;

      await touch (page, "touchStart", [ SPHERE ]);
      await touch (page, "touchEnd", [ ]);
      await settle (page, 300);
      out .anchor = await fields (page, [ "Front.isBound", "Second.isBound" ]);
      return out;
   },

   // One finger rotates (EXAMINE), two fingers apart zoom in.
   async navigate (page, env)
   {
      await page .evaluate (pageLoad, `${env .worlds}sensors.wrl`);
      await settle (page);

      const out = { start: rounded (await fields (page, CAMERA)) };

      await gesture (page, [ EMPTY ], [ [ EMPTY [0] + 40, EMPTY [1] - 20 ] ]);
      await settle (page, 800);
      out .rotated = rounded (await fields (page, CAMERA));

      await gesture (page, [ [ 330, 60 ], [ 370, 60 ] ], [ [ 300, 60 ], [ 395, 60 ] ], 6, ZOOM_SETTLE);
      await settle (page, 800);
      out .pinched = rounded (await fields (page, CAMERA));
      return out;
   },
};

// --- coordinate cases (a context with a device pixel ratio of 2) ------------------------------------------------

export const scaleCases = {
   // A canvas of another size at an offset: the same object under the same point, with contentScale 1 and auto.
   async coordinates (page, env)
   {
      const out = { };

      for (const scale of [ "1", "auto" ])
      {
         await page .evaluate (scale =>
         {
            const canvas = document .querySelector ("x3d-canvas");

            canvas .setAttribute ("contentScale", scale);
            canvas .style .cssText = "display:block;width:333px;height:211px;margin:23px 0 0 41px";
         },
         scale);

         await page .evaluate (pageLoad, `${env .worlds}sensors.wrl`);
         await settle (page);

         await page .mouse .move (41 + 82, 23 + 106, { steps: 2 });
         await page .mouse .down ();
         await settle (page, 100);

         out [scale] = {
            framebuffer: await page .evaluate (() =>
            {
               const browser = document .querySelector ("x3d-canvas") .browser;
               const canvas  = browser .getCanvas ?.() ?? browser .getContext () .canvas;

               return [ canvas .width, canvas .height ];
            }),
            pressed: rounded (await fields (page, [ "LeftTouch.isActive", "LeftTouch.hitPoint_changed" ]), 2),
         };

         await page .mouse .up ();
         await settle (page, 100);
      }

      return out;
   },
};

// --- loading cases ----------------------------------------------------------------------------------------------

export const loadingCases = {
   // Inline and ImageTexture with relative URLs; the pictures are compared by the caller.
   async success (page, env)
   {
      const from   = env .server .requests .length;
      const events = await page .evaluate (pageLoad, `${env .worlds}loading.wrl`);

      await settle (page, 300);
      return { events, served: served (env, from), console: loadMessages (page .log, env), picture: await env .picture (page) };
   },

   // 404 responses, and responses that are no X3D and no image: the world loads without them.
   async failures (page, env)
   {
      const out = { };

      for (const name of [ "loading-missing", "loading-invalid" ])
      {
         page .log .length = 0;

         const from   = env .server .requests .length;
         const events = await page .evaluate (pageLoad, `${env .worlds}${name}.wrl`);

         await settle (page, 300);
         out [name] = { events, served: served (env, from), console: loadMessages (page .log, env), picture: await env .picture (page) };
      }

      return out;
   },

   // MFString fallbacks: the candidates are tried in order.
   async fallback (page, env)
   {
      page .log .length = 0;

      const from   = env .server .requests .length;
      const events = await page .evaluate (pageLoad, `${env .worlds}loading-fallback.wrl`);

      await settle (page, 300);
      return { events, served: served (env, from), console: loadMessages (page .log, env), picture: await env .picture (page) };
   },

   // The world waits for its Inline and texture; the page and the old world go on meanwhile.
   async delayed (page, env)
   {
      env .server .arm ("inline");
      env .server .arm ("texture");

      // The old world draws first: its first frames compile the render pipelines, which on a CPU renderer
      // (lavapipe) takes every core for a while.
      await page .evaluate (pageLoad, `${env .worlds}sensors.wrl`);
      await settle (page, 1000);
      await page .evaluate (url =>
      {
         const canvas = document .querySelector ("x3d-canvas");

         window .delayedEvents = [ ];

         for (const name of [ "initialized", "load", "error", "shutdown" ])
            canvas .addEventListener (name, () => window .delayedEvents .push (name));

         window .delayedLoad = canvas .browser .loadURL (new window .X3D_NS .MFString (url)) .then (() => "resolved", e => e .message);
      },
      `${env .worlds}loading-held.wrl`);

      // The page is not blocked: frames and timers run while the responses are held.
      // Nothing blocks the page's main thread: timers and messages run on time for 0.6 s. (The frame rate is no
      // measure of it: a CPU renderer such as lavapipe draws few frames.)
      const responsive = await page .evaluate (async () =>
      {
         const start   = performance .now ();
         const channel = new MessageChannel ();
         let late = 0, latency = 0;

         while (performance .now () - start < 600)
         {
            const t = performance .now ();

            await new Promise (r => setTimeout (r, 10));
            late = Math .max (late, performance .now () - t - 10);

            const m = performance .now ();

            await new Promise (r => { channel .port1 .onmessage = r; channel .port2 .postMessage (0); });
            latency = Math .max (latency, performance .now () - m);
         }

         return { timerLateUnder100ms: late < 100, messageLatencyUnder50ms: latency < 50 };
      });

      const out = {
         responsive,
         held: await page .evaluate (() => ({ events: window .delayedEvents .slice (), world: document .querySelector ("x3d-canvas") .browser .currentScene .worldURL .replace (/^.*\//, "") })),
      };

      env .server .release ("inline");
      await settle (page, 300);
      out .inlineReleased = await page .evaluate (() => ({ events: window .delayedEvents .slice (), world: document .querySelector ("x3d-canvas") .browser .currentScene .worldURL .replace (/^.*\//, "") }));

      env .server .release ("texture");
      out .result = await page .evaluate (() => window .delayedLoad);
      await settle (page, 300);
      out .done = await page .evaluate (() => ({ events: window .delayedEvents .slice (), world: document .querySelector ("x3d-canvas") .browser .currentScene .worldURL .replace (/^.*\//, "") }));
      out .picture = await env .picture (page);
      return out;
   },

   // A newer loadURL while the first one waits for its responses: the first is aborted and never replaces the newer
   // world, also when its responses come later.
   async replacedDuringLoad (page, env)
   {
      env .server .arm ("inline");
      env .server .arm ("texture");
      page .log .length = 0;

      const out = await page .evaluate (async ([ held, box ]) =>
      {
         const browser = document .querySelector ("x3d-canvas") .browser;
         const MF      = window .X3D_NS .MFString;
         const first   = browser .loadURL (new MF (held)) .then (() => "resolved", e => e .message);

         await new Promise (r => setTimeout (r, 300));

         const second = await browser .loadURL (new MF (box)) .then (() => "resolved", e => e .message);

         return { second, firstAfterSecond: await Promise .race ([ first, new Promise (r => setTimeout (() => r ("pending"), 300)) ]) };
      },
      [ `${env .worlds}loading-held.wrl`, `${env .base}/web/example/box.wrl` ]);

      env .server .release ("inline");
      env .server .release ("texture");
      await settle (page, 500);

      out .world = await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .currentScene .worldURL .replace (/^.*\//, ""));
      out .material = await fields (page, [ "BoxMaterial.diffuseColor" ]);
      out .errors = page .log .filter (line => /pageerror/ .test (line));
      return out;
   },

   // Removal of the element while the world loads, then its return: the load completes.
   async removedDuringLoad (page, env)
   {
      env .server .arm ("inline");
      env .server .arm ("texture");

      await page .evaluate (url =>
      {
         window .removed     = document .querySelector ("x3d-canvas");
         window .removedLoad = window .removed .browser .loadURL (new window .X3D_NS .MFString (url)) .then (() => "resolved", e => e .message);
      },
      `${env .worlds}loading-held.wrl`);

      await page .waitForTimeout (200);
      await page .evaluate (() => window .removed .remove ());
      env .server .release ("inline");
      env .server .release ("texture");
      await page .waitForTimeout (300);
      await page .evaluate (() => document .body .prepend (window .removed));

      const result = await page .evaluate (() => window .removedLoad);

      await settle (page, 200);
      return { result, world: await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .currentScene .worldURL .replace (/^.*\//, "")) };
   },

   // Relative URLs of a scene from createX3DFromString resolve against the page; a cross-origin Inline fails.
   async createdScene (page, env)
   {
      page .log .length = 0;

      const from = env .server .requests .length;
      const out  = await page .evaluate (async foreign =>
      {
         const browser = document .querySelector ("x3d-canvas") .browser;
         const scene   = await browser .createX3DFromString (`#VRML V2.0 utf8
            DEF Relative Inline { url "worlds/parts/inner.wrl" }
            DEF Foreign Inline { url "${foreign}" }
            Viewpoint { position 0 0 8 }`);

         await browser .replaceWorld (scene);
         await new Promise (r => setTimeout (r, 300));

         return { pageRelative: scene .worldURL === document .baseURI, roots: Array .from (scene .rootNodes, n => n .getNodeTypeName ()) };
      },
      `${env .foreign}/tests/browser/worlds/parts/inner.wrl`);

      await settle (page, 300);

      // The page directory is /tests/browser/ for X_ITE_R and /x_ite/src/ for X_ITE (mounted to the same files). The
      // two requests go to two servers at once: the order they arrive in is no part of the result.
      out .served = served (env, from) .map (path => path .replace (/^(foreign:)?\/(tests\/browser|x_ite\/src)\//, "$1PAGE/")) .sort ();
      out .console = loadMessages (page .log, env);
      return out;
   },

   // Two ImageTextures and two Inlines with the same URLs.
   async repeated (page, env)
   {
      const from = env .server .requests .length;

      await page .evaluate (pageLoad, `${env .worlds}repeated.wrl`);
      await settle (page, 300);

      const paths = served (env, from);

      return {
         texture: paths .filter (p => p .endsWith ("tex/red.png")) .length,
         inline: paths .filter (p => p .endsWith ("parts/inner.wrl")) .length,
      };
   },

   // EXTERNPROTO: a fallback candidate, and a world that waits for the PROTO file.
   async externProto (page, env)
   {
      page .log .length = 0;
      env .server .arm ("proto");

      const from = env .server .requests .length;
      const out  = { events: await page .evaluate (pageLoad, `${env .worlds}extern.wrl`) };

      await settle (page, 300);
      out .instance = await fields (page, [ "Instance.color" ]);
      out .typeName = await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("Instance") .getNodeTypeName ());
      out .served = served (env, from);
      out .console = loadMessages (page .log, env);
      out .picture = await env .picture (page);

      await page .evaluate (url =>
      {
         window .externLoad = document .querySelector ("x3d-canvas") .browser .loadURL (new window .X3D_NS .MFString (url)) .then (() => "resolved", e => e .message);
      },
      `${env .worlds}extern-held.wrl`);
      await page .waitForTimeout (400);
      out .heldWorld = await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .currentScene .worldURL .replace (/^.*\//, ""));
      env .server .release ("proto");
      out .heldResult = await page .evaluate (() => window .externLoad);
      await settle (page, 200);
      out .heldInstance = await fields (page, [ "Instance.color" ]);
      return out;
   },
};

// Disposal while a world waits for its responses (no page error, the late responses change nothing).
export async function disposedDuringLoad (page, env)
{
   env .server .arm ("inline");
   env .server .arm ("texture");

   await page .evaluate (url =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;

      window .disposedLoad = browser .loadURL (new window .X3D_NS .MFString (url)) .then (() => "resolved", e => e .message);
   },
   `${env .worlds}loading-held.wrl`);

   await page .waitForTimeout (200);
   await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .dispose ());
   env .server .release ("inline");
   env .server .release ("texture");
   await page .waitForTimeout (500);

   return {
      result: await page .evaluate (() => Promise .race ([ window .disposedLoad, new Promise (r => setTimeout (() => r ("pending"), 200)) ])),
      errors: page .log .filter (line => /pageerror/ .test (line)),
   };
}

// --- scene API cases --------------------------------------------------------------------------------------------

export const apiCases = {
   // createX3DFromString: a Promise of a scene that is not the world yet; its worldURL is the page's.
   async createScene (page, env)
   {
      await page .evaluate (pageLoad, `${env .base}/web/example/box.wrl`);
      await settle (page);

      return page .evaluate (async () =>
      {
         const browser = document .querySelector ("x3d-canvas") .browser;
         const promise = browser .createX3DFromString (`#VRML V2.0 utf8
            DEF Root Transform { children Shape { geometry Box { } } }
            DEF View Viewpoint { position 0 0 7 description "View" }`);
         const scene   = await promise;

         return {
            isPromise:    promise instanceof Promise,
            pageWorldURL: scene .worldURL === document .baseURI,
            roots:        Array .from (scene .rootNodes, n => n .getNodeTypeName ()),
            root:         scene .getNamedNode ("Root") .getNodeTypeName (),
            notCurrent:   browser .currentScene !== scene,
            currentWorld: browser .currentScene .worldURL .replace (/^.*\//, ""),
         };
      });
   },

   // replaceWorld (scene): the order of the callbacks and events, and the new current scene.
   async replaceOrder (page, env)
   {
      await page .evaluate (pageLoad, `${env .base}/web/example/box.wrl`);
      await settle (page);

      return page .evaluate (async () =>
      {
         const X3D     = window .X3D_NS;
         const canvas  = document .querySelector ("x3d-canvas");
         const browser = canvas .browser;
         const log     = [ ];
         const names   = { [X3D .X3DConstants .SHUTDOWN_EVENT]: "SHUTDOWN_EVENT", [X3D .X3DConstants .INITIALIZED_EVENT]: "INITIALIZED_EVENT", [X3D .X3DConstants .INITIALIZED_ERROR]: "INITIALIZED_ERROR" };
         const scene   = await browser .createX3DFromString ("#VRML V2.0 utf8\nDEF View Viewpoint { position 0 0 7 }");

         browser .addBrowserCallback ("case", event => log .push (names [event] ?? event));

         for (const name of [ "initialized", "load", "error", "shutdown" ])
            canvas .addEventListener (name, () => log .push (name));

         const replaced = browser .replaceWorld (scene);
         const out      = { isPromise: replaced instanceof Promise, logAtCall: log .slice () };

         await new Promise (requestAnimationFrame);
         out .logAfterFrame = log .slice ();
         out .result  = await replaced .then (v => `resolved ${v}`, e => e .message);
         out .log     = log .slice ();
         out .current = browser .currentScene === scene;
         out .view    = String (scene .getNamedNode ("View") .position);

         browser .removeBrowserCallback ("case");
         return out;
      });
   },

   // createX3DFromString with a syntax error, and replaceWorld (null).
   async invalid (page, env)
   {
      await page .evaluate (pageLoad, `${env .base}/web/example/box.wrl`);
      await settle (page);

      return page .evaluate (async () =>
      {
         const browser = document .querySelector ("x3d-canvas") .browser;
         const out     = { };

         try
         {
            await browser .createX3DFromString ("#VRML V2.0 utf8\nTransform { children [ Box { ");
            out .syntax = "no error";
         }
         catch (error)
         {
            out .syntax = { name: error .constructor .name, firstLine: String (error .message) .trim () .split ("\n") [0] };
         }

         out .worldKept   = browser .currentScene .worldURL .replace (/^.*\//, "");
         out .nullReplace = await browser .replaceWorld (null) .then (() => "resolved", e => e .message);
         out .emptyRoots  = browser .currentScene .rootNodes .length;
         return out;
      });
   },

   // A replaceWorld while another one waits for its frame: the older one rejects.
   async abortedReplace (page, env)
   {
      await page .evaluate (pageLoad, `${env .base}/web/example/box.wrl`);
      await settle (page);

      return page .evaluate (async () =>
      {
         const browser = document .querySelector ("x3d-canvas") .browser;
         const a       = await browser .createX3DFromString ("#VRML V2.0 utf8\nDEF A Transform { }");
         const b       = await browser .createX3DFromString ("#VRML V2.0 utf8\nDEF B Transform { }");
         const first   = browser .replaceWorld (a) .then (() => "resolved", e => e .message);
         const second  = browser .replaceWorld (b) .then (() => "resolved", e => e .message);

         return { first: await first, second: await second, current: browser .currentScene === b };
      });
   },

   // The scene's nodes: rootNodes, getNamedNode and its error, field values and a field update.
   async sceneAccess (page, env)
   {
      await page .evaluate (pageLoad, `${env .worlds}viewpoints.wrl`);
      await settle (page);

      return page .evaluate (() =>
      {
         const scene   = document .querySelector ("x3d-canvas") .browser .currentScene;
         const attempt = f => { try { return f (); } catch (error) { return `error: ${error .message}`; } };
         const two     = scene .getNamedNode ("Two");

         const out = {
            roots:       Array .from (scene .rootNodes, n => `${n .getNodeTypeName ()} ${n .getNodeName ()}`),
            missing:     attempt (() => scene .getNamedNode ("NoSuchNode")),
            description: two .description,
            position:    String (two .position),
            fieldType:   two .getField ("position") .getTypeName (),
         };

         two .description = "Changed";
         out .changed = two .description;
         return out;
      });
   },

   // nextViewpoint, previousViewpoint, firstViewpoint, lastViewpoint: the user viewpoints (with a description), in
   // order, bound with the next frame.
   async viewpointCycle (page, env)
   {
      await page .evaluate (pageLoad, `${env .worlds}viewpoints.wrl`);
      await settle (page);
      return viewpointCalls (page, [ [ "nextViewpoint" ], [ "nextViewpoint" ], [ "nextViewpoint" ], [ "previousViewpoint" ], [ "lastViewpoint" ], [ "firstViewpoint" ] ]);
   },

   // changeViewpoint (name): any viewpoint of the layer, also one without a description; an unknown name does nothing.
   async changeViewpoint (page, env)
   {
      await page .evaluate (pageLoad, `${env .worlds}viewpoints.wrl`);
      await settle (page);
      return viewpointCalls (page, [ [ "changeViewpoint", "Hidden" ], [ "nextViewpoint" ], [ "changeViewpoint", "NoSuchViewpoint" ], [ "changeViewpoint", "Three" ] ]);
   },

   // activeViewpoint and viewAll.
   async viewAll (page, env)
   {
      await page .evaluate (pageLoad, `${env .worlds}viewpoints.wrl`);
      await settle (page);

      const active = () => page .evaluate (() => document .querySelector ("x3d-canvas") .browser .activeViewpoint ?.getNodeName () ?? null);
      const out    = { active: await active () };

      await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .viewAll (0));
      await settle (page, 300);
      out .viewAll = rounded (await fields (page, [ "Camera.position_changed", "Camera.orientation_changed" ]), 3);
      out .activeAfter = await active ();

      await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .replaceWorld (null));
      await settle (page);
      out .emptyWorld = await active ();
      return out;
   },

   // Old scene and node handles after a replacement.
   async staleHandles (page, env)
   {
      await page .evaluate (pageLoad, `${env .base}/web/example/box.wrl`);
      await settle (page);

      return page .evaluate (async world =>
      {
         const browser  = document .querySelector ("x3d-canvas") .browser;
         const oldScene = browser .currentScene;

         await browser .loadURL (new window .X3D_NS .MFString (world));

         return {
            sameScene: browser .currentScene === oldScene,
            newNode:   String (browser .currentScene .getNamedNode ("BoxMaterial") .diffuseColor),
         };
      },
      `${env .base}/web/example/box.wrl`);
   },
};

// The bound viewpoints of viewpoints.wrl right after each call and a few frames later.
async function viewpointCalls (page, calls)
{
   const bound = () => page .evaluate (() =>
   {
      const scene = document .querySelector ("x3d-canvas") .browser .currentScene;

      return [ "One", "Two", "Hidden", "Three" ] .filter (name => scene .getNamedNode (name) .isBound) .join (" ");
   });

   const out = [ `start ${await bound ()}` ];

   for (const [ name, ... args ] of calls)
   {
      const immediately = await page .evaluate (([ name, args ]) =>
      {
         const browser = document .querySelector ("x3d-canvas") .browser;

         browser [name] (... args);
         return [ "One", "Two", "Hidden", "Three" ] .filter (n => browser .currentScene .getNamedNode (n) .isBound) .join (" ");
      },
      [ name, args ]);

      await settle (page, 100);
      out .push (`${name}(${args .join (",")}): ${immediately} -> ${await bound ()}`);
   }

   return out;
}

// --- groups -----------------------------------------------------------------------------------------------------

const DESKTOP = { viewport: { width: 400, height: 340 }, deviceScaleFactor: 1 };

// The cases and the browser context each group runs in. The touch context is a desktop Chrome with touch input and
// a device pixel ratio of 2: an emulation, no proof of a mobile browser or GPU.
export const CASE_GROUPS = {
   input:   { context: DESKTOP, cases: inputCases },
   touch:   { context: { ... DESKTOP, deviceScaleFactor: 2, hasTouch: true }, cases: touchCases },
   scale:   { context: { viewport: { width: 400, height: 300 }, deviceScaleFactor: 2 }, cases: scaleCases },
   loading: { context: DESKTOP, cases: { ... loadingCases, disposedDuringLoad } },
   api:     { context: DESKTOP, cases: apiCases },
};

// The values of a case with each picture (a Buffer) replaced by the name of its file, which goes into `pictures`.
export function splitPictures (value, key, pictures)
{
   if (Buffer .isBuffer (value))
   {
      const file = `${key}.png`;

      pictures [file] = value;
      return { picture: file };
   }

   if (Array .isArray (value))
      return value .map ((v, i) => splitPictures (v, `${key}.${i}`, pictures));

   if (value && typeof value === "object")
      return Object .fromEntries (Object .entries (value) .map (([ k, v ]) => [ k, splitPictures (v, `${key}.${k}`, pictures) ]));

   return value;
}
