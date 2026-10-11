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

const wrappers = new Map (); // attached handle → wrapper (field object identity)
const nodes    = new Map (); // node id → SFNode proxy (SFNodeCache)

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
         let w = wrappers .get (r .__h);

         if (!w)
            wrappers .set (r .__h, w = wrap (r .__t, r .__h));

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
   getFieldDefinitions () { return X ("nfields", this [N]); },
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
   let w = nodes .get (id);

   if (!w)
   {
      const target = new SFNodeBase ();

      target [N] = id;
      nodes .set (id, w = new Proxy (target, nodeHandler));
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

// X3DConstants (field types and access types used by scripts).
const X3DConstants = { initializeOnly: 1, inputOnly: 2, outputOnly: 4, inputOutput: 7 };

["SFBool", "SFColor", "SFColorRGBA", "SFDouble", "SFFloat", "SFImage", "SFInt32", "SFMatrix3d", "SFMatrix3f", "SFMatrix4d", "SFMatrix4f",
 "SFNode", "SFQuaternion", "SFRotation", "SFString", "SFTime", "SFVec2d", "SFVec2f", "SFVec3d", "SFVec3f", "SFVec4d", "SFVec4f",
 "MFBool", "MFColor", "MFColorRGBA", "MFDouble", "MFFloat", "MFImage", "MFInt32", "MFMatrix3d", "MFMatrix3f", "MFMatrix4d",
 "MFMatrix4f", "MFNode", "MFQuaternion", "MFRotation", "MFString", "MFTime", "MFVec2d", "MFVec2f", "MFVec3d", "MFVec3f",
 "MFVec4d", "MFVec4f"] .forEach ((name, i) => X3DConstants [name] = i);

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
   createX3DFromString (s)     { return executionContext (currentScript); },
   createVrmlFromURL (url, node, event) { X ("createVrmlFromURL", A (url), A (node), String (event), currentScript); },
   loadURL ()                  { return Promise .resolve (); },
   addRoute (a, b, c, d)       { X ("addRoute", currentScript, A (a), String (b), A (c), String (d)); },
   deleteRoute (a, b, c, d)    { X ("deleteRoute", currentScript, A (a), String (b), A (c), String (d)); },
   beginUpdate ()              { },
   endUpdate ()                { },
   getBrowserOption ()         { },
   setBrowserOption ()         { },
   getBrowserProperty ()       { return false; },
   getRenderingProperty ()     { },
   firstViewpoint ()           { X ("brequest", "firstViewpoint"); },
   lastViewpoint ()            { X ("brequest", "lastViewpoint"); },
   nextViewpoint ()            { X ("brequest", "nextViewpoint"); },
   previousViewpoint ()        { X ("brequest", "previousViewpoint"); },
   changeViewpoint ()          { },
   viewAll ()                  { },
   getScriptNode ()            { return nodeWrapper (currentScript); },
};

function executionContext (script)
{
   return {
      get rootNodes ()        { return R (X ("rootNodes", script)); },
      get worldURL ()         { return X ("worldURL", script); },
      get specificationVersion () { return X ("specVersion", script); },
      getWorldURL ()          { return X ("worldURL", script); },
      getSpecificationVersion () { return X ("specVersion", script); },
      createNode (type)       { return R (X ("createNode", script, String (type))); },
      createProto (type)      { return R (X ("createProto", script, String (type))); },
      getNamedNode (name)     { return R (X ("namedNode", script, String (name))); },
      addRoute (a, b, c, d)   { X ("addRoute", script, A (a), String (b), A (c), String (d)); },
      deleteRoute (a, b, c, d) { X ("deleteRoute", script, A (a), String (b), A (c), String (d)); },
   };
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

      const names = [name];

      if (accessType === 7)
         names .push (name + "_changed");

      for (const n of names)
      {
         if (n in descriptors)
            continue;

         descriptors [n] = {
            get () { return R (X ("fv", h)); },
            set (v) { X ("set", h, A (v)); },
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
})(globalThis);
