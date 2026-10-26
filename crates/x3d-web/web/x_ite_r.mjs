// X_ITE_R page adapter: defines <x3d-canvas> and the X_ITE-named JavaScript API on top of the Rust engine
// (crates/x3d-web, built into ./pkg by tools/build-web.sh). Everything X3D (parsing, the scene graph, events,
// rendering) runs in Rust; this file only fetches the world, runs the requestAnimationFrame loop and converts
// values. The names follow X_ITE 16.5.0 (oracle/x_ite-pin); web/API.md lists what exists and what does not.

import init, { WebBrowser } from "./pkg/x3d_web.js";

const wasm = init ();

// X3DConstants: the access types and browser events with X_ITE's values.
const X3DConstants = Object .freeze ({
   initializeOnly: 0b001,
   inputOnly:      0b010,
   outputOnly:     0b100,
   inputOutput:    0b111,

   CONNECTION_ERROR:  0,
   BROWSER_EVENT:     1,
   INITIALIZED_EVENT: 2,
   SHUTDOWN_EVENT:    3,
   INITIALIZED_ERROR: 4,
});

const ACCESS = { initializeOnly: 0b001, inputOnly: 0b010, outputOnly: 0b100, inputOutput: 0b111 };

// --- field values ------------------------------------------------------------------------------------------------

// Fixed-size numeric fields: the component names and the VRML text the engine reads.
function numericField (typeName, names)
{
   return class
   {
      constructor (... values)
      {
         names .forEach ((name, i) => this [name] = Number (values [i] ?? 0));
      }

      static fromString (text)
      {
         return new this (... text .trim () .split (/\s+/) .map (Number));
      }

      getTypeName () { return typeName; }
      toString () { return names .map (name => String (this [name])) .join (" "); }
   };
}

const SFColor     = numericField ("SFColor",     [ "r", "g", "b" ]);
const SFColorRGBA = numericField ("SFColorRGBA", [ "r", "g", "b", "a" ]);
const SFVec2f     = numericField ("SFVec2f",     [ "x", "y" ]);
const SFVec3f     = numericField ("SFVec3f",     [ "x", "y", "z" ]);
const SFVec4f     = numericField ("SFVec4f",     [ "x", "y", "z", "w" ]);
const SFVec2d     = numericField ("SFVec2d",     [ "x", "y" ]);
const SFVec3d     = numericField ("SFVec3d",     [ "x", "y", "z" ]);
const SFVec4d     = numericField ("SFVec4d",     [ "x", "y", "z", "w" ]);
const SFRotation  = numericField ("SFRotation",  [ "x", "y", "z", "angle" ]);

const NUMERIC = { SFColor, SFColorRGBA, SFVec2f, SFVec3f, SFVec4f, SFVec2d, SFVec3d, SFVec4d, SFRotation };

// The VRML encoding of a string: quotes, with `"` and `\` escaped (X3DGenerator.EscapeString). Other characters,
// newlines included, stand as they are.
const vrmlString = string => `"${String (string) .replace (/["\\]/g, c => `\\${c}`)}"`;

// The string of a VRML-encoded SFString.
const parseVrmlString = text => text .trim () .slice (1, -1) .replace (/\\(.)/gs, "$1");

class MFString extends Array
{
   constructor (... values)
   {
      super ();
      this .push (... values .map (String));
   }

   getTypeName () { return "MFString"; }
   toString () { return `[${Array .from (this, vrmlString) .join (", ")}]`; }
}

// The VRML text of a value assigned from JavaScript, for a field of `typeName`.
function vrmlOf (typeName, value)
{
   switch (typeName)
   {
      case "SFBool":
         return value ? "TRUE" : "FALSE";
      case "SFString":
         return vrmlString (value);
      case "SFFloat":
      case "SFDouble":
      case "SFTime":
      case "SFInt32":
         return String (Number (value));
      case "MFString":
         return `[${Array .from (value, vrmlString) .join (", ")}]`;
   }

   if (typeName in NUMERIC && value instanceof NUMERIC [typeName])
      return value .toString ();

   throw new TypeError (`X_ITE_R: cannot assign ${value} to a field of type ${typeName}.`);
}

// --- nodes -------------------------------------------------------------------------------------------------------

// The wasm object of a browser (read-only, for tests and diagnostics).
const _engine = Symbol .for ("X_ITE_R.engine");

// The scene and world of a node or scene handle: its scene id in the engine and the world generation of that scene
// (an Anchor can replace the world inside a scene). Every use checks both first, so an old handle never reaches a
// node of a newer world.
function engineOf (handle)
{
   const engine = handle .browser [_engine];

   if (!engine || !engine .exists (handle .scene) || engine .worldGeneration (handle .scene) !== handle .generation)
      throw new Error ("X_ITE_R: the node belongs to a world that was replaced.");

   return engine;
}

// X3DField.valueOf of a field read through a node: numbers, booleans and strings as JavaScript values, the vector
// types as value objects, SFNode and MFNode as nodes; anything else as an X3DField (its `toString ()` text).
function fieldValue (node, name)
{
   const engine = engineOf (node);
   const [ typeName, , text ] = engine .field (node .scene, node .id, name);

   switch (typeName)
   {
      case "SFBool":
         return text === "TRUE";
      case "SFFloat":
      case "SFDouble":
      case "SFTime":
      case "SFInt32":
         return Number (text);
      case "SFString":
         return parseVrmlString (text);
   }

   if (typeName in NUMERIC)
      return NUMERIC [typeName] .fromString (text);

   // SFNode: the node or null; MFNode: an Array of them (X_ITE: an MFNode).
   if (typeName === "SFNode" || typeName === "MFNode")
   {
      const nodes = Array .from (engine .nodeValue (node .scene, node .id, name), n => n < 0 ? null : SFNode (node .browser, node, Number (n)));

      return typeName === "SFNode" ? nodes [0] : nodes;
   }

   return new X3DField (node, name);
}

// X3DNode.getField (name): one field of one node.
class X3DField
{
   #node; #name;

   constructor (node, name)
   {
      this .#node = node;
      this .#name = name;
   }

   #info () { return engineOf (this .#node) .field (this .#node .scene, this .#node .id, this .#name); }

   getName ()       { return this .#name; }
   getTypeName ()   { return this .#info () [0]; }
   getAccessType () { return ACCESS [this .#info () [1]]; }
   getValue ()      { return fieldValue (this .#node, this .#name); }
   setValue (value) { engineOf (this .#node) .setField (this .#node .scene, this .#node .id, this .#name, vrmlOf (this .getTypeName (), value)); }
   toString ()      { return this .#info () [2]; }
}

// The X3DNode behind an SFNode proxy (for the browser functions that take a node).
const _node = Symbol ("X_ITE_R.node");

// SFNode: a node of one world. `owner` is a handle of the same world (a scene or a node). Field names read and
// assign like X_ITE's SFNode proxy.
function SFNode (browser, owner, id)
{
   const node   = { browser, id, scene: owner .scene, generation: owner .generation };
   const target = {
      getNodeTypeName: () => engineOf (node) .typeName (node .scene, id),
      getNodeName:     () => engineOf (node) .nodeName (node .scene, id) ?? "",
      getField:        name => { engineOf (node) .field (node .scene, id, name); return new X3DField (node, name); },
      getTypeName:     () => "SFNode",
      toString:        () => `${engineOf (node) .typeName (node .scene, id)} { }`,
      [_node]:         node,
   };

   // The field's [typeName, accessType, value], or null when the node has no such field.
   const field = key =>
   {
      const engine = engineOf (node);

      try { return engine .field (node .scene, id, key); } catch { return null; }
   };

   return new Proxy (target, {
      get (target, key)
      {
         if (typeof key === "string" && !(key in target))
         {
            const info = field (key);

            // X_ITE allows reading initializeOnly fields too; an inputOnly field reads as undefined.
            if (info)
               return info [1] === "inputOnly" ? undefined : fieldValue (node, key);
         }

         return Reflect .get (target, key);
      },
      set (target, key, value)
      {
         if (typeof key === "string")
         {
            const info = field (key);

            if (info)
            {
               if (info [1] === "outputOnly")
                  return false;

               engineOf (node) .setField (node .scene, id, key, vrmlOf (info [0], value));
               return true;
            }
         }

         return Reflect .set (target, key, value);
      },
      has (target, key)
      {
         return Boolean (typeof key === "string" && field (key)) || Reflect .has (target, key);
      },
   });
}

// --- scene and browser -------------------------------------------------------------------------------------------

// The scene ids of the X3DScene objects the page dropped: a scene that is not the world is released with them.
const sceneRegistry = new FinalizationRegistry (({ browser, scene }) => browser [_engine] ?.discard (scene));

class X3DScene
{
   #browser;
   #handle;

   // `scene` is the engine's scene id, or null for the empty world the browser starts with.
   constructor (browser, scene, generation)
   {
      this .#browser = browser;
      this .#handle  = { browser, scene, generation };

      if (scene !== null)
         sceneRegistry .register (this, { browser, scene });
   }

   get [_node] () { return this .#handle; }

   #engine ()
   {
      return this .#handle .scene === null ? null : engineOf (this .#handle);
   }

   get worldURL () { return this .#engine () ?.worldURL (this .#handle .scene) ?? this .#browser .baseURL; }
   getWorldURL () { return this .worldURL; }

   get rootNodes ()
   {
      // A BigInt64Array of node ids, -1 for NULL.
      return Array .from (this .#engine () ?.rootNodes (this .#handle .scene) ?? [ ], n => n < 0 ? null : SFNode (this .#browser, this .#handle, Number (n)));
   }

   getNamedNode (name)
   {
      name = String (name);

      const id = this .#engine () ?.namedNode (this .#handle .scene, name);

      if (id === undefined)
         throw new Error (`Named node '${name}' not found.`);

      return SFNode (this .#browser, this .#handle, id);
   }
}

// A rejection of the engine as the Error X_ITE gives: "TypeError: text" is a TypeError with the message "text".
function engineError (error)
{
   const match = String (error ?.message ?? error) .match (/^(Error|TypeError|RangeError|SyntaxError|URIError): ([^]*)$/);

   if (!match)
      return error instanceof Error ? error : new Error (String (error));

   return new globalThis [match [1]] (match [2]);
}

// The touch-action of the canvas: a navigating viewer takes every gesture on it (no page scroll or zoom there);
// with NONE the page keeps its own touch behavior.
const TOUCH_ACTION = { NONE: "auto" };

class X3DBrowser
{
   #element;
   #canvas;
   #engine = null;
   #ready;
   #scene;
   #scenes = new Map ();
   #callbacks = new Map ([ 0, 1, 2, 3, 4 ] .map (event => [ event, new Map () ]));
   #frame = 0;
   #frames = 0;
   #fileLoader = null;
   #reject = null;
   #disposed = false;
   #listeners = new AbortController ();
   #buttons = 0;
   #pointer = [ 0, 0 ];
   #touches = new Map ();
   #touchMoved = false;
   #lastTouch = -Infinity;

   constructor (element)
   {
      this .#element = element;
      this .#canvas  = document .createElement ("canvas");
      this .#scene   = new X3DScene (this, null, 0);

      // X3DCoreContext: the browser's own element is focusable (tabindex 0) and takes the keyboard.
      this .#canvas .tabIndex = 0;
      this .#canvas .style .cssText = "display:block;width:100%;height:100%;outline:none;touch-action:none";
      element .shadowRoot .append (this .#canvas);

      this .#ready = wasm .then (() => WebBrowser .create (this .#canvas)) .then (engine =>
      {
         if (this .#disposed)
         {
            engine .free ();
            throw new Error ("Browser disposed.");
         }

         this .#engine = engine;
         this .#listen ();
         this .#start ();

         // X3DBrowser.initialize: replaceWorld with an empty scene, which ends in "initialized load". A loadURL
         // then always replaces a world, so it starts with "shutdown".
         this .#call (X3DConstants .INITIALIZED_EVENT);
         this .#dispatch ("initialized load");
         return this;
      });

      // Like X3DCanvasElement when X_ITE cannot start: the element shows its own content (a <slot>) instead, with a
      // message when it has none.
      this .#ready .catch (error =>
      {
         console .error (error);

         if (this .#disposed)
            return;

         this .#fallback (`X_ITE_R cannot start: ${error ?.message ?? error}`);
      });
   }

   #fallback (message)
   {
      const slot = document .createElement ("slot");

      slot .textContent = message;
      this .#canvas .remove ();
      this .#element .shadowRoot .append (slot);
   }

   get [_engine] () { return this .#engine; }

   // X3DBrowser.name, version, getName, getVersion: this engine's own name and version.
   get name ()    { return "X_ITE_R"; }
   get version () { return "0.1.0"; }
   getName ()     { return this .name; }
   getVersion ()  { return this .version; }

   get baseURL () { return document .baseURI; }

   get currentScene () { return this .#scene; }

   getWorldURL () { return this .#scene .worldURL; }

   getCanvas () { return this .#canvas; }

   // X_ITE_R: resolves when the Rust engine and its WebGPU device are ready (X_ITE has no equivalent; it is
   // ready synchronously). Rejects when the page has no WebGPU, or the adapter cannot run the renderer.
   ready () { return this .#ready; }

   // The X3DScene object of engine scene `id` in its current world.
   #sceneOf (id)
   {
      const generation = this .#engine .worldGeneration (id);
      const key        = `${id}:${generation}`;
      let scene        = this .#scenes .get (key) ?.deref ();

      if (!scene)
         this .#scenes .set (key, new WeakRef (scene = new X3DScene (this, id, generation)));

      return scene;
   }

   // The engine and scene of a handle the page passed (an X3DScene of this browser), or null.
   #sceneId (scene)
   {
      const handle = scene instanceof X3DScene ? scene [_node] : null;

      if (!handle || handle .browser !== this || handle .scene === null)
         return null;

      engineOf (handle);
      return handle .scene;
   }

   // Wait until engine scene `id` has loaded what it holds the load for (FileLoader: its loadCount is 0).
   // `aborted ()` ends the wait; the scene is then released.
   async #loaded (id, aborted)
   {
      while (true)
      {
         if (!this .#engine || aborted ())
         {
            this .#engine ?.discard (id);
            throw new Error ("aborted");
         }

         if (this .#engine .loaded (id, Date .now () / 1000))
            return;

         this .#flushLog (id);
         await this .#engine .progress (id);
      }
   }

   // X3DBrowser.loadURL (url [, parameter]): replace the world with the first url that loads.
   loadURL (url, parameter = new MFString ())
   {
      return new Promise ((resolve, reject) =>
      {
         if (!(url instanceof MFString))
            throw new Error ("Browser.loadURL: url must be of type MFString.");

         if (!(parameter instanceof MFString))
            throw new Error ("Browser.loadURL: parameter must be of type MFString.");

         // `this [_fileLoader] ?.abort ()`: the older loadURL rejects with "Loading of X3D file aborted.".
         const loader = { abort: false };

         if (this .#fileLoader)
            this .#fileLoader .abort = true;

         this .#fileLoader = loader;

         this .#load (url, loader) .then (id =>
         {
            if (this .#fileLoader !== loader)
            {
               this .#engine ?.discard (id);
               return reject (new Error ("Loading of X3D file aborted."));
            }

            this .#fileLoader = null;
            this .#replaceWorld (id) .then (resolve, reject);
         },
         error =>
         {
            if (this .#fileLoader !== loader || this .#disposed)
               return reject (new Error ("Loading of X3D file aborted."));

            this .#fileLoader = null;
            console .error (error);
            this .#call (X3DConstants .INITIALIZED_ERROR);
            this .#dispatch ("error");
            reject (new Error ("Couldn't load X3D file."));
         });
      });
   }

   // FileLoader.createX3DFromURL: the first url that fetches and parses, then its loads (Inline, textures,
   // EXTERNPROTO). An abort ends the fetches; the scene loads that started go on, like FileLoader's.
   async #load (urls, loader)
   {
      await this .#ready;

      let lastError = new Error ("no url");

      for (const u of urls)
      {
         if (loader .abort || this .#disposed)
            throw new Error ("aborted");

         let id;

         try
         {
            const response = await fetch (new URL (u, this .baseURL));

            if (!response .ok)
               throw new Error (`${response .status} ${response .statusText}: ${response .url}`);

            const bytes = new Uint8Array (await response .arrayBuffer ());

            if (loader .abort || !this .#engine)
               throw new Error ("aborted");

            id = this .#engine .prepare (bytes, response .url, Date .now () / 1000);
         }
         catch (error)
         {
            lastError = error;
            continue;
         }

         // A newer loadURL that started meanwhile takes over (the caller rejects this one).
         await this .#loaded (id, () => this .#disposed || this .#fileLoader !== loader);
         return id;
      }

      throw lastError;
   }

   // X3DBrowser.createX3DFromString (x3dSyntax): a Promise of the new scene, set up and loaded. The page is an
   // external browser: the scene's world URL is the page's base URL.
   async createX3DFromString (x3dSyntax)
   {
      x3dSyntax = String (x3dSyntax);

      await this .#ready;

      if (!this .#engine)
         throw new Error ("Browser disposed.");

      let id;

      try
      {
         id = this .#engine .prepareString (x3dSyntax, this .baseURL, Date .now () / 1000);
      }
      catch (error)
      {
         throw engineError (error);
      }

      this .#flushLog (id);
      await this .#loaded (id, () => this .#disposed);
      return this .#sceneOf (id);
   }

   // X3DBrowser.replaceWorld (scene): a Promise that resolves after "initialized load" of the new world. `null`
   // or `undefined` replaces the world with an empty scene.
   async replaceWorld (scene)
   {
      await this .#ready;

      if (!this .#engine)
         throw new Error ("Browser disposed.");

      let id = this .#sceneId (scene);

      if (id === null)
      {
         if (scene !== null && scene !== undefined)
            throw new Error ("X_ITE_R: Browser.replaceWorld: scene must be an X3DScene of this browser, null or undefined.");

         id = this .#engine .prepareString ("#X3D V4.0 utf8\n", this .baseURL, Date .now () / 1000);
         await this .#loaded (id, () => this .#disposed);
      }

      if (id === this .#engine .current)
         throw new Error ("X_ITE_R: the scene is already the world.");

      // `this [_fileLoader] ?.abort ()`: a loadURL that is still fetching fails.
      if (this .#fileLoader)
         this .#fileLoader .abort = true;

      return this .#replaceWorld (id);
   }

   // X3DBrowser.replaceWorld of a loaded engine scene: the old world is still current while its shutdown runs, one
   // frame after the call; "initialized load" follows in the next frame (checkLoadCount). A newer replaceWorld
   // rejects this one with "Replacing world aborted.".
   #replaceWorld (id)
   {
      return new Promise ((resolve, reject) =>
      {
         this .#reject ?.(new Error ("Replacing world aborted."));
         this .#reject = reject;

         const current = () => this .#reject === reject && this .#engine;
         const nextFrame = () => new Promise (requestAnimationFrame);

         (async () =>
         {
            // After X3DBrowser.initialize there is always a world (the empty one first).
            {
               await nextFrame ();

               if (!current ())
                  return this .#engine ?.discard (id);

               this .#call (X3DConstants .SHUTDOWN_EVENT);
               this .#dispatch ("shutdown");

               // There could be a replaceWorld from a shutdown event handler.
               if (!current ())
                  return this .#engine ?.discard (id);
            }

            if (!this .#engine .exists (id))
               throw new Error ("X_ITE_R: the scene is gone.");

            this .#releaseInput ();
            this .#engine .replace (id, Date .now () / 1000);
            this .#scene = this .#sceneOf (id);
            this .#flushLog ();

            await nextFrame ();

            if (!current ())
               return;

            this .#reject = null;
            this .#call (X3DConstants .INITIALIZED_EVENT);
            this .#dispatch ("initialized load");
            resolve ();
         })
         () .catch (error =>
         {
            if (this .#reject === reject)
               this .#reject = null;

            reject (error);
         });
      });
   }

   // --- viewpoints ----------------------------------------------------------------------------------------------

   // The layerNode argument of the viewpoint functions: X_ITE_R has the active layer only.
   #activeLayer (layerNode)
   {
      if (layerNode !== undefined && layerNode !== null && layerNode [_node])
         throw new Error ("X_ITE_R: the layerNode argument is not supported; use the active layer (omit it).");
   }

   // X3DBrowser.firstViewpoint, previousViewpoint, nextViewpoint, lastViewpoint ([layerNode]): the viewpoint
   // binds with the next frame's events.
   firstViewpoint (layerNode)    { this .#activeLayer (layerNode); this .#engine ?.viewpointRequest ("firstViewpoint"); }
   previousViewpoint (layerNode) { this .#activeLayer (layerNode); this .#engine ?.viewpointRequest ("previousViewpoint"); }
   nextViewpoint (layerNode)     { this .#activeLayer (layerNode); this .#engine ?.viewpointRequest ("nextViewpoint"); }
   lastViewpoint (layerNode)     { this .#activeLayer (layerNode); this .#engine ?.viewpointRequest ("lastViewpoint"); }

   // X3DBrowser.changeViewpoint ([layerNode,] name).
   changeViewpoint (layerNode, name)
   {
      if (arguments .length === 1)
      {
         name      = layerNode;
         layerNode = undefined;
      }

      this .#activeLayer (layerNode);
      this .#engine ?.viewpointRequest (`changeViewpoint:${String (name)}`);
   }

   // X3DBrowser.viewAll ([layerNode,] [transitionTime = 1]).
   viewAll (layerNode, transitionTime = 1)
   {
      if (arguments .length === 1 && typeof layerNode === "number")
      {
         transitionTime = layerNode;
         layerNode      = undefined;
      }

      this .#activeLayer (layerNode);
      this .#engine ?.viewAll (+transitionTime);
   }

   // X3DBrowserContext.getActiveViewpoint (): the bound viewpoint of the active layer; null for its default
   // viewpoint (X_ITE: the default X3DViewpointNode, which is not in the scene).
   getActiveViewpoint ()
   {
      const id = this .#engine ?.activeViewpoint ();

      return id === undefined ? null : SFNode (this, this .#scene [_node], id);
   }

   // --- browser callbacks and events ----------------------------------------------------------------------------

   // X3DBrowser.addBrowserCallback (key, [event,] callback) and removeBrowserCallback (key, [event]).
   addBrowserCallback (key, event, callback)
   {
      if (arguments .length === 2)
      {
         for (const map of this .#callbacks .values ())
            map .set (key, event);
      }
      else
      {
         this .#callbacks .get (event) ?.set (key, callback);
      }
   }

   removeBrowserCallback (key, event)
   {
      if (arguments .length === 1)
      {
         for (const map of this .#callbacks .values ())
            map .delete (key);
      }
      else
      {
         this .#callbacks .get (event) ?.delete (key);
      }
   }

   getBrowserCallbacks (event) { return this .#callbacks .get (event); }

   #call (event)
   {
      for (const callback of [ ... this .#callbacks .get (event) .values () ])
      {
         try { callback (event); } catch (error) { console .error (error); }
      }
   }

   // X3DCoreContext.callBrowserEventHandler: the on<name> property (not for load and error), then the DOM event.
   #dispatch (events)
   {
      for (const name of events .split (" "))
      {
         const event = new CustomEvent (name);

         try
         {
            if (name !== "load" && name !== "error")
               this .#element [`on${name}`] ?.(event);
         }
         catch (error)
         {
            console .error (error);
         }

         this .#element .dispatchEvent (event);
      }
   }

   #flushLog (id = this .#engine ?.current)
   {
      if (id === undefined)
         return;

      for (const line of this .#engine ?.takeLog (id) ?? [ ])
         console .warn (line);
   }

   // What the engine has for the page after an event or a frame: the cursor, the browser events of an Anchor that
   // replaced the world, and the pages and windows an Anchor or loadURL opens.
   #afterEngine ()
   {
      const engine = this .#engine;

      if (!engine)
         return;

      const cursor = engine .cursor;

      if (this .#canvas .style .cursor !== cursor)
         this .#canvas .style .cursor = cursor;

      const touchAction = TOUCH_ACTION [engine .navigationType] ?? "none";

      if (this .#canvas .style .touchAction !== touchAction)
         this .#canvas .style .touchAction = touchAction;

      for (const event of engine .takeEvents ())
      {
         switch (event)
         {
            // X3DConstants of the engine (X_ITE's internal numbers of the browser events).
            case 1004:
               this .#call (X3DConstants .SHUTDOWN_EVENT);
               this .#dispatch ("shutdown");
               break;
            case 1003:
               this .#scene = this .#sceneOf (engine .current);
               this .#call (X3DConstants .INITIALIZED_EVENT);
               this .#dispatch ("initialized load");
               break;
         }
      }

      const foreign = engine .takeForeign ();

      for (let i = 0; i < foreign .length; i += 2)
      {
         // The loadURL callback of a foreign URL: `window .open (url, target)`, or `location = url`.
         if (foreign [i])
            window .open (foreign [i + 1], foreign [i]);
         else
            location = foreign [i + 1];
      }
   }

   // --- input ---------------------------------------------------------------------------------------------------

   // X3DPointingDeviceSensorContext.getPointerFromEvent: framebuffer pixels, the origin at the bottom left.
   #pointerOf (event)
   {
      const rect = this .#canvas .getBoundingClientRect ();

      return [
         (event .clientX - rect .left) / rect .width * this .#canvas .width,
         (1 - (event .clientY - rect .top) / rect .height) * this .#canvas .height,
      ];
   }

   // The touches still down, as pointer coordinates, in the order they started (TouchEvent.touches).
   #touchPoints () { return Array .from (this .#touches .values (), t => t .pointer) .flat (); }

   // `window .innerWidth` at pointer scale: ExamineViewer's pinch zoom factor.
   #zoomWidth ()
   {
      const rect = this .#canvas .getBoundingClientRect ();

      return rect .width ? innerWidth * this .#canvas .width / rect .width : innerWidth;
   }

   // The touchmove of the touches that moved since the last one: Pointer Events report each touch on its own,
   // TouchEvent all of them together, once per frame; the viewers' two-touch gestures need them together.
   #flushTouches ()
   {
      if (!this .#touchMoved)
         return;

      this .#touchMoved = false;
      this .#engine ?.touch (1, this .#touchPoints (), this .#zoomWidth ());
   }

   // Pointer Events of the canvas become the mouse and touch events of X_ITE's PointingDevice and viewers. A
   // pointer that is down is captured, so a drag goes on outside the canvas (X_ITE listens on the document then).
   // The keyboard listeners are on the element, as X3DCoreContext's are.
   #listen ()
   {
      const canvas = this .#canvas;
      const signal = this .#listeners .signal;
      const engine = () => this .#engine;
      const on     = (target, type, listener, options = { }) => target .addEventListener (type, listener, { signal, ... options });

      on (canvas, "pointerdown", event =>
      {
         if (!engine ())
            return;

         // PointingDevice.mousedown, touchstart: `browser .getElement () .focus ({ preventScroll: true })`.
         canvas .focus ({ preventScroll: true });
         canvas .setPointerCapture ?.(event .pointerId);

         if (event .pointerType === "touch")
         {
            this .#flushTouches ();
            this .#touches .set (event .pointerId, { pointer: this .#pointerOf (event) });
            this .#lastTouch = event .timeStamp;
            engine () .touch (0, this .#touchPoints (), this .#zoomWidth ());
         }
         else
         {
            this .#pointer = this .#pointerOf (event);
            this .#buttons = event .buttons;
            engine () .pointer (0, event .button, ... this .#pointer);
         }

         this .#afterEngine ();
      });

      on (canvas, "pointermove", event =>
      {
         if (!engine ())
            return;

         if (event .pointerType === "touch")
         {
            const touch = this .#touches .get (event .pointerId);

            if (touch)
            {
               touch .pointer    = this .#pointerOf (event);
               this .#touchMoved = true;
            }

            return;
         }

         this .#pointer = this .#pointerOf (event);

         // A second button that goes down or up while another is down is a pointermove with `button` set
         // (Pointer Events "chorded buttons"); X_ITE gets a mousedown or mouseup for it.
         if (event .button >= 0 && event .buttons !== this .#buttons)
         {
            const down = (event .buttons & ~this .#buttons) !== 0;

            this .#buttons = event .buttons;
            engine () .pointer (down ? 0 : 1, event .button, ... this .#pointer);
         }
         else
         {
            engine () .pointer (2, 0, ... this .#pointer);
         }

         this .#afterEngine ();
      });

      const up = event =>
      {
         if (!engine ())
            return;

         if (event .pointerType === "touch")
         {
            if (!this .#touches .has (event .pointerId))
               return;

            this .#flushTouches ();
            this .#touches .delete (event .pointerId);
            this .#lastTouch = event .timeStamp;
            engine () .touch (2, this .#touchPoints (), this .#zoomWidth ());
         }
         else if (event .type === "pointercancel")
         {
            this .#releaseInput ();
         }
         else
         {
            this .#pointer = this .#pointerOf (event);
            this .#buttons = event .buttons;
            engine () .pointer (1, event .button, ... this .#pointer);
         }

         this .#afterEngine ();
      };

      on (canvas, "pointerup", up);
      on (canvas, "pointercancel", up);

      on (canvas, "dblclick", event =>
      {
         // A double tap is the viewers' own touch gesture; the dblclick that follows it is not.
         if (!engine () || event .timeStamp - this .#lastTouch < 1000)
            return;

         engine () .pointer (3, 0, ... this .#pointerOf (event));
         this .#afterEngine ();
      });

      on (canvas, "wheel", event =>
      {
         if (!engine ())
            return;

         // X3DViewer.wheel: `event .preventDefault ()` when the viewer zooms.
         if (engine () .wheel (event .deltaY, ... this .#pointerOf (event)))
            event .preventDefault ();

         this .#afterEngine ();
      },
      { passive: false });

      for (const type of [ "keydown", "keyup" ])
      {
         on (this .#element, type, event =>
         {
            if (!engine ())
               return;

            engine () .key (type === "keydown", event .key, event .keyCode);
            this .#afterEngine ();
         });
      }

      // A hidden page draws nothing; it draws again when it is shown.
      on (document, "visibilitychange", () => document .visibilityState === "hidden" ? this .#stop () : this .#start ());
   }

   // The buttons and touches that are down are released (removal, cancellation, a new world): the sensors and the
   // viewer end their drag.
   #releaseInput ()
   {
      const engine = this .#engine;

      for (let button = 0; button < 5; ++ button)
      {
         if (this .#buttons & (1 << [ 0, 2, 1, 3, 4 ] [button]))
            engine ?.pointer (1, button, ... this .#pointer);
      }

      this .#buttons = 0;

      if (this .#touches .size)
      {
         this .#touchMoved = false;
         this .#touches .clear ();
         engine ?.touch (2, [ ], this .#zoomWidth ());
      }
   }

   // --- frames --------------------------------------------------------------------------------------------------

   // The contentScale attribute (X_ITE's ContentScale browser option): a number, or "auto" for the device pixel
   // ratio. The default is 1.
   #contentScale ()
   {
      const value = this .#element .getAttribute ("contentScale");

      if (value === "auto")
         return devicePixelRatio;

      return Math .max (Number (value ?? 1), 0) || 1;
   }

   // X3DRenderingContext.limitFramebufferSize: the CSS size times contentScale, within the device's limit.
   #framebufferSize ()
   {
      const scale = this .#contentScale ();
      const max   = this .#engine .maxTextureDimension;

      let width  = this .#canvas .clientWidth  * scale;
      let height = this .#canvas .clientHeight * scale;

      if (width > max || height > max)
      {
         if (width > height)
         {
            height = max * height / width;
            width  = max;
         }
         else
         {
            width  = max * width / height;
            height = max;
         }
      }

      return [ Math .max (width | 0, 1), Math .max (height | 0, 1) ];
   }

   // The frame loop runs while the element is in a document, the page is visible and the browser is not disposed.
   #start ()
   {
      if (this .#frame || !this .#engine || this .#disposed || !this .#element .isConnected || document .visibilityState === "hidden")
         return;

      const tick = () =>
      {
         this .#frame = requestAnimationFrame (tick);
         ++ this .#frames;

         const engine = this .#engine;

         // A lost device draws nothing more: X_ITE's webglcontextlost calls the CONNECTION_ERROR callbacks.
         if (engine .lost)
         {
            this .#stop ();
            this .#call (X3DConstants .CONNECTION_ERROR);
            this .#fallback ("X_ITE_R stopped: the WebGPU device is lost.");
            return;
         }

         this .#flushTouches ();

         const [ width, height ] = this .#framebufferSize ();

         // Assigning the size, even the same one, replaces the canvas texture: only on a change.
         if (this .#canvas .width !== width || this .#canvas .height !== height)
         {
            this .#canvas .width  = width;
            this .#canvas .height = height;
         }

         try
         {
            engine .frame (Date .now () / 1000, width, height);
            this .#flushLog ();
         }
         catch (error)
         {
            console .error (error);
         }

         this .#afterEngine ();
      };

      this .#frame = requestAnimationFrame (tick);
   }

   #stop ()
   {
      cancelAnimationFrame (this .#frame);
      this .#frame = 0;
   }

   // X_ITE_R introspection for tests: whether a frame is scheduled and how many have run.
   get [Symbol .for ("X_ITE_R.frameLoop")] () { return { running: this .#frame !== 0, frames: this .#frames }; }

   connectedCallback ()    { this .#start (); }
   disconnectedCallback () { this .#stop (); this .#releaseInput (); }

   attributeChangedCallback (name, oldValue, newValue)
   {
      switch (name .toLowerCase ())
      {
         case "src":
         {
            if (newValue)
               this .loadURL (new MFString (newValue)) .catch (error => console .error (error));

            break;
         }
         case "url":
         {
            if (newValue)
            {
               this .loadURL (this .#element .url) .catch (error => console .error (error));
            }

            break;
         }
      }
   }

   // X3DBrowser.dispose (): the worlds are unloaded, their requests end, the input listeners go, the frame loop
   // stops and the engine is freed.
   dispose ()
   {
      if (this .#disposed)
         return;

      this .#disposed = true;
      this .#fileLoader = null;
      this .#releaseInput ();
      this .#listeners .abort ();
      this .#stop ();

      if (this .#engine)
      {
         this .#engine .free ();
         this .#engine = null;
      }
   }
}

class X3DCanvasElement extends HTMLElement
{
   constructor ()
   {
      super ();

      this .attachShadow ({ mode: "open", delegatesFocus: true });
      this .shadowRoot .innerHTML = "<style>:host { display: inline-block; width: 300px; height: 150px; }</style>";

      Object .defineProperty (this, "browser", { value: new X3DBrowser (this), enumerable: true });
   }

   get src ()      { return this .getAttribute ("src"); }
   set src (value) { this .setAttribute ("src", value); }

   get url ()      { return new MFString (... [ ... (this .getAttribute ("url") ?? "") .matchAll (/"(?:[^"\\]|\\.)*"/gs) ] .map (m => parseVrmlString (m [0]))); }
   set url (value) { this .setAttribute ("url", Array .from (value, vrmlString) .join (", ")); }

   connectedCallback ()    { this .browser .connectedCallback (); }
   disconnectedCallback () { this .browser .disconnectedCallback (); }

   static get observedAttributes () { return [ "src", "url" ]; }

   attributeChangedCallback (name, oldValue, newValue)
   {
      this .browser .attributeChangedCallback (name, oldValue, newValue);
   }

   toDataURL (... args) { return this .browser .getCanvas () .toDataURL (... args); }
   toBlob (... args)    { return this .browser .getCanvas () .toBlob (... args); }

   get [Symbol .toStringTag] () { return "X3DCanvasElement"; }
}

customElements .define ("x3d-canvas", X3DCanvasElement);

// X3D: X_ITE's namespace object, for the parts listed in API.md.
const X3D = Object .freeze ({
   X3DConstants,
   SFColor, SFColorRGBA, SFRotation, SFVec2d, SFVec2f, SFVec3d, SFVec3f, SFVec4d, SFVec4f, MFString,

   getBrowser (element)
   {
      if (!element)
         return document .querySelector ("x3d-canvas, X3DCanvas") ?.browser;

      if (element instanceof X3DCanvasElement)
         return element .browser;

      return Array .from (document .querySelectorAll (String (element)))
         .find (element => element .nodeName .match (/^(?:x3d-canvas|X3DCanvas)$/i)) ?.browser;
   },
});

globalThis .X3D ??= X3D;

export default X3D;
