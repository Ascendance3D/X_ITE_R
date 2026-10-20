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

const infoConstructors = new Map ();

function infoConstructor (typeName)
{
   let f = infoConstructors .get (typeName);

   if (!f)
   {
      f = function () { };
      Object .defineProperty (f, "typeName", { value: typeName, enumerable: true });
      infoConstructors .set (typeName, f);
   }

   return f;
}

function infoValues (target) { const s = target [INFO] .source; return typeof s === "function" ? s () : s; }

const infoProto =
{
   *[Symbol .iterator] () { yield* infoValues (this); },
   getTypeName () { return this [INFO] .typeName; },
   has (key) { return this [INFO] .aliases ?.has (key) || infoValues (this) .some (v => this [INFO] .keyOf (v) === key); },
   get (key) { return this [INFO] .aliases ?.get (key) ?? infoValues (this) .find (v => this [INFO] .keyOf (v) === key); },
   // Base/X3DInfoArray.js add, alias, update, remove, clear: only the arrays that hold their values (the supported components and
   // profiles, the copies map, filter and slice make).
   add (key, value) { return infoAdd (this, key, value); },
   alias (alias, value) { infoMutable (this) .aliases .set (alias, value); },
   update (oldKey, newKey, value) { infoUpdate (this, oldKey, newKey, value); },
   remove (key) { infoRemove (this, key); },
   clear () { const i = infoMutable (this); i .source .length = 0; i .aliases .clear (); },
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

function copyInfo (array, values) { const i = array [INFO]; return infoArray (i .typeName, values, i .keyOf, i .check, i .make); }

function infoMutable (array)
{
   const i = array [INFO];

   if (typeof i .source === "function")
      throw new Error (`${i .typeName} is a view of its execution context.`);

   i .aliases ??= new Map ();
   return i;
}

function infoAdd (array, key, spec)
{
   const i = infoMutable (array);

   // ComponentInfoArray.add (name, { level, title, ... }) and ProfileInfoArray.add (name, { title, ... }) make the value.
   const value = i .make ? i .make (key, spec) : spec;

   if (i .aliases .has (key) || i .source .some (v => i .keyOf (v) === key))
      throw new Error (`Couldn't add value to ${i .typeName}, key '${key}' already exists.`);

   if (!i .check (value))
      throw new Error (`Couldn't add value to ${i .typeName}, value for key '${key}' has wrong type.`);

   i .source .push (value);
}

function infoUpdate (array, oldKey, newKey, value)
{
   const i = infoMutable (array);

   if (!i .check (value))
      throw new Error (`Couldn't update value of ${i .typeName}, value for key '${newKey}' has wrong type.`);

   const find = key => i .aliases .get (key) ?? i .source .find (v => i .keyOf (v) === key);
   const old = find (oldKey);

   if (oldKey !== newKey)
      infoRemove (array, newKey);

   i .aliases .delete (oldKey);
   i .aliases .set (newKey, value);

   const index = old === undefined ? -1 : i .source .indexOf (old);

   if (old !== undefined)
   {
      if (index > -1)
         i .source [index] = value;
   }
   else
   {
      i .source .push (value);
   }
}

function infoRemove (array, key)
{
   const i = infoMutable (array);
   const value = i .aliases .get (key) ?? i .source .find (v => i .keyOf (v) === key);

   if (value === undefined)
      return;

   const index = i .source .indexOf (value);

   i .aliases .delete (key);

   if (index > -1)
      i .source .splice (index, 1);
}

// `source` is a function (a live view) or the array of values; X3DInfoArray.add checks each value.
function infoArray (typeName, source, keyOf, check, make)
{
   const target = Object .create (infoProto);

   Object .defineProperties (target, { constructor: { value: infoConstructor (typeName) }, [INFO]: { value: { typeName, source: typeof source === "function" ? source : [... source], keyOf, check, make, aliases: undefined } }, length: { get () { return infoValues (this) .length; } } });

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
   get baseURL ()              { return X ("baseURL", currentScript); },
   get currentFrameRate ()     { return X ("framerate"); },
   get currentSpeed ()         { return 0; },
   getName ()                  { return X ("bname"); },
   getVersion ()               { return X ("bversion"); },
   getDescription ()           { return X ("bdescription"); },
   setDescription (v)          { X ("bsetdescription", String (v)); },
   getCurrentTime ()           { return X ("time"); },
   getCurrentSpeed ()          { return 0; },
   getCurrentFrameRate ()      { return X ("framerate"); },
   getWorldURL ()              { return X ("worldURL", currentScript); },
   getBaseURL ()               { return X ("baseURL", currentScript); },
   print (... args)            { println (... args); },
   println (... args)          { println (... args); },
   createVrmlFromString (s)    { return new Fields .MFNode (... X ("createVrml", String (s), currentScript) .map (R .bind (null)) .map (id => typeof id === "number" ? nodeObject (id) : id)); },
   createX3DFromString (s)     { return browserScene ("string", String (s)); },
   createX3DFromURL (url, node, event)
   {
      if (arguments .length === 3)
         return Browser .createVrmlFromURL (url, node, event);

      if (!(url instanceof Fields .MFString))
         throw new Error ("Browser.createX3DFromURL: url must be of type MFString.");

      return browserScene ("url", Array .from (url));
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
               if (!isProfileInfo (profile))
                  throw new Error ("Couldn't create scene: profile must be of type ProfileInfo.");

               for (const component of components)
               {
                  if (!isComponentInfo (component))
                     throw new Error ("Couldn't create scene: component must be of type ComponentInfo.");
               }
            }

            // scene .updateComponent (component): a component of the same name is replaced where it stands.
            const given = new Map ();

            for (const c of components)
               given .set (c .name, c);

            const r = X ("browserScene", currentScript, "create", args .length ? profile .name : undefined, [... given .values ()] .map (c => [c .name, c .level]));
            const scene = executionContext (- (r .ctx + 2));

            sceneComponents .set (- (r .ctx + 2), given);
            resolve (scene);
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
      node  = node instanceof SFNode && node .getValue () ? node : null;
      event = String (event);

      if (!(url instanceof Fields .MFString))
         throw new Error ("Browser.createVrmlFromURL: url must be of type MFString.");

      if (!node)
         throw new Error ("Browser.createVrmlFromURL: node must be of type X3DNode.");

      const field = node .getValue () .getField (event); // X_ITE reads the target node here: no deprecation warning

      if (!field .isInput ())
         throw new Error (`Browser.createVrmlFromURL: event named '${event}' must be a input field.`);

      if (field .getType () !== X3DConstants .MFNode)
         throw new Error (`Browser.createVrmlFromURL: event named '${event}' must be of type MFNode.`);

      // The event is sent once the scene is complete; a scene that fails to load sends nothing.
      browserScene ("url", Array .from (url)) .then (scene => field .setValue (scene .rootNodes), () => { });
   },
   // X3DBrowser.loadURL: the runtime fetches the world (browser_load.rs) and settles the promise (`__settlePlain`) once the
   // new world is installed. A wrong argument rejects the promise.
   loadURL (url, parameter = new Fields .MFString ())
   {
      return new Promise ((resolve, reject) =>
      {
         if (!(url instanceof Fields .MFString))
            throw new Error ("Browser.loadURL: url must be of type MFString.");

         if (!(parameter instanceof Fields .MFString))
            throw new Error ("Browser.loadURL: parameter must be of type MFString.");

         const r = X ("browserScene", currentScript, "load", Array .from (url), Array .from (parameter));

         if (r .error !== undefined)
            reject (new Error (r .error));
         else if (r .id === undefined)
            resolve ();
         else
            settlers .set (r .id, { resolve, reject, plain: true, script: currentScript });
      });
   },
   // X3DBrowser.replaceWorld: an X3DScene, an MFNode (VRML97), or anything else for an empty world.
   replaceWorld (scene)
   {
      return new Promise ((resolve, reject) =>
      {
         let kind = "none", argument;

         if (scene instanceof Fields .MFNode)
         {
            kind     = "nodes";
            argument = A (scene);
         }
         else if (contextKeys .has (scene) && scene .isScene)
         {
            kind     = "scene";
            argument = contextKeys .get (scene);
         }

         const r = X ("replaceWorld", currentScript, kind, argument);

         settlers .set (r .id, { resolve, reject, plain: true, script: currentScript });
      });
   },
   // X3DBrowserContext.initialized () and shutdown (): the browser's SFTime event fields (a copy of the value here).
   // `initialized` is set to the browser time by checkLoadCount; X_ITE 16.5.0 never sets `shutdown`.
   initialized ()              { return new Fields .SFTime (X ("browserTime", "initialized")); },
   shutdown ()                 { return new Fields .SFTime (- 1); },
   addBrowserCallback (... args)
   {
      switch (args .length)
      {
         case 2:
         {
            const [key, object] = args;

            for (const [event, map] of browserCallbacks)
               browserCallbacks .set (event, new Map (map) .set (key, { object, script: currentScript }));

            break;
         }
         case 3:
         {
            const [key, event, object] = args, map = new Map (browserCallbacks .get (event));

            browserCallbacks .set (event, map);
            map .set (key, { object, script: currentScript });
            break;
         }
      }
   },
   removeBrowserCallback (... args)
   {
      switch (args .length)
      {
         case 1:
         {
            for (const map of browserCallbacks .values ())
               map .delete (args [0]);

            break;
         }
         case 2:
         {
            browserCallbacks .get (args [1]) .delete (args [0]);
            break;
         }
      }
   },
   getBrowserCallbacks (event)
   {
      if (arguments .length === 1)
         return new Map ([... browserCallbacks .get (event)] .map (([k, c]) => [k, c .object]));

      return new Map ([... browserCallbacks] .flatMap (([, map]) => [... map] .map (([k, c]) => [k, c .object])));
   },
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
   changeViewpoint (name)      { X ("brequest", "changeViewpoint:" + String (name)); },
   viewAll ()                  { },
   getScriptNode ()            { return nodeObject (currentScript); },
   // X3DBrowser: the supported profiles and components, getProfile, getComponent and loadComponents.
   get supportedProfiles ()    { return getSupportedProfiles (); },
   get supportedComponents ()  { return getSupportedComponents (); },
   getComponent (name, level)  { return getComponent (name, level); },
   getProfile (name)
   {
      const profile = getSupportedProfiles () .get (String (name));

      if (profile)
         return profile;

      throw new Error (`Profile '${name}' is not supported.`);
   },
   addSupportedProfile (profile)       { getSupportedProfiles () .add (profile .name, profile); },
   updateSupportedProfile (profile)    { getSupportedProfiles () .update (profile .name, profile .name, profile); },
   removeSupportedProfile (name)       { return getSupportedProfiles () .remove (String (name)); },
   getSupportedProfile (name)
   {
      const profile = getSupportedProfiles () .get (String (name));

      if (profile)
         return profile;

      throw new Error (`Supported profile '${name}' does not exists.`);
   },
   getSupportedProfiles ()             { return getSupportedProfiles (); },
   addSupportedComponent (component)   { getSupportedComponents () .add (component .name, component); },
   updateSupportedComponent (component) { getSupportedComponents () .update (component .name, component .name, component); },
   removeSupportedComponent (name)     { getSupportedComponents () .remove (String (name)); },
   getSupportedComponent (name)
   {
      const component = getSupportedComponents () .get (String (name));

      if (component)
         return component;

      throw new Error (`Supported component '${name}' does not exists.`);
   },
   getSupportedComponents ()           { return getSupportedComponents (); },
   // X3DBrowser.loadComponents: the components are all built in here, so the promise only checks the arguments.
   loadComponents (... args)
   {
      const component = [ ];

      for (const arg of args)
      {
         if (arg && typeof arg .hasComponent === "function" && arg .profile !== undefined)
            component .push (... (arg .profile ?? getSupportedProfiles () .get ("Full")) .components, ... arg .components);

         else if (isProfileInfo (arg))
            component .push (... arg .components);

         else if (arg instanceof Object && arg [INFO] ?.typeName === "ComponentInfoArray")
            component .push (... arg);

         else if (isComponentInfo (arg))
            component .push (arg);

         else if (typeof arg === "string")
            component .push (getComponent (arg));
      }

      return Promise .all (component .map (() => undefined));
   },
};

// X3DBrowser.createX3DFromString / createX3DFromURL: the scene is made by the runtime (browser_load.rs), which
// settles the promise once the scene is complete (`__settle`).
const settlers = new Map ();

// X3DBrowser [_browserCallbacks]: the functions of addBrowserCallback by event. A Map is replaced, not edited, when a
// callback is added, so callbacks added while the others run wait for the next event.
const browserCallbacks = new Map ([1001, 1002, 1003, 1004, 1005] .map (event => [event, new Map ()]));

global .__browserEvent = function (event)
{
   const previous = currentScript;

   try
   {
      for (const { object, script } of browserCallbacks .get (event) .values ())
      {
         currentScript = script;

         try { object (event); }
         catch (error) { scriptConsole .error (error); }
      }
   }
   finally
   {
      currentScript = previous;
   }
};

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

// A promise continuation runs as the Script that made the promise (X_ITE: each Script has its own Browser object).
global .__endSettle = function () { currentScript = -1; };

global .__settle = function (id, ok, value)
{
   const settler = settlers .get (id);

   settlers .delete (id);

   if (!settler)
      return;

   if (settler .script !== undefined)
      currentScript = settler .script;

   if (settler .plain)
      return ok ? settler .resolve () : settler .reject (new Error (value));

   if (ok)
      settler .resolve (executionContext (- (value + 2)));
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

// Configuration/{ComponentInfo,ProfileInfo,ComponentInfoArray,ProfileInfoArray,SupportedComponents,SupportedProfiles,UnitInfo}.js.
const PROVIDER_URL = "https://create3000.github.io/x_ite/";

// Browser/Networking/URLs.js getProviderURL (component): the built-in components share the project's URL, the external ones are
// `assets/components/<name>Component.js` next to the script X_ITE runs from (the page; none: the project's own script).
function getProviderURL (component)
{
   if (!component)
      return PROVIDER_URL;

   const script = configTable () .scriptURL || PROVIDER_URL + "x_ite.min.js";
   const min    = /\.min\.m?js$/ .test (script) ? ".min" : "";
   const path   = script .replace (/[?#].*$/, "");

   return path .substring (0, path .lastIndexOf ("/") + 1) + `assets/components/${component}Component${min}.js`;
}

// X3DObject: toString and the Symbol .toStringTag of ComponentInfo and ProfileInfo.
function infoClass (typeName, methods)
{
   const f = function () { };

   Object .defineProperty (f, "name", { value: typeName });
   Object .defineProperty (f, "typeName", { value: typeName, enumerable: true });
   Object .defineProperties (f .prototype, {
      [Symbol .toStringTag]: { get () { return typeName; } },
      getTypeName: { value () { return typeName; } },
      toString: { value () { return `[object ${typeName}]`; } },
      ... Object .fromEntries (Object .entries (methods) .map (([k, v]) => [k, { value: v }])),
   });

   return f;
}

const ComponentInfoBase = infoClass ("ComponentInfo",
{
   toVRMLString () { return `COMPONENT ${this .name} : ${this .level}`; },
   toXMLString () { return `<component name='${this .name}' level='${this .level}'/>`; },
   toJSONString () { return `\n{\n  "@name": ${JSON .stringify (this .name)},\n  "@level": ${this .level}\n}`; },
});

const ProfileInfoBase = infoClass ("ProfileInfo",
{
   toVRMLString () { return `PROFILE ${this .name}`; },
   toXMLString () { return String (this .name); },
   toJSONString () { return String (this .name); },
});

function ComponentInfo (name, level, title, providerURL, external = false, dependencies = [ ])
{
   const o = Object .create (ComponentInfoBase .prototype);

   Object .defineProperties (o, {
      name: { value: name, enumerable: true },
      level: { value: level, enumerable: true },
      title: { value: title, enumerable: true },
      providerURL: { value: providerURL || getProviderURL (external && name), enumerable: true },
      external: { value: external },
      dependencies: { value: dependencies },
      providerUrl: { get () { return this .providerURL; } },
   });

   return o;
}

function ProfileInfo (name, title, providerURL, components)
{
   const o = Object .create (ProfileInfoBase .prototype);

   Object .defineProperties (o, {
      name: { value: name, enumerable: true },
      title: { value: title, enumerable: true },
      providerURL: { value: providerURL || getProviderURL (), enumerable: true },
      components: { value: components, enumerable: true },
      providerUrl: { get () { return this .providerURL; } },
   });

   return o;
}

const isComponentInfo = c => c instanceof ComponentInfoBase;
const isProfileInfo   = p => p instanceof ProfileInfoBase;

const makeComponent = (name, { level, title, providerURL, external = false, dependencies = [ ] }) => ComponentInfo (name, level, title, providerURL, external, dependencies);
const makeProfile   = (name, { title, providerURL, components }) => ProfileInfo (name, title, providerURL, componentArray (components));

function componentArray (list) { return infoArray ("ComponentInfoArray", Array .from (list, c => c), c => c .name, isComponentInfo, makeComponent); }
function profileArray (list)   { return infoArray ("ProfileInfoArray", Array .from (list, p => p), p => p .name, isProfileInfo, makeProfile); }

// SupportedComponents.js / SupportedProfiles.js, built on first use from the table of the pinned X_ITE (title, level, external,
// dependencies; X_ITE's aliases).
let config = null;
let supportedComponents = null;
let supportedProfiles = null;

function configTable ()
{
   if (config)
      return config;

   config = X ("configTable");

   const components = componentArray ([ ]);

   for (const [name, title, level, external, dependencies] of config .components)
      components .add (name, { title, level, external, dependencies });

   components .alias ("H-Anim", components .get ("HAnim"));
   components .alias ("TextureProjector", components .get ("TextureProjection"));
   components .alias ("ProjectiveTextureMapping", components .get ("TextureProjection"));

   const profiles = profileArray ([ ]);

   supportedComponents = components;
   supportedProfiles   = profiles;

   // SupportedProfiles.js getComponent (name, level): the table's component at that level.
   const component = (name, level) =>
   {
      const c = components .get (name);
      return ComponentInfo (c .name, level, c .title, c .providerURL, c .external, c .dependencies);
   };

   for (const [name, title, list] of config .profiles)
      profiles .add (name, { title, components: list .map (([n, l]) => component (n, l)) });

   profiles .alias ("MPEG-4", profiles .get ("MPEG4Interactive"));

   return config;
}

function getSupportedComponents () { configTable (); return supportedComponents; }
function getSupportedProfiles ()   { configTable (); return supportedProfiles; }

// X3DBrowser.getComponent (name, level)
function getComponent (name, level)
{
   name   = String (name);
   level |= 0;

   const component = getSupportedComponents () .get (name);

   if (component)
   {
      return ComponentInfo (component .name,
         Math .min (Math .max (level || component .level, 1), component .level),
         component .title,
         component .providerURL,
         component .external,
         component .dependencies);
   }

   throw new Error (`Component '${name}' at level '${level}' is not supported.`);
}

// The ComponentInfo objects a script gave a scene (identity), by scene and name.
const sceneComponents = new Map ();
const profileValues = new Map (); // what setProfile () was given when it is not a ProfileInfo

function componentObject (key, name, level)
{
   const given = sceneComponents .get (key) ?.get (name);

   if (given && given .level === level)
      return given;

   let list = sceneComponents .get (key);

   if (!list)
      sceneComponents .set (key, list = new Map ());

   const c = getSupportedComponents () .get (name);
   const o = c ? ComponentInfo (name, level, c .title, c .providerURL, c .external, c .dependencies) : ComponentInfo (name, level, undefined, undefined, false, [ ]);

   list .set (name, o);
   return o;
}

function sceneComponentGiven (key, component)
{
   let list = sceneComponents .get (key);

   if (!list)
      sceneComponents .set (key, list = new Map ());

   list .set (component .name, component);
}

function profileInfo (name)
{
   if (name === null)
      return null;

   return getSupportedProfiles () .get (name) ?? null;
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
const contextKeys = new WeakMap ();

function executionContext (script)
{
   const key = X ("ctxOf", script);

   let object = contextObjects .get (key);

   if (!object)
   {
      contextObjects .set (key, object = makeExecutionContext (- (key + 2)));
      contextKeys .set (object, key);
   }

   return object;
}

// X3DExecutionContext / X3DScene change fields (sceneGraph_changed, bbox_changed, profile_changed, metadata_changed): SFTime output
// fields. The runtime fires them (ctx_events.rs); a Script sees them through addFieldCallback only.
const ctxFieldCallbacks = new Map (); // context * 4 + field → Map (key → { fn, script })
const ctxFieldObjects = new Map ();

function ctxFieldObject (ctx, index, name)
{
   const id = ctx * 4 + index;

   let o = ctxFieldObjects .get (id);

   if (!o)
   {
      o = {
         getName ()           { return name; },
         getTypeName ()       { return "SFTime"; },
         getType ()           { return X3DConstants .SFTime; },
         getAccessType ()     { return X3DConstants .outputOnly; },
         isInput ()           { return false; },
         isOutput ()          { return true; },
         isReadable ()        { return true; },
         isWritable ()        { return false; },
         getValue ()          { return X ("time"); },
         valueOf ()           { return X ("time"); },
         toString ()          { return String (X ("time")); },
         // X3DField.addFieldCallback: the Map is replaced, so callbacks added while the others run wait for the next event.
         addFieldCallback (key, fn)
         {
            X ("ctxObserve", ctx);
            ctxFieldCallbacks .set (id, new Map (ctxFieldCallbacks .get (id)) .set (key, { fn, script: currentScript }));
         },
         removeFieldCallback (key)
         {
            const map = new Map (ctxFieldCallbacks .get (id));

            map .delete (key);
            ctxFieldCallbacks .set (id, map);
         },
         getFieldCallbacks () { return new Map ([... (ctxFieldCallbacks .get (id) ?? [ ])] .map (([k, c]) => [k, c .fn])); },
      };

      ctxFieldObjects .set (id, o);
   }

   return o;
}

global .__ctxEvent = function (ctx, index)
{
   const previous = currentScript;

   try
   {
      for (const { fn, script } of ctxFieldCallbacks .get (ctx * 4 + index) ?.values () ?? [ ])
      {
         currentScript = script;

         try { fn (X ("time")); }
         catch (error) { scriptConsole .error (error); }
      }
   }
   finally
   {
      currentScript = previous;
   }
};

// The classes a Script sees as globals (Script.js createGlobalObject): their instances are the objects made here.
function brandClass (typeName, length, test)
{
   const C = { [typeName]: function () { } } [typeName];

   Object .defineProperty (C, "length", { value: length });
   Object .defineProperty (C, "typeName", { value: typeName, enumerable: true });
   Object .defineProperty (C, Symbol .hasInstance, { value: test });
   return C;
}

const X3DExecutionContextClass = brandClass ("X3DExecutionContext", 1, o => contextKeys .has (o));
const X3DSceneClass            = brandClass ("X3DScene", 1, o => contextKeys .has (o) && o .isScene);
const X3DBrowserClass          = brandClass ("X3DBrowser", 1, o => o === Browser);
const X3DRouteClass            = brandClass ("X3DRoute", 5, o => routeIds .has (o));

function makeExecutionContext (script)
{
   const ctxKey = - (script + 2);
   const info = () => X ("sceneInfo", script);
   let namedTouched = false;
   const node = n => R (n);
   const metaValues = v => Array .isArray (v) ? v .map (String) : [String (v)];

   // One live array per context.
   const named    = infoArray ("NamedNodesArray", () => X ("nodeArrays", script, "named") .map (R), n => n .getNodeName (), n => n instanceof SFNode);
   const imported = infoArray ("ImportedNodesArray", () => X ("nodeArrays", script, "imported") .map (m => importedObject (script, m .id)), n => n .getImportedName (), n => importedKeys .has (n));
   const exported = infoArray ("ExportedNodesArray", () => X ("nodeArrays", script, "exported") .map (m => exportedObject (script, m .id)), n => n .getExportedName (), n => exportedKeys .has (n));
   const protos   = infoArray ("ProtoDeclarationArray", () => X ("cx", script, "protoIds", false) .map (id => protoObject (false, id)), p => p .name, p => protoKeys .get (p)?.[0] === false);
   const externs  = infoArray ("ExternProtoDeclarationArray", () => X ("cx", script, "protoIds", true) .map (id => protoObject (true, id)), p => p .name, p => protoKeys .get (p)?.[0] === true);
   const routes   = infoArray ("RouteArray", () => X ("cx", script, "routeList") .map (r => routeObject (script, r .id)), r => r .getId (), r => routeIds .has (r));
   const comps    = infoArray ("ComponentInfoArray", () => info () .components .map (([name, level]) => componentObject (script, name, level)), c => c .name, isComponentInfo);
   const units    = infoArray ("UnitInfoArray", () => info () .units .map (([category, name, conversionFactor]) => { const o = Object .freeze ({ category, name, conversionFactor }); infoObjects .add (o); return o; }), u => u .category, u => infoObjects .has (u));

   return {
      get constructor ()      { return info () .isScene ? X3DSceneClass : X3DExecutionContextClass; },
      get isScene ()          { return info () .isScene; },
      get sceneGraph_changed () { return ctxFieldObject (ctxKey, 0, "sceneGraph_changed"); },
      get bbox_changed ()     { return ctxFieldObject (ctxKey, 1, "bbox_changed"); },
      get profile_changed ()  { return info () .isScene ? ctxFieldObject (ctxKey, 2, "profile_changed") : undefined; },
      get metadata_changed () { return info () .isScene ? ctxFieldObject (ctxKey, 3, "metadata_changed") : undefined; },
      get _sceneGraph_changed () { return ctxFieldObject (ctxKey, 0, "sceneGraph_changed"); },
      get _bbox_changed ()    { return ctxFieldObject (ctxKey, 1, "bbox_changed"); },
      get _profile_changed () { return info () .isScene ? ctxFieldObject (ctxKey, 2, "profile_changed") : undefined; },
      get _metadata_changed () { return info () .isScene ? ctxFieldObject (ctxKey, 3, "metadata_changed") : undefined; },
      // A plain property: the real clock, assigned by updateNamedNode and removeNamedNode (and by every DEF); never an event.
      get _namedNodes_changed () { return namedTouched || named .length ? X ("time") : undefined; },
      get encoding ()         { return info () .encoding; },
      get profile ()          { const given = profileValues .get (script); return given ? given .value : profileInfo (info () .profile); },
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
      // X3DBaseNode.getField: the change fields are private, they have no names a Script can look up.
      getField (name)         { throw new Error (`Unknown field '${name}' in node class ${info () .isScene ? "X3DScene" : "X3DExecutionContext"}.`); },
      getFieldDefinitions ()  { return infoArray ("FieldDefinitionArray", [ ], d => d .name, d => false); },
      getEncoding ()          { return this .encoding; },
      getProfile ()           { return this .profile; },
      getComponents ()        { return comps; },
      updateUnit (category, name, conversionFactor)
      {
         // Private function: nothing happens for a category the scene does not have.
         X ("updateUnit", script, String (category), String (name), + conversionFactor);
      },
      setProfile (profile)
      {
         // The value is kept as it is (a Script may pass anything); a ProfileInfo is also named for the scene (PROFILE of the printed scene).
         if (isProfileInfo (profile) || profile === null)
            profileValues .delete (script);
         else
            profileValues .set (script, { value: profile });

         X ("setProfile", script, isProfileInfo (profile) ? profile .name : null);
      },
      hasComponent (name, level = 0)
      {
         if (isComponentInfo (name))
            ({ name, level } = name);

         const profile = this .profile ?? getSupportedProfiles () .get ("Full");

         return [profile .components .get (name), comps .get (name)] .some (component => component && level <= component .level);
      },
      addComponent (component)
      {
         if (comps .get (component .name))
            throw new Error (`Couldn't add component. Component '${component .name}' already exists.`);

         // ComponentInfoArray.add (name, { level, title, ... }) makes the stored ComponentInfo.
         const stored = makeComponent (component .name, component);

         sceneComponentGiven (script, stored);
         X ("addComponent", script, String (stored .name), stored .level | 0);
      },
      updateComponent (component)
      {
         if (!isComponentInfo (component))
            throw new Error (`Couldn't update value of ComponentInfoArray, value for key '${component .name}' has wrong type.`);

         sceneComponentGiven (script, component);
         X ("updateComponent", script, String (component .name), component .level);
      },
      removeComponent (name)  { X ("removeComponent", script, String (name)); sceneComponents .get (script) ?.delete (String (name)); },
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
      addNamedNode (name, n)  { X ("cx", script, "named", "add", String (name), A (n)); namedTouched = true; },
      updateNamedNode (name, n) { X ("cx", script, "named", "update", String (name), A (n)); namedTouched = true; },
      removeNamedNode (name)  { X ("cx", script, "named", "remove", String (name)); namedTouched = true; },
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
      set rootNodes (value)   { this .setRootNodes (value); },
      getWorldInfos ()        { return R (X ("worldInfos", script)); },
      get worldURL ()         { return X ("worldURL", script); },
      get baseURL ()          { return X ("worldURL", script); },
      getBaseURL ()           { return X ("worldURL", script); },
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
   if (!(n instanceof SFNode))
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
for (const name of ["sin", "cos", "tan", "exp", "log", "acos", "asin", "atan2", "log10", "sqrt", "atan", "sinh", "cosh", "tanh", "expm1", "log1p", "log2", "cbrt", "pow", "hypot"])
{
   const f = X ("mathfn", name);

   if (f)
   {
      // The built-in's name and length (Math.hypot.length is 2).
      Object .defineProperty (f, "name", { value: name, configurable: true });
      Object .defineProperty (f, "length", { value: Math [name] .length, configurable: true });
      Object .defineProperty (Math, name, { value: f, writable: true, configurable: true, enumerable: false });
   }
}

// The page's console (Script globals fall through to `window`): log, warn and error reach the browser's log.
const consoleText = args => args .map (a => typeof a === "string" ? a : String (a)) .join (" ");
const scriptConsole =
{
   log: (... args) => { X ("console", "log", consoleText (args)); },
   warn: (... args) => { X ("console", "warn", consoleText (args)); },
   error: (... args) => { X ("console", "error", consoleText (args)); },
   // The page's other console methods are not hooked by the oracle page and never reach its recorder.
   info: () => { }, debug: () => { }, trace: () => { }, assert: () => { }, dir: () => { }, dirxml: () => { }, table: () => { },
   group: () => { }, groupCollapsed: () => { }, groupEnd: () => { }, count: () => { }, countReset: () => { }, time: () => { },
   timeEnd: () => { }, timeLog: () => { }, clear: () => { },
};

// Timers: the oracle page keeps one virtual queue, fired in due order at the start of a frame (`runTimers`) while the browser time and
// Date.now () stand at the due time. setInterval is rearmed after each run.
let timerSeq = 0, timerList = [];

function addTimer (fn, ms, args, repeat)
{
   const id = ++ timerSeq, delay = Math .max (+ms || 0, 0);

   timerList .push ({ id, due: X ("datenow") + delay, delay, seq: id, fn: () => fn (... args), repeat, script: currentScript });
   return id;
}

const timerGlobals =
{
   setTimeout (fn, ms = 0, ... args)  { return addTimer (fn, ms, args, false); },
   setInterval (fn, ms = 0, ... args) { return addTimer (fn, ms, args, true); },
   clearTimeout (id)                  { timerList = timerList .filter (t => t .id !== id); },
   clearInterval (id)                 { timerList = timerList .filter (t => t .id !== id); },
};

Date .now = () => X ("datenow");

global .__nextTimer = function (untilMs)
{
   timerList .sort ((a, b) => a .due - b .due || a .seq - b .seq);

   const t = timerList [0];

   return !t || t .due > untilMs ? undefined : t .due;
};

global .__fireTimer = function ()
{
   const t = timerList .shift ();

   if (t .repeat)
   {
      t .due += Math .max (t .delay, 1);
      timerList .push (t);
   }

   const previous = currentScript;

   currentScript = t .script;

   try
   {
      t .fn ();
   }
   catch (error)
   {
      X ("console", "error", v8String (error, sources .get (t .script)));
   }
   finally
   {
      currentScript = previous;
   }
};

// Script.js createGlobalObject: `function SFNode (vrmlSyntax)`, whose prototype is Fields.SFNode's.
const ScriptSFNode = (() => function SFNode (vrmlSyntax)
{
   // Fields .SFNode () .fromString (vrmlSyntax, getScriptNode () .getExecutionContext ()).
   return nodeObject (X ("sfnodeFromString", String (vrmlSyntax), currentScript));
}) ();

ScriptSFNode .prototype = SFNode .prototype;

const standardGlobals =
{
   ... timerGlobals,
   NULL: null, FALSE: false, TRUE: true,
   console: scriptConsole,
   print: println, trace: println,
   Browser,
   X3DConstants,
   X3DField, X3DArrayField,
   X3DBrowser: X3DBrowserClass, X3DExecutionContext: X3DExecutionContextClass, X3DScene: X3DSceneClass, X3DRoute: X3DRouteClass,
   // Script.js lists neither SFBool, SFInt32, SFFloat, SFDouble nor SFTime as globals; its SFNode is its own function.
   ... Object .fromEntries (Object .entries (Fields) .filter (([k]) => !["SFNode", "SFBool", "SFInt32", "SFFloat", "SFDouble", "SFTime"] .includes (k))),
   SFNode: ScriptSFNode,
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

   const word = /[\w$]/;

   // The text V8 prints for the expression that ends at index `e`, and the index before its first character.
   const expr = e =>
   {
      while (e >= 0 && /\s/ .test (line [e]))
         -- e;

      if (line [e] === ")")
      {
         let depth = 0, k = e;

         for (; k >= 0; -- k)
         {
            if (line [k] === ")")
               ++ depth;
            else if (line [k] === "(" && -- depth === 0)
               break;
         }

         if (k < 0)
            return undefined;

         let before = k - 1;

         while (before >= 0 && /\s/ .test (line [before]))
            -- before;

         if (before < 0 || !word .test (line [before]))
            return ["(intermediate value)", before];

         const callee = expr (before);

         if (!callee)
            return undefined;

         // `new X (…)`: no name.
         const prefix = line .slice (0, callee [1] + 1) .replace (/\s+$/, "");

         if (/\bnew$/ .test (prefix))
            return ["(intermediate value)", prefix .length - 4];

         return [`${callee [0]}(...)`, callee [1]];
      }

      if (e < 0 || !word .test (line [e]))
         return undefined;

      const end = e;

      while (e >= 0 && word .test (line [e]))
         -- e;

      const name = line .slice (e + 1, end + 1);
      let d = e;

      while (d >= 0 && /\s/ .test (line [d]))
         -- d;

      if (line [d] === ".")
      {
         const receiver = expr (d - 1);

         return receiver ? [`${receiver [0]}.${name}`, receiver [1]] : undefined;
      }

      return [name, e];
   };

   return expr (i) ?.[0];
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
   else if (message === "invalid array length")
      message = "Invalid array length";
   else if (message === "expecting hex digit" || message === "expecting %hh")
      message = "URI malformed";
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

   // The components the Script's scene loaded before its Scripts run (X3DBrowser.loadComponents (scene)): the profile's, then
   // the COMPONENT statements' (a scene without a profile loads Full).
   {
      const info = X ("sceneInfo", script);
      const profile = getSupportedProfiles () .get (info .profile ?? "Full");

      for (const component of profile ?.components ?? [ ])
         loadComponentConstants (component .name);

      for (const [name] of info .components)
         loadComponentConstants (name);
   }

   const descriptors = { };

   for (const [k, v] of Object .entries (standardGlobals))
      descriptors [k] = { value: v };

   // Script.js createContext: the callback of an inputOnly field is its name, of an inputOutput field `set_` + its name.
   const callbackFields = new Map ();

   scriptFields .set (script, callbackFields);

   for (const [name, accessType, h, type] of fields)
   {
      const field = attachedField (h, type);

      if (accessType === 2 || accessType === 7)
         callbackFields .set (accessType === 2 ? name : "set_" + name, field);

      if (accessType === 2)
         continue;

      // Script.js: `get: field .valueOf .bind (field), set: field .setValue .bind (field)`.
      const names = [name];

      if (accessType === 7)
         names .push (name + "_changed");

      for (const n of names)
      {
         if (n in descriptors)
            continue;

         descriptors [n] = {
            get: field .valueOf .bind (field),
            set: field .setValue .bind (field),
         };
      }
   }

   const globalObject = Object .create (Object .prototype, descriptors);

   globalObjects .set (script, globalObject);

   sourceText += "\n;\n[" + callbacks .map (c => `typeof ${c} !== "undefined" ? ${c} : undefined`) .join (",") + "];";

   const previous = currentScript;

   currentScript = script;

   try
   {
      const result = evaluate .call (nodeObject (script), globalObject, sourceText);
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

const copies = new Map (); // "script:callback" → the cached copies of set_field__
const scriptFields = new Map (); // script → (callback name → the Script's field)
const globalObjects = new Map (); // script → its global object

__bridge .withBase = () => globalObjects .get (currentScript) ?? global;

// Script.call__ / set_field__: returns the thrown value as a string, or undefined.
global .__call = function (script, name, value, time, hasValue, reason)
{
   const callback = contexts .get (script) ?.get (name);

   if (typeof callback !== "function")
      return undefined;

   const previous = currentScript;

   currentScript = script;

   try
   {
      let result;

      if (hasValue)
      {
         // Script.set_field__: one copy of the field per callback, reused (`cache .pop () ?? field .create ()`).
         const field = scriptFields .get (script) ?.get (name) ?? R (value), key = script + ":" + name;
         const cache = copies .get (key) ?? copies .set (key, [ ]) .get (key);
         const copy = cache .pop () ?? field .create ();

         try
         {
            copy .assign (field);
            result = callback .call (nodeObject (script), copy .valueOf (), time);
         }
         finally
         {
            cache .push (copy);
         }
      }
      else
      {
         result = callback .call (nodeObject (script), time);
      }

      // call__ is async: `await callback .call (…)` throws a rejection into the same catch.
      if (result !== null && (typeof result === "object" || typeof result === "function") && typeof result .then === "function")
      {
         const source = sources .get (script);

         result .then (undefined, error => X ("asyncError", script, String (reason), v8String (error, source)));
      }

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
   for (const id of ids)
   {
      const ref = nodeRefs .get (id) ?.deref ();

      // A Script's objects of the node from now on stand for the empty tombstone node; its field objects keep their value and
      // no longer reach a field (the id may be handed out again).
      if (ref)
      {
         for (const entry of ref [FIELDS] .values ())
         {
            attachedOf .delete (entry .field);

            if (entry .field instanceof X3DArrayField)
               attachedOf .delete (entry .field .getTarget ());
         }

         ref [NODE]        = tombstone;
         ref [FIELDS]      = new Map ();
         ref [DEFINITIONS] = undefined;
         ref [USER_DATA]   = undefined;
      }

      nodeRefs .delete (id);
      pinned .delete (id);
      contexts .delete (id);
      sources .delete (id);
      scriptFields .delete (id);
      globalObjects .delete (id);
   }
};

// release.rs asks which of `ids` (nodes only JavaScript could hold) a Script still reaches: their objects are unpinned, the host
// collects garbage, and the objects that are left are what the Script holds (X_ITE: the garbage collector frees the others).
global .__unpin = function (ids)
{
   for (const id of ids)
      pinned .delete (id);
};

global .__heldNodes = function (ids)
{
   const held = [ ];

   for (const id of ids)
   {
      const ref = nodeRefs .get (id) ?.deref ();

      if (ref === undefined)
         continue;

      pinned .set (id, ref);
      held .push (id);
   }

   return held;
};

// engine.rs: before and after every call into the Script realm (bridge.js).
global .__enter = enter;
global .__leave = leave;

// The Script was shut down: its global object and callbacks are not needed any more.
global .__destroy = function (script)
{
   contexts .delete (script);
   sources .delete (script);
};
})(globalThis);
