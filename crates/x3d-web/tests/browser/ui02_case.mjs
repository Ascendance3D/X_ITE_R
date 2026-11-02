// The browser user interface 02 cases (issue #141, the rest of the port): the notifications of the Debug keys, Shift+F8 for a
// GeoViewpoint and for each encoding of the world, and the "Loading n files" notifications (Browser/Core/X3DCoreContext.js
// [_keydown], Browser/Networking/X3DNetworkingContext.js [_set_loadCount] of X_ITE 16.5.0). oracle/x_ite-web-runtime-02.mjs
// (--suite ui02) runs them against pinned X_ITE 16.5.0 and records the values; ui02.test.mjs runs them against X_ITE_R and
// compares (runtime02_harness.mjs). Like ui01, every case drives the real interface: Playwright's keyboard on the focused canvas,
// and the clipboard and the notification are read back.

import { vrml, wait, frames, holdTimers, flush, loadWorld, pageHelpers, PLAIN_WORLD } from "./ui01_case.mjs";

const data = (type, text) => `data:${type},${encodeURIComponent (text)}`;

const BASE = "COMPONENT Geospatial:1\n";

// --- the Debug keys -------------------------------------------------------------------------------------------------------

// Ctrl+1 to Ctrl+5 set the Shading option and Ctrl+S ends and begins the update; each sets the description, which the
// notification shows. With the Debug option false the keys do nothing and set no description.
async function debugKeys (page)
{
   await loadWorld (page, PLAIN_WORLD);
   await page .evaluate (pageHelpers);
   await page .evaluate (() => { window .ui .setOption ("Notifications", true); window .ui .setOption ("Debug", false); window .ui .canvas () .focus (); });

   const out = { };

   const read = async label =>
   {
      await frames (page);
      await wait (page, 100);

      out [label] = await page .evaluate (() =>
      {
         const browser = window .ui .canvas () .browser;
         const live    = browser .isLive ();

         return { shading: browser .getBrowserOption ("Shading"), live: typeof live === "boolean" ? live : live .getValue (), description: browser .description, notification: window .ui .notification () };
      });
   };

   const press = async (label, key) => { await page .keyboard .press (key); await read (label); };

   await read ("start");
   await press ("offCtrl1", "Control+1");
   await press ("offCtrlS", "Control+s");

   await page .evaluate (() => window .ui .setOption ("Debug", true));
   await press ("ctrl1", "Control+1");
   await press ("ctrl2", "Control+2");
   await press ("ctrl3", "Control+3");
   await press ("ctrl4", "Control+4");
   await press ("ctrl5", "Control+5");
   await press ("ctrlS", "Control+s");
   await press ("ctrlS2", "Control+s");

   // The notification is the last description; a menu entry changes the option without one.
   await page .evaluate (() => { window .ui .canvas () .browser .description = ""; });
   await read ("cleared");
   await press ("ctrl3Again", "Control+3");

   // Notifications off: the description is set, the notification stays as it was.
   await page .evaluate (() => window .ui .setOption ("Notifications", false));
   await press ("notificationsOff", "Control+1");

   return out;
}

// --- Shift+F8 -------------------------------------------------------------------------------------------------------------

const VP_VRML = `#X3D V4.0 utf8\nPROFILE Full\nDEF G Viewpoint { description "G" position 1 2 10 orientation 0 1 0 0.5 centerOfRotation 0 1 0 fieldOfView 0.6 }\nShape { geometry Box { } }\n`;
const GEO_VRML = `#X3D V4.0 utf8\nPROFILE Full\n${BASE}DEF G GeoViewpoint { description "Geo" geoSystem [ "GD" "WE" ] position 10 20 1000000 orientation 0 1 0 0.3 centerOfRotation 10 20 0 fieldOfView 0.7 }\nShape { geometry Box { } }\n`;
const GEO_ORIGIN_VRML = `#X3D V4.0 utf8\nPROFILE Full\n${BASE}DEF G GeoViewpoint { description "Geo" geoOrigin GeoOrigin { geoSystem [ "GD" "WE" ] geoCoords 10 20 0 rotateYUp TRUE } geoSystem [ "GD" "WE" ] position 10.5 20.5 5000 orientation 0 1 0 0.3 centerOfRotation 10 20 0 }\nShape { geometry Box { } }\n`;
const GEO_UTM_VRML = `#X3D V4.0 utf8\nPROFILE Full\n${BASE}DEF G GeoViewpoint { description "Geo" geoSystem [ "UTM" "Z33" "N" ] position 500000 5000000 3000 }\nShape { geometry Box { } }\n`;
const GEO_GC_VRML = `#X3D V4.0 utf8\nPROFILE Full\n${BASE}DEF G GeoViewpoint { description "Geo" geoSystem [ "GC" ] position 5000000 3000000 4000000 }\nShape { geometry Box { } }\n`;
const ORTHO_VRML = `#X3D V4.0 utf8\nPROFILE Full\nDEF G OrthoViewpoint { description "O" position 1 2 10 orientation 0 1 0 0.5 centerOfRotation 0 1 0 fieldOfView [ -2 -2 2 2 ] }\nShape { geometry Box { } }\n`;
const NO_VP_VRML = `#X3D V4.0 utf8\nPROFILE Full\nShape { geometry Box { } }\n`;

const XML = body => `<?xml version="1.0" encoding="UTF-8"?>\n<X3D profile="Full" version="4.0"><Scene>${body}</Scene></X3D>`;
const JSN = children => JSON .stringify ({ X3D: { "@profile": "Full", "@version": "4.0", Scene: { "-children": children } } });

const VP_XML  = XML (`<Viewpoint DEF="G" description="G" position="1 2 10" orientation="0 1 0 0.5" centerOfRotation="0 1 0" fieldOfView="0.6"/><Shape><Box/></Shape>`);
const GEO_XML = XML (`<GeoViewpoint DEF="G" description="Geo" geoSystem='"GD" "WE"' position="10 20 1000000" orientation="0 1 0 0.3" centerOfRotation="10 20 0" fieldOfView="0.7"/><Shape><Box/></Shape>`);
const GEO_ORIGIN_XML = XML (`<GeoViewpoint DEF="G" description="Geo" geoSystem='"GD" "WE"' position="10.5 20.5 5000" orientation="0 1 0 0.3" centerOfRotation="10 20 0"><GeoOrigin geoSystem='"GD" "WE"' geoCoords="10 20 0" rotateYUp="true"/></GeoViewpoint><Shape><Box/></Shape>`);
const VP_JSON = JSN ([ { Viewpoint: { "@DEF": "G", "@description": "G", "@position": [ 1, 2, 10 ], "@orientation": [ 0, 1, 0, 0.5 ], "@centerOfRotation": [ 0, 1, 0 ], "@fieldOfView": 0.6 } }, { Shape: { "-geometry": { Box: { } } } } ]);
const GEO_JSON = JSN ([ { GeoViewpoint: { "@DEF": "G", "@description": "Geo", "@geoSystem": [ "GD", "WE" ], "@position": [ 10, 20, 1000000 ], "@orientation": [ 0, 1, 0, 0.3 ], "@centerOfRotation": [ 10, 20, 0 ], "@fieldOfView": 0.7 } }, { Shape: { "-geometry": { Box: { } } } } ]);

// The clipboard text of Shift+F8 for the world at `url`, and what the browser says about it.
async function copy (page, url)
{
   await loadWorld (page, url);
   await page .evaluate (pageHelpers);
   await page .evaluate (() => { window .ui .setOption ("Notifications", true); window .ui .canvas () .focus (); navigator .clipboard .writeText ("before"); });
   await frames (page);
   await page .keyboard .press ("Shift+F8");
   await frames (page);
   await wait (page, 200);

   return page .evaluate (async () =>
   {
      const browser = window .ui .canvas () .browser;

      return { clipboard: await navigator .clipboard .readText () .catch (e => `error ${e .message}`), description: browser .description, active: browser .activeViewpoint ?.getNodeTypeName () ?? null };
   });
}

async function shiftF8Geo (page)
{
   await page .context () .grantPermissions ([ "clipboard-read", "clipboard-write" ]);

   return {
      geo: await copy (page, data ("model/x3d-vrml", GEO_VRML)),
      geoOrigin: await copy (page, data ("model/x3d-vrml", GEO_ORIGIN_VRML)),
      utm: await copy (page, data ("model/x3d-vrml", GEO_UTM_VRML)),
      gc: await copy (page, data ("model/x3d-vrml", GEO_GC_VRML)),
      geoXml: await copy (page, data ("model/x3d+xml", GEO_XML)),
      geoOriginXml: await copy (page, data ("model/x3d+xml", GEO_ORIGIN_XML)),
      geoJson: await copy (page, data ("model/x3d+json", GEO_JSON)),
   };
}

// The text is in the encoding the world was parsed from; a scene made by the page (createScene) has the encoding SCRIPTED,
// which is XML.
async function shiftF8Encodings (page)
{
   await page .context () .grantPermissions ([ "clipboard-read", "clipboard-write" ]);

   const out =
   {
      vrml: await copy (page, data ("model/x3d-vrml", VP_VRML)),
      xml: await copy (page, data ("model/x3d+xml", VP_XML)),
      json: await copy (page, data ("model/x3d+json", VP_JSON)),
      ortho: await copy (page, data ("model/x3d-vrml", ORTHO_VRML)),
      noViewpoint: await copy (page, data ("model/x3d-vrml", NO_VP_VRML)),
   };

   // A VRML text with an extension that says otherwise: the content decides.
   out .vrmlUrlX3d = await copy (page, `${data ("model/x3d-vrml", VP_VRML)}#file.x3d`);

   // createX3DFromString: the encoding of its parser. replaceWorld with it, and with a scene of createScene.
   out .fromString = await page .evaluate (async ([ vrmlText, xmlText ]) =>
   {
      const browser = window .ui ?.canvas () .browser ?? document .querySelector ("x3d-canvas") .browser;
      const result  = { };

      for (const [ name, text ] of [ [ "vrml", vrmlText ], [ "xml", xmlText ] ])
      {
         const scene = await browser .createX3DFromString (text);

         await browser .replaceWorld (scene);
         result [name] = scene .encoding;
      }

      return result;
   }, [ VP_VRML, VP_XML ]);

   // Shift+F8 in the world that a replaceWorld made.
   const keyAfter = async () =>
   {
      await page .evaluate (pageHelpers);
      await page .evaluate (() => { window .ui .canvas () .focus (); return navigator .clipboard .writeText ("before"); });
      await frames (page);
      await page .keyboard .press ("Shift+F8");
      await frames (page);
      await wait (page, 200);

      return page .evaluate (() => navigator .clipboard .readText ());
   };

   await page .evaluate (async text =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;

      await browser .replaceWorld (await browser .createX3DFromString (text));
   }, VP_VRML);
   out .vrmlString = await keyAfter ();

   await page .evaluate (async () =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const scene   = await browser .createScene (browser .getProfile ("Full"));
      const node    = scene .createNode ("Viewpoint");

      node .position = new window .X3D_NS .SFVec3f (3, 4, 5);
      scene .rootNodes .push (node);
      await browser .replaceWorld (scene);
   });
   out .scripted = await keyAfter ();

   return out;
}

// --- "Loading n files" ----------------------------------------------------------------------------------------------------

const INLINE_URL = data ("model/x3d-vrml", "#X3D V4.0 utf8\nPROFILE Full\nShape { geometry Sphere { } }\n");
const BAD_URL    = data ("model/x3d-vrml", "this is not a world");
const PNG        = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

const LOADING_WORLD = vrml (`
DEF I1 Inline { load FALSE url "${INLINE_URL}" }
DEF I2 Inline { load FALSE url "${INLINE_URL}" }
DEF I3 Inline { load FALSE url "${INLINE_URL}" }
DEF BAD Inline { load FALSE url "${BAD_URL}" }
Shape { appearance Appearance { texture DEF TEX ImageTexture { } } geometry Box { } }
`);

// What the browser said while a load ran: every description set (the calls of setDescription), the notification, the last text.
async function loading (page)
{
   await loadWorld (page, LOADING_WORLD);
   await page .evaluate (pageHelpers);
   await page .evaluate (() =>
   {
      const browser = window .ui .canvas () .browser;
      const set     = browser .setDescription;

      window .said = [ ];
      browser .setDescription = function (text) { window .said .push (text); return set .call (this, text); };
      window .ui .setOption ("Notifications", true);
   });
   await frames (page);

   const out = { };

   const step = async (label, script, arg, ms = 400) =>
   {
      await page .evaluate (() => { window .said .length = 0; });
      await page .evaluate (script, arg);
      await frames (page);
      await wait (page, ms);
      await frames (page);
      out [label] = await page .evaluate (() => ({ said: window .said .slice (), description: window .ui .canvas () .browser .description, notification: window .ui .notification () }));
   };

   await step ("one", () => { window .ui .node ("I1") .load = true; });
   await step ("two", () => { window .ui .node ("I2") .load = true; window .ui .node ("I3") .load = true; });
   await step ("again", () => { window .ui .node ("I1") .load = false; window .ui .node ("I1") .load = true; });
   await step ("bad", () => { window .ui .node ("BAD") .load = true; });
   await step ("texture", url => { window .ui .node ("TEX") .url = [ url ]; }, PNG);

   // Notifications off: the descriptions are said, the notification is not shown.
   await page .evaluate (() => { window .ui .setOption ("Notifications", false); window .ui .canvas () .browser .description = ""; });
   await step ("notificationsOff", () => { window .ui .node ("I1") .url = [ window .ui .node ("I2") .url [0] + " " ]; window .ui .node ("I1") .load = false; window .ui .node ("I1") .load = true; });

   return out;
}

const cases = { debugKeys, shiftF8Geo, shiftF8Encodings, loading };

export const CASE_GROUPS =
{
   browserUi02: { context: { viewport: { width: 1000, height: 800 }, deviceScaleFactor: 1 }, cases },
};
