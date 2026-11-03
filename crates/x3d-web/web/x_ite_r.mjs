// X_ITE_R page adapter: defines <x3d-canvas> and the X_ITE-named JavaScript API on top of the Rust engine
// (crates/x3d-web, built into ./pkg by tools/build-web.sh). Everything X3D (parsing, the scene graph, events,
// rendering) runs in Rust; this file only fetches the world, runs the requestAnimationFrame loop and converts
// values. The names follow X_ITE 16.5.0 (oracle/x_ite-pin); web/API.md lists what exists and what does not.

import init, {
   WebBrowser, parseFieldWithoutScene, printEmptyScene, configTable, htmlSupport, scriptOp,
   chainExists, chainWorldGeneration, chainTakeReleasedNodes, chainFieldNames, chainFieldData, chainSetFieldData, chainTakeFieldChanges, chainObserveField,
   chainTypeName, chainNodeName, chainNamedNode, chainParseField, chainRootNodes, chainSetRootNodes, chainNodeValue, chainRoutes, chainGetRoute, chainAddRoute, chainDeleteRoute, chainFieldDefinitions,
} from "./pkg/x3d_web.js";
import { createScriptRealm } from "./pkg/script.mjs";
import { AudioMirror } from "./audio_mirror.mjs";
import { VideoMirror } from "./video_mirror.mjs";
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

// The constants of the external components a world loads: X_ITE adds the constants of an external component as its module
// loads, and a world loads the components of its PROFILE, its COMPONENT statements and what those depend on, so after the
// world's first frame every node type of those components is in X3DConstants, whether or not the world has a node of
// them. `profile` is the engine's profile name, `list` its [name, level, ...] components (profileAndComponents).
function worldComponentConstants (profile, list)
{
   const config = supported ();
   const names  = [ ];
   const add    = name =>
   {
      const component = config .components .get (name);

      if (component && !names .includes (component .name))
      {
         names .push (component .name);

         for (const dependency of component .dependencies)
            add (dependency);
      }
   };

   for (const component of config .profiles .get (profile) ?.components ?? [ ])
      add (component .name);

   for (let i = 0; i < list .length; i += 2)
      add (list [i]);

   for (const name of names)
   {
      for (const constant of X3D_COMPONENT_CONSTANTS [name] ?? [ ])
      {
         if (!Object .hasOwn (X3DConstants, constant))
            X3DConstants .addConstant (constant, nextConstant ++);
      }
   }
}

// The wasm object of a browser (read-only, for tests and diagnostics).
const _engine = Symbol .for ("X_ITE_R.engine");

// The engine of a browser while a field callback runs in the engine's route chain (issue #202).
const _inChain = Symbol ("X_ITE_R.inChain");

// The end of the route chain of a browser: the calls that were held back during it run (issue #202).
const _chainEnd = Symbol ("X_ITE_R.chainEnd");

// The engine calls of a field callback that runs in the route chain. The `WebBrowser` is borrowed by the export that runs the
// chain, so these go through the engine's free functions, which reach the world in the middle of its chain. Other calls
// throw: they would need the `WebBrowser` (API.md, "Script nodes").
const CHAIN_OPERATIONS = {
   exists: scene => chainExists (scene),
   worldGeneration: scene => chainWorldGeneration (scene),
   takeReleasedNodes: scene => chainTakeReleasedNodes (scene),
   fieldNames: (scene, node) => chainFieldNames (scene, node),
   fieldData: (scene, node, name) => chainFieldData (scene, node, name),
   setFieldData: (scene, node, name, data) => chainSetFieldData (scene, node, name, data),
   takeFieldChanges: scene => chainTakeFieldChanges (scene),
   observeField: (scene, node, name, on) => chainObserveField (scene, node, name, on),
   typeName: (scene, node) => chainTypeName (scene, node),
   nodeName: (scene, node) => chainNodeName (scene, node),
   namedNode: (scene, name) => chainNamedNode (scene, name),
   parseField: (scene, ... args) => chainParseField (scene, ... args),
   rootNodes: scene => chainRootNodes (scene),
   setRootNodes: (scene, list) => chainSetRootNodes (scene, list),
   nodeValue: (scene, node, name) => chainNodeValue (scene, node, name),
   routes: scene => chainRoutes (scene),
   fieldDefinitions: (scene, node) => chainFieldDefinitions (scene, node),
   getRoute: (scene, ... args) => chainGetRoute (scene, ... args),
   addRoute: (scene, ... args) => chainAddRoute (scene, ... args),
   deleteRoute: (scene, route) => chainDeleteRoute (scene, route),
};

const chainEngine = new Proxy ({ }, {
   get (target, name)
   {
      return CHAIN_OPERATIONS [name] ?? (() =>
      {
         throw new Error (`X_ITE_R: the browser call '${String (name)}' is not available inside a field callback that runs in the route chain.`);
      });
   },
});

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
const EPOCH       = Symbol ("epoch");       // the interfaceEpoch its field names were read in

// A PROTO declaration whose interface changes changes the fields of its instances, in the call (updateInstances) or in a
// later frame (requestUpdateInstances). Once a page has used a declaration, every frame and every such call starts a new
// epoch, and a node reads its field names again the first time it is used in one.
let interfaceEpoch = 0;
let declarationsUsed = false;

function checkInterface (ref, engine)
{
   if (ref [EPOCH] === interfaceEpoch)
      return;

   ref [EPOCH] = interfaceEpoch;

   if (!ref [NAMES])
      return;

   const h     = ref [_node];
   const names = Array .from (engine .fieldNames (h .scene, h .id));

   if (names .length === ref [NAMES] .size && names .every ((name, i) => name === Array .from (ref [NAMES]) [i]))
      return;

   ref [NAMES]       = undefined;
   ref [DEFINITIONS] = undefined;
   ref [FIELDS] .clear ();
}
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

      checkInterface (this, engineOf (h));

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
   checkInterface (ref, engine);

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

   if (declarationsUsed)
      ++ interfaceEpoch;

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

// DOMIntegration listens to the load state of its Inline nodes (X3DUrlObject `_loadState`, a private field): per browser the scene
// and, per node, the function to call.
const loadStateTaps = new WeakMap ();

// The events of one field in a cycle are one event for X_ITE's interest: the node is looked at once, when the engine is free.
function scheduleLoadState (tap, scene, node, data)
{
   // IN_PROGRESS_STATE does nothing to DOMIntegration, and X_ITE merges it with the state that follows in the same cycle.
   if (data === 1)
      return;

   const fn = tap ?.scene === scene ? tap .nodes .get (node) : undefined;

   if (!fn || tap .pending .has (node))
      return;

   tap .pending .add (node);

   queueMicrotask (() =>
   {
      tap .pending .delete (node);
      fn ();
   });
}

function deliverFieldEvents (browser, scene, table, events, printUpTo)
{
   const tap = loadStateTaps .get (browser);

   if (!table ?.refs .size && !table ?.rootNodes && !tap)
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

      if (events [i + 1] === "loadState")
      {
         scheduleLoadState (tap, scene, events [i], events [i + 2]);
         continue;
      }

      const ref   = table ?.refs .get (events [i]);
      const entry = ref ?.[FIELDS] .get (events [i + 1]);

      if (!entry)
         continue;

      const callbacks = entry .field .getFieldCallbacks ();

      if (!callbacks .size)
         continue;

      const failed = callFieldCallbacks (ref, entry, events [i + 2]);

      if (failed)
      {
         browser [_engine] .haltEvents (scene);
         reportError (failed .error);
         return;
      }
   }
}

// The callbacks of a field with the value `data` of its event: they see the value of the event, and the field holds the current
// one again after them. X_ITE walks the live Map it started with: a callback removed in place by an earlier one is skipped, and
// addFieldCallback copies the Map, so a callback added meanwhile waits for the next event. A callback that throws ends the
// delivery: `{ error }`.
function callFieldCallbacks (ref, entry, data)
{
   setFieldData (entry .field, data, ref [_node]);

   try
   {
      for (const callback of entry .field .getFieldCallbacks () .values ())
      {
         try
         {
            callback (entry .field .valueOf ());
         }
         catch (error)
         {
            return { error };
         }
      }
   }
   finally
   {
      setFieldData (entry .field, entry .data, ref [_node]);
   }
}

// The engine calls the page's field callbacks in its route chain, where X_ITE's X3DField.processEvent calls them (issue #202):
// the callbacks of the event of field `name` of `node`, with the engine reachable through the chain calls only. The lines the
// event follows are on the console already. `true`: a callback threw; the error is reported, and the engine processes no event
// again, as X_ITE's event loop.
function deliverChainEvent (browser, scene, node, name, data)
{
   const previous = browser [_inChain] (chainEngine);

   try
   {
      retireReleased (browser, scene);

      // DOMIntegration: the engine is borrowed here, so the Inline element is looked at when the chain is over.
      if (name === "loadState")
      {
         scheduleLoadState (loadStateTaps .get (browser), scene, node, data);
         return false;
      }

      const ref   = nodeRefTables .get (browser) ?.get (scene) ?.refs .get (node);
      const entry = ref ?.[FIELDS] .get (name);

      if (!entry ?.field .getFieldCallbacks () .size)
         return false;

      // The engine's value of the field is the value of its event now.
      entry .data = data;

      const failed = callFieldCallbacks (ref, entry, data);

      if (!failed)
         return false;

      reportError (failed .error);
      return true;
   }
   finally
   {
      browser [_inChain] (previous);

      // The calls the callback made that need the `WebBrowser` itself (setLive, the disposal) run now that the chain is over.
      if (!previous)
         browser [_chainEnd] ();
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
      Object .defineProperty (this, _infoArray, { value: [ ], writable: true, configurable: true });
      Object .defineProperty (this, _infoIndex, { value: new Map (), writable: true, configurable: true });
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
   toReversed ()                { return new (this .constructor) ([ ... this [_infoArray] ] .reverse ()); }
   toSorted (compareFn)         { return new (this .constructor) ([ ... this [_infoArray] ] .sort (compareFn)); }

   toSpliced (start, deleteCount, ... insertValues)
   {
      const array = [ ... this [_infoArray] ];

      array .splice (start, deleteCount, ... insertValues);

      return new (this .constructor) (array);
   }

   with (index, value)
   {
      const array = [ ... this [_infoArray] ];

      array [index] = value;

      return new (this .constructor) (array);
   }

   getTypeName () { return this .constructor .typeName ?? this .constructor .name; }

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
// The text and world URL of an X3DScene object whose world was replaced (X3DBrowser.replaceWorld).
const replacedScenes = new WeakMap ();

// The engine's replaced world that a Script's functions and nodes in the page still reach goes when the page dropped them
// (issue #202). A held value does not refer to the realm.
const realmRegistry = new FinalizationRegistry (({ engine, host }) =>
{
   try { engine .releaseRetired (host); } catch { /* the engine is gone */ }
});

const sceneRegistry = new FinalizationRegistry (({ browser, scene }) => browser [_engine] ?.discard (scene));

// Execution/NamedNodesArray.js, ExportedNodesArray.js and X3DExportedNode.js: the live lists getNamedNodes () and getExportedNodes ()
// give. They are X3DInfoArrays whose list is read from the engine at each use; `filter`, `map`, `slice` and `copy` give a list
// of the same class that holds the values it was made from.
const liveInfoArray = (array, list) =>
{
   const read = () =>
   {
      const values = list ();
      const index  = new Map ();

      for (const value of values)
         index .set (value instanceof X3DExportedNode ? value .exportedName : value instanceof X3DImportedNode ? value .importedName : value .getNodeName (), value);

      return [ values, index ];
   };

   Object .defineProperty (array, _infoArray, { get: () => read () [0], set () { } });
   Object .defineProperty (array, _infoIndex, { get: () => read () [1], set () { } });

   return array;
};

class NamedNodesArray extends X3DInfoArray
{
   constructor (values = [ ], list = null)
   {
      const entries = Array .from (values, value => [ value .getNodeName (), value ]);
      const array   = super ([ ], Fields .SFNode);

      for (const [ key, value ] of entries)
         array .add (key, value);

      if (list)
         liveInfoArray (array, list);

      return array;
   }

   getTypeName () { return "NamedNodesArray"; }
}

// X3DExportedNode: `EXPORT localName AS exportedName` of a scene. The object keeps the node and the description it was made
// with; the description setter also changes the scene's record while this object still is the scene's.
class X3DExportedNode
{
   #scene;
   #handle;
   #exportedName;
   #localId;
   #description;

   constructor (scene, handle, exportedName, localId, description)
   {
      this .#scene        = scene;
      this .#handle       = handle;
      this .#exportedName = exportedName;
      this .#localId      = localId;
      this .#description  = description;
   }

   getTypeName ()         { return "X3DExportedNode"; }
   getExecutionContext () { return this .#scene; }
   getExportedName ()     { return this .#exportedName; }
   getDescription ()      { return this .#description; }

   setDescription (value)
   {
      this .#description = String (value);

      if (exportedNodeCache .get (this .#scene) ?.get (this .#exportedName) === this)
         engineOf (this .#handle) .setExportedNode (this .#handle .scene, this .#exportedName, this .#localId, this .#description);
   }

   get exportedName () { return this .#exportedName; }
   get localNode ()    { return nodeObject (this .#handle, this .#localId); }
   get description ()  { return this .#description; }
   set description (value) { this .setDescription (value); }

   #localName () { return engineOf (this .#handle) .nodeName (this .#handle .scene, this .#localId) ?? ""; }

   toVRMLString ()
   {
      const local = this .#localName ();

      return `EXPORT ${local}${this .#exportedName !== local ? ` AS ${this .#exportedName}` : ""}${this .#description ? ` DESCRIPTION "${this .#description}"` : ""}`;
   }

   toXMLString ()
   {
      const local = this .#localName ();

      return `<EXPORT localDEF='${local}'${this .#exportedName !== local ? ` AS='${this .#exportedName}'` : ""}${this .#description ? ` DESCRIPTION='${this .#description}'` : ""}/>`;
   }

   toJSONString ()
   {
      const local = this .#localName ();

      return `\n{ "EXPORT":\n  {\n    "@localDEF": "${local}"${this .#exportedName !== local ? `,\n    "@AS": "${this .#exportedName}"` : ""}${this .#description ? `,\n    "@DESCRIPTION": "${this .#description}"` : ""}\n  }\n}`;
   }

   get [Symbol .toStringTag] () { return "X3DExportedNode"; }
}

Object .defineProperty (X3DExportedNode, "typeName", { value: "X3DExportedNode", enumerable: true });

for (const key of Object .getOwnPropertyNames (X3DExportedNode .prototype))
{
   if (key !== "constructor" && !/^(exportedName|localNode|description)$/ .test (key))
      Object .defineProperty (X3DExportedNode .prototype, key, { enumerable: false });
}

const exportedNodeCache = new WeakMap (); // X3DScene → Map (exported name → its X3DExportedNode)

class ExportedNodesArray extends X3DInfoArray
{
   constructor (values = [ ], list = null)
   {
      const entries = Array .from (values, value => [ value .getExportedName (), value ]);
      const array   = super ([ ], X3DExportedNode);

      for (const [ key, value ] of entries)
         array .add (key, value);

      if (list)
         liveInfoArray (array, list);

      return array;
   }

   getTypeName () { return "ExportedNodesArray"; }
}

// X3DImportedNode: `IMPORT inlineName.exportedName AS importedName` of a scene (Execution/X3DImportedNode.js). The object keeps
// the Inline, the names and the description it was made with; a rename changes the importedName of the object that is the
// scene's, the description setter also changes the scene's record while this object still is the scene's.
// getInlineNode (), getExportedNode () and getInstance () give the SFNode where X_ITE gives its internal node.
class X3DImportedNode
{
   #scene;
   #handle;
   #inlineId;
   #exportedName;
   #importedName;
   #instanceId;
   #description;

   constructor (scene, handle, inlineId, exportedName, importedName, instanceId, description)
   {
      this .#scene        = scene;
      this .#handle       = handle;
      this .#inlineId     = inlineId;
      this .#exportedName = exportedName;
      this .#importedName = importedName;
      this .#instanceId   = instanceId;
      this .#description  = description;
   }

   get instanceId () { return this .#instanceId; }

   rename (importedName) { this .#importedName = importedName; }

   getTypeName ()         { return "X3DImportedNode"; }
   getExecutionContext () { return this .#scene; }
   getInlineNode ()       { return nodeObject (this .#handle, this .#inlineId); }
   getExportedName ()     { return this .#exportedName; }
   getImportedName ()     { return this .#importedName; }
   getInstance ()         { return nodeObject (this .#handle, this .#instanceId); }
   getDescription ()      { return this .#description; }

   getExportedNode ()
   {
      const id = engineOf (this .#handle) .importedExportedNode (this .#handle .scene, this .#importedName);

      if (id === undefined || importedNodeCache .get (this .#scene) ?.get (this .#importedName) !== this)
         throw new Error (`Exported node '${this .#exportedName}' not found.`);

      return nodeObject (this .#handle, id);
   }

   setDescription (value)
   {
      this .#description = String (value);

      if (importedNodeCache .get (this .#scene) ?.get (this .#importedName) === this)
         engineOf (this .#handle) .setImportedDescription (this .#handle .scene, this .#importedName, this .#description);
   }

   get inlineNode ()    { return this .getInlineNode (); }
   get exportedName ()  { return this .#exportedName; }
   get exportedNode ()  { return this .getExportedNode (); }
   get importedName ()  { return this .#importedName; }
   get instance ()      { return this .getInstance (); }
   get description ()   { return this .#description; }
   set description (value) { this .setDescription (value); }

   get [Symbol .toStringTag] () { return "X3DImportedNode"; }
}

Object .defineProperty (X3DImportedNode, "typeName", { value: "X3DImportedNode", enumerable: true });

for (const key of Object .getOwnPropertyNames (X3DImportedNode .prototype))
{
   if (key !== "constructor" && !/^(inlineNode|exportedName|exportedNode|importedName|instance|description)$/ .test (key))
      Object .defineProperty (X3DImportedNode .prototype, key, { enumerable: false });
}

const importedNodeCache = new WeakMap (); // X3DScene → Map (imported name → its X3DImportedNode)

class ImportedNodesArray extends X3DInfoArray
{
   constructor (values = [ ], list = null)
   {
      const entries = Array .from (values, value => [ value .getImportedName (), value ]);
      const array   = super ([ ], X3DImportedNode);

      for (const [ key, value ] of entries)
         array .add (key, value);

      if (list)
         liveInfoArray (array, list);

      return array;
   }

   getTypeName () { return "ImportedNodesArray"; }
}

for (const type of [ NamedNodesArray, ExportedNodesArray, ImportedNodesArray ])
{
   for (const key of Object .getOwnPropertyNames (type .prototype))
   {
      if (key !== "constructor")
         Object .defineProperty (type .prototype, key, { enumerable: false });
   }

   Object .defineProperty (type, "typeName", { value: type .name, enumerable: true });
}

// --- PROTO and EXTERNPROTO declarations -----------------------------------------------------------------------------------
//
// Prototype/X3DProtoDeclaration.js, X3DExternProtoDeclaration.js, X3DProtoDeclarationNode.js, ProtoDeclarationArray.js and
// ExternProtoDeclarationArray.js of X_ITE, and the declaration members of X3DExecutionContext.js (issue #137). The engine owns
// the declarations; a declaration object stands for one of them by its index in the scene's table (declIds, declInfo, declOp)
// and is made once per (scene, kind, index), so `scene .protos .get ("A") === scene .protos .get ("A")`. A declaration that
// is removed from its scene stays usable, as in X_ITE. After the scene's world is replaced every use throws the world-ownership
// error of the node handles.
//
// Kept differences (API.md): the appinfo and documentation of a declaration are kept by the page and put into its text forms
// here; the field objects of getField () and getUserDefinedFields () are copies (writing one does not change the declaration);
// the body (getBody ()) is a read-only view; addUserDefinedField on an EXTERNPROTO does nothing.

const declCache = new WeakMap (); // X3DScene → Map ("p" or "e" + index → declaration object)

const xmlEscape = text => String (text) .replace (/&/g, "&amp;") .replace (/</g, "&lt;") .replace (/>/g, "&gt;") .replace (/"/g, "&quot;") .replace (/'/g, "&apos;");

class X3DProtoDeclarationNode
{
   #scene;
   #extern;
   #index;
   #appInfo = "";
   #documentation = "";
   #definitions = null;
   #definitionsKey = null;
   #userFields = null;
   #fieldObjects = new Map ();

   constructor (scene, extern, index)
   {
      declarationsUsed = true;

      this .#scene  = scene;
      this .#extern = extern;
      this .#index  = index;
   }

   get [_node] () { return this .#scene [_node]; }

   get index ()  { return this .#index; }
   get scene ()  { return this .#scene; }

   // The engine's description of the declaration.
   info ()
   {
      const handle = this .#scene [_node];
      const info   = engineOf (handle) .declInfo (handle .scene, this .#extern, this .#index);

      if (info === undefined)
         throw new Error ("X_ITE_R: the declaration is no longer known to its world.");

      return info;
   }

   engine () { return engineOf (this .#scene [_node]); }

   get isExternProto ()  { return this .#extern; }
   get name ()           { return this .info () .name; }
   get fields ()         { return this .getFieldDefinitions (); }
   get appInfo ()        { return this .getAppInfo (); }
   set appInfo (value)   { this .setAppInfo (value); }
   get documentation ()  { return this .getDocumentation (); }
   set documentation (value) { this .setDocumentation (value); }

   getName ()               { return this .name; }
   getBrowser ()            { return this .#scene .getBrowser (); }
   getExecutionContext ()   { return this .#scene; }
   canUserDefinedFields ()  { return true; }
   getCloneCount ()         { return 0; }

   getAppInfo ()            { return this .#appInfo; }
   setAppInfo (value)       { this .#appInfo = String (value); }
   getDocumentation ()      { return this .#documentation; }
   setDocumentation (value) { this .#documentation = String (value); }

   // The class's definitions (`metadata`), then the declared fields, rebuilt only when the interface changed.
   getFieldDefinitions ()
   {
      const info = this .info ();
      const key  = JSON .stringify (info .fields);

      if (key !== this .#definitionsKey)
      {
         const handle = this .#scene [_node];

         this .#definitionsKey = key;
         this .#fieldObjects .clear ();

         const fields = info .fields .map (([ name, accessType, typeName, value ]) =>
         {
            const field = fieldFromData (typeName, value, handle);

            field .setName (name);
            field .setAccessType (accessType);
            this .#fieldObjects .set (name, field);

            return field;
         });

         this .#userFields  = fields .slice (1);
         this .#definitions = new FieldDefinitionArray (info .fields .map (([ name, accessType ], i) => new X3DFieldDefinition (accessType, name, fields [i])));
      }

      return this .#definitions;
   }

   // The declared fields (not `metadata`) as field objects, one per name until the interface changes.
   getUserDefinedFields ()
   {
      this .getFieldDefinitions ();

      return this .#userFields;
   }

   getField (name)
   {
      name = String (name);

      this .getFieldDefinitions ();

      const field = this .#fieldObjects .get (name);

      if (field)
         return field;

      throw new Error (`Unknown field '${name}' in node class ${this .getTypeName ()}.`);
   }

   getFieldDefinition (name)
   {
      const definition = this .getFieldDefinitions () .get (String (name));

      if (definition)
         return definition;

      throw new Error (`Unknown field '${name}' in node class ${this .getTypeName ()}.`);
   }

   // createInstance (executionContext): an instance in the declaration's context, not part of the scene.
   createInstance (executionContext)
   {
      executionContext .getBrowser ();

      const handle = this .#scene [_node];
      const id     = engineOf (handle) .declNewInstance (handle .scene, this .#extern, this .#index);

      return nodeObject (handle, id);
   }

   newInstance () { return this .createInstance (this .#scene); }

   requestUpdateInstances () { this .#update (false); }
   updateInstances ()        { this .#update (true); }

   #update (now)
   {
      const handle = this .#scene [_node];

      try { engineOf (handle) .declUpdateInstances (handle .scene, this .#extern, this .#index, now); }
      catch (error) { throw engineError (error); }

      ++ interfaceEpoch;
   }

   // X3DBaseNode.addUserDefinedField (accessType, name, field) and removeUserDefinedField (name) of a PROTO.
   addUserDefinedField (accessType, name, field)
   {
      name = String (name);

      field .setTainted (true);

      const handle = this .#scene [_node];

      if (this .#extern)
         return;

      try { engineOf (handle) .declUserField (handle .scene, this .#index, name, accessType, field .getTypeName (), fieldData (field, handle)); }
      catch (error) { throw engineError (error); }

      this .#update (true);
   }

   removeUserDefinedField (name)
   {
      const handle = this .#scene [_node];

      if (this .#extern)
         return;

      try { engineOf (handle) .declUserField (handle .scene, this .#index, String (name), 0, "", null); }
      catch (error) { throw engineError (error); }
   }

   #text (format)
   {
      const handle = this .#scene [_node];
      let text;

      try { text = engineOf (handle) .declString (handle .scene, this .#extern, this .#index, format); }
      catch (error) { throw engineError (error); }

      const appInfo       = this .getAppInfo ();
      const documentation = this .getDocumentation ();

      // The engine knows no appinfo or documentation: they go after the name of the declaration.
      if (format === "xml")
      {
         let attributes = "";

         if (appInfo)
            attributes += ` appinfo='${xmlEscape (appInfo)}'`;

         if (documentation)
            attributes += ` documentation='${xmlEscape (documentation)}'`;

         const tag = this .#extern ? "ExternProtoDeclare" : "ProtoDeclare";

         if (attributes)
            text = text .replace (new RegExp (`^(<${tag} name='[^']*')`), `$1${attributes}`);
      }
      else if (format === "json")
      {
         let lines = "";

         if (appInfo)
            lines += `,\n    "@appinfo": ${JSON .stringify (appInfo)}`;

         if (documentation)
            lines += `,\n    "@documentation": ${JSON .stringify (documentation)}`;

         if (lines)
            text = text .replace (/^(\n\{ "[A-Za-z]+":\n  \{\n    "@name": "(?:[^"\\]|\\.)*")/, `$1${lines}`);
      }

      return text;
   }

   toVRMLString () { return this .#text ("vrml"); }
   toXMLString ()  { return this .#text ("xml"); }
   toJSONString () { return this .#text ("json"); }
}

// The body of a PROTO: a read-only X3DExecutionContext view over the engine's context of the declaration.
class X3DExecutionContext
{
   #declaration;
   #ctx;
   #rootNodes = null;
   #namedNodes = null;

   constructor (declaration, ctx)
   {
      this .#declaration = declaration;
      this .#ctx         = ctx;
   }

   #handle () { return this .#declaration .scene [_node]; }

   getTypeName ()         { return "X3DExecutionContext"; }
   getOuterNode ()        { return this .#declaration; }
   getLocalScene ()       { return this .#declaration .scene; }
   getExecutionContext () { return this .#declaration .scene; }
   getBrowser ()          { return this .#declaration .scene .getBrowser (); }
   get isScene ()         { return false; }

   get rootNodes ()
   {
      const handle = this .#handle ();
      const ids    = engineOf (handle) .ctxRootNodes (handle .scene, this .#ctx);
      const nodes  = Array .from (ids, id => id === 0xffffffff ? null : nodeObject (handle, id));

      if (!this .#rootNodes)
         this .#rootNodes = new Fields .MFNode ();

      this .#rootNodes .assign (new Fields .MFNode (... nodes));

      return this .#rootNodes;
   }

   getRootNodes () { return this .rootNodes; }

   #named ()
   {
      const handle = this .#handle ();

      return Array .from (engineOf (handle) .ctxNamedNodes (handle .scene, this .#ctx), id => nodeObject (handle, id));
   }

   get namedNodes () { return this .getNamedNodes (); }

   getNamedNodes ()
   {
      return this .#namedNodes ??= new NamedNodesArray ([ ], () => this .#named ());
   }

   getNamedNode (name)
   {
      name = String (name);

      const node = this .#named () .find (node => node .getNodeName () === name);

      if (node)
         return node;

      throw new Error (`Unknown named node '${name}'.`);
   }

   toVRMLString ()  { return this .#print ("vrml"); }
   toXMLString ()   { return this .#print ("xml"); }
   toJSONString ()  { return this .#print ("json"); }

   #print (format)
   {
      const handle = this .#handle ();

      const text = engineOf (handle) .ctxString (handle .scene, this .#ctx, format);

      // The engine prints a context as a scene: the file header (identifier, PROFILE, COMPONENT, UNIT, META) is not the body's.
      return format === "vrml" ? text .replace (/^(?:(?:#X3D|PROFILE|COMPONENT|UNIT|META)[^\n]*\n|\n)*/, "") : text;
   }
}

for (const key of Object .getOwnPropertyNames (X3DExecutionContext .prototype))
   Object .defineProperty (X3DExecutionContext .prototype, key, { enumerable: false });

class X3DProtoDeclaration extends X3DProtoDeclarationNode
{
   #body = null;

   constructor (scene, index) { super (scene, false, index); }

   getTypeName ()         { return "X3DProtoDeclaration"; }
   getType ()             { return [ X3DConstants .X3DProtoDeclarationNode, X3DConstants .X3DProtoDeclaration ]; }
   getProtoDeclaration () { return this; }

   getBody ()
   {
      const handle = this [_node];
      const ctx    = engineOf (handle) .declBody (handle .scene, this .index);

      return this .#body ??= new X3DExecutionContext (this, ctx);
   }
}

class X3DExternProtoDeclaration extends X3DProtoDeclarationNode
{
   constructor (scene, index) { super (scene, true, index); }

   getTypeName () { return "X3DExternProtoDeclaration"; }
   getType ()     { return [ X3DConstants .X3DProtoDeclarationNode, X3DConstants .X3DUrlObject, X3DConstants .X3DExternProtoDeclaration ]; }

   get urls ()      { return new Fields .MFString (... this .info () .urls); }
   get loadState () { return X3DConstants [this .info () .loadState]; }

   getProtoDeclaration ()
   {
      const loaded = this .info () .loaded;

      return loaded === undefined ? undefined : declarationOf (this .scene, false, loaded);
   }

   getAppInfo ()            { return this .getProtoDeclaration () ?.getAppInfo () || super .getAppInfo (); }
   getDocumentation ()      { return this .getProtoDeclaration () ?.getDocumentation () || super .getDocumentation (); }

   setAppInfo (value)       { const proto = this .getProtoDeclaration (); if (proto) proto .setAppInfo (value); else super .setAppInfo (value); }
   setDocumentation (value) { const proto = this .getProtoDeclaration (); if (proto) proto .setDocumentation (value); else super .setDocumentation (value); }

   // loadNow () loads again (the state is IN_PROGRESS at once); unloadNow () takes the file away.
   async loadNow () { this .#op ("loadNow"); }
   unloadNow ()     { this .#op ("unloadNow"); }

   #op (what)
   {
      const handle = this [_node];

      engineOf (handle) .externUrlOp (handle .scene, this .index, what, [ ], 0);
   }
}

for (const type of [ X3DProtoDeclarationNode, X3DProtoDeclaration, X3DExternProtoDeclaration ])
{
   for (const key of Object .getOwnPropertyNames (type .prototype))
   {
      if (key !== "constructor" && !/^(isExternProto|name|fields|appInfo|documentation|urls|loadState)$/ .test (key))
         Object .defineProperty (type .prototype, key, { enumerable: false });
   }
}

Object .defineProperty (X3DProtoDeclaration, "typeName", { value: "X3DProtoDeclaration", enumerable: true });
Object .defineProperty (X3DExternProtoDeclaration, "typeName", { value: "X3DExternProtoDeclaration", enumerable: true });

// The declaration object of (scene, kind, index), made once.
function declarationOf (scene, extern, index)
{
   let cache = declCache .get (scene);

   if (!cache)
      declCache .set (scene, cache = new Map ());

   const key = `${extern ? "e" : "p"}${index}`;

   let declaration = cache .get (key);

   if (!declaration)
      cache .set (key, declaration = extern ? new X3DExternProtoDeclaration (scene, index) : new X3DProtoDeclaration (scene, index));

   return declaration;
}

// The live arrays of a scene's declarations, keyed by the name the declaration has now.
class ProtoDeclarationArray extends X3DInfoArray
{
   constructor (values = [ ], list = null)
   {
      const array = super ([ ], X3DProtoDeclaration);

      if (list)
         liveDeclarationArray (array, list);

      return array;
   }

}

class ExternProtoDeclarationArray extends X3DInfoArray
{
   constructor (values = [ ], list = null)
   {
      const array = super ([ ], X3DExternProtoDeclaration);

      if (list)
         liveDeclarationArray (array, list);

      return array;
   }

}

function liveDeclarationArray (array, list)
{
   const read = () =>
   {
      const values = list ();
      const index  = new Map ();

      for (const value of values)
         index .set (value .name, value);

      return [ values, index ];
   };

   Object .defineProperty (array, _infoArray, { get: () => read () [0], set () { } });
   Object .defineProperty (array, _infoIndex, { get: () => read () [1], set () { } });

   return array;
}

for (const type of [ ProtoDeclarationArray, ExternProtoDeclarationArray ])
   Object .defineProperty (type, "typeName", { value: type .name, enumerable: true });

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

   getBrowser () { return this .#browser; }

   // X3DExecutionContext: the PROTO and EXTERNPROTO declarations (issue #137), live arrays that hold one object per declaration.
   #protos       = null;
   #externprotos = null;

   #declarations (extern)
   {
      const handle = this .#handle;

      return handle .scene === null ? [ ] : Array .from (engineOf (handle) .declIds (handle .scene, extern), index => declarationOf (this, extern, index));
   }

   get protos ()       { return this .#protos ??= new ProtoDeclarationArray ([ ], () => this .#declarations (false)); }
   get externprotos () { return this .#externprotos ??= new ExternProtoDeclarationArray ([ ], () => this .#declarations (true)); }

   getProtoDeclarations ()       { return this .protos; }
   getExternProtoDeclarations () { return this .externprotos; }

   #declarationOf (extern, name)
   {
      const engine = this .#engine ();

      if (!engine)
         throw new Error (`${extern ? "Extern proto" : "Proto"} declaration '${name}' not found.`);

      try { return declarationOf (this, extern, engine .declGet (this .#handle .scene, extern, String (name))); }
      catch (error) { throw engineError (error); }
   }

   getProtoDeclaration (name)       { return this .#declarationOf (false, name); }
   getExternProtoDeclaration (name) { return this .#declarationOf (true, name); }

   // The index a declaration argument stands for: -1 for something that is no declaration of that kind, -2 for one of another scene.
   #declOp (extern, op, name, declaration)
   {
      const type   = extern ? X3DExternProtoDeclaration : X3DProtoDeclaration;
      const engine = this .#engine ();
      const decl   = declaration instanceof type ? (declaration .scene === this && declaration .scene [_node] .generation === this .#handle .generation ? declaration .index : -2) : -1;

      if (!engine)
         throw new Error ("X_ITE_R: the browser is not ready; await browser .ready () first.");

      try { engine .declOp (this .#handle .scene, extern, op, String (name), decl); }
      catch (error) { throw engineError (error); }
      finally
      {
         // The warnings of an add or update (a name that a built-in node has, an EXTERNPROTO it overrides).
         for (const line of engine .takeLog (this .#handle .scene))
            printLogLine (line);
      }
   }

   addProtoDeclaration (name, proto)             { this .#declOp (false, "add", name, proto); }
   updateProtoDeclaration (name, proto)          { this .#declOp (false, "update", name, proto); }
   removeProtoDeclaration (name)                 { this .#declOp (false, "remove", name, undefined); }
   addExternProtoDeclaration (name, externproto)    { this .#declOp (true, "add", name, externproto); }
   updateExternProtoDeclaration (name, externproto) { this .#declOp (true, "update", name, externproto); }
   removeExternProtoDeclaration (name)              { this .#declOp (true, "remove", name, undefined); }

   getUniqueProtoName (name = "")       { return this .#uniqueName (false, name); }
   getUniqueExternProtoName (name = "") { return this .#uniqueName (true, name); }

   #uniqueName (extern, name)
   {
      const engine = this .#engine ();

      return engine ? engine .declUniqueName (this .#handle .scene, extern, String (name)) : String (name);
   }

   get worldURL () { return this .#engine () ?.worldURL (this .#handle .scene) ?? this .#browser .baseURL; }
   getWorldURL () { return this .worldURL; }

   // X3DScene.encoding: the format the scene was parsed from (`VRML`, `XML`, `JSON`, `GLTF`, ...), `SCRIPTED` for a scene
   // that the page made. (`setEncoding` is not implemented.)
   get encoding () { return this .#engine () ?.encoding (this .#handle .scene) ?? "SCRIPTED"; }
   getEncoding () { return this .encoding; }

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

   // X3DExecutionContext.createProto (name): an instance of a PROTO or EXTERNPROTO the scene declares, not part of the scene
   // until it is added. 
   createProto (name)
   {
      name = String (name);

      const engine = this .#engine ();

      if (!engine)
         throw new Error ("X_ITE_R: the browser is not ready; await browser .ready () first.");

      const id = engine .createProto (this .#handle .scene, name);

      if (id === undefined)
         throw new Error (`Unknown proto or externproto type '${name}'.`);

      return nodeObject (this .#handle, id);
   }

   // X3DExecutionContext.createNode (typeName): a new node of a concrete node type, made in this scene and not part of it until
   // it is added (a root node, or the value of a node's field). As in X_ITE, a node type of an external component is known once
   // the component is loaded, which a world's PROFILE or COMPONENT statement or a node of the component does (X3DConstants).
   createNode (typeName)
   {
      const name = String (typeName);

      if (!Object .hasOwn (NODE_TYPES, name) || (componentOf .has (name) && !Object .hasOwn (X3DConstants, name)))
         throw new Error (`Unknown node type '${name}'.`);

      const engine = this .#engine ();

      if (!engine)
         throw new Error ("X_ITE_R: the browser is not ready; await browser .ready () first.");

      const id = Number (engine .createVrmlFromString (this .#handle .scene, `${name} { }`, this .#browser .baseURL) [0]);

      // The VRML parser keeps no root node of a type like GeoOrigin, which only another node holds.
      if (id < 0)
         throw new Error (`X_ITE_R: a ${name} node cannot be made on its own.`);

      return nodeObject (this .#handle, id);
   }

   getNamedNode (name)
   {
      name = String (name);

      const id = this .#engine () ?.namedNode (this .#handle .scene, name);

      if (id === undefined)
         throw new Error (`Named node '${name}' not found.`);

      return nodeObject (this .#handle, id);
   }

   // The checks that addNamedNode and updateNamedNode share, in X_ITE's order (X3DExecutionContext.js): the node must be an
   // X3DNode of this scene, and the name must not be empty. Returns the node's handle.
   #namedNodeArguments (verb, name, node)
   {
      const handle = handleOf (node);

      if (!handle)
         throw new Error (`Couldn't ${verb} named node: node must be of type X3DNode.`);

      if (handle .scene !== this .#handle .scene || handle .browser !== this .#handle .browser)
         throw new Error (`Couldn't ${verb} named node: node does not belong to this execution context.`);

      if (!name)
         throw new Error (`Couldn't ${verb} named node: node name is empty.`);

      return handle;
   }

   // X3DExecutionContext.addNamedNode (name, node): a node that has no name gets one.
   addNamedNode (name, node)
   {
      name = String (name);

      const handle = this .#namedNodeArguments ("add", name, node);
      const engine = this .#engine ();

      if (engine .namedNode (this .#handle .scene, name) !== undefined)
         throw new Error (`Couldn't add named node: node named '${name}' is already in use.`);

      const own = engine .nodeName (this .#handle .scene, handle .id) ?? "";

      if (own && engine .namedNode (this .#handle .scene, own) === handle .id)
         throw new Error (`Couldn't add named node: node named '${own}' already has a name.`);

      engine .setNamedNode (this .#handle .scene, name, handle .id);
   }

   // X3DExecutionContext.updateNamedNode (name, node): the node gets the name; the node that had it loses it.
   updateNamedNode (name, node)
   {
      name = String (name);

      const handle = this .#namedNodeArguments ("update", name, node);

      this .#engine () .setNamedNode (this .#handle .scene, name, handle .id);
   }

   // X3DExecutionContext.removeNamedNode (name): the node of that name loses it; an unknown name does nothing.
   removeNamedNode (name)
   {
      this .#engine () ?.removeNamedNode (this .#handle .scene, String (name));
   }

   // X3DExecutionContext.getNamedNodes (): the live NamedNodesArray of the scene, the same object each time.
   #namedNodes    = null;
   #exportedNodes = null;

   getNamedNodes ()
   {
      return this .#namedNodes ??= new NamedNodesArray ([ ], () =>
         Array .from (this .#engine () ?.namedNodeList (this .#handle .scene) ?? [ ], id => nodeObject (this .#handle, id)));
   }

   get namedNodes () { return this .getNamedNodes (); }

   // X3DScene.getExportedNodes (): the live ExportedNodesArray; an X3DExportedNode lasts until its name is replaced or removed.
   getExportedNodes ()
   {
      return this .#exportedNodes ??= new ExportedNodesArray ([ ], () =>
      {
         const cache = exportedNodeCache .get (this) ?? new Map ();
         const next  = new Map ();

         for (const [ name, local, description ] of this .#engine () ?.exportedNodeList (this .#handle .scene) ?? [ ])
         {
            const old = cache .get (name);

            next .set (name, old ?? new X3DExportedNode (this, this .#handle, name, local, description));
         }

         exportedNodeCache .set (this, next);

         return [ ... next .values () ];
      });
   }

   get exportedNodes () { return this .getExportedNodes (); }

   // X3DScene.addExportedNode (exportedName, node, description = ""): the name must be free; then it is updateExportedNode.
   addExportedNode (exportedName, node, description = "")
   {
      exportedName = String (exportedName);

      if (this .#engine () ?.exportedNode (this .#handle .scene, exportedName) !== undefined)
         throw new Error (`Couldn't add exported node: exported name '${exportedName}' already in use.`);

      this .updateExportedNode (exportedName, node, description);
   }

   // X3DScene.updateExportedNode (exportedName, node, description = ""), checks in X_ITE's order (the page cannot hold an
   // imported node instance, so that check is the engine's alone).
   updateExportedNode (exportedName, node, description = "")
   {
      exportedName = String (exportedName);
      description  = String (description);

      const handle = handleOf (node);

      if (!handle)
         throw new Error ("Couldn't update exported node: node must be of type X3DNode.");

      if (handle .scene !== this .#handle .scene || handle .browser !== this .#handle .browser)
         throw new Error ("Couldn't update exported node: node does not belong to this execution context.");

      if (!exportedName)
         throw new Error ("Couldn't update exported node: node exported name is empty.");

      this .#engine () .setExportedNode (this .#handle .scene, exportedName, handle .id, description);

      // X_ITE makes a new X3DExportedNode for each update.
      exportedNodeCache .get (this) ?.delete (exportedName);
   }

   // X3DScene.removeExportedNode (exportedName): an unknown name does nothing.
   removeExportedNode (exportedName)
   {
      exportedName = String (exportedName);

      this .#engine () ?.removeExportedNode (this .#handle .scene, exportedName);
      exportedNodeCache .get (this) ?.delete (exportedName);
   }

   // X3DScene.getImportedNodes (): the live ImportedNodesArray. An X3DImportedNode lasts until its name is replaced or removed;
   // a rename keeps it.
   #importedNodes = null;

   getImportedNodes ()
   {
      return this .#importedNodes ??= new ImportedNodesArray ([ ], () =>
      {
         const cache = importedNodeCache .get (this) ?? new Map ();
         const next  = new Map ();

         for (const [ name, inline, exported, description, instance ] of this .#engine () ?.importedNodeList (this .#handle .scene) ?? [ ])
         {
            const old = [ ... cache .values () ] .find (item => item .instanceId === instance);

            if (old && old .importedName !== name)
               old .rename (name);

            next .set (name, old ?? new X3DImportedNode (this, this .#handle, inline, exported, name, instance, description));
         }

         importedNodeCache .set (this, next);

         return [ ... next .values () ];
      });
   }

   get importedNodes () { return this .getImportedNodes (); }

   // X3DExecutionContext.addImportedNode (inlineNode, exportedName, importedName = exportedName, description = ""): the imported
   // name must be free; then it is updateImportedNode.
   addImportedNode (inlineNode, exportedName, importedName = exportedName, description = "")
   {
      exportedName = String (exportedName);
      importedName = String (importedName);

      if (this .#engine () ?.importedInstance (this .#handle .scene, importedName) !== undefined)
         throw new Error (`Couldn't add imported node: imported name '${importedName}' already in use.`);

      this .updateImportedNode (inlineNode, exportedName, importedName, description);
   }

   // X3DExecutionContext.updateImportedNode (inlineNode, exportedName, importedName, description = ""), checks in X_ITE's
   // order; the record of the imported name is replaced and goes to the end of the list.
   updateImportedNode (inlineNode, exportedName, importedName, description = "")
   {
      const handle = handleOf (inlineNode);

      exportedName = String (exportedName);
      importedName = importedName === undefined ? exportedName : String (importedName);
      description  = String (description);

      if (!handle || engineOf (handle) .typeName (handle .scene, handle .id) !== "Inline")
         throw new Error ("Node must be of type Inline node.");

      if (handle .scene !== this .#handle .scene || handle .browser !== this .#handle .browser)
         throw new Error ("Couldn't update imported node: Inline node does not belong to this execution context.");

      if (!exportedName)
         throw new Error ("Couldn't update imported node: exported name is empty.");

      if (!importedName)
         throw new Error ("Couldn't update imported node: imported name is empty.");

      this .#engine () .setImportedNode (this .#handle .scene, handle .id, exportedName, importedName, description);

      importedNodeCache .get (this) ?.delete (importedName);
   }

   // X3DExecutionContext.renameImportedNode (oldImportedName, newImportedName): the X3DImportedNode stays.
   renameImportedNode (oldImportedName, newImportedName)
   {
      // X_ITE looks the old name up as given: only the text of a name finds one.
      if (typeof oldImportedName !== "string")
         throw new Error ("Couldn't rename imported node: imported node does not exists.");

      try
      {
         this .#engine () .renameImportedNode (this .#handle .scene, oldImportedName, String (newImportedName));
      }
      catch (error)
      {
         throw pageError (error);
      }

      this .getImportedNodes () .length; // moves the cached X3DImportedNode to its new name
   }

   // X3DExecutionContext.removeImportedNode (importedName): an unknown name does nothing.
   removeImportedNode (importedName)
   {
      importedName = String (importedName);

      this .#engine () ?.removeImportedNode (this .#handle .scene, importedName);
      importedNodeCache .get (this) ?.delete (importedName);
   }

   // X3DExecutionContext.getImportedNode (importedName): the instance node, or an Error.
   getImportedNode (importedName)
   {
      importedName = String (importedName);

      const id = this .#engine () ?.importedInstance (this .#handle .scene, importedName);

      if (id === undefined)
         throw new Error (`Imported node '${importedName}' not found.`);

      return nodeObject (this .#handle, id);
   }

   getUniqueImportName (name)
   {
      return this .#engine () ?.uniqueImportName (this .#handle .scene, String (name)) ?? String (name);
   }

   // X3DExecutionContext.getLocalNode (name): a named node, else the instance of an imported node.
   getLocalNode (name)
   {
      name = String (name);

      const engine = this .#engine ();

      const named = engine ?.namedNode (this .#handle .scene, name);

      if (named !== undefined)
         return nodeObject (this .#handle, named);

      const imported = engine ?.importedInstance (this .#handle .scene, name);

      if (imported !== undefined)
         return nodeObject (this .#handle, imported);

      throw new Error (`Unknown named or imported node '${name}'.`);
   }

   // X3DScene.getExportedNode (exportedName): the local node, or an Error.
   getExportedNode (exportedName)
   {
      exportedName = String (exportedName);

      const id = this .#engine () ?.exportedNode (this .#handle .scene, exportedName);

      if (id === undefined)
         throw new Error (`Exported node '${exportedName}' not found.`);

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

// The rendering properties that are the value of a browser option (LogarithmicDepthBuffer). Shading, Antialiased,
// ContentScale and Multisampling are derived from the options and the context (#derivedRendering); the rest describe
// X_ITE's WebGL context and are not implemented.
const RENDERING_OPTIONS     = [ "LogarithmicDepthBuffer" ];
const RENDERING_DERIVED     = [ "Shading", "Antialiased", "AntiAliased", "ContentScale", "Multisampling" ];
const RENDERING_UNSUPPORTED = [ "MaxTextureSize", "TextureUnits", "MaxLights", "ColorDepth", "TextureMemory", "MaxAnisotropicDegree", "MaxSamples", "PixelsPerPoint", "SoundDestinations", "XRSession" ];

// Shading.js: the names of the shading option; "POINTSET" is "POINT", any other text is GOURAUD.
const SHADINGS = [ "POINT", "WIREFRAME", "FLAT", "GOURAUD", "PHONG" ];

// --- The browser's own user interface (issue #141) -----------------------------------------------------------------
// Browser/Core/ContextMenu.js, BrowserTimings.js and the splash screen of X3DCoreContext.js and X3DNetworkingContext.js of
// X_ITE 16.5.0, in the shadow DOM of the element. The menu is X_ITE's own (it does not use the jQuery contextMenu library): a
// `contextmenu` event of the element builds it, a mousedown or contextmenu on its layer closes it, an entry that is not a
// checkbox or radio closes it when it is clicked. There are no menu keys. The rules are those of x_ite.css with the light
// theme's variables; the images (logo, icons, backgrounds) are assets and are not drawn, so the checkbox and radio marks are
// CSS shapes (API.md, "Known differences").

const UI_CSS = String.raw`
:host { --red: #e12b2b; --orange: #ff9230; --text-color: black; --text-color-dark: black; --text-shadow-color: white; --link-color: var(--red);
  --progress-background: rgb(0, 0, 0); --splash-screen-background: rgb(252, 252, 252);
  --gradient: linear-gradient(to bottom, rgba(247, 247, 252, 0.642) 0%, rgba(222, 221, 226, 0.6) 100%);
  --border: 4px solid rgba(229, 229, 234, 0.66); --button-border: 1px solid rgba(255, 255, 255, 0.49); --tint-color: rgba(255, 255, 255, 0.37);
  --tint-hover-color: rgba(255, 255, 255, 0.46); --table-tint-color: var(--tint-color); --icon-filter: invert(100%); }
.x_ite-private-browser { position: relative; overflow: hidden; height: 100%; outline: none; }
.x_ite-private-menu, .x_ite-private-menu * { all: revert; }

.x_ite-private-browser-timings {
  position: absolute;
  bottom: 0px;
  left: 0px;
  backdrop-filter: blur(8px);
  background: var(--gradient);
  background-clip: padding-box;
  border-top: var(--border);
  border-right: var(--border);
  border-top-right-radius: 12px;
  padding: 8px 12px 10px 12px;
  color: var(--text-color);
  font-family: X_ITE PT Sans, sans-serif;
  font-size: 9pt;
  white-space: pre;
  pointer-events: none;
}

.x_ite-private-browser-timings :is(table, thead, tbody, tfoot, tr, th, td) {
  box-sizing: border-box;
  color: var(--text-color);
  font-size: inherit;
  line-height: 1.1;
  text-align: left;
}

.x_ite-private-browser-timings table {
  overflow: hidden;
  width: 100%;
  max-width: 100%;
  border-collapse: separate;
  border-spacing: 2px;
  empty-cells: show;
}

.x_ite-private-browser-timings thead th {
  padding-top: 3px;
  background-size: 17px 17px;
  padding-left: 20px;
  padding-bottom: 5px;
  font-weight: bold;
  color: var(--text-color-dark);
  filter: var(--icon-filter);
}

.x_ite-private-browser-timings td:first-child {
  padding-right: 10px;
}

.x_ite-private-browser-timings table.more tr.x_ite-private-more td {
  padding-bottom: 5px;
}

.x_ite-private-browser-timings table.less tr.x_ite-private-more~tr {
  display: none;
}

.x_ite-private-browser-timings td {
  padding-bottom: 2px;
}

.x_ite-private-browser-timings td.pointer {
  pointer-events: all;
}

.x_ite-private-browser-timings tfoot td:first-child {
  padding-right: 0;
}

.x_ite-private-browser-timings tfoot td {
  padding-top: 4px;
}

.x_ite-private-browser-timings button {
  box-sizing: border-box;
  border: var(--button-border);
  border-radius: 10px;
  width: 100%;
  height: 14pt;
  margin-top: 2px;
  padding: 0;
  background: var(--tint-color);
  color: var(--text-color);
  font-family: X_ITE PT Sans, sans-serif;
  font-style: italic;
  font-size: 9pt;
  line-height: 1.1;
  text-align: center;
  pointer-events: all;
  cursor: pointer;
}

.x_ite-private-browser-timings button:hover {
  background-color: var(--tint-hover-color);
}

/* World Info */
.x_ite-private-world-info-overlay {
  position: absolute;
  inset: 0;
}

.x_ite-private-world-info {
  overflow-y: auto;
  position: absolute;
  inset: 0;
  margin: 21pt;
  backdrop-filter: blur(8px);
  background: var(--gradient);
  background-clip: padding-box;
  border: var(--border);
  border-radius: 15px;
  padding: 21pt;
  padding-top: 0;
}

.x_ite-private-world-info .x_ite-private-world-info-buttons {
  border-bottom: 4px solid var(--table-tint-color);
  margin-bottom: 12pt;
}

.x_ite-private-world-info button {
  width: 100pt;
  height: 21pt;
  background: none;
  border: none;
  color: var(--text-color);
}

.x_ite-private-world-info button:hover {
  background-color: var(--table-tint-color);
}

.x_ite-private-world-info button.active {
  font-weight: bold;
}

.x_ite-private-world-info :is(.x_ite-private-world-info-top, .x_ite-private-world-info-title, .x_ite-private-world-info-info, p) {
  box-sizing: border-box;
  font-family: X_ITE PT Sans, sans-serif;
  font-style: normal;
  line-height: 1.3;
  text-align: left;
  color: var(--text-color);
}

.x_ite-private-world-info .x_ite-private-world-info-top {
  margin: 0.3em 0;
  font-weight: bold;
  font-style: italic;
  font-size: 9pt;
}

.x_ite-private-world-info .x_ite-private-world-info-title {
  margin: 0.3em 0;
  font-weight: bold;
  font-size: 16pt;
  overflow-wrap: break-word;
  word-break: break-word;
}

.x_ite-private-world-info .x_ite-private-world-info-info,
.x_ite-private-world-info .x_ite-private-world-info-info p {
  margin: 0.5em 0 0.5em 0;
  font-size: 11pt;
  overflow-wrap: break-word;
  word-break: break-word;
}

.x_ite-private-world-info table {
  margin: 0.5em 0;
  border-collapse: collapse;
  line-height: 1.3;
  width: 100%;
}

.x_ite-private-world-info tr:nth-child(odd) {
  background-color: var(--table-tint-color);
}

.x_ite-private-world-info td {
  padding: 4pt;
  vertical-align: top;
  font-family: X_ITE PT Sans, sans-serif;
  font-size: 10pt;
  color: var(--text-color);
  overflow-wrap: break-word;
  word-break: break-word;
}

.x_ite-private-world-info td:first-child {
  min-width: 70px;
}

.x_ite-private-world-info a {
  color: var(--link-color);
}

/* Context Menu */

.x_ite-private-menu.context-menu-layer {
  position: fixed;
  z-index: 10000;
  inset: 0;
}

.x_ite-private-menu.context-menu-list,
.x_ite-private-menu .context-menu-list {
  display: none;
  overscroll-behavior: none;
  box-sizing: border-box;
  min-width: 120px;
  margin: 0;
  margin-left: 1px;
  border: 4px solid transparent;
  border-radius: 12px;
  padding: 5px 10px 6px 9px;
  color: var(--text-color);
  font-family: X_ITE PT Sans, sans-serif;
  font-size: 10pt;
  white-space: pre;
  line-height: 1.2;
  list-style-type: none;
}

.x_ite-private-menu.context-menu-list {
  position: fixed;
  z-index: 10000;
  isolation: isolate;
}

.x_ite-private-menu .context-menu-list {
  position: absolute;
  top: -13px;
}

.x_ite-private-menu .context-menu-background {
  position: absolute;
  inset: 0;
  box-sizing: border-box;
  min-width: 120px;
  margin: 0;
  margin-left: 1px;
  outline: none;
  border: var(--border);
  border-radius: 12px;
  padding: 5px 10px 9px 9px;
  background: var(--gradient);
  backdrop-filter: blur(8px);
  background-clip: padding-box;
}

.x_ite-private-menu .context-menu-item {
  cursor: pointer;
  position: relative;
  border-radius: 12px;
  margin: 0;
  padding: 0px 2px;
  font-size: 10pt;
  line-height: 1;
  text-align: left;
}

.x_ite-private-menu .context-menu-item {
  margin: 0 4px 0 4px;
}

.x_ite-private-menu .context-menu-item:first-of-type {
  margin-top: 4px;
}

.x_ite-private-menu .context-menu-item:last-of-type {
  margin-bottom: 4px;
}

.x_ite-private-menu .context-menu-item:hover:not(.context-menu-not-selectable) {
  background-color: var(--red);
}

.x_ite-private-menu .context-menu-item:hover > .context-menu-list {
  display: block;
}

.x_ite-private-menu .context-menu-not-selectable {
  cursor: default;
}

.x_ite-private-menu .context-menu-separator {
  cursor: default;
  margin: 2px 0;
  border-top: 0;
  border-bottom: 1px solid rgb(138, 138, 138);
  padding-top: 0;
  padding-bottom: 0;
}

.x_ite-private-menu .context-menu-separator+.context-menu-separator {
  display: none !important;
}

.x_ite-private-menu .context-menu-submenu:after {
  content: "";
  position: absolute;
  display: block;
  right: 4px;
  top: 8px;
  margin-right: 2px;
  /* CSS triangle */
  border-style: solid;
  border-width: 4px 0 4px 5px;
  border-color: transparent transparent transparent #e8eaed; /* color from icons */
  filter: var(--icon-filter);
  width: 0;
  height: 0;
}

.x_ite-private-menu .context-menu-item > span {
  display: block;
  width: 100%;
  padding: 5px 24px;
}

.x_ite-private-menu label {
  cursor: pointer;
  display: block;
  box-sizing: border-box;
  margin: 0;
  padding: 5px 24px;
  width: 100%;
  font-size: 10pt;
  font-weight: normal;
  line-height: 1;
}

.x_ite-private-menu label span {
  color: var(--text-color-dark);
  filter: var(--icon-filter);
}

.x_ite-private-menu label:has(>input[type=radio]:checked) {
  font-weight: bold;
}

/* vertically align inside labels */
.x_ite-private-menu label > * {
  vertical-align: top;
  font-size: 10pt;
  line-height: 1;
}

.x_ite-private-menu input {
  cursor: pointer;
  appearance: none;
}

/* position checkboxes and radios like icons */
.x_ite-private-menu input[type=checkbox],
.x_ite-private-menu input[type=radio] {
  display: inline-block;
  position: absolute;
  top: 3px;
  width: 17px;
  height: 17px;
  margin: 0;
  margin-left: -22px;
  padding: 0;
  filter: var(--icon-filter);
}

.x_ite-private-menu input:checked {
  filter: var(--orange-filter);
}

.x_ite-private-menu .context-menu-input:hover input:checked {
  filter: var(--icon-filter);
}

.x_ite-private-menu input[type=checkbox]:checked {
}

.x_ite-private-menu input[type=radio]:checked {
}

.x_ite-private-menu span {
  padding-left: 0px;
}

/* Item styles */

.x_ite-private-menu .context-menu-title {
  font-weight: bold;
}

/* Icons */

.x_ite-private-menu .context-menu-icon > span {
  display: block;
  background-repeat: no-repeat;
  background-position: 2px 3px;
  color: var(--text-color-dark);
  filter: var(--icon-filter);
}

.x_ite-private-menu .x_ite-private-icon-logo > span {
  background-position: 2px 4px;
  filter: none;
  color: var(--text-color);
}

.x_ite-private-icon-viewpoint > span {
}

.x_ite-private-menu .x_ite-private-viewpoint input {
  background-image: none;
}

.x_ite-private-menu .x_ite-private-viewpoint input:checked {
}

.x_ite-private-icon-viewer > span {
}

.x_ite-private-menu .x_ite-private-examine-viewer :is(input, input:checked) {
}

.x_ite-private-menu .x_ite-private-walk-viewer :is(input, input:checked) {
}

.x_ite-private-menu .x_ite-private-fly-viewer :is(input, input:checked) {
}

.x_ite-private-menu .x_ite-private-plane-viewer :is(input, input:checked) {
}

.x_ite-private-menu .x_ite-private-lookat-viewer :is(input, input:checked) {
}

.x_ite-private-menu .x_ite-private-none-viewer :is(input, input:checked) {
}

.x_ite-private-icon-primitive-quality > span {
}

.x_ite-private-icon-texture-quality > span {
}

.x_ite-private-icon-shading > span {
}

.x_ite-private-icon-animations > span {
}

.x_ite-private-icon-play > span {
}

.x_ite-private-icon-stop > span {
}

.x_ite-private-icon-enter-fullscreen > span {
}

.x_ite-private-icon-leave-fullscreen > span {
}

.x_ite-private-icon-world-info > span {
}

.x_ite-private-icon-info > span {
}

/* Splash Screen and Spinner */

.x_ite-private-splash-screen {
  pointer-events: none;
  position: absolute;
  inset: 0;
  background: var(--splash-screen-background);
}

.x_ite-private-spinner {
  --offset-x: 100px;
  --offset-y: -48px;
  --size: 32px;
  --padding: 8px;
  --center-x: calc(458 / 1024 * var(--size) + var(--padding));
  --center-y: calc(539 / 1024 * var(--size) + var(--padding));
  position: absolute;
  top: calc(50% - (var(--size) / 2) + calc(var(--size) / 2 - var(--center-y)) + var(--offset-y));
  left: calc(50% - (var(--size) / 2) + calc(var(--size) / 2 - var(--center-x)) + var(--offset-x));
  width: var(--size);
  height: var(--size);
  padding: var(--padding);
  background-size: var(--size) var(--size);
  animation: x_ite-spinner-animation 8s linear infinite;
  transform-origin: var(--center-x) var(--center-y);
}

@keyframes x_ite-spinner-animation {
  100% {
    transform: rotate(360deg);
  }
}

.x_ite-private-progress {
  position: absolute;
  top: calc(50% - 28pt - 11pt);
  left: calc(50% - 100px);
  width: 195px;
}

.x_ite-private-x_ite {
  color: var(--text-color);
  font-size: 32pt;
  text-align: center;
  text-shadow: 1px 1px 0 var(--text-shadow-color);
  font-family: X_ITE PT Sans, sans-serif;
}

.x_ite-private-x_ite b {
  font-family: X_ITE PT Sans, sans-serif;
  font-weight: bold;
}

.x_ite-private-spinner-text {
  color: var(--text-color);
  font-size: 11pt;
  text-align: center;
  text-shadow: 1px 1px 0 var(--text-shadow-color);
  font-family: X_ITE PT Sans, sans-serif;
}

.x_ite-private-progressbar {
  overflow: hidden;
  margin: 8px;
  border-radius: 4px;
  background: var(--progress-background);
  height: 8px;
}

.x_ite-private-progressbar div {
  width: 0%;
  height: 100%;
  background: var(--red);
}

/* Button Bar [No Sound, WebXR] */

.x_ite-private-buttons {
  display: flex;
  gap: 16px;
  justify-content: center; /* Zentriert horizontal (Hauptachse) */
  align-items: center;     /* Zentriert vertikal (Querachse) */
  position: absolute;
  padding: 0px 24px 0px 16px;
  border: var(--border);
  border-radius: 32px;
  right: -16px;
  bottom: 10px;
  background: var(--gradient);
  background-clip: padding-box;
}

.x_ite-private-buttons:is(:empty, :not(:has(:not(.x_ite-private-hidden)))) {
  display: none;
  opacity: 0;
}

.x_ite-private-button {
  background-repeat: no-repeat;
  cursor: pointer;
  width: 30px;
  height: 30px;
  filter: var(--icon-filter);
}

.x_ite-private-button:hover {
  scale: 1.2;
}

.x_ite-private-no-sound-button {
  order: 1;
}

.x_ite-private-xr-button {
  order: 2;
}

/* Animations */

.x_ite-private-fade {
  opacity: 1;
  transition: display 300ms allow-discrete, opacity 300ms ease-in-out;
}

@starting-style {
  .x_ite-private-fade {
    opacity: 0;
  }
}

.x_ite-private-hidden {
  display: none;
  opacity: 0;
}

.x_ite-private-splash-screen.x_ite-private-fade {
  transition: none;
}

.x_ite-private-splash-screen.x_ite-private-hidden {
  transition: display 200ms allow-discrete, opacity 200ms ease-in-out;
}

.x_ite-private-button.x_ite-private-fade {
  transition: display 0s 300ms allow-discrete, opacity 300ms ease-in-out, scale 200ms ease-in-out;
}

/* Firefox specific animation rules, see:
 * https://bugzilla.mozilla.org/show_bug.cgi?id=1882408
 */

/* X_ITE_R: the images are not drawn; the spinner, the marks of checkboxes and radios are CSS shapes. */
.x_ite-private-spinner { border: 3px solid transparent; border-top-color: var(--red); border-radius: 50%; box-sizing: content-box; }
.x_ite-private-menu input[type=checkbox], .x_ite-private-menu input[type=radio] { border: 2px solid var(--text-color); border-radius: 50%; box-sizing: border-box; width: 14px; height: 14px; margin-left: -20px; top: 5px; }
.x_ite-private-menu input[type=checkbox] { border-radius: 3px; }
.x_ite-private-menu input:checked { background: var(--orange); }
`;

const f0 = (() => { const format = new Intl .NumberFormat (navigator .language || navigator .userLanguage, { notation: "standard", minimumFractionDigits: 0, maximumFractionDigits: 0 }) .format; return value => format (Number .isFinite (value) ? value : 0); })();
const f2 = (() => { const format = new Intl .NumberFormat (navigator .language || navigator .userLanguage, { notation: "standard", minimumFractionDigits: 2, maximumFractionDigits: 2 }) .format; return value => format (Number .isFinite (value) ? value : 0); })();

// locale/gettext.js: X_ITE_R has the English texts only.
const _ = text => text;

class BrowserUI
{
   #browser;
   #host;
   #root;
   #wrapper;
   #disposed = false;
   #timers = new Set ();

   // Splash screen.
   #splash;
   #loading = false;

   // BrowserTimings.
   #timings;
   #timingsParts;
   // BrowserTimings.initialize: the type is flipped once before the first set_type__, so the panel starts as LESS.
   #timingsType = "MORE";
   #timingsOn = false;
   #timingsLast = null;
   #timingsSum = 0;
   #timingsCount = 0;
   #timingsSystem = 0;
   #timingsEngine = 0;
   #timingsReset = 0;

   // ContextMenu.
   #hide = null;
   #userMenu = null;
   #contextMenu;

   // `host`: { engine (), viewpoints (), bind (id), activeViewpoint (), animations () }, the page-side access to the engine.
   constructor (browser, host, root, wrapper, canvas)
   {
      this .#browser = browser;
      this .#host    = host;
      this .#root    = root;
      this .#wrapper = wrapper;

      // X3DCoreContext: the splash screen, a sibling of the surface.
      wrapper .insertAdjacentHTML ("beforeend", /* html */ `
<div class="x_ite-private-splash-screen x_ite-private-fade x_ite-private-hidden">
   <div class="x_ite-private-spinner"></div>
   <div class="x_ite-private-progress">
      <div class="x_ite-private-x_ite">
            ${browser .getName ()}<b>X3D</b>
         </div>
      <div class="x_ite-private-progressbar"><div></div></div>
      <div class="x_ite-private-spinner-text"></div>
   </div>
</div>`);

      this .#splash = wrapper .lastElementChild;

      this .#initializeTimings ();

      const menu = this;

      // The ContextMenu node of X_ITE: the user's menu and `hide ()`.
      this .#contextMenu = Object .create (null, { });
      Object .defineProperties (this .#contextMenu,
      {
         getUserMenu: { value: () => this .#userMenu },
         setUserMenu: { value: userMenu => { this .#userMenu = userMenu; } },
         userMenu:    { get: () => this .#userMenu, set: userMenu => { this .#userMenu = userMenu; }, enumerable: true },
         hide:        { value: () => menu .hideMenu () },
         getTypeName: { value: () => "ContextMenu" },
      });
   }

   get contextMenu () { return this .#contextMenu; }

   #later (callback, delay)
   {
      const id = setTimeout (() => { this .#timers .delete (id); callback (); }, delay);

      this .#timers .add (id);
      return id;
   }

   // --- splash screen ---------------------------------------------------------------------------------------------

   getSplashScreen () { return this .#splash; }

   // X3DNetworkingContext.setBrowserLoading.
   setBrowserLoading (value)
   {
      if (this .#disposed)
         return;

      this .#loading = value;
      this .#loadingDisplay = 0;

      const canvas = this .#browser .getCanvas ();

      if (value)
      {
         this .#root .querySelector (".x_ite-private-world-info") ?.remove ();

         this .#setLoadText ("Loading 1 file", 0);

         if (this .#browser .getBrowserOption ("SplashScreen"))
         {
            this .hideMenu ();
            canvas .style .display = "none";
            this .#splash .classList .remove ("x_ite-private-hidden");
         }
      }
      else
      {
         if (this .#browser .getBrowserOption ("SplashScreen"))
         {
            canvas .style .display = "block";

            // Defer until promises are resolved.
            this .#later (() =>
            {
               if (!this .#loading)
                  this .#splash .classList .add ("x_ite-private-hidden");
            }, 0);
         }
      }
   }

   // X3DNetworkingContext [_set_loadCount] with the browser not loading: each change of the display load count (the engine's
   // loading objects: Inlines, textures, Scripts, EXTERNPROTOs) says "Loading n file(s)", or "Loading done" at zero, as the
   // browser's description. While the browser loads the splash screen shows the text instead, and resetLoadCount starts again.
   #loadingDisplay = 0;

   loadingCounts (counts)
   {
      for (const count of counts)
      {
         if (this .#loading || count === this .#loadingDisplay)
            continue;

         this .#loadingDisplay = count;
         this .#browser .setDescription (count ? (count === 1 ? _("Loading %1 file") : _("Loading %1 files")) .replace ("%1", count) : _("Loading done"));
      }
   }

   #setLoadText (text, percent)
   {
      this .#splash .querySelector (".x_ite-private-spinner-text") .textContent = text;
      this .#splash .querySelector (".x_ite-private-progressbar div") .style .width = `${percent}%`;
   }

   // X3DBrowser.loadURL, no scene: "Failed loading world." in the splash screen (the canvas shows if there is no splash screen).
   loadFailed ()
   {
      if (this .#disposed)
         return;

      if (!this .#browser .getBrowserOption ("SplashScreen"))
         this .#browser .getCanvas () .style .display = "block";

      this .#later (() => { this .#splash .querySelector (".x_ite-private-spinner-text") .textContent = _("Failed loading world."); }, 0);
   }

   // X3DBrowser.loadURL with a scene: the canvas shows at once if there is no splash screen.
   loadSucceeded ()
   {
      if (!this .#browser .getBrowserOption ("SplashScreen"))
         this .#browser .getCanvas () .style .display = "block";
   }

   // X3DCoreContext.attributeChangedCallback "splashscreen".
   splashScreenAttribute ()
   {
      if (!this .#browser .getBrowserOption ("SplashScreen"))
         this .#splash .classList .add ("x_ite-private-hidden");
   }

   // --- option changes --------------------------------------------------------------------------------------------

   optionChanged (name)
   {
      if (name === "Timings")
         this .#setTimingsEnabled ();
   }

   // --- BrowserTimings ----------------------------------------------------------------------------------------------

   #initializeTimings ()
   {
      this .#wrapper .insertAdjacentHTML ("beforeend", /* html */ `
<div class="x_ite-private-browser-timings x_ite-private-fade x_ite-private-hidden"><table>
   <thead>
      <tr>
         <th colspan="2">${_("Browser Timings")}</th>
      </tr>
   </thead>
   <tbody>
      <tr>
         <td>${_("Frame rate")}:</td>
         <td class="frame-rate"></td>
      </tr>
      <tr class="x_ite-private-more">
         <td>${_("Speed")}:</td>
         <td class="speed"></td>
      </tr>
      <tr>
         <td>${_("Browser")}:</td>
         <td class="browser-time"></td>
      </tr>
      <tr>
         <td>${_("X3D total")}:</td>
         <td class="x3d-total"></td>
      </tr>
      <tr>
         <td>${_("Event Processing")}:</td>
         <td class="event-processing"></td>
      </tr>
      <tr>
         <td>${_("Pointer")}:</td>
         <td class="pointer"></td>
      </tr>
      <tr>
         <td>${_("Camera")}:</td>
         <td class="camera"></td>
      </tr>
      <tr>
         <td>${_("Picking")}:</td>
         <td class="picking"></td>
      </tr>
      <tr>
         <td>${_("Collision Detection")}:</td>
         <td class="collision-detection"></td>
      </tr>
      <tr>
         <td>${_("Rendering")}:</td>
         <td class="rendering"></td>
      </tr>
      <tr>
         <td>${_("Number of Primitives")}:</td>
         <td class="primitives pointer" title="${_("Points; Lines; Triangles")}"></td>
      </tr>
      <tr>
         <td>${_("Number of Shapes")}:</td>
         <td class="shapes pointer" title="${_("Opaque Shapes + Transparent Shapes")}"></td>
      </tr>
      <tr>
         <td>${_("Number of Sensors")}:</td>
         <td class="sensors"></td>
      </tr>
   </tbody>
   <tfoot>
      <tr>
         <td colspan="2"><button></button></td>
      </tr>
   </tfoot>
</table></div>`);

      const element = this .#wrapper .lastElementChild;
      const q       = selector => element .querySelector (selector);

      this .#timings      = element;
      this .#timingsParts = {
         table: q ("table"), frameRate: q (".frame-rate"), speed: q (".speed"), browserTime: q (".browser-time"),
         x3dTotal: q (".x3d-total"), eventProcessing: q (".event-processing"), pointerTime: q (".pointer"), cameraTime: q (".camera"),
         pickingTime: q (".picking"), collisionTime: q (".collision-detection"), renderTime: q (".rendering"),
         numPrimitives: q (".primitives"), numShapes: q (".shapes"), sensors: q (".sensors"), button: q ("button"),
      };

      // BrowserTimings.initialize: the first type is LESS (X_ITE keeps the last one in localStorage; here it is the default).
      const { button } = this .#timingsParts;
      const signal     = this .#host .signal;

      // X_ITE: a click on the panel leaves the focus in the browser, so the keys keep working when the panel hides.
      button .addEventListener ("mousedown", event => { event .preventDefault (); this .#browser .getCanvas () .focus ({ preventScroll: true }); }, { signal });
      button .addEventListener ("click",    () => this .#setTimingsType (), { signal });
      // A tap gives touchend and then a click; X_ITE's panel changes once for a tap, so the touch is the tap here.
      button .addEventListener ("touchend", event => { event .preventDefault (); this .#setTimingsType (); }, { signal });

      this .#setTimingsType ();
   }

   #setTimingsEnabled ()
   {
      const element = this .#timings;

      if (this .#browser .getBrowserOption ("Timings"))
      {
         element .classList .remove ("x_ite-private-hidden");
         this .#timingsOn = true;
         this .#resetTimings ();
      }
      else
      {
         element .classList .add ("x_ite-private-hidden");
         this .#timingsOn = false;
         ++ this .#timingsReset;
      }
   }

   #setTimingsType ()
   {
      const { table, button } = this .#timingsParts;

      if (this .#timingsType === "MORE")
      {
         this .#timingsType = "LESS";
         table .classList .add ("less");
         table .classList .remove ("more");
         button .textContent = _("More Properties");
      }
      else
      {
         this .#timingsType = "MORE";
         table .classList .add ("more");
         table .classList .remove ("less");
         button .textContent = _("Less Properties");
      }

      this .#buildTimings ();
   }

   // BrowserTimings.reset: after the next frame the stop watch starts again.
   #resetTimings ()
   {
      const reset = ++ this .#timingsReset;

      requestAnimationFrame (() =>
      {
         if (this .#disposed || reset !== this .#timingsReset)
            return;

         this .#timingsLast = null; this .#timingsSum = 0; this .#timingsCount = 0; this .#timingsSystem = 0; this .#timingsEngine = 0;
         this .#buildTimings ();
      });
   }

   // BrowserTimings.update, at each frame of the frame loop: `systemMs` is the time of the frame in the page, `engineMs` the
   // time of the engine's frame (the stage that the page can measure).
   frame (stamp, systemMs, engineMs)
   {
      if (!this .#timingsOn)
         return;

      if (this .#timingsLast !== null)
      {
         this .#timingsSum += stamp - this .#timingsLast;
         ++ this .#timingsCount;
      }

      this .#timingsLast    = stamp;
      this .#timingsSystem += systemMs;
      this .#timingsEngine += engineMs;

      if (this .#timingsSum > 1000)
      {
         this .#buildTimings ();
         this .#timingsSum = 0; this .#timingsCount = 0; this .#timingsSystem = 0; this .#timingsEngine = 0;
      }
   }

   #buildTimings ()
   {
      const p = this .#timingsParts;

      if (this .#timingsCount)
      {
         p .frameRate .textContent = `${f2 (1000 / (this .#timingsSum / this .#timingsCount))} ${_("fps")}`;
      }
      else
      {
         p .frameRate .textContent = `${f2 (0)} ${_("fps")}`;
      }

      // The engine measures no speed, camera, picking, collision or counts: these rows show 0 (API.md).
      p .speed .textContent = `${f2 (0)} ${_("m/s")}`;

      if (this .#timingsType !== "MORE")
         return;

      const n = Math .max (this .#timingsCount, 1);

      p .browserTime     .textContent = `${f2 (this .#timingsSystem / n)} ${_("ms")}`;
      p .x3dTotal        .textContent = `${f2 (this .#timingsEngine / n)} ${_("ms")}`;
      p .eventProcessing .textContent = `${f2 (0)} ${_("ms")}`;
      p .pointerTime     .textContent = `${f2 (0)} ${_("ms")}`;
      p .cameraTime      .textContent = `${f2 (0)} ${_("ms")}`;
      p .pickingTime     .textContent = `${f2 (0)} ${_("ms")}`;
      p .collisionTime   .textContent = `${f2 (0)} ${_("ms")}`;
      p .renderTime      .textContent = `${f2 (0)} ${_("ms")}`;
      p .numPrimitives   .textContent = `${f0 (0)}; ${f0 (0)}; ${f0 (0)}`;
      p .numShapes       .textContent = `${f0 (0)} + ${f0 (0)}`;
      p .sensors         .textContent = f0 (0);
   }

   // --- ContextMenu -----------------------------------------------------------------------------------------------

   hideMenu ()
   {
      this .#hide ?.();
   }

   // ContextMenu.initialize: the `contextmenu` event of the element.
   showMenu (event)
   {
      if (this .#disposed)
         return;

      const
         root  = this .#root,
         menu  = this .#build (),
         level = 1;

      event .preventDefault ();
      this .hideMenu ();

      if (!menu)
         return;

      // Layer

      const layer = document .createElement ("div");

      layer .classList .add ("context-menu-layer", menu .className);

      layer .addEventListener ("mousedown",   () => this .hideMenu ());
      layer .addEventListener ("contextmenu", () => this .hideMenu ());

      root .append (layer);

      this .#hide = () =>
      {
         this .#hide = null;

         layer .remove ();

         for (const child of ul .children)
            child .classList .add ("x_ite-private-hidden");

         this .#later (() => ul .remove (), 1_000);

         return false;
      };

      // Menu

      const ul = document .createElement ("ul");

      ul .classList .add ("context-menu-root", "context-menu-list", menu .className);

      ul .addEventListener ("contextmenu", () => this .hideMenu ());

      root .append (ul);

      const background = document .createElement ("div");

      background .classList .add ("context-menu-background");

      ul .append (background);

      for (const k in menu .items)
         ul .append (this .#createItem (menu .items [k], "context-menu-root", k, level + 1));

      // Show
      // Must animate children because of blurish background.

      for (const child of ul .children)
         child .classList .add ("x_ite-private-fade", "x_ite-private-hidden");

      ul .style .display = "block";

      for (const child of ul .children)
         child .classList .remove ("x_ite-private-hidden");

      // Reposition menu if to right or to low.

      this .#offset (ul, { left: event .pageX, top: event .pageY });
      this .#offset (ul, { left: event .pageX, top: event .pageY }); // Do it again.

      const rect = ul .getBoundingClientRect ();

      if (rect .left + ul .offsetWidth > window .innerWidth)
         this .#offset (ul, { left: window .scrollX + Math .max (0, window .innerWidth - ul .offsetWidth) });

      if (rect .top + ul .offsetHeight > window .innerHeight)
         this .#offset (ul, { top: window .scrollY + Math .max (0, window .innerHeight - ul .offsetHeight) });

      // Display submenus on the left or right side.
      // If the submenu is higher than vh, add scrollbars.

      const parentRect = ul .getBoundingClientRect ();

      for (const submenu of ul .querySelectorAll ("ul"))
      {
         submenu .style .display = "block";

         const
            width        = submenu .clientWidth + ul .clientWidth,
            position     = parentRect .left + width > window .innerWidth ? "right" : "left",
            windowHeight = (window .visualViewport ?.height ?? window .innerHeight);

         // Background
         submenu .children [0] .style .height = `${submenu .clientHeight}px`;

         submenu .style [position] = `${ul .clientWidth - 28}px`;

         if (submenu .clientHeight >= windowHeight)
         {
            submenu .style .top       = `${-submenu .closest ("li") .getBoundingClientRect () .top}px`;
            submenu .style .maxHeight = "100dvh";
            submenu .style .overflowY = "scroll";
         }
         else
         {
            // Submenu goes out of page.

            const bottom = submenu .getBoundingClientRect () .top + submenu .clientHeight - (window .scrollY + windowHeight - 20);

            if (bottom > 0)
            {
               this .#offset (submenu, { top: submenu .getBoundingClientRect () .top - bottom });
            }
         }

         submenu .style .display = "";
      }
   }

   #createItem (item, parent, key, level)
   {
      const li = document .createElement ("li");

      li .classList .add ("context-menu-item");

      switch (typeof item)
      {
         case "string":
         {
            if (item .match (/^-+$/))
               li .classList .add ("context-menu-separator", "context-menu-not-selectable");

            break;
         }
         case "object":
         {
            if (typeof item .className === "string")
               li .classList .add (... item .className .split (/\s+/) .filter (s => s));

            switch (item .type)
            {
               case "radio":
               case "checkbox":
               {
                  const
                     label = document .createElement ("label"),
                     input = document .createElement ("input");

                  li    .append (label);
                  label .append (input);

                  input .setAttribute ("type", item .type);
                  input .setAttribute ("name", `context-menu-input-${item .radio || parent}`);

                  const span = document .createElement ("span");

                  span .textContent = item .name;

                  label .append (span);

                  if (item .selected)
                     input .setAttribute ("checked", "");

                  li .classList .add ("context-menu-input");

                  this .#addEvents (item, input, false);
                  break;
               }
               default:
               {
                  if (item .name)
                  {
                     const span = document .createElement ("span");

                     span .textContent = item .name;

                     li .append (span);
                  }

                  this .#addEvents (item, li, true);
                  break;
               }
            }

            break;
         }
      }

      if (typeof item .items === "object" && level < 3)
      {
         const ul = document .createElement ("ul");

         ul .classList .add ("context-menu-list");

         ul .style .zIndex = level;

         li .append (ul);

         const background = document .createElement ("div");

         background .classList .add ("context-menu-background");

         ul .append (background);

         for (const k in item .items)
            ul .append (this .#createItem (item .items [k], key, k, level + 1));

         li .classList .add ("context-menu-submenu");
      }

      return li;
   }

   #addEvents (item, element, hide)
   {
      if (typeof item .callback === "function")
      {
         element .addEventListener ("click", item .callback);

         if (hide)
            element .addEventListener ("click", () => this .hideMenu ());
      }

      if (typeof item .events === "object")
      {
         for (const k in item .events)
         {
            if (typeof item .events [k] === "function")
               element .addEventListener (k, item .events [k]);
         }
      }
   }

   #offset (element, { left, top })
   {
      const
         rect    = element .getBoundingClientRect (),
         cssTop  = parseFloat (element .style .top)  || 0,
         cssLeft = parseFloat (element .style .left) || 0;

      if (left !== undefined)
         element .style .left = `${left - (rect .left + window .scrollX) + cssLeft}px`;

      if (top !== undefined)
         element .style .top = `${top - (rect .top + window .scrollY) + cssTop}px`;
   }

   #createUserMenu ()
   {
      const userMenu = { };

      if (typeof this .#userMenu === "function")
      {
         let menu;

         try { menu = this .#userMenu (this .#browser); } catch (error) { console .error (error); }

         if (menu instanceof Object)
         {
            for (const key in menu)
               userMenu [`user-${key}`] = menu [key];
         }
      }

      return userMenu;
   }

   #build ()
   {
      const
         browser    = this .#browser,
         host       = this .#host,
         element    = browser .getElement (),
         fullscreen = document .fullscreenElement === element;

      if (!browser .getBrowserOption ("ContextMenu"))
         return;

      const info = host .worldInfo ();

      const menu = {
         className: "x_ite-private-menu",
         items: {
            "title": {
               name: `${browser .getName ()} v${browser .getVersion ()}`,
               className: "context-menu-title context-menu-icon x_ite-private-icon-logo context-menu-not-selectable",
            },
            "separator0": "--------",
            "viewpoints": {
               name: _("Viewpoints"),
               className: "context-menu-icon x_ite-private-icon-viewpoint",
               items: this .#getViewpoints (),
            },
            "available-viewers": {
               name: _("Available Viewers"),
               className: "context-menu-icon x_ite-private-icon-viewer",
               items: this .#getAvailableViewers (),
            },
            "straighten-horizon": {
               name: _("Straighten Horizon"),
               type: "checkbox",
               selected: browser .getBrowserOption ("StraightenHorizon"),
               callback: event =>
               {
                  const straightenHorizon = event .target .checked;

                  browser .setBrowserOption ("StraightenHorizon", straightenHorizon);

                  if (straightenHorizon)
                     browser .setDescription (`${_("Straighten Horizon")}: ${_("on")}`);
                  else
                     browser .setDescription (`${_("Straighten Horizon")}: ${_("off")}`);
               },
            },
            "display-rubberband": {
               name: _("Display Rubberband"),
               type: "checkbox",
               selected: browser .getBrowserOption ("Rubberband"),
               callback: event =>
               {
                  const rubberband = event .target .checked;

                  browser .setBrowserOption ("Rubberband", rubberband);

                  if (rubberband)
                     browser .setDescription (`${_("Rubberband")}: ${_("on")}`);
                  else
                     browser .setDescription (`${_("Rubberband")}: ${_("off")}`);
               },
            },
            "separator1": "--------",
            "primitive-quality": {
               name: _("Primitive Quality"),
               className: "context-menu-icon x_ite-private-icon-primitive-quality",
               items: this .#qualityItems ("primitive-quality", "PrimitiveQuality", _("Primitive Quality")),
            },
            "texture-quality": {
               name: _("Texture Quality"),
               className: "context-menu-icon x_ite-private-icon-texture-quality",
               items: this .#qualityItems ("texture-quality", "TextureQuality", _("Texture Quality")),
            },
            "shading": {
               name: _("Shading"),
               className: "context-menu-icon x_ite-private-icon-shading",
               items: Object .fromEntries ([ [ "point", "POINT", "Points" ], [ "wireframe", "WIREFRAME", "Wireframe" ], [ "flat", "FLAT", "Flat" ],
                  [ "gouraud", "GOURAUD", "Gouraud" ], [ "phong", "PHONG", "Phong" ] ] .map (([ key, value, text ]) => [ key,
               {
                  name: _(text),
                  type: "radio",
                  radio: "shading",
                  selected: browser .getBrowserOption ("Shading") === value,
                  callback: () =>
                  {
                     browser .setBrowserOption ("Shading", value);
                     browser .setDescription (`${_("Shading")}: ${_(text)}`);
                  },
               } ])),
            },
            "separator2": "--------",

            // Insert custom user menu items.
            ... this .#createUserMenu (),

            "separator3": "--------",

            ... this .#createAnimationsMenu (),

            "browser-timings": {
               name: _("Browser Timings"),
               type: "checkbox",
               selected: browser .getBrowserOption ("Timings"),
               callback: event =>
               {
                  browser .setBrowserOption ("Timings", event .target .checked);
               },
            },
            "fullscreen": {
               name: fullscreen ? _("Leave Fullscreen") : _("Fullscreen"),
               className: `context-menu-icon ${fullscreen ? "x_ite-private-icon-leave-fullscreen" : "x_ite-private-icon-enter-fullscreen"}`,
               callback: () =>
               {
                  if (fullscreen)
                     document .exitFullscreen () .catch (Function .prototype);
                  else
                     element .requestFullscreen ({ navigationUI: "hide" }) .catch (Function .prototype);
               },
            },
            "separator4": "--------",
            "world-info": {
               name: _("Show World Info"),
               className: "context-menu-icon x_ite-private-icon-world-info",
               callback: () => this .#showWorldInfo (),
            },
            "about": {
               name: _("About X_ITE"),
               className: "context-menu-icon x_ite-private-icon-info",
               callback ()
               {
                  window .open (PROVIDER_URL);
               },
            },
         },
      };

      if (Object .keys (menu .items .viewpoints .items) .length === 0)
         delete menu .items ["viewpoints"];

      if (Object .keys (menu .items ["available-viewers"] .items) .length < 2)
      {
         delete menu .items ["available-viewers"];
      }

      if (!host .currentViewer () .match (/^(?:EXAMINE|FLY)$/))
      {
         delete menu .items ["straighten-horizon"];
      }

      if (!host .currentViewer () .match (/^(?:WALK|FLY)$/))
      {
         delete menu .items ["display-rubberband"];
      }

      if (!browser .getBrowserOption ("Debug"))
      {
         delete menu .items ["shading"];
      }

      if (!info .hasInfo && !info .meta .length)
      {
         delete menu .items ["world-info"];
      }

      return menu;
   }

   #qualityItems (radio, option, label)
   {
      const browser = this .#browser;

      return Object .fromEntries ([ [ "high", "HIGH", "High" ], [ "medium", "MEDIUM", "Medium" ], [ "low", "LOW", "Low" ] ] .map (([ key, value, text ]) => [ key,
      {
         name: _(text),
         type: "radio",
         radio,
         selected: browser .getBrowserOption (option) === value,
         callback: () =>
         {
            browser .setBrowserOption (option, value);
            browser .setDescription (`${label}: ${_(text .toLowerCase ())}`);
         },
      } ]));
   }

   #getViewpoints ()
   {
      const
         host    = this .#host,
         current = host .activeViewpoint (),
         menu    = { };

      for (const { id, name } of host .viewpoints ())
      {
         menu [`Viewpoint-${id}`] = {
            name,
            type: "radio",
            radio: "viewpoints",
            selected: id === current,
            className: "x_ite-private-viewpoint",
            callback: () =>
            {
               host .bind (id);
            },
         };
      }

      return menu;
   }

   #getAvailableViewers ()
   {
      const
         browser       = this .#browser,
         host          = this .#host,
         currentViewer = host .currentViewer (),
         menu          = { };

      for (const viewer of host .availableViewers ())
      {
         menu [viewer] = {
            name: _(this .#getViewerName (viewer)),
            type: "radio",
            radio: "viewers",
            selected: viewer === currentViewer,
            className: `x_ite-private-${viewer .toLowerCase ()}-viewer`,
            callback: () =>
            {
               host .setViewer (viewer);
               browser .setDescription (_(this .#getViewerName (viewer)));
            },
         };
      }

      return menu;
   }

   #getViewerName (viewer)
   {
      switch (viewer)
      {
         case "EXAMINE":
            return _("Examine Viewer");
         case "WALK":
            return _("Walk Viewer");
         case "FLY":
            return _("Fly Viewer");
         case "PLANE":
            return _("Plane Viewer");
         case "LOOKAT":
            return _("Look At Viewer");
         case "NONE":
            return _("None Viewer");
      }
   }

   #createAnimationsMenu ()
   {
      const animations = (() => { try { return this .#browser .currentScene .getExportedNode ("Animations"); } catch (error) { return null; } })();

      if (!animations)
         return { };

      const timeSensors = (() =>
      {
         try { return Array .from (animations .children, group => group .children [0]); } catch (error) { return null; }
      })()
         ?.filter (node => node ?.getNodeTypeName ?.() === "TimeSensor");

      if (!timeSensors ?.length)
         return { };

      return {
         "animations": {
            name: _("Animations"),
            className: "context-menu-icon x_ite-private-icon-animations",
            items: {
               ... Object .fromEntries (timeSensors .map ((timeSensor, i) =>
               {
                  return [`animation-${i}`, {
                     name: timeSensor .description,
                     type: "checkbox",
                     selected: timeSensor .isActive,
                     callback ()
                     {
                        if (timeSensor .isActive)
                        {
                           timeSensor .stopTime = Date .now () / 1000;
                        }
                        else
                        {
                           timeSensor .loop      = true;
                           timeSensor .startTime = Date .now () / 1000;
                        }
                     },
                  }];
               })),

               "separator2": "--------",

               "all": {
                  name: _("All"),
                  className: "context-menu-icon x_ite-private-icon-play",
                  callback ()
                  {
                     for (const timeSensor of timeSensors)
                     {
                        timeSensor .loop      = true;
                        timeSensor .startTime = Date .now () / 1000;
                     }
                  },
               },
               "none": {
                  name: _("None"),
                  className: "context-menu-icon x_ite-private-icon-stop",
                  callback ()
                  {
                     for (const timeSensor of timeSensors)
                        timeSensor .stopTime = Date .now () / 1000;
                  },
               },
            },
         },
      };
   }

   // The "Show World Info" entry (ContextMenu.build "world-info").
   #showWorldInfo ()
   {
      const browser = this .#browser;
      const info    = this .#host .worldInfo ();

      this .#root .querySelector (".x_ite-private-world-info") ?.remove ();

      const
         priv         = this .#wrapper,
         overlay      = document .createElement ("div"),
         div          = document .createElement ("div"),
         content      = document .createElement ("div"),
         hasWorldInfo = info .hasInfo && (info .title .length || info .info .length);

      overlay .classList .add ("x_ite-private-world-info-overlay");
      priv .append (overlay);

      div .classList .add ("x_ite-private-world-info", "x_ite-private-fade", "x_ite-private-hidden");
      overlay .append (div);

      const buttons = document .createElement ("div");

      buttons .classList .add ("x_ite-private-world-info-buttons");
      div .append (buttons);

      const linkify = (element, string) =>
      {
         const
            email = /([a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z0-9]{2,})/ig,
            link  = /(https?:\/\/[-a-zA-Z0-9@:%._+~#=]+\.[a-zA-Z0-9]{2,}\b[-a-zA-Z0-9()@:%_+.~#?&/=]*\b\/?)/ig;

         const replaced_text = string
            .replace (email, `<a href="mailto:$1" target="_blank">$1</a>`)
            .replace (link, `<a href="$1" target="_blank">$1</a>`);

         element .innerHTML = replaced_text;

         for (const a of element .querySelectorAll ("a"))
            a .addEventListener ("click", event => event .stopPropagation ());
      };

      const worldInfoButton = document .createElement ("button");

      worldInfoButton .classList .add ("x_ite-private-browser-button");
      worldInfoButton .style .width = "100pt";
      worldInfoButton .textContent = _("World Info");
      buttons .append (worldInfoButton);

      worldInfoButton .addEventListener ("click", event =>
      {
         event .preventDefault ();
         event .stopPropagation ();
         event .stopImmediatePropagation ();

         const
            title = info .title,
            lines = info .info;

         buttons .querySelector ("button") .classList .remove ("active");
         worldInfoButton .classList .add ("active");

         content .replaceChildren ();

         const top = document .createElement ("div");

         top .classList .add ("x_ite-private-world-info-top");
         top .textContent = _("World Info");
         content .append (top);

         if (title .length)
         {
            const t = document .createElement ("div");

            t .classList .add ("x_ite-private-world-info-title");
            t .textContent = title;

            content .append (t);
         }

         for (const line of lines)
         {
            const l = document .createElement ("div");

            l .classList .add ("x_ite-private-world-info-info");

            linkify (l, line);

            content .append (l);
         }
      });

      const metaDataButton = document .createElement ("button");

      metaDataButton .classList .add ("x_ite-private-browser-button");
      metaDataButton .style .width = "100pt";
      metaDataButton .textContent = _("Metadata");
      buttons .append (metaDataButton);

      metaDataButton .addEventListener ("click", event =>
      {
         event .preventDefault ();
         event .stopPropagation ();
         event .stopImmediatePropagation ();

         buttons .querySelector ("button") .classList .remove ("active");
         worldInfoButton .classList .add ("active");

         content .replaceChildren ();

         const top = document .createElement ("div");

         top .classList .add ("x_ite-private-world-info-top");
         top .textContent = _("Metadata");
         content .append (top);

         const table = document .createElement ("table");

         content .append (table);

         for (const [key, value] of info .meta)
         {
            const
               tr  = document .createElement ("tr"),
               td1 = document .createElement ("td"),
               td2 = document .createElement ("td");

            td1 .textContent = `${key}:`;

            linkify (td2, value);

            tr .append (td1, td2);
            table .append (tr);
         }
      });

      if (!hasWorldInfo)
         worldInfoButton .style .display = "none";

      if (!info .meta .length)
         metaDataButton .style .display = "none";

      if (hasWorldInfo)
         worldInfoButton .click ();
      else
         metaDataButton .click ();

      div .append (content);

      div .classList .remove ("x_ite-private-hidden");

      overlay .addEventListener ("click", () =>
      {
         div .classList .add ("x_ite-private-hidden");

         this .#later (() => overlay .remove (), 300);
      });
   }

   // X3DBrowser.dispose: the menu, the world info, the timers and the elements go.
   dispose ()
   {
      if (this .#disposed)
         return;

      this .#hide ?.();
      this .#disposed = true;

      for (const id of this .#timers)
         clearTimeout (id);

      this .#timers .clear ();
      this .#root .querySelectorAll (".x_ite-private-menu, .x_ite-private-world-info-overlay") .forEach (element => element .remove ());
      this .#timings ?.remove ();
      this .#splash ?.remove ();
   }
}

// --- DOM integration: X3D content written as HTML ---------------------------------------------------------------------------
//
// Browser/DOMIntegration.js and the HTML parts of Parser/XMLParser.js and Parser/HTMLSupport.js of X_ITE (issue #146). X_ITE
// parses the DOM elements itself; here the engine parses XML text, so an HTML `<X3D>` element is written as XML first, with the
// names X_ITE's HTML parser would look up: `TRANSFORM` is `Transform`, `diffusecolor` is `diffuseColor`, `PROTODECLARE` is
// `ProtoDeclare`. Everything else (the values, DEF and USE, the warnings) is the engine's XML parser's.

let htmlTables = null;

// The conversion of the `<X3D>` element that a scene was imported from (`importDocument`), see htmlToXML.
const htmlImports = new WeakMap ();

// The function DOMIntegration wants called with the scene of an `<X3D>` element once the engine has made it (before it is loaded).
const importHooks = new WeakMap ();

// HTMLSupport: node type names (exact or upper case) and field names (exact or lower case) of the pinned X_ITE.
function htmlSupportTables ()
{
   if (htmlTables)
      return htmlTables;

   const [ types, fields ] = htmlSupport ();
   const tables = { types: new Map (), fields: new Map () };

   for (const name of types)
   {
      tables .types .set (name, name);
      tables .types .set (name .toUpperCase (), name);
   }

   tables .types .set ("ProtoInstance", "ProtoInstance");
   tables .types .set ("PROTOINSTANCE", "ProtoInstance");

   for (const name of fields)
   {
      tables .fields .set (name, name);
      tables .fields .set (name .toLowerCase (), name);
   }

   return htmlTables = tables;
}

// The elements the XML parser knows by name (upper case or as written), with the name the engine's XML parser knows.
const XML_ELEMENT_NAMES = new Map ([
   "head", "component", "unit", "meta", "Scene", "ExternProtoDeclare", "ProtoDeclare", "ProtoInterface", "ProtoBody", "IS",
   "ProtoInstance", "fieldValue", "field", "connect", "ROUTE", "IMPORT", "EXPORT", "X3D",
] .flatMap (name => [ [ name, name ], [ name .toUpperCase (), name ] ]));

const xmlText   = text => String (text) .replace (/&/g, "&amp;") .replace (/</g, "&lt;") .replace (/>/g, "&gt;");
const xmlAttr   = text => xmlText (text) .replace (/"/g, "&quot;") .replace (/\t/g, "&#9;") .replace (/\n/g, "&#10;") .replace (/\r/g, "&#13;");
const xmlName   = name => /^[A-Za-z_][\w.:-]*$/ .test (name);
const RESERVED  = new Map ([ [ "def", "DEF" ], [ "use", "USE" ], [ "containerfield", "containerField" ] ]);

// The attributes an element is read with by name (getAttribute, case-insensitive in HTML) and the names they have in XML.
const ATTRIBUTE_NAMES = new Map ([
   "name", "url", "appinfo", "documentation", "accessType", "type", "value", "nodeField", "protoField", "fromNode", "fromField",
   "toNode", "toField", "inlineDEF", "importedDEF", "exportedDEF", "AS", "DESCRIPTION", "localDEF", "level", "category",
   "conversionFactor", "content", "profile", "version", "DEF", "USE", "containerField",
] .map (name => [ name .toLowerCase (), name ]));

// The PROTO names and fields that the HTML parser has seen in a world (XMLParser's `protoNames` and `protoFields`), kept for
// the elements that are added to it later.
const newHtmlState = () => ({ protoNames: new Map (), protoFields: new Map () });

// An HTML `<X3D>` element as the XML text of the same X3D document, or with `fragment` the element of that document that was
// added to it. The result is the text and the elements of the page that its XML elements are, in document order (null for the
// XML elements that no page element is): a mutation finds the node of an element with them.
function htmlToXML (root, state = newHtmlState (), fragment = null)
{
   const { types, fields } = htmlSupportTables ();
   const { protoNames, protoFields } = state; // upper case or exact name → name of a declared PROTO; PROTO → (lower case or exact name → field name)
   const out         = [ ];
   const elements    = [ ];
   const get         = (element, name) => element .getAttribute (name);

   // `attribute="value"` of the attributes the element is read with.
   const named = (element, names) => names .map (name =>
   {
      const value = get (element, name);

      return value === null ? "" : ` ${ATTRIBUTE_NAMES .get (name .toLowerCase ()) ?? name}="${xmlAttr (value)}"`;
   }) .join ("");

   const children = (element, callback) => { for (const child of Array .from (element .childNodes)) callback (child); };

   const inherit = (element, tag, names, body) =>
   {
      elements .push (element);
      out .push (`<${tag}${named (element, names)}`);

      if (body)
      {
         out .push (">");
         body ();
         out .push (`</${tag}>`);
      }
      else
      {
         out .push ("/>");
      }
   };

   // head children: component, unit, meta.
   const headChild = element =>
   {
      switch (element .nodeName)
      {
         case "component": case "COMPONENT": inherit (element, "component", [ "name", "level" ]); return;
         case "unit": case "UNIT": inherit (element, "unit", [ "category", "name", "conversionFactor" ]); return;
         case "meta": case "META": inherit (element, "meta", [ "name", "content" ]); return;
      }
   };

   // The field elements of a PROTO or EXTERNPROTO interface, and the names of the fields.
   const interfaceFields = element =>
   {
      const names = new Map ();

      children (element, child =>
      {
         if (child .nodeName === "field" || child .nodeName === "FIELD")
         {
            fieldElement (child);

            const name = get (child, "name");

            if (name && ! [ "DEF", "USE", "containerField" ] .some (r => r === name || r .toLowerCase () === name))
               names .set (name, name) .set (name .toLowerCase (), name);
         }
      });

      return names;
   };

   const fieldElement = element => inherit (element, "field", [ "accessType", "type", "name", "appinfo", "documentation", "value" ], () => children (element, childElement));

   const attributesOf = (element, fieldMap) =>
   {
      let text = "";

      for (const attribute of Array .from (element .attributes))
      {
         const reserved = RESERVED .get (attribute .name .toLowerCase ());

         if (reserved)
         {
            text += ` ${reserved}="${xmlAttr (attribute .value)}"`;
            continue;
         }

         const name = (fieldMap ?? fields) .get (attribute .name);

         if (name !== undefined)
            text += ` ${name}="${xmlAttr (attribute .value)}"`;
      }

      return text;
   };

   // X_ITE's HTMLParser moves the text of a Script into elements and CDATA sections (XMLParser.scriptElement).
   const scriptChildren = element =>
   {
      let document;

      try
      {
         document = new DOMParser () .parseFromString (element .outerHTML, "application/xml");

         if (document .getElementsByTagName ("parsererror") .length)
            throw new Error ("Invalid XML.");
      }
      catch (error)
      {
         // XMLParser.nodeElement: the Error object, as a development build of X_ITE logs it.
         console .error (error);
         return false;
      }

      element .textContent = "// Content moved into childNodes.";

      // The live childNodes, as X_ITE walks them: appending a node takes it out of the list, so every second child stays.
      for (const child of document .children [0] .childNodes)
      {
         if (child .nodeType === 1 || child .nodeType === 4)
            element .append (child);
      }

      return true;
   };

   const childElement = element =>
   {
      switch (element .nodeName)
      {
         case "#comment": case "#text":
            return;

         case "#cdata-section":
            out .push (`<![CDATA[${element .data .replace (/\]\]>/g, "]]]]><![CDATA[>")}]]>`);
            return;

         case "ExternProtoDeclare": case "EXTERNPROTODECLARE":
         {
            const name = get (element, "name");

            if (!name)
               return;

            inherit (element, "ExternProtoDeclare", [ "name", "url", "appinfo", "documentation" ], () =>
            {
               protoFields .set (name, interfaceFields (element));
            });

            protoNames .set (name, name) .set (name .toUpperCase (), name);
            return;
         }

         case "ProtoDeclare": case "PROTODECLARE":
         {
            const name = get (element, "name");

            if (!name)
               return;

            const kids = Array .from (element .childNodes);
            const iface = kids .find (child => child .nodeName === "ProtoInterface" || child .nodeName === "PROTOINTERFACE");
            const body  = kids .find (child => child .nodeName === "ProtoBody" || child .nodeName === "PROTOBODY");

            inherit (element, "ProtoDeclare", [ "name", "appinfo", "documentation" ], () =>
            {
               if (iface)
               {
                  elements .push (iface);
                  out .push ("<ProtoInterface>");
                  protoFields .set (name, interfaceFields (iface));
                  out .push ("</ProtoInterface>");
               }
               else
               {
                  protoFields .set (name, new Map ());
               }

               if (body)
                  inherit (body, "ProtoBody", [ ], () => children (body, childElement));
            });

            protoNames .set (name, name) .set (name .toUpperCase (), name);
            return;
         }

         case "IS":
            inherit (element, "IS", [ ], () => children (element, child =>
            {
               if (child .nodeName === "connect" || child .nodeName === "CONNECT")
                  inherit (child, "connect", [ "nodeField", "protoField" ]);
            }));
            return;

         case "ProtoInstance": case "PROTOINSTANCE":
            inherit (element, "ProtoInstance", [ "name", "DEF", "USE", "containerField" ], () => children (element, childElement));
            return;

         case "fieldValue": case "FIELDVALUE":
            inherit (element, "fieldValue", [ "name", "value" ], () => children (element, childElement));
            return;

         case "field": case "FIELD":
            fieldElement (element);
            return;

         case "ROUTE":
            inherit (element, "ROUTE", [ "fromNode", "fromField", "toNode", "toField" ]);
            return;

         case "IMPORT":
            elements .push (element);
            out .push (`<IMPORT${named (element, [ "inlineDEF" ])}`);
            {
               const exported = get (element, "importedDEF") || get (element, "exportedDEF");

               out .push (`${exported === null ? "" : ` importedDEF="${xmlAttr (exported)}"`}${named (element, [ "AS", "DESCRIPTION" ])}/>`);
            }
            return;

         case "EXPORT":
            inherit (element, "EXPORT", [ "localDEF", "AS", "DESCRIPTION" ]);
            return;

         default:
            nodeElement (element);
            return;
      }
   };

   const nodeElement = element =>
   {
      // A built-in node first, then a PROTO of the name (XMLParser.nodeElement).
      const typeName  = types .get (element .nodeName);
      const protoName = typeName === undefined ? protoNames .get (element .nodeName) : undefined;
      const tag       = typeName ?? protoName ?? element .nodeName;

      if (!xmlName (tag))
         return;

      const use = get (element, "USE");

      if (use)
      {
         elements .push (element);
         out .push (`<${tag} USE="${xmlAttr (use)}"${typeName === "ProtoInstance" ? named (element, [ "name" ]) : ""}/>`);
         return;
      }

      if (element .nodeName === "SCRIPT" && !scriptChildren (element))
         return;

      const fieldMap = protoName === undefined ? undefined : protoFields .get (protoName);

      elements .push (element);
      out .push (`<${tag}${attributesOf (element, protoName === undefined ? undefined : fieldMap ?? new Map ())}>`);
      children (element, childElement);
      out .push (`</${tag}>`);
   };

   // The field that an attribute of an element is read into (XMLParser.attributeToCamelCase), or undefined.
   const fieldName = (element, name) =>
   {
      const protoName = types .has (element .nodeName) ? undefined : protoNames .get (element .nodeName);

      return (protoName === undefined ? fields : protoFields .get (protoName) ?? new Map ()) .get (name);
   };

   if (fragment)
   {
      childElement (fragment);
   }
   else
   {
      // The X3D element: the head (head elements first, then the X3D element's own head children), then the scene.
      const scene = Array .from (root .childNodes) .find (child => child .nodeName === "Scene" || child .nodeName === "SCENE");

      elements .push (null);
      out .push (`<X3D${named (root, [ "profile", "version" ])}>`);
      elements .push (null);
      out .push ("<head>");
      children (root, child => { if (child .nodeName === "head" || child .nodeName === "HEAD") children (child, headChild); });
      children (root, headChild);
      out .push ("</head>");
      elements .push (scene ?? null);
      out .push ("<Scene>");
      children (root, child => { if (child .nodeName === "Scene" || child .nodeName === "SCENE") children (child, childElement); });
      out .push ("</Scene>");
      out .push ("</X3D>");
   }

   return { xml: out .join (""), elements, fieldName, state };
}

// Browser/DOMIntegration.js: the last `<X3D>` child of the canvas is the world, and the changes of its elements are the world's
// changes. X_ITE keeps the node of an element in `element [_node]`; here `#targets` holds what the XML parse of the engine made
// of an element (see htmlToXML, `Session::xml_targets`).
class DOMIntegration
{
   #browser;
   #observer;
   #rootElement = null;
   #changes = null;
   #html = null;
   #handle = null;
   #scene = null;
   #targets = new WeakMap ();
   #bound = new WeakSet ();
   #elements = new Map (); // node id → its element

   constructor (browser)
   {
      this .#browser  = browser;
      this .#observer = new MutationObserver (() => this .#processCanvasMutation ());

      this .#observer .observe (browser .getElement (), { attributes: false, childList: true });
      this .#processCanvasMutation ();
   }

   #processCanvasMutation ()
   {
      this .#processRootElement (this .#browser .getElement () .querySelector (":scope > X3D:last-of-type"));
   }

   async #processRootElement (rootElement)
   {
      try
      {
         if (rootElement === this .#rootElement)
            return;

         this .#rootElement = rootElement;
         this .#changes ?.disconnect ();
         this .#changes = null;
         this .#html    = null;
         this .#handle  = null;
         this .#scene   = null;
         this .#targets = new WeakMap ();
         this .#bound   = new WeakSet ();
         this .#elements = new Map ();
         loadStateTaps .delete (this .#browser);

         if (!rootElement)
         {
            this .#browser .replaceWorld (null);
            return;
         }

         // The scene is known to the page as soon as the engine made it; its Inline elements are looked at then.
         importHooks .set (rootElement, { prepared: scene => this .#prepared (rootElement, scene), started: () => this .#started (rootElement) });

         const scene = await this .#browser .importDocument (rootElement);

         this .#browser .replaceWorld (scene);

         // Start observing, as X_ITE does after the world was replaced.
         const html   = htmlImports .get (scene);
         const handle = scene [_node];

         if (!html || rootElement !== this .#rootElement || !handle ?.browser || handle .scene !== this .#handle ?.scene)
         {
            return;
         }

         this .#changes = new MutationObserver (mutations =>
         {
            for (const mutation of mutations)
               this .#processMutation (mutation);
         });

         this .#changes .observe (rootElement, { attributes: true, childList: true, subtree: true });

         // Connect to node events.
         this .#addEventDispatchersAll (rootElement);
      }
      catch (error)
      {
         console .error ("Error importing document:", error);
      }
   }

   // The engine made the scene of `rootElement`: what its elements made, and the Inline elements, which are loading.
   #prepared (rootElement, scene)
   {
      const html   = htmlImports .get (scene);
      const handle = scene [_node];

      if (!html || rootElement !== this .#rootElement || !handle ?.browser)
         return;

      this .#html   = html;
      this .#handle = handle;
      this .#scene  = scene; // the page's scene object stays

      this .#register (engineOf (handle) .xmlTargets (handle .scene), html .elements);
   }

   // The loads of the world started: add the Inline elements from the initial scene.
   #started (rootElement)
   {
      const handle = this .#handle;

      if (!handle || rootElement !== this .#rootElement)
         return;

      loadStateTaps .set (this .#browser, { scene: handle .scene, nodes: new Map (), pending: new Set () });

      this .#processInlineElements (rootElement);
   }

   // What the elements of an XML text made, `[index, kind, a, b]` each, and the page elements of that text.
   #register (targets, elements)
   {
      for (let i = 0; i < targets .length; i += 4)
      {
         const element = elements [targets [i]];

         if (!element)
            continue;

         this .#targets .set (element, { kind: targets [i + 1], a: targets [i + 2], b: targets [i + 3] });

         if (targets [i + 1] === 0)
            this .#elements .set (targets [i + 2], element);
      }
   }

   #node (element)
   {
      const target = this .#targets .get (element);

      return target ?.kind === 0 ? target .a : undefined;
   }

   #processMutation (mutation)
   {
      if (!this .#handle)
         return;

      try
      {
         switch (mutation .type)
         {
            case "attributes":
            {
               this .#processAttribute (mutation);
               break;
            }
            case "childList":
            {
               for (const node of mutation .addedNodes)
                  this .#processAddedNode (node);

               for (const node of mutation .removedNodes)
                  this .#processRemovedNode (node);

               break;
            }
         }
      }
      finally
      {
         // The warnings and errors of the engine's parser.
         const handle = this .#handle;

         if (handle .browser [_engine] ?.exists (handle .scene))
         {
            for (const line of handle .browser [_engine] .takeLog (handle .scene))
               printLogLine (line);
         }
      }
   }

   #processAttribute ({ target: element, attributeName })
   {
      const node = this .#node (element);

      if (node !== undefined)
      {
         const attribute = element .getAttributeNode (attributeName);

         // XMLParser.nodeAttribute: a missing attribute or a name of no field throws, and the parser ignores that.
         const name = attribute && this .#html .fieldName (element, attribute .name);

         if (name !== undefined && name !== null)
            engineOf (this .#handle) .xmlAttribute (this .#handle .scene, node, name, attribute .value);
      }
      else
      {
         // The attribute of a child that is not a node, such as fieldValue (or ROUTE): the child is read again.
         const parentNode = this .#node (element .parentNode);

         if (parentNode !== undefined)
            this .#child (element, parentNode);
      }
   }

   // XMLParser.childElement (element) with the node `parent` (-1: among the root nodes of the context `ctx`, -1: of the world) as the
   // parent. The elements are those of the page, or the ones the Inline's document made.
   #child (element, parent, ctx = -1)
   {
      const { xml, elements } = htmlToXML (null, this .#html .state, element);

      if (xml === "")
         return;

      const handle = this .#handle;

      this .#register (engineOf (handle) .xmlChild (handle .scene, parent, ctx, xml), elements);

      // A field element made a new field object for the node: the page's field objects are the new ones.
      if (parent >= 0 && /^field$/i .test (element .nodeName))
         this .#renewFields (parent);
   }

   // The field objects of node `id` are read again (a user-defined field was replaced).
   #renewFields (id)
   {
      const ref = nodeRef (this .#handle, id);

      ref [NAMES]       = undefined;
      ref [DEFINITIONS] = undefined;
      ref [FIELDS] .clear ();

      // X_ITE does not connect the new field object to the element: it fires no event any more.
   }

   #processAddedNode (element)
   {
      // Only process element nodes.
      if (element .nodeType !== Node .ELEMENT_NODE)
         return;

      if (element .nodeName === "X3D")
         return;

      if (this .#targets .has (element))
         return;

      const parent = element .parentNode;

      // The root scene or the scene of an Inline, a node, or else the world.
      if (/^(?:Scene|SCENE)$/ .test (parent .nodeName))
      {
         const target = this .#targets .get (parent);

         this .#child (element, -1, target ?.kind === 1 ? target .a : -1);
      }
      else
      {
         this .#child (element, this .#node (parent) ?? -1);
      }

      // Now after creating nodes need to look again for Inline elements, then attach event dispatchers.
      this .#processInlineElements (element);
      this .#addEventDispatchers (element);
      this .#addEventDispatchersAll (element);
   }

   #processRemovedNode (element)
   {
      // Works also for root nodes, and removes ROUTE elements.
      const target = this .#targets .get (element);

      if (!target)
         return;

      const handle = this .#handle;

      switch (target .kind)
      {
         case 0: engineOf (handle) .disposeNode (handle .scene, target .a); break;
         case 1: engineOf (handle) .disposeContext (handle .scene, target .a); break;
         case 2: engineOf (handle) .deleteRouteById (handle .scene, target .a, target .b); break;
      }

      this .#targets .delete (element);
   }

   // --- Inline elements ---------------------------------------------------------------------------------------------------

   #processInlineElements (element)
   {
      if (/^(?:Inline|INLINE)$/ .test (element .nodeName))
         this .#processInlineElement (element);

      for (const inlineElement of element .querySelectorAll ("Inline"))
         this .#processInlineElement (inlineElement);
   }

   #processInlineElement (element)
   {
      const node = this .#node (element);

      if (node === undefined)
         return;

      const handle = this .#handle;
      const tap    = loadStateTaps .get (this .#browser);

      // `node ._loadState .addInterest ("appendInlineChildElement", this, element)`
      tap .nodes .set (node, () => this .#guarded (() => this .#appendInlineChildElement (element)));

      engineOf (handle) .observeField (handle .scene, node, "loadState", true);

      this .#appendInlineChildElement (element);
   }

   // The work of a notification: the engine's lines follow it, and an error is the page's.
   #guarded (fn)
   {
      try
      {
         if (this .#handle)
            fn ();
      }
      catch (error)
      {
         console .error (error);
      }
   }

   #appendInlineChildElement (element)
   {
      const node   = this .#node (element);
      const handle = this .#handle;

      if (node === undefined || !handle)
         return;

      const engine = engineOf (handle);
      const state  = engine .loadState (handle .scene, node);

      switch (state)
      {
         case 0: // NOT_STARTED_STATE
         case 3: // FAILED_STATE
         {
            // Remove all child nodes.
            while (element .firstChild)
               element .removeChild (element .lastChild);

            break;
         }
         case 2: // COMPLETE_STATE
         {
            // Remove all child nodes.
            while (element .firstChild)
               element .removeChild (element .lastChild);

            // Add the scene as child node of the Inline element: the document the engine read.
            const inline = engine .inlineXml (handle .scene, node);

            if (!inline)
               break;

            const document = new DOMParser () .parseFromString (inline [0], "application/xml");
            const x3d      = document .documentElement;

            if (!x3d || document .getElementsByTagName ("parsererror") .length)
               break;

            this .#register (inline [1], Array .from (document .querySelectorAll ("*")));

            element .append (x3d);

            // Add Inline elements, and connect to node events.
            this .#processInlineElements (x3d);
            this .#addEventDispatchersAll (x3d);

            break;
         }
      }

      switch (state)
      {
         case 2:
            element .dispatchEvent (new CustomEvent ("load", { detail: { node: nodeObject (handle, node) } }));
            break;
         case 3:
            element .dispatchEvent (new CustomEvent ("error", { detail: { node: nodeObject (handle, node) } }));
            break;
      }
   }

   // --- Field events ------------------------------------------------------------------------------------------------------

   #addEventDispatchersAll (element)
   {
      for (const childElement of element .querySelectorAll ("*"))
         this .#addEventDispatchers (childElement);
   }

   #addEventDispatchers (element)
   {
      // Check for USE nodes; they do not emit events.
      if (element .nodeName === "ROUTE" || this .#bound .has (element))
         return;

      const node = this .#node (element);

      if (node === undefined)
         return;

      this .#bound .add (element);

      const ref = nodeRef (this .#handle, node);

      for (const definition of ref .getFieldDefinitions ())
         this .#bindFieldCallback (ref, element, definition);
   }

   #bindFieldCallback (ref, element, definition)
   {
      if (definition .accessType !== X3DConstants .outputOnly && definition .accessType !== X3DConstants .inputOutput)
         return;

      attachedField (ref, definition .name) .addFieldCallback (this, value => this .#fieldCallback (element, definition .name, value));
   }

   #fieldCallback (element, name, value)
   {
      const node = this .#node (element);

      if (node === undefined)
         return;

      element .dispatchEvent (new CustomEvent (name, { detail: { node: nodeObject (this .#handle, node), value } }));
   }

   dispose ()
   {
      this .#observer .disconnect ();
      this .#changes ?.disconnect ();
      loadStateTaps .delete (this .#browser);
   }
}

class X3DBrowser
{
   #element;
   #canvas;
   #engine = null;
   #chain = null;
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
   // The real Web Audio nodes of the engine's Sound nodes, on #audioContext (audio_mirror.mjs).
   #audioMirror = null;
   // The <video> elements of the engine's MovieTextures (video_mirror.mjs).
   #videoMirror = null;

   constructor (element)
   {
      this .#element = element;
      this .#canvas  = document .createElement ("canvas");
      this .#scene   = new X3DScene (this, null, 0);

      // X3DCoreContext: the browser's own element is focusable (tabindex 0) and takes the keyboard.
      this .#canvas .tabIndex = 0;
      this .#canvas .style .cssText = "display:block;width:100%;height:100%;outline:none;touch-action:none";
      // X3DCoreContext: `.x_ite-private-browser` holds the canvas and the user interface's overlays (BrowserUI).
      const wrapper = document .createElement ("div");

      wrapper .classList .add ("x_ite-private-browser");
      element .shadowRoot .append (wrapper);
      wrapper .append (this .#canvas);

      // Notification.initialize: `div.x_ite-private-notification > span`, in a zero-height box before the canvas that
      // stands for X_ITE's positioned surface; the rules are in the shadow <style> of X3DCanvasElement.
      const box = document .createElement ("div");

      this .#notification = document .createElement ("div");
      this .#notificationSpan = document .createElement ("span");
      this .#notification .classList .add ("x_ite-private-notification");
      this .#notification .append (this .#notificationSpan);
      box .classList .add ("x_ite-private-notification-box");
      box .append (this .#notification);
      this .#canvas .before (box);

      this .#ui = new BrowserUI (this,
      {
         signal: this .#listeners .signal,
         worldInfo: () => this .#worldInfo (),
         viewpoints: () => Array .from (this .#engine ?.userViewpoints () ?? [ ], id => ({ id, name: this .#engine .viewpointDescription (id) })),
         activeViewpoint: () => this .#engine ?.activeViewpoint (),
         bind: id => this .#engine ?.viewpointRequest (`bindViewpoint:${id}`),
         currentViewer: () => this .#engine ?.currentViewer () ?? "NONE",
         availableViewers: () => this .#engine ?.availableViewers () ?? [ ],
         setViewer: viewer => this .#engine ?.setViewer (viewer),
      },
      element .shadowRoot, wrapper, this .#canvas);


      this .#ready = wasm .then (() =>
      {
         // The MovieTexture time: ISO/IEC 19775-1 18.4.2 by default, `movietiming="pinned"` for the pinned X_ITE 16.5.0
         // behavior (docs/movie-texture-timing.md). It applies to the worlds made after it is set.
         const timing = element .getAttribute ("movietiming");

         if (timing !== null)
            WebBrowser .setMovieTiming (timing);

         return WebBrowser .create (this .#canvas);
      }) .then (engine =>
      {
         if (this .#disposed)
         {
            engine .free ();
            throw new Error ("Browser disposed.");
         }

         this .#engine = engine;
         // The Scripts of a world run only when the page lets them (the `scripts` attribute, read when the world starts):
         // they run with the privileges of the page, in its JavaScript engine (API.md, "Script nodes").
         // A Script's print and console lines go to the page's console in the Script's call, as X_ITE logs them.
         this .#engine .setLogSink (printLogLine);
         // The field callbacks run in the route chain, as X_ITE's do.
         this .#engine .setFieldEventSink ((scene, node, name, data) => deliverChainEvent (this, scene, node, name, data));
         this .#engine .setScriptRealm (host =>
         {
            if (this .#element .getAttribute ("scripts") !== "true")
               return null;

            const realm = createScriptRealm (host, scriptOp);

            // Functions and node objects of a Script that the page keeps work after the world was replaced: the engine keeps
            // the replaced world until the page has dropped every one of them, which the realm they all reach shows.
            realmRegistry .register (realm, { engine, host });

            return realm;
          });
         // An endUpdate () before the engine was ready.
         this .#engine .setLive (this .#live);
         // Mute and SoundIntensity set before the engine was ready: the engine takes them for the worlds to come.
         this .#applySoundOptions ();
         // The options the engine reads itself, set before the engine was ready.
         this .#applyEngineOptions ();
         // Shading and Multisampling set before the engine was ready, and the samples the device supports.
         this .#applyDerivedRendering ();
         this .#listen ();
         this .#start ();

         // X3DBrowser.initialize: replaceWorld with an empty scene, which ends in "initialized load". A loadURL
         // then always replaces a world, so it starts with "shutdown".
         // A browser that is stopped from the start (update="false") never gets its frame: X_ITE fires nothing then, also
         // not when it starts later.
         this .#ui .setBrowserLoading (false);

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

      // X3DBrowser: `this [_DOMIntegration] = new DOMIntegration (this)`; the world written inside the element.
      this .#dom = new DOMIntegration (this);
   }

   #dom;
   #ui;

   // The WorldInfo and META statements of the world for the menu entry "Show World Info".
   #worldInfo ()
   {
      const [ hasInfo, title, info, meta ] = this .#engine ?.worldInfo () ?? [ false, "", [ ], [ ] ];

      return { hasInfo: !! hasInfo, title: title ?? "", info: Array .from (info ?? [ ]), meta: Array .from (meta ?? [ ], pair => Array .from (pair)) };
   }

   // X3DBrowser.getContextMenu: the ContextMenu node (its `userMenu` and `hide ()`).
   getContextMenu () { return this .#ui .contextMenu; }

   // X3DCoreContext.getElement: the <x3d-canvas>.
   getElement () { return this .#element; }

   #fallback (message)
   {
      const slot = document .createElement ("slot");

      slot .textContent = message;
      this .#ui ?.dispose ();
      this .#canvas .parentNode ?.remove ();
      this .#canvas .remove ();
      this .#element .shadowRoot .append (slot);
   }

   get [_engine] () { return this .#chain ?? this .#engine; }

   // The engine calls of the page's code in the engine's route chain (`engine` is `chainEngine`), or null: the engine itself.
   // Returns what was set before.
   [_inChain] (engine)
   {
      const previous = this .#chain;

      this .#chain = engine;
      return previous;
   }

   // Calls that need the `WebBrowser` while the engine runs its route chain (it is borrowed) wait here until the chain is over.
   #held = [ ];

   #afterChain (call)
   {
      if (this .#chain)
         this .#held .push (call);
      else
         call ();
   }

   [_chainEnd] ()
   {
      const held = this .#held .splice (0);

      for (const call of held)
         call ();
   }

   // X3DBrowser.name, version, getName, getVersion: this engine's own name and version.
   get name ()    { return "X_ITE_R"; }
   get version () { return "0.1.0"; }
   getName ()     { return this .name; }
   getVersion ()  { return this .version; }

   // X3DBrowser.description, getDescription, setDescription and Notification.js. The text is the SFString `string` of
   // X_ITE's Notification (a set value is coerced to a string, and a set always counts as a change). X_ITE handles the
   // change in its next event cycle, once for any number of sets; here that is the next microtask. The Notifications
   // option is read then; an empty string, or the option off, leaves the element as it is. Otherwise the text is
   // shown, the width is that of the text, and a timer of 5000 ms hides it (a new text clears the timer).
   #notification     = null;
   #notificationSpan = null;
   #notificationTime = null;
   #notificationPending = false;
   #description      = new Fields .SFString ();

   get description ()      { return this .getDescription (); }
   set description (value) { this .setDescription (value); }

   getDescription () { return this .#description .getValue (); }

   setDescription (value)
   {
      this .#description .setValue (value);

      if (this .#notificationPending)
         return;

      this .#notificationPending = true;
      queueMicrotask (() => { this .#notificationPending = false; this .#showNotification (); });
   }

   #showNotification ()
   {
      if (!this .getBrowserOption ("Notifications") || this .#description .getValue () .length === 0)
         return;

      clearTimeout (this .#notificationTime);

      this .#notificationSpan .textContent = this .#description .getValue ();

      Object .assign (this .#notification .style,
      {
         visibility: "visible",
         width: `${this .#notificationSpan .clientWidth}px`,
         transition: "width 300ms ease-in-out",
      });

      this .#notificationTime = setTimeout (() =>
      {
         Object .assign (this .#notification .style,
         {
            visibility: "hidden",
            width: "0px",
            transition: "visibility 0s 300ms, width 300ms ease-in-out",
         });
      },
      5000);
   }

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

   // The derived rendering properties (BrowserOptions.set_Shading__, set_ContentScale__, set_Multisampling__,
   // set_Antialiased__). Multisampling is clamped to the samples the device supports (X_ITE: getMaxSamples ()), and
   // Antialiased is true while that count is above 0, whatever the Antialiased option says.
   #derived = { Shading: "GOURAUD", Antialiased: true, ContentScale: 1, Multisampling: 4 };

   #maxSamples () { return this .#engine ?.maxSamples ?? 4; }

   #updateShading ()
   {
      const text = this .#options .get ("Shading") .getValue () .replace ("POINTSET", "POINT");

      this .#derived .Shading = SHADINGS .includes (text) ? text : "GOURAUD";
      this .#engine ?.setShading (this .#derived .Shading);
   }

   #updateContentScale ()
   {
      const value = this .#options .get ("ContentScale") .getValue ();

      this .#derived .ContentScale = value === -1 ? this .#contentScale () : Math .max (value, 0) || 1;
   }

   #updateMultisampling ()
   {
      const requested = this .#options .get ("Multisampling") .getValue ();
      const samples   = Math .min (Math .max (isNaN (requested) ? 4 : requested, 0), this .#maxSamples ());

      this .#derived .Multisampling = this .#options .get ("Antialiased") .getValue () ? samples : 0;
      this .#derived .Antialiased   = samples > 0;
      this .#engine ?.setSampleCount (this .#derived .Multisampling);
   }

   // Applies every derived property to the engine, once it exists.
   #applyDerivedRendering ()
   {
      this .#updateShading ();
      this .#updateContentScale ();
      this .#updateMultisampling ();
   }

   getBrowserOption (name) { return X3DBrowser .#field (this .#options, name, "BrowserOptions", true) .getValue (); }

   setBrowserOption (name, value)
   {
      const field = X3DBrowser .#field (this .#options, name, "BrowserOptions", true);

      field .setValue (value);

      this .#ui ?.optionChanged (field === this .#options .get ("Timings") ? "Timings" : name);

      // BrowserOptions.addInterest: the options with an effect that is not read when it is needed. ContentScale and
      // MaximumFrameRate are read by the frame loop; Mute and SoundIntensity go to the engine (the gain of its sound
      // destinations); the rest is stored (API.md).
      if (field === this .#options .get ("AutoUpdate"))
         this .#setAutoUpdate ();

      if (field === this .#options .get ("Mute") || field === this .#options .get ("SoundIntensity"))
         this .#applySoundOptions ();

      const engineOption = X3DBrowser .#engineOptions .find (name => this .#options .get (name) === field);

      if (engineOption)
         this .#applyEngineOption (engineOption);

      if (field === this .#options .get ("Shading"))
         this .#updateShading ();

      if (field === this .#options .get ("ContentScale"))
         this .#updateContentScale ();

      if (field === this .#options .get ("Multisampling") || field === this .#options .get ("Antialiased"))
         this .#updateMultisampling ();
   }

   // The options the shared engine reads itself (WebBrowser.setEngineOption): EnableInlineViewpoints
   // (X3DLayerNode.getUserViewpoints), LoadUrlObjects (X3DUrlObject.requestImmediateLoad), StraightenHorizon
   // (BrowserOptions.set_StraightenHorizon__, X3DBrowser.viewAll) and Gravity (X3DRenderObject.gravitate).
   static #engineOptions = [ "EnableInlineViewpoints", "LoadUrlObjects", "StraightenHorizon", "Gravity", "Cache" ];

   #applyEngineOptions ()
   {
      for (const name of X3DBrowser .#engineOptions)
         this .#applyEngineOption (name);
   }

   #applyEngineOption (name)
   {
      if (!this .#engine)
         return;

      const value = this .#options .get (name) .getValue ();

      this .#engine .setEngineOption (name, typeof value === "boolean" ? value : false, typeof value === "number" ? value : 0);
   }

   // The options Mute and SoundIntensity, now and for the worlds to come (the engine keeps them).
   #applySoundOptions ()
   {
      if (!this .#engine)
         return;

      this .#engine .setSoundOptions (this .#options .get ("Mute") .getValue (), this .#options .get ("SoundIntensity") .getValue ());
      this .#flushAudio (this .#engine);
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

      if (RENDERING_DERIVED .includes (name))
         return this .#derived [ALIASES [name] ?? name];

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

   // The world's PROFILE and COMPONENT statements put the node types of their external components into X3DConstants.
   #worldConstants ()
   {
      if (!this .#engine || this .#engine .current === undefined)
         return;

      const [ profile, list ] = this .#engine .profileAndComponents (this .#engine .current);

      worldComponentConstants (profile ?? "", Array .from (list ?? [ ]));
   }

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
   async #loaded (id, aborted, started = null)
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

         // The loads of the world have started: what DOMIntegration does when the world was replaced.
         if (started)
         {
            const callback = started;

            started = null;
            callback ();
         }

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
         this .#ui .setBrowserLoading (true);

         this .#load (url, loader) .then (id =>
         {
            if (this .#fileLoader !== loader)
            {
               this .#engine ?.discard (id);
               return reject (new Error ("Loading of X3D file aborted."));
            }

            this .#fileLoader = null;
            this .#ui .loadSucceeded ();
            this .#replaceWorld (id, true) .then (resolve, reject);
         },
         () =>
         {
            if (this .#fileLoader !== loader || this .#disposed)
               return reject (new Error ("Loading of X3D file aborted."));

            // The engine reported the candidates that failed (FileLoader.printError).
            this .#fileLoader = null;
            this .#ui .loadFailed ();
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
         // X_ITE's classic parser logs the last line of its report (`Error: Expected '}' ...`) and rejects with the report.
         const report = String (error ?.message ?? error) .match (/(?:^|\n)\*+\nParser error at line [^]*\n([^\n]+)\n\*+\n?$/);

         if (report)
            console .error (new Error (report [1] .replace (/^Unexpected end of file\. (?=Expected)/, "")));

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
      this .#engine .setEncoding (id, "SCRIPTED"); // A scene that no parser made.
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
      let html = null;
      let prepared = null;

      if (typeof dom === "string")
         text = dom;
      else if (dom instanceof HTMLElement)
      {
         await this .#ready;

         // X_ITE's XMLParser reads an `<X3D>` element; any other element gives a scene with a profile and nothing in it.
         if (dom .nodeName !== "X3D")
            return this .#emptyScene ();

         html = htmlToXML (dom);
         text = html .xml;
         prepared = importHooks .get (dom);
         importHooks .delete (dom);
      }
      else if (dom instanceof Node)
         text = new XMLSerializer () .serializeToString (dom);

      if (text !== null)
      {
         const xml = new DOMParser () .parseFromString (text, "application/xml");

         if (xml .getElementsByTagName ("parsererror") .length || xml .documentElement ?.nodeName !== "X3D")
            text = null;
      }

      if (text === null)
         return this .#emptyScene ();

      // DOMIntegration finds the node of an element with `htmlImports`.
      const scene = await this .#importText (text, html && (scene =>
      {
         htmlImports .set (scene, html);
         prepared ?.prepared (scene);
      }), prepared ?.started);

      return scene;
   }

   // The scene of text that createX3DFromString parses, with the page's URL as its world URL.
   async #importText (text, prepared = null, started = null)
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

      // DOMIntegration looks at the Inline elements of the world while their loads run, as X_ITE does.
      prepared ?.(this .#sceneOf (id));
      await this .#loaded (id, () => this .#disposed, started);
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
         const engine = this .#engine;
         const handle = scene instanceof X3DScene ? scene [_node] : null;
         const mine   = handle ?.browser === this && handle .scene !== null;
         let id       = null;
         let owned    = false;
         let same     = false;
         let fill     = null;
         let revive   = null;

         if (mine && (!engine .exists (handle .scene) || engine .worldGeneration (handle .scene) !== handle .generation))
         {
            // A scene of a world that was replaced: X_ITE keeps it usable, and it becomes the world again with what it held.
            // The engine has disposed that world, so the scene is made again from the text kept when the world went.
            revive = replacedScenes .get (scene);

            if (!revive)
               engineOf (handle);

            console .error ("X_ITE_R: Browser.replaceWorld: the scene belongs to a world that was replaced; the world is made again from the scene's text as it was then (nodes of the old world stay unusable).");
         }
         else if (mine)
         {
            id = this .#sceneId (scene);

            // The scene that is the world: X_ITE runs the same shutdown and initialize events and keeps it.
            if (id === engine .current)
            {
               same = true;
               console .error ("X_ITE_R: Browser.replaceWorld: the scene is already the world; it stays the world.");
            }
         }
         else if (scene !== null && scene !== undefined)
         {
            // X_ITE takes any other value for no scene: an empty world, or for an MFNode a world with its nodes as root nodes.
            console .error ("X_ITE_R: Browser.replaceWorld: the argument is no X3DScene of this browser; the world is replaced by " +
               (scene instanceof Fields .MFNode ? "a world with the root nodes of the MFNode." : "an empty world."));

            if (scene instanceof Fields .MFNode)
            {
               const list = Array .from (scene);

               fill = target => this .#fillRootNodes (target, list);
            }
         }

         if (!revive && id === null)
         {
            // An empty scene has nothing to load.
            id = engine .prepareEmpty (this .baseURL, Date .now () / 1000);
            engine .loaded (id, Date .now () / 1000);
            owned = true;
         }

         // `this [_fileLoader] ?.abort ()`: a loadURL that is still fetching fails.
         if (this .#fileLoader)
            this .#abortLoad (this .#fileLoader);

         if (revive)
         {
            return this .#reviveScene (scene, revive) .then (revived => this .#replaceWorld (revived, false, scene));
         }

         return this .#replaceWorld (id, owned, owned ? null : scene, same, fill);
      }
      catch (error)
      {
         return Promise .reject (error);
      }
   }

   // The root nodes of the MFNode `list` of replaceWorld as the root nodes of the empty scene `id`: the nodes are printed
   // and made again in `id` (the old world is still current), as X_ITE's scene takes nodes of another scene. A NULL or
   // a node of a world that is gone stays NULL.
   #fillRootNodes (id, list)
   {
      const engine = this .#engine;
      const groups = new Map ();
      const roots  = list .map (() => null);

      list .forEach ((node, i) =>
      {
         const h = node ?.[_node] ?? handleOf (node);

         if (!h || h .browser !== this || h .released || !engine .exists (h .scene) || engine .worldGeneration (h .scene) !== h .generation)
            return;

         if (!groups .has (h .scene))
            groups .set (h .scene, [ ]);

         groups .get (h .scene) .push ([ i, h .id ]);
      });

      for (const [ scene, entries ] of groups)
      {
         const text = engine .printNodes (scene, "vrml", "MFNode", entries .map (e => e [1]));
         const made = engine .createVrmlFromString (id, text, this .baseURL);

         entries .forEach ((e, k) => { roots [e [0]] = k < made .length && made [k] >= 0 ? Number (made [k]) : null; });
      }

      engine .setRootNodes (id, roots);
      this .#flushLog (id);
   }

   // A scene of a world that was replaced, made again: the text and world URL kept when the world went give a new engine
   // scene, which the page's X3DScene object takes over (its identity stays). Resolves with the engine scene id.
   async #reviveScene (scene, kept)
   {
      const handle = scene [_node];
      const engine = this .#engine;
      const now    = Date .now () / 1000;
      const id     = engine .prepareString (kept .text, kept .url, now);

      this .#flushLog (id);
      await this .#loaded (id, () => this .#disposed);

      handle .scene      = id;
      handle .generation = engine .worldGeneration (id);
      sceneRegistry .register (scene, { browser: this, scene: id });
      this .#scenes .set (`${id}:${handle .generation}`, new WeakRef (scene));
      return id;
   }

   // The text of the world that a replace removes, kept with its X3DScene object: X_ITE keeps a replaced scene usable, so the
   // scene can become the world again (replaceWorld). The object holds it, and the text goes with the object.
   #keepText ()
   {
      const engine  = this .#engine;
      const current = engine .current;
      const outgoing = this .#scene;

      if (current === undefined || !outgoing || outgoing [_node] .scene !== current)
         return;

      try
      {
         replacedScenes .set (outgoing, { text: engine .printScene (current, "vrml"), url: engine .worldURL (current) });
      }
      catch (error)
      {
         // A world that cannot print is not made again: its scene stays stale.
      }
   }

   // X3DBrowser.replaceWorld of a loaded engine scene: the old world is still current while its shutdown runs, one
   // frame after the call; "initialized load" follows in the next frame (checkLoadCount). A newer replaceWorld
   // rejects this one with "Replacing world aborted.". `owned`: the adapter made the scene (loadURL, the empty
   // world), so a call that a newer one replaced releases it, unless the newer call uses the same scene. A scene of
   // the page (createX3DFromString) stays: its X3DScene releases it.
   #replaceWorld (id, owned, scene = null, same = false, fill = null)
   {
      return new Promise ((resolve, reject) =>
      {
         this .#reject ?.(new Error ("Replacing world aborted."));
         this .#ui .setBrowserLoading (true);
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

            if (!same)
            {
               fill ?.(id);
               this .#releaseInput ();
               // The clock the engine keeps is the page's latest: with the canvas detached no frame has given a newer one.
               this .#audioClock (this .#engine);
               this .#keepText ();
               this .#engine .replace (id, Date .now () / 1000);
               // The replaced world's sources stop now, not at the next frame (the canvas may be detached).
               this .#flushAudio (this .#engine);
               this .#scene = this .#sceneOf (id);
               this .#flushLog ();
               this .setDescription ("");
            }

            await nextFrame ();

            if (!current ())
               return;

            this .#reject         = null;
            this .#replacing      = null;
            this .#replacingScene = null;
            this .#worldConstants ();
            this .#ui .setBrowserLoading (false);
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
      this .#afterChain (() => this .#engine ?.setLive (this .#live));
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
   // As X_ITE's: only 2 or 3 arguments register (2: every event type), only 1 or 2 remove, an event type that is not one of the
   // five is added by a 3-argument add and throws a TypeError in a 2-argument remove; getBrowserCallbacks () with no argument
   // is the Map of every registration (a key twice if it is in two event types).
   addBrowserCallback (... args)
   {
      if (args .length === 2)
      {
         for (const map of this .#callbacks .values ())
            map .set (args [0], args [1]);
      }
      else if (args .length === 3)
      {
         const [ key, event, callback ] = args;
         const map = new Map (this .#callbacks .get (event));

         this .#callbacks .set (event, map);
         map .set (key, callback);
      }
   }

   removeBrowserCallback (... args)
   {
      if (args .length === 1)
      {
         for (const map of this .#callbacks .values ())
            map .delete (args [0]);
      }
      else if (args .length === 2)
      {
         this .#callbacks .get (args [1]) .delete (args [0]);
      }
   }

   getBrowserCallbacks (event)
   {
      if (arguments .length === 1)
         return this .#callbacks .get (event);

      return new Map ([ ... this .#callbacks ] .flatMap (([ , map ]) => [ ... map ]));
   }

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
               this .#worldConstants ();
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

      // Script.loadData: the page ends, so the Scripts get `shutdown ()` (X_ITE listens for `unload` on the window too).
      on (window, "unload", () => { if (this .#engine && !this .#chain) this .#unloadScripts (this .#engine); });

      // ContextMenu.initialize: the `contextmenu` event of the element.
      on (this .#element, "contextmenu", event => this .#ui .showMenu (event));

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

            // PointingDevice.touchstart: a long tap (1200 ms) shows the context menu if no sensor is under the finger (the
            // pointer cursor stands for that here) and the finger stays within 7 pixels.
            clearTimeout (this .#touchMenu);

            if (this .#touches .size === 1 && engine () .cursor !== "pointer")
            {
               const start = [ event .pageX, event .pageY ];

               this .#touchStart = start;
               this .#touchMenu  = setTimeout (() => this .#ui .showMenu ({ preventDefault () { }, pageX: start [0], pageY: start [1] }), 1200);
            }
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

               if (Math .hypot (this .#touchStart [0] - event .pageX, this .#touchStart [1] - event .pageY) > 7)
                  clearTimeout (this .#touchMenu);
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
            clearTimeout (this .#touchMenu);
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
            // X3DCoreContext [_keydown], [_keyup]: first the browser's own keys, then the KeySensor feed.
            if (type === "keydown")
               this .#browserKeyDown (event);
            else
               this .#browserKeyUp (event);

            if (!engine ())
               return;

            engine () .key (type === "keydown", event .key, event .keyCode);
            this .#afterEngine ();
         });
      }

      // A hidden page draws nothing; it draws again when it is shown.
      on (document, "visibilitychange", () => document .visibilityState === "hidden" ? this .#stop () : this .#start ());
   }

   // X3DCoreContext.getControlKey: the state of the Ctrl key as the browser's own keydown and keyup handlers keep it (the
   // keyCode of the event, not event .ctrlKey), so a keyup the page never saw leaves the key down. Shift and the other
   // modifiers are not kept: only Shift+F8 (not done) reads them.
   #controlKey = false;
   #shiftKey = false;
   #touchMenu = null;
   #touchStart = [ 0, 0 ];

   // X3DCoreContext [_keydown] (event). The keys call event .preventDefault () where X_ITE does, and only there. Ctrl+1 to
   // Ctrl+5 (Shading) and Ctrl+S (begin/end update) act only if the Debug option is set; their `setDescription` notification
   // is shown where the browser has a description (see API.md). Not done: Shift+F8 (copies the active viewpoint as text).
   #browserKeyDown (event)
   {
      const shading = (name, text) =>
      {
         if (! this .getBrowserOption ("Debug") || ! this .#controlKey)
            return;

         event .preventDefault ();
         this .setBrowserOption ("Shading", name);
         this .setDescription ?.(`Shading: ${text}`);
      };

      switch (event .keyCode)
      {
         case 16: this .#shiftKey = true; break;
         case 17: this .#controlKey = true; break;
         case 49: shading ("POINT",     "Pointset");  break;
         case 50: shading ("WIREFRAME", "Wireframe"); break;
         case 51: shading ("FLAT",      "Flat");      break;
         case 52: shading ("GOURAUD",   "Gouraud");   break;
         case 53: shading ("PHONG",     "Phong");     break;
         case 83: // s
         {
            if (! this .getBrowserOption ("Debug") || ! this .#controlKey)
               break;

            event .preventDefault ();

            if (this .isLive ())
               this .endUpdate ();
            else
               this .beginUpdate ();

            this .setDescription ?.(this .isLive () ? "Begin Update" : "End Update");
            break;
         }
         case 171: // Plus // Firefox
         case 187: // Plus // Opera
         {
            if (! this .#controlKey)
               break;

            event .preventDefault ();
            this .setBrowserOption ("Timings", ! this .getBrowserOption ("Timings"));
            break;
         }
         case 119: // F8
         {
            if (this .#shiftKey)
            {
               event .preventDefault ();
               this .#copyViewpoint ();
            }

            break;
         }
         case 36: event .preventDefault (); this .firstViewpoint ();    break; // Home
         case 35: event .preventDefault (); this .lastViewpoint ();     break; // End
         case 33: event .preventDefault (); this .previousViewpoint (); break; // Page Up
         case 34: event .preventDefault (); this .nextViewpoint ();     break; // Page Down
      }
   }

   // X3DCoreContext [_keydown] Shift+F8: the active viewpoint (a Viewpoint, OrthoViewpoint or GeoViewpoint) with its user position,
   // orientation, center of rotation and field of view as a node of the world's encoding, to the clipboard. The engine makes
   // the text (the native viewer's Shift+F8 is the same function).
   #copyViewpoint ()
   {
      const text = this .#engine ?.viewpointText ();

      if (text === undefined || text === null)
         return;

      navigator .clipboard .writeText (text) .then (() =>
      {
         this .setDescription (_("Viewpoint copied to clipboard."));

         console .info ("Viewpoint copied to clipboard.");
         console .debug (text);
      },
      Function .prototype);
   }

   // X3DCoreContext [_keyup] (event): `event .which`.
   #browserKeyUp (event)
   {
      if (event .which === 16)
         this .#shiftKey = false;

      if (event .which === 17)
         this .#controlKey = false;
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
   // context, its clock stands still, as X_ITE's does. The engine's Web Audio calls are made on real nodes of this
   // context (#flushAudio), so the page plays the sound (web/API.md, "Sound output").
   #audioClock (engine)
   {
      if (!this .#audioContext)
      {
         if (!engine .hasAudio ())
            return;

         this .#audioContext = new AudioContext ();
         this .#audioMirror  = new AudioMirror (this .#audioContext, { buffer: index => this .#engine ?.audioBuffer (index), bytes: index => this .#engine ?.audioBytes (index) });
         this .#audioWaiting = true;
         this .#resumeAudio ();
      }

      engine .setAudioClock (this .#audioContext .currentTime);
   }

   // Option B of issue #140: the calls the engine's Sound nodes made on Web Audio nodes since the last flush are made on
   // real nodes of the page's AudioContext, which reach its destination (audio_mirror.mjs). The context is made first:
   // an engine that has audio has recorded the calls that make its nodes.
   #flushAudio (engine)
   {
      this .#audioClock (engine);

      if (!this .#audioMirror)
         return;

      const json = engine .takeAudioOps ();

      if (json !== undefined)
         this .#audioMirror .apply (JSON .parse (json));
   }

   // The calls the engine's MovieTextures made on <video> elements since the last frame are made (video_mirror.mjs).
   #flushVideos (engine)
   {
      const calls = engine .takeVideoOps ();

      if (calls === undefined)
         return;

      this .#videoMirror ??= new VideoMirror (() => this .#engine);
      this .#videoMirror .apply (calls);
   }

   // X3DSoundContext.startAudioElements for the AudioContext: resume () until it succeeds once.
   #resumeAudio ()
   {
      this .#audioMirror ?.startElements ();

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

         const begin = performance .now ();
         let   frameMs = 0;

         try
         {
            // The loads that started since the last frame, or that the last frame left, are what the browser's `_loadCount` event
            // finds when the frame starts its events.
            this .#ui .loadingCounts (engine .displayLoadCounts ());

            this .#audioClock (engine);
            this .#videoMirror ?.push ();
            engine .frame (time, width, height);
            frameMs = performance .now () - begin;

            // X3DSoundContext.startAudioElement: a media element started in this frame tries the waiting resume ()
            // again, so a user activation from outside the canvas also unlocks the context. The resume () comes before the
            // nodes of the frame start (X_ITE calls it when the element starts): a source's start () can itself unlock the
            // context, and the resume () then finds it running.
            if (engine .takeAudioStarts ())
            {
               this .#audioClock (engine);
               this .#resumeAudio ();
            }

            this .#flushAudio (engine);
            this .#flushVideos (engine);

            processFieldEvents (this, engine .current);
            this .#sceneRequestsDone ();

            // X3DViewpointNode.set_bound__, X3DPointingDeviceSensorNode.set_over__: `browser .setDescription (description)`.
            for (const description of engine .takeDescriptions ())
               this .setDescription (description);
         }
         catch (error)
         {
            console .error (error);
         }

         this .#ui .frame (stamp, performance .now () - begin, frameMs);

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

   connectedCallback ()
   {
      // The first world loads behind the splash screen (X3DBrowser.initialize); the attributes are read by now.
      if (!this .#firstLoad)
      {
         this .#firstLoad = true;

         if (!this .#engine)
            this .#ui .setBrowserLoading (true);
      }

      this .#start ();
      this .#checkUpdate ();
   }

   #firstLoad = false;
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
         case "splashscreen":                 this .setBrowserOption ("SplashScreen",                 boolean ?? true); this .#ui .splashScreenAttribute (); break;
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

   // The Scripts of the current world get `shutdown ()` (the browser ends: `dispose ()`, the page's `unload`), and what they print
   // goes to the console. A second call does nothing.
   #unloadScripts (engine)
   {
      try
      {
         engine .unload ();

         for (const line of engine .takeLog (engine .current) ?? [ ])
            printLogLine (line);
      }
      catch (error)
      {
         console .error (error);
      }
   }

   // X3DBrowser.dispose (): the worlds are unloaded, their requests end, the input listeners go, the frame loop
   // stops and the engine is freed.
   dispose ()
   {
      if (this .#disposed)
         return;

      this .#disposed = true;
      this .#dom ?.dispose ();

      // X_ITE's loadURL goes on after `dispose ()`: when its file arrives, replaceWorld sends the browser's `shutdown` event (it
      // does so for a browser with no load in flight never). The world that was loaded is not shut down by it.
      if (this .#fileLoader)
      {
         requestAnimationFrame (() =>
         {
            this .#call (X3DConstants .SHUTDOWN_EVENT);
            this .#dispatch ("shutdown");
         });
      }

      this .#fileLoader = null;
      this .#releaseInput ();
      this .#listeners .abort ();
      clearTimeout (this .#notificationTime);
      clearTimeout (this .#touchMenu);
      this .#ui .dispose ();
      this .#stop ();
      this .#setAutoUpdate ();

      if (this .#engine)
      {
         // In the route chain of a field callback the engine is borrowed: it is freed when the chain is over.
         const engine = this .#engine;

         this .#engine = null;
         this .#afterChain (() =>
         {
            // ISO/IEC 14772-1 4.12.3: the world is unloaded, so its Scripts get `shutdown ()` (X_ITE 16.5.0 calls none; issue #292).
            this .#unloadScripts (engine);
            engine .free ();
         });
      }

      // The AudioContext is the page's own (#audioClock makes it). X_ITE never closes its AudioContext (its browser has no such
      // step in dispose), so it is left to the page here too (issue #293); the nodes of the worlds are disconnected.
      this .#videoMirror ?.dispose ();
      this .#videoMirror  = null;
      this .#audioMirror ?.dispose ();
      this .#audioMirror  = null;
      this .#audioContext = null;
      this .#audioWaiting = false;
   }
}

class X3DCanvasElement extends HTMLElement
{
   constructor ()
   {
      super ();

      this .attachShadow ({ mode: "open", delegatesFocus: true });
      // The notification rules are those of x_ite.css (.x_ite-private-notification) with the light theme's variables
      // filled in; X_ITE's logo (an asset) is not drawn, its space is kept.
      this .shadowRoot .innerHTML = `<style>${UI_CSS}
:host { display: inline-block; width: 300px; height: 150px; }
.x_ite-private-notification-box { position: relative; height: 0; }
.x_ite-private-notification { visibility: hidden; position: absolute; top: 20px; right: 0px; width: 0px; box-sizing: content-box;
  backdrop-filter: blur(8px); background: linear-gradient(to bottom, rgba(247, 247, 252, 0.642) 0%, rgba(222, 221, 226, 0.6) 100%);
  background-clip: padding-box; border: 4px solid rgba(229, 229, 234, 0.66); border-right: none; border-top-left-radius: 16px;
  border-bottom-left-radius: 16px; padding: 5px 0px 3px 7px; color: black; font-family: X_ITE PT Sans, sans-serif; font-size: 10pt;
  line-height: 1.1; white-space: pre; pointer-events: none; }
.x_ite-private-notification span { display: inline-block; position: relative; top: -1px; padding: 0 16px 2px 22px; }</style>`;

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
   X3DProtoDeclaration,
   X3DExternProtoDeclaration,
   ProtoDeclarationArray,
   ExternProtoDeclarationArray,
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
