// X_ITE_R Browser Preview page. Every control calls the X_ITE-named API of x_ite_r.mjs; the Rust engine does the X3D work.

import X3D from "../x_ite_r.mjs";

// The approved test worlds of the preview. All are newly authored clean-room files of this directory.
const WORLDS = [
   { url: "worlds/vrml97-basic.wrl", title: "VRML97 basic (sensors, animation)" },
   { url: "worlds/x3d-basic.x3d",    title: "X3D basic (XML encoding)" },
   { url: "worlds/navigation.wrl",   title: "Navigation (EXAMINE, WALK, FLY)" },
   { url: "worlds/resources.wrl",    title: "Resources (Inline, EXTERNPROTO, textures)" },
   { url: "worlds/missing-on-purpose.wrl", title: "Missing world (failed-URL test)" },
   { url: "../example/interactive.wrl",    title: "Interactive example (Anchor, Inline, texture)" },
];

const VIEWERS = [ [ "NavExamine", "EXAMINE" ], [ "NavWalk", "WALK" ], [ "NavFly", "FLY" ] ];

const $ = id => document .getElementById (id);

const canvas  = $("canvas");
const browser = canvas .browser;

// Events of the page, newest last. The smoke test reads the same list (globalThis .X_ITE_R_PREVIEW .events).
const events = [ ];

function log (text, kind = "")
{
   const line = `${new Date () .toISOString () .slice (11, 23)} ${text}`;

   events .push (line);

   const element = document .createElement ("div");

   element .textContent = line;
   element .className   = kind;

   $("log") .append (element);
   $("log") .scrollTop = $("log") .scrollHeight;
}

function setStatus (text, kind = "")
{
   $("status") .textContent = text;
   $("status") .className   = kind;
}

// Build facts: build-info.json is written by tools/package-web-preview.sh. A development tree has none.
async function showBuild ()
{
   $("version") .textContent = `${browser .name} ${browser .version}`;

   try
   {
      const response = await fetch ("../build-info.json", { cache: "no-store" });

      if (!response .ok)
         throw new Error (`HTTP ${response .status}`);

      const info = await response .json ();

      $("build") .textContent = `${info .commit} (${info .release}, rustc ${info .rustc}, wasm-bindgen ${info .wasm_bindgen})`;
   }
   catch
   {
      $("build") .textContent = "no build-info.json (development tree)";
   }
}

// `?backend=` is applied by backend.mjs, before the engine module runs.

async function showWebGpu ()
{
   try
   {
      await browser .ready ();

      const engine  = browser [Symbol .for ("X_ITE_R.engine")];
      const backend = engine ?.backend === "webgl2" ? "WebGL2" : "WebGPU";
      let   text    = `${backend} started: ${engine ?.adapterInfo ?? "adapter"}`;

      // The page's own WebGPU adapter request, for the vendor and architecture (the engine has its own device).
      if (backend === "WebGPU")
      {
         const info = (await navigator .gpu ?.requestAdapter ()) ?.info;

         if (info)
            text += ` — ${[ info .vendor, info .architecture, info .device, info .description ] .filter (Boolean) .join (", ")}`;
      }

      if (engine ?.fallbackReason)
         text += ` (WebGPU did not start: ${engine .fallbackReason})`;

      $("webgpu") .textContent = text;
      $("webgpu") .className   = "ok";
      $("webgpu") .dataset .backend = engine ?.backend ?? "";
      log (`Graphics: ${text}`, "ok");
      return true;
   }
   catch (error)
   {
      $("webgpu") .textContent = `failed: ${error}`;
      $("webgpu") .className   = "error";
      log (`Graphics failed: ${error}`, "error");
      return false;
   }
}

// Nodes of the current world that the controls use. A node belongs to one world, so they are found again on each load.
let nodes = { };

function named (scene, name)
{
   try { return scene .getNamedNode (name); }
   catch { return undefined; }
}

function findNodes ()
{
   const scene = browser .currentScene;

   nodes = { };

   for (const name of [ "ColorTarget", "Touch", "Drag", "Clock", "Viewer", "ProtoBox", ... VIEWERS .map (v => v [0]) ])
   {
      const node = named (scene, name);

      if (node)
         nodes [name] = node;
   }

   // Viewer type: only the NavigationInfo nodes that the world has.
   const select = $("viewer");

   select .replaceChildren (... VIEWERS .filter (([ name ]) => nodes [name]) .map (([ name, type ]) => new Option (type, name)));
   select .disabled = select .options .length === 0;
   $("viewer-hint") .textContent = select .disabled ? "This world has no NavigationInfo nodes for the control. Load \"Navigation\"." : "Binds the NavigationInfo node of that type (set_bind).";

   for (const button of document .querySelectorAll ("#color-set button"))
      button .disabled = !nodes .ColorTarget;
}

let generation = 0;
// The URL of the last world that loaded (the restart link starts it again).
let lastWorld = null;

async function loadWorld (url)
{
   const current = ++ generation;

   setStatus (`loading ${url} …`);
   log (`loadURL ${url}`);

   try
   {
      await browser .loadURL (new X3D .MFString (url));

      if (current !== generation)
         return false;

      findNodes ();
      lastWorld = url;
      setStatus (`loaded ${url}`, "ok");
      log (`loaded ${browser .currentScene .worldURL}`, "ok");
      return true;
   }
   catch (error)
   {
      if (current === generation)
      {
         setStatus (`error: ${url} did not load (${error}). The previous world stays.`, "error");
         log (`error ${url}: ${error}`, "error");
      }

      return false;
   }
   finally
   {
      $("world-url") .textContent = browser .currentScene ?.worldURL || "(none)";
   }
}

function format (value)
{
   if (value === undefined || value === null)
      return "—";

   return typeof value === "object" ? value .toString () : String (value);
}

function showValues ()
{
   let text = "";

   try
   {
      const rows = [ ];

      rows .push ([ "activeViewpoint", browser .activeViewpoint ?.getNodeName () ?? "(default)" ]);

      if (nodes .Touch)
         rows .push ([ "Touch.isActive", nodes .Touch .isActive ], [ "Touch.touchTime", nodes .Touch .touchTime ]);

      if (nodes .Drag)
         rows .push ([ "Drag.translation_changed", nodes .Drag .translation_changed ]);

      if (nodes .Clock)
         rows .push ([ "Clock.fraction_changed", nodes .Clock .fraction_changed ]);

      if (nodes .Viewer)
         rows .push ([ "Viewer.position_changed", nodes .Viewer .position_changed ], [ "Viewer.orientation_changed", nodes .Viewer .orientation_changed ]);

      if (nodes .ColorTarget)
      {
         const type = nodes .ColorTarget .getNodeTypeName ();

         rows .push ([ `ColorTarget (${type})`, type === "PhysicalMaterial" ? nodes .ColorTarget .baseColor : nodes .ColorTarget .diffuseColor ]);
      }

      text = rows .map (([ name, value ]) => `${name} = ${format (value)}`) .join ("\n");
   }
   catch
   {
      // The world was replaced or the browser disposed; the next load finds the nodes again.
      nodes = { };
      text  = "(no world)";
   }

   $("values") .textContent = text;
   requestAnimationFrame (showValues);
}

function setColor (text)
{
   const target = nodes .ColorTarget;

   if (!target)
      return;

   const [ r, g, b ] = text .split (" ") .map (Number);
   const color       = new X3D .SFColor (r, g, b);

   if (target .getNodeTypeName () === "PhysicalMaterial")
      target .baseColor = color;
   else
      target .diffuseColor = color;

   log (`ColorTarget color = ${text}`);
}

function bindViewer (name)
{
   const node = nodes [name];

   if (!node)
      return;

   node .set_bind = true;
   log (`${name}.set_bind = TRUE`);
}

// Controls
const select = $("world");

for (const world of WORLDS)
   select .add (new Option (world .title, world .url));

$("load")         .onclick = () => loadWorld (select .value);
$("reload-world") .onclick = () => loadWorld (select .value);
select .onchange  = () => loadWorld (select .value);
$("previous")     .onclick = () => browser .previousViewpoint ();
$("next")         .onclick = () => browser .nextViewpoint ();
$("view-all")     .onclick = () => browser .viewAll ();
$("viewer")       .onchange = event => bindViewer (event .target .value);

for (const button of document .querySelectorAll ("#color-set button"))
   button .onclick = () => setColor (button .dataset .color);

// No control loads a world once the engine is gone: after Dispose or a lost device.
function disableControls ()
{
   for (const element of document .querySelectorAll ("button, select"))
      element .disabled = true;
}

$("dispose") .onclick = () =>
{
   // A pending loadWorld () skips its status writes.
   ++ generation;
   browser .dispose ();
   nodes = { };
   setStatus ("browser disposed; reload the page to start again");
   log ("browser.dispose ()");
   disableControls ();
};

canvas .addEventListener ("load",  () => log ("event load"));
canvas .addEventListener ("error", () => log ("event error", "error"));

// Start: ?world=<one of WORLDS> selects the first world.
const requested = new URLSearchParams (location .search) .get ("world");

if (WORLDS .some (world => world .url === requested))
   select .value = requested;

globalThis .X_ITE_R_PREVIEW = { browser, events, loadWorld, setColor, bindViewer, worlds: WORLDS, get nodes () { return nodes; } };

showBuild ();
showValues ();

// A lost graphics device stops the engine (X_ITE's CONNECTION_ERROR). The page offers one restart, and on WebGPU one with
// WebGL2; it never restarts by itself, so a device that is lost again cannot start a loop.
browser .addBrowserCallback ("preview-connection", X3D .X3DConstants .CONNECTION_ERROR, () =>
{
   const engineBackend = $("webgpu") .dataset .backend;
   const query         = engineBackend === "webgpu" ? "?backend=webgl2" : "";
   const world         = lastWorld ?? new URLSearchParams (location .search) .get ("world");
   const href          = `${location .pathname}${query}${world ? `${query ? "&" : "?"}world=${encodeURIComponent (world)}` : ""}`;

   // A pending loadWorld () skips its status writes, and the controls are off: the status keeps the restart link.
   ++ generation;
   disableControls ();
   setStatus (`stopped: the ${engineBackend === "webgl2" ? "WebGL2" : "WebGPU"} device is lost.`, "error");
   log ("Graphics device lost", "error");

   const restart = document .createElement ("a");

   restart .id          = "restart";
   restart .href        = href;
   restart .textContent = engineBackend === "webgpu" ? " Start again with WebGL2" : " Start again";
   $("status") .append (restart);
});

if (await showWebGpu ())
   await loadWorld (select .value);
else
   setStatus ("X_ITE_R could not start: neither WebGPU nor WebGL2 is available", "error");
