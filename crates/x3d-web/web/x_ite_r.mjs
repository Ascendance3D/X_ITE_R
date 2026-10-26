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

class MFString extends Array
{
   constructor (... values)
   {
      super ();
      this .push (... values .map (String));
   }

   getTypeName () { return "MFString"; }
   toString () { return `[${this .map (s => JSON .stringify (s)) .join (", ")}]`; }
}

// The VRML text of a value assigned from JavaScript, for a field of `typeName`.
function vrmlOf (typeName, value)
{
   switch (typeName)
   {
      case "SFBool":
         return value ? "TRUE" : "FALSE";
      case "SFString":
         return JSON .stringify (String (value));
      case "SFFloat":
      case "SFDouble":
      case "SFTime":
      case "SFInt32":
         return String (Number (value));
      case "MFString":
         return `[${Array .from (value, s => JSON .stringify (String (s))) .join (", ")}]`;
   }

   if (typeName in NUMERIC && value instanceof NUMERIC [typeName])
      return value .toString ();

   throw new TypeError (`X_ITE_R: cannot assign ${value} to a field of type ${typeName}.`);
}

// --- nodes -------------------------------------------------------------------------------------------------------

// The wasm object of a browser (read-only, for tests and diagnostics).
const _engine = Symbol .for ("X_ITE_R.engine");
const _id     = Symbol ("X_ITE_R.node");

// X3DField.valueOf of a field read through a node: numbers, booleans and strings as JavaScript values, the vector
// types as value objects; anything else as its `toString ()` text.
function fieldValue (browser, id, name)
{
   const [ typeName, , text ] = browser [_engine] .field (id, name);

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
         return JSON .parse (text);
   }

   if (typeName in NUMERIC)
      return NUMERIC [typeName] .fromString (text);

   // SFNode: the node or null; MFNode: an Array of them (X_ITE: an MFNode).
   if (typeName === "SFNode" || typeName === "MFNode")
   {
      const nodes = Array .from (browser [_engine] .nodeValue (id, name), n => n < 0 ? null : SFNode (browser, Number (n)));

      return typeName === "SFNode" ? nodes [0] : nodes;
   }

   return new X3DField (browser, id, name);
}

// X3DNode.getField (name): one field of one node.
class X3DField
{
   #browser; #id; #name;

   constructor (browser, id, name)
   {
      this .#browser = browser;
      this .#id      = id;
      this .#name    = name;
   }

   #info () { return this .#browser [_engine] .field (this .#id, this .#name); }

   getName ()       { return this .#name; }
   getTypeName ()   { return this .#info () [0]; }
   getAccessType () { return ACCESS [this .#info () [1]]; }
   getValue ()      { return fieldValue (this .#browser, this .#id, this .#name); }
   setValue (value) { this .#browser [_engine] .setField (this .#id, this .#name, vrmlOf (this .getTypeName (), value)); }
   toString ()      { return this .#info () [2]; }
}

// SFNode: a node of the world. Field names read and assign like X_ITE's SFNode proxy.
function SFNode (browser, id)
{
   const target = {
      [_id]: id,
      getNodeTypeName: () => browser [_engine] .typeName (id),
      getNodeName:     () => browser [_engine] .nodeName (id) ?? "",
      getField:        name => { browser [_engine] .field (id, name); return new X3DField (browser, id, name); },
      getTypeName:     () => "SFNode",
      toString:        () => `${browser [_engine] .typeName (id)} { }`,
   };

   const field = key =>
   {
      try { return browser [_engine] .field (id, key); } catch { return null; }
   };

   return new Proxy (target, {
      get (target, key)
      {
         if (typeof key === "string" && !(key in target))
         {
            const info = field (key);

            // X_ITE allows reading initializeOnly fields too; an inputOnly field reads as undefined.
            if (info)
               return info [1] === "inputOnly" ? undefined : fieldValue (browser, id, key);
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

               browser [_engine] .setField (id, key, vrmlOf (info [0], value));
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

      this .#ready .catch (error => console .error (error));
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

         this .#load (url) .then (() =>
         {
            if (this .#loading !== loading)
               return reject (new Error ("Loading of X3D file aborted."));

            this .#loading = null;
            resolve ();
         },
         error =>
         {
            if (this .#loading !== loading)
               return reject (new Error ("Loading of X3D file aborted."));

            this .#loading = null;
            console .error (error);
            this .#call (X3DConstants .INITIALIZED_ERROR);
            this .#dispatch ("error");
            reject (new Error ("Couldn't load X3D file."));
         });
      });
   }

   async #load (urls)
   {
      await this .#ready;

      let lastError = new Error ("no url");

      for (const u of urls)
      {
         try
         {
            const response = await fetch (new URL (u, this .baseURL));

            if (!response .ok)
               throw new Error (`${response .status} ${response .statusText}: ${response .url}`);

            const bytes = new Uint8Array (await response .arrayBuffer ());

            if (this .#disposed)
               throw new Error ("Browser disposed.");

            this .#engine .load (bytes, response .url, Date .now () / 1000);
            this .#call (X3DConstants .SHUTDOWN_EVENT);
            this .#dispatch ("shutdown");

            this .#flushLog ();
            this .#call (X3DConstants .INITIALIZED_EVENT);
            this .#dispatch ("initialized load");
            return;
         }
         catch (error)
         {
            lastError = error;
         }
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
               const urls = [ ... newValue .matchAll (/"((?:[^"\\]|\\.)*)"/g) ] .map (m => JSON .parse (`"${m [1]}"`));

               this .loadURL (new MFString (... urls)) .catch (error => console .error (error));
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
   browser;

   constructor ()
   {
      super ();

      this .attachShadow ({ mode: "open", delegatesFocus: true });
      this .shadowRoot .innerHTML = "<style>:host { display: inline-block; width: 300px; height: 150px; }</style>";
      this .browser = new X3DBrowser (this);
   }

   get src ()      { return this .getAttribute ("src"); }
   set src (value) { this .setAttribute ("src", value); }

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

   getBrowser (element = "x3d-canvas")
   {
      const canvas = typeof element === "string" ? document .querySelector (element) : element;

      return canvas ?.browser ?? null;
   },
});

globalThis .X3D ??= X3D;

export default X3D;
