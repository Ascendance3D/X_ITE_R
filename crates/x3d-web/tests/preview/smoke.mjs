// Release smoke test of the X_ITE_R Browser Preview package (tools/package-web-preview.sh) at a URL.
//
//   node tests/preview/smoke.mjs <base URL of the package> [--out DIR] [--channel chrome|msedge|chromium]
//                                [--webgpu-flags] [--headed] [--pause MS]
//
// <base URL> is the directory that holds x_ite_r.mjs, pkg/ and preview/ (a local server of the package directory,
// or the staging server). The browser is a new, clean profile. Without --webgpu-flags it gets no flags, as a user's
// browser; --webgpu-flags adds the flags that Chrome and Edge on Linux need for WebGPU (Vulkan, unsafe WebGPU, GPU in
// headless mode). Chrome and Edge on Windows and macOS have WebGPU without flags.
//
// Each check reads values from the engine through the page's JavaScript API, or pixels from a screenshot of the
// canvas; loading the page is never a pass by itself. Evidence: results.json, console.txt and a screenshot of each
// step in --out. Exit status 0 only when every check passes.

import { chromium } from "playwright";
import { webDriverPage } from "./webdriver.mjs";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

const argv = process .argv .slice (2);
const opt  = name => { const i = argv .indexOf (name); return i >= 0 ? argv [i + 1] : undefined; };
const base = new URL (argv .find (a => /^https?:/ .test (a)) ?? "http://127.0.0.1:8000/");

if (!base .pathname .endsWith ("/"))
   base .pathname += "/";

const out     = opt ("--out") ?? "preview-smoke";
const channel = opt ("--channel");
const pause   = Number (opt ("--pause") ?? 0); // milliseconds after each check, to watch a --headed run
const W = 640, H = 480;
const BG = [ 38, 38, 51 ]; // skyColor 0.15 0.15 0.2

await mkdir (out, { recursive: true });

const results = [ ];
let   page;
const consoleLines = [ ];
const pointerLines = [ ];
let   step = 0;

function record (name, pass, detail)
{
   results .push ({ name, pass: !!pass, detail });
   const text = detail ? (typeof detail === "string" ? detail : JSON .stringify (detail)) : "";

   console .log (`${pass ? "PASS" : "FAIL"} ${name}${text ? ` — ${text .length > 400 ? text .slice (0, 400) + " …" : text}` : ""}`);
}

async function check (name, fn)
{
   try
   {
      const { pass, detail } = await fn ();

      record (name, pass, detail);

      // WebDriver: the pointer events of a failed check, as the page got them.
      if (page ?.pointerLog)
      {
         const lines = await page .pointerLog ();

         if (!pass)
            pointerLines .push (`--- ${name}`, ... lines);
      }

      if (pause)
         await page ?.waitForTimeout (pause);
   }
   catch (error)
   {
      record (name, false, `exception: ${error ?.stack ?? error}`);
   }
}

// 1. HTTP: status and Content-Type of the files the page needs.
const HTTP = [
   [ "preview/index.html", /^text\/html/ ],
   [ "preview/preview.mjs", /^(text|application)\/javascript/ ],
   [ "x_ite_r.mjs", /^(text|application)\/javascript/ ],
   [ "audio_mirror.mjs", /^(text|application)\/javascript/ ],
   [ "video_mirror.mjs", /^(text|application)\/javascript/ ],
   [ "pkg/x3d_web.js", /^(text|application)\/javascript/ ],
   [ "pkg/fields.mjs", /^(text|application)\/javascript/ ],
   [ "pkg/script.mjs", /^(text|application)\/javascript/ ],
   [ "pkg/x3d_web_bg.wasm", /^application\/wasm$/ ],
   [ "build-info.json", /json/ ],
   [ "preview/worlds/vrml97-basic.wrl", /./ ],
   [ "preview/worlds/x3d-basic.x3d", /./ ],
   [ "preview/textures/checker8.png", /^image\/png/ ],
];

for (const [ path, type ] of HTTP)
{
   await check (`http ${path}`, async () =>
   {
      const response = await fetch (new URL (path, base), { cache: "no-store" });
      const ct       = response .headers .get ("content-type") ?? "";

      await response .arrayBuffer ();
      return { pass: response .status === 200 && type .test (ct), detail: `${response .status} ${ct}` };
   });
}

await check ("http missing world is 404", async () =>
{
   const response = await fetch (new URL ("preview/worlds/missing-on-purpose.wrl", base));

   await response .arrayBuffer ();
   return { pass: response .status === 404, detail: `${response .status}` };
});

// 2. The browser: a new profile, no extensions.
const args = argv .includes ("--webgpu-flags")
   ? (process .env .SMOKE_FLAGS ?? "--enable-unsafe-webgpu --enable-features=Vulkan --use-angle=vulkan --ignore-gpu-blocklist --enable-gpu") .split (" ")
   : [ ];

// --webdriver <endpoint> --browser safari|firefox: a W3C WebDriver session (safaridriver, geckodriver) instead of
// Playwright; see webdriver.mjs for what it records differently.
const webdriver = opt ("--webdriver");
const responses = [ ];
let browser, helperBrowser, helper, decode, getResponses, pressAt, releaseAt, dragPath, wheelAt;

if (webdriver)
{
   // Firefox's WebGPU canvas needs a window (headless Firefox 158 has no "webgpu" context), and Safari has no headless
   // mode, so these run in a window. A window under the desktop's own cursor also gets that cursor's synthetic
   // pointermove events (Firefox sends them when the content under the cursor changes): keep the cursor off it.
   page = await webDriverPage (webdriver, { browserName: opt ("--browser") ?? "safari" });
   // --window x,y,width,height: where the window goes, e.g. away from the desktop cursor (default 0,0,700,1000: the page
   // in one column, the canvas at the top left).
   const [ wx, wy, ww, wh ] = (opt ("--window") ?? "0,0,700,1000") .split (",") .map (Number);

   await page .setWindowRect ({ x: wx, y: wy, width: ww, height: wh });
   decode       = b64 => page .decode (b64);
   getResponses = () => page .responses ();
   pressAt      = (x, y) => page .press (x, y);
   releaseAt    = () => page .release ();
   dragPath     = (from, to, steps, hold) => page .drag (from, to, steps, hold);
   wheelAt      = async (x, y, dy) => { await page .mouse .move (x, y); await page .mouse .wheel (0, dy); };
}
else
{
   browser = await chromium .launch ({ channel, headless: !argv .includes ("--headed"), args });

   const context = await browser .newContext ({ viewport: { width: 1280, height: 1000 }, deviceScaleFactor: 1 });

   // Decodes the screenshots: a headless browser of its own, so a --headed run shows only the page.
   helperBrowser = await chromium .launch ({ channel, headless: true });
   helper        = await helperBrowser .newPage ();
   page          = await context .newPage ();

   page .on ("console",   message => consoleLines .push (`[${message .type ()}] ${message .text ()}`));
   page .on ("pageerror", error   => consoleLines .push (`[pageerror] ${error .message}`));
   page .on ("response",  response => responses .push ({ url: response .url (), status: response .status (), type: response .headers () ["content-type"] ?? "" }));

   decode = b64 => helper .evaluate (async b64 =>
   {
      const bitmap = await createImageBitmap (await (await fetch (`data:image/png;base64,${b64}`)) .blob ());
      const c      = new OffscreenCanvas (bitmap .width, bitmap .height);
      const g      = c .getContext ("2d");

      g .drawImage (bitmap, 0, 0);
      return { w: bitmap .width, h: bitmap .height, px: Array .from (g .getImageData (0, 0, bitmap .width, bitmap .height) .data) };
   },
   b64);

   getResponses = async () => responses;
   pressAt      = async (x, y) => { await page .mouse .move (x, y); await frames (3); await page .mouse .down (); };
   releaseAt    = () => page .mouse .up ();
   dragPath     = async (from, to, steps, hold) =>
   {
      await page .mouse .move (from [0], from [1]);
      await page .mouse .down ();
      await page .mouse .move (to [0], to [1], { steps });

      if (hold)
         await page .waitForTimeout (hold);

      await page .mouse .up ();
   };
   wheelAt = async (x, y, dy) => { await page .mouse .move (x, y); await page .mouse .wheel (0, dy); };
}

const facts = webdriver
   ? { base: base .href, browser: page .browserName, browserVersion: page .browserVersion, driver: "webdriver", console: "recorded after each navigation (webdriver.mjs)" }
   : { base: base .href, browserVersion: browser .version (), channel: channel ?? "playwright chromium", flags: args };

const preview = (fn, arg) => page .evaluate (fn, arg);

async function waitStatus (re, timeout = 30000)
{
   await page .waitForFunction (source => new RegExp (source) .test (document .getElementById ("status") .textContent), re .source, { timeout });
   return page .locator ("#status") .textContent ();
}

async function canvasBox ()
{
   const box = await page .locator ("x3d-canvas") .boundingBox ();

   return { x: box .x + 1, y: box .y + 1 }; // inside the 1px border
}

// A screenshot of the canvas as RGBA rows; saved as evidence.
async function shot (name)
{
   // The splash screen of a load fades out in 200 ms after the world is there (X_ITE's X3DCoreContext): wait until it
   // is hidden and its fade is over, so that the picture is the world.
   await page .waitForFunction (() =>
   {
      const splash = document .querySelector ("x3d-canvas") .shadowRoot ?.querySelector (".x_ite-private-splash-screen");

      return !splash || (splash .classList .contains ("x_ite-private-hidden") && getComputedStyle (splash) .opacity === "0");
   },
   null, { timeout: 10000 }) .catch (() => { });
   await page .evaluate (() => new Promise (r => requestAnimationFrame (() => requestAnimationFrame (r))));

   const png  = await page .locator ("x3d-canvas") .screenshot ();
   const file = `${String (++ step) .padStart (2, "0")}-${name}.png`;

   await writeFile (join (out, file), png);

   const data = await decode (png .toString ("base64"));

   // The screenshot is in device pixels (2 per CSS pixel on a Retina display): x and y are CSS pixels of the canvas.
   const scale = data .w / (W + 2);
   const at    = (x, y) => { const i = (Math .round ((y + 1) * scale) * data .w + Math .round ((x + 1) * scale)) * 4; return data .px .slice (i, i + 3); };

   // Pixels that differ from the background.
   let fg = 0;

   for (let y = 0; y < H; y += 4)
      for (let x = 0; x < W; x += 4)
         if (at (x, y) .some ((v, k) => Math .abs (v - BG [k]) > 12)) ++ fg;

   return { file, at, fg: fg / ((W / 4) * (H / 4)), region (x0, y0, x1, y1)
   {
      const colors = [ ];

      for (let y = y0; y < y1; y += 2) for (let x = x0; x < x1; x += 2) colors .push (at (x, y));

      return colors;
   } };
}

const isBg   = c => c .every ((v, k) => Math .abs (v - BG [k]) <= 12);
const vec    = text => String (text) .trim () .split (/\s+/) .map (Number);
const dist   = (a, b) => Math .hypot (... a .map ((v, k) => v - b [k]));
const frames = n => page .evaluate (n => new Promise (r => { let i = 0; const f = () => ++ i >= n ? r () : requestAnimationFrame (f); requestAnimationFrame (f); }), n);

async function drag (from, to, { steps = 12, hold = 0 } = { })
{
   const o = await canvasBox ();

   await dragPath ([ o .x + from [0], o .y + from [1] ], [ o .x + to [0], o .y + to [1] ], steps, hold);
   await frames (3);
}

const field = (node, name) => preview (([ node, name ]) => String (globalThis .X_ITE_R_PREVIEW .nodes [node] ?.[name] ?? ""), [ node, name ]);

// 3. The page.
await page .goto (new URL ("preview/index.html", base) .href);

await check ("page starts: WebGPU engine and first world", async () =>
{
   const status = await waitStatus (/^(loaded|error|X_ITE_R could not)/, 60000);
   const webgpu = await page .locator ("#webgpu") .textContent ();

   facts .webgpu  = webgpu;
   facts .version = await page .locator ("#version") .textContent ();
   facts .build   = await page .locator ("#build") .textContent ();
   facts .userAgent = await preview (() => navigator .userAgent);
   facts .adapter = await preview (async () =>
   {
      const a = await navigator .gpu ?.requestAdapter ();
      return a ? { vendor: a .info .vendor, architecture: a .info .architecture, device: a .info .device, description: a .info .description, isFallbackAdapter: a .info .isFallbackAdapter ?? a .isFallbackAdapter } : null;
   });

   return { pass: /^started/ .test (webgpu) && /^loaded .*vrml97-basic\.wrl/ .test (status), detail: { webgpu, status } };
});

await check ("version and build commit shown", async () =>
{
   return { pass: /^X_ITE_R 0\.1\.0$/ .test (facts .version) && /^[0-9a-f]{40} /.test (facts .build), detail: `${facts .version} / ${facts .build}` };
});

await check ("VRML97 world renders (red, blue, green shapes)", async () =>
{
   const s = await shot ("vrml97-basic");
   const red = s .at (175, 240), blue = s .at (465, 240), green = s .at (320, 100);

   return { pass: red [0] > 1.5 * red [2] && red [0] > 80 && blue [2] > 1.5 * blue [0] && green [1] > 1.3 * green [0], detail: { red, blue, green, foreground: s .fg .toFixed (3), file: s .file } };
});

await check ("TouchSensor: isActive while pressed, touchTime after", async () =>
{
   const o = await canvasBox ();

   await pressAt (o .x + 175, o .y + 240);
   await frames (3);
   const active = await field ("Touch", "isActive");
   await releaseAt ();
   await frames (3);
   const after = await field ("Touch", "isActive"), time = Number (await field ("Touch", "touchTime"));

   return { pass: active === "true" && after === "false" && time > 0, detail: { active, after, touchTime: time } };
});

await check ("PlaneSensor: drag moves the blue box", async () =>
{
   await drag ([ 465, 240 ], [ 525, 200 ]);
   const t = vec (await field ("Drag", "translation_changed"));
   const s = await shot ("planesensor");

   return { pass: t [0] > 0.5 && t [1] > 0.3, detail: { translation_changed: t, file: s .file } };
});

await check ("TimeSensor: fraction and the cone's rotation advance", async () =>
{
   const read = () => preview (() => ({ f: Number (globalThis .X_ITE_R_PREVIEW .nodes .Clock .fraction_changed), r: String (globalThis .X_ITE_R_PREVIEW .browser .currentScene .getNamedNode ("Spinner") .rotation) }));
   const a = await read ();
   await page .waitForTimeout (700);
   const b = await read ();

   return { pass: a .f !== b .f && a .r !== b .r, detail: { before: a, after: b } };
});

await check ("JavaScript API: Material.diffuseColor change is drawn", async () =>
{
   await preview (() => globalThis .X_ITE_R_PREVIEW .setColor ("0.15 0.8 0.2"));
   await frames (3);
   const value = await field ("ColorTarget", "diffuseColor");
   const s = await shot ("color-green");
   const c = s .at (175, 240);

   return { pass: c [1] > 1.5 * c [0] && /^0\.15 0\.8 0\.2/ .test (value), detail: { diffuseColor: value, pixel: c, file: s .file } };
});

await check ("EXAMINE: drag turns the view, wheel zooms", async () =>
{
   const p0 = vec (await field ("Viewer", "position_changed"));
   await drag ([ 320, 430 ], [ 440, 430 ]);
   await page .waitForTimeout (400);
   const p1 = vec (await field ("Viewer", "position_changed"));
   const o  = await canvasBox ();
   await wheelAt (o .x + 320, o .y + 430, -400);
   await page .waitForTimeout (600);
   const p2 = vec (await field ("Viewer", "position_changed"));
   const s  = await shot ("examine");

   return { pass: dist (p0, p1) > 0.5 && Math .abs (dist (p2, [ 0, 0, 0 ]) - dist (p1, [ 0, 0, 0 ])) > 0.2, detail: { start: p0, rotated: p1, zoomed: p2, file: s .file } };
});

// World selection through the page's own control.
await check ("world selection control loads Navigation", async () =>
{
   await page .selectOption ("#world", "worlds/navigation.wrl");
   const status = await waitStatus (/^(loaded|error)/);
   const viewers = await page .locator ("#viewer option") .allTextContents ();
   const s = await shot ("navigation");

   return { pass: /^loaded .*navigation\.wrl/ .test (status) && viewers .join () === "EXAMINE,WALK,FLY" && s .fg > 0.2, detail: { status, viewers, foreground: s .fg .toFixed (3), file: s .file } };
});

for (const [ name, type ] of [ [ "NavWalk", "WALK" ], [ "NavFly", "FLY" ] ])
{
   await check (`${type}: dragging up moves the viewer forward`, async () =>
   {
      await page .selectOption ("#viewer", name);
      await frames (3);
      const p0 = vec (await field ("Viewer", "position_changed"));
      await drag ([ 320, 300 ], [ 320, 200 ], { steps: 6, hold: 1200 });
      await page .waitForTimeout (300);
      const p1 = vec (await field ("Viewer", "position_changed"));
      const s  = await shot (type .toLowerCase ());

      return { pass: p1 [2] < p0 [2] - 0.5, detail: { before: p0, after: p1, file: s .file } };
   });
}

await check ("X3D (XML) world renders, PhysicalMaterial field set, texture loads", async () =>
{
   const ok = await preview (() => globalThis .X_ITE_R_PREVIEW .loadWorld ("worlds/x3d-basic.x3d"));
   await frames (5);
   await preview (() => globalThis .X_ITE_R_PREVIEW .setColor ("0.15 0.8 0.2"));
   await page .waitForTimeout (500);
   const s = await shot ("x3d-basic");
   const pyramid = s .at (320, 250), sphere = s .at (146, 240), stripes = s .region (470, 210, 520, 270) .filter (c => !isBg (c));
   const orange = stripes .some (c => c [0] > 150 && c [2] < 90), white = stripes .some (c => c [0] > 150 && c [2] > 150);
   const tex = (await getResponses ()) .find (r => r .url .endsWith ("/preview/textures/stripes.png"));

   return { pass: ok && !isBg (pyramid) && sphere [1] > sphere [0] && orange && white && tex ?.status === 200,
            detail: { pyramid, sphere, orange, white, texture: tex, file: s .file } };
});

await check ("Inline, EXTERNPROTO and relative texture URLs load and render", async () =>
{
   const ok = await preview (() => globalThis .X_ITE_R_PREVIEW .loadWorld ("worlds/resources.wrl"));
   await frames (5);
   const s = await shot ("resources");
   const want = [ "/preview/worlds/parts/inline-part.wrl", "/preview/worlds/protos/colored-box.wrl", "/preview/textures/checker8.png", "/preview/textures/stripes.png" ];
   const all  = await getResponses ();
   const got  = want .map (w => all .find (r => new URL (r .url) .pathname .endsWith (w)));
   const center = s .region (300, 220, 340, 260);
   const yellow = center .some (c => c [0] > 150 && c [1] > 120 && c [2] < 90), navy = center .some (c => c [2] > 1.5 * c [0] && c [2] > 60);
   const inline = s .region (130, 225, 165, 255) .filter (c => !isBg (c)) .length;
   const proto  = s .at (494, 240);
   const state  = await preview (() => { const p = globalThis .X_ITE_R_PREVIEW .browser .currentScene .externprotos [0]; return { loadState: p .loadState, complete: globalThis .X3D .X3DConstants .COMPLETE_STATE }; });

   return { pass: ok && got .every (r => r ?.status === 200) && yellow && navy && inline > 20 && proto [0] > 120 && proto [2] > 120 && proto [1] < 0.7 * proto [0] && state .loadState === state .complete,
            detail: { requests: got .map ((r, k) => `${want [k]} ${r ?.status}`), checker: { yellow, navy }, inlinePixels: inline, proto, externproto: state, file: s .file } };
});

await check ("failed URL: error shown, previous world kept, next world loads", async () =>
{
   const before = await preview (() => globalThis .X_ITE_R_PREVIEW .browser .currentScene .worldURL);
   const ok     = await preview (() => globalThis .X_ITE_R_PREVIEW .loadWorld ("worlds/missing-on-purpose.wrl"));
   const status = await page .locator ("#status") .textContent ();
   const kept   = await preview (() => globalThis .X_ITE_R_PREVIEW .browser .currentScene .worldURL);
   const s1     = await shot ("failed-url");
   const next   = await preview (() => globalThis .X_ITE_R_PREVIEW .loadWorld ("worlds/vrml97-basic.wrl"));
   await frames (5);
   const s2     = await shot ("after-failure");

   return { pass: !ok && /^error/ .test (status) && kept === before && s1 .fg > 0.05 && next && !isBg (s2 .at (175, 240)),
            detail: { status, kept, files: [ s1 .file, s2 .file ] } };
});

await check ("page reload starts the engine again", async () =>
{
   await page .reload ();
   const status = await waitStatus (/^(loaded|error|X_ITE_R could not)/, 60000);
   const s = await shot ("reload");

   return { pass: /^loaded/ .test (status) && !isBg (s .at (175, 240)), detail: { status, file: s .file } };
});

await check ("dispose: the browser stops without page errors", async () =>
{
   await page .flushConsole ?.();
   consoleLines .push (... (page .consoleLines ?? [ ]) .splice (0));
   const errors = consoleLines .filter (l => l .startsWith ("[pageerror]")) .length;
   await page .click ("#dispose");
   await page .waitForTimeout (500);
   const status = await page .locator ("#status") .textContent ();
   await page .flushConsole ?.();
   consoleLines .push (... (page .consoleLines ?? [ ]) .splice (0));
   const after  = consoleLines .filter (l => l .startsWith ("[pageerror]")) .length;

   return { pass: /disposed/ .test (status) && after === errors, detail: { status } };
});

// Console: the only errors expected are those of the missing world.
await check ("console: no unexplained errors", async () =>
{
   await page .flushConsole ?.();
   consoleLines .push (... (page .consoleLines ?? [ ]) .splice (0));
   const expected = /missing-on-purpose\.wrl|status of 404/;
   const errors   = consoleLines .filter (l => /^\[(error|pageerror)\]/ .test (l));
   const unexplained = errors .filter (l => !expected .test (l)) .map (l => l .split ("\n") [0]);

   return { pass: unexplained .length === 0, detail: { errors: errors .length, unexplained } };
});

await writeFile (join (out, "console.txt"), consoleLines .join ("\n") + "\n");

if (pointerLines .length)
   await writeFile (join (out, "pointer.txt"), pointerLines .join ("\n") + "\n");
await writeFile (join (out, "results.json"), JSON .stringify ({ facts, results, responses: await getResponses () .catch (() => [ ]) }, null, 1) + "\n");

if (webdriver)
   await page .close ();
else
{
   await browser .close ();
   await helperBrowser .close ();
}

const failed = results .filter (r => !r .pass);

console .log (`\nfacts: ${JSON .stringify (facts)}`);
console .log (`checks: ${results .length} run, ${results .length - failed .length} passed, ${failed .length} failed`);
process .exit (failed .length ? 1 : 0);
