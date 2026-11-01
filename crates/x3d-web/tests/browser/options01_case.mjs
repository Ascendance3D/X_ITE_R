// The browser options 01 cases (issue #136): getBrowserOption, setBrowserOption, getBrowserProperty,
// getRenderingProperty, print and println; part 4 (issue #141): browser.description and the notification; part 5: the keys of the browser (debug shortcuts).
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

async function loadWorld (page, url)
{
   await page .evaluate (pageLoadURL, [ url ]);

   const result = await page .evaluate (() => window .pending);

   if (result !== "resolved")
      throw new Error (`the world did not load: ${result}\n${page .log .join ("\n")}`);

   await settle (page, 300);
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

// The keys that X3DCoreContext handles itself ([_keydown] of X_ITE 16.5.0), sent with Playwright's keyboard to the focused
// canvas: Home, End, Page Up and Page Down change the viewpoint; Ctrl+Plus toggles the Timings option; Ctrl+1 to Ctrl+5
// (Shading) and Ctrl+S (end/begin update) act only with the Debug option set. Compared after each key: whether the page's
// keydown listener saw `defaultPrevented`, the bound viewpoint, the options Timings, Shading and Debug, isLive, and what a
// KeySensor of the world got (the feed to the world stays; the sensor is disabled until the end, because an enabled
// KeySensor calls preventDefault for every key). Not compared: Shift+F8 (clipboard), the notification text.
const KEYS_WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Interchange
COMPONENT KeyDeviceSensor:1
DEF V1 Viewpoint { description "V1" position 0 0 10 }
DEF V2 Viewpoint { description "V2" position 0 0 12 }
DEF V3 Viewpoint { description "V3" position 0 0 14 }
DEF K KeySensor { enabled FALSE }
Shape { geometry Box { } }
`)}`;

async function debugKeys (page)
{
   await page .evaluate (pageLoadURL, [ KEYS_WORLD ]);

   const result = await page .evaluate (() => window .pending);

   if (result !== "resolved")
      throw new Error (`the world did not load: ${result}\n${page .log .join ("\n")}`);

   await settle (page, 100);

   await page .evaluate (() =>
   {
      const canvas  = document .querySelector ("x3d-canvas");
      const browser = canvas .browser;
      const scene   = browser .currentScene;

      window .seen   = [ ];
      window .pressed = [ ];

      window .addEventListener ("keydown", event => window .seen .push ([ event .keyCode, event .defaultPrevented ]));
      scene .getNamedNode ("K") .addFieldCallback ("keys", "keyPress", value => window .pressed .push (String (value)));
      canvas .focus ();
   });

   const frames = () => page .evaluate (() => new Promise (r => requestAnimationFrame (() => requestAnimationFrame (() => requestAnimationFrame (() => requestAnimationFrame (() => r ())))))) ;

   const state = () => page .evaluate (() =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const scene   = browser .currentScene;
      const seen    = window .seen .splice (0);
      const pressed = window .pressed .splice (0);

      return {
         seen, pressed,
         bound:   [ "V1", "V2", "V3" ] .filter (name => scene .getNamedNode (name) .isBound) .join (","),
         timings: browser .getBrowserOption ("Timings"), shading: browser .getBrowserOption ("Shading"),
         rendering: browser .getRenderingProperty ("Shading"), live: browser .isLive (), debug: browser .getBrowserOption ("Debug"),
      };
   });

   const out = { initial: await state () };

   const press = async (label, ... keys) =>
   {
      for (const key of keys)
         await page .keyboard .press (key);

      await frames ();
      out [label] = await state ();
   };

   // The viewpoint keys, with and without Debug.
   await press ("end",      "End");
   await press ("home",     "Home");
   await press ("pageDown", "PageDown");
   await press ("pageDown2", "PageDown");
   await press ("pageUp",   "PageUp");
   await press ("pageDown3", "PageDown", "PageDown", "PageDown");

   // Ctrl+Plus: the Timings option (Ctrl+= is keyCode 187, as Ctrl+Shift+= with the plus sign).
   await press ("plainPlus", "=");
   await press ("ctrlPlus",  "Control+=");
   await press ("ctrlPlus2", "Control+Shift+=");
   await press ("ctrlPlus3", "Control+=");

   // The Debug keys with the Debug option false: nothing happens and the keys are not claimed.
   await press ("debugOffCtrl1", "Control+1");
   await press ("debugOffCtrlS", "Control+s");

   await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .setBrowserOption ("Debug", true));

   await press ("plain1", "1");
   await press ("plainS", "s");
   await press ("ctrl1", "Control+1");
   await press ("ctrl2", "Control+2");
   await press ("ctrl3", "Control+3");
   await press ("ctrl4", "Control+4");
   await press ("ctrl5", "Control+5");
   await press ("shiftCtrl1", "Control+Shift+!");
   await press ("ctrlS",  "Control+s");
   await press ("ctrlS2", "Control+s");
   await press ("altCtrl1", "Alt+Control+1");
   await press ("keyDisabled", "a");

   // An enabled KeySensor gets the keys as before (and claims every key, as KeySensor.js does).
   await page .evaluate (() => { document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("K") .enabled = true; });
   await press ("key",    "a", "Shift+b", "Home");

   // KeySensor.js calls preventDefault for every key while it is enabled; X_ITE_R's KeySensor does not (API.md, known
   // differences): only the key codes of this step are compared, not whether they were claimed.
   out .key .seen = out .key .seen .map (([ code ]) => code);

   return out;
}

// LoadUrlObjects (X3DUrlObject.requestImmediateLoad of X_ITE 16.5.0): with the option FALSE an Inline of a world does not
// start to load. The option is read when a load starts, so a load that is complete stays, setting the option TRUE does not
// start the load by itself, and a new `url` does. Observed through the public SAI: the Inline's document has the only
// Viewpoint, so `browser.firstViewpoint ()` binds it (`browser.activeViewpoint`, its description) only after the Inline has
// loaded; an Inline's viewpoints are listed, not bound by themselves.
const INNER = encodeURIComponent (`#X3D V4.0 utf8
PROFILE Full
Viewpoint { description "inline viewpoint" }
`);

const URL_WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Interchange
DEF I Inline { url "data:model/x3d-vrml,${INNER}" }
Shape { geometry Box { } }
`)}`;

async function loadUrlObjects (page)
{
   const browser = fn => page .evaluate (fn);
   const prints  = async () =>
   {
      await settle (page, 500);
      await browser (() => document .querySelector ("x3d-canvas") .browser .firstViewpoint ());
      await settle (page, 300);

      return browser (() => document .querySelector ("x3d-canvas") .browser .activeViewpoint ?.description ?? null);
   };

   const out = { };

   // The default: the Inline loads.
   await loadWorld (page, URL_WORLD);
   out .default = { option: await browser (() => document .querySelector ("x3d-canvas") .browser .getBrowserOption ("LoadUrlObjects")), active: await prints () };

   // The option FALSE before the world: the Inline does not start.
   await browser (() => document .querySelector ("x3d-canvas") .browser .setBrowserOption ("LoadUrlObjects", false));
   await loadWorld (page, URL_WORLD);
   out .off = { option: await browser (() => document .querySelector ("x3d-canvas") .browser .getBrowserOption ("LoadUrlObjects")), active: await prints () };

   // A new url with the option still FALSE: still nothing.
   const setUrl = url => page .evaluate (url => { document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("I") .url = [ url ]; }, url);

   await setUrl (`data:model/x3d-vrml,${INNER}`);
   await settle (page, 300);
   out .offUrl = await prints ();

   // The option TRUE: nothing starts by itself; a new url does.
   await browser (() => document .querySelector ("x3d-canvas") .browser .setBrowserOption ("LoadUrlObjects", true));
   await settle (page, 300);
   out .onIdle = await prints ();

   await setUrl (`data:model/x3d-vrml,${INNER}`);
   await settle (page, 300);
   out .onUrl = await prints ();

   // A load that is complete stays when the option goes FALSE, and a new url then loads nothing.
   await browser (() => document .querySelector ("x3d-canvas") .browser .setBrowserOption ("LoadUrlObjects", false));
   await setUrl (`data:model/x3d-vrml,${INNER}#again`);
   await settle (page, 300);
   out .completeThenOff = await prints ();

   await browser (() => document .querySelector ("x3d-canvas") .browser .setBrowserOption ("LoadUrlObjects", true));

   return out;
}

// EnableInlineViewpoints (X3DLayerNode.getUserViewpoints of X_ITE 16.5.0): the viewpoints with a description that
// firstViewpoint, previousViewpoint, nextViewpoint and lastViewpoint walk include those of Inline scenes only while the
// option is TRUE; the option is read at each call. Observed through the public SAI: the description of
// `browser.activeViewpoint` after each call, with the option true (the default), false, and true again.
const INLINE_VP = encodeURIComponent (`#X3D V4.0 utf8
PROFILE Interchange
Viewpoint { description "inline one" }
Viewpoint { description "inline two" }
`);

const VP_WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Interchange
Viewpoint { description "main one" }
Inline { url "data:model/x3d-vrml,${INLINE_VP}" }
Viewpoint { description "main two" }
Viewpoint { }
`)}`;

async function enableInlineViewpoints (page)
{
   await loadWorld (page, VP_WORLD);
   await settle (page, 500);

   const walk = () => page .evaluate (async () =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const wait    = () => new Promise (resolve => setTimeout (resolve, 150));
      const active  = () => browser .activeViewpoint ?.description ?? null;
      const out     = { option: browser .getBrowserOption ("EnableInlineViewpoints") };

      for (const call of [ "firstViewpoint", "nextViewpoint", "nextViewpoint", "nextViewpoint", "nextViewpoint", "nextViewpoint",
                           "previousViewpoint", "previousViewpoint", "lastViewpoint", "previousViewpoint" ])
      {
         browser [call] ();
         await wait ();
         (out .sequence ??= [ ]) .push ([ call, active () ]);
      }

      return out;
   });

   const out = { };

   out .on = await walk ();

   await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .setBrowserOption ("EnableInlineViewpoints", false));
   out .off = await walk ();

   await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .setBrowserOption ("EnableInlineViewpoints", true));
   out .onAgain = await walk ();

   return out;
}

// StraightenHorizon (BrowserOptions.set_StraightenHorizon__ and X3DBrowser.viewAll of X_ITE 16.5.0): setting the option
// TRUE straightens the active layer's view with a one second transition (X3DViewpointNode.straightenView); viewAll
// straightens the new view while the option is TRUE. Observed through the public SAI: the `orientation_changed` and
// `position_changed` of a ProximitySensor around the world (the user's view, rounded to four decimals) after the
// transition, and after `viewAll (0)`.
const STRAIGHTEN_WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Interchange
DEF P ProximitySensor { size 1e6 1e6 1e6 }
Viewpoint { position 0 0 10 orientation 0 0 1 0.6 }
Transform { translation 2 1 0 children Shape { geometry Box { size 2 3 1 } } }
`)}`;

async function straightenHorizon (page)
{
   const read = () => page .evaluate (() =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const P       = browser .currentScene .getNamedNode ("P");
      const round   = array => array .map (value => Math .round (value * 10000) / 10000 + 0);

      return { option: browser .getBrowserOption ("StraightenHorizon"), orientation: round ([ P .orientation_changed .x, P .orientation_changed .y, P .orientation_changed .z, P .orientation_changed .angle ]), position: round ([ P .position_changed .x, P .position_changed .y, P .position_changed .z ]) };
   });

   const out = { };

   const option = value => page .evaluate (value => document .querySelector ("x3d-canvas") .browser .setBrowserOption ("StraightenHorizon", value), value);

   // The default (TRUE): the view of a rolled viewpoint is straight at once.
   await loadWorld (page, STRAIGHTEN_WORLD);
   out .initial = await read ();

   // FALSE before the world: the view stays rolled, whatever the option does later when it stays FALSE; TRUE straightens it.
   await option (false);
   await loadWorld (page, STRAIGHTEN_WORLD);
   out .off = await read ();

   await option (false);
   await settle (page, 1500);
   out .offAgain = await read ();

   await option (true);
   await settle (page, 1800);
   out .on = await read ();

   // viewAll with the option FALSE and TRUE, each on a fresh world with a rolled view.
   for (const value of [ false, true ])
   {
      await option (false);
      await loadWorld (page, STRAIGHTEN_WORLD);
      await option (value);
      await settle (page, 1800);
      await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .viewAll (0));
      await settle (page, 500);
      out [value ? "viewAllOn" : "viewAllOff"] = await read ();
   }

   await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .setBrowserOption ("StraightenHorizon", true));

   return out;
}

// Gravity (X3DRenderObject.gravitate of X_ITE 16.5.0): a WALK viewer above the floor falls with the acceleration `Gravity`
// (the speed grows by Gravity / frame rate) and stops at the avatar height; with Gravity 0 it never starts to fall. The
// option is read in every frame. Observed through the public SAI: the user's position (a ProximitySensor's
// `position_changed`, four decimals) after the frames of the fall, which the floor and the avatar height fix.
const GRAVITY_WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Interchange
DEF P ProximitySensor { size 1e6 1e6 1e6 }
NavigationInfo { type "WALK" avatarSize [ 0.25 1.6 0.75 ] }
Viewpoint { position 0 3 10 }
Transform { translation 0 -0.1 0 children Shape { geometry Box { size 100 0.2 100 } } }
`)}`;

async function gravity (page)
{
   const read = () => page .evaluate (() =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const P       = browser .currentScene .getNamedNode ("P");
      const round   = array => array .map (value => Math .round (value * 10000) / 10000 + 0);

      return { option: browser .getBrowserOption ("Gravity"), position: round ([ P .position_changed .x, P .position_changed .y, P .position_changed .z ]) };
   });

   const option = value => page .evaluate (value => document .querySelector ("x3d-canvas") .browser .setBrowserOption ("Gravity", value), value);
   const out    = { };

   // The default: the viewer falls from 3 to the avatar height 1.6 above the floor.
   await loadWorld (page, GRAVITY_WORLD);
   await settle (page, 3000);
   out .default = await read ();

   // Gravity 0 before the world: the viewer stays where the viewpoint put it.
   await option (0);
   await loadWorld (page, GRAVITY_WORLD);
   await settle (page, 3000);
   out .zero = await read ();

   // The option is read in every frame: the viewer falls when it is set.
   await option (9.80665);
   await settle (page, 3000);
   out .restored = await read ();

   await option (9.80665);

   return out;
}

const cases = { defaults, setValid, coercion, unknownNames, properties, rendering, renderingDerived, renderingContext, output, notification, debugKeys, loadUrlObjects, enableInlineViewpoints, straightenHorizon, gravity };

for (const [ name, run ] of Object .entries (cases))
   cases [name] = async (page, env) => { await page .evaluate (pageAttempt); return run (page, env); };

export const CASE_GROUPS = {
   browserOptions: { context: DESKTOP, cases },
};
