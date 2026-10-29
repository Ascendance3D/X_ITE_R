/**
 * Web browser lane 02 oracle: run the runtime 02 cases (crates/x3d-web/tests/browser/runtime02_case.mjs: input,
 * touch, coordinates, resource loading, scene API) against the pinned X_ITE in headless Chrome and record the values
 * and the pictures. crates/x3d-web/tests/browser/runtime02.test.mjs runs the same cases against X_ITE_R and compares.
 *
 *   node oracle/x_ite-web-runtime-02.mjs <out_dir> [--runs 5] [--only <group.case>] [--suite runtime02|fallback01|loadurl01|dataurl01|degenerate01|gltfworld01|gltfworld02|fields01|fields02|fields03|fields04|routes01|scenecreate01]
 *
 * `--suite fallback01` records the web resource fallback 01 cases (tests/browser/fallback01_case.mjs) instead, into
 * <out_dir>/fallback01-baseline.json; their test is fallback01.test.mjs.
 * `--suite loadurl01` and `--suite dataurl01` record the web loadURL messages 01 and web data URL compatibility 01
 * cases in the same way (<out_dir>/loadurl01-baseline.json, <out_dir>/dataurl01-baseline.json).
 * `--suite degenerate01` records the degenerate triangle 01 cases (<out_dir>/degenerate01-baseline.json).
 * `--suite gltfworld01` records the glTF world buffers 01 cases (<out_dir>/gltfworld01-baseline.json).
 * `--suite gltfworld02` records the glTF world buffers 02 cases (<out_dir>/gltfworld02-baseline.json).
 * `--suite fields01` records the field API 01 cases (<out_dir>/fields01-baseline.json).
 * `--suite fields02` records the field API 02 cases (<out_dir>/fields02-baseline.json).
 * `--suite routes01` records the routes 01 cases (<out_dir>/routes01-baseline.json).
 * `--suite scenecreate01` records the scene creation 01 cases (<out_dir>/scenecreate01-baseline.json).
 *
 * Writes <out_dir>/runtime02-baseline.json (the values of the first run, the Chrome version and the WebGL renderer)
 * and one PNG per picture. Every run must give identical values; the run fails otherwise. The pictures of each run
 * are hashed and recorded, not compared.
 *
 * The server is the one of the tests (tests/browser/server.mjs), with the crate directory as its root. The pinned
 * source is mounted at /x_ite/ and the test worlds also at /x_ite/src/worlds/, so a page-relative URL resolves to
 * the same file in both engines. The page is served from memory; nothing is written into the pinned source and its
 * digest (tools/candidate.lock) does not move. Needs oracle/x_ite-pin/node_modules (see oracle/README.md,
 * "Recording from a worktree") and Playwright from XITE_TESTS (default ../../x_ite-tests next to the repository).
 * Headless only; all non-local requests are aborted.
 */
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { startServer } from "../crates/x3d-web/tests/browser/server.mjs";
import { splitPictures } from "../crates/x3d-web/tests/browser/runtime02_case.mjs";

const ROOT  = normalize (join (fileURLToPath (import.meta.url), "../.."));
const CRATE = join (ROOT, "crates/x3d-web");
const TESTS = process .env .XITE_TESTS ?? join (ROOT, "../../x_ite-tests");
const { chromium } = createRequire (join (TESTS, "/")) ("playwright");

const argv   = process .argv .slice (2);
const opt    = (name, def) => { const i = argv .indexOf (name); return i >= 0 ? argv [i + 1] : def; };
const outDir = resolve (argv .find ((a, i) => !a .startsWith ("--") && !argv [i - 1] ?.startsWith ("--")));
const RUNS   = Number (opt ("--runs", "5"));
const ONLY   = opt ("--only", null);
const SUITE  = opt ("--suite", "runtime02");

if (![ "runtime02", "fallback01", "loadurl01", "dataurl01", "degenerate01", "gltfworld01", "gltfworld02", "fields01", "fields02", "fields03", "fields04", "routes01", "scenecreate01" ] .includes (SUITE))
   throw new Error (`unknown suite ${SUITE}`);

const { CASE_GROUPS } = await import (`../crates/x3d-web/tests/browser/${SUITE}_case.mjs`);

const PAGE = "/x_ite/src/__x_ite_r_web_runtime_02__.html";

// The page of the cases: X_ITE's defaults except those that would hide the canvas or add overlays, and a text field
// outside the canvas (the keyboard case).
const HTML = `<!DOCTYPE html>
<meta charset="utf-8">
<style>html, body { margin: 0; padding: 0; background: #fff; }</style>
<!-- The libraries x_ite-pin/src/shot.html loads before X_ITE. -->
<script src="../node_modules/libtess/libtess.cat.js"></script>
<script src="../node_modules/@create3000/libgif/libgif.js"></script>
<script src="../node_modules/apng-js/lib/index.js"></script>
<!-- Texturing3D (DICOM) libraries, as upstream x_ite.html loads them: createScene loads every component of the Full profile. -->
<script src="../node_modules/CharLS.js/dist/charLS-DynamicMemory-browser.js"></script>
<script src="../node_modules/dicom-parser/dist/dicomParser.js"></script>
<script src="../node_modules/jpeg-js/lib/decoder.js"></script>
<script src="../node_modules/OpenJPEG.js/dist/openJPEG-DynamicMemory-browser.js"></script>
<x3d-canvas splashScreen="false" contextMenu="false" notifications="false" timings="false" contentScale="1"
   style="display:block;width:400px;height:300px"></x3d-canvas>
<input id="outside" style="display:block;width:100px">
<script type="module">
await import ("./x_ite.js");
window .X3D_NS = window [Symbol .for ("X_ITE.X3D")];
window .ready = true;
</script>
`;

const server = await startServer (CRATE, {
   mounts: { "/x_ite/src/worlds/": join (CRATE, "tests/browser/worlds"), "/x_ite/": join (ROOT, "oracle/x_ite-pin") },
   pages: { [PAGE]: HTML },
});

const browser = await chromium .launch ({
   executablePath: "/usr/bin/google-chrome",
   headless: true,
   args: [ "--headless=new", "--no-sandbox", "--use-gl=angle", "--use-angle=gl", "--enable-gpu", "--ignore-gpu-blocklist" ],
});

const env = {
   base: server .base,
   foreign: server .foreign,
   worlds: `${server .base}/tests/browser/worlds/`,
   server,
   picture: page => page .locator ("x3d-canvas") .screenshot (),
};

let gl;

async function runCase (group, name, run)
{
   const context = await browser .newContext (group .context);
   const allowed = [ server .base, server .foreign ];

   await context .route (url => !allowed .some (origin => url .href .startsWith (origin)), route => route .abort ());

   const page = await context .newPage ();

   page .log = [ ];
   page .on ("console", m => page .log .push (`${m .type ()}: ${m .text () .split ("\n") [0]}`));
   page .on ("pageerror", e => page .log .push (`pageerror: ${e .message}`));

   try
   {
      await page .goto (`${server .base}${PAGE}`);
      await page .waitForFunction (() => window .ready, null, { timeout: 30000 })
         .catch (error => { throw new Error (`${error .message}\n${page .log .join ("\n")}`); });

      gl ??= await page .evaluate (() =>
      {
         const context = document .querySelector ("x3d-canvas") .browser .getContext ();
         const ext     = context .getExtension ("WEBGL_debug_renderer_info");

         return ext ? context .getParameter (ext .UNMASKED_RENDERER_WEBGL) : context .getParameter (context .RENDERER);
      });

      return await run (page, env);
   }
   finally
   {
      await context .close ();
   }
}

async function runOnce ()
{
   const values = { }, pictures = { };

   for (const [ groupName, group ] of Object .entries (CASE_GROUPS))
   {
      for (const [ name, run ] of Object .entries (group .cases))
      {
         const key = `${groupName}.${name}`;

         // The key of the tests (XR_CASE): `group.case`.
         if (ONLY && ONLY !== key)
            continue;

         values [key] = splitPictures (await runCase (group, name, run), key, pictures);
      }
   }

   if (!Object .keys (values) .length)
      throw new Error (`no case is named ${ONLY}`);

   return { values, pictures };
}

const runs = [ ];

for (let i = 0; i < RUNS; ++ i)
{
   runs .push (await runOnce ());
   console .log (`run ${i + 1} of ${RUNS}`);
}

const chrome = browser .version ();

await browser .close ();
await server .close ();

const sha  = data => createHash ("sha256") .update (data) .digest ("hex");
const json = runs .map (r => JSON .stringify (r .values));

if (json .some (j => j !== json [0]))
{
   console .error ("FAIL: the runs recorded different values");

   for (const key of Object .keys (runs [0] .values))
   {
      const each = runs .map (r => JSON .stringify (r .values [key]));

      if (each .some (j => j !== each [0]))
         each .forEach ((j, i) => console .error (`${key} run ${i}: ${j}`));
   }

   process .exit (1);
}

await mkdir (outDir, { recursive: true });

for (const [ file, data ] of Object .entries (runs [0] .pictures))
   await writeFile (join (outDir, file), data);

await writeFile (join (outDir, `${SUITE}-baseline.json`), JSON .stringify ({
   pinned: (await readFile (join (ROOT, "oracle/x_ite-pin/PINNED_COMMIT"), "utf8")) .trim (),
   chrome,
   gl,
   runs: RUNS,
   identicalValues: true,
   pictures: Object .fromEntries (Object .keys (runs [0] .pictures) .map (file => [ file, runs .map (r => sha (r .pictures [file])) ])),
   values: runs [0] .values,
}, null, 1) + "\n");

console .log (`${RUNS} runs, identical values, ${Object .keys (runs [0] .values) .length} cases, ${Object .keys (runs [0] .pictures) .length} pictures`);
