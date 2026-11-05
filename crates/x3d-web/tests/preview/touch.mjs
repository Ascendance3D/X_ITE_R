// Touch on the Browser Preview in Chromium's mobile emulation (EMULATION, not a real device): a phone (390 x 844,
// DPR 3) or a tablet (820 x 1180, DPR 2) context with touch, and touch events sent through the DevTools protocol
// (Input.dispatchTouchEvent), so several fingers move at once.
//
//   node tests/preview/touch.mjs <base URL> [--device phone|tablet] [--webgpu-flags] [--backend auto|webgpu|webgl2]
//                                [--out DIR]
//
// Checks: the canvas fits the screen; tap on a TouchSensor; one-finger drag of a PlaneSensor; one-finger EXAMINE
// rotation; two-finger pinch zoom; touchcancel releases a sensor; tap on an Anchor jumps to its viewpoint; dispose
// with touches after it has no page error. Exit status 0 only when every check passes.
//
// What emulation does not show: a real device's GPU, its browser's own touch gestures (scroll, zoom of the page),
// palm rejection, and the timing of a real touch screen.

import { chromium } from "playwright";
import { mkdirSync, writeFileSync } from "node:fs";

const argv    = process .argv .slice (2);
const opt     = name => { const i = argv .indexOf (name); return i >= 0 ? argv [i + 1] : undefined; };
const base    = new URL (argv .find (a => /^https?:/ .test (a)) ?? "http://127.0.0.1:8000/");
const device  = opt ("--device") ?? "phone";
const backend = opt ("--backend") ?? "auto";
const out     = opt ("--out") ?? `touch-${device}`;
const flags   = "--enable-unsafe-webgpu --enable-features=Vulkan --use-angle=vulkan --ignore-gpu-blocklist --enable-gpu" .split (" ");
const DEVICES =
{
   phone:  { viewport: { width: 390, height: 844 },  deviceScaleFactor: 3, isMobile: true, hasTouch: true },
   tablet: { viewport: { width: 820, height: 1180 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true },
};

mkdirSync (out, { recursive: true });

const browser = await chromium .launch ({ args: argv .includes ("--webgpu-flags") ? flags : [ ] });
const context = await browser .newContext (DEVICES [device]);
const page    = await context .newPage ();
const cdp     = await context .newCDPSession (page);
const console_ = [ ];

page .on ("console",   m => console_ .push (`[${m .type ()}] ${m .text ()}`));
page .on ("pageerror", e => console_ .push (`[pageerror] ${e}`));

const status = () => page .locator ("#status") .textContent ();
const field  = (node, name) => page .evaluate (([ node, name ]) => String (globalThis .X_ITE_R_PREVIEW .nodes [node] ?.[name] ?? ""), [ node, name ]);
const vec    = text => String (text) .trim () .split (/\s+/) .map (Number);
const frames = n => page .evaluate (n => new Promise (r => { let i = 0; const f = () => ++ i >= n ? r () : requestAnimationFrame (f); requestAnimationFrame (f); }), n);

async function load (url)
{
   await page .evaluate (url => X_ITE_R_PREVIEW .loadWorld (url), url);
   await page .waitForFunction (() => /^(loaded|error)/ .test (document .getElementById ("status") .textContent), null, { timeout: 60000 });
   await page .waitForTimeout (400); // the splash screen fades out
   return status ();
}

// The canvas in view; `at (fx, fy)` is a point of it as fractions of its width and height (the worlds are made for 4:3).
async function canvas ()
{
   await page .evaluate (() => document .getElementById ("canvas") .scrollIntoView ({ block: "center" }));
   const b = await page .locator ("x3d-canvas") .boundingBox ();

   return { box: b, at: (fx, fy) => ({ x: b .x + 1 + fx * (b .width - 2), y: b .y + 1 + fy * (b .height - 2) }) };
}

const touch = (type, points) => cdp .send ("Input.dispatchTouchEvent", { type, touchPoints: points .map ((p, id) => ({ x: p .x, y: p .y, id, radiusX: 4, radiusY: 4, force: 1 })) });

// Fingers from `from` to `to` (arrays of points) in `steps` moves, one frame each.
async function gesture (from, to, steps = 12)
{
   await touch ("touchStart", from);
   await frames (2);

   for (let i = 1; i <= steps; ++ i)
   {
      await touch ("touchMove", from .map ((p, k) => ({ x: p .x + (to [k] .x - p .x) * i / steps, y: p .y + (to [k] .y - p .y) * i / steps })));
      await frames (1);
   }

   await touch ("touchEnd", [ ]);
   await frames (3);
}

const results = [ ];

async function check (name, fn)
{
   try
   {
      const { pass, detail } = await fn ();

      results .push ({ name, pass, detail });
      console .log (`${pass ? "PASS" : "FAIL"} ${name} — ${JSON .stringify (detail)}`);
   }
   catch (error)
   {
      results .push ({ name, pass: false, detail: { exception: String (error) } });
      console .log (`FAIL ${name} — exception: ${error}`);
   }
}

await page .goto (new URL (`preview/index.html${backend === "auto" ? "" : "?backend=" + backend}`, base) .href);
await page .waitForFunction (() => /^loaded/ .test (document .getElementById ("status") .textContent), null, { timeout: 120000 });
await page .waitForTimeout (400);

const graphics = await page .locator ("#webgpu") .textContent ();

await check ("the canvas fits the screen width", async () =>
{
   const { box } = await canvas ();
   const width   = await page .evaluate (() => document .documentElement .clientWidth);

   await page .screenshot ({ path: `${out}/01-page.png` });

   return { pass: box .x >= 0 && box .x + box .width <= width + 0.5, detail: { box, screen: width } };
});

await check ("tap: TouchSensor isActive while the finger is down, touchTime after", async () =>
{
   const p = (await canvas ()) .at (175 / 640, 240 / 480);

   await touch ("touchStart", [ p ]);
   await frames (3);
   const active = await field ("Touch", "isActive");
   await touch ("touchEnd", [ ]);
   await frames (3);
   const after = await field ("Touch", "isActive"), time = Number (await field ("Touch", "touchTime"));

   return { pass: active === "true" && after === "false" && time > 0, detail: { active, after, touchTime: time } };
});

// Pinned X_ITE (PointingDevice.js) listens to touchstart, touchmove and touchend only: it has no answer for a cancelled
// touch to compare with. X_ITE_R ends the press on pointercancel. The check records the state after the cancel, and passes
// when nothing stays pressed: the next tap gives a full isActive TRUE, FALSE and touchTime cycle.
await check ("touchcancel: the next tap works (no sensor stays pressed)", async () =>
{
   const p      = (await canvas ()) .at (175 / 640, 240 / 480);
   const before = Number (await field ("Touch", "touchTime"));

   await touch ("touchStart", [ p ]);
   await frames (3);
   const active = await field ("Touch", "isActive");
   await touch ("touchCancel", [ ]);
   await frames (3);
   const cancelled = { isActive: await field ("Touch", "isActive"), touchTime: Number (await field ("Touch", "touchTime")) };

   await touch ("touchStart", [ p ]);
   await frames (3);
   const again = await field ("Touch", "isActive");
   await touch ("touchEnd", [ ]);
   await frames (3);
   const after = await field ("Touch", "isActive"), time = Number (await field ("Touch", "touchTime"));

   return {
      pass: active === "true" && again === "true" && after === "false" && time > Math .max (before, cancelled .touchTime),
      detail: { active, cancelled, touchTimeBefore: before, again, after, touchTime: time },
   };
});

await check ("one-finger drag: the PlaneSensor moves the blue box", async () =>
{
   const c = await canvas ();

   await gesture ([ c .at (465 / 640, 240 / 480) ], [ c .at (525 / 640, 200 / 480) ]);
   const t = vec (await field ("Drag", "translation_changed"));

   await page .screenshot ({ path: `${out}/02-planesensor.png` });

   return { pass: t [0] > 0.3 && t [1] > 0.2, detail: { translation_changed: t } };
});

await check ("one-finger drag in EXAMINE turns the view", async () =>
{
   const s     = await load ("worlds/navigation.wrl");
   const c     = await canvas ();
   const start = await field ("Viewer", "orientation_changed");

   await gesture ([ c .at (0.3, 0.8) ], [ c .at (0.6, 0.8) ]);
   const turned = await field ("Viewer", "orientation_changed");

   return { pass: /^loaded/ .test (s) && turned !== "" && turned !== start, detail: { status: s, start, turned } };
});

await check ("two-finger pinch apart zooms in (EXAMINE)", async () =>
{
   await load ("worlds/navigation.wrl");
   const c      = await canvas ();
   const before = vec (await field ("Viewer", "position_changed"));

   await gesture ([ c .at (0.4, 0.75), c .at (0.6, 0.75) ], [ c .at (0.25, 0.75), c .at (0.75, 0.75) ]);
   const after = vec (await field ("Viewer", "position_changed"));
   const len   = v => Math .hypot (... v);

   await page .screenshot ({ path: `${out}/03-pinch.png` });

   return { pass: before .length === 3 && after .length === 3 && len (after) < len (before) - 0.1, detail: { before, after } };
});

await check ("tap on an Anchor jumps to its viewpoint", async () =>
{
   const s = await load ("worlds/anchor.wrl"); // a box under an Anchor to the viewpoint "Side"
   const c = await canvas ();

   await touch ("touchStart", [ c .at (0.5, 0.5) ]);
   await frames (2);
   await touch ("touchEnd", [ ]);
   await page .waitForTimeout (1500); // the viewpoint transition
   await frames (3);
   const position = vec (await field ("Viewer", "position_changed"));

   return { pass: /^loaded/ .test (s) && Math .abs (position [0] - 10) < 0.5 && Math .abs (position [2]) < 0.5, detail: { status: s, position } };
});

await check ("dispose, then touches: no page error", async () =>
{
   const errors = console_ .filter (l => l .startsWith ("[pageerror]")) .length;
   const c      = await canvas ();

   await page .tap ("#dispose");
   await page .waitForTimeout (300);
   await gesture ([ c .at (0.3, 0.3), c .at (0.6, 0.6) ], [ c .at (0.4, 0.4), c .at (0.5, 0.5) ], 4);
   const s = await status ();

   return { pass: /disposed/ .test (s) && console_ .filter (l => l .startsWith ("[pageerror]")) .length === errors, detail: { status: s } };
});

const summary = { device, emulation: true, version: browser .version (), backend, graphics, results, console: console_ };

writeFileSync (`${out}/results.json`, JSON .stringify (summary, null, 1));
console .log (`checks: ${results .length} run, ${results .filter (r => r .pass) .length} passed, ${results .filter (r => !r .pass) .length} failed (${device}, emulation, ${graphics})`);

await browser .close ();
process .exit (results .every (r => r .pass) ? 0 : 1);
