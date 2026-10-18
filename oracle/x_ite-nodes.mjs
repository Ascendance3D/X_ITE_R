/**
 * X1/X2 oracle: dump X_ITE's own node, field, component and profile tables from the
 * pinned source, so the Rust port uses X_ITE's definitions (not X3DUOM's) as truth.
 *
 *   node oracle/x_ite-nodes.mjs oracle/x_ite-nodes.json
 *
 * Headless. Same server pattern as x_ite-dump.mjs. Nothing is loaded from the network.
 * Field default values are dumped as raw JS values (full double precision), so Rust can
 * reproduce X_ITE's exact isDefaultValue () comparisons.
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
   const browser = document .getElementById ("c") .browser;

   const num = v => Number .isFinite (v) ? v : String (v);

   function value (f)
   {
      const t = f .getTypeName ();

      switch (t)
      {
         case "SFNode":   return f .getValue () ? "<node>" : null;
         case "MFNode":   return f .length;
         case "SFBool":   return f .getValue ();
         case "SFString": return f .getValue ();
         case "SFInt32":
         case "SFFloat":
         case "SFDouble":
         case "SFTime":   return num (f .getValue ());
         case "SFImage":  return [f .width, f .height, f .comp, ... Array .from (f .array)];
         case "MFImage":  return Array .from (f, i => [i .width, i .height, i .comp, ... Array .from (i .array)]);
         case "MFString": return Array .from (f);
         case "MFBool":   return Array .from (f, v => !! v);
      }

      if (t .startsWith ("SF"))
         return Array .from (f, num);

      return Array .from (f, v => typeof v === "object" ? Array .from (v, num) : num (v));
   }

   function aliases (node)
   {
      const fields = node .getPredefinedFields ();
      const out    = { };

      for (const key of Reflect .ownKeys (fields))
      {
         if (typeof key !== "symbol")
            continue;

         const map = Reflect .get (fields, key);

         if (!(map instanceof Map))
            continue;

         for (const [name, field] of map)
         {
            if (name !== field .getName ())
               out [name] = field .getName ();
         }
      }

      return out;
   }

   // A VRML 2.0 scene loads exactly what a corpus file loads.
   const vrml2 = await browser .createX3DFromString ("#VRML V2.0 utf8\n");
   const vrml2Available = Array .from (browser .getConcreteNodes (), C => C .typeName) .sort ();

   // Then load everything so the table is complete.
   // Component by component: a few external ones need globals dump.html does not load (e.g. CharLS).
   const failedComponents = [ ];

   for (const c of browser .getSupportedComponents ())
   {
      try { await browser .loadComponents (c .name); }
      catch (error) { failedComponents .push (`${c .name}: ${error}`); }
   }

   const x3d = await browser .createX3DFromString ("#X3D V4.0 utf8\nPROFILE Core\n");

   // X3D 3.x scenes: constructors change defaults and aliases for `getSpecificationVersion () <= 3.x`.
   const legacyVersions = ["3.0", "3.1", "3.2", "3.3"];
   const legacyScenes   = [ ];

   for (const v of legacyVersions)
      legacyScenes .push ([v, await browser .createX3DFromString (`#X3D V${v} utf8\nPROFILE Core\n`)]);

   // X3D 4.1: constructors that test `getSpecificationVersion () <= 4.0` (OrthoViewpoint,
   // TextureProjector, TextureProjectorParallel) start differently than in the 4.0 scene above.
   const newerVersions = ["4.1"];

   for (const v of newerVersions)
      legacyScenes .push ([v, await browser .createX3DFromString (`#X3D V${v} utf8\nPROFILE Core\n`)]);

   const nodes = [ ];

   for (const ConcreteNode of browser .getConcreteNodes ())
   {
      const entry = {
         typeName: ConcreteNode .typeName,
         component: ConcreteNode .componentInfo .name,
         componentLevel: ConcreteNode .componentInfo .level,
         containerField: ConcreteNode .containerField,
         from: ConcreteNode .specificationRange .from,
         to: ConcreteNode .specificationRange .to,
      };

      for (const [key, scene] of [["vrml2", vrml2], ["x3d", x3d], ... legacyScenes])
      {
         try
         {
            const node = new ConcreteNode (scene);

            entry [key] = {
               canUserDefinedFields: node .canUserDefinedFields (),
               fields: Array .from (node .getFieldDefinitions (), d => ({
                  accessType: d .getAccessType (),
                  name: d .getName (),
                  type: d .getValue () .getTypeName (),
                  value: value (d .getValue ()),
               })),
               aliases: aliases (node),
               // Instance values right after construction; differs from the definition only if a constructor changed it.
               instance: Array .from (node .getPredefinedFields (), f => [f .getName (), f .getTypeName (), value (f)]),
               units: Object .fromEntries (Array .from (node .getPredefinedFields (), f => [f .getName (), f .getUnit ()]) .filter (([, u]) => u)),
            };
         }
         catch (error)
         {
            entry [key] = { error: String (error) };
         }
      }

      nodes .push (entry);
   }

   const components = Array .from (browser .getSupportedComponents (), c => ({
      name: c .name,
      level: c .level,
      external: !! c .external,
      dependencies: Array .from (c .dependencies ?? [ ]),
   }));

   const profiles = Array .from (browser .getSupportedProfiles (), p => ({
      name: p .name,
      components: Array .from (p .components, c => ({ name: c .name, level: c .level })),
   }));

   return { browserName: browser .name, browserVersion: browser .version, failedComponents, vrml2Available, components, profiles, nodes };
});

await browser .close ();
server .close ();

result .pinned = (await readFile (join (ROOT, "x_ite-pin/PINNED_COMMIT"), "utf8")) .trim ();

await writeFile (outArg, JSON .stringify (result, null, 1));
console .log (`${result .nodes .length} node types, ${result .vrml2Available .length} available to VRML 2.0, ${result .components .length} components, ${result .profiles .length} profiles`);
