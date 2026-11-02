// The browser user interface 01 cases (issue #141): the descriptions the browser sets itself, the splash screen, the timings
// panel, the context menu and Shift+F8 (Browser/Core/ContextMenu.js, BrowserTimings.js, Notification.js, X3DCoreContext.js and
// X3DNetworkingContext.js of X_ITE 16.5.0). oracle/x_ite-web-runtime-02.mjs (--suite ui01) runs them against pinned X_ITE 16.5.0
// and records the values; ui01.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs). Every case drives the
// real interface (Playwright's mouse and keyboard on the canvas, clicks on the menu entries) and reads the resulting state of
// the browser; the DOM snapshots (texts, classes, checked marks) are only the description of what was clicked.

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { pageLoadURL } from "./loadurl01_case.mjs";

const vrml = text => `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8\nPROFILE Full\n${text}`)}`;

const MENU_WORLD = vrml (`
META "creator" "Menu Test"
META "reference" "see https://example.org/page"
DEF V1 Viewpoint { description "First" position 0 0 10 }
DEF V2 Viewpoint { description "Second" position 0 0 12 }
Viewpoint { position 0 0 14 }
DEF NI NavigationInfo { type [ "EXAMINE" "WALK" "FLY" "ANY" ] }
WorldInfo { title "Menu World" info [ "line one" "mail me@example.org" ] }
DEF Animations Group { children [
   Group { children DEF T1 TimeSensor { description "Spin" cycleInterval 2 } }
   Group { children DEF T2 TimeSensor { description "Wave" cycleInterval 3 } }
] }
EXPORT Animations
Shape { geometry Box { } }
`);

const PLAIN_WORLD = vrml (`
NavigationInfo { type "EXAMINE" }
Shape { geometry Box { } }
`);

const SENSOR_WORLD = vrml (`
DEF V1 Viewpoint { description "Sensor View" position 0 0 4 }
Shape { appearance Appearance { material Material { } } geometry Box { size 4 4 4 } }
DEF TS TouchSensor { description "Touch me" }
Group { children [ DEF TS2 TouchSensor { description "" } ] }
`);

// A TouchSensor that covers the whole box, under the Box shape of the same group.
const TOUCH_WORLD = vrml (`
DEF V1 Viewpoint { description "Sensor View" position 0 0 4 }
Group { children [
   DEF TS TouchSensor { description "Touch me" }
   Shape { appearance Appearance { material Material { } } geometry Box { size 4 4 4 } }
] }
`);

const wait = (page, ms) => page .waitForTimeout (ms);
const frames = page => page .evaluate (() => new Promise (r => requestAnimationFrame (() => requestAnimationFrame (() => requestAnimationFrame (() => requestAnimationFrame (() => r ())))))) ;

async function loadWorld (page, url, ms = 300)
{
   await page .evaluate (pageLoadURL, [ url ]);

   const result = await page .evaluate (() => window .pending);

   if (result !== "resolved")
      throw new Error (`the world did not load: ${result}\n${page .log .join ("\n")}`);

   await settle (page, ms);
}

// In-page helpers: the shadow root, a tree of the open menu, the notification, the options.
function pageHelpers ()
{
   const canvas = () => document .querySelector ("x3d-canvas");
   const root   = () => canvas () .shadowRoot;

   window .ui =
   {
      canvas, root,
      option: name => canvas () .browser .getBrowserOption (name),
      setOption: (name, value) => canvas () .browser .setBrowserOption (name, value),
      node: name => canvas () .browser .currentScene .getNamedNode (name),
      menus: () => Array .from (root () .querySelectorAll ("ul.x_ite-private-menu")),
      layers: () => Array .from (root () .querySelectorAll (".context-menu-layer")),
      tree (ul)
      {
         return Array .from (ul .children) .filter (li => li .tagName === "LI") .map (li =>
         {
            const input = li .querySelector (":scope > label > input");
            const sub   = li .querySelector (":scope > ul");
            const text  = (input ? input .parentElement .querySelector ("span") : li .querySelector (":scope > span")) ?.textContent ?? null;

            return {
               classes: Array .from (li .classList) .filter (c => !c .startsWith ("x_ite-private-fade") && c !== "x_ite-private-hidden"),
               text, type: input ?.type ?? null, name: input ?.name ?? null, checked: input ? input .checked : null,
               sub: sub ? window .ui .tree (sub) : null,
            };
         });
      },
      menu: () => { const ul = window .ui .menus () [0]; return ul ? window .ui .tree (ul) : null; },
      notification ()
      {
         const el = root () .querySelector (".x_ite-private-notification");

         return el ? { text: el .firstElementChild .textContent, visibility: el .style .visibility } : null;
      },
      // The menu item whose text is `text` (a label's input, or the item).
      item (text)
      {
         for (const li of root () .querySelectorAll ("ul.x_ite-private-menu li"))
         {
            const span = li .querySelector (":scope > label > span") ?? li .querySelector (":scope > span");

            if (span ?.textContent === text)
               return li .querySelector (":scope > label > input") ?? li;
         }

         return null;
      },
   };
}

const CENTER = { x: 120, y: 90 };

// A right click on the canvas, as a user does it: Playwright's mouse at a point of the canvas.
async function rightClick (page, at = CENTER)
{
   const box = await page .locator ("x3d-canvas") .boundingBox ();

   await page .mouse .click (box .x + at .x, box .y + at .y, { button: "right" });
   await frames (page);
}

// The entry at a path of texts ("Shading", "Points"): the submenus on the way open by hovering, then the entry is clicked (hover only:
// `hover: true`).
async function clickPath (page, path, hover = false)
{
   path = [ path ] .flat ();

   for (let i = 0; i < path .length; ++ i)
   {
      const handle = await page .evaluateHandle (([ path, i ]) =>
      {
         let ul = window .ui .menus () .at (-1);

         for (let k = 0; k <= i; ++ k)
         {
            const li = Array .from (ul .children) .find (li =>
               (li .querySelector (":scope > label > span") ?? li .querySelector (":scope > span")) ?.textContent === path [k]);

            if (!li)
               return null;

            if (k === i)
               return li .querySelector (":scope > label > input") ?? li;

            ul = li .querySelector (":scope > ul");
         }
      }, [ path, i ]);

      if (!(await handle .evaluate (e => !!e)))
         return false;

      if (i === path .length - 1 && !hover)
         await handle .asElement () .click ({ force: true });
      else
         await handle .asElement () .hover ({ force: true });

      await frames (page);
   }

   return true;
}

const clickItem = (page, text) => clickPath (page, text);

const state = page => page .evaluate (() =>
{
   const browser = window .ui .canvas () .browser;
   const bound   = [ "V1", "V2" ] .filter (name => { try { return browser .currentScene .getNamedNode (name) .isBound; } catch (e) { return false; } });

   return {
      bound: bound .join (","), shading: browser .getBrowserOption ("Shading"), straighten: browser .getBrowserOption ("StraightenHorizon"),
      rubberband: browser .getBrowserOption ("Rubberband"), primitive: browser .getBrowserOption ("PrimitiveQuality"),
      texture: browser .getBrowserOption ("TextureQuality"), timings: browser .getBrowserOption ("Timings"),
      description: browser .description, notification: window .ui .notification (), menus: window .ui .menus () .length,
      layers: window .ui .layers () .length,
   };
});

// --- descriptions ------------------------------------------------------------------------------------------------------

// The descriptions the browser sets itself: a bound viewpoint (X3DViewpointNode.set_bound__), an over TouchSensor
// (X3DPointingDeviceSensorNode.set_over__) and the replaced world (X3DBrowser.replaceWorld sets ""). Every set is read from
// `browser.description` after the frames it needs.
async function descriptions (page)
{
   await loadWorld (page, TOUCH_WORLD);
   await page .evaluate (pageHelpers);
   await page .evaluate (() => { window .ui .setOption ("Notifications", true); window .sets = [ ]; });

   const out = { };
   const read = async label => { await frames (page); out [label] = await state (page); };

   await read ("loaded");

   const box = await page .locator ("x3d-canvas") .boundingBox ();
   const at  = (x, y) => page .mouse .move (box .x + x, box .y + y);

   await page .evaluate (() => { window .ui .canvas () .browser .description = "manual"; });
   await read ("manual");

   await at (200, 150);
   await at (201, 151);
   await wait (page, 100);
   await read ("overSensor");

   await page .evaluate (() => { window .ui .canvas () .browser .description = "after"; });
   await at (2, 2);
   await at (3, 3);
   await wait (page, 100);
   await read ("leftSensor");

   await at (200, 150);
   await at (201, 151);
   await wait (page, 100);
   await read ("overAgain");

   // A world without the sensor: the replaced world sets "" and the new bound viewpoint its description.
   await loadWorld (page, vrml (`DEF V2 Viewpoint { description "Other" }\nShape { geometry Box { } }\n`));
   await page .evaluate (pageHelpers);
   await read ("replaced");

   await loadWorld (page, vrml (`Viewpoint { position 0 0 9 }\nShape { geometry Box { } }\n`));
   await read ("noDescription");

   // The Notifications option off: the description changes, nothing shows.
   await page .evaluate (() => { window .ui .setOption ("Notifications", false); window .ui .canvas () .browser .nextViewpoint (); });
   await read ("notificationsOff");

   return out;
}

// --- splash screen -----------------------------------------------------------------------------------------------------

const splashState = page => page .evaluate (() =>
{
   const canvas = document .querySelector ("x3d-canvas");
   const splash = canvas .shadowRoot .querySelector (".x_ite-private-splash-screen");

   return {
      splash: !!splash,
      hidden: splash ?.classList .contains ("x_ite-private-hidden") ?? null,
      display: splash ? getComputedStyle (splash) .display : null,
      canvasHidden: getComputedStyle (canvas .browser .getCanvas ()) .display === "none",
      title: splash ?.querySelector (".x_ite-private-x_ite") ?.textContent ?? null,
      hasProgressbar: !!splash ?.querySelector (".x_ite-private-progressbar div"),
      childClasses: splash ? Array .from (splash .children, c => c .className) : null,
      option: canvas .browser .getBrowserOption ("SplashScreen"),
   };
});

// SplashScreen: the splash screen shows while a world loads (the canvas hidden) and goes after `initialized`; it stays with the
// message "Failed loading world." when no URL loads; with the option false the canvas stays and the splash screen does not show.
async function splash (page, env)
{
   const out = { };
   const slow = `${env .base}/ui01-slow/world.wrl`;
   const body = "#VRML V2.0 utf8\nShape { geometry Box { } }\n";

   await page .route (slow, async route => { await new Promise (r => setTimeout (r, 700)); await route .fulfill ({ status: 200, contentType: "model/vrml", body }); });
   await page .route (`${env .base}/ui01-slow/missing.wrl`, async route => { await new Promise (r => setTimeout (r, 300)); await route .fulfill ({ status: 404, body: "no" }); });

   await settle (page, 100);
   out .initial = await splashState (page);

   await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .setBrowserOption ("SplashScreen", true));
   out .optionOn = await splashState (page);

   await page .evaluate (pageLoadURL, [ slow ]);
   await wait (page, 250);
   out .loading = await splashState (page);
   await wait (page, 150);
   out .loading2 = await splashState (page);
   out .result = await page .evaluate (() => window .pending);
   await wait (page, 400);
   out .loaded = await splashState (page);

   // The splash screen is hidden after the load: the text it showed ends with the load done.
   out .afterText = await page .evaluate (() => document .querySelector ("x3d-canvas") .shadowRoot .querySelector (".x_ite-private-spinner-text") .textContent);

   // A failing load.
   await page .evaluate (pageLoadURL, [ `${env .base}/ui01-slow/missing.wrl` ]);
   await wait (page, 150);
   out .failing = await splashState (page);
   out .failedResult = await page .evaluate (() => window .pending);
   await wait (page, 200);
   out .failed = await splashState (page);
   out .failedText = await page .evaluate (() => document .querySelector ("x3d-canvas") .shadowRoot .querySelector (".x_ite-private-spinner-text") .textContent);

   // A load that works replaces the failed splash.
   await page .evaluate (pageLoadURL, [ slow ]);
   await page .evaluate (() => window .pending);
   await wait (page, 300);
   out .recovered = await splashState (page);

   // The option false: no splash screen, the canvas stays.
   await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .setBrowserOption ("SplashScreen", false));
   await page .evaluate (pageLoadURL, [ slow ]);
   await wait (page, 250);
   out .offLoading = await splashState (page);
   await page .evaluate (() => window .pending);
   await wait (page, 300);
   out .offLoaded = await splashState (page);

   // The option set while the splash screen shows.
   await page .evaluate (() => document .querySelector ("x3d-canvas") .browser .setBrowserOption ("SplashScreen", true));
   await page .evaluate (pageLoadURL, [ slow ]);
   await wait (page, 250);
   await page .evaluate (() => document .querySelector ("x3d-canvas") .setAttribute ("splashScreen", "false"));
   await wait (page, 100);
   out .attributeOff = await splashState (page);
   await page .evaluate (() => window .pending);
   await wait (page, 400);
   out .attributeOffLoaded = await splashState (page);

   // replaceWorld shows it too.
   await page .evaluate (() => { const b = document .querySelector ("x3d-canvas") .browser; b .setBrowserOption ("SplashScreen", true); window .pending = b .replaceWorld (null) .then (() => "resolved"); });
   await wait (page, 400);
   out .replaced = await splashState (page);

   return out;
}

// --- timings -----------------------------------------------------------------------------------------------------------

const timingsState = (page, withRate = false) => page .evaluate (withRate => 
{
   const canvas = document .querySelector ("x3d-canvas");
   const el     = canvas .shadowRoot .querySelector (".x_ite-private-browser-timings");

   if (!el)
      return null;

   const rows = Array .from (el .querySelectorAll ("tbody tr"), tr => [ tr .children [0] .textContent, tr .children [1] .className, tr .children [1] .title, tr .className ]);
   const num  = /^\d+[.,]\d\d [a-z/]+$/;
   const values = Array .from (el .querySelectorAll ("tbody tr"), tr => tr .children [1] .textContent);

   return {
      hidden: el .classList .contains ("x_ite-private-hidden"), display: getComputedStyle (el) .display,
      head: el .querySelector ("thead th") .textContent, rows, button: el .querySelector ("button") .textContent,
      table: Array .from (el .querySelector ("table") .classList), option: canvas .browser .getBrowserOption ("Timings"),
      numeric: values .map (v => num .test (v) || /^[\d.,]+; [\d.,]+; [\d.,]+$/ .test (v) || /^[\d.,]+ \+ [\d.,]+$/ .test (v) || /^[\d.,]+$/ .test (v) || v === ""),
      rowDisplay: Array .from (el .querySelectorAll ("tbody tr"), tr => getComputedStyle (tr) .display),
      focus: [ document .activeElement ?.tagName, !!canvas .shadowRoot .activeElement ],
      pointerEvents: [ getComputedStyle (el) .pointerEvents, getComputedStyle (el .querySelector ("button")) .pointerEvents ],
      position: [ getComputedStyle (el) .position, getComputedStyle (el) .left, getComputedStyle (el) .bottom ],
      ... (withRate ? { frameRate: parseFloat (values [0] .replace (",", ".")) } : { }),
   };
}, withRate);

// Timings: the panel is hidden until the option is set (the key Ctrl+Plus, the menu or setBrowserOption), shows the frame rate,
// toggles between "More Properties" and "Less Properties" with its button, and hides when the option is false again.
async function timings (page)
{
   await loadWorld (page, PLAIN_WORLD);
   await page .evaluate (pageHelpers);

   const out = { };

   out .initial = await timingsState (page);

   await page .evaluate (() => window .ui .setOption ("Timings", true));
   await frames (page);
   out .on = await timingsState (page);

   await wait (page, 1700);
   const running = await timingsState (page, true);

   // X_ITE draws on demand, so an idle world has no frame rate to compare; the frame rate is recorded in `detail`, not compared.
   out .running = { numeric: running .numeric };
   out .detail = { frameRate: running .frameRate };

   const button = page .locator ("x3d-canvas") .locator (".x_ite-private-browser-timings button");

   await button .click ({ force: true });
   await frames (page);
   out .more = await timingsState (page);

   await wait (page, 1300);
   const more = await timingsState (page);

   out .moreRunning = { numeric: more .numeric };

   await button .click ({ force: true });
   await frames (page);
   out .less = await timingsState (page);

   // The keyboard: Ctrl+Plus toggles the option and the panel.
   await page .evaluate (() => window .ui .canvas () .focus ());
   await page .keyboard .press ("Control+=");
   await frames (page);
   out .keyOff = await timingsState (page);
   await page .keyboard .press ("Control+=");
   await frames (page);
   out .keyOn = await timingsState (page);

   // The pointer goes through the panel to the canvas (pointer-events none), but not through its button.
   await page .evaluate (() => { window .ui .root () .querySelector ("canvas") .addEventListener ("pointerdown", () => { window .hits = (window .hits ?? 0) + 1; }); });
   const box = await page .locator ("x3d-canvas") .boundingBox ();

   await page .mouse .click (box .x + 40, box .y + 60);
   out .pointerThrough = await page .evaluate (() => window .hits ?? 0);

   await page .evaluate (() => window .ui .setOption ("Timings", false));
   await frames (page);
   out .off = await timingsState (page);

   // The element attribute.
   await page .evaluate (() => window .ui .canvas () .setAttribute ("timings", "true"));
   await frames (page);
   out .attributeOn = (await timingsState (page)) .hidden;
   await page .evaluate (() => window .ui .canvas () .setAttribute ("timings", "false"));
   await frames (page);
   out .attributeOff = (await timingsState (page)) .hidden;

   return out;
}

// --- context menu ------------------------------------------------------------------------------------------------------

// The entries and texts of the menu in several states, and what each entry does.
async function menuEntries (page)
{
   await loadWorld (page, MENU_WORLD);
   await page .evaluate (pageHelpers);
   await page .evaluate (() => { window .ui .setOption ("ContextMenu", true); window .ui .setOption ("Notifications", true); });
   await page .evaluate (() =>
   {
      window .contextEvents = [ ];
      window .ui .canvas () .addEventListener ("contextmenu", event => window .contextEvents .push (event .defaultPrevented), { once: false });
      document .addEventListener ("contextmenu", event => window .contextEvents .push (`doc:${event .defaultPrevented}`));
   });

   const out = { };

   await rightClick (page);
   out .contextEvents = await page .evaluate (() => window .contextEvents .splice (0));
   out .world = await state (page);
   out .menu = await page .evaluate (() => window .ui .menu ());

   // A submenu opens when its entry is hovered; the entries are in the DOM before.
   await page .evaluate (() => window .ui .setOption ("Debug", true));
   await page .keyboard .press ("Escape");
   await frames (page);
   out .escape = await state (page);

   await rightClick (page);
   out .withDebug = await page .evaluate (() => window .ui .menu () .map (item => item .text));
   out .closedOnReopen = (await state (page)) .menus;

   return out;
}

async function menuActions (page)
{
   await loadWorld (page, MENU_WORLD);
   await page .evaluate (pageHelpers);
   await page .evaluate (() => { window .ui .setOption ("ContextMenu", true); window .ui .setOption ("Notifications", true); window .ui .setOption ("Debug", true); });

   const out = { };
   const step = async (label, fn) => { await fn (); await wait (page, 500); out [label] = await state (page); };

   await rightClick (page);

   // Radio and checkbox entries: the option and the description change, the menu stays.
   for (const [ label, text ] of [ [ "points", "Points" ], [ "wireframe", "Wireframe" ], [ "flat", "Flat" ], [ "gouraud", "Gouraud" ], [ "phong", "Phong" ] ])
      await step (`shading_${label}`, () => clickPath (page, [ "Shading", text ]));

   await step ("straightenOn",  () => clickItem (page, "Straighten Horizon"));
   await step ("straightenOff", () => clickItem (page, "Straighten Horizon"));

   // The Walk viewer allows the rubberband entry.
   await step ("viewerWalk", () => clickPath (page, [ "Available Viewers", "Walk Viewer" ]));
   await step ("menuAfterWalk", async () => { await page .keyboard .press ("Escape"); await rightClick (page); });
   out .walkMenu = await page .evaluate (() => window .ui .menu () .map (item => item .text));
   await step ("rubberbandOff", () => clickItem (page, "Display Rubberband"));
   await step ("rubberbandOn",  () => clickItem (page, "Display Rubberband"));
   await step ("viewerFly", () => clickPath (page, [ "Available Viewers", "Fly Viewer" ]));
   await step ("viewerExamine", () => clickPath (page, [ "Available Viewers", "Examine Viewer" ]));

   out .submenus = await page .evaluate (() => Array .from (window .ui .root () .querySelectorAll (".context-menu-submenu"), li => li .querySelector (":scope > span") ?.textContent));

   const submenuClick = (menuText, entry) => clickPath (page, [ menuText, entry ]);

   await step ("primitiveLow",     () => submenuClick ("Primitive Quality", "Low"));
   await step ("primitiveHigh",    () => submenuClick ("Primitive Quality", "High"));
   await step ("textureMedium",    () => submenuClick ("Texture Quality", "Medium"));
   await step ("textureLow",       () => submenuClick ("Texture Quality", "Low"));

   // Viewpoints: the entries are the user viewpoints (descriptions); a click binds that viewpoint.
   await step ("viewpointSecond", () => submenuClick ("Viewpoints", "Second"));
   await step ("viewpointFirst",  () => submenuClick ("Viewpoints", "First"));
   out .menuStays = (await state (page)) .menus;

   await step ("timingsOn",  () => clickItem (page, "Browser Timings"));
   await step ("timingsOff", () => clickItem (page, "Browser Timings"));

   // Animations: a checkbox starts and stops its TimeSensor; All and None act on every one.
   const sensors = () => page .evaluate (() => [ "T1", "T2" ] .map (name => window .ui .node (name) .isActive));

   await step ("animSpinOn", () => submenuClick ("Animations", "Spin"));
   out .animSpinOn_active = await sensors ();
   await step ("animAll", () => submenuClick ("Animations", "All"));
   out .animAll_active = await sensors ();
   await rightClick (page);
   await step ("animNone", () => submenuClick ("Animations", "None"));
   out .animNone_active = await sensors ();
   await rightClick (page);
   await step ("animWaveOn", () => submenuClick ("Animations", "Wave"));
   out .animWaveOn_active = await sensors ();
   await step ("animWaveOff", () => submenuClick ("Animations", "Wave"));
   out .animWaveOff_active = await sensors ();

   return out;
}

// Opening, closing and what the click reaches.
async function menuOpenClose (page)
{
   await loadWorld (page, TOUCH_WORLD);
   await page .evaluate (pageHelpers);
   await page .evaluate (() =>
   {
      window .ui .setOption ("ContextMenu", true);
      window .opened = [ ];
      window .window_open = [ ];
      window .open = url => { window .window_open .push (url); return null; };
      const ts = window .ui .node ("TS");

      ts .addFieldCallback ("t", "isActive", value => window .opened .push (`active:${value}`));
      ts .addFieldCallback ("t", "isOver", value => window .opened .push (`over:${value}`));
      window .downs = [ ];

      for (const type of [ "mousedown", "mouseup", "click", "contextmenu", "pointerdown" ])
         window .ui .canvas () .addEventListener (type, event => window .downs .push ([ type, event .defaultPrevented, event .button ]), true);
   });

   const out = { };
   const box = await page .locator ("x3d-canvas") .boundingBox ();

   // Open over a TouchSensor with the right button.
   await rightClick (page, { x: 200, y: 150 });
   out .open = { ... await state (page), events: await page .evaluate (() => window .opened .splice (0)), dom: await page .evaluate (() => window .downs .splice (0)) };

   // A left click on the layer closes it (mousedown on the layer); the sensor gets nothing.
   await page .mouse .click (box .x + 5, box .y + 5);
   await frames (page);
   out .layerClick = { ... await state (page), events: await page .evaluate (() => window .opened .splice (0)), dom: await page .evaluate (() => window .downs .splice (0)) };
   out .layersAfter = await page .evaluate (() => window .ui .layers () .length);
   await wait (page, 1200);
   out .removed = await page .evaluate (() => window .ui .menus () .length);

   // A right click while the menu is open: the layer closes it, the new event opens it again at the new point.
   await rightClick (page, { x: 100, y: 100 });
   await rightClick (page, { x: 150, y: 120 });
   out .reopen = { ... await state (page), position: await page .evaluate (() => { const r = window .ui .menus () .at (-1) .getBoundingClientRect (); return [ Math .round (r .left), Math .round (r .top) ]; }) };
   await wait (page, 1200);
   out .reopenAfter = await state (page);

   // Escape and the arrow keys do nothing.
   for (const key of [ "Escape", "ArrowDown", "Enter" ])
      await page .keyboard .press (key);

   await frames (page);
   out .keys = await state (page);

   // An entry that is not a checkbox or radio closes the menu: About opens the provider's page.
   await clickItem (page, "About X_ITE");
   out .about = { ... await state (page), opened: await page .evaluate (() => window .window_open) };
   await wait (page, 1200);
   out .aboutAfter = await state (page);

   // The edge of the page: the menu stays inside the viewport.
   await page .mouse .click (box .x + box .width - 2, box .y + box .height - 2, { button: "right" });
   await frames (page);
   out .edge = await page .evaluate (() =>
   {
      const r = window .ui .menus () .at (-1) .getBoundingClientRect ();

      return { right: r .right <= window .innerWidth + 1, bottom: r .bottom <= window .innerHeight + 1, left: r .left >= 0, top: r .top >= 0 };
   });
   await page .keyboard .press ("Escape");

   // The user menu.
   await page .evaluate (() =>
   {
      window .userClicks = [ ];
      window .ui .canvas () .browser .getContextMenu () .userMenu = browser => ({ "one": { name: "Custom Entry", callback: () => window .userClicks .push ("one") },
         "two": { name: "Custom Check", type: "checkbox", selected: true, callback: event => window .userClicks .push (`two:${event .target .checked}`) } });
   });
   await page .mouse .click (box .x + 20, box .y + 20, { button: "right" });
   await frames (page);
   out .userMenu = await page .evaluate (() => window .ui .menu () .map (item => item .text));
   await clickItem (page, "Custom Check");
   out .userCheck = { ... await state (page), clicks: await page .evaluate (() => window .userClicks .slice ()) };
   await clickItem (page, "Custom Entry");
   out .userEntry = { ... await state (page), clicks: await page .evaluate (() => window .userClicks .slice ()) };
   await wait (page, 1200);

   // ContextMenu false: nothing opens, the event is still claimed.
   await page .evaluate (() => { window .ui .setOption ("ContextMenu", false); window .contextSeen = [ ]; window .ui .canvas () .addEventListener ("contextmenu", e => window .contextSeen .push (e .defaultPrevented)); });
   await page .mouse .click (box .x + 20, box .y + 20, { button: "right" });
   await frames (page);
   out .optionOff = { ... await state (page), seen: await page .evaluate (() => window .contextSeen) };

   return out;
}

// The world info entry.
async function menuWorldInfo (page)
{
   await loadWorld (page, MENU_WORLD);
   await page .evaluate (pageHelpers);
   await page .evaluate (() => window .ui .setOption ("ContextMenu", true));

   const out = { };
   const info = () => page .evaluate (() =>
   {
      const overlays = window .ui .root () .querySelectorAll (".x_ite-private-world-info-overlay");
      const div      = window .ui .root () .querySelector (".x_ite-private-world-info");

      return {
         overlays: overlays .length, hidden: div ?.classList .contains ("x_ite-private-hidden") ?? null,
         buttons: div ? Array .from (div .querySelectorAll ("button"), b => [ b .textContent, b .className, b .style .display ]) : null,
         top: div ?.querySelector (".x_ite-private-world-info-top") ?.textContent ?? null,
         title: div ?.querySelector (".x_ite-private-world-info-title") ?.textContent ?? null,
         infos: div ? Array .from (div .querySelectorAll (".x_ite-private-world-info-info"), d => d .innerHTML) : null,
         table: div ? Array .from (div .querySelectorAll ("tr"), tr => tr .innerHTML .replace (/<\/?(td|a)[^>]*>/g, "|")) : null,
      };
   });

   await rightClick (page);
   out .menuHas = await page .evaluate (() => !!window .ui .item ("Show World Info"));
   await clickItem (page, "Show World Info");
   await wait (page, 100);
   out .shown = await info ();
   out .menuClosed = (await state (page)) .layers;

   // The Metadata button.
   await page .locator ("x3d-canvas") .locator (".x_ite-private-world-info button") .nth (1) .click ({ force: true });
   await frames (page);
   out .metadata = await info ();

   await page .locator ("x3d-canvas") .locator (".x_ite-private-world-info button") .nth (0) .click ({ force: true });
   await frames (page);
   out .worldInfoAgain = await info ();

   // A click on the overlay closes it.
   const box = await page .locator ("x3d-canvas") .boundingBox ();

   await page .mouse .click (box .x + 5, box .y + 5);
   await frames (page);
   out .closing = await info ();
   await wait (page, 500);
   out .closed = await info ();

   // Opened twice: the first overlay stays.
   await rightClick (page);
   await clickItem (page, "Show World Info");
   await rightClick (page);
   await clickItem (page, "Show World Info");
   await wait (page, 100);
   out .twice = await info ();
   await page .mouse .click (box .x + 5, box .y + 5);
   await wait (page, 500);
   out .twiceClosed = await info ();

   // A world without a WorldInfo and META statements has no entry.
   await loadWorld (page, PLAIN_WORLD);
   await page .evaluate (pageHelpers);
   await page .evaluate (() => window .ui .setOption ("ContextMenu", true));
   await rightClick (page);
   out .plainHas = await page .evaluate (() => !!window .ui .item ("Show World Info"));
   out .plainMenu = await page .evaluate (() => window .ui .menu () .map (item => item .text));

   return out;
}

// Shift+F8: the active viewpoint as text on the clipboard.
async function shiftF8 (page)
{
   await page .context () .grantPermissions ([ "clipboard-read", "clipboard-write" ]);
   await loadWorld (page, vrml (`DEF V1 Viewpoint { description "A" position 1 2 10 orientation 0 1 0 0.5 centerOfRotation 0 1 0 fieldOfView 0.6 }\nDEF V2 Viewpoint { description "B" position 0 0 12 }\nShape { geometry Box { } }\n`));
   await page .evaluate (pageHelpers);
   await page .evaluate (() => { window .ui .setOption ("Notifications", true); window .ui .canvas () .focus (); window .infos = [ ]; const log = console .info; console .info = (... a) => { window .infos .push (a .join (" ")); }; });

   const out = { };
   const read = async label =>
   {
      await frames (page);
      await wait (page, 150);
      out [label] = {
         clipboard: await page .evaluate (() => navigator .clipboard .readText ()) .catch (e => `error ${e .message}`),
         description: await page .evaluate (() => window .ui .canvas () .browser .description), infos: await page .evaluate (() => window .infos .splice (0)),
      };
   };

   await page .evaluate (() => navigator .clipboard .writeText ("before"));
   await page .keyboard .press ("F8");
   await read ("plainF8");

   await page .keyboard .press ("Shift+F8");
   await read ("shiftF8");

   await page .evaluate (() => window .ui .canvas () .browser .nextViewpoint ());
   await wait (page, 1500);
   await page .keyboard .press ("Shift+F8");
   await read ("second");

   const claimed = await page .evaluate (() => { window .seen = [ ]; window .addEventListener ("keydown", e => window .seen .push ([ e .keyCode, e .shiftKey, e .defaultPrevented ])); return true; });

   await page .keyboard .press ("F8");
   await page .keyboard .press ("Shift+F8");
   await frames (page);
   out .claimed = claimed && await page .evaluate (() => window .seen);

   return out;
}

// Cleanup: dispose (), the element removed and a world replaced leave nothing behind.
async function cleanup (page)
{
   await loadWorld (page, MENU_WORLD);
   await page .evaluate (pageHelpers);
   await page .evaluate (() => { window .ui .setOption ("ContextMenu", true); window .ui .setOption ("Timings", true); window .ui .setOption ("SplashScreen", true); window .ui .setOption ("Notifications", true); });

   const out = { };
   const counts = () => page .evaluate (() =>
   {
      const shadow = window .ui .canvas () ?.shadowRoot;

      return { menus: shadow ?.querySelectorAll ("ul.x_ite-private-menu, .context-menu-layer") .length ?? null, overlays: shadow ?.querySelectorAll (".x_ite-private-world-info-overlay") .length ?? null };
   });

   await rightClick (page);
   await clickItem (page, "Show World Info");
   out .open = await counts ();

   // A world replaced while the world info and the menu are open.
   await rightClick (page);
   await page .evaluate (() => { window .ui .canvas () .browser .description = "x"; });
   await loadWorld (page, PLAIN_WORLD);
   out .replaced = await counts ();
   await wait (page, 1200);
   out .replacedAfter = await counts ();

   // The element removed while the menu is open.
   await page .evaluate (pageHelpers);
   await page .evaluate (() => { window .ui .setOption ("ContextMenu", true); });
   await rightClick (page);
   out .beforeRemoval = await counts ();

   // The element removed while the menu is open, then put back; then dispose ().
   out .removal = await page .evaluate (async () =>
   {
      const canvas = window .ui .canvas ();
      const shadow = canvas .shadowRoot;
      const parent = canvas .parentNode;
      const left   = () => shadow .querySelectorAll ("ul.x_ite-private-menu, .context-menu-layer, .x_ite-private-world-info-overlay") .length;
      const result = { before: left () };

      canvas .remove ();
      await new Promise (r => setTimeout (r, 1200));
      result .removed = left ();

      parent .prepend (canvas);
      await new Promise (r => setTimeout (r, 300));
      result .back = { children: Array .from (shadow .children, c => c .tagName + "." + c .className) .filter (c => !c .startsWith ("STYLE")) };

      return result;
   });

   out .disposed = await page .evaluate (async () =>
   {
      const canvas  = window .ui .canvas ();
      const shadow  = canvas .shadowRoot;

      canvas .browser .description = "last";
      canvas .browser .dispose ();
      await new Promise (r => setTimeout (r, 1500));

      return {
         canvasInside: shadow .contains (canvas .browser .getCanvas ()), menus: shadow .querySelectorAll ("ul.x_ite-private-menu, .context-menu-layer") .length,
         timings: shadow .querySelectorAll (".x_ite-private-browser-timings") .length, splash: shadow .querySelectorAll (".x_ite-private-splash-screen") .length,
      };
   });

   return out;
}

// The panel's button on a touch screen: a tap gives pointer events, touch events and a click; BrowserTimings listens to both
// `click` and `touchend`. The panel and the menu do not take the touch from the canvas elsewhere.
async function timingsTouch (page)
{
   await loadWorld (page, PLAIN_WORLD);
   await page .evaluate (pageHelpers);
   await page .evaluate (() => { window .ui .setOption ("Timings", true); window .ui .setOption ("ContextMenu", true); });
   await frames (page);

   const out = { };
   const type = () => page .evaluate (() => window .ui .root () .querySelector (".x_ite-private-browser-timings button") .textContent);
   const box  = await page .locator ("x3d-canvas") .boundingBox ();
   const rect = await page .evaluate (() => { const r = window .ui .root () .querySelector (".x_ite-private-browser-timings button") .getBoundingClientRect (); return [ r .x + r .width / 2, r .y + r .height / 2 ]; });

   out .before = await type ();
   await page .touchscreen .tap (rect [0], rect [1]);
   await frames (page);
   out .afterTap = await type ();
   await page .touchscreen .tap (rect [0], rect [1]);
   await frames (page);
   out .afterSecondTap = await type ();

   // A tap elsewhere on the canvas reaches the world (a TouchSensor under it).
   await page .evaluate (() => { window .touches = [ ]; const c = window .ui .root () .querySelector ("canvas"); for (const t of [ "touchstart", "touchend", "pointerdown", "click" ]) c .addEventListener (t, e => window .touches .push ([ t, e .defaultPrevented ])); });
   await page .touchscreen .tap (box .x + 200, box .y + 100);
   await frames (page);
   out .canvasTap = await page .evaluate (() => window .touches .splice (0));
   out .menus = await page .evaluate (() => window .ui .menus () .length);

   return out;
}

const cases = { descriptions, splash, timings, timingsTouch, menuEntries, menuActions, menuOpenClose, menuWorldInfo, shiftF8, cleanup };

export const CASE_GROUPS = {
   browserUi: { context: { viewport: { width: 1000, height: 800 }, deviceScaleFactor: 1, hasTouch: true }, cases },
};
