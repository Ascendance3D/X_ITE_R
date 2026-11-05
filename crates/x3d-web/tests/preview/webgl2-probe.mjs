// The WebGL2 proof: the real browser module draws a red triangle and a green quad on a dark blue clear color through
// wgpu's WebGL2 backend (`webgl2Probe` in src/webgl_probe.rs), and this script checks the pixels of a screenshot.
// An adapter that starts but draws nothing fails.
//
//   node tests/preview/webgl2-probe.mjs <base URL of the package or of crates/x3d-web/web> [--browser chromium|firefox|webkit]
//                                       [--channel chrome|msedge] [--no-webgpu] [--out DIR]
//
// --no-webgpu starts Chromium with WebGPU switched off, as a browser that has no WebGPU. Exit status 0 only when the
// three pixel checks pass and the backend reports no validation error.

import { chromium, firefox, webkit } from "playwright";
import { mkdirSync, writeFileSync } from "node:fs";

const argv    = process .argv .slice (2);
const opt     = name => { const i = argv .indexOf (name); return i >= 0 ? argv [i + 1] : undefined; };
const base    = new URL (argv .find (a => /^https?:/ .test (a)) ?? "http://127.0.0.1:8000/");
const kind    = opt ("--browser") ?? "chromium";
const channel = opt ("--channel");
const out     = opt ("--out") ?? "webgl2-probe";
const args    = argv .includes ("--no-webgpu") ? [ "--disable-features=WebGPU,Vulkan", "--disable-webgpu" ] : [ ];

mkdirSync (out, { recursive: true });

const type    = { chromium, firefox, webkit } [kind];
const browser = await type .launch ({ channel, headless: !argv .includes ("--headed"), args: kind === "chromium" ? args : [ ] });
const page    = await (await browser .newContext ({ viewport: { width: 800, height: 600 } })) .newPage ();
const console_ = [ ];

page .on ("console", m => console_ .push (`[${m .type ()}] ${m .text ()}`));
page .on ("pageerror", e => console_ .push (`[pageerror] ${e}`));

await page .goto (new URL ("build-info.json", base) .href) .catch (() => page .goto (base .href));

const result = await page .evaluate (async pkg =>
{
   document .documentElement .innerHTML = "<body style='margin:0;background:#fff'><canvas id=c width=400 height=300 style='display:block'></canvas></body>";

   const module = await import (pkg);

   await module .default ();

   const canvas = document .getElementById ("c");
   const hasWebGpu = !! navigator .gpu;

   try
   {
      return { hasWebGpu, report: JSON .parse (await module .webgl2Probe (canvas)) };
   }
   catch (error)
   {
      return { hasWebGpu, error: String (error ?.message ?? error) };
   }
},
new URL ("pkg/x3d_web.js", base) .href);

// Pixels of the canvas, from a screenshot of the canvas element (the browser's own presentation of the WebGL2 canvas).
await page .waitForTimeout (500);

const shot = await page .locator ("#c") .screenshot ();

writeFileSync (`${out}/${kind}${channel ? "-" + channel : ""}.png`, shot);

const pixels = await page .evaluate (async data =>
{
   const image = new Image ();

   image .src = `data:image/png;base64,${data}`;
   await image .decode ();

   const c = document .createElement ("canvas");

   c .width  = image .width;
   c .height = image .height;

   const g = c .getContext ("2d");

   g .drawImage (image, 0, 0);

   const at = (x, y) => Array .from (g .getImageData (Math .round (x * image .width), Math .round (y * image .height), 1, 1) .data .slice (0, 3));

   return { size: [ image .width, image .height ], background: at (0.02, 0.02), triangle: at (0.25, 0.6), quad: at (0.75, 0.5) };
},
shot .toString ("base64"));

const near = (p, q) => p .every ((v, i) => Math .abs (v - q [i]) <= 12);
const checks = {
   background: near (pixels .background, [ 0, 0, 102 ]),
   triangle:   near (pixels .triangle,   [ 255, 0, 0 ]),
   quad:       near (pixels .quad,       [ 0, 255, 0 ]),
   noValidationError: !! result .report && result .report .validation_error === null,
};

const summary = {
   browser: kind,
   channel: channel ?? null,
   version: browser .version (),
   args,
   userAgent: await page .evaluate (() => navigator .userAgent),
   ...result,
   pixels,
   checks,
   console: console_,
};

writeFileSync (`${out}/${kind}${channel ? "-" + channel : ""}.json`, JSON .stringify (summary, null, 1));
console .log (JSON .stringify (summary, null, 1));

await browser .close ();

process .exit (Object .values (checks) .every (Boolean) ? 0 : 1);
