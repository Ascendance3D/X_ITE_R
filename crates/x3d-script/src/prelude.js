// Script globals for the Rust port of X_ITE's Script node (Components/Scripting/Script.js,
// Fields/*.js, Base/X3DField.js, Base/X3D*ArrayField.js). Field values live in the Rust
// runtime; every field object here is a handle. `__x (op, …)` is the only native entry point.
//
// Ported from X_ITE (https://github.com/create3000/x_ite), MIT, Copyright (c) 2009 Holger Seelig
// and other contributors.
(function (global)
{
"use strict";

const X = global .__x;
const H = Symbol ("handle");
const N = Symbol ("node");

delete global .__x;

// R0.3-F: the caches hold WeakRefs, like X_ITE's SFNodeCache and field objects: a wrapper that no script
// variable reaches is garbage, and `__heldNodes ()` reports the ones that are alive (release.rs).
const wrappers = new Map (); // attached handle → WeakRef of the wrapper (field object identity)

// R0.3-E: every object that holds a field handle is counted as its owner; when QuickJS frees the
// object the slot loses that owner, and a slot with no owner is released (engine.rs, `State`).
const owners = new FinalizationRegistry (h =>
{
   X ("disown", h);

   const ref = wrappers .get (h);

   if (ref && ref .deref () === undefined)
      wrappers .delete (h);
});

function own (object, h)
{
   X ("own", h);
   owners .register (object, h);
}
const nodes    = new Map (); // node id → WeakRef of the SFNode proxy (SFNodeCache)
const targets  = new Map (); // node id → WeakRef of the target of that proxy, to repoint it when the node is released

const nodeOwners = new FinalizationRegistry (id =>
{
   const ref = nodes .get (id);

   if (ref && ref .deref () === undefined)
   {
      nodes .delete (id);
      targets .delete (id);
   }
});

// Rust → JS value
function R (r)
{
   if (r === null || typeof r !== "object")
      return r;

   if ("__n" in r)
      return r .__n === null ? null : nodeWrapper (r .__n);

   if ("__h" in r)
   {
      if (r .__a)
      {
         let w = wrappers .get (r .__h) ?.deref ();

         if (!w)
            wrappers .set (r .__h, new WeakRef (w = wrap (r .__t, r .__h)));

         return w;
      }

      return wrap (r .__t, r .__h);
   }

   if (Array .isArray (r))
      return r .map (R);

   return r;
}

// JS → Rust value
function A (v)
{
   if (v instanceof X3DField)
      return { __h: v [H] };

   if (v && typeof v === "object" && v [N] !== undefined)
      return { __n: v [N] };

   if (Array .isArray (v))
      return v .map (A);

   return v;
}

const HANDLE = { };

function wrap (type, h)
{
   const C = classes [type];

   if (!C)
      return undefined;

   return new C (HANDLE, h);
}

// X3DField
class X3DField
{
   constructor (... args)
   {
      if (args [0] === HANDLE)
         this [H] = args [1];
      else
         this [H] = X ("new", this .constructor .typeName, ... args .map (A));

      own (this, this [H]);
   }
   getTypeName ()       { return this .constructor .typeName; }
   getType ()           { return X3DConstants [this .constructor .typeName]; }
   getName ()           { return X ("fname", this [H]); }
   getAccessType ()     { return X ("faccess", this [H]); }
   isInitializable ()   { return !! (this .getAccessType () & 1); }
   isInput ()           { return !! (this .getAccessType () & 2); }
   isOutput ()          { return !! (this .getAccessType () & 4); }
   isReadable ()        { return this .getAccessType () !== 2; }
   isWritable ()        { return this .getAccessType () !== 4; }
   copy ()              { return R (X ("copy", this [H])); }
   equals (other)       { return X ("eq", this [H], A (other)); }
   isDefaultValue ()    { return X ("isdef", this [H]); }
   setValue (value)     { X ("set", this [H], A (value)); }
   assign (value)       { X ("set", this [H], A (value)); }
   getValue ()          { return R (X ("fv", this [H])); }
   valueOf ()           { return this; }
   toString ()          { return X ("str", this [H]); }
   toVRMLString ()      { return X ("str", this [H]); }
   addFieldCallback ()  { }
   removeFieldCallback () { }
   addReferencesCallback () { }
   addRouteCallback ()  { }
}

function method (name)
{
   return function (... args)
   {
      return R (X ("m", this [H], name, ... args .map (A)));
   };
}

function props (C, names)
{
   names .forEach ((name, index) =>
   {
      const d = {
         get () { return X ("g", this [H], name); },
         set (v) { X ("s", this [H], name, +v); },
      };

      Object .defineProperty (C .prototype, name, Object .assign ({ enumerable: true, configurable: true }, d));
      Object .defineProperty (C .prototype, index, Object .assign ({ enumerable: false, configurable: true }, d));
   });

   C .prototype [Symbol .iterator] = function* ()
   {
      for (const name of names)
         yield this [name];
   };
}

const classes = { };

function define (typeName, base, names, methods, statics = { })
{
   const C = { [typeName]: class extends base { } } [typeName];

   Object .defineProperty (C, "typeName", { value: typeName });

   if (names)
      props (C, names);

   for (const m of methods)
      C .prototype [m] = method (m);

   Object .assign (C, statics);

   classes [typeName] = C;
   return C;
}

const vecMethods = ["abs", "add", "clamp", "distance", "divide", "divVec", "dot", "inverse", "length", "lerp", "max", "min", "multiply", "multVec", "negate", "normalize", "reflect", "subtract"];

for (const [name, comps] of [["SFVec2d", 2], ["SFVec2f", 2], ["SFVec3d", 3], ["SFVec3f", 3], ["SFVec4d", 4], ["SFVec4f", 4]])
{
   const methods = comps === 3 ? vecMethods .concat (["cross"]) : vecMethods;

   define (name, X3DField, ["x", "y", "z", "w"] .slice (0, comps), methods);
}

define ("SFColor", X3DField, ["r", "g", "b"], ["getHSV", "setHSV", "lerp", "linearToSRGB", "sRGBToLinear"]);
define ("SFColorRGBA", X3DField, ["r", "g", "b", "a"], ["getHSVA", "setHSVA", "lerp", "linearToSRGB", "sRGBToLinear"]);
define ("SFRotation", X3DField, ["x", "y", "z", "angle"], ["getAxis", "getMatrix", "getQuaternion", "inverse", "multiply", "multVec", "setAxis", "setMatrix", "setQuaternion", "slerp", "straighten"]);
define ("SFQuaternion", X3DField, ["x", "y", "z", "w"], ["conjugate", "dot", "inverse", "multiply", "multVec", "negate", "normalize", "slerp", "squad", "length", "add", "subtract", "divide"]);

const matrixMethods = ["setTransform", "getTransform", "determinant", "transpose", "inverse", "multLeft", "multRight", "multVecMatrix", "multMatrixVec", "multDirMatrix", "multMatrixDir", "translate", "rotate", "scale"];

for (const name of ["SFMatrix3d", "SFMatrix3f"])
   define (name, X3DField, null, matrixMethods);

for (const name of ["SFMatrix4d", "SFMatrix4f", "VrmlMatrix"])
   define (name, X3DField, null, matrixMethods);

for (const C of [classes .SFMatrix3d, classes .SFMatrix3f, classes .SFMatrix4d, classes .SFMatrix4f, classes .VrmlMatrix])
{
   const size = C .typeName .startsWith ("SFMatrix3") ? 9 : 16;

   for (let i = 0; i < size; ++ i)
   {
      Object .defineProperty (C .prototype, i,
      {
         get () { return X ("g", this [H], i); },
         set (v) { X ("s", this [H], i, +v); },
         configurable: true,
      });
   }

   C .prototype [Symbol .iterator] = function* () { for (let i = 0; i < size; ++ i) yield this [i]; };
}

classes .VrmlMatrix .prototype .getMatrix = function () { return this; };
classes .VrmlMatrix .prototype .setByVertexTransform = method ("setByVertexTransform");

define ("SFImage", X3DField, ["width", "height", "comp"], []);
Object .defineProperty (classes .SFImage .prototype, "array",
{
   get () { return R (X ("g", this [H], "array")); },
   set (v) { X ("s", this [H], "array", A (v)); },
});

// Scalar fields: value objects, valueOf returns the primitive.
for (const name of ["SFBool", "SFDouble", "SFFloat", "SFInt32", "SFString", "SFTime"])
{
   const C = define (name, X3DField, null, []);

   C .prototype .valueOf = function () { return X ("fv", this [H]); };
}

Object .defineProperty (classes .SFString .prototype, "length", { get () { return X ("fv", this [H]) .length; } });

// MF fields: proxies with index access.
const mfHandler =
{
   get (target, key)
   {
      if (typeof key === "string")
      {
         const index = +key;

         if (Number .isInteger (index) && index >= 0)
            return R (X ("at", target [H], index));
      }

      return Reflect .get (target, key);
   },
   set (target, key, value)
   {
      if (typeof key === "string")
      {
         const index = +key;

         if (Number .isInteger (index) && index >= 0)
         {
            X ("put", target [H], index, A (value));
            return true;
         }
      }

      return Reflect .set (target, key, value);
   },
   has (target, key)
   {
      if (typeof key === "string")
      {
         const index = +key;

         if (Number .isInteger (index) && index >= 0)
            return index < X ("len", target [H]);
      }

      return Reflect .has (target, key);
   },
   ownKeys (target)
   {
      const keys = [ ];

      for (let i = 0, l = X ("len", target [H]); i < l; ++ i)
         keys .push (String (i));

      return keys .concat (Reflect .ownKeys (target));
   },
   getOwnPropertyDescriptor (target, key)
   {
      if (typeof key === "string")
      {
         const index = +key;

         if (Number .isInteger (index) && index >= 0 && index < X ("len", target [H]))
            return { value: this .get (target, key), writable: true, enumerable: true, configurable: true };
      }

      return Reflect .getOwnPropertyDescriptor (target, key);
   },
};

class X3DArrayField extends X3DField
{
   constructor (... args)
   {
      super (... args);

      return new Proxy (this, mfHandler);
   }
   get length ()         { return X ("len", this [H]); }
   set length (value)    { X ("setlen", this [H], value >>> 0); }
   toArray ()            { const a = [ ]; for (let i = 0, l = this .length; i < l; ++ i) a .push (this [i]); return a; }
   *[Symbol .iterator] () { for (let i = 0, l = this .length; i < l; ++ i) yield this [i]; }
   push (... args)       { const a = this .toArray (); const r = a .push (... args); this .setValue (a); return r; }
   pop ()                { const a = this .toArray (); const r = a .pop (); this .setValue (a); return r; }
   shift ()              { const a = this .toArray (); const r = a .shift (); this .setValue (a); return r; }
   unshift (... args)    { const a = this .toArray (); const r = a .unshift (... args); this .setValue (a); return r; }
   splice (... args)     { const a = this .toArray (); const r = a .splice (... args); this .setValue (a); return new this .constructor (... r); }
   reverse ()            { const a = this .toArray (); a .reverse (); this .setValue (a); return this; }
   sort (f)              { const a = this .toArray (); a .sort (f); this .setValue (a); return this; }
   fill (v, s, e)        { const a = this .toArray (); a .fill (v, s, e); this .setValue (a); return this; }
   resize (n, v)         { const a = this .toArray (); while (a .length < n) a .push (v); a .length = n; this .setValue (a); }
   indexOf (v, s)        { return this .toArray () .indexOf (v, s); }
   lastIndexOf (... a)   { return this .toArray () .lastIndexOf (... a); }
   includes (v, s)       { return this .toArray () .includes (v, s); }
   find (f, t)           { return this .toArray () .find (f, t); }
   findIndex (f, t)      { return this .toArray () .findIndex (f, t); }
   findLast (f, t)       { return this .toArray () .findLast (f, t); }
   findLastIndex (f, t)  { return this .toArray () .findLastIndex (f, t); }
   forEach (f, t)        { this .toArray () .forEach (f, t); }
   every (f, t)          { return this .toArray () .every (f, t); }
   some (f, t)           { return this .toArray () .some (f, t); }
   reduce (... a)        { return this .toArray () .reduce (... a); }
   reduceRight (... a)   { return this .toArray () .reduceRight (... a); }
   join (s)              { return this .toArray () .join (s); }
   at (i)                { return this .toArray () .at (i); }
   entries ()            { return this .toArray () .entries (); }
   keys ()               { return this .toArray () .keys (); }
   values ()             { return this .toArray () .values (); }
   filter (f, t)         { return new this .constructor (... this .toArray () .filter (f, t)); }
   map (f, t)            { return new this .constructor (... this .toArray () .map (f, t)); }
   slice (s, e)          { return new this .constructor (... this .toArray () .slice (s, e)); }
   concat (... args)     { let a = this .toArray (); for (const x of args) a = a .concat (x instanceof X3DArrayField ? x .toArray () : x); return new this .constructor (... a); }
   valueOf ()            { return this; }
}

for (const name of ["MFBool", "MFColor", "MFColorRGBA", "MFDouble", "MFFloat", "MFImage", "MFInt32", "MFMatrix3d", "MFMatrix3f",
                    "MFMatrix4d", "MFMatrix4f", "MFNode", "MFQuaternion", "MFRotation", "MFString", "MFTime", "MFVec2d", "MFVec2f",
                    "MFVec3d", "MFVec3f", "MFVec4d", "MFVec4f"])
{
   define (name, X3DArrayField, null, []);
}

// SFNode (SFNodeCache): one proxy per node.
const nodeHandler =
{
   get (target, key)
   {
      if (typeof key === "string" && !(key in SFNodeProto))
      {
         const r = X ("nget", target [N], key);

         if (r !== NOFIELD)
            return R (r);
      }

      return Reflect .get (target, key);
   },
   set (target, key, value)
   {
      if (typeof key === "string")
      {
         const r = X ("nset", target [N], key, A (value));

         if (r === 1)
            return true;

         if (r === 2)
            return false;
      }

      return Reflect .set (target, key, value);
   },
   has (target, key)
   {
      if (typeof key === "string" && X ("nhas", target [N], key))
         return true;

      return Reflect .has (target, key);
   },
};

const NOFIELD = X ("nofield");

class SFNodeBase { }

const SFNodeProto =
{
   getNodeTypeName ()   { return X ("ntype", this [N]); },
   getNodeName ()       { return X ("nname", this [N]); },
   getNodeDisplayName () { return X ("nname", this [N]) .replace (/_\d+$/, ""); },
   getNodeType ()       { return [ ]; },
   getField (name)      { const r = X ("nfield", this [N], String (name)); if (r === NOFIELD) throw new Error (`Unknown field '${name}' in node class ${this .getNodeTypeName ()}.`); return R (r); },
   getFieldDefinitions () { return X ("nfields", this [N]) .map (f => ({ ... f, dataType: X3DConstants [f .dataType] })); },
   addFieldCallback ()  { },
   removeFieldCallback () { },
   getNodeUserData ()   { },
   setNodeUserData ()   { },
   removeNodeUserData () { },
   valueOf ()           { return this; },
   toString ()          { return X ("nstr", this [N]); },
   toVRMLString ()      { return X ("nstr", this [N]); },
   getValue ()          { return this; },
   copy ()              { return this; },
   equals (other)       { return other === this; },
   getTypeName ()       { return "SFNode"; },
   getType ()           { return X3DConstants .SFNode; },
   dispose ()           { },
};

Object .assign (SFNodeBase .prototype, SFNodeProto);

function nodeWrapper (id)
{
   let w = nodes .get (id) ?.deref ();

   if (!w)
   {
      const target = new SFNodeBase ();

      target [N] = id;
      w = new Proxy (target, nodeHandler);
      targets .set (id, new WeakRef (target));
      nodes .set (id, new WeakRef (w));
      nodeOwners .register (w, id);
   }

   return w;
}

function SFNode (vrmlSyntax)
{
   // Script.js SFNode: Fields .SFNode () .fromString (vrmlSyntax, executionContext).
   return nodeWrapper (X ("sfnodeFromString", String (vrmlSyntax), currentScript));
}

SFNode .prototype = SFNodeBase .prototype;
Object .defineProperty (SFNode, "typeName", { value: "SFNode" });
classes .SFNode = SFNode;


// Base/X3DInfoArray.js: the arrays of an execution context (namedNodes, importedNodes, exportedNodes, protos, externprotos, routes,
// components, units). A context's own array is a live view; the copies `filter`, `map` and `slice` make hold their values and check them.
const INFO = Symbol ("info");

const infoHandler =
{
   get (target, key)
   {
      if (typeof key === "string")
      {
         const index = + key;

         if (Number .isInteger (index) && index >= 0)
            return infoValues (target) [index];
      }

      return Reflect .get (target, key);
   },
   set (target, key, value)
   {
      if (typeof key === "string")
      {
         const index = + key;

         if (Number .isInteger (index) && index >= 0)
            return false;
      }

      return Reflect .set (target, key, value);
   },
   has (target, key)
   {
      if (typeof key === "string")
      {
         const index = + key;

         if (Number .isInteger (index) && index >= 0)
            return index < infoValues (target) .length;
      }

      return Reflect .has (target, key);
   },
   ownKeys (target)
   {
      return Object .keys (infoValues (target)) .concat (Reflect .ownKeys (target));
   },
   getOwnPropertyDescriptor (target, key)
   {
      if (typeof key === "string")
      {
         const index = + key;

         if (Number .isInteger (index) && index >= 0)
         {
            const values = infoValues (target);

            if (index < values .length)
               return { value: values [index], writable: false, enumerable: true, configurable: true };
         }
      }

      return Reflect .getOwnPropertyDescriptor (target, key);
   },
};

function infoValues (target) { const s = target [INFO] .source; return typeof s === "function" ? s () : s; }

const infoProto =
{
   *[Symbol .iterator] () { yield* infoValues (this); },
   getTypeName () { return this [INFO] .typeName; },
   has (key) { return infoValues (this) .some (v => this [INFO] .keyOf (v) === key); },
   get (key) { return infoValues (this) .find (v => this [INFO] .keyOf (v) === key); },
   equals (array) { const a = infoValues (this), b = infoValues (array); return a === b || (a .length === b .length && a .every ((v, i) => v === b [i])); },
   ... Object .fromEntries (["at", "entries", "every", "find", "findIndex", "findLast", "findLastIndex", "flat", "flatMap", "forEach", "includes", "indexOf",
                             "join", "keys", "lastIndexOf", "reduce", "reduceRight", "some", "values"] .map (name => [name, function (... args) { return Array .prototype [name] .apply (infoValues (this), args); }])),
   filter (f, t) { return copyInfo (this, Array .prototype .filter .call (this, f, t)); },
   map (f, t) { return copyInfo (this, Array .prototype .map .call (this, f, t)); },
   slice (start, end) { return copyInfo (this, Array .prototype .slice .call (this, start, end)); },
   toReversed () { return copyInfo (this, [... this] .reverse ()); },
   toSorted (f) { return copyInfo (this, [... this] .sort (f)); },
   toSpliced (start, count, ... values) { const a = [... this]; a .splice (start, count, ... values); return copyInfo (this, a); },
   with (index, value) { const a = [... this]; a [index] = value; return copyInfo (this, a); },
};

for (const key of Object .keys (infoProto))
   Object .defineProperty (infoProto, key, { enumerable: false });

function copyInfo (array, values) { const i = array [INFO]; return infoArray (i .typeName, values, i .keyOf, i .check); }

// `source` is a function (a live view) or the array of values; X3DInfoArray.add checks each value.
function infoArray (typeName, source, keyOf, check)
{
   const target = Object .create (infoProto);

   Object .defineProperties (target, { [INFO]: { value: { typeName, source, keyOf, check } }, length: { get () { return infoValues (this) .length; } } });

   if (typeof source !== "function")
   {
      const keys = new Set ();

      for (const value of source)
      {
         const key = keyOf (value);

         if (keys .has (key))
            throw new Error (`Couldn't add value to ${typeName}, key '${key}' already exists.`);

         if (!check (value))
            throw new Error (`Couldn't add value to ${typeName}, value for key '${key}' has wrong type.`);

         keys .add (key);
      }
   }

   return new Proxy (target, infoHandler);
}

const infoObjects = new WeakSet (); // ComponentInfo and UnitInfo
const componentInfos = new WeakSet ();
const profileInfos = new WeakSet ();

// X3DConstants (field types and access types used by scripts).
// Base/X3DConstants.js: access types, then one value counting up from 1001 per name.
const X3DConstants = { initializeOnly: 1, inputOnly: 2, outputOnly: 4, inputOutput: 7 };

["CONNECTION_ERROR", "BROWSER_EVENT", "INITIALIZED_EVENT", "SHUTDOWN_EVENT", "INITIALIZED_ERROR",
 "NOT_STARTED_STATE", "IN_PROGRESS_STATE", "COMPLETE_STATE", "FAILED_STATE",
 "SFBool", "SFColor", "SFColorRGBA", "SFDouble", "SFFloat", "SFImage", "SFInt32", "SFMatrix3d", "SFMatrix3f", "SFMatrix4d", "SFMatrix4f",
 "SFNode", "SFQuaternion", "SFRotation", "SFString", "SFTime", "SFVec2d", "SFVec2f", "SFVec3d", "SFVec3f", "SFVec4d", "SFVec4f", "VrmlMatrix",
 "MFBool", "MFColor", "MFColorRGBA", "MFDouble", "MFFloat", "MFImage", "MFInt32", "MFMatrix3d", "MFMatrix3f", "MFMatrix4d",
 "MFMatrix4f", "MFNode", "MFQuaternion", "MFRotation", "MFString", "MFTime", "MFVec2d", "MFVec2f", "MFVec3d", "MFVec3f",
 "MFVec4d", "MFVec4f"] .forEach ((name, i) => X3DConstants [name] = 1001 + i);

// The script node whose code is running (for Browser.currentScene and SFNode ()).
let currentScript = -1;

// Browser (a Proxy of X3DBrowser in X_ITE; only the API scripts use is provided).
const println = (... args) => X ("print", args .map (String) .join (" "));

const Browser =
{
   get name ()                 { return X ("bname"); },
   get version ()              { return X ("bversion"); },
   get description ()          { return X ("bdescription"); },
   set description (v)         { X ("bsetdescription", String (v)); },
   get currentScene ()         { return executionContext (currentScript); },
   getExecutionContext ()      { return executionContext (-1); },
   get baseURL ()              { return X ("worldURL", currentScript); },
   get currentFrameRate ()     { return 0; },
   get currentSpeed ()         { return 0; },
   getName ()                  { return X ("bname"); },
   getVersion ()               { return X ("bversion"); },
   getDescription ()           { return X ("bdescription"); },
   setDescription (v)          { X ("bsetdescription", String (v)); },
   getCurrentTime ()           { return X ("time"); },
   getCurrentSpeed ()          { return 0; },
   getCurrentFrameRate ()      { return 0; },
   getWorldURL ()              { return X ("worldURL", currentScript); },
   getBaseURL ()               { return X ("worldURL", currentScript); },
   print (... args)            { println (... args); },
   println (... args)          { println (... args); },
   createVrmlFromString (s)    { return new classes .MFNode (... X ("createVrml", String (s), currentScript) .map (R .bind (null)) .map (id => typeof id === "number" ? nodeWrapper (id) : id)); },
   createX3DFromString (s)     { return browserScene ("string", String (s)); },
   createX3DFromURL (url, node, event)
   {
      if (arguments .length === 3)
         return Browser .createVrmlFromURL (url, node, event);

      if (!(url instanceof classes .MFString))
         throw new Error ("Browser.createX3DFromURL: url must be of type MFString.");

      return browserScene ("url", url .toArray ());
   },
   createScene (... args)
   {
      return new Promise ((resolve, reject) =>
      {
         try
         {
            const [profile, ... components] = args;

            if (args .length)
            {
               if (!profileInfos .has (profile))
                  throw new Error ("Couldn't create scene: profile must be of type ProfileInfo.");

               for (const component of components)
               {
                  if (!componentInfos .has (component))
                     throw new Error ("Couldn't create scene: component must be of type ComponentInfo.");
               }
            }

            const r = X ("browserScene", currentScript, "create", args .length ? profile .name : undefined, components .map (c => [c .name, c .level]));

            resolve (executionContext (- (r .ctx + 1)));
         }
         catch (error)
         {
            reject (error);
         }
      });
   },
   importJS (json) { return browserScene ("json", typeof json === "string" ? json : JSON .stringify (json)); },
   importDocument () { return Promise .reject (new Error ("Browser.importDocument: there is no DOM in a native Script realm.")); },
   createVrmlFromURL (url, node, event)
   {
      node  = node instanceof SFNodeBase ? node : null;
      event = String (event);

      if (!(url instanceof classes .MFString))
         throw new Error ("Browser.createVrmlFromURL: url must be of type MFString.");

      if (!node)
         throw new Error ("Browser.createVrmlFromURL: node must be of type X3DNode.");

      const field = node .getField (event);

      if (!field .isInput ())
         throw new Error (`Browser.createVrmlFromURL: event named '${event}' must be a input field.`);

      if (field .getType () !== X3DConstants .MFNode)
         throw new Error (`Browser.createVrmlFromURL: event named '${event}' must be of type MFNode.`);

      // The event is sent once the scene is complete; a scene that fails to load sends nothing.
      browserScene ("url", url .toArray ()) .then (scene => field .setValue (scene .rootNodes), () => { });
   },
   loadURL ()                  { return Promise .resolve (); },
   addRoute (a, b, c, d)       { X ("addRoute", currentScript, A (a), String (b), A (c), String (d)); },
   deleteRoute (a, b, c, d)    { X ("deleteRoute", currentScript, A (a), String (b), A (c), String (d)); },
   beginUpdate ()              { },
   endUpdate ()                { },
   getBrowserOption (name)     { return browserField ("options", "BrowserOptions", name); },
   setBrowserOption (name, value) { const type = X ("optionType", String (name)); if (type === null) throw unknownField ("BrowserOptions", name); X ("setOption", String (name), convertOption (type, value)); },
   getBrowserProperty (name)   { return browserField ("properties", "BrowserProperties", name); },
   getRenderingProperty (name) { return browserField ("rendering", "RenderingProperties", name); },
   firstViewpoint ()           { X ("brequest", "firstViewpoint"); },
   lastViewpoint ()            { X ("brequest", "lastViewpoint"); },
   nextViewpoint ()            { X ("brequest", "nextViewpoint"); },
   previousViewpoint ()        { X ("brequest", "previousViewpoint"); },
   changeViewpoint ()          { },
   viewAll ()                  { },
   getScriptNode ()            { return nodeWrapper (currentScript); },
   getComponent (name, level)  { return componentInfo (X ("browserComponent", String (name), level | 0)); },
   getProfile (name)           { const p = profileInfo (String (name)); if (!p .components .length && X ("profileComponents", String (name)) === null) throw new Error (`Profile '${name}' is not supported.`); return p; },
};

// X3DBrowser.createX3DFromString / createX3DFromURL: the scene is made by the runtime (browser_load.rs), which
// settles the promise once the scene is complete (`__settle`).
const settlers = new Map ();

function browserScene (kind, argument)
{
   return new Promise ((resolve, reject) =>
   {
      const r = X ("browserScene", currentScript, kind, argument);

      if (r .error !== undefined)
         reject (new Error (r .error));
      else
         settlers .set (r .id, { resolve, reject });
   });
}

global .__settle = function (id, ok, value)
{
   const settler = settlers .get (id);

   settlers .delete (id);

   if (!settler)
      return;

   if (ok)
      settler .resolve (executionContext (- (value + 1)));
   else
      settler .reject (new Error (value));
};

// X3DBrowser.getBrowserOption …: `getField (name) .getValue ()`; X3DBaseNode.getField throws for
// an unknown name. setValue converts like the field's set (): SFBool !!, SFInt32 |0, numbers +,
// SFString String ().
function unknownField (typeName, name) { return new Error (`Unknown field '${name}' in node class ${typeName}.`); }
function browserField (set, typeName, name) { const v = X ("browserField", set, String (name)); if (v === null) throw unknownField (typeName, name); return v; }
function convertOption (type, value)
{
   switch (type)
   {
      case "SFBool":   return !! value;
      case "SFInt32":  return value | 0;
      case "SFString": return String (value);
      default:         return + value;
   }
}

// Configuration/{ComponentInfo,ProfileInfo,UnitInfo}.js (title and providerURL not kept).
function componentInfo ([name, level]) { const o = Object .freeze ({ name, level, title: undefined, providerURL: undefined }); infoObjects .add (o); componentInfos .add (o); return o; }
function componentArray (list)         { return infoArray ("ComponentInfoArray", list .map (componentInfo), c => c .name, c => infoObjects .has (c)); }
function profileInfo (name)
{
   if (name === null)
      return null;

   const components = X ("profileComponents", name);

   const o = Object .freeze ({ name, title: undefined, providerURL: undefined, components: componentArray (components || [ ]) });

   profileInfos .add (o);
   return o;
}

// X3DScene.fromUnit / toUnit ("acceleration:" is X_ITE's spelling: acceleration is never converted).
function unitConversion (units, category, value, from)
{
   const factor = name => units .get (name) .conversionFactor;
   const apply = f => from ? value * f : value / f;

   switch (category)
   {
      case "angle": case "force": case "length": case "mass": return apply (factor (category));
      case "acceleration:": return apply (factor ("length"));
      case "angularRate": return apply (factor ("angle"));
      case "area": return apply (factor ("length") ** 2);
      case "speed": return apply (factor ("length"));
      case "volume": return apply (factor ("length") ** 3);
   }

   return value;
}

// One object per execution context (`Browser .currentScene === Browser .currentScene`).
const contextObjects = new Map ();

function executionContext (script)
{
   const key = X ("ctxOf", script);

   let object = contextObjects .get (key);

   if (!object)
      contextObjects .set (key, object = makeExecutionContext (- (key + 1)));

   return object;
}

function makeExecutionContext (script)
{
   const info = () => X ("sceneInfo", script);
   const node = n => R (n);
   const metaValues = v => Array .isArray (v) ? v .map (String) : [String (v)];

   // One live array per context.
   const named    = infoArray ("NamedNodesArray", () => X ("nodeArrays", script, "named") .map (R), n => n .getNodeName (), n => n instanceof SFNodeBase);
   const imported = infoArray ("ImportedNodesArray", () => X ("nodeArrays", script, "imported") .map (m => importedObject (script, m .id)), n => n .getImportedName (), n => importedKeys .has (n));
   const exported = infoArray ("ExportedNodesArray", () => X ("nodeArrays", script, "exported") .map (m => exportedObject (script, m .id)), n => n .getExportedName (), n => exportedKeys .has (n));
   const protos   = infoArray ("ProtoDeclarationArray", () => X ("cx", script, "protoIds", false) .map (id => protoObject (false, id)), p => p .name, p => protoKeys .get (p)?.[0] === false);
   const externs  = infoArray ("ExternProtoDeclarationArray", () => X ("cx", script, "protoIds", true) .map (id => protoObject (true, id)), p => p .name, p => protoKeys .get (p)?.[0] === true);
   const routes   = infoArray ("RouteArray", () => X ("cx", script, "routeList") .map (r => routeObject (script, r .id)), r => r .getId (), r => routeIds .has (r));
   const comps    = infoArray ("ComponentInfoArray", () => info () .components .map (componentInfo), c => c .name, c => infoObjects .has (c));
   const units    = infoArray ("UnitInfoArray", () => info () .units .map (([category, name, conversionFactor]) => { const o = Object .freeze ({ category, name, conversionFactor }); infoObjects .add (o); return o; }), u => u .category, u => infoObjects .has (u));

   return {
      get isScene ()          { return info () .isScene; },
      get encoding ()         { return info () .encoding; },
      get profile ()          { return profileInfo (info () .profile); },
      get components ()       { return comps; },
      get units ()            { return units; },
      get protos ()           { return protos; },
      get externprotos ()     { return externs; },
      get routes ()           { return routes; },
      get namedNodes ()       { return named; },
      get importedNodes ()    { return imported; },
      get exportedNodes ()    { return exported; },
      getNamedNodes ()        { return named; },
      getImportedNodes ()     { return imported; },
      getExportedNodes ()     { return exported; },
      getTypeName ()          { return info () .isScene ? "X3DScene" : "X3DExecutionContext"; },
      getEncoding ()          { return this .encoding; },
      getProfile ()           { return this .profile; },
      getComponents ()        { return comps; },
      hasComponent (name, level = 0) { if (name && typeof name === "object") ({ name, level } = name); return X ("hasComponent", script, String (name), level | 0); },
      addComponent (c)        { X ("addComponent", script, String (c .name), c .level | 0); },
      getUnits ()             { return units; },
      getUnit (category)      { return units .get (String (category)); },
      fromUnit (category, value) { return unitConversion (units, category, value, true); },
      toUnit (category, value) { return unitConversion (units, category, value, false); },
      getMetaData (name)      { const v = info () .meta .filter (e => e [0] === String (name)) .map (e => e [1]); return v .length ? v : undefined; },
      getMetaDatas ()         { return info () .meta; },
      setMetaData (name, v)   { X ("metaData", script, "set", String (name), metaValues (v)); },
      addMetaData (name, v)   { X ("metaData", script, "add", String (name), metaValues (v)); },
      removeMetaData (name)   { X ("metaData", script, "remove", String (name)); },
      addRootNode (n)         { rootNodeArgument (n); X ("rootNode", script, "add", A (n)); },
      removeRootNode (n)      { rootNodeArgument (n); X ("rootNode", script, "remove", A (n)); },
      addNamedNode (name, n)  { X ("cx", script, "named", "add", String (name), A (n)); },
      updateNamedNode (name, n) { X ("cx", script, "named", "update", String (name), A (n)); },
      removeNamedNode (name)  { X ("cx", script, "named", "remove", String (name)); },
      getUniqueName (name)    { return X ("cx", script, "unique", "named", String (name)); },
      getUniqueImportName (name) { return X ("cx", script, "unique", "imported", String (name)); },
      getUniqueProtoName (name) { return X ("cx", script, "unique", "proto", String (name)); },
      getUniqueExternProtoName (name) { return X ("cx", script, "unique", "extern", String (name)); },
      getUniqueExportName (name) { return X ("cx", script, "unique", "export", String (name)); },
      getImportedNode (name)  { return R (X ("cx", script, "importedGet", String (name))); },
      getLocalNode (name)     { return R (X ("cx", script, "localNode", String (name))); },
      getNodeOrImportedNode (n) { return R (X ("cx", script, "nodeOrImported", A (n))); },
      addImportedNode (inline, exportedName, importedName = exportedName, description = "") { X ("cx", script, "importedAdd", A (inline), String (exportedName), importedName === undefined ? undefined : String (importedName), String (description)); },
      updateImportedNode (inline, exportedName, importedName, description = "") { X ("cx", script, "importedUpdate", A (inline), String (exportedName), importedName === undefined ? undefined : String (importedName), String (description)); },
      renameImportedNode (oldName, newName) { X ("cx", script, "importedRename", String (oldName), String (newName)); },
      removeImportedNode (name) { X ("cx", script, "importedRemove", String (name)); },
      addExportedNode (name, n, description = "") { X ("cx", script, "exportedUpdate", String (name), A (n), String (description), "add"); },
      updateExportedNode (name, n, description = "") { X ("cx", script, "exportedUpdate", String (name), A (n), String (description), "update"); },
      removeExportedNode (name) { X ("cx", script, "exportedRemove", String (name)); },
      getExportedNode (name)  { return R (X ("exportedNode", script, "get", String (name))); },
      getProtoDeclaration (name) { return protoObject (false, X ("cx", script, "protoGet", false, String (name))); },
      getExternProtoDeclaration (name) { return protoObject (true, X ("cx", script, "protoGet", true, String (name))); },
      addProtoDeclaration (name, p) { X ("cx", script, "protoOp", false, "add", String (name), protoId (false, p)); },
      updateProtoDeclaration (name, p) { X ("cx", script, "protoOp", false, "update", String (name), protoId (false, p)); },
      removeProtoDeclaration (name) { X ("cx", script, "protoOp", false, "remove", String (name)); },
      addExternProtoDeclaration (name, p) { X ("cx", script, "protoOp", true, "add", String (name), protoId (true, p)); },
      updateExternProtoDeclaration (name, p) { X ("cx", script, "protoOp", true, "update", String (name), protoId (true, p)); },
      removeExternProtoDeclaration (name) { X ("cx", script, "protoOp", true, "remove", String (name)); },
      getRoutes ()            { return routes; },
      getRootNodes ()         { return this .rootNodes; },
      setRootNodes (value)    { X ("cx", script, "rootSet", A (value)); },
      getRoute (a, b, c, d)   { const id = X ("cx", script, "routeGet", A (a), String (b), A (c), String (d)); return id === null ? undefined : routeObject (script, id); },
      getProtoDeclarations () { return protos; },
      getExternProtoDeclarations () { return externs; },
      toVRMLString ()         { return X ("sceneString", script, "vrml"); },
      toXMLString ()          { return X ("sceneString", script, "xml"); },
      toJSONString ()         { return X ("sceneString", script, "json"); },
      get rootNodes ()        { return R (X ("rootNodes", script)); },
      getWorldInfos ()        { return R (X ("worldInfos", script)); },
      get worldURL ()         { return X ("worldURL", script); },
      get specificationVersion () { return X ("specVersion", script); },
      getWorldURL ()          { return X ("worldURL", script); },
      getSpecificationVersion () { return X ("specVersion", script); },
      createNode (type)       { return R (X ("createNode", script, String (type))); },
      createProto (type)      { return R (X ("createProto", script, String (type))); },
      getNamedNode (name)     { return R (X ("namedNode", script, String (name))); },
      addRoute (a, b, c, d)   { return routeObject (script, X ("cx", script, "routeAdd", A (a), String (b), A (c), String (d))); },
      deleteRoute (a, b, c, d)
      {
         // Case: sourceNode, sourceField, destinationNode, destinationField.
         const route = arguments .length === 4 ? this .getRoute (a, b, c, d) : a;

         if (routeIds .has (route))
            X ("cx", script, "routeDelete", routeIds .get (route));
      },
   };
}

// SFNodeCache .get (X3DCast (X3DNode, node, false)): a value that is no node is no WeakMap key.
function rootNodeArgument (n)
{
   if (!(n instanceof SFNodeBase))
      throw new TypeError ("Invalid value used as weak map key");
}

// X3DRoute objects, one per route.
const routeObjects = new Map (); // route id → object
const routeIds     = new WeakMap (); // object → route id

function routeObject (script, id)
{
   let o = routeObjects .get (id);

   if (!o)
   {
      const get = key => () => { const r = X ("cx", script, "routeList") .find (r => r .id === id); return r ? (key .endsWith ("Node") ? R (r [key]) : r [key]) : undefined; };

      o = { };
      Object .defineProperties (o, { id: { value: id }, getId: { value () { return id; } }, sourceNode: { get: get ("sourceNode"), enumerable: true }, sourceField: { get: get ("sourceField"), enumerable: true },
                                     destinationNode: { get: get ("destinationNode"), enumerable: true }, destinationField: { get: get ("destinationField"), enumerable: true } });
      routeObjects .set (id, o);
      routeIds .set (o, id);
   }

   return o;
}

// X3DImportedNode / X3DExportedNode objects, one per record.
const importedObjects = new Map ();
const importedKeys    = new WeakSet ();
const exportedObjects = new Map ();
const exportedKeys    = new WeakSet ();

function importedObject (script, id)
{
   let o = importedObjects .get (id);

   if (!o)
   {
      const record = () => X ("nodeArrays", script, "imported") .find (m => m .id === id);

      o = { };
      Object .defineProperties (o, {
         inlineNode:   { get: () => R (record () .inlineNode), enumerable: true },
         exportedName: { get: () => record () .exportedName, enumerable: true },
         exportedNode: { get: () => R (X ("cx", script, "importedExported", id)), enumerable: true },
         importedName: { get: () => record () .importedName, enumerable: true },
         description:  { get: () => record () .description, enumerable: true },
         getInlineNode: { value () { return this .inlineNode; } },
         getExportedName: { value () { return this .exportedName; } },
         getExportedNode: { value () { return this .exportedNode; } },
         getImportedName: { value () { return this .importedName; } },
         getDescription: { value () { return this .description; } },
         setDescription: { value (v) { X ("cx", script, "importedDescription", id, String (v)); } },
      });
      importedObjects .set (id, o);
      importedKeys .add (o);
   }

   return o;
}

function exportedObject (script, id)
{
   let o = exportedObjects .get (id);

   if (!o)
   {
      const record = () => X ("nodeArrays", script, "exported") .find (m => m .id === id);

      o = { };
      Object .defineProperties (o, {
         exportedName: { get: () => record () .exportedName, enumerable: true },
         localNode:    { get: () => R (record () .localNode), enumerable: true },
         description:  { get: () => record () .description, enumerable: true },
         getExportedName: { value () { return this .exportedName; } },
         getLocalNode: { value () { return this .localNode; } },
         getDescription: { value () { return this .description; } },
      });
      exportedObjects .set (id, o);
      exportedKeys .add (o);
   }

   return o;
}

// X3DProtoDeclaration / X3DExternProtoDeclaration objects, one per declaration.
const protoObjects = new Map (); // "p3" → object
const protoKeys    = new WeakMap (); // object → [extern, id]

function protoObject (extern, id)
{
   const key = (extern ? "e" : "p") + id;
   let o = protoObjects .get (key);

   if (!o)
   {
      const info = () => X ("cx", -1, "protoInfo", extern, id);
      const fields = () => Object .freeze (info () .fields .map (f => Object .freeze ({ name: f .name, accessType: f .accessType, dataType: X3DConstants [f .dataType], value: R (f .value) })));

      o = Object .create (null);
      Object .defineProperties (o, { name: { get: () => info () .name, enumerable: true }, isExternProto: { value: extern, enumerable: true }, fields: { get: fields, enumerable: true } });

      if (extern)
         Object .defineProperty (o, "urls", { get: () => R (info () .urls), enumerable: true });

      protoObjects .set (key, o);
      protoKeys .set (o, [extern, id]);
   }

   return o;
}

function protoId (extern, p)
{
   const k = p && typeof p === "object" ? protoKeys .get (p) : undefined;

   return k && k [0] === extern ? k [1] : undefined;
}

// X3DProtoDeclaration / X3DExternProtoDeclaration as the SAI sees them.
function protoArray (list)
{
   return Object .freeze (list .map (p => Object .freeze ({
      name: p .name,
      isExternProto: p .isExternProto,
      fields: Object .freeze (p .fields .map (f => Object .freeze ({ name: f .name, accessType: f .accessType, dataType: X3DConstants [f .dataType], value: R (f .value) }))),
      ... (p .isExternProto ? { urls: R (p .urls) } : { }),
   })));
}

// Math functions: V8's (fdlibm) results, not the C library's.
for (const name of ["sin", "cos", "tan", "exp", "log", "acos", "asin", "atan2", "log10", "sqrt", "atan", "sinh", "cosh", "tanh", "expm1", "log1p", "log2", "cbrt", "pow"])
{
   const f = X ("mathfn", name);

   if (f)
      Math [name] = f;
}

const standardGlobals =
{
   NULL: null, FALSE: false, TRUE: true,
   print: println, trace: println,
   Browser,
   X3DConstants,
   X3DField, X3DArrayField,
   SFNode,
   ... Object .fromEntries (Object .entries (classes) .filter (([k]) => k !== "SFNode")),
};

// Script.createGlobalObject + createContext + evaluate.js (`with (globalObject) eval (…)`).
const contexts = new Map ();

const evaluate = new Function (`with (arguments [0])
{
   return eval ((() =>
   {
      const sourceText = arguments [1];

      delete arguments [0];
      delete arguments [1];

      arguments .length = 0;

      return sourceText;
   })
   ());
}`);

// V8 wording for the QuickJS errors that historical scripts commonly raise, so that
// Script.setError prints what X_ITE in Chromium prints.
function calleeAt (stack, source)
{
   const m = /\(<input>:(\d+):(\d+)\)/ .exec (String (stack));

   if (!m)
      return undefined;

   const line = source .split ("\n") [m [1] - 1];

   if (line === undefined)
      return undefined;

   // QuickJS reports either the "(" of the call or the start of the callee's last name.
   let i = m [2] - 1;

   if (/[\w$]/ .test (line [i] ?? ""))
   {
      while (i < line .length && /[\w$]/ .test (line [i]))
         ++ i;
   }

   -- i;

   const ws = () => { while (i >= 0 && /\s/ .test (line [i])) -- i; };
   const ident = () => { const end = i; while (i >= 0 && /[\w$]/ .test (line [i])) -- i; return line .slice (i + 1, end + 1); };

   ws ();

   const parts = [ident ()];

   if (!parts [0])
      return undefined;

   for (;;)
   {
      const save = i;

      ws ();

      if (line [i] !== ".")
      {
         i = save;
         break;
      }

      -- i;
      ws ();

      const id = ident ();

      if (!id)
         return undefined;

      parts .unshift (id);
   }

   return parts .join (".");
}

function v8String (error, source)
{
   if (!(error instanceof Error))
      return String (error);

   let message = String (error .message), m;

   if ((m = /^cannot read property '(.*)' of (null|undefined)$/ .exec (message)))
      message = `Cannot read properties of ${m [2]} (reading '${m [1]}')`;
   else if ((m = /^cannot set property '(.*)' of (null|undefined)$/ .exec (message)))
      message = `Cannot set properties of ${m [2]} (setting '${m [1]}')`;
   else if (message === "not a function" && source !== undefined)
   {
      const callee = calleeAt (error .stack, source);

      if (callee)
         message = `${callee} is not a function`;
   }

   return message ? `${error .name}: ${message}` : String (error .name);
}

const sources = new Map ();

global .__create = function (script, sourceText, callbacks, fields)
{
   sources .set (script, sourceText);

   const descriptors = { };

   for (const [k, v] of Object .entries (standardGlobals))
      descriptors [k] = { value: v };

   for (const [name, accessType, h] of fields)
   {
      if (accessType === 2)
         continue;

      // The accessors hold the field's handle through this object, so that the slot goes when they do.
      const ref = { h };

      own (ref, h);

      const names = [name];

      if (accessType === 7)
         names .push (name + "_changed");

      for (const n of names)
      {
         if (n in descriptors)
            continue;

         descriptors [n] = {
            get () { return R (X ("fv", ref .h)); },
            set (v) { X ("set", ref .h, A (v)); },
         };
      }
   }

   const globalObject = Object .create (Object .prototype, descriptors);

   sourceText += "\n;\n[" + callbacks .map (c => `typeof ${c} !== "undefined" ? ${c} : undefined`) .join (",") + "];";

   const previous = currentScript;

   currentScript = script;

   try
   {
      const result = evaluate .call (nodeWrapper (script), globalObject, sourceText);
      const context = new Map ();

      callbacks .forEach ((c, i) => context .set (c, result [i]));
      contexts .set (script, context);

      return callbacks .map ((c, i) => typeof result [i] === "function");
   }
   catch (error)
   {
      throw v8String (error, sources .get (script));
   }
   finally
   {
      currentScript = previous;
   }
};

// Script.call__ / set_field__: returns the thrown value as a string, or undefined.
global .__call = function (script, name, value, time, hasValue)
{
   const callback = contexts .get (script) ?.get (name);

   if (typeof callback !== "function")
      return undefined;

   const previous = currentScript;

   currentScript = script;

   try
   {
      if (hasValue)
         callback .call (nodeWrapper (script), R (value), time);
      else
         callback .call (nodeWrapper (script), time);

      return undefined;
   }
   catch (error)
   {
      return v8String (error, sources .get (script));
   }
   finally
   {
      currentScript = previous;
   }
};

// Nodes were released and their ids may belong to other nodes soon: the wrappers that scripts still hold
// point to the empty tombstone node from now on.
global .__release = function (ids, tombstone, detached)
{
   // Their field handles keep the value they had and are not the field's any more (engine.rs): no
   // later `getField` returns these wrappers.
   for (const h of detached)
      wrappers .delete (h);

   for (const id of ids)
   {
      const target = targets .get (id) ?.deref ();

      if (target)
         target [N] = tombstone;

      targets .delete (id);
      nodes .delete (id);
      contexts .delete (id);
      sources .delete (id);
   }
};

// The nodes that JavaScript reaches (wrappers that are alive); the host collects garbage first.
global .__heldNodes = function ()
{
   const held = [ ];

   for (const [id, ref] of nodes)
   {
      if (ref .deref () !== undefined)
         held .push (id);
   }

   return held;
};

// The Script was shut down: its global object and callbacks are not needed any more.
global .__destroy = function (script)
{
   contexts .delete (script);
   sources .delete (script);
};
})(globalThis);
