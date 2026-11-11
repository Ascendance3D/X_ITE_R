/**
 * REA node:FontLibrary / sub:text.fonts: CID-keyed CFF and CFF2 fonts through the pinned X_ITE (opentype.js 2.0.0). For every font of
 * oracle/synthetic-rea-t-cff-cid (authored by oracle/tools/gen_cff_cid_fonts.py) it records: whether the font loads, the console text of
 * the load, the font metrics, and per glyph the advance before and after the path is built, `Glyph.getPath (0, 0, 1)` commands, the raw path
 * commands, the libtess triangles of PolygonText (or the exception text where building throws), and the console text of those calls.
 *
 *   node oracle/x_ite-cff-cid-fonts.mjs OUT.json
 *
 * Every font is parsed afresh because opentype.js replaces a glyph's advanceWidth with the
 * charstring width when it builds the glyph's path. crates/x3d-text/tests/rea_t_cff_cid.rs compares against the JSON.
 */
import { createServer } from "node:http";
import { readFile, writeFile, readdir } from "node:fs/promises";
import { createRequire } from "node:module";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { checkOutput } from "./check-output.mjs";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
const { chromium } = createRequire (join (process .env .XITE_TESTS ?? join (ROOT, "../../../x_ite-tests"), "/")) ("playwright");
const OUT = resolve (process .argv [2] ?? "oracle/rea-t-cff-cid-baseline/fonts.json");

checkOutput (OUT);

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json" };
// FONT_DIR (below oracle/) and FONT_CHARS pick another font set (oracle/tools/rea_t_cff_items.py); the defaults record the CID-keyed lane.
const DIR   = process .env .FONT_DIR ?? "/synthetic-rea-t-cff-cid";
const CHARS = process .env .FONT_CHARS ?? "AB OHFWVZ";

const server = createServer (async (req, res) =>
{
   const path = normalize (join (ROOT, decodeURIComponent (new URL (req .url, "http://x") .pathname)));

   if (!path .startsWith (ROOT)) { res .writeHead (403); return res .end (); }

   try
   {
      const body = await readFile (path);
      res .writeHead (200, { "Content-Type": TYPES [extname (path) .toLowerCase ()] ?? "application/octet-stream", "Cache-Control": "no-store" });
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

try
{
   const context = await browser .newContext ({ viewport: { width: 512, height: 512 } });

   await context .route (url => !url .href .startsWith (base), route => route .abort ());

   const page = await context .newPage ();
   let   logs = [ ];

   page .on ("console", m => { logs .push (`${m .type ()}: ${m .text () .slice (0, 400) .split (base) .join ("BASE")}`); if (process .env .TEXT_DEBUG) console .log ("[page]", m .text () .slice (0, 300)); });
   page .on ("pageerror", e => logs .push (`pageerror: ${String (e) .slice (0, 400)}`));

   await page .goto (`${base}/x_ite-pin/src/shot.html`);
   await page .waitForFunction (() => window .ready === true, null, { timeout: 30000 });

   const SETUP = async ({ CHARS }) =>
   {
      const browser  = document .getElementById ("c") .browser;
      const OpenType = await import ("../node_modules/opentype.js/dist/opentype.mjs");
      const scene    = await browser .createX3DFromString ("#VRML V2.0 utf8\nDEF T Text { string \"x\" }");
      const node     = scene .getNamedNode ("T") .getValue ();

      // The parse X_ITE's browser .loadFont does (X3DTextContext.js: OpenType .parse (decompressed)), without the scene around it: a font whose
      // glyph path throws leaves the loading of a whole scene pending (see oracle/rea-t-cff-cid-baseline/cases.txt, tc-novstore).
      window .fontInfo = async function (url)
      {
         const info = { };
         let   font;

         try
         {
            font = OpenType .parse (await (await fetch (url)) .arrayBuffer ());
         }
         catch (error)
         {
            info .parseError = String (error);
            return info;
         }

         info .unitsPerEm     = font .unitsPerEm;
         info .ascender       = font .ascender;
         info .descender      = font .descender;
         info .numGlyphs      = font .numGlyphs;
         info .outlinesFormat = font .outlinesFormat;
         info .tables         = Object .keys (font .tables) .sort ();
         info .isCIDFont      = !! font .isCIDFont;
         info .glyphs         = { };

         for (const ch of CHARS)
         {
            const glyph         = font .charToGlyph (ch);
            const advanceBefore = glyph .advanceWidth;
            const g             = { index: glyph .index, advanceBefore };

            try
            {
               const geo = node .textGeometry .getGlyphGeometry (font, glyph, 1);
               const v   = [ ];

               for (const p of geo)
                  v .push (p .x, p .y);

               g .commands    = glyph .getPath (0, 0, 1) .commands;
               g .rawCommands = glyph .path .commands;
               g .vertices    = v;
            }
            catch (error)
            {
               g .error = String (error);
            }

            g .advanceAfter = glyph .advanceWidth;
            info .glyphs [ch] = g;
         }

         return info;
      };
   };

   await page .evaluate (SETUP, { CHARS });

   const names  = (await readdir (join (ROOT, DIR .slice (1)))) .filter (f => f .endsWith (".otf") && (!process .env .FONT_ONLY || f .includes (process .env .FONT_ONLY))) .sort ();
   const result = { };

   for (const file of names)
   {
      logs = [ ];
      result [file] = await page .evaluate (u => window .fontInfo (u), `${base}${DIR}/${file}?n=1`);
      result [file] .console = logs .filter (l => ! l .startsWith ("debug:") && ! l .startsWith ("info:"));
   }

   const json = { pinned: "oracle/x_ite-pin (X_ITE 16.5.0, opentype.js 2.0.0)", fonts: result };

   await writeFile (OUT, JSON .stringify (json) + "\n");
   console .log (`${names .length} fonts`);
}
finally
{
   await browser .close ();
   server .close ();
}
