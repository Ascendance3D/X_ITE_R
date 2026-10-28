// Field API 02 tests of X_ITE_R's <x3d-canvas>, in real headless Chromium with WebGPU: the text of nodes (toString,
// toVRMLString, toXMLString, toJSONString) and values read from text (fromString, fromVRMLString, fromXMLString)
// (fields02_case.mjs, issue #137), compared with the pinned X_ITE 16.5.0 recording in
// oracle/web-browser-fields-02-baseline (runtime02_harness.mjs).
//
//   tools/build-web.sh && (cd crates/x3d-web && npm ci && npm test)

import { referenceCases } from "./runtime02_harness.mjs";

referenceCases ("fields02.test.mjs");

// The exported wasm functions printNodes, parseField and parseFieldWithoutScene called directly (X_ITE_R only: the
// pinned X_ITE has no such functions). The page API over them is compared with X_ITE by the cases above.
import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { DESKTOP, settle } from "./runtime02_case.mjs";
import { pageLoadURL } from "./loadurl01_case.mjs";
import { openPage } from "./runtime02_harness.mjs";

const RAW_WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Interchange
DEF T Transform { children [ DEF G Group { } ] }
`)}`;

describe ("wasm exports, called directly", () =>
{
   test ("printNodes, parseField and parseFieldWithoutScene", { timeout: 180_000 }, async () =>
   {
      const page = await openPage (DESKTOP);

      try
      {
         await page .evaluate (pageLoadURL, [ RAW_WORLD ]);
         assert .equal (await page .evaluate (() => window .pending), "resolved", page .log .join ("\n"));
         await settle (page, 100);

         const out = await page .evaluate (async () =>
         {
            const X3D    = window .X3D_NS;
            const canvas = document .querySelector ("x3d-canvas");
            const scene  = canvas .browser .currentScene;
            const engine = canvas .browser [Symbol .for ("X_ITE_R.engine")];
            const key    = Object .getOwnPropertySymbols (Object .getPrototypeOf (scene)) .find (s => s .description === "X_ITE_R.node");
            const world  = scene [key] .scene;
            const named  = scene .getNamedNode ("T");
            const T      = (named [key] ?? named .getValue ?.() ?.[key]) .id;
            const attempt = fn => { try { return fn (); } catch (e) { return `${e .constructor .name}: ${e .message}`; } };
            const free    = await import (new URL ("/web/pkg/x3d_web.js", location .href));
            const r = { };

            // printNodes: the three text forms, then the refused arguments.
            r .print = [ "vrml", "xml", "json" ] .map (kind => attempt (() => engine .printNodes (world, kind, "SFNode", T) .slice (0, 12)));
            r .printMF = attempt (() => engine .printNodes (world, "vrml", "MFNode", [ T ]) .slice (0, 12));
            r .printKind = attempt (() => engine .printNodes (world, "yaml", "SFNode", T));
            r .printType = [ "SFString", "SFInt32", "MFFloat", "SFVec3f" ] .map (type => attempt (() => engine .printNodes (world, "vrml", type, T)));
            r .printUnknown = attempt (() => engine .printNodes (world, "vrml", "SFBogus", T));
            r .printBadData = [ "x", 1.5, -1, 4294967295 ] .map (data => attempt (() => engine .printNodes (world, "vrml", "SFNode", data)));
            r .printWorld = attempt (() => engine .printNodes (9999, "vrml", "SFNode", T));

            // parseField with the scene: the node is made in the owning world and usable there.
            const id = engine .parseField (world, "SFNode", "DEF N Group { }", false, "f", undefined);

            r .parsed = [ typeof id, engine .typeName (world, id), engine .nodeName (world, id), scene .getNamedNode ("N") ?.getNodeTypeName () ?? null,
                          engine .printNodes (world, "vrml", "SFNode", id) .slice (0, 11) ];
            r .parsedMF = attempt (() => { const ids = engine .parseField (world, "MFNode", "[ Group { } Transform { } ]", false, "f", undefined); return [ Array .isArray (ids) || ArrayBuffer .isView (ids), ids .length ]; });
            r .parsedXML = [ engine .parseField (world, "SFNode", "<Transform/>", true, "f", undefined), attempt (() => engine .parseField (world, "SFVec3f", "1, 2, 3", true, "f", undefined)) ];
            r .parseUnknownType = attempt (() => engine .parseField (world, "SFBogus", "1", false, "f", undefined));
            r .parseBad = [ attempt (() => engine .parseField (world, "SFVec3f", "1 2 x", false, "f", undefined)), attempt (() => engine .parseField (world, "SFNode", "Bogus { }", false, "f", undefined)) ];
            r .parseWorld = attempt (() => engine .parseField (9999, "SFVec3f", "1 2 3", false, "f", undefined));

            // parseFieldWithoutScene.
            const none = (type, text, xml, receiver = "undefined") => attempt (() => Array .from (free .parseFieldWithoutScene (type, text, xml, "f", receiver)));

            r .none = [ none ("SFVec3f", "1 2 3", false), none ("SFNode", "NULL", false), none ("MFNode", "[ ]", false),
                        none ("SFNode", "<Transform/>", true), none ("MFNode", "<Group/>", true), none ("MFString", "a b", true),
                        none ("SFNode", "Transform { }", false), none ("SFNode", "Transform { }", false, "null"),
                        none ("SFNode", "", false), none ("SFVec3f", "1 2 x", false), none ("SFBogus", "1", false) ];

            return r;
         });

         assert .ok (out .print [0] .startsWith ("DEF T Transf") && out .print [1] .startsWith ("<Transform ") && out .print [2] .startsWith ("{ \"Transfor"), JSON .stringify (out .print));
         assert .ok (out .printMF .startsWith ("DEF T Transf"), out .printMF);
         assert .match (out .printKind, /unknown text form 'yaml'/);

         for (const message of out .printType)
            assert .match (message, /not a node field type/);

         assert .match (out .printUnknown, /unknown field type 'SFBogus'/);

         for (const message of out .printBadData)
            assert .match (message, /^Error: /, message);

         assert .match (out .printWorld, /^Error: /);

         assert .deepEqual (out .parsed .slice (1, 4), [ "Group", "N", "Group" ]);
         assert .ok (out .parsed [4] .startsWith ("DEF N Group"), out .parsed [4]);
         assert .deepEqual (out .parsedMF, [ true, 2 ]);
         assert .equal (out .parsedXML [0], null);
         assert .deepEqual (out .parsedXML [1] .length, 3);
         assert .match (out .parseUnknownType, /unknown field type 'SFBogus'/);

         // An unknown node is a warning and NULL with a scene (the fromString case "Bogus { }" is compared with X_ITE above).
         assert .match (out .parseBad [0], /^Error: Couldn't read value for field 'f'\.$/);
         assert .equal (out .parseBad [1], null);

         assert .match (out .parseWorld, /^Error: /);

         assert .equal (out .none [0] [0] .length, 3);
         assert .equal (out .none [1] [0], null);
         assert .equal (out .none [2] [0] .length, 0);
         assert .equal (out .none [3] [0], null);
         assert .equal (out .none [4] [0] .length, 0);
         assert .match (out .none [5] [1], /is an MFString array/);
         assert .equal (out .none [6], "Error: TypeError: Cannot read properties of undefined (reading 'createNode')");
         assert .match (out .none [7], /properties of null /);
         assert .equal (out .none [8], "Error: Couldn't read value for field 'f'.");
         assert .equal (out .none [9], "Error: Couldn't read value for field 'f'.");
         assert .match (out .none [10], /unknown field type 'SFBogus'/);
      }
      finally
      {
         await page .context () .close ();
      }
   });
});
