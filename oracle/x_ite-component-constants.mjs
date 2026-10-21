/**
 * REA batch 09 cloud: the X3DConstants each component of the pinned X_ITE adds when it is loaded (its node types, in the order its
 * module registers them). The built-in components are in X_ITE's main script and registered at start-up; the external ones
 * (assets/components/*Component.js) when a world or a Script needs them, after their dependencies. A Script sees the numbers
 * (X3DConstants, SFNode.getNodeType ()), which therefore depend on what the world loaded.
 *
 *   node oracle/x_ite-component-constants.mjs oracle/x_ite-component-constants.json
 *
 * One fresh page (x_ite-pin/src/nodes.html) per component: its dependencies are loaded first, then the component, and the
 * constants that appeared in between are its own. Headless; nothing is loaded from the network.
 *
 * Not recorded: the order in which the external components of one profile register when a world loads them together. X3DBrowser
 * .loadComponents imports them at once (Promise.all) and they register as their modules finish loading; three recordings of
 * Immersive, Interactive, MedicalInterchange and Full gave three different orders (2026-10-20). The port numbers them in the
 * order X_ITE requests them.
 */
import { createServer } from "node:http";
import { readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
const { chromium } = createRequire (join (ROOT, "../../../x_ite-tests/")) ("playwright");

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json" };

const [ outArg ] = process .argv .slice (2);

const server = createServer (async (req, res) =>
{
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

async function withPage (f)
{
   const context = await browser .newContext ();

   await context .route (url => !url .href .startsWith (base), route => route .abort ());

   const page = await context .newPage ();

   await page .goto (`${base}/x_ite-pin/src/nodes.html`);
   await page .waitForFunction (() => window .ready, null, { timeout: 30000 });

   try { return await page .evaluate (f .fn, f .arg); }
   finally { await context .close (); }
}

const components = await withPage ({ fn: () =>
{
   const browser = document .getElementById ("c") .browser;

   return Array .from (browser .getSupportedComponents (), c => ({ name: c .name, external: c .external, dependencies: Array .from (c .dependencies) }));
} });

const result = { components, constants: { } };

for (const { name, external } of components)
{
   if (!external)
      continue;

   result .constants [name] = await withPage ({ arg: name, fn: async name =>
   {
      const browser      = document .getElementById ("c") .browser;
      const X3DConstants = window [Symbol .for ("X_ITE.X3D")] .X3DConstants;
      const component    = browser .getSupportedComponents () .get (name);

      for (const dependency of component .dependencies)
         await browser .loadComponents (dependency);

      const before = new Set (Object .keys (X3DConstants));

      await browser .loadComponents (name);

      return Object .entries (X3DConstants) .filter (([key]) => !before .has (key)) .map (([key]) => key);
   } });
}

await browser .close ();
server .close ();

await writeFile (outArg, JSON .stringify (result, null, 1));
