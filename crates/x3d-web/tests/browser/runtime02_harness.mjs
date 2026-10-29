// The shared part of the browser runtime 02 tests of X_ITE_R's <x3d-canvas> (runtime02*.test.mjs): the test server
// and Chromium, a page with the test canvas, and the reference cases compared with the pinned X_ITE 16.5.0 recording in
// oracle/web-browser-02-baseline (oracle/x_ite-web-runtime-02.mjs). The tests are in several files so that CI can run
// them in parallel shards (shards.mjs); each file runs in its own process with its own server and browser.
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)
//
// XR_CASE=<group.case> runs one reference case. XR_WEB_EVIDENCE=<dir> writes this run's values and pictures there.

import { after, before, describe, test } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { compareImages } from "./api_case.mjs";
import { CASE_GROUPS as FALLBACK01_GROUPS } from "./fallback01_case.mjs";
import { CASE_GROUPS as DATAURL01_GROUPS } from "./dataurl01_case.mjs";
import { CASE_GROUPS as DEGENERATE01_GROUPS } from "./degenerate01_case.mjs";
import { CASE_GROUPS as GLTFWORLD01_GROUPS } from "./gltfworld01_case.mjs";
import { CASE_GROUPS as GLTFWORLD02_GROUPS } from "./gltfworld02_case.mjs";
import { CASE_GROUPS as FIELDS01_GROUPS } from "./fields01_case.mjs";
import { CASE_GROUPS as FIELDS02_GROUPS } from "./fields02_case.mjs";
import { CASE_GROUPS as FIELDS03_GROUPS } from "./fields03_case.mjs";
import { CASE_GROUPS as FIELDS04_GROUPS } from "./fields04_case.mjs";
import { CASE_GROUPS as VIEWPOINTS01_GROUPS } from "./viewpoints01_case.mjs";
import { CASE_GROUPS as LOADURL01_GROUPS } from "./loadurl01_case.mjs";
import { CASE_GROUPS as RUNTIME02_GROUPS, splitPictures } from "./runtime02_case.mjs";
import { startServer } from "./server.mjs";

const CRATE    = normalize (join (fileURLToPath (import.meta.url), "../../.."));
const REPO     = join (CRATE, "../..");
const BASELINE = process .env .XR_BASELINE ?? join (REPO, "oracle/web-browser-02-baseline");
const EVIDENCE = process .env .XR_WEB_EVIDENCE;
export const ONLY     = process .env .XR_CASE;

// The recordings: the runtime 02 cases, the web resource fallback 01 cases (fallback01_case.mjs, recorded with
// `--suite fallback01` into oracle/web-browser-fallback-01-baseline) and the web loadURL messages 01 cases
// (loadurl01_case.mjs, `--suite loadurl01` into oracle/web-browser-loadurl-01-baseline), and the web data URL
// compatibility 01 cases (dataurl01_case.mjs, `--suite dataurl01` into oracle/web-browser-data-url-01-baseline), and
// the degenerate triangle 01 cases (degenerate01_case.mjs, `--suite degenerate01` into
// oracle/web-browser-degenerate-01-baseline), and the glTF world buffers 01 and 02 cases (gltfworld01_case.mjs,
// gltfworld02_case.mjs, `--suite gltfworld01|gltfworld02` into oracle/web-browser-gltf-world-01|02-baseline), and the
// field API 01 cases (fields01_case.mjs, `--suite fields01` into oracle/web-browser-fields-01-baseline), and the field API 02 cases (fields02_case.mjs, `--suite fields02` into
// oracle/web-browser-fields-02-baseline).
const SUITES = {
   runtime02:  { dir: BASELINE, file: "runtime02-baseline.json", groups: RUNTIME02_GROUPS },
   fallback01: { dir: process .env .XR_FALLBACK_BASELINE ?? join (REPO, "oracle/web-browser-fallback-01-baseline"), file: "fallback01-baseline.json", groups: FALLBACK01_GROUPS },
   loadurl01:  { dir: process .env .XR_LOADURL_BASELINE ?? join (REPO, "oracle/web-browser-loadurl-01-baseline"), file: "loadurl01-baseline.json", groups: LOADURL01_GROUPS },
   dataurl01:  { dir: process .env .XR_DATAURL_BASELINE ?? join (REPO, "oracle/web-browser-data-url-01-baseline"), file: "dataurl01-baseline.json", groups: DATAURL01_GROUPS },
   degenerate01: { dir: process .env .XR_DEGENERATE_BASELINE ?? join (REPO, "oracle/web-browser-degenerate-01-baseline"), file: "degenerate01-baseline.json", groups: DEGENERATE01_GROUPS },
   gltfworld01: { dir: process .env .XR_GLTFWORLD_BASELINE ?? join (REPO, "oracle/web-browser-gltf-world-01-baseline"), file: "gltfworld01-baseline.json", groups: GLTFWORLD01_GROUPS },
   gltfworld02: { dir: process .env .XR_GLTFWORLD02_BASELINE ?? join (REPO, "oracle/web-browser-gltf-world-02-baseline"), file: "gltfworld02-baseline.json", groups: GLTFWORLD02_GROUPS },
   fields01: { dir: process .env .XR_FIELDS01_BASELINE ?? join (REPO, "oracle/web-browser-fields-01-baseline"), file: "fields01-baseline.json", groups: FIELDS01_GROUPS },
   fields02: { dir: process .env .XR_FIELDS02_BASELINE ?? join (REPO, "oracle/web-browser-fields-02-baseline"), file: "fields02-baseline.json", groups: FIELDS02_GROUPS },
   fields03: { dir: process .env .XR_FIELDS03_BASELINE ?? join (REPO, "oracle/web-browser-fields-03-baseline"), file: "fields03-baseline.json", groups: FIELDS03_GROUPS },
   viewpoints01: { dir: process .env .XR_VIEWPOINTS01_BASELINE ?? join (REPO, "oracle/web-browser-viewpoints-01-baseline"), file: "viewpoints01-baseline.json", groups: VIEWPOINTS01_GROUPS },
   fields04: { dir: process .env .XR_FIELDS04_BASELINE ?? join (REPO, "oracle/web-browser-fields-04-baseline"), file: "fields04-baseline.json", groups: FIELDS04_GROUPS },
};

for (const suite of Object .values (SUITES))
   suite .reference = JSON .parse (await readFile (join (suite .dir, suite .file), "utf8"));

// Every case group with the suite it belongs to.
const CASE_GROUPS = Object .fromEntries (Object .entries (SUITES) .flatMap (([ name, suite ]) =>
   Object .entries (suite .groups) .map (([ groupName, group ]) => [ groupName, { ... group, suite: name } ])));

// The values where X_ITE_R knowingly differs from X_ITE 16.5.0: case → [ path, X_ITE's value, X_ITE_R's value ].
// The X_ITE value is checked against the recording, so a new recording cannot hide a difference. Each entry is
// listed in web/API.md.
const KNOWN_DIFFERENCES = {
   "nodeOwnership.nodeOwnership": [
      // X_ITE stores a node of another scene (the owner stays that scene); X_ITE_R refuses it, warns, and keeps the field as it was.
      // The pinned SFNode proxy would keep a plain property, so the page reads the node back, but the node has no such field value.
      [ [ "s02", 3 ], "Collision { proxy DEF OT Transform { } }", "Collision { }" ],
      [ [ "s03", 2 ], "Group { children [ DEF G Group { } DEF OT Transform { } DEF T Transform { translation 1 2 3 children [ DEF S Shape { appearance Appearance { material DEF M Material { diffuseColor 0.2 0.4 0.9 } } geometry Box { size 2 3 4 } } USE S ] } ] }", "Group { }" ],
      [ [ "s04", 1 ], 3, 2 ],
      [ [ "s04", 2 ], ["Transform:T", "Group:G", "Transform:OT"], ["Transform:T", "Group:G"] ],
      [ [ "s12" ], ["Group { children [ DEF OT Transform { } Transform { } ] }", "<Group> <Transform DEF='OT'/> <Transform/> </Group>"], ["Group { }", "<Group/>"] ],
      // X_ITE keeps the nodes of a replaced world usable; X_ITE_R refuses their stale handles (API.md).
      [ [ "s09" ], [true, "Transform", "page", "1 2 3"], "Error: X_ITE_R: the node belongs to a world that was replaced." ],
      // s13: the refused value leaves the page's field usable. The operation after it goes on (the page reads the node it
      // stored), and the engine, and so the text form, keeps the value it had.
      [ [ "s13", 0, 1 ], "Group { children [ DEF G Group { } DEF OT Transform { } DEF T Transform { translation 1 2 3 children [ DEF S Shape { appearance Appearance { material DEF M Material { diffuseColor 0.2 0.4 0.9 } } geometry Box { size 2 3 4 } } USE S ] } ] }", "Group { }" ],
      // The refusals of s02, s03, s12 and s13 (the assignment and the push after it) each warn once (s04 pushes into rootNodes and refuses without a warning).
      [ [ "log" ], [], ["warn: X_ITE_R: a node can only be used in the world it belongs to.", "warn: X_ITE_R: a node can only be used in the world it belongs to.", "warn: X_ITE_R: a node can only be used in the world it belongs to.", "warn: X_ITE_R: a node can only be used in the world it belongs to.", "warn: X_ITE_R: a node can only be used in the world it belongs to."] ],
   ],
   "fieldCascade.nested": [
      // X_ITE queues the event of a field set inside a callback behind the route events still waiting: D's route event
      // (2 0 0) overwrites the nested value (9 9 9), and the nested event comes in the same cascade with the value D then holds.
      // X_ITE_R delivers the callbacks after the frame: the nested set goes in the next frame and its value stays.
      [ [ "final", 0 ], "2 0 0", "9 9 9" ],
      [ [ "seq", 5 ], "D|2 0 0|later|0", "D|9 9 9|later|1" ],
   ],
   "input.interrupted": [
      // X_ITE has no touchcancel handler: the TouchSensor stays active. X_ITE_R releases it (pointercancel).
      [ [ "touchCancelled", "LeftTouch.isActive" ], true, false ],
      // X_ITE's mouseup listener is on the document and still gets the release after the canvas is removed;
      // X_ITE_R releases the buttons that are down when the canvas leaves the document.
      [ [ "removed" ], "true", "false" ],
      // X_ITE's release after the removal checks the pointer against the detached canvas (an empty rectangle, so a
      // NaN pointer and no hit) and ends isOver. X_ITE_R's pointer has not moved: it is still over the box.
      [ [ "restored", "LeftTouch.isOver" ], false, true ],
   ],
   "loading.failures": [
      // X_ITE names the blob URL of the image data that does not decode; the Rust engine has none and names the file.
      [ [ "loading-invalid", "console", 0 ],
         "error: ImageTexture: Couldn't load URL 'http://test/garbage/texture.png'. Error: Couldn't load image 'blob:http://test/ID': error.",
         "error: ImageTexture: Couldn't load URL 'http://test/garbage/texture.png'. Error: Couldn't load image 'http://test/garbage/texture.png': error." ],
   ],
   "loading.repeated": [
      // X_ITE fetches once per node (the HTTP cache may answer); X_ITE_R fetches a URL once per world.
      [ [ "texture" ], 2, 1 ],
      [ [ "inline" ], 2, 1 ],
   ],
   "loading.disposedDuringLoad": [
      // X_ITE's loadURL never settles after dispose (); X_ITE_R's rejects.
      [ [ "result" ], "pending", "Loading of X3D file aborted." ],
   ],
   "fallback.content": [
      // X_ITE names the blob URL of the image data that does not decode; the Rust engine has none and names the file.
      [ [ "console", 4 ],
         "warning: ImageTexture: Couldn't load URL 'http://test/garbage/t-first.png', trying URL 2 of 2. Error: Couldn't load image 'blob:http://test/ID': error.",
         "warning: ImageTexture: Couldn't load URL 'http://test/garbage/t-first.png', trying URL 2 of 2. Error: Couldn't load image 'http://test/garbage/t-first.png': error." ],
   ],
   "fallback.gltfFallback": [
      // GLTF2Parser does not remove a failed buffer from the loading objects: X_ITE's world never finishes loading.
      [ [ "result" ], "pending", "resolved" ],
   ],
   "fallback.gltfFailed": [
      [ [ "result" ], "pending", "resolved" ],
   ],
   "fallback.repeated": [
      // X_ITE fetches once per node (the HTTP cache may answer); X_ITE_R fetches a URL once per world.
      [ [ "first" ], 2, 1 ],
      [ [ "second" ], 2, 1 ],
   ],
   "fallback.cancelledBeforeFallback": [
      // A newer loadURL ends the loads of the older world in X_ITE_R. X_ITE's Inline FileLoader of the older world
      // goes on: it reports the held candidate that does not parse and fetches the next one.
      [ [ "console" ], [ "warning: Inline: Couldn't load URL 'http://test/hold/fc/garbage/q-first.wrl', trying URL 2 of 2. Error: Couldn't parse X3D. No suitable file handler found for 'http://test/hold/fc/garbage/q-first.wrl'." ], [ ] ],
      [ [ "served" ],
         [ "/tests/browser/worlds/fallback/held-first.wrl", "/hold/fc/garbage/q-first.wrl", "/tests/browser/worlds/fallback/parts/d.wrl", "/tests/browser/worlds/fallback/parts/q-next.wrl" ],
         [ "/tests/browser/worlds/fallback/held-first.wrl", "/hold/fc/garbage/q-first.wrl", "/tests/browser/worlds/fallback/parts/d.wrl" ] ],
   ],
   "loadURL.disposedWhilePending": [
      // X_ITE's loadURL never settles after dispose (); X_ITE_R's rejects.
      [ [ "first" ], "pending", "Loading of X3D file aborted." ],
      // X_ITE's dispose () fires the shutdown event of the world; X_ITE_R's does not.
      [ [ "events" ], [ "shutdown" ], [ ] ],
   ],
   "font.pending": [
      // X_ITE's FontStyle holds the scene's loadCount until its font file is in; X_ITE_R's scene does not wait for
      // fonts (the FontStyle itself waits for the response).
      [ [ "before", "loaded" ], "pending", "loaded" ],
   ],
   "audio.decodeFallbackHeld": [
      // X_ITE's AudioClip holds the scene's loadCount until a candidate decodes; X_ITE_R's scene does not wait for
      // the decode (the AudioClip itself waits for the next response).
      [ [ "before", "loaded" ], "pending", "loaded" ],
   ],
   "dataURL.worldBadBase64": [
      // X_ITE's fetch of a data URL that does not decode logs Chromium's own network error first; X_ITE_R decodes
      // the URL itself (no request) and gives the same rejection, without that line.
      [ [ "console" ],
         [ "error: Failed to load resource: net::ERR_INVALID_URL", "warning: Couldn't load data URL, trying URL 2 of 2. TypeError: Failed to fetch" ],
         [ "warning: Couldn't load data URL, trying URL 2 of 2. TypeError: Failed to fetch" ] ],
   ],
   "dataURL.imageBadBase64": [
      [ [ "console" ],
         [ "error: Failed to load resource: net::ERR_INVALID_URL", "warning: ImageTexture: Couldn't load data URL, trying URL 2 of 2. TypeError: Failed to fetch" ],
         [ "warning: ImageTexture: Couldn't load data URL, trying URL 2 of 2. TypeError: Failed to fetch" ] ],
   ],
   "dataURL.worldEmpty": [
      // An empty document: GoldenGate of the recording page tries DICOMParser, which needs CharLS (the page does not
      // load it), so X_ITE rejects and loads the next candidate. X_ITE_R's parser takes empty text as an empty world
      // (existing parser behavior, not changed here).
      [ [ "world" ], "fallback/parts/w.wrl", "data" ],
      [ [ "served" ], [ "/tests/browser/worlds/fallback/parts/w.wrl" ], [ ] ],
      [ [ "console" ], [ "warning: Couldn't load data URL, trying URL 2 of 2. ReferenceError: CharLS is not defined" ], [ ] ],
   ],
   // The web data URL compatibility 01 cases show the same known differences as the cases above.
   "dataURLWorld.badBase64": [
      // Chromium's own network error line of X_ITE's fetch (dataURL.worldBadBase64).
      [ [ "console" ],
         [ "error: Failed to load resource: net::ERR_INVALID_URL", "warning: Couldn't load data URL, trying URL 2 of 2. TypeError: Failed to fetch" ],
         [ "warning: Couldn't load data URL, trying URL 2 of 2. TypeError: Failed to fetch" ] ],
   ],
   "dataURLWorld.noComma": [
      [ [ "console" ],
         [ "error: Failed to load resource: net::ERR_INVALID_URL", "warning: Couldn't load data URL, trying URL 2 of 2. TypeError: Failed to fetch" ],
         [ "warning: Couldn't load data URL, trying URL 2 of 2. TypeError: Failed to fetch" ] ],
   ],
   "dataURLWorld.rawHashHeaderUpper": [
      // `#` starts the fragment, so the document is empty (dataURL.worldEmpty): X_ITE's recording page fails in
      // DICOMParser; X_ITE_R's world is empty.
      [ [ "world" ], "fallback/parts/w.wrl", "data" ],
      [ [ "served" ], [ "/tests/browser/worlds/fallback/parts/w.wrl" ], [ ] ],
      [ [ "console" ], [ "warning: Couldn't load data URL, trying URL 2 of 2. ReferenceError: CharLS is not defined" ], [ ] ],
   ],
   "dataURLImage.tabBeforeBase64": [
      // X_ITE names the blob URL of the image data that does not decode; X_ITE_R names the URL.
      [ [ "console" ],
         [ "warning: ImageTexture: Couldn't load data URL, trying URL 2 of 2. Error: Couldn't load image 'blob:<id>': error." ],
         [ "warning: ImageTexture: Couldn't load data URL, trying URL 2 of 2. Error: Couldn't load image 'data:image/gif;%09base64,R0lGODdhAgACAIEAAADIAP///wAAAAAAACwAAAAAAgACAAAIBgABCAQQEAA7': error." ] ],
   ],
   "dataURLImage.badBase64": [
      [ [ "console" ],
         [ "error: Failed to load resource: net::ERR_INVALID_URL", "warning: ImageTexture: Couldn't load data URL, trying URL 2 of 2. TypeError: Failed to fetch" ],
         [ "warning: ImageTexture: Couldn't load data URL, trying URL 2 of 2. TypeError: Failed to fetch" ] ],
   ],
   "dataURLGltf.badBase64": [
      // GLTF2Parser does not remove a failed buffer from the loading objects (fallback.gltfFailed), and Chromium's
      // own network error line of X_ITE's fetch.
      [ [ "result" ], "pending", "resolved" ],
      [ [ "events" ], [ "shutdown" ], [ "shutdown", "initialized", "load" ] ],
      [ [ "console" ],
         [ "error: Failed to load resource: net::ERR_INVALID_URL", "error: Couldn't load data URL. TypeError: Failed to fetch" ],
         [ "error: Couldn't load data URL. TypeError: Failed to fetch" ] ],
   ],
   // The glTF world buffers 01 cases show the same known differences as the cases above.
   "gltfWorld.badBase64": [
      // Chromium's own network error line of X_ITE's fetch (dataURL.worldBadBase64).
      [ [ "console" ],
         [ "error: Failed to load resource: net::ERR_INVALID_URL", "error: Couldn't load data URL. TypeError: Failed to fetch" ],
         [ "error: Couldn't load data URL. TypeError: Failed to fetch" ] ],
   ],
   "gltfWorld.badBase64Inline": [
      // GLTF2Parser does not remove a failed buffer from the loading objects (fallback.gltfFailed), and Chromium's
      // own network error line of X_ITE's fetch (dataURLGltf.badBase64).
      [ [ "result" ], "pending", "resolved" ],
      [ [ "events" ], [ "shutdown" ], [ "shutdown", "initialized", "load" ] ],
      [ [ "console" ],
         [ "error: Failed to load resource: net::ERR_INVALID_URL", "error: Couldn't load data URL. TypeError: Failed to fetch" ],
         [ "error: Couldn't load data URL. TypeError: Failed to fetch" ] ],
   ],
   "gltfWorld.missingInline": [
      // GLTF2Parser does not remove a failed buffer from the loading objects (fallback.gltfFailed).
      [ [ "result" ], "pending", "resolved" ],
      [ [ "events" ], [ "shutdown" ], [ "shutdown", "initialized", "load" ] ],
   ],
   "gltfWorld.fallback": [
      // The same for a world candidate: the failed buffer of the first candidate stays a loading object of X_ITE's
      // browser, so the next candidate becomes the world but its loadURL never finishes (fallback.gltfFallback).
      [ [ "result" ], "pending", "resolved" ],
      [ [ "events" ], [ "shutdown" ], [ "shutdown", "initialized", "load" ] ],
   ],
   "gltfWorldHeld.fallback": [
      // The same as gltfWorld.fallback (fallback.gltfFallback), while the held buffer is in flight and after it came.
      // The next candidate is requested and becomes the world while the buffer is held, in both.
      [ [ "during", "result" ], "pending", "resolved" ],
      [ [ "during", "events" ], [ "shutdown" ], [ "shutdown", "initialized", "load" ] ],
      [ [ "result" ], "pending", "resolved" ],
      [ [ "events" ], [ "shutdown" ], [ "shutdown", "initialized", "load" ] ],
   ],
   "fallback.disposedDuringFallback": [
      // X_ITE's loadURL never settles after dispose (); X_ITE_R's rejects.
      [ [ "result" ], "pending", "Loading of X3D file aborted." ],
      // X_ITE draws a frame after dispose () and throws; X_ITE_R's frame loop has stopped.
      [ [ "errors" ], [ "pageerror: Couldn't create frame buffer." ], [ ] ],
   ],
};

// A test that waits longer has hung (a held response that was never released, a frame that never comes): it fails
// with its name instead of running into the CI job's limit. The slowest test takes 35 s on lavapipe here.
export const CASE_TIMEOUT = 180_000;

// The phone and tablet tests draw full-size framebuffers (1170 x 1800, 2360 x 984) on the CPU: on CI's lavapipe, with
// the 2 CPUs of a private-repository runner, the phone test took from 163 s to 404 s (PR #117). Their cap is 2.5 times
// the slowest run.
export const MOBILE_TIMEOUT = 1_010_000;

// Each test file that imports this module gets one server and one browser, for all of its tests.
export let server;
let browser;

before (async () =>
{
   server  = await startServer (CRATE);
   browser = await chromium .launch ({
      headless: true,
      args: [ "--enable-unsafe-webgpu", "--enable-features=Vulkan,VulkanFromANGLE", "--use-angle=vulkan",
         "--use-vulkan=native", "--ignore-gpu-blocklist", "--enable-gpu" ],
   });

   await logAdapter ();
});

// The WebGPU adapter Chromium chose for this test process, as the page sees it (GPUAdapterInfo): NVIDIA, SwiftShader
// (vendor google, architecture swiftshader) or lavapipe (vendor mesa, device llvmpipe). Only this tells which one ran;
// VK_ICD_FILENAMES does not. Printed once per test file and kept with the evidence.
async function logAdapter ()
{
   const context = await browser .newContext ();

   try
   {
      const page = await context .newPage ();

      await page .goto (`${server .base}/tests/browser/page.html`);

      const info = await page .evaluate (async () =>
      {
         const adapter = await navigator .gpu ?.requestAdapter ({ powerPreference: "high-performance" });
         const i       = adapter ?.info;

         return i ? { vendor: i .vendor, architecture: i .architecture, device: i .device, description: i .description } : null;
      });

      console .log (`WebGPU adapter: ${JSON .stringify (info) ?? "unknown"}`);
      await evidence ("webgpu-adapter.json", JSON .stringify (info, null, 1) + "\n");
   }
   catch (error)
   {
      console .log (`WebGPU adapter: unknown (${error .message})`);
   }
   finally
   {
      await context .close ();
   }
}

after (async () =>
{
   await browser ?.close ();
   await server ?.close ();
});

// A new page with the test canvas in a new context, ready (engine and WebGPU device made).
export async function openPage (contextOptions, { init } = { })
{
   const context = await browser .newContext (contextOptions);
   const allowed = [ server .base, server .foreign ];

   await context .route (url => !allowed .some (origin => url .href .startsWith (origin)), route => route .abort ());

   if (init)
      await context .addInitScript (init);

   const page = await context .newPage ();

   page .log = [ ];
   page .on ("console", m => page .log .push (`${m .type ()}: ${m .text () .split ("\n") [0]}`));
   page .on ("pageerror", e => page .log .push (`pageerror: ${e .message}`));

   await page .goto (`${server .base}/tests/browser/runtime02.html`);
   await page .waitForFunction (() => window .ready || window .startError, null, { timeout: 30000 });

   if (!init)
   {
      const startError = await page .evaluate (() => window .startError);

      assert .equal (startError, undefined, `X_ITE_R did not start: ${startError}\n${page .log .join ("\n")}`);
   }

   return page;
}

// The picture the Rust renderer made, read back from the GPU (WebBrowser.readPixels) as a PNG. Headless Chromium does
// not always composite a WebGPU canvas (no display compositor, e.g. on lavapipe), so a page screenshot could be empty.
export async function picture (page)
{
   const base64 = await page .evaluate (async () =>
   {
      const canvas = document .querySelector ("x3d-canvas");
      const engine = canvas .browser [Symbol .for ("X_ITE_R.engine")];
      const pixels = await engine .readPixels ();
      const { width, height } = canvas .browser .getCanvas ();
      const image  = new OffscreenCanvas (width, height);

      image .getContext ("2d") .putImageData (new ImageData (new Uint8ClampedArray (pixels .buffer), width, height), 0, 0);

      const bytes = new Uint8Array (await (await image .convertToBlob ({ type: "image/png" })) .arrayBuffer ());

      return btoa (Array .from (bytes, b => String .fromCharCode (b)) .join (""));
   });

   return Buffer .from (base64, "base64");
}

const env = () => ({
   base: server .base,
   foreign: server .foreign,
   worlds: `${server .base}/tests/browser/worlds/`,
   server,
   picture,
});

export const frameLoop = page => page .evaluate (() => document .querySelector ("x3d-canvas") .browser [Symbol .for ("X_ITE_R.frameLoop")]);
export const loadURL   = (page, url) => page .evaluate (url => document .querySelector ("x3d-canvas") .browser .loadURL (new window .X3D_NS .MFString (url)), url);

async function evidence (name, data)
{
   if (!EVIDENCE)
      return;

   await mkdir (EVIDENCE, { recursive: true });
   await writeFile (join (EVIDENCE, name), data);
}

// The value at `path`, and a copy of `value` with `replacement` there.
const at = (value, path) => path .reduce ((v, k) => v ?.[k], value);

function withValue (value, path, replacement)
{
   const copy = structuredClone (value);

   path .slice (0, -1) .reduce ((v, k) => v [k], copy) [path .at (-1)] = replacement;
   return copy;
}

// --- reference cases ----------------------------------------------------------------------------------------------

// The reference case groups of each test file. A group in no file, or a file here that does not exist, fails every
// file that imports this module, so no group is left out in silence. Each file named here calls referenceCases.
const GROUP_FILES = {
   input:   "runtime02_input.test.mjs",
   touch:   "runtime02_input.test.mjs",
   scale:   "runtime02_input.test.mjs",
   loading: "runtime02.test.mjs",
   api:     "runtime02.test.mjs",
   fallback: "fallback01.test.mjs",
   loadURL: "loadurl01.test.mjs",
   font:    "loadurl01.test.mjs",
   movie:   "loadurl01.test.mjs",
   audio:   "loadurl01.test.mjs",
   dataURL: "loadurl01.test.mjs",
   dataURLWorld: "dataurl01.test.mjs",
   dataURLImage: "dataurl01.test.mjs",
   dataURLGltf:  "dataurl01.test.mjs",
   degenerateTriangle: "degenerate01.test.mjs",
   gltfWorld: "gltfworld01.test.mjs",
   gltfWorldHeld: "gltfworld02.test.mjs",
   fieldAPI: "fields01.test.mjs",
   fieldText: "fields02.test.mjs",
   nodeOwnership: "fields03.test.mjs",
   fieldCascade: "fields04.test.mjs",
   viewpointsApi: "viewpoints01.test.mjs",
};

assert .deepEqual (Object .keys (GROUP_FILES) .sort (), Object .keys (CASE_GROUPS) .sort (), "GROUP_FILES must name every case group");

for (const file of new Set (Object .values (GROUP_FILES)))
   assert .ok (existsSync (join (dirname (fileURLToPath (import.meta.url)), file)), `GROUP_FILES: ${file} does not exist`);

// The reference cases of the groups that GROUP_FILES gives to `file`, the name of the calling test file.
export function referenceCases (file)
{
   const groups = Object .entries (CASE_GROUPS) .filter (([ groupName ]) => GROUP_FILES [groupName] === file);

   assert .ok (groups .length, `GROUP_FILES gives no case group to ${file}`);

   for (const [ groupName, group ] of groups)
   {
      const { dir, reference } = SUITES [group .suite];

      describe (`${group .suite} ${groupName} cases, compared with pinned X_ITE`, () =>
      {
         for (const [ name, run ] of Object .entries (group .cases))
         {
            const key = `${groupName}.${name}`;

            test (key, { skip: ONLY && ONLY !== key, timeout: CASE_TIMEOUT }, async () =>
            {
               const page = await openPage (group .context);

               try
               {
                  const pictures = { };
                  const ours     = splitPictures (await run (page, env ()), key, pictures);
                  let   theirs   = reference .values [key];

                  assert .ok (theirs, `no X_ITE recording of ${key}`);

                  await evidence (`${key}.json`, JSON .stringify (ours, null, 1) + "\n");

                  for (const [ path, xite, xiteR ] of KNOWN_DIFFERENCES [key] ?? [ ])
                  {
                     assert .deepEqual (at (theirs, path), xite, `${key}: the recording no longer has the known difference at ${path .join (".")}`);
                     theirs = withValue (theirs, path, xiteR);
                  }

                  assert .deepEqual (ours, theirs, `${key}\n${page .log .join ("\n")}`);

                  // The pictures: the same background, silhouette and colors as X_ITE's (the thresholds of runtime 01).
                  for (const [ file, data ] of Object .entries (pictures))
                  {
                     const recorded = await readFile (join (dir, file));
                     const m        = await page .evaluate (compareImages, [ recorded .toString ("base64"), data .toString ("base64") ]);

                     await evidence (file, data);
                     await evidence (`${file}.metrics.json`, JSON .stringify (m, null, 1) + "\n");

                     assert .deepEqual (m .backgroundB, m .backgroundA, `${file}: background`);
                     assert .ok (m .silhouetteIoU >= 0.97, `${file}: silhouette IoU ${m .silhouetteIoU}`);
                     assert .ok (m .meanAbsDiff <= 8, `${file}: mean color difference ${m .meanAbsDiff}`);
                  }

                  assert .deepEqual (page .log .filter (line => line .startsWith ("pageerror")), [ ], "page errors");
               }
               finally
               {
                  await page .context () .close ();
               }
            });
         }
      });
   }
}

// --- lifecycle and mobile emulation -------------------------------------------------------------------------------

export const DESKTOP = { viewport: { width: 400, height: 340 }, deviceScaleFactor: 1 };

// A phone-sized emulation: no proof of a mobile browser or a mobile GPU (see web/API.md, "Mobile support").
export const PHONE = { viewport: { width: 390, height: 844 }, deviceScaleFactor: 3, isMobile: true, hasTouch: true };

// A mobile page's viewport: without `<meta name="viewport">` a mobile browser lays the page out 980 px wide.
export const mobileViewport = page => page .evaluate (async () =>
{
   const meta = Object .assign (document .createElement ("meta"), { name: "viewport", content: "width=device-width, initial-scale=1" });

   document .head .append (meta);

   for (let i = 0; i < 3; ++ i)
      await new Promise (requestAnimationFrame);
});

// A tablet-sized emulation, portrait first.
export const TABLET = { viewport: { width: 820, height: 1180 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true };
