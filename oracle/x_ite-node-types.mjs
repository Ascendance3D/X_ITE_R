/**
 * REA batch 09 cloud: dump the type chain X_ITE gives every node class (X3DBaseNode.getType (): the X3DConstants of the class and
 * of each abstract type it adds, in the order the constructors add them), and X3DConstants as it stands once every component is
 * loaded. A Script sees both: SFNode.getNodeType () and X3DConstants.
 *
 *   node oracle/x_ite-node-types.mjs oracle/x_ite-node-types.json
 *
 * Headless, same server and page (x_ite-pin/src/nodes.html) as x_ite-nodes.mjs. Nothing is loaded from the network.
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
const context = await browser .newContext ();

await context .route (url => !url .href .startsWith (base), route => route .abort ());

const page = await context .newPage ();

await page .goto (`${base}/x_ite-pin/src/nodes.html`);
await page .waitForFunction (() => window .ready, null, { timeout: 30000 });

const result = await page .evaluate (async () =>
{
   const browser      = document .getElementById ("c") .browser;
   const X3DConstants = window [Symbol .for ("X_ITE.X3D")] .X3DConstants;
   const constants    = () => Object .entries (X3DConstants);

   // What a world that loads nothing extra sees.
   await browser .createX3DFromString ("#X3D V4.0 utf8\nPROFILE Core\n");

   const initialConstants = constants ();
   const failedComponents = [ ];

   for (const c of browser .getSupportedComponents ())
   {
      try { await browser .loadComponents (c .name); }
      catch (error) { failedComponents .push (`${c .name}: ${error}`); }
   }

   const scene = await browser .createX3DFromString ("#X3D V4.0 utf8\nPROFILE Core\n");
   const types = { };

   for (const ConcreteNode of browser .getConcreteNodes ())
   {
      const node = new ConcreteNode (scene);

      types [ConcreteNode .typeName] = Array .from (node .getType (), t => X3DConstants [t]);
   }

   return {
      failedComponents,
      initialConstants,
      constants: constants (),
      abstractNodes: Array .from (browser .getAbstractNodes (), C => C .typeName),
      types,
   };
});

await browser .close ();
server .close ();

await writeFile (outArg, JSON .stringify (result, null, 1));
