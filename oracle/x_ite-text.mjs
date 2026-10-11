/**
 * X8 text oracle: drive the pinned X_ITE (headless system Chrome, page x_ite-pin/src/shot.html)
 * and record what Text/FontStyle compute: font metrics, kerning, per-glyph triangle output of
 * PolygonText (libtess), and the layout arrays (vertices, texCoords, bounds) of Text nodes.
 *
 *   node oracle/x_ite-text.mjs oracle/x_ite-text.json
 *
 * Every case is synthetic. crates/x3d-text/tests/oracle.rs compares against the JSON.
 */
import { createServer } from "node:http";
import { readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
const { chromium } = createRequire (join (ROOT, "../../../x_ite-tests/")) ("playwright");

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json", ".woff2": "font/woff2" };

const FAMILIES = [ "SERIF", "SANS", "TYPEWRITER" ];
const STYLES   = [ "PLAIN", "ITALIC", "BOLD", "BOLDITALIC" ];

// name: [ string[], FontStyle fields, Text fields ]
const LAYOUTS = {
   default:          [ [ "Hello World" ], "", "" ],
   multi:            [ [ "Hello", "Wide World", "ij" ], "", "" ],
   justify_mid_mid:  [ [ "Hello", "Wide World", "ij" ], 'justify [ "MIDDLE" "MIDDLE" ]', "" ],
   justify_end_end:  [ [ "Hello", "Wide World", "ij" ], 'justify [ "END" "END" ]', "" ],
   justify_first_first: [ [ "Hello", "Wide World", "ij" ], 'justify [ "FIRST" "FIRST" ]', "" ],
   justify_begin_begin: [ [ "Hello", "Wide World", "ij" ], 'justify [ "BEGIN" "BEGIN" ]', "" ],
   justify_end_mid:  [ [ "Hello", "Wide World", "ij" ], 'justify [ "END" "MIDDLE" ]', "" ],
   justify_mid_end:  [ [ "Hello", "Wide World", "ij" ], 'justify [ "MIDDLE" "END" ]', "" ],
   justify_single:   [ [ "Hello", "Wide World", "ij" ], 'justify "MIDDLE"', "" ],
   size_spacing:     [ [ "Hello", "Wide World", "ij" ], "size 0.5 spacing 2", "" ],
   size_spacing_b:   [ [ "Hello", "Wide World", "ij" ], "size 1.5 spacing 0.7", "" ],
   ltr_false:        [ [ "Hello", "Wide World", "ij" ], "leftToRight FALSE", "" ],
   ltr_false_end:    [ [ "Hello", "Wide World", "ij" ], 'leftToRight FALSE justify [ "END" "BEGIN" ]', "" ],
   ttb_false:        [ [ "Hello", "Wide World", "ij" ], "topToBottom FALSE", "" ],
   ttb_false_mid:    [ [ "Hello", "Wide World", "ij" ], 'topToBottom FALSE justify [ "MIDDLE" "MIDDLE" ]', "" ],
   vertical_tt:      [ [ "ABC", "defg", "H" ], "horizontal FALSE size 0.9", "" ],
   vertical_ft:      [ [ "ABC", "defg", "H" ], "horizontal FALSE topToBottom FALSE size 0.9", "" ],
   vertical_tf:      [ [ "ABC", "defg", "H" ], "horizontal FALSE leftToRight FALSE size 0.9", "" ],
   vertical_ff:      [ [ "ABC", "defg", "H" ], "horizontal FALSE leftToRight FALSE topToBottom FALSE size 0.9", "" ],
   vertical_mid:     [ [ "ABC", "defg", "H" ], 'horizontal FALSE justify [ "MIDDLE" "MIDDLE" ] size 0.9', "" ],
   vertical_end:     [ [ "ABC", "defg", "H" ], 'horizontal FALSE justify [ "END" "END" ] size 0.9', "" ],
   vertical_length:  [ [ "ABC", "defg", "H" ], "horizontal FALSE size 0.9", "length [ 5 3 ]" ],
   length:           [ [ "Hello", "Wide World", "ij" ], "", "length [ 3 6 ]" ],
   length_mid:       [ [ "Hello", "Wide World", "ij" ], 'justify "MIDDLE"', "length [ 3 12 0 ]" ],
   maxextent:        [ [ "Hello", "Wide World", "ij" ], "", "maxExtent 4" ],
   maxextent_big:    [ [ "Hello", "Wide World", "ij" ], "", "maxExtent 40" ],
   length_maxextent: [ [ "Hello", "Wide World", "ij" ], 'justify "END"', "length [ 3 6 ] maxExtent 4" ],
   empty:            [ [ "" ], "", "" ],
   none:             [ [ ], "", "" ],
   empty_lines:      [ [ "", "a", "", "b" ], "", "" ],
   latin1:           [ [ "Café Ünï ñ ß", "© ½ æøå" ], "", "" ],
   notdef:           [ [ "AЖ中B", "Ωmega" ], "", "" ],
   spaces:           [ [ "  lead", "trail  ", "a   b" ], 'justify "MIDDLE"', "" ],
   kerning:          [ [ "AVAV To Wa Te Yo" ], "", "" ],
   sans_bold:        [ [ "The quick fox", "AVAV To Wa 0123" ], 'family "SANS" style "BOLD"', "" ],
   mono_italic:      [ [ "The quick fox", "AVAV To Wa 0123" ], 'family "TYPEWRITER" style "ITALIC"', "" ],
   fallback:         [ [ "Fallback" ], 'family [ "NoSuchFont" "SANS" ] style "BOLD"', "" ],
   unknown_family:   [ [ "Unknown" ], 'family "NoSuchFont"', "" ],
   lower_family:     [ [ "lower" ], 'family "sans"', "" ],
   bad_style:        [ [ "Bad style" ], 'style "BLINK"', "" ],
   neg_size:         [ [ "Neg" ], "size -1", "" ],
};

const GLYPH_CHARS = "AaWgeE8&@Qi.,ofyéüß";
const KERN_CHARS  = "AVTWYoaeLrP.,fgijl 0123yvkwtT";

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

   page .on ("console", m => { if (process .env .TEXT_DEBUG) console .log ("[page]", m .text () .slice (0, 300)); });
   page .on ("crash", () => console .log ("[crash]"));
   page .on ("close", () => console .log ("[close]"));
   page .on ("pageerror", e => console .log ("[pageerror]", String (e) .slice (0, 300)));

   await page .goto (`${base}/x_ite-pin/src/shot.html`);
   await page .waitForFunction (() => window .ready === true, null, { timeout: 30000 });

   const SETUP = ({ GLYPH_CHARS, KERN_CHARS }) =>
   {
      const browser = document .getElementById ("c") .browser;
      const arr     = a => Array .from (a .getValue ? a .getValue () : a);
      const sleep   = ms => new Promise (r => setTimeout (r, ms));

      window .makeText = async function (strings, fs, textFields)
      {
         const str   = strings .map (s => JSON .stringify (s)) .join (" ");
         const scene = await browser .createX3DFromString (`#VRML V2.0 utf8\nDEF T Text { string [ ${str} ] ${textFields} fontStyle DEF F FontStyle { ${fs} } }`);
         const node  = scene .getNamedNode ("T") .getValue ();
         const fnode = scene .getNamedNode ("F") .getValue ();

         for (let i = 0; i < 200 && fnode .checkLoadState () < 2; ++ i)
            await sleep (25);

         return { scene, node, fnode };
      };

      function glyphInfo (node, font, ch)
      {
         const glyph = font .charToGlyph (ch);
         const geo   = node .textGeometry .getGlyphGeometry (font, glyph, 1);
         const v     = [ ];

         for (const p of geo)
            v .push (p .x, p .y);

         let area = 0;

         for (let i = 0; i + 5 < v .length; i += 6)
            area += ((v [i + 2] - v [i]) * (v [i + 5] - v [i + 1]) - (v [i + 4] - v [i]) * (v [i + 3] - v [i + 1])) / 2;

         return { index: glyph .index, advance: glyph .advanceWidth, vertices: v, area };
      }

      window .fontInfo = async function (family, style, full)
      {
         const { node, fnode } = await window .makeText ([ "x" ], `family "${family}" style "${style}"`, "");
         const font = fnode .getFont ();
         const info = { loaded: !! font };

         if (font)
         {
            info .unitsPerEm = font .unitsPerEm;
            info .ascender   = font .ascender;
            info .descender  = font .descender;
            info .numGlyphs  = font .numGlyphs;
            info .hasGpos    = !! font .tables .gpos;
            info .hasKern    = !! font .tables .kern;
            info .kerning    = { };

            for (const a of KERN_CHARS)
            {
               for (const b of KERN_CHARS)
               {
                  const k = font .getKerningValue (font .charToGlyph (a), font .charToGlyph (b));

                  if (k)
                     info .kerning [a + b] = k;
               }
            }

            info .glyphs = { };

            for (const ch of GLYPH_CHARS)
            {
               const g = glyphInfo (node, font, ch);

               info .glyphs [ch] = { index: g .index, advance: g .advance, triangles: g .vertices .length / 6, area: g .area, vertices: full ? g .vertices : undefined };
            }
         }

         return info;
      };

      window .layout = async function (strings, fs, textFields)
      {
         const { node, fnode } = await window .makeText (strings, fs, textFields);

         node .rebuild ();

         return {
            strings, fontStyle: fs, text: textFields,
            loadState: fnode .checkLoadState (),
            vertices:  arr (node .getVertices ()),
            texCoords: node .getMultiTexCoords () .length ? arr (node .getMultiTexCoords () [0]) : [ ],
            normals:   arr (node .getNormals ()),
            min: Array .from (node .getMin ()),
            max: Array .from (node .getMax ()),
            origin:     arr (node ._origin),
            textBounds: arr (node ._textBounds),
            lineBounds: Array .from (node ._lineBounds) .map (v => arr (v)),
            solid: node .isSolid (),
            ccw: node .getCCW (),
         };
      };
   };

   await page .evaluate (SETUP, { GLYPH_CHARS, KERN_CHARS });

   const result = { fonts: { }, layouts: { } };

   for (const family of FAMILIES)
      for (const style of STYLES)
         result .fonts [`${family}-${style}`] = await page .evaluate (([ f, s ]) => window .fontInfo (f, s, [ "SERIF-PLAIN", "SANS-BOLD", "TYPEWRITER-ITALIC" ] .includes (`${f}-${s}`)), [ family, style ]);

   for (const [ name, [ strings, fs, textFields ] ] of Object .entries (LAYOUTS))
      result .layouts [name] = await page .evaluate (([ a, b, c ]) => window .layout (a, b, c), [ strings, fs, textFields ]);

   const json = { pinned: "oracle/x_ite-pin (X_ITE 16.5.0)", browser: browser .version (), ... result };

   await writeFile (resolve (process .argv [2] ?? "oracle/x_ite-text.json"), JSON .stringify (json) + "\n");
   console .log (`${Object .keys (result .layouts) .length} layouts, ${Object .keys (result .fonts) .length} fonts`);
}
finally
{
   await browser .close ();
   server .close ();
}
