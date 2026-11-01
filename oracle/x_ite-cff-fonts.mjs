/**
 * REA issue 74 oracle: OpenType-CFF fonts through the pinned X_ITE (opentype.js 2.0.0). Records, for the
 * synthetic fonts of oracle/synthetic-rea-b24-cff (authored by oracle/tools/gen_cff_fonts.py): font metrics,
 * kerning, the glyph path commands (`Glyph.getPath (0, 0, 1)`), the libtess triangles of PolygonText, the Text
 * layout arrays, a FontLibrary route, and what happens for a bare CFF file and for garbage.
 *
 *   node oracle/x_ite-cff-fonts.mjs oracle/synthetic-rea-b24-cff-baseline.json
 *
 * Every layout loads a fresh font (a `?n` query string disables X_ITE's font cache), because opentype.js
 * replaces a glyph's advanceWidth with the charstring width when it builds the glyph's path.
 * crates/x3d-text/tests/rea_b24_cff.rs compares against the JSON.
 */
import { createServer } from "node:http";
import { readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { checkOutput } from "./check-output.mjs";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
const { chromium } = createRequire (join (ROOT, "../../../x_ite-tests/")) ("playwright");
const OUT = resolve (process .argv [2] ?? "oracle/synthetic-rea-b24-cff-baseline.json");

checkOutput (OUT);

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json", ".woff2": "font/woff2" };
const DIR   = "/synthetic-rea-b24-cff";

const CHARS = "ABOSiHFQÉfWV Z";

// name: [ string[], FontStyle fields, Text fields ]
const LAYOUTS = {
   all:         [ [ "ABOSiHFQ", "ÉfWV Z" ], "", "" ],
   kern:        [ [ "ABAB BA OS", "AB" ], "", "" ],
   quirk:       [ [ "WVWV", "AWV V" ], "", "" ],
   justify_mid: [ [ "AB OS", "HiF", "QÉ" ], 'justify [ "MIDDLE" "MIDDLE" ]', "" ],
   size_spacing: [ [ "ABO", "SiH" ], "size 1.5 spacing 0.7", "" ],
   vertical:    [ [ "ABO", "SiH" ], "horizontal FALSE size 0.9", "" ],
   rtl:         [ [ "ABO", "SiHW" ], "leftToRight FALSE", "" ],
   length:      [ [ "ABOS", "WV" ], "", "length [ 3 1 ]" ],
   maxextent:   [ [ "ABOS HFQ" ], "", "maxExtent 4" ],
   notdef:      [ [ "AZЖB" ], "", "" ],
};

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
   const logs = [ ];

   page .on ("console", m => { logs .push (`${m .type ()}: ${m .text () .slice (0, 300) .split (base) .join ("BASE")}`); if (process .env .TEXT_DEBUG) console .log ("[page]", m .text () .slice (0, 300)); });
   page .on ("pageerror", e => console .log ("[pageerror]", String (e) .slice (0, 300)));

   await page .goto (`${base}/x_ite-pin/src/shot.html`);
   await page .waitForFunction (() => window .ready === true, null, { timeout: 30000 });

   const SETUP = ({ CHARS }) =>
   {
      const browser = document .getElementById ("c") .browser;
      const arr     = a => Array .from (a .getValue ? a .getValue () : a);
      const sleep   = ms => new Promise (r => setTimeout (r, ms));
      let   counter = 0;

      async function make (strings, fs, textFields, scenePrefix, setup)
      {
         const str   = strings .map (s => JSON .stringify (s)) .join (" ");
         const scene = await browser .createX3DFromString (`${scenePrefix}${setup}\nDEF T Text { string [ ${str} ] ${textFields} fontStyle DEF F FontStyle { ${fs} } }`);
         const node  = scene .getNamedNode ("T") .getValue ();
         const fnode = scene .getNamedNode ("F") .getValue ();

         for (let i = 0; i < 200 && fnode .checkLoadState () < 2; ++ i)
            await sleep (25);

         return { scene, node, fnode };
      }

      const VRML = "#VRML V2.0 utf8\n";
      const X3DV = "#X3D V4.1 utf8\nPROFILE Full\n";

      function fresh (url)
      {
         return `${url}?n=${++ counter}`;
      }

      window .fontInfo = async function (url)
      {
         const { node, fnode } = await make ([ "x" ], `family ${JSON .stringify (fresh (url))}`, "", VRML, "");
         const font = fnode .getFont ();
         const info = { loaded: !! font, loadState: fnode .checkLoadState () };

         if (font)
         {
            info .unitsPerEm = font .unitsPerEm;
            info .ascender   = font .ascender;
            info .descender  = font .descender;
            info .numGlyphs  = font .numGlyphs;
            info .hasGpos    = !! font .tables .gpos;
            info .hasKern    = !! font .tables .kern;
            info .hasCff     = !! font .tables .cff;
            info .glyphNames = Array .from ({ length: font .numGlyphs }, (_, i) => font .glyphs .get (i) .name);
            info .kerning    = { };

            for (const a of CHARS)
               for (const b of CHARS)
               {
                  const k = font .getKerningValue (font .charToGlyph (a), font .charToGlyph (b));

                  if (k)
                     info .kerning [a + b] = k;
               }

            info .glyphs = { };

            for (const ch of CHARS)
            {
               const glyph         = font .charToGlyph (ch);
               const advanceBefore = glyph .advanceWidth;
               const geo           = node .textGeometry .getGlyphGeometry (font, glyph, 1);
               const v             = [ ];

               for (const p of geo)
                  v .push (p .x, p .y);

               info .glyphs [ch] = {
                  index: glyph .index,
                  advanceBefore,
                  advanceAfter: glyph .advanceWidth,
                  xMin: glyph .xMin ?? null,
                  commands: glyph .getPath (0, 0, 1) .commands,
                  rawCommands: glyph .path .commands,
                  vertices: v,
               };
            }
         }

         return info;
      };

      window .layout = async function (strings, fs, textFields, url, mode)
      {
         let fnode, node;

         if (mode === "library")
         {
            ({ node, fnode } = await make (strings, `family "CffLib"${fs ? " " + fs : ""}`, textFields, X3DV,
               `FontLibrary { family "CffLib" url ${JSON .stringify (fresh (url))} }`));
         }
         else
         {
            ({ node, fnode } = await make (strings, `family ${JSON .stringify (fresh (url))}${fs ? " " + fs : ""}`, textFields, VRML, ""));
         }

         node .rebuild ();

         return {
            strings, fontStyle: fs, text: textFields, mode,
            loadState: fnode .checkLoadState (),
            vertices:  arr (node .getVertices ()),
            texCoords: node .getMultiTexCoords () .length ? arr (node .getMultiTexCoords () [0]) : [ ],
            normals:   arr (node .getNormals ()),
            min: Array .from (node .getMin ()),
            max: Array .from (node .getMax ()),
            origin:     arr (node ._origin),
            textBounds: arr (node ._textBounds),
            lineBounds: Array .from (node ._lineBounds) .map (v => arr (v)),
         };
      };

      // A FontLibrary that loads `url`: the load state of the library node and whether the family is found.
      window .failing = async function (url)
      {
         const { scene, fnode } = await make ([ "A" ], `family "CffLib"`, "", X3DV,
            `DEF L FontLibrary { family "CffLib" url ${JSON .stringify (fresh (url))} }`);

         for (let i = 0; i < 40; ++ i)
            await sleep (25);

         return { libraryLoadState: scene .getNamedNode ("L") .getValue () .checkLoadState (), loaded: !! fnode .getFont () };
      };
   };

   await page .evaluate (SETUP, { CHARS });

   const result = { fonts: { }, layouts: { }, failing: { } };
   const url    = `${base}${DIR}/cff-basic.otf`;

   result .fonts ["cff-basic"] = await page .evaluate (u => window .fontInfo (u), url);

   for (const [ name, [ strings, fs, textFields ] ] of Object .entries (LAYOUTS))
      result .layouts [name] = await page .evaluate (([ a, b, c, u ]) => window .layout (a, b, c, u, "family"), [ strings, fs, textFields, url ]);

   result .layouts ["library_all"] = await page .evaluate (([ a, b, c, u ]) => window .layout (a, b, c, u, "library"), [ LAYOUTS .all [0], "", "", url ]);
   result .layouts ["library_kern"] = await page .evaluate (([ a, b, c, u ]) => window .layout (a, b, c, u, "library"), [ LAYOUTS .kern [0], "size 2", "", url ]);

   for (const file of [ "cff-bare.cff", "cff-basic.otf" ])
   {
      logs .length = 0;
      result .failing [file] = await page .evaluate (u => window .failing (u), `${base}${DIR}/${file}`);
      result .failing [file] .console = [ ... logs ];
   }

   const json = { pinned: "oracle/x_ite-pin (X_ITE 16.5.0, opentype.js 2.0.0)", browser: browser .version (), ... result };

   await writeFile (OUT, JSON .stringify (json) + "\n");
   console .log (`${Object .keys (result .layouts) .length} layouts`);
}
finally
{
   await browser .close ();
   server .close ();
}
