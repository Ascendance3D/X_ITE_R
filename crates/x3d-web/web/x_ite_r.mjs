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

// A node of one world: its browser, its id in the engine, and the world it belongs to. After loadURL replaces the
// world, the id may name a node of the new world, so every use checks the world first.
function engineOf (node)
{
   const engine = node .browser [_engine];

   if (!engine || engine .generation !== node .generation)
      throw new Error ("X_ITE_R: the node belongs to a world that was replaced.");

   return engine;
}

// X3DField.valueOf of a field read through a node: numbers, booleans and strings as JavaScript values, the vector
// types as value objects, SFNode and MFNode as nodes; anything else as an X3DField (its `toString ()` text).
function fieldValue (node, name)
{
   const engine = engineOf (node);
   const [ typeName, , text ] = engine .field (node .id, name);

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
      const nodes = Array .from (engine .nodeValue (node .id, name), n => n < 0 ? null : SFNode (node .browser, Number (n)));

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

   #info () { return engineOf (this .#node) .field (this .#node .id, this .#name); }

   getName ()       { return this .#name; }
   getTypeName ()   { return this .#info () [0]; }
   getAccessType () { return ACCESS [this .#info () [1]]; }
   getValue ()      { return fieldValue (this .#node, this .#name); }
   setValue (value) { engineOf (this .#node) .setField (this .#node .id, this .#name, vrmlOf (this .getTypeName (), value)); }
   toString ()      { return this .#info () [2]; }
}

// SFNode: a node of the current world. Field names read and assign like X_ITE's SFNode proxy.
function SFNode (browser, id)
{
   const node   = { browser, id, generation: browser [_engine] .generation };
   const target = {
      getNodeTypeName: () => engineOf (node) .typeName (id),
      getNodeName:     () => engineOf (node) .nodeName (id) ?? "",
      getField:        name => { engineOf (node) .field (id, name); return new X3DField (node, name); },
      getTypeName:     () => "SFNode",
      toString:        () => `${engineOf (node) .typeName (id)} { }`,
   };

   // The field's [typeName, accessType, value], or null when the node has no such field.
   const field = key =>
   {
      const engine = engineOf (node);

      try { return engine .field (id, key); } catch { return null; }
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

               engineOf (node) .setField (id, key, vrmlOf (info [0], value));
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

class X3DScene
{
   #browser;

   constructor (browser) { this .#browser = browser; }

   get worldURL () { return this .#browser [_engine] ?.worldURL ?? this .#browser .baseURL; }
   getWorldURL () { return this .worldURL; }

   get rootNodes ()
   {
      // A BigInt64Array of node ids, -1 for NULL.
      return Array .from (this .#browser [_engine] ?.rootNodes () ?? [ ], n => n < 0 ? null : SFNode (this .#browser, Number (n)));
   }

   getNamedNode (name)
   {
      name = String (name);

      const id = this .#browser [_engine] ?.namedNode (name);

      if (id === undefined)
         throw new Error (`Named node '${name}' not found.`);

      return SFNode (this .#browser, id);
   }
}

class X3DBrowser
{
   #element;
   #canvas;
   #engine = null;
   #ready;
   #scene;
   #callbacks = new Map ([ 0, 1, 2, 3, 4 ] .map (event => [ event, new Map () ]));
   #frame = 0;
   #frames = 0;
   #loading = null;
   #disposed = false;

   constructor (element)
   {
      this .#element = element;
      this .#canvas  = document .createElement ("canvas");
      this .#scene   = new X3DScene (this);

      this .#canvas .style .cssText = "display:block;width:100%;height:100%";
      element .shadowRoot .append (this .#canvas);

      this .#ready = wasm .then (() => WebBrowser .create (this .#canvas)) .then (engine =>
      {
         if (this .#disposed)
         {
            engine .free ();
            throw new Error ("Browser disposed.");
         }

         this .#engine = engine;
         this .#start ();

         // X3DBrowser.initialize: replaceWorld with an empty scene, which ends in "initialized load". A loadURL
         // then always replaces a world, so it starts with "shutdown".
         this .#call (X3DConstants .INITIALIZED_EVENT);
         this .#dispatch ("initialized load");
         return this;
      });

      // Like X3DCanvasElement when X_ITE cannot start: the element shows its own content (a <slot>) instead.
      this .#ready .catch (error =>
      {
         console .error (error);

         if (this .#disposed)
            return;

         this .#canvas .remove ();
         element .shadowRoot .append (document .createElement ("slot"));
      });
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
   // ready synchronously). Rejects when the page has no WebGPU.
   ready () { return this .#ready; }

   // X3DBrowser.loadURL (url [, parameter]): replace the world with the first url that loads.
   loadURL (url, parameter = new MFString ())
   {
      return new Promise ((resolve, reject) =>
      {
         if (!(url instanceof MFString))
            throw new Error ("Browser.loadURL: url must be of type MFString.");

         if (!(parameter instanceof MFString))
            throw new Error ("Browser.loadURL: parameter must be of type MFString.");

         const loading = this .#loading = { };
         const aborted = () => this .#loading !== loading;

         this .#load (url, aborted) .then (() =>
         {
            this .#loading = null;
            resolve ();
         },
         error =>
         {
            if (aborted ())
               return reject (new Error ("Loading of X3D file aborted."));

            this .#loading = null;
            console .error (error);
            this .#call (X3DConstants .INITIALIZED_ERROR);
            this .#dispatch ("error");
            reject (new Error ("Couldn't load X3D file."));
         });
      });
   }

   // FileLoader: the first url that fetches and parses, then replaceWorld. A newer loadURL or dispose aborts this
   // one at every step, so an older load never replaces the world of a newer one.
   async #load (urls, aborted)
   {
      await this .#ready;

      let lastError = new Error ("no url");

      for (const u of urls)
      {
         if (aborted ())
            throw new Error ("aborted");

         try
         {
            const response = await fetch (new URL (u, this .baseURL));

            if (!response .ok)
               throw new Error (`${response .status} ${response .statusText}: ${response .url}`);

            const bytes = new Uint8Array (await response .arrayBuffer ());

            if (aborted ())
               throw new Error ("aborted");

            this .#engine .prepare (bytes, response .url);
         }
         catch (error)
         {
            lastError = error;
            continue;
         }

         // X3DBrowser.replaceWorld: the old world is still current while its shutdown runs.
         this .#call (X3DConstants .SHUTDOWN_EVENT);
         this .#dispatch ("shutdown");

         // A shutdown handler can start another load.
         if (aborted ())
            throw new Error ("aborted");

         this .#engine .replace (Date .now () / 1000);
         this .#flushLog ();
         this .#call (X3DConstants .INITIALIZED_EVENT);
         this .#dispatch ("initialized load");
         return;
      }

      throw lastError;
   }

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

   #flushLog ()
   {
      for (const line of this .#engine ?.takeLog () ?? [ ])
         console .warn (line);
   }

   // The frame loop runs while the element is in a document and the browser is not disposed.
   #start ()
   {
      if (this .#frame || !this .#engine || this .#disposed || !this .#element .isConnected)
         return;

      const tick = () =>
      {
         this .#frame = requestAnimationFrame (tick);
         ++ this .#frames;

         const width  = Math .max (1, this .#canvas .clientWidth);
         const height = Math .max (1, this .#canvas .clientHeight);

         // Assigning the size, even the same one, replaces the canvas texture: only on a change.
         if (this .#canvas .width !== width || this .#canvas .height !== height)
         {
            this .#canvas .width  = width;
            this .#canvas .height = height;
         }

         try
         {
            this .#engine .frame (Date .now () / 1000, width, height);
            this .#flushLog ();
         }
         catch (error)
         {
            console .error (error);
         }
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
   disconnectedCallback () { this .#stop (); }

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

   // X3DBrowser.dispose (): the world is unloaded, the frame loop stops and the engine is freed.
   dispose ()
   {
      if (this .#disposed)
         return;

      this .#disposed = true;
      this .#loading  = null;
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
