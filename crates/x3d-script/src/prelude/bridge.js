
// --- bridge.js ------------------------------------------------------------------------------------------------------------
//
// The field objects a Script sees are X_ITE's own (fields.js) and hold their value in JavaScript, as in X_ITE. This file is the
// boundary to the Rust runtime, which owns the nodes:
//
// * X3DNodeRef stands where X_ITE has the X3DNode itself: the value of an SFNode, the parent of a node's fields. It is the
//   SAI-visible edge of the port (owner decision 2026-10-20): what X_ITE hands out as an internal node object is this object.
// * A node field (a field of a node, a Script's own field) is one object per field for as long as the node lives, its value
//   mirrored with the runtime: refreshed from it before a Script runs (`enter`, the fields the runtime changed), sent to it when
//   the object makes an event (X3DChildObject.addEvent reaches the X3DNodeRef parent), and sent without an event when the Script
//   changed the value in place (`leave`, X3DField.set and the internal value objects).
// * Values cross as plain data (engine.rs value_data): fieldData / fieldFromData.

// X3DConstants: the node types of a world with the Core profile (oracle/x_ite-node-types.json). The node types of other
// components are numbered after them when a node of one is first seen (X_ITE numbers them in the order the components load).
for (const [name, value] of X3D_CONSTANTS_CORE)
   X3DConstants .addConstant (name, value);

let nextConstant = Math .max (... X3D_CONSTANTS_CORE .map (([, value]) => value)) + 1;

// X3DBrowser.loadComponents: a component's dependencies first, then the component; an external one adds the constants of its
// node types as its module loads (oracle/x_ite-component-constants.json). The built-in ones are in X3D_CONSTANTS_CORE.
const componentTable  = new Map (X3D_COMPONENTS .map (([name, external, dependencies]) => [name, { external, dependencies }]));
const loadedComponents = new Set ();

function loadComponentConstants (name)
{
   if (loadedComponents .has (name))
      return;

   loadedComponents .add (name);

   const info = componentTable .get (name);

   if (!info)
      return;

   for (const dependency of info .dependencies)
      loadComponentConstants (dependency);

   if (!info .external)
      return;

   for (const constant of X3D_COMPONENT_CONSTANTS [name] ?? [ ])
   {
      if (!Object .hasOwn (X3DConstants, constant))
         X3DConstants .addConstant (constant, nextConstant ++);
   }
}

function constantOf (name)
{
   if (!Object .hasOwn (X3DConstants, name))
      X3DConstants .addConstant (name, nextConstant ++);

   return X3DConstants [name];
}

// --- X3DNodeRef --------------------------------------------------------------------------------------------------------------

// An X3DNodeRef keeps what X_ITE's node keeps: its field objects, its FieldDefinitionArray, its user data and (SFNodeCache) its
// SFNode object. The runtime owns the nodes; `pinned` holds the object of every node until the runtime releases it, except while
// release.rs asks whether a Script still reaches a node only JavaScript could hold (__unpin, __heldNodes).
const NODE        = Symbol ("node");
const FIELDS      = Symbol ("fields");      // field id (engine.rs fkey_id) → { field, h, id, data }
const DEFINITIONS = Symbol ("definitions"); // its FieldDefinitionArray
const USER_DATA   = Symbol ("userData");    // X3DObject.setUserData of the node (kept by the node, not by its SFNode object)
const nodeRefs    = new Map (); // node id → WeakRef of its X3DNodeRef
const pinned      = new Map (); // node id → its X3DNodeRef

const nodeRefRegistry = new FinalizationRegistry (id =>
{
   if (nodeRefs .get (id) ?.deref () === undefined)
      nodeRefs .delete (id);
});

function X3DNodeRef (id)
{
   X3DChildObject .call (this);

   this [NODE]        = id;
   this [FIELDS]      = new Map ();
   this [DEFINITIONS] = undefined;
   this [USER_DATA]   = undefined;
}

Object .assign (Object .setPrototypeOf (X3DNodeRef .prototype, X3DChildObject .prototype),
{
   getNodeId ()
   {
      return this [NODE];
   },
   getTypeName ()
   {
      return X ("ntype", this [NODE]);
   },
   getType ()
   {
      // A PROTO instance is an X3DPrototypeInstance (an X3DNode).
      const names = NODE_TYPES [this .getTypeName ()] ?? ["X3DNode", "X3DPrototypeInstance"];

      return names .map (constantOf);
   },
   getName ()
   {
      return X ("nname", this [NODE]);
   },
   getDisplayName ()
   {
      return this .getName () .replace (/_\d+$/, "");
   },
   getField (name)
   {
      const r = X ("nfield", this [NODE], String (name));

      if (r === NOFIELD)
         throw new Error (`Unknown field '${name}' in node class ${this .getTypeName ()}.`);

      return attachedField (r .__h, r .__t);
   },
   // X3DBaseNode [_fieldDefinitions]: one FieldDefinitionArray per node, of the class's definitions (their values are the
   // defaults) and of the user-defined fields (their values are the fields).
   getFieldDefinitions ()
   {
      return this [DEFINITIONS] ??= new FieldDefinitionArray (X ("nfields", this [NODE]) .map (fieldDefinition));
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
      pushField (field, true);
   },
   addEventObject (field)
   {
      pushField (field, true);
   },
   // The SFNode objects of the node are its parents in X_ITE (cloneCount, parentsChanged); the runtime keeps those. A node's
   // object here does not hold them: the Script releases a node it created by letting go of its objects (__heldNodes).
   // Not for a field object either (X3DField.addParent): its parents are a set it holds.
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
   toStream (generator)
   {
      generator .string += X ("nstream", this [NODE]);
   },
   toVRMLStream (generator)
   {
      generator .string += X ("nstr", this [NODE]);
   },
   toXMLStream (generator)
   {
      generator .string += X ("nprint", this [NODE], "xml");
   },
   toJSONStream (generator)
   {
      generator .string += X ("nprint", this [NODE], "json");
   },
});

for (const key of Object .keys (X3DNodeRef .prototype))
   Object .defineProperty (X3DNodeRef .prototype, key, { enumerable: false });

const classDefinitions      = new Map (); // the definitions of the node classes, one object each (X_ITE: Class .fieldDefinitions)

function fieldDefinition ({ name, accessType, dataType, value, unit, handle })
{
   if (handle !== undefined)
      return new X3DFieldDefinition (accessType, name, attachedField (handle, dataType));

   const key = JSON .stringify ([name, accessType, dataType, value, unit]);

   let definition = classDefinitions .get (key);

   if (!definition)
   {
      const field = fieldFromData (dataType, value);

      if (unit !== undefined)
         field .setUnit (unit);

      definition = new X3DFieldDefinition (accessType, name, field);
      classDefinitions .set (key, definition);
   }

   return definition;
}

function nodeRef (id)
{
   let ref = nodeRefs .get (id) ?.deref ();

   if (!ref)
   {
      ref = new X3DNodeRef (id);
      nodeRefs .set (id, new WeakRef (ref));
      nodeRefRegistry .register (ref, id);
      pinned .set (id, ref);
   }

   return ref;
}

// The field object of a node field, if the node's object has one.
function entryOf (id)
{
   return nodeRefs .get (+ id .slice (0, id .indexOf (":"))) ?.deref () ?.[FIELDS] .get (id);
}

// The node fields that have a field object.
function * entries ()
{
   for (const weak of nodeRefs .values ())
   {
      const ref = weak .deref ();

      if (ref)
         yield * ref [FIELDS] .values ();
   }
}

function nodeObject (id)
{
   return id === null || id === undefined ? null : SFNodeCache .get (nodeRef (id));
}

function nodeIdOf (sfnode)
{
   return sfnode .getValue () ?.[NODE] ?? null;
}

// --- values ------------------------------------------------------------------------------------------------------------------

const NOFIELD = X ("nofield");

// The plain data of a field object (engine.rs value_data).
function fieldData (field)
{
   const type = field .getTypeName ();

   switch (type)
   {
      case "SFBool": case "SFDouble": case "SFFloat": case "SFInt32": case "SFString": case "SFTime":
         return field .getValue ();
      case "SFNode":
         return nodeIdOf (field);
      case "SFImage":
         return [field .width, field .height, field .comp, Array .from (field .array)];
      case "SFRotation":
      {
         const r = field .getValue (), s = __bridge .rotation4, q = r [s ._quaternion];

         return [r [s ._x], r [s ._y], r [s ._z], r [s ._angle], r [s ._tainted] ? 1 : 0, q .x, q .y, q .z, q .w];
      }
      case "MFNode":
         return Array .from (field .getValue (), node => nodeIdOf (node));
      case "MFString":
         return Array .from (field .getValue (), s => s .getValue ());
      case "MFBool":
         return Array .from (field, v => !! v);
      case "MFImage":
         return Array .from (field .getValue (), image => fieldData (image));
   }

   if (type .startsWith ("MF"))
   {
      const target = field .getTarget ();

      return Array .from (target .getValue () .subarray (0, target .length * target .getComponents ()));
   }

   return Array .from (field .getValue ());
}

// A new field object of type `type` with the value `data`.
function fieldFromData (type, data)
{
   const C = Fields [type];

   switch (type)
   {
      case "SFBool": case "SFDouble": case "SFFloat": case "SFInt32": case "SFString": case "SFTime":
         return new C (data);
      case "SFNode":
         return new C (data === null ? null : nodeRef (data));
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
         return new C (... data .map (nodeObject));
      case "MFString":
         return new C (... data);
      case "MFBool":
         return new C (... data);
      case "MFImage":
         return new C (... data .map (d => fieldFromData ("SFImage", d)));
   }

   if (type .startsWith ("MF"))
   {
      const field = new C (), target = field .getTarget ();

      target .set (data, data .length / target .getComponents ());
      return field;
   }

   // Components that are not numbers (raw values, see rawToken) go into the internal value object as they are.
   if (data .some (x => typeof x !== "number"))
   {
      const field = new C (), value = field .getValue ();
      const keys  = typeof value [0] === "number" ? data .map ((_, i) => i) : ["x", "y", "z", "w"];

      data .forEach ((x, i) => value [keys [i]] = x);
      return field;
   }

   return new C (... data);
}

// Give `field` the value `data` without an event (X3DField.set): the internal value objects stay the same objects.
function setFieldData (field, data)
{
   const value = fieldFromData (field .getTypeName (), data);

   if (field instanceof X3DArrayField)
      field .getTarget () .set (value .getValue (), value .length);
   else
      field .set (value .getValue ());
}

function sameData (a, b)
{
   if (a === b)
      return true;

   if (Number .isNaN (a) && Number .isNaN (b))
      return true;

   if (!Array .isArray (a) || !Array .isArray (b) || a .length !== b .length)
      return false;

   for (let i = 0; i < a .length; ++ i)
   {
      if (!sameData (a [i], b [i]))
         return false;
   }

   return true;
}

// --- node fields -------------------------------------------------------------------------------------------------------------

const attachedOf = new WeakMap (); // field object → its entry (X3DNodeRef [FIELDS])

function attachedField (h, type)
{
   const id   = X ("fmirror", h);
   const node = nodeRef (+ id .slice (0, id .indexOf (":")));

   let entry = node [FIELDS] .get (id);

   if (entry)
      return entry .field;

   const data  = rawData (h, X ("fexport", h));
   const field = fieldFromData (type, data);
   const unit  = X ("funit", h);

   field .setName (X ("fname", h));
   field .setAccessType (X ("faccess", h));

   if (unit !== undefined)
      field .setUnit (unit);

   field .addParent (node);

   entry = { field, h, id, data };

   node [FIELDS] .set (id, entry);
   attachedOf .set (field, entry);

   // An array field is a Proxy of its target, and X_ITE's array code makes its events on the target.
   if (field instanceof X3DArrayField)
      attachedOf .set (field .getTarget (), entry);

   // So is an SFNode: SFNode.addFieldCallback (key, callback) adds the callback to its target (X3DField.prototype .call (target)).
   // The proxy lists the target's own symbols; the target is the one that is not the proxy.
   if (field instanceof SFNode)
   {
      for (const symbol of Object .getOwnPropertySymbols (field))
      {
         const value = field [symbol];

         if (value instanceof SFNode && value !== field)
            attachedOf .set (value, entry);
      }
   }

   return field;
}

function pushField (field, event)
{
   const entry = attachedOf .get (field);

   if (!entry)
      return;

   entry .data = fieldData (field);
   const token = rawToken (field, entry .data);

   X ("fpush", entry .h, entry .data, event, token, rawValues .get (token) ?.texts);
}

// X_ITE's Vector3 (and the other internal value objects) hold any JavaScript value a Script puts there; the runtime uses each
// component's ToNumber. The data with its components as they are is kept here under a token that goes with the runtime's value
// (to the fields a route passes it on to), so the Script objects of those fields hold the same components.
const rawValues = new Map ();
let rawSeq = 0;

// The field types whose internal value object (Vector2/3/4, Color3/4) a Script can fill with any value.
const rawTypes = new Set (["SFColor", "SFColorRGBA", "SFVec2d", "SFVec2f", "SFVec3d", "SFVec3f", "SFVec4d", "SFVec4f"]);

function hasRaw (field, data)
{
   return rawTypes .has (field .getTypeName ()) && Array .isArray (data) && !data .every (x => typeof x === "number");
}

function rawToken (field, data)
{
   if (!hasRaw (field, data))
      return undefined;

   const copy = data .slice ();

   rawValues .set (++ rawSeq, { data: copy, texts: textsOf (copy) });
   return rawSeq;
}

function textsOf (data)
{
   return data .map (x =>
   {
      if (typeof x === "number")
         return null;

      try { return String (x); } catch { return null; }
   });
}

// X_ITE takes `String (component)` when it prints. The runtime's texts are taken again when the Script's code has run (leave) and
// before the Script prints; the values no field holds any more are forgotten.
function rawRefresh ()
{
   if (!rawValues .size)
      return;

   const live = new Set (X ("frawlive"));

   for (const [token, raw] of rawValues)
   {
      if (!live .has (token))
      {
         rawValues .delete (token);
         continue;
      }

      const texts = textsOf (raw .data);

      if (texts .every ((t, i) => t === raw .texts [i]))
         continue;

      raw .texts = texts;
      X ("frawtexts", token, texts);
   }
}

global .__rawRefresh = rawRefresh;

// The data of a node field from the runtime, with the Script's raw components when they went with the value.
function rawData (h, data)
{
   const token = X ("frawtok", h);

   return token === undefined ? data : rawValues .get (token) ?.data ?? data;
}

// Before a Script runs: the node fields the runtime changed since.
function enter ()
{
   const dirty = X ("fdirty");

   for (let i = 0; i < dirty .length; i += 2)
   {
      const entry = entryOf (dirty [i]);

      if (!entry)
         continue;

      entry .data = rawData (entry .h, dirty [i + 1]);
      setFieldData (entry .field, entry .data);
   }
}

// After a Script ran (and its promise jobs): what it changed in place, without an event, goes to the runtime.
function leave ()
{
   for (const entry of entries ())
   {
      const data = fieldData (entry .field);

      if (sameData (data, entry .data))
         continue;

      entry .data = data;
      const token = rawToken (entry .field, data);

      X ("fpush", entry .h, data, false, token, rawValues .get (token) ?.texts);
   }

   rawRefresh ();
}

__bridge .fieldCallbackAdded = function (field)
{
   const entry = attachedOf .get (field);

   if (entry)
      X ("fobserve", entry .h, true);
};

// X3DField.processEvent of an observed node field: `callback (this .valueOf ())` for each field callback, after the value was set
// and the routes were given the event.
//
// A callback that throws ends the loop, as in X_ITE, where the exception leaves processEvent and the remaining callbacks of the
// event do not run. X_ITE's event loop does not catch it either, and its browser stops advancing (oracle fcb-throw.wrl). Owner
// decision 2026-10-20: the port reports it at error level and the runtime goes on; the field keeps its value, its routes their
// event, and the next event runs every callback again.
global .__fieldEvent = function (id)
{
   const entry = entryOf (id);

   if (!entry)
      return;

   enter ();

   const field = entry .field, previous = currentScript;

   try
   {
      for (const callback of field .getFieldCallbacks () .values ())
      {
         currentScript = callbackScripts .get (callback) ?? previous;
         callback (field .valueOf ());
      }
   }
   catch (error)
   {
      scriptConsole .error (error);
   }
   finally
   {
      currentScript = previous;
   }

   leave ();
};

// The Script that added a callback (a callback runs as that Script: Browser.currentScene).
const callbackScripts = new WeakMap ();

{
   const addFieldCallback = X3DField .prototype .addFieldCallback;

   Object .defineProperty (X3DField .prototype, "addFieldCallback",
   {
      value: { addFieldCallback (key, object)
      {
         if (typeof object === "function" && !callbackScripts .has (object))
            callbackScripts .set (object, currentScript);

         return addFieldCallback .call (this, key, object);
      } } .addFieldCallback,
      writable: true, configurable: true, enumerable: false,
   });
}

// --- X3DField.fromVRMLString / fromXMLString ---------------------------------------------------------------------------------
//
// VRMLParser.js and XMLParser.js put these on X3DField.prototype (the parsers are the runtime's here): the value is read into
// the field with setValue, so a node field makes an event. `scene` is an X3DScene the Script has (contextKeys, rest.js): the
// nodes of the value are made in it and the field's unit converts the value.

function parseField (field, value, scene, format)
{
   const key  = scene !== null && typeof scene === "object" ? contextKeys .get (scene) : undefined;
   const data = X ("parsefield", field .getTypeName (), String (value), format, field .getName (), key ?? null, key !== undefined ? field .getUnit () : undefined);

   field .setValue (fieldFromData (field .getTypeName (), data));
}

X3DField .prototype .fromVRMLString = function (value, scene)
{
   parseField (this, value, scene, "vrml");
};

X3DField .prototype .fromXMLString = function (value, scene)
{
   parseField (this, value, scene, "xml");
};

// --- Rust ↔ JavaScript ---------------------------------------------------------------------------------------------------

// Rust → JS value
function R (r)
{
   if (r === null || typeof r !== "object")
      return r;

   if ("__n" in r)
      return nodeObject (r .__n);

   if ("__h" in r)
      return r .__a ? attachedField (r .__h, r .__t) : fieldFromData (r .__t, X ("fexport", r .__h));

   if (Array .isArray (r))
      return r .map (R);

   return r;
}

// JS → Rust value
function A (v)
{
   if (v instanceof SFNode)
      return { __n: nodeIdOf (v) };

   if (v instanceof X3DField)
   {
      const entry = attachedOf .get (v);

      if (entry)
      {
         // Its current value, also when it was changed in place.
         const data = fieldData (v);

         if (!sameData (data, entry .data))
         {
            entry .data = data;
            X ("fpush", entry .h, data, false);
         }

         return { __h: entry .h };
      }

      return { __h: X ("fimport", v .getTypeName (), fieldData (v)) };
   }

   if (Array .isArray (v))
      return v .map (A);

   return v;
}

// --- node streams ------------------------------------------------------------------------------------------------------------
//
// X_ITE prints a node into the generator it was given (X3DBaseNode.toVRMLStream …). The runtime prints nodes (the scene's
// printer, with its DEF/USE names), so an SFNode or MFNode field streams its whole value at once.

function streamNodes (field, generator, kind)
{
   const h = X ("fimport", field .getTypeName (), fieldData (field));

   switch (kind)
   {
      case "stream": generator .string += X ("strn", h); break;
      case "vrml":   generator .string += X ("str", h); break;
      case "xml":    generator .string += X ("fprint", h, "xml"); break;
      case "json":   generator .string += X ("fprint", h, "json"); break;
   }
}

// SFNode streams its node (X3DNodeRef prints it); MFNode streams all its nodes in one print.
{
   const methods =
   {
      toStream (generator) { streamNodes (this, generator, "stream"); },
      toVRMLStream (generator) { streamNodes (this, generator, "vrml"); },
      toXMLStream (generator) { streamNodes (this, generator, "xml"); },
      toJSONStream (generator) { streamNodes (this, generator, "json"); },
   };

   for (const [name, value] of Object .entries (methods))
      Object .defineProperty (Fields .MFNode .prototype, name, { value, writable: true, configurable: true, enumerable: false });
}
