// The browser options 01 cases (issue #136): getBrowserOption, setBrowserOption, getBrowserProperty,
// getRenderingProperty, print and println; part 4 (issue #141): browser.description and the notification.
// oracle/x_ite-web-runtime-02.mjs (--suite options01) runs them against pinned X_ITE 16.5.0 and records the values;
// options01.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { pageLoadURL } from "./loadurl01_case.mjs";

const WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Interchange
Shape { geometry Box { } }
`)}`;

// BrowserOptions.js of X_ITE 16.5.0: name, field type, a valid value other than the default.
const OPTIONS = [
   [ "SplashScreen", "SFBool", false ], [ "Dashboard", "SFBool", false ], [ "Rubberband", "SFBool", false ],
   [ "EnableInlineViewpoints", "SFBool", false ], [ "Antialiased", "SFBool", false ],
   [ "TextureQuality", "SFString", "HIGH" ], [ "PrimitiveQuality", "SFString", "LOW" ],
   [ "QualityWhenMoving", "SFString", "LOW" ], [ "Shading", "SFString", "PHONG" ], [ "MotionBlur", "SFBool", true ],
   [ "AlwaysUpdateGeometries", "SFBool", true ], [ "AutoUpdate", "SFBool", true ], [ "Cache", "SFBool", false ],
   [ "ColorSpace", "SFString", "LINEAR" ], [ "ContentScale", "SFDouble", 2 ],
   [ "ContextMenu", "SFBool", false ], [ "Debug", "SFBool", true ], [ "Exposure", "SFDouble", 1.5 ],
   [ "DisplayColorSpace", "SFString", "DISPLAY_P3" ], [ "Gravity", "SFDouble", 3.5 ], [ "LoadUrlObjects", "SFBool", false ],
   [ "LogarithmicDepthBuffer", "SFBool", true ], [ "MaximumFrameRate", "SFDouble", 30 ],
   [ "MetadataReference", "SFString", "ref" ], [ "Multisampling", "SFInt32", 2 ], [ "Mute", "SFBool", true ],
   [ "Notifications", "SFBool", false ], [ "OrderIndependentTransparency", "SFBool", true ],
   [ "SoundIntensity", "SFFloat", 0.5 ], [ "StraightenHorizon", "SFBool", false ],
   [ "TextCompression", "SFString", "SCALING" ], [ "Timings", "SFBool", true ], [ "ToneMapping", "SFString", "ACES_NARKOWICZ" ],
   [ "WallFriction", "SFDouble", 0.25 ], [ "XRSessionMode", "SFString", "IMMERSIVE_AR" ],
];

const PROPERTIES = [ "ABSTRACT_NODES", "CONCRETE_NODES", "EXTERNAL_INTERACTIONS", "PROTOTYPE_CREATE", "DOM_IMPORT", "XML_ENCODING", "CLASSIC_VRML_ENCODING", "BINARY_ENCODING" ];

const RENDERING = [ "Shading", "Antialiased", "ContentScale", "LogarithmicDepthBuffer", "Multisampling" ];

const RENDERING_OTHER = [ "MaxTextureSize", "TextureUnits", "MaxLights", "ColorDepth", "TextureMemory", "MaxAnisotropicDegree", "MaxSamples", "PixelsPerPoint", "SoundDestinations", "XRSession" ];

// Representative option per type for the coercion edges, and the values set.
const EDGES = {
   SFBool:   [ "Mute",              [ 0, 1, "x", "", null, undefined, [ ], "false", NaN ] ],
   SFInt32:  [ "Multisampling",     [ 3.7, -3.7, "5", "x", NaN, null, Infinity, 2 ** 31, true ] ],
   SFFloat:  [ "SoundIntensity",    [ "2.5", 0.1, "x", null, true, NaN, 1e40 ] ],
   SFDouble: [ "Gravity",           [ "2.5", "x", null, true, NaN, Infinity, "" ] ],
   SFString: [ "MetadataReference", [ 5, true, null, undefined, [ 1, 2 ], { }, "" ] ],
};

async function load (page)
{
   await page .evaluate (pageLoadURL, [ WORLD ]);

   const result = await page .evaluate (() => window .pending);

   if (result !== "resolved")
      throw new Error (`the world did not load: ${result}\n${page .log .join ("\n")}`);

   await settle (page, 100);
}

// The result of `fn`, or its error as `Name: message`; the same text in both engines. NaN and undefined are
// written as text, which the JSON of the recording would turn into null.
export function pageAttempt ()
{
   const show = value => typeof value === "number" && !Number .isFinite (value) ? `number:${value}` : value === undefined ? "undefined" : value;

   // X_ITE changes a rendering property when it handles the option's event, in its next frames.
   window .frames = async () =>
   {
      for (let i = 0; i < 4; ++ i)
         await new Promise (requestAnimationFrame);

      await new Promise (resolve => setTimeout (resolve, 100));
   };

   window .show    = show;
   window .attempt = fn =>
   {
      try
      {
         return show (fn ());
      }
      catch (error)
      {
         return `${error ?.constructor ?.name}: ${error ?.message}`;
      }
   };
}

// getBrowserOption of every option on a new browser (the page sets contentScale, splashScreen, notifications and
// timings attributes) and its JavaScript type, and the properties.
async function defaults (page)
{
   await load (page);

   return page .evaluate (([ options ]) =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;

      return Object .fromEntries (options .map (([ name ]) =>
         [ name, [ attempt (() => browser .getBrowserOption (name)), attempt (() => typeof browser .getBrowserOption (name)) ] ]));
   }, [ OPTIONS ]);
}

// Each option set to a valid value and read back, in the order of OPTIONS on one page (as recorded): an option is
// read after the options before it were set.
async function setValid (page)
{
   await load (page);

   return page .evaluate (([ options ]) =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const out     = { };

      for (const [ name, type, value ] of options)
      {
         const before = attempt (() => browser .getBrowserOption (name));
         const set    = attempt (() => browser .setBrowserOption (name, value));

         out [name] = { type, before, set, after: attempt (() => browser .getBrowserOption (name)) };
      }

      return out;
   }, [ OPTIONS ]);
}

// The coercion of a value of the wrong kind, per field type.
async function coercion (page)
{
   await load (page);

   return page .evaluate (([ edges ]) =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const out     = { };

      for (const [ type, [ name, values ] ] of Object .entries (edges))
      {
         out [`${type}:${name}`] = values .map (value => ({
            value: show (value) === value ? JSON .stringify (value) ?? "undefined" : show (value),
            set: attempt (() => browser .setBrowserOption (name, value)),
            after: attempt (() => browser .getBrowserOption (name)),
            kind: attempt (() => typeof browser .getBrowserOption (name)),
         }));
      }

      // setBrowserOption with no value.
      out .noValue = [ attempt (() => browser .setBrowserOption ("Mute")), attempt (() => browser .getBrowserOption ("Mute")) ];

      return out;
   }, [ EDGES ]);
}

// An unknown name, for get and set, and for the properties and the rendering properties; the same text in both.
async function unknownNames (page)
{
   await load (page);

   return page .evaluate (() =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const names   = [ "Bogus", "", "shading", "SHADING", "AntiAliased", "set_Shading", "Shading_changed", "toString", "__proto__", 5, null, undefined ];

      return names .map (name => ({
         name: show (name) === name ? JSON .stringify (name) : show (name),
         get: attempt (() => browser .getBrowserOption (name)),
         set: attempt (() => browser .setBrowserOption (name, true)),
         property: attempt (() => browser .getBrowserProperty (name)),
         rendering: attempt (() => browser .getRenderingProperty (name)),
      }));
   });
}

// The browser properties, all constants.
async function properties (page)
{
   await load (page);

   return page .evaluate (([ names ]) =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;

      return Object .fromEntries (names .map (name =>
         [ name, [ attempt (() => browser .getBrowserProperty (name)), attempt (() => typeof browser .getBrowserProperty (name)) ] ]));
   }, [ PROPERTIES ]);
}

// getRenderingProperty. X_ITE's values are those of the context (the shading name, the samples clamped to the
// context's maximum, ...). The five properties of the options: the first values, LogarithmicDepthBuffer set, the alias and the unknown names.
async function rendering (page)
{
   await load (page);

   return page .evaluate (async ([ names ]) =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const read    = list => Object .fromEntries (list .map (name => [ name, attempt (() => browser .getRenderingProperty (name)) ]));
      const out     = { defaults: read (names), logarithmic: { } };

      for (const value of [ true, false, 1, 0 ])
      {
         attempt (() => browser .setBrowserOption ("LogarithmicDepthBuffer", value));
         await frames ();
         out .logarithmic [String (value)] = attempt (() => browser .getRenderingProperty ("LogarithmicDepthBuffer"));
      }

      out .alias   = attempt (() => browser .getRenderingProperty ("AntiAliased"));
      out .unknown = [ "Bogus", "", "shading", "BrowserOptions", "set_Shading", "Shading_changed", "LogarithmicDepthBuffer_changed", 5, null, undefined ]
         .map (name => attempt (() => browser .getRenderingProperty (name)));

      return out;
   }, [ RENDERING ]);
}

// The properties X_ITE derives from the options: Shading, Antialiased, ContentScale and Multisampling after options
// are set (the maximum number of samples is the context's, so no value above 4 is set).
async function renderingDerived (page)
{
   await load (page);

   return page .evaluate (async ([ names ]) =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const read    = list => Object .fromEntries (list .map (name => [ name, attempt (() => browser .getRenderingProperty (name)) ]));
      const out     = { };

      for (const [ name, value ] of [
         [ "Shading", "PHONG" ], [ "Shading", "WIREFRAME" ], [ "Shading", "POINTSET" ], [ "Shading", "x" ], [ "Shading", "FLAT" ],
         [ "ContentScale", 2 ], [ "ContentScale", 1 ],
         [ "Multisampling", 2 ], [ "Multisampling", 0 ], [ "Multisampling", NaN ], [ "Multisampling", 4 ],
         [ "Antialiased", false ], [ "Antialiased", true ],
      ])
      {
         attempt (() => browser .setBrowserOption (name, value));
         await frames ();
         out [`${name}=${show (value)}`] = { option: attempt (() => browser .getBrowserOption (name)), properties: read (names) };
      }

      return out;
   }, [ [ "Shading", "Antialiased", "ContentScale", "Multisampling" ] ]);
}

// The properties of X_ITE's WebGL context (the values are those of the GPU the recording was made on).
async function renderingContext (page)
{
   await load (page);

   return page .evaluate (([ names ]) =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;

      return Object .fromEntries (names .map (name => [ name, attempt (() => browser .getRenderingProperty (name)) ]));
   }, [ RENDERING_OTHER ]);
}

// print and println: the console messages (the arguments joined with a space).
async function output (page)
{
   await load (page);

   page .log .length = 0;

   const returned = await page .evaluate (() =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const X3D     = window .X3D_NS;

      return [
         attempt (() => browser .print ("a", 1, true)),
         attempt (() => browser .println ("x", "y")),
         attempt (() => browser .print ()),
         attempt (() => browser .println ()),
         attempt (() => browser .print (null, undefined, [ 1, 2 ], { })),
         attempt (() => browser .println (new X3D .SFVec3f (1, 2, 3), new X3D .SFBool (true))),
         attempt (() => browser .print ("line1\nline2")),
         attempt (() => browser .println (0, -0, NaN, 1.5)),
         attempt (() => browser .println ("")),
      ];
   });

   await settle (page, 100);

   // Only the messages of print and println: a warning or error of the page would be a failure.
   return { returned, log: [ ... page .log ] };
}

// browser.description and the notification it shows (Notification.js of X_ITE 16.5.0): the element
// `.x_ite-private-notification > span` in the shadow DOM, its text and inline styles, the 5000 ms hide, the Notifications
// option. The 5000 ms timer is controlled: the page takes over setTimeout and clearTimeout for that delay only, holds the
// callback and runs it when the case says so, so no case waits for the hide. What is compared is the text, the class,
// the inline visibility and transition, whether the inline width is the width of the span, and a few computed styles (not
// the visibility, which a running transition keeps for 300 ms);
// not the pixel width (fonts) and not the background (X_ITE draws a logo from its assets).
async function notification (page)
{
   await load (page);

   return page .evaluate (async () =>
   {
      const canvas  = document .querySelector ("x3d-canvas");
      const browser = canvas .browser;
      const held    = [ ];
      const realSet = window .setTimeout;
      const realClr = window .clearTimeout;
      const BASE    = 900000000;

      window .setTimeout = (fn, delay, ... args) =>
      {
         if (delay !== 5000)
            return realSet (fn, delay, ... args);

         held .push ({ fn, delay, cleared: false, ran: false });

         return BASE + held .length - 1;
      };

      window .clearTimeout = id =>
      {
         if (id >= BASE && id < BASE + held .length)
            held [id - BASE] .cleared = true;
         else
            realClr (id);
      };

      const element = () => canvas .shadowRoot .querySelector (".x_ite-private-notification");

      const snap = () =>
      {
         const el = element ();

         if (!el)
            return null;

         const span     = el .firstElementChild;
         const computed = getComputedStyle (el);

         return {
            tag: el .tagName, className: el .className, children: Array .from (el .children, child => child .tagName),
            text: span ?.textContent ?? null, visibility: el .style .visibility, transition: el .style .transition,
            widthIsSpanWidth: el .style .width === `${span ?.clientWidth}px`, widthPositive: parseFloat (el .style .width) > 0,
            width: el .style .width === "0px" ? "0px" : el .style .width === "" ? "" : "set",
            inShadow: canvas .shadowRoot .contains (el),
            computed: [ computed .position, computed .top, computed .right, computed .whiteSpace, computed .pointerEvents ],
         };
      };

      const timers = () => held .map (t => [ t .delay, t .cleared, t .ran ]);
      const run    = () => { const t = held .findLast (t => !t .cleared && !t .ran); t .ran = true; t .fn (); };
      const state  = async () => { const immediate = snap (); await frames (); return { immediate, snap: snap (), timers: timers (), description: attempt (() => browser .description) }; };
      const out    = { };

      try
      {
         out .api = {
            inBrowser: "description" in browser, getter: typeof browser .getDescription, setter: typeof browser .setDescription,
            initial: attempt (() => browser .description), initialGetter: attempt (() => browser .getDescription ()),
         };
         out .initial = { snap: snap (), timers: timers (), visibility: getComputedStyle (element ()) .visibility };

         // The page sets notifications="false": nothing shows until the option is set.
         out .optionAtStart = browser .getBrowserOption ("Notifications");
         browser .description = "pre";
         out .pageDefault = await state ();

         browser .setBrowserOption ("Notifications", true);
         browser .description = "Hello";
         out .show = await state ();

         browser .description = "Hello";
         out .same = await state ();

         browser .description = "A\nB";
         browser .description = "C";
         out .replace = await state ();

         browser .setDescription ("D");
         out .method = await state ();

         browser .description = "";
         out .empty = await state ();

         run ();
         out .hidden = await state ();

         browser .description = "again";
         out .again = await state ();

         browser .setBrowserOption ("Notifications", false);
         browser .description = "off";
         out .off = await state ();
         run ();
         out .offRun = await state ();

         browser .setBrowserOption ("Notifications", true);
         browser .description = "on";
         out .on = await state ();

         out .coercion = { };

         for (const [ key, value ] of [ [ "number", 5 ], [ "null", null ], [ "undefined", undefined ], [ "true", true ], [ "object", { } ], [ "array", [ 1, 2 ] ] ])
         {
            const set = attempt (() => { browser .description = value; return "ok"; });

            out .coercion [key] = { set, ... await state () };
         }
      }
      finally
      {
         window .setTimeout   = realSet;
         window .clearTimeout = realClr;
      }

      return out;
   });
}

const cases = { defaults, setValid, coercion, unknownNames, properties, rendering, renderingDerived, renderingContext, output, notification };

for (const [ name, run ] of Object .entries (cases))
   cases [name] = async (page, env) => { await page .evaluate (pageAttempt); return run (page, env); };

export const CASE_GROUPS = {
   browserOptions: { context: DESKTOP, cases },
};
