// Base/X3DInfoArray.js: the arrays a Script gets (an execution context's namedNodes, importedNodes, exportedNodes, protos,
// externprotos, routes, components, units; the browser's supported profiles and components; field definitions) are instances of
// X_ITE's own classes (config.js, fields.js). A context's own array is a live view of the runtime: before each use it is filled
// again with the context's current values when they changed. The copies `filter`, `map` and `slice` make are X_ITE's.
const INFO_ARRAY_TYPES = () => ({ ComponentInfoArray, ProfileInfoArray, UnitInfoArray, ConcreteNodesArray, AbstractNodesArray, NamedNodesArray,
   ImportedNodesArray, ExportedNodesArray, ProtoDeclarationArray, ExternProtoDeclarationArray, RouteArray, FieldDefinitionArray });

// The live view's own changes do not reach the runtime: they throw (an execution context's arrays change through its methods).
const INFO_ARRAY_CHANGES = new Set (["add", "alias", "update", "remove", "clear", "assign"]);

function infoArray (typeName, source, keyOf)
{
   const array = new (INFO_ARRAY_TYPES () [typeName]) ();

   const fill = values =>
   {
      X3DInfoArray .prototype .clear .call (array);

      for (const value of values)
         X3DInfoArray .prototype .add .call (array, keyOf (value), value);
   };

   if (typeof source !== "function")
   {
      fill (source);
      return array;
   }

   let last = null;

   const sync = () =>
   {
      const values = source ();

      if (last && values .length === last .length && values .every ((v, i) => v === last [i]))
         return;

      last = values;
      fill (values);
   };

   // The view's own target is empty (the proxy invariants then hold for whatever X_ITE's array reports); every trap is the array's.
   const view = new Proxy (Object .create (Object .getPrototypeOf (array)),
   {
      get (target, key)
      {
         if (INFO_ARRAY_CHANGES .has (key))
            return function () { throw new Error (`${typeName} is a view of its execution context.`); };

         sync ();
         return Reflect .get (array, key);
      },
      set (target, key, value) { sync (); return Reflect .set (array, key, value); },
      has (target, key) { sync (); return Reflect .has (array, key); },
      ownKeys (target) { sync (); return Reflect .ownKeys (array); },
      getOwnPropertyDescriptor (target, key)
      {
         sync ();

         const descriptor = Reflect .getOwnPropertyDescriptor (array, key);

         if (descriptor)
            descriptor .configurable = true;

         return descriptor;
      },
      getPrototypeOf (target) { return Object .getPrototypeOf (array); },
   });

   return view;
}

// What config.js names from X_ITE's engine (gen_prelude_xite.py ENGINE): URLs.getProviderURL, HTMLSupport (the page's custom
// elements: nothing to register), and the engine classes below.
const URLs = { getProviderURL: component => getProviderURL (component) };
const HTMLSupport = { addConcreteNode () { }, addAbstractNode () { } };

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
   // X3DBrowser.viewAll (layerNode, transitionTime = 1): one number argument is the time; the layer is X3DCast or the active
   // layer. The viewer runs X3DLayerNode.viewAll with the StraightenHorizon option.
   viewAll (layerNode, transitionTime = 1)
   {
      if (arguments .length === 1 && typeof layerNode === "number")
         transitionTime = layerNode;

      const ref = layerNode instanceof SFNode ? layerNode .getValue () : layerNode;
      const layer = ref instanceof X3DNodeRef && ref .getType () .includes (X3DConstants .X3DLayerNode) ? ref .getNodeId () : -1;

      X ("brequest", `viewAll:${layer},${+transitionTime}`);
   },
   // X3DBrowserContext.getScriptNode: X_ITE's Script node itself (here its X3DNodeRef), not an SFNode.
   getScriptNode ()            { return currentScript >= 0 ? nodeRef (currentScript) : null; },
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

         else if (arg instanceof ComponentInfoArray)
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
         settlers .set (r .id, { resolve, reject, script: currentScript });
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

const isComponentInfo = c => c instanceof ComponentInfo;
const isProfileInfo   = p => p instanceof ProfileInfo;

const makeComponent = (name, { level, title, providerURL, external = false, dependencies = [ ] }) => new ComponentInfo (name, level, title, providerURL, external, dependencies);

function componentArray (list) { return new ComponentInfoArray (list); }
function profileArray (list)   { return new ProfileInfoArray (list); }

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
      return new ComponentInfo (c .name, level, c .title, c .providerURL, c .external, c .dependencies);
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
      return new ComponentInfo (component .name,
         Math .min (Math .max (level || component .level, 1), component .level),
         component .title,
         component .providerURL,
         component .external,
         component .dependencies);
   }

   throw new Error (`Component '${name}' at level '${level}' is not supported.`);
}

// X3DScene.units: one UnitInfo per category until updateUnit replaces it.
const unitObjects = new Map ();

function unitObject (key, category, name, conversionFactor)
{
   let list = unitObjects .get (key);

   if (!list)
      unitObjects .set (key, list = new Map ());

   const u = list .get (category);

   if (u && u .name === name && Object .is (u .conversionFactor, conversionFactor))
      return u;

   const o = new UnitInfo (category, name, conversionFactor);

   list .set (category, o);
   return o;
}

// The ComponentInfo objects a script gave a scene (identity), by scene and name.
const sceneComponents = new Map ();
const profileValues = new Map (); // what setProfile () was given when it is not a ProfileInfo

// The ComponentInfo of a component of a scene: the one a Script gave (kept as it is, X3DScene.addComponent / updateComponent),
// else one made from the scene's name and level. `text` is the runtime's `String (level)` of it.
function componentObject (key, name, level, text = String (level))
{
   const given = sceneComponents .get (key) ?.get (name);

   if (given && String (given .level) === text)
      return given;

   let list = sceneComponents .get (key);

   if (!list)
      sceneComponents .set (key, list = new Map ());

   const c = getSupportedComponents () .get (name);
   const o = c ? new ComponentInfo (name, level, c .title, c .providerURL, c .external, c .dependencies) : new ComponentInfo (name, level, undefined, undefined, false, [ ]);

   list .set (name, o);
   return o;
}

function sceneComponentGiven (key, component)
{
   let list = sceneComponents .get (key);

   if (!list)
      sceneComponents .set (key, list = new Map ());

   list .set (String (component .name), component);
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

// One object per execution context (`Browser .currentScene === Browser .currentScene`). `contextObjects` pins each object;
// `contextRefs` finds it again while release.rs asks whether a Script still holds a scene (__unpinScenes, __heldScenes).
const contextObjects = new Map ();
const contextRefs = new Map ();
const contextKeys = new WeakMap ();

function executionContext (script)
{
   const key = X ("ctxOf", script);

   let object = contextObjects .get (key);

   if (!object)
   {
      contextObjects .set (key, object = makeExecutionContext (- (key + 2)));
      contextRefs .set (key, new WeakRef (object));
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
   const C = { [typeName]: function () { throw new TypeError ("Cannot read properties of undefined (reading 'getBrowser')"); } } [typeName];

   Object .defineProperty (C, "length", { value: length });
   Object .defineProperty (C, "typeName", { value: typeName, enumerable: true });
   Object .defineProperty (C, Symbol .hasInstance, { value: test });
   return C;
}

const X3DBrowserClass          = brandClass ("X3DBrowser", 1, o => o === Browser);

// --- X3DBrowser ----------------------------------------------------------------------------------------------------------------
//
// X_ITE's Browser is the X3DBrowser itself: X3DBrowser → X3DBrowserContext → X3DBaseNode → X3DChildObject → X3DObject (the last two
// are X_ITE's own, fields.js). Each member of Browser above is put on the class that defines it in X_ITE (oracle brx-api, the
// own property names of each level); the members that only exist for X_ITE's page (WebGL, DOM, the dashboard) are SAI-edge
// stand-ins (owner decision 2026-10-20): getCanvas, getElement, getNotification, getContextMenu, getViewer, getBrowserTime and
// getBrowserTimings return null, getLocalStorage throws a TypeError (as it does in X_ITE's Script realm), resize and dispose
// go to the viewer. A node X_ITE hands out (getActiveViewpoint …, getScriptNode) is its X3DNodeRef.

const BROWSER_OWN = new Set (("abstractNodes activeLayer activeNavigationInfo activeViewpoint addAbstractNode addBrowserCallback addBrowserListener " +
   "addConcreteNode addRoute addSupportedComponent addSupportedProfile baseURL beginUpdate bindViewpoint callBrowserCallbacks changeViewpoint " +
   "checkLoadCount concreteNodes constrainTranslation contextMenu createScene createVrmlFromString createVrmlFromURL createX3DFromString " +
   "createX3DFromURL currentFrameRate currentScene currentSpeed deleteRoute description dispose element endUpdate fieldTypes firstViewpoint " +
   "getAbstractNode getAbstractNodes getBrowserCallbacks getBrowserOption getBrowserProperty getClosestObject getComponent getConcreteNode " +
   "getConcreteNodes getDescription getFieldType getFieldTypes getName getProfile getRenderingProperty getSupportedComponent " +
   "getSupportedComponents getSupportedProfile getSupportedProfiles getVersion getWelcomeMessage getWorldURL importDocument importJS initialize " +
   "isContextLost lastViewpoint loadComponents loadURL loseContext name nextViewpoint previousViewpoint print println providerURL providerUrl " +
   "removeAbstractNode removeBrowserCallback removeBrowserListener removeConcreteNode removeSupportedComponent removeSupportedProfile replaceWorld " +
   "scriptURL setBrowserOption setDescription supportedComponents supportedProfiles toJSONStream toVRMLStream toXMLStream updateAbstractNode " +
   "updateConcreteNode updateSupportedComponent updateSupportedProfile version viewAll") .split (" "));

const BASE_NODE_OWN = new Set (("addAlias addChildObject addChildObjects addEvent addEventObject addNodeEvent addPredefinedField addType " +
   "addUserDefinedField canUserDefinedFields changeField checkLiveState convertFields dispose getBrowser getChangedFields getConvertedFields " +
   "getExecutionContext getExtendedEventHandling getField getFieldDefinition getFieldDefinitions getFields getInnerNode getLive getPredefinedField " +
   "getPredefinedFields getScene getType getUserDefinedField getUserDefinedFields initialize isDefaultValue isInitialized isLive name_changed " +
   "parentsChanged parents_changed removePredefinedField removeUserDefinedField setExecutionContext setLive setName setup typeName_changed") .split (" "));

// X3DBrowser instances own these (X3DBrowserContext binds them in its constructor).
const BROWSER_INSTANCE = new Set (["advanceTime", "getCurrentFrameRate", "getCurrentTime"]);

const NODE_FIELDS = Symbol ("nodeFields");

// An engine class of X_ITE: its objects come from the port, its constructor needs an execution context (a TypeError without one).
function namedClass (typeName)
{
   const C = { [typeName]: function () { throw new TypeError ("Cannot read properties of undefined (reading 'getBrowser')"); } } [typeName];

   Object .defineProperty (C, "typeName", { value: typeName, enumerable: true });
   return C;
}

// X3DBaseNode: the field list of a node-like object of the port (Browser, BrowserOptions …), `this [NODE_FIELDS]`: name → { field,
// definition, refresh }.
// X3DBaseNode (executionContext, browser = …) has no typeName.
const X3DBaseNodeClass = { X3DBaseNode: function (executionContext) { throw new TypeError ("Cannot read properties of undefined (reading 'getBrowser')"); } } .X3DBaseNode;

X3DBaseNodeClass .prototype = Object .create (X3DChildObject .prototype, { constructor: { value: X3DBaseNodeClass, writable: true, configurable: true } });

Object .assign (X3DBaseNodeClass .prototype,
{
   getType () { return Array .from (this [BASE_NODE_TYPES] ?? [ ]); },
   getField (name)
   {
      const entry = this [NODE_FIELDS] ?.get (String (name));

      if (!entry)
         throw unknownField (this .getTypeName (), name);

      entry .refresh ?.(entry .field);
      return entry .field;
   },
   getFieldDefinitions () { return this [NODE_DEFINITIONS] ??= new FieldDefinitionArray (Array .from (this [NODE_FIELDS] ?.values () ?? [ ], e => e .definition)); },
   getFieldDefinition (name)
   {
      const definition = this .getFieldDefinitions () .get (String (name));

      if (definition)
         return definition;

      throw unknownField (this .getTypeName (), name);
   },
   getFields () { return Array .from (this [NODE_FIELDS] ?.keys () ?? [ ], name => this .getField (name)); },
   getPredefinedFields () { return this .getFields (); },
   getUserDefinedFields () { return [ ]; },
   canUserDefinedFields () { return false; },
   // A Script has no event of the browser's own fields to add (X3DBaseNode.addEvent taints the node for the next cascade).
   addEvent (field) { },
   isInitialized () { return true; },
   getBrowser () { return Browser; },
   // The objects of the port a Script holds are set up and initialized; setup () and initialize () have nothing left to do.
   setup () { },
   initialize () { },
   isLive () { return this [CONTEXT] ? X ("cx", contextKeys .get (this) * -1 - 2, "live") : true; },
   getLive () { return new Fields .SFBool (this .isLive ()); },
   // X3DBaseNode.addChildObjects (accessType, name, field, …): `this ._name` is the field.
   addChildObjects (... args)
   {
      for (let i = 0; i + 2 < args .length; i += 3)
      {
         const field = args [i + 2];

         field .setName ?.(args [i + 1]);
         Object .defineProperty (this, "_" + args [i + 1], { get () { return field; }, set (value) { field .setValue (value); }, configurable: true });
      }
   },
});

const BASE_NODE_TYPES  = Symbol ("nodeTypes");
const NODE_DEFINITIONS = Symbol ("nodeDefinitions");

// A field of a node-like object: its definition holds the default, getField the current value (`refresh`).
function nodeField (owner, name, accessType, typeName, initial, refresh)
{
   const field = fieldFromData (typeName, initial);

   field .setName (name);
   field .setAccessType (accessType);

   owner [NODE_FIELDS] .set (name, { field, refresh, definition: new X3DFieldDefinition (accessType, name, fieldFromData (typeName, initial)) });
   return field;
}

// The data of a primitive field value.
function primitiveData (typeName, value)
{
   return typeName === "SFString" ? String (value) : value;
}

const X3DBrowserContextClass = namedClass ("X3DBrowserContext");

X3DBrowserContextClass .prototype = Object .create (X3DBaseNodeClass .prototype, { constructor: { value: X3DBrowserContextClass, writable: true, configurable: true } });

// X3DBrowserContext.getBrowserOptions / getBrowserProperties / getRenderingProperties: one X3DBaseNode each, its fields the
// browser's (definitions: the defaults, browser_options.rs; values: the current ones).
const browserNodes = new Map ();

function browserNode (set, typeName, accessType)
{
   let node = browserNodes .get (set);

   if (node)
      return node;

   const C = namedClass (typeName);

   C .prototype = Object .create (X3DBaseNodeClass .prototype, { constructor: { value: C, writable: true, configurable: true } });

   node = Object .create (C .prototype);
   node [NODE_FIELDS] = new Map ();

   for (const [name, type, initial] of X ("browserDefs", set))
   {
      const field = nodeField (node, name, accessType, type, primitiveData (type, initial), field => field .set (primitiveData (type, X ("browserField", set, name))));

      // BrowserOptions fields are inputOutput: a value a Script gives the field is the option's.
      if (set === "options")
         Object .defineProperty (field, "setValue", { value (value) { Browser .setBrowserOption (name, value); field .set (primitiveData (type, X ("browserField", set, name))); }, configurable: true, writable: true });
   }

   browserNodes .set (set, node);
   return node;
}

// The bound nodes of the active layer, or the layer's default nodes (X_ITE: the nodes at the bottom of its stacks).
const ACTIVE_TYPES = { layer: "Layer", navigationInfo: "NavigationInfo", viewpoint: "Viewpoint" };

function activeNode (kind)
{
   return nodeRef (X ("bactive", kind) ?? X ("bdefault", ACTIVE_TYPES [kind]));
}

Object .assign (X3DBrowserContextClass .prototype,
{
   getActiveLayer () { return activeNode ("layer"); },
   getActiveNavigationInfo () { return activeNode ("navigationInfo"); },
   getActiveViewpoint () { return activeNode ("viewpoint"); },
   getCurrentViewer () { return X ("bviewer"); },
   getBrowserOptions () { return browserNode ("options", "BrowserOptions", X3DConstants .inputOutput); },
   getBrowserProperties () { return browserNode ("properties", "BrowserProperties", X3DConstants .initializeOnly); },
   getRenderingProperties () { return browserNode ("rendering", "RenderingProperties", X3DConstants .outputOnly); },
   isContextLost () { return false; },
   getViewer () { return null; },
   getCanvas () { return null; },
   getElement () { return null; },
   getNotification () { return null; },
   getContextMenu () { return null; },
   getBrowserTime () { return null; },
   getBrowserTimings () { return null; },
   getLocalStorage () { throw new TypeError ("Cannot read properties of undefined (reading 'localStorage')"); },
   resize (width, height) { X ("brequest", `resize:${+width},${+height}`); },
   dispose () { X ("brequest", "dispose"); },
});

X3DBrowserClass .prototype = Object .create (X3DBrowserContextClass .prototype, { constructor: { value: X3DBrowserClass, writable: true, configurable: true } });

Object .assign (X3DBrowserClass .prototype,
{
   bindViewpoint (layerNode, viewpointNode)
   {
      const cast = (type, node) =>
      {
         const ref = node instanceof SFNode ? node .getValue () : node;

         return ref instanceof X3DNodeRef && ref .getType () .includes (X3DConstants [type]) ? ref : null;
      };

      layerNode     = cast ("X3DLayerNode", layerNode);
      viewpointNode = cast ("X3DViewpointNode", viewpointNode);

      if (!layerNode)
         throw new Error ("Browser.bindViewpoint: layerNode must be of type X3DLayerNode.");

      if (!viewpointNode)
         throw new Error ("Browser.bindViewpoint: viewpointNode must be of type X3DViewpointNode.");

      X ("brequest", `bindViewpoint:${viewpointNode .getNodeId ()}`);
   },
   toVRMLStream (generator) { generator .string += Browser .currentScene .toVRMLString (); },
   toXMLStream (generator) { generator .string += Browser .currentScene .toXMLString (); },
   toJSONStream (generator) { generator .string += Browser .currentScene .toJSONString (); },
   dispose () { X ("brequest", "dispose"); },
});

// X3DBrowser.activeLayer / activeNavigationInfo / activeViewpoint: null while the layer's default node is bound, else the SFNode.
for (const kind of ["layer", "navigationInfo", "viewpoint"])
{
   const key = "active" + kind [0] .toUpperCase () + kind .slice (1);

   Object .defineProperty (Browser, key, { get () { const id = X ("bactive", kind); return id === null ? null : nodeObject (id); }, enumerable: true, configurable: true });
}

// Each member of the Browser literal to the class that defines it.
for (const key of Reflect .ownKeys (Browser))
{
   if (BROWSER_INSTANCE .has (key))
   {
      Object .defineProperty (Browser, key, { enumerable: true });
      continue;
   }

   const descriptor = Object .getOwnPropertyDescriptor (Browser, key);
   const target     = BROWSER_OWN .has (key) ? X3DBrowserClass .prototype : BASE_NODE_OWN .has (key) ? X3DBaseNodeClass .prototype : X3DBrowserContextClass .prototype;

   // Methods are not enumerable (X_ITE: `for (const key of Object .keys (X3DBrowser .prototype)) … enumerable: false`); the
   // accessors are.
   descriptor .enumerable = "get" in descriptor || "set" in descriptor;

   delete Browser [key];
   Object .defineProperty (target, key, descriptor);
}

// X3DBaseNode.getExecutionContext: the Browser's (the world), or an execution context's own.
{
   const browserExecutionContext = X3DBaseNodeClass .prototype .getExecutionContext;

   Object .defineProperty (X3DBaseNodeClass .prototype, "getExecutionContext", {
      value: { getExecutionContext () { return this [CONTEXT] ? this [CONTEXT] .getExecutionContext () : browserExecutionContext .call (this); } } .getExecutionContext,
      writable: true, configurable: true,
   });
}

for (const C of [X3DBaseNodeClass, X3DBrowserContextClass, X3DBrowserClass])
{
   for (const key of Object .keys (C .prototype))
   {
      const descriptor = Object .getOwnPropertyDescriptor (C .prototype, key);

      if (!("get" in descriptor) && !("set" in descriptor))
         Object .defineProperty (C .prototype, key, { enumerable: false });
   }
}

Object .setPrototypeOf (Browser, X3DBrowserClass .prototype);

// The Browser's own fields: the active nodes (outputOnly SFNode; X3DBrowserContext addChildObjects).
Browser [NODE_FIELDS] = new Map ();
Browser [BASE_NODE_TYPES]  = [X3DConstants .X3DBrowser];

for (const kind of ["layer", "navigationInfo", "viewpoint"])
   nodeField (Browser, "active" + kind [0] .toUpperCase () + kind .slice (1), X3DConstants .outputOnly, "SFNode", null, field => field .set (activeNode (kind)));

// X3DTimeContext binds advanceTime to the browser; a Script has no frame to advance.
Object .defineProperty (Browser, "advanceTime", { value: function () { }, writable: true, configurable: true, enumerable: true });

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
   const comps    = infoArray ("ComponentInfoArray", () => info () .components .map (([name, level, text]) => componentObject (script, name, level, text)), c => c .name, isComponentInfo);
   const units    = infoArray ("UnitInfoArray", () => info () .units .map (([category, name, conversionFactor]) => unitObject (script, category, name, conversionFactor)), u => u .category);

   const impl = {
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
      getFieldDefinitions ()  { return new FieldDefinitionArray (); },
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
         X ("addComponent", script, String (stored .name), stored .level | 0, String (stored .level));
         touchContext (object, "_components_changed");
      },
      updateComponent (component)
      {
         if (!isComponentInfo (component))
            throw new Error (`Couldn't update value of ComponentInfoArray, value for key '${component .name}' has wrong type.`);

         sceneComponentGiven (script, component);
         X ("updateComponent", script, String (component .name), component .level | 0, String (component .level));
         touchContext (object, "_components_changed");
      },
      removeComponent (name)  { X ("removeComponent", script, String (name)); sceneComponents .get (script) ?.delete (String (name)); touchContext (object, "_components_changed"); },
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
      addNamedNode (name, n)  { X ("cx", script, "named", "add", String (name), A (n)); namedTouched = true; touchContext (object, "_namedNodes_changed"); },
      updateNamedNode (name, n) { X ("cx", script, "named", "update", String (name), A (n)); namedTouched = true; touchContext (object, "_namedNodes_changed"); },
      removeNamedNode (name)  { X ("cx", script, "named", "remove", String (name)); namedTouched = true; touchContext (object, "_namedNodes_changed"); },
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
      deleteRoute (a)
      {
         const [, b, c, d] = arguments;

         // Case: sourceNode, sourceField, destinationNode, destinationField.
         const route = arguments .length === 4 ? this .getRoute (a, b, c, d) : a;

         if (routeIds .has (route))
            X ("cx", script, "routeDelete", routeIds .get (route));
      },
      // X3DScene.dispose: the port does not tear a scene down from a Script (its nodes stay the runtime's); the object lets go of
      // its interests and user data (X3DObject.dispose).
      dispose ()              { X3DObject .prototype .dispose .call (object); },
      toVRMLStream (generator) { generator .string += this .toVRMLString (); },
      toXMLStream (generator)  { generator .string += this .toXMLString (); },
      toJSONStream (generator) { generator .string += this .toJSONString (); },
      getExecutionContext ()  { const c = X ("cx", script, "parentCtx"); return c === null ? null : executionContext (- (c + 2)); },
      getLocalScene ()        { return this .isScene ? object : executionContext (- (X ("cx", script, "sceneRoot") + 2)); },
      getOuterNode ()         { return R (X ("cx", script, "outerNode")); },
   };

   if (script === undefined)
      return impl;

   // X_ITE's X3DScene / X3DExecutionContext: the members are its classes' (contextClasses); the object owns what X_ITE's does.
   const object = Object .create (impl .isScene ? X3DSceneClass .prototype : X3DExecutionContextClass .prototype);

   Object .defineProperty (object, CONTEXT, { value: impl });
   Object .defineProperty (object, BASE_NODE_TYPES, { value: [X3DConstants .X3DExecutionContext, ... impl .isScene ? [X3DConstants .X3DScene] : [ ]] });

   // X3DBaseNode.addChildObjects: the change fields are own, not enumerable, accessors.
   for (const key of ["_sceneGraph_changed", "_bbox_changed", "_profile_changed", "_metadata_changed"])
      Object .defineProperty (object, key, { ... Object .getOwnPropertyDescriptor (impl, key), enumerable: false });

   // `this ._namedNodes_changed = …` (a DEF, add/update/removeNamedNode) and `this ._components_changed = …` (a COMPONENT
   // statement, add/update/removeComponent): plain properties, there once set.
   if (impl .namedNodes .length || namedTouched)
      touchContext (object, "_namedNodes_changed");

   if (impl .isScene && info () .components .length)
      touchContext (object, "_components_changed");

   return object;
}

const CONTEXT = Symbol ("context");

function touchContext (o, key)
{
   if (o && !Object .prototype .hasOwnProperty .call (o, key))
      Object .defineProperty (o, key, { get () { return X ("time"); }, enumerable: true, configurable: true });
}

// X_ITE's engine classes a Script can reach (Script.js globals): X3DRoute, X3DImportedNode, X3DExportedNode, X3DProtoDeclarationNode,
// X3DProtoDeclaration, X3DExternProtoDeclaration. The objects a Script gets (routeObject, importedObject, exportedObject,
// protoObject) are the port's, made from these prototypes. `new` makes what X_ITE's constructor makes without an execution context:
// a route that cannot connect (a warning) and an exported node with nothing in it; the others throw a TypeError as X_ITE's do.
function engineClass (typeName, length, parent, construct)
{
   const C = { [typeName]: function (... args)
   {
      if (!construct)
         throw new TypeError ("Cannot read properties of undefined (reading 'getBrowser')");

      construct .apply (this, args);
   } } [typeName];

   Object .defineProperty (C, "length", { value: length });
   Object .defineProperty (C, "typeName", { value: typeName, enumerable: true });

   C .prototype = Object .create (parent, { constructor: { value: C, writable: true, configurable: true } });
   return C;
}

const X3DRoute = engineClass ("X3DRoute", 5, X3DObject .prototype, function (executionContext, sourceNode, sourceField, destinationNode, destinationField)
{
   // X3DRoute.reconnect: a route whose nodes have no such fields does not connect.
   scriptConsole .warn (new AggregateError ([ ], "Couldn't connect route."));
});

Object .assign (X3DRoute,
{
   checkFields (sourceField, destinationField)
   {
      if (sourceField && destinationField)
      {
         if (sourceField .getType () !== destinationField .getType ())
            throw new Error (`Bad ROUTE specification: source field type must match destination field type of fields named "${sourceField .getName ()}" and "${destinationField .getName ()}".`);
      }

      if (sourceField)
      {
         if (!sourceField .isOutput ())
            throw new Error (`Bad ROUTE specification: source field "${sourceField .getName ()}" must be an output.`);
      }

      if (destinationField)
      {
         if (!destinationField .isInput ())
            throw new Error (`Bad ROUTE specification: destination field "${destinationField .getName ()}" must be an input.`);
      }
   },
});

Object .defineProperty (X3DRoute, "checkFields", { enumerable: false });

const X3DImportedNode = engineClass ("X3DImportedNode", 5, X3DObject .prototype);

const X3DExportedNode = engineClass ("X3DExportedNode", 4, X3DObject .prototype, function () { });

const X3DProtoDeclarationNode = engineClass ("X3DProtoDeclarationNode", 1, X3DBaseNodeClass .prototype);
const X3DProtoDeclaration = engineClass ("X3DProtoDeclaration", 1, X3DProtoDeclarationNode .prototype);
const X3DExternProtoDeclaration = engineClass ("X3DExternProtoDeclaration", 2, X3DProtoDeclarationNode .prototype);

// The members of these objects, on the class X_ITE defines them on (oracle scx-members): methods not enumerable, accessors
// enumerable. Each object's ENGINE_STATE says what the runtime knows it by.
const ENGINE_STATE = Symbol ("engine");

function engineMembers (C, members)
{
   for (const [key, member] of Object .entries (members))
   {
      if (typeof member === "function")
         Object .defineProperty (C .prototype, key, { value: member, writable: true, configurable: true, enumerable: false });
      else
         Object .defineProperty (C .prototype, key, { ... member, configurable: true, enumerable: true });
   }
}

function protoFields (state)
{
   const fields = state .info () .fields;
   const key    = JSON .stringify (fields .map (f => [f .name, f .accessType, f .dataType]));

   if (state .fieldsKey === key)
      return state .fields;

   const value = (dataType, v) => v instanceof X3DField ? v
      : dataType === "SFNode" ? new Fields .SFNode (v instanceof SFNode ? v .getValue () : null)
      : fieldFromData (dataType, v);

   state .fieldsKey = key;
   state .fields    = new FieldDefinitionArray (fields .map (f => new X3DFieldDefinition (f .accessType, f .name, value (f .dataType, R (f .value)))));
   return state .fields;
}

engineMembers (X3DRoute,
{
   getExecutionContext () { return executionContext (this [ENGINE_STATE] .script); },
   getSourceNode () { const r = this [ENGINE_STATE] .record (); return r ? R (r .sourceNode) : undefined; },
   getSourceField () { return this [ENGINE_STATE] .record () ?.sourceField; },
   getDestinationNode () { const r = this [ENGINE_STATE] .record (); return r ? R (r .destinationNode) : undefined; },
   getDestinationField () { return this [ENGINE_STATE] .record () ?.destinationField; },
   sourceNode: { get () { return this .getSourceNode (); } },
   sourceField: { get () { return this .getSourceField (); } },
   destinationNode: { get () { return this .getDestinationNode (); } },
   destinationField: { get () { return this .getDestinationField (); } },
});

// X3DRoute / X3DImportedNode to*Stream: printed outside a scene (X3DObject.toXXXString), the generator has not met the nodes
// they name, so X_ITE throws.
for (const [C, message] of [[X3DRoute, () => "Source node does not exist in scene graph."],
                            [X3DImportedNode, (stream) => `X3DImportedNode.${stream}: Inline node does not exist.`]])
{
   engineMembers (C,
   {
      toVRMLStream (generator) { throw new Error (message ("toVRMLStream")); },
      toXMLStream (generator) { throw new Error (message ("toXMLStream")); },
      toJSONStream (generator) { throw new Error (message ("toJSONStream")); },
   });
}

// The node an exported node names (X_ITE's getLocalNode hands out the node; the port's an SFNode of its X3DNodeRef).
function exportedLocalNode (exported)
{
   const local = exported .getLocalNode ();

   return local instanceof SFNode ? local .getValue () : local;
}

// X3DExportedNode.toVRMLStream … (Execution/X3DExportedNode.js).
engineMembers (X3DExportedNode,
{
   toVRMLStream (generator)
   {
      const localName = generator .Name (exportedLocalNode (this)), exportedName = this .getExportedName (), description = this .getDescription ();

      generator .Indent ();
      generator .string += "EXPORT";
      generator .Space ();
      generator .string += localName;

      if (exportedName !== localName)
      {
         generator .Space ();
         generator .string += "AS";
         generator .Space ();
         generator .string += exportedName;
      }

      if (description)
      {
         generator .Space ();
         generator .string += "DESCRIPTION";
         generator .Space ();
         generator .string += '"';
         generator .string += description;
         generator .string += '"';
      }
   },
   toXMLStream (generator)
   {
      const localName = generator .Name (exportedLocalNode (this)), exportedName = this .getExportedName (), description = this .getDescription ();

      generator .openTag ("EXPORT");
      generator .attribute ("localDEF", localName);

      if (exportedName !== localName)
         generator .attribute ("AS", exportedName);

      if (description)
         generator .attribute ("DESCRIPTION", description);

      generator .closeTag ("EXPORT");
   },
   toJSONStream (generator)
   {
      const localName = generator .Name (exportedLocalNode (this)), exportedName = this .getExportedName (), description = this .getDescription ();

      generator .TidyBreak ();
      generator .Indent ();

      generator .beginObject ("EXPORT", false, true);
      generator .stringProperty ("@localDEF", localName, false);

      if (exportedName !== localName)
         generator .stringProperty ("@AS", exportedName);

      if (description)
         generator .stringProperty ("@DESCRIPTION", description);

      generator .endObject ();
      generator .endObject ();
   },
});

engineMembers (X3DImportedNode,
{
   getExecutionContext () { return executionContext (this [ENGINE_STATE] .script); },
   getInlineNode () { return R (this [ENGINE_STATE] .record () .inlineNode); },
   getExportedName () { return this [ENGINE_STATE] .record () .exportedName; },
   getExportedNode () { const { script, id } = this [ENGINE_STATE]; return R (X ("cx", script, "importedExported", id)); },
   getImportedName () { return this [ENGINE_STATE] .record () .importedName; },
   getDescription () { return this [ENGINE_STATE] .record () .description; },
   setDescription (value) { const { script, id } = this [ENGINE_STATE]; X ("cx", script, "importedDescription", id, String (value)); },
   inlineNode: { get () { return this .getInlineNode (); } },
   exportedName: { get () { return this .getExportedName (); } },
   exportedNode: { get () { return this .getExportedNode (); } },
   importedName: { get () { return this .getImportedName (); } },
   description: { get () { return this .getDescription (); }, set (value) { this .setDescription (value); } },
});

engineMembers (X3DExportedNode,
{
   getExecutionContext () { return executionContext (this [ENGINE_STATE] .script); },
   getExportedName () { return this [ENGINE_STATE] ?.record () .exportedName; },
   getLocalNode () { return this [ENGINE_STATE] ? R (this [ENGINE_STATE] .record () .localNode) : undefined; },
   getDescription () { return this [ENGINE_STATE] ?.record () .description; },
   setDescription (value) { const s = this [ENGINE_STATE]; if (s) X ("cx", s .script, "exportedDescription", s .id, String (value)); },
   exportedName: { get () { return this .getExportedName (); } },
   localNode: { get () { return this .getLocalNode (); } },
   description: { get () { return this .getDescription (); }, set (value) { this .setDescription (value); } },
});

engineMembers (X3DProtoDeclarationNode,
{
   getAppInfo () { return this [ENGINE_STATE] .appInfo; },
   setAppInfo (value) { this [ENGINE_STATE] .appInfo = String (value); },
   getDocumentation () { return this [ENGINE_STATE] .documentation; },
   setDocumentation (value) { this [ENGINE_STATE] .documentation = String (value); },
   name: { get () { return this [ENGINE_STATE] .info () .name; } },
   // X3DProtoDeclarationNode.fields: the interface as a FieldDefinitionArray (one array while the interface stays the same).
   fields: { get () { return protoFields (this [ENGINE_STATE]); } },
   appInfo: { get () { return this .getAppInfo (); }, set (value) { this .setAppInfo (value); } },
   documentation: { get () { return this .getDocumentation (); }, set (value) { this .setDocumentation (value); } },
});

Object .defineProperty (X3DProtoDeclaration .prototype, "isExternProto", { value: false, writable: true, configurable: true, enumerable: true });
Object .defineProperty (X3DExternProtoDeclaration .prototype, "isExternProto", { value: true, writable: true, configurable: true, enumerable: true });

// X3DExternProtoDeclaration.getAppInfo … read and write the loaded PROTO's, else the declaration's own. The port keeps the
// declaration's own only (the loaded PROTO's appinfo and documentation are not passed to Scripts).
// The declarations print through the runtime's printer (Scene::proto_declaration_string), in X3DObject.toXXXString's frame.
for (const [C, extern] of [[X3DProtoDeclaration, false], [X3DExternProtoDeclaration, true]])
{
   engineMembers (C,
   {
      toVRMLStream (generator) { generator .string += X ("cx", -1, "protoString", extern, this [ENGINE_STATE] .id, "vrml"); },
      toXMLStream (generator) { generator .string += X ("cx", -1, "protoString", extern, this [ENGINE_STATE] .id, "xml"); },
      toJSONStream (generator) { generator .string += X ("cx", -1, "protoString", extern, this [ENGINE_STATE] .id, "json"); },
   });
}

engineMembers (X3DProtoDeclaration, { getProtoDeclaration () { return this; } });

engineMembers (X3DExternProtoDeclaration,
{
   // X3DExternProtoDeclaration.getProtoDeclaration: the PROTO of the loaded scene, null before.
   getProtoDeclaration () { const p = this [ENGINE_STATE] .info () .loaded; return p === undefined ? null : protoObject (false, p); },
   getAppInfo () { return X3DProtoDeclarationNode .prototype .getAppInfo .call (this); },
   setAppInfo (value) { X3DProtoDeclarationNode .prototype .setAppInfo .call (this, value); },
   getDocumentation () { return X3DProtoDeclarationNode .prototype .getDocumentation .call (this); },
   setDocumentation (value) { X3DProtoDeclarationNode .prototype .setDocumentation .call (this, value); },
   urls: { get () { return R (this [ENGINE_STATE] .info () .urls); } },
   loadState: { get () { return X3DConstants [this [ENGINE_STATE] .info () .loadState]; } },
});

for (const C of [X3DProtoDeclaration, X3DExternProtoDeclaration])
{
   Object .defineProperty (C, "fieldDefinitions", {
      value: new FieldDefinitionArray ([new X3DFieldDefinition (X3DConstants .inputOutput, "metadata", new Fields .SFNode ())]),
      enumerable: true,
   });
}

// X3DBrowser.fieldDefinitions: the browser's (X3DBaseNode.getFieldDefinitions reads the class's).
Object .defineProperty (X3DBrowserClass, "fieldDefinitions", { get () { return Browser .getFieldDefinitions (); }, enumerable: true, configurable: true });

// X3DExecutionContext and X3DScene (X3DScene → X3DExecutionContext → X3DBaseNode → X3DChildObject → X3DObject): each member a
// Script can use sits on the class X_ITE defines it on (oracle scx-api: the own names of each level of the pin's prototypes);
// it runs on the context's state. Members X_ITE's classes have that the port does not (engine internals: addLoadingObject,
// getBBox, setEncoding …) are absent.
const X3DExecutionContextClass = namedClass ("X3DExecutionContext");
const X3DSceneClass            = namedClass ("X3DScene");

Object .defineProperty (X3DExecutionContextClass, "length", { value: 1 });
Object .defineProperty (X3DSceneClass, "length", { value: 1 });

X3DExecutionContextClass .prototype = Object .create (X3DBaseNodeClass .prototype, { constructor: { value: X3DExecutionContextClass, writable: true, configurable: true } });
X3DSceneClass .prototype = Object .create (X3DExecutionContextClass .prototype, { constructor: { value: X3DSceneClass, writable: true, configurable: true } });

{
   const EXECUTION_CONTEXT_OWN = ("addExternProtoDeclaration addImportedNode addNamedNode addProtoDeclaration addRoute baseURL components " +
      "createNode createProto deleteRoute encoding externprotos fromUnit getBaseURL getComponents getEncoding getExternProtoDeclaration " +
      "getExternProtoDeclarations getImportedNode getImportedNodes getLocalNode getLocalScene getNamedNode getNamedNodes getNodeOrImportedNode " +
      "getOuterNode getProfile getProtoDeclaration getProtoDeclarations getRootNodes getRoute getRoutes getSpecificationVersion " +
      "getUniqueExternProtoName getUniqueImportName getUniqueName getUniqueProtoName getUnits getWorldInfos getWorldURL hasComponent importedNodes " +
      "namedNodes profile protos removeExternProtoDeclaration removeImportedNode removeNamedNode removeProtoDeclaration renameImportedNode " +
      "rootNodes routes setRootNodes specificationVersion toJSONStream toUnit toVRMLStream toXMLStream units updateExternProtoDeclaration dispose " +
      "updateImportedNode updateNamedNode updateProtoDeclaration worldURL") .split (" ");
   const SCENE_OWN = ("addComponent addExportedNode addMetaData addRootNode baseURL bbox_changed components encoding exportedNodes fromUnit " +
      "getBaseURL getComponents getEncoding getExportedNode getExportedNodes getLocalScene getMetaData getMetaDatas getProfile " +
      "getSpecificationVersion getUniqueExportName getUnit getUnits getWorldURL hasComponent metadata_changed profile profile_changed " +
      "removeComponent removeExportedNode removeMetaData removeRootNode rootNodes sceneGraph_changed setMetaData setProfile setRootNodes " +
      "specificationVersion toJSONStream toUnit toVRMLStream toXMLStream units updateComponent updateExportedNode updateUnit worldURL dispose") .split (" ");

   const template = makeExecutionContext (undefined);

   const install = (target, key) =>
   {
      const d = Object .getOwnPropertyDescriptor (template, key);

      if (!d)
         throw new Error (`X3DExecutionContext: no ${key}`);

      if ("value" in d)
      {
         const f = { [key] (... args) { return this [CONTEXT] [key] (... args); } } [key];

         Object .defineProperty (f, "length", { value: d .value .length });
         Object .defineProperty (target, key, { value: f, writable: true, configurable: true, enumerable: false });
      }
      else
      {
         // The field accessors are not enumerable (X3DBaseNode.addChildObjects), the others are.
         const field = key .endsWith ("_changed");

         Object .defineProperty (target, key, {
            get: d .get && { [key] () { return this [CONTEXT] [key]; } } [key],
            set: d .set && { [key] (value) { this [CONTEXT] [key] = value; } } [key],
            enumerable: !field, configurable: true,
         });
      }
   };

   for (const key of EXECUTION_CONTEXT_OWN) install (X3DExecutionContextClass .prototype, key);
   for (const key of SCENE_OWN) install (X3DSceneClass .prototype, key);

   Object .defineProperty (X3DExecutionContextClass .prototype, "isScene", { value: false, writable: true, configurable: true, enumerable: true });
   Object .defineProperty (X3DSceneClass .prototype, "isScene", { value: true, writable: true, configurable: true, enumerable: true });
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
      o = Object .create (X3DRoute .prototype);
      Object .defineProperty (o, ENGINE_STATE, { value: { script, id, record: () => X ("cx", script, "routeList") .find (r => r .id === id) } });
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
      o = Object .create (X3DImportedNode .prototype);
      Object .defineProperty (o, ENGINE_STATE, { value: { script, id, record: () => X ("nodeArrays", script, "imported") .find (m => m .id === id) } });
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
      o = Object .create (X3DExportedNode .prototype);
      Object .defineProperty (o, ENGINE_STATE, { value: { script, id, record: () => X ("nodeArrays", script, "exported") .find (m => m .id === id) } });
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
      o = Object .create ((extern ? X3DExternProtoDeclaration : X3DProtoDeclaration) .prototype);
      Object .defineProperty (o, ENGINE_STATE, { value: { extern, id, info: () => X ("cx", -1, "protoInfo", extern, id), appInfo: "", documentation: "" } });

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
   // The page's clearTimeout and clearInterval take their id as an optional argument (length 0).
   clearTimeout (id = undefined)      { timerList = timerList .filter (t => t .id !== id); },
   clearInterval (id = undefined)     { timerList = timerList .filter (t => t .id !== id); },
};

// The page's Date.now is the page's: the browser time is Date.now () there already.
if (!PAGE)
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
   // Script.js: `browser .println .bind (browser)`.
   print: Browser .println .bind (Browser), trace: Browser .println .bind (Browser),
   Browser,
   X3DConstants,
   X3DField, X3DArrayField,
   X3DBrowser: X3DBrowserClass, X3DExecutionContext: X3DExecutionContextClass, X3DScene: X3DSceneClass,
   ComponentInfo, ComponentInfoArray, ProfileInfo, ProfileInfoArray, ConcreteNodesArray, AbstractNodesArray, UnitInfo, UnitInfoArray,
   NamedNodesArray, ImportedNodesArray, X3DImportedNode, ExportedNodesArray, X3DExportedNode, ExternProtoDeclarationArray,
   ProtoDeclarationArray, X3DExternProtoDeclaration, X3DProtoDeclaration, X3DProtoDeclarationNode, RouteArray, X3DRoute,
   X3DBaseNode: X3DBaseNodeClass,
   X3DFieldDefinition, FieldDefinitionArray,
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

// release.rs asks which of the scenes `ids` (root contexts of createX3DFromURL / createVrmlFromURL scenes none of whose
// nodes anything else keeps) a Script still reaches through their X3DScene object: as for nodes, the objects are unpinned,
// the host collects garbage, and the objects that are left are held (X_ITE: the garbage collector frees the others).
global .__unpinScenes = function (ids)
{
   for (const id of ids)
      contextObjects .delete (id);
};

global .__heldScenes = function (ids)
{
   const held = [ ];

   for (const id of ids)
   {
      const object = contextRefs .get (id) ?.deref ();

      if (object === undefined)
      {
         contextRefs .delete (id);
         continue;
      }

      contextObjects .set (id, object);
      held .push (id);
   }

   return held;
};

// The scenes were released and their context ids may be handed out again: their objects and change-field callbacks go.
global .__releaseScenes = function (ids)
{
   for (const id of ids)
   {
      contextObjects .delete (id);
      contextRefs .delete (id);
      sceneComponents .delete (- (id + 2));
      profileValues .delete (- (id + 2));

      for (let index = 0; index < 4; ++ index)
      {
         ctxFieldCallbacks .delete (id * 4 + index);
         ctxFieldObjects .delete (id * 4 + index);
      }
   }
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
