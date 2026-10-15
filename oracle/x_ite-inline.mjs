/**
 * R0.3-C oracle: drive the pinned X_ITE frame by frame through Inline reloads and record what it
 * does, step by step.
 *
 *   node oracle/x_ite-inline.mjs <case_dir> <out.json> [case-name ...]
 *
 * <case_dir> holds one <name>.json scenario next to the worlds it names (read in place, never
 * copied). A scenario:
 *
 *   { "world": "main.wrl", "t0": 946684800, "dt": 0.1, "frames": 12,
 *     "actions": [ { "frame": 3, "type": "url", "name": "Widget", "url": ["lib-b.wrl"] }, ... ] }
 *
 * Action types (see inline.html): url, load, autoRefresh, autoRefreshTimeLimit, loadNow, unloadNow,
 * remove (take the Inline out of its Group). `name` is a DEF path: "Mid/Leaf" is the node Leaf in the
 * scene that the Inline Mid loaded.
 * One more is done here: { "type": "publish", "path": "late.wrl", "from": "lib-b.wrl" } makes the
 * file `path` (next to the world) exist from then on, with the content of `from`: a resource that
 * becomes available while the world runs.
 *
 * Step k (k = 1 .. frames) is: apply the actions of frame k at the previous frame's time, settle,
 * advance the virtual clock to t0 + k * dt and traverse one browser frame, settle. "Settle" means no
 * request is in flight and the page's continuations have run, so a fetch started in step k always
 * completes before step k + 1 (the rule of x_ite-anim.mjs's `settle`, and of Runtime::settle).
 * Step 0 is the state after the world has loaded.
 *
 * The result of a step: the console messages logged in it (in order), the state of every named Inline
 * (load state, load, url, the root node types of the scene it shows, its child count, whether its scene
 * is live), and the scene text when it changed. A scenario with "oracle": "skip" is not run (the
 * pinned X_ITE does not finish it); a case that does not settle within 20 s is recorded as a timeout.
 *
 * Headless. All non-local requests are aborted.
 */
import { createServer } from "node:http";
import { readFile, readdir, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { extname, join, normalize, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
// XITE_TESTS: the x_ite-tests checkout whose node_modules holds playwright. The default is right only
// for the main scratch tree; a worktree sets it.
const { chromium } = createRequire (join (process .env .XITE_TESTS ?? join (ROOT, "../../../x_ite-tests"), "/")) ("playwright");

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json" };

const [ caseArg, outArg, ... only ] = process .argv .slice (2);
const caseDir = resolve (caseArg);
const names   = (await readdir (caseDir)) .filter (f => f .endsWith (".json")) .map (f => f .slice (0, -5)) .filter (n => !only .length || only .includes (n)) .sort ();

const overlay = new Map ();

const server = createServer (async (req, res) =>
{
   const published = overlay .get (new URL (req .url, "http://x") .pathname);

   if (published)
   {
      res .writeHead (200, { "Content-Type": "model/vrml", "Cache-Control": "no-store" });
      return res .end (published);
   }

   const path = normalize (join (ROOT, decodeURIComponent (new URL (req .url, "http://x") .pathname)));

   if (!path .startsWith (ROOT)) { res .writeHead (403); return res .end (); }

   try
   {
      const body = await readFile (path);
      res .writeHead (200, { "Content-Type": TYPES [extname (path)] ?? "model/vrml", "Cache-Control": "no-store" });
      res .end (body);
   }
   catch { res .writeHead (404, { "Cache-Control": "no-store" }); res .end (); }
});

await new Promise (r => server .listen (0, "127.0.0.1", r));

const base    = `http://127.0.0.1:${server .address () .port}`;
const browser = await chromium .launch ({ headless: true });
const norm    = s => s .replaceAll (base, "http://oracle");
const results = [ ];

for (const name of names)
{
   overlay .clear ();

   const scenario = JSON .parse (await readFile (join (caseDir, `${name}.json`), "utf8"));
   const t0       = scenario .t0 ?? 946684800;
   const dt       = scenario .dt ?? 0.1;
   const frames   = scenario .frames ?? 1;
   const actions  = scenario .actions ?? [ ];

   if (scenario .oracle === "skip")
   {
      results .push ({ case: name, ok: true, skipped: scenario .skipReason ?? "not run", steps: [ ] });
      console .log (`skip ${name}`);
      continue;
   }

   const context = await browser .newContext ();
   await context .route (url => !url .href .startsWith (base), route => route .abort ());

   const page = await context .newPage ();
   let inFlight = 0;

   page .on ("request",         () => ++ inFlight);
   page .on ("requestfinished", () => -- inFlight);
   page .on ("requestfailed",   () => -- inFlight);

   const yieldTask = () => page .evaluate (() => new Promise (resolve =>
   {
      const channel = new MessageChannel ();
      channel .port1 .onmessage = () => resolve ();
      channel .port2 .postMessage (0);
   }));

   const deadline = Date .now () + 20000;

   const settle = async () =>
   {
      for (let quiet = 0; quiet < 3;)
      {
         if (Date .now () > deadline)
            throw new Error ("oracle timeout");

         while (inFlight > 0 || await page .evaluate (() => window .pendingFetch ()) > 0)
         {
            if (Date .now () > deadline)
               throw new Error ("oracle timeout");

            await new Promise (r => setTimeout (r, 2));
         }

         await yieldTask ();
         await yieldTask ();

         quiet = inFlight > 0 ? 0 : quiet + 1;
      }
   };

   const result = { case: name, ok: false, steps: [ ] };
   let lastVrml = null;

   const record = async k =>
   {
      const snap = await page .evaluate (() => window .snapshot ());
      const logs = await page .evaluate (() => window .drainConsole ());

      const step = {
         k,
         console: logs .map (l => norm (`${l .level}: ${l .text}`)),
         loadCount: snap .loadCount,
         inlines: snap .inlines .map (i => ({ ... i, url: i .url .map (norm) })),
      };

      const vrml = norm (snap .vrml);

      if (vrml !== lastVrml)
         step .vrml = vrml;

      lastVrml = vrml;
      result .steps .push (step);
   };

   try
   {
      await page .goto (`${base}/x_ite-pin/src/inline.html`);
      await page .waitForFunction (() => window .ready, null, { timeout: 30000 });

      const url    = `${base}/${relative (ROOT, join (caseDir, scenario .world)) .split ("/") .map (encodeURIComponent) .join ("/")}`;
      await page .evaluate (([u, t]) => window .beginLoad (u, t), [ url, t0 ]);

      for (let i = 0;; ++ i)
      {
         await settle ();

         const loaded = await page .evaluate (() => window .loaded);

         if (loaded)
         {
            if (!loaded .ok)
               throw new Error (loaded .error);

            break;
         }

         if (i > 500)
            throw new Error ("oracle timeout");

         await page .evaluate (t => window .step (t), t0);
      }

      await settle ();
      await record (0);

      for (let k = 1; k <= frames; ++ k)
      {
         await page .evaluate (n => window .setStep (n), k);

         const prev = t0 + (k - 1) * dt;

         for (const a of actions .filter (a => a .frame === k))
         {
            if (a .type === "publish")
            {
               overlay .set (`/${relative (ROOT, join (caseDir, a .path)) .split ("/") .map (encodeURIComponent) .join ("/")}`, await readFile (join (caseDir, a .from)));
               continue;
            }

            await page .evaluate (ms => window .setDate (ms), prev * 1000);
            await page .evaluate (a => window .act (a), a);
         }

         await settle ();
         await page .evaluate (ms => window .setDate (ms), (t0 + k * dt) * 1000);
         await page .evaluate (t => window .step (t), t0 + k * dt);
         await settle ();
         await record (k);
      }

      result .ok = true;
   }
   catch (e)
   {
      result .error = String (e ?.message ?? e) .split ("\n") [0];
      result .consoleAtError = (await page .evaluate (() => window .drainConsole ()) .catch (() => [ ])) .map (l => norm (`${l .level}: ${l .text}`));
   }

   results .push (result);
   console .log (`${result .ok ? "ok  " : "FAIL"} ${name}${result .ok ? "" : "  " + result .error}`);

   await context .close ();
}

await browser .close ();
server .close ();
await writeFile (outArg, JSON .stringify ({ pinned: (await readFile (join (ROOT, "x_ite-pin/PINNED_COMMIT"), "utf8")) .trim (), results }, null, 1));
