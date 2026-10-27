// The web data URL compatibility 01 cases: raw tab, LF and CR characters in and after the scheme of a data URL, for
// the world's loadURL, an ImageTexture and a glTF buffer, with an http control. Chromium keeps such a character after
// the `data:` scheme (as %09, %0A, %0D) and removes it in the scheme; the URL standard removes it everywhere.
// oracle/x_ite-web-runtime-02.mjs (--suite dataurl01) runs them against pinned X_ITE 16.5.0 and records the values;
// dataurl01.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).

import { readFileSync } from "node:fs";
import { DESKTOP, settle } from "./runtime02_case.mjs";
import { orderedMessages, pageLoadURL, sceneCase, served, worldName } from "./loadurl01_case.mjs";

const W = "/tests/browser/worlds/fallback/parts/w.wrl";

// A world whose WorldInfo title (DEF Info) has a tab, LF and CR. In the URL they are raw characters (`TITLE_RAW`), and
// the rest is escaped: the title shows whether the browser kept them ("a\tb\nc\rd") or removed them ("abcd").
const TITLE_WORLD = '#VRML V2.0 utf8\nDEF Info WorldInfo { title "a\tb\nc\rd" }\n';
const TITLE_RAW   = `${encodeURIComponent ('#VRML V2.0 utf8\nDEF Info WorldInfo { title "')}a\tb\nc\rd${encodeURIComponent ('" }\n')}`;

// A world with a raw `#VRML` header (PR #123, G9).
const HASH_WORLD = "#VRML V2.0 utf8\nDEF Info WorldInfo { title \"hash\" }\n";

const fixture = path => readFileSync (new URL (`../../../../${path}`, import.meta.url));
const GIF     = fixture ("crates/x3d-web/tests/browser/worlds/tex/green.gif") .toString ("base64");

// A world-level loadURL of `urls` (data URLs, or page-relative paths starting with "/"), bounded: the result within
// `ms` ("pending" if loadURL has not settled), the world ("data", a test world, or "page"), the browser events, the
// requests and the messages.
async function worldCase (page, env, urls, ms = 5000)
{
   page .log .length = 0;

   const from = env .server .requests .length;

   await page .evaluate (pageLoadURL, urls .map (url => url .startsWith ("/") ? `${env .base}${url}` : url));

   const result = await page .evaluate (ms => Promise .race ([ window .pending, new Promise (r => setTimeout (() => r ("pending"), ms)) ]), ms);

   await settle (page, 300);

   const url   = await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .currentScene ?.worldURL ?? "");
   const world = url .startsWith ("data:") ? "data" : await worldName (page);

   return { result, world, events: await page .evaluate (() => window .events), served: served (env, from), console: orderedMessages (page .log, env) };
}

// The title of the world's DEF Info as code points, or null.
const title = page => page .evaluate (() =>
{
   const scene = document .querySelector ("x3d-canvas") .browser .currentScene;
   const info  = scene .rootNodes .length ? scene .getNamedNode ("Info") : null;

   return info ? Array .from (info .title, ch => ch .codePointAt (0) .toString (16)) .join (" ") : null;
});

// A world-level loadURL whose first candidate is the data URL `url` and whose second is W.
async function dataWorldCase (page, env, url)
{
   const out = await worldCase (page, env, [ url, W ]);

   return { ... out, title: out .world === "data" ? await title (page) : null };
}

// The points of the first Coordinate of the world (glTF), numbers to 7 decimals, or null.
const coordinates = page => page .evaluate (() =>
{
   const found = [ ];
   const walk  = node =>
   {
      if (!node || found .length)
         return;

      if (node .getNodeTypeName () === "Coordinate")
         return found .push (String (node .point));

      for (const name of [ "children", "geometry", "coord" ])
      {
         let value;

         try { value = node [name]; } catch { continue; }

         if (value ?.getNodeTypeName)
            walk (value);
         else if (value && typeof value !== "string" && typeof value .length === "number")
            for (const child of value) walk (child);
      }
   };

   for (const node of document .querySelector ("x3d-canvas") .browser .currentScene ?.rootNodes ?? [ ])
      walk (node);

   return found .length ? found [0] .replace (/-?\d+(?:\.\d+)?(?:e[-+]?\d+)?/gi, n => String (Number (Number (n) .toFixed (7)) || 0)) : null;
});

// A glTF world whose buffer is a data URL: loadURL of the file, then its coordinates. A world with a glTF Inline
// (`.wrl`) for the buffers that fail and for the file control: X_ITE_R's world-level glTF neither waits for an
// external buffer nor reports a buffer failure like X_ITE (known, outside this lane); its Inline does both.
async function gltfCase (page, env, file)
{
   const out = await worldCase (page, env, [ `/tests/browser/worlds/dataurl/${file}` ]);

   return { ... out, coordinates: out .result === "resolved" && file .endsWith (".gltf") ? await coordinates (page) : null };
}

const IMAGE_WORLD = urls => `#X3D V4.0 utf8
PROFILE Interchange
Viewpoint { position 0 0 4 }
Shape { appearance Appearance { texture ImageTexture { url [ ${urls .map (u => `"${u}"`) .join (" ")} ] } } geometry Box { } }
`;

async function imageCase (page, env, urls)
{
   const c = await sceneCase (page, env, IMAGE_WORLD (urls));

   // X_ITE names the blob URL of image data that does not decode; its id differs in each run.
   return { loaded: await c .loaded (), served: c .served (), console: c .console () .map (line => line .replace (/blob:[^']*/g, "blob:<id>")) };
}

export const worldCases = {
   // `data:`: FileLoader's text branch keeps the characters (the control).
   lowerRaw: (page, env) => dataWorldCase (page, env, `data:model/vrml,${TITLE_RAW}`),

   // An upper-case scheme: the text branch takes only `data:`, so fetch decodes the URL. Chrome keeps the characters.
   upperRaw: (page, env) => dataWorldCase (page, env, `DATA:model/vrml,${TITLE_RAW}`),

   // Leading tabs and newlines, then `Data:`: kept.
   leadingTabRaw: (page, env) => dataWorldCase (page, env, `\t\nData:model/vrml,${TITLE_RAW}`),

   // A leading space: removed like the URL standard.
   leadingSpaceRaw: (page, env) => dataWorldCase (page, env, ` DATA:model/vrml,${TITLE_RAW}`),

   // A tab in the scheme: the scheme is `data`, and the characters are removed.
   schemeTabRaw: (page, env) => dataWorldCase (page, env, `da\tta:model/vrml,${TITLE_RAW}`),

   // Escaped: the title keeps them.
   upperEscaped: (page, env) => dataWorldCase (page, env, `DATA:model/vrml,${encodeURIComponent (TITLE_WORLD)}`),

   // LF and CR in the scheme, then base64.
   schemeNewlineBase64: (page, env) => dataWorldCase (page, env, `D\nA\rTA:model/vrml;base64,${btoa (TITLE_WORLD)}`),

   // Raw CR LF in the base64 body are skipped.
   base64BodyNewlines: (page, env) => dataWorldCase (page, env, `DATA:model/vrml;base64,${btoa (TITLE_WORLD) .replace (/(.{8})/g, "$1\r\n")}`),

   // After `DATA:`, `;<TAB>base64` is no base64 flag: the document is the base64 text, which no parser handles.
   tabBeforeBase64: (page, env) => dataWorldCase (page, env, `DATA:model/vrml;\tbase64,${btoa (TITLE_WORLD)}`),

   // After `da<TAB>ta:`, the tab is removed and `;base64` is the flag.
   schemeTabTabBeforeBase64: (page, env) => dataWorldCase (page, env, `da\tta:model/vrml;\tbase64,${btoa (TITLE_WORLD)}`),

   // A tab after the MIME type and before `;base64` keeps the flag.
   tabAfterMime: (page, env) => dataWorldCase (page, env, `DATA:model/vrml\t;base64,${btoa (TITLE_WORLD)}`),

   // Base64 with a tab that does not decode: fetch rejects, and the next candidate loads.
   badBase64: (page, env) => dataWorldCase (page, env, "DA\tTA:model/vrml;base64,QQ=\t@"),

   // No comma: fetch rejects.
   noComma: (page, env) => dataWorldCase (page, env, "DATA:\tmodel/vrml"),

   // A raw `#VRML` header: `#` starts the URL's fragment (PR #123 found that pinned X_ITE's loadURL did not settle
   // then). Recorded within the bound of worldCase, so a load that does not settle is "pending".
   rawHashHeader: (page, env) => dataWorldCase (page, env, `data:model/vrml,${HASH_WORLD}`),
   rawHashHeaderUpper: (page, env) => dataWorldCase (page, env, `DATA:model/vrml,${HASH_WORLD}`),

   // `d<TAB>ata-x:` is no data scheme: fetch rejects it, then the next candidate.
   otherScheme: (page, env) => dataWorldCase (page, env, "d\tata-x:,y"),
};

export const imageCases = {
   // After `data:`, `;<TAB>base64` is no base64 flag: the GIF does not decode, and the file is requested.
   tabBeforeBase64: (page, env) => imageCase (page, env, [ `data:image/gif;\tbase64,${GIF}`, "BASE/tests/browser/worlds/tex/green.gif" ]),

   // After `da<TAB>ta:` or ` data:`, the tab is removed: the GIF decodes, and the next candidate is not requested.
   schemeTabTabBeforeBase64: (page, env) => imageCase (page, env, [ `da\tta:image/gif;\tbase64,${GIF}`, "BASE/status/404/k4.gif" ]),
   leadingSpaceTabBeforeBase64: (page, env) => imageCase (page, env, [ ` data:image/gif;\tbase64,${GIF}`, "BASE/status/404/k5.gif" ]),

   // Tabs in the scheme and the base64 body: the GIF decodes; the next candidate is not requested.
   schemeAndBodyTabs: (page, env) => imageCase (page, env, [ `DA\tTA:image/gif;base64,${GIF .slice (0, 8)}\t${GIF .slice (8)}`, "BASE/status/404/k1.gif" ]),

   // A raw LF in the base64 body of a lower-case URL.
   bodyNewline: (page, env) => imageCase (page, env, [ `data:image/gif;base64,${GIF .slice (0, 8)}\n${GIF .slice (8)}`, "BASE/status/404/k2.gif" ]),

   // Base64 with a tab that does not decode, then a file.
   badBase64: (page, env) => imageCase (page, env, [ "DA\tTA:image/gif;base64,R0lG\t@@@@", "BASE/tests/browser/worlds/tex/green.gif" ]),

   // The http control: tabs and line breaks in a path are removed.
   httpTabs: (page, env) => imageCase (page, env, [ "BASE/tests/browser/worlds/te\tx/gr\neen.gif", "BASE/status/404/k3.gif" ]),
};

export const gltfCases = {
   // The control: an external buffer file (Inline).
   file: (page, env) => gltfCase (page, env, "inline-file.wrl"),

   // A percent body whose byte 0x09 is a raw tab (JSON "\t"): `data:` and `DATA:` keep it.
   lowerTab: (page, env) => gltfCase (page, env, "gltf-lower-tab.gltf"),
   upperTab: (page, env) => gltfCase (page, env, "gltf-upper-tab.gltf"),

   // A tab in the scheme and in the base64 body.
   schemeTab: (page, env) => gltfCase (page, env, "gltf-scheme-tab.gltf"),

   // A tab in the scheme removes the raw tab of a percent body: the buffer is one byte short (Inline).
   schemeTabPercent: (page, env) => gltfCase (page, env, "inline-scheme-tab-percent.wrl"),

   // After `DATA:`, `;<TAB>base64` is no base64 flag: the buffer is the base64 text.
   tabBeforeBase64: (page, env) => gltfCase (page, env, "gltf-tab-base64-flag.gltf"),

   // After `da<TAB>ta:`, the tab is removed and the flag is kept: the buffer decodes.
   schemeTabTabBeforeBase64: (page, env) => gltfCase (page, env, "gltf-scheme-tab-base64-flag.gltf"),

   // Base64 that does not decode (Inline).
   badBase64: (page, env) => gltfCase (page, env, "inline-bad-base64.wrl"),
};

export const CASE_GROUPS = {
   dataURLWorld: { context: DESKTOP, cases: worldCases },
   dataURLImage: { context: DESKTOP, cases: imageCases },
   dataURLGltf: { context: DESKTOP, cases: gltfCases },
};
