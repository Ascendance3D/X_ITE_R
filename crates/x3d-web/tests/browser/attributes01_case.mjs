// The canvas attributes 01 cases (issue #136, part 3): the attributes of <x3d-canvas> and the browser options they
// set (the attribute at start and on change, removed), the options that do not write the attributes back, the
// framebuffer size of contentScale (attribute and option), MaximumFrameRate and the `update` attribute with the
// AutoUpdate option (the frame loop stopped and started). oracle/x_ite-web-runtime-02.mjs (--suite attributes01) runs
// them against pinned X_ITE 16.5.0 and records the values; attributes01.test.mjs runs them against X_ITE_R and
// compares (runtime02_harness.mjs).
//
// What the recordings hold is what both engines can give on any machine: the option values, the framebuffer size,
// and the TimeSensor `time` events of a frame loop as ceilings (rate * seconds + 1) and as "none" or "some", never
// as exact counts.

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { pageLoadURL } from "./loadurl01_case.mjs";
import { pageAttempt } from "./options01_case.mjs";

// A world with a looping TimeSensor, whose `time` events are the frames the browser processes.
const WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Interchange
DEF T TimeSensor { loop TRUE cycleInterval 1 }
Shape { geometry Box { } }
`)}`;

// X3DCanvasElement.observedAttributes that set an option (X3DCoreContext.attributeChangedCallback of X_ITE 16.5.0):
// attribute, option, the values tried. Every value goes through the attribute change; the first is also the start
// value of a new element.
const BOOLEANS = [ "true", "false", "TRUE", "", "x" ];
const STRINGS  = [ "LINEAR", "", "x" ];
const NUMBERS  = [ "2.5", "x", "", "0", "-1", "3" ];

const ATTRIBUTES = [
   [ "antialiased", "Antialiased", BOOLEANS ], [ "cache", "Cache", BOOLEANS ],
   [ "colorSpace", "ColorSpace", STRINGS ], [ "contentScale", "ContentScale", [ "2", "auto", "x", "", "0", "0.5" ] ],
   [ "contextMenu", "ContextMenu", BOOLEANS ], [ "debug", "Debug", BOOLEANS ],
   [ "displayColorSpace", "DisplayColorSpace", [ "DISPLAY_P3", "", "x" ] ], [ "exposure", "Exposure", NUMBERS ],
   [ "logarithmicDepthBuffer", "LogarithmicDepthBuffer", BOOLEANS ], [ "maximumFrameRate", "MaximumFrameRate", NUMBERS ],
   [ "multisampling", "Multisampling", NUMBERS ], [ "notifications", "Notifications", BOOLEANS ],
   [ "orderIndependentTransparency", "OrderIndependentTransparency", BOOLEANS ],
   [ "splashScreen", "SplashScreen", BOOLEANS ], [ "textCompression", "TextCompression", [ "SCALING", "", "x" ] ],
   [ "timings", "Timings", BOOLEANS ], [ "toneMapping", "ToneMapping", [ "ACES_NARKOWICZ", "", "x" ] ],
   [ "update", "AutoUpdate", [ "auto", "true", "false", "TRUE", "", "x" ] ],
   [ "xrSessionMode", "XRSessionMode", [ "IMMERSIVE_AR", "", "x" ] ],
];

// Every option, to prove that none writes an attribute.
const OPTION_VALUES = [
   [ "SplashScreen", false ], [ "Antialiased", false ], [ "AutoUpdate", true ], [ "Cache", false ], [ "ColorSpace", "LINEAR" ],
   [ "ContentScale", 3 ], [ "ContextMenu", false ], [ "Debug", true ], [ "DisplayColorSpace", "DISPLAY_P3" ], [ "Exposure", 2 ],
   [ "LogarithmicDepthBuffer", true ], [ "MaximumFrameRate", 30 ], [ "Multisampling", 2 ], [ "Notifications", false ],
   [ "OrderIndependentTransparency", true ], [ "TextCompression", "SCALING" ], [ "Timings", true ],
   [ "ToneMapping", "ACES_NARKOWICZ" ], [ "XRSessionMode", "IMMERSIVE_AR" ], [ "Mute", true ], [ "SoundIntensity", 0.5 ],
];

const OBSERVED = [ "antialiased", "baseURL", "cache", "colorSpace", "contentScale", "contextMenu", "debug", "displayColorSpace",
   "exposure", "extensions", "logarithmicDepthBuffer", "maximumFrameRate", "multisampling", "notifications", "orderIndependentTransparency",
   "splashScreen", "src", "textCompression", "timings", "toneMapping", "update", "url", "xrSessionMode" ];

async function load (page)
{
   await page .evaluate (pageLoadURL, [ WORLD ]);

   const result = await page .evaluate (() => window .pending);

   if (result !== "resolved")
      throw new Error (`the world did not load: ${result}\n${page .log .join ("\n")}`);

   await settle (page, 100);
}

// In the page: the TimeSensor `time` events since the call of `watch`, and a window of `ms` milliseconds.
function pageWatch ()
{
   window .ticks = 0;

   const T = document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("T");

   T .getField ("time") .addFieldCallback ("count", () => ++ window .ticks);
}

function pageWindow ()
{
   window .framebuffer = () =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const canvas  = browser .getCanvas ?.() ?? browser .getContext () .canvas;

      return [ canvas .width, canvas .height ];
   };

   window .count = async (ms, reset = true) =>
   {
      if (reset)
         window .ticks = 0;

      const start = performance .now ();

      await new Promise (resolve => setTimeout (resolve, ms));

      return { ticks: window .ticks, seconds: (performance .now () - start) / 1000 };
   };

   // The result of a window as the recording keeps it: no events, some events, or no more than rate * seconds + 1.
   window .summary = (result, rate) => ({
      ... rate !== undefined && rate < 10 ? { } : { none: result .ticks === 0, some: result .ticks > 0 },
      // A window of a second holds no event for sure only above a few frames a second.
      ... rate === undefined ? { } : { atMostRate: result .ticks <= Math .floor (rate * result .seconds) + 1 },
   });
}

const watch = page => page .evaluate (pageWatch);

// --- the attributes and the options -----------------------------------------------------------------------------

// Each attribute on its own element. The start value (the element made by the parser with the attribute), then each
// value set, then the attribute removed; the option after each. `show` writes NaN as text.
async function options (page)
{
   return page .evaluate (async ([ attributes ]) =>
   {
      const out       = { };
      const container = document .createElement ("div");

      container .style .cssText = "position:fixed;left:0;top:0;width:60px;height:40px";
      document .body .append (container);

      for (const [ attribute, option, values ] of attributes)
      {
         container .innerHTML = `<x3d-canvas ${attribute}="${values [0]}" style="display:block;width:50px;height:30px"></x3d-canvas>`;

         const element = container .firstElementChild;
         const browser = element .browser;
         const read    = () => [ attempt (() => browser .getBrowserOption (option)), attempt (() => typeof browser .getBrowserOption (option)) ];
         const steps   = [ ];

         for (const value of values)
         {
            element .setAttribute (attribute, value);
            steps .push ([ value, ... read () ]);
         }

         element .removeAttribute (attribute);
         steps .push ([ null, ... read () ]);

         out [attribute] = { option, start: [ values [0], ... steps [0] .slice (1) ], steps };

         // An attribute that was set again with the value it has, and the attribute in capitals (the HTML parser lowercases it).
         element .setAttribute (attribute, values [0]);
         element .setAttribute (attribute .toUpperCase (), values [1]);
         out [attribute] .upper = read ();

         browser .dispose ();
         container .innerHTML = "";
      }

      container .remove ();

      return out;
   }, [ ATTRIBUTES ]);
}

// An element with several attributes at start; a second one with none. The options of each, and the attributes read back.
async function startValues (page)
{
   return page .evaluate (async ([ attributes ]) =>
   {
      const container = document .createElement ("div");

      container .style .cssText = "position:fixed;left:0;top:0;width:60px;height:40px";
      document .body .append (container);

      const read = element => Object .fromEntries (attributes .map (([ , option ]) => [ option, attempt (() => element .browser .getBrowserOption (option)) ]));
      const out  = { };

      container .innerHTML = `<x3d-canvas splashScreen="false" contextMenu="false" notifications="false" timings="true" contentScale="2" exposure="0.5"
         maximumFrameRate="25" multisampling="2" toneMapping="ACES_NARKOWICZ" debug="true" update="false" cache="false"
         style="display:block;width:50px;height:30px"></x3d-canvas>`;
      out .attributes = read (container .firstElementChild);
      container .firstElementChild .browser .dispose ();

      container .innerHTML = `<x3d-canvas style="display:block;width:50px;height:30px"></x3d-canvas>`;
      out .none = read (container .firstElementChild);
      container .firstElementChild .browser .dispose ();

      // createElement and setAttribute before the element is in the document.
      const element = document .createElement ("x3d-canvas");

      element .setAttribute ("exposure", "0.25");
      element .setAttribute ("update", "auto");
      out .created = read (element);
      container .append (element);
      out .connected = read (element);
      element .browser .dispose ();

      container .remove ();

      return out;
   }, [ ATTRIBUTES ]);
}

// An option set does not change the attribute; the page's own element, which has attributes.
async function optionsKeepAttributes (page)
{
   return page .evaluate (async ([ values, observed ]) =>
   {
      const element = document .querySelector ("x3d-canvas");
      const browser = element .browser;
      const read    = () => Object .fromEntries (observed .map (name => [ name, element .getAttribute (name) ]));
      const out     = { before: read (), steps: [ ] };

      for (const [ name, value ] of values)
      {
         attempt (() => browser .setBrowserOption (name, value));
         out .steps .push ([ name, attempt (() => browser .getBrowserOption (name)), read () ]);
      }

      // An attribute set after an option set: the attribute wins; and the option set again after it.
      element .setAttribute ("exposure", "0.75");
      out .attributeAfter = attempt (() => browser .getBrowserOption ("Exposure"));
      browser .setBrowserOption ("Exposure", 4);
      out .optionAfter = [ attempt (() => browser .getBrowserOption ("Exposure")), element .getAttribute ("exposure") ];

      return out;
   }, [ OPTION_VALUES, OBSERVED ]);
}

// The attributes that are not options: baseURL (a base for the relative URLs), extensions and the event handlers. Only
// what a page can read of them in both engines: that setting them does not throw and does not change an option.
async function otherAttributes (page)
{
   return page .evaluate (async () =>
   {
      const container = document .createElement ("div");

      container .style .cssText = "position:fixed;left:0;top:0;width:60px;height:40px";
      document .body .append (container);
      container .innerHTML = `<x3d-canvas style="display:block;width:50px;height:30px"></x3d-canvas>`;

      const element = container .firstElementChild;
      const browser = element .browser;
      const out     = { };

      out .baseURL = [ attempt (() => String (browser .baseURL) .startsWith ("http://")) ];
      attempt (() => element .setAttribute ("baseURL", "http://base.test/dir/"));
      out .baseURL .push (attempt (() => browser .baseURL === "http://base.test/dir/"));

      attempt (() => element .setAttribute ("extensions", "x"));
      out .extensions = attempt (() => element .getAttribute ("extensions"));

      window .fired = [ ];
      attempt (() => element .setAttribute ("oninitialized", "window.fired.push ('initialized')"));
      attempt (() => element .setAttribute ("onshutdown", "window.fired.push ('shutdown')"));
      out .handlers = [ typeof element .oninitialized, typeof element .onshutdown ];

      browser .dispose ();
      container .remove ();

      return out;
   });
}

// --- contentScale -----------------------------------------------------------------------------------------------

// The framebuffer of the page's canvas (400 x 300 CSS pixels) after each attribute value and option value, and the
// option after the attribute. `ratio` is the device pixel ratio of the context.
async function contentScale (page)
{
   return page .evaluate (async ([ steps ]) =>
   {
      const element = document .querySelector ("x3d-canvas");
      const browser = element .browser;
      const out     = { ratio: devicePixelRatio, start: [ framebuffer (), attempt (() => browser .getBrowserOption ("ContentScale")), element .getAttribute ("contentScale") ], steps: [ ] };

      for (const [ kind, value ] of steps)
      {
         if (kind === "attribute")
            value === null ? element .removeAttribute ("contentScale") : element .setAttribute ("contentScale", value);
         else
            attempt (() => browser .setBrowserOption ("ContentScale", value));

         await frames ();

         out .steps .push ([ kind, show (value), framebuffer (), attempt (() => browser .getBrowserOption ("ContentScale")), element .getAttribute ("contentScale") ]);
      }

      // The CSS size changes while the scale is auto and while it is 2.
      for (const scale of [ "auto", "2" ])
      {
         element .setAttribute ("contentScale", scale);
         element .style .width  = "200px";
         element .style .height = "100px";
         await frames ();
         out [`resized ${scale}`] = framebuffer ();
         element .style .width  = "400px";
         element .style .height = "300px";
         await frames ();
      }

      return out;
   }, [ [
      [ "attribute", "2" ], [ "attribute", "0.5" ], [ "attribute", "auto" ], [ "attribute", "x" ], [ "attribute", "0" ], [ "attribute", "-3" ],
      [ "attribute", "" ], [ "attribute", "3" ], [ "attribute", null ],
      [ "option", 2 ], [ "option", -1 ], [ "option", 0 ], [ "option", 0.25 ], [ "option", 1 ], [ "option", NaN ], [ "option", -5 ],
   ] ]);
}

// --- frame rate and update --------------------------------------------------------------------------------------

// The MaximumFrameRate set by the attribute and by the option: the `time` events of one second never exceed the
// rate (ceilings only: a slow machine runs fewer frames).
async function frameRate (page)
{
   await load (page);
   await watch (page);

   return page .evaluate (async () =>
   {
      const element = document .querySelector ("x3d-canvas");
      const browser = element .browser;
      const out     = { };

      out .unlimited = summary (await count (500), 1000);

      for (const rate of [ 10, 30, 0, 1 ])
      {
         element .setAttribute ("maximumFrameRate", String (rate));
         await frames ();
         out [`attribute ${rate}`] = [ attempt (() => browser .getBrowserOption ("MaximumFrameRate")), summary (await count (1000), rate) ];
      }

      for (const rate of [ 10, 20, -5, 0 ])
      {
         attempt (() => browser .setBrowserOption ("MaximumFrameRate", rate));
         await frames ();
         out [`option ${rate}`] = [ attempt (() => browser .getBrowserOption ("MaximumFrameRate")), summary (await count (1000), Math .max (rate, 1)) ];
      }

      // The limit lifted again.
      attempt (() => browser .setBrowserOption ("MaximumFrameRate", 1000));
      await frames ();
      out .lifted = summary (await count (500), 1000);

      return out;
   });
}

// The `update` attribute and the AutoUpdate option: the frame loop is stopped (no `time` events at all) and started.
async function update (page)
{
   await load (page);
   await watch (page);

   return page .evaluate (async () =>
   {
      const element = document .querySelector ("x3d-canvas");
      const browser = element .browser;
      const out     = { steps: [ ] };
      const option  = () => attempt (() => browser .getBrowserOption ("AutoUpdate"));
      const step    = async (name, action) =>
      {
         await action ();
         await frames ();
         out .steps .push ([ name, option (), summary (await count (300)) ]);
      };

      await step ("start", () => { });
      await step ("update=false", () => element .setAttribute ("update", "false"));
      await step ("update=true", () => element .setAttribute ("update", "true"));
      await step ("update=false again", () => element .setAttribute ("update", "false"));
      await step ("update=false repeated", () => element .setAttribute ("update", "false"));
      await step ("removed", () => element .removeAttribute ("update"));
      await step ("update=x", () => element .setAttribute ("update", "x"));
      await step ("update=false", () => element .setAttribute ("update", "false"));
      await step ("update=TRUE", () => element .setAttribute ("update", "TRUE"));
      await step ("update=false", () => element .setAttribute ("update", "false"));
      await step ("update=true, before auto", () => element .setAttribute ("update", "true"));
      await step ("update=auto", () => element .setAttribute ("update", "auto"));
      await step ("hidden", () => { element .style .display = "none"; });
      await step ("shown", () => { element .style .display = "block"; });
      await step ("option AutoUpdate false", () => browser .setBrowserOption ("AutoUpdate", false));
      await step ("hidden, no auto update", () => { element .style .display = "none"; });
      await step ("shown, no auto update", () => { element .style .display = "block"; });
      await step ("option AutoUpdate true", () => browser .setBrowserOption ("AutoUpdate", true));
      await step ("hidden, option", () => { element .style .display = "none"; });
      await step ("shown, option", () => { element .style .display = "block"; });
      await step ("update=false after auto", () => element .setAttribute ("update", "false"));
      await step ("hidden, update=false", () => { element .style .display = "none"; });
      await step ("shown, update=false", () => { element .style .display = "block"; });
      await step ("update=true", () => element .setAttribute ("update", "true"));
      await step ("update=false before auto", () => element .setAttribute ("update", "false"));
      await step ("update=auto while stopped", () => element .setAttribute ("update", "auto"));
      await step ("hidden, auto while stopped", () => { element .style .display = "none"; });
      await step ("shown, auto while stopped", () => { element .style .display = "block"; });
      await step ("update=true after auto", () => element .setAttribute ("update", "true"));

      return out;
   });
}

// loadURL while update="false": whether it resolves, and the events and time events after it.
async function loadWhileStopped (page)
{
   await page .evaluate (() => document .querySelector ("x3d-canvas") .setAttribute ("update", "false"));
   await settle (page, 100);

   const result = await page .evaluate (async ([ url ]) =>
   {
      const element = document .querySelector ("x3d-canvas");
      const browser = element .browser;
      const events  = [ ];
      const names   = [ "initialized", "load", "error", "shutdown" ];
      const record  = event => events .push (event .type);

      names .forEach (name => element .addEventListener (name, record));

      const outcome = await Promise .race ([
         browser .loadURL (new window .X3D_NS .MFString (url)) .then (() => "resolved", error => `rejected: ${error .message}`),
         new Promise (resolve => setTimeout (() => resolve ("pending after 3 s"), 3000)),
      ]);

      names .forEach (name => element .removeEventListener (name, record));

      return { outcome, events };
   }, [ WORLD ]);

   const out = { ... result };

   out .world = await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("T") ?.getTypeName ?.() ?? "no node")
      .catch (error => `error: ${error .message .split ("\n") [0]}`);

   await page .evaluate (pageWatch) .catch (() => { });
   out .stopped = await page .evaluate (async () => summary (await count (300)));

   await page .evaluate (() => document .querySelector ("x3d-canvas") .setAttribute ("update", "true"));
   await settle (page, 100);
   out .resumed = await page .evaluate (async () => summary (await count (300)));

   return out;
}

// An element made with update="false" and a src: whether it loads, and the time events afterward. The element is new
// so its start is the stopped state.
async function startStopped (page)
{
   return page .evaluate (async ([ url ]) =>
   {
      const container = document .createElement ("div");

      container .style .cssText = "position:fixed;left:0;top:0;width:60px;height:40px";
      document .body .append (container);

      const events = [ ];
      const out    = { };

      container .innerHTML = `<x3d-canvas update="false" style="display:block;width:50px;height:30px"></x3d-canvas>`;

      const element = container .firstElementChild;
      const browser = element .browser;

      for (const name of [ "initialized", "load", "error", "shutdown" ])
         element .addEventListener (name, event => events .push (event .type));

      await frames ();
      await frames ();
      out .startEvents = [ ... events ];
      out .option      = attempt (() => browser .getBrowserOption ("AutoUpdate"));

      out .load = await Promise .race ([
         browser .loadURL (new window .X3D_NS .MFString (url)) .then (() => "resolved", error => `rejected: ${error .message}`),
         new Promise (resolve => setTimeout (() => resolve ("pending after 3 s"), 3000)),
      ]);

      out .events = [ ... events ];

      browser .currentScene .getNamedNode ("T") ?.getField ("time") .addFieldCallback ("count", () => ++ window .ticks);
      out .stopped = summary (await count (300));

      element .setAttribute ("update", "true");
      await frames ();
      out .resumed = summary (await count (300));
      out .eventsAfterResume = [ ... events ];

      browser .dispose ();
      container .remove ();

      return out;
   }, [ WORLD ]);
}

// ----------------------------------------------------------------------------------------------------------------

const cases = { options, startValues, optionsKeepAttributes, otherAttributes, contentScale, frameRate, update, loadWhileStopped, startStopped };

for (const [ name, run ] of Object .entries (cases))
{
   cases [name] = async (page, env) =>
   {
      await page .evaluate (pageAttempt);
      await page .evaluate (pageWindow);

      return run (page, env);
   };
}

export const CASE_GROUPS = {
   canvasAttributes: { context: DESKTOP, cases },
   canvasAttributesScaled: { context: { ... DESKTOP, deviceScaleFactor: 2 }, cases: { contentScale: cases .contentScale } },
};
