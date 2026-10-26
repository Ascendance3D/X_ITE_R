// Browser integration tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU.
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)
//
// The API case (api_case.mjs) is compared with the pinned X_ITE 16.5.0 recording in
// oracle/web-browser-01-baseline (oracle/x_ite-web-api.mjs). XR_WEB_EVIDENCE=<dir> writes this run's values, metrics
// and screenshots there. Chromium needs a Vulkan driver for WebGPU: on a machine without a GPU, Mesa's lavapipe
// (package mesa-vulkan-drivers).

import { after, before, describe, test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { compareImages, phaseFailure, phaseLoad, phaseUpdate } from "./api_case.mjs";

const CRATE    = normalize (join (fileURLToPath (import.meta.url), "../../.."));
const REPO     = join (CRATE, "../..");
const BASELINE = join (REPO, "oracle/web-browser-01-baseline");
const EVIDENCE = process .env .XR_WEB_EVIDENCE;
const WORLD    = "web/example/box.wrl";

// The X_ITE values every X_ITE_R run must give, from the pinned recording.
const reference = JSON .parse (await readFile (join (BASELINE, "api-baseline.json"), "utf8"));

// The parts of the case where X_ITE_R knowingly differs from X_ITE 16.5.0. Each entry is listed in web/API.md.
const KNOWN_DIFFERENCES = { };

const TYPES = { ".html": "text/html", ".mjs": "text/javascript", ".js": "text/javascript", ".wasm": "application/wasm" };

let server, base, browser;
const requests = [ ];

before (async () =>
{
   server = createServer (async (req, res) =>
   {
      const path = normalize (join (CRATE, decodeURIComponent (new URL (req .url, "http://x") .pathname)));

      requests .push (req .url);

      if (!path .startsWith (CRATE)) { res .writeHead (403); return res .end (); }

      try
      {
         const body = await readFile (path);
         res .writeHead (200, { "Content-Type": TYPES [extname (path)] ?? "model/vrml", "Cache-Control": "no-store" });
         res .end (body);
      }
      catch { res .writeHead (404, { "Cache-Control": "no-store" }); res .end (); }
   });

   await new Promise (r => server .listen (0, "127.0.0.1", r));
   base = `http://127.0.0.1:${server .address () .port}`;

   browser = await chromium .launch ({
      headless: true,
      args: [ "--enable-unsafe-webgpu", "--enable-features=Vulkan,VulkanFromANGLE", "--use-angle=vulkan",
         "--use-vulkan=native", "--ignore-gpu-blocklist", "--enable-gpu" ],
   });
});

after (async () =>
{
   await browser ?.close ();
   server ?.close ();
});

// A new page with the test canvas, ready (engine and WebGPU device made). Collects console output and page errors.
async function openPage ()
{
   const context = await browser .newContext ({ viewport: { width: 400, height: 300 }, deviceScaleFactor: 1 });

   await context .route (url => !url .href .startsWith (base), route => route .abort ());

   const page = await context .newPage ();

   page .log = [ ];
   page .on ("console", m => page .log .push (`${m .type ()}: ${m .text ()}`));
   page .on ("pageerror", e => page .log .push (`pageerror: ${e .message}`));

   await page .goto (`${base}/tests/browser/page.html`);
   await page .waitForFunction (() => window .ready || window .startError, null, { timeout: 30000 });

   const startError = await page .evaluate (() => window .startError);

   assert .equal (startError, undefined, `X_ITE_R did not start: ${startError}\n${page .log .join ("\n")}`);
   return page;
}

const frameLoop = page => page .evaluate (() => document .querySelector ("x3d-canvas") ?.browser [Symbol .for ("X_ITE_R.frameLoop")]);

// The picture the Rust renderer made, read back from the GPU (WebBrowser.readPixels) as a PNG. Headless Chromium does
// not always composite a WebGPU canvas (no display compositor, e.g. on lavapipe), so a page screenshot could be empty.
async function screenshot (page)
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

async function evidence (name, data)
{
   if (!EVIDENCE)
      return;

   await mkdir (EVIDENCE, { recursive: true });
   await writeFile (join (EVIDENCE, name), typeof data === "string" || Buffer .isBuffer (data) ? data : JSON .stringify (data, null, 1) + "\n");
}

describe ("x3d-canvas", () =>
{
   test ("the world of the recording is the example world", async () =>
   {
      const sha = createHash ("sha256") .update (await readFile (join (CRATE, WORLD))) .digest ("hex");

      assert .equal (reference .worldSha256, sha);
      assert .equal (reference .pinned, "34cdf9f97");
   });

   test ("1. the browser module initializes on WebGPU", async () =>
   {
      const page = await openPage ();
      const info = await page .evaluate (() => document .querySelector ("x3d-canvas") .browser [Symbol .for ("X_ITE_R.engine")] .adapterInfo);

      assert .match (info, /BrowserWebGpu/);
      assert .deepEqual (page .log .filter (l => /error/i .test (l)), [ ]);
      await evidence ("adapter.txt", info + "\n");
      await page .context () .close ();
   });

   test ("2. <x3d-canvas> is a registered custom element with a browser", async () =>
   {
      const page = await openPage ();
      const r = await page .evaluate (() =>
      {
         const canvas = document .querySelector ("x3d-canvas");

         return {
            defined:   typeof customElements .get ("x3d-canvas"),
            tag:       Object .prototype .toString .call (canvas),
            browser:   typeof canvas .browser,
            same:      window .X3D_NS .getBrowser () === canvas .browser,
            canvas:    canvas .shadowRoot .querySelector ("canvas") === canvas .browser .getCanvas (),
            name:      canvas .browser .name,
            getName:   canvas .browser .getName (),
         };
      });

      assert .deepEqual (r, { defined: "function", tag: "[object X3DCanvasElement]", browser: "object", same: true,
         canvas: true, name: "X_ITE_R", getName: "X_ITE_R" });
      await page .context () .close ();
   });

   test ("3-9. the API case gives the values of pinned X_ITE, from the Rust engine", async () =>
   {
      requests .length = 0;

      const page    = await openPage ();
      const load    = await page .evaluate (phaseLoad, `${base}/${WORLD}`);
      const shot1   = await screenshot (page);
      const update  = await page .evaluate (phaseUpdate);
      const shot2   = await screenshot (page);
      const failure = await page .evaluate (phaseFailure, `${base}/web/example/no-such-world.wrl`);
      const values  = { load, update, failure };

      // 4. The scene the API reads is the Rust engine's: the wasm object answers for it, and no X_ITE code was fetched.
      const owned = await page .evaluate (() =>
      {
         const engine = document .querySelector ("x3d-canvas") .browser [Symbol .for ("X_ITE_R.engine")];
         const id     = engine .namedNode ("BoxMaterial");

         return {
            engine:    engine .constructor .name,
            typeName:  engine .typeName (id),
            field:     engine .field (id, "diffuseColor"),
            rootNodes: Array .from (engine .rootNodes (), n => engine .typeName (Number (n))),
            xite:      Symbol .for ("X_ITE.X3D") in window,
         };
      });

      await evidence ("values.json", values);
      await evidence ("before.png", shot1);
      await evidence ("after.png", shot2);
      await evidence ("requests.json", requests);
      await evidence ("console.json", page .log);

      assert .deepEqual (owned, { engine: "WebBrowser", typeName: "Material", field: [ "SFColor", "inputOutput", "1 0 0" ],
         rootNodes: [ "Viewpoint", "Transform" ], xite: false });
      assert .ok (requests .every (r => !/x_ite\.(js|mjs|min)/ .test (r) || r .includes ("x_ite_r.mjs")), requests .join ("\n"));
      assert .ok (requests .some (r => r .endsWith ("x3d_web_bg.wasm")));

      // 3, 5, 6, 8, 9: every recorded value, except a known difference.
      for (const [ phase, expected ] of Object .entries (reference .values))
      {
         for (const [ key, value ] of Object .entries (expected))
         {
            const name = `${phase}.${key}`;

            if (name in KNOWN_DIFFERENCES)
               continue;

            assert .deepEqual (values [phase] [key], value, name);
         }
      }

      // 9. The pictures: the same silhouette and close colors before and after the update.
      const pictures = { };

      for (const [ name, ours ] of [ [ "before", shot1 ], [ "after", shot2 ] ])
      {
         const theirs = await readFile (join (BASELINE, `${name}.png`));

         pictures [name] = await page .evaluate (compareImages, [ theirs .toString ("base64"), ours .toString ("base64") ]);
      }

      await evidence ("image-comparison.json", pictures);

      for (const [ name, m ] of Object .entries (pictures))
      {
         assert .deepEqual (m .backgroundB, m .backgroundA, `${name}: background`);
         assert .ok (m .silhouetteIoU >= 0.97, `${name}: silhouette IoU ${m .silhouetteIoU}`);
         assert .ok (m .meanAbsDiff <= 8, `${name}: mean color difference ${m .meanAbsDiff}`);
      }

      // 6. The update is visible: red goes up, blue goes down.
      assert .ok (pictures .after .meanColorB [0] > pictures .before .meanColorB [0] + 50);
      assert .ok (pictures .after .meanColorB [2] < pictures .before .meanColorB [2] - 50);

      await page .context () .close ();
   });

   test ("7. removal stops the frame loop, a new connection restarts it, dispose ends it", async () =>
   {
      const page  = await openPage ();
      const count = async () => (await frameLoop (page)) .frames;

      assert .equal ((await frameLoop (page)) .running, true);

      await page .evaluate (() => { window .removed = document .querySelector ("x3d-canvas"); window .removed .remove (); });

      const stopped = await page .evaluate (() => window .removed .browser [Symbol .for ("X_ITE_R.frameLoop")]);

      assert .equal (stopped .running, false);
      await page .waitForTimeout (300);
      assert .equal ((await page .evaluate (() => window .removed .browser [Symbol .for ("X_ITE_R.frameLoop")])) .frames, stopped .frames);

      await page .evaluate (() => document .body .append (window .removed));
      await page .waitForTimeout (300);
      assert .equal ((await frameLoop (page)) .running, true);
      assert .ok ((await count ()) > stopped .frames);

      await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .dispose ());

      const disposed = await frameLoop (page);

      await page .waitForTimeout (300);
      assert .equal (disposed .running, false);
      assert .equal (await count (), disposed .frames);
      assert .equal (await page .evaluate (() => document .querySelector ("x3d-canvas") .browser [Symbol .for ("X_ITE_R.engine")]), null);
      assert .deepEqual (page .log .filter (l => /error/i .test (l)), [ ]);
      await page .context () .close ();
   });

   test ("8. a world that does not parse fires error and rejects loadURL", async () =>
   {
      const page = await openPage ();
      const r    = await page .evaluate (async () =>
      {
         const canvas = document .querySelector ("x3d-canvas");
         const events = [ ];

         for (const name of [ "load", "error", "shutdown" ])
            canvas .addEventListener (name, () => events .push (name));

         const url = URL .createObjectURL (new Blob ([ "#VRML V2.0 utf8\nTransform { children [ Box { " ]));

         try
         {
            await canvas .browser .loadURL (new window .X3D_NS .MFString (url));
            return { events, error: "no error" };
         }
         catch (error)
         {
            return { events, error: error .message, hasWorld: canvas .browser [Symbol .for ("X_ITE_R.engine")] .hasWorld };
         }
      });

      assert .deepEqual (r, { events: [ "error" ], error: "Couldn't load X3D file.", hasWorld: false });
      await page .context () .close ();
   });
});
