// The degenerate triangle 01 cases: TriangleSet worlds whose triangles have two or three equal points (zero area),
// with valid, collinear and very thin controls, and the failing world loaded again and again on one page. X_ITE's
// MikkTSpace wasm traps when every triangle has two equal points; X3DGeometryNode.generateTangents catches that, and the
// geometry stays without tangents. oracle/x_ite-web-runtime-02.mjs (--suite degenerate01) runs them against pinned
// X_ITE 16.5.0 and records the values; degenerate01.test.mjs runs them against X_ITE_R and compares
// (runtime02_harness.mjs).

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { orderedMessages, pageLoadURL } from "./loadurl01_case.mjs";

const world = points => `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8\nPROFILE Interchange\nShape { appearance Appearance { material Material { } } geometry TriangleSet { coord Coordinate { point [ ${points} ] } } }\n`)}`;

export const WORLDS = {
   valid:            world ("0 0 0, 1 0 0, 0 1 0"),
   twoEqual:         world ("1 1 1, 1 2 1, 1 1 1"),
   threeEqual:       world ("1 1 1, 1 1 1, 1 1 1"),
   collinear:        world ("0 0 0, 1 0 0, 2 0 0"),
   thin:             world ("0 0 0, 1 0 0, 0 0.000001 0"),
   validAndTwoEqual: world ("0 0 0, 1 0 0, 0 1 0, 1 1 1, 1 2 1, 1 1 1"),
   severalEqual:     world ("1 1 1, 1 2 1, 1 1 1, 2 2 2, 2 2 2, 3 3 3, 0 0 0, 0 0 0, 0 0 0"),
   // The world of the PR #124 finding F1.
   finding:          `data:model/x3d-vrml,${encodeURIComponent ("#X3D V4.0 utf8\nPROFILE Interchange\nShape { appearance Appearance { material PhysicalMaterial { metallic 0 } } geometry TriangleSet { normalPerVertex FALSE coord Coordinate { point [ 12.07843 12.07843 12.07843, 12.07843 12.20734 12.09163, 12.07843 12.07843 12.07843 ] } } }\n")}`,
};

// The load messages. X_ITE's generateTangents logs the error it caught only when DEVELOPMENT is true: the pinned
// source is, X_ITE's dist build makes it false (webpack.config.js). So its "RuntimeError" lines are no load messages.
const messages = (page, env) => orderedMessages (page .log, env) .filter (line => !line .startsWith ("error: RuntimeError: "));

// A world-level loadURL of `url`: the result, the browser events, the TriangleSet and its number of points, the
// messages, and the picture.
async function worldCase (page, env, url)
{
   page .log .length = 0;

   await page .evaluate (pageLoadURL, [ url ]);

   const result = await page .evaluate (() => Promise .race ([ window .pending, new Promise (r => setTimeout (() => r ("pending"), 5000)) ]));

   await settle (page, 300);

   const geometry = await page .evaluate (() =>
   {
      const shape = document .querySelector ("x3d-canvas") .browser .currentScene ?.rootNodes [0];
      const node  = shape ?.geometry;

      // The number of points from the field's text: X_ITE_R's MFVec3f has no length (an X3DField with its text).
      return node ? { type: node .getNodeTypeName (), points: (String (node .coord .point) .match (/[-+.\deE]+/g) ?? [ ]) .length / 3 } : null;
   });

   return { result, events: await page .evaluate (() => window .events), geometry, console: messages (page, env), picture: await env .picture (page) };
}

const cases = Object .fromEntries (Object .entries (WORLDS) .map (([ name, url ]) => [ name, (page, env) => worldCase (page, env, url) ]));

// The failing world three times on one page, then the valid one: each world replaces the one before.
cases .repeated = async (page, env) =>
{
   const out = [ ];

   for (const url of [ WORLDS .twoEqual, WORLDS .twoEqual, WORLDS .twoEqual, WORLDS .valid ])
      out .push (await worldCase (page, env, url));

   return out;
};

export const CASE_GROUPS = {
   degenerateTriangle: { context: DESKTOP, cases },
};
