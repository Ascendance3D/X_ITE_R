// Script globals for the Rust port of X_ITE's Script node (Components/Scripting/Script.js).
//
// The prelude is these files, in order (engine.rs PRELUDE):
//
//   head.js        this file: the native entry point `__x (op, …)` and `__bridge`, which the generated files call
//   math.js        generated: X_ITE's standard/Math classes (Vector3, Rotation4, Matrix4, …), verbatim
//   fields.js      generated: X_ITE's field object model (X3DObject … X3DField, every SF and MF class, the generators), verbatim
//   config.js      generated: ComponentInfo, ProfileInfo, UnitInfo and the info arrays (ComponentInfoArray … RouteArray), verbatim
//   node_types.js  generated: X3DBaseNode.getType () of every node class and X3DConstants, recorded from the pinned X_ITE
//   bridge.js      the boundary: the X3DNode a field belongs to, node fields mirrored with the Rust runtime, values in and out
//   rest.js        Browser, execution contexts, configuration, the Script global object and the entry points of engine.rs
//
// Ported from X_ITE (https://github.com/create3000/x_ite), MIT, Copyright (c) 2009 Holger Seelig
// and other contributors.
(function (global)
{
"use strict";

const X = global .__x;
const H = Symbol ("handle");

// The browser build (x3d-web, web/pkg/script.mjs): the realm is an object of the page whose prototype is the page's
// global object, and the engine is the page's (V8 in Chromium). The prelude then leaves the page's built-ins alone.
const PAGE = global .__page === true;

delete global .__x;
delete global .__page;

// X_ITE runs in the page; its modules write their polyfills to `window` (Features.js). The Script realm is the page here too.
const window = global;

// The page's console: X_ITE's modules log to it while they load (Features.js) and run. log, warn and error reach the browser's
// log; the oracle page does not record the others.
if (global .console === undefined)
{
   const text = args => args .map (a => typeof a === "string" ? a : String (a)) .join (" ");
   const none = () => { };

   global .console =
   {
      log: (... args) => { X ("console", "log", text (args)); },
      warn: (... args) => { X ("console", "warn", text (args)); },
      error: (... args) => { X ("console", "error", text (args)); },
      info: none, debug: none, trace: none, assert: none, dir: none, dirxml: none, table: none, group: none, groupCollapsed: none,
      groupEnd: none, count: none, countReset: none, time: none, timeEnd: none, timeLog: none, clear: none,
   };
}

// The well-known symbols of explicit resource management (Chromium has them; X3DObject defines [Symbol .dispose]).
for (const name of ["dispose", "asyncDispose"])
{
   if (typeof Symbol [name] !== "symbol")
      Object .defineProperty (Symbol, name, { value: Symbol (`Symbol.${name}`) });
}

// Error.captureStackTrace (target): V8's `stack` starts with a header line (`Error`, or `name: message` for an error), then the
// frames. QuickJS gives the frames only. The frames themselves are the engine's (a kept difference: they name QuickJS's source
// positions). The page's engine has V8's own.
if (!PAGE && typeof Error .captureStackTrace === "function")
{
   const captureStackTrace = Error .captureStackTrace;

   Object .defineProperty (Error, "captureStackTrace",
   {
      value: { captureStackTrace (target, ... args)
      {
         captureStackTrace .call (this, target, ... args);

         if (target !== null && typeof target === "object" && !(target instanceof Error) && typeof target .stack === "string")
            target .stack = "Error\n" + target .stack;
      } } .captureStackTrace,
      writable: true, configurable: true, enumerable: false,
   });
}

// What the generated files call (they hold no reference to the runtime themselves). bridge.js adds the rest.
const __bridge =
{
   intlFormat (scientific, precision, value) { return X ("intlfmt", scientific, precision, value); },
   fieldCallbackAdded () { },
   // V8's `this` for a call through `with (globalObject)`: the global object of the Script that runs (rest.js).
   withBase () { return global; },
};
