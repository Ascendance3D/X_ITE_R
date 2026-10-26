/**
 * Web browser lane 01 oracle: run the browser API case (crates/x3d-web/tests/browser/api_case.mjs) against the
 * pinned X_ITE in headless Chrome and record the values and two canvas screenshots (before and after the field
 * update). crates/x3d-web/tests/browser/api.test.mjs runs the same case against X_ITE_R and compares.
 *
 *   node oracle/x_ite-web-api.mjs <out_dir> [--runs 5]
 *
 * Writes <out_dir>/api-baseline.json (the values of the first run, the world's sha256, the Chrome version and the
 * WebGL renderer) and <out_dir>/before.png, after.png. Every run must give byte-identical values; the run fails
 * otherwise. The screenshots of each run are compared too and their differences recorded.
 *
 * The page is served from memory at a path inside x_ite-pin/src/ (X_ITE resolves its assets/ from the page URL), so
 * nothing is written into the pinned source and its digest (tools/candidate.lock) does not move. Needs
 * oracle/x_ite-pin/node_modules (see oracle/README.md, "Recording from a worktree") and Playwright from XITE_TESTS
 * (default ../../x_ite-tests next to the repository). Headless only; all non-local requests are aborted.
 */
import { createServer } from "node:http";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { phaseLoad, phaseUpdate, phaseFailure } from "../crates/x3d-web/tests/browser/api_case.mjs";

const ROOT  = normalize (join (fileURLToPath (import.meta.url), "../.."));
const TESTS = process .env .XITE_TESTS ?? join (ROOT, "../../x_ite-tests");
const { chromium } = createRequire (join (TESTS, "/")) ("playwright");

const argv   = process .argv .slice (2);
const opt    = (name, def) => { const i = argv .indexOf (name); return i >= 0 ? argv [i + 1] : def; };
const outDir = resolve (argv .find ((a, i) => !a .startsWith ("--") && !argv [i - 1] ?.startsWith ("--")));
const RUNS   = Number (opt ("--runs", "5"));

const WORLD = "crates/x3d-web/web/example/box.wrl";
const PAGE  = "/oracle/x_ite-pin/src/__x_ite_r_web_api_01__.html";

// The page of the case: X_ITE's defaults except those that would hide the canvas or add overlays.
const HTML = `<!DOCTYPE html>
<meta charset="utf-8">
<style>html, body { margin: 0; padding: 0; background: #fff; }</style>
<!-- The libraries x_ite-pin/src/shot.html loads before X_ITE. -->
<script src="../node_modules/libtess/libtess.cat.js"></script>
<script src="../node_modules/@create3000/libgif/libgif.js"></script>
<script src="../node_modules/apng-js/lib/index.js"></script>
<x3d-canvas splashScreen="false" contextMenu="false" notifications="false" timings="false" contentScale="1"
   style="display:block;width:400px;height:300px"></x3d-canvas>
<script type="module">
await import ("./x_ite.js");
window .X3D_NS = window [Symbol .for ("X_ITE.X3D")];
window .ready = true;
</script>
`;

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css",
   ".json": "application/json", ".png": "image/png", ".wasm": "application/wasm" };

const server = createServer (async (req, res) =>
{
   const pathname = decodeURIComponent (new URL (req .url, "http://x") .pathname);

   if (pathname === PAGE)
   {
      res .writeHead (200, { "Content-Type": "text/html", "Cache-Control": "no-store" });
      return res .end (HTML);
   }

   const path = normalize (join (ROOT, pathname));

   if (!path .startsWith (ROOT)) { res .writeHead (403); return res .end (); }

   try
   {
      const body = await readFile (path);
      res .writeHead (200, { "Content-Type": TYPES [extname (path) .toLowerCase ()] ?? "model/vrml", "Cache-Control": "no-store" });
      res .end (body);
   }
   catch { res .writeHead (404, { "Cache-Control": "no-store" }); res .end (); }
});

await new Promise (r => server .listen (0, "127.0.0.1", r));

const base    = `http://127.0.0.1:${server .address () .port}`;
const browser = await chromium .launch ({
   executablePath: "/usr/bin/google-chrome",
   headless: true,
   args: [ "--headless=new", "--no-sandbox", "--use-gl=angle", "--use-angle=gl", "--enable-gpu", "--ignore-gpu-blocklist" ],
});

async function runOnce ()
{
   const context = await browser .newContext ({ viewport: { width: 400, height: 300 }, deviceScaleFactor: 1 });

   await context .route (url => !url .href .startsWith (base), route => route .abort ());

   const page = await context .newPage ();
   const console_ = [ ];

   page .on ("console", m => console_ .push (`${m .type ()}: ${m .text () .replaceAll (base, "http://oracle") .split ("\n") [0]}`));
   page .on ("pageerror", e => console_ .push (`pageerror: ${e .message}`));

   try
   {
      await page .goto (`${base}${PAGE}`);
      await page .waitForFunction (() => window .ready, null, { timeout: 30000 })
         .catch (error => { throw new Error (`${error .message}\n${console_ .join ("\n")}`); });

      const load    = await page .evaluate (phaseLoad, `${base}/${WORLD}`);
      const before  = await page .locator ("x3d-canvas") .screenshot ();
      const update  = await page .evaluate (phaseUpdate);
      const after   = await page .locator ("x3d-canvas") .screenshot ();
      const failure = await page .evaluate (phaseFailure, `${base}/crates/x3d-web/web/example/no-such-world.wrl`);
      const gl      = await page .evaluate (() =>
      {
         const context = document .querySelector ("x3d-canvas") .browser .getContext ();
         const ext     = context .getExtension ("WEBGL_debug_renderer_info");

         return ext ? context .getParameter (ext .UNMASKED_RENDERER_WEBGL) : context .getParameter (context .RENDERER);
      });

      return { values: { load, update, failure }, before, after, gl, console: console_ };
   }
   finally
   {
      await context .close ();
   }
}

const runs = [ ];

for (let i = 0; i < RUNS; ++ i)
   runs .push (await runOnce ());

const chrome = browser .version ();

await browser .close ();
server .close ();

const sha = data => createHash ("sha256") .update (data) .digest ("hex");
const json = runs .map (r => JSON .stringify (r .values));

if (json .some (j => j !== json [0]))
{
   console .error ("FAIL: the runs recorded different values");
   json .forEach ((j, i) => console .error (`run ${i}: ${j}`));
   process .exit (1);
}

await mkdir (outDir, { recursive: true });
await writeFile (join (outDir, "before.png"), runs [0] .before);
await writeFile (join (outDir, "after.png"), runs [0] .after);
await writeFile (join (outDir, "api-baseline.json"), JSON .stringify ({
   pinned: (await readFile (join (ROOT, "oracle/x_ite-pin/PINNED_COMMIT"), "utf8")) .trim (),
   world: WORLD,
   worldSha256: sha (await readFile (join (ROOT, WORLD))),
   chrome,
   gl: runs [0] .gl,
   runs: RUNS,
   identicalValues: true,
   screenshots: runs .map (r => ({ before: sha (r .before), after: sha (r .after) })),
   console: runs [0] .console,
   values: runs [0] .values,
}, null, 1) + "\n");

console .log (`${RUNS} runs, identical values; before ${sha (runs [0] .before) .slice (0, 12)}, after ${sha (runs [0] .after) .slice (0, 12)}`);
