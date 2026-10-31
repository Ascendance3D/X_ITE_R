// X_ITE_R page adapter: defines <x3d-canvas> and the X_ITE-named JavaScript API on top of the Rust engine
// (crates/x3d-web, built into ./pkg by tools/build-web.sh). Everything X3D (parsing, the scene graph, events,
// rendering) runs in Rust; this file only fetches the world, runs the requestAnimationFrame loop and converts
// values. The names follow X_ITE 16.5.0 (oracle/x_ite-pin); web/API.md lists what exists and what does not.

import init, { WebBrowser, parseFieldWithoutScene, printEmptyScene, configTable, scriptOp } from "./pkg/x3d_web.js";
import { createScriptRealm } from "./pkg/script.mjs";
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

// The table of scene `scene` of `browser` in world generation `generation`: its X3DNodeRefs (`refs`) and the page's
// `rootNodes` field of the scene (`rootNodes`, made on its first read).
function sceneTable (browser, scene, generation)
{
   const tables = nodeRefTables .get (browser) ?? nodeRefTables .set (browser, new Map ()) .get (browser);

   let table = tables .get (scene);

   if (!table || table .generation !== generation)
      tables .set (scene, table = { generation, refs: new Map (), rootNodes: null });

   return table;
}

function sceneRefs (browser, scene, generation)
{
   return sceneTable (browser, scene, generation) .refs;
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
      refs .set (id, ref = new X3DNodeRef ({ browser: owner .browser, scene: owner .scene, generation: owner .generation, id, keep: owner .keep }));
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

// The node id of the events of a scene's rootNodes (session.rs ROOT_NODES_EVENT).
const ROOT_NODES_EVENT = 4294967295;

// The parent of a scene's rootNodes field (X_ITE: the X3DScene, an X3DBaseNode): its events reach the engine.
function RootNodesParent (handle)
{
   X3DChildObject .call (this);

   this [_node] = handle;
}

Object .setPrototypeOf (RootNodesParent .prototype, X3DChildObject .prototype);

for (const key of [ "addEvent", "addEventObject" ])
   Object .defineProperty (RootNodesParent .prototype, key, { value (field) { pushField (field); }, writable: true, configurable: true });

Object .defineProperty (RootNodesParent .prototype, "isInitialized", { value () { return true; }, writable: true, configurable: true });

// X3DExecutionContext.rootNodes of the world of scene handle `h`: one live field object per world, an inputOutput MFNode
// named `rootNodes` as X3DScene's. Its changes (push, splice, length, assignment) reach the engine as its node fields'
// do; a change of the engine's list reaches the object when it is read.
function rootNodesField (h)
{
   const engine = engineOf (h);
   const table  = sceneTable (h .browser, h .scene, h .generation);

   if (table .rootNodes)
   {
      refreshRootNodes (table);

      return table .rootNodes .entry .field;
   }

   const data = rootNodesData (engine, h);

   const field  = fieldFromData ("MFNode", data, h);
   const parent = new RootNodesParent (h);

   field .setName ("rootNodes");
   field .setAccessType (X3DConstants .inputOutput);
   field .addParent (parent);

   const attached = { ref: parent, name: "rootNodes", entry: { field, data }, root: true };

   attachedOf .set (field, attached);
   attachedOf .set (field .getTarget (), attached);

   table .rootNodes = attached;

   return field;
}

// The engine's root nodes of the world of scene handle `h`, as MFNode data.
const rootNodesData = (engine, h) => Array .from (engine .rootNodes (h .scene), n => n < 0 ? null : Number (n));

// The retained rootNodes field of a scene table takes the engine's list when it changed (a Script, an assignment of the
// page). A value the engine refused (a node of another world) stays in the page's object, as with a node field.
function refreshRootNodes (table)
{
   const { ref, entry } = table .rootNodes;
   const h    = ref [_node];
   const data = rootNodesData (engineOf (h), h);

   if (data .length !== entry .data .length || data .some ((id, i) => id !== entry .data [i]))
   {
      entry .data = data;
      setFieldData (entry .field, data, h);
   }
}

// X3DCast (X3DConstants .X3DNode, node, false) of addRootNode and removeRootNode: the node of an SFNode of this adapter,
// else null (SFNodeCache .get (null) then throws X_ITE's TypeError).
function rootNodeCast (node)
{
   const value = node instanceof Fields .SFNode ? node .getValue () : node;

   return value instanceof X3DNodeRef ? value : null;
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
      // The scene's rootNodes: `rootNodes .assign (value)` of the world.
      if (attached .root)
         engineOf (h) .setRootNodes (h .scene, data);
      else
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

// The engine's lines start with the console function X_ITE calls: "error: ", "warn: ", "info: " or "log: ".
function printLogLine (line)
{
   const [ , level, text ] = line .match (/^(error|warn|info|log): ([^]*)$/) ?? [ , "warn", line ];

   console [level] (text);
}

// The string of a VRML-encoded SFString.
const parseVrmlString = text => text .trim () .slice (1, -1) .replace (/\\(.)/gs, "$1");

// The engine's changes of the fields the page holds objects of, in scene `scene` of `browser`.
function syncScene (browser, scene)
{
   retireReleased (browser, scene);

   const table = nodeRefTables .get (browser) ?.get (scene);

   // The retained rootNodes field first: a scene can hold it with no node field object (issue #133). A table of a world
   // that was replaced is left alone.
   if (table ?.rootNodes && browser [_engine] .worldGeneration (scene) === table .generation)
      refreshRootNodes (table);

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
// the value its event delivered. A callback that throws ends the delivery as X_ITE's event loop ends: the error is
// uncaught (reported to the page, not to the console), the callbacks and events after it never run, and the engine
// processes no event again.
//
// The console lines of the frame (a Script's Browser.print, the engine's messages) go out in the same pass. Each event
// carries the number of lines made before its X3DField.processEvent began; they print before its callbacks, so the
// callbacks come before the lines that the event's routes make, as in X_ITE.
function processFieldEvents (browser, scene)
{
   retireReleased (browser, scene);

   const table = nodeRefTables .get (browser) ?.get (scene);

   // The events are taken even with no object to call: they would pile up in the engine. The lines follow at once, so
   // their numbers count from the same line.
   const events = browser [_engine] .takeFieldEvents (scene);
   const lines  = browser [_engine] .takeLog (scene);
   let   printed = 0;

   const printUpTo = count =>
   {
      for (; printed < Math .min (count, lines .length); ++ printed)
         printLogLine (lines [printed]);
   };

   try
   {
      deliverFieldEvents (browser, scene, table, events, printUpTo);
   }
   finally
   {
      printUpTo (lines .length);
   }
}

function deliverFieldEvents (browser, scene, table, events, printUpTo)
{
   if (!table ?.refs .size && !table ?.rootNodes)
      return;

   syncScene (browser, scene);

   for (let i = 0; i < events .length; i += 4)
   {
      printUpTo (events [i + 3]);

      // A callback may have released nodes: their ids are not the nodes of the events any more.
      retireReleased (browser, scene);

      // An event of the scene's rootNodes. X_ITE calls its callbacks with the live field (MFNode.valueOf), so they see
      // its value now, not that of the event.
      if (events [i] === ROOT_NODES_EVENT)
      {
         const field = table .rootNodes ?.entry .field;

         if (!field)
            continue;

         for (const callback of field .getFieldCallbacks () .values ())
         {
            try
            {
               callback (field .valueOf ());
            }
            catch (error)
            {
               browser [_engine] .haltEvents (scene);
               reportError (error);
               return;
            }
         }

         continue;
      }

      const ref   = table .refs .get (events [i]);
      const entry = ref ?.[FIELDS] .get (events [i + 1]);

      if (!entry)
         continue;

      const callbacks = entry .field .getFieldCallbacks ();

      if (!callbacks .size)
         continue;

      // The callbacks see the value of their event; the field holds the current one again after them.
      setFieldData (entry .field, events [i + 2], ref [_node]);

      try
      {
         // X_ITE walks the live Map it started with: a callback removed in place by an earlier one is skipped, and
         // addFieldCallback copies the Map, so a callback added meanwhile waits for the next event.
         for (const callback of callbacks .values ())
         {
            try
            {
               callback (entry .field .valueOf ());
            }
            catch (error)
            {
               browser [_engine] .haltEvents (scene);
               reportError (error);
               return;
            }
         }
      }
      finally
      {
         setFieldData (entry .field, entry .data, ref [_node]);
      }
   }
}

// X3DField.addFieldCallback of a node's field object: the engine reports its events from now on.
__bridge .fieldCallbackAdded = function (field)
{
   const attached = attachedOf .get (field);

   if (!attached)
      return;

   const h = attached .ref [_node];

   if (attached .root)
      engineOf (h) .observeRootNodes (h .scene, true);
   else
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

// --- routes ------------------------------------------------------------------------------------------------------
//
// X3DExecutionContext.routes, addRoute, deleteRoute and getRoute (Execution/X3DExecutionContext.js, Routing/X3DRoute.js,
// Routing/RouteArray.js). The engine owns the routes; an X3DRoute object of the page stands for one of them, and the
// scene gives the same object for the same route until the route is deleted.

const routeScene = new WeakMap (); // X3DRoute → the X3DScene it belongs to

const ROUTE_STREAM = "Source node does not exist in scene graph.";

class X3DRoute
{
   // The engine's record `[id, source, sourceField, destination, destinationField, givenSourceField, givenDestinationField]`.
   #data;
   #deleted = false;

   constructor (scene, data)
   {
      routeScene .set (this, scene);
      this .#data = data;
   }

   update (data) { this .#data = data; }
   retire ()     { this .#deleted = true; }

   get id ()       { return this .#data [0]; }
   get deleted ()  { return this .#deleted; }

   getTypeName ()   { return "X3DRoute"; }
   getId ()         { return this .#data [0]; }
   getExecutionContext () { return routeScene .get (this); }

   // A route that was deleted prints the field names it was given, like X3DRoute after disconnect ().
   getSourceField ()      { return this .#deleted ? this .#data [5] : this .#data [2]; }
   getDestinationField () { return this .#deleted ? this .#data [6] : this .#data [4]; }

   getSourceNode ()      { return nodeObject (routeScene .get (this) [_node], this .#data [1]); }
   getDestinationNode () { return nodeObject (routeScene .get (this) [_node], this .#data [3]); }

   dispose () { routeScene .get (this) .deleteRoute (this); }

   // A route alone is not in a scene graph that a generator knows: X_ITE's toStream refuses it.
   toVRMLString ()  { throw new Error (ROUTE_STREAM); }
   toXMLString ()   { throw new Error (ROUTE_STREAM); }
   toJSONString ()  { throw new Error (ROUTE_STREAM); }
}

for (const key of [ "update", "retire", "id", "deleted", "getTypeName", "getId", "getExecutionContext", "getSourceNode", "getSourceField",
                    "getDestinationNode", "getDestinationField", "dispose", "toVRMLString", "toXMLString", "toJSONString" ])
   Object .defineProperty (X3DRoute .prototype, key, { enumerable: false });

Object .defineProperties (X3DRoute .prototype,
{
   sourceNode:       { get: X3DRoute .prototype .getSourceNode,                                              enumerable: true },
   sourceField:      { get: X3DRoute .prototype .getSourceField,                                             enumerable: true },
   destinationNode:  { get: X3DRoute .prototype .getDestinationNode,                                         enumerable: true },
   destinationField: { get: X3DRoute .prototype .getDestinationField,                                        enumerable: true },
});

Object .defineProperty (X3DRoute, "typeName", { value: "X3DRoute", enumerable: true });

// X3DInfoArray of routes: read-only, live; the routes by index, and by the id of the route (has, get).
const routeListOf = new WeakMap (); // RouteArray → function giving the current X3DRoute objects

const routeArrayHandler =
{
   get (target, key)
   {
      if (typeof key === "string")
      {
         const index = +key;

         if (Number .isInteger (index) && index >= 0)
            return routeListOf .get (target) () [index];
      }

      return Reflect .get (target, key);
   },
   set (target, key, value)
   {
      if (typeof key === "string")
      {
         const index = +key;

         if (Number .isInteger (index) && index >= 0)
            return false;
      }

      return Reflect .set (target, key, value);
   },
   has (target, key)
   {
      if (typeof key === "string")
      {
         const index = +key;

         if (Number .isInteger (index) && index >= 0)
            return index < routeListOf .get (target) () .length;
      }

      return Reflect .has (target, key);
   },
   ownKeys (target)
   {
      return Object .keys (routeListOf .get (target) ()) .concat (Reflect .ownKeys (target));
   },
   getOwnPropertyDescriptor (target, key)
   {
      if (typeof key === "string")
      {
         const index = +key;

         if (Number .isInteger (index) && index >= 0)
         {
            const list = routeListOf .get (target) ();

            if (index < list .length)
               return { value: list [index], writable: false, enumerable: true, configurable: true };
         }
      }

      return Reflect .getOwnPropertyDescriptor (target, key);
   },
};

class RouteArray
{
   constructor (list)
   {
      const proxy = new Proxy (this, routeArrayHandler);

      routeListOf .set (this, list);
      routeListOf .set (proxy, list);

      return proxy;
   }

   get length ()          { return routeListOf .get (this) () .length; }
   getTypeName ()         { return "RouteArray"; }
   has (id)               { return routeListOf .get (this) () .some (route => route .id === id); }
   get (id)               { return routeListOf .get (this) () .find (route => route .id === id); }
   *[Symbol .iterator] () { yield* routeListOf .get (this) (); }
}

for (const key of [ "getTypeName", "has", "get" ])
   Object .defineProperty (RouteArray .prototype, key, { enumerable: false });

Object .defineProperty (RouteArray, "typeName", { value: "RouteArray", enumerable: true });

// --- Configuration: ComponentInfo, ProfileInfo and their arrays ----------------------------------------------------------
//
// Configuration/ComponentInfo.js, ProfileInfo.js, ComponentInfoArray.js, ProfileInfoArray.js and Base/X3DInfoArray.js of
// X_ITE. The supported components and profiles (SupportedComponents.js, SupportedProfiles.js) are the engine's table of the
// pinned X_ITE (configTable), with X_ITE's aliases.

const PROVIDER_URL = "https://create3000.github.io/x_ite/";

// URLs.getProviderURL (component): the project's URL for a built-in component; an external one is a file next to the script.
function providerURLOf (component)
{
   if (!component)
      return PROVIDER_URL;

   return new URL (`assets/components/${component}Component.js`, import .meta .url) .href;
}

class ComponentInfo
{
   constructor (name, level, title, providerURL, external = false, dependencies = [ ])
   {
      Object .defineProperties (this,
      {
         name: { value: name, enumerable: true },
         level: { value: level, enumerable: true },
         title: { value: title, enumerable: true },
         providerURL: { value: providerURL || providerURLOf (external && name), enumerable: true },
         external: { value: external },
         dependencies: { value: dependencies },
      });
   }

   getTypeName () { return "ComponentInfo"; }

   get providerUrl () { return this .providerURL; } // legacy

   get [Symbol .toStringTag] () { return this .getTypeName (); }

   toVRMLString () { return `COMPONENT ${this .name} : ${this .level}`; }
}

class ProfileInfo
{
   constructor (name, title, providerURL, components)
   {
      Object .defineProperties (this,
      {
         name: { value: name, enumerable: true },
         title: { value: title, enumerable: true },
         providerURL: { value: providerURL || providerURLOf (), enumerable: true },
         components: { value: components, enumerable: true },
      });
   }

   getTypeName () { return "ProfileInfo"; }

   get providerUrl () { return this .providerURL; } // legacy

   get [Symbol .toStringTag] () { return this .getTypeName (); }

   toVRMLString () { return `PROFILE ${this .name}`; }
}

// X3DInfoArray: a read-only, array-like list of values, with a name index (`get`, `has`; aliases are names too).
const _infoArray = Symbol ("X_ITE_R.infoArray");
const _infoIndex = Symbol ("X_ITE_R.infoIndex");
const _infoType  = Symbol ("X_ITE_R.infoType");

const isIndex = key => typeof key === "string" && Number .isInteger (+key) && +key >= 0;

const infoArrayHandler =
{
   get (target, key)             { return isIndex (key) ? target [_infoArray] [+key] : Reflect .get (target, key); },
   set (target, key, value)      { return isIndex (key) ? false : Reflect .set (target, key, value); },
   has (target, key)             { return isIndex (key) ? +key < target [_infoArray] .length : Reflect .has (target, key); },
   ownKeys (target)              { return Object .keys (target [_infoArray]) .concat (Reflect .ownKeys (target)); },
   getOwnPropertyDescriptor (target, key)
   {
      if (isIndex (key))
      {
         const descriptor = +key < target [_infoArray] .length ? Reflect .getOwnPropertyDescriptor (target [_infoArray], key) : undefined;

         if (descriptor)
            descriptor .writable = false;

         return descriptor;
      }

      return Reflect .getOwnPropertyDescriptor (target, key);
   },
};

class X3DInfoArray
{
   constructor (values, valueType)
   {
      Object .defineProperty (this, _infoArray, { value: [ ], writable: true });
      Object .defineProperty (this, _infoIndex, { value: new Map (), writable: true });
      Object .defineProperty (this, _infoType, { value: valueType });

      for (const value of values)
         this .add (value .name, value);

      return new Proxy (this, infoArrayHandler);
   }

   get length () { return this [_infoArray] .length; }

   *[Symbol .iterator] () { yield* this [_infoArray]; }

   copy ()
   {
      return new (this .constructor) (this [_infoArray]);
   }

   equals (array)
   {
      const a = this [_infoArray], b = array [_infoArray];

      return a === b || (a .length === b .length && a .every ((value, i) => value === b [i]));
   }

   has (key) { return this [_infoIndex] .has (key); }
   get (key) { return this [_infoIndex] .get (key); }

   add (key, value)
   {
      if (this [_infoIndex] .has (key))
         throw new Error (`Couldn't add value to ${this .getTypeName ()}, key '${key}' already exists.`);

      if (!(value instanceof this [_infoType]))
         throw new Error (`Couldn't add value to ${this .getTypeName ()}, value for key '${key}' has wrong type.`);

      this [_infoArray] .push (value);
      this [_infoIndex] .set (key, value);
   }

   alias (alias, value) { this [_infoIndex] .set (alias, value); }

   // X3DInfoArray.update: the value of `oldKey` is replaced in place, or the value is added at the end.
   update (oldKey, newKey, value)
   {
      if (!(value instanceof this [_infoType]))
         throw new Error (`Couldn't update value of ${this .getTypeName ()}, value for key '${newKey}' has wrong type.`);

      const oldValue = this [_infoIndex] .get (oldKey);

      if (oldKey !== newKey)
         this .remove (newKey);

      this [_infoIndex] .delete (oldKey);
      this [_infoIndex] .set (newKey, value);

      const index = oldValue === undefined ? -1 : this [_infoArray] .indexOf (oldValue);

      if (index > -1)
         this [_infoArray] [index] = value;
      else
         this [_infoArray] .push (value);
   }

   remove (key)
   {
      const value = this [_infoIndex] .get (key);

      if (value === undefined)
         return;

      const index = this [_infoArray] .indexOf (value);

      this [_infoIndex] .delete (key);

      if (index > -1)
         this [_infoArray] .splice (index, 1);
   }

   filter (callbackFn, thisArg) { return new (this .constructor) (this [_infoArray] .filter (callbackFn, thisArg)); }
   map (callbackFn, thisArg)    { return new (this .constructor) (this [_infoArray] .map (callbackFn, thisArg)); }
   slice (start, end)           { return new (this .constructor) (this [_infoArray] .slice (start, end)); }

   get [Symbol .toStringTag] () { return this .getTypeName (); }
}

// The non-destructive Array functions of X3DInfoArray.
for (const name of [ "at", "entries", "every", "find", "findIndex", "findLast", "findLastIndex", "flat", "flatMap", "forEach", "includes", "indexOf", "join", "keys", "lastIndexOf", "reduce", "reduceRight", "some", "values" ])
   X3DInfoArray .prototype [name] = function (... args) { return Array .prototype [name] .apply (this [_infoArray], args); };

// The values of a new array are new objects made from the descriptions (`add (name, { ... })`), as in X_ITE.
class ComponentInfoArray extends X3DInfoArray
{
   constructor (values = [ ]) { return super (values, ComponentInfo); }

   add (name, { level, title, providerURL, external = false, dependencies = [ ] })
   {
      super .add (name, new ComponentInfo (name, level, title, providerURL, external, dependencies));
   }

   getTypeName () { return "ComponentInfoArray"; }
}

class ProfileInfoArray extends X3DInfoArray
{
   constructor (values = [ ]) { return super (values, ProfileInfo); }

   add (name, { title, providerURL, components })
   {
      super .add (name, new ProfileInfo (name, title, providerURL, new ComponentInfoArray (components)));
   }

   getTypeName () { return "ProfileInfoArray"; }
}

for (const type of [ ComponentInfo, ProfileInfo, X3DInfoArray, ComponentInfoArray, ProfileInfoArray ])
{
   for (const key of Object .getOwnPropertyNames (type .prototype))
   {
      if (key !== "constructor")
         Object .defineProperty (type .prototype, key, { enumerable: false });
   }
}

for (const type of [ ComponentInfo, ProfileInfo, ComponentInfoArray, ProfileInfoArray ])
   Object .defineProperty (type, "typeName", { value: type .name, enumerable: true });

// The browser's SupportedComponents and SupportedProfiles, made from the engine's table on first use (the engine must be loaded).
let supportedConfig = null;

function supported ()
{
   if (supportedConfig)
      return supportedConfig;

   if (!wasmReady)
      throw new Error ("X_ITE_R: the engine is not loaded yet; wait for browser .ready ().");

   const [ componentTable, profileTable ] = configTable ();
   const components = new ComponentInfoArray ();

   for (const [ name, title, level, external, dependencies ] of componentTable)
      components .add (name, { title, level, external, dependencies: Array .from (dependencies) });

   components .alias ("H-Anim", components .get ("HAnim"));
   components .alias ("TextureProjector", components .get ("TextureProjection"));
   components .alias ("ProjectiveTextureMapping", components .get ("TextureProjection"));

   const profiles = new ProfileInfoArray ();

   // SupportedProfiles.js getComponent (name, level): a ComponentInfo of the component at that level.
   const component = (name, level) =>
   {
      const c = components .get (name);

      return new ComponentInfo (c .name, level, c .title, c .providerURL, c .external, c .dependencies);
   };

   for (const [ name, title, list ] of profileTable)
   {
      const profileComponents = [ ];

      for (let i = 0; i < list .length; i += 2)
         profileComponents .push (component (list [i], list [i + 1]));

      profiles .add (name, { title, components: profileComponents });
   }

   profiles .alias ("MPEG-4", profiles .get ("MPEG4Interactive"));

   return supportedConfig = { components, profiles };
}

// X3DBrowser.getComponent (name, level): a new ComponentInfo, its level clamped to 1 ... the supported level.
function getComponent (name, level)
{
   name   = String (name);
   level |= 0;

   const component = supported () .components .get (name);

   if (component)
   {
      return new ComponentInfo (component .name,
         Math .min (Math .max (level || component .level, 1), component .level),
         component .title,
         component .providerURL,
         component .external,
         component .dependencies);
   }

   throw new Error (`Component '${name}' at level '${level}' is not supported.`);
}

// The profile and components X3DBrowser.createScene gave a scene (X3DScene.setProfile, updateComponent), by X3DScene.
const givenSceneInfo = new WeakMap ();

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

   // X3DScene.profile and components: those createScene gave, or those of the engine's scene (its PROFILE and COMPONENT
   // statements). The profile is the browser's ProfileInfo of that name, the components a ComponentInfoArray made once.
   #components = null;

   get profile ()
   {
      const given = givenSceneInfo .get (this);

      if (given)
         return given .profile;

      const name = this .#engine () ?.profileAndComponents (this .#handle .scene) [0] ?? null;

      return name === null ? null : supported () .profiles .get (name) ?? null;
   }

   get components ()
   {
      const given = givenSceneInfo .get (this);

      if (given)
         return given .components;

      if (this .#components)
         return this .#components;

      const list       = this .#engine () ?.profileAndComponents (this .#handle .scene) [1] ?? [ ];
      const components = new ComponentInfoArray ();

      // X3DParser: `scene .updateComponent (browser .getComponent (name, level))`.
      for (let i = 0; i < list .length; i += 2)
         components .update (list [i], list [i], getComponent (list [i], list [i + 1]));

      return this .#components = components;
   }

   getProfile ()    { return this .profile; }
   getComponents () { return this .components; }

   // X3DScene.rootNodes (X3DExecutionContext.js, X3DScene.js): a live inputOutput MFNode, the same object on every
   // read. The empty world of a canvas before any load has no engine scene: its field holds what the page gives it.
   #emptyRootNodes = null;

   get rootNodes ()
   {
      if (this .#handle .scene === null)
         return this .#emptyRootNodes ??= new Fields .MFNode ();

      return rootNodesField (this .#handle);
   }

   set rootNodes (value) { this .setRootNodes (value); }

   getRootNodes () { return this .rootNodes; }

   // X3DScene.setRootNodes: `this .getRootNodes () .assign (value)`.
   setRootNodes (value)
   {
      this .rootNodes .assign (value);
   }

   // X3DScene.addRootNode: a node that is not a root node yet is pushed.
   addRootNode (node)
   {
      node = SFNodeCache .get (rootNodeCast (node));

      const rootNodes = this .rootNodes;

      if (rootNodes .includes (node))
         return;

      rootNodes .push (node);
   }

   // X3DScene.removeRootNode: the root nodes without the node, assigned (an event also when nothing was removed).
   removeRootNode (node)
   {
      node = SFNodeCache .get (rootNodeCast (node));

      const rootNodes = this .rootNodes;

      rootNodes .assign (rootNodes .filter (rootNode => rootNode !== node));
   }

   // --- routes (see "routes" above) ---

   #routes = new Map (); // id → X3DRoute
   #routeArray = null;

   // The X3DRoute objects of the engine's routes now: one object per route, the deleted ones retired.
   #routeList ()
   {
      const engine = this .#engine ();
      const list   = [ ];
      const seen   = new Set ();

      for (const data of engine ?.routes (this .#handle .scene) ?? [ ])
         list .push (this .#routeOf (data)), seen .add (data [0]);

      for (const [ id, route ] of this .#routes)
      {
         if (!seen .has (id))
         {
            route .retire ();
            this .#routes .delete (id);
         }
      }

      return list;
   }

   // The X3DRoute object for the engine's record of a route.
   #routeOf (data)
   {
      const id    = Number (data [0]);
      const route = this .#routes .get (id) ?? new X3DRoute (this, data);

      route .update (data);
      this .#routes .set (id, route);
      return route;
   }

   // X3DCast (X3DNode, node, false) of the arguments of a route function, and their ids in this scene: the message names the
   // argument (`source node` of addRoute, `sourceNode` of getRoute).
   #routeNodes (sourceNode, destinationNode, source, destination)
   {
      const nodes = [ [ sourceNode, source ], [ destinationNode, destination ] ];

      for (const [ node, name ] of nodes)
      {
         if (!handleOf (node))
            throw new Error (`Bad ROUTE specification: ${name} must be of type X3DNode.`);
      }

      return nodes .map (([ node ]) => nodeIdOf (node, this .#handle));
   }

   get routes ()
   {
      return this .#routeArray ??= new RouteArray (() => this .#routeList ());
   }

   getRoutes () { return this .routes; }

   addRoute (sourceNode, sourceField, destinationNode, destinationField)
   {
      sourceField      = String (sourceField);
      destinationField = String (destinationField);

      const [ source, destination ] = this .#routeNodes (sourceNode, destinationNode, "source node", "destination node");

      this .#routeList ();

      try
      {
         return this .#routeOf (this .#engine () .addRoute (this .#handle .scene, source, sourceField, destination, destinationField));
      }
      catch (error)
      {
         throw engineError (error);
      }
   }

   getRoute (sourceNode, sourceField, destinationNode, destinationField)
   {
      sourceField      = String (sourceField);
      destinationField = String (destinationField);

      const [ source, destination ] = this .#routeNodes (sourceNode, destinationNode, "sourceNode", "destinationNode");

      this .#routeList ();

      try
      {
         const data = this .#engine () .getRoute (this .#handle .scene, source, sourceField, destination, destinationField);

         return data ? this .#routeOf (data) : undefined;
      }
      catch (error)
      {
         throw engineError (error);
      }
   }

   deleteRoute (route)
   {
      // Case: sourceNode, sourceField, destinationNode, destinationField.
      if (arguments .length === 4)
         route = this .getRoute (... arguments);

      if (!(route instanceof X3DRoute) || routeScene .get (route) !== this || route .deleted)
         return;

      this .#routeList ();

      if (this .#engine () ?.deleteRoute (this .#handle .scene, route .id))
      {
         route .retire ();
         this .#routes .delete (route .id);
      }
   }

   // X3DObject.toVRMLString, toXMLString, toJSONString of the scene (options are not supported, see API.md).
   // The empty world of a canvas before any load has no engine scene: its text is that of a scene no source set up.
   #print (kind) { return this .#handle .scene === null ? printEmptyScene (kind) : this .#engine () .printScene (this .#handle .scene, kind); }
   toVRMLString () { return this .#print ("vrml"); }
   toXMLString ()  { return this .#print ("xml"); }
   toJSONString () { return this .#print ("json"); }

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
   #live = true;
   #fileLoader = null;

   // The createVrmlFromURL requests in the world's runtime: request id → { field, handle of the node }.
   #sceneRequests = new Map ();
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
   // X3DSoundContext: the page's AudioContext, and whether its resume () has not succeeded yet (X_ITE keeps it in
   // `_audioElements` until then).
   #audioContext = null;
   #audioWaiting = false;

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
         // The Scripts of a world run only when the page lets them (the `scripts` attribute, read when the world starts):
         // they run with the privileges of the page, in its JavaScript engine (API.md, "Script nodes").
         this .#engine .setScriptRealm (host => this .#element .getAttribute ("scripts") === "true"
            ? createScriptRealm (host, scriptOp)
            : null);
         // An endUpdate () before the engine was ready.
         this .#engine .setLive (this .#live);
         this .#listen ();
         this .#start ();

         // X3DBrowser.initialize: replaceWorld with an empty scene, which ends in "initialized load". A loadURL
         // then always replaces a world, so it starts with "shutdown".
         // A browser that is stopped from the start (update="false") never gets its frame: X_ITE fires nothing then, also
         // not when it starts later.
         if (this .#live)
         {
            this .#call (X3DConstants .INITIALIZED_EVENT);
            this .#dispatch ("initialized load");
         }

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

   setBrowserOption (name, value)
   {
      const field = X3DBrowser .#field (this .#options, name, "BrowserOptions", true);

      field .setValue (value);

      // BrowserOptions.addInterest: the options with an effect that is not read when it is needed. ContentScale and
      // MaximumFrameRate are read by the frame loop; Mute and SoundIntensity go to the engine (the gain of its sound
      // destinations); the rest is stored (API.md).
      if (field === this .#options .get ("AutoUpdate"))
         this .#setAutoUpdate ();

      if (field === this .#options .get ("Mute") || field === this .#options .get ("SoundIntensity"))
         this .#engine ?.setSoundOptions (this .#options .get ("Mute") .getValue (), this .#options .get ("SoundIntensity") .getValue ());
   }

   // --- update ------------------------------------------------------------------------------------------------

   // X3DBrowser.beginUpdate and endUpdate, as the `update` attribute and the AutoUpdate option call them: the live state
   // of setLive (see isLive), and endUpdate also lets the input go (X_ITE: getViewer () .disconnect ()).
   #wasLive;
   #isIntersecting;
   #checkUpdateListener = null;
   #intersectionObserver = null;
   #lastStamp = -Infinity;
   #processed = 0;

   #beginUpdate () { this .setLive (true); }

   #endUpdate ()
   {
      this .#releaseInput ();
      this .setLive (false);
   }

   // BrowserOptions.set_AutoUpdate__.
   #setAutoUpdate ()
   {
      if (this .#checkUpdateListener)
         document .removeEventListener ("visibilitychange", this .#checkUpdateListener);

      this .#checkUpdateListener = null;
      this .#intersectionObserver ?.disconnect ();
      this .#intersectionObserver = null;

      if (!this .#options .get ("AutoUpdate") .getValue () || this .#disposed)
      {
         this .#wasLive = undefined;
         return;
      }

      this .#wasLive ??= this .#live;

      this .#checkUpdateListener = () => this .#checkUpdate ();

      document .addEventListener ("visibilitychange", this .#checkUpdateListener);

      this .#intersectionObserver = new IntersectionObserver (entries =>
      {
         this .#isIntersecting = entries .some (entry => entry .isIntersecting);

         this .#checkUpdate ();
      });

      this .#intersectionObserver .observe (this .#element);
   }

   // BrowserOptions.checkUpdate: a hidden or scrolled-out canvas stops, and starts again if it was live.
   #checkUpdate ()
   {
      if (!this .#options .get ("AutoUpdate") .getValue ())
         return;

      if (this .#isIntersecting === undefined)
         return;

      if (!document .hidden && this .#isIntersecting)
      {
         if (this .#wasLive)
            this .#beginUpdate ();

         this .#wasLive = undefined;
      }
      else
      {
         // A second time here, the browser is not live any more: keep what the first time found.
         this .#wasLive ??= this .#live;

         this .#endUpdate ();
      }
   }

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

   // X3DBrowser.addRoute / deleteRoute: those of the current scene, without the route (X3DBrowser.js:880).
   addRoute (sourceNode, sourceField, destinationNode, destinationField)
   {
      this .#scene .addRoute (sourceNode, sourceField, destinationNode, destinationField);
   }

   deleteRoute (sourceNode, sourceField, destinationNode, destinationField)
   {
      this .#scene .deleteRoute (sourceNode, sourceField, destinationNode, destinationField);
   }

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
      await this .#loaded (id, () => this .#disposed || (!loader .detached && this .#fileLoader !== loader));
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

   // X3DBrowser.createScene ([profile, component, ...]): a Promise of a new, empty scene that is not the world, with the
   // ProfileInfo and the ComponentInfo objects as given (any argument takes the profile branch).
   async createScene (profile, ... components)
   {
      // `new X3DScene (this)`: no profile and no components until they are given.
      const given = { profile: null, components: new ComponentInfoArray () };

      if (arguments .length)
      {
         if (!(profile instanceof ProfileInfo))
            throw new Error ("Couldn't create scene: profile must be of type ProfileInfo.");

         given .profile = profile;

         for (const component of components)
         {
            if (!(component instanceof ComponentInfo))
               throw new Error ("Couldn't create scene: component must be of type ComponentInfo.");

            // X3DScene.updateComponent.
            given .components .update (component .name, component .name, component);
         }
      }

      const scene = await this .#emptyScene ();

      givenSceneInfo .set (scene, given);
      return scene;
   }

   // X3DBrowser.getProfile (name): the supported ProfileInfo (an alias is a name too).
   getProfile (name)
   {
      const profile = supported () .profiles .get (String (name));

      if (profile)
         return profile;

      throw new Error (`Profile '${name}' is not supported.`);
   }

   getSupportedProfile (name)
   {
      const profile = supported () .profiles .get (String (name));

      if (profile)
         return profile;

      throw new Error (`Supported profile '${name}' does not exists.`);
   }

   getSupportedProfiles () { return supported () .profiles; }

   get supportedProfiles () { return this .getSupportedProfiles (); }

   // X3DBrowser.getComponent (name, level): a new ComponentInfo, at the supported level when `level` is 0 or none.
   getComponent (name, level) { return getComponent (name, level); }

   getSupportedComponent (name)
   {
      const component = supported () .components .get (String (name));

      if (component)
         return component;

      throw new Error (`Supported component '${name}' does not exists.`);
   }

   getSupportedComponents () { return supported () .components; }

   get supportedComponents () { return this .getSupportedComponents (); }

   // The empty scene of createScene and the imports that find no content: its world URL is the page's (`new X3DScene
   // (this)`: `location`).
   async #emptyScene ()
   {
      await this .#ready;

      if (!this .#engine)
         throw new Error ("Browser disposed.");

      const now = Date .now () / 1000;
      const id  = this .#engine .prepareString ("#X3D V4.0 utf8\n", String (location), now);

      this .#engine .loaded (id, now);
      return this .#sceneOf (id);
   }

   // X3DBrowser.createVrmlFromString (vrmlSyntax): the root nodes (an MFNode) of the VRML text, made at once. The nodes
   // are the current world's own, so the page can put them into a field of the world (X_ITE's belong to a scene of their
   // own, which X_ITE_R's fields refuse, see API.md). The browser must be ready: a synchronous call cannot wait for it.
   createVrmlFromString (vrmlSyntax)
   {
      const text = `${vrmlSyntax}`;

      if (!this .#engine)
         throw new Error ("X_ITE_R: the browser is not ready; await browser .ready () first.");

      const engine = this .#engine;
      const now    = Date .now () / 1000;

      // Before the first world (the page's empty one) there is no scene to make nodes in: they belong to a scene of their
      // own, which their handles keep alive.
      const own = engine .current === undefined;
      const id  = own ? engine .prepareString ("#X3D V4.0 utf8\n", this .baseURL, now) : engine .current;
      let   ids;

      try
      {
         if (own)
            engine .loaded (id, now);

         ids = engine .createVrmlFromString (id, text, this .baseURL);
      }
      catch (error)
      {
         this .#flushLog (id);

         if (own)
            engine .discard (id);

         throw engineError (error);
      }
      finally
      {
         this .#flushLog (id);
      }

      const scene = this .#sceneOf (id);
      const owner = scene [_node];

      if (own)
         owner .keep = scene;

      return new Fields .MFNode (... Array .from (ids, n => nodeObject (owner, Number (n))));
   }

   // X3DBrowser.createX3DFromURL (url): a Promise of the scene of the first url that loads. The world stays. The
   // callback form createX3DFromURL (url, node, event) is createVrmlFromURL.
   createX3DFromURL (url, node, event)
   {
      if (arguments .length === 3)
         return this .createVrmlFromURL (url, node, event);

      if (!(url instanceof Fields .MFString))
         throw new Error ("Browser.createX3DFromURL: url must be of type MFString.");

      return this .#loadScene (url);
   }

   // X3DBrowser.createVrmlFromURL (url, node, event): the root nodes of the scene go to the input MFNode field `event`
   // of `node` after the scene loaded; a url that does not load changes nothing.
   createVrmlFromURL (url, node, event)
   {
      const value = node instanceof Fields .SFNode ? X3DField .prototype .getValue .call (node) : null;
      const ref   = value instanceof X3DNodeRef ? value : null;

      event = String (event);

      if (!(url instanceof Fields .MFString))
         throw new Error ("Browser.createVrmlFromURL: url must be of type MFString.");

      if (!ref)
         throw new Error ("Browser.createVrmlFromURL: node must be of type X3DNode.");

      const field = ref .getField (event);

      if (!field .isInput ())
         throw new Error (`Browser.createVrmlFromURL: event named '${event}' must be a input field.`);

      if (field .getType () !== X3DConstants .MFNode)
         throw new Error (`Browser.createVrmlFromURL: event named '${event}' must be of type MFNode.`);

      const handle = ref [_node];
      const engine = this .#engine;

      // A node of the world: the scene loads in the world's own runtime (FileLoader for the X3DWorld), so its root nodes
      // are the world's and go into the field once the scene is loaded and live (#sceneRequestsDone, after a frame).
      if (engine && handle .scene === engine .current && engine .worldGeneration (handle .scene) === handle .generation)
      {
         const request = engine .createX3DFromURL (handle .scene, Array .from (url, String), this .baseURL);

         this .#sceneRequests .set (request, { field, handle });
         return;
      }

      // A node of a scene that is not the world (it has no frames to load in): the scene of the url is a scene of its
      // own, whose nodes the field refuses (API.md).
      this .#loadScene (url) .then (scene =>
      {
         try
         {
            field .setValue (scene .rootNodes);
         }
         catch (error)
         {
            console .error (error .message);
         }
      },
      () => { });
   }

   // The createVrmlFromURL requests of the world that settled: `field .setValue (scene .rootNodes)` for each scene that
   // loaded (X3DBrowser.js: after `scene .setLive (true)`); a scene that did not load changes nothing. The requests of a
   // world that is gone are dropped.
   #sceneRequestsDone ()
   {
      const engine = this .#engine;

      if (!engine || !this .#sceneRequests .size)
         return;

      const world   = engine .current;
      const settled = world === undefined ? [ ] : engine .takeSceneRequests (world);

      for (let i = 0; i < settled .length; i += 2)
      {
         const request = this .#sceneRequests .get (settled [i]);
         const nodes   = settled [i + 1];

         this .#sceneRequests .delete (settled [i]);

         // A scene that did not load changes nothing; the field of a world that an Anchor replaced inside this scene
         // (another world generation) is not touched.
         if (!request || typeof nodes === "string" || request .handle .scene !== world || engine .worldGeneration (world) !== request .handle .generation)
            continue;

         try
         {
            request .field .setValue (new Fields .MFNode (... Array .from (nodes, n => nodeObject (request .handle, Number (n)))));
         }
         catch (error)
         {
            console .error (error .message);
         }
      }

      for (const [ id, { handle } ] of this .#sceneRequests)
      {
         if (handle .scene !== world || engine .worldGeneration (handle .scene) !== handle .generation)
            this .#sceneRequests .delete (id);
      }
   }

   // FileLoader.createX3DFromURL (url, null, callback) for the X3DWorld, without a replaceWorld: the scene of the first
   // candidate that loads, or the Error "Couldn't load X3D file.". The messages of the candidates that fail are the
   // engine's. Unlike loadURL it is not the browser's one load: another call or a replaceWorld does not abort it.
   async #loadScene (urls)
   {
      let id;

      try
      {
         id = await this .#load (urls, { abort: false, id: null, detached: true });
      }
      catch
      {
         throw new Error ("Couldn't load X3D file.");
      }

      return this .#sceneOf (id);
   }

   // X3DBrowser.importJS (json): a Promise of the scene of X3D JSON (a string or an object). A value that is no X3D JSON
   // gives an empty scene, like X_ITE's.
   async importJS (json)
   {
      let input = json;

      if (typeof json === "string")
      {
         try
         {
            input = JSON .parse (json);
         }
         catch
         {
            input = undefined;
         }
      }

      const valid = input instanceof Object && Object .keys (input) .length === 1 && input .X3D instanceof Object;

      return valid ? this .#importText (JSON .stringify (input)) : this .#emptyScene ();
   }

   // X3DBrowser.importDocument (dom): a Promise of the scene of an X3D XML string, XMLDocument or XML element. An HTML
   // element (X_ITE's HTMLParser) is not supported; XML that does not parse gives an empty scene, like X_ITE's.
   async importDocument (dom)
   {
      if (dom === undefined)
         throw new TypeError ("Cannot read properties of undefined (reading 'nodeName')");

      let text = null;

      if (typeof dom === "string")
         text = dom;
      else if (dom instanceof HTMLElement)
         throw new Error ("X_ITE_R: Browser.importDocument: an HTML element is not supported; pass the XML string, an XMLDocument or an XML element.");
      else if (dom instanceof Node)
         text = new XMLSerializer () .serializeToString (dom);

      if (text !== null)
      {
         const xml = new DOMParser () .parseFromString (text, "application/xml");

         if (xml .getElementsByTagName ("parsererror") .length || xml .documentElement ?.nodeName !== "X3D")
            text = null;
      }

      return text === null ? this .#emptyScene () : this .#importText (text);
   }

   // The scene of text that createX3DFromString parses, with the page's URL as its world URL.
   async #importText (text)
   {
      await this .#ready;

      if (!this .#engine)
         throw new Error ("Browser disposed.");

      let id;

      try
      {
         id = this .#engine .prepareString (text, String (location), Date .now () / 1000);
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
            id = this .#engine .prepareEmpty (this .baseURL, Date .now () / 1000);
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
            // The clock the engine keeps is the page's latest: with the canvas detached no frame has given a newer one.
            this .#audioClock (this .#engine);
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

   // X3DCast (type, node) in the current world: the node id when `node` is an SFNode of this world whose node types include
   // `type`, else undefined. X_ITE casts a node of any scene; a node of another world or a replaced one is no layer here.
   #cast (type, node)
   {
      const handle = handleOf (node);
      const owner  = this .#scene ?.[_node];

      if (!handle || !owner || handle .browser !== owner .browser || handle .scene !== owner .scene || handle .generation !== owner .generation || handle .released)
         return undefined;

      const typeName = engineOf (handle) .typeName (handle .scene, handle .id);

      return (NODE_TYPES [typeName] ?? [ ]) .includes (type) ? handle .id : undefined;
   }

   // A layer request: `layer:<node>:<request>` runs it on that layer; none, or a value that is no X3DLayerNode: the
   // active layer, like `X3DCast (X3DConstants .X3DLayerNode, layerNode) ?? this .getActiveLayer ()`.
   #layerRequest (layerNode, request)
   {
      const id = this .#cast ("X3DLayerNode", layerNode);

      this .#engine ?.viewpointRequest (id === undefined ? request : `layer:${id}:${request}`);
   }

   // X3DBrowser.firstViewpoint, previousViewpoint, nextViewpoint, lastViewpoint ([layerNode]): the viewpoint
   // binds with the next frame's events.
   firstViewpoint (layerNode)    { this .#layerRequest (layerNode, "firstViewpoint"); }
   previousViewpoint (layerNode) { this .#layerRequest (layerNode, "previousViewpoint"); }
   nextViewpoint (layerNode)     { this .#layerRequest (layerNode, "nextViewpoint"); }
   lastViewpoint (layerNode)     { this .#layerRequest (layerNode, "lastViewpoint"); }

   // X3DBrowser.changeViewpoint ([layerNode,] name).
   changeViewpoint (layerNode, name)
   {
      if (arguments .length === 1)
      {
         name      = layerNode;
         layerNode = undefined;
      }

      this .#layerRequest (layerNode, `changeViewpoint:${String (name)}`);
   }

   // X3DBrowser.bindViewpoint (layerNode, viewpointNode): both must be of their type, the viewpoint binds with the next
   // frame's events (its layer is the one that holds it; X_ITE only checks the type of layerNode).
   bindViewpoint (layerNode, viewpointNode)
   {
      const layer = this .#cast ("X3DLayerNode", layerNode);
      const vp    = this .#cast ("X3DViewpointNode", viewpointNode);

      if (layer === undefined)
         throw new Error ("Browser.bindViewpoint: layerNode must be of type X3DLayerNode.");

      if (vp === undefined)
         throw new Error ("Browser.bindViewpoint: viewpointNode must be of type X3DViewpointNode.");

      this .#engine ?.viewpointRequest (`bindViewpoint:${vp}`);
   }

   // X3DBrowser.viewAll ([layerNode,] [transitionTime = 1]).
   viewAll (layerNode, transitionTime = 1)
   {
      if (arguments .length === 1 && typeof layerNode === "number")
      {
         transitionTime = layerNode;
         layerNode      = undefined;
      }

      this .#engine ?.viewAll (+transitionTime, this .#cast ("X3DLayerNode", layerNode));
   }

   // X3DBrowserContext.isLive, setLive, beginUpdate, endUpdate: the browser's live state. A browser that is not live
   // pauses the TimeSensors and the like of its world; the page's own field assignments still send their events.
   isLive () { return this .#live; }

   setLive (value)
   {
      this .#live = !! value .valueOf ();
      this .#engine ?.setLive (this .#live);
   }

   beginUpdate () { this .#beginUpdate (); }
   endUpdate ()   { this .#endUpdate (); }

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

      for (const line of this .#engine ?.takeLog (id) ?? [ ])
         printLogLine (line);
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
         // any case (X_ITE matches only lower case) never runs: the page does not run a world's code through a
         // navigation (Script nodes run only when the page lets them, API.md "Script nodes"). A new window gets no
         // opener.
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

      // X3DSoundContext.initialize: these events of the canvas try the waiting resume () again (startAudioElements),
      // so a user gesture unlocks the AudioContext under the browser's autoplay policy.
      for (const type of [ "blur", "click", "contextmenu", "dblclick", "focus", "keydown", "keyup", "mousedown", "mouseup", "pointerup", "touchend", "touchstart", "wheel" ])
         on (canvas, type, () => this .#resumeAudio ());

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

   // The ContentScale browser option (the contentScale attribute sets it): -1 is "auto", the device pixel ratio. Any other
   // value is used as it is, like X3DRenderingContext.limitFramebufferSize: 0, a negative or a NaN value gives a 1 x 1
   // framebuffer.
   #contentScale ()
   {
      const value = this .#options .get ("ContentScale") .getValue ();

      return value === -1 ? devicePixelRatio : value;
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

   // X3DSoundContext.getAudioContext: the engine's first Sound node makes the audio context (the engine's Web Audio
   // port). The page then makes its own AudioContext and calls resume () (startAudioElement), as X_ITE does. The
   // engine's context does not render in the page, so its clock stays at 0; before each frame it takes this
   // context's currentTime, the clock X_ITE's AudioElement reads. While the browser's autoplay policy holds the
   // context, its clock stands still, as X_ITE's does. Nothing is connected to this context: X_ITE_R plays no sound
   // in a page yet (web/API.md).
   #audioClock (engine)
   {
      if (!this .#audioContext)
      {
         if (!engine .hasAudio ())
            return;

         this .#audioContext = new AudioContext ();
         this .#audioWaiting = true;
         this .#resumeAudio ();
      }

      engine .setAudioClock (this .#audioContext .currentTime);
   }

   // X3DSoundContext.startAudioElements for the AudioContext: resume () until it succeeds once.
   #resumeAudio ()
   {
      if (!this .#audioWaiting)
         return;

      const context = this .#audioContext;

      context .resume ()
         .then (() => { if (this .#audioContext === context) this .#audioWaiting = false; })
         .catch (Function .prototype);
   }

   // The frame loop runs while the element is in a document, the page is visible and the browser is not disposed.
   #start ()
   {
      if (this .#frame || !this .#engine || this .#disposed || !this .#element .isConnected || document .visibilityState === "hidden")
         return;

      const tick = stamp =>
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

         // X3DBrowserContext.traverse: a frame sooner than 1 / MaximumFrameRate after the last one is skipped
         // (X3DTimeContext.advanceTime). A browser that is not live still processes the page's events and draws; its
         // engine pauses the TimeSensors and the like (setLive).
         // The interval is measured with the frame's timestamp: X_ITE draws on demand, so its first frame after an event comes
         // long after the last one, where this loop runs every frame and Date.now () between two of them can be shorter than
         // the frame period. The browser time is Date.now (), as X_ITE's.
         if ((stamp - this .#lastStamp) / 1000 <= 1 / Math .max (this .#options .get ("MaximumFrameRate") .getValue (), 1))
            return;

         const time = Date .now () / 1000;

         this .#lastStamp = stamp;
         ++ this .#processed;

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
            this .#audioClock (engine);
            engine .frame (time, width, height);

            // X3DSoundContext.startAudioElement: a media element started in this frame tries the waiting resume ()
            // again, so a user activation from outside the canvas also unlocks the context.
            if (engine .takeAudioStarts ())
            {
               this .#audioClock (engine);
               this .#resumeAudio ();
            }

            processFieldEvents (this, engine .current);
            this .#sceneRequestsDone ();
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
   get [Symbol .for ("X_ITE_R.frameLoop")] () { return { running: this .#frame !== 0, frames: this .#frames, processed: this .#processed, live: this .#live }; }

   connectedCallback ()    { this .#start (); this .#checkUpdate (); }
   disconnectedCallback () { this .#stop (); this .#releaseInput (); this .#checkUpdate (); }

   // X3DCoreContext.parseBooleanAttribute: only "true" and "false" are values; any other text is the default.
   static #boolean (value)
   {
      if (value === "true")
         return true;

      if (value === "false")
         return false;
   }

   // X3DCoreContext.attributeChangedCallback: an attribute sets its browser option. Removing the attribute (null) sets
   // the default of the attribute, not the default of the option. The options themselves never write an attribute.
   attributeChangedCallback (name, oldValue, newValue)
   {
      const boolean = X3DBrowser .#boolean (newValue);

      switch (name .toLowerCase ())
      {
         case "antialiased":                  this .setBrowserOption ("Antialiased",                  boolean ?? true);  break;
         case "cache":                        this .setBrowserOption ("Cache",                        boolean ?? true);  break;
         case "colorspace":                   this .setBrowserOption ("ColorSpace",                   newValue || "LINEAR_WHEN_PHYSICAL_MATERIAL"); break;
         case "contentscale":                 this .setBrowserOption ("ContentScale",                 newValue === "auto" ? -1 : newValue); break;
         case "contextmenu":                  this .setBrowserOption ("ContextMenu",                  boolean ?? true);  break;
         case "debug":                        this .setBrowserOption ("Debug",                        boolean ?? false); break;
         case "displaycolorspace":            this .setBrowserOption ("DisplayColorSpace",            newValue || "SRGB"); break;
         case "exposure":                     this .setBrowserOption ("Exposure",                     newValue); break;
         case "logarithmicdepthbuffer":       this .setBrowserOption ("LogarithmicDepthBuffer",       boolean ?? false); break;
         case "maximumframerate":             this .setBrowserOption ("MaximumFrameRate",             newValue); break;
         case "multisampling":                this .setBrowserOption ("Multisampling",                newValue); break;
         case "notifications":                this .setBrowserOption ("Notifications",                boolean ?? true);  break;
         case "orderindependenttransparency": this .setBrowserOption ("OrderIndependentTransparency", boolean ?? false); break;
         case "splashscreen":                 this .setBrowserOption ("SplashScreen",                 boolean ?? true);  break;
         case "textcompression":              this .setBrowserOption ("TextCompression",              newValue || "CHAR_SPACING"); break;
         case "timings":                      this .setBrowserOption ("Timings",                      boolean ?? false); break;
         case "tonemapping":                  this .setBrowserOption ("ToneMapping",                  newValue || "NONE"); break;
         case "xrsessionmode":                this .setBrowserOption ("XRSessionMode",                newValue || "IMMERSIVE_VR"); break;
         case "oninitialized":
         case "onshutdown":
         {
            try
            {
               this .#element [name] = new Function ("event", newValue);
            }
            catch (error)
            {
               console .error (error);
            }

            break;
         }
         case "update":
         {
            if (newValue === "auto")
            {
               this .setBrowserOption ("AutoUpdate", true);
            }
            else
            {
               this .setBrowserOption ("AutoUpdate", false);

               if (boolean ?? true)
                  this .#beginUpdate ();
               else
                  this .#endUpdate ();
            }

            break;
         }
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
      this .#setAutoUpdate ();

      if (this .#engine)
      {
         this .#engine .free ();
         this .#engine = null;
      }

      // The AudioContext is the page's own (#audioClock makes it): it goes with the browser. Closing it twice is
      // harmless, but it is closed once.
      const context = this .#audioContext;

      this .#audioContext = null;
      this .#audioWaiting = false;
      context ?.close () .catch (Function .prototype);
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

   static get observedAttributes ()
   {
      const attributes = [ "antialiased", "baseURL", "cache", "colorSpace", "contentScale", "contextMenu", "debug", "displayColorSpace",
         "exposure", "extensions", "logarithmicDepthBuffer", "maximumFrameRate", "multisampling", "notifications", "oninitialized",
         "onshutdown", "orderIndependentTransparency", "splashScreen", "src", "textCompression", "timings", "toneMapping", "update",
         "url", "xrSessionMode" ];

      return attributes .concat (attributes .map (attribute => attribute .toLowerCase ()));
   }

   attributeChangedCallback (name, oldValue, newValue)
   {
      this .browser .attributeChangedCallback (name, oldValue, newValue);
   }

   toDataURL (... args) { return this .browser .getCanvas () .toDataURL (... args); }
   toBlob (... args)    { return this .browser .getCanvas () .toBlob (... args); }
   captureStream (... args) { return this .browser .getCanvas () .captureStream (... args); }

   get [Symbol .toStringTag] () { return "X3DCanvasElement"; }
}

customElements .define ("x3d-canvas", X3DCanvasElement);

// X3D: X_ITE's namespace object, for the parts listed in API.md.
let promise; // The return value of X3D (), as in X_ITE.

// X3D (onfulfilled, onrejected): a promise that resolves when the document is loaded and every <x3d-canvas> has a browser.
const X3D = Object .assign (function (onfulfilled, onrejected)
{
   promise ??= new Promise ((resolve, reject) =>
   {
      const ready = () =>
      {
         try
         {
            if (Array .from (document .querySelectorAll ("x3d-canvas")) .every (canvas => canvas .browser))
               resolve ();
            else
               throw new Error ("Couldn't create browser.");
         }
         catch (error)
         {
            reject (error);
         }
      };

      if (document .readyState === "complete")
         ready ();
      else
         document .addEventListener ("DOMContentLoaded", ready, { once: true });
   });

   return promise .then (onfulfilled) .catch (onrejected);
},
{
   X3DConstants,
   X3DFieldDefinition,
   FieldDefinitionArray,
   X3DField,
   X3DArrayField,
   X3DRoute,
   RouteArray,
   ComponentInfo,
   ComponentInfoArray,
   ProfileInfo,
   ProfileInfoArray,
   ... Fields,
},
{
   noConflict: (() =>
   {
      const
         _had = "X3D" in window,
         _X3D = window .X3D;

      return function ()
      {
         if (window .X3D === X3D)
         {
            if (_had)
               window .X3D = _X3D;
            else
               delete window .X3D;
         }

         return X3D;
      };
   })(),

   getBrowser (element)
   {
      if (!element)
         return document .querySelector ("x3d-canvas, X3DCanvas") ?.browser;

      if (element instanceof X3DCanvasElement)
         return element .browser;

      return Array .from (document .querySelectorAll (String (element)))
         .find (element => element .nodeName .match (/^(?:x3d-canvas|X3DCanvas)$/i)) ?.browser;
   },

   createBrowser (... args)
   {
      const element = document .createElement ("x3d-canvas");

      if (args .length)
         element .browser .loadURL (... args);

      return element;
   },

   toString () { return Object .prototype .toString .call (this); },
   [Symbol .toStringTag]: "X3D",
});

globalThis .X3D ??= X3D;

export default X3D;
