/**
 * AudioDestination.mediaDeviceID oracle (issue #81): the pinned X_ITE in headless Chromium. One node,
 * its mediaDeviceID set to "", the first enumerated output id, an unknown id, "default", and back to "".
 * Records the node's audio element `sinkId` after each step, the devices the page can enumerate, the
 * console output and the unhandled rejections. X_ITE swallows a setSinkId rejection
 * (X3DSoundContext.startAudioElements: `.catch (Function .prototype)`), so no log is expected.
 *
 *   node oracle/x_ite-audio-sink.mjs <out.json> [--runs N]
 *
 * With --runs N the whole recording is repeated in fresh browsers and the run fails unless all N
 * recordings are byte-identical. Loads the pinned src/dump.html page unedited (as x_ite-formats.mjs).
 * Prepare a worktree as in oracle/README.md "Recording from a worktree".
 */
import { createServer } from "node:http";
import { readFile, readdir, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { extname, join, normalize, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
const { chromium } = createRequire (join (process .env .XITE_TESTS ?? join (ROOT, "../../../x_ite-tests"), "/")) ("playwright");

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json" };


const args   = process .argv .slice (2);
const runsAt = args .indexOf ("--runs");
const runs   = runsAt >= 0 ? Number (args .splice (runsAt, 2) [1]) : 1;
const [ outArg ] = args;

const server = createServer (async (req, res) =>
{
   const path = normalize (join (ROOT, decodeURIComponent (new URL (req .url, "http://x") .pathname)));

   if (!path .startsWith (ROOT)) { res .writeHead (403); return res .end (); }

   try
   {
      const body = await readFile (path);
      res .writeHead (200, { "Content-Type": TYPES [extname (path)] ?? "application/octet-stream", "Cache-Control": "no-store" });
      res .end (body);
   }
   catch { res .writeHead (404, { "Cache-Control": "no-store" }); res .end (); }
});

await new Promise (r => server .listen (0, "127.0.0.1", r));

const base = `http://127.0.0.1:${server .address () .port}`;


async function record ()
{
   const browser = await chromium .launch ({ headless: true });
   const context = await browser .newContext ();
   await context .route (url => !url .href .startsWith (base), route => route .abort ());

   const page = await context .newPage ();
   const log  = [ ];
   page .on ("console", m => log .push (`${m .type ()}: ${m .text ()}`));
   page .on ("pageerror", e => log .push (`pageerror: ${e .message}`));
   await page .goto (`${base}/x_ite-pin/src/dump.html`);
   await page .waitForFunction (() => window .ready, null, { timeout: 30000 });

   const result = await page .evaluate (async () =>
   {
      const wait = ms => new Promise (r => setTimeout (r, ms));
      const rejections = [ ];
      window .addEventListener ("unhandledrejection", e => rejections .push (`${e .reason ?.name}: ${e .reason ?.message}`));

      const browser = document .getElementById ("c") .browser;
      await browser .replaceWorld (await browser .createX3DFromString ("#X3D V4.0 utf8\nPROFILE Full\nDEF D AudioDestination { }\n"));
      const node = browser .currentScene .getNamedNode ("D");
      await wait (300);

      let devices = [ ];
      try { devices = (await navigator .mediaDevices .enumerateDevices ()) .filter (d => d .kind === "audiooutput") .map (d => ({ id: d .deviceId, label: d .label })); }
      catch (e) { devices = `${e ?.name}: ${e ?.message}`; }

      const steps = [ ];
      const sinkId = () => node .getValue () .audioElement .sinkId;
      const step = async (name, id) =>
      {
         if (id !== undefined) node .mediaDeviceID = id;
         await wait (400);
         steps .push ({ name, mediaDeviceID: node .mediaDeviceID, sinkId: sinkId (), hasSetSinkId: typeof node .getValue () .audioElement .setSinkId });
      };

      await step ("initial");
      await step ("empty", "");
      if (Array .isArray (devices) && devices .length) await step ("first-output", devices [0] .id);
      await step ("default", "default");
      await step ("unknown", "no-such-device-0000");
      await step ("empty-after-unknown", "");
      await step ("unknown-again", "no-such-device-0000");
      await step ("unknown-then-default", "default");

      return { devices, steps, rejections, maxChannelCount: node .maxChannelCount };
   });

   const userAgent = await page .evaluate (() => navigator .userAgent);
   await browser .close ();
   return { pinned: (await readFile (join (ROOT, "x_ite-pin/PINNED_COMMIT"), "utf8")) .trim (), userAgent, ... result, log: log .filter (l => !/^(info|log): |GL Driver Message/ .test (l)) };
}

const texts = [ ];

for (let i = 0; i < runs; ++ i)
{
   texts .push (JSON .stringify (await record (), null, 1));
   console .log (`run ${i + 1}: ${texts [i] .length} bytes`);
}

server .close ();

if (texts .some (t => t !== texts [0]))
{
   console .error ("recordings differ between runs");
   process .exit (1);
}

await writeFile (outArg, texts [0]);
console .log (texts [0]);
console .log (`${runs} identical run(s)`);
