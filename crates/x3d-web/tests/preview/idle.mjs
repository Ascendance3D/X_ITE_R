// The idle benchmark of the Browser Preview: the preview page shows a world and nobody touches it. For each world it
// records, over --seconds (default 20):
//
//   * the requestAnimationFrame intervals: median, p99, max, and the dropped frames (an interval longer than 1.5 times
//     the median: the browser left out at least one display frame);
//   * the main-thread time of all requestAnimationFrame callbacks of one display frame (engine frame, preview panel);
//   * the stalls: every interval over 50 ms, with its time;
//   * long tasks (PerformanceObserver "longtask", Chromium only) and the JS heap growth (performance.memory, Chromium).
//
//   node tests/preview/idle.mjs <base URL> [--browser chromium|firefox|webkit] [--channel chrome|msedge]
//                               [--webgpu-flags] [--headed] [--backend auto|webgpu|webgl2] [--seconds 20] [--out FILE]
//
// The numbers are a measurement, not a check: the script always exits 0 when the page loads.

import { chromium, firefox, webkit } from "playwright";
import { writeFileSync } from "node:fs";

const argv    = process .argv .slice (2);
const opt     = name => { const i = argv .indexOf (name); return i >= 0 ? argv [i + 1] : undefined; };
const base    = new URL (argv .find (a => /^https?:/ .test (a)) ?? "http://127.0.0.1:8000/");
const kind    = opt ("--browser") ?? "chromium";
const channel = opt ("--channel");
const backend = opt ("--backend") ?? "auto";
const seconds = Number (opt ("--seconds") ?? 20);
const out     = opt ("--out");
const worlds  = (opt ("--worlds") ?? "worlds/vrml97-basic.wrl,worlds/navigation.wrl,worlds/resources.wrl,worlds/x3d-basic.x3d") .split (",");
const flags   = "--enable-unsafe-webgpu --enable-features=Vulkan --use-angle=vulkan --ignore-gpu-blocklist --enable-gpu" .split (" ");
const args    = kind === "chromium" && argv .includes ("--webgpu-flags") ? flags : [ ];

// The package root: a base without a trailing slash would lose its last path segment in new URL ("preview/...", base).
if (!base .pathname .endsWith ("/"))
   base .pathname += "/";

const browser = await { chromium, firefox, webkit } [kind] .launch ({ channel, headless: !argv .includes ("--headed"), args });
const context = await browser .newContext ({ viewport: { width: 1280, height: 1000 } });

// Every requestAnimationFrame callback is timed; the sum per display frame is the main-thread cost of that frame.
await context .addInitScript (() =>
{
   const raf = window .requestAnimationFrame .bind (window);

   window .__idle = { frame: 0, stamps: [ ], cost: [ ], longTasks: [ ] };
   window .requestAnimationFrame = callback => raf (stamp =>
   {
      const begin = performance .now ();

      try { callback (stamp); }
      finally { window .__idle .cost [window .__idle .frame] = (window .__idle .cost [window .__idle .frame] ?? 0) + performance .now () - begin; }
   });

   const tick = stamp => { ++ window .__idle .frame; window .__idle .stamps .push (stamp); raf (tick); };

   raf (tick);

   try
   {
      new PerformanceObserver (list => { for (const e of list .getEntries ()) window .__idle .longTasks .push (e .duration); })
         .observe ({ type: "longtask", buffered: true });
   }
   catch { }
});

const page = await context .newPage ();

await page .goto (new URL (`preview/index.html${backend === "auto" ? "" : "?backend=" + backend}`, base) .href);
await page .waitForFunction (() => /^loaded/ .test (document .getElementById ("status") .textContent), null, { timeout: 120000 });

const graphics = await page .locator ("#webgpu") .textContent ();
const results  = [ ];

for (const world of worlds)
{
   await page .evaluate (url => X_ITE_R_PREVIEW .loadWorld (url), world);
   await page .waitForFunction (() => /^loaded/ .test (document .getElementById ("status") .textContent), null, { timeout: 120000 });
   await page .waitForTimeout (2000);

   const r = await page .evaluate (ms => new Promise (resolve =>
   {
      const idle  = window .__idle;
      const f0    = idle .frame, s0 = idle .stamps .length, l0 = idle .longTasks .length;
      const heap0 = performance .memory ?.usedJSHeapSize;

      setTimeout (() =>
      {
         const stamps = idle .stamps .slice (s0);
         const gaps   = stamps .slice (1) .map ((s, i) => s - stamps [i]) .sort ((a, b) => a - b);
         const cost   = idle .cost .slice (f0 + 1, idle .frame - 1) .filter (c => c !== undefined) .sort ((a, b) => a - b);
         const q      = (a, p) => a .length ? +a [Math .min (a .length - 1, Math .floor (a .length * p))] .toFixed (2) : null;
         const median = q (gaps, 0.5);

         resolve ({
            frames: stamps .length,
            fps: +(stamps .length / (ms / 1000)) .toFixed (1),
            intervalMs: { median, p99: q (gaps, 0.99), max: q (gaps, 1) },
            dropped: gaps .filter (g => g > 1.5 * median) .length,
            // Every interval over 50 ms: [ ms since the measurement started, length in ms ].
            stalls: stamps .slice (1) .map ((s, i) => [ Math .round (stamps [i] - stamps [0]), Math .round (s - stamps [i]) ]) .filter (([ , g ]) => g > 50),
            costMs: { median: q (cost, 0.5), p95: q (cost, 0.95), p99: q (cost, 0.99), max: q (cost, 1) },
            longTasks: idle .longTasks .slice (l0) .map (d => Math .round (d)),
            heapGrowthMB: heap0 === undefined ? null : +((performance .memory .usedJSHeapSize - heap0) / 1e6) .toFixed (2),
         });
      },
      ms);
   }),
   seconds * 1000);

   results .push ({ world, ...r });
   console .log (world, JSON .stringify (r));
}

const summary = { browser: kind, channel: channel ?? null, version: browser .version (), headed: argv .includes ("--headed"), backend, graphics, seconds, results };

if (out)
   writeFileSync (out, JSON .stringify (summary, null, 1));

console .log (JSON .stringify ({ browser: summary .browser, channel: summary .channel, version: summary .version, graphics }));

await browser .close ();
