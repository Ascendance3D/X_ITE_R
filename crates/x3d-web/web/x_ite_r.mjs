// X_ITE_R page adapter: defines <x3d-canvas> and the X_ITE-named JavaScript API on top of the Rust engine
// (crates/x3d-web, built into ./pkg by tools/build-web.sh). Everything X3D (parsing, the scene graph, events,
// rendering) runs in Rust; this file only fetches the world, runs the requestAnimationFrame loop and converts
// values. The names follow X_ITE 16.5.0 (oracle/x_ite-pin); web/API.md lists what exists and what does not.

import init, { WebBrowser, parseFieldWithoutScene } from "./pkg/x3d_web.js";
import
{
   __bridge, X3DConstants, X3DChildObject, X3DField, X3DArrayField, X3DFieldDefinition, FieldDefinitionArray, SFNodeCache,
   Fields, NODE_TYPES, X3D_CONSTANTS_CORE, X3D_COMPONENT_CONSTANTS,
}
from "./pkg/fields.mjs";

const wasm = init ();

// Whether the engine's code is loaded: the text of a field value is read by it (X3DField.fromVRMLString).
let wasmReady = false;

wasm .then (() => wasmReady = true);

// --- field values ------------------------------------------------------------------------------------------------
//
// The field objects of the page are X_ITE's own (pkg/fields.mjs, the classes the Script engine uses too) and hold
// their value in JavaScript, as in X_ITE. The engine owns the nodes. An X3DNodeRef stands where X_ITE has the
// X3DNode: the value of an SFNode, the parent of the node's field objects. A node field has one object for as long
// as the page uses the node; its value is mirrored with the engine:
//
// * the engine's changes (events, routes, sensors) reach the object after each frame and before each getField
//   (takeFieldChanges);
// * the object's events (setValue, an assignment through the SFNode proxy, a changed component or element) reach
//   the engine at once (X3DChildObject.addEvent → setFieldData), and the field sends its event in the next frame;
// * the field callbacks (addFieldCallback) run after each frame, once per event, in event order, with the value the
//   event delivered (takeFieldEvents).
//
// Values cross as plain data (x3d-runtime data.rs, the shape of the Script prelude's bridge.js).

// X3DConstants: X_ITE's, with the node types of the Core profile (oracle/x_ite-node-types.json). The node types of
// other components are numbered after them when a node of one is first seen (X_ITE numbers them as the components
// load).
for (const [name, value] of X3D_CONSTANTS_CORE)
   X3DConstants .addConstant (name, value);

let nextConstant = Math .max (... X3D_CONSTANTS_CORE .map (([, value]) => value)) + 1;

const componentOf = new Map ();

for (const [component, names] of Object .entries (X3D_COMPONENT_CONSTANTS))
{
   for (const name of names)
      componentOf .set (name, component);
}

function constantOf (name)
{
   // An external component adds the constants of all its node types when it loads.
   for (const constant of X3D_COMPONENT_CONSTANTS [componentOf .get (name)] ?? [ name ])
   {
      if (!Object .hasOwn (X3DConstants, constant))
         X3DConstants .addConstant (constant, nextConstant ++);
   }

   if (!Object .hasOwn (X3DConstants, name))
      X3DConstants .addConstant (name, nextConstant ++);

   return X3DConstants [name];
}

// The wasm object of a browser (read-only, for tests and diagnostics).
const _engine = Symbol .for ("X_ITE_R.engine");

// The handle of a node or scene: its browser, its scene id in the engine, the world generation of that scene (an
// Anchor can replace the world inside a scene) and, for a node, its id.
const _node = Symbol ("X_ITE_R.node");

// The engine of a handle. Every use checks the scene and its world generation first, so an old handle never reaches
// a node of a newer world.
function engineOf (handle)
{
   const engine = handle .browser [_engine];

   if (!engine || !engine .exists (handle .scene) || engine .worldGeneration (handle .scene) !== handle .generation)
      throw new Error ("X_ITE_R: the node belongs to a world that was replaced.");

   retireReleased (handle .browser, handle .scene);

   if (handle .released)
      throw new Error (RELEASED);

   return engine;
}

const RELEASED = "X_ITE_R: the node was released; it is no longer in the world.";

// An error of the engine as a page Error.
const pageError = error => error instanceof Error && error .constructor === Error ? error : new Error (String (error ?.message ?? error));

const FIELDS      = Symbol ("fields");      // field name → { field, data }
const NAMES       = Symbol ("names");       // the names of the node's fields
const DEFINITIONS = Symbol ("definitions"); // its FieldDefinitionArray
const USER_DATA   = Symbol ("userData");

// The X3DNodeRefs of a browser: scene id → { generation, refs: node id → X3DNodeRef }.
const nodeRefTables = new WeakMap ();

function sceneRefs (browser, scene, generation)
{
   const tables = nodeRefTables .get (browser) ?? nodeRefTables .set (browser, new Map ()) .get (browser);

   let table = tables .get (scene);

   if (!table || table .generation !== generation)
      tables .set (scene, table = { generation, refs: new Map () });

   return table .refs;
}

function X3DNodeRef (handle)
{
   X3DChildObject .call (this);

   this [_node]       = handle;
   this [FIELDS]      = new Map ();
   this [NAMES]       = undefined;
   this [DEFINITIONS] = undefined;
   this [USER_DATA]   = undefined;
}

Object .assign (Object .setPrototypeOf (X3DNodeRef .prototype, X3DChildObject .prototype),
{
   getTypeName ()
   {
      const h = this [_node];

      return engineOf (h) .typeName (h .scene, h .id);
   },
   getType ()
   {
      // A PROTO instance is an X3DPrototypeInstance (an X3DNode).
      return (NODE_TYPES [this .getTypeName ()] ?? [ "X3DNode", "X3DPrototypeInstance" ]) .map (constantOf);
   },
   getName ()
   {
      const h = this [_node];

      return engineOf (h) .nodeName (h .scene, h .id) ?? "";
   },
   getDisplayName ()
   {
      return this .getName () .replace (/_\d+$/, "");
   },
   getField (name)
   {
      return attachedField (this, String (name));
   },
   // X3DBaseNode.getFieldDefinitions: the class's definitions (their values are the defaults), then the user-defined
   // fields (their values are the fields).
   getFieldDefinitions ()
   {
      const h = this [_node];

      return this [DEFINITIONS] ??= new FieldDefinitionArray (Array .from (engineOf (h) .fieldDefinitions (h .scene, h .id),
         ([ name, accessType, dataType, value ]) => value === undefined
            ? new X3DFieldDefinition (accessType, name, attachedField (this, name))
            : classDefinition (name, accessType, dataType, value)));
   },
   getFieldDefinition (name)
   {
      const definition = this .getFieldDefinitions () .get (name);

      if (definition)
         return definition;

      throw new Error (`Unknown field '${name}' in node class ${this .getTypeName ()}.`);
   },
   getUserData (key)
   {
      return this [USER_DATA] ?.get (key);
   },
   setUserData (key, value)
   {
      (this [USER_DATA] ??= new Map ()) .set (key, value);
   },
   removeUserData (key)
   {
      this [USER_DATA] ?.delete (key);
   },
   // X3DBaseNode.addEvent (field): a field of the node made an event.
   addEvent (field)
   {
      pushField (field);
   },
   addEventObject (field)
   {
      pushField (field);
   },
   // The engine keeps the parents of a node.
   addParent ()
   { },
   removeParent ()
   { },
   isInitialized ()
   {
      return true;
   },
   setup ()
   { },
   dispose ()
   { },
   // X3DNode.toStream: the node's type name only. The other text forms of an SFNode or MFNode print the node in full
   // (below, "text of nodes").
   toStream (generator)
   {
      generator .string += `${this .getTypeName ()} { }`;
   },
   toVRMLStream (generator)
   {
      this .toStream (generator);
   },
});

for (const key of Object .keys (X3DNodeRef .prototype))
   Object .defineProperty (X3DNodeRef .prototype, key, { enumerable: false });

// The definitions of the node classes, one object each (X_ITE: Class .fieldDefinitions).
const classDefinitions = new Map ();

function classDefinition (name, accessType, dataType, value)
{
   const key = JSON .stringify ([ name, accessType, dataType, value ]);

   let definition = classDefinitions .get (key);

   if (!definition)
      classDefinitions .set (key, definition = new X3DFieldDefinition (accessType, name, fieldFromData (dataType, value, null)));

   return definition;
}

// The engine released nodes of scene `scene` of `browser`: a node that takes the id of one is another node, so the
// X3DNodeRef of the id refuses to be used from now on and the id gets a new X3DNodeRef.
function retireReleased (browser, scene)
{
   const released = browser [_engine] ?.takeReleasedNodes (scene);

   if (!released ?.length)
      return;

   const refs = nodeRefTables .get (browser) ?.get (scene) ?.refs;

   for (const id of released)
   {
      const ref = refs ?.get (id);

      if (!ref)
         continue;

      refs .delete (id);
      ref [_node] .released = true;
   }
}

// The X3DNodeRef of node `id` of the world of `owner` (a scene or node handle).
function nodeRef (owner, id)
{
   retireReleased (owner .browser, owner .scene);

   const refs = sceneRefs (owner .browser, owner .scene, owner .generation);

   let ref = refs .get (id);

   if (!ref)
   {
      refs .set (id, ref = new X3DNodeRef ({ browser: owner .browser, scene: owner .scene, generation: owner .generation, id }));
      SFNodeCache .set (ref, guardNode (new Fields .SFNode (ref)));
   }

   return ref;
}

// An SFNode as the page gets it: X_ITE's own, which in addition refuses to be used when its node belongs to a world
// that was replaced (API.md, known differences). X_ITE's proxy would read such a field as undefined: it catches the
// error of getField.
function guardNode (sfnode)
{
   const check = key =>
   {
      if (typeof key !== "string")
         return;

      const node = X3DField .prototype .getValue .call (sfnode);

      if (node instanceof X3DNodeRef)
         engineOf (node [_node]);
   };

   return new Proxy (sfnode,
   {
      get (target, key) { check (key); return Reflect .get (target, key); },
      set (target, key, value) { check (key); return Reflect .set (target, key, value); },
      has (target, key) { check (key); return Reflect .has (target, key); },
   });
}

// The SFNode of node `id` of the world of `owner`, or null.
function nodeObject (owner, id)
{
   return id === null || id === undefined || id < 0 ? null : SFNodeCache .get (nodeRef (owner, Number (id)));
}

// The handle of an SFNode of this adapter, or undefined.
function handleOf (node)
{
   return node instanceof Fields .SFNode ? node .getValue () ?.[_node] : undefined;
}

const FOREIGN = "X_ITE_R: a node can only be used in the world it belongs to.";

// The node id of an SFNode in the world of `owner`; null for NULL.
function nodeIdOf (sfnode, owner)
{
   const handle = handleOf (sfnode);

   if (!handle)
      return null;

   if (handle .browser !== owner .browser || handle .scene !== owner .scene || handle .generation !== owner .generation)
   {
      // X_ITE's SFNode and MFNode setters swallow an error and keep a plain property instead (SFNode.js, the `set` trap),
      // so the refusal would pass without a trace. The warning names it.
      console .warn (FOREIGN);

      throw new Error (FOREIGN);
   }

   retireReleased (owner .browser, owner .scene);

   if (handle .released)
      throw new Error (RELEASED);

   return handle .id;
}

const attachedOf = new WeakMap (); // field object (and its target) → { ref, name, entry }

// X3DNode.getField (name): the node's field object, current with the engine.
function attachedField (ref, name)
{
   const h      = ref [_node];
   const engine = engineOf (h);

   // X_ITE's SFNode proxy asks for a field first for every name, its own methods included: only a field name or the
   // `set_` or `_changed` name of one goes to the engine, which resolves those names as X3DBaseNode.getField does.
   // Both names give the same field object.
   ref [NAMES] ??= new Set (engine .fieldNames (h .scene, h .id));

   if (!ref [NAMES] .has (name))
   {
      const base = name .replace (/^set_|_changed$/, "");

      if (base === name || !ref [NAMES] .has (base))
         throw new Error (`Unknown field '${name}' in node class ${ref .getTypeName ()}.`);

      try
      {
         engine .fieldData (h .scene, h .id, name);
      }
      catch (error)
      {
         throw pageError (error);
      }

      name = base;
   }

   syncScene (h .browser, h .scene);

   let entry = ref [FIELDS] .get (name);

   if (entry)
      return entry .field;

   let info;

   try
   {
      info = engine .fieldData (h .scene, h .id, name);
   }
   catch (error)
   {
      throw pageError (error);
   }

   const [ type, accessType, data ] = info;
   const value = fieldFromData (type, data, h);
   const field = type === "SFNode" ? guardNode (value) : value;

   field .setName (name);
   field .setAccessType (accessType);
   field .addParent (ref);

   entry = { field, data };
   ref [FIELDS] .set (name, entry);

   const attached = { ref, name, entry };

   attachedOf .set (field, attached);
   attachedOf .set (value, attached);

   // An array field is a Proxy of its target, and X_ITE's array code makes its events on the target.
   if (field instanceof X3DArrayField)
      attachedOf .set (field .getTarget (), attached);

   // So is an SFNode (its addFieldCallback adds to its target): the target is the own symbol value that is an SFNode
   // and not the proxy.
   if (field instanceof Fields .SFNode)
   {
      for (const symbol of Object .getOwnPropertySymbols (value))
      {
         const target = value [symbol];

         if (target instanceof Fields .SFNode && target !== field && target !== value)
            attachedOf .set (target, attached);
      }
   }

   return field;
}

// X3DChildObject.addEvent of a node's field object: the engine takes its value.
function pushField (field)
{
   const attached = attachedOf .get (field);

   if (!attached)
      return;

   const h = attached .ref [_node];
   let data;

   try
   {
      data = fieldData (attached .entry .field, h);
   }
   catch (error)
   {
      // A node of another world refuses the whole value and is warned about already. X_ITE takes it, so the page's
      // operation goes on: the engine keeps the value it had, and the page's field is not left broken for the next one.
      if (error ?.message === FOREIGN)
         return;

      throw error;
   }

   attached .entry .data = data;

   try
   {
      engineOf (h) .setFieldData (h .scene, h .id, attached .name, data);
   }
   catch (error)
   {
      throw pageError (error);
   }
}

// The VRML encoding of a string: quotes, with `"` and `\` escaped (X3DGenerator.EscapeString). Other characters,
// newlines included, stand as they are.
const vrmlString = string => `"${String (string) .replace (/["\\]/g, c => `\\${c}`)}"`;

// The string of a VRML-encoded SFString.
const parseVrmlString = text => text .trim () .slice (1, -1) .replace (/\\(.)/gs, "$1");

// The engine's changes of the fields the page holds objects of, in scene `scene` of `browser`.
function syncScene (browser, scene)
{
   retireReleased (browser, scene);

   const table = nodeRefTables .get (browser) ?.get (scene);

   if (!table ?.refs .size)
      return;

   const changes = browser [_engine] .takeFieldChanges (scene);

   for (let i = 0; i < changes .length; i += 3)
   {
      const ref   = table .refs .get (changes [i]);
      const entry = ref ?.[FIELDS] .get (changes [i + 1]);

      if (!entry)
         continue;

      entry .data = changes [i + 2];
      setFieldData (entry .field, entry .data, ref [_node]);
   }
}

// After a frame: the engine's changes, then the field callbacks of the events of the frame, in event order, each with
// the value its event delivered. A callback that throws is reported, and the other callbacks run.
function processFieldEvents (browser, scene)
{
   retireReleased (browser, scene);

   const table = nodeRefTables .get (browser) ?.get (scene);

   if (!table ?.refs .size)
      return;

   const events = browser [_engine] .takeFieldEvents (scene);

   syncScene (browser, scene);

   for (let i = 0; i < events .length; i += 3)
   {
      const ref   = table .refs .get (events [i]);
      const entry = ref ?.[FIELDS] .get (events [i + 1]);

      if (!entry)
         continue;

      const callbacks = entry .field .getFieldCallbacks ();

      if (!callbacks .size)
         continue;

      // The callbacks see the value of their event; the field holds the current one again after them.
      setFieldData (entry .field, events [i + 2], ref [_node]);

      for (const callback of Array .from (callbacks .values ()))
      {
         try
         {
            callback (entry .field .valueOf ());
         }
         catch (error)
         {
            console .error (error);
         }
      }

      setFieldData (entry .field, entry .data, ref [_node]);
   }
}

// X3DField.addFieldCallback of a node's field object: the engine reports its events from now on.
__bridge .fieldCallbackAdded = function (field)
{
   const attached = attachedOf .get (field);

   if (!attached)
      return;

   const h = attached .ref [_node];

   engineOf (h) .observeField (h .scene, h .id, attached .name, true);
};

// The plain data of a field object; `owner` is the handle of the world its nodes must belong to.
function fieldData (field, owner)
{
   const type = field .getTypeName ();

   switch (type)
   {
      case "SFBool": case "SFDouble": case "SFFloat": case "SFInt32": case "SFString": case "SFTime":
         return field .getValue ();
      case "SFNode":
         return nodeIdOf (field, owner);
      case "SFImage":
         return [ field .width, field .height, field .comp, Array .from (field .array) ];
      case "SFRotation":
      {
         const r = field .getValue (), s = __bridge .rotation4, q = r [s ._quaternion];

         return [ r [s ._x], r [s ._y], r [s ._z], r [s ._angle], r [s ._tainted] ? 1 : 0, q .x, q .y, q .z, q .w ];
      }
      case "MFNode":
         return Array .from (field, node => nodeIdOf (node, owner));
      case "MFString":
         return Array .from (field, String);
      case "MFBool":
         return Array .from (field, v => !! v);
      case "MFImage":
         return Array .from (field, image => fieldData (image, owner));
   }

   if (type .startsWith ("MF"))
   {
      const target = field .getTarget ();

      return Array .from (target .getValue () .subarray (0, target .length * target .getComponents ()));
   }

   return Array .from (field .getValue (), Number);
}

// A new field object of type `type` with the value `data`; `owner` is the handle of the world of its nodes.
function fieldFromData (type, data, owner)
{
   const C = Fields [type];

   switch (type)
   {
      case "SFBool": case "SFDouble": case "SFFloat": case "SFInt32": case "SFString": case "SFTime":
         return new C (data);
      case "SFNode":
         return new C (data === null ? null : nodeRef (owner, data));
      case "SFImage":
         return new C (data [0], data [1], data [2], new Fields .MFInt32 (... data [3]));
      case "SFRotation":
      {
         const field = new C (), r = field .getValue (), s = __bridge .rotation4;

         r [s ._x] = data [0]; r [s ._y] = data [1]; r [s ._z] = data [2]; r [s ._angle] = data [3];
         r [s ._tainted] = !! data [4];
         r [s ._quaternion] .set (data [5], data [6], data [7], data [8]);
         return field;
      }
      case "MFNode":
         return new C (... data .map (id => nodeObject (owner, id)));
      case "MFString":
      case "MFBool":
         return new C (... data);
      case "MFImage":
         return new C (... data .map (d => fieldFromData ("SFImage", d, owner)));
   }

   if (type .startsWith ("MF"))
   {
      const field = new C (), target = field .getTarget ();

      target .set (data, data .length / target .getComponents ());
      return field;
   }

   return new C (... data);
}

// Give `field` the value `data` without an event (X3DField.set): the internal value objects stay the same objects.
function setFieldData (field, data, owner)
{
   const value = fieldFromData (field .getTypeName (), data, owner);

   if (field instanceof X3DArrayField)
      field .getTarget () .set (value .getValue (), value .length);
   else
      field .set (value .getValue ());
}

// --- text of nodes, values read from text ------------------------------------------------------------------------
//
// X3DNode.toStream prints `TypeName { }` only, so toString () needs nothing here. toVRMLString (), toXMLString () and
// toJSONString () print the node in full: the engine's printer (the one of the Script engine, with the scene's
// DEF/USE names) prints all nodes of an SFNode or MFNode at once. Options of the generators are not supported.
{
   const printed = (kind, original) => function (generator)
   {
      const type  = this .getTypeName ();
      const nodes = type === "SFNode" ? [ this ] : Array .from (this);
      const owner = nodes .map (handleOf) .find (Boolean);

      // NULL, or no node: nothing for the engine to name.
      if (!owner)
         return original .call (this, generator);

      generator .string += engineOf (owner) .printNodes (owner .scene, kind, type, fieldData (this, owner));
   };

   for (const type of [ "SFNode", "MFNode" ])
   {
      const prototype = Fields [type] .prototype;

      for (const [ method, kind ] of [ [ "toVRMLStream", "vrml" ], [ "toXMLStream", "xml" ], [ "toJSONStream", "json" ] ])
      {
         const original = prototype [method];

         Object .defineProperty (prototype, method, { value: printed (kind, original), writable: true, configurable: true, enumerable: false });
      }
   }
}

// X3DField.fromVRMLString / fromXMLString (value, scene) (VRMLParser.js and XMLParser.js put these on X3DField): the
// value is read into the field with setValue, so a node field makes an event. `scene` is an X3DScene of this adapter:
// the nodes of the value are made in its world and the field's unit converts the value. Without a scene, a value that
// needs a node is a TypeError, as in X_ITE.
function parseField (field, value, scene, xml)
{
   const handle = scene ?.[_node];
   const type   = field .getTypeName ();
   const name   = field .getName ();

   if (scene && !handle ?.browser)
      throw new TypeError ("scene .createNode is not a function");

   let data;

   try
   {
      if (handle)
      {
         data = engineOf (handle) .parseField (handle .scene, type, String (value), xml, name, field .getUnit ?.());
      }
      else
      {
         if (!wasmReady)
            throw new Error ("X_ITE_R: the engine is not loaded yet; wait for browser .ready ().");

         // The parser's warnings go to the console, as the browser's console prints them in X_ITE.
         const [ result, ... warnings ] = parseFieldWithoutScene (type, String (value), xml, name, scene === null ? "null" : "undefined");

         for (const warning of warnings)
            console .warn (warning);

         data = result;
      }
   }
   catch (error)
   {
      throw engineError (error);
   }

   field .setValue (fieldFromData (type, data, handle));
}

X3DField .prototype .fromVRMLString = function (value, scene) { parseField (this, value, scene, false); };
X3DField .prototype .fromXMLString  = function (value, scene) { parseField (this, value, scene, true); };

for (const key of [ "fromVRMLString", "fromXMLString" ])
   Object .defineProperty (X3DField .prototype, key, { enumerable: false });

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
      return new Fields .MFNode (... Array .from (this .#engine () ?.rootNodes (this .#handle .scene) ?? [ ], n => nodeObject (this .#handle, Number (n))));
   }

   getNamedNode (name)
   {
      name = String (name);

      const id = this .#engine () ?.namedNode (this .#handle .scene, name);

      if (id === undefined)
         throw new Error (`Named node '${name}' not found.`);

      return nodeObject (this .#handle, id);
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

// BrowserOptions.js fieldDefinitions of X_ITE 16.5.0: name, field type, initial value.
const BROWSER_OPTIONS = [
   [ "SplashScreen", "SFBool", true ], [ "Dashboard", "SFBool", true ], [ "Rubberband", "SFBool", true ],
   [ "EnableInlineViewpoints", "SFBool", true ], [ "Antialiased", "SFBool", true ],
   [ "TextureQuality", "SFString", "MEDIUM" ], [ "PrimitiveQuality", "SFString", "MEDIUM" ],
   [ "QualityWhenMoving", "SFString", "SAME" ], [ "Shading", "SFString", "GOURAUD" ], [ "MotionBlur", "SFBool", false ],
   [ "AlwaysUpdateGeometries", "SFBool", false ], [ "AutoUpdate", "SFBool", false ], [ "Cache", "SFBool", true ],
   [ "ColorSpace", "SFString", "LINEAR_WHEN_PHYSICAL_MATERIAL" ], [ "ContentScale", "SFDouble", 1 ],
   [ "ContextMenu", "SFBool", true ], [ "Debug", "SFBool", false ], [ "Exposure", "SFDouble", 1 ],
   [ "DisplayColorSpace", "SFString", "SRGB" ], [ "Gravity", "SFDouble", 9.80665 ], [ "LoadUrlObjects", "SFBool", true ],
   [ "LogarithmicDepthBuffer", "SFBool", false ], [ "MaximumFrameRate", "SFDouble", 80 ],
   [ "MetadataReference", "SFString", "" ], [ "Multisampling", "SFInt32", 4 ], [ "Mute", "SFBool", false ],
   [ "Notifications", "SFBool", true ], [ "OrderIndependentTransparency", "SFBool", false ],
   [ "SoundIntensity", "SFFloat", 1 ], [ "StraightenHorizon", "SFBool", true ],
   [ "TextCompression", "SFString", "CHAR_SPACING" ], [ "Timings", "SFBool", false ], [ "ToneMapping", "SFString", "NONE" ],
   [ "WallFriction", "SFDouble", 0 ], [ "XRSessionMode", "SFString", "IMMERSIVE_VR" ],
];

// BrowserProperties.js fieldDefinitions: constants.
const BROWSER_PROPERTIES = [
   [ "ABSTRACT_NODES", true ], [ "CONCRETE_NODES", true ], [ "EXTERNAL_INTERACTIONS", true ], [ "PROTOTYPE_CREATE", true ],
   [ "DOM_IMPORT", true ], [ "XML_ENCODING", true ], [ "CLASSIC_VRML_ENCODING", true ], [ "BINARY_ENCODING", false ],
];

// The aliases of BrowserOptions and RenderingProperties: AntiAliased is Antialiased.
const ALIASES = Object .assign (Object .create (null), { AntiAliased: "Antialiased" });

// The rendering properties that are the value of a browser option. X_ITE's Shading, Antialiased, ContentScale and
// Multisampling properties are derived from the options and the context (the shading name, the samples clamped to
// the context's maximum, whether the context is multisampled) and are not implemented.
const RENDERING_OPTIONS     = [ "LogarithmicDepthBuffer" ];
const RENDERING_UNSUPPORTED = [ "Shading", "Antialiased", "AntiAliased", "ContentScale", "Multisampling", "MaxTextureSize", "TextureUnits", "MaxLights", "ColorDepth", "TextureMemory", "MaxAnisotropicDegree", "MaxSamples", "PixelsPerPoint", "SoundDestinations", "XRSession" ];

class X3DBrowser
{
   #element;
   #canvas;
   #engine = null;
   #ready;
   #scene;
   #scenes = new Map ();
   #callbacks = new Map ([ "CONNECTION_ERROR", "BROWSER_EVENT", "INITIALIZED_EVENT", "SHUTDOWN_EVENT", "INITIALIZED_ERROR" ]
      .map (name => [ X3DConstants [name], new Map () ]));
   #frame = 0;
   #frames = 0;
   #fileLoader = null;
   #reject = null;
   #replacing = null;
   // The X3DScene object of the page that the replaceWorld in progress makes the world: while the call waits for
   // frames, nothing else may hold it, and its FinalizationRegistry entry would discard the engine scene.
   #replacingScene = null;
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

   // X3DBrowser.getBrowserOption, setBrowserOption, getBrowserProperty, getRenderingProperty, print, println. The
   // values live in typed fields of the pinned definitions (BrowserOptions.js, BrowserProperties.js), so the
   // coercion and the unknown-name error are X_ITE's. A set option is stored and read back; the renderer honors
   // only the options API.md lists.
   #options    = new Map (BROWSER_OPTIONS .map (([ name, type, value ]) => [ name, new Fields [type] (value) ]));
   #properties = new Map (BROWSER_PROPERTIES .map (([ name, value ]) => [ name, new Fields .SFBool (value) ]));

   // X3DBaseNode.getField: the name, or `set_name` and `name_changed` of an inputOutput field (the options only), or
   // an alias; any other name throws, and a name that is not a string throws the TypeError of X_ITE's `name.match`.
   static #field (map, name, typeName, inputOutput = false)
   {
      const field = map .get (name) ?? map .get (ALIASES [name]);

      if (field)
         return field;

      const match = name .match (/^set_(.*?)$/) ?? name .match (/^(.*?)_changed$/);

      if (match && inputOutput && map .has (match [1]))
         return map .get (match [1]);

      throw new Error (`Unknown field '${name}' in node class ${typeName}.`);
   }

   getBrowserOption (name) { return X3DBrowser .#field (this .#options, name, "BrowserOptions", true) .getValue (); }

   setBrowserOption (name, value) { X3DBrowser .#field (this .#options, name, "BrowserOptions", true) .setValue (value); }

   getBrowserProperty (name) { return X3DBrowser .#field (this .#properties, name, "BrowserProperties") .getValue (); }

   // The rendering properties that are the value of a browser option (LogarithmicDepthBuffer). The properties X_ITE
   // derives from the options and the context (Shading, Antialiased, ContentScale, Multisampling) and those that
   // describe its WebGL context (MaxTextureSize, TextureUnits, MaxLights, ColorDepth, TextureMemory,
   // MaxAnisotropicDegree, MaxSamples, PixelsPerPoint, SoundDestinations, XRSession) are not implemented (API.md).
   getRenderingProperty (name)
   {
      if (typeof name !== "string")
         name .match (/^/); // X_ITE's getField throws the TypeError of `name.match` for a name that is not a string.

      if (RENDERING_OPTIONS .includes (name))
         return this .getBrowserOption (name);

      if (RENDERING_UNSUPPORTED .includes (name))
         throw new Error (`X_ITE_R: the rendering property '${name}' is not implemented.`);

      throw new Error (`Unknown field '${name}' in node class RenderingProperties.`);
   }

   print (... args) { console .log (args .join (" ")); }

   println (... args) { console .log (args .join (" ")); }

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

         // The messages of a load go out when the response comes, as X_ITE's do.
         const loaded = this .#engine .loaded (id, Date .now () / 1000);

         this .#flushLog (id);

         if (loaded)
            return;

         await this .#engine .progress (id);
      }
   }

   // X3DBrowser.loadURL (url [, parameter]): replace the world with the first url that loads.
   loadURL (url, parameter = new Fields .MFString ())
   {
      return new Promise ((resolve, reject) =>
      {
         if (!(url instanceof Fields .MFString))
            throw new Error ("Browser.loadURL: url must be of type MFString.");

         if (!(parameter instanceof Fields .MFString))
            throw new Error ("Browser.loadURL: parameter must be of type MFString.");

         // FileLoader calls back at once when it fetches no candidate (there is none, or each is the empty URL), before
         // this load is the browser's `[_fileLoader]`: X3DBrowser takes it for an older one. Nothing is aborted and no
         // event fires; the empty URLs are reported first.
         if (Array .from (url) .every (u => String (u) === ""))
         {
            if (this .#engine)
               this .#reportOnly (url);
            else if (url .length)
               this .#ready .then (() => this .#engine && this .#reportOnly (url));

            return reject (new Error ("Loading of X3D file aborted."));
         }

         // `this [_fileLoader] ?.abort ()`: the older loadURL rejects with "Loading of X3D file aborted.".
         const loader = { abort: false, id: null };

         if (this .#fileLoader)
            this .#abortLoad (this .#fileLoader);

         this .#fileLoader = loader;

         this .#load (url, loader) .then (id =>
         {
            if (this .#fileLoader !== loader)
            {
               this .#engine ?.discard (id);
               return reject (new Error ("Loading of X3D file aborted."));
            }

            this .#fileLoader = null;
            this .#replaceWorld (id, true) .then (resolve, reject);
         },
         () =>
         {
            if (this .#fileLoader !== loader || this .#disposed)
               return reject (new Error ("Loading of X3D file aborted."));

            // The engine reported the candidates that failed (FileLoader.printError).
            this .#fileLoader = null;
            this .#call (X3DConstants .INITIALIZED_ERROR);
            this .#dispatch ("error");
            reject (new Error ("Couldn't load X3D file."));
         });
      });
   }

   // The messages of a loadURL whose candidates fail without a request.
   #reportOnly (urls)
   {
      const load = this .#engine .loadURL (Array .from (urls, String), this .baseURL);

      this .#engine .loadStep (load, Date .now () / 1000);
      this .#flushLog (load);
      this .#engine .discard (load);
   }

   // FileLoader.abort () of a loadURL: the candidate in flight is its last. The engine ends that request and reports
   // it when it fails, like FileLoader.loadDocumentError.
   #abortLoad (loader)
   {
      loader .abort = true;

      if (loader .id !== null)
         this .#engine ?.abortLoad (loader .id);
   }

   // FileLoader.createX3DFromURL for the X3DWorld: the engine tries the candidates in order, waits for each response
   // and reports each candidate that fails (WebBrowser.loadURL). Then the scene's own loads (Inline, textures,
   // EXTERNPROTO). An abort ends the request in flight; the scene loads that started go on, like FileLoader's.
   async #load (urls, loader)
   {
      await this .#ready;

      if (loader .abort || !this .#engine)
         throw new Error ("aborted");

      const load = loader .id = this .#engine .loadURL (Array .from (urls, String), this .baseURL);
      let   id;

      try
      {
         while (true)
         {
            if (!this .#engine)
               throw new Error ("aborted");

            // The messages of a candidate go out when its response comes, as X_ITE's do.
            id = this .#engine .loadStep (load, Date .now () / 1000);

            this .#flushLog (load);

            if (id !== 0)
               break;

            await this .#engine .progress (load);
         }
      }
      finally
      {
         loader .id = null;
         this .#follow (load);
      }

      if (id < 0)
         throw new Error ("no candidate loaded");

      // A newer loadURL that started meanwhile takes over (the caller rejects this one).
      await this .#loaded (id, () => this .#disposed || this .#fileLoader !== loader);
      return id;
   }

   // GLTF2Parser's buffer FileLoaders of a candidate that was rejected while they were in flight go on: each logs its
   // own failure when its response comes. Then the load goes.
   async #follow (load)
   {
      while (this .#engine ?.followLoad (load))
      {
         this .#flushLog (load);
         await this .#engine .progress (load);
      }

      this .#flushLog (load);
      this .#engine ?.discard (load);
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
   // or `undefined` replaces the world with an empty scene. When the engine is ready, the world's shutdown is
   // scheduled at once (X_ITE: `await nextFrame ()` in the call), before frames the page asks for after the call.
   replaceWorld (scene)
   {
      if (!this .#engine)
      {
         return this .#ready .then (() =>
         {
            if (!this .#engine)
               throw new Error ("Browser disposed.");

            return this .replaceWorld (scene);
         });
      }

      try
      {
         let id    = this .#sceneId (scene);
         let owned = false;

         if (id === null)
         {
            if (scene !== null && scene !== undefined)
               throw new Error ("X_ITE_R: Browser.replaceWorld: scene must be an X3DScene of this browser, null or undefined.");

            // An empty scene has nothing to load.
            id = this .#engine .prepareString ("#X3D V4.0 utf8\n", this .baseURL, Date .now () / 1000);
            this .#engine .loaded (id, Date .now () / 1000);
            owned = true;
         }

         if (id === this .#engine .current)
            throw new Error ("X_ITE_R: the scene is already the world.");

         // `this [_fileLoader] ?.abort ()`: a loadURL that is still fetching fails.
         if (this .#fileLoader)
            this .#abortLoad (this .#fileLoader);

         return this .#replaceWorld (id, owned, owned ? null : scene);
      }
      catch (error)
      {
         return Promise .reject (error);
      }
   }

   // X3DBrowser.replaceWorld of a loaded engine scene: the old world is still current while its shutdown runs, one
   // frame after the call; "initialized load" follows in the next frame (checkLoadCount). A newer replaceWorld
   // rejects this one with "Replacing world aborted.". `owned`: the adapter made the scene (loadURL, the empty
   // world), so a call that a newer one replaced releases it, unless the newer call uses the same scene. A scene of
   // the page (createX3DFromString) stays: its X3DScene releases it.
   #replaceWorld (id, owned, scene = null)
   {
      return new Promise ((resolve, reject) =>
      {
         this .#reject ?.(new Error ("Replacing world aborted."));
         this .#reject         = reject;
         this .#replacing      = id;
         this .#replacingScene = scene;

         const current   = () => this .#reject === reject && this .#engine;
         const nextFrame = () => new Promise (requestAnimationFrame);
         const release   = () =>
         {
            if (owned && this .#replacing !== id && this .#engine ?.current !== id)
               this .#engine ?.discard (id);
         };

         (async () =>
         {
            // After X3DBrowser.initialize there is always a world (the empty one first).
            {
               await nextFrame ();

               if (!current ())
                  return release ();

               this .#call (X3DConstants .SHUTDOWN_EVENT);
               this .#dispatch ("shutdown");

               // There could be a replaceWorld from a shutdown event handler.
               if (!current ())
                  return release ();
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

            this .#reject         = null;
            this .#replacing      = null;
            this .#replacingScene = null;
            this .#call (X3DConstants .INITIALIZED_EVENT);
            this .#dispatch ("initialized load");
            resolve ();
         })
         () .catch (error =>
         {
            if (this .#reject === reject)
            {
               this .#reject         = null;
               this .#replacing      = null;
               this .#replacingScene = null;
            }

            reject (error);
         });
      });
   }

   // --- viewpoints ----------------------------------------------------------------------------------------------

   // The layerNode argument of the viewpoint functions: X_ITE_R has the active layer only.
   #activeLayer (layerNode)
   {
      if (layerNode !== undefined && layerNode !== null && handleOf (layerNode))
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

   // X3DBrowser.activeViewpoint: the bound viewpoint of the active layer; null for its default viewpoint.
   get activeViewpoint ()
   {
      const id = this .#engine ?.activeViewpoint ();

      return id === undefined ? null : nodeObject (this .#scene [_node], id);
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

      // The engine's lines start with the console function X_ITE calls: "error: ", "warn: ", "info: " or "log: ".
      for (const line of this .#engine ?.takeLog (id) ?? [ ])
      {
         const [ , level, text ] = line .match (/^(error|warn|info|log): ([^]*)$/) ?? [ , "warn", line ];

         console [level] (text);
      }
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
         // The loadURL callback of a foreign URL: `window .open (url, target)`, or `location = url`. A script URL in
         // any case (X_ITE matches only lower case) never runs: the engine has no script engine, and the page must not
         // run code from a world. A new window gets no opener.
         let url;

         try
         {
            url = new URL (foreign [i + 1], this .baseURL);
         }
         catch
         {
            continue;
         }

         if ([ "javascript:", "vbscript:", "ecmascript:", "vrmlscript:", "data:" ] .includes (url .protocol))
         {
            console .warn (`X_ITE_R: Anchor URL '${foreign [i + 1]}' was not opened.`);
            continue;
         }

         if (foreign [i])
            window .open (url .href, foreign [i], "noopener");
         else
            location = url .href;
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
            processFieldEvents (this, engine .current);
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
               this .loadURL (new Fields .MFString (newValue)) .catch (error => console .error (error));

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

   get url ()      { return new Fields .MFString (... [ ... (this .getAttribute ("url") ?? "") .matchAll (/"(?:[^"\\]|\\.)*"/gs) ] .map (m => parseVrmlString (m [0]))); }
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
   X3DFieldDefinition,
   FieldDefinitionArray,
   X3DField,
   X3DArrayField,
   ... Fields,

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
